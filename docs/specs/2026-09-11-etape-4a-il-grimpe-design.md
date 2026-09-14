# Étape 4a — il grimpe les bords de l'écran

*Design validé le 2026-09-11. Il argumente depuis `docs/specs/2026-09-08-design.md`
(la spec), et suppose lues les cinq décisions de design de `CLAUDE.md`.*

---

## 1. Le périmètre, et ce qu'il exclut

L'ordre de construction prévoit une étape 4 « il grimpe » qui couvre **les bords des
fenêtres**. Ce lot-ci en est la **moitié basse**, décidée le 2026-09-11 : les bords de
**l'écran** seulement.

| Dans ce lot | Hors de ce lot |
|---|---|
| les deux murs et le plafond de chaque écran | les plateformes de **fenêtres** |
| il grimpe de lui-même (une envie tirée au sort) | le recensement des fenêtres à 8 Hz, `EnumWindows`, `DWMWA_CLOAKED` |
| il s'accroche quand on le **jette** contre un mur | `DwmGetWindowAttribute` / `DWMWA_EXTENDED_FRAME_BOUNDS` |
| il traverse le plafond, il redescend, il se lâche | la **soustraction d'intervalles 1D** (décision n° 2) |
| il passe d'un écran au voisin, au sol comme au plafond | l'étape 5 (« il suit » l'application au premier plan) |

**Pourquoi ce découpage tient debout.** L'étape 4 complète est chère à cause du monde
(énumérer les fenêtres, les filtrer, les désoccluter), pas à cause de l'escalade. Le
monde étant « une liste de plateformes » (spec §5.1), livrer d'abord les quatre bords
d'un écran donne l'escalade entière — physique, comportement, animations, transitions de
coin — contre une seule source de plateformes déjà disponible et déjà testée. Quand les
fenêtres arriveront, elles n'ajouteront que des `Platform` : **rien de ce document n'aura
à être réécrit.** C'est exactement la promesse que `world.rs` porte en commentaire depuis
l'étape 1.

> **Un mot sur la numérotation.** Ce lot s'appelle 4a et non 4 pour la même raison que
> 1a/1b : il est agréable en lui-même, il se solde par un commit, et il ne préempte rien.

---

## 2. Le monde : quatre plateformes par écran au lieu d'une

`World::from_screens` produit aujourd'hui **un** rectangle par écran — le sol, 1 px
d'épaisseur, face `Top`. Il en produira jusqu'à **quatre** :

| Plateforme | Rectangle | Face exposée | `point_on(face, off)` |
|---|---|---|---|
| sol *(existant)* | `(left, bottom, w, 1)` | `Top` | `(left + off, bottom)` — offset vers la droite |
| mur gauche | `(left − 1, top, 1, h)` | `Right` | `(left, top + off)` — offset vers le **bas** |
| mur droit | `(right, top, 1, h)` | `Left` | `(right, top + off)` — offset vers le bas |
| plafond | `(left, top − 1, w, 1)` | `Bottom` | `(left + off, top)` — offset vers la droite |

Tout est pris sur la **zone de travail**, jamais sur l'écran complet. C'est le piège
Windows n° 3 appliqué aux trois nouvelles faces : sur l'écran complet, il grimperait
derrière la barre des tâches.

### 2.1 Quatre rectangles fins plutôt qu'un grand rectangle à quatre faces

C'est le choix le plus difficile à défaire de ce document.

L'alternative évidente est de donner à chaque écran **un** `Platform` dont le `rect` est
la zone de travail entière et les `faces` les quatre côtés. Elle est séduisante — un
écran, une plateforme — et elle est **fausse** ici, pour une raison de sémantique : le
personnage se tient à l'*intérieur* de ce rectangle. Le sol deviendrait donc la face
`Bottom` et le plafond la face `Top`, ce qui inverse le sens de `Face::Top`, aujourd'hui
« le dessus, on marche dessus ». Il faudrait alors retoucher `nearest_floor`,
`atterrissage`, `face_voisine`, leurs tests, et le commentaire de `geom::Face` lui-même.

Avec des rectangles fins dont la face **pointe vers l'intérieur de l'écran**, le point
rendu par `point_on` est rigoureusement le même, et :

- `geom.rs` ne change **pas d'une ligne** ;
- `Face::Top` garde son sens unique ;
- `Attachment`, `world_position` et `hors_bornes` fonctionnent tels quels (§3.1).

Le prix est un `Rect` artificiel de 1 px d'épaisseur. C'est déjà ce que fait le sol
depuis l'étape 1, avec la constante `EPAISSEUR_DU_SOL` et sa justification : « le sol n'a
pas d'épaisseur réelle, mais un `Rect` en demande une ».

### 2.2 Les identités : quatre par écran

`PlatformId` vaut aujourd'hui le `HMONITOR`. Il en faut quatre, distincts et **toujours
indépendants de la géométrie** (spec §5.2, condition de la décision n° 1) :

```
PlatformId::ecran(monitor, role)  ==  PlatformId(monitor << 2 | role)
role ∈ { 0 = sol, 1 = mur gauche, 2 = mur droit, 3 = plafond }
```

`HMONITOR` et `HWND` sont des poignées en espace utilisateur, largement sous 2⁴⁷ sur
Windows x64 : décaler de deux bits ne perd rien et ne peut pas collisionner. Le test
`l_identite_survit_a_un_changement_de_resolution` reste vrai sans être modifié.

> **Alternative écartée** : faire de `PlatformId` une structure `{ source: u64, role: u8 }`.
> Plus explicite, mais elle touche toutes les signatures et tous les tests qui écrivent
> `PlatformId(999_999)`, pour un gain nul — le constructeur nommé donne déjà la lisibilité.

### 2.3 Les bords entre deux écrans ne sont pas des murs

Avant de pousser un mur, on vérifie qu'aucun **autre écran ne le touche** à ±8 px — la
tolérance de `face_voisine`, reprise telle quelle et pour la même raison (des résolutions
ou des échelles différentes peuvent laisser quelques pixels de jeu). S'il y a un voisin,
**le mur n'est pas créé du tout**.

```
   plafond A              plafond B
 ┌───────────────────┬───────────────────┐
 ║                   ┆                   ║     ║ = mur posé
 ║   écran A         ┆    écran B        ║     ┆ = mur PAS posé
 ║                   ┆                   ║         (bord partagé)
 └───────────────────┴───────────────────┘
     sol A                 sol B
```

Vu de l'utilisateur, le bureau entier se comporte donc comme **une seule boîte** : il
grimpe sur l'extérieur, et rien ne l'arrête au milieu. Vu du code, ce sont bien six
plateformes (2 sols + 2 plafonds + 2 murs).

**Le sol et le plafond restent en deux morceaux, et c'est volontaire.** Le sol l'est déjà
depuis l'étape 1 ; `face_voisine` fait passer de l'un à l'autre en marchant, c'est testé,
et fusionner les rectangles casserait l'identité par `HMONITOR` (débrancher un écran ne
ferait plus disparaître son sol). Le plafond reprend **le même mécanisme, généralisé aux
faces `Bottom`**.

> **Le cas assumé comme imparfait** : deux écrans dont les zones de travail n'ont pas le
> même haut. Les plafonds ne sont pas à la même hauteur, la tolérance refuse la jonction,
> et il fait demi-tour au milieu du plafond. Le traitement rigoureux est la soustraction
> d'intervalles 1D, explicitement réservée à l'étape 4 complète. Une adjacence
> **partielle** (deux écrans de hauteurs différentes) fait, par la même règle binaire,
> perdre tout le mur plutôt que sa moitié libre : conservateur, jamais de mur fantôme.

Le plafond, lui, n'est jamais supprimé : il n'y a rien au-dessus du bureau.

### 2.4 Ce que ce changement donne gratuitement

`hors_bornes` et `world_position` s'appliquent aux nouvelles faces sans une ligne de
plus. Un écran débranché, ou une zone de travail qui rétrécit parce qu'on a déplacé la
barre des tâches, fait **tomber** un personnage accroché à son mur — par le même test qui
le fait déjà tomber de son sol. C'est la décision n° 1 qui paie une deuxième fois.

---

## 3. L'accroche : le contact, et rien de plus

### 3.1 `Attachment` ne change pas

`On { platform, face, offset }` décrit déjà un mur : c'est `face: Face::Right` et un
offset qui compte vers le bas. Aucune variante à ajouter, et donc `world_position`,
`hors_bornes`, `window_top_left` et `position_conservant_le_sprite` restent intacts.

### 3.2 `atterrissage` devient `contact`

```
atterrissage(world, avant, apres) -> Option<(PlatformId, f32)>          // aujourd'hui
contact(world, avant, apres)      -> Option<(PlatformId, Face, f32)>    // demain
```

Trois règles, **dans cet ordre** :

1. **Le sol d'abord**, règle actuelle inchangée : on descend, on franchit la ligne `y` de
   la face, `x` d'arrivée est au-dessus de la face. Le code existant devient la première
   moitié de `contact`.
2. **Puis les murs** : on a franchi la ligne `x` de la face pendant le pas **dans le bon
   sens** — vers la gauche pour une face `Right`, vers la droite pour une face `Left` —
   et le `y` d'arrivée tombe dans la hauteur du mur. L'offset est `apres.y − rect.top()`.
3. **Le plafond attrape aussi.** Lancé vers le haut, il s'accroche au plafond exactement
   comme il s'accroche à un mur : on a franchi la ligne `y` de la face `Bottom` **en
   montant**, et le `x` d'arrivée tombe dans la largeur du plafond. L'offset est
   `apres.x − rect.left()`, vers la droite, comme au sol.

**Les deux premières règles sont relevées dans le source, pas inventées** (`Fall.java`) :

- `hasNext()` teste `getFloor().isOn(pos) || getWall().isOn(pos)` — donc un mur arrête
  une chute, exactement comme un sol, et **sans aucun seuil de vitesse** ;
- la boucle de sous-pas fait `break OUTER` sur le sol **avant** de tester le mur, d'où la
  priorité de la règle 1. Sans elle, un lancer dans le coin de l'écran s'accrocherait au
  mur trois pixels au-dessus du sol au lieu d'atterrir.

> **⚠️ La règle 3 diverge délibérément de `Fall.java`, et voici pourquoi il faut le
> savoir.** La version d'origine de ce document disait l'inverse — « le plafond
> n'attrape rien, lancé vers le haut il passe devant et retombe » — et c'était une
> lecture fidèle de la source : `hasNext()` ne teste que `getFloor()` et `getWall()`,
> jamais un plafond, qui n'existe d'ailleurs pas comme notion chez Shimeji-ee. Ce
> comportement a été **implémenté, puis essayé à l'écran** (2026-09-12), et l'auteur a
> préféré l'inverse : voir un personnage lancé vers le haut passer devant le plafond et
> retomber paraissait faux, là où l'accroche — cohérente avec celle d'un mur — paraissait
> juste. On assume donc de diverger de Shimeji-ee sur ce point précis. Le raisonnement
> d'origine reste vrai pour décrire `Fall.java` ; il ne s'applique simplement plus à ce
> projet. C'est le même geste que la correction du bug d'ordre du balancier (`CLAUDE.md`,
> §« Ce qui ne s'explique pas en commentaire ») : on garde la trace de la décision
> écartée plutôt que de réécrire l'histoire comme si le choix avait toujours été celui-ci.
>
> Techniquement, la règle 3 est le miroir de la règle 1 (le sol), pas des murs : on ne
> s'accroche qu'**en montant** (symétrique de « on ne s'accroche au sol qu'en
> descendant »), et l'offset compte vers la **droite** comme au sol — c'est la règle 2
> (les murs, avec leur offset vers le bas) qui est l'exception structurelle parmi les
> trois, pas le plafond.

### 3.3 Aucun nouveau réflexe

Le Réflexe 3 (la chute) appelle `contact` au lieu d'`atterrissage`, et pose `land`,
`grabWall` ou `grabCeiling` selon la face rendue — `grabCeiling` depuis que la règle 3
attrape (§3.2). **L'ordre des réflexes ne bouge pas** — et il ne doit pas : « porté »
passe avant « plateforme disparue » pour la raison consignée dans `reflex.rs`.

### 3.4 L'orientation sur un mur : il regarde le mur

Mur gauche → `Facing::Left`, mur droit → `Facing::Right`. Au plafond, l'orientation suit
le **sens du déplacement**, comme au sol. Quand il arrive en marchant,
c'est déjà son orientation ; quand il arrive par un lancer, il faut la poser
explicitement. Shimeji obtient le même résultat par omission : ses séquences
`WalkAndGrabBottomLeftWall` n'émettent aucun `Look` avant `ClimbWall`, le mascotte garde
le sens de sa marche.

### 3.5 Se lâcher

`Attachment::Falling { pos: la position courante, vel: Vec2::zero() }`. La gravité et les
frottements existants font le reste. `orienter_selon_la_chute` a une zone morte, donc une
vitesse horizontale nulle ne le retourne pas : il tombe droit, dans le sens où il était
accroché.

---

## 4. Le comportement : une intention qui possède tout le monde vertical

### 4.1 Une seule nouvelle intention, `Grimper`

`Flaner` continue de ne connaître **que le sol**.

L'alternative — généraliser `Flaner` à n'importe quelle face — est un piège :
`avancer` déplace l'offset *le long de la face courante*, donc un `Flaner` sur un mur
ferait monter et descendre le personnage en pose de marche, allure et demi-tours compris.
Il faudrait une pose par allure **et** par face. Séparer coûte une variante d'enum ;
fondre coûterait une matrice.

Quatre phases, sur le modèle de `PhaseRepos` qui existe déjà :

| Phase | Ce qu'il fait | Pose |
|---|---|---|
| `Rejoindre` | marche vers le mur le plus proche **de son écran** ; au bord, il s'accroche | `walk` |
| `Paroi { cible }` | l'offset va vers `cible`, à la vitesse d'escalade | `climbWall` |
| `Accroche` | il ne bouge plus, durée tirée | `grabWall` |
| `Plafond { cible }` | il traverse, s'il est arrivé en haut | `climbCeiling` |

La phase s'appelle `Paroi` et non `Monter` parce qu'elle sert dans les **deux sens** :
monter, c'est une cible plus petite que l'offset courant ; redescendre, une cible plus
grande. Elle ne porte pas non plus le nom de l'intention — `Intention::Grimper` et
`PhaseGrimpe::Paroi` doivent rester lisibles côte à côte dans un `match`. C'est la structure exacte de
`ClimbWall` chez Shimeji, dont les deux animations sont conditionnées par
`TargetY < mascot.anchor.y`. Une phase, pas deux.

Si l'écran courant n'a **aucun mur** (l'écran du milieu d'une rangée de trois), l'intention
échoue immédiatement et une autre est tirée. C'est le même esprit que la couverture
partielle : pas de cas particulier, une option qui n'existe pas.

### 4.2 Les transitions de coin sont des décisions, pas de la géométrie

Sol → mur en fin de `Rejoindre`, mur → plafond quand l'offset atteint 0, plafond → mur à
l'autre bout, mur → sol en fin de descente : tout cela vit dans l'intention, au même
endroit et pour la même raison que `face_voisine` — « ce sol en prolonge-t-il un autre ? »
n'a de sens que pour quelqu'un qui marche dessus.

`face_voisine` n'est généralisée que pour **un** cas : le plafond d'un écran au plafond du
voisin. Même règle, même tolérance de 8 px, même conséquence déjà acceptée en §2.3.

### 4.3 La sortie, tirée au sort

En fin d'`Accroche`, un `weighted` sur deux poids venus de `config.json` : **se lâcher**
ou **redescendre**. Décision n° 5 — on règle s'il est casse-cou ou prudent sans
recompiler, et le tirage est ce qui empêche de deviner la suite (décision n° 3 appliquée
à autre chose qu'un signal).

### 4.4 Le délai d'abandon passe à 120 s pour l'escalade

L'escalade va à **16 px/s** (§5) : un mur de 1032 px prend **64 s**, et la marche jusqu'au
bord en ajoute jusqu'à 19. Avec les 20 s actuelles, il abandonnerait toujours au tiers du
mur et **n'atteindrait jamais le plafond**.

`DELAI_ABANDON` devient donc une fonction de l'intention : **20 s par défaut, 120 s pour
`Grimper`**. La décision n° 4 écrit « ~20 s » avec un tilde ; c'est une règle de sécurité
anti-blocage, pas un trait de caractère, et elle n'a pas de raison d'être identique pour
une intention trois fois plus lente. Rien d'autre ne change.

### 4.5 La règle qui ferme le trou : fini sur un mur ⇒ il lâche

> **Toute intention qui se termine — finie, échouée, ou expirée — alors que le personnage
> est accroché à une face `Left`, `Right` ou `Bottom` le fait se lâcher.**

Sans elle, le tirage suivant peut sortir `Flaner`, et l'on retombe précisément sur le
personnage qui « marche » verticalement décrit en §4.1. Avec elle, le cas est
structurellement impossible : **le sol est le seul endroit où l'on peut ne rien faire.**

Elle couvre d'un coup l'expiration des 120 s, l'échec, le rechargement à chaud et le
retour de verrouillage. Et elle rend le délai d'abandon lisible à l'œil : au bout de deux
minutes il en a marre, il lâche, il tombe. C'est `FallFromWall` de Shimeji.

### 4.6 Ce qui s'ajoute ailleurs, sans logique nouvelle

- **`desire.rs`** : une ligne `Grimper`, poses requises `["grabWall", "climbWall"]`. Un
  pack sans ces frames ne grimpera **jamais**, sans un cas particulier — la couverture
  partielle joue toute seule (spec §8.6).
- **`config.json`** : `poids_grimper`, `vitesse_escalade`, `poids_lacher`,
  `poids_redescendre`, durée d'accroche. Toutes optionnelles, comme le reste.
- **`menu_perso.rs`** : une ligne dans `ENVIES`. ⚠️ **Dans la même tâche, pas « plus
  tard »** — c'est la règle de `CLAUDE.md`, et son oubli ne casse aucun test et ne produit
  aucun message.
- **`sim.rs`** : l'escalade comptée dans la trace, pour que `--sim` prouve qu'il grimpe
  sans écran.

### 4.7 Écarté : la flânerie qui grimpe au bord

La spec §6.3 autorise, au bord du sol, un tirage entre demi-tour, se laisser tomber et
s'accrocher. Avec l'envie `Grimper`, ce serait un **second chemin vers le même résultat**.
YAGNI : le demi-tour de `flaner` ne bouge pas.

---

## 5. Les constantes, et d'où elles viennent

Relevées dans `conf/actions.xml` et `Fall.java` de Shimeji-ee, **jamais réglées à l'œil** —
c'est la leçon explicite de l'étape 1a, où les quatre réglages devinés étaient tous faux.

| Constante | Valeur | Source |
|---|---|---|
| vitesse d'escalade | **16 px/s** | `ClimbWall` : 36 px de déplacement sur 56 ticks de 40 ms |
| durée d'accroche | 500–1500 ms | `HoldOntoWall` : `${500+Math.random()*1000}` |
| mur = ligne exacte | `workArea.left` / `.right` | `Wall.java::getX()` |
| seuil de vitesse pour s'accrocher | **aucun** | `Fall.java::hasNext()` ne teste que `isOn` |

> **Le calcul de la vitesse, en entier, parce qu'il est facile à rater.** `ClimbWall`
> enchaîne huit poses de durées 16, 4, 4, 4, 16, 4, 4, 4 ticks, de vitesses 0, −1, −1, −1,
> 0, −2, −2, −2 px/tick. Déplacement : `3×4×1 + 3×4×2 = 36 px`. Durée : `56 × 40 ms =
> 2,24 s`. Soit **16,1 px/s**, soit **trois fois plus lent que la marche** (50 px/s). Les
> deux longues poses immobiles de 16 ticks sont ce qui donne le « il se hisse » plutôt que
> « il glisse » : ne pas les lisser.

`characters/blob/mascot.json` déclare **déjà** `grabWall` (13), `climbWall` (14/12/13),
`grabCeiling` (23, ancre `64,48`) et `climbCeiling` (23/24/25). **Aucun travail de contenu
sur `blob`.**

> ⚠️ **Correction à porter dans `CLAUDE.md`** : sa table des frames dit « 23, 24, 25 →
> agripper une paroi verticale (escalade) ». C'est **faux** — ce sont les frames du
> *plafond* ; le mur, c'est 12/13/14. `docs/specs/2026-09-09-frames-shimeji.md` est juste
> et fait foi.

---

## 6. Le seul réglage qui se mesure et ne se décide pas : l'ancre sur un mur

`Wall.java` place le mur exactement sur `workArea.left`, et `isOn` exige
`ancre.x == mur.x`. Chez Shimeji, l'ancre `64,128` de `grabWall` tombe donc **pile sur le
bord**, et la moitié du sprite se retrouve hors écran.

Est-ce que c'est le rendu voulu ou un artefact qu'on ne veut pas reproduire ? **Ça ne se
décide pas sur document** — c'est exactement le genre de réglage que l'étape 1a a prouvé
indevinable, et que l'ajout du pack `luffy` a confirmé une seconde fois (« ne pas recopier
l'ancre de `blob` »).

La tâche correspondante du plan devra donc : le poser sur un mur, **regarder**, puis
ajuster l'ancre de `grabWall` et `climbWall` dans `mascot.json`. C'est de la **donnée** —
aucune recompilation, et surtout **aucune compensation de décalage dans `attach.rs`**, que
son en-tête interdit explicitement.

---

## 7. Comment on vérifie, sans écran

Le projet exige que tout ce qui demanderait un clic reçoive un équivalent scriptable.

| Ce qu'on veut prouver | Comment |
|---|---|
| deux écrans adjacents ne produisent que deux murs | test sur `World::from_screens` avec `FakeProbe::deux_ecrans` |
| un mur disparu fait tomber celui qui s'y accroche | test sur `hors_bornes`, déjà écrit pour le sol |
| un lancer contre un mur s'y accroche | test sur `contact`, segment traversant la ligne `x` |
| le sol gagne sur le mur dans un coin | test sur `contact` avec un segment qui franchit les deux |
| il monte, atteint le plafond, le traverse, redescend | `--sim`, en comptant les phases |
| il ne reste jamais « en marche sur un mur » | invariant vérifié à chaque image de `--sim` : pose ∈ {climbWall, grabWall, climbCeiling, grabCeiling} si la face est verticale |
| le CPU n'a pas bougé | `SHIMEJI_CACHE=1` en release, **la seule mesure comparable** |

> ⚠️ **Le rappel de la quatrième hypothèse.** L'escalade change la charge de travail :
> un personnage accroché ne déplace pas sa fenêtre, un personnage qui grimpe la déplace de
> 16 px/s au lieu de 50. Le CPU « en marche » mesurera donc le **comportement**, pas le
> code — il est **non comparable**, et seule la mesure en mode caché tranche.

---

## 8. Ce que ce lot laisse intact pour la suite

- les plateformes de **fenêtres** : elles n'ajouteront que des `Platform`, et tout le §3
  et le §4 s'y appliqueront sans modification ;
- la **soustraction d'intervalles 1D** (décision n° 2), qui remplacera la règle binaire du
  §2.3 le jour où elle existera — et améliorera au passage les écrans mal alignés ;
- l'**étape 5** (« il suit »), qui aura besoin d'une intention `AllerÀ(surface)` dont
  `Rejoindre` est déjà une version dégénérée à un seul pas.
