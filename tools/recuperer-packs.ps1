<#
    Recupere des packs de personnages depuis le catalogue shimejis.xyz.

    Responsabilite unique : transformer un slug du catalogue en un dossier
    `characters/<slug>/` complet et directement utilisable par l'application
    (spec 8.1). Le script ne touche a rien d'autre.

    --------------------------------------------------------------------
    POURQUOI CE SCRIPT EXISTE

    L'extension Chrome « Shimeji Browser Extension » ne contient AUCUN
    sprite : elle les tire du CDN `sprites.shimejis.xyz`, qui sert les frames
    individuelles deja en 128x128 et deja nommees `shimeN.png`. C'est donc un
    telechargement, pas une extraction. Voir le CLAUDE.md, section
    « Ajouter un pack depuis shimejis.xyz ».

    --------------------------------------------------------------------
    CE QU'IL FAIT EN PLUS DE TELECHARGER, ET POURQUOI

    1. Il MESURE l'ancre et la hitbox au lieu de recopier celles de `blob`.
       La numerotation des poses est un standard de fait et se transpose ;
       les proportions du dessin, NON. Luffy est un chibi dont le chapeau
       touche le bord haut de la boite : le `y = 20` de la hitbox de `blob`
       l'aurait ampute. C'est la lecon de l'etape 1a, ou les quatre reglages
       faits a l'oeil se sont tous reveles faux.

    2. Il n'ecrit QUE les poses dont toutes les frames existent. Un pack
       incomplet n'est pas une erreur : la spec 8.7 prevoit qu'un personnage
       sans images d'escalade ne grimpe jamais, l'option etant simplement
       retiree du tirage d'envies. Aucun cas particulier a coder — il suffit
       de ne pas declarer la pose.

    --------------------------------------------------------------------
    EXEMPLES

        # La liste des slugs, sans rien telecharger
        .\tools\recuperer-packs.ps1 -ListeSeule

        # Un seul personnage, pour verifier que tout marche
        .\tools\recuperer-packs.ps1 -Slug one-piece-zoro-01

        # Les 427 packs structures (hors depots communautaires a hash)
        .\tools\recuperer-packs.ps1 -Curated

        # Tout le catalogue (~2000 packs, ~1,5 Go)
        .\tools\recuperer-packs.ps1 -Tous
#>

[CmdletBinding()]
param(
    # Les slugs a recuperer, explicitement. Prioritaire sur -Tous / -Curated.
    [string[]] $Slug,

    # Tout le catalogue.
    [switch] $Tous,

    # Seulement les packs structures : on ecarte les slugs finissant par un
    # hash hexadecimal de 6 caracteres, qui sont les depots communautaires
    # (souvent des doublons ou des versions incompletes).
    [switch] $Curated,

    # Affiche la liste et s'arrete. Sert a choisir avant de telecharger.
    [switch] $ListeSeule,

    # Engendre ui/catalogue.json et s'arrete, sans rien telecharger d'autre
    # que la page du repertoire. C'est un geste de MAINTENANCE, joue a la
    # main pour rafraichir le catalogue, jamais un chemin d'execution : si le
    # markup de shimejis.xyz change, c'est ce script qui casse, sur la
    # machine du developpeur, avec un message — pas le catalogue chez
    # l'utilisateur.
    [switch] $Index,

    # Plafond, pour tester sur un echantillon.
    [int] $Limite = 0,

    [string] $Destination = "characters",

    # Politesse envers le CDN : une pause entre chaque image. A 60 ms et
    # 46 images, un pack coute ~3 s. Ne pas descendre a 0 sur un gros lot.
    [int] $DelaiMs = 60,

    # Re-telecharge un pack deja present. Par defaut on le saute, ce qui rend
    # le script REPRENABLE : on peut l'interrompre et le relancer.
    [switch] $Force
)

$ErrorActionPreference = 'Stop'
# Sans ca, Invoke-WebRequest affiche une barre de progression qui coute plus
# cher que le telechargement lui-meme.
$ProgressPreference = 'SilentlyContinue'

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Net.Http

# ── Le mesureur de silhouette ───────────────────────────────────────────
# En C# et non en PowerShell : `GetPixel` appele 16384 fois par image mettrait
# plus d'une seconde, soit ~50 minutes pour 2000 packs. `LockBits` + une copie
# memoire fait la meme chose en quelques millisecondes.
if (-not ("MesureSprite" -as [type])) {
Add-Type -ReferencedAssemblies System.Drawing @"
using System;
using System.IO;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public class MesureSprite {
    // Rend { minX, minY, maxX, maxY } des pixels non transparents,
    // ou { -1, -1, -1, -1 } si l'image est entierement vide.
    public static int[] BoiteOpaque(string chemin) {
        using (Bitmap bmp = new Bitmap(chemin)) {
            Rectangle r = new Rectangle(0, 0, bmp.Width, bmp.Height);
            BitmapData d = bmp.LockBits(r, ImageLockMode.ReadOnly, PixelFormat.Format32bppArgb);
            int stride = d.Stride;
            byte[] buf = new byte[stride * bmp.Height];
            Marshal.Copy(d.Scan0, buf, 0, buf.Length);
            bmp.UnlockBits(d);

            int minX = int.MaxValue, maxX = -1, minY = int.MaxValue, maxY = -1;
            for (int y = 0; y < bmp.Height; y++) {
                for (int x = 0; x < bmp.Width; x++) {
                    // Format32bppArgb est range en BGRA : l'alpha est le 4e octet.
                    // Le seuil de 16 ignore l'antialiasing quasi invisible.
                    if (buf[y * stride + x * 4 + 3] > 16) {
                        if (x < minX) minX = x;
                        if (x > maxX) maxX = x;
                        if (y < minY) minY = y;
                        if (y > maxY) maxY = y;
                    }
                }
            }
            if (maxX < 0) return new int[] { -1, -1, -1, -1 };
            return new int[] { minX, minY, maxX, maxY };
        }
    }

    public static int[] Dimensions(string chemin) {
        using (Bitmap bmp = new Bitmap(chemin)) {
            return new int[] { bmp.Width, bmp.Height };
        }
    }

    // Recadre une frame sur une toile de W x H en la collant a (dx, dy).
    //
    // C'est l'operation qui rend utilisables les packs dont les frames n'ont
    // pas toutes la meme taille. En collant chaque frame de facon que SON
    // ancre tombe au meme point de la toile, on obtient un jeu d'images
    // homogene ou UNE seule ancre vaut pour toutes. Rien n'est deforme ni
    // redimensionne : on ajoute du vide autour.
    public static void Recadrer(string chemin, int W, int H, int dx, int dy) {
        byte[] brut = File.ReadAllBytes(chemin);
        using (MemoryStream ms = new MemoryStream(brut))
        using (Bitmap src = new Bitmap(ms))
        using (Bitmap dst = new Bitmap(W, H, PixelFormat.Format32bppArgb))
        using (Graphics g = Graphics.FromImage(dst)) {
            // NearestNeighbor et PixelOffsetMode.Half : c'est du pixel-art,
            // toute interpolation le rendrait flou. Ici on ne fait que
            // translater, mais autant l'interdire explicitement.
            g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.NearestNeighbor;
            g.PixelOffsetMode = System.Drawing.Drawing2D.PixelOffsetMode.Half;
            g.Clear(Color.Transparent);
            g.DrawImage(src, dx, dy, src.Width, src.Height);
            dst.Save(chemin, ImageFormat.Png);
        }
    }
}
"@
}

# ── La table de reference des poses ─────────────────────────────────────
# Tiree de `conf/actions.xml` de Shimeji-ee, PAS devinee. Justification pose
# par pose dans docs/specs/2026-09-09-frames-shimeji.md. C'est la meme table
# que characters/blob/mascot.json — si l'un change, changer l'autre.
#
# `[ordered]` : une hashtable PowerShell ordinaire ne garde pas l'ordre
# d'insertion, et le manifeste genere serait melange a chaque execution.
$POSES = [ordered]@{
    'stand'         = @{ frames = @(1) }
    'walk'          = @{ frames = @(1,2,1,3); frameMs = 240; loop = $true }
    'run'           = @{ frames = @(1,2,1,3); frameMs = 80;  loop = $true }
    'sit'           = @{ frames = @(11) }
    'sleep'         = @{ frames = @(21) }
    'fall'          = @{ frames = @(4) }
    'land'          = @{ frames = @(18,19); frameMs = 160 }

    'dragged'       = @{ frames = @(1);  anchor = @(64,8) }
    'draggedRight1' = @{ frames = @(5);  anchor = @(64,8) }
    'draggedRight2' = @{ frames = @(7);  anchor = @(64,8) }
    'draggedRight3' = @{ frames = @(9);  anchor = @(64,8) }
    'draggedLeft1'  = @{ frames = @(6);  anchor = @(64,8) }
    'draggedLeft2'  = @{ frames = @(8);  anchor = @(64,8) }
    'draggedLeft3'  = @{ frames = @(10); anchor = @(64,8) }

    'sprawl'        = @{ frames = @(21) }
    'tripping'      = @{ frames = @(19,18,20,20,19); frameMs = 160 }
    'creep'         = @{ frames = @(20,20,21,21,21); frameMs = 160; loop = $true }
    'sitDangle'     = @{ frames = @(31,32,31,33); frameMs = 400; loop = $true; anchor = @(64,112) }
    'sitLegsUp'     = @{ frames = @(30); anchor = @(64,112) }
    'sitLookUp'     = @{ frames = @(26) }
    'spinHead'      = @{ frames = @(26,15,27,16,28,17,29,11); frameMs = 200 }
    'jump'          = @{ frames = @(22) }

    'grabWall'      = @{ frames = @(13) }
    'climbWall'     = @{ frames = @(14,12,13); frameMs = 160; loop = $true }
    'grabCeiling'   = @{ frames = @(23); anchor = @(64,48) }
    'climbCeiling'  = @{ frames = @(23,24,25); frameMs = 160; loop = $true; anchor = @(64,48) }

    'split'         = @{ frames = @(42,43,44,45,46); frameMs = 160 }
}

# Les poses SANS lesquelles un personnage ne peut pas vivre. Un pack qui n'a
# pas de quoi tenir debout et marcher n'est pas installable : mieux vaut le
# refuser bruyamment que livrer un dossier qui fera echouer le chargement.
$POSES_VITALES = @('stand', 'walk')

$CDN = "https://sprites.shimejis.xyz/directory"
$CATALOGUE = "https://shimejis.xyz/directory"

$http = New-Object System.Net.Http.HttpClient
$http.Timeout = [TimeSpan]::FromSeconds(30)
$http.DefaultRequestHeaders.Add("User-Agent", "shimeji-desktop/0.1 (script de recuperation de packs)")

# --- Recuperer une URL en octets, ou $null si absente ---------------------
function Get-Octets($url) {
    try {
        $reponse = $http.GetAsync($url).GetAwaiter().GetResult()
        if (-not $reponse.IsSuccessStatusCode) { return $null }
        return $reponse.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult()
    } catch {
        return $null
    }
}

# --- La taille d'un PNG, lue dans son en-tete -----------------------------
# On lit les 24 premiers octets au lieu d'ouvrir un Bitmap : verifier les
# 46 frames de 2000 packs ferait 95 000 decodages complets pour deux entiers.
# Dans un PNG, IHDR place la largeur a l'offset 16 et la hauteur a 20, en
# gros-boutiste — d'ou le [Array]::Reverse, x86 etant petit-boutiste.
function Get-DimensionsPng($chemin) {
    $fs = [System.IO.File]::OpenRead($chemin)
    try {
        $tete = New-Object byte[] 24
        $lus = $fs.Read($tete, 0, 24)
        if ($lus -lt 24) { return @(0, 0) }
        $l = $tete[16..19]; [Array]::Reverse($l)
        $h = $tete[20..23]; [Array]::Reverse($h)
        return @([BitConverter]::ToInt32($l, 0), [BitConverter]::ToInt32($h, 0))
    } finally {
        $fs.Dispose()
    }
}

# --- La liste des slugs du catalogue --------------------------------------
# Le HTML est mis en cache 24 h : le retelecharger a chaque lancement serait
# inutile, et le script doit pouvoir tourner plusieurs fois sans marteler le
# site.
# Le HTML du repertoire, telecharge au plus une fois par 24 h.
#
# Sorti de Get-Slugs pour que le mode -Index s'en serve aussi : il a besoin
# du markup entier, pas seulement des slugs, puisque les franchises sont des
# titres de section.
function Get-CatalogueHtml {
    $cache = Join-Path $env:TEMP "shimejis-directory.html"
    $frais = $false
    if (Test-Path $cache) {
        $age = (Get-Date) - (Get-Item $cache).LastWriteTime
        if ($age.TotalHours -lt 24) { $frais = $true }
    }
    if (-not $frais) {
        Write-Host "catalogue : telechargement de $CATALOGUE"
        $octets = Get-Octets $CATALOGUE
        if ($null -eq $octets) { throw "catalogue injoignable : $CATALOGUE" }
        [System.IO.File]::WriteAllBytes($cache, $octets)
    } else {
        Write-Host "catalogue : cache local (moins de 24 h)"
    }

    return [System.IO.File]::ReadAllText($cache)
}

function Get-Slugs {
    $html = Get-CatalogueHtml
    # Chaque personnage apparait dans une vignette pointant sur sa frame 1.
    $trouves = [regex]::Matches($html, 'directory/([a-z0-9-]+)/img/shime1\.png')
    $liste = New-Object System.Collections.Generic.HashSet[string]
    foreach ($m in $trouves) { [void]$liste.Add($m.Groups[1].Value) }
    return ($liste | Sort-Object)
}

# --- Le mode -Index : engendrer ui/catalogue.json -------------------------
#
# Le plan annoncait la franchise comme une « incertitude assumee », avec un
# repli mettant tout le monde dans « Catalogue » ou « Communaute ». Verifie
# le 2026-09-14 : les franchises SONT dans le markup, en titres de section
# (<h2 class="_cardTitle_...">), chacun suivi de la grille de ses vignettes.
# Le decoupage classe 2353 slugs sur 2353, sans un seul orphelin. Le repli
# est donc abandonne au profit des vrais noms.
function New-Index {
    # HttpUtility n'est pas chargee par defaut en PowerShell 5.1 : sans ce
    # Add-Type, l'appel echoue sur « type introuvable ».
    Add-Type -AssemblyName System.Web

    $html = Get-CatalogueHtml

    # On decoupe sur les titres de carte. -split avec un groupe capturant
    # rend une liste alternee : [0] = avant le 1er titre, puis (titre, corps)
    # a l'infini. D'ou le pas de 2 a partir de l'indice 1.
    $morceaux = [regex]::Split($html, '<h2[^>]*_cardTitle_[^>]*>(.*?)</h2>')

    $packs = New-Object System.Collections.Generic.List[object]
    $vus = New-Object System.Collections.Generic.HashSet[string]

    for ($i = 1; $i -lt $morceaux.Count - 1; $i += 2) {
        # Le titre peut contenir du balisage et des entites HTML
        # (« Assassin&#x27;s Creed ») : on retire l'un et on decode l'autre.
        $franchise = [System.Web.HttpUtility]::HtmlDecode(
            ($morceaux[$i] -replace '<[^>]+>', '')
        ).Trim()

        foreach ($m in [regex]::Matches($morceaux[$i + 1],
                        'directory/([a-z0-9_-]+)/img/shime1\.png')) {
            $slug = $m.Groups[1].Value
            # Un meme pack peut apparaitre dans deux sections : le premier
            # titre rencontre gagne, et le doublon est ignore.
            if (-not $vus.Add($slug)) { continue }

            # Le suffixe hexadecimal de 6 caracteres marque un depot
            # communautaire ; on le retire du nom affiche, jamais du slug.
            if ($slug -match '^(.*)-([0-9a-f]{6})$') {
                $base = $Matches[1]
            } else {
                $base = $slug
            }
            $nom = (Get-Culture).TextInfo.ToTitleCase(($base -replace '-', ' '))

            $packs.Add([pscustomobject]@{
                slug      = $slug
                nom       = $nom
                franchise = $franchise
            })
        }
    }

    Write-Host "$($packs.Count) packs, $((($packs | Select-Object -ExpandProperty franchise) | Sort-Object -Unique).Count) franchises"

    # .ToArray() et NON @($packs) : en PowerShell 5.1, envelopper une
    # List[object] dans @() a l'interieur d'un [pscustomobject]@{...} leve
    # « Les types des arguments ne correspondent pas » — une
    # ArgumentException opaque qui designe la ligne du cast, pas la valeur
    # fautive. .ToArray() rend un Object[] franc, et le cast passe.
    $doc = [pscustomobject]@{
        genere_le = (Get-Date -Format 'yyyy-MM-dd')
        source    = $CATALOGUE
        packs     = $packs.ToArray()
    }

    # -Depth : sans lui, ConvertTo-Json aplatit les objets imbriques a partir
    # du niveau 2 et ecrit « System.Object[] ».
    $json = $doc | ConvertTo-Json -Depth 4

    # SANS BOM, imperativement : serde_json le refuse avec le message
    # trompeur « expected value at line 1 column 1 ». UTF8Encoding($false)
    # est la seule facon fiable en PS 5.1 — Out-File -Encoding utf8 ecrit un
    # BOM.
    $sortie = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\ui\catalogue.json'))
    [System.IO.File]::WriteAllText(
        $sortie, $json, (New-Object System.Text.UTF8Encoding($false))
    )
    Write-Host "ecrit : $sortie"
}

# --- Les ancres declarees par le pack lui-meme ----------------------------
# Chaque pack expose son `actions.xml` sur le CDN, et c'est LA source de
# verite pour l'ancre de chaque frame. On y lit deux attributs de position,
# rien d'autre : cela ne revient pas sur la decision du CLAUDE.md de ne pas
# implementer les actions.xml de Shimeji, qui vise leur SEMANTIQUE de
# comportement (sequences imbriquees, conditions, expressions Java).
#
# Deux schemas coexistent : l'original japonais de Group-Finity et la
# traduction anglaise de Shimeji-ee. Les memes attributs, deux noms.
#   画像       = Image        (le fichier)
#   基準座標   = ImageAnchor  (le point d'ancrage, en pixels dans l'image)
#
# Rend une table « numero de frame -> @(x, y) », ou $null si le fichier est
# absent : l'appelant sait alors qu'il doit se rabattre sur la convention.
function Get-AncresDuPack($slug) {
    $octets = Get-Octets "$CDN/$slug/actions.xml"
    if ($null -eq $octets -or $octets.Length -lt 50) { return $null }

    $xml = [System.Text.Encoding]::UTF8.GetString($octets)
    # Les fichiers de Group-Finity portent un BOM UTF-8, qui devient un
    # caractere U+FEFF une fois decode et ferait echouer une comparaison.
    $xml = $xml.TrimStart([char]0xFEFF)

    # Les noms d'attributs japonais sont construits par POINTS DE CODE et
    # non ecrits en clair. Raison : PowerShell 5.1 lit un .ps1 depourvu de
    # BOM comme de l'ANSI, et les ideogrammes deviennent alors des octets
    # parasites AU MILIEU D'UNE REGEX — erreur de syntaxe, pas simple
    # affichage abime. Le fichier porte un BOM, mais un editeur peut le
    # retirer sans prevenir ; ecrit ainsi, le script y survit.
    $poseJa = -join ([char]0x30DD, [char]0x30FC, [char]0x30BA)   # ポーズ  = Pose
    $imgJa  = -join ([char]0x753B, [char]0x50CF)                 # 画像    = Image
    $ancJa  = -join ([char]0x57FA, [char]0x6E96, [char]0x5EA7, [char]0x6A19)  # 基準座標 = ImageAnchor

    $ancres = @{}
    # On isole chaque element de pose, puis on y cherche les deux attributs :
    # leur ORDRE varie d'un pack a l'autre, donc une regex unique les
    # capturant tous les deux d'un coup raterait la moitie des fichiers.
    foreach ($el in [regex]::Matches($xml, "<(?:$poseJa|Pose)\s[^>]*/?>")) {
        $bloc = $el.Value
        $mImg = [regex]::Match($bloc, "(?:$imgJa|Image)=`"([^`"]*)`"")
        $mAnc = [regex]::Match($bloc, "(?:$ancJa|ImageAnchor)=`"(\d+)\s*,\s*(\d+)`"")
        if (-not $mImg.Success -or -not $mAnc.Success) { continue }

        $mNum = [regex]::Match($mImg.Groups[1].Value, 'shime(\d+)\.png$')
        if (-not $mNum.Success) { continue }

        $n = [int]$mNum.Groups[1].Value
        $ancres[$n] = @([int]$mAnc.Groups[1].Value, [int]$mAnc.Groups[2].Value)
    }

    if ($ancres.Count -eq 0) { return $null }
    return $ancres
}

# --- Union-find : la racine de la composante d'une frame ------------------
# `$parent` est une hashtable, donc un type par REFERENCE en PowerShell : la
# compression de chemin faite ici est bien vue par l'appelant.
function Get-Racine($parent, $x) {
    while ($parent[$x] -ne $x) {
        $parent[$x] = $parent[$parent[$x]]   # compression de chemin
        $x = $parent[$x]
    }
    return $x
}

# --- Quelles poses ce pack peut-il jouer ? --------------------------------
# Une pose n'est retenue que si TOUTES ses frames existent. Declarer une pose
# a laquelle il manque une image donnerait un trou visible dans l'animation ;
# ne pas la declarer la retire proprement du tirage d'envies (spec 8.7,
# « couverture partielle »).
function Get-PosesRetenues($presentes) {
    $retenues = [ordered]@{}
    $ecartees = @()
    foreach ($nom in $POSES.Keys) {
        $def = $POSES[$nom]
        $completes = $true
        foreach ($f in $def.frames) {
            if (-not $presentes.Contains($f)) { $completes = $false; break }
        }
        if ($completes) { $retenues[$nom] = $def } else { $ecartees += $nom }
    }
    return @{ retenues = $retenues; ecartees = $ecartees }
}

# --- Construire le manifeste d'un pack ------------------------------------
# Appelee APRES recadrage. `$ancresParPose` donne, pour chaque pose retenue,
# le point d'ancrage dans la toile commune de $W x $H.
function New-Manifeste($slug, $presentes, $boite, $W, $H, $retenues, $ecartees, $ancresParPose, $origineAncres, $avert) {

    # --- La hitbox, mesuree sur la frame recadree -------------------------
    # Une colonne de 48 px de large — la largeur de `blob`, validee a
    # l'usage — mais CENTREE sur la silhouette reelle et remontee jusqu'au
    # sommet du dessin. C'est ce dernier point qui compte : un personnage
    # dont la tete touche le bord haut de la boite verrait sa tete amputee
    # par un `y` fixe. C'est le cas de Luffy.
    $minX = $boite[0]; $minY = $boite[1]; $maxX = $boite[2]
    $centre = [int](($minX + $maxX) / 2)
    $lh = [Math]::Min(48, $W)
    $hx = [Math]::Max(0, [Math]::Min($W - $lh, $centre - [int]($lh / 2)))
    $hy = $minY
    $hh = $H - $minY

    # --- Le JSON, ecrit a la main -----------------------------------------
    # `ConvertTo-Json` de PowerShell 5.1 met un element de tableau par ligne
    # et ne garde pas l'ordre des cles : le manifeste serait illisible. On
    # veut exactement la mise en forme de characters/blob/mascot.json.
    $sb = New-Object System.Text.StringBuilder
    [void]$sb.AppendLine("{")

    $jour = (Get-Date).ToString("yyyy-MM-dd")
    $src = "Genere par tools/recuperer-packs.ps1 le $jour. Sprites tires du catalogue shimejis.xyz, slug '$slug' (CDN $CDN/$slug/img/shimeN.png). L'art n'est PAS de nous : voir l'avertissement du CLAUDE.md avant toute publication du depot."
    [void]$sb.AppendLine("  ""_source"": ""$src"",")
    [void]$sb.AppendLine("")

    $mes = "Ancres : $origineAncres. Les frames d'une MEME pose sont recadrees alignees sur leur ancre, ce qui permet de n'en declarer qu'une par pose ; deux poses distinctes gardent en revanche des ancres differentes. Forcer une ancre unique pour tout le pack ferait exploser la toile : les poses de plafond ont leur ancre en haut du dessin et celles au sol en bas, et la toile devrait couvrir les deux. Mesure a l'appui, le pack de blob passe de 128x128 a 150x193 si on l'impose - et la toile est la taille de la FENETRE, dont la surface commande le cout de SetWindowPos, seul poste de CPU du projet. Toile retenue : ${W}x${H}, calculee sur les pixels opaques et non sur les bords declares des images. Les poses dragged* portent leur ancre remontee de 120 px : Dragged.java place l'ancre a curseur + (0,120), le personnage etant tenu par la tete. Hitbox mesuree sur shime1 apres recadrage - boite opaque x $($boite[0])..$($boite[2]), y $($boite[1])..$($boite[3]) - et non recopiee sur blob : la numerotation des poses se transpose d'un pack a l'autre, les proportions du dessin non. Frames presentes : $($presentes.Count)."
    if ($ecartees.Count -gt 0) {
        $mes += " Poses ECARTEES faute de frames, donc retirees du tirage d'envies (spec 8.7) : $($ecartees -join ', ')."
    }
    foreach ($a in $avert) { $mes += " ATTENTION : $a." }
    [void]$sb.AppendLine("  ""_mesures"": ""$mes"",")
    [void]$sb.AppendLine("")

    [void]$sb.AppendLine("  ""id"": ""$slug"",")
    [void]$sb.AppendLine("  ""name"": ""$slug"",")
    [void]$sb.AppendLine("  ""frameSize"": [$W, $H],")
    [void]$sb.AppendLine("  ""scale"": 1,")
    [void]$sb.AppendLine("  ""hitbox"": [$hx, $hy, $lh, $hh],")
    [void]$sb.AppendLine("")
    [void]$sb.AppendLine("  ""poses"": {")

    $lignes = @()
    foreach ($nom in $retenues.Keys) {
        $def = $retenues[$nom]
        $bouts = @("""frames"": [" + ($def.frames -join ", ") + "]")
        if ($def.ContainsKey('frameMs')) { $bouts += """frameMs"": $($def.frameMs)" }
        if ($def.ContainsKey('loop'))    { $bouts += """loop"": true" }

        # L'ancre est ecrite sur CHAQUE pose : le repli de Rust est [64,128]
        # en dur, faux des que la toile n'est pas en 128x128.
        #
        # Les poses de glisser sont la seule exception, et elle ne vient pas
        # du pack : c'est NOTRE calcul, tire de Dragged.java. Les 120 px sont
        # le decalage entre le curseur et l'ancre du sprite.
        $a = $ancresParPose[$nom]
        if ($nom -like 'dragged*') { $ay = $a[1] - 120 } else { $ay = $a[1] }
        $bouts += """anchor"": [$($a[0]), $ay]"

        $lignes += "    ""$nom"": { " + ($bouts -join ", ") + " }"
    }
    [void]$sb.AppendLine(($lignes -join ",`r`n"))
    [void]$sb.AppendLine("  }")
    [void]$sb.AppendLine("}")

    return @{ ok = $true; json = $sb.ToString(); poses = $retenues.Count }
}

# --- Recuperer un pack ----------------------------------------------------
function Get-Pack($slug) {
    $dossier = Join-Path $Destination $slug
    $img = Join-Path $dossier "img"
    $manifeste = Join-Path $dossier "mascot.json"

    if ((Test-Path $manifeste) -and (-not $Force)) {
        return @{ etat = "saute"; slug = $slug }
    }

    New-Item -ItemType Directory -Force -Path $img | Out-Null

    # --- Les frames -------------------------------------------------------
    # 1..46 est le vocabulaire standard. On continue au-dela tant qu'on
    # trouve, quelques packs allant plus loin. On s'arrete apres deux
    # absences consecutives : un trou isole n'est pas une fin de serie.
    #
    # On NE REUTILISE JAMAIS des frames deja sur le disque. Le recadrage
    # plus bas est destructif, et reappliquer a une image deja recadree les
    # ancres de l'actions.xml, qui se rapportent a l'image D'ORIGINE,
    # decalerait le personnage un peu plus a chaque passage sans rien
    # signaler. Une execution interrompue entre le recadrage et l'ecriture
    # du manifeste laisse exactement cet etat. La reprise reste possible au
    # grain du PACK, un dossier ayant deja son mascot.json etant saute.
    $presentes = New-Object System.Collections.Generic.HashSet[int]
    $absencesDeSuite = 0
    $n = 1
    while ($true) {
        $octets = Get-Octets "$CDN/$slug/img/shime$n.png"
        # Le seuil de 100 octets ecarte les pages d'erreur deguisees en
        # reponse 200, qui existent sur certains CDN.
        if ($null -ne $octets -and $octets.Length -gt 100) {
            [System.IO.File]::WriteAllBytes((Join-Path $img "shime$n.png"), $octets)
            [void]$presentes.Add($n); $absencesDeSuite = 0
        } else {
            $absencesDeSuite++
        }
        if ($DelaiMs -gt 0) { Start-Sleep -Milliseconds $DelaiMs }

        $n++
        if ($n -gt 46 -and $absencesDeSuite -ge 2) { break }
        if ($n -gt 80) { break }
    }

    if ($presentes.Count -eq 0) {
        Remove-Item -Recurse -Force $dossier -ErrorAction SilentlyContinue
        return @{ etat = "vide"; slug = $slug; raison = "aucune frame servie par le CDN" }
    }
    if (-not $presentes.Contains(1)) {
        Remove-Item -Recurse -Force $dossier -ErrorAction SilentlyContinue
        return @{ etat = "rejete"; slug = $slug; raison = "pas de shime1.png" }
    }

    $r = Get-PosesRetenues $presentes
    $retenues = $r.retenues
    $ecartees = $r.ecartees
    foreach ($vitale in $POSES_VITALES) {
        if (-not $retenues.Contains($vitale)) {
            Remove-Item -Recurse -Force $dossier -ErrorAction SilentlyContinue
            return @{ etat = "rejete"; slug = $slug; raison = "pose vitale manquante : $vitale" }
        }
    }

    # --- Les ancres, la taille et la silhouette de chaque frame -----------
    $avert = @()
    $ancres = Get-AncresDuPack $slug
    if ($null -ne $ancres) {
        $origine = "lues dans l'actions.xml du pack, servi par le CDN"
    } else {
        $origine = "actions.xml absent ou illisible : ancre supposee au milieu-bas de chaque frame, la convention de Shimeji-ee"
        $avert += "actions.xml indisponible, ancres deduites et non lues"
    }

    $infos = @{}
    foreach ($f in $presentes) {
        $chemin = Join-Path $img "shime$f.png"
        $d = Get-DimensionsPng $chemin
        if ($d[0] -le 0 -or $d[1] -le 0) {
            Remove-Item -Recurse -Force $dossier -ErrorAction SilentlyContinue
            return @{ etat = "rejete"; slug = $slug; raison = "shime$f.png illisible" }
        }
        # Sans actions.xml : le milieu-bas, ce que declarent toutes les poses
        # au sol de Shimeji-ee (64,128 sur une image de 128x128).
        if ($null -ne $ancres -and $ancres.ContainsKey($f)) { $a = $ancres[$f] }
        else { $a = @([int]($d[0] / 2), $d[1]) }
        $infos[$f] = @{ ax = $a[0]; ay = $a[1]; boite = [MesureSprite]::BoiteOpaque($chemin) }
    }

    # --- Les composantes : quelles frames doivent partager une ancre ------
    # Le manifeste declare UNE ancre par pose. Les frames d'une meme pose
    # doivent donc etre alignees entre elles, mais deux poses distinctes
    # peuvent parfaitement avoir des ancres differentes : c'est ce que fait
    # blob, [64,128] au sol et [64,48] au plafond.
    #
    # Une frame pouvant servir a plusieurs poses (shime1 est dans stand,
    # walk, run et dragged), l'appartenance se propage. On unit donc les
    # frames de chaque pose, et chaque composante obtenue recoit une ancre.
    $parent = @{}
    foreach ($f in $presentes) { $parent[$f] = $f }
    foreach ($nom in $retenues.Keys) {
        $frames = $retenues[$nom].frames
        $tete = Get-Racine $parent $frames[0]
        foreach ($f in $frames) {
            $autre = Get-Racine $parent $f
            if ($autre -ne $tete) { $parent[$autre] = $tete }
        }
    }

    # --- La toile : la plus grande composante decide ----------------------
    # Dans une composante on aligne les frames sur leur ancre : il faut a
    # gauche le plus grand debordement gauche, a droite le plus grand
    # debordement droit, et de meme en vertical.
    #
    # Ces debordements se mesurent sur les PIXELS OPAQUES et non sur les
    # bords declares des images : la toile devient la taille de la FENETRE
    # du personnage, et le seul poste de CPU du projet est SetWindowPos sur
    # une fenetre en couche, dont le cout suit la SURFACE composee.
    $comp = @{}
    foreach ($f in $presentes) {
        $c = Get-Racine $parent $f
        # Tout a zero, y compris les debordements droite et bas : un
        # plancher a 1 ajouterait une ligne et une colonne vides a toutes
        # les toiles, et le pack de blob sortirait en 128x129 au lieu de
        # 128x128. Le cas « composante entierement transparente » est
        # attrape par le plancher global sur $W et $H, plus bas.
        if (-not $comp.ContainsKey($c)) { $comp[$c] = @{ ax = 0; ay = 0; droite = 0; bas = 0 } }
        $i = $infos[$f]
        $b = $i.boite
        if ($b[0] -lt 0) { continue }   # frame entierement transparente
        # +1 : `boite` donne le dernier pixel opaque, pas le bord suivant.
        $g = $i.ax - $b[0]; $dr = ($b[2] + 1) - $i.ax
        $ht = $i.ay - $b[1]; $bs = ($b[3] + 1) - $i.ay
        if ($g  -gt $comp[$c].ax)     { $comp[$c].ax     = $g }
        if ($ht -gt $comp[$c].ay)     { $comp[$c].ay     = $ht }
        if ($dr -gt $comp[$c].droite) { $comp[$c].droite = $dr }
        if ($bs -gt $comp[$c].bas)    { $comp[$c].bas    = $bs }
    }

    $W = 1; $H = 1
    foreach ($c in $comp.Keys) {
        $lc = $comp[$c].ax + $comp[$c].droite
        $hc = $comp[$c].ay + $comp[$c].bas
        if ($lc -gt $W) { $W = $lc }
        if ($hc -gt $H) { $H = $hc }
    }

    # Garde-fou : une toile demesuree trahit une frame aberrante, et la
    # fenetre du personnage prendrait cette taille a l'ecran.
    if ($W -gt 512 -or $H -gt 512) {
        Remove-Item -Recurse -Force $dossier -ErrorAction SilentlyContinue
        return @{ etat = "rejete"; slug = $slug; raison = "toile demesuree apres recadrage : ${W}x${H}" }
    }

    foreach ($f in $presentes) {
        $c = Get-Racine $parent $f
        $i = $infos[$f]
        [MesureSprite]::Recadrer((Join-Path $img "shime$f.png"), $W, $H, ($comp[$c].ax - $i.ax), ($comp[$c].ay - $i.ay))
    }

    # L'ancre de chaque pose est celle de sa composante.
    $ancresParPose = @{}
    foreach ($nom in $retenues.Keys) {
        $c = Get-Racine $parent $retenues[$nom].frames[0]
        $ancresParPose[$nom] = @($comp[$c].ax, $comp[$c].ay)
    }

    $boite = [MesureSprite]::BoiteOpaque((Join-Path $img "shime1.png"))
    if ($boite[0] -lt 0) {
        Remove-Item -Recurse -Force $dossier -ErrorAction SilentlyContinue
        return @{ etat = "rejete"; slug = $slug; raison = "shime1.png entierement transparent" }
    }

    $m = New-Manifeste $slug $presentes $boite $W $H $retenues $ecartees $ancresParPose $origine $avert
    if (-not $m.ok) {
        Remove-Item -Recurse -Force $dossier -ErrorAction SilentlyContinue
        return @{ etat = "rejete"; slug = $slug; raison = $m.raison }
    }

    # Sans BOM, imperativement : serde_json refuse un fichier qui en porte
    # un, avec le message trompeur « expected value at line 1 column 1 ».
    # C'est le piege documente dans config.rs, et PowerShell 5.1 en ecrit un
    # par defaut avec Out-File comme avec Set-Content -Encoding utf8.
    $sansBom = New-Object System.Text.UTF8Encoding $false
    [System.IO.File]::WriteAllText($manifeste, $m.json, $sansBom)

    return @{
        etat = "ok"; slug = $slug; frames = $presentes.Count
        poses = $m.poses; ecartees = $ecartees; avert = $avert
        taille = "${W}x${H}"
    }
}

# --- Le corps du script ---------------------------------------------------

# -Index se traite EN TETE et sort : il ne telecharge aucun sprite, et ne
# doit donc pas passer par la mecanique de selection des cibles.
if ($Index) {
    New-Index
    exit 0
}

if ($Slug) {
    $cibles = $Slug
} elseif ($Tous -or $Curated -or $ListeSeule) {
    $cibles = Get-Slugs
    if ($Curated) {
        # Les depots communautaires portent un hash hexadecimal de 6
        # caracteres en suffixe : souvent des doublons ou des versions
        # incompletes du meme personnage.
        $cibles = $cibles | Where-Object { $_ -notmatch '-[0-9a-f]{6}$' }
    }
} else {
    Write-Host "Rien a faire : donne -Slug <nom>, ou -Curated, ou -Tous."
    Write-Host "(-ListeSeule pour voir le catalogue sans rien telecharger.)"
    exit 0
}

if ($Limite -gt 0) { $cibles = $cibles | Select-Object -First $Limite }

if ($ListeSeule) {
    $cibles
    Write-Host ""
    Write-Host "$($cibles.Count) slugs."
    exit 0
}

Write-Host "$($cibles.Count) pack(s) a traiter vers '$Destination', delai $DelaiMs ms."
Write-Host ""

$compteurs = @{ ok = 0; saute = 0; rejete = 0; vide = 0 }
$i = 0
$debut = Get-Date

foreach ($s in $cibles) {
    $i++
    Write-Progress -Activity "Recuperation des packs" -Status "$i / $($cibles.Count) : $s" -PercentComplete (100 * $i / $cibles.Count)
    $r = Get-Pack $s
    $compteurs[$r.etat]++

    switch ($r.etat) {
        "ok" {
            $extra = ""
            if ($r.ecartees.Count -gt 0) { $extra = "  ($($r.ecartees.Count) pose(s) ecartee(s))" }
            Write-Host ("  [{0,4}/{1}] {2,-42} {3,2} frames, {4,2} poses, {5}{6}" -f $i, $cibles.Count, $s, $r.frames, $r.poses, $r.taille, $extra)
            foreach ($a in $r.avert) { Write-Warning "    $s : $a" }
        }
        "saute" {
            Write-Host ("  [{0,4}/{1}] {2,-45} deja present" -f $i, $cibles.Count, $s) -ForegroundColor DarkGray
        }
        default {
            Write-Host ("  [{0,4}/{1}] {2,-45} REJETE : {3}" -f $i, $cibles.Count, $s, $r.raison) -ForegroundColor Yellow
        }
    }
}

Write-Progress -Activity "Recuperation des packs" -Completed
$duree = (Get-Date) - $debut

Write-Host ""
Write-Host "-- Bilan ------------------------------------------------"
Write-Host "  installes : $($compteurs.ok)"
Write-Host "  deja la   : $($compteurs.saute)"
Write-Host "  rejetes   : $($compteurs.rejete)"
Write-Host "  vides     : $($compteurs.vide)"
Write-Host "  duree     : $([int]$duree.TotalMinutes) min $([int]$duree.Seconds) s"

if (Test-Path $Destination) {
    $taille = (Get-ChildItem -Recurse -File $Destination | Measure-Object -Property Length -Sum).Sum
    Write-Host "  sur disque: $([math]::Round($taille / 1MB, 1)) Mo dans '$Destination'"
}
