# Ne pas se superposer — conception

*2026-09-24. Demande de l'auteur, conçue avec lui question par question.*

## 1. Le besoin

> « Quand par exemple on fait s'asseoir tout le monde ou monter sur le mur,
> j'aimerais que les personnages ne se superposent pas : ils font une file pour
> monter sur le mur et montent un par un, et pour s'asseoir ils ne s'asseyent
> pas au même endroit. De manière générale je trouve ça cool qu'ils ne se
> superposent pas, même s'ils ont toujours le droit de se traverser. »

**La règle, en une phrase : on se traverse toujours, on ne s'arrête jamais
l'un sur l'autre.**

Ce que l'auteur a tranché :

| Question | Réponse |
|---|---|
| Au mur, quand le suivant se lance-t-il ? | quand le précédent est monté **d'une hauteur de corps** |
| Espacement à l'arrêt | **côte à côte**, silhouettes presque jointes (une rangée sur un banc) |
| Lâché à la souris sur un autre | il **se décale** : la règle vaut aussi après son geste |
| Sur les murs et au plafond | **on se traverse**, même accroché — seule l'**entrée** sur le mur se fait un par un |
| En plus | **« Tout le monde › Rester accroché »** |

Réussi, c'est : « Tout le monde › S'asseoir », et l'on voit une rangée au lieu
d'un tas ; « Tout le monde › Grimper », et l'on voit une file au pied du mur
qui avance, et une chenille qui monte.

**Écarté** : une vraie hitbox où ils se bousculent (l'auteur l'a évoquée, puis
écartée) — elle contredirait « ils se traversent », et demanderait une physique
de contact que le projet n'a pas. Et des places réservées d'avance (« siège
numéroté ») : de la navigation calculée, que la décision n° 4 exclut, et qui ne
couvrirait pas leur vie ordinaire hors des ordres.

## 2. Qui occupe une place

Une **place** est un intervalle le long d'une face de plateforme : l'offset du
personnage ± une demi-largeur de corps. Même unité que la décision n° 1
(plateforme, face, distance au bord) — aucune coordonnée d'écran.

La **largeur de corps** est celle de la hitbox de sa pose `stand`, multipliée
par son échelle d'affichage : `stand` plutôt que la pose courante, pour que la
largeur ne change pas quand il s'assoit ou s'endort (la rangée ne se
réarrangerait pas à chaque changement de pose).

| Où | Qui occupe |
|---|---|
| **au sol** (face `Top`) | ceux qui sont **à l'arrêt** — voir ci-dessous. Celui qui marche n'occupe rien : c'est ce qui garde les traversées |
| sur un mur, au plafond | **personne**. On s'y traverse librement |

**À l'arrêt** = tout ce qui ne déplace pas le personnage : `SeReposer` (assis,
endormi, se levant, étalé), `Jouer`, `Flaner` en allure `Arret`, et l'attente
dans la file d'un mur (§4). Pas en chute, pas porté.

## 3. Qui cède

Quand deux places se chevauchent, **le dernier arrivé** bouge. Chaque
personnage retient depuis quand il est à l'arrêt :

```rust
// character/mod.rs
pub arrete_depuis: Option<Duration>,
```

Tenu par `behavior::pas` : posé à la première image où il est à l'arrêt au sol,
effacé dès qu'il marche, tombe ou est attrapé. **Pour la session seulement**,
comme les tenues.

Un occupant **plus ancien** est celui dont `arrete_depuis` est plus petit ; à
égalité (deux arrêts dans la même image), le plus petit numéro d'acteur. C'est
déterministe, sans hasard.

Ce seul principe couvre : s'asseoir là où quelqu'un est déjà assis, être lâché
sur quelqu'un, deux arrêts simultanés.

**Céder, c'est** marcher (pose `walk`) jusqu'à la **place libre la plus
proche** sur la même plateforme, puis reprendre l'intention en cours. Sans place
libre (écran plein), il reste où il est : **un chevauchement accepté vaut mieux
qu'une errance**.

### Où ça se branche

Dans `behavior::pas`, **avant** la couche 2 (`poursuivre`), à un seul endroit :
si l'intention en cours est à l'arrêt, au sol, et qu'un occupant plus ancien
chevauche sa place, il fait un pas vers la place libre et l'image s'arrête là.

- `flaner`, `se_reposer`, `jouer` ne changent **pas** : une future activité à
  l'arrêt en profitera d'office.
- Les minuteries de l'intention continuent de courir pendant qu'il se décale :
  une pause de 5 s dont il a passé 1 s à se décaler dure 4 s sur place. Rien de
  visible, et aucun état de plus.

## 4. La file au pied du mur

Seule la phase `Rejoindre` de `intention::grimper` change.

**Le mur est libre** quand aucun personnage ne se tient, sur ce mur, à moins
d'une **hauteur de corps** du bas. (C'est la seule chose qu'un personnage voit
du mur : la zone de départ.) Il s'accroche alors comme aujourd'hui.

**Le mur est pris** : au lieu de marcher jusqu'au mur, il marche jusqu'à **la
place libre au sol la plus proche du mur**, et s'y arrête, face au mur, pose
`stand`. Pendant cette attente il **occupe** sa place (§2) — c'est ce qui range
les suivants derrière lui, en rangée.

**La file avance d'elle-même** : il vise à chaque image la place libre la plus
proche du mur ; quand le premier s'accroche, sa place se libère, et chacun
glisse d'un cran.

**L'attente ne compte pas dans le délai d'abandon** de l'escalade — sinon le
dixième renoncerait avant son tour. Mais une file bloquée pour de bon reste
possible (un personnage qui ne monte plus) : au-delà de **2 minutes
d'attente**, il abandonne et part flâner. La navigation a le droit d'échouer
(décision n° 4).

Sur un écran à deux murs, chacun va au mur le plus proche : deux files, qui
vident la foule deux fois plus vite.

## 5. « Tout le monde › Rester accroché »

Une ligne de plus dans la table `TOUS`, `Commande::Basculer(Tenue::ResterAccroche)`,
avec une coche comme les autres actions tenues.

- **Déjà sur un mur ou au plafond** : il s'arrête sur place et y reste — le
  « Rester accroché » personnel, pour tous.
- **Au sol** : il part au mur (file comprise), monte jusqu'à une hauteur tirée,
  et s'y fige. Aujourd'hui, un `ResterAccroche` reçu au sol est **refusé**
  (`peut_tenir`, `Imposer`) : il devient « va au mur, puis reste accroché ».
- **Décocher** les relâche tous : chacun reprend une escalade ordinaire, et
  redescend quand elle finit.

## 6. La plomberie

**`behavior/place.rs`**, nouveau, pur (ni Tauri, ni écran) :

```rust
pub struct Occupant {
    pub acteur: u32,
    pub platform: PlatformId,
    pub face: Face,
    pub debut: f32,           // offset − demi-largeur
    pub fin: f32,             // offset + demi-largeur
    pub arrete_depuis: Option<Duration>,  // None = en mouvement
}

/// La place libre la plus proche de `offset` sur cette face, ou `None`.
pub fn place_libre(…) -> Option<f32>;
/// Le bas de ce mur est-il libre ?
pub fn bas_du_mur_libre(…) -> bool;
```

La liste contient **tous** les personnages posés (`Attachment::On`), en
mouvement compris ; chaque question filtre ce qui la concerne. `place_libre` ne
compte que ceux **à l'arrêt, sur la même face de sol** (§2) ;
`bas_du_mur_libre` compte **tous** ceux qui sont sur ce mur, en mouvement
compris — c'est la zone de départ, la seule contrainte verticale (§4).

**`behavior::pas_parmi(…, autres: &[Occupant])`**, nouveau. L'actuelle `pas`
devient `pas_parmi(…, &[])` — un personnage seul au monde : **la simulation et
les tests existants ne changent pas**. Seule la boucle de `main.rs` appelle
`pas_parmi`.

**La boucle** construit les occupants une fois par image, avant de faire
avancer les personnages : une passe sur les acteurs (hors départs), O(n).
Chacun reçoit la liste entière et s'y ignore lui-même par son numéro.

## 7. Tests

- **`place.rs`**, sur des cas écrits à la main : place libre à gauche, à
  droite, aucune place, sol trop court, égalité départagée par le numéro.
- **Un harnais à plusieurs**, qui fait avancer N personnages ensemble comme la
  boucle (occupants recalculés à chaque image), et ces scénarios :
  - 5 × « Tout le monde › S'asseoir » partis du même point → au bout de 20 s,
    tous assis, aucun chevauchement ;
  - 5 × « Tout le monde › Grimper » → jamais deux dans la zone de départ d'un
    mur, et tous finissent sur le mur ;
  - un personnage lâché sur un autre, assis → il se décale ;
  - deux personnages qui se croisent en marchant → aucun ne dévie (la règle
    « on se traverse » est verrouillée) ;
  - « Tout le monde › Rester accroché » depuis le sol → tous figés au mur ;
  - une file bloquée → abandon après 2 minutes, pas avant.
- **La matrice** `toute_action_du_menu_se_fait_quel_que_soit_l_etat_de_depart`
  passe toujours, et couvre d'office la nouvelle ligne.

## 8. Le CPU

À 15 personnages : 15 × 15 comparaisons d'intervalles par image, rien face au
budget de 16,7 ms. Vérifié quand même avec `SHIMEJI_CADENCE=1` (travail par
image), comparé à avant — **sans** toucher à l'instance release de l'auteur.

## 9. Hors périmètre

- L'étape 3b complète : qu'ils se remarquent et réagissent l'un à l'autre.
- Les fenêtres comme plateformes (étape 4b) : la règle s'y appliquera
  d'elle-même, puisqu'elle ne parle que de plateformes et de faces.
- Une physique de collision.
