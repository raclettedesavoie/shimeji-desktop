# Discussion de conception — 2026-09-08

Transcription de la session qui a produit le besoin, le design, la spec et le plan de
l'étape 0. Extraite du transcript de la session Claude Code : seuls les messages et les
décisions sont conservés ; les appels d'outils, leurs résultats et le raisonnement interne
sont écartés.

**Le digest décisionnel est dans `2026-09-08-journal-decisions.md`** — plus court, et
organisé par décision plutôt que chronologiquement.

---

## 👤 Alexis

Hey j'avais fais cette petite extension sur vscode mais finalement j'aimerai faire un truc similaire sur mon bureau windows j'aimerai animer des personnages animes sur mon bureau genre un luffy qui marche, mange, cours, dors, etc...

---

## 🤖 Claude

J'utilise la skill **brainstorming** pour cadrer ça. Vu que c'est un nouveau projet (nouvelle plateforme, nouvelle architecture), je le classe **architectural** : questions → approches → design → spec. Dis-moi si tu veux quelque chose de plus léger.

Je commence par regarder ce que tu as déjà.

J'ai lu le code. Bonne nouvelle : **le cœur est déjà là**. Ton [overlay.js](overlay.js) crée déjà une `BrowserWindow` Electron transparente, `alwaysOnTop`, `skipTaskbar`, `focusable: false`, click-through — c'est exactement ce qu'il faut pour un desktop pet Windows. Et [overlay.html](overlay.html) contient déjà un moteur de sprites (atlas + frames + one-shots + flip).

Ce qui change en passant sur le bureau :
- **On enlève** tout le couplage VSCode : le webview sensor, le fichier `.luffy-layout`, les zones editor/sidebar/panel, l'IPC par fichiers (`.luffy-anim`, `.luffy-speed`…).
- **On ajoute** ce qui fait un vrai pet : une machine à états de comportement (c'est ça qui donne « mange, dort, court »), un tray icon, l'autostart Windows, la gestion multi-écrans, et l'interaction souris (drag & drop du perso).

Je vois aussi le dossier [zoro/](zoro/) et plein de nouvelles planches Luffy (run, jump, grab, attack…) — donc le multi-perso et un vrai catalogue d'animations font partie de l'idée.

Avant de proposer des approches, une question de fond :

---

### ❓ Question à choix

**C'est quoi le cœur de l'expérience que tu veux ? Ça détermine toute l'architecture.**

Options proposées : Ambiant / décoratif · Réactif à ton PC · Tamagotchi · Ambiant d'abord, extensible

### ✅ Décision d'Alexis

"C'est quoi le cœur de l'expérience que tu veux ? Ça détermine toute l'architecture."="Réactif à ton PC".

## 🤖 Claude

Parfait, **réactif au PC** — c'est le plus fun et ça reste faisable. Prochaine question, la plus déterminante pour l'effort : **quels signaux** il écoute.

J'ai classé par coût technique réel sur Windows, parce que ça change beaucoup la charge de travail :

---

### ❓ Question à choix

**Quels signaux système Luffy doit-il capter ? (choix multiples — prends ce qui te fait vraiment plaisir, on peut en ajouter plus tard)**

Options proposées : Le pack gratuit · + Appli au premier plan · + RAM et disque · + Frappe clavier en direct

### ✅ Décision d'Alexis

"Quels signaux système Luffy doit-il capter ? (choix multiples — prends ce qui te fait vraiment plaisir, on peut en ajouter plus tard)"="+ Frappe clavier en direct".

## 🤖 Claude

Note : tu n'as coché que la frappe clavier. Je pars du principe que **le pack gratuit est inclus d'office** (dormir quand tu es inactif, c'est la brique la plus emblématique d'un pet réactif, et ça coûte quasi rien) — dis-moi si tu ne le veux pas.

Sur le clavier, j'ai creusé et il y a un **vrai moyen terme** que j'avais pas mentionné :

---

### ❓ Question à choix

**Comment mesurer ton activité de frappe ? Le rendu visuel est quasi identique, mais les implications techniques sont très différentes.**

Options proposées : GetLastInputInfo (recommandé) · Vrai hook clavier global · Idle Electron seulement

### ✅ Décision d'Alexis

"Comment mesurer ton activité de frappe ? Le rendu visuel est quasi identique, mais les implications techniques sont très différentes."="Idle Electron seulement".

## 🤖 Claude

Noté : **idle Electron seulement** (zéro dépendance) **+ fenêtre active**. Donc le jeu de signaux se fixe à :

| Signal | Source | Effet type |
|---|---|---|
| Inactivité | `powerMonitor.getSystemIdleTime()` | s'assoit → s'endort |
| Fenêtre active | à décider (voir plus bas) | comportement selon l'appli |
| Charge CPU | `os.cpus()` en delta | court / s'épuise |
| Heure | `Date` | mange à midi, dort la nuit |
| Verrouillage / veille | événements `powerMonitor` | disparaît proprement |

Pour la fenêtre active, il n'y a pas d'équivalent « déjà dans Electron ». Je recommanderai **`koffi`** (FFI à binaires précompilés, aucune compilation, aucun hook — on appelle juste `GetForegroundWindow`) plutôt que `get-windows` (module natif à compiler) ou un spawn PowerShell chaque seconde. Je détaillerai dans le design.

Dernière question de cadrage, et c'est la plus coûteuse en effort :

---

### ❓ Question à choix

**Où Luffy a-t-il le droit de vivre sur ton bureau ? C'est le fork qui change le plus le volume de travail.**

Options proposées : Sol du bureau (recommandé) · Style Shimeji · Sol d'abord, fenêtres après

### ✅ Décision d'Alexis

"Où Luffy a-t-il le droit de vivre sur ton bureau ? C'est le fork qui change le plus le volume de travail."="Style Shimeji".

## 🤖 Claude

**Shimeji** donc. Un mot d'honnêteté avant : c'est bien le chemin le plus long des trois (énumération de fenêtres en continu, carte de collisions qui bouge, et surtout des animations d'escalade/suspension que tes sprites GBA n'ont probablement pas). J'y vais, mais je vais structurer le design pour que tu aies un perso **qui marche sur le sol dès les premières étapes**, l'escalade venant se brancher dessus — sinon tu n'as rien à regarder pendant longtemps.

Bonne surprise en revanche : **ton choix « fenêtre active » devient quasi gratuit**. L'énumération de fenêtres dont Shimeji a besoin te donne `GetForegroundWindow` dans le même appel FFI. Les deux features partagent toute leur infrastructure.

---

## Trois approches

**A — Forker l'app Electron existante** *(recommandé)*
On garde [overlay.js](overlay.js) et [overlay.html](overlay.html), on retire le VSCode (sensor webview, zones, IPC par fichiers `.luffy-*` → vrai `ipcMain`/`ipcRenderer`), et on ajoute trois couches neuves : les bindings Windows en koffi, la source de plateformes, et la machine à états de comportement. JS simple, aucun build step, aucune compilation native.
*Pour :* tu as un pet qui marche sur ton bureau très vite, et tout ton travail sur les sprites reste valable. *Contre :* pas de types, donc la machine à états et la physique demandent de la discipline.

**B — Réécrire le renderer en canvas + TypeScript + bundler**
Vraie boucle de jeu sur `<canvas>`, manifeste de sprites typé, structure façon ECS.
*Pour :* plus tenable à 2 persos × 15 animations, et le canvas gère mieux 20+ sprites. *Contre :* toolchain à monter avant de voir le premier pixel, et tu n'as pas de problème de perf à résoudre — un seul perso en DOM tient les 60fps sans effort.

**C — Natif (C# / WinUI ou Rust)**
*Pour :* intégration fenêtres parfaite, empreinte mémoire ridicule face à Electron (~80 Mo). *Contre :* tu jettes le moteur de sprites et tout ce que tu connais du projet. Je ne le recommande pas.

**Je recommande A, avec deux emprunts à B** — les seuls morceaux de structure qui paient vraiment ici :

1. **Un manifeste de sprites déclaratif** (`characters/luffy/manifest.json`) au lieu des rectangles de frames codés en dur dans [overlay.html:97-190](overlay.html#L97-L190). C'est *le* point de rupture : écrire à la main les coordonnées de 15 animations × 2 persos, c'est là que le projet meurt. Avec un manifeste, ajouter « mange » c'est éditer du JSON.
2. **La couche « monde » derrière une interface** (une liste de plateformes rectangulaires) — pour que la physique et les comportements ne sachent jamais si une plateforme est le sol ou la barre de titre de VSCode. C'est ce qui permet de livrer le sol d'abord et de brancher les fenêtres ensuite sans rien réécrire.

Pas de TypeScript, pas de bundler, pas de canvas — YAGNI.

---

Ça te va comme direction ? Si oui je détaille le design section par section (le modèle d'accroche mur/plafond, la machine à états, la couche Windows, le manifeste, et la stratégie multi-écran qui est le piège le moins visible).

---

## 👤 Alexis

Si j'avais encore rien fait qu'est ce que tu recommenderais ?*

---

## 🤖 Claude

Bonne question, et la réponse honnête n'est pas « la même chose ». Sans code existant, je ne partirais pas sur Electron.

## Pourquoi

Ce projet a deux moitiés qui tirent dans des directions opposées :

- **Moitié « perso »** — sprites, animations, machine à états, physique. Les technos web sont excellentes là-dessus (CSS, `image-rendering: pixelated`, ton moteur actuel tient déjà debout).
- **Moitié « Shimeji »** — `EnumWindows`, `DwmGetWindowAttribute`, ordre de plan, `GetForegroundWindow`. C'est du win32 brut, et c'est là que vit toute la difficulté réelle de ce que tu as choisi.

Electron est bon sur la première et **mauvais sur la seconde** : tu accèdes à win32 par FFI, donc tu décris à la main en JS des structs comme `RECT` et `WINDOWINFO`. Une taille de struct fausse ne te donne pas une erreur, elle te donne un segfault. Et tu vas faire ça sur l'API la plus centrale du projet.

## Ce que je recommanderais greenfield : Tauri v2

Backend Rust, front HTML/CSS. Le point clé : la crate `windows` (officielle Microsoft) expose `EnumWindows`, `DwmGetWindowAttribute`, `GetForegroundWindow` en **bindings typés** — exactement les APIs dont ce projet dépend, avec le compilateur qui vérifie tes structs. Et tu gardes tout le travail sprites en CSS.

| | Electron | Tauri v2 | C# WPF | Godot 4 |
|---|---|---|---|---|
| Sprites / animation | ★★★ CSS | ★★★ CSS | ★★ à la main | ★★★ natif |
| win32 fenêtres | ★ FFI à la main | ★★★ typé | ★★★ P/Invoke | ★ GDExtension requise |
| Overlay transparent | ★★★ éprouvé | ★★ ça marche, moins balisé | ★★★ | ★★ |
| Poids | ~150 Mo | ~10 Mo | ~30 Mo | ~60 Mo |
| Langage pour toi | connu | à apprendre | à apprendre | à apprendre |

**C# WPF** est le second sérieux : P/Invoke est de première classe, tray et autostart sont triviaux, ~30 Mo. Tu perds juste CSS pour l'animation.

**Godot** est piégeux malgré les apparences : c'est le meilleur des quatre pour animer un perso (state machine d'animation, découpe d'atlas à la souris, drag & drop intégré)… et le pire pour énumérer des fenêtres Windows. Or c'est ça, ton besoin.

## Ce que je ferais quand même à ta place

Tu écris du JS, ce projet est un projet plaisir, et personne ne finit un moteur de physique Shimeji **en apprenant Rust en même temps**. La meilleure archi que tu n'achèves pas vaut moins que celle où tu es rapide.

Donc : **je reste sur Electron**, en isolant la douleur FFI dans un seul module d'environ 150 lignes (`win32/windows.js`) écrit une fois, testé une fois, et plus jamais touché. Le reste du projet ne voit qu'une fonction `listPlatforms()`.

Une remarque au passage, qui compte plus que le choix de techno : **une fenêtre par personnage** (petite, déplacée par le process principal) plutôt qu'un overlay plein écran comme fait ton code actuel. C'est ce que fait le vrai Shimeji, ça supprime le besoin de click-through, et ça allège énormément la composition. J'y reviens dans le design.

---

### ❓ Question à choix

**On part sur quoi, alors ?**

Options proposées : Electron (recommandé) · Tauri v2 (Rust) · C# WPF · Electron d'abord, porté ensuite

### ✅ Décision d'Alexis

"On part sur quoi, alors ?"="Tauri v2 (Rust)".

## 🤖 Claude

**Tauri v2** — bon choix si apprendre Rust fait partie du plaisir. Ça change une chose importante que je vais expliquer tout de suite, parce que ça touche à l'endroit où vit la logique.

Et une bonne nouvelle d'abord : **tes PNG sont réutilisables à 100 %**, et les rectangles de frames dans [overlay.html:97-190](overlay.html#L97-L190) sont de la donnée que je vais juste déplacer dans des manifestes. Rien de ce travail n'est perdu.

---

# Design — Section 1 : le modèle de fenêtre

C'est la décision qui contraint tout le reste, donc je commence par là.

Ton code actuel utilise **une fenêtre transparente plein écran** avec du click-through ([overlay.js:96-116](overlay.js#L96-L116)). Pour du Shimeji multi-écran, je recommande d'abandonner ce modèle pour **une petite fenêtre par personnage**, déplacée par Rust — c'est ce que fait le vrai Shimeji.

```
Modèle actuel (plein écran)          Modèle proposé (une fenêtre / perso)

┌═══════════════════════════┐        ┌───────────────────────────┐
║  overlay transparent      ║        │                     ┌───┐ │
║  1920×1080 always-on-top  ║        │   ┌─────────┐       │ ᵒ─│ │ ← fenêtre
║   ┌─────────┐             ║        │   │ VSCode  │       │/|\│ │   128×128
║   │ VSCode  │      ᵒ─     ║        │   └─────────┘       └───┘ │
║   └─────────┘     /|\     ║        │  ┌───┐                    │
║═══════════════════════════║        │  │ᵒ─ │ ← 2e perso         │
└═══════════════════════════┘        │  └───┘                    │
                                     └───────────────────────────┘
1 couche au-dessus de tout           N fenêtres minuscules
```

**Pourquoi ce changement :**

- **Le multi-écran devient gratuit.** Une seule fenêtre plein écran ne peut pas couvrir proprement deux moniteurs de DPI différents — c'est le piège le moins visible de ce projet et il est structurel. Avec une fenêtre par perso, traverser d'un écran à l'autre c'est juste écrire une position.
- **Plus de couche always-on-top au-dessus de tout ton bureau**, qui coûte en composition et peut se battre avec les jeux en plein écran exclusif.
- **L'ordre de plan par personnage devient naturel** — un perso peut passer derrière une fenêtre, l'autre devant.

**Le coût :** tu perds le hit-testing pixel-parfait gratuit du DOM. La boîte de 128×128 attraperait les clics dans ses zones transparentes. Solution : Rust garde `set_ignore_cursor_events(true)` en permanence, sonde `GetCursorPos` à ~30 Hz (c'est quasi gratuit) et ne repasse à `false` que quand le curseur entre dans la **hitbox serrée** déclarée par l'animation courante. Simple, et entièrement côté Rust.

---

## La conséquence : où vit la logique

Avec ce modèle, la position est appliquée par Rust (`window.set_position`). Si la physique vivait en JS comme aujourd'hui, tu paierais un aller-retour IPC **60 fois par seconde par personnage** pour rapatrier une position que Rust doit de toute façon appliquer. Donc :

| Couche | Où | Rôle |
|---|---|---|
| Énumération fenêtres, signaux, tray, config | **Rust** | possède l'OS |
| Physique, accroches, machine à états | **Rust** | a besoin de la carte des plateformes, que Rust possède |
| Dessin du sprite | **Webview** | ~60 lignes de JS : reçoit `{atlas, sx, sy, w, h, flip}`, pose un `background-position` |

Le webview devient volontairement bête. Ça met tout l'intéressant en Rust — ce qui est cohérent avec ce que tu veux tirer du projet — et ça garde un seul endroit qui détient l'état.

**À noter honnêtement :** j'avais mis ★★ à Tauri sur « overlay transparent » et je maintiens. Donc l'étape 1 du plan sera un **spike jetable** : une fenêtre Tauri transparente, sans bordure, always-on-top, hors taskbar, avec un PNG qui bouge, repositionnée à 60 Hz sur tes deux écrans. Si ça coince sur ton Win11, on le sait avant d'avoir écrit une ligne de physique — pas après.

---

Est-ce que cette section te paraît juste ? En particulier le passage à une fenêtre par personnage, et le fait que la logique de comportement passe en Rust plutôt qu'en JS — c'est le vrai renversement par rapport à ce que tu as aujourd'hui.

Si c'est bon, la section 2 attaque **le modèle d'accroche** (sol / mur / plafond / chute / porté) et la carte des plateformes, qui est le cœur du Shimeji.

---

## 👤 Alexis

Avant toute chose fais moi un résumé du besoin métier sans les spécification techniques stp

---

## 🤖 Claude

Bonne idée de faire ce point avant d'aller plus loin. Voilà le besoin, sans une ligne de technique.

---

# Luffy sur le bureau — le besoin

## L'idée en une phrase

Des personnages d'anime en pixel-art **vivent leur vie sur mon bureau Windows** : ils se déplacent, s'occupent, se reposent, et **réagissent à ce que je fais sur ma machine**.

## Ce que le personnage fait de sa vie

Il n'attend pas mes ordres, il **enchaîne des activités tout seul**, avec une part d'aléatoire pour qu'on ne devine pas la suite :

- se déplacer — marcher, courir, faire une roulade
- s'arrêter — rester planté, s'asseoir, regarder autour
- se reposer — s'allonger, dormir
- manger
- s'amuser — sauter, lancer une attaque, s'entraîner

L'important n'est pas la liste, c'est **qu'il ait l'air vivant** : jamais figé, jamais parfaitement prévisible.

## Où il vit

Il n'est pas coincé en bas de l'écran. **Il explore.**

- il marche sur le bureau, au-dessus de la barre des tâches
- il **grimpe le long des bords de mes fenêtres** et **s'assoit sur leur barre de titre**
- il escalade les bords de l'écran, se suspend au plafond
- si la fenêtre sur laquelle il était disparaît, **il tombe** — et il atterrit
- il circule sur **tous mes écrans**, pas seulement le principal

C'est ça qui fait la différence entre une décoration et un personnage qui habite vraiment mon bureau.

## Ce à quoi il réagit

C'est le cœur de l'expérience : mon PC influence son humeur.

| Quand… | …il |
|---|---|
| je ne touche plus à rien | s'assoit, puis s'endort |
| je reviens | se réveille |
| ma machine rame, le CPU chauffe | s'agite, court, s'épuise |
| il est midi | mange |
| il est tard le soir | traîne, dort davantage |
| je passe sur une autre application | change d'attitude selon l'appli — il s'entraîne quand je code, il traîne quand je navigue |
| ma batterie est faible | fatigue |
| je verrouille ma session | disparaît proprement |

## Comment j'interagis avec lui

- je peux **l'attraper à la souris et le lâcher** n'importe où — il tombe et se rattrape
- je peux **cliquer dessus** pour le faire réagir
- je peux le **cacher ou le rappeler** à tout moment, sans quitter quoi que ce soit
- il ne me gêne jamais : il ne vole pas le focus, il n'apparaît pas dans la barre des tâches, mes clics le traversent

## Le casting

Plusieurs personnages, **Luffy en premier, Zoro ensuite**. Ajouter un personnage doit rester une opération de contenu — fournir ses images et décrire ses animations — **sans retoucher le comportement**.

## Ce que ce n'est explicitement pas

Ces choix sont déjà tranchés, autant les écrire :

- ❌ **Pas un Tamagotchi** — pas de faim ni d'énergie à surveiller, aucune obligation, il ne meurt pas si je l'ignore
- ❌ **Pas de capture de frappe** — on ne lit aucune touche, on sait seulement si je suis actif ou non
- ❌ **Pas un assistant** — il ne me notifie rien, ne me rappelle rien, n'a aucune utilité productive
- ❌ **Pas un jeu** — pas de score, pas de progression

C'est du plaisir visuel, et rien d'autre. Toute idée qui ajoute une obligation est hors sujet.

---

## Deux points de contenu à savoir

Indépendamment de toute technique, deux trous à combler côté **images** :

1. **« Manger » et « dormir » n'existent pas encore** dans tes planches Luffy. Tu as marche, course, saut, roulade et beaucoup d'attaques — mais pas de repas ni de sommeil. Ce sont pourtant deux comportements que tu as cités en premier.
2. **L'escalade non plus.** Un perso qui grimpe le long d'une fenêtre a besoin d'images où il est accroché à une paroi. Chez Zoro tu as un « coucher » exploitable pour le sommeil, mais rien pour l'escalade.

Le comportement saura s'en passer proprement — un personnage sans animation d'escalade ne grimpera simplement jamais — mais deux des activités que tu voulais voir dépendent d'images à trouver ou à dessiner.

---

## Reste à trancher (fonctionnel)

1. Les deux persos sont-ils **à l'écran en même temps**, et si oui **se remarquent-ils** l'un l'autre ?
2. Quand il réagit à mon appli active, est-ce qu'il **change juste d'attitude**, ou est-ce qu'il **va physiquement vers cette fenêtre** pour s'asseoir dessus ?
3. Est-ce qu'il **démarre avec Windows** ?
4. Ton code actuel a des **bulles de dialogue** (« VIANDE!! 🍖 », « Je serai le Roi des Pirates! ») — on les garde ?

Dis-moi si ce résumé correspond bien à ce que tu as en tête, et réponds aux quatre points quand tu veux — ensuite je reprends le design technique là où je l'ai laissé.

---

## 👤 Alexis

ma machine rame, le CPU chauffe	s'agite, court, s'épuise non ca je veux pas forcément
Les deux persos sont-ils à l'écran en même temps, et si oui se remarquent-ils l'un l'autre ?
oui et pourquoi pas 
2.Quand il réagit à mon appli active, est-ce qu'il change juste d'attitude, ou est-ce qu'il va physiquement vers cette fenêtre pour s'asseoir dessus ?
pourquoi pas se déplacer physiquement 
3 oui 
4) non je pense pas

---

## 🤖 Claude

Compris, je mets à jour. Les deltas :

- ❌ **CPU retiré** — il ne réagit plus à la charge machine
- ✅ **Les deux persos cohabitent** et se remarquent
- ✅ **Il se déplace physiquement** vers la fenêtre de l'appli active pour s'y installer
- ✅ **Démarre avec Windows**
- ❌ **Bulles de dialogue retirées**

---

## Deux de tes réponses changent la taille du projet

Autant que tu le saches maintenant, en termes simples :

**« Il va physiquement vers la fenêtre active »** — ça transforme le personnage. Jusqu'ici il déambulait au hasard : il n'avait aucune intention, juste des envies. Là, il doit se **fixer une destination et savoir y aller** : quelle fenêtre, où elle est, comment l'atteindre depuis là où il est, quoi faire si elle bouge en route ou se ferme avant qu'il arrive. C'est le passage d'un personnage qui erre à un personnage qui **poursuit un but** — et c'est un morceau à part entière.

**« Ils se remarquent »** — même chose. Deux persos indépendants, c'est juste deux fois le même. Deux persos qui se remarquent, ça veut dire qu'ils s'observent, se rapprochent, réagissent l'un à l'autre — et vu qui c'est, Luffy et Zoro qui se chamaillent, ça s'écrit tout seul. Mais c'est une couche de comportement en plus, pas une copie.

Rien de tout ça n'est déraisonnable. Simplement, avec l'escalade des fenêtres déjà au programme, **le comportement devient la plus grosse partie du projet** — plus grosse que la partie système. D'où le point suivant.

---

## L'ordre dans lequel tu verras les choses arriver

Je veux éviter que tu attendes longtemps devant rien. Chaque étape est **agréable en elle-même**, et je place volontairement en fin ce qui dépend d'images que tu n'as pas encore.

| | Étape | Ce que tu as à la fin |
|---|---|---|
| 0 | Validation technique | rien de joli — juste la preuve que l'affichage tient |
| 1 | **Luffy vit sur le sol** | il marche, court, s'arrête, fait demi-tour, sur tous tes écrans. Tu peux l'attraper et le lâcher, il tombe. Il démarre avec Windows, tu le caches quand tu veux. **C'est déjà un desktop pet complet.** |
| 2 | **Il te réagit** | il s'endort quand tu pars, se réveille quand tu reviens, mange à midi |
| 3 | **Zoro arrive** | ils cohabitent et se remarquent |
| 4 | **Il grimpe** | les bords de fenêtres, les barres de titre, il tombe quand la fenêtre se ferme |
| 5 | **Il te suit** | il se déplace vers l'appli que tu utilises |

J'ai mis **Zoro avant l'escalade** exprès : Zoro ne demande aucune image nouvelle (tu as déjà son idle, sa course, ses attaques, son « coucher »), alors que l'escalade en demande. Si tu attends d'avoir les images d'escalade, tu bloques ; là tu ne bloques jamais.

**Le trou de contenu s'est agrandi, du coup.** Il te manque des images pour : **manger** (étape 2), **dormir** pour Luffy (étape 2 — Zoro l'a), et **s'accrocher à une paroi** (étape 4).

---

### ❓ Question à choix

**Cet ordre te convient, ou tu veux voir l'escalade plus tôt ?**

Options proposées : Cet ordre me va · L'escalade avant Zoro · Escalade avant même les réactions · Le strict minimum d'abord

### ✅ Décision d'Alexis

"Cet ordre te convient, ou tu veux voir l'escalade plus tôt ?"="Cet ordre me va".

## 🤖 Claude

Parfait, on garde cet ordre. Retour au design.

Petite note au passage : ta réponse « ils se remarquent » **renforce** la Section 1. Avec une fenêtre par personnage, les persos sont dans des affichages séparés et ne peuvent pas se voir entre eux — mais comme toute la logique est côté Rust, c'est Rust qui détient les deux persos et leurs positions. Le comportement social tombe donc pile au bon endroit. Si la logique était restée en JS, il aurait fallu inventer un canal entre les deux fenêtres.

---

# Design — Section 2 : le monde

C'est le cœur du Shimeji. Une seule idée porte tout : **une plateforme**.

Tout ce sur quoi un perso peut se tenir, s'accrocher ou se suspendre est un rectangle avec une face utilisable. Il n'existe que deux sources de rectangles :

```
                    ┌─ plafond de l'écran ─┐
        ╔═══════════╧══════════════════════╧═══════════╗
        ║                                              ║
   mur  ║      ┌───── dessus (barre de titre) ──┐      ║  mur
  gauche║      │                                │      ║ droit
        ║  mur │           VSCode               │ mur  ║
        ║gauche│                                │droit ║
        ║      └───── dessous (suspension) ─────┘      ║
        ║                                              ║
        ╚══════════════════ sol ═══════════════════════╝
              (au-dessus de la barre des tâches)

   Source 1 : chaque écran   → sol, 2 murs, plafond
   Source 2 : chaque fenêtre → dessus, 2 murs, dessous
```

Le personnage, la physique et les comportements **ne savent jamais** si la plateforme sous leurs pieds est le bas de ton écran ou la barre de titre de VSCode. C'est ce qui permet de livrer l'étape 1 (sol seul) et de brancher l'étape 4 (fenêtres) sans rien réécrire.

## La décision qui désamorce le pire bug du projet

Tes fenêtres bougent, se redimensionnent et se ferment **en permanence**. Un perso accroché à l'une d'elles est donc perpétuellement sur le point de perdre son support. Si on gère ça mal, le projet passe sa vie à corriger des cas particuliers.

La réponse tient en une phrase : **un perso accroché ne stocke jamais sa position absolue.**

Il stocke `(quelle plateforme, quelle face, à quelle distance du bord)`. Sa position à l'écran est **recalculée** à chaque image depuis le rectangle courant de sa plateforme.

Cette seule décision rend gratuits tous les cas qui feraient mal autrement :

| Ce qui arrive | Ce qu'il faut coder |
|---|---|
| tu déplaces la fenêtre | **rien** — il voyage avec, naturellement |
| tu la redimensionnes | **rien** — il reste à sa distance du bord |
| elle rétrécit sous ses pieds | un test : ma distance dépasse le bord → je tombe |
| tu la fermes ou la minimises | un test : ma plateforme n'existe plus → je tombe |

Un perso qui reste assis sur ta barre de titre pendant que tu balades la fenêtre, c'est *le* moment qui fait sourire avec un Shimeji. Ici, c'est une conséquence gratuite du modèle, pas une feature à écrire.

## L'occlusion : ne jamais se tenir sur un bord invisible

Il est assis sur VSCode. Tu passes sur Chrome, qui recouvre VSCode. Sans traitement, il se retrouve debout sur un bord qu'on ne voit plus.

Je recommande de le garder **toujours visible au premier plan** (c'est un animal de compagnie, tu veux le voir), mais de **ne jamais exposer comme plateforme un bord recouvert**. Et c'est étonnamment simple, parce qu'un bord est un segment : il suffit de lui retirer les fenêtres situées au-dessus.

```
bord haut de VSCode    ├────────────────────────────┤
Chrome par-dessus                ▓▓▓▓▓▓▓▓▓▓▓
                                 ↓ soustraction
plateformes réelles    ├────────┤           ├───────┤
```

De la soustraction d'intervalles en 1D — quelques dizaines de lignes, pas de géométrie compliquée. Il ne peut alors physiquement pas s'asseoir sur du vide.

## Trois traps que je préfère nommer maintenant

1. **Le rectangle que Windows te donne n'est pas celui que tu vois.** Une fenêtre Win10/11 déclare une bordure de redimensionnement invisible d'environ 7 px de chaque côté. Si on l'utilise telle quelle, le perso est assis 7 px au-dessus de la barre de titre, dans le vide — subtilement faux, et très visible sur un pixel-art. Il faut demander les bornes **visuelles** au compositeur. C'est le détail n°1 qui sépare un Shimeji qui a l'air juste d'un Shimeji qui a l'air cassé.
2. **Windows 11 est plein de fenêtres fantômes.** Des applications modernes déclarent des fenêtres invisibles qui se présentent pourtant comme visibles. Sans filtre, ton perso s'assoit sur des rectangles qui n'existent pas à l'écran. Il faut écarter ces fenêtres masquées, les minimisées, les fenêtres outils, les minuscules — et **les fenêtres de tes propres persos**, sinon Luffy s'assoit sur Zoro.
3. **Le sol, c'est la zone de travail, pas l'écran.** Sinon il marche sous ta barre des tâches.

## Trois horloges, pas une

Recenser toutes les fenêtres 60 fois par seconde serait du gaspillage. Mais rafraîchir trop lentement fait traîner le perso derrière une fenêtre que tu déplaces. D'où trois rythmes :

| Rythme | Quoi | Pourquoi |
|---|---|---|
| **60 Hz** | physique, dessin, **et la seule plateforme occupée** | interroger *une* fenêtre est quasi gratuit → il colle parfaitement à la fenêtre que tu déplaces |
| **~8 Hz** | recensement complet, filtres, occlusion | une fenêtre qui apparaît est détectée en ~125 ms : imperceptible |
| **~2 Hz** | inactivité, appli active, heure, batterie | aucun de ces signaux ne bouge vite |

C'est le compromis qui rend l'ensemble fluide sans faire chauffer ton CPU pour un jouet.

---

Cette section te paraît juste ? Le point sur lequel j'aimerais surtout ton avis : **garder le perso toujours au premier plan** en filtrant les bords recouverts, plutôt que de le laisser passer derrière tes fenêtres.

Si c'est bon, la Section 3 traite **la machine à états** — comment il choisit quoi faire, comment « aller vers l'appli active » cohabite avec « faire ce dont j'ai envie », et comment Luffy et Zoro se remarquent.

---

## 👤 Alexis

Après je me dis si je veux pouvoir le mettre sur un autre ordi faudra que la personne installe rust ,

---

## 🤖 Claude

Non, et c'est même l'inverse : **Tauri se distribue mieux qu'Electron.**

Rust est un outil de **compilation**, pas un runtime. C'est comme un compilateur C : tu l'as sur ta machine pour produire le `.exe`, la personne qui reçoit le `.exe` n'a besoin de rien. Tu compiles chez toi, tu envoies un binaire natif Windows.

## Ce que la machine d'en face doit avoir

| | Tauri | Electron |
|---|---|---|
| Rust installé | **non** | — |
| Node installé | **non** | **non** |
| Poids à envoyer | **~10 Mo** | ~150 Mo |
| Prérequis | WebView2 | aucun |

Les deux seuls prérequis réels, et ils sont bénins :

1. **WebView2** — le moteur de rendu d'Edge. **Déjà présent sur tout Windows 11** et sur Windows 10 à jour. Et si tu veux zéro risque, Tauri sait embarquer son installateur dans le tien : tu choisis entre le télécharger à l'installation, l'embarquer, ou l'ignorer.
2. **Le runtime Visual C++** — présent sur la quasi-totalité des PC (des dizaines d'applis l'installent). Et si tu veux être vraiment tranquille, on compile en **liant statiquement** ce runtime : le `.exe` devient totalement autonome. Une ligne de configuration, à mettre en place une fois.

Donc dans le pire cas, tu envoies un exe et ça marche. Là où Electron t'aurait fait envoyer 150 Mo de Chromium.

## Ce que tu peux livrer

Tauri produit au choix un **installateur** (MSI ou NSIS, avec raccourci et démarrage auto) ou un **exe portable** — tu le poses sur le bureau, tu double-cliques, ça tourne.

## Une conséquence sur le design, du coup

Ta question soulève un vrai point d'architecture. Tauri sait **embarquer** l'interface dans le binaire — ce serait le réflexe. Mais pour les personnages, je recommande l'inverse : **garder les images et leurs descriptions dans un dossier à côté de l'exe**, pas dedans.

```
LuffyDesktop/
├── luffy-desktop.exe
└── characters/
    ├── luffy/
    └── zoro/
```

Trois raisons :

- **tu ajoutes un perso sans recompiler** — tu déposes un dossier, tu relances
- **ton pote peut mettre le sien** sans rien savoir de Rust
- **tu itères sur les animations sans build** — tu ajustes une frame, tu relances, tu vois

Ça reste un dossier unique à zipper, donc la distribution ne se complique pas. C'est juste plus modulaire, et ça sert directement ton envie de plusieurs persos.

## Un détail à savoir

Un exe non signé déclenche l'alerte SmartScreen de Windows (« Windows a protégé votre PC ») — la personne clique sur « Informations complémentaires » → « Exécuter quand même », et c'est fini. Pour faire disparaître ça il faut un certificat de signature payant (~200 €/an). Pour un projet plaisir partagé à des potes, ça ne vaut pas le coup ; autant juste le prévenir.

---

Du coup je reviens à ma question sur la Section 2, que tu n'as pas encore tranchée : **je garde le perso toujours au premier plan** en filtrant les bords de fenêtres recouverts (donc il ne peut jamais s'asseoir sur du vide, mais il flotte visuellement au-dessus de tout) — ou tu préfères qu'il **passe derrière tes fenêtres** quand elles le recouvrent, au risque de le perdre de vue ?

---

## 👤 Alexis

Pour ta question garde le perso au premier plan d'ailleurs j'aime beaucoup les personnages shijemi donc je t'ai mis un dossier shijemi pour utiliser ce personnage en test au lieu de mes perso pixélisé ca sera mieux je pense

---

## 🤖 Claude

Excellent réflexe. Un vrai Shimeji apporte potentiellement **exactement les animations qui me manquaient** (escalade, suspension, sommeil). Je regarde ce que tu as déposé.

Ah, c'est le jeu d'images **Shimeji-ee canonique** : `shime1.png` → `shime46.png`, exactement les 46 planches standard. Il manque juste le dossier `conf/` (les `actions.xml` / `behaviors.xml` qui décrivent les enchaînements) — tu n'as que les images.

Laisse-moi regarder ce qu'elles contiennent réellement plutôt que de me fier à la convention.

Tous en **128×128** — ce qui valide exactement la fenêtre de 128×128 que je proposais en Section 1.

(Note : le `convert` détecté est l'outil Windows de conversion de système de fichiers, pas ImageMagick — je ne le touche pas. J'utilise PIL, qui est présent.)

Je monte une planche-contact pour identifier chaque frame visuellement.

C'est le **mascotte blob blanc par défaut de Shimeji-ee** — l'originale. Et c'est une très bonne idée. Laisse-moi vérifier de près les poses qui m'intéressent le plus, celles qui me manquaient.

Confirmé, et c'est mieux que ce que j'espérais. Voilà ce que contient réellement le jeu :

| Frames | Pose | Ce que ça débloque |
|---|---|---|
| **23, 24, 25** | bras en L qui agrippe une paroi verticale | **l'escalade** — le trou n°1 |
| **34, 35, 36** | corps coupé par un bord, jambes pendantes | **se hisser par-dessus un bord** de fenêtre |
| **22** | bras tendus vers le haut, corps suspendu | **suspension au plafond** |
| **39, 40, 41** | tête posée sur le bras, s'affaisse | **s'asseoir puis dormir** — le trou n°2 |
| **10** | corps basculé, bras en arrière | **la chute** |
| **4** | à plat ventre, traits de vitesse | **l'atterrissage** |
| **18, 20, 21** | allongé, bras qui tire | **ramper** |
| **44, 45, 46** | tête large, 2 → 3 → 4 yeux | **le dédoublement** (la signature Shimeji) |

**Les deux trous que j'avais signalés sont bouchés.** Escalade et sommeil existent, dessinés pour ça.

Et Section 2 validée : **toujours au premier plan**, avec les bords recouverts filtrés.

---

# Ce que ça change dans le design

## 1. La numérotation Shimeji est un standard de fait

Les 46 slots ne sont pas propres à ce personnage : **tous les packs Shemeji du net utilisent la même numérotation pour les mêmes poses**, parce qu'ils sont tous dessinés contre le même fichier de configuration d'origine.

Combiné à la décision du message précédent — **les personnages dans un dossier à côté de l'exe** — ça donne quelque chose de fort :

> N'importe quel personnage Shimeji trouvé sur internet devient **utilisable en le déposant dans le dossier**. Sans recompiler, sans rien coder.

C'est un gain que je n'avais pas anticipé, et il vaut plus que tout ce que j'aurais pu concevoir moi-même. Le vocabulaire d'animations du projet devient donc **les slots Shimeji**, et non une invention maison.

## 2. Deux sortes de frames

Shimeji fait **un fichier par pose**. Tes planches Luffy sont des **atlas** à découper. Le manifeste doit donc accepter les deux :

```
frame = un fichier entier          → "shime23.png"
frame = une région d'un atlas      → luffy_walk.png à (0, 64, 64×64)
```

Petite addition, et elle suffit à faire cohabiter les deux mondes.

## 3. Un point d'ancrage par frame — et ton bug actuel disparaît

Ton code compense à la main le décalage visuel quand la largeur du sprite change ([overlay.html:302-310](overlay.html#L302-L310)), avec un traitement asymétrique selon le sens de marche. Ce contournement existe parce que tes frames ont des tailles différentes.

Les frames Shimeji sont **toutes en 128×128** avec le personnage positionné dans la boîte. En déclarant un **point d'ancrage** (le sol sous ses pieds, la main qui agrippe), la position devient simplement « place l'ancre ici ». Le contournement disparaît, et avec lui la classe de bugs visuels qui va avec.

Bonus : le 128×128 **valide la fenêtre de 128×128** que je proposais en Section 1, sans que j'aie eu à le deviner.

## 4. Le risque de contenu ne bloque plus rien

C'était mon inquiétude principale. Elle tombe :

- **on construit et on teste contre le blob**, qui a toutes les poses
- **Luffy et Zoro deviennent des personnages à couverture partielle** — ils remplissent les slots qu'ils peuvent, et les comportements dont les images manquent ne se déclenchent simplement jamais pour eux

Tu peux donc développer l'escalade **maintenant**, sans attendre d'avoir dessiné un Luffy accroché à un mur. C'est exactement la dégradation gracieuse que j'avais prévue — elle sert dès le premier jour.

---

Reste un choix, et c'est le dernier point de cadrage avant la machine à états :

---

### ❓ Question à choix

**Jusqu'où pousser la compatibilité Shimeji ?**

Options proposées : JSON maison sur slots Shimeji (recommandé) · Lire les vrais XML Shimeji · JSON maison, sans lien avec Shimeji

### ✅ Décision d'Alexis

"Jusqu'où pousser la compatibilité Shimeji ?"="JSON maison sur slots Shimeji (recommandé)".

## 🤖 Claude

Adopté. Le vocabulaire de poses devient les slots Shimeji, avec notre propre JSON par-dessus.

---

# Design — Section 3 : le comportement

C'est la plus grosse partie du projet. Trois exigences se percutent ici : il doit **faire ce dont il a envie**, **aller physiquement vers ton appli active**, et **remarquer l'autre perso** — sans que ça devienne un empilement de cas particuliers.

La réponse est de séparer en trois couches qui ne se parlent que dans un sens.

```
                 chaque image (60 Hz)
                         │
                         ▼
   ┌──────────────────────────────────────────────┐
   │  1. RÉFLEXES — non négociables               │
   │     plateforme disparue   → je tombe         │
   │     attrapé à la souris   → je suis porté    │
   │     contact avec le sol   → j'atterris       │
   │     session verrouillée   → je disparais     │
   └──────────────────────────────────────────────┘
                         │ rien ne s'est imposé
                         ▼
   ┌──────────────────────────────────────────────┐
   │  2. INTENTION en cours — une seule à la fois │
   │     Flâner  ·  AllerÀ(surface)               │
   │     SeReposer  ·  Jouer(action)              │
   │     → traduite en déplacement + animation    │
   └──────────────────────────────────────────────┘
                         │ finie · échouée · expirée
                         ▼
   ┌──────────────────────────────────────────────┐
   │  3. ENVIE — tirage aléatoire pondéré         │
   │     les signaux MODIFIENT les poids          │
   │     les slots manquants RETIRENT les options │
   └──────────────────────────────────────────────┘
```

## L'idée qui tient toute la section

> **Les signaux ne commandent pas, ils biaisent.**

C'est le point que je défends le plus dans ce design. Si « inactif 2 min » **déclenchait** le sommeil, tu aurais un afficheur d'état système déguisé en personnage : parfaitement prévisible, mort en trois jours. Si « inactif 2 min » **multiplie par 8 l'envie de dormir**, il s'endort presque toujours — mais parfois il s'assoit, parfois il traîne encore un peu. C'est cette marge qui fait qu'on a envie de le regarder.

Concrètement, la table d'envies :

| Intention | Poids de base | Modificateurs |
|---|---|---|
| Flâner | 5 | × 0,2 si inactif > 2 min |
| Se reposer / dormir | 1 | **× 8** si inactif > 2 min · × 3 le soir |
| Manger | 1 | **× 6** entre 12 h et 14 h |
| Aller à la fenêtre active | 2 | × 4 si tu viens de changer d'appli |
| Aller voir l'autre perso | 1 | × 3 s'il est proche |
| S'amuser (attaque, saut) | 1 | × 3 selon l'appli active |

Trois propriétés qui découlent gratuitement de cette forme :

- **Ajouter un signal = ajouter une ligne.** Aucun nouveau chemin de code. Si tu veux réintroduire le CPU plus tard, c'est une ligne.
- **Les persos à couverture partielle marchent tout seuls.** Pas d'images de sommeil pour Luffy → l'option est retirée du tirage. Rien à coder.
- **C'est de la donnée**, donc réglable dans un fichier de config à côté de l'exe — tu ajustes son caractère sans recompiler.

Et « il change d'attitude selon l'appli » devient de la donnée aussi, éditable par toi :

```
Code.exe    → s'amuser ×3,  flâner ×0,5
chrome.exe  → se reposer ×2, flâner ×1,5
```

## Aller vers ta fenêtre active

Il faut vraiment traverser l'écran, donc naviguer. Je **ne veux pas** de vrai calcul de chemin : la carte des plateformes change 8 fois par seconde, un chemin calculé est périmé avant d'être parcouru.

À la place, une décision locale à chaque image — « d'où je suis, quel est mon prochain pas ? » :

| Situation | Prochain pas |
|---|---|
| la cible est sur ma surface | j'avance vers elle |
| elle est au-dessus | je cherche une paroi escaladable de ce côté, je grimpe |
| elle est en dessous | je vais au bord le plus proche et je me laisse tomber |
| sa surface a disparu | l'intention échoue |

Ça échoue parfois — il se coince dans un coin, il prend une route idiote. **Et c'est très bien.** Un animal de compagnie qui prend un chemin bête est attachant ; un qui calcule l'itinéraire optimal a l'air d'un robot. Le hasard de la bêtise joue en notre faveur.

Avec une seule règle de sécurité qui remplace toute une classe de bugs :

> **Toute intention a un délai d'abandon (~20 s).** Passé ce délai, elle échoue et il retourne flâner.

Je ne cherche donc jamais à énumérer les situations de blocage — je rends le blocage temporaire par construction. C'est le pendant de la décision de la Section 2 : là je rendais les fenêtres qui bougent gratuites, ici je rends les échecs de navigation inoffensifs.

## Luffy et Zoro se remarquent

Version minimale volontairement :

- une intention **« aller voir l'autre »**, dont le poids monte quand il est proche
- quand ils sont sur la même surface à faible distance → une **rencontre** : chacun joue une réaction (et vu qui c'est, Luffy et Zoro qui se chamaillent, tes planches d'attaques sont déjà parfaites pour ça)
- un temps de repos après, sinon ils se chamaillent en boucle

Aucun protocole de négociation n'est nécessaire : comme Rust détient les deux persos (Section 1), une rencontre est une simple vérification côté coordinateur. C'est le troisième dividende de cette décision — après le multi-écran et le comportement social qui trouve sa place naturellement.

Je m'arrête là volontairement : **pas de système de relations, pas d'humeur partagée, pas d'amitié qui évolue.** Une intention et une interaction. Si tu en veux plus, ça s'ajoutera comme des lignes dans la table.

---

Cette section te va ? Les deux points sur lesquels j'aimerais ton avis :

1. **« Les signaux biaisent au lieu de commander »** — es-tu d'accord pour qu'il ne s'endorme pas *systématiquement* quand tu t'absentes, mais presque toujours ?
2. **La navigation autorisée à échouer** — un perso qui se coince 20 secondes puis renonce et repart flâner, ça te convient, ou tu veux qu'il arrive toujours à destination ?

Si c'est bon, il ne reste que la Section 4 (le tour du propriétaire : tray, démarrage auto, config, et ce qui est testable automatiquement) et j'écris la spec.

---

## 👤 Alexis

On pourrait peut-être faire un projet à part nouveau git non ? je veux pas utiliser mes sprite zoro et luffy on poourrait faire un claude.md non pour résumer le besoin et la stack technique ?

---

## 🤖 Claude

Bonne idée sur les deux points. Et **abandonner Luffy/Zoro simplifie réellement le design** :

- **Le manifeste n'a plus qu'une seule sorte de frame** (un fichier par pose). Les atlas à découper disparaissent — c'était la seule complication que tes planches GBA imposaient.
- **Plus de personnages à couverture partielle** à gérer au départ. Le filtrage par slots manquants reste dans le design, mais il ne sert plus qu'aux packs tiers que tu déposeras.
- Le projet devient net : **un moteur de desktop pet compatible Shimeji**, testé sur le blob, et n'importe quel pack du net se dépose dedans.

Et « les deux persos se remarquent » tient toujours : deux blobs. C'est même canonique — tes frames 44-46 sont l'animation de **dédoublement**, la signature Shimeji.

Une question avant de créer quoi que ce soit :

---

### ❓ Question à choix

**Où et sous quel nom je crée le nouveau dépôt ?**

Options proposées : Documents\shimeji-rs · Documents\desk-mascot · Un sous-dossier de `op`

### ✅ Décision d'Alexis

"Où et sous quel nom je crée le nouveau dépôt ?"="shijemi-desktop plutot ? ".

## 🤖 Claude

`shimeji-desktop`, ça marche. Un détail au passage : ça s'écrit **Shimeji** (しめじ, un champignon) — tu l'as écrit « shijemi » plusieurs fois, mais ton dossier est bien orthographié. Comme toute la valeur du projet est la compatibilité avec les packs Shimeji, je prends l'orthographe correcte ; c'est un `mv` si tu préfères la tienne.

Le heredoc s'est fait piéger par le quoting. J'écris directement.

Fait. Le dépôt existe.

```
Documents\shimeji-desktop\
├── .git\                      ← initialisé, rien de commité
├── .gitignore                 ← Rust/Tauri
├── CLAUDE.md                  ← le besoin + la stack + les décisions
├── characters\blob\img\       ← les 46 frames
└── docs\specs\                ← vide, la spec ira là
```

Je n'ai **rien commité** — je te laisse la main sur le premier commit. Ton dossier `Shimeji/` dans `op` est intact, j'ai copié.

## Ce que contient CLAUDE.md

Pas juste un résumé : les décisions **et leur raison**, pour qu'une session future ne les défasse pas par ignorance.

- **Le besoin** — ce qu'il fait, où il vit, à quoi il réagit, comment on interagit, et les 6 hors-sujets explicites (Tamagotchi, capture de frappe, CPU, bulles…)
- **La stack** — Tauri v2 / Rust, et *pourquoi* plutôt qu'Electron, avec la réponse à ta question sur la distribution
- **L'architecture** — une fenêtre par personnage, la logique en Rust, le monde en plateformes, les trois horloges
- **Les 5 décisions à ne pas défaire** — chacune avec la classe de bugs qu'elle supprime. C'est la partie qui compte le plus : « position dérivée, jamais stockée », « les signaux biaisent, ils ne commandent pas », « la navigation est autorisée à échouer »
- **Les personnages** — le format, les slots Shimeji, la couverture partielle, et l'inventaire des poses de `blob`
- **L'ordre de construction** — les 6 étapes, avec le spike de l'étape 0 marqué non négociable
- **Les 4 pièges Windows** déjà identifiés, dont la bordure invisible de 7 px

J'y ai aussi noté que l'art du blob n'est pas de nous — sans conséquence en privé, à vérifier avant de publier le dépôt.

---

Il reste **la Section 4** du design, la plus courte : le tour du propriétaire (icône dans le tray, démarrage avec Windows, le fichier de config, et surtout **ce qui est testable automatiquement** — non trivial pour un truc qui est par nature visuel et aléatoire).

Ensuite j'écris la spec complète dans `docs/specs/` et on passe au plan d'implémentation.

Tu veux que j'enchaîne sur la Section 4, ou tu préfères commiter et regarder CLAUDE.md d'abord ?

---

## 👤 Alexis

Ok donc je peux créer une session claude la bas ?

---

## 🤖 Claude

Oui, tout à fait. Tu ouvres une session dans `Documents\shimeji-desktop` et **CLAUDE.md sera chargé automatiquement** — la session saura ce qu'on construit, avec quelle stack, et surtout quelles décisions ne pas défaire.

Deux réserves pratiques, par contre.

## Commite d'abord

Le dépôt est initialisé mais vide côté git. Une session qui démarre sur un dépôt sans aucun commit ne peut pas distinguer ce que tu as écrit de ce qu'elle a écrit :

```
cd C:\Users\alri\Documents\shimeji-desktop
git add -A && git commit -m "chore: besoin, décisions de design et personnage de test"
```

## Le design n'est pas fini — et ce qui manque n'est pas dans CLAUDE.md

C'est le vrai point. CLAUDE.md contient les **conclusions**, pas la discussion qui y a mené. Et il manque encore :

- **la Section 4** — on ne l'a jamais écrite : tray, démarrage auto, format exact du fichier de config, et surtout **ce qui est testable automatiquement** sur un truc visuel et aléatoire (c'est loin d'être évident, et une session fraîche va probablement dire « c'est visuel, on teste à la main » — ce qui est faux : la physique, la soustraction d'intervalles pour l'occlusion, le tirage pondéré et le filtrage par slots sont tous testables sans écran)
- **la spec complète** — le détail du modèle d'accroche, la forme du manifeste, la table d'envies

Une session neuve devra re-déduire tout ça depuis CLAUDE.md. Elle y arrivera à peu près, mais elle perdra les nuances qu'on a mis six échanges à trancher.

**Donc je te recommande de finir ici** : Section 4, puis j'écris la spec dans `docs/specs/`, tu commites, et *ensuite* tu ouvres la session là-bas avec un design complet. Elle attaque directement le plan et le spike de l'étape 0, sans rien re-deviner.

## Si tu préfères y aller maintenant

C'est jouable, CLAUDE.md est assez dense pour ça. Donne-lui ce premier message :

> Lis CLAUDE.md. On n'a pas encore écrit la Section 4 du design (tray, démarrage auto, config, stratégie de test) ni la spec complète. Commence par la spec dans `docs/specs/`, puis le plan d'implémentation, puis le spike de l'étape 0 — sans échafauder l'app avant que le spike ait prouvé que la fenêtre transparente tient.

Un dernier point que je peux ajouter à CLAUDE.md si tu veux : **que tu apprends Rust sur ce projet**. Ce n'est pas dedans, et ça change la façon dont une session devrait t'expliquer les choses — expliquer le Rust au lieu de le supposer acquis. Une ligne, mais utile pour toutes les sessions futures.

Tu veux que je finisse la Section 4 et la spec ici, ou tu bascules là-bas ?

---

## 👤 Alexis

Non mais vazy continue d'ici fait tout ce que tu peux puis je continuerai la-bas quand t'auras finis

---

## 🤖 Claude

Compris, je déroule tout ce qui ne demande pas tes yeux. Je vérifie d'abord ce que ta machine a déjà.

Deux constats sur ta machine, dont un qui va t'éviter une heure perdue :

| | |
|---|---|
| **Rust** | ❌ absent — je ne peux donc **rien compiler ni vérifier** |
| **WebView2** | ✅ présent (152.0.4191.66) — un prérequis validé |
| **Node** | ⚠️ v14.17.0, de 2021 |
| **Visual Studio Installer** | présent, composants C++ à confirmer |

**Le piège Node.** Tauri s'installe soit par npm, soit par cargo. La voie npm exigerait Node 18+ — ton Node 14 la ferme. Mais comme on a décidé « pas de bundler, front statique », **on n'a besoin de Node nulle part** : on passe par `cargo-tauri` et ton Node 14 devient hors sujet. Ça aurait été un faux problème de démarrage classique.

Comme Rust manque, je ne peux pas écrire de spike vérifié — je m'en tiens donc au design, à la spec et au plan, et je te laisse la compilation. Je note les commandes d'installation exactes dans le plan.

---

# Design — Section 4 : le tour du propriétaire

Court, sauf le dernier point.

**Tray.** Afficher/cacher · ajouter un personnage · en retirer un · ouvrir le dossier des personnages · **recharger les personnages** · démarrer avec Windows · quitter. Le rechargement à chaud compte plus qu'il n'y paraît : c'est lui qui rend agréable le réglage des animations — tu ajustes un timing dans le JSON, tu recharges, tu vois. Sans lui, chaque essai coûte un redémarrage.

**Démarrage auto.** Une case dans le tray, écrite dans la clé `Run` de l'utilisateur courant. Au démarrage automatique, aucune fenêtre ni notification.

**Config.** Un JSON, **cherché à côté de l'exe d'abord** (mode portable), sinon `%APPDATA%`. Le mode portable est celui que tu utiliseras ; le repli existe pour le cas où l'exe est installé dans un dossier non inscriptible. Même règle pour `characters/`.

## Ce qui est testable automatiquement

C'est le point non trivial, et l'intuition est fausse : un programme visuel et aléatoire *paraît* intestable. En réalité **presque tout le cœur est une fonction pure.**

| Testable sans écran | Nature du test |
|---|---|
| Soustraction d'intervalles (occlusion) | un segment + N rectangles → sous-segments attendus |
| Filtrage des fenêtres | fenêtres factices → celles retenues |
| Position dérivée de `(plateforme, face, offset)` | on bouge le rectangle → la position suit |
| Transitions d'accroche | plateforme retirée → chute ; rétrécie → chute |
| Chute | intégration déterministe : hauteur/durée connues |
| Tirage pondéré | 10 000 tirages à graine fixe → distribution attendue |
| Filtrage par slots manquants | manifeste amputé → l'intention n'est jamais tirée |
| Délai d'abandon | horloge injectée → échec à 20 s |
| Manifeste malformé | erreur claire, aucune panique |

Mais ça **impose trois contraintes de conception dès le départ** — impossibles à rattraper après :

1. **Le temps est injecté**, jamais lu depuis l'horloge système → sinon rien de temporel n'est testable
2. **L'aléatoire est injecté et graînable** → sinon rien de pondéré n'est testable
3. **La couche win32 est derrière une interface** → sinon aucun test ne peut fournir de fausses fenêtres

Ce sont les tests qui dictent ces trois choix, pas l'inverse. D'où leur place dans la spec.

**Et un outil qui vaut son coût :** un **mode simulation** sans écran — on déroule 10 minutes de comportement contre un monde factice à graine fixe, et on imprime la trace. « Il a dormi, mangé, flâné, jamais bloqué » se vérifie alors sans regarder l'écran, et les régressions de comportement deviennent détectables.

**Ce qui restera à ton œil :** que la transparence fonctionne (le spike), qu'il soit joliment posé sur une barre de titre, et qu'il ait l'air vivant. Rien d'autre.

---

J'écris la spec.

Auto-revue de la spec — trois ambiguïtés à lever dans le manifeste.

Spec écrite et auto-revue. Je passe au plan d'implémentation.

Je crée le plan d'implémentation avec la skill writing-plans.

**Vérification de portée d'abord** — la spec couvre 6 étapes ; c'est trop pour un seul plan. Et surtout, l'étape 0 est un **verrou de décision** : son résultat peut renvoyer tout le projet vers Electron, et il conditionne des choix d'implémentation de l'étape 1 (le déplacement à 60 Hz saccade-t-il ? la transparence exige-t-elle un réglage particulier ?). Écrire l'étape 1 en détail avant d'avoir la réponse, **sans pouvoir compiler**, produirait de la fiction.

Donc : **plan 1 = étape 0, complet et exécutable**. Le plan de l'étape 1 s'écrit après, éclairé par les résultats.

Auto-revue du plan. Un point mérite une garde : je n'ai pas pu compiler, et deux appels d'API sont les candidats les plus probables à un écart de signature.

Je mets CLAUDE.md à jour : l'état de la chaîne d'outils, le piège Node, et le fait que tu apprends Rust — ça change la façon dont une session future doit t'expliquer les choses.

Terminé. Tout ce qui ne demandait ni compilateur ni tes yeux est fait et commité.

## Ce qui existe maintenant

```
Documents\shimeji-desktop\          commit 42b595c
├── CLAUDE.md                                    355 l.
├── docs\specs\2026-09-08-design.md              635 l.  ← le design complet
├── docs\plans\2026-09-08-etape-0-spike-overlay.md 581 l. ← exécutable tel quel
└── characters\blob\img\shime1..46.png
```

**Le design est complet** — les 4 sections, y compris la Section 4 qu'on n'avait pas faite. La spec contient aussi une §13 « écarté explicitement » qui liste les 9 options rejetées et pourquoi : c'est ce qui empêchera une session future de reproposer un overlay plein écran ou de parser les XML Shimeji.

**Le plan de l'étape 0 est complet** : 3 tâches, l'installation de la chaîne d'outils avec les commandes exactes, le code du spike en entier, et une grille de vérification des 7 propriétés avec **le remède établi pour chaque échec** — pour que tu ne diagnostiques pas à l'aveugle si la transparence coince.

## Ce que je n'ai pas pu faire, et pourquoi

**Rust n'est pas installé**, donc je n'ai **rien compilé ni vérifié**. Le code du spike est écrit avec soin mais non validé : j'ai marqué ça franchement dans le plan, et ajouté une table des deux appels d'API les plus susceptibles d'un écart de signature (`available_monitors`, `shadow`) avec leur correction. Ne sois pas surpris si le premier `cargo build` grogne.

**Je n'ai pas écrit le plan de l'étape 1**, volontairement. Les résultats du spike — la fluidité à 60 Hz, le comportement multi-DPI, les attributs de fenêtre réellement nécessaires — conditionnent plusieurs choix d'implémentation. Écrire ça avant la réponse, sans pouvoir compiler, aurait été de la fiction. La **structure de fichiers** de l'étape 1 est en revanche déjà verrouillée (annexe A), donc rien n'est à rediscuter.

## Pour continuer là-bas

Ouvre une session dans `Documents\shimeji-desktop` et donne-lui :

> Lis CLAUDE.md, puis `docs/plans/2026-09-08-etape-0-spike-overlay.md`. Exécute-le tâche par tâche. La Tâche 1 (installation de la chaîne d'outils) est pour moi, guide-moi. N'échafaude pas l'application avant que le spike ait répondu.

La Tâche 1 est à faire par toi : l'installation des composants C++ de Visual Studio ouvre une interface graphique, un agent bloquerait dessus.

**Un piège que le plan t'évite** : n'installe pas Node 18. Ton Node 14 semble être un blocage pour Tauri, mais on passe par `cargo-tauri` et le front est statique — Node ne sert à rien ici. C'est noté dans CLAUDE.md pour qu'aucune session ne « corrige » ça.

J'ai aussi gardé en mémoire que tu préfères la recommandation sur le fond avant le repli confortable — tu as pris Rust et l'escalade des fenêtres après que j'aie signalé leur coût dans les deux cas.

---

## 👤 Alexis

j'ai installé rust normalement

---

## 🤖 Claude

Parfait, ça débloque la compilation — je peux au moins vérifier que le code du spike tient. Je regarde ce que la machine a maintenant.

Rust n'est pas dans le `PATH` de cette session. C'est très probablement l'environnement de mon processus qui est **antérieur à ton installation** — je vérifie sur le disque plutôt que dans le `PATH`.

**Rust est bien installé** — cargo 1.98.1, cible `x86_64-pc-windows-msvc` présente. C'est bien l'environnement de mon processus qui est périmé ; je passe par les chemins absolus.

Mais je vois un autre manque : **Visual Studio 2022 Community est là, sans la charge de travail C++** — ni toolset MSVC, ni SDK Windows. Je confirme, et je crée le spike en parallèle.

Confirmé : **charge de travail C++ absente**. Sans linker, je ne peux pas compiler — et `cargo check` non plus, parce que le script de build de Tauri doit lui-même être lié pour s'exécuter.

Mais je peux **télécharger les sources de Tauri et vérifier les signatures réelles** — ce qui répond exactement aux deux incertitudes que j'avais laissées dans le plan. J'écris le reste du spike puis je vérifie.

Tauri **2.11.5** résolu. Je vérifie les signatures réelles dans les sources.

Trois signatures confirmées telles quelles, dont `shadow` — la réserve que j'avais mise dans le plan tombe. Reste deux points à trancher : sur quel type vit `available_monitors`, et si `WebviewWindow` traverse un thread.

Deux résultats importants. `available_monitors` vit dans `impl App<Wry>` — donc l'appel sur `app` dans `setup` est correct. Mais **je ne peux pas confirmer que `WebviewWindow` est `Send`** : aucune implémentation explicite, c'est déduit des champs. Sans compilateur, je ne vais pas parier là-dessus dans un thread.

Je vérifie le motif alternatif, qui est de toute façon plus robuste.

Le motif `AppHandle` est confirmé et c'est celui dont je suis sûr — c'est le point d'entrée documenté pour le travail multi-thread dans Tauri, et il gère en prime la fenêtre fermée. Je réécris le spike avec lui plutôt que de parier sur `WebviewWindow: Send`.

Je tente la compilation pour te donner l'erreur exacte plutôt qu'une supposition.

Erreur capturée, et elle révèle **deux** problèmes, pas un. Regarde la note :

```
= note: link: extra operand '...rcgu.o'
note: in the Visual Studio installer, ensure the "C++ build tools" workload is selected
```

`extra operand` est un message d'erreur **GNU**, pas MSVC. Le `link.exe` trouvé n'est pas le linker de Visual Studio — c'est le `link` de coreutils livré avec Git for Windows. rustc s'est rabattu sur le `PATH` faute de trouver une installation C++, et y a pêché le mauvais binaire. C'est un piège classique de Rust sous Windows, et exactement le genre d'erreur qui fait perdre une soirée. Je le consigne.

Oui — c'est le seul vrai bloquant. Je réponds en détail juste après, je finis d'abord de consigner.

Oui, c'est le seul bloquant qui reste. Et pour être clair : **tu n'écriras pas une ligne de C++.**

## Pourquoi

Rust compile en natif, mais il ne fournit pas son propre **linker** ni les bibliothèques système Windows. La cible `x86_64-pc-windows-msvc` emprunte les deux à Microsoft : `link.exe` et le SDK Windows. Ils arrivent avec la charge de travail C++, d'où le nom trompeur.

Ton VS 2022 Community est déjà là, mais installé **sans** cette charge — donc pas de `VC\Tools\MSVC\`, pas de `Windows Kits\10\`. C'est une **modification** de l'installation existante, pas une nouvelle.

## Comment

Le plus simple, en PowerShell :

```powershell
& "C:\Program Files (x86)\Microsoft Visual Studio\Installer\setup.exe" modify `
  --installPath "C:\Program Files\Microsoft Visual Studio\2022\Community" `
  --add Microsoft.VisualStudio.Workload.NativeDesktop --includeRecommended --passive
```

Ou à la souris : **Visual Studio Installer** → *Modifier* sur Community → cocher **« Développement Desktop en C++ »** → Modifier.

Compte **2 à 4 Go** et quelques minutes.

## Ensuite, deux commandes et tu vois le résultat

Le spike est **déjà écrit et commité** — j'ai vérifié tous ses appels d'API contre les sources de Tauri 2.11.5 pendant que j'y étais, donc les incertitudes que j'avais laissées dans le plan sont levées.

```powershell
cargo install tauri-cli --version "^2"     # quelques minutes
cd C:\Users\alri\Documents\shimeji-desktop\spike
cargo tauri dev
```

**Lance-les depuis PowerShell, pas depuis Git Bash.** C'est le second piège que j'ai trouvé : dans un shell Git Bash, rustc pêche le `link.exe` de Git for Windows (un utilitaire de liens durs de coreutils) au lieu de celui de Visual Studio. Ça donne une erreur `extra operand` totalement opaque. Après installation de la charge C++ ça devrait se régler seul, mais autant éviter la question.

Tu devrais alors voir une silhouette blanche traverser ton écran en ondulant, et la console imprimer la topologie de tes moniteurs. Les 7 propriétés à vérifier à l'œil sont dans le plan (Tâche 3), avec le remède pour chaque échec — et la grille à remplir est déjà dans `docs/specs/2026-09-08-spike-0-resultat.md`.

## Où j'en suis

| | |
|---|---|
| Design complet, spec, plan étape 0 | ✅ commité |
| Spike écrit, API vérifiées | ✅ commité (`6afb03a`) |
| Compilation | ❌ bloquée sur la charge C++ |
| Les 7 vérifications visuelles | ⬜ pour toi, après installation |

C'est tout ce que je pouvais faire sans linker ni tes yeux.

---

## 👤 Alexis

j'ai installé c++ sur 2026

---

## 🤖 Claude

Je vérifie l'installation puis je compile — depuis PowerShell, pour éviter le piège du `link.exe` de Git.

Le SDK Windows 10.0.26100.0 est apparu, mais pas de toolset MSVC — et tu dis « sur 2026 ». Tu as donc installé la charge C++ sur **Visual Studio 2026**, pas sur le 2022 Community que je regardais. Je cherche au bon endroit.

Trouvé. Tu l'as installé sur **Visual Studio 18 Insiders** (le VS 2026 pré-version) :

```
C:\Program Files\Microsoft Visual Studio\18\Insiders
  └── VC\Tools\MSVC\14.51.36231\bin\Hostx64\x64\link.exe   ✅
```

vswhere confirme que c'est bien **cette** installation qui porte la charge C++ — mais elle n'apparaît qu'avec `-prerelease`. La question est donc de savoir si rustc sait détecter une installation Insiders. J'essaie.

Gros progrès : ça a compilé **jusqu'à `tauri v2.11.5`** — donc rustc a bien trouvé le MSVC des Insiders, le linker fonctionne. L'échec est maintenant dans *notre* script de build. Je récupère l'erreur réelle.

Erreur nette : `icons/icon.ico` manquant — tauri-build en a besoin pour la ressource Windows. Et je repère au passage une erreur de ma part dans la config : `frontendDist: "../ui"` pointe **hors** du dossier `spike` (le chemin est relatif au `tauri.conf.json`). Je corrige les deux.

**Ça compile.** Exit 0, exe de 12 Mo produit — mes vérifications de signatures tenaient. Je ne le lance pas moi-même : ça poserait une fenêtre mobile au premier plan sur ton écran sans prévenir, et de toute façon la vérification est visuelle.

Je traite ta demande sur les commentaires, puis je consigne.

Petit nettoyage : `spike/gen/schemas/` est généré par tauri-build, il n'a rien à faire dans git.

Mon motif était mal formé : `gen/schemas/` contient un `/`, donc git l'ancre à la racine et il ne matche pas `spike/gen/schemas/`. Il faut `**/`.

Tout est en place. Voilà où on en est.

## Ce qui s'est passé

**Le C++ était bien installé — sur VS 18 Insiders, pas sur 2022 Community.** Mon premier diagnostic disait « absent » parce que j'interrogeais `vswhere` sans `-prerelease` : une installation Insiders reste invisible sans ce drapeau. C'est consigné, pour ne pas refaire l'erreur.

Bonne nouvelle derrière : **rustc 1.98.1 détecte l'installation Insiders tout seul**, sans variable d'environnement ni Developer PowerShell.

## Le spike compile

```
Finished `dev` profile in 2m 24s     exit 0
target\debug\spike-overlay.exe       12,3 Mo
```

Mes vérifications de signatures contre les sources de Tauri tenaient — aucun appel n'a été refusé. Deux vrais bugs sont apparus, corrigés :

| Bug | Correction |
|---|---|
| `icons/icon.ico` exigé par tauri-build | icône générée depuis `shime1.png`, tailles 16 → 256 |
| `frontendDist: "../ui"` pointait **hors** du projet | `"./ui"` — le chemin est résolu depuis le dossier du `tauri.conf.json`, et j'avais recopié la convention `src-tauri/` sur une arborescence plate. Mon erreur. |

Et un bonus : **tu n'as pas besoin de `cargo-tauri`.** Comme le front est statique, les assets sont embarqués à la compilation — `cargo run` suffit. Ça t'économise le `cargo install tauri-cli` que le plan prévoyait.

## Tes commentaires : c'est dans CLAUDE.md

J'ai ajouté une section **Conventions de code**, marquée comme une exigence explicite de ta part et non un choix de style, pour qu'aucune session future ne « nettoie » les commentaires par souci de concision. Elle couvre : commenter le *pourquoi*, **renvoyer à la section de la spec** que le bloc applique, expliquer systématiquement les constructions Rust non évidentes, scander les fonctions longues par bandeaux, et un en-tête de responsabilité par module.

## À toi — lance-le

```powershell
cd C:\Users\alri\Documents\shimeji-desktop\spike
.\target\debug\spike-overlay.exe
```

⚠️ **Pour l'arrêter : `Ctrl+C` dans le terminal.** La fenêtre est volontairement non focalisable, sans bordure, hors taskbar et hors Alt+Tab — donc tu ne peux pas la fermer normalement. C'est le comportement voulu, mais ça se retourne contre toi au moment de quitter. En secours : `Stop-Process -Name spike-overlay`.

Tu devrais voir une silhouette blanche traverser l'écran en ondulant, et la console imprimer la topologie de tes moniteurs.

**Les 7 propriétés à vérifier** sont dans le plan (Tâche 3), et la grille à remplir t'attend dans `docs/specs/2026-09-08-spike-0-resultat.md`. Chaque échec possible a déjà son remède écrit — tu ne diagnostiques rien à l'aveugle.

Colle-moi la sortie console et ce que tu observes, je consigne le résultat et j'enchaîne sur le plan de l'étape 1.

---

## 👤 Alexis

Y'a bien tout dans le claude.md pour que je puisse lancer une nouvelle session la bas ?

---

## 🤖 Claude

Bonne question — je vérifie au lieu de te l'affirmer. Je relis ce qui est réellement dedans.

Non — **pas encore**, et c'est trois vrais trous. Je te les montre parce que chacun ferait perdre du temps à une session fraîche :

| Problème | Conséquence |
|---|---|
| La table dit `rustc / cargo ❌ absents` | **faux** — elle réinstallerait Rust |
| Aucune commande de build nulle part | elle devinerait, et raterait le piège du `Ctrl+C` |
| « **Aucun code n'existe** » | **faux** — le spike existe et compile |

Je corrige.

Oui, maintenant. Les trois trous sont bouchés et commités (`7b77450`).

## Ce qu'une session fraîche a désormais

| | |
|---|---|
| **Le besoin** | ce qu'il fait, où il vit, à quoi il réagit, + les 6 hors-sujets explicites |
| **La stack et son pourquoi** | Tauri/Rust vs Electron, la distribution |
| **La chaîne d'outils** | ✅ tout installé, avec les **3 pièges de ta machine** |
| **Compiler et lancer** | les commandes, + le piège du `Ctrl+C` |
| **Les conventions de code** | commenter abondamment, renvoyer à la spec |
| **L'architecture** | fenêtre par perso, logique en Rust, plateformes, 3 horloges |
| **Les 5 décisions à ne pas défaire** | chacune avec la classe de bugs qu'elle supprime |
| **Le format des personnages** | slots Shimeji, ancres, couverture partielle |
| **L'ordre de construction** | les 6 étapes |
| **Les 4 pièges Windows** | dont la bordure invisible de 7 px |
| **L'état réel** | le spike compile, la prochaine action, ce qu'il a déjà établi |

Les trois pièges valaient particulièrement d'y figurer — chacun m'a coûté un diagnostic pendant cette session, et une session fraîche les rencontrerait à l'identique :

1. **`vswhere` sans `-prerelease`** conclut à tort que le C++ est absent (il est sur VS 18 Insiders)
2. **Ne pas installer Node** — la voie npm de Tauri semble bloquée par ton Node 14, mais Node ne sert nulle part
3. **Compiler depuis PowerShell** — en Git Bash, rustc pêche le `link.exe` de Git et rend une erreur `extra operand` illisible

## Pour démarrer là-bas

```
Lis CLAUDE.md. Le spike de l'étape 0 compile mais les 7 vérifications visuelles
ne sont pas faites. Aide-moi à les faire et remplis la grille dans
docs/specs/2026-09-08-spike-0-resultat.md, puis écris le plan de l'étape 1.
```

CLAUDE.md fait 481 lignes, ce qui est dense mais tout est sectionné — et la moitié est du *pourquoi*, précisément ce qu'une session fraîche ne peut pas reconstituer seule.

Tu peux aussi simplement lancer le spike maintenant et me coller le résultat, si tu préfères finir l'étape 0 ici avant de basculer.

---

## 👤 Alexis

Tu peux pas mettre ta discussion dans le dossier la bas sinon pour que je puisse la retrouver stp ?

---

## 🤖 Claude

Deux choses différentes, et je peux faire les deux. Je regarde d'abord si le transcript brut de la session est accessible.

2,1 Mo de JSONL — complet mais illisible (résultats d'outils, images encodées, rappels système). Je vais en extraire **la discussion réelle** en Markdown lisible, plutôt que de te déposer un blob. Je regarde la structure.

Structure claire. J'extrais les messages **et les réponses aux questions à choix** — beaucoup de décisions se sont prises par là, il serait dommage de les perdre.

98 Ko lisibles, avec les 8 questions et les 8 décisions. Je vérifie que le nettoyage a bien fonctionné.

Un résidu : le **corps des skills** (brainstorming, writing-plans) a été injecté comme message utilisateur et se retrouve dans le fichier. Je le filtre.

---
