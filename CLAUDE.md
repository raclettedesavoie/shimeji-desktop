# shimeji-desktop

Un *desktop pet* pour Windows. Des personnages en pixel-art vivent sur le bureau :
ils se déplacent, se reposent, **grimpent sur les bords des fenêtres ouvertes**, et
**réagissent à l'activité de la machine**. Compatible avec les packs de personnages
**Shimeji** existants — on en dépose un dans un dossier, il fonctionne.

Projet plaisir, personnel, Windows uniquement.

---

## Le besoin

### Ce que le personnage fait

Il n'attend aucun ordre. Il **enchaîne des activités tout seul** avec une part
d'aléatoire, pour qu'on ne devine jamais la suite : se déplacer, s'arrêter, s'asseoir,
dormir, manger, sauter, s'amuser. L'objectif n'est pas la liste des animations, c'est
**qu'il ait l'air vivant** — jamais figé, jamais prévisible.

### Où il vit

Il n'est pas cantonné au bas de l'écran. Il **explore** :

- il marche sur le bureau, au-dessus de la barre des tâches
- il **grimpe le long des bords des fenêtres** et s'assoit sur leur barre de titre
- il escalade les bords de l'écran, se suspend au plafond
- si la fenêtre sous ses pieds disparaît, **il tombe** — et il atterrit
- il circule sur **tous les écrans**

### Ce à quoi il réagit

| Quand… | …il |
|---|---|
| l'utilisateur ne touche plus à rien | s'assoit, puis s'endort |
| l'utilisateur revient | se réveille |
| il est midi | mange |
| il est tard le soir | traîne, dort davantage |
| l'application au premier plan change | change d'attitude **et se déplace vers cette fenêtre** |
| la batterie est faible | fatigue |
| la session est verrouillée | disparaît proprement |

### Interaction

On peut **l'attraper à la souris et le lâcher** n'importe où (il tombe et se rattrape),
**cliquer dessus** pour le faire réagir, et le **cacher ou le rappeler** à tout moment.
Il ne gêne jamais : pas de vol de focus, absent de la barre des tâches, les clics le
traversent.

### Plusieurs personnages

Plusieurs personnages coexistent à l'écran et **se remarquent** : ils s'approchent et
réagissent l'un à l'autre. Ajouter un personnage est une **opération de contenu**
(déposer un dossier), jamais une modification de code.

### Hors sujet — décisions déjà prises

- ❌ **Pas un Tamagotchi** — aucune stat à surveiller, aucune obligation, il ne meurt pas
- ❌ **Aucune capture de frappe** — on sait seulement si l'utilisateur est actif, jamais quelle touche
- ❌ **Pas un assistant** — il ne notifie rien, ne rappelle rien, n'a aucune utilité productive
- ❌ **Pas un jeu** — pas de score, pas de progression
- ❌ **Pas de charge CPU comme signal** — écarté explicitement
- ❌ **Pas de bulles de dialogue**

C'est du plaisir visuel, rien d'autre. **Toute idée qui ajoute une obligation est hors sujet.**

---

## Stack

| Couche | Choix |
|---|---|
| Application | **Tauri v2** — binaire natif ~10 Mo |
| Logique | **Rust** — toute la logique, sans exception |
| Affichage | **Webview** — HTML/CSS minimal, ~60 lignes de JS |
| APIs Windows | crate **`windows`** (officielle Microsoft), bindings typés |
| Contenu | fichiers **externes**, à côté de l'exe |

Volontairement **pas** de TypeScript, pas de bundler, pas de framework front, pas de
canvas. Le webview est un afficheur de sprite délibérément bête.

### Pourquoi Tauri plutôt qu'Electron

Ce projet a deux moitiés : l'animation d'un personnage (le web est excellent) et
l'introspection des fenêtres Windows (`EnumWindows`, `DwmGetWindowAttribute`,
`GetForegroundWindow`). Electron n'atteint la seconde que par FFI, en décrivant à la
main des structs win32 en JS — un mauvais calibre de struct donne un segfault, pas une
erreur, et c'est sur l'API la plus centrale du projet. Rust les expose **typées**.
Bonus : ~10 Mo à distribuer au lieu de ~150.

### Distribution

Rust est une dépendance de **compilation**, pas d'exécution. On livre un exe ; la
machine cible n'a besoin que de **WebView2** (déjà présent sur Windows 11) et du runtime
Visual C++ — à **lier statiquement** pour que l'exe soit totalement autonome.

### Chaîne d'outils — opérationnelle (vérifié le 2026-09-08)

**Tout est installé, ne rien réinstaller.** Le spike de l'étape 0 compile.

| Outil | État |
|---|---|
| rustc / cargo | ✅ 1.98.1 |
| cible `x86_64-pc-windows-msvc` | ✅ installée |
| Charge C++ / MSVC 14.51.36231 / SDK 10.0.26100.0 | ✅ sur **Visual Studio 18 Insiders** |
| WebView2 | ✅ 152.0.4191.66 |
| `cargo-tauri` | ⬜ non installé, et **inutile** |
| Node | ⚠️ v14.17.0 — **délibérément inutilisé** |

**Trois pièges de cette machine, chacun ayant déjà coûté un diagnostic :**

> **1. La charge C++ vit sur VS 18 Insiders, pas sur 2022 Community.** `vswhere` ne la
> voit **qu'avec `-prerelease`** ; sans ce drapeau on conclut à tort qu'elle est absente.
> rustc, lui, la détecte seul — aucune variable d'environnement, aucun *Developer
> PowerShell* nécessaire.

> **2. Ne pas installer Node.** Tauri s'installe par npm *ou* par cargo. La voie npm
> exigerait Node 18+, que cette machine n'a pas — mais le front est statique, donc **Node
> ne sert nulle part**. Il n'y a rien à corriger.

> **3. Compiler depuis PowerShell, pas depuis Git Bash.** Dans un shell Git Bash, rustc
> peut pêcher le `link.exe` de Git for Windows (un utilitaire coreutils de liens durs) au
> lieu du linker MSVC, et rendre une erreur `extra operand` totalement opaque.

### Compiler et lancer

Depuis **PowerShell** (voir piège 3).

```powershell
cd C:\Users\alri\Documents\shimeji-desktop\src-tauri
cargo test               # la suite complète, sans écran
cargo run                # l'application : un personnage sur le sol
cargo run -- --sim 30    # 30 min de comportement sans écran (spec §10.3)
cargo run -- --demarrage etat|on|off   # le démarrage avec Windows, scriptable
```

**Cinq variables d'environnement de diagnostic.** Les trois premières ont chacune servi
à démentir une hypothèse fausse — voir « Mesurer le CPU » plus bas ; les deux dernières
remplacent un clic dans le tray :

| Variable | Ce qu'elle fait |
|---|---|
| `SHIMEJI_CADENCE=1` | images/s réelles, travail par image, **et le taux de déplacement** |
| `SHIMEJI_SANS_BOUCLE=1` | crée la fenêtre et n'anime rien |
| `SHIMEJI_TRACE=1` | trace chaque image servie par le schéma URI |
| `SHIMEJI_CACHE=1` | démarre caché, comme si « Afficher » était décoché |
| `SHIMEJI_QUITTER_APRES=<s>` | appelle `exit(0)` — la ligne de « Quitter » — après *s* secondes |

**Et un fichier témoin** : créer `characters/recharger.txt` déclenche un rechargement à
chaud, puis le fichier est supprimé.

> **Le principe, récurrent sur ce projet : tout ce qui demanderait un clic reçoit un
> équivalent scriptable.** Le tray, le démarrage automatique et le rechargement se
> constatent normalement à la souris ; à ce jour, seule **la dépêche d'un clic de menu**
> par le tray n'a pas d'équivalent, tout le reste se vérifie sans humain — sonde de
> styles Win32, `--demarrage etat`, fichier témoin, `SHIMEJI_CACHE`,
> `SHIMEJI_QUITTER_APRES`, énumération des fenêtres, lecture de l'en-tête PE.

> ⚠️ **`cargo test` ne reconstruit pas l'exe.** Il compile le harnais de test. Après une
> correction, `cargo build` avant de relancer l'application — sinon on vérifie un binaire
> plus ancien que la source, et l'on conclut à tort que le correctif ne marche pas. C'est
> arrivé une fois, sur le correctif du BOM.

Le **spike de l'étape 0** est archivé dans `docs/spike-etape-0/` (`cargo run` y fonctionne
aussi). Voir son `README.md` pour ce qu'il prouve et la sonde de styles Win32 qui
l'accompagne. **Ne pas le faire évoluer vers l'application** : il lui manque délibérément
les deux styles étendus que son analyse a révélés nécessaires.

### ⚠️ Mesurer le CPU — vérification systématique

**Un desktop pet qui consomme se fait désinstaller.** Toute modification du chemin à
60 Hz — la boucle de `main.rs`, `render.rs`, la sonde — doit être suivie d'une **mesure**,
pas d'une intuition.

### ⚠️⚠️ La méthodologie AVANT les chiffres — trois hypothèses fausses de suite

**Mesurer sur 10 secondes ne veut rien dire, et c'est le piège central ici.** La
consommation dépend entièrement de **ce que le personnage est en train de faire** :
en marche il déplace sa fenêtre à chaque image, à l'arrêt il ne la déplace pas du tout.
Relevé sur des tranches de 5 s consécutives, le taux de déplacement va de **0 % à 87 %**
des images. Deux mesures de 10 s sur la même version peuvent donc donner 12 % et 25 %.

> **Trois hypothèses ont été formulées et démenties par la mesure, dans cet ordre.**
> Les garder ici parce que chacune paraissait évidente :
>
> 1. « C'est `set_size` appelé à chaque image. » → **Faux.** Le retirer n'a rien changé
>    (14,8 % → 14,1 %). Il a été séparé quand même : appeler une API du système 60 fois
>    par seconde pour une valeur constante est une faute par principe.
> 2. « Le coût est inhérent à la fenêtre en couche, notre boucle n'y est pour rien. » →
>    **Faux.** Avec `SHIMEJI_SANS_BOUCLE=1` — la fenêtre créée, aucune animation — la
>    consommation est de **0 %**. Tout vient de la boucle.
> 3. « Notre logique 60 Hz est trop lourde. » → **Faux aussi.** `SHIMEJI_CADENCE=1`
>    montre **58 img/s** et un travail de **100 à 900 µs par image**, soit 1 à 5 % du
>    budget de 16,7 ms. Le calcul n'est pas le problème.
>
> **Ce qui coûte réellement** : chaque `set_position` est dispatché au thread principal
> de Tauri, qui appelle `SetWindowPos` sur une fenêtre **en couche** — Windows y refait
> une composition alpha. Le coût est donc proportionnel au **nombre de déplacements**,
> et c'est le seul levier.

### Le protocole, et les outils

**Toujours 40 à 60 secondes**, pour que marche et arrêt s'équilibrent :

```powershell
$p = Get-Process -Name shimeji-desktop
$c = $p.CPU; Start-Sleep -Seconds 60; $p.Refresh()
"$([math]::Round((($p.CPU - $c) / 60) * 100, 1)) % d'un coeur"
```

Les variables de diagnostic, **conservées** parce que ce sont elles qui ont démenti les
hypothèses 2 et 3 :

| Variable | Ce qu'elle donne |
|---|---|
| `SHIMEJI_CADENCE=1` | images/s réelles, travail moyen par image, **et le nombre de déplacements sur le nombre d'images** — c'est ce dernier chiffre qui explique tout |
| `SHIMEJI_SANS_BOUCLE=1` | crée la fenêtre et n'anime rien : sépare le coût de la fenêtre de celui de la boucle |
| `SHIMEJI_CACHE=1` | la boucle tourne entièrement, mais ne déplace ni ne dessine rien : sépare **notre calcul** du coût des déplacements |
| `SHIMEJI_TRACE=1` | trace chaque image servie par le schéma URI (a diagnostiqué le sprite invisible) |

**Chiffres de référence, build *debug*, mesures de 40 à 60 s, le 2026-09-09 :**

| Configuration | CPU |
|---|---|
| fenêtre seule, **aucune boucle** (`SHIMEJI_SANS_BOUCLE=1`) | **0 %** |
| `set_position` à chaque image, quoi qu'il arrive | **21 %** |
| **`set_position` seulement si la position a changé au pixel** | **12,3 %** |
| **la même chose, build `release`** (exe de 2,7 Mo) | **12 %** |
| **caché** (`SHIMEJI_CACHE=1`), build `release` | **0,9 %** |

> **Le relevé « caché » chiffre enfin le partage.** En mode caché la boucle tourne
> *entièrement* — sonde du curseur, hit-testing, physique, comportement, 60 fois par
> seconde ; **seuls le déplacement et la poussée du sprite sont sautés.** Donc :
> **0,9 % = tout ce que nous calculons**, et les **~11 points restants = `SetWindowPos`
> sur une fenêtre en couche.** C'est la confirmation directe du diagnostic, et la raison
> pour laquelle optimiser notre code ne rapporterait rien.

> **Le `release` ne gagne rien sur le debug, et c'est cohérent.** Notre travail par image
> ne représente que 1 à 5 % du budget de 16,7 ms — les optimisations du compilateur n'ont
> presque rien sur quoi mordre. Le coût est dans `SetWindowPos` de Windows, que le profil
> release ne change pas. Le profil visant d'ailleurs la **taille** (`opt-level = "z"`,
> LTO), il n'y avait pas de raison d'attendre mieux.
>
> Corollaire : **inutile de repasser le profil en `opt-level = 3`.** On paierait la taille
> de l'exe — un objectif de la spec §4, tenu ici à 2,7 Mo contre ~10 Mo visés — pour un
> gain nul.

**Pistes restantes**, par rentabilité décroissante :

1. **Suspendre la boucle quand la session est verrouillée** — prévu comme réflexe à
   l'étape 2, et c'est aussi une optimisation.
2. **Descendre à 8 Hz les images où la position ne change pas** — le personnage à l'arrêt
   n'a besoin ni de 60 déplacements ni de 60 décisions par seconde. Gain modeste, le
   travail de calcul étant déjà négligeable.
3. ~~Ne pas appeler `set_position` quand la position n'a pas changé~~ — **appliqué**,
   21 % → 12,3 %.
4. ~~Ne rien dessiner quand les personnages sont cachés~~ — **appliqué** (Tâche 1 de 1b,
   tirée en avant) **et mesuré** : 12 % → **0,9 %**. Caché, il ne reste plus que notre
   propre calcul.

> **Ce qu'il ne faut PAS faire :** descendre la cadence sous 60 Hz. L'étape 0 a établi
> que 60 Hz est fluide sur cette machine, et le travail par image ne représente que 1 à
> 5 % du budget. Ce serait payer en fluidité ce qui ne coûte rien.

**`cargo run` suffit — pas besoin de `cargo tauri dev`.** Le front étant statique, les
assets sont embarqués dans le binaire à la compilation. Le CLI Tauri ne devient nécessaire
que pour produire un installateur.

> **Pour arrêter l'application : « Quitter » dans le menu du tray.** Depuis la Tâche 1
> du plan 1b, c'est la voie normale. Ce qui suit ne vaut que si le tray n'a pas pu
> s'installer — le message `tray non installé` le dirait.
>
> ⚠️ **En secours : `Stop-Process -Name shimeji-desktop`** (ou `Ctrl+C` dans le terminal,
> en debug seulement — le build `release` n'a pas de console). La fenêtre est volontairement
> non focalisable, sans bordure, hors taskbar et hors Alt+Tab : elle ne peut donc **pas** se
> fermer normalement. C'est le comportement voulu, et c'est exactement pourquoi « Quitter »
> était la Tâche 1 du plan 1b, avant le retrait de la console.

**Deux exigences de `tauri-build` découvertes à l'étape 0**, valables aussi pour
l'application :

- `icons/icon.ico` est **obligatoire**, même pour un projet jetable — son absence fait
  échouer le script de build.
- `frontendDist` est résolu **relativement au dossier du `tauri.conf.json`**. Le spike a
  une arborescence plate, donc `"./ui"` ; l'étape 1 utilisera `src-tauri/`, où le `"../ui"`
  conventionnel sera correct.

### L'auteur apprend Rust sur ce projet

Le choix de Tauri est en partie motivé par l'envie d'apprendre Rust. Conséquence pour
toute session d'assistance : **expliquer le Rust plutôt que le supposer acquis** — la
propriété, l'emprunt, les traits, `Result`. Préférer du code simple et lisible à du code
idiomatique et dense : un `match` explicite vaut mieux qu'une chaîne de combinateurs
astucieuse.

---

## Conventions de code

**Le code de ce projet doit être abondamment commenté**, en français, pour être relu
facilement des semaines plus tard par quelqu'un qui apprend encore Rust. C'est une
exigence explicite de l'auteur, pas une préférence de style — ne pas « nettoyer » les
commentaires au nom de la concision.

### Commenter le *pourquoi*, pas le *quoi*

```rust
// ✗ inutile — le code le dit déjà
// incrémente le compteur
count += 1;

// ✓ utile — explique une décision non évidente
// On repart du rectangle courant de la plateforme au lieu de mémoriser la
// position : c'est ce qui rend gratuit le déplacement de la fenêtre (décision n° 1).
let pos = platform.rect.point_on(face, offset);
```

### Renvoyer à la décision que le code applique

Quand un bloc met en œuvre une décision du design, **citer la section de la spec**. C'est
ce qui permet, en relisant, de retrouver le raisonnement sans le reconstituer :

```rust
// Les clics traversent en permanence ; on ne les réactive que dans la
// hitbox de la pose courante (spec §3.3).
win.set_ignore_cursor_events(true)?;
```

### Expliquer les constructions Rust non évidentes

Tout ce qui n'est pas du Rust élémentaire mérite une ligne : `let Some(x) = … else`,
les durées de vie annotées, `Arc`/`Mutex`, les `impl Trait`, les combinateurs sur
`Option`/`Result`, et toute raison liée à l'emprunt.

```rust
// `let … else` : si la fenêtre a été fermée, on sort du thread. Équivalent
// d'un `match` dont la branche None ferait `return`.
let Some(win) = handle.get_webview_window(LABEL) else {
    return;
};
```

### Découper les fonctions longues par bandeaux

Les boucles et les `setup` deviennent vite illisibles. Les scander :

```rust
// ── Topologie des écrans ────────────────────────────────
// ── La fenêtre du personnage ────────────────────────────
// ── Déplacement à 60 Hz ─────────────────────────────────
```

### En-tête de fichier

Chaque module s'ouvre sur deux ou trois lignes disant **sa responsabilité unique** et la
section de la spec dont il relève. Si cet en-tête devient difficile à écrire, c'est que le
fichier fait trop de choses.

### Ce qui ne s'explique pas en commentaire

Un nom mal choisi ne se rattrape pas par un commentaire. Nommer d'abord, commenter
ensuite.

---

## Architecture

### Une fenêtre par personnage

Chaque personnage est **sa propre petite fenêtre de 128×128**, transparente, sans
bordure, hors taskbar, toujours au premier plan, déplacée par Rust. C'est ce que fait le
vrai Shimeji.

**Ne pas revenir à un overlay transparent plein écran** : une seule fenêtre ne peut pas
couvrir proprement deux moniteurs de DPI différents, et c'est un défaut structurel. Avec
une fenêtre par personnage, changer d'écran, c'est écrire une position.

Le hit-testing : les clics traversent en permanence, et Rust ne les réactive que quand le
curseur entre dans la **hitbox serrée** de l'animation courante (`GetCursorPos` sondé à
~30 Hz, quasi gratuit).

### Où vit la logique

La position étant appliquée par Rust, faire calculer la physique en JS coûterait un
aller-retour IPC **60 fois par seconde par personnage**. Donc :

| Couche | Où |
|---|---|
| Fenêtres, signaux, tray, config, **physique, comportement** | **Rust** |
| Dessin du sprite | Webview (reçoit `{frame, flip}`, pose un `background-position`) |

Corollaire : Rust détenant **tous** les personnages, le comportement social est une
simple vérification côté coordinateur — aucun canal entre fenêtres à inventer.

### Le monde = une liste de plateformes

Tout ce sur quoi un personnage peut se tenir, s'accrocher ou se suspendre est un
rectangle avec une face utilisable. Deux sources seulement :

- **chaque écran** → sol, 2 murs, plafond
- **chaque fenêtre** → dessus (barre de titre), 2 murs, dessous (suspension)

La physique et les comportements **ne savent jamais** si une plateforme est le bas de
l'écran ou la barre de titre de VSCode. C'est ce qui permet de livrer le sol d'abord et
de brancher les fenêtres ensuite sans rien réécrire.

### Trois horloges

| Rythme | Quoi | Pourquoi |
|---|---|---|
| **60 Hz** | physique, dessin, **et la seule plateforme occupée** | interroger *une* fenêtre est quasi gratuit → il colle à la fenêtre qu'on déplace |
| **~8 Hz** | recensement complet des fenêtres, filtres, occlusion | une fenêtre qui apparaît est vue en ~125 ms : imperceptible |
| **~2 Hz** | inactivité, appli active, heure, batterie | aucun de ces signaux ne bouge vite |

---

## Décisions de design à ne pas défaire

Chacune remplace une classe entière de bugs. Les annuler ramène les bugs.

### 1. Un personnage accroché ne stocke jamais sa position absolue

Il stocke `(plateforme, face, distance au bord)`. Sa position écran est **recalculée à
chaque image** depuis le rectangle courant de la plateforme.

Conséquences **gratuites** : la fenêtre bouge → il voyage avec ; elle est redimensionnée
→ il garde sa distance au bord ; elle rétrécit sous lui ou se ferme → un seul test, il
tombe. Un personnage assis sur une barre de titre qu'on balade est *le* moment qui fait
sourire avec un Shimeji — ici c'est une conséquence du modèle, pas une feature.

### 2. Toujours au premier plan, mais jamais sur un bord recouvert

Il reste visible au-dessus de tout. En échange, **un bord recouvert n'est pas exposé
comme plateforme** : un bord est un segment, on lui retire les fenêtres de z-order
supérieur (soustraction d'intervalles 1D). Il ne peut alors physiquement pas s'asseoir
sur du vide.

### 3. Les signaux biaisent, ils ne commandent pas

Si « inactif 2 min » **déclenchait** le sommeil, on aurait un afficheur d'état système
déguisé en personnage : prévisible, mort en trois jours. « Inactif 2 min » **multiplie
par 8 l'envie de dormir** — il s'endort presque toujours, mais parfois il s'assoit,
parfois il traîne encore. **Cette marge est le produit.**

### 4. La navigation est autorisée à échouer

Pas de calcul de chemin : la carte change 8 fois par seconde, un chemin est périmé avant
d'être parcouru. Décision **locale** à chaque image (cible au-dessus → grimper ; en
dessous → se laisser tomber ; même surface → avancer).

Il se coince parfois, il prend des routes idiotes — **et c'est souhaitable** : un pet qui
prend un chemin bête est attachant, un qui calcule l'itinéraire optimal a l'air d'un
robot.

Une seule règle de sécurité remplace toute l'énumération des cas de blocage :

> **Toute intention a un délai d'abandon (~20 s)**, après quoi elle échoue et il repart
> flâner.

### 5. Le comportement est de la donnée, pas du code

Trois couches, qui ne se parlent que vers le bas :

1. **Réflexes** (60 Hz, non négociables) — plateforme disparue → chute ; attrapé → porté ; contact → atterrissage ; session verrouillée → planqué
2. **Intention** (une seule à la fois) — `Flâner` · `AllerÀ(surface)` · `SeReposer` · `Jouer(action)`
3. **Envie** — tirage pondéré, où les **signaux modifient les poids** et les **slots manquants retirent les options**

Ajouter un signal = ajouter une ligne de table. Aucun nouveau chemin de code. Les poids
vivent dans la config → on règle son caractère **sans recompiler**.

---

## Personnages

Les personnages sont des **fichiers externes**, jamais embarqués dans le binaire : on en
ajoute un sans recompiler, on itère sur les animations sans build, et n'importe qui peut
déposer le sien.

```
shimeji-desktop.exe
characters/
└── blob/
    ├── mascot.json      ← manifeste (poses, timings, ancres, hitbox)
    └── img/
        └── shime1.png … shime46.png   (128×128 chacun)
```

### Le vocabulaire de poses = les slots Shimeji

La numérotation `shime1..46` est un **standard de fait** : tous les packs Shimeji
utilisent les mêmes numéros pour les mêmes poses. En visant ce vocabulaire, **tout pack
trouvé sur internet fonctionne** avec nos comportements, sans code.

On lit les **images** des packs par leur numérotation. On **n'implémente pas** les
`actions.xml` / `behaviors.xml` de Shimeji : ces XML sont baroques (séquences imbriquées,
sélecteurs conditionnels, expressions Java évaluées à l'exécution), c'est un projet en
soi, et ça enfermerait le design dans la sémantique du Shimeji Java — où nos signaux
système n'existent pas.

### Une frame = un fichier + une ancre

Toutes les frames sont en **128×128**, le personnage placé dans la boîte. Chaque pose
déclare une **ancre** (le sol sous ses pieds, la main qui agrippe) : positionner devient
« place l'ancre ici ».

> Ne **pas** recalculer une compensation de décalage quand la taille du sprite change —
> l'ancre existe exactement pour rendre ça inutile.

### Couverture partielle

Un personnage sans images d'escalade **ne grimpera jamais** : l'option est retirée du
tirage d'envies. Aucun cas particulier à coder. C'est ce qui rend les packs tiers
utilisables même incomplets.

### Le personnage de test — `blob`

Le mascotte blanc par défaut de Shimeji-ee, 46 frames. Il possède **toutes** les poses,
y compris celles qui débloquent les comportements difficiles :

| Frames | Pose |
|---|---|
| 23, 24, 25 | agripper une paroi verticale (**escalade**) |
| 34, 35, 36 | se hisser par-dessus un bord |
| 22 | suspension au plafond |
| 39, 40, 41 | s'asseoir puis dormir |
| 10 / 4 | chute / atterrissage |
| 18, 20, 21 | ramper |
| 44, 45, 46 | dédoublement (signature Shimeji) |

Développer contre `blob` **découple « le moteur marche » de « j'ai les bons dessins »**.

> ⚠️ L'art de ce mascotte n'est pas de nous. Sans importance en usage privé ; à vérifier
> avant toute publication du dépôt.

---

## Ordre de construction

Chaque étape est agréable en elle-même, et aucune ne dépend d'un dessin manquant.

| | Étape | Résultat |
|---|---|---|
| ✅ 0 | Validation technique | fenêtre transparente, sans bordure, au premier plan, hors taskbar, clics traversants, PNG déplacé à 60 Hz sur 2 écrans |
| ✅ 1 | **Il vit sur le sol** | marche, court, s'arrête, demi-tour, tous les écrans ; attrapable et il tombe ; tray, démarrage auto |
| **2** | **Il réagit** ← *la prochaine* | s'endort quand on part, se réveille au retour, mange à midi |
| 3 | **Un deuxième personnage** | ils coexistent et se remarquent |
| 4 | **Il grimpe** | bords de fenêtres, barres de titre, chute quand la fenêtre se ferme |
| 5 | **Il suit** | se déplace vers l'application au premier plan |

**L'étape 0 est un spike jetable et non négociable.** Le seul point faible de Tauri face
à Electron est justement l'overlay transparent : il faut le prouver sur cette machine
avant d'écrire une ligne de physique, pas après.

---

## Pièges Windows déjà identifiés

1. **Le rectangle que Windows donne n'est pas celui qu'on voit.** Une fenêtre Win10/11
   déclare ~7 px de bordure de redimensionnement invisible de chaque côté. Utilisé tel
   quel, le personnage est assis 7 px au-dessus de la barre de titre, dans le vide —
   subtilement faux, et très visible sur du pixel-art. Demander les bornes **visuelles**
   au compositeur (`DwmGetWindowAttribute` / `DWMWA_EXTENDED_FRAME_BOUNDS`).
2. **Windows 11 est plein de fenêtres fantômes.** Des applis modernes déclarent des
   fenêtres invisibles qui se présentent comme visibles. Écarter les fenêtres masquées
   (`DWMWA_CLOAKED`), les minimisées, les fenêtres outils, les minuscules — **et les
   fenêtres des personnages eux-mêmes**, sinon ils s'assoient les uns sur les autres.
3. **Le sol est la zone de travail, pas l'écran** — sinon il marche sous la barre des
   tâches.
4. **Coordonnées** : tout en pixels physiques du bureau virtuel ; n'appliquer le facteur
   d'échelle qu'au dimensionnement du sprite.

---

## État actuel

**L'étape 1 est terminée — l'application existe et se vit au quotidien.** Un personnage
`blob` marche, court, s'arrête, fait demi-tour, circule sur les deux écrans, s'attrape à
la souris, se lance et atterrit ; un tray l'affiche, le cache, le recharge, le fait
démarrer avec Windows et le quitte ; un `config.json` règle son caractère sans
recompiler. **140 tests**, exe release de **2,7 Mo**, **12 % d'un cœur** en marche et
**0,9 %** caché.

| Où | Contenu |
|---|---|
| `docs/specs/2026-09-08-design.md` | le design complet — le *pourquoi* de chaque décision |
| `docs/plans/2026-09-08-etape-0-spike-overlay.md` | le plan de l'étape 0, **soldé** ; **annexe A = structure de fichiers verrouillée pour l'étape 1** |
| `docs/specs/2026-09-08-spike-0-resultat.md` | **le résultat de l'étape 0** : grille remplie, décision de stack, API vérifiées, et les 2 découvertes à appliquer |
| `docs/plans/2026-09-08-etape-1a-il-vit-sur-le-sol.md` | le plan de l'étape 1a, **exécuté** — 11 tâches |
| `docs/plans/2026-09-09-etape-1b-tour-du-proprietaire.md` | le plan de l'étape 1b, **soldé** — tray, config, démarrage auto, rechargement à chaud, CPU |
| `docs/specs/2026-09-09-frames-shimeji.md` | **la correspondance frames → poses**, tirée des sources de Shimeji-ee — à lire avant de toucher au `mascot.json` |
| `docs/conception/2026-09-08-journal-decisions.md` | **pourquoi** chaque décision, et ce qu'elle a écarté — à lire avant d'en défaire une |
| `docs/conception/2026-09-08-discussion.md` | la discussion de conception intégrale, verbatim |
| `docs/spike-etape-0/` | le spike **archivé et gelé** + la sonde Win32 rejouable — ne pas le faire évoluer vers l'application |
| `characters/blob/img/` | les 46 frames du personnage de test |

### Ce que l'étape 0 a tranché (2026-09-08)

Les sept propriétés sont vertes. Les trois dont l'absence aurait renvoyé la stack vers
Electron — transparence réelle, premier plan, clics traversants — sont acquises **et**
expliquées par des styles étendus Win32 relevés, pas seulement constatées à l'œil. Aucun
remède du plan n'a eu à être appliqué. Le design reste valable à 100 %.

Quatre conséquences à respecter en écrivant l'étape 1 :

1. **60 Hz est fluide.** La cadence visée est la cadence retenue — le repli « 30 Hz +
   interpolation » de la spec §12 est **abandonné**, ne pas le réintroduire.
2. **Poser `WS_EX_NOACTIVATE` dès la création de la fenêtre.** Tauri ne le pose pas, et le
   focus n'est aujourd'hui préservé que parce que la fenêtre est inatteignable au clic.
   L'étape 1 réactive les clics dans la hitbox (§3.3) : sans ce style, **attraper le
   personnage volerait le focus de l'éditeur** — exactement ce que « il ne gêne jamais »
   interdit.
3. **Poser `WS_EX_TOOLWINDOW` au même endroit** — l'exclusion d'Alt+Tab observée ne repose
   aujourd'hui que sur une heuristique de Windows 11, `skip_taskbar` ne couvrant que la
   barre des tâches.
4. **Épingler `windows = "0.61"`**, la version dont dépend Tauri 2.11.5 : deux versions
   majeures de cette crate donnent deux types `HWND` **distincts**, et l'erreur de
   compilation parle alors de deux types de même nom.

### ⚠️ Correction du 2026-09-08 : l'« échelle 1 » du spike était un artefact

Le spike avait conclu « multi-DPI non observable sur cette machine, les deux écrans sont à
l'échelle 1 ». **C'est faux** : l'écran est à **125 %**, et le spike mesurait à travers la
virtualisation DPI de Windows.

Un processus qui n'a pas déclaré sa conscience du DPI se fait **mentir** : Windows lui rend
des pixels *logiques* en les présentant comme des pixels d'écran, et annonce 96 ppp quel
que soit le réglage réel. Mesuré côte à côte sur le même écran :

| | largeur | hauteur utile | échelle annoncée |
|---|---|---|---|
| processus **non** conscient du DPI | 1536 | 816 | 1 |
| processus conscient (`PER_MONITOR_AWARE_V2`) | **1920** | **1020** | **1,25** |

**Cinquième conséquence, donc, et la plus facile à oublier :**

5. **`SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)` doit être la PREMIÈRE
   instruction de `main`** — pas laissée à Tauri, qui ne la fixe qu'à la création de sa
   boucle d'événements, alors que la sonde système sert avant (diagnostic) et après
   (boucle 60 Hz). Sans appel explicite, les deux ne verraient pas le même bureau : la
   pire forme du bug, reproductible seulement à moitié. C'est fait dans
   `probe::win32::activer_conscience_dpi`.

Bonne nouvelle au passage : le multi-DPI n'est **plus un risque dormant**, la mise à
l'échelle du sprite étant exercée dès le premier lancement. Reste non éprouvé : **deux
écrans d'échelles différentes** — `FakeProbe::ecran_a_gauche_hidpi()` couvre l'hypothèse
côté tests.

### L'étape 1a est faite (2026-09-09)

Les 11 tâches du plan 1a sont exécutées, **126 tests**. Le personnage marche, court,
s'arrête, fait demi-tour, circule sur les deux écrans, s'attrape à la souris, **se lance**,
tombe et atterrit.

Quatre passes de correction ont suivi, toutes déclenchées par une observation à l'œil, et
toutes tranchées en lisant les **sources de Shimeji-ee** (dans `Downloads/shimejieesrc (2)`)
plutôt qu'en réglant à l'œil :

| Ce qui était faux | La vérité, et sa source |
|---|---|
| il marchait **à reculons** | les sprites sont dessinés vers la **gauche** — `Walk` a `Velocity="-2,0"` |
| `walk`, `run`, `sit`, `fall`, `land` : mauvaises frames | `conf/actions.xml`, relevé complet dans `docs/specs/2026-09-09-frames-shimeji.md` |
| ancre `[64,120]` | `[64,128]` — il était enfoncé de 8 px sous le sol |
| chute **2× trop rapide** | `Fall.java` a un **frottement de l'air** (`RESISTANCEY = 0,1`), vitesse limite 500 px/s |
| balancier = une animation | c'est un **ressort amorti** (`Dragged.java`), dont le retard choisit la frame |
| il tombait à la verticale | l'action `Thrown` **lance** avec `cursor.dx/dy` lissé |
| lancé à droite, il tombait tête à gauche | `Fall.java` : l'orientation suit la vitesse horizontale |

> **La leçon, à retenir pour les étapes suivantes :** tout ce qui avait été « réglé à
> l'œil » s'est révélé faux, et la spec §8.5 l'annonçait elle-même (« un point de
> départ »). Avant d'inventer une constante d'animation ou de physique, **la chercher
> dans le source**.

### L'étape 1b est faite (2026-09-09)

Les 6 tâches sont exécutées, un commit chacune, de `67dfb0c` à `77811fc`. Ce que ça change
au quotidien :

| | Ce qui existe maintenant |
|---|---|
| **Tray** | afficher / cacher, recharger, « Démarrer avec Windows », ouvrir le dossier, **Quitter** |
| **`config.json`** | échelle, vitesses, poids d'envies, dossier des personnages — **toutes les clés optionnelles**, l'absence de fichier n'est pas une erreur |
| **Démarrage auto** | clé `HKCU…\Run`, chemin **entre guillemets**, et la case du tray lit le **registre**, jamais la config |
| **Rechargement à chaud** | le manifeste est relu sans redémarrer ; en cas d'erreur **rien ne change** et le message est bruyant |
| **Release** | plus de console (`windows_subsystem`), sortie prouvée par `SHIMEJI_QUITTER_APRES` |

Trois choses apprises en exécutant, qui valent plus que le code :

1. **Un `config.json` parfaitement valide peut être rejeté** — `serde_json` refuse le
   **BOM UTF-8** que les éditeurs Windows ajoutent, avec le message trompeur
   `expected value at line 1 column 1`. D'où `config::lire_json`, qui retire le BOM et
   sert aussi au manifeste.
2. **`cargo test` ne reconstruit pas l'exe** — la première vérification du correctif du
   BOM a tourné sur un binaire plus vieux que la source, et a conclu à tort à un échec.
3. **Shimeji-ee a lui aussi des bugs**, et il ne faut pas les recopier : son test `d < 0`
   passe **avant** la bande neutre de ±10, qui devient inatteignable pour un retard
   négatif — le personnage finissait chaque glisser légèrement penché. Corrigé ici en
   testant la bande neutre d'abord, symétriquement.

> **Reste un clic que personne n'a fait :** que le menu du tray **dépêche** ses clics.
> Chaque action est prouvée autrement (`--demarrage etat`, `recharger.txt`,
> `SHIMEJI_CACHE`, `SHIMEJI_QUITTER_APRES`), mais le clic lui-même ne se script pas. Un
> clic sur « Quitter » couvre les cinq entrées : c'est le même gestionnaire.

### La prochaine action

**Écrire le plan de l'étape 2 — « Il réagit »**, dans `docs/plans/`. Le contenu est
cadré par l'annexe du plan 1b et par la spec §7 :

| Contenu | Fichiers |
|---|---|
| Les signaux : inactivité, appli au premier plan, heure, batterie, verrouillage | `signals.rs`, `probe/` étendu |
| Les modificateurs par application dans `config.json` | `config.rs` + `desire.rs` |
| `tirer_avec` branché sur les signaux — **le point d'entrée existe déjà** | `behavior/mod.rs`, une ligne |
| S'endormir, se réveiller, manger | `intention.rs` |
| Suspendre la boucle quand la session est verrouillée | `main.rs` |

> ⚠️ **Deux points de contenu, dont un déjà tranché.**
>
> 1. ~~L'animation de sommeil n'existe pas.~~ **Tranché le 2026-09-09** : il **s'assoit
>    (11) puis s'affale (21)**, et la pose est déclarée sous le nom `sleep` — le code ne
>    doit pas savoir qu'il s'agit d'un substitut. Détail et sprites vérifiés dans
>    `docs/specs/2026-09-09-frames-shimeji.md`.
> 2. **L'inactivité ne se mesure pas par capture de frappe** — c'est une exclusion
>    explicite du besoin. `GetLastInputInfo` rend un simple compteur de millisecondes,
>    sans jamais dire *quelle* touche : c'est la seule voie acceptable.

> **Et le rappel qui tient toute l'étape 2 :** décision n° 3, **les signaux biaisent, ils
> ne commandent pas**. Si « inactif 2 min » *déclenche* le sommeil, on a livré un
> afficheur d'état système déguisé en personnage. Il doit **multiplier un poids**.

Rappel de périmètre : l'étape 2 **n'a toujours pas** de plateformes de fenêtres. Le monde
n'expose que le sol de chaque écran, donc **pas de soustraction d'intervalles 1D** et
**pas de filtrage de fenêtres** — ils appartiennent à l'étape 4. YAGNI. Seul le signal
« appli au premier plan » touche aux fenêtres, et il ne demande que `GetForegroundWindow`,
pas un recensement.

### Ce que le spike a déjà établi

- **Le motif `AppHandle` + recherche de la fenêtre par label** est retenu pour `render.rs`,
  plutôt que de déplacer une `WebviewWindow` dans un thread : `WebviewWindow: Send` n'est
  pas garanti explicitement, et ce motif gère en prime la fenêtre fermée.
- Les appels `available_monitors`, `shadow`, `set_ignore_cursor_events`, `set_position`,
  `get_webview_window`, `handle()` et **`hwnd()`** existent tels qu'employés dans
  **tauri 2.11.5** (vérifiés dans les sources, emplacements consignés).
- Côté Win32, `GetWindowLongPtrW` / `SetWindowLongPtrW`, `GWL_EXSTYLE` et les constantes
  `WS_EX_*` sont vérifiés dans **windows 0.61.3**, *feature*
  `Win32_UI_WindowsAndMessaging`. Attention : `WINDOW_EX_STYLE` est un *newtype*, il faut
  `.0` puis `as isize` pour combiner les bits — détail et code dans le résultat du spike.
- La fenêtre de **128×128** correspond exactement à la taille des frames Shimeji.

Le projet vient d'un prototype d'extension VSCode (`../op`) où le personnage marchait en
bas de l'éditeur. Seules les **idées** en sont reprises ; ni le code, ni les sprites
Luffy/Zoro ne sont réutilisés.
