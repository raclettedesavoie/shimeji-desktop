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
```

Le **spike de l'étape 0** est archivé dans `docs/spike-etape-0/` (`cargo run` y fonctionne
aussi). Voir son `README.md` pour ce qu'il prouve et la sonde de styles Win32 qui
l'accompagne. **Ne pas le faire évoluer vers l'application** : il lui manque délibérément
les deux styles étendus que son analyse a révélés nécessaires.

### ⚠️ Mesurer le CPU — vérification systématique

**Un desktop pet qui consomme se fait désinstaller.** Toute modification du chemin à
60 Hz — la boucle de `main.rs`, `render.rs`, la sonde — doit être suivie d'une **mesure**,
pas d'une intuition.

```powershell
$p = Get-Process -Name shimeji-desktop
$c = $p.CPU; Start-Sleep -Seconds 10; $p.Refresh()
"$([math]::Round((($p.CPU - $c) / 10) * 100, 1)) % d'un coeur"
```

**Chiffres de référence, mesurés le 2026-09-09 en build *debug* :**

| Configuration | CPU |
|---|---|
| application complète, `set_position` seul à 60 Hz | **~14 %** |
| application avec `set_size` en plus à chaque image | ~15 % |
| **spike de l'étape 0**, qui ne fait *que* déplacer une fenêtre à 60 Hz | **~18 %** |

> **La leçon de ces trois lignes :** le spike consomme **plus** que l'application
> complète. Le coût est donc **inhérent au déplacement d'une fenêtre en couche à 60 Hz** —
> la composition alpha que Windows refait à chaque déplacement —, et **non** dans notre
> physique, notre comportement ou notre rendu. Chercher l'optimisation de ce côté-là
> serait chercher au mauvais endroit.
>
> Corollaire méthodologique : **comparer au spike avant d'optimiser quoi que ce soit.**
> C'est le témoin, et c'est une raison de plus de l'avoir archivé plutôt que supprimé.

⬜ **Non encore mesuré : le build `release`.** Le profil release est réglé pour la taille
(`opt-level = "z"`, LTO) et non pour la vitesse ; l'écart avec debug reste à établir. À
faire **avant** de conclure quoi que ce soit sur la consommation du produit livré.

Trois pistes d'optimisation identifiées mais **non appliquées**, faute d'avoir mesuré le
release d'abord — les inscrire ici évite de les redécouvrir :

1. **Ne pas appeler `set_position` quand la position arrondie n'a pas changé.** À l'arrêt
   (~3 tirages sur 10), ce sont 60 appels par seconde entièrement gratuits à supprimer.
2. **Descendre la cadence quand rien ne bouge** — 60 Hz est nécessaire au mouvement, pas
   à l'immobilité.
3. **Suspendre la boucle quand la session est verrouillée** — déjà prévu comme réflexe à
   l'étape 2, et c'est aussi une optimisation.

**`cargo run` suffit — pas besoin de `cargo tauri dev`.** Le front étant statique, les
assets sont embarqués dans le binaire à la compilation. Le CLI Tauri ne devient nécessaire
que pour produire un installateur.

> ⚠️ **Pour arrêter un personnage : `Ctrl+C` dans le terminal.** La fenêtre est
> volontairement non focalisable, sans bordure, hors taskbar et hors Alt+Tab — elle ne
> peut donc **pas** se fermer normalement. C'est le comportement voulu, mais il se
> retourne contre soi au moment de quitter. En secours :
> `Stop-Process -Name spike-overlay`. L'application de l'étape 1 aura une entrée « Quitter »
> dans le tray, ce qui règle le problème pour de bon.

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
| 0 | Validation technique | fenêtre transparente, sans bordure, au premier plan, hors taskbar, clics traversants, PNG déplacé à 60 Hz sur 2 écrans |
| 1 | **Il vit sur le sol** | marche, court, s'arrête, demi-tour, tous les écrans ; attrapable et il tombe ; tray, démarrage auto |
| 2 | **Il réagit** | s'endort quand on part, se réveille au retour, mange à midi |
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

**L'étape 0 est terminée : les 7 propriétés sont vertes, la stack Tauri est validée.**
L'application, elle, n'existe pas encore — aucune ligne de physique n'est écrite.

| Où | Contenu |
|---|---|
| `docs/specs/2026-09-08-design.md` | le design complet — le *pourquoi* de chaque décision |
| `docs/plans/2026-09-08-etape-0-spike-overlay.md` | le plan de l'étape 0, **soldé** ; **annexe A = structure de fichiers verrouillée pour l'étape 1** |
| `docs/specs/2026-09-08-spike-0-resultat.md` | **le résultat de l'étape 0** : grille remplie, décision de stack, API vérifiées, et les 2 découvertes à appliquer |
| `docs/plans/2026-09-08-etape-1a-il-vit-sur-le-sol.md` | le plan de l'étape 1a, **exécuté** — 11 tâches |
| `docs/plans/2026-09-09-etape-1b-tour-du-proprietaire.md` | **le plan à exécuter** : tray, config, démarrage auto, rechargement à chaud, CPU |
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

### La prochaine action

**Exécuter le plan de l'étape 1b**, `docs/plans/2026-09-09-etape-1b-tour-du-proprietaire.md`
— 6 tâches, dans l'ordre, chacune se fermant sur un commit.

> ⚠️ **Tant que la Tâche 1 de 1b n'est pas faite, l'application ne se ferme que par
> `Stop-Process -Name shimeji-desktop`.** La fenêtre est sans bordure, non focalisable,
> hors taskbar et hors Alt+Tab : c'est voulu, et ça se retourne contre soi au moment de
> quitter. C'est pour ça que le tray vient en premier dans ce plan, avant la config qui
> est pourtant plus structurante.

La **Tâche 6 de 1b** porte la mesure du `release` et les deux optimisations de CPU
identifiées — dans cet ordre, la mesure d'abord. On a déjà fait l'expérience de l'inverse
à l'étape 1a, en accusant `set_size` à tort.

Rappel de périmètre : ni 1a ni 1b **n'ont** de plateformes de fenêtres. Le monde n'expose
que le sol de chaque écran, donc **pas de soustraction d'intervalles 1D** et **pas de
filtrage de fenêtres** — ces deux morceaux appartiennent à l'étape 4. YAGNI.

> **Une inconnue de contenu attend l'étape 2 :** Shimeji-ee **n'a aucune animation de
> sommeil**, alors que l'étape 2 promet « il s'endort quand on part ». Les trois issues
> possibles et la recommandation sont dans `docs/specs/2026-09-09-frames-shimeji.md`.

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
