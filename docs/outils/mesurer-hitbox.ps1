# Mesure la hitbox réelle de chaque pose d'un pack de personnage, et l'écrit
# dans son `mascot.json`.
#
# ── Pourquoi cet outil existe ────────────────────────────────────────────
#
# La hitbox est la zone où les clics cessent de traverser (spec §3.3). Elle
# était jusqu'ici RECOPIÉE d'un pack à l'autre et réglée à l'œil — et elle
# était fausse : sur `blob`, `[40, 20, 48, 108]` couvrait 48 px de large là où
# le dessin en fait 91, et commençait 19 px sous le sommet du crâne. Un clic
# droit sur l'épaule du personnage ne l'atteignait donc pas.
#
# C'est la même leçon que l'ancre à l'étape 1a et que la hitbox de `luffy` :
# **sur ce projet, tout ce qui a été réglé à l'œil s'est révélé faux.** D'où
# une mesure, et un outil pour la refaire sur n'importe quel pack déposé.
#
# ── Ce qu'elle mesure ────────────────────────────────────────────────────
#
# Pour chaque pose : l'union, sur toutes ses frames, des bornes des pixels
# dont l'alpha vaut au moins 16 (le seuil écarte l'antialiasing quasi
# invisible, pas l'ombre portée). C'est donc la boîte englobante du dessin —
# volontairement pas un test par pixel : Windows ne sait activer la traversée
# des clics que par fenêtre entière, pas par pixel.
#
# ── Usage ────────────────────────────────────────────────────────────────
#
#   .\docs\outils\mesurer-hitbox.ps1 -Pack blob            # affiche
#   .\docs\outils\mesurer-hitbox.ps1 -Pack blob -Ecrire    # écrit le json

param(
    [Parameter(Mandatory = $true)][string]$Pack,
    [switch]$Ecrire
)

Add-Type -AssemblyName System.Drawing

$racine = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$dir = Join-Path $racine "characters\$Pack"
$chemin = Join-Path $dir 'mascot.json'

if (-not (Test-Path $chemin)) { throw "pack introuvable : $chemin" }

$texte = Get-Content $chemin -Raw -Encoding UTF8
# Le BOM, que les éditeurs Windows ajoutent et que serde_json refuse — la
# même précaution que `config::lire_json` côté Rust.
$texte = $texte -replace '^﻿', ''
$m = $texte | ConvertFrom-Json

# Bornes des pixels opaques d'une frame, mémoïsées : les poses partagent
# largement leurs frames (walk et run ont exactement les mêmes).
$cache = @{}
function Bornes($n) {
    if ($cache.ContainsKey($n)) { return $cache[$n] }
    $p = Join-Path $dir "img\shime$n.png"
    if (-not (Test-Path $p)) { $cache[$n] = $null; return $null }

    $b = [System.Drawing.Bitmap]::FromFile($p)
    # LockBits et non GetPixel : 46 frames x 16 384 pixels, c'est la
    # différence entre une seconde et une minute.
    $r = New-Object System.Drawing.Rectangle 0, 0, $b.Width, $b.Height
    $d = $b.LockBits($r, [System.Drawing.Imaging.ImageLockMode]::ReadOnly,
        [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $buf = New-Object byte[] ($d.Stride * $b.Height)
    [System.Runtime.InteropServices.Marshal]::Copy($d.Scan0, $buf, 0, $buf.Length)
    $b.UnlockBits($d)

    $minx = [int]::MaxValue; $miny = [int]::MaxValue; $maxx = -1; $maxy = -1
    for ($y = 0; $y -lt $b.Height; $y++) {
        $ligne = $y * $d.Stride
        for ($x = 0; $x -lt $b.Width; $x++) {
            if ($buf[$ligne + $x * 4 + 3] -ge 16) {
                if ($x -lt $minx) { $minx = $x }
                if ($x -gt $maxx) { $maxx = $x }
                if ($y -lt $miny) { $miny = $y }
                if ($y -gt $maxy) { $maxy = $y }
            }
        }
    }
    $b.Dispose()

    $res = if ($maxx -lt 0) { $null } else { @($minx, $miny, $maxx, $maxy) }
    $cache[$n] = $res
    return $res
}

$mesures = [ordered]@{}
foreach ($nom in $m.poses.PSObject.Properties.Name) {
    $minx = [int]::MaxValue; $miny = [int]::MaxValue; $maxx = -1; $maxy = -1
    foreach ($f in $m.poses.$nom.frames) {
        $bb = Bornes $f
        if ($null -eq $bb) { continue }
        if ($bb[0] -lt $minx) { $minx = $bb[0] }
        if ($bb[1] -lt $miny) { $miny = $bb[1] }
        if ($bb[2] -gt $maxx) { $maxx = $bb[2] }
        if ($bb[3] -gt $maxy) { $maxy = $bb[3] }
    }
    if ($maxx -lt 0) { Write-Warning "$nom : aucune frame lisible"; continue }
    $mesures[$nom] = @($minx, $miny, ($maxx - $minx + 1), ($maxy - $miny + 1))
    "{0,-14} [{1}, {2}, {3}, {4}]" -f $nom, $mesures[$nom][0], $mesures[$nom][1], $mesures[$nom][2], $mesures[$nom][3]
}

if (-not $Ecrire) { "";  "(rien écrit — relancer avec -Ecrire)"; return }

# ── L'écriture ───────────────────────────────────────────────────────────
#
# Une substitution ligne à ligne plutôt qu'un ConvertTo-Json du document
# entier : le manifeste est écrit à la main, avec ses champs `_source` et
# `_hitbox` et son alignement en colonnes. Le réécrire par sérialisation
# perdrait tout cela, et le diff deviendrait illisible.
$lignes = Get-Content $chemin -Encoding UTF8
$sortie = foreach ($l in $lignes) {
    $ok = $l -match '^(\s*)"([A-Za-z0-9]+)":(\s*)\{(.*)\}(,?)\s*$'
    if ($ok -and $mesures.Contains($Matches[2])) {
        $nom = $Matches[2]
        $corps = $Matches[4].TrimEnd()
        # Une hitbox déjà présente est remplacée, pas dupliquée.
        $corps = $corps -replace ',\s*"hitbox":\s*\[[^\]]*\]', ''
        $h = $mesures[$nom]
        "$($Matches[1])`"$nom`":$($Matches[3]){$corps, `"hitbox`": [$($h[0]), $($h[1]), $($h[2]), $($h[3])] }$($Matches[5])"
    }
    else { $l }
}

# UTF8 sans BOM, pour la raison rappelée plus haut.
[System.IO.File]::WriteAllLines($chemin, $sortie, (New-Object System.Text.UTF8Encoding $false))
""
"écrit : $chemin"
