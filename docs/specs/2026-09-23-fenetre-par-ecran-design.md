# Une fenêtre par écran — la conception

**2026-09-23.** Remplace « une fenêtre par personnage » par « une fenêtre par
écran ». Mesures qui la fondent : `docs/specs/2026-09-22-spike-fenetre-par-ecran.md`.

---

## 1. Le problème, et ce qu'on achète

À 15 personnages, la file du thread principal prend **8 à 14 secondes** de
retard. L'utilisateur ne peut plus ouvrir le gestionnaire pour réduire son
roster : l'application est piégée dans l'état dont elle devrait permettre de
sortir.

Trois spikes ont cerné la cause, chacun en éliminant une explication :

| Spike | Ce qu'il a montré |
|---|---|
| `spike-deplacements-groupes` | grouper les messages ne change rien — le coût est **par fenêtre**, pas par message |
| `spike-deplacement-30hz` | espacer les déplacements divise la latence par 3 — elle reste en **secondes** |
| `spike-fenetre-par-ecran` | 3 fenêtres fixes + `eval` groupés : **27 ms** au lieu de 14 000 |

> ⚠️ **Ce qu'on achète est l'interactivité, PAS une économie de CPU.** À 15
> personnages sur 3 écrans : 91 % aujourd'hui contre ~105 % demain. Le total ne
> baisse pas. Ce qui change, c'est **où** il est dépensé — le thread principal
> passe de ~59 % à ~4 %, et ce thread est celui qui livre les clics.
>
> Une fenêtre **repeinte** à 60 Hz coûte ~34 % ; **déplacée**, ~12 %. Repeindre
> est plus cher par fenêtre. L'overlay ne gagne que parce qu'il en utilise 3 au
> lieu de 15 — et il gagne surtout parce que ces 34 % partent dans des
> processus qui ne bloquent rien.

Le bénéfice se formule donc ainsi : **un plafond plat au lieu d'une falaise.**

| personnages sur 1 écran | aujourd'hui | overlay |
|---|---|---|
| 1 | ~12 % | ~36 % |
| 4 | ~35 % | ~36 % |
| 15 | ~90 %, **figé** | ~36 %, fluide |
| 50 | impossible | ~36 % |

## 2. Les deux décisions écrites qu'on rouvre

Elles sont dans CLAUDE.md, elles ont été prises contre des bugs réels, et
l'auteur les rouvre en connaissance des mesures (2026-09-22).

### 2.1 « Ne pas revenir à un overlay transparent plein écran »

> *« une seule fenêtre ne peut pas couvrir proprement deux moniteurs de DPI
> différents, et c'est un défaut structurel »*

**La raison invoquée est le multi-DPI, et elle est réelle** : la machine de
l'auteur a trois écrans dont un portable à 125 %. Une fenêtre Windows n'a
**qu'un seul DPI à la fois** ; une fenêtre unique couvrant les trois rendrait
le pixel-art flou et mal dimensionné sur le portable.

**Une fenêtre PAR ÉCRAN échappe entièrement à cette raison** : chaque fenêtre
couvre un seul écran et prend son DPI. La décision visait la version « une
seule fenêtre pour tout » ; c'est elle qui reste interdite.

### 2.2 « Toute la logique en Rust, sans exception »

Le webview reçoit une position 15 fois par seconde et en dessine 60 : il
**interpole** entre deux positions reçues.

**Ce n'est pas de la logique.** Le JS ne calcule aucune trajectoire, ne décide
d'aucun déplacement, ne renvoie rien à Rust, et n'invente jamais une position :
le facteur d'interpolation est **borné à 1**, donc le sprite s'arrête sur la
dernière position connue si l'envoi suivant tarde. Il glisse entre deux vérités
que Rust a calculées — c'est du lissage, pas une vérité de plus.

Le prix, mesuré et jugé : **~66 ms de retard visuel**, invisible à l'œil de
l'auteur (2026-09-22).

> ⚠️ **Et la règle n° 1 de l'étape 0 (« 60 Hz est la cadence retenue ») reste
> respectée à la lettre ET dans l'esprit** : la physique tourne toujours à
> 60 Hz, et le **dessin aussi**. L'auteur a explicitement refusé le dessin à
> 30 Hz, jugé « un peu plus saccadé » — seul l'**envoi** est espacé, et c'est
> ce que la règle ne mentionne pas parce qu'à l'époque dessiner et déplacer
> étaient le même geste.

## 3. Le modèle : trois cadences découplées

| Quoi | Fréquence | Où |
|---|---|---|
| physique, comportement, signaux | **60 Hz** | Rust — **inchangé** |
| envoi des positions | **15 Hz** | Rust → webview, un `eval` par écran |
| dessin | **60 Hz** | le webview seul, `requestAnimationFrame` |

C'est le découplage qui fait tout : 44 `eval`/s au lieu de 900 `SetWindowPos`/s
sur la file, et 60 images/s à l'écran quand même.

## 4. Ce qui change, ce qui ne change pas

### Ne change pas — et c'est la décision n° 1 qui l'offre

Un personnage ne stocke jamais sa position absolue, seulement
`(plateforme, face, distance au bord)`. **Toute la physique, l'escalade, les
signaux, le monde, le comportement, la régulation de charge sont intacts.**
Seule la couche d'affichage change : au lieu d'écrire une position de fenêtre,
on écrit une position CSS.

### Change

| Aujourd'hui | Demain |
|---|---|
| N fenêtres 128×128, une par personnage | 1 fenêtre par **écran occupé**, à la taille de sa zone de travail |
| `render::placer` à chaque déplacement | une position dans la charge utile |
| `render::dimensionner` au changement de taille | une taille dans la charge utile |
| `render::pousser` (1 sprite) au changement de pose | `render::pousser_ecran` (N sprites) à 15 Hz |
| `traverser_les_clics` **par personnage** | par **écran**, vrai si le curseur est sur *un* personnage de cet écran |
| `ui/index.html` + `pet.js` (1 sprite) | `ui/overlay.html` + `overlay.js` (N sprites + lissage) |

## 5. Les cinq décisions de cette conception

### 5.1 Une fenêtre n'existe que si l'écran porte un personnage

Un écran vide ne doit rien coûter. La fenêtre est créée quand le premier
personnage arrive sur l'écran, détruite quand le dernier le quitte.

*Pourquoi :* le péage de ~34 % est **par fenêtre animée**. Sur une machine à
trois écrans dont un seul est occupé, c'est la différence entre 36 % et 107 %.

### 5.2 Un personnage à cheval sur deux écrans est dessiné dans les DEUX

Aujourd'hui sa fenêtre traverse librement la frontière. Avec une fenêtre par
écran, il serait **coupé** au bord — une régression visible.

Il est donc émis dans la charge utile de **chaque** écran que son rectangle
touche, aux coordonnées relatives de cet écran. Le sprite dépasse hors de la
fenêtre et se fait couper, mais l'autre moitié est dessinée par la fenêtre
voisine : les deux morceaux se rejoignent.

*Pourquoi pas « accepter la coupure » :* le multi-écran est un des trois
arguments qui ont fait choisir cette architecture. Le casser ici serait
absurde.

### 5.3 Le webview ne saute jamais un dessin inutile

La boucle de dessin n'écrit un `transform` que si la position **arrondie** a
changé. Un personnage endormi n'écrit rien ; un déplacement sous-pixellique non
plus.

*Pourquoi :* mesuré — 15 figurants immobiles coûtent 24 % contre 101 % en
mouvement. C'est ce qui rend gratuite la nuit, quand la régulation de charge a
tout endormi.

> ⚠️ **Mais un seul personnage en mouvement fait repeindre toute sa fenêtre.**
> Endormir 11 personnages sur 15 n'a rendu que 17 points, les 4 restants étant
> répartis sur les 3 écrans. Ne pas espérer de cette optimisation autre chose
> que le cas « tout le monde dort ».

### 5.4 Rust n'envoie rien quand rien n'a changé

Symétrique du point précédent, côté Rust : si aucun personnage de l'écran n'a
bougé ni changé de pose depuis le dernier envoi, **aucun `eval` n'est émis**.

*Pourquoi :* un `eval` coûte ~2,9 ms de CPU. 44/s en permanence, c'est ~13 %
payés pour rien quand tout le monde dort.

### 5.5 L'ordre de superposition entre personnages est l'ordre du DOM

Aujourd'hui c'est Windows qui arbitre entre fenêtres `TOPMOST`, sans qu'on le
contrôle. Demain c'est l'ordre d'insertion des `<img>`, donc l'ordre du roster.

*Pourquoi :* on ne perd rien — l'ordre actuel n'est ni choisi ni stable. Et on
gagne un contrôle qu'on n'avait pas, si l'étape 3b en a besoin.

## 6. Ce que cette conception ne traite pas

- **Le péage de ~34 % par écran animé.** On ne sait pas ce qu'il contient
  (échange de surface, transfert vers DWM, composition logicielle de WebView2).
  C'est la seule piste restante pour descendre plus bas, et elle n'est pas
  engagée.
- **L'étape 4b** (plateformes de fenêtres) et l'**étape 5** (suivre
  l'application au premier plan) : indépendantes, elles ne touchent que le
  monde et le comportement, que ce changement ne modifie pas.
- **Le comportement social** (étape 3b), toujours de côté.

## 7. Le critère de retour en arrière

Si, la migration faite, la latence mesurée à 15 personnages n'est **pas** sous
100 ms, ou si le CPU dépasse 120 % dans le cas « 15 personnages sur 3 écrans »,
la migration a échoué et la branche n'est pas fusionnée. Les deux se mesurent
avec `docs/outils/mesurer-roster.ps1` et `SHIMEJI_SIGNAUX=1`.
