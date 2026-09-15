# Journal des etapes — ce que chacune a etabli

> Extrait de `CLAUDE.md` le 2026-09-14 pour alleger le contexte charge a chaque session.
> **Rien n'a ete reecrit.** CLAUDE.md ne garde que l'etat courant, les regles encore
> actives et la prochaine action ; le recit de chaque etape — et surtout les hypotheses
> fausses qu'elle a corrigees — est ici.
>
> Complementaire de `docs/conception/2026-09-08-journal-decisions.md`, qui dit *pourquoi*
> chaque decision a ete prise. Celui-ci dit *ce que l'execution a appris*.

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

> ✅ **Le dernier clic a été fait le 2026-09-10.** Chaque action du tray était déjà
> prouvée autrement (`--demarrage etat`, `recharger.txt`, `SHIMEJI_CACHE`,
> `SHIMEJI_QUITTER_APRES`), mais la **dépêche** du clic, non — c'est la seule chose du
> projet qui ne se script pas. Un clic sur « Quitter », sur le build release : processus
> disparu, aucun résidu. Les cinq entrées partageant le même gestionnaire, ce clic les
> couvre toutes.
>
> Détail utile pour la suite : **Windows 11 masque par défaut l'icône des applications
> qu'il ne connaît pas.** Elle est derrière le chevron `^` de la zone de notification,
> pas directement visible — ce qui se confond facilement avec « le tray ne s'installe
> pas ».

### La journée simulée — la preuve d'ensemble de l'étape 2 (2026-09-10)

`cargo run -- --sim 1440` déroule **24 h de comportement sans écran, sans horloge
réelle et sans humain**, contre une chronologie d'activité scriptée
(`sim::signaux_de_la_journee`) : présent 9 h-12 h, 14 h-18 h, 20 h-22 h ; absent le
reste — pause déjeuner, soirée, nuit. C'est **la vérification d'ensemble de
l'étape** : elle ne dit pas seulement qu'il dort, elle dit **quand**.

La sortie ajoute un histogramme du sommeil par heure locale, et c'est lui qui
prouve le signal, pas un total :

```
sommeil par heure :
  00 h  2930 s ########################
  ...
  05 h  3210 s ##########################
  ...
sommeil (9 h-11 h, il travaille) : ~0 s
```

Un test dédié, `sim::tests::une_journee_entiere_dort_au_bon_moment`, verrouille
quatre propriétés d'un coup (une seule simulation de 24 h pour les quatre — la
lancer quatre fois multiplierait par quatre son coût) :

1. il dort la nuit (2 h-4 h) ;
2. il dort **au bon moment** — la nuit reçoit plus de cinq fois le sommeil du matin
   (9 h-11 h, où il est censé être au clavier) ;
3. il se réveille (la chronologie compte cinq retours) ;
4. **la marge survit** — même avec ×8 sur le repos toute la nuit, il n'a pas dormi
   100 % du temps. C'est le test de la décision n° 3 : si le signal *commandait*
   au lieu de biaiser, aucun autre test du projet ne s'en apercevrait.

La chronologie est une **fonction pure de la minute** (pas d'état, pas
d'aléatoire) : deux exécutions de `--sim 1440` rendent donc la **même
signature**, comme pour n'importe quelle graine fixe.

> Ce test de 24 h coûte environ **7 s** à lui seul (5,2 millions d'images), contre
> 0,43 s pour tout le reste de la suite auparavant. Il reste sous le seuil de 10 s
> fixé pour cette tâche, donc il n'est **pas** marqué `#[ignore]` — mais c'est
> maintenant le test le plus lent du projet, à surveiller si la suite continue de
> grossir.

### L'étape 4a est faite (2026-09-11)

Les 7 tâches sont exécutées. Le personnage grimpe les murs et le plafond de **chaque
écran** (pas encore les fenêtres, voir plus bas), de lui-même ou parce qu'on l'a jeté
contre un bord, s'y accroche un temps tiré au sort, puis redescend ou se laisse tomber.
`World::from_screens` expose désormais quatre plateformes par écran (sol, deux murs,
plafond) au lieu d'une seule ; `geom.rs` est resté inchangé, exactement comme le design le
visait.

Une régression a été trouvée **par l'invariant du monde vertical** ajouté à `sim.rs` à la
dernière tâche — la même vérification qui aurait dû attraper « il marche sur un mur »
depuis le début, et qui a effectivement attrapé un bug réel, survécu à trois tâches et
leurs relectures : quand l'intention `Grimper` expirait à son délai d'abandon (120 s)
pendant que le personnage était encore accroché au plafond, il y restait accroché — et la
couche 3 lui repostait aussitôt un `Grimper` neuf, qui le faisait « marcher » en pose
`walk` le long de la face verticale. Corrigé par deux garde-fous complémentaires plutôt
qu'un seul : `lacher_si_accroche` (une fonction unique, appelée à la fois par la garde de
chaque image et par le point où le délai d'abandon efface l'intention — deux copies de la
même règle avaient justement fini par diverger) et une garde structurelle sur la phase
`Choisir`, qui refuse désormais de partir d'autre chose que `Face::Top`. **La leçon à
retenir : une règle de sécurité qui ne vit qu'à un seul des endroits où elle s'applique
finit par ne protéger qu'un des deux chemins.**

> ⚠️ **La mesure de l'ancre de `grabWall`/`climbWall` reste à faire par l'auteur.** Elle
> demande de REGARDER le personnage accroché à un mur — la seule vérification du projet
> qui ne se scripte pas. Marche à suivre : `cargo build` puis `cargo run` avec
> `SHIMEJI_ESCALADE=1` (voir le tableau des variables de diagnostic) — le personnage part
> grimper dès la première image, et la console trace chaque changement de phase ; si le
> rendu ne convient pas à l'œil, l'ancre se corrige dans `characters/blob/mascot.json`,
> jamais dans `attach.rs` (voir « Décisions de design à ne pas défaire », n° 1).


---

## Plusieurs personnages — 2026-09-15

L'étape « ils coexistent ». Pas « ils se remarquent » : le comportement social
reste de côté, à la demande de l'auteur.

### Le CPU a été mesuré AVANT d'être conçu, et le piège s'est refermé une cinquième fois

Le brief de l'auteur nommait lui-même le risque : « un personnage en marche
coûte 12 % d'un cœur, donc deux ~24 %, quatre ~48 % — dis-moi franchement s'il
faut plafonner ». La mesure a été faite **avant** d'écrire une ligne de design,
en lançant N instances du binaire release — le coût des déplacements étant par
fenêtre et par mouvement, N processus produisent la même charge que N
personnages, à notre calcul partagé près.

Le relevé brut :

| N | CPU total |
|---|---|
| 1 | 14,1 % |
| 2 | **11,8 %** |
| 4 | 43,8 % |

**Deux personnages ont coûté moins qu'un.** Le piège du dossier CPU, à
l'identique : le chiffre « en marche » mesure le **comportement** — combien de
fois le personnage a bougé — et pas le code. Il avait déjà piégé le projet
quatre fois ; la cinquième n'a coûté que dix minutes, parce que le dossier
prévenait.

Les mesures comparables ont donné la loi : **le coût est linéaire dans le
nombre de DÉPLACEMENTS, pas de fenêtres**. Quatre fenêtres en couche ne coûtent
pas plus cher par mouvement qu'une seule. La bonne variable n'était donc pas le
nombre de personnages mais **le nombre de personnages qui marchent** — et le
biais de l'étape 2 les fait beaucoup s'arrêter.

L'auteur a tranché en connaissance de cause : **aucun plafond**, un simple
avertissement à partir de 10.

### Et la mesure d'après implémentation a été meilleure que la prédiction

| Roster | Caché | En marche |
|---|---|---|
| 1 | 0,7 % | — |
| 4 | 0,9 % | — |
| **10** | **0,7 %** | **67,9 %**, cadence tenue à 58,8 img/s |

**Le mode caché est plat.** L'argument « la part partagée est payée une seule
fois par la boucle unique » n'était qu'un raisonnement d'architecture ; il est
maintenant chiffré — 0,7 % à dix personnages là où l'approximation par N
processus en prévoyait 8.

La loi s'est même révélée **pessimiste** : elle prédisait 98 % à N=10, on
mesure 67,9 %. Dix fenêtres pilotées par une boucle coûtent moins cher par
mouvement que dix processus.

**Conséquence directe : l'optimisation prévue n'a pas été écrite.** Grouper les
déplacements dans un `BeginDeferWindowPos` devait traiter un problème que la
mesure ne montre pas. L'écrire aurait été optimiser sur une intuition, dans le
chemin le plus sensible du programme — exactement ce que ce projet refuse
depuis l'étape 1b. L'idée reste consignée dans le design, avec son protocole.

### Ce qui était gratuit, et ce qui ne l'était pas

L'auteur pressentait que l'apparition en chute serait **gratuite**. C'était
juste, et pour la raison qu'il avait identifiée : `Attachment::Falling` existe
depuis l'étape 1, les réflexes à 60 Hz gèrent chute et atterrissage depuis
l'étape 4a. Le module `apparition.rs` ne fait que **choisir un écran et un x**.

Le seul vrai risque y était ailleurs, et il a été testé plutôt que supposé :
depuis le 2026-09-12, **le plafond attrape** (divergence assumée de
Shimeji-ee), et un personnage qui apparaît en haut de la zone de travail naît
à quelques pixels de sa face accrochable. Ce qui sauve, c'est que
`contact_plafond` n'attrape **qu'en montant**. Le test le verrouille : qui
assouplirait un jour cette condition casserait l'apparition, et l'apprendrait
en `cargo test` plutôt qu'à l'écran.

Le **départ**, lui, n'était pas gratuit. L'auteur demandait « une animation où
il plierait les jambes pour préparer son saut ». Vérification faite dans le
relevé des 46 poses : **cette frame n'existe pas** dans le vocabulaire Shimeji,
qui n'a que `jump` — la frame 22, une seule image. Le départ est donc composé
avec ce qui existe : `sit` 120 ms, `jump` 150 ms, `fall`. Et comme partout, la
couverture partielle s'applique seule — un pack sans `sit` ni `jump` tombe
directement, sans un cas particulier de plus.

Le point de conception du départ : il **court-circuite `behavior::pas`** au
lieu d'ajouter un état au comportement. Le réflexe d'atterrissage est non
négociable par définition ; y mettre un « sauf si je pars » aurait été le
premier pas vers un réflexe plein de cas particuliers.

### ⚠️ Le piège de la session : PowerShell a double-encodé un fichier source

Un `Get-Content | Set-Content -Encoding utf8` lancé pour corriger **un seul
commentaire** a relu tout `main.rs` en cp1252 puis l'a réécrit en UTF-8 : les
660 lignes accentuées sont devenues « mÃªme », et un BOM s'est ajouté en tête.

**Le code compilait, et les 299 tests passaient.** Seuls les commentaires
étaient touchés — c'est précisément ce qui rend ce défaut facile à ne pas voir,
et il n'a été repéré qu'en relisant une sortie de `grep`.

> **Règle qui en découle : ne jamais faire passer un fichier source par
> `Get-Content` / `Set-Content`.** Les éditions passent par les outils
> d'édition, ou par Python en UTF-8 explicite.

La réparation a demandé de reconstruire la table inverse de cp1252 à la main :
.NET laisse passer tels quels les cinq octets que cp1252 ne définit pas (0x81,
0x8D, 0x8F, 0x90, 0x9D), là où Python refuse de les encoder.

### Une trace qui mentait, et un défaut latent corrigé au passage

`SHIMEJI_CADENCE=1` annonçait « 550 % » de taux de déplacement à dix
personnages. Le pourcentage comparait des placements **tous acteurs confondus**
à un nombre d'images compté **une seule fois** : à dix personnages il ne
pouvait que dépasser 100 %. Il est désormais rapporté au nombre d'acteurs —
550 % à dix, c'était 55 % par acteur, soit exactement ce qu'on attendait.

Et `tray::basculer_visibilite` masquait **toutes** les fenêtres du programme,
donc aussi celle du catalogue — y compris quand c'est depuis elle qu'on venait
de désactiver quelqu'un. Sans conséquence tant qu'il n'y avait qu'une fenêtre ;
corrigé en filtrant sur le préfixe `pet-`.

### Ce que la généralisation a obligé à trancher

Trois règles qui n'existaient pas à un personnage, et qui auraient été des bugs
silencieux si on les avait laissées implicites :

1. **Un seul RNG, semé une fois.** Un `XorShift32::seeded(index)` par
   personnage serait retombé en plein dans le piège des graines séquentielles :
   tous seraient apparus au même `x` et auraient tiré la même première envie.
2. **Un seul personnage élu sous le curseur.** Sinon deux personnages
   superposés sont attrapés ensemble par un même clic et se suivent jusqu'au
   relâchement.
3. **Les labels de fenêtre ne sont jamais réutilisés.** Ils viennent d'un
   compteur monotone : retirer `pet-1` puis en ajouter un réattribuerait
   `pet-1` pendant que Windows détruit encore la fenêtre précédente.

Et le menu contextuel a demandé un soin particulier : son `continue` sautait
l'image entière parce que l'instant est périmé au retour du menu, qui bloque.
Le transformer en `continue` de la boucle `for` aurait étendu le défaut aux
N−1 autres personnages au lieu de le corriger. C'est un `break` plus un drapeau.


---

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

