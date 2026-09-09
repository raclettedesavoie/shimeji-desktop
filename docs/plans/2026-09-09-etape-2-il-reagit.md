# Étape 2 — « Il réagit » : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Le personnage réagit à la machine — il s'endort quand on s'en va, se réveille quand on revient, traîne le soir et sur batterie faible, joue selon l'application au premier plan, et se planque quand la session se verrouille.

**Architecture:** Les cinq signaux sont lus **d'un coup**, à ~2 Hz, par une nouvelle méthode de `SystemProbe`. Un module pur, `signals.rs`, les traduit en **`Biais`** — un paquet de `f32`, un par intention. Le comportement ne voit jamais un signal : il reçoit un `Biais` dans ses `Entrees` et le passe à `desire::tirer_avec`, écrite à l'étape 1a *pour ce moment*. Les signaux **multiplient des poids**, ils ne déclenchent rien (décision n° 3) ; la seule chose qu'ils s'autorisent en plus est d'**interrompre** un sommeil, jamais de choisir la suite.

**Tech Stack:** Rust, crate `windows` 0.61 (quatre *features* de plus), `serde`. Aucune dépendance nouvelle hors des *features*.

**Spec:** `docs/specs/2026-09-09-etape-2-design.md` — lire d'abord, le plan argumente depuis elle. Le design d'ensemble est dans `docs/specs/2026-09-08-design.md` §7 (comportement) et §5.5 (les trois horloges).

---

## Global Constraints

Reprises telles quelles de `CLAUDE.md` et de la spec. **Elles s'appliquent à chaque tâche**, on ne les répète pas ensuite.

- **Compiler depuis PowerShell, pas depuis Git Bash** — sinon rustc pêche le `link.exe` de Git for Windows et rend `extra operand`, une erreur opaque.
- **`cargo test` ne reconstruit pas l'exe.** Après une correction, `cargo build` avant de relancer l'application.
- **Commentaires abondants, en français**, expliquant le *pourquoi*. Citer la section de la spec qu'un bloc applique. Expliquer toute construction Rust non élémentaire. C'est une exigence explicite de l'auteur, pas un style.
- **`windows = "0.61"`**, épinglée : deux versions majeures donnent deux `HWND` distincts. Les *features* s'activent une par une.
- **Horloge, aléatoire et sonde système sont INJECTÉS** (spec §10.2). Aucun `Instant::now()`, aucun `rand::random()`, aucun appel Win32 direct dans le comportement.
- **Décision n° 3 — les signaux biaisent, ils ne commandent pas.** Le seul verbe autorisé est *multiplier*. Si l'on écrit « si inactif alors dormir », l'étape est ratée.
- **Décision n° 4 — le délai d'abandon de 20 s reste uniforme**, `DELAI_ABANDON`, sans exception, y compris pour le sommeil.
- **Décision n° 5 — le comportement est de la donnée.** Tout nouveau poids, seuil ou durée va dans `config.json`, avec un défaut.
- **Aucune capture de frappe.** `GetLastInputInfo` et rien d'autre : pas de hook clavier, pas de `GetAsyncKeyState` sur des touches, pas de titre de fenêtre. Seul le **nom de l'exécutable** au premier plan est lu.
- **Mesurer le CPU après toute modification du chemin 60 Hz**, protocole de **40 à 60 secondes** (voir `CLAUDE.md`, « Mesurer le CPU »). Une mesure de 10 s ne veut rien dire.
- **Périmètre :** pas de plateformes de fenêtres, pas d'occlusion, pas de filtrage de fenêtres (étape 4). Pas d'`AllerÀ(fenêtre active)` (étape 5). Pas de `Manger` (reporté, spec §2).

---

## Structure de fichiers

| Fichier | Responsabilité | Tâche |
|---|---|---|
| `src/signals.rs` **(créé)** | `Signaux` + `Config` → `Biais`. **Fonction pure**, aucun appel système, aucun état. | 1 |
| `src/probe/mod.rs` | Les types `Signaux` / `Batterie`, puis `fn signaux()` sur le trait. | 1, 2 |
| `src/probe/fake.rs` | Des signaux que le test décide. | 2 |
| `src/probe/win32.rs` | Les cinq appels Windows. **Le seul fichier `unsafe` de l'étape.** | 2 |
| `src/config.rs` | `SignauxReglages`, `applications`, `Envies.jouer`. | 1, 3 |
| `src/behavior/mod.rs` | `Entrees.biais` / `.utilisateur_actif`, le branchement de `tirer_avec`, l'interruption. | 1, 5 |
| `src/behavior/desire.rs` | Deux lignes de table pour `Jouer`. | 3 |
| `src/behavior/intention.rs` | `Jouer(Jeu)`, la phase de sommeil. | 3, 4 |
| `src/character/manifest.rs` | Les constantes de pose nouvelles. | 3, 4 |
| `src/main.rs` | Le battement 2 Hz, `--signaux`, `SHIMEJI_SIGNAUX`, le verrouillage. | 2, 6 |
| `src/sim.rs` | La journée scriptée, et les chiffres de sommeil. | 7 |
| `characters/blob/mascot.json` | La pose `spinHead`. | 3 |
| `Cargo.toml` | Quatre *features* `windows`. | 2 |
| `config.exemple.json`, `CLAUDE.md` | Documentation des nouvelles clés et de la sixième variable. | 6, 7 |

**Ordre des tâches, et pourquoi il n'est pas négociable :**

| | Tâche | Pourquoi elle vient là |
|---|---|---|
| 1 | Le biais, sans Windows | c'est le cœur, et il se teste sans écran ni `unsafe` |
| 2 | La sonde réelle | inutile avant que quelqu'un consomme des signaux |
| 3 | `Jouer(Jeu)` | ajoute une variante à `Intention`, que la tâche 4 va lire |
| 4 | Dormir | a besoin du `Biais` (seuil) de la tâche 1 |
| 5 | Se réveiller | n'a de sens qu'une fois qu'il dort |
| 6 | Brancher la boucle | tout le reste doit exister et être testé |
| 7 | La journée simulée | c'est la preuve d'ensemble, donc en dernier |

> **Le tableau des poses**, à garder sous les yeux (source : `docs/specs/2026-09-09-frames-shimeji.md`) :
>
> | Pose | Frames | Durée | Ancre | Action Shimeji-ee |
> |---|---|---|---|---|
> | `sit` | 11 | fixe | 64,128 | `Sit` |
> | `sleep` | 21 | fixe | 64,128 | `Sprawl` / `LieDown` |
> | `spinHead` **(à déclarer)** | 26,15,27,16,28,17,29,11 | 200 ms | 64,128 | `SitAndSpinHeadAction` |
> | `sitDangle` | 31,32,31,33 | 400 ms, boucle | **64,112** | `SitAndDangleLegs` |

---

## Tâche 1 : Le biais — des signaux aux poids, sans une ligne de Windows

**Files:**
- Create: `src/signals.rs`
- Modify: `src/probe/mod.rs` (les types `Signaux` et `Batterie` — **pas encore** la méthode du trait)
- Modify: `src/config.rs` (`SignauxReglages`, `ModifsAppli`, deux champs de `Config`)
- Modify: `src/behavior/mod.rs` (`Entrees` gagne deux champs ; `pas` branche `tirer_avec`)
- Modify: `src/main.rs` (`mod signals;`, et `Biais::neutre()` dans les `Entrees`)
- Modify: `src/sim.rs` (`Biais::neutre()` dans les `Entrees`)
- Modify: `config.exemple.json`

**Interfaces:**
- Consomme : `behavior::intention::Intention`, `config::Config`, `desire::TableEnvies::tirer_avec`
- Produit :
  - `probe::Signaux { inactivite: Duration, appli_active: Option<String>, heure: u8, batterie: Batterie, session_verrouillee: bool }`
  - `probe::Batterie { pourcent: Option<u8>, sur_secteur: bool }`
  - `signals::Biais` avec `fn neutre() -> Biais` et `fn pour(&self, i: Intention) -> f32`
  - `signals::biais_de(s: &Signaux, c: &Config) -> Biais`
  - `config::SignauxReglages`, `config::ModifsAppli`, `Config::signaux`, `Config::applications`
  - `behavior::Entrees::biais: signals::Biais`, `behavior::Entrees::utilisateur_actif: bool`

> **Cette tâche ne change AUCUN comportement observable.** Elle installe la couture et la laisse neutre : `Biais::neutre()` partout, donc les 140 tests existants doivent passer **sans être modifiés**. C'est la propriété qui rend la tâche vérifiable — si un test d'étape 1 casse, c'est que la couture n'est pas neutre.

- [ ] **Step 1 : Écrire les tests de `signals.rs`, qui ne compile pas encore**

Créer `src/signals.rs` avec **seulement** le module de tests ci-dessous, pour partir d'un échec franc.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::intention::Intention;
    use crate::config::{Config, ModifsAppli};
    use crate::probe::{Batterie, Signaux};
    use std::time::Duration;

    /// Le point de départ : personne n'est parti, il est 15 h, la machine est
    /// sur secteur, la session est ouverte. **Aucun signal ne mord.**
    fn rien_de_special() -> Signaux {
        Signaux {
            inactivite: Duration::ZERO,
            appli_active: None,
            heure: 15,
            batterie: Batterie {
                pourcent: None,
                sur_secteur: true,
            },
            session_verrouillee: false,
        }
    }

    #[test]
    fn sans_signal_le_biais_est_neutre() {
        let b = biais_de(&rien_de_special(), &Config::default());
        assert_eq!(b, Biais::neutre());
        assert_eq!(b.pour(Intention::Flaner), 1.0);
        assert_eq!(b.pour(Intention::SeReposer), 1.0);
    }

    #[test]
    fn inactif_multiplie_le_sommeil_et_divise_la_flanerie() {
        // Les valeurs de la spec §7.2 : flâner ×0,2 et se reposer ×8.
        let mut s = rien_de_special();
        s.inactivite = Duration::from_secs(180); // > 2 min

        let b = biais_de(&s, &Config::default());
        assert_eq!(b.pour(Intention::SeReposer), 8.0);
        assert_eq!(b.pour(Intention::Flaner), 0.2);
    }

    #[test]
    fn le_seuil_d_inactivite_est_franc() {
        // Juste avant les 120 s : rien. Juste après : tout.
        let mut s = rien_de_special();
        s.inactivite = Duration::from_secs(119);
        assert_eq!(biais_de(&s, &Config::default()), Biais::neutre());

        s.inactivite = Duration::from_secs(121);
        assert_ne!(biais_de(&s, &Config::default()), Biais::neutre());
    }

    #[test]
    fn le_creneau_du_soir_passe_par_minuit() {
        // **LE test qui attrape le bug évident.** Le créneau par défaut est
        // 22 h → 6 h : écrit naïvement `heure >= 22 && heure < 6`, il est
        // TOUJOURS faux. Il faut le comparer en OU quand il passe minuit.
        let c = Config::default();
        let soir = |h: u8| {
            let mut s = rien_de_special();
            s.heure = h;
            biais_de(&s, &c).pour(Intention::SeReposer)
        };

        for h in [22, 23, 0, 3, 5] {
            assert_eq!(soir(h), 3.0, "il est {h} h : c'est le soir ou la nuit");
        }
        for h in [6, 12, 18, 21] {
            assert_eq!(soir(h), 1.0, "il est {h} h : ce n'est pas le soir");
        }
    }

    #[test]
    fn pas_de_batterie_n_est_pas_une_batterie_vide() {
        // **Le piège de `GetSystemPowerStatus`** : sur une machine sans
        // batterie, `BatteryLifePercent` vaut 255. Confondre « pas de
        // batterie » et « batterie à plat » ferait traîner un pet sur une
        // tour de bureau, en permanence, sans raison visible.
        let mut s = rien_de_special();
        s.batterie = Batterie {
            pourcent: None,
            sur_secteur: false,
        };
        assert_eq!(biais_de(&s, &Config::default()), Biais::neutre());
    }

    #[test]
    fn la_batterie_faible_ne_fatigue_que_sur_batterie() {
        let c = Config::default();
        let mut s = rien_de_special();

        // 10 % mais branché : il ne se décharge pas, donc rien.
        s.batterie = Batterie {
            pourcent: Some(10),
            sur_secteur: true,
        };
        assert_eq!(biais_de(&s, &c), Biais::neutre());

        // 10 % et débranché : ×2 sur le repos.
        s.batterie = Batterie {
            pourcent: Some(10),
            sur_secteur: false,
        };
        assert_eq!(biais_de(&s, &c).pour(Intention::SeReposer), 2.0);
    }

    #[test]
    fn les_modificateurs_se_multiplient() {
        // Inactif, le soir, sur batterie faible : 8 × 3 × 2 = 48.
        //
        // C'est VOULU (spec §4) : il s'endort quasi certainement — et
        // « quasi » reste le produit. Additionner donnerait 13, ce qui
        // écraserait la différence entre « un signal » et « tous ».
        let mut s = rien_de_special();
        s.inactivite = Duration::from_secs(300);
        s.heure = 23;
        s.batterie = Batterie {
            pourcent: Some(5),
            sur_secteur: false,
        };

        let b = biais_de(&s, &Config::default());
        assert_eq!(b.pour(Intention::SeReposer), 48.0);
        assert_eq!(b.pour(Intention::Flaner), 0.2);
    }

    #[test]
    fn l_appli_active_applique_ses_modificateurs() {
        let mut c = Config::default();
        c.applications.insert(
            "chrome.exe".to_string(),
            // `..Default::default()` et non tous les champs : la Tâche 3
            // ajoutera `jouer` à cette structure, et un littéral exhaustif
            // ferait alors échouer la compilation de CE test — pour rien.
            ModifsAppli {
                flaner: Some(1.5),
                se_reposer: Some(2.0),
                ..Default::default()
            },
        );

        let mut s = rien_de_special();
        s.appli_active = Some("chrome.exe".to_string());

        let b = biais_de(&s, &c);
        assert_eq!(b.pour(Intention::Flaner), 1.5);
        assert_eq!(b.pour(Intention::SeReposer), 2.0);
    }

    #[test]
    fn une_appli_sans_ligne_ne_change_rien() {
        // La table est vide par défaut : n'importe quelle application doit
        // donner un biais neutre, sinon le défaut du fichier de config
        // changerait le caractère du personnage sans que rien ne le dise.
        let mut s = rien_de_special();
        s.appli_active = Some("notepad.exe".to_string());
        assert_eq!(biais_de(&s, &Config::default()), Biais::neutre());
    }

    #[test]
    fn un_modificateur_absent_laisse_le_poids_intact() {
        // `ModifsAppli` a des champs `Option` : une ligne qui ne parle que de
        // `flaner` ne doit pas remettre `seReposer` à zéro. Avec des `f32`
        // nus, un champ absent vaudrait 0.0 — donc INTERDIRAIT l'intention,
        // ce qui est le contraire d'un défaut inoffensif.
        let mut c = Config::default();
        c.applications.insert(
            "Code.exe".to_string(),
            ModifsAppli {
                flaner: Some(0.5),
                se_reposer: None,
                ..Default::default()
            },
        );

        let mut s = rien_de_special();
        s.appli_active = Some("Code.exe".to_string());

        let b = biais_de(&s, &c);
        assert_eq!(b.pour(Intention::Flaner), 0.5);
        assert_eq!(b.pour(Intention::SeReposer), 1.0);
    }
}
```

- [ ] **Step 2 : Lancer les tests pour vérifier qu'ils échouent**

Run : `cargo test signals`
Attendu : **échec de compilation** — `Signaux`, `Batterie`, `Biais`, `biais_de`, `ModifsAppli` et `Config::applications` n'existent pas.

- [ ] **Step 3 : Les types de la sonde**

Dans `src/probe/mod.rs`, après `MouseState` :

```rust
/// L'état du système à un instant, tel que le comportement a besoin de le
/// connaître (design de l'étape 2, §3).
///
/// **Un instantané et non cinq accesseurs.** La raison est la cohérence :
/// lues à cinq instants différents, ces valeurs pourraient montrer « session
/// verrouillée » et « actif il y a 10 ms » dans la même image du
/// comportement. Un instantané rend cet état impossible par construction.
///
/// Pas `Copy` : `appli_active` est une `String`. C'est exactement pourquoi le
/// comportement ne reçoit PAS cette structure mais un `signals::Biais`, qui
/// est `Copy` — voir `Entrees`.
#[derive(Debug, Clone, PartialEq)]
pub struct Signaux {
    /// Depuis combien de temps l'utilisateur n'a touché à rien.
    ///
    /// ⚠️ `GetLastInputInfo` rend un **compteur de millisecondes**, jamais
    /// une touche. C'est la seule voie compatible avec « aucune capture de
    /// frappe », qui est une exclusion explicite du besoin.
    pub inactivite: std::time::Duration,

    /// Le nom de fichier de l'exécutable au premier plan — `"Code.exe"`.
    ///
    /// Le nom seul, jamais le chemin complet : c'est ce que l'utilisateur
    /// écrira dans `config.json`, et un chemin serait impossible à deviner.
    /// `None` quand la fenêtre au premier plan n'appartient à aucun processus
    /// interrogeable — écran de connexion, fenêtre d'élévation UAC.
    pub appli_active: Option<String>,

    /// L'heure locale, de 0 à 23. Rien de plus fin : aucun signal du projet
    /// ne dépend de la minute.
    pub heure: u8,

    pub batterie: Batterie,

    /// Vrai pendant que la session est verrouillée (Win+L, veille avec mot de
    /// passe, changement d'utilisateur).
    pub session_verrouillee: bool,
}

/// L'état de la batterie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Batterie {
    /// `None` sur une machine **sans** batterie, ou quand Windows dit ne pas
    /// savoir.
    ///
    /// ⚠️ **Ce n'est pas `Some(100)`.** `GetSystemPowerStatus` rend 255 quand
    /// il n'y a pas de batterie ; confondre les deux ferait fatiguer un pet
    /// sur une tour de bureau, en permanence et sans raison visible.
    pub pourcent: Option<u8>,

    pub sur_secteur: bool,
}
```

- [ ] **Step 4 : Les réglages de config**

Dans `src/config.rs`, ajouter après `Allures` :

```rust
/// Les seuils et multiplicateurs des signaux (design de l'étape 2, §4).
///
/// Tout est ici plutôt qu'en dur dans `signals.rs` : c'est la décision n° 5,
/// et c'est aussi ce qui permet de régler « à partir de combien de temps
/// d'absence il s'endort » sans recompiler — le réglage qu'on voudra
/// certainement toucher après une journée d'usage.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SignauxReglages {
    /// Au-delà de cette durée sans aucune entrée, l'utilisateur est
    /// considéré comme parti.
    pub inactivite_secondes: f32,
    pub inactif_flaner: f32,
    pub inactif_se_reposer: f32,

    /// Le créneau « tard le soir », en heures locales. **Il passe par
    /// minuit** quand `debut > fin`, ce qui est le cas par défaut.
    pub soir_debut: u8,
    pub soir_fin: u8,
    pub soir_se_reposer: f32,

    /// En dessous de ce pourcentage **et** sur batterie, il fatigue.
    pub batterie_seuil: u8,
    pub batterie_se_reposer: f32,

    /// À partir de quel biais de repos il s'affale au lieu de rester assis
    /// (Tâche 4). 2,0 = « il faut qu'un signal ait au moins doublé l'envie
    /// de repos ».
    pub seuil_sommeil: f32,
}

impl Default for SignauxReglages {
    fn default() -> Self {
        // Les valeurs de départ de la spec §7.2.
        SignauxReglages {
            inactivite_secondes: 120.0,
            inactif_flaner: 0.2,
            inactif_se_reposer: 8.0,
            soir_debut: 22,
            soir_fin: 6,
            soir_se_reposer: 3.0,
            batterie_seuil: 20,
            batterie_se_reposer: 2.0,
            seuil_sommeil: 2.0,
        }
    }
}

/// Ce qu'une application au premier plan change au caractère du personnage.
///
/// Des `Option<f32>` et non des `f32` nus : une ligne qui ne parle que de
/// `flaner` ne doit pas remettre les autres poids à zéro. Avec des `f32`, un
/// champ absent vaudrait `0.0` — ce qui **interdirait** l'intention, le
/// contraire d'un défaut inoffensif.
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ModifsAppli {
    pub flaner: Option<f32>,
    pub se_reposer: Option<f32>,
}
```

Puis dans `Config`, deux champs de plus :

```rust
    pub signaux: SignauxReglages,

    /// Les modificateurs par application, `"Code.exe"` → ses poids.
    ///
    /// Une table associative, donc **ajouter une application ne demande
    /// aucun code** : c'est la forme la plus littérale de « ajouter un
    /// signal = ajouter une ligne » (décision n° 5).
    ///
    /// `BTreeMap` et non `HashMap` : l'ordre d'itération est stable, donc un
    /// message de diagnostic qui les liste ne change pas d'ordre d'une
    /// exécution à l'autre. Le coût de recherche est sans importance — la
    /// table a trois entrées et n'est consultée que 2 fois par seconde.
    pub applications: std::collections::BTreeMap<String, ModifsAppli>,
```

Et dans `impl Default for Config` :

```rust
            signaux: SignauxReglages::default(),
            // Vide par défaut : aucun modificateur d'application n'est
            // imposé. Le fichier d'exemple en montre deux, commentés par
            // leur seule présence.
            applications: std::collections::BTreeMap::new(),
```

- [ ] **Step 5 : `signals.rs`, la fonction pure**

En tête de `src/signals.rs`, **avant** le module de tests du Step 1 :

```rust
//! Les signaux du système, traduits en biais d'envie (design §7.2, décision
//! n° 3 ; design de l'étape 2 §4).
//!
//! Responsabilité unique : `Signaux` + `Config` → `Biais`. **Rien d'autre.**
//! Aucun appel système, aucun état, aucune horloge — une fonction pure, donc
//! testable en entier sans écran ni attente.
//!
//! > **Le seul verbe autorisé ici est MULTIPLIER.** Si l'on se surprend à
//! > écrire « si inactif alors dormir », la décision n° 3 est perdue et l'on
//! > a livré un afficheur d'état système déguisé en personnage.
//!
//! # Pourquoi un `Biais` plutôt que les `Signaux` eux-mêmes
//!
//! Trois raisons, par ordre d'importance :
//!
//! 1. **`Entrees` reste `Copy`.** L'étape 1a a conçu `Entrees` pour grossir
//!    précisément ici. Y mettre `Signaux`, qui contient une `String`,
//!    casserait son `Copy` et se paierait dans les signatures des trois
//!    couches.
//! 2. **La comparaison de chaînes sort du chemin 60 Hz.** Chercher
//!    `"Code.exe"` dans la table des applications se fait **2 fois par
//!    seconde**, pas 60. Sur un projet qui a mesuré son CPU trois fois de
//!    suite, ce n'est pas un détail de style.
//! 3. **Le point d'entrée existait déjà** : `desire::tirer_avec`, écrite à
//!    l'étape 1a avec son test, prend exactement un multiplicateur par
//!    intention.

use crate::behavior::intention::Intention;
use crate::config::Config;
use crate::probe::Signaux;
use std::time::Duration;

/// Un multiplicateur par intention.
///
/// Un champ par intention plutôt qu'une table associative : elles se comptent
/// sur les doigts, et un `match` exhaustif fait échouer la compilation quand
/// on en ajoute une — ce qui est exactement le rappel qu'on veut.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Biais {
    pub flaner: f32,
    pub se_reposer: f32,
}

impl Biais {
    /// Le biais qui ne biaise rien. C'est celui de toute l'étape 1, et celui
    /// que la simulation et la boucle passent tant que la Tâche 6 n'a pas
    /// branché la sonde.
    pub fn neutre() -> Biais {
        Biais {
            flaner: 1.0,
            se_reposer: 1.0,
        }
    }

    pub fn pour(&self, i: Intention) -> f32 {
        match i {
            Intention::Flaner => self.flaner,
            Intention::SeReposer => self.se_reposer,
        }
    }
}

/// Vrai si `heure` tombe dans le créneau `[debut, fin[`.
///
/// ⚠️ **Le créneau par défaut passe par minuit** (22 h → 6 h). Écrit
/// naïvement `heure >= debut && heure < fin`, il serait **toujours faux** :
/// aucune heure n'est à la fois ≥ 22 et < 6. C'est le genre de bug qui ne se
/// voit pas — le signal ne mord jamais, et l'on cherche ailleurs.
fn dans_le_creneau(heure: u8, debut: u8, fin: u8) -> bool {
    if debut <= fin {
        // Créneau ordinaire, par exemple 12 h → 14 h.
        heure >= debut && heure < fin
    } else {
        // Créneau à cheval sur minuit : 22 h → 6 h.
        heure >= debut || heure < fin
    }
}

/// Traduit l'état du système en multiplicateurs d'envie.
///
/// Les modificateurs **se multiplient** entre eux : inactif, le soir, sur
/// batterie faible donne 8 × 3 × 2 = ×48 sur le repos. Additionner donnerait
/// 13 et écraserait la différence entre « un signal » et « tous les
/// signaux ».
pub fn biais_de(s: &Signaux, c: &Config) -> Biais {
    let r = &c.signaux;
    let mut b = Biais::neutre();

    // ── L'utilisateur est parti ─────────────────────────────────────────
    if s.inactivite >= Duration::from_secs_f32(r.inactivite_secondes) {
        b.flaner *= r.inactif_flaner;
        b.se_reposer *= r.inactif_se_reposer;
    }

    // ── Tard le soir, ou la nuit ────────────────────────────────────────
    if dans_le_creneau(s.heure, r.soir_debut, r.soir_fin) {
        b.se_reposer *= r.soir_se_reposer;
    }

    // ── Batterie faible ─────────────────────────────────────────────────
    //
    // `if let Some(p)` : `None` veut dire « pas de batterie », PAS « batterie
    // vide » (voir `Batterie::pourcent`). Et la condition `!sur_secteur` est
    // indispensable : un portable branché à 10 % se recharge, il ne
    // s'épuise pas.
    if let Some(pourcent) = s.batterie.pourcent {
        if !s.batterie.sur_secteur && pourcent < r.batterie_seuil {
            b.se_reposer *= r.batterie_se_reposer;
        }
    }

    // ── L'application au premier plan ───────────────────────────────────
    //
    // `as_ref()` sur l'`Option<String>` pour emprunter la chaîne sans la
    // déplacer ; `get` sur la table accepte un `&str` grâce à `Borrow`.
    if let Some(nom) = s.appli_active.as_ref() {
        if let Some(m) = c.applications.get(nom.as_str()) {
            // `if let Some(x)` sur chaque champ : un modificateur absent
            // laisse le poids intact, il ne le remet pas à zéro.
            if let Some(x) = m.flaner {
                b.flaner *= x;
            }
            if let Some(x) = m.se_reposer {
                b.se_reposer *= x;
            }
        }
    }

    b
}
```

- [ ] **Step 6 : Déclarer le module et lancer les tests**

Dans `src/main.rs`, à côté des autres `mod` :

```rust
mod signals;
```

Run : `cargo test signals`
Attendu : **9 tests passent.**

- [ ] **Step 7 : Brancher la couture — « une ligne », comme promis**

Dans `src/behavior/mod.rs`, deux champs de plus sur `Entrees` :

```rust
    /// Les multiplicateurs d'envie du moment (décision n° 3).
    ///
    /// Recalculés à ~2 Hz par `signals::biais_de` et transportés tels quels
    /// jusqu'ici. Le comportement ne voit **jamais** un signal : il ne voit
    /// que des poids déjà multipliés, ce qui rend impossible d'écrire « si
    /// inactif alors dormir ».
    pub biais: crate::signals::Biais,

    /// L'utilisateur vient-il de toucher à quelque chose ?
    ///
    /// Dérivé du même seuil que le biais (`inactiviteSecondes`), mais gardé
    /// à part parce qu'il ne sert pas à la même chose : le biais **pondère un
    /// tirage**, celui-ci **interrompt un sommeil** (Tâche 5). Deux usages,
    /// deux champs — les fondre obligerait à deviner l'un depuis l'autre.
    pub utilisateur_actif: bool,
```

Puis, dans `pas`, la couche 3 :

```rust
    // ── Couche 3 : tirer une nouvelle envie ─────────────────────────────
    //
    // **La ligne que l'étape 1a avait écrite pour ce moment.** `tirer_avec`
    // existait déjà, avec son test (`un_multiplicateur_biaise_sans_commander`) :
    // brancher les signaux ne touche donc ni `desire.rs`, ni `intention.rs`,
    // ni `reflex.rs`. C'est la décision n° 5 qui se paie ici.
    if let Some(kind) = table.tirer_avec(&ch.manifest, rng, |i| e.biais.pour(i)) {
        ch.intention = Some(intention::ActiveIntention::nouvelle(kind, maintenant));
    }
```

- [ ] **Step 8 : Réparer les deux appelants, en NEUTRE**

Dans `src/sim.rs`, la construction des `Entrees` :

```rust
    let entrees = Entrees {
        souris: Point::new(0.0, 0.0),
        echelle_affichage: 1.0,
        bouton_gauche: false,
        curseur_sur_le_personnage: false,
        // Neutre jusqu'à la Tâche 7, qui jouera une journée entière.
        biais: crate::signals::Biais::neutre(),
        utilisateur_actif: true,
    };
```

Dans `src/main.rs`, la construction des `Entrees` de la boucle, à l'identique :

```rust
        let entrees = Entrees {
            souris: m.pos,
            echelle_affichage,
            bouton_gauche: m.left_down,
            curseur_sur_le_personnage: sur_le_personnage,
            // Neutre jusqu'à la Tâche 6, qui branche la sonde à 2 Hz.
            biais: signals::Biais::neutre(),
            utilisateur_actif: true,
        };
```

Et dans les tests de `src/behavior/reflex.rs` et `src/behavior/intention.rs`, ajouter les deux champs partout où des `Entrees` sont construites.

> **Astuce pour les trouver toutes :** le compilateur les liste. `cargo test`
> échouera avec `missing fields biais and utilisateur_actif` en donnant chaque
> emplacement. Ne pas les chercher à la main.

- [ ] **Step 9 : Lancer TOUTE la suite**

Run : `cargo test`
Attendu : **149 tests passent** (140 + 9), et **aucun test d'étape 1 modifié**. Si un test de comportement change de résultat, la couture n'est pas neutre : c'est un bug de cette tâche, pas un test à ajuster.

- [ ] **Step 10 : Documenter les clés dans `config.exemple.json`**

Ajouter, en gardant la propriété « toutes les clés sont optionnelles » :

```json
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
    "Code.exe": { "flaner": 0.5 },
    "chrome.exe": { "flaner": 1.5, "seReposer": 2.0 }
  }
```

- [ ] **Step 11 : Commit**

```bash
git add src/signals.rs src/probe/mod.rs src/config.rs src/behavior src/main.rs src/sim.rs config.exemple.json
git commit -m "feat(etape-2): le biais, des signaux aux poids d'envie

signals.rs est une fonction PURE : Signaux + Config -> Biais. Aucun appel
systeme, aucun etat, aucune horloge, donc testable en entier sans ecran.

Un Biais et non les Signaux eux-memes, pour trois raisons dans cet ordre :
Entrees reste Copy (les Signaux portent une String) ; la recherche de
l'application dans la table sort du chemin 60 Hz pour aller a 2 Hz ; et
tirer_avec existait deja, ecrite a l'etape 1a POUR ce moment.

Cette tache ne change AUCUN comportement : le biais est neutre partout, et
les 140 tests de l'etape 1 passent sans etre modifies. C'est ce qui la rend
verifiable.

Deux pieges couverts par un test chacun :
- le creneau du soir PASSE PAR MINUIT (22h -> 6h). Ecrit naivement en ET, il
  serait toujours faux et le signal ne mordrait jamais.
- pas de batterie n'est pas une batterie vide. GetSystemPowerStatus rend 255
  sur une tour de bureau ; d'ou Option<u8>.

Les modificateurs se multiplient : inactif + soir + batterie = x48 sur le
repos. Additionner donnerait 13 et ecraserait la difference entre un signal
et tous les signaux."
```

---

## Tâche 2 : La sonde — cinq appels Windows, et une sous-commande pour les voir

**Files:**
- Modify: `src/probe/mod.rs` (`fn signaux()` sur le trait)
- Modify: `src/probe/fake.rs` (des signaux réglables)
- Modify: `src/probe/win32.rs` (les cinq appels)
- Modify: `src/main.rs` (la sous-commande `--signaux`)
- Modify: `Cargo.toml` (quatre *features*)

**Interfaces:**
- Consomme : `probe::Signaux`, `probe::Batterie` (Tâche 1)
- Produit :
  - `SystemProbe::signaux(&self) -> Signaux`
  - `FakeProbe::set_signaux(&self, s: Signaux)`
  - la sous-commande `shimeji-desktop --signaux`

> **C'est le seul `unsafe` de l'étape.** Les cinq appels ont été **vérifiés
> dans les sources de `windows` 0.61.3** le 2026-09-09, signatures comprises.
> Ne pas en deviner un sixième : le relever d'abord.

- [ ] **Step 1 : Écrire le test de la sonde factice**

Dans le module `tests` de `src/probe/fake.rs` :

```rust
    #[test]
    fn les_signaux_se_reglent_a_travers_une_reference_partagee() {
        // Même contrainte que pour la souris : le trait expose `&self`, donc
        // un test qui n'a qu'une référence partagée doit pouvoir changer les
        // signaux. `Signaux` n'étant pas `Copy` (il porte une `String`),
        // c'est un `RefCell` et non un `Cell`.
        let p = FakeProbe::un_ecran();
        let vue: &dyn SystemProbe = &p;

        // Le défaut : personne n'est parti, il est 15 h, sur secteur, session
        // ouverte. C'est « rien de spécial », comme dans les tests de
        // `signals.rs`.
        let d = vue.signaux();
        assert_eq!(d.inactivite, std::time::Duration::ZERO);
        assert!(!d.session_verrouillee);
        assert_eq!(d.heure, 15);

        p.set_signaux(Signaux {
            inactivite: std::time::Duration::from_secs(300),
            appli_active: Some("Code.exe".to_string()),
            heure: 23,
            batterie: Batterie {
                pourcent: Some(7),
                sur_secteur: false,
            },
            session_verrouillee: true,
        });

        let s = vue.signaux();
        assert_eq!(s.inactivite, std::time::Duration::from_secs(300));
        assert_eq!(s.appli_active.as_deref(), Some("Code.exe"));
        assert_eq!(s.heure, 23);
        assert_eq!(s.batterie.pourcent, Some(7));
        assert!(s.session_verrouillee);
    }
```

- [ ] **Step 2 : Lancer le test pour vérifier qu'il échoue**

Run : `cargo test les_signaux_se_reglent`
Attendu : **échec de compilation** — `set_signaux` et `SystemProbe::signaux` n'existent pas.

- [ ] **Step 3 : La méthode du trait**

Dans `src/probe/mod.rs`, dans `trait SystemProbe` :

```rust
    /// Tout ce qui change lentement, lu **d'un coup** (design §5.5 : ~2 Hz).
    ///
    /// Appelée deux fois par seconde et pas davantage : aucun de ces signaux
    /// ne bouge vite, et cinq appels système à 60 Hz seraient 300 appels par
    /// seconde pour des valeurs qui changent toutes les minutes.
    fn signaux(&self) -> Signaux;
```

- [ ] **Step 4 : La sonde factice**

Dans `src/probe/fake.rs` — le champ, le défaut et le réglage :

```rust
pub struct FakeProbe {
    screens: Vec<ScreenInfo>,
    mouse: Cell<MouseState>,

    // `RefCell` et non `Cell` : `Signaux` n'est pas `Copy`, il porte le nom
    // de l'application active. `Cell::get` exige `Copy` ; `RefCell` prête à
    // la place, au prix d'un compteur d'emprunts vérifié à l'exécution.
    signaux: RefCell<Signaux>,
}
```

`use std::cell::{Cell, RefCell};` en tête, et `use super::{Batterie, MouseState, ScreenInfo, Signaux, SystemProbe};`.

Dans `FakeProbe::new`, initialiser :

```rust
            // Le défaut est délibérément « rien de spécial » : aucun signal
            // ne mord, donc un test d'étape 1 qui ignore les signaux garde
            // exactement le comportement qu'il avait.
            signaux: RefCell::new(Signaux {
                inactivite: std::time::Duration::ZERO,
                appli_active: None,
                heure: 15,
                batterie: Batterie {
                    pourcent: None,
                    sur_secteur: true,
                },
                session_verrouillee: false,
            }),
```

Et les deux méthodes :

```rust
    pub fn set_signaux(&self, s: Signaux) {
        *self.signaux.borrow_mut() = s;
    }

    /// Raccourci pour le cas le plus fréquent des tests : « il est parti
    /// depuis N secondes ». Écrire les cinq champs à chaque fois noierait
    /// l'intention du test dans du remplissage.
    pub fn set_inactivite(&self, d: std::time::Duration) {
        self.signaux.borrow_mut().inactivite = d;
    }
```

```rust
impl SystemProbe for FakeProbe {
    // … screens() et mouse() inchangées …

    fn signaux(&self) -> Signaux {
        // `clone` : le trait rend une valeur possédée, et `Signaux` n'est pas
        // `Copy`. Deux fois par seconde, une chaîne de vingt caractères —
        // sans importance.
        self.signaux.borrow().clone()
    }
}
```

- [ ] **Step 5 : Lancer le test de la sonde factice**

Run : `cargo test probe`
Attendu : **échec** — `Win32Probe` n'implémente pas `signaux`, donc le paquet ne compile pas. C'est le rappel utile : on ne peut pas ajouter une méthode au trait sans la vraie sonde.

- [ ] **Step 6 : Les quatre *features* de `windows`**

Dans `Cargo.toml`, sous `[dependencies.windows]` :

```toml
    "Win32_System_Threading",           # OpenProcess, QueryFullProcessImageNameW
    "Win32_System_SystemInformation",   # GetLocalTime, GetTickCount
    "Win32_System_Power",               # GetSystemPowerStatus
    "Win32_System_RemoteDesktop",       # WTSQuerySessionInformationW — verrouillage
```

- [ ] **Step 7 : Les cinq appels, dans `probe/win32.rs`**

```rust
// ── Les signaux (étape 2) ───────────────────────────────────────────────
//
// Les cinq appels ci-dessous ont été vérifiés dans les sources de
// `windows` 0.61.3 le 2026-09-09 — signatures comprises. Les emplacements
// sont dans `docs/specs/2026-09-09-etape-2-design.md` §3.

use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::RemoteDesktop::{
    WTSFreeMemory, WTSQuerySessionInformationW, WTSSessionInfoEx, WTSINFOEXW,
    WTS_SESSIONSTATE_LOCK,
};
use windows::Win32::System::SystemInformation::{GetLocalTime, GetTickCount};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// La session courante, pour `WTSQuerySessionInformationW`.
///
/// ⚠️ **`WTS_CURRENT_SESSION` n'existe pas dans la crate `windows`** — vérifié
/// dans les sources. La valeur est `(DWORD)-1` dans `wtsapi32.h`, donc
/// `u32::MAX`. On la définit ici, avec ce commentaire, plutôt que d'écrire un
/// `0xFFFFFFFF` nu que personne ne pourrait relier à sa source.
const SESSION_COURANTE: u32 = u32::MAX;

/// Depuis combien de temps l'utilisateur n'a touché à rien.
///
/// ⚠️ **Ce compteur ne dit JAMAIS quelle touche a été pressée** — c'est
/// l'exclusion « aucune capture de frappe » du besoin, et c'est la seule
/// raison pour laquelle ce signal est acceptable.
fn inactivite() -> std::time::Duration {
    // `cbSize` doit être renseigné AVANT l'appel : c'est ainsi que Windows
    // sait quelle version de la structure on lui passe. Oublié, l'appel
    // échoue sans autre explication.
    let mut lii = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };

    let ok = unsafe { GetLastInputInfo(&mut lii) };
    if !ok.as_bool() {
        // Échec : on rend zéro, soit « l'utilisateur vient d'agir ». C'est le
        // défaut prudent — il ne s'endormira pas à cause d'une erreur de
        // sonde, alors que rendre « inactif depuis 10 min » l'endormirait à
        // tort et sans explication.
        return std::time::Duration::ZERO;
    }

    let maintenant = unsafe { GetTickCount() };

    // `wrapping_sub` et non `-` : `GetTickCount` repasse à zéro au bout de
    // 49,7 jours de fonctionnement. La soustraction qui déborde rend le bon
    // écart malgré le tour ; une soustraction ordinaire paniquerait en debug.
    let ecart_ms = maintenant.wrapping_sub(lii.dwTime);
    std::time::Duration::from_millis(ecart_ms as u64)
}

/// Le nom de fichier de l'exécutable au premier plan — `"Code.exe"`.
fn appli_active() -> Option<String> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        // Aucune fenêtre au premier plan : ça arrive pendant un changement de
        // bureau, ou sur l'écran de verrouillage.
        return None;
    }

    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }

    // `PROCESS_QUERY_LIMITED_INFORMATION` et non `PROCESS_QUERY_INFORMATION` :
    // le droit limité suffit à lire le chemin de l'image, et il est accordé
    // même sur des processus d'un autre niveau d'intégrité. Le droit complet
    // échouerait sur toute application élevée.
    //
    // `ok()?` : l'échec est normal (processus protégé, processus qui vient de
    // mourir) et n'est pas une erreur à signaler — on rend `None`.
    let processus = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;

    let mut tampon = [0u16; 260]; // MAX_PATH
    let mut taille = tampon.len() as u32;

    let resultat = unsafe {
        QueryFullProcessImageNameW(
            processus,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(tampon.as_mut_ptr()),
            &mut taille,
        )
    };

    // Le handle se ferme dans TOUS les cas, y compris en cas d'échec de
    // l'appel ci-dessus. C'est pour ça qu'on ne fait pas `?` sur `resultat`
    // avant cette ligne : un `return` précoce fuirait le handle.
    unsafe { let _ = windows::Win32::Foundation::CloseHandle(processus); };
    resultat.ok()?;

    // `taille` contient maintenant la longueur écrite, sans le zéro final.
    let chemin = String::from_utf16_lossy(&tampon[..taille as usize]);

    // Le NOM seul, jamais le chemin : c'est ce que l'utilisateur écrira dans
    // `config.json`, et un chemin complet y serait indevinable.
    //
    // `rsplit('\\').next()` rend le dernier segment ; sur un chemin sans
    // antislash il rend la chaîne entière, ce qui est correct.
    chemin.rsplit('\\').next().map(|s| s.to_string())
}

/// L'heure locale, 0 à 23.
///
/// `GetLocalTime` et non l'heure UTC : « il est tard le soir » se juge à
/// l'heure de l'utilisateur. Et surtout pas la crate `chrono` — ce serait une
/// dépendance entière pour lire un `u16`.
fn heure_locale() -> u8 {
    let t = unsafe { GetLocalTime() };
    // `wHour` est un `u16` de 0 à 23 : le `as u8` ne peut pas tronquer.
    t.wHour as u8
}

fn batterie() -> super::Batterie {
    let mut etat = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut etat) }.is_err() {
        // Sonde en échec : on rend « sur secteur, pourcentage inconnu », donc
        // aucun biais. Le personnage ne doit pas fatiguer à cause d'une
        // erreur de lecture.
        return super::Batterie {
            pourcent: None,
            sur_secteur: true,
        };
    }

    // ⚠️ `BatteryLifePercent` vaut **255** quand le pourcentage est inconnu —
    // ce qui est le cas sur toute machine sans batterie. Le rendre tel quel
    // donnerait « 255 % », et le comparer à un seuil donnerait « pas de
    // batterie faible » par accident plutôt que par raison.
    let pourcent = if etat.BatteryLifePercent <= 100 {
        Some(etat.BatteryLifePercent)
    } else {
        None
    };

    // `ACLineStatus` : 0 hors secteur, 1 sur secteur, 255 inconnu. On traite
    // « inconnu » comme « sur secteur », le défaut qui ne fatigue pas.
    let sur_secteur = etat.ACLineStatus != 0;

    super::Batterie {
        pourcent,
        sur_secteur,
    }
}

/// La session est-elle verrouillée ?
fn session_verrouillee() -> bool {
    let mut tampon = windows::core::PWSTR::null();
    let mut octets: u32 = 0;

    // `None` pour le serveur = la machine locale.
    let appel = unsafe {
        WTSQuerySessionInformationW(
            None,
            SESSION_COURANTE,
            WTSSessionInfoEx,
            &mut tampon,
            &mut octets,
        )
    };

    if appel.is_err() || tampon.is_null() {
        // On rend « déverrouillée » : le défaut qui laisse le personnage
        // vivre. Le contraire le ferait disparaître sur une erreur de sonde,
        // ce qui ressemblerait à un plantage.
        return false;
    }

    // `WTSQuerySessionInformationW` ALLOUE : il faut libérer avec
    // `WTSFreeMemory`, sinon on fuit une centaine d'octets deux fois par
    // seconde — soit ~17 Mo par jour.
    //
    // On lit d'abord, on libère ensuite, et on ne sort qu'après.
    let verrouillee = unsafe {
        let info = &*(tampon.0 as *const WTSINFOEXW);

        // `Level` doit valoir 1 pour que l'union porte un
        // `WTSInfoExLevel1`. Lire l'union sans vérifier serait une lecture
        // de mémoire non initialisée.
        if info.Level == 1 {
            info.Data.WTSInfoExLevel1.SessionFlags == WTS_SESSIONSTATE_LOCK as i32
        } else {
            false
        }
    };

    unsafe { WTSFreeMemory(tampon.0 as *mut core::ffi::c_void) };
    verrouillee
}
```

Et l'implémentation du trait :

```rust
    fn signaux(&self) -> super::Signaux {
        super::Signaux {
            inactivite: inactivite(),
            appli_active: appli_active(),
            heure: heure_locale(),
            batterie: batterie(),
            session_verrouillee: session_verrouillee(),
        }
    }
```

- [ ] **Step 8 : Lancer la suite**

Run : `cargo test`
Attendu : **150 tests passent** (149 + 1).

- [ ] **Step 9 : La sous-commande `--signaux`, pour voir la vraie sonde**

Un test ne peut pas vérifier que `GetLastInputInfo` rend la bonne valeur : il n'y a rien à comparer. La vérification est donc **une sous-commande qui imprime l'instantané**, sur le modèle de `--demarrage etat` du plan 1b.

Dans `main`, avec les autres sous-commandes :

```rust
    // `--signaux` : imprime l'instantané de la sonde et sort.
    //
    // C'est la seule vérification possible des cinq appels Windows : aucun
    // test ne peut savoir depuis combien de temps l'utilisateur n'a rien
    // touché. Rendue scriptable plutôt que laissée à l'œil, comme le reste
    // du projet — on peut la lancer deux fois à 5 s d'intervalle et vérifier
    // que l'inactivité a bien augmenté de 5 s.
    if args.iter().any(|a| a == "--signaux") {
        probe::win32::activer_conscience_dpi();
        let sonde = probe::win32::Win32Probe::new();
        let s = sonde.signaux();
        println!("inactivite        : {:.1} s", s.inactivite.as_secs_f32());
        println!("appli active      : {}", s.appli_active.as_deref().unwrap_or("(aucune)"));
        println!("heure locale      : {} h", s.heure);
        match s.batterie.pourcent {
            Some(p) => println!("batterie          : {p} %"),
            None => println!("batterie          : (aucune, ou inconnue)"),
        }
        println!("sur secteur       : {}", s.batterie.sur_secteur);
        println!("session verrouillee : {}", s.session_verrouillee);
        return;
    }
```

- [ ] **Step 10 : Vérifier la vraie sonde, sans clic**

```powershell
cd src-tauri
cargo build
.\target\debug\shimeji-desktop.exe --signaux
Start-Sleep -Seconds 6
.\target\debug\shimeji-desktop.exe --signaux
```

Attendu, et **chaque ligne est une vérification distincte** :

| Ligne | Attendu |
|---|---|
| `inactivite` | proche de 0 s aux deux appels — on vient de taper la commande. **Ne pas toucher souris ni clavier** pendant les 6 s, et le second appel doit alors afficher ~6 s. C'est ce qui prouve que le compteur avance. |
| `appli active` | `WindowsTerminal.exe`, `powershell.exe` ou `Code.exe` — le terminal d'où l'on lance. Un chemin complet serait un bug (`rsplit`). |
| `heure locale` | l'heure au mur. Si c'est décalé, c'est `GetSystemTime` au lieu de `GetLocalTime`. |
| `batterie` | `(aucune, ou inconnue)` sur une tour ; un pourcentage sur un portable. **`255 %` serait le bug de `BatteryLifePercent`.** |
| `session verrouillee` | `false` — on est en train de regarder l'écran. |

- [ ] **Step 11 : Vérifier le verrouillage, la seule ligne qui demande un geste**

```powershell
# Lancer en tâche de fond, verrouiller, déverrouiller, relire :
Start-Job { Start-Sleep -Seconds 12; & "$PWD\target\debug\shimeji-desktop.exe" --signaux } | Out-Null
rundll32.exe user32.dll,LockWorkStation
# … déverrouiller, puis :
Receive-Job -Wait *
```

Attendu : `session verrouillee : true` sur la lecture faite pendant le verrouillage.

> **`rundll32 user32.dll,LockWorkStation` verrouille sans clic** — c'est
> l'équivalent scriptable de Win+L. Le déverrouillage, lui, demande le mot de
> passe : c'est le seul geste humain de cette tâche, et il est irréductible.

- [ ] **Step 12 : Commit**

```bash
git add src/probe Cargo.toml src/main.rs
git commit -m "feat(etape-2): la sonde des cinq signaux, et --signaux pour les voir

Une seule methode de plus sur SystemProbe, qui rend un INSTANTANE : lues a
cinq instants differents, ces valeurs pourraient montrer « session
verrouillee » et « actif il y a 10 ms » dans la meme image du comportement.

GetLastInputInfo rend un compteur de millisecondes, JAMAIS une touche. C'est
la seule voie compatible avec l'exclusion « aucune capture de frappe ».

Cinq pieges, chacun commente a son emplacement :
- LASTINPUTINFO.cbSize doit etre renseigne avant l'appel, sinon echec muet.
- GetTickCount repasse a zero apres 49,7 jours : wrapping_sub, pas -.
- BatteryLifePercent vaut 255 quand il n'y a pas de batterie. Rendu tel quel,
  on aurait « 255 % » et un pet qui ne fatigue jamais par accident.
- WTSQuerySessionInformationW ALLOUE : WTSFreeMemory obligatoire, sinon on
  fuit ~17 Mo par jour a 2 Hz. Et Level doit valoir 1 avant de lire l'union.
- WTS_CURRENT_SESSION n'existe pas dans la crate : defini ici a u32::MAX,
  avec le commentaire qui dit d'ou vient la valeur.

PROCESS_QUERY_LIMITED_INFORMATION et non PROCESS_QUERY_INFORMATION : le droit
limite suffit a lire le chemin et fonctionne sur les applications elevees.

Toutes les sondes en echec rendent le defaut qui LAISSE VIVRE le personnage :
inactivite zero, sur secteur, session ouverte. Une erreur de sonde ne doit pas
l'endormir ni le faire disparaitre.

--signaux imprime l'instantane et sort : c'est la seule verification possible
des cinq appels, aucun test ne pouvant savoir depuis combien de temps
l'utilisateur n'a rien touche. Deux appels a 6 s d'intervalle montrent le
compteur avancer."
```

---

## Tâche 3 : `Jouer(Jeu)` — deux animations, deux lignes de table

**Files:**
- Modify: `src/character/manifest.rs` (deux constantes de pose)
- Modify: `src/behavior/intention.rs` (`Intention::Jouer(Jeu)`, `EtatIntention::Jeu`, `fn jouer`)
- Modify: `src/behavior/desire.rs` (deux lignes de table)
- Modify: `src/signals.rs` (`Biais.jouer`)
- Modify: `src/config.rs` (`Envies.jouer`, `ModifsAppli.jouer`)
- Modify: `src/sim.rs` (le jeton de signature)
- Modify: `characters/blob/mascot.json` (la pose `spinHead`)
- Modify: `config.exemple.json`

**Interfaces:**
- Consomme : `signals::Biais` (Tâche 1), `desire::EntreeEnvie`, `intention::Issue`
- Produit :
  - `intention::Jeu { TeteQuiTourne, JambesQuiBalancent }`, avec `fn pose(&self) -> &'static str`
  - `intention::Intention::Jouer(Jeu)`
  - `manifest::POSE_SPIN_HEAD = "spinHead"`, `manifest::POSE_SIT_DANGLE = "sitDangle"`
  - `config::Envies::jouer`, `config::ModifsAppli::jouer`, `signals::Biais::jouer`

> **Deux lignes de table et non une intention qui choisit.** `poses_requises`
> est une liste **ET** : une seule ligne exigerait les deux animations, et un
> pack n'en ayant qu'une ne jouerait jamais. Séparées, la couverture partielle
> (spec §8.6) joue **par animation**. C'est aussi la lecture littérale de la
> notation `Jouer(action)` de la spec §7.1.

- [ ] **Step 1 : Écrire les tests**

Dans le module `tests` de `src/behavior/desire.rs` :

```rust
    #[test]
    fn chaque_jeu_est_retire_separement_du_tirage() {
        // **LE test des deux lignes.** Un pack qui n'a que `spinHead` doit
        // jouer quand même — avec cette animation seulement. Une intention
        // `Jouer` unique exigeant les deux poses ne jouerait pas du tout.
        use crate::behavior::intention::Jeu;

        let m = manifeste_avec(&["stand", "walk", "sit", "spinHead"]);
        let table = TableEnvies::defaut();
        let mut rng = XorShift32::seeded(3);

        let mut vus = std::collections::BTreeSet::new();
        for _ in 0..2_000 {
            if let Some(i) = table.tirer(&m, &mut rng) {
                vus.insert(i);
            }
        }

        assert!(
            vus.contains(&Intention::Jouer(Jeu::TeteQuiTourne)),
            "il a spinHead : il doit pouvoir se tourner la tête"
        );
        assert!(
            !vus.contains(&Intention::Jouer(Jeu::JambesQuiBalancent)),
            "il n'a pas sitDangle : cette animation doit être retirée"
        );
    }

    #[test]
    fn le_biais_de_jeu_porte_sur_les_deux_animations() {
        // Le modificateur par application dit « jouer ×3 » sans distinguer
        // les animations : les deux lignes doivent donc en profiter.
        use crate::behavior::intention::Jeu;

        let m = manifeste_avec(&["stand", "walk", "sit", "spinHead", "sitDangle"]);
        let table = TableEnvies::defaut();
        let mut rng = XorShift32::seeded(11);

        let (mut jeux, mut autres) = (0, 0);
        for _ in 0..10_000 {
            let mult = |i: Intention| match i {
                Intention::Jouer(_) => 3.0,
                _ => 1.0,
            };
            match table.tirer_avec(&m, &mut rng, mult) {
                Some(Intention::Jouer(_)) => jeux += 1,
                Some(_) => autres += 1,
                None => {}
            }
        }

        // Poids : flâner 5, reposer 1, deux jeux à 1 × 3 = 6. Donc 6 / 12.
        let part = jeux as f32 / (jeux + autres) as f32;
        assert!((part - 0.5).abs() < 0.03, "part des jeux : {part}");
    }
```

Dans le module `tests` de `src/behavior/intention.rs` :

```rust
    #[test]
    fn jouer_pose_l_animation_du_jeu_tire() {
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);

        for (jeu, pose) in [
            (Jeu::TeteQuiTourne, POSE_SPIN_HEAD),
            (Jeu::JambesQuiBalancent, POSE_SIT_DANGLE),
        ] {
            ch.intention = Some(ActiveIntention::nouvelle(
                Intention::Jouer(jeu),
                Duration::ZERO,
            ));
            let issue = poursuivre(&mut ch, &m, &reglages(), Duration::ZERO, DT, &mut rng);
            assert_eq!(issue, Issue::EnCours);
            assert_eq!(ch.pose, pose, "jeu {jeu:?}");
        }
    }

    #[test]
    fn jouer_ne_deplace_pas_le_personnage() {
        // Les deux jeux sont des animations assises : `Velocity="0,0"` dans
        // `actions.xml`. Si le personnage dérivait, c'est qu'une vitesse
        // traîne quelque part.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::Jouer(Jeu::TeteQuiTourne),
            Duration::ZERO,
        ));

        let ou = offset_de(&ch);
        for i in 0..120 {
            poursuivre(
                &mut ch,
                &m,
                &reglages(),
                Duration::from_secs_f32(i as f32 * DT),
                DT,
                &mut rng,
            );
        }
        assert_eq!(offset_de(&ch), ou);
    }

    #[test]
    fn jouer_se_termine_avant_le_delai_d_abandon() {
        // Comme le repos : la durée est bornée sous `DELAI_ABANDON`, sinon
        // l'issue serait `Echouee` au lieu de `Finie` et la trace du mode
        // simulation mentirait.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::Jouer(Jeu::TeteQuiTourne),
            Duration::ZERO,
        ));

        let mut issue = Issue::EnCours;
        for i in 0..(20 * 60) {
            issue = poursuivre(
                &mut ch,
                &m,
                &reglages(),
                Duration::from_secs_f32(i as f32 * DT),
                DT,
                &mut rng,
            );
            if issue != Issue::EnCours {
                break;
            }
        }
        assert_eq!(issue, Issue::Finie);
    }

    #[test]
    fn jouer_sans_la_pose_echoue_au_lieu_de_figer() {
        // Défense en profondeur, comme `se_reposer_sans_pose_sit_echoue` :
        // le tirage filtre déjà, mais une config bricolée ne doit pas
        // produire un personnage invisible.
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "stand": { "frames": [1] }, "walk": { "frames": [2] } }
        }"#;
        let m = monde();
        let mut ch = Character::new(
            serde_json::from_str(json).unwrap(),
            Attachment::On {
                platform: m.platforms()[0].id,
                face: Face::Top,
                offset: 500.0,
            },
            Point::new(500.0, 1032.0),
        );
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::Jouer(Jeu::TeteQuiTourne),
            Duration::ZERO,
        ));

        assert_eq!(
            poursuivre(&mut ch, &m, &reglages(), Duration::ZERO, DT, &mut rng),
            Issue::Echouee
        );
    }
```

- [ ] **Step 2 : Lancer les tests pour vérifier qu'ils échouent**

Run : `cargo test jouer`
Attendu : **échec de compilation** — `Jeu`, `Intention::Jouer`, `POSE_SPIN_HEAD` et `POSE_SIT_DANGLE` n'existent pas.

- [ ] **Step 3 : Les constantes de pose**

Dans `src/character/manifest.rs`, avec les autres :

```rust
/// Assis, il se tourne la tête. Source : `SitAndSpinHeadAction`.
pub const POSE_SPIN_HEAD: &str = "spinHead";

/// Assis à balancer les jambes. Source : `SitAndDangleLegs`.
///
/// ⚠️ Son ancre est `64,112` et non `64,128` : les jambes pendent **sous** la
/// ligne de contact. Sur le sol, elles descendent donc de 16 px dans la barre
/// des tâches — c'est ce que fait Shimeji-ee, qui déclare bien cette action
/// avec `BorderType="Floor"`.
pub const POSE_SIT_DANGLE: &str = "sitDangle";
```

- [ ] **Step 4 : Le type `Jeu`, et la variante d'intention**

Dans `src/behavior/intention.rs` :

```rust
/// À quoi il joue.
///
/// Un `enum` et non un nom de pose libre : la table d'envies a besoin d'une
/// clé `Copy + Eq`, et une variante par animation permet à la couverture
/// partielle de les retirer **séparément** (spec §8.6).
///
/// C'est la lecture littérale de `Jouer(action)` de la spec §7.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Jeu {
    /// Assis, la tête qui tourne — 8 frames, 200 ms chacune.
    TeteQuiTourne,
    /// Assis à balancer les jambes — 4 frames, 400 ms, en boucle.
    JambesQuiBalancent,
}

impl Jeu {
    /// La pose que ce jeu demande. **Une seule** : c'est ce qui rend le
    /// retrait par couverture partielle exact — une animation absente ne
    /// retire que son propre jeu.
    pub fn pose(&self) -> &'static str {
        match self {
            Jeu::TeteQuiTourne => POSE_SPIN_HEAD,
            Jeu::JambesQuiBalancent => POSE_SIT_DANGLE,
        }
    }

    /// Les poses requises, sous la forme attendue par la table d'envies.
    ///
    /// `&'static [&'static str]` : la table stocke des tranches statiques
    /// pour n'allouer jamais. Une constante par jeu, donc, plutôt qu'un
    /// `Vec` construit à la volée.
    pub fn poses_requises(&self) -> &'static [&'static str] {
        match self {
            Jeu::TeteQuiTourne => &[POSE_SPIN_HEAD],
            Jeu::JambesQuiBalancent => &[POSE_SIT_DANGLE],
        }
    }
}
```

Puis la variante et son état :

```rust
pub enum Intention {
    Flaner,
    SeReposer,
    Jouer(Jeu),
}
```

```rust
pub enum EtatIntention {
    Flanerie { allure: Allure, jusqu_a: Duration },
    Repos { jusqu_a: Duration },
    Jeu { jusqu_a: Duration },
}
```

Dans `ActiveIntention::nouvelle`, la branche correspondante :

```rust
            Intention::Jouer(_) => EtatIntention::Jeu {
                jusqu_a: Duration::ZERO,
            },
```

- [ ] **Step 5 : `fn jouer`**

Dans `src/behavior/intention.rs` :

```rust
/// Jouer : poser une animation assise et la laisser tourner.
///
/// Plus simple que `se_reposer`, dont il ne partage pas la logique de phases :
/// un jeu n'a pas d'étape. Les deux fonctions restent séparées pour cette
/// raison — les fondre demanderait un paramètre « as-tu des phases ? », qui
/// est le signe d'une mauvaise abstraction.
fn jouer(
    ch: &mut Character,
    jeu: Jeu,
    ai: &mut ActiveIntention,
    maintenant: Duration,
    rng: &mut dyn Rng,
) -> Issue {
    let EtatIntention::Jeu { mut jusqu_a } = ai.etat else {
        ch.intention = None;
        return Issue::Echouee;
    };

    // Défense en profondeur : `desire.rs` filtre déjà sur la pose, mais une
    // config bricolée pourrait proposer ce jeu à un personnage qui n'a pas
    // l'animation — et un personnage posé sur une pose inexistante serait
    // invisible. Mieux vaut échouer et re-tirer.
    if !ch.manifest.has_pose(jeu.pose()) {
        ch.intention = None;
        return Issue::Echouee;
    }

    if jusqu_a == Duration::ZERO {
        // Première image : on tire la durée du jeu.
        //
        // 4 à 15 s, la même plage que le repos : bornée sous
        // `DELAI_ABANDON` pour que l'issue soit `Finie` et non `Echouee`.
        jusqu_a = maintenant + Duration::from_secs_f32(rng.range(4.0, 15.0));
        ai.etat = EtatIntention::Jeu { jusqu_a };
    } else if maintenant >= jusqu_a {
        ch.intention = None;
        return Issue::Finie;
    }

    ch.set_pose(jeu.pose(), maintenant);
    Issue::EnCours
}
```

Et la branche dans `poursuivre`, à côté des deux autres :

```rust
        Intention::Jouer(jeu) => {
            let issue = jouer(ch, jeu, &mut ai, maintenant, rng);
            if ch.intention.is_some() {
                ch.intention = Some(ai);
            }
            issue
        }
```

> **Le `if ch.intention.is_some()`** reprend la correction de l'étape 1a :
> sans lui, réécrire `ch.intention` ressusciterait une intention que `jouer`
> vient d'annuler.

- [ ] **Step 6 : Les deux lignes de table**

Dans `src/behavior/desire.rs`, `depuis_config` :

```rust
                // **Deux lignes et non une**, pour que la couverture partielle
                // joue par animation (spec §8.6) : un pack qui n'a que l'une
                // des deux joue quand même.
                //
                // Les deux partagent le poids `envies.jouer` : « jouer ×3 »
                // dans un modificateur d'application ne distingue pas les
                // animations, et devoir régler chaque jeu séparément serait
                // du réglage pour rien.
                EntreeEnvie {
                    intention: Intention::Jouer(Jeu::TeteQuiTourne),
                    base: config.envies.jouer,
                    poses_requises: Jeu::TeteQuiTourne.poses_requises(),
                },
                EntreeEnvie {
                    intention: Intention::Jouer(Jeu::JambesQuiBalancent),
                    base: config.envies.jouer,
                    poses_requises: Jeu::JambesQuiBalancent.poses_requises(),
                },
```

- [ ] **Step 7 : Le poids, le modificateur, et le biais**

Dans `src/config.rs`, `Envies` gagne `pub jouer: f32` avec le défaut `1.0` (spec §7.2), et `ModifsAppli` gagne `pub jouer: Option<f32>`.

Dans `src/signals.rs`, `Biais` gagne `pub jouer: f32` — `neutre()` le met à `1.0`, `pour()` gagne son bras :

```rust
            // `Jouer(_)` : le biais ne distingue pas les animations. « jouer
            // ×3 » vaut pour les deux, et les distinguer serait un réglage de
            // plus sans effet observable.
            Intention::Jouer(_) => self.jouer,
```

Et `biais_de` applique le modificateur d'application :

```rust
            if let Some(x) = m.jouer {
                b.jouer *= x;
            }
```

- [ ] **Step 8 : Le jeton de signature du mode simulation**

Dans `src/sim.rs`, le `match` qui construit la signature. **Ne pas mettre de
bras `_`** : c'est ce `match` exhaustif qui fera échouer la compilation quand
une intention s'ajoutera, et qui empêche une signature silencieusement
ambiguë.

```rust
                let jeton = match kind {
                    behavior::intention::Intention::Flaner => 1u64,
                    behavior::intention::Intention::SeReposer => 2u64,
                    // Deux jetons distincts : deux histoires qui jouent à des
                    // choses différentes doivent donner des signatures
                    // différentes.
                    behavior::intention::Intention::Jouer(
                        behavior::intention::Jeu::TeteQuiTourne,
                    ) => 3u64,
                    behavior::intention::Intention::Jouer(
                        behavior::intention::Jeu::JambesQuiBalancent,
                    ) => 4u64,
                };
```

- [ ] **Step 9 : Déclarer `spinHead` dans le manifeste de `blob`**

Dans `characters/blob/mascot.json`, à côté de `sitLookUp` :

```json
    "spinHead":     { "frames": [26, 15, 27, 16, 28, 17, 29, 11], "frameMs": 200 },
```

Frames et durée relevées dans `SitAndSpinHeadAction` (`Duration="5"` ticks × 40 ms). **Pas de `loop`** : l'animation revient à la frame 11, donc elle se relit proprement, et `jouer` la maintient posée le temps voulu.

- [ ] **Step 10 : Lancer la suite**

Run : `cargo test`
Attendu : **156 tests passent** (150 + 6).

> ⚠️ **Deux tests d'étape 1 vont légitimement bouger** :
> `la_table_par_defaut_privilegie_la_flanerie` et
> `un_multiplicateur_biaise_sans_commander` calculent des pourcentages sur
> une table de deux lignes. Avec quatre lignes, les proportions changent.
> **Recalculer les attentes, ne pas relâcher les tolérances** : flâner 5,
> reposer 1, jouer 1 + 1 sur un total de 8, donc 62,5 % / 12,5 % / 25 %.
> Et si le manifeste de test n'a pas les poses de jeu, la table reste à deux
> lignes et rien ne change — vérifier lequel des deux cas s'applique avant de
> toucher un chiffre.

- [ ] **Step 11 : Voir les deux animations, à l'œil**

```powershell
cargo build
.\target\debug\shimeji-desktop.exe
```

À vérifier :

1. Il finit par s'asseoir et **se tourner la tête** — 8 frames fluides, sans saut.
2. Il finit par s'asseoir et **balancer les jambes**.
3. **Le point à juger :** `sitDangle` descend de 16 px dans la barre des tâches. Est-ce que ça a l'air voulu, ou cassé ? C'est ce que fait Shimeji-ee ; si ça déplaît, la correction est **une valeur d'ancre dans `mascot.json`** — `[64, 128]` — et un rechargement à chaud, sans recompiler.

- [ ] **Step 12 : Commit**

```bash
git add src characters/blob/mascot.json config.exemple.json
git commit -m "feat(etape-2): Jouer(Jeu) — deux animations, deux lignes de table

Intention::Jouer(Jeu) donne enfin un contenu a la couche Jouer(action) de la
spec §7.1, et met en service sept frames jusqu'ici inutilisees (15, 16, 17,
27, 28, 29).

spinHead vient de SitAndSpinHeadAction : frames 26,15,27,16,28,17,29,11 a
200 ms (Duration=5 ticks x 40 ms). Releve dans actions.xml, pas devine — la
lecon de l'etape 1a, ou tout ce qui avait ete regle a l'oeil s'est revele
faux.

DEUX LIGNES DE TABLE ET NON UNE, et c'est le point de conception : poses_requises
est une liste ET, donc une intention Jouer unique exigerait les deux
animations, et un pack n'en ayant qu'une ne jouerait jamais. Separees, la
couverture partielle joue par animation. Un test le verifie avec un manifeste
qui n'a que spinHead.

Les deux lignes partagent le poids envies.jouer, et le biais ne distingue pas
les animations : « jouer x3 » dans un modificateur d'application vaut pour les
deux. Les distinguer serait un reglage de plus sans effet observable.

Le match des jetons de signature reste EXHAUSTIF, sans bras _ : c'est lui qui
fera echouer la compilation a la prochaine intention."
```

---

## Tâche 4 : Dormir — deux phases, et la continuité de pose

**Files:**
- Modify: `src/character/manifest.rs` (`POSE_SLEEP`)
- Modify: `src/behavior/intention.rs` (`PhaseRepos`, `se_reposer`)
- Modify: `src/config.rs` (`Reglages` transporte `seuil_sommeil`)

**Interfaces:**
- Consomme : `signals::Biais` via `Entrees` (Tâche 1), `config::SignauxReglages::seuil_sommeil`
- Produit :
  - `manifest::POSE_SLEEP = "sleep"`
  - `intention::PhaseRepos { Assis, Endormi }`
  - `EtatIntention::Repos { phase: PhaseRepos, jusqu_a: Duration }`

> **`se_reposer` a besoin du biais**, pour comparer au seuil de sommeil.
> `poursuivre` gagne donc `e: &Entrees` en paramètre. C'est le seul changement
> de signature de la tâche, et il touche `behavior/mod.rs` et les tests de
> `intention.rs`.

- [ ] **Step 1 : Écrire les tests**

Dans le module `tests` de `src/behavior/intention.rs` :

```rust
    /// Des `Entrees` inertes, avec un biais de repos choisi.
    ///
    /// Toutes les autres valeurs sont neutres : la souris est loin, aucun
    /// bouton n'est enfoncé. Un seul curseur pour tous les tests de sommeil.
    fn entrees_avec_biais_repos(x: f32) -> Entrees {
        Entrees {
            souris: Point::new(0.0, 0.0),
            echelle_affichage: 1.0,
            bouton_gauche: false,
            curseur_sur_le_personnage: false,
            biais: crate::signals::Biais {
                flaner: 1.0,
                se_reposer: x,
                jouer: 1.0,
            },
            utilisateur_actif: true,
        }
    }

    #[test]
    fn sans_signal_il_reste_assis_et_ne_s_affale_pas() {
        // **Une sieste ne s'improvise pas.** Sans signal, le biais vaut 1,
        // donc sous le seuil de 2 : il s'assoit et c'est tout. S'il
        // s'affalait de lui-même, « il dort quand tu t'en vas » perdrait tout
        // son sens — il dormirait tout le temps.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        let e = entrees_avec_biais_repos(1.0);

        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::SeReposer,
            Duration::ZERO,
        ));

        for i in 0..(14 * 60) {
            let t = Duration::from_secs_f32(i as f32 * DT);
            if poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng) != Issue::EnCours {
                break;
            }
            assert_eq!(ch.pose, POSE_SIT, "à {:.1} s il devrait être assis", t.as_secs_f32());
        }
    }

    #[test]
    fn avec_un_signal_il_s_assoit_puis_s_affale() {
        // La promesse de l'étape, dans l'ordre : 11 puis 21. C'est
        // l'ENCHAÎNEMENT qui dit « il dort », pas la frame 21 seule.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        let e = entrees_avec_biais_repos(8.0); // comme « inactif > 2 min »

        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::SeReposer,
            Duration::ZERO,
        ));

        // Première image : assis.
        poursuivre(&mut ch, &m, &e, &reglages(), Duration::ZERO, DT, &mut rng);
        assert_eq!(ch.pose, POSE_SIT);

        // Il finit par s'affaler, et en moins de 20 s (le délai d'abandon).
        let mut endormi_a = None;
        for i in 1..(20 * 60) {
            let t = Duration::from_secs_f32(i as f32 * DT);
            poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng);
            if ch.pose == POSE_SLEEP {
                endormi_a = Some(t);
                break;
            }
        }
        assert!(endormi_a.is_some(), "il ne s'est jamais affalé");
    }

    #[test]
    fn re_tirer_le_repos_pendant_le_sommeil_ne_le_fait_pas_se_rasseoir() {
        // **LE test de la continuité de pose**, et le seul qui justifie
        // qu'on n'ait PAS touché au délai d'abandon (décision n° 4).
        //
        // Un sommeil dure 20 à 60 s, le délai d'abandon coupe à 20 s, donc
        // l'intention est re-tirée. Sans continuité, on le verrait se
        // rasseoir puis se raffaler toutes les 20 secondes — un tic visible
        // à l'écran, absurde et inexplicable pour qui regarde.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        let e = entrees_avec_biais_repos(8.0);

        // On le met directement dans l'état « endormi ».
        ch.set_pose(POSE_SLEEP, Duration::ZERO);
        assert_eq!(ch.pose, POSE_SLEEP);

        // Une intention de repos FRAÎCHE, comme après un re-tirage.
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::SeReposer,
            Duration::from_secs(30),
        ));

        // La première image ne doit PAS le rasseoir.
        poursuivre(
            &mut ch,
            &m,
            &e,
            &reglages(),
            Duration::from_secs(30),
            DT,
            &mut rng,
        );
        assert_eq!(
            ch.pose, POSE_SLEEP,
            "il s'est rassis : la continuité de pose est cassée"
        );
    }

    #[test]
    fn sans_la_pose_sleep_il_reste_assis_au_lieu_d_echouer() {
        // Couverture partielle appliquée à une PHASE et non à une intention
        // (spec §8.6). Un pack sans pose de sommeil doit se reposer
        // normalement — assis — et non voir son repos échouer.
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "stand": { "frames": [1] }, "walk": { "frames": [2] },
                       "sit": { "frames": [11] } }
        }"#;
        let m = monde();
        let mut ch = Character::new(
            serde_json::from_str(json).unwrap(),
            Attachment::On {
                platform: m.platforms()[0].id,
                face: Face::Top,
                offset: 500.0,
            },
            Point::new(500.0, 1032.0),
        );
        let mut rng = XorShift32::seeded(1);
        let e = entrees_avec_biais_repos(8.0);

        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::SeReposer,
            Duration::ZERO,
        ));

        for i in 0..(19 * 60) {
            let t = Duration::from_secs_f32(i as f32 * DT);
            let issue = poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng);
            assert_ne!(issue, Issue::Echouee, "le repos ne doit pas échouer");
            if issue != Issue::EnCours {
                break;
            }
            assert_eq!(ch.pose, POSE_SIT);
        }
    }
```

- [ ] **Step 2 : Lancer les tests pour vérifier qu'ils échouent**

Run : `cargo test dormir OR cargo test repos`
Attendu : **échec de compilation** — `POSE_SLEEP` n'existe pas, et `poursuivre` ne prend pas d'`Entrees`.

- [ ] **Step 3 : La constante de pose**

Dans `src/character/manifest.rs` :

```rust
/// Affalé sur le ventre — notre pose de sommeil.
///
/// ⚠️ **Shimeji-ee n'a AUCUNE animation de sommeil**, et aucune frame du pack
/// n'a les yeux fermés : les yeux du blob sont deux points. La frame 21
/// (`Sprawl`) est le substitut le plus lisible, et elle est déclarée sous le
/// nom `sleep` **pour que le code ignore qu'il s'agit d'un substitut** — un
/// pack tiers avec une vraie pose de sommeil la déclarerait au même nom, et
/// rien ne changerait ici (spec §8.6).
///
/// Détail et sprites vérifiés : `docs/specs/2026-09-09-frames-shimeji.md`.
pub const POSE_SLEEP: &str = "sleep";
```

- [ ] **Step 4 : Le seuil dans les réglages**

Dans `src/config.rs`, `Reglages` gagne un champ, et `Reglages::depuis` le remplit depuis `config.signaux.seuil_sommeil` :

```rust
    /// À partir de quel biais de repos il s'affale au lieu de rester assis.
    pub seuil_sommeil: f32,
```

- [ ] **Step 5 : Les phases**

Dans `src/behavior/intention.rs` :

```rust
/// Où en est un repos.
///
/// Deux phases et non deux intentions : « dormir » n'est pas un choix
/// distinct de « se reposer », c'est **la suite** de se reposer quand un
/// signal pousse dans ce sens. En faire deux lignes de table demanderait au
/// tirage de savoir qu'on est déjà assis, ce qui n'a rien à y faire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseRepos {
    Assis,
    Endormi,
}
```

```rust
    Repos {
        phase: PhaseRepos,
        jusqu_a: Duration,
    },
```

Et dans `ActiveIntention::nouvelle` :

```rust
            Intention::SeReposer => EtatIntention::Repos {
                phase: PhaseRepos::Assis,
                jusqu_a: Duration::ZERO,
            },
```

- [ ] **Step 6 : `se_reposer`, réécrite**

```rust
/// Se reposer : s'asseoir, et s'endormir si un signal y pousse.
///
/// **Le sommeil n'est pas une intention à part** : c'est la seconde phase du
/// repos. Voir `PhaseRepos`.
fn se_reposer(
    ch: &mut Character,
    ai: &mut ActiveIntention,
    e: &super::Entrees,
    reglages: &Reglages,
    maintenant: Duration,
    rng: &mut dyn Rng,
) -> Issue {
    let EtatIntention::Repos {
        mut phase,
        mut jusqu_a,
    } = ai.etat
    else {
        ch.intention = None;
        return Issue::Echouee;
    };

    // Défense en profondeur : le tirage ne devrait jamais proposer cette
    // intention à un personnage sans `sit` (desire.rs le filtre). Mais une
    // config bricolée pourrait y parvenir, et un personnage assis sur une
    // pose inexistante serait invisible — mieux vaut échouer.
    if !ch.manifest.has_pose(POSE_SIT) {
        ch.intention = None;
        return Issue::Echouee;
    }

    // ── La continuité de pose ───────────────────────────────────────────
    //
    // **C'est ce qui permet de ne PAS toucher au délai d'abandon**
    // (décision n° 4). Un sommeil dure 20 à 60 s, le délai coupe à 20 s, donc
    // l'intention est re-tirée — et comme un signal met ×8 sur le repos, elle
    // est presque toujours re-tirée en `SeReposer`.
    //
    // Sans cette reprise, chaque re-tirage repartirait en phase `Assis` et
    // l'on verrait le personnage se rasseoir puis se raffaler toutes les
    // 20 secondes. Avec elle, le re-tirage est **invisible**.
    if phase == PhaseRepos::Assis && ch.pose == POSE_SLEEP {
        phase = PhaseRepos::Endormi;
        // On repart sur une durée de sommeil fraîche : c'est bien un nouveau
        // repos, seulement il ne recommence pas par la position assise.
        jusqu_a = Duration::ZERO;
    }

    if jusqu_a == Duration::ZERO {
        // Première image de cette phase : on tire sa durée.
        let (min, max) = match phase {
            // Assis : la plage de l'étape 1a, inchangée.
            PhaseRepos::Assis => (4.0, 15.0),

            // Endormi : 20 à 60 s. Relevé dans `actions.xml`, action
            // `LieDown` — `Sprawl` pendant `${500+Math.random()*1000}` ticks
            // à 40 ms. La leçon de l'étape 1a : chercher la constante dans le
            // source plutôt que de l'inventer.
            PhaseRepos::Endormi => (20.0, 60.0),
        };
        jusqu_a = maintenant + Duration::from_secs_f32(rng.range(min, max));
        ai.etat = EtatIntention::Repos { phase, jusqu_a };
    } else if maintenant >= jusqu_a {
        // ── La phase est écoulée : s'endormir, ou terminer ──────────────
        //
        // La condition du sommeil, et **la seule ligne de tout le fichier
        // qui regarde un biais** : il faut qu'un signal ait au moins doublé
        // l'envie de repos (`seuilSommeil`, 2,0 par défaut). Sans signal le
        // biais vaut 1, donc il reste assis — **une sieste ne s'improvise
        // pas.**
        //
        // Noter la forme : on ne teste PAS « est-ce que l'utilisateur est
        // parti ». On teste un poids. C'est la décision n° 3 appliquée à la
        // lettre : le comportement ne sait pas ce qu'est l'inactivité.
        let veut_dormir = e.biais.pour(Intention::SeReposer) >= reglages.seuil_sommeil;

        if phase == PhaseRepos::Assis && veut_dormir && ch.manifest.has_pose(POSE_SLEEP) {
            phase = PhaseRepos::Endormi;
            jusqu_a = Duration::ZERO; // sera tirée à l'image suivante
            ai.etat = EtatIntention::Repos { phase, jusqu_a };
        } else {
            // Soit il n'a pas de raison de dormir, soit il n'a pas la pose
            // (couverture partielle appliquée à une PHASE), soit il vient de
            // finir sa nuit.
            ch.intention = None;
            return Issue::Finie;
        }
    }

    let pose = match phase {
        PhaseRepos::Assis => POSE_SIT,
        PhaseRepos::Endormi => POSE_SLEEP,
    };
    ch.set_pose(pose, maintenant);
    Issue::EnCours
}
```

- [ ] **Step 7 : Faire descendre les `Entrees` jusqu'à `poursuivre`**

`poursuivre` gagne `e: &super::Entrees` en troisième paramètre, et le passe à `se_reposer`. Dans `behavior/mod.rs`, l'appel devient :

```rust
    match intention::poursuivre(ch, world, e, reglages, maintenant, dt, rng) {
```

> **Astuce :** le compilateur liste tous les appels à corriger dans les tests.
> Ne pas les chercher à la main.

- [ ] **Step 8 : Lancer la suite**

Run : `cargo test`
Attendu : **160 tests passent** (156 + 4).

- [ ] **Step 9 : Le voir dormir, sans attendre deux minutes**

Le seuil d'inactivité est réglable : on le descend à 5 s pour l'observer tout de suite. C'est exactement à ça que sert la décision n° 5.

```powershell
# Dans config.json, à côté de l'exe :
#   { "signaux": { "inactiviteSecondes": 5 } }
cargo build
.\target\debug\shimeji-desktop.exe
```

> ⚠️ **Rien ne se passera encore** : la boucle passe `Biais::neutre()` jusqu'à
> la Tâche 6. Cette observation est donc **reportée au Step 9 de la Tâche 6** —
> ne pas conclure ici que le sommeil ne marche pas. Les quatre tests de cette
> tâche sont la vérification, et elle est complète.

- [ ] **Step 10 : Commit**

```bash
git add src
git commit -m "feat(etape-2): dormir — deux phases, et la continuite de pose

Il s'assoit (11) PUIS il s'affale (21). En sequence et pas en pose unique,
parce que c'est l'enchainement qui fait la lisibilite : ce n'est pas la frame
21 qui dit « il dort », c'est le passage de 11 a 21 apres un moment
d'immobilite.

La duree du sommeil est 20 a 60 s — relevee dans actions.xml, action LieDown
(Sprawl pendant 500+random*1000 ticks a 40 ms), pas inventee.

LE DELAI D'ABANDON DE 20 s RESTE UNIFORME (decision n° 4). Un sommeil de 60 s
est donc coupe et re-tire, et c'est la CONTINUITE DE POSE qui absorbe le
re-tirage : deja dans la pose sleep, l'intention repart en phase Endormi. Sans
elle, on verrait le personnage se rasseoir puis se raffaler toutes les 20 s —
un tic visible et inexplicable. Aucune des deux decisions n'est entamee, et un
test verifie exactement ce point.

Il ne s'affale que si biais.pour(SeReposer) >= seuilSommeil (2,0 par defaut).
Sans signal le biais vaut 1 : UNE SIESTE NE S'IMPROVISE PAS. Et la forme
compte — on ne teste pas « est-ce que l'utilisateur est parti », on teste un
POIDS. Le comportement ne sait pas ce qu'est l'inactivite, c'est la decision
n° 3 a la lettre.

Un pack sans pose sleep se repose assis, sans echouer : la couverture
partielle appliquee a une PHASE et non a une intention."
```

---

## Tâche 5 : Se réveiller — les signaux peuvent interrompre, jamais choisir

**Files:**
- Modify: `src/behavior/mod.rs` (l'interruption, dans `pas`)

**Interfaces:**
- Consomme : `Entrees::utilisateur_actif` (Tâche 1), `intention::PhaseRepos` (Tâche 4)
- Produit : rien de nouveau — c'est un bloc de règle dans `pas`

> **La frontière que cette tâche trace, et qu'il faut garder :**
>
> | Un signal peut… | |
> |---|---|
> | ✅ **arrêter** ce qu'il est en train de faire | l'interruption |
> | ❌ **choisir** ce qu'il fera ensuite | ce serait la décision n° 3 perdue |

- [ ] **Step 1 : Écrire les tests**

Dans le module `tests` de `src/behavior/mod.rs` (à créer s'il n'existe pas) :

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::manifest::{POSE_SIT, POSE_SLEEP};
    use crate::character::attach::Attachment;
    use crate::character::Character;
    use crate::geom::{Face, Point};
    use crate::rng::XorShift32;
    use crate::world::World;
    use std::time::Duration;

    const DT: f32 = 1.0 / 60.0;

    fn monde() -> World {
        World::from_screens(&crate::probe::fake::FakeProbe::un_ecran().screens())
    }

    /// Un `blob` complet, posé sur le sol.
    fn perso(m: &World) -> Character {
        let sol = m.platforms()[0];
        Character::new(
            Manifest::load(std::path::Path::new("../characters/blob"))
                .expect("le personnage de test doit être lisible"),
            Attachment::On {
                platform: sol.id,
                face: Face::Top,
                offset: 500.0,
            },
            sol.rect.point_on(Face::Top, 500.0),
        )
    }

    fn entrees(actif: bool, biais_repos: f32) -> Entrees {
        Entrees {
            souris: Point::new(0.0, 0.0),
            echelle_affichage: 1.0,
            bouton_gauche: false,
            curseur_sur_le_personnage: false,
            biais: crate::signals::Biais {
                flaner: 1.0,
                se_reposer: biais_repos,
                jouer: 1.0,
            },
            utilisateur_actif: actif,
        }
    }

    #[test]
    fn redevenir_actif_reveille_le_personnage_endormi() {
        let m = monde();
        let mut ch = perso(&m);
        let mut rng = XorShift32::seeded(1);
        let table = desire::TableEnvies::defaut();
        let r = crate::config::Reglages::depuis(&crate::config::Config::default());

        // Il dort.
        ch.set_pose(POSE_SLEEP, Duration::ZERO);
        ch.intention = Some(intention::ActiveIntention {
            kind: intention::Intention::SeReposer,
            depuis: Duration::ZERO,
            etat: intention::EtatIntention::Repos {
                phase: intention::PhaseRepos::Endormi,
                jusqu_a: Duration::from_secs(60),
            },
        });

        // L'utilisateur revient.
        let e = entrees(true, 1.0);
        pas(&mut ch, &m, &e, &table, &r, Duration::from_secs(10), DT, &mut rng);

        // L'intention de repos ne doit plus être là. Elle a pu être remplacée
        // dans la même image par un nouveau tirage — c'est le fonctionnement
        // voulu — donc on vérifie qu'il ne DORT plus, pas que l'intention est
        // vide.
        assert_ne!(
            ch.pose, POSE_SLEEP,
            "il dort encore alors que l'utilisateur est revenu"
        );
    }

    #[test]
    fn le_reveil_ne_choisit_pas_la_suite() {
        // **LE test de la frontière.** Le signal ARRÊTE le sommeil ; il ne
        // décide pas de ce qui vient après. Sur 200 réveils, on doit voir
        // PLUSIEURS suites différentes — sinon c'est un déclenchement
        // déguisé, et la décision n° 3 est perdue.
        let m = monde();
        let table = desire::TableEnvies::defaut();
        let r = crate::config::Reglages::depuis(&crate::config::Config::default());
        let mut rng = XorShift32::seeded(4);
        let e = entrees(true, 1.0);

        let mut suites = std::collections::BTreeSet::new();
        for i in 0..200 {
            let mut ch = perso(&m);
            ch.set_pose(POSE_SLEEP, Duration::ZERO);
            ch.intention = Some(intention::ActiveIntention {
                kind: intention::Intention::SeReposer,
                depuis: Duration::ZERO,
                etat: intention::EtatIntention::Repos {
                    phase: intention::PhaseRepos::Endormi,
                    jusqu_a: Duration::from_secs(60),
                },
            });

            let t = Duration::from_secs(10 + i);
            pas(&mut ch, &m, &e, &table, &r, t, DT, &mut rng);

            if let Some(ai) = ch.intention {
                suites.insert(ai.kind);
            }
        }

        assert!(
            suites.len() >= 2,
            "le réveil mène toujours à la même chose ({suites:?}) : c'est un \
             déclenchement, pas une interruption"
        );
    }

    #[test]
    fn etre_actif_n_empeche_pas_de_s_asseoir() {
        // **LE piège, et la raison d'être de la phase.** Une pause normale se
        // prend PENDANT qu'on travaille. Si « actif → termine le repos »
        // s'appliquait à la position assise, le personnage ne s'assiérait
        // plus jamais tant qu'on touche au clavier — c'est-à-dire au seul
        // moment où on le regarde.
        let m = monde();
        let mut ch = perso(&m);
        let mut rng = XorShift32::seeded(1);
        let table = desire::TableEnvies::defaut();
        let r = crate::config::Reglages::depuis(&crate::config::Config::default());

        ch.set_pose(POSE_SIT, Duration::ZERO);
        ch.intention = Some(intention::ActiveIntention {
            kind: intention::Intention::SeReposer,
            depuis: Duration::ZERO,
            etat: intention::EtatIntention::Repos {
                phase: intention::PhaseRepos::Assis,
                jusqu_a: Duration::from_secs(10),
            },
        });

        let e = entrees(true, 1.0);
        for i in 0..60 {
            let t = Duration::from_secs_f32(1.0 + i as f32 * DT);
            pas(&mut ch, &m, &e, &table, &r, t, DT, &mut rng);
        }

        assert_eq!(
            ch.pose, POSE_SIT,
            "il a été interrompu alors qu'il était seulement assis"
        );
    }

    #[test]
    fn il_continue_de_dormir_tant_que_personne_ne_revient() {
        let m = monde();
        let mut ch = perso(&m);
        let mut rng = XorShift32::seeded(1);
        let table = desire::TableEnvies::defaut();
        let r = crate::config::Reglages::depuis(&crate::config::Config::default());

        ch.set_pose(POSE_SLEEP, Duration::ZERO);
        ch.intention = Some(intention::ActiveIntention {
            kind: intention::Intention::SeReposer,
            depuis: Duration::ZERO,
            etat: intention::EtatIntention::Repos {
                phase: intention::PhaseRepos::Endormi,
                jusqu_a: Duration::from_secs(60),
            },
        });

        // Toujours parti, et un biais qui pousse au sommeil.
        let e = entrees(false, 8.0);
        for i in 0..(10 * 60) {
            let t = Duration::from_secs_f32(1.0 + i as f32 * DT);
            pas(&mut ch, &m, &e, &table, &r, t, DT, &mut rng);
        }

        assert_eq!(ch.pose, POSE_SLEEP, "il s'est réveillé tout seul");
    }
}
```

`use crate::character::manifest::Manifest;` en tête du module de tests.

- [ ] **Step 2 : Lancer les tests pour vérifier qu'ils échouent**

Run : `cargo test reveil`
Attendu : **`redevenir_actif_reveille_le_personnage_endormi` échoue** — il dort encore. Les deux autres peuvent déjà passer : c'est normal, ils vérifient qu'on ne casse rien.

- [ ] **Step 3 : L'interruption**

Dans `src/behavior/mod.rs`, dans `pas`, **entre** les réflexes et `poursuivre` :

```rust
    // ── L'interruption : un signal ARRÊTE, il ne CHOISIT pas ────────────
    //
    // Ajout à la décision n° 3, documenté dans le design de l'étape 2 §6.
    //
    // Le problème : si le réveil passait par le tirage, il dormirait jusqu'à
    // 20 s après le retour de l'utilisateur — le temps que l'intention
    // expire. Trop lent pour « il se réveille au retour ».
    //
    // La règle : redevenir actif **termine** le sommeil. La couche 3 re-tire
    // juste après, avec des poids redevenus normaux, et il part flâner OU se
    // rasseoir OU jouer. Le signal n'a pas choisi — c'est ce qui distingue
    // une interruption d'un déclenchement.
    //
    // ⚠️ **Depuis la phase `Endormi` SEULEMENT.** Une pause normale se prend
    // pendant que l'utilisateur travaille : appliquer la règle à la position
    // assise empêcherait le personnage de se reposer tant qu'on touche au
    // clavier, c'est-à-dire au seul moment où on le regarde. Le sommeil, lui,
    // n'est atteint que parce qu'un signal a poussé le biais au-dessus du
    // seuil — donc, en pratique, parce que l'utilisateur était parti.
    if e.utilisateur_actif {
        // `matches!` : on ne veut lire que la phase, sans démonter toute
        // l'intention ni la reconstruire.
        let dort = matches!(
            ch.intention,
            Some(intention::ActiveIntention {
                etat: intention::EtatIntention::Repos {
                    phase: intention::PhaseRepos::Endormi,
                    ..
                },
                ..
            })
        );
        if dort {
            // On efface l'intention et on NE POSE AUCUNE POSE : la couche 3,
            // juste en dessous, va tirer la suite et c'est elle qui décidera
            // de la pose. Poser `stand` ici serait précisément « choisir ».
            ch.intention = None;
        }
    }
```

- [ ] **Step 4 : Lancer la suite**

Run : `cargo test`
Attendu : **164 tests passent** (160 + 4).

- [ ] **Step 5 : Commit**

```bash
git add src/behavior/mod.rs
git commit -m "feat(etape-2): se reveiller — les signaux interrompent, ils ne choisissent pas

Ajout a la decision n° 3, et il merite d'etre nomme : un signal peut ARRETER
ce que fait le personnage, jamais CHOISIR ce qu'il fera ensuite.

Sans cette regle, le reveil passerait par le tirage et il dormirait jusqu'a
20 s apres le retour de l'utilisateur, le temps que l'intention expire. Avec
elle, il se reveille au battement suivant du 2 Hz — ~0,5 s — puis part flaner
OU se rasseoir OU jouer, selon le tirage. Un test verifie qu'on voit
PLUSIEURS suites sur 200 reveils : une seule suite signifierait un
declenchement deguise.

DEPUIS LA PHASE ENDORMI SEULEMENT, et c'est le piege de la tache. Une pause
normale se prend pendant qu'on travaille : appliquer la regle a la position
assise empecherait le personnage de se reposer tant qu'on touche au clavier,
c'est-a-dire au seul moment ou on le regarde. Un test le verrouille.

L'interruption n'impose AUCUNE pose. Poser stand ici serait precisement
« choisir » — c'est la couche 3, juste en dessous, qui decide."
```

---

## Tâche 6 : Brancher la boucle — 2 Hz, le verrouillage, et la mesure

**Files:**
- Modify: `src/main.rs` (le battement 2 Hz, `SHIMEJI_SIGNAUX`, le verrouillage)
- Modify: `CLAUDE.md` (la sixième variable, les nouvelles clés, la mesure)

**Interfaces:**
- Consomme : `SystemProbe::signaux` (Tâche 2), `signals::biais_de` (Tâche 1), `tray::basculer_visibilite` (étape 1b)
- Produit : le comportement observable de l'étape

- [ ] **Step 1 : Le battement à 2 Hz**

Dans `boucle`, à côté des deux autres périodes :

```rust
    const PERIODE_SIGNAUX: Duration = Duration::from_millis(500); // 2 Hz
```

Et l'état correspondant, à côté de `dernier_recensement` :

```rust
    let mut dernier_signal = Duration::ZERO;

    // Le biais courant, recalculé à 2 Hz et transporté à 60 Hz.
    //
    // Neutre au démarrage : la première demi-seconde, le personnage se
    // comporte comme à l'étape 1. Rien à corriger — attendre les signaux
    // avant de bouger serait une demi-seconde de figement au lancement.
    let mut biais = signals::Biais::neutre();
    let mut utilisateur_actif = true;

    // La session est-elle verrouillée ? Mémorisé pour ne basculer les
    // fenêtres que sur CHANGEMENT.
    let mut verrouille = false;

    // Diagnostic : `SHIMEJI_SIGNAUX=1`.
    let trace_signaux = std::env::var("SHIMEJI_SIGNAUX").is_ok();
```

Puis le bloc lui-même, **avant** le recensement à 8 Hz (l'ordre n'a pas d'importance fonctionnelle, mais mettre les horloges du plus lent au plus rapide se relit mieux) :

```rust
        // ── ~2 Hz : les signaux (spec §5.5, design étape 2 §9) ──────────
        //
        // Cinq appels système toutes les 500 ms. À comparer aux 60
        // `SetWindowPos` par seconde qui coûtent 11 points de CPU : c'est du
        // bruit. Mesuré quand même — voir `CLAUDE.md`.
        if maintenant.saturating_sub(dernier_signal) >= PERIODE_SIGNAUX {
            let s = sonde.signaux();

            // Le biais : de la donnée pure, calculée par une fonction pure.
            biais = signals::biais_de(&s, &config_courante);

            // « Actif » se dérive du MÊME seuil que le biais, pour qu'il soit
            // impossible d'être « actif » et « inactif » dans la même image.
            utilisateur_actif = s.inactivite
                < Duration::from_secs_f32(config_courante.signaux.inactivite_secondes);

            // ── Le verrouillage : le quatrième réflexe ──────────────────
            //
            // C'est un réflexe au sens du design (décision n° 5) : non
            // négociable, immédiat, on ne biaise pas un poids pour
            // disparaître d'un écran de verrouillage.
            //
            // Mais il ne s'implémente PAS dans `reflex.rs`, et c'est
            // délibéré : son effet porte sur la FENÊTRE, pas sur l'accroche
            // du personnage. `reflex.rs` ne connaît ni Tauri ni le tray, et
            // c'est ce qui le garde testable sans écran.
            if s.session_verrouillee != verrouille {
                verrouille = s.session_verrouillee;

                // On ne rend visible que si l'utilisateur n'avait pas
                // lui-même décoché « Afficher » : le déverrouillage ne doit
                // pas défaire son choix.
                let voulu = visibilite.load(std::sync::atomic::Ordering::Relaxed);
                tray::basculer_visibilite(&handle, voulu && !verrouille);

                println!(
                    "session {} — personnage {}",
                    if verrouille { "verrouillée" } else { "déverrouillée" },
                    if verrouille { "planqué" } else { "de retour" }
                );
            }

            if trace_signaux {
                println!(
                    "signaux : inactif {:.0} s · {} · {} h · batterie {} · verrouillé {} \
                     → flâner ×{:.2} reposer ×{:.2} jouer ×{:.2}",
                    s.inactivite.as_secs_f32(),
                    s.appli_active.as_deref().unwrap_or("-"),
                    s.heure,
                    match s.batterie.pourcent {
                        Some(p) => format!("{p} %"),
                        None => "-".to_string(),
                    },
                    s.session_verrouillee,
                    biais.flaner,
                    biais.se_reposer,
                    biais.jouer,
                );
            }

            dernier_signal = maintenant;
        }
```

> **`config_courante` et `reglages_signaux` n'existent pas encore.** La boucle
> reçoit `reglages: Reglages` mais pas la `Config` complète, dont
> `biais_de` a besoin (la table des applications). Deux options, et la
> première est la bonne :
>
> 1. **Passer la `Config` à `boucle`**, et la remplacer au rechargement à
>    chaud comme le reste. Une donnée de plus dans la signature, et
>    `Rechargement` gagne un champ `config`.
> 2. ~~Une variable globale~~ — écartée : c'est ce que la spec §10.2 interdit,
>    et ça rendrait `biais_de` intestable.
>
> Faire l'option 1 : `boucle(..., mut config_courante: config::Config, ...)`.
> Les seuils s'écrivent alors `config_courante.signaux.inactivite_secondes` —
> **un seul nom**, pas une copie locale qui pourrait se désynchroniser du
> rechargement à chaud.

- [ ] **Step 2 : Transporter le biais dans les `Entrees`**

Remplacer les deux valeurs neutres posées à la Tâche 1 :

```rust
            biais,
            utilisateur_actif,
```

- [ ] **Step 3 : Le verrouillage suspend la boucle**

Le test de visibilité de l'étape 1b devient :

```rust
        // Caché par l'utilisateur, OU session verrouillée : rien à dessiner.
        //
        // Le verrouillage emprunte exactement le chemin de « caché », qui est
        // déjà mesuré à 0,9 % (`CLAUDE.md`). C'est donc aussi la première des
        // pistes CPU restantes, et elle se referme ici.
        let visible = visibilite.load(std::sync::atomic::Ordering::Relaxed) && !verrouille;
```

- [ ] **Step 4 : Le rechargement à chaud doit aussi recharger la config**

Dans `src/rechargement.rs`, `Rechargement` gagne :

```rust
    /// La config complète, pour `signals::biais_de` — la table des
    /// applications en fait partie.
    ///
    /// Redondante avec `reglages` et `table`, qui en sont dérivés. On la
    /// transporte quand même plutôt que de reconstruire : `preparer` l'a déjà
    /// lue, et la relire dans la boucle serait une entrée-sortie à 8 Hz.
    pub config: crate::config::Config,
```

`preparer` la remplit (elle appelle déjà `config::charger()`), et la boucle l'applique à côté de `reglages` et `table` :

```rust
                    config_courante = r.config;
```

- [ ] **Step 5 : Compiler et lancer la suite**

Run : `cargo test`
Attendu : **164 tests**, tous verts. Aucun test nouveau à cette tâche : elle branche des pièces déjà testées. Ce qui la vérifie, ce sont les Steps 6 à 9.

- [ ] **Step 6 : Vérifier les signaux en marche**

```powershell
cargo build
$env:SHIMEJI_SIGNAUX = "1"
.\target\debug\shimeji-desktop.exe
```

Attendu, deux lignes par seconde :

```
signaux : inactif 0 s · Code.exe · 15 h · batterie - · verrouillé false → flâner ×1.00 reposer ×1.00 jouer ×1.00
```

Puis, en ne touchant plus à rien pendant deux minutes, la ligne doit devenir :

```
signaux : inactif 121 s · Code.exe · 15 h · batterie - · verrouillé false → flâner ×0.20 reposer ×8.00 jouer ×1.00
```

**C'est la vérification centrale de l'étape** : le biais change tout seul, et il change au bon moment.

- [ ] **Step 7 : Vérifier le verrouillage, et mesurer**

```powershell
# Lancer, puis dans une autre console :
$p = Get-Process -Name shimeji-desktop
$c = $p.CPU
rundll32.exe user32.dll,LockWorkStation
Start-Sleep -Seconds 60
# … deverrouiller, puis :
$p.Refresh()
"verrouille : $([math]::Round((($p.CPU - $c) / 60) * 100, 1)) % d'un coeur"
```

Attendu : **~0,9 %**, le même chiffre que `SHIMEJI_CACHE=1` — puisque c'est le même chemin. Et à l'écran, au déverrouillage : le personnage est de retour, éventuellement ailleurs (son comportement a continué de tourner).

> ⚠️ **Vérifier aussi le cas croisé** : décocher « Afficher » dans le tray,
> verrouiller, déverrouiller. Le personnage doit rester **caché** — le
> déverrouillage ne doit pas défaire le choix de l'utilisateur. C'est ce que
> fait le `voulu && !verrouille`.

- [ ] **Step 8 : Mesurer le CPU en marche normale**

Protocole de 60 s de `CLAUDE.md`, sur le build **release** :

```powershell
cargo build --release
Start-Process .\target\release\shimeji-desktop.exe
Start-Sleep -Seconds 5
$p = Get-Process -Name shimeji-desktop
$c = $p.CPU; Start-Sleep -Seconds 60; $p.Refresh()
"etape 2, release : $([math]::Round((($p.CPU - $c) / 60) * 100, 1)) %"
```

Attendu : **~12 %**, inchangé par rapport à l'étape 1. Cinq appels système par demi-seconde ne doivent pas se voir. **Si le chiffre monte de plus d'un point, chercher** — le suspect le plus probable est `appli_active`, qui ouvre un handle de processus deux fois par seconde.

- [ ] **Step 9 : Le voir dormir et se réveiller — la promesse de l'étape**

Descendre le seuil pour ne pas attendre deux minutes :

```powershell
# config.json, à côté de l'exe :
#   { "signaux": { "inactiviteSecondes": 8 } }
.\target\debug\shimeji-desktop.exe
```

À vérifier, dans l'ordre :

| | Attendu |
|---|---|
| 1 | On ne touche à rien : au bout de ~8 s il **s'assoit** (frame 11) |
| 2 | Puis, quelques secondes après, il **s'affale** (frame 21) |
| 3 | Il **reste** affalé — pas de va-et-vient assis/affalé toutes les 20 s. C'est la continuité de pose de la Tâche 4 ; si ça clignote, elle est cassée |
| 4 | On bouge la souris : il se **réveille en moins d'une seconde** |
| 5 | Et ce qu'il fait ensuite **change d'un réveil à l'autre** — parfois il marche, parfois il se rassoit. C'est l'interruption de la Tâche 5, et son absence de choix |

- [ ] **Step 10 : Mettre `CLAUDE.md` à jour**

Trois endroits :

1. Le tableau des variables de diagnostic gagne `SHIMEJI_SIGNAUX=1` — **la sixième**.
2. Le tableau de référence du CPU gagne la ligne « étape 2, release, en marche ».
3. La piste CPU n° 1 (« suspendre la boucle quand la session est verrouillée ») passe en **appliquée**, avec le chiffre mesuré.

- [ ] **Step 11 : Commit**

```bash
git add src CLAUDE.md
git commit -m "feat(etape-2): la troisieme horloge — 2 Hz, et le verrouillage

Le battement des signaux vit dans le thread de la boucle, comme le 8 Hz : un
simple compteur. Contrairement au rechargement a chaud de l'etape 1b, rien ne
traverse un thread, donc aucune synchronisation a inventer.

Cinq appels systeme toutes les 500 ms, et le biais est ensuite transporte a
60 Hz sans etre recalcule. C'est tout l'interet de la forme choisie a la
tache 1 : la recherche de l'application dans la table se fait 2 fois par
seconde, pas 60.

Le verrouillage de session est le quatrieme reflexe promis par la decision
n° 5 — non negociable, immediat. Mais il ne s'implemente PAS dans reflex.rs,
et c'est delibere : son effet porte sur la FENETRE, pas sur l'accroche du
personnage. reflex.rs ne connait ni Tauri ni le tray, et c'est ce qui le garde
testable sans ecran.

Il emprunte exactement le chemin de « cache », deja mesure a 0,9 % : c'est
donc aussi la premiere des pistes CPU restantes, et elle se referme ici.

Le deverrouillage ne defait pas le choix de l'utilisateur : voulu && !verrouille,
et non un rendu visible inconditionnel.

La boucle recoit desormais la Config complete — biais_de a besoin de la table
des applications. Pas de variable globale : c'est ce que la spec §10.2 interdit,
et ca rendrait biais_de intestable."
```

---

## Tâche 7 : La journée simulée — la preuve d'ensemble, sans écran

**Files:**
- Modify: `src/sim.rs` (la chronologie, et les chiffres de sommeil)
- Modify: `CLAUDE.md` (la commande, et ce qu'elle prouve)

**Interfaces:**
- Consomme : `signals::biais_de`, `probe::Signaux` (Tâches 1 et 2)
- Produit :
  - `sim::signaux_de_la_journee(minute: u32) -> Signaux`
  - `Resume::secondes_endormi: u64`, `Resume::reveils: u64`, `Resume::endormi_par_heure: [u32; 24]`

> **C'est la vérification d'ensemble de l'étape**, et elle ne demande ni
> écran, ni horloge réelle, ni humain : on déroule 24 h de comportement avec
> une chronologie d'activité scriptée, et l'on regarde **quand** il a dormi.

- [ ] **Step 1 : Écrire les tests**

Dans le module `tests` de `src/sim.rs` :

```rust
    #[test]
    fn la_chronologie_couvre_une_journee_entiere() {
        // L'heure doit avancer, faire le tour, et rester dans 0..24.
        let h = |min| signaux_de_la_journee(min).heure;
        assert_eq!(h(0), 9, "la journée commence à 9 h");
        assert_eq!(h(60), 10);
        assert_eq!(h(15 * 60), 0, "9 h + 15 h = minuit");
        for min in 0..(24 * 60) {
            assert!(signaux_de_la_journee(min).heure < 24);
        }
    }

    #[test]
    fn la_chronologie_alterne_presence_et_absence() {
        // Aux heures de travail il est là ; la nuit il est parti. Sans cette
        // alternance, la simulation ne prouverait rien : un biais constant ne
        // se distingue pas d'un biais absent.
        let inactif = |min| signaux_de_la_journee(min).inactivite;
        assert_eq!(inactif(30), std::time::Duration::ZERO, "10 h : il travaille");
        assert!(
            inactif(15 * 60) > std::time::Duration::from_secs(120),
            "minuit : il est parti depuis longtemps"
        );
    }

    #[test]
    fn une_journee_entiere_dort_au_bon_moment() {
        // **LE test de l'étape**, et il vérifie quatre choses d'un coup.
        //
        // Une seule simulation pour les quatre : dérouler 24 h fait
        // 5,2 millions d'images, soit une à trois secondes en debug. La
        // lancer quatre fois multiplierait par quatre le temps de la suite
        // entière, qui tient aujourd'hui en 0,43 s.
        //
        // > Si ce test dépasse ~10 s sur la machine, le marquer `#[ignore]`
        // > et s'appuyer sur `--sim 1440` (Step 8), qui est de toute façon
        // > l'artefact qu'on lit.
        let r = executer(24 * 60, 42, blob(), &defauts()).expect("la simulation doit aboutir");

        // ── 1. Il dort ──────────────────────────────────────────────────
        // 2 h, 3 h, 4 h : personne devant la machine.
        let nuit: u32 = (2..=4).map(|h| r.endormi_par_heure[h]).sum();
        assert!(nuit > 0, "il n'a pas dormi de la nuit");

        // ── 2. Il dort AU BON MOMENT ────────────────────────────────────
        // C'est la différence entre un signal branché et un signal qui
        // marche : un total de sommeil ne dirait rien, seule la ventilation
        // par heure le dit. 9 h-11 h, il est au clavier.
        let matin: u32 = (9..=11).map(|h| r.endormi_par_heure[h]).sum();
        assert!(
            nuit > matin * 5,
            "il dort autant le matin que la nuit : le signal ne mord pas              (matin {matin} s, nuit {nuit} s)"
        );

        // ── 3. Il se réveille ───────────────────────────────────────────
        // La chronologie compte cinq retours de l'utilisateur.
        assert!(r.reveils > 0, "il ne s'est jamais réveillé");

        // ── 4. LA MARGE SURVIT — le test de la décision n° 3 ────────────
        //
        // Même avec ×8 sur le repos pendant toute la nuit, il ne doit PAS
        // avoir dormi 100 % du temps. « Cette marge est le produit. » Si
        // elle disparaissait, le signal COMMANDERAIT au lieu de biaiser, et
        // aucun autre test ne s'en apercevrait.
        let nuit_complete: u32 = (0..6).map(|h| r.endormi_par_heure[h]).sum();
        let six_heures: u32 = 6 * 3600;
        assert!(
            nuit_complete < (six_heures as f32 * 0.95) as u32,
            "il a dormi {nuit_complete} s sur {six_heures} : la marge a disparu"
        );

        // ── Et la décision n° 4 tient toujours ──────────────────────────
        // Le sommeil est la plus longue immobilité du programme : c'est ici
        // qu'un blocage se verrait.
        assert!(
            r.blocage_max < crate::behavior::intention::DELAI_ABANDON,
            "blocage de {:?}, au-delà du délai d'abandon",
            r.blocage_max
        );
    }
```

- [ ] **Step 2 : Lancer les tests pour vérifier qu'ils échouent**

Run : `cargo test sim`
Attendu : **échec de compilation** — `signaux_de_la_journee` et les trois champs de `Resume` n'existent pas.

- [ ] **Step 3 : La chronologie**

Dans `src/sim.rs` :

```rust
/// La journée scriptée que la simulation joue.
///
/// **Pourquoi une chronologie et pas des signaux constants :** un biais
/// constant ne se distingue pas d'un biais absent. Ce qu'on veut prouver,
/// c'est que le personnage dort **au bon moment** — donc il faut des moments.
///
/// Déterministe et sans aléatoire : à `minute` égale, mêmes signaux. C'est ce
/// qui garde la simulation comparable d'une exécution à l'autre, comme la
/// graine du générateur.
///
/// La journée commence à **9 h** — l'heure à laquelle on lance son ordinateur,
/// donc celle qui rend la trace lisible.
pub fn signaux_de_la_journee(minute: u32) -> crate::probe::Signaux {
    const DEBUT: u32 = 9;
    let heure = ((DEBUT + minute / 60) % 24) as u8;

    // Présent : 9 h-12 h, 14 h-18 h, 20 h-22 h. Absent le reste du temps —
    // pause déjeuner, soirée, nuit.
    let present = matches!(heure, 9..=11 | 14..=17 | 20..=21);

    // Combien de temps s'est-il écoulé depuis la dernière minute de présence ?
    //
    // On le calcule en remontant plutôt qu'en gardant un état : la fonction
    // reste pure, donc appelable pour n'importe quelle minute dans n'importe
    // quel ordre — ce qu'un test fait justement.
    let inactivite = if present {
        std::time::Duration::ZERO
    } else {
        let mut recul = 1u32;
        while recul <= minute {
            let h = ((DEBUT + (minute - recul) / 60) % 24) as u8;
            if matches!(h, 9..=11 | 14..=17 | 20..=21) {
                break;
            }
            recul += 1;
        }
        std::time::Duration::from_secs(recul as u64 * 60)
    };

    crate::probe::Signaux {
        inactivite,
        // Une application au premier plan pendant les heures de travail : la
        // table des modificateurs est vide par défaut, donc ça ne change rien
        // — mais un utilisateur qui ajoute une ligne le verra dans la trace.
        appli_active: if present {
            Some("Code.exe".to_string())
        } else {
            None
        },
        heure,
        // Sur secteur : la batterie a ses propres tests unitaires, et la
        // faire varier ici brouillerait la lecture du signal d'inactivité.
        batterie: crate::probe::Batterie {
            pourcent: None,
            sur_secteur: true,
        },
        // Le verrouillage n'est pas un signal de comportement : il porte sur
        // la fenêtre (Tâche 6), que la simulation n'a pas.
        session_verrouillee: false,
    }
}
```

- [ ] **Step 4 : Les trois chiffres de `Resume`**

```rust
    /// Combien de secondes il a passées dans la pose de sommeil.
    pub secondes_endormi: u64,

    /// Combien de fois il est passé de la pose de sommeil à autre chose.
    pub reveils: u64,

    /// Les secondes de sommeil, ventilées par heure locale.
    ///
    /// **C'est le chiffre qui prouve l'étape** : il ne suffit pas qu'il
    /// dorme, il faut qu'il dorme quand l'utilisateur n'est pas là. Un total
    /// ne le dirait pas.
    pub endormi_par_heure: [u32; 24],
```

- [ ] **Step 5 : Brancher la chronologie dans la boucle de simulation**

Remplacer les `Entrees` constantes par un recalcul à 2 Hz, comme la vraie boucle :

```rust
    // Les mêmes 2 Hz que la boucle réelle (spec §5.5) : la simulation doit
    // exercer le comportement au même rythme, sinon elle vérifierait autre
    // chose.
    const IMAGES_PAR_SIGNAL: u64 = 30; // 60 Hz / 2 Hz

    let mut biais = crate::signals::Biais::neutre();
    let mut utilisateur_actif = true;
    let mut heure_courante: u8 = 9;
```

Dans la boucle, avant `behavior::pas` :

```rust
        if i % IMAGES_PAR_SIGNAL == 0 {
            let minute = (i / (60 * 60)) as u32;
            let s = signaux_de_la_journee(minute);
            biais = crate::signals::biais_de(&s, config);
            utilisateur_actif =
                s.inactivite < Duration::from_secs_f32(config.signaux.inactivite_secondes);
            heure_courante = s.heure;
        }

        let entrees = Entrees {
            souris: Point::new(0.0, 0.0),
            echelle_affichage: 1.0,
            bouton_gauche: false,
            curseur_sur_le_personnage: false,
            biais,
            utilisateur_actif,
        };
```

Et la comptabilité du sommeil, après `behavior::pas` :

```rust
        // ── Le sommeil, compté par heure ────────────────────────────────
        let dort = ch.pose == crate::character::manifest::POSE_SLEEP;
        if dort {
            // Une image vaut DT seconde ; on compte en images et on convertit
            // à la fin pour ne pas accumuler d'erreur de flottant sur
            // 5 millions d'images.
            images_endormi += 1;
            images_endormi_par_heure[heure_courante as usize] += 1;
        }
        if dormait && !dort {
            resume.reveils += 1;
        }
        dormait = dort;
```

Avec, avant la boucle :

```rust
    let mut images_endormi: u64 = 0;
    let mut images_endormi_par_heure = [0u64; 24];
    let mut dormait = false;
```

Et après la boucle :

```rust
    // Images → secondes, une seule fois.
    resume.secondes_endormi = (images_endormi as f32 * DT) as u64;
    for h in 0..24 {
        resume.endormi_par_heure[h] = (images_endormi_par_heure[h] as f32 * DT) as u32;
    }
```

- [ ] **Step 6 : Lancer les tests**

Run : `cargo test sim`
Attendu : **167 tests passent** (164 + 3).

- [ ] **Step 7 : Imprimer l'histogramme**

Là où `--sim` imprime son résumé, dans `main.rs` :

```rust
    println!("endormi           : {} s au total", r.secondes_endormi);
    println!("réveils           : {}", r.reveils);
    println!("sommeil par heure :");
    for h in 0..24 {
        let s = r.endormi_par_heure[h];
        if s == 0 {
            continue;
        }
        // Une barre par tranche de 2 minutes, pour que 24 lignes tiennent
        // dans un terminal.
        let barre = "#".repeat((s / 120).min(60) as usize);
        println!("  {h:02} h {s:5} s {barre}");
    }
```

- [ ] **Step 8 : Dérouler une journée**

```powershell
cargo run -- --sim 1440
```

Attendu — et **c'est la preuve de l'étape** :

| Ligne | Attendu |
|---|---|
| `blocage_max` | **sous 20 s**, comme à l'étape 1 (décision n° 4) |
| `poses_vues` | contient `sleep`, `sit`, `spinHead`, `sitDangle` |
| `réveils` | plusieurs — la chronologie compte cinq retours |
| l'histogramme | **des barres la nuit et à midi, presque rien de 9 h à 12 h** |

- [ ] **Step 9 : Vérifier la reproductibilité**

```powershell
cargo run -- --sim 1440 | Select-String "signature"
cargo run -- --sim 1440 | Select-String "signature"
```

Attendu : **la même signature**. Les signaux venant d'une fonction pure de la minute, ajouter la chronologie ne doit pas avoir introduit d'indéterminisme.

- [ ] **Step 10 : Commit**

```bash
git add src CLAUDE.md
git commit -m "test(etape-2): une journee de 24 h simulee, sans ecran ni horloge

C'est la verification d'ensemble de l'etape, et elle ne demande ni ecran, ni
horloge reelle, ni humain : on deroule 24 h de comportement avec une
chronologie d'activite scriptee, et on regarde QUAND il a dormi.

Une chronologie et pas des signaux constants, parce qu'un biais constant ne se
distingue pas d'un biais absent. Ce qu'on prouve, c'est qu'il dort AU BON
MOMENT — donc il faut des moments. La fonction est PURE de la minute, donc
appelable dans n'importe quel ordre et reproductible : la signature de deux
executions doit rester identique.

Resume gagne endormi_par_heure, et c'est ce chiffre qui prouve l'etape : un
total ne dirait pas s'il dort quand l'utilisateur est la. Le test compare le
matin (9-11 h, il travaille) a la nuit (2-4 h, personne) et exige un facteur 5.

Et un test verifie que LA MARGE SURVIT : meme avec x8 sur le repos pendant
toute la nuit, il n'a pas dormi 100 % du temps. « Cette marge est le produit »
— si elle disparaissait, le signal commanderait au lieu de biaiser, et la
decision n° 3 serait perdue sans qu'aucun autre test ne s'en apercoive.

La simulation recalcule les signaux au MEME rythme que la boucle reelle
(2 Hz), sinon elle verifierait autre chose que ce qui tourne."
```

---

## Auto-revue

### Couverture de la spec de l'étape 2

| Section de la spec | Tâche |
|---|---|
| §1 périmètre — pas de fenêtres, pas d'`AllerÀ` | contraintes globales, et aucune tâche ne les touche |
| §2 `Manger` reporté | **rien à implémenter** ; la promesse est retirée des docs en Tâche 6 Step 10 |
| §3 `Signaux` / `Batterie`, instantané | Tâche 1 (types), Tâche 2 (trait + win32) |
| §3 les cinq APIs et leurs trois pièges | Tâche 2 Step 7 |
| §4 `signals.rs`, fonction pure | Tâche 1 |
| §4 la table des modificateurs | Tâche 1 (inactivité, soir, batterie, applications), Tâche 3 (`jouer`) |
| §4 le branchement en une ligne | Tâche 1 Step 7 |
| §5 dormir, deux phases, 20-60 s | Tâche 4 |
| §5 le délai d'abandon absorbé par la continuité de pose | Tâche 4 Step 6, et son test |
| §6 interrompre, jamais choisir | Tâche 5 |
| §6 depuis `Endormi` seulement | Tâche 5, test `etre_actif_n_empeche_pas_de_s_asseoir` |
| §7 `Jouer(Jeu)`, deux lignes | Tâche 3 |
| §7 l'ancre de `sitDangle`, à juger à l'œil | Tâche 3 Step 11 |
| §8 verrouillage : réflexe **et** optimisation CPU | Tâche 6 Steps 3 et 7 |
| §9 la troisième horloge | Tâche 6 Step 1 |
| §10 `FakeProbe` réglable | Tâche 2 Step 4 |
| §10 `--sim` d'une journée | Tâche 7 |
| §10 `SHIMEJI_SIGNAUX` | Tâche 6 Step 1 |
| §10 les deux choses qui restent à l'œil | Tâche 3 Step 11, Tâche 6 Step 9 |
| §11 les trois dérives à éviter | rappelées dans les contraintes globales |

**Aucun trou.** Deux points de la spec n'ont pas de code et c'est correct :
`Manger` (reporté) et le périmètre (une contrainte, pas une tâche).

### Cohérence des types entre tâches

- `Biais` gagne son champ `jouer` en **Tâche 3**, pas en Tâche 1 : `Intention::Jouer`
  n'existe pas avant. Les tests de la Tâche 1 construisent donc un `Biais` à
  **deux** champs, et ceux des Tâches 4 et 5 à **trois**. C'est voulu et
  cohérent avec l'ordre des tâches.
- `poursuivre` gagne `e: &Entrees` en **Tâche 4** seulement. Les appels des
  Tâches 1 à 3 gardent l'ancienne signature — l'exécutant qui lit la Tâche 3
  hors ordre doit le savoir, d'où cette note.
- `Entrees` gagne ses deux champs en **Tâche 1**, donc tous les tests
  ultérieurs les fournissent. `entrees_avec_biais_repos` (Tâche 4) et
  `entrees` (Tâche 5) sont deux fabriques distinctes, dans deux modules de
  tests distincts : ce n'est pas une duplication à factoriser, chacune vit à
  côté de ses tests.
- `Reglages::seuil_sommeil` (Tâche 4) est dérivé de
  `Config::signaux::seuil_sommeil` (Tâche 1). Le champ de config existe donc
  **avant** son usage, ce qui laisse la Tâche 1 sans code mort visible : il est
  lu par `Reglages::depuis` dès la Tâche 4.

### Ce qui reste incertain

**Un seul point, et il est isolé** : la lecture de l'union `WTSINFOEXW`
(Tâche 2 Step 7). C'est le seul endroit de l'étape où l'on déréférence un
pointeur rendu par Windows, et la garde `info.Level == 1` est indispensable —
sans elle, on lirait de la mémoire non initialisée. Le reste des `unsafe` sont
des appels à des fonctions dont la signature a été vérifiée.

Le risque résiduel est **d'usage, pas de correction** : on ne sait pas encore
si un seuil d'inactivité de 2 minutes est agréable. C'est réglable sans
recompiler, ce qui est exactement la raison d'être de la décision n° 5.
