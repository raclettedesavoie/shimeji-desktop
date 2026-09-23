# Une fenêtre par écran — la mesure

**2026-09-23.** Ce que la migration change, mesuré sur la configuration qui
échouait. Conception : `docs/specs/2026-09-23-fenetre-par-ecran-design.md`.
Plan : `docs/plans/2026-09-23-fenetre-par-ecran.md`.

Machine : 3 écrans (2× 1920×1080 à 100 %, portable 1920×1080 à **125 %**),
build **release**, Teams et Visual Studio ouverts en fond, 60 s par essai.

---

## 1. Le verdict

| 15 personnages | CPU total | dont `shimeji-desktop` | Latence de la file |
|---|---|---|---|
| **avant** (une fenêtre par personnage) | 91 % | **~59 %** | **8 000 à 14 000 ms** |
| **après** (une fenêtre par écran) | **85,7 %** | **6,2 %** | **0 ms médiane, 13 ms max** |

Critère de retour en arrière (conception §7) : latence sous 100 ms et CPU sous
120 %. **Les deux sont franchis largement.**

> **Le CPU total ne baisse presque pas — et c'était annoncé.** Ce qui change,
> c'est **où** il est dépensé : le thread principal passe de ~59 % à 6,2 %, et
> c'est lui qui livre les clics et ouvre les fenêtres. Le reste part dans des
> processus renderer qui ne bloquent rien.
>
> Ce n'est pas une économie, c'est un gain d'**interactivité**. Le présenter
> autrement serait malhonnête.

## 2. Le prix pour un petit roster

| | CPU total | dont `shimeji-desktop` | Latence |
|---|---|---|---|
| 1 personnage, avant | ~12 % | ~12 % | 0 ms |
| **1 personnage, après** | **18,1 %** | 2,6 % | 0 / 9 ms |

**L'overlay est plus cher en dessous de 3 ou 4 personnages**, parce qu'il y a
un péage fixe par écran animé (~34 % à 60 Hz de dessin, mesuré au spike). Le
point de croisement est vers 3 ou 4 personnages par écran ; au-delà, le coût
cesse de croître avec le roster.

C'est le troc accepté en connaissance de cause : un **plafond plat** au lieu
d'une **falaise**.

## 3. Le mode caché redevient gratuit

| 15 personnages, session verrouillée | CPU |
|---|---|
| total | **0,8 %** |

Aucune fenêtre n'existe : `repartir` n'émet rien quand rien n'est visible, donc
toutes les fenêtres d'écran se ferment. Mesuré par accident — la session s'est
verrouillée pendant un essai — et c'est la meilleure preuve que la règle §5.1
fonctionne.

## 4. Les deux défauts trouvés en cours de route

### 4.1 Les pixels physiques appliqués comme des pixels CSS

**Constaté à l'œil, et par aucun autre moyen.** Sur l'écran portable à 125 %,
des personnages « tombaient complètement » et il n'en restait que 11 sur 15.

Les sprites sont calculés en pixels **physiques**, et la fenêtre d'un écran est
posée en pixels physiques elle aussi — correct. Mais le webview dessine en
pixels **CSS**, qui valent `physique / scale`. Envoyer une position physique
telle quelle l'**étire de 25 %** sur cet écran : plus le personnage est à droite
ou en bas, plus il dérive, jusqu'à sortir par le bas.

> ⚠️ C'est le **piège n° 4 des « coordonnées »** de CLAUDE.md sous une forme
> nouvelle. La règle disait « n'appliquer le facteur d'échelle qu'au
> dimensionnement du sprite » ; elle supposait que la surface de dessin était
> l'écran. Ici la surface est une **fenêtre**, et elle impose sa propre unité.
>
> Aucun test ne pouvait l'attraper avant qu'il existe : `repartir` ne
> connaissait pas l'échelle. Quatre tests le verrouillent maintenant, dont
> l'échelle nulle — une division par zéro rendrait `inf`, puis un sprite absent
> **sans le moindre message**.

### 4.2 La course au démarrage, aggravée

`eval` rend `Ok` dès que le message est **posté**. Si `overlay.js` n'a pas
encore défini `window.poserTous`, le JavaScript lève une exception que
**personne n'observe**, et la charge est perdue en silence — pendant que le
dédoublonnage (§5.4) l'enregistre comme envoyée. L'écran resterait vide jusqu'au
prochain changement, c'est-à-dire **indéfiniment** pour un personnage endormi.

L'ancien `AMORCAGE` protégeait de la même course pour `render::pousser`. Il a
donc été **conservé et déplacé par écran**, pas supprimé comme code mort :
pendant 2 s après la création d'une fenêtre, tout est renvoyé à chaque tour sans
se fier au dédoublonnage.

## 5. Un délai de grâce, ajouté par prudence

Une fenêtre d'écran vide n'est pas fermée tout de suite, mais après **3 s**.

Créer une fenêtre WebView2 coûte des centaines de millisecondes, pendant
lesquelles les personnages de cet écran ne sont pas dessinés. Sans ce délai, un
personnage qui fait des allers-retours sur un bord d'écran ferait battre la
fenêtre voisine — et clignoter ses occupants.

Le coût de l'attente est nul ou presque : le péage se paie au **contenu qui
change**, pas à l'existence de la fenêtre (2,9 % mesurés pour trois fenêtres
immobiles, au spike).

## 5 bis. Le délai de grâce a été remplacé : les fenêtres restent ouvertes

Le délai de grâce réduisait le battement, mais **chaque fermeture gelait
encore brièvement** l'application — constaté à l'œil par l'auteur. Détruire
une fenêtre WebView2, puis la recréer quand un personnage revient, est un
travail lourd fait par le thread principal.

Les fenêtres restent donc ouvertes tant que les personnages sont visibles, et
`overlay.js` **suspend sa boucle de dessin** quand il n'a plus aucun sprite.

Mesuré processus par processus, un `blob` sur trois écrans, 45 s :

| Processus | CPU |
|---|---|
| renderer de la fenêtre avec le personnage | 10,3 % |
| renderer d'une fenêtre vide | **0,8 %** |
| renderer de l'autre fenêtre vide | **0,2 %** |
| processus GPU, partagé | 9,8 % |
| navigateur et utilitaires | 1,6 % |

**Deux fenêtres vides coûtent ~1 %.** Le prix du gel supprimé est négligeable.

> ⚠️ **Une première mesure disait 30,3 % contre 18,1 % avant**, soit 12
> points imputés aux fenêtres vides. C'était faux : l'écart venait du
> comportement du personnage pendant l'essai (il marchait plus ou moins). Seule
> la décomposition par processus isole le coût d'une fenêtre vide — c'est le
> piège « la mesure en marche ne se compare plus d'une version à l'autre » de
> CLAUDE.md, rencontré une fois de plus.

## 6. Les pièges de mesure rencontrés

> ⚠️ **Une session verrouillée fausse tout, en silence.** Deux essais
> successifs ont rendu 0,8 % et « aucune fenêtre » : l'écran était verrouillé,
> les personnages planqués, et rien ne l'indiquait dans le résumé. Vérifier
> `verrouillé false` dans la trace `SHIMEJI_SIGNAUX=1` avant de croire un
> chiffre bas.

> ⚠️ **`mesurer-roster.ps1` ne suffit plus.** Il ne relève que le processus
> principal, qui ne fait plus que 6 % du total. Utiliser
> `docs/outils/mesurer-overlay.ps1`, qui relève aussi l'arbre WebView2.

> ⚠️ **Une capture d'écran doit être prise par un processus conscient du
> DPI.** `CopyFromScreen` depuis un PowerShell ordinaire rend des pixels
> logiques en les présentant comme physiques — et l'on capture alors le mauvais
> écran, ce qui a coûté un faux diagnostic (« aucun personnage n'est affiché »,
> alors qu'ils étaient sur l'écran voisin). Appeler
> `SetProcessDpiAwarenessContext(-4)` d'abord.

## 7. Ce qui reste ouvert

- **Le péage de ~34 % par écran animé** n'a pas été attaqué. On ne sait pas ce
  qu'il contient (échange de surface, transfert vers DWM, composition logicielle
  de WebView2). C'est la seule piste pour descendre plus bas.
- **Un personnage à cheval sur deux écrans n'est absorbé que par un seul** : le
  bord lointain du sprite laisse passer le clic. Accepté.
- **L'ordre de superposition entre personnages** est désormais l'ordre du DOM,
  donc l'ordre du roster. Il n'était ni choisi ni stable avant.
