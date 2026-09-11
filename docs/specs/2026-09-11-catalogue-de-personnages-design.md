# Le catalogue de personnages — design

> Statut : **design approuvé**, prêt pour un plan d'implémentation.
> Date : 2026-09-11. Branche de travail : `catalogue-de-personnages`, partant de
> `etape-2-il-reagit` — **pas** de `etape-4a-il-grimpe`, où la session parallèle
> réécrit la boucle 60 Hz. C'est la traduction en git de la contrainte de §6.
> Relève de la spec §8.1 (les personnages sont des fichiers externes) et §9.3.

---

## 1. Pourquoi

Aujourd'hui `characters/` contient six packs versionnés, 3,4 Mo. Le catalogue
complet de shimejis.xyz ferait ~2000 packs et plusieurs Go : intenable dans git.

**Mais le poids n'est pas la vraie raison.** Aucun sprite du dépôt n'est de nous,
et le `CLAUDE.md` en tire déjà la conséquence : le dépôt n'est pas publiable en
l'état. En ne livrant aucun sprite — ou presque — il le redevient.

L'utilisateur parcourt donc les shimejis disponibles **depuis l'application** et
installe ceux qu'il veut, à la manière de l'extension Chrome « Shimeji Browser
Extension ».

### Ce que ce document ne couvre pas

- **L'étape 3** (plusieurs personnages simultanés). Le catalogue est conçu pour
  l'accueillir sans changer de forme, il ne l'anticipe pas davantage.
- **Les étapes 4 et 5** (escalade, suivi de l'application active). Une session
  parallèle réécrit la boucle 60 Hz pour elles ; ce design **évite délibérément**
  `main.rs`, `attach.rs` et `render.rs`. C'est une contrainte de cadrage, pas une
  préférence — voir §6.
- La **désinstallation** et la **mise à jour** d'un pack. YAGNI : supprimer un
  dossier est un geste de l'explorateur, et l'entrée de tray « Ouvrir le dossier »
  existe déjà. À ajouter le jour où l'usage le réclame, en le sachant.

---

## 2. Les décisions de cadrage, déjà prises

Reprises ici pour que ce document se lise seul.

1. **Le catalogue seul, pas l'étape 3.** Il est presque entièrement en fichiers
   neufs et ne croise aucune autre session — c'est pour ça qu'il passe en premier.
2. **Deux gestes séparés.** Le clic dans le catalogue **installe** dans une
   bibliothèque locale ; un **second geste** choisit qui s'affiche. Contre
   « télécharger et utiliser aussitôt », qui confond deux intentions distinctes.
3. **Ouvrable depuis le tray ET depuis le clic droit sur le personnage.**
4. **Design calqué sur celui de shimejis.xyz**, jetons mesurés dans leur DOM.

---

## 3. Les faits vérifiés qui fondent ce design

Aucun n'est supposé. Chacun a été relevé avant d'écrire ce document.

| Fait | Où il a été vérifié |
|---|---|
| `"csp": null` — **aucune** politique n'est injectée, le webview peut charger le CDN directement | `src-tauri/tauri.conf.json` |
| `config::resoudre` rend **un seul** dossier, le premier trouvé ; il n'est pas extensible en « dépôt puis bibliothèque » | `config.rs:334` |
| Le gestionnaire `shime://` prend déjà le nom du personnage **depuis l'URL**, pas d'une variable figée : il sert donc déjà n'importe quel personnage du dossier | `main.rs:488` |
| Les **ancres** et les **hitbox** sont **déjà par pose** ; seule la taille est unique pour tout le pack | `manifest.rs:192`, `manifest.rs:258` |
| `set_size` existe déjà, isolé dans sa propre fonction, délibérément hors de la boucle 60 Hz | `render.rs:306` |
| Le redimensionnement est déjà gardé par `derniere_taille != Some(taille)`, donc déjà « sur changement seulement » | `main.rs:961` |
| `rechargement::preparer` prend **déjà** le nom du personnage en paramètre | `rechargement.rs` |
| `pet.js` lit `index.html#<perso>` **une seule fois** pour construire `BASE` | `ui/pet.js` |
| Le CDN sert les frames en `https://sprites.shimejis.xyz/directory/<slug>/img/shimeN.png`, plus un `actions.xml` par pack | `tools/recuperer-packs.ps1` |

### La mesure qui a tranché le point le plus lourd

`tools/recuperer-packs.ps1` ne se contente pas de télécharger : il **recompose une
toile** par pack — lecture des ancres dans l'`actions.xml`, alignement des frames
d'une même pose sur leur ancre, recadrage sur les pixels opaques, ré-encodage.

La question était : peut-on s'en passer et installer les PNG bruts ? Mesure sur
les quatre packs que ce script a produits :

| Pack | Toile produite |
|---|---|
| `group-finity-blank-guy` | 128×128 — identique à la source |
| `naruto-kakashi` | 128×128 — identique à la source |
| `one-piece-zoro-01` | **185×155** |
| `pierrot-54acb5` | **156×167** |

**Deux sur quatre sortent du 128×128.** Installer brut serait donc faux pour
environ la moitié du catalogue, avec un défaut visible : un tremblement d'un ou
deux pixels à l'intérieur d'une même pose. Le raccourci est écarté **sur preuve**.

La sortie retenue n'est ni le recadrage ni le brut, mais le modèle de
Shimeji-ee — **taille et ancre par frame, fenêtre qui s'adapte** (§6).

---

## 4. Architecture d'ensemble

> **La couture : parcourir, c'est de la présentation ; installer, c'est de la
> logique.**

Le webview fait le parcours (vignettes tirées du CDN, grille, recherche) ; Rust
fait **tout le réseau de l'installation** — c'est la part qui mérite des tests :
quelles frames existent, quelles poses en découlent, les ancres, la hitbox,
l'écriture atomique, l'échec à mi-parcours.

### Pourquoi ça ne contredit pas « le webview est un afficheur bête »

Cette règle du `CLAUDE.md` est justifiée par le **coût de l'IPC à 60 Hz** : faire
calculer la physique en JS coûterait un aller-retour soixante fois par seconde et
par personnage. **Un catalogue n'est pas un chemin 60 Hz.** La règle ne s'applique
pas ici, et il vaut mieux le dire que le contourner en silence.

L'alternative — Rust détient la liste, filtre en Rust, relaie les 2196 vignettes
par un schéma `shime://` étendu au distant, avec cache disque — a été écartée :
trois fois le code pour rendre testable la partie qui n'a pas de bugs intéressants
(afficher une grille d'images), et un cache de vignettes qui ramène par la fenêtre
les Go qu'on cherchait à sortir.

### Les modules

```
src-tauri/src/
├── catalogue/
│   ├── mod.rs            l'orchestration : slug → dossier installé
│   ├── index.rs          lire catalogue.json (embarqué dans ui/)
│   ├── reseau.rs         GET sur le CDN — la SEULE pièce qui touche au réseau
│   └── installation.rs   en-têtes PNG, actions.xml, poses retenues, mascot.json
├── config.rs             + dossier_bibliotheque(), dossier_du_personnage()
└── character/manifest.rs + la table `frames` (facultative)
ui/
├── catalogue.html        la fenêtre du catalogue
├── catalogue.js
└── catalogue.json        l'index pré-généré
```

Le découpage sert le test, pas l'esthétique : `installation.rs` est fait de
**fonctions pures** (octets → taille, pixels → hitbox, XML → ancres), et
`reseau.rs` passe derrière un trait sur le modèle de `probe/fake.rs`.

### Les dépendances

| Besoin | Choix | Pourquoi |
|---|---|---|
| HTTP | **WinHttp** via la crate `windows`, déjà épinglée en 0.61 | **aucune** dépendance nouvelle, aucune pile TLS embarquée. Même logique que le reste du projet : l'API Windows typée |
| PNG | la crate **`png`**, en **décodage seul, sur une image** | la hitbox se **mesure** (leçon de l'étape 1a). Pas `image` : on ne veut que le PNG, et l'exe fait 2,7 Mo — un objectif tenu de la spec §4 |
| XML | **aucune** | on lit deux attributs au motif, comme le script. Un parseur complet serait payer cher une robustesse sans usage |

---

## 5. Où vivent les personnages

### Trois dossiers, trois rôles

| Dossier | Rôle | Qui écrit |
|---|---|---|
| `characters/` du dépôt | `blob` seul, le personnage de référence du moteur | nous, à la main |
| `%APPDATA%\shimeji-desktop\characters\` | **la bibliothèque** — tout ce que le catalogue installe | le code, jamais l'humain |
| le CDN | le catalogue ; rien n'est conservé en local | personne |

### La règle de résolution

**`config::resoudre` n'est pas touché.** Ses appelants gardent exactement le
comportement qu'ils ont — c'est ce qui rend ce changement sans risque de
régression. Deux fonctions sont ajoutées à côté :

```rust
/// La bibliothèque : là où le catalogue INSTALLE. Toujours le même chemin,
/// jamais cherché — c'est une destination d'écriture, pas une ressource à
/// découvrir.
pub fn dossier_bibliotheque() -> Option<PathBuf>

/// Où trouver le personnage nommé `nom` : la bibliothèque D'ABORD, le
/// dossier livré ENSUITE.
pub fn dossier_du_personnage(nom: &str) -> Option<PathBuf>
```

**Bibliothèque d'abord, dépôt ensuite**, et c'est le seul ordre défendable : si
l'utilisateur installe un pack portant le nom d'un pack livré, c'est le sien qui
doit gagner. L'inverse produirait un « j'ai installé ce personnage et il ne se
passe rien » indébogable.

Ça referme le piège connu : en `cargo run`, `resoudre("characters")` trouvait
toujours le dépôt et masquait `%APPDATA%`. Ici les deux racines sont consultées,
dans un ordre fixe et écrit.

### Ce que ça oblige à changer

1. `main.rs:198` — `Manifest::load(&dossier.join(&nom))` devient
   `Manifest::load(&dossier_du_personnage(&nom)?)`.
2. Le gestionnaire `shime://` (`main.rs:488`) — même substitution. Sa validation
   du nom contre la traversée de chemin reste intacte, et devient d'autant plus
   nécessaire qu'elle protège désormais deux racines.
3. `rechargement::preparer` — prend le dossier **du personnage** au lieu du
   dossier parent plus le nom. Sa signature se simplifie.

---

## 6. La fenêtre adaptative : la donnée maintenant, le moteur plus tard

### Le modèle visé

Shimeji-ee donne à **chaque frame** sa propre taille et sa propre ancre, et la
fenêtre suit. C'est ce qui fait qu'un shimeji ne tremble pas alors que ses images
changent de taille — et ça fait disparaître tout le travail de pixels à
l'installation.

| | Avec recadrage | **Avec fenêtre adaptative** |
|---|---|---|
| Décoder 46 PNG | oui | **non** |
| Aligner, recomposer une toile | oui | **non** |
| Ré-encoder 46 PNG | oui | **non** |
| Lire la taille de chaque frame | — | l'**en-tête PNG**, 8 octets, sans décoder |
| Décoder une image | 46 | **1** (la hitbox) |

Bénéfice inattendu : **les octets écrits sur le disque sont exactement ceux du
CDN**. Un pack installé par l'application et un pack téléchargé à la main
deviennent identiques, donc comparables.

### La forme dans le manifeste

La taille et l'ancre sont des propriétés de **l'image**, pas de la pose — et une
même image sert dans plusieurs poses. Elles vont donc dans une table à part,
indexée par numéro de frame :

```json
"frames": {
  "1":  { "size": [128, 128], "anchor": [64, 128] },
  "23": { "size": [96, 140],  "anchor": [48, 4] }
}
```

**Tout est facultatif.** Image absente de la table → repli sur le `frameSize` du
manifeste et l'`anchor` de la pose, c'est-à-dire le comportement actuel exact.
`blob` n'est pas retouché et continue de marcher : le changement est **purement
additif côté données**, et c'est ce qui le rend sûr.

### Le découpage imposé par le cadrage

La consommation de cette table touche `attach.rs`, `render.rs` et la boucle de
`main.rs` — précisément le terrain des étapes 4 et 5, qu'une session parallèle
réécrit. Elle sort donc du périmètre.

| | Maintenant | Plus tard, tâche à part |
|---|---|---|
| `catalogue/` | ✅ neuf, ne croise rien | |
| L'installation **écrit** la table `frames` | ✅ | |
| `manifest.rs` la **lit** (champ facultatif, ignoré du moteur) | ✅ additif | |
| `attach.rs` / `render.rs` / `main.rs` la **consomment** | | ⬜ après stabilisation de la boucle |

Les packs installés aujourd'hui deviendront donc corrects **sans être
retéléchargés** : l'information est déjà sur le disque. La tâche différée se
réduit à brancher trois lectures sur une donnée déjà parsée et déjà testée.

### Ce que la tâche différée devra faire, pour mémoire

| Où | Aujourd'hui | Après |
|---|---|---|
| `attach.rs:157-163` | `pose.anchor`, largeur du manifeste | l'ancre et la largeur **de la frame courante** |
| `main.rs:960` | `window_size(&manifest, echelle)` | `window_size(&manifest, frame, echelle)` |

Le garde `derniere_taille != Some(taille)` reste tel quel : `set_size` ne sera
appelé qu'au changement de frame, **6 à 10 fois par seconde** au lieu de 60. La
discipline mesurée du projet est préservée sans être réécrite.

⚠️ **Le point délicat, et le seul.** `attach.rs:202-209` assure une continuité au
changement de **pose** : la position est recalculée pour que l'ancre reste au même
point du monde, sans quoi le personnage sauterait visuellement. Avec des ancres
par frame, cette continuité doit s'appliquer **à chaque changement de frame**.
L'oublier donne un sautillement d'un ou deux pixels à chaque image d'une
animation. C'est le premier test à écrire, et il se joue sans écran : une suite de
frames de tailles et d'ancres différentes doit laisser l'ancre monde immobile.

Hors périmètre de cette tâche différée : **pas de hitbox par frame**. Elle reste
par pose — elle sert au hit-testing du curseur, qui tolère quelques pixels
d'approximation. YAGNI.

Le CSS ne bouge pas : `#pet { width: 100%; height: 100% }` fait déjà remplir la
fenêtre au sprite, donc un dessin au 1:1 dès lors que la fenêtre est à la taille
de la frame.

---

## 7. L'index : `catalogue.json`

### Contenu

```json
{
  "genere_le": "2026-09-11",
  "source": "https://shimejis.xyz/directory",
  "packs": [
    { "slug": "one-piece-luffy-01", "nom": "Luffy", "franchise": "One Piece" },
    { "slug": "zoro-8bd774",        "nom": "Zoro",  "franchise": "Communauté" }
  ]
}
```

Trois champs. **Pas de nombre de frames** : le HTML du répertoire n'expose que les
`shime1.png`, donc le compte demanderait 2063 sondages du CDN pour une information
dont l'installation a besoin de toute façon, et qui vieillirait mal.
**Pas d'URL** non plus : elle se déduit du slug par une règle unique, écrite une
fois dans le code — un champ `url` par pack serait 2063 occasions de diverger.

Taille estimée : ~130 Ko. Il vit dans `ui/`, donc **embarqué dans le binaire**
avec le reste du front : aucun fichier à distribuer, aucun chemin à résoudre,
aucune absence à gérer.

### Génération

Par `tools/recuperer-packs.ps1`, réduit à un mode `-Index` : il tire le HTML du
répertoire (~1 Mo), en extrait les slugs par
`directory/([a-z0-9-]+)/img/shime1\.png`, et écrit le JSON **sans BOM**.

C'est un geste de **maintenance**, joué à la main pour rafraîchir le catalogue —
jamais un chemin d'exécution. C'est tout l'intérêt du choix : si leur markup
change, c'est le script qui casse, sur la machine du développeur, avec un message.
Pas le catalogue chez l'utilisateur.

Le script perd donc le téléchargement, que l'application fait désormais mieux et
sous test. **Une seule référence pour la logique d'installation**, en Rust ; deux
copies auraient divergé. L'équivalent en ligne de commande est rendu par
`cargo run -- --installer <slug>`, conformément à la règle du projet selon
laquelle tout ce qui demanderait un clic reçoit un équivalent scriptable.

### ⚠️ L'incertitude assumée

La **franchise** et le **nom lisible** ne se déduisent pas du slug de façon
fiable. Deux familles :

- les **427 packs structurés** (`one-piece-luffy-01`) : la franchise est un titre
  dans le HTML, au-dessus de la grille. **Hypothèse non vérifiée** — leur HTML n'a
  pas encore été lu.
- les **1636 dépôts communautaires** (`zoro-8bd774`) : pas de franchise. On retire
  le suffixe hexadécimal de 6 caractères, on capitalise → `Zoro`, franchise
  `Communauté`.

Si le titre se révèle inexploitable, le repli est **une seule liste alphabétique**
et la recherche par nom fait le travail que le regroupement aurait fait. Le
catalogue reste utilisable, il est juste moins joli. Cette incertitude est écrite
ici pour ne pas être découverte au milieu de l'implémentation.

---

## 8. L'installation

### Le déroulé

1. Valider le slug (§11).
2. Créer `%APPDATA%\…\characters\.<slug>.partiel\img\`.
3. Télécharger `shime1.png`, `shime2.png`… en continuant au-delà de 46 tant que
   les fichiers existent, et en s'arrêtant après deux absences consécutives —
   la règle du script actuel, conservée.
4. Pour chaque frame, lire la taille dans l'**en-tête PNG** (8 octets, aucun
   décodage).
5. Télécharger l'`actions.xml` du pack et y lire les ancres. S'il est absent ou
   illisible : ancre supposée au milieu-bas de chaque frame, la convention de
   Shimeji-ee, et le manifeste le **consigne** dans un champ de mesure.
6. Décoder **shime1** seul, pour mesurer la boîte opaque → la hitbox.
7. Écrire `mascot.json`, **sans BOM**, en ne déclarant **que les poses dont
   toutes les frames existent** (spec §8.7).
8. **Renommer** `.<slug>.partiel` en `<slug>`.

### Le renommage final n'est pas un détail

Une coupure réseau au milieu laisse un dossier au nom pointé, ignoré au
chargement, nettoyé au prochain essai. **Jamais un personnage qui charge à
moitié** — c'est le genre de panne qui se diagnostique très mal, parce qu'elle
ressemble à un bug du moteur.

### La couverture partielle n'est pas une erreur

Un pack sans images d'escalade ne grimpera jamais : l'option est simplement
retirée du tirage d'envies (spec §8.7). Aucun cas particulier à coder — il suffit
de ne pas déclarer la pose. C'est la règle qui rend les packs tiers utilisables
même incomplets, et elle est reprise telle quelle du script.

---

## 9. Le catalogue à l'écran

### Une fenêtre ordinaire, tout l'inverse de celle du personnage

La fenêtre du personnage est sans bordure, non focalisable, hors taskbar, clics
traversants. Le catalogue est **exactement le contraire** : décorée,
redimensionnable, focalisable, dans la barre des tâches. Il faut résister à la
tentation de réutiliser quoi que ce soit de l'autre.

Elle est créée **à la demande** et détruite à la fermeture, pas masquée : une
fenêtre WebView2 vivante coûte de la mémoire pour rien, et le catalogue s'ouvre
quelques fois dans une vie.

### Deux écrans, parce que deux gestes

C'est la décision de cadrage n° 2, rendue visible :

- **« Catalogue »** — la grille des packs, groupés par franchise. Clic sur une
  carte → **installer**. Une carte déjà installée porte une pastille et son clic
  ne fait rien.
- **« Ma bibliothèque »** — ce qui est sur le disque, `blob` compris. Clic sur une
  carte → **afficher celui-ci**.

Deux écrans, deux verbes. Personne ne peut confondre « je le télécharge » et
« je l'affiche », ce qui était tout le point.

### Les ~2200 vignettes, sans virtualisation

> Deux nombres circulent dans ce document et ils ne se contredisent pas :
> **2196 vignettes** comptées dans leur page (certains packs y figurent sous
> plusieurs entrées) pour **2063 slugs distincts** extraits du HTML. C'est le
> second qui fait foi pour `catalogue.json`, un slug donnant un dossier.

`<img loading="lazy">`. WebView2 est Chromium : le navigateur ne charge que ce qui
approche du viewport, nativement. **Zéro ligne de défilement virtuel** — on ne
réécrit pas un scroller, on déclare une intention et le moteur fait le travail.

Les vignettes **sont** les `shime1.png`, tirées directement du CDN. La CSP étant
nulle (§3), rien ne s'y oppose.

La recherche filtre 2063 entrées en mémoire : un `filter` sur un tableau.

### Jetons de design

Mesurés dans le DOM de shimejis.xyz, pas approchés :

| | |
|---|---|
| Fond | `rgb(36, 36, 36)` |
| Texte | `rgb(201, 201, 201)` |
| Police | pile système (`-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, …`) |
| Vignette | 128×128, `object-fit: contain`, `border-radius: 0` |
| Carte | 223×144, `padding: 8px`, fond transparent, sans bordure ni ombre |
| Grille | **flex wrap**, pas une CSS grid |
| Organisation | par franchise, alphabétique |

### La frontière IPC : trois commandes, un événement

```rust
#[tauri::command] async fn installer(slug: String) -> Result<(), String>
#[tauri::command] fn bibliotheque() -> Vec<PackInstalle>
#[tauri::command] fn choisir(nom: String) -> Result<(), String>
```

`catalogue.json` étant dans `ui/`, la liste se lit par un `fetch` local :
**aucune commande** pour ça, ce serait faire transiter 130 Ko par l'IPC pour une
donnée statique.

`installer` est `async` — 46 téléchargements prennent ~3 s, et une commande
bloquante figerait la fenêtre. Elle émet un événement `installation`
(`{slug, fait, total}`) pour la barre de progression. Un seul événement, une seule
forme.

---

## 10. Les points d'entrée, et le changement à chaud

### Un seul gestionnaire de menu, sans exception

Règle non négociable du projet : **un seul `on_menu_event` dans tout le
programme**, parce que Tauri livre chaque événement de menu à *tous* les
gestionnaires — une action branchée deux fois s'exécuterait deux fois, et deux
bascules s'annulent.

- `tray.rs` gagne une entrée « Catalogue de personnages… » ;
- `menu_perso.rs` gagne la même entrée ;
- les deux émettent **le même identifiant**, et `actions::executer` gagne **un
  seul cas**.

C'est l'application stricte de la règle « toute nouvelle action se branche au menu
du clic droit, dans la même tâche ».

### Changer de personnage à chaud

Le chemin existe déjà à 95 % : `rechargement::preparer` désigne **déjà** le
personnage par paramètre et transporte un `Manifest` complet jusqu'à la boucle ;
`main.rs:788` lui passe simplement toujours le même. `choisir(nom)` l'appelle
pour un autre personnage — après la simplification de signature de §5, en lui
donnant le dossier rendu par `dossier_du_personnage(nom)`. C'est tout côté Rust.

Le seul point dur est côté webview : `pet.js` lit `index.html#<perso>` **une seule
fois** au démarrage pour construire `BASE`. Après un changement, il demanderait
encore les images de l'ancien personnage — un personnage parfaitement animé avec
le mauvais dessin, exactement le scénario que le commentaire de `main.rs:272`
redoutait.

Correction minimale, dans un mécanisme existant : `window.recharger(version)`
prend un second argument, le nom, et recalcule `BASE`. **Une ligne de JS**, dans
la fonction faite pour ça.

### Le choix doit survivre au redémarrage

`choisir` écrit le nom dans `config.json`, sans quoi le geste ne sert à rien.
Deux pièges, traités explicitement :

1. **On réécrit le fichier réellement chargé**, dont on mémorise le chemin — pas
   un chemin recalculé. Sinon `resoudre` trouve celui du dépôt pendant que
   l'écriture va dans `%APPDATA%`, et le réglage paraît sans effet. C'est le
   piège de §5 sous un autre angle.
2. **Édition chirurgicale, pas sérialisation complète** : on relit en
   `serde_json::Value`, on remplace la seule clé `personnages`, on réécrit. Une
   sérialisation depuis `Config` perdrait toutes les clés inconnues et remettrait
   les valeurs par défaut partout — l'utilisateur verrait son fichier réglé à la
   main écrasé par un clic dans une autre fenêtre.

Écriture **sans BOM** dans les deux cas.

---

## 11. Vérification

Constante du projet depuis l'étape 0 : tout ce qui peut être faux se teste **sans
écran, sans réseau et sans clic**.

### Le réseau derrière un trait

`reseau.rs` expose un trait, sur le modèle de `probe/fake.rs`. Les tests
d'installation servent des octets **fabriqués en mémoire** : une installation se
joue de bout en bout — téléchargement, en-têtes, `actions.xml`, écriture du
`mascot.json` — sans sortir de la machine, en quelques millisecondes.

C'est ce qui rend testable le cas qui compte le plus : **l'échec à mi-parcours**.
Le réseau coupe à la frame 20 → `.slug.partiel` reste, `<slug>` n'existe pas, un
second essai réussit.

### Les fonctions pures

| Quoi | Testée sur |
|---|---|
| taille d'une frame depuis l'en-tête PNG | 8 octets fabriqués à la main, plus un fichier non-PNG qui doit être **refusé** |
| hitbox depuis la boîte opaque | une image minuscule construite en mémoire, alpha connu |
| ancres lues dans l'`actions.xml` | un XML en dur, plus le cas « absent » → repli documenté |
| poses retenues | un pack à trous → seules les poses complètes sont déclarées |
| `catalogue.json` | bien formé, mal formé, **avec BOM** |
| la table `frames` du manifeste | présente, absente (→ repli, `blob` inchangé) |

### ⚠️ Le piège de test évité d'avance

`dossier_du_personnage` dépend de `%APPDATA%`, lu par `env::var`. Le tester en
modifiant la variable d'environnement serait **global au processus**, donc
instable quand `cargo test` tourne en parallèle — le genre d'échec qui n'arrive
qu'une fois sur dix et coûte une soirée.

Donc la fonction publique reste une coquille de trois lignes qui résout les deux
racines et délègue à une fonction **paramétrée par les deux chemins**, seule
testée. **Aucune variable d'environnement dans aucun test.**

### L'équivalent scriptable

`cargo run -- --installer <slug>` fait une vraie installation, vrai réseau, et
imprime le chemin obtenu. Il remplace le clic dans la grille, et vérifie le seul
morceau que les tests ne couvrent pas : **que le CDN réponde bien ce qu'on
croit**.

### Ce qu'on ne mesure pas, et pourquoi

**Pas de mesure CPU.** La boucle 60 Hz n'est pas touchée — c'est tout l'objet du
découpage de §6 — et la fenêtre du catalogue est transitoire. Les chiffres de
référence du `CLAUDE.md` restent valables tels quels.

Lancer une mesure « en marche » pour se rassurer serait une faute de méthode : la
section « la méthodologie AVANT les chiffres » du `CLAUDE.md` explique pourquoi
elle ne prouverait rien. On mesurera le jour où la fenêtre adaptative arrivera,
par `SHIMEJI_CACHE=1`, la seule configuration où la charge ne dépend pas du
comportement.

### Sûreté

Le slug vient d'un fichier embarqué, mais il construit un **chemin** et une
**URL**. Il est donc validé avant les deux, sur le modèle de la validation qui
protège déjà le schéma `shime://` (`main.rs:476`) : caractères anodins seulement,
jamais de `..`, et l'écriture reste sous la bibliothèque.

---

## 12. Le sort des six packs versionnés

| Pack | Devenir |
|---|---|
| `blob` | **conservé** — le personnage de référence du moteur, 196 Ko, et le seul recours sans réseau |
| `luffy`, `naruto-kakashi`, `one-piece-zoro-01`, `pierrot-54acb5`, `group-finity-blank-guy` | **retirés** — 3,2 Mo, tous sous droits |

Les cinq retirés restent **installables en un clic** : ils sont dans le catalogue.
Rien n'est perdu, c'est déplacé du dépôt vers la bibliothèque.

Le retrait se fait **dans la même tâche que l'arrivée du catalogue**, jamais
avant : la branche ne doit jamais passer par un état où l'application n'a plus
rien à afficher.

⚠️ Un `config.json` local peut nommer un des cinq (`personnages: ["luffy"]`).
Après le retrait, ce nom ne résout plus et l'application s'arrête sur
`personnage « luffy » illisible` — le comportement actuel de `main.rs:201`.
Acceptable et bruyant, mais **à vérifier sur la config locale avant de commiter le
retrait**.

⚠️ Le gain juridique reste **partiel** : `blob` est la mascotte de Shimeji-ee et
son art n'est pas de nous non plus. Un dépôt totalement net demanderait de le
sortir aussi — mais l'application sans réseau n'aurait alors **aucun** personnage,
et le premier lancement tomberait sur un écran vide. Arbitrage assumé.

---

## 13. Résumé des fichiers touchés

| Fichier | Nature |
|---|---|
| `src-tauri/src/catalogue/{mod,index,reseau,installation}.rs` | **neufs** |
| `ui/{catalogue.html,catalogue.js,catalogue.json}` | **neufs** |
| `src-tauri/src/config.rs` | +2 fonctions ; `resoudre` **intact** |
| `src-tauri/src/character/manifest.rs` | +1 champ facultatif (`frames`) |
| `src-tauri/src/{tray,menu_perso,actions}.rs` | +1 entrée de menu, +1 cas |
| `src-tauri/src/rechargement.rs` | signature simplifiée |
| `src-tauri/src/main.rs` | 2 substitutions d'appel + l'enregistrement des commandes. **Pas la boucle 60 Hz.** |
| `ui/pet.js` | 1 ligne (`recharger` recalcule `BASE`) |
| `tools/recuperer-packs.ps1` | réduit au mode `-Index` |
| `characters/` | 5 packs retirés |

**`attach.rs`, `render.rs` et la boucle 60 Hz de `main.rs` ne sont pas touchés** —
c'est la contrainte de cadrage, et elle est vérifiable d'un coup d'œil au diff.
