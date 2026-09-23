# Le menu sur mesure et les actions tenues — conception

*2026-09-23. Brainstorming avec l'auteur, validé section par section.*

## 1. Ce qu'on veut

Trois demandes de l'auteur, traitées ensemble parce qu'elles partagent la
même table (`ENVIES`) et le même menu :

1. **Des actions qui durent.** « S'asseoir » au clic droit : il reste assis
   jusqu'à ce qu'on le déplace, ou qu'on décoche l'action. Pareil pour
   presque toutes les actions, sauf les gestes ponctuels.
2. **Un menu en deux blocs** : ce personnage, puis **Tout le monde**, avec
   les mêmes actions — un intitulé de section, pas « tout le monde » répété
   à chaque ligne. « Tout le monde au mur » quitte le tray.
3. **Un menu au style de celui de l'extension Shimeji pour navigateur** :
   fond pêche clair, coins arrondis, ombre douce, lignes aérées, police
   arrondie, séparateurs fins, défilement si la liste est longue.

**Hors de cette spec** : les nouvelles actions (ramper, s'allonger, se
dédoubler), et les actions de navigation de l'extension (*walk left and
sit*, *grab left wall*, *pull up shimeji*) — **écartées** par l'auteur.

### Critère de réussite

- Un personnage assis au menu est **encore assis** cinq minutes plus tard.
- L'attraper à la souris lui rend sa vie normale.
- Le menu ressemble à celui de l'extension, sur les trois écrans et quel que
  soit leur DPI.
- Pendant que le menu est ouvert, **seul** le personnage cliqué s'arrête —
  acquis le 2026-09-23 avec le menu natif, et à ne pas perdre.

## 2. Les actions tenues

| Action | Où | Nature |
|---|---|---|
| S'asseoir | sol | **tenue** |
| Balancer les jambes | sol | **tenue** |
| Flâner | sol | **tenue** — il ne fait que se promener |
| Grimper au mur | sol, mur | **tenue** — il vit sur les murs et le plafond |
| Rester accroché | mur, plafond | **tenue** — immobile |
| Faire tourner la tête | sol | ponctuelle |
| Redescendre | mur | ponctuelle — **efface** « Grimper au mur » |
| Se lâcher | mur, plafond | ponctuelle — **efface** l'action tenue |

« **Monter plus haut** » disparaît : « Grimper au mur » tenu le rend
redondant. Au mur, le menu montre « Grimper au mur » déjà coché.

### 2.1 Le modèle

Chaque personnage porte une **action tenue** facultative :

```rust
// Sur `Character` : `None` = sa vie normale, tirée au sort.
pub tenue: Option<Tenue>,

pub enum Tenue { Asseoir, BalancerLesJambes, Flaner, Grimper, ResterAccroche }
```

Quand son intention en cours se termine et qu'une action est tenue, la
couche 3 (le tirage d'envies) **n'est pas consultée** : l'intention suivante
est celle de l'action tenue. C'est le seul endroit où le code change de
chemin. Les couches 1 (réflexes) et 2 (intention) restent telles quelles.

**Pas de seconde vérité** : la coche du menu se **déduit** de `tenue`, elle
n'est stockée nulle part ailleurs — la même règle que l'interrupteur de la
bibliothèque.

**La session seulement** : `tenue` n'est pas écrite dans `config.json`. Au
redémarrage, chacun reprend sa vie normale.

### 2.2 Ce qui l'efface

- **L'attraper à la souris** : le réflexe « attrapé → porté » efface
  `tenue`. Le lâcher le fait tomber, puis il reprend sa vie normale.
- **Décocher** l'action au menu.
- **Choisir une autre action**, tenue ou ponctuelle. « Balancer les jambes »
  choisi pendant qu'il est assis remplace l'une par l'autre ; « Faire
  tourner la tête » efface l'action tenue, puis il reprend sa vie normale.
- **Redescendre / Se lâcher** effacent l'action tenue au mur.
- **Une action devenue impossible** : pack rechargé sans ses poses, plateforme
  disparue. La règle de `behavior::pas` qui refuse une commande impossible
  vaut aussi pour une action tenue : on l'efface, on ne la force pas.

### 2.3 Face aux signaux (décision n° 3)

Une action tenue est un **ordre de l'utilisateur**, pas un signal système :
elle ne contredit pas « les signaux biaisent, ils ne commandent pas ».

- **Au sol** : quand l'utilisateur est inactif, il **s'assoupit sans se
  lever** — la pose endormie, puis le retour à l'action tenue quand
  l'utilisateur revient. Elle reste cochée. Il obéit, et il a toujours l'air
  vivant. Un personnage qui flâne s'assoit puis s'endort de même, et reprend
  sa promenade au réveil.
- **Au mur et au plafond** : **rien ne l'interrompt**, décision de l'auteur.

### 2.4 « Grimper au mur » tenu

Il monte, redescend, traverse le plafond, marque des pauses — mais ne
**repasse jamais au sol de lui-même**. Concrètement, dans `grimper()`
(`intention.rs`) : la sortie de la phase `Accroche`, qui tire aujourd'hui
entre continuer, redescendre et se lâcher, **ne tire plus que les sorties
qui le gardent sur la paroi** ; et une descente de `Paroi` qui atteint le bas
repart vers le haut au lieu de poser le personnage au sol.

Les **pauses** dépendent du défaut d'unité de `DUREE_ACCROCHE` (§6) : elles
ne deviendront visibles qu'une fois ce défaut corrigé.

## 3. Le contenu du menu

`menu_perso::lignes` reste la **description pure** du menu, testable sans
écran. Elle gagne deux choses :

```rust
pub enum Ligne {
    Entree { id: &'static str, libelle: &'static str, coche: bool },
    Titre(&'static str),
    Separateur,
}
```

Et elle reçoit ce qu'il faut pour calculer les coches : la `tenue` du
personnage cliqué, et celle de **tous** les présents.

**L'ordre du menu :**

1. Les actions de **ce personnage**, là où il est (sol / mur / plafond —
   `menu_perso::Ou`, inchangé). La coche vient de `tenue`.
2. `Titre("Tout le monde")`, puis les **actions du sol** seulement. Les
   autres personnages sont n'importe où, et « Se lâcher » n'a de sens que
   pour qui est accroché.
   - **Coché** quand *tous* les présents tiennent cette action.
   - **Cliquer** une action non cochée la donne à tous ceux qui peuvent
     l'exécuter (un pack sans escalade ignore « Grimper au mur »).
   - **Cliquer** une action cochée l'efface chez tous.
3. Cacher ce personnage · Cacher tous les personnages · Catalogue.
4. Séparateur, puis Quitter.

**Le tray** perd « Tout le monde au mur ». L'identifiant `ID_TOUS_AU_MUR`
disparaît avec lui, remplacé par les entrées `tous.*` de la section.

**Les identifiants** : une action de la section « Tout le monde » a son
propre identifiant (`tous.asseoir`…), décodé vers la boîte
`Actions::pour_tous`. Un clic sur une entrée du personnage continue d'aller
au **seul demandeur** (`menu_perso::commande_pour`) — la séparation des deux
boîtes est ce qui empêche N personnages d'exécuter l'envie d'un seul.

La règle de CLAUDE.md « toute nouvelle action se branche au menu » tient
toujours : une ligne d'`ENVIES`. Elle y gagne une colonne : tenue ou
ponctuelle.

## 4. L'affichage : une fenêtre webview dédiée

### 4.1 Pourquoi celle-ci, et pas une autre

| Approche | Verdict |
|---|---|
| **Une fenêtre webview dédiée, créée une fois au démarrage** | **retenue** : le style de l'extension en HTML/CSS |
| Dessiner le menu dans la fenêtre d'écran existante | écartée : elle laisse passer les clics et ne prend jamais le focus, donc le « clic ailleurs » est acrobatique, et un menu près d'un bord est coupé |
| Menu Win32 dessiné par son propriétaire | écartée : on ne change que couleurs et police, au prix de beaucoup de code de dessin ; ni coins arrondis ni ombre |

⚠️ **Créée une seule fois, cachée, puis montrée** — jamais créée à chaque
clic droit. Créer une fenêtre WebView2 est un travail lourd du thread
principal : c'est exactement le gel constaté à la fermeture des fenêtres
d'écran (CLAUDE.md, « Une fenêtre par ÉCRAN »).

### 4.2 La fenêtre

- `menu.html` + `menu.css` + `menu.js` dans `ui/`, **sans framework ni
  bundler** — le parti pris du projet.
- Transparente et sans bordure, pour que le CSS fasse les coins arrondis et
  l'ombre ; `always_on_top`, `skip_taskbar`, `WS_EX_TOOLWINDOW`.
- **Elle prend le focus**, contrairement aux fenêtres d'écran : c'est ce qui
  permet de la fermer quand on clique ailleurs (`blur`). Le focus est rendu
  ensuite à la fenêtre qui l'avait, avec les fonctions déjà écrites pour le
  menu natif (`render::fenetre_au_premier_plan`, `prendre_le_premier_plan`,
  `rendre_le_premier_plan`).
- Elle se ferme sur : un choix, `blur`, Échap.

### 4.3 Le trajet d'un clic droit

1. La boucle 60 Hz détecte le clic droit sur le personnage (inchangé).
2. Elle construit `lignes` et les envoie à la fenêtre du menu, avec la
   position du curseur : un `eval` de `window.afficherMenu(lignes, x, y)`.
3. `menu.js` dessine les lignes, **mesure** la hauteur du menu, puis
   demande à Rust de placer la fenêtre là, **repliée à l'intérieur de
   l'écran du curseur** — un menu ouvert près du bas s'ouvre vers le haut.
4. L'utilisateur clique. `menu.js` appelle une commande Tauri
   (`choisir_entree_menu(id)`), qui repasse par le **même**
   `actions::executer`. Ce n'est pas un second `on_menu_event`.
5. La fermeture, quelle qu'en soit la cause, lève le drapeau que la boucle
   attend déjà (`menu_en_cours`), et le personnage cliqué repart.

⚠️ **Pixels et DPI** : la position du curseur est en pixels physiques, la
fenêtre se place en pixels physiques, mais le CSS mesure en pixels CSS. La
conversion est celle d'`overlay::repartir`, avec l'échelle de l'écran du
curseur. Tester sur l'écran du portable, qui est à 125 %.

### 4.4 Ce que devient le menu natif

`menu_natif.rs` est **retiré** une fois la fenêtre en place : garder deux
affichages serait garder deux chemins à tenir d'accord. Son en-tête explique
pourquoi le menu de Tauri figeait tout le monde. Ce savoir passe dans
l'en-tête du nouveau module, parce que la fenêtre webview ne doit pas non
plus être ouverte depuis un callback qui bloquerait `tao`.

## 5. Les tests

Sans écran, comme le reste du projet :

- **`lignes`** : les coches suivent `tenue` ; « Tout le monde » coché si et
  seulement si tous les présents tiennent l'action ; aucun séparateur ni
  titre mal placé ; « Monter plus haut » absent ; « Tout le monde au mur »
  absent du tray.
- **Actions tenues, en simulation** (horloge factice, graine unique,
  CLAUDE.md « Semer l'aléatoire une seule fois ») :
  - assis au-delà de `DELAI_ABANDON` (20 s) → toujours assis ;
  - attrapé → `tenue` effacée ;
  - utilisateur inactif → pose endormie, puis retour à l'action tenue au
    retour, toujours cochée ;
  - au mur, inactivité → rien ne change ;
  - « Grimper au mur » tenu : sur une longue simulation, il ne pose
    **jamais** le pied au sol.
- **La fenêtre du menu** : sa construction et la commande de choix se
  vérifient par une variable d'environnement de diagnostic
  (`SHIMEJI_MENU_OUVERT=1` ouvre le menu au démarrage) — la règle du projet
  selon laquelle tout ce qui demande un clic a un équivalent scriptable. Le
  **rendu**, lui, se juge à l'œil par l'auteur.
- **CPU** : une fenêtre cachée de plus. La mesurer avec
  `docs/outils/mesurer-overlay.ps1` et comparer aux chiffres de CLAUDE.md.

## 6. Défauts constatés pendant cette conception — à traiter À PART

Hors de cette spec, chacun avec sa propre recherche de cause :

1. **`DUREE_ACCROCHE` : une erreur d'unité.** `HoldOntoWall` vaut
   `Duration="${500+Math.random()*1000}"`, en **ticks de 40 ms**
   (`2026-09-09-frames-shimeji.md`, ligne 29) — donc **20 à 60 s**. Le code a
   lu des millisecondes et en a tiré 0,5 à 1,5 s : c'est pourquoi il ne
   s'arrête jamais au mur. **Vérifier toutes les durées converties de la même
   façon** : la même lecture a pu être faite ailleurs.
2. **Sur le mur de gauche, il est dessiné comme sur un mur de droite** —
   presque entièrement hors de l'écran.
3. **L'écran du milieu n'a aucun mur** : les bords communs à deux écrans ne
   sont pas des murs, ce qui est voulu, mais `mur_le_plus_proche`
   (`intention.rs`) ne cherche que sur l'écran du personnage. Proposition :
   s'il n'y en a aucun, viser le plus proche sur n'importe quel écran.
