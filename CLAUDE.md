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

### État de la chaîne d'outils (relevé le 2026-09-08)

| Outil | État |
|---|---|
| `rustc` / `cargo` / `rustup` | ❌ absents — à installer, voir le plan de l'étape 0 |
| WebView2 | ✅ présent (152.0.4191.66) |
| Node | ⚠️ v14.17.0 — **délibérément inutilisé** |
| Composants C++ de Visual Studio | à confirmer |

> **Le piège Node.** Tauri s'installe par npm *ou* par cargo. La voie npm exigerait
> Node 18+, que cette machine n'a pas. Comme le front est statique (pas de bundler),
> **on n'a besoin de Node nulle part** : on passe par `cargo-tauri`, et le Node 14
> devient hors sujet. Ne pas « corriger » ça en installant Node — il n'y a rien à
> corriger.

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

**Aucun code n'existe.** Le design est complet, l'étape 0 est planifiée, la chaîne
d'outils n'est pas installée.

| Document | Contenu |
|---|---|
| `docs/specs/2026-09-08-design.md` | le design complet — le *pourquoi* de chaque décision |
| `docs/plans/2026-09-08-etape-0-spike-overlay.md` | le plan de l'étape 0, exécutable tel quel |
| `characters/blob/img/` | les 46 frames du personnage de test |

**La prochaine action est le plan de l'étape 0**, qui commence par installer Rust. Son
livrable n'est pas du code mais une **réponse écrite** dans
`docs/specs/2026-09-08-spike-0-resultat.md` : la fenêtre transparente tient-elle sur
cette machine ? Tant que cette réponse n'existe pas, **ne pas échafauder l'application**
— un résultat négatif renverrait la stack vers Electron, et le design resterait valable
à ~90 %.

Le plan de l'étape 1 s'écrira **après** cette réponse : les résultats du spike
(fluidité à 60 Hz, comportement DPI, attributs de fenêtre nécessaires) en conditionnent
plusieurs choix. La structure de fichiers de l'étape 1 est en revanche déjà verrouillée —
annexe A du plan de l'étape 0.

Le projet vient d'un prototype d'extension VSCode (`../op`) où le personnage marchait en
bas de l'éditeur. Seules les **idées** en sont reprises ; ni le code, ni les sprites
Luffy/Zoro ne sont réutilisés.
