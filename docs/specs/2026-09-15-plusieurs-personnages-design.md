# Plusieurs personnages simultanés — design

> Étape suivante après le catalogue. Quatre choses, dans l'ordre d'importance
> donné par l'auteur :
>
> 1. plusieurs personnages vivent à l'écran en même temps ;
> 2. dans « Ma bibliothèque », un interrupteur et un compteur par personnage
>    l'ajoutent ou le retirent de ceux qui vivent — « choisir » ne REMPLACE
>    plus ;
> 3. une poubelle sur chaque carte supprime le pack du disque, définitivement ;
> 4. un personnage qui s'active apparaît en TOMBANT du haut de l'écran, à un
>    `x` aléatoire.

---

## 1. Ce que ce document ne couvre pas

**Le comportement social reste hors sujet.** C'était l'étape 3 du plan de
construction, et elle reste de côté à la demande de l'auteur. Les personnages
**coexistent** ; ils ne se remarquent pas, ne s'approchent pas, ne réagissent
pas l'un à l'autre. L'intention « aller voir l'autre » de la spec §7.4 n'est
pas implémentée ici.

Ce n'est pas un oubli, et c'est une bonne nouvelle pour plus tard : Rust
détenant tous les personnages dans un seul `Vec` (§3), l'étape 3 deviendra une
vérification sur ce `Vec`, sans aucun canal à inventer entre fenêtres.

Ne sont pas couverts non plus : les plateformes de fenêtres (étape 4b, fusionnée
puis revertée délibérément sur cette branche), et le suivi de l'application au
premier plan (étape 5).

---

## 2. ⚠️ Le CPU : la mesure d'abord, parce que c'est le vrai risque

**Un desktop pet qui consomme se fait désinstaller.** C'est le risque n° 1 de
cette étape, et il a donc été mesuré **avant** d'écrire une ligne de design,
comme le veut `docs/specs/2026-09-09-mesure-cpu.md`.

### Le protocole, et pourquoi il ne demandait pas d'implémenter d'abord

Le coût des déplacements est **par fenêtre et par mouvement**. On peut donc le
chiffrer sans avoir écrit le code de N personnages : **N instances du binaire
release**, chacune animant son personnage, produisent exactement la charge de
déplacement que produira une boucle unique pilotant N personnages — plus
(N−1) fois notre calcul partagé, qui est connu et soustrayable.

Le script de mesure est conservé dans `docs/outils/mesurer-n.ps1` et
`docs/outils/mesurer-normalise.ps1`.

### ⚠️ La première mesure a reproduit le piège du dossier CPU, à l'identique

| Relevé brut « en marche », 60 s, build release | CPU total |
|---|---|
| N=1 | **14,1 %** |
| N=2 | **11,8 %** |
| N=4 | 43,8 % |

**N=2 a coûté MOINS que N=1.** Ce n'est pas une anomalie, c'est exactement ce
que le dossier CPU annonce depuis l'étape 2 : le CPU « en marche » mesure le
**comportement** — combien de fois le personnage a bougé — et non le code. Le
taux de déplacement varie plus que le nombre de personnages. Le relevé brut est
**inutilisable**, et c'est la cinquième fois que ce piège se referme sur ce
projet.

### Les deux mesures qui, elles, se comparent

**Première : le mode caché**, où la charge est identique par construction
puisque rien n'est jamais déplacé.

| Configuration | CPU total | Par personnage |
|---|---|---|
| N=4, `SHIMEJI_CACHE=1`, release | **3,3 %** | **0,8 %** |

Notre calcul complet — sonde du curseur, hit-testing, physique, comportement,
les cinq signaux — coûte **0,8 % par personnage**, exactement la référence de
l'étape 4a. Et dans le design retenu (§3), la part *partagée* de ces 0,8 %
(signaux à 2 Hz, recensement à 8 Hz, sonde du curseur) est payée **une seule
fois** : la mesure ci-dessus est donc une borne haute.

**Seconde : la normalisation par le nombre de placements.** `SHIMEJI_CADENCE=1`
donne les déplacements réellement effectués ; le coût par déplacement, lui, se
compare d'une mesure à l'autre. Build debug, que le dossier CPU autorise pour
cet usage (12,3 % debug contre 12 % release — l'écart est dans le bruit).

| | CPU | Placements/s | Taux | **Coût par (placement/s)** |
|---|---|---|---|---|
| N=1 | 16,6 % | 45 | 73 % | **0,351 point** |
| N=4 | 29,8 % | 97 | 38 % | **0,274 point** |

### ⚠️ Le verdict, et l'hypothèse qu'il dément

**Le coût est linéaire dans le nombre de DÉPLACEMENTS, pas dans le nombre de
fenêtres** — et même très légèrement décroissant par déplacement à N=4.

Quatre fenêtres en couche ne coûtent donc **pas** plus cher, par mouvement,
qu'une seule : Windows ne paie pas de surcoût de composition à les avoir
côte à côte. L'hypothèse de départ — « deux personnages qui marchent coûteront
~24 %, quatre ~48 % » — avait la bonne intuition mais **la mauvaise variable**.
La variable n'est pas le nombre de personnages :

> **C'est le nombre de personnages qui MARCHENT.** Un personnage assis, endormi,
> accroché à un mur ou simplement à l'arrêt ne déplace pas sa fenêtre et coûte
> **zéro**. Le biais de l'étape 2 les fait beaucoup s'arrêter, et c'est ce qui
> explique le taux de 38 % observé à N=4 contre 73 % à N=1.

**La loi mesurée**, à retenir pour toute décision ultérieure :

```
CPU (% d'un cœur) ≈ 0,9 + 0,3 × (placements par seconde, tous personnages confondus)
```

Soit, par personnage : ~18 % s'il marchait sans jamais s'arrêter, **~7 % au
taux moyen réellement observé**.

### La décision : pas de plafond, un avertissement à partir de 10

**Décision de l'auteur, prise en connaissance de la mesure :** aucun plafond
dur. Le nombre de personnages actifs n'est pas borné par le code. À partir de
**10 personnages actifs**, la fenêtre de la bibliothèque affiche un
avertissement disant que la consommation peut devenir importante, et
l'application l'écrit aussi sur sa sortie standard — l'équivalent scriptable de
l'avertissement.

L'avertissement **n'interdit rien** : il ne désactive aucun bouton, ne demande
aucune confirmation, ne se ferme pas par un réglage. C'est un panneau, pas une
barrière.

> ### ⚠️ La réserve, consignée ici plutôt que re-débattue
>
> À 10 personnages, la loi mesurée donne ~72 % d'un cœur au taux observé, et
> **au-delà d'un cœur entier si tous marchent en même temps**.
>
> Le point dur n'est alors plus la consommation mais la **file de
> déplacements** : chaque `set_position` est dispatché au thread principal de
> Tauri. Notre boucle, elle, ne bloque pas et tiendra ses 60 Hz ; c'est le
> thread principal qui peut prendre du retard, et le symptôme serait des
> personnages qui traînent derrière leur position calculée — pas une chute de
> cadence visible dans `SHIMEJI_CADENCE`.
>
> **Ce comportement n'est pas mesuré à ce jour.** Le plan en fait une tâche de
> mesure à N=10, dont le résultat sera consigné ici. Rien n'en est déduit
> d'avance.

### La piste d'optimisation, non promise

Une seule piste peut aplatir la courbe, et elle n'a **pas** été mesurée :

1. appeler `SetWindowPos` **directement depuis le thread de la boucle** sur le
   `HWND`, au lieu de passer par `set_position` qui dispatche au thread
   principal — une traversée de moins par déplacement ;
2. et surtout grouper les N déplacements d'une image dans **une seule
   transaction** `BeginDeferWindowPos` / `DeferWindowPos` / `EndDeferWindowPos`,
   pour que Windows fasse **une** passe de composition au lieu de N.

C'est la seule idée qui s'attaque au terme dominant. Le plan en fait une tâche
**avec go/no-go sur mesure** : si le gain ne se voit pas sur 60 s, le code est
jeté et la mesure consignée. On n'applique pas une optimisation non mesurée —
c'est la règle du projet, et quatre hypothèses évidentes y sont déjà mortes.

---

## 3. Architecture : une boucle, N personnages

### Les trois options, et pourquoi celle-ci

| Option | Écartée parce que |
|---|---|
| **Un thread par personnage** — la boucle actuelle presque inchangée | Paierait N fois la sonde de signaux, N fois le recensement du monde, N fois la sonde du curseur. Ruineux quand l'étape 4b reviendra avec `EnumWindows`. Et il faudrait répliquer visibilité, verrouillage et rechargement dans chaque thread |
| **Un coordinateur + N threads** lisant un état partagé | Toute la machinerie de verrous lus à 60 Hz × N, pour aucun bénéfice que l'option retenue n'ait déjà |
| **✅ Une boucle, un `Vec<Acteur>`** | — |

La boucle unique est d'ailleurs **ce que `CLAUDE.md` décrit déjà** : « Rust
détenant *tous* les personnages, le comportement social est une simple
vérification côté coordinateur ». On ne choisit pas une architecture, on
applique celle qui était prévue.

### Ce qui est partagé, ce qui est par personnage

`boucle` a aujourd'hui une vingtaine de variables locales. La généralisation
consiste d'abord à les trier, et ce tri **est** le design :

| Reste une locale de `boucle` (partagé) | Entre dans `Acteur` (par personnage) |
|---|---|
| `monde`, `ecrans_echelle`, `echelle_affichage` | `ch: Character` |
| `biais`, `utilisateur_actif`, `verrouille` | `dernier_rendu`, `derniere_taille`, `dernier_coin` |
| `rng` | `clics_traversent` |
| `reglages`, `table`, `config_courante` | `derniere_trace_grimpe` |
| les horloges (2 Hz, 8 Hz) et les compteurs de cadence | `label`, `nom` |
| la sonde, la souris, `bouton_droit_precedent` | `depart` (§6) |

```rust
/// Un personnage à l'écran : sa fenêtre, son état, et le peu de mémoire de
/// rendu qu'il faut pour n'appeler Windows que sur changement.
struct Acteur {
    /// Le label de sa fenêtre Tauri. Jamais réutilisé — voir plus bas.
    label: String,
    /// Le pack dont il est une instance. Plusieurs acteurs peuvent partager
    /// le même nom : c'est tout l'objet des doublons (§4).
    nom: String,
    ch: character::Character,
    …
}
```

Le corps à 60 Hz devient : le partagé une fois, puis une boucle `for` sur le
`Vec<Acteur>`.

### Les trois pièges de la généralisation

Chacun serait un bug silencieux si on le laissait implicite.

> **1. Une seule graine, un seul RNG — jamais un par personnage.**
>
> La tentation est d'écrire `XorShift32::seeded(index)` pour chaque nouveau
> personnage. Ce serait tomber en plein dans le **piège des graines
> séquentielles** documenté dans `CLAUDE.md` : l'état initial d'un petit entier
> laisse `next_u32` dans les bits de poids faible, et `weighted` retombe
> systématiquement sur l'index de poids faible de la table. Tous les
> personnages tireraient donc **la même première envie**, et la mesure qui
> chercherait à le démentir tomberait elle-même dans le piège.
>
> Le RNG reste **unique, partagé, et on le laisse simplement avancer.**

> **2. Un seul curseur, donc un seul élu.**
>
> Le hit-testing désigne **au plus un** personnage par image : le premier du
> `Vec` dont la hitbox de la pose courante contient le curseur. Tous les autres
> reçoivent `curseur_sur_le_personnage = false`.
>
> Sans cette règle, deux personnages superposés seraient attrapés **ensemble**
> par un seul clic, et se suivraient jusqu'au relâchement. Un personnage déjà
> `Dragged` garde la priorité sur tout autre, sinon un glisser rapide passant
> au-dessus d'un voisin transférerait la prise.
>
> L'ordre de départage est celui du `Vec` : arbitraire, mais **stable** —
> le même personnage gagne tant que rien ne change. Deux fenêtres toujours au
> premier plan n'ont de toute façon pas de z-order que nous contrôlions.

> **3. Les labels ne sont jamais réutilisés.**
>
> `label_de(index)` existe déjà et rend `pet-{index}`. Avec des ajouts et des
> retraits, l'index dans le `Vec` **ne peut plus servir de label** : retirer
> `pet-1` puis en ajouter un réattribuerait `pet-1` pendant que Windows détruit
> encore la fenêtre précédente — et Tauri refuserait le label, ou pire, servirait
> l'ancienne fenêtre.
>
> Le label vient donc d'un **compteur monotone** qui ne redescend jamais :
> `pet-0`, `pet-1`, … `pet-37`. Il n'a aucun sens sémantique, et c'est voulu.

### La création et la destruction des fenêtres depuis le thread de la boucle

Les fenêtres naissent et meurent maintenant **en cours d'exécution**, depuis le
thread de la boucle et non plus depuis `setup`.

C'est légitime : `RuntimeHandle` de Tauri poste un message au thread principal
quand on l'appelle depuis un autre thread. À l'inverse, `tauri-runtime-wry`
**panique explicitement** si `WindowMessage::Close` ou `Destroy` est traité
*sur* le thread principal (`lib.rs:3492`) — c'est-à-dire si on ferme une fenêtre
depuis un gestionnaire d'événements. Notre boucle étant un thread à part, elle
est du bon côté.

> ⚠️ **Ce raisonnement est lu dans les sources, pas vérifié à l'exécution.** Le
> plan en fait sa **première tâche**, isolée et jetable si elle échoue :
> créer et détruire une fenêtre de personnage depuis le thread de la boucle, en
> boucle, et vérifier qu'aucune ne fuit. Tout le reste du plan en dépend.

Chaque fenêtre créée reçoit **le même traitement qu'aujourd'hui** :
`set_ignore_cursor_events(true)`, puis `render::appliquer_styles_etendus` —
`WS_EX_NOACTIVATE` et `WS_EX_TOOLWINDOW`, les deux découvertes de l'étape 0.
Les oublier sur les fenêtres n° 2 et suivantes donnerait des personnages qui
volent le focus et apparaissent dans Alt+Tab, un bug qui ne se verrait que
sur le deuxième personnage.

---

## 4. Le modèle de données : `personnages` devient un multi-ensemble

### Le type ne change pas, son sens change

`config.personnages` est **déjà** un `Vec<String>`. C'est le point
d'articulation que l'auteur avait identifié, et il tient : la liste devient un
**multi-ensemble**, où une répétition vaut un exemplaire de plus.

```json
{ "personnages": ["blob", "blob", "luffy"] }
```

Deux blob et un luffy. Le compteur affiché sur une carte, c'est **le nombre
d'occurrences de son nom** dans cette liste. Il n'y a aucune autre source de
vérité : pas de compte « en sommeil », pas d'état mémorisé pour l'interrupteur.

`config::ecrire_personnage(chemin, nom)`, qui écrit aujourd'hui un tableau à un
seul élément, devient `ecrire_personnages(chemin, &[String])`. L'édition reste
**chirurgicale** — relecture en `serde_json::Value`, remplacement de la seule
clé `personnages`, réécriture sans BOM. Sérialiser depuis `Config` perdrait les
clés inconnues et écraserait un fichier réglé à la main.

### Une seule commande pour les quatre gestes

```rust
#[tauri::command]
fn definir_compte(nom: String, combien: usize) -> Result<(), String>
```

| Geste dans « Ma bibliothèque » | Appel |
|---|---|
| clic sur la carte | `definir_compte(nom, n + 1)` |
| bouton `−` | `definir_compte(nom, n − 1)` |
| interrupteur **éteint** | `definir_compte(nom, 0)` |
| interrupteur **allumé** | `definir_compte(nom, 1)` |

**L'interrupteur n'est que le reflet de `compte > 0`.** Éteindre trois blob puis
rallumer en ramène **un**, pas trois : c'est la décision de l'auteur, et elle
supprime toute une classe de bugs — il n'y a pas deux vérités à tenir d'accord,
donc rien à désynchroniser.

`choisir` **disparaît**, remplacée par celle-ci. C'est le changement de sens
demandé en tête de ce document : « choisir » ne remplace plus, il ajoute.

`bibliotheque()` rend désormais, par pack :

```rust
struct PackInstalle {
    nom: String,
    /// Combien d'exemplaires vivent à l'écran. 0 = éteint.
    compte: usize,
    /// La poubelle est-elle active ? Voir §7.
    supprimable: bool,
}
```

Le champ `actif: bool` disparaît : `compte > 0` le dit.

### La liste vide est autorisée

Décocher le dernier personnage donne **zéro personnage à l'écran**. C'est
permis, c'est la décision de l'auteur, et c'est cohérent avec « Cacher » du
tray. L'application continue de vivre dans la zone de notification, et un clic
dans la bibliothèque ramène quelqu'un.

⚠️ **Conséquence à ne pas défaire** : `main.rs` fait aujourd'hui
`personnages.first().unwrap_or("blob")` et **quitte avec un message** si le
personnage est introuvable. Ce repli doit disparaître pour la liste vide — une
liste vide n'est plus une erreur de configuration, c'est un état normal. En
revanche, un **nom introuvable** dans une liste non vide reste une erreur : il
est signalé bruyamment et ignoré, sans empêcher les autres de vivre.

### La réconciliation : le cœur de l'étape

Toute modification de la liste passe par une fonction **pure**, donc testable
sans écran :

```rust
enum ActionRoster { Creer(String), RetirerUn(String) }

/// Ce qu'il faut faire pour que les acteurs présents correspondent à la liste
/// voulue. Fonction pure : elle ne connaît ni Tauri, ni le disque.
fn reconcilier(presents: &[String], voulus: &[String]) -> Vec<ActionRoster>
```

Elle compte les occurrences de chaque nom des deux côtés, et rend les créations
et retraits nécessaires. **Quand il faut en retirer un sur trois, c'est le plus
récemment ajouté qui part** — c'est ce qu'attend quelqu'un qui vient de cliquer
`+` puis `−`.

Le chemin complet, qui réutilise la mécanique du rechargement à chaud :

1. la commande (thread de Tauri) écrit `config.json`, **lit les manifestes
   nécessaires depuis le disque**, et dépose le tout dans la boîte partagée avec
   un numéro de version ;
2. la boucle (60 Hz) voit la version changer, appelle `reconcilier`, crée et
   retire.

**Aucune entrée-sortie à 60 Hz**, et le verrou n'est jamais tenu pendant une
lecture de fichier : exactement la discipline de `rechargement.rs` aujourd'hui.

> **Et le rechargement à chaud emprunte le MÊME chemin.** Le fichier témoin
> `characters/recharger.txt` relit la config, recharge les manifestes, et
> réconcilie. Un seul chemin de code, pas deux — donc pas de second chemin qui
> diverge à la première correction.

`Manifest` dérive déjà `Clone` : chaque acteur possède sa copie. Quelques Ko par
personnage, aucune raison d'introduire un `Arc` et l'emprunt partagé qui va
avec.

---

## 5. L'arrivée : il tombe du haut de l'écran

### Elle est bien gratuite — vérifié, pas supposé

L'intuition de l'auteur était juste, et elle tient à la décision n° 1 :
`Attachment::Falling { pos, vel }` existe déjà, et les réflexes à 60 Hz gèrent
déjà la chute puis l'atterrissage (`contact()`, étape 4a).

Un personnage qui s'active naît donc ainsi, et **rien d'autre n'est écrit** :

```rust
Attachment::Falling {
    pos: Point::new(x_tiré_au_sort, haut_de_la_zone_de_travail),
    vel: Vec2::new(0.0, 0.0),   // vitesse nulle : il se laisse tomber
}
```

- **l'écran** est tiré au sort parmi ceux qui existent ;
- **le `x`** est tiré au sort sur la largeur de sa zone de travail, en gardant
  une demi-largeur de sprite de marge de chaque côté pour qu'il n'arrive pas à
  moitié hors champ ;
- le tirage utilise le **RNG partagé** de la boucle (piège n° 1 du §3).

### ⚠️ Le seul point qui demande un test

La plateforme « plafond » de chaque écran est posée **juste au-dessus** de la
zone de travail (`world.rs` : `Rect::new(z.left(), z.top() - EPAISSEUR, …)`) et
expose sa face `Bottom`, celle à laquelle on se suspend.

Un personnage qui apparaît au sommet de la zone de travail naît donc à quelques
pixels d'une surface accrochable. **Il faut prouver qu'il tombe au lieu de s'y
agripper** — le contraire donnerait des personnages qui apparaissent collés au
plafond, ce qui n'est pas ce qui a été demandé.

C'est un test de `contact()` sans écran, et c'est le seul vrai risque de cette
partie.

---

## 6. Le départ : il se ramasse, il saute, il tombe

### ⚠️ Un point de contenu, vérifié dans le vocabulaire des poses

L'auteur demandait une animation où le personnage « plierait les jambes pour
préparer son saut ».

**Il n'existe aucune frame de ce genre dans le vocabulaire Shimeji.** Le relevé
complet des 46 poses (`docs/specs/2026-09-09-frames-shimeji.md`, tiré des
sources de Shimeji-ee) ne contient que `jump` — la frame 22, **une seule
image**, sans préparation.

Ce qu'on peut composer avec ce qui existe, et qui donne la lecture cherchée :

| Phase | Pose | Durée | Ce qu'on voit |
|---|---|---|---|
| 1 | `sit` (frame 11) | ~120 ms | il se ramasse |
| 2 | `jump` (frame 22) | ~150 ms | il se détend |
| 3 | `fall` (frame 4) | jusqu'en bas | il tombe hors de l'écran |

Puis la fenêtre est détruite et l'acteur retiré du `Vec`.

### Ce que ça coûte, et qu'il faut assumer

Contrairement à l'arrivée, le départ **n'est pas gratuit** : le personnage doit
**traverser le sol**, ce que les réflexes interdisent précisément par
construction.

La solution la moins invasive, et celle retenue : un champ `depart:
Option<Depart>` sur l'`Acteur`, testé **avant** tout le reste. Quand il est
posé, la boucle **court-circuite `behavior::pas`** et fait tomber le personnage
par une dizaine de lignes de physique écrites sur place.

> **Pourquoi court-circuiter plutôt qu'ajouter un état au comportement :** le
> réflexe d'atterrissage est non négociable par définition (spec §7.1, couche 1).
> Y introduire une exception « sauf si je suis en train de partir » le rendrait
> négociable, et c'est le premier pas vers un réflexe plein de cas particuliers.
> Un acteur en départ n'est plus un personnage vivant — il est en train de
> quitter la scène, et la scène n'a plus son mot à dire.

Deux conséquences, à écrire dans le code :

- un personnage en départ n'est **ni attrapable ni cliquable** : il est ignoré
  par le hit-testing et n'ouvre pas de menu ;
- il est retiré dès que son `y` dépasse le bas de son écran d'une hauteur de
  sprite. Un garde-fou de durée (~3 s) le retire de toute façon — un acteur
  qui ne partirait jamais fuirait une fenêtre à chaque désactivation.

### La couverture partielle s'applique toute seule

Un pack sans `jump`, ou sans `sit`, **saute simplement l'étape manquante** et
passe à la suivante. Aucun cas particulier à coder : c'est la règle §8.6, qui
retire du jeu ce qui n'est pas dessiné. Un pack qui n'aurait ni l'un ni l'autre
tombe directement, ce qui reste parfaitement lisible.

---

## 7. La suppression : irréversible, donc cadrée

### Ce qui est supprimable

> **Un pack est supprimable si et seulement si son dossier résout dans la
> bibliothèque `%APPDATA%\shimeji-desktop\characters\`.**

La règle est **générale**, et surtout pas un cas particulier nommé « blob ».
`blob` est livré dans le dépôt, donc non supprimable, et sa poubelle est
désactivée — mais pour la raison qui vaut aussi pour tout futur pack de
référence : **nous ne supprimons jamais un fichier versionné.**

Le dossier livré n'est d'ailleurs peut-être même pas inscriptible : une
installation ordinaire le pose à côté de l'exe, dans `Program Files`.

**Le cas de l'homonyme**, qui doit être écrit plutôt que découvert : un pack
présent dans les **deux** racines est supprimable, on efface la copie de la
bibliothèque — et **celle du dépôt réapparaît alors dans la liste**. C'est la
conséquence directe de la règle de résolution « bibliothèque d'abord, dépôt
ensuite » du design du catalogue. Ça peut surprendre ; ça ne peut pas détruire
de données.

### L'ordre des opérations, qui n'est pas un détail

```
1. definir_compte(nom, 0)      — retrait IMMÉDIAT, sans animation de départ
2. attendre que ses acteurs soient partis
3. std::fs::remove_dir_all(dossier)
```

**Sans l'animation de départ, et c'est délibéré.** Effacer les PNG pendant
qu'une fenêtre les réclame encore par le schéma `shime://` donnerait un
personnage à moitié dessiné en pleine chute — et un adieu animé sur un geste
irréversible serait de toute façon déplacé. La suppression est brutale parce
qu'elle est définitive.

L'étape 2 est ce qui rend l'étape 3 sûre : la commande ne supprime le dossier
qu'une fois la boucle ayant confirmé qu'aucun acteur de ce nom ne vit plus.

### La confirmation

La suppression est **définitive et sans corbeille**. La fenêtre demande
confirmation avant d'effacer, en nommant le pack. C'est le seul geste de toute
l'application qui détruise quelque chose.

En cas d'échec (fichier verrouillé, permissions), l'erreur est **bruyante** dans
la barre d'état de la fenêtre : une suppression silencieusement ratée laisserait
croire que le pack est parti alors qu'il reviendra au prochain démarrage.

---

## 8. « Ma bibliothèque » : ce que la carte devient

La carte de 223×144 porte désormais cinq choses au lieu de deux :

```
┌───────────────────────────┐
│                       🗑   │  ← poubelle, coin haut droit
│         [vignette]        │
│                           │
│   blob        ─ 2 +   ◉━  │  ← nom · compteur · interrupteur
└───────────────────────────┘
```

| Élément | Geste | Effet |
|---|---|---|
| le fond de la carte | clic | `definir_compte(nom, n + 1)` |
| `−` | clic | `definir_compte(nom, n − 1)`, jamais en dessous de 0 |
| `+` | clic | identique au fond de la carte |
| l'interrupteur | clic | `0` s'il est allumé, `1` s'il est éteint |
| la poubelle | clic | confirmation, puis suppression (§7) |

Le compteur n'est affiché que si `compte > 0` : une bibliothèque de 40 packs
dont 2 sont actifs ne doit pas être un mur de zéros.

**L'écran « Catalogue » ne change pas.** Il installe, et lui seul ; la
bibliothèque active et supprime. C'est la décision de cadrage n° 2 du design du
catalogue — deux écrans, deux verbes — et elle passe à trois verbes sans
brouiller la frontière.

### L'avertissement à 10

Quand le total des exemplaires actifs atteint **10**, un bandeau apparaît en
tête de l'écran « Ma bibliothèque » :

> ⚠️ 10 personnages à l'écran. Chacun qui marche consomme du processeur ; à ce
> nombre, la consommation peut devenir notable.

Il n'interdit rien, ne désactive aucun bouton, ne demande aucune confirmation.
Il disparaît si le total redescend sous 10. La même ligne part sur la sortie
standard — l'équivalent scriptable, selon la règle du projet.

### Les jetons de design ne bougent pas

Fond `rgb(36,36,36)`, texte `rgb(201,201,201)`, pile de polices système,
vignette 128×128 `object-fit: contain`, grille en flex wrap. Ces valeurs ont été
**mesurées** dans le DOM de shimejis.xyz, pas approchées : les nouveaux éléments
s'y conforment au lieu d'introduire une seconde palette.

---

## 9. Le menu contextuel, et le tray

### Le menu contextuel suit son personnage

`menu_perso::ouvrir` prend déjà le manifeste et le contexte (`Ou`) **en
paramètres** : il est généralisable sans y toucher. Le menu s'ouvre pour le
personnage élu par le hit-testing (§3), avec **son** manifeste et **son**
contexte d'accroche.

La commande choisie revient par la boîte aux lettres partagée, et la boucle
l'applique **au seul acteur qui a ouvert le menu**. C'est trivialement correct :
`menu_perso::ouvrir` **bloque** jusqu'à la fermeture du menu, donc la boucle sait
encore de qui il s'agit quand elle lit la réponse. Aucune indirection à inventer.

> **Et la règle non négociable tient** : il n'y a toujours **qu'un seul
> `on_menu_event`** dans tout le programme, installé par `tray.rs`. Tauri livre
> chaque événement de menu à *tous* les gestionnaires ; un second exécuterait
> chaque action deux fois.

### Le tray

« Afficher » cache et montre **tous** les personnages, comme aujourd'hui.
Le verrouillage de session les fait tous disparaître. Aucun changement de sens.

⚠️ `tray::basculer_visibilite` et le chemin « caché » de la boucle doivent
parcourir le `Vec` : oublier un acteur laisserait un personnage seul à l'écran
après un « Cacher », ou pire, après un verrouillage de session — ce qui est un
défaut de confidentialité, pas un défaut cosmétique.

---

## 10. Vérification

### Ce qui se teste sans écran, en `cargo test`

| Ce qu'on prouve | Comment |
|---|---|
| `reconcilier` rend les bonnes créations et retraits | fonction pure, tables de cas |
| retirer un sur trois retire **le plus récent** | fonction pure |
| `config.json` : aller-retour du multi-ensemble, clés inconnues préservées, sans BOM | comme `config_tests.rs` aujourd'hui |
| la liste vide est acceptée au chargement et ne fait pas quitter | `charger` + démarrage |
| un nom introuvable dans une liste non vide est ignoré, les autres vivent | idem |
| **le point d'apparition ne s'agrippe pas au plafond** | `contact()`, le vrai risque du §5 |
| le `x` d'apparition reste dans la zone de travail, marges comprises | fonction pure + RNG semé une fois |
| le départ sort de l'écran en temps borné, et le garde-fou l'y force | physique du départ, horloge injectée |
| la règle de suppressibilité (bibliothèque oui, dépôt non) | résolution de chemins |
| un seul personnage est élu sous le curseur, même superposés | élection, fonction pure |

### Les équivalents scriptables — la règle du projet

Tout ce qui demanderait un clic en reçoit un :

| Variable | Ce qu'elle remplace |
|---|---|
| `SHIMEJI_PERSONNAGES=blob,blob,luffy` | remplace `SHIMEJI_CHANGER` : installe directement un roster de départ |
| `SHIMEJI_ROSTER=<t>:<liste>` | joue un changement de roster après *t* secondes — le clic dans la bibliothèque, y compris une désactivation, donc le **départ observable** sans humain |
| `SHIMEJI_CADENCE=1` | inchangée, mais rend désormais les placements **tous acteurs confondus** — c'est le chiffre de la loi du §2 |

### Ce qui ne se vérifie qu'à l'œil

**Le départ.** Que `sit → jump → fall` se lise comme « il se ramasse et il
saute » est un jugement esthétique : aucune assertion ne le rend. Les durées
(120 ms / 150 ms) sont des points de départ à régler, et le rechargement à chaud
du manifeste est fait pour ça.

C'est la seconde vérification du projet à demander un humain, après l'ancre de
`grabWall` restée ouverte depuis l'étape 4a.

### La mesure, obligatoire avant de solder

1. **N=10, 60 s, build release** — le chiffre annoncé par la loi, et surtout le
   comportement de la file de déplacements décrit dans la réserve du §2. Le
   résultat est consigné **ici**, quel qu'il soit.
2. **`SHIMEJI_CACHE=1` à N=4 et N=10** — la seule mesure comparable d'une
   version à l'autre. Elle doit rester proche de `0,9 % + ε × N` ; une dérive
   signifierait que la boucle unique fait du travail par personnage qu'elle ne
   devrait pas faire.

---

## 11. Les fichiers touchés

| Fichier | Ce qui change |
|---|---|
| `src-tauri/src/main.rs` | `Acteur`, le `Vec`, le tri partagé/par personnage, la création et la destruction de fenêtres, l'élection sous le curseur, l'apparition et le départ |
| `src-tauri/src/roster.rs` | **nouveau** — `reconcilier`, pure et testée |
| `src-tauri/src/config.rs` | `ecrire_personnages`, le multi-ensemble, la liste vide |
| `src-tauri/src/commandes.rs` | `definir_compte` et `supprimer` remplacent `choisir` ; `PackInstalle` gagne `compte` et `supprimable` |
| `src-tauri/src/actions.rs` | `perso: Mutex<(String, PathBuf)>` devient le roster partagé |
| `src-tauri/src/rechargement.rs` | la demande porte un roster, plus un personnage |
| `src-tauri/src/tray.rs` | visibilité et verrouillage sur tous les acteurs |
| `ui/catalogue.js`, `ui/catalogue.html` | la carte de bibliothèque, le bandeau des 10 |
| `docs/outils/mesurer-n.ps1`, `mesurer-normalise.ps1` | **nouveaux** — les scripts de mesure du §2 |

`menu_perso.rs`, `behavior/`, `character/`, `world.rs`, `geom.rs` et `render.rs`
**ne changent pas** : ils prennent déjà leurs paramètres un par un, ce qui est
précisément ce qui rend cette étape possible.
