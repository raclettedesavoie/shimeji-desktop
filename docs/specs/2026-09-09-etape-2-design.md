# Étape 2 — « Il réagit » : design

**Écrit le 2026-09-09**, après l'étape 1 (soldée, tag `etape-1`).

Ce document complète `docs/specs/2026-09-08-design.md` §7. Il ne le remplace pas :
la table d'envies, les trois couches et la décision n° 3 y sont déjà. Ce qu'on ajoute
ici, c'est **comment les signaux entrent dans le programme**, et les quatre décisions
que l'écriture de §7 avait laissées ouvertes.

---

## 1. Périmètre

| Livré | Reporté, et où |
|---|---|
| Les cinq signaux : inactivité, appli au premier plan, heure, batterie, verrouillage | — |
| `Dormir` : il s'assoit, puis il s'affale | — |
| `Se réveiller` quand l'utilisateur revient | — |
| `Jouer(Jeu)` avec deux animations, et les modificateurs par application | — |
| Le verrouillage de session : il se planque, et la boucle se suspend | — |
| ❌ `Manger` | **étape ultérieure** — voir §2 |
| ❌ `AllerÀ(fenêtre active)` | **étape 5** — demande une cible qui est une fenêtre |
| ❌ Plateformes de fenêtres, occlusion, filtrage | **étape 4** — le monde n'expose toujours que le sol |

Rappel : le monde de l'étape 2 est **toujours celui de l'étape 1** — un sol par écran.
Le seul signal qui touche aux fenêtres est « appli au premier plan », et il ne demande
que `GetForegroundWindow`, pas un recensement.

---

## 2. `Manger` est reporté, et ce n'est pas un oubli

**Shimeji-ee n'a aucune animation de repas.** Aucune action `Eat`, `Food` ni `Drink`
dans `conf/actions.xml` — le relevé complet des 80 actions est dans
`docs/specs/2026-09-09-frames-shimeji.md`. C'est le même constat que pour le sommeil,
en plus dur : pour dormir, la frame 21 (`Sprawl`) était un substitut acceptable ;
pour manger, **il n'y a rien qui ressemble à manger.**

Décision de l'auteur : **reporter**, et retirer la promesse des documents plutôt que
de livrer une machinerie dont l'effet serait invisible. Le signal horaire reste, mais
il ne sert plus qu'au « tard le soir ».

> **Ce qui rend le report peu coûteux :** `Manger` sera **une ligne** dans la table
> d'envies, exigeant une pose `eat`. Le jour où un pack la fournit — ou le jour où on
> la dessine — rien d'autre ne bouge. C'est la décision n° 5 qui rend ça vrai, et il
> n'y a donc aucune raison de forcer maintenant.

---

## 3. La sonde rend un instantané, pas cinq accesseurs

Une seule méthode s'ajoute à `SystemProbe` :

```rust
/// Tout ce qui change lentement, lu d'un coup à ~2 Hz (design §5.5).
fn signaux(&self) -> Signaux;
```

```rust
/// L'état du système à un instant, tel que le comportement a besoin de le connaître.
#[derive(Debug, Clone, PartialEq)]
pub struct Signaux {
    /// Depuis combien de temps l'utilisateur n'a touché à rien.
    ///
    /// ⚠️ `GetLastInputInfo` rend un COMPTEUR, jamais une touche. C'est la
    /// seule voie compatible avec « aucune capture de frappe », qui est une
    /// exclusion explicite du besoin.
    pub inactivite: Duration,

    /// Le nom de fichier de l'exécutable au premier plan — `"Code.exe"`.
    ///
    /// Le nom seul, jamais le chemin : c'est ce que l'utilisateur écrira dans
    /// `config.json`, et un chemin complet y serait impossible à deviner.
    /// `None` si la fenêtre au premier plan n'appartient à aucun processus
    /// interrogeable (écran de connexion, élévation UAC).
    pub appli_active: Option<String>,

    /// L'heure locale, 0 à 23. Rien de plus fin : aucun signal du projet ne
    /// dépend de la minute.
    pub heure: u8,

    pub batterie: Batterie,

    /// Vrai pendant que la session est verrouillée (Win+L, écran de veille
    /// avec mot de passe, changement d'utilisateur).
    pub session_verrouillee: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Batterie {
    /// `None` sur une machine sans batterie — un fixe, ou un état inconnu.
    /// **Ce n'est pas 100 %** : confondre les deux ferait fatiguer un pet sur
    /// une tour de bureau.
    pub pourcent: Option<u8>,
    pub sur_secteur: bool,
}
```

### Pourquoi un instantané plutôt que cinq méthodes

Deux raisons, la première étant la seule qui compte :

1. **La cohérence entre les cinq valeurs.** Lues à cinq instants différents, on peut
   observer « session verrouillée » et « actif il y a 10 ms » dans la même image du
   comportement. Un instantané rend cet état impossible par construction.
2. Côté tests, c'est **une** structure à fabriquer, et `FakeProbe` gagne un seul
   champ modifiable.

### Les APIs, vérifiées dans `windows` 0.61.3

Relevé le 2026-09-09 dans les sources de la crate — pas supposé :

| Signal | Appel | Module de la crate |
|---|---|---|
| inactivité | `GetLastInputInfo` + `LASTINPUTINFO` | `Win32/UI/Input/KeyboardAndMouse` |
| appli active | `GetForegroundWindow` → `GetWindowThreadProcessId` → `OpenProcess` → `QueryFullProcessImageNameW` | `Win32/UI/WindowsAndMessaging`, `Win32/System/Threading` |
| heure | `GetLocalTime` → `SYSTEMTIME.wHour` | `Win32/System/SystemInformation` |
| batterie | `GetSystemPowerStatus` → `SYSTEM_POWER_STATUS` | `Win32/System/Power` |
| verrouillage | `WTSQuerySessionInformationW(WTSSessionInfoEx)` → `WTSINFOEX_LEVEL1_W.SessionFlags` | `Win32/System/RemoteDesktop` |

Trois détails relevés en même temps, chacun étant une occasion de perdre une heure :

- **`WTS_CURRENT_SESSION` n'existe pas dans la crate.** C'est `0xFFFF_FFFF` ; on le
  définit nous-mêmes, avec le commentaire qui dit d'où il vient.
- `WTSQuerySessionInformationW` alloue : il **faut** `WTSFreeMemory` après lecture.
- `SYSTEM_POWER_STATUS.BatteryFlag` vaut `128` quand il n'y a **pas** de batterie, et
  `BatteryLifePercent` vaut alors `255`. D'où le `Option<u8>` ci-dessus : `255` n'est
  pas un pourcentage.

Nouvelles *features* de la crate à activer : `Win32_System_Power`,
`Win32_System_SystemInformation`, `Win32_System_RemoteDesktop`, `Win32_System_Threading`.

---

## 4. `signals.rs` traduit les signaux en biais

C'est le cœur de l'étape, et c'est une **fonction pure** :

```rust
/// Un multiplicateur par intention. `Copy`, minuscule, calculé à 2 Hz.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Biais { /* un f32 par intention */ }

impl Biais {
    pub fn pour(&self, i: Intention) -> f32;
    /// Le biais neutre : tout à 1. C'est celui de l'étape 1.
    pub fn neutre() -> Biais;
}

pub fn biais_de(s: &Signaux, c: &Config) -> Biais;
```

### Trois raisons à cette forme, par ordre d'importance

1. **`Entrees` reste `Copy`.** L'étape 1a a conçu `Entrees` pour grossir précisément
   ici. Y mettre `Signaux` avec son `String` casserait le `Copy` de la structure, et
   toutes les signatures des trois couches s'en ressentiraient. Un `Biais` est un
   paquet de `f32`.
2. **La comparaison de chaînes sort du chemin 60 Hz.** `"Code.exe" == appli_active`
   se fait **2 fois par seconde**, pas 60. Sur un projet qui a mesuré son CPU trois
   fois, ce n'est pas un détail de style.
3. **Le point d'entrée existe déjà.** `desire::tirer_avec(manifest, rng, mult)` a été
   écrit à l'étape 1a *pour ce moment*, avec son test. Le branchement est **une ligne**
   dans `behavior/mod.rs` :

   ```rust
   //  étape 1 :  table.tirer(&ch.manifest, rng)
   //  étape 2 :
   table.tirer_avec(&ch.manifest, rng, |i| e.biais.pour(i))
   ```

### La table des modificateurs

Valeurs de départ de la spec §7.2, **toutes dans `config.json`** (décision n° 5) :

| Condition | Effet |
|---|---|
| inactif > 2 min | flâner ×0,2 · se reposer **×8** |
| heure ≥ 22 ou < 6 | se reposer ×3 |
| batterie < 20 % **et** pas sur secteur | se reposer ×2 |
| appli active, par application | `Code.exe` → jouer ×3, flâner ×0,5 |

Les modificateurs se **multiplient** entre eux : inactif le soir sur batterie faible
donne 8 × 3 × 2 = ×48 sur le repos. C'est voulu — il s'endort quasi certainement, et
« quasi » reste le produit.

```json
"envies": {
  "flaner": 5.0,
  "seReposer": 1.0,
  "jouer": 1.0
},
"signaux": {
  "inactiviteSecondes": 120,
  "inactifFlaner": 0.2,
  "inactifSeReposer": 8.0,
  "soirDebut": 22,
  "soirFin": 6,
  "soirSeReposer": 3.0,
  "batterieSeuil": 20,
  "batterieSeReposer": 2.0,
  "seuilSommeil": 2.0
},
"applications": {
  "Code.exe":   { "jouer": 3.0, "flaner": 0.5 },
  "chrome.exe": { "seReposer": 2.0, "flaner": 1.5 }
}
```

`applications` est une table associative, donc **ajouter une application ne demande
aucun code** — c'est la forme la plus littérale de « ajouter un signal = ajouter une
ligne ».

---

## 5. Dormir : une intention à deux phases

`EtatIntention::Repos` gagne une phase :

```rust
Repos { phase: PhaseRepos, jusqu_a: Duration }

enum PhaseRepos {
    Assis,    // pose `sit`   — frame 11
    Endormi,  // pose `sleep` — frame 21
}
```

Il s'assoit d'abord. Il ne s'affale que si **`biais.pour(SeReposer) >= seuilSommeil`**
— seuil réglable, **2,0 par défaut**, donc atteint dès qu'un signal double l'envie de
repos. Sans signal, le biais vaut 1, il reste assis, et l'intention se termine comme à
l'étape 1 : **une sieste ne s'improvise pas.**

Un seuil chiffré et non « si le biais est élevé » : c'est exactement le genre de
formule qui se lit bien dans une spec et s'implémente de trois façons différentes.

**Durée du sommeil : 20 à 60 secondes.** Ce n'est pas réglé à l'œil, c'est `LieDown`
dans `actions.xml` — `Sprawl` pendant `${500+Math.random()*1000}` ticks, à 40 ms le
tick. La leçon de l'étape 1a s'applique : chercher la constante dans le source avant
de l'inventer.

### Le délai d'abandon reste uniforme, et voici comment

La décision n° 4 impose 20 s à **toute** intention, sans exception. Un sommeil de 60 s
est donc coupé au bout de 20 s, l'intention échoue, et la couche 3 re-tire — avec un
×8 sur le repos, elle re-tire `SeReposer` presque à coup sûr.

Le risque est **visuel** : re-tirer relancerait la phase `Assis`, donc on le verrait
se rasseoir puis se raffaler toutes les 20 secondes. La correction tient en une règle,
dans `se_reposer` :

> **Si le personnage est déjà dans la pose `sleep`, l'intention démarre en phase
> `Endormi`.** Le re-tirage devient invisible à l'œil.

C'est la **continuité de pose** qui absorbe la règle des 20 s, et non une exception à
la règle. Aucune des deux décisions n'est entamée.

---

## 6. Se réveiller : les signaux peuvent interrompre, jamais choisir

**C'est un ajout à la décision n° 3, et il mérite d'être écrit.**

Le problème : si le réveil passait par le tirage, il dormirait jusqu'à **20 s** après
le retour de l'utilisateur — le temps que l'intention expire. Trop lent pour « il se
réveille au retour », qui est une promesse de l'étape.

La règle retenue :

> Redevenir actif **termine** le repos en cours (`Issue::Finie`), **et seulement
> depuis la phase `Endormi`.** La couche 3 re-tire immédiatement, avec des poids
> redevenus normaux.

> ### ⚠️ Pourquoi « seulement depuis `Endormi` », et pas depuis `Assis`
>
> Parce qu'une pause normale se prend **pendant que l'utilisateur travaille.** La
> règle « actif → termine le repos » appliquée à la phase `Assis` empêcherait le
> personnage de s'asseoir tant qu'on touche au clavier : il ne se reposerait plus
> jamais aux heures où l'on regarde l'écran, ce qui est précisément le moment où on
> le voit.
>
> Le sommeil, lui, n'est **atteint** que si un signal a poussé le biais au-dessus du
> seuil (§5) — c'est-à-dire, en pratique, parce que l'utilisateur était parti. En
> réveiller le personnage à son retour est donc cohérent, et sans effet de bord sur
> les siestes ordinaires.

La frontière que ça trace, et qu'il faut garder :

| | Autorisé | Interdit |
|---|---|---|
| Un signal peut… | **arrêter** ce qu'il est en train de faire | **choisir** ce qu'il fera ensuite |

Il se réveille en ~0,5 s (le battement du 2 Hz), puis part flâner *ou* se rasseoir
*ou* jouer, selon le tirage. Un déclenchement direct aurait donné « il se réveille et
il marche », **toujours** — exactement l'afficheur d'état système déguisé en
personnage que la décision n° 3 interdit.

Emplacement : `behavior/mod.rs`, juste avant `poursuivre`, en un bloc commenté qui
cite cette section. Pas dans `reflex.rs` : un réflexe est une **conséquence physique**
(la plateforme a disparu, on m'attrape), pas une réaction à un signal.

---

## 7. `Jouer(Jeu)` — deux lignes de table, pas une

```rust
Intention::Jouer(Jeu::TeteQuiTourne)       // pose `spinHead`
Intention::Jouer(Jeu::JambesQuiBalancent)  // pose `sitDangle`
```

| Pose | Frames | Durée | Source |
|---|---|---|---|
| `spinHead` **(nouvelle)** | 26, 15, 27, 16, 28, 17, 29, 11 | 200 ms | `SitAndSpinHeadAction` |
| `sitDangle` (déjà déclarée) | 31, 32, 31, 33 | 400 ms, en boucle | `SitAndDangleLegs` |

**Deux lignes distinctes et non une intention qui choisit**, parce que
`poses_requises` est une liste **ET** : une seule ligne exigerait les deux animations,
et un pack n'en ayant qu'une ne jouerait jamais. Séparées, la couverture partielle
(§8.6) joue **par animation**. C'est aussi la lecture littérale de la notation
`Jouer(action)` de la spec §7.1.

`Intention` devient donc `enum Intention { Flaner, SeReposer, Jouer(Jeu) }` et reste
`Copy + Eq` — donc toujours utilisable comme clé de la table d'envies.

Sept frames jusqu'ici inutilisées entrent en service : 15, 16, 17, 27, 28, 29.

> ⚠️ **À juger à l'œil :** `sitDangle` porte l'ancre `64,112`, donc les jambes pendent
> **16 px sous la ligne du sol**, dans la barre des tâches. C'est ce que fait
> Shimeji-ee (`BorderType="Floor"` sur cette action), donc on le reproduit — mais
> c'est typiquement le genre de détail qui se juge à l'écran, pas en test.

---

## 8. Le verrouillage : un réflexe, et l'optimisation CPU n° 1

Session verrouillée → **il se planque** : c'est le quatrième réflexe promis par la
décision n° 5, et le seul qui manquait.

Concrètement, c'est le chemin déjà écrit pour « caché » : la boucle saute le placement
et le rendu, exactement comme `SHIMEJI_CACHE=1`. Donc :

- c'est aussi la **première des pistes CPU restantes** de `CLAUDE.md` ;
- attendu **~0,9 %** pendant le verrouillage, contre 12 % en marche ;
- et c'est mesurable sans clic, `SHIMEJI_CACHE` ayant déjà prouvé le chiffre.

Réflexe et non intention : c'est non négociable et immédiat. On ne biaise pas un poids
pour disparaître d'un écran de verrouillage.

---

## 9. La troisième horloge

Le battement à ~2 Hz vit **dans le thread de la boucle**, comme le 8 Hz : un simple
compteur d'images. Aucune synchronisation à inventer — contrairement au rechargement à
chaud de l'étape 1b, rien ici ne traverse un thread.

```
60 Hz  physique, dessin, hit-testing            (étape 1)
 8 Hz  recensement des fenêtres                 (étape 4 — encore vide)
 2 Hz  signaux → Biais, et l'interruption       ← NOUVEAU
```

Coût attendu : cinq appels système toutes les 500 ms. À comparer aux 60 `SetWindowPos`
par seconde qui coûtent 11 points de CPU — c'est du bruit. **Mesuré quand même**, per
`CLAUDE.md`.

---

## 10. Vérification

Le principe du projet ne change pas : **tout ce qui demanderait un humain reçoit un
équivalent scriptable.**

| Quoi | Comment |
|---|---|
| `signals.rs` | fonction pure : `Signaux` + `Config` → `Biais`. Se teste seule, sans horloge ni écran. |
| Les biais mordent | le test `un_multiplicateur_biaise_sans_commander` **existe déjà** (étape 1a) et devient le test de bout en bout |
| Le réveil | `FakeProbe` passe de « inactif 5 min » à « actif », et l'on vérifie que le repos se **termine** — et qu'il ne choisit pas la suite |
| Une journée entière | `--sim 1440` joue 24 h avec une chronologie d'activité scriptée ; le résumé rend **le temps endormi par heure** et **le nombre de réveils** |
| Les signaux réels | `SHIMEJI_SIGNAUX=1` imprime l'instantané à chaque battement de 2 Hz — la sixième variable de diagnostic |
| Le verrouillage | mesure CPU pendant une session verrouillée, protocole de 60 s |

### Ce qui reste à l'œil humain, et qu'aucun test ne remplace

1. **« Est-ce qu'il a l'air de dormir ? »** — la séquence 11 → 21 ne se juge pas en
   test. C'est le pari du substitut `sprawl`, et il se valide à l'écran.
2. **`sitDangle` sur la barre des tâches** — voir §7.

---

## 11. Ce que l'étape 2 ne doit pas devenir

Trois dérives, chacune ayant déjà été écartée ailleurs dans le projet, et que la
proximité des signaux rend tentantes :

- ❌ **Un afficheur d'état système.** Si l'on se surprend à écrire « si inactif alors
  dormir », la décision n° 3 est perdue. Le seul verbe autorisé est *multiplier*.
- ❌ **Un Tamagotchi.** La batterie fatigue le personnage ; elle ne lui donne pas une
  jauge, ni un besoin à satisfaire.
- ❌ **Une capture de frappe.** `GetLastInputInfo` et rien d'autre. Pas de hook clavier,
  pas de `GetAsyncKeyState` sur des touches, pas de titre de fenêtre — seulement le
  nom de l'exécutable au premier plan.
