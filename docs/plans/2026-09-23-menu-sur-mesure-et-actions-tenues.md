# Le menu sur mesure et les actions tenues — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal :** qu'une action choisie au clic droit dure tant qu'on ne l'arrête pas, que le menu offre une section « Tout le monde », et qu'il s'affiche dans une fenêtre webview au style de l'extension Shimeji.

**Architecture :** chaque `Character` porte une `tenue: Option<Tenue>`. Quand son intention finit, `behavior::pas` relance celle de la tenue au lieu de tirer au sort. Le menu reste décrit par la fonction pure `menu_perso::lignes`, enrichie de coches et d'un titre de section. Son affichage passe d'un menu Win32 natif (`menu_natif.rs`, retiré) à une fenêtre webview créée une fois au démarrage (`menu_fenetre.rs` + `ui/menu.*`).

**Tech Stack :** Rust, Tauri 2.11.5, crate `windows` 0.61, HTML/CSS/JS sans framework ni bundler.

**Spec :** `docs/specs/2026-09-23-menu-sur-mesure-et-actions-tenues-design.md`

## Global Constraints

- Code **abondamment commenté en français**, le *pourquoi* plutôt que le *quoi*, et les constructions Rust non évidentes expliquées (CLAUDE.md, « Conventions de code »). L'auteur apprend Rust.
- **Ne jamais éditer une source par PowerShell `Get-Content`/`Set-Content`** : outil d'édition, ou Python en UTF-8 explicite.
- Compiler **depuis PowerShell**, dans `src-tauri` : `cargo test --quiet`, et `cargo build 2>&1 | Select-Object -Last 40`.
- `cargo test` ne reconstruit pas l'exe : `cargo build` avant de lancer l'application.
- `windows = "0.61"` épinglé ; pas de Node, pas de bundler, pas de framework front.
- **Un seul `on_menu_event`** dans tout le programme (celui de `tray.rs`). La fenêtre du menu n'en ajoute aucun : elle appelle des commandes Tauri, qui repassent par `actions::executer`.
- Toute action jouable est **une ligne de la table `ENVIES`** de `menu_perso.rs`.
- Tout ce qui demande un clic a un **équivalent scriptable** (variable `SHIMEJI_…`).
- Aléatoire des tests : **semer une fois**, laisser l'état avancer (CLAUDE.md, « Semer l'aléatoire une seule fois »).

## Avant de commencer

L'arbre de travail contient des changements **non commités** d'une session précédente : menu natif, fenêtres à l'écran entier, « Cacher ce personnage ». Il contient aussi ceux d'une **autre session**, « Tout le monde au mur ». Ce plan touche les mêmes fichiers. **L'auteur les commite avant la tâche 1**, sinon chaque commit de ce plan embarquerait le travail des autres. Vérifier `git status` : seuls des fichiers hors de ce plan peuvent rester modifiés.

## Review Focus

1. **Un menu ouvert près du bord bas ou droit d'un écran** : il doit s'ouvrir vers le haut ou vers la gauche, entièrement visible. Test `le_menu_se_replie_dans_l_ecran` (tâche 5).
2. **Un menu sur l'écran du portable à 125 %** : il doit avoir la bonne taille, ni coupé ni flottant dans du vide. Test `la_taille_suit_l_echelle_de_l_ecran` (tâche 5).
3. **Windows refuse le premier plan au menu**, et `blur` ne se déclenche jamais : un clic ailleurs doit quand même le fermer. Test `un_clic_hors_du_menu_est_detecte` (tâche 5) et fermeture par la boucle.
4. **Un pack rechargé sans ses poses pendant qu'une action est tenue** : l'action est effacée, pas forcée sur une image absente. Test `une_tenue_devenue_injouable_est_effacee` (tâche 2).
5. **« Tout le monde › S'asseoir » alors qu'un seul personnage est déjà assis** : tout le monde s'assoit, et un second clic relève tout le monde. Tests `resoudre_pour_tous_*` (tâche 4).

---

### Task 1 : La tenue, le modèle et ce qui l'efface à la saisie

**Files :**
- Create : `src-tauri/src/behavior/tenue.rs`
- Create : `src-tauri/src/behavior/tenue_tests.rs`
- Modify : `src-tauri/src/behavior/mod.rs` (déclarer le module)
- Modify : `src-tauri/src/character/mod.rs` (champ `tenue` sur `Character`, et `Character::new`)
- Modify : `src-tauri/src/behavior/reflex.rs:225` (la saisie efface la tenue)
- Test : `src-tauri/src/behavior/tests.rs`

**Interfaces :**
- Produit : `behavior::tenue::Tenue { Asseoir, BalancerLesJambes, Flaner, Grimper, ResterAccroche }` (`Debug, Clone, Copy, PartialEq, Eq`) ; `Tenue::intention(self) -> Intention` ; `Tenue::au_sol(self) -> bool` ; `tenue::intention_pour(t: Tenue, e: &Entrees, reglages: &Reglages, table: &TableEnvies, manifeste: &Manifest, maintenant: Duration) -> ActiveIntention` ; `tenue::sert_une_tenue_au_mur(ch: &Character) -> bool` ; champ `Character::tenue: Option<Tenue>`.

- [ ] **Step 1 : Écrire les tests du module**

Créer `src-tauri/src/behavior/tenue_tests.rs` :

```rust
//! Les tests de `tenue` — la correspondance tenue → intention, et la
//! substitution par l'assoupissement (spec §2.3).

use super::*;
use crate::behavior::intention::{EtatIntention, PhaseGrimpe};
use crate::behavior::Entrees;
use crate::geom::Point;

fn reglages() -> Reglages {
    Reglages::depuis(&crate::config::Config::default())
}

fn blob() -> Manifest {
    Manifest::load(std::path::Path::new("../characters/blob"))
        .expect("le personnage de test doit être lisible")
}

/// Des entrées où seuls la présence de l'utilisateur et le poids du repos
/// varient — les deux choses qui décident de l'assoupissement.
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
            grimper: 1.0,
        },
        utilisateur_actif: actif,
        commande: None,
    }
}

#[test]
fn chaque_tenue_donne_son_intention() {
    assert_eq!(Tenue::Asseoir.intention(), Intention::SeReposer);
    assert_eq!(
        Tenue::BalancerLesJambes.intention(),
        Intention::Jouer(Jeu::JambesQuiBalancent)
    );
    assert_eq!(Tenue::Flaner.intention(), Intention::Flaner);
    assert_eq!(Tenue::Grimper.intention(), Intention::Grimper);
    // Rester accroché EST une escalade, arrêtée : la règle de sécurité du
    // monde vertical exempte précisément `Grimper`.
    assert_eq!(Tenue::ResterAccroche.intention(), Intention::Grimper);
}

#[test]
fn seules_les_tenues_de_sol_sont_au_sol() {
    assert!(Tenue::Asseoir.au_sol());
    assert!(Tenue::BalancerLesJambes.au_sol());
    assert!(Tenue::Flaner.au_sol());
    assert!(!Tenue::Grimper.au_sol());
    assert!(!Tenue::ResterAccroche.au_sol());
}

#[test]
fn utilisateur_present_chaque_tenue_joue_la_sienne() {
    let (r, t, m) = (reglages(), TableEnvies::defaut(), blob());
    let e = entrees(true, 1.0);
    for tenue in [Tenue::Asseoir, Tenue::BalancerLesJambes, Tenue::Flaner, Tenue::Grimper] {
        let ai = intention_pour(tenue, &e, &r, &t, &m, Duration::from_secs(1));
        assert_eq!(ai.kind, tenue.intention(), "{tenue:?}");
    }
}

#[test]
fn utilisateur_parti_une_tenue_de_sol_s_assoupit() {
    // ×8 : ce que produit « inactif depuis 2 min » (décision n° 3).
    let (r, t, m) = (reglages(), TableEnvies::defaut(), blob());
    let e = entrees(false, 8.0);
    for tenue in [Tenue::Asseoir, Tenue::BalancerLesJambes, Tenue::Flaner] {
        let ai = intention_pour(tenue, &e, &r, &t, &m, Duration::from_secs(1));
        assert_eq!(ai.kind, Intention::SeReposer, "{tenue:?}");
    }
}

#[test]
fn au_mur_l_absence_ne_change_rien() {
    // Décision de l'auteur : au mur, rien n'interrompt une action tenue.
    let (r, t, m) = (reglages(), TableEnvies::defaut(), blob());
    let e = entrees(false, 8.0);
    let ai = intention_pour(Tenue::Grimper, &e, &r, &t, &m, Duration::from_secs(1));
    assert_eq!(ai.kind, Intention::Grimper);
    let ai = intention_pour(Tenue::ResterAccroche, &e, &r, &t, &m, Duration::from_secs(1));
    assert!(matches!(
        ai.etat,
        EtatIntention::Grimpe { phase: PhaseGrimpe::Accroche, .. }
    ));
}
```

- [ ] **Step 2 : Écrire le test de la saisie**

À la fin de `src-tauri/src/behavior/tests.rs` :

```rust
/// Attraper un personnage efface son action tenue (spec §2.2) : le lâcher
/// le fait tomber, puis il reprend sa vie normale.
#[test]
fn l_attraper_efface_son_action_tenue() {
    let m = monde();
    let mut ch = perso(&m);
    ch.tenue = Some(tenue::Tenue::Asseoir);
    let table = desire::TableEnvies::defaut();
    let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
    let mut rng = XorShift32::seeded(7);

    let mut e = entrees(true, 1.0);
    e.bouton_gauche = true;
    e.curseur_sur_le_personnage = true;

    pas(&mut ch, &m, &e, &table, &reglages, Duration::from_secs(1), DT, &mut rng);

    assert_eq!(ch.attachment, Attachment::Dragged);
    assert_eq!(ch.tenue, None);
}
```

- [ ] **Step 3 : Vérifier que ça ne compile pas**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 20`
Expected : erreurs `unresolved import` / `no field tenue`.

- [ ] **Step 4 : Écrire le module**

Créer `src-tauri/src/behavior/tenue.rs` :

```rust
//! L'action TENUE d'un personnage : un ordre du menu qui dure tant qu'on ne
//! l'arrête pas (spec « menu sur mesure et actions tenues » §2).
//!
//! Responsabilité unique : dire quelle intention sert une tenue, et quand
//! l'assoupissement la remplace. Fonctions pures — ni Tauri, ni écran.

use super::desire::TableEnvies;
use super::intention::{ActiveIntention, Intention, Jeu};
use super::Entrees;
use crate::character::manifest::Manifest;
use crate::character::Character;
use crate::config::Reglages;
use std::time::Duration;

/// Ce qu'un personnage fait tant qu'on ne l'arrête pas.
///
/// **Aucune coche n'est stockée ailleurs** : le menu la DÉDUIT de ce champ
/// (spec §2.1), la même règle que l'interrupteur de la bibliothèque — une
/// seule vérité, rien à tenir d'accord.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tenue {
    Asseoir,
    BalancerLesJambes,
    Flaner,
    /// Il vit sur les murs et le plafond, sans jamais revenir au sol de
    /// lui-même (spec §2.4).
    Grimper,
    /// Immobile sur sa paroi.
    ResterAccroche,
}

impl Tenue {
    /// L'intention qui sert cette tenue.
    ///
    /// `ResterAccroche` rend `Grimper` : c'est une escalade arrêtée, et la
    /// règle de sécurité du monde vertical (`behavior::pas`) n'exempte que
    /// `Grimper` — une autre intention le ferait tomber.
    pub fn intention(self) -> Intention {
        match self {
            Tenue::Asseoir => Intention::SeReposer,
            Tenue::BalancerLesJambes => Intention::Jouer(Jeu::JambesQuiBalancent),
            Tenue::Flaner => Intention::Flaner,
            Tenue::Grimper | Tenue::ResterAccroche => Intention::Grimper,
        }
    }

    /// Vrai pour une tenue qui se joue au sol. C'est elle, et elle seule,
    /// que l'inactivité fait s'assoupir (spec §2.3).
    pub fn au_sol(self) -> bool {
        matches!(self, Tenue::Asseoir | Tenue::BalancerLesJambes | Tenue::Flaner)
    }
}

/// L'intention à poser pour servir `t`, maintenant.
///
/// # L'assoupissement (spec §2.3)
///
/// Une tenue de sol, quand l'utilisateur est parti, pose `SeReposer` à la
/// place : il s'assoit, puis `se_reposer` l'endort par sa propre règle — la
/// MÊME condition que le sommeil ordinaire, recopiée ici sans rien y
/// ajouter. À son retour, l'interruption de `behavior::pas` le réveille, et
/// la tenue relance sa propre intention. Elle ne s'est jamais décochée.
///
/// Le test porte sur un POIDS et sur un FAIT, exactement comme
/// `se_reposer` : il n'est pas un déclenchement (décision n° 3).
///
/// `table.jouable(…, SeReposer)` : un pack sans pose assise ne s'assoupit
/// pas, sans quoi `se_reposer` échouerait à chaque image, et la tenue
/// relancerait l'échec 60 fois par seconde.
pub fn intention_pour(
    t: Tenue,
    e: &Entrees,
    reglages: &Reglages,
    table: &TableEnvies,
    manifeste: &Manifest,
    maintenant: Duration,
) -> ActiveIntention {
    let assoupi = t.au_sol()
        && !e.utilisateur_actif
        && e.biais.pour(Intention::SeReposer) >= reglages.seuil_sommeil
        && table.jouable(manifeste, Intention::SeReposer);
    if assoupi {
        return ActiveIntention::nouvelle(Intention::SeReposer, maintenant);
    }

    match t {
        // Un ORDRE : il court jusqu'au mur, et sur un mur il reprend
        // l'escalade là où il est (phase `Choisir`).
        Tenue::Grimper => ActiveIntention::grimper_sur_ordre(maintenant),
        // L'intention qu'un lancer contre une paroi installe déjà.
        Tenue::ResterAccroche => ActiveIntention::accroche(maintenant),
        autre => ActiveIntention::nouvelle(autre.intention(), maintenant),
    }
}

/// Son intention en cours sert-elle une tenue AU MUR ?
///
/// C'est ce qui l'exempte du délai d'abandon (`intention::poursuivre`) : au
/// mur, rien n'interrompt une tenue (spec §2.3). Au sol, pas d'exemption, et
/// c'est voulu — une flânerie ne se termine QUE par ce délai. C'est donc lui
/// qui la relance toutes les 20 s, et qui laisse l'assoupissement s'y
/// brancher.
///
/// `matches!` sur un couple : les deux `Option` doivent être `Some`, et la
/// garde `if` pose la condition sur leur contenu.
pub fn sert_une_tenue_au_mur(ch: &Character) -> bool {
    matches!(
        (ch.tenue, ch.intention),
        (Some(t), Some(ai)) if !t.au_sol() && ai.kind == Intention::Grimper
    )
}

#[cfg(test)]
#[path = "tenue_tests.rs"]
mod tests;
```

Dans `src-tauri/src/behavior/mod.rs`, à côté des autres `pub mod` du module (`desire`, `intention`, `reflex`), ajouter :

```rust
pub mod tenue;
```

Dans `src-tauri/src/character/mod.rs`, dans `pub struct Character`, après le champ `intention` :

```rust
    /// L'action tenue, choisie au menu et gardée tant qu'on ne l'arrête pas
    /// (spec « menu sur mesure et actions tenues » §2). `None` : sa vie
    /// normale, tirée au sort. **Pour la session seulement** : jamais écrite
    /// dans `config.json`.
    pub tenue: Option<crate::behavior::tenue::Tenue>,
```

et dans le littéral de `Character::new`, après `intention: None,` :

```rust
            tenue: None,
```

Dans `src-tauri/src/behavior/reflex.rs`, juste après `ch.attachment = Attachment::Dragged;` (ligne ~225) :

```rust
                // L'attraper efface son action tenue (spec §2.2) : c'est le
                // geste universel pour « laisse tomber ce que tu fais ».
                ch.tenue = None;
```

- [ ] **Step 5 : Vérifier que tout passe**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 5`
Expected : `test result: ok.`, avec 6 tests de plus qu'avant.

- [ ] **Step 6 : Commit**

```bash
git add src-tauri/src/behavior/tenue.rs src-tauri/src/behavior/tenue_tests.rs src-tauri/src/behavior/mod.rs src-tauri/src/character/mod.rs src-tauri/src/behavior/reflex.rs src-tauri/src/behavior/tests.rs
git commit -m "feat(comportement): la tenue, une action du menu qui dure"
```

---

### Task 2 : Les commandes qui tiennent, et la relance à chaque fin d'intention

**Files :**
- Modify : `src-tauri/src/menu_perso.rs` (enum `Commande`)
- Modify : `src-tauri/src/behavior/mod.rs` (`pas` : bloc de la commande, couches 2 et 3)
- Modify : `src-tauri/src/behavior/intention.rs:538` (`poursuivre` : exemption du délai d'abandon)
- Test : `src-tauri/src/behavior/tests.rs`

**Interfaces :**
- Consomme : tout ce que produit la tâche 1.
- Produit : `menu_perso::Commande::{Basculer(Tenue), Tenir(Tenue), Relacher(Tenue)}` ; la variante `Commande::ResterAccroche` **disparaît** (remplacée par `Basculer(Tenue::ResterAccroche)`).

- [ ] **Step 1 : Écrire les tests**

À la fin de `src-tauri/src/behavior/tests.rs` :

```rust
// ── Les actions tenues (spec « menu sur mesure » §2) ──────────────────────

fn reglages_defaut() -> crate::config::Reglages {
    crate::config::Reglages::depuis(&crate::config::Config::default())
}

/// Joue `n` images d'affilée avec les mêmes entrées, à partir de `debut`.
/// Rend l'instant de la dernière. Le RNG est semé UNE fois par l'appelant
/// (CLAUDE.md, « Semer l'aléatoire une seule fois »).
fn jouer_images(
    ch: &mut Character,
    m: &World,
    e: &Entrees,
    rng: &mut XorShift32,
    debut: Duration,
    n: u32,
    mut chaque: impl FnMut(&Character),
) -> Duration {
    let table = desire::TableEnvies::defaut();
    let reglages = reglages_defaut();
    let mut t = debut;
    for _ in 0..n {
        t += Duration::from_secs_f32(DT);
        pas(ch, m, e, &table, &reglages, t, DT, rng);
        chaque(ch);
    }
    t
}

fn commander(ch: &mut Character, m: &World, c: crate::menu_perso::Commande, t: Duration, rng: &mut XorShift32) {
    let mut e = entrees(true, 1.0);
    e.commande = Some(c);
    pas(ch, m, &e, &desire::TableEnvies::defaut(), &reglages_defaut(), t, DT, rng);
}

#[test]
fn assis_au_menu_il_l_est_encore_une_minute_plus_tard() {
    use crate::menu_perso::Commande;
    let m = monde();
    let mut ch = perso(&m);
    let mut rng = XorShift32::seeded(11);
    let t0 = Duration::from_secs(1);

    commander(&mut ch, &m, Commande::Basculer(tenue::Tenue::Asseoir), t0, &mut rng);
    assert_eq!(ch.tenue, Some(tenue::Tenue::Asseoir));

    // Une image pour que `se_reposer` pose la pose, puis une minute : trois
    // fois le délai d'abandon, et quatre fois la plus longue pause assise.
    let t = jouer_images(&mut ch, &m, &entrees(true, 1.0), &mut rng, t0, 1, |_| {});
    jouer_images(&mut ch, &m, &entrees(true, 1.0), &mut rng, t, 3600, |ch| {
        assert_eq!(ch.tenue, Some(tenue::Tenue::Asseoir));
        assert_eq!(ch.pose, POSE_SIT);
    });
}

#[test]
fn basculer_deux_fois_rend_sa_vie_normale() {
    use crate::menu_perso::Commande;
    let m = monde();
    let mut ch = perso(&m);
    let mut rng = XorShift32::seeded(11);

    commander(&mut ch, &m, Commande::Basculer(tenue::Tenue::Asseoir), Duration::from_secs(1), &mut rng);
    commander(&mut ch, &m, Commande::Basculer(tenue::Tenue::Asseoir), Duration::from_secs(2), &mut rng);

    assert_eq!(ch.tenue, None);
}

#[test]
fn une_autre_tenue_remplace_la_premiere() {
    use crate::menu_perso::Commande;
    let m = monde();
    let mut ch = perso(&m);
    let mut rng = XorShift32::seeded(11);

    commander(&mut ch, &m, Commande::Basculer(tenue::Tenue::Asseoir), Duration::from_secs(1), &mut rng);
    commander(&mut ch, &m, Commande::Basculer(tenue::Tenue::BalancerLesJambes), Duration::from_secs(2), &mut rng);

    assert_eq!(ch.tenue, Some(tenue::Tenue::BalancerLesJambes));
}

#[test]
fn une_action_ponctuelle_efface_la_tenue() {
    use crate::menu_perso::Commande;
    let m = monde();
    let mut ch = perso(&m);
    let mut rng = XorShift32::seeded(11);

    commander(&mut ch, &m, Commande::Basculer(tenue::Tenue::Asseoir), Duration::from_secs(1), &mut rng);
    commander(
        &mut ch,
        &m,
        Commande::Intention(intention::Intention::Jouer(intention::Jeu::TeteQuiTourne)),
        Duration::from_secs(2),
        &mut rng,
    );

    assert_eq!(ch.tenue, None);
    assert_eq!(
        ch.intention.map(|i| i.kind),
        Some(intention::Intention::Jouer(intention::Jeu::TeteQuiTourne))
    );
}

#[test]
fn il_flane_encore_une_minute_plus_tard() {
    // La flânerie ne finit QUE par le délai d'abandon : sans relance par la
    // tenue, il repartirait au tirage au bout de 20 s.
    use crate::menu_perso::Commande;
    let m = monde();
    let mut ch = perso(&m);
    let mut rng = XorShift32::seeded(11);
    let t0 = Duration::from_secs(1);

    commander(&mut ch, &m, Commande::Basculer(tenue::Tenue::Flaner), t0, &mut rng);
    jouer_images(&mut ch, &m, &entrees(true, 1.0), &mut rng, t0, 3600, |ch| {
        assert_eq!(ch.tenue, Some(tenue::Tenue::Flaner));
        assert_eq!(ch.intention.map(|i| i.kind), Some(intention::Intention::Flaner));
    });
}

#[test]
fn absent_il_s_assoupit_puis_reprend_sa_tenue_au_retour() {
    use crate::menu_perso::Commande;
    let m = monde();
    let mut ch = perso(&m);
    let mut rng = XorShift32::seeded(11);
    let t0 = Duration::from_secs(1);

    commander(&mut ch, &m, Commande::Basculer(tenue::Tenue::Asseoir), t0, &mut rng);

    // Parti : ×8 sur le repos, et plus aucune activité. En 60 s, la pause
    // assise (15 s au plus) a eu le temps de basculer en sommeil.
    let mut a_dormi = false;
    let t = jouer_images(&mut ch, &m, &entrees(false, 8.0), &mut rng, t0, 3600, |ch| {
        if ch.pose == POSE_SLEEP {
            a_dormi = true;
        }
        assert_eq!(ch.tenue, Some(tenue::Tenue::Asseoir), "toujours cochée");
    });
    assert!(a_dormi, "il aurait dû s'assoupir");

    // Revenu : une seconde plus tard, il est de nouveau assis, pas endormi.
    jouer_images(&mut ch, &m, &entrees(true, 1.0), &mut rng, t, 60, |_| {});
    assert_eq!(ch.pose, POSE_SIT);
    assert_eq!(ch.tenue, Some(tenue::Tenue::Asseoir));
}

#[test]
fn une_tenue_de_sol_recue_sur_un_mur_est_ignoree() {
    use crate::menu_perso::Commande;
    let m = monde();
    let mur = mur_gauche(&m);
    let mut ch = perso(&m);
    ch.attachment = Attachment::On { platform: mur.id, face: Face::Right, offset: 300.0 };
    ch.intention = Some(intention::ActiveIntention::accroche(Duration::from_secs(1)));
    let mut rng = XorShift32::seeded(11);

    commander(&mut ch, &m, Commande::Basculer(tenue::Tenue::Asseoir), Duration::from_secs(1), &mut rng);

    assert_eq!(ch.tenue, None);
}

#[test]
fn une_tenue_devenue_injouable_est_effacee() {
    // Un rechargement à chaud a retiré la pose assise : la tenue ne doit
    // pas être forcée sur une image absente (spec §2.2, Review Focus n° 4).
    use crate::menu_perso::Commande;
    let m = monde();
    let mut ch = perso(&m);
    let mut rng = XorShift32::seeded(11);
    let t0 = Duration::from_secs(1);

    commander(&mut ch, &m, Commande::Basculer(tenue::Tenue::Asseoir), t0, &mut rng);
    ch.manifest.poses.remove(POSE_SIT);
    // L'intention en cours échoue dès l'image suivante, et la relance est
    // refusée : 20 images suffisent largement.
    jouer_images(&mut ch, &m, &entrees(true, 1.0), &mut rng, t0, 20, |_| {});

    assert_eq!(ch.tenue, None);
}
```

⚠️ Si `Manifest::poses` n'est pas un champ public de type `HashMap<String, _>` (à vérifier dans `character/manifest.rs`), remplacer `ch.manifest.poses.remove(POSE_SIT)` par la méthode du manifeste qui retire une pose. Elle existe pour `manifeste_sans` dans `intention_tests.rs` : la reprendre à l'identique.

- [ ] **Step 2 : Vérifier que ça ne compile pas**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 20`
Expected : `no variant named Basculer`.

- [ ] **Step 3 : Les trois commandes**

Dans `src-tauri/src/menu_perso.rs`, dans `pub enum Commande`, **supprimer** la variante `ResterAccroche` et son commentaire, puis ajouter :

```rust
    /// Tenir cette action, ou la relâcher si c'est déjà celle qu'il tient —
    /// ce que fait un clic sur une ligne du menu du personnage, dont la coche
    /// dit l'état (spec §3). Résolue par `behavior::pas`, qui seul connaît
    /// `ch.tenue` au moment où la commande arrive.
    Basculer(crate::behavior::tenue::Tenue),

    /// La tenir, quoi qu'il tienne déjà. Ce que devient un `Basculer` de la
    /// section « Tout le monde » quand tous ne la tiennent pas encore.
    Tenir(crate::behavior::tenue::Tenue),

    /// La relâcher s'il la tient, ne rien faire sinon. L'intention en cours
    /// continue : il reprend sa vie normale à la fin de celle-ci.
    Relacher(crate::behavior::tenue::Tenue),
```

Remplacer dans la table `ENVIES` la ligne `perso.rester` par :

```rust
    (
        "perso.rester",
        "Rester accroché",
        &[Ou::Mur, Ou::Plafond],
        Commande::Basculer(crate::behavior::tenue::Tenue::ResterAccroche),
    ),
```

Dans `lignes`, le `match commande` qui calcule `jouable` devient :

```rust
        let jouable = match commande {
            Commande::Intention(i) => table.jouable(manifeste, *i),
            Commande::Basculer(t) | Commande::Tenir(t) | Commande::Relacher(t) => {
                table.jouable(manifeste, t.intention())
            }
            Commande::Redescendre | Commande::SeLacher => true,
        };
```

Run : `rg -n "Commande::ResterAccroche" src-tauri/src`. Remplacer chaque occurrence restante, dans les tests, par `Commande::Basculer(crate::behavior::tenue::Tenue::ResterAccroche)`.

- [ ] **Step 4 : `behavior::pas` résout la commande et relance la tenue**

Dans `src-tauri/src/behavior/mod.rs`, remplacer `if let Some(cmd) = e.commande {` et la ligne `match cmd {` qui suit par :

```rust
    if let Some(cmd) = e.commande {
        // ── `Basculer` se résout ICI ────────────────────────────────────
        //
        // Le menu a affiché la coche d'après `ch.tenue` ; entre-temps, rien
        // n'a pu la changer que ce personnage lui-même. C'est donc ici, et
        // seulement ici, qu'on sait si le clic coche ou décoche.
        let cmd = match cmd {
            crate::menu_perso::Commande::Basculer(t) if ch.tenue == Some(t) => {
                crate::menu_perso::Commande::Relacher(t)
            }
            crate::menu_perso::Commande::Basculer(t) => crate::menu_perso::Commande::Tenir(t),
            autre => autre,
        };

        match cmd {
```

Dans le bras `Commande::Intention(voulue)`, dans la branche `} else if table.jouable(&ch.manifest, voulue) {`, ajouter en première ligne :

```rust
                    // Une action ponctuelle choisie au menu remplace l'action
                    // tenue : on la joue, puis il reprend sa vie (spec §2.2).
                    ch.tenue = None;
```

Supprimer le bras `crate::menu_perso::Commande::ResterAccroche => { … }`. À sa place, ajouter :

```rust
            crate::menu_perso::Commande::Tenir(t) => {
                // Le garde-fou de l'endroit, le même que pour une intention
                // de sol : une tenue de sol sur une paroi le ferait tomber,
                // « Rester accroché » au sol n'a pas de sens. Seul `Grimper`
                // vaut partout — au sol il part au mur, sur un mur il reprend.
                let sur_une_paroi = matches!(
                    ch.attachment,
                    crate::character::attach::Attachment::On { face, .. } if face != Face::Top
                );
                let a_sa_place = match t {
                    tenue::Tenue::Grimper => true,
                    tenue::Tenue::ResterAccroche => sur_une_paroi,
                    _ => !sur_une_paroi,
                };
                if a_sa_place && table.jouable(&ch.manifest, t.intention()) {
                    ch.tenue = Some(t);
                    ch.intention = Some(tenue::intention_pour(
                        t,
                        e,
                        reglages,
                        table,
                        &ch.manifest,
                        maintenant,
                    ));
                    return r;
                }
            }

            crate::menu_perso::Commande::Relacher(t) => {
                // Seulement si c'est bien celle-là : un « Tout le monde ›
                // S'asseoir » décoché ne doit pas relever un personnage qui
                // flâne.
                if ch.tenue == Some(t) {
                    ch.tenue = None;
                }
            }

            // Résolue plus haut en `Tenir` ou `Relacher` : ne peut plus
            // arriver ici. Un bras vide plutôt qu'un `unreachable!()`, qui
            // ferait paniquer la boucle 60 Hz si la résolution changeait.
            crate::menu_perso::Commande::Basculer(_) => {}
```

Dans le bras `Commande::Redescendre`, juste avant `ch.intention = Some(intention::ActiveIntention::redescendre(maintenant));` :

```rust
                    // Redescendre met fin à « Grimper au mur » (spec §2).
                    ch.tenue = None;
```

Dans le bras `Commande::SeLacher`, juste avant `ch.intention = None;` :

```rust
                ch.tenue = None;
```

Remplacer le bloc de la couche 2 :

```rust
    // ── Couche 2 : poursuivre l'intention en cours ──────────────────────
    match intention::poursuivre(ch, world, e, reglages, maintenant, dt, rng) {
        intention::Issue::EnCours => return r,
        // Finie ou échouée : on passe à la couche 3.
        intention::Issue::Finie | intention::Issue::Echouee => {}
    }
```

par :

```rust
    // ── Couche 2 : poursuivre l'intention en cours ──────────────────────
    //
    // Retenu AVANT : `poursuivre` efface l'intention quand elle finit, et
    // l'on ne saurait plus ensuite ce qu'elle servait.
    let servait_au_mur = tenue::sert_une_tenue_au_mur(ch);
    match intention::poursuivre(ch, world, e, reglages, maintenant, dt, rng) {
        intention::Issue::EnCours => return r,
        intention::Issue::Finie => {}
        intention::Issue::Echouee => {
            // Au mur, une tenue n'a pas de délai d'abandon : un échec y veut
            // dire une paroi perdue — l'action est devenue impossible, on
            // l'efface (spec §2.2). `poursuivre` l'a déjà fait lâcher.
            if servait_au_mur {
                ch.tenue = None;
            }
        }
    }

    // ── L'action tenue, AVANT le tirage : elle, elle choisit ────────────
    //
    // C'est le seul endroit où le code change de chemin (spec §2.1). Les
    // réflexes et l'intention en cours ne savent rien des tenues.
    //
    // `jouable` : un rechargement à chaud a pu retirer ses poses. Relancer
    // quand même donnerait une intention qui échoue à chaque image.
    if let Some(t) = ch.tenue {
        if table.jouable(&ch.manifest, t.intention()) {
            ch.intention = Some(tenue::intention_pour(
                t,
                e,
                reglages,
                table,
                &ch.manifest,
                maintenant,
            ));
            return r;
        }
        ch.tenue = None;
    }
```

- [ ] **Step 5 : L'exemption du délai d'abandon, au mur seulement**

Dans `src-tauri/src/behavior/intention.rs`, fonction `poursuivre`, remplacer :

```rust
    let issue = if maintenant.saturating_sub(ai.depuis) > delai_abandon(ai.kind, reglages) {
```

par :

```rust
    // Une tenue AU MUR n'a pas de délai d'abandon : c'est un ordre de
    // l'utilisateur, que rien n'interrompt (spec §2.3). Au sol, le délai
    // reste — c'est lui qui relance la flânerie toutes les 20 s.
    let exempte = super::tenue::sert_une_tenue_au_mur(ch);
    let issue = if !exempte
        && maintenant.saturating_sub(ai.depuis) > delai_abandon(ai.kind, reglages)
    {
```

- [ ] **Step 6 : Vérifier**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 15`
Expected : `test result: ok.` Si un test antérieur échoue, lire son message avant de toucher à quoi que ce soit : c'est probablement un test qui utilisait `Commande::ResterAccroche` (Step 3).

- [ ] **Step 7 : Commit**

```bash
git add src-tauri/src/menu_perso.rs src-tauri/src/behavior/mod.rs src-tauri/src/behavior/intention.rs src-tauri/src/behavior/tests.rs
git commit -m "feat(comportement): une action tenue se relance a chaque fin d'intention"
```

---

### Task 3 : « Grimper au mur » et « Rester accroché » tenus

**Files :**
- Modify : `src-tauri/src/behavior/intention.rs` (`grimper` : phases `Paroi` et `Accroche`)
- Test : `src-tauri/src/behavior/tests.rs`

**Interfaces :**
- Consomme : `Character::tenue`, `Tenue::{Grimper, ResterAccroche}`, `jouer_images` (tâche 2).

- [ ] **Step 1 : Écrire les tests**

À la fin de `src-tauri/src/behavior/tests.rs` :

```rust
fn perso_au_mur(m: &World, t: tenue::Tenue) -> Character {
    let mur = mur_gauche(m);
    let mut ch = perso(m);
    ch.attachment = Attachment::On { platform: mur.id, face: Face::Right, offset: 300.0 };
    ch.tenue = Some(t);
    ch.intention = Some(tenue::intention_pour(
        t,
        &entrees(true, 1.0),
        &reglages_defaut(),
        &desire::TableEnvies::defaut(),
        &ch.manifest,
        Duration::from_secs(1),
    ));
    ch
}

#[test]
fn grimper_tenu_ne_repose_jamais_le_pied_au_sol() {
    // Dix minutes, soit cinq fois le délai d'abandon de l'escalade : il
    // monte, redescend, passe au plafond — mais jamais au sol, jamais en l'air.
    let m = monde();
    let mut ch = perso_au_mur(&m, tenue::Tenue::Grimper);
    let mut rng = XorShift32::seeded(23);

    jouer_images(&mut ch, &m, &entrees(true, 1.0), &mut rng, Duration::from_secs(1), 36_000, |ch| {
        assert!(
            matches!(ch.attachment, Attachment::On { face, .. } if face != Face::Top),
            "il a quitté la paroi : {:?}",
            ch.attachment
        );
        assert_eq!(ch.tenue, Some(tenue::Tenue::Grimper));
    });
}

#[test]
fn rester_accroche_tenu_ne_bouge_plus() {
    let m = monde();
    let mut ch = perso_au_mur(&m, tenue::Tenue::ResterAccroche);
    let depart = ch.attachment;
    let mut rng = XorShift32::seeded(23);

    // Cinq minutes, utilisateur absent : au mur, rien ne l'interrompt.
    jouer_images(&mut ch, &m, &entrees(false, 8.0), &mut rng, Duration::from_secs(1), 18_000, |ch| {
        assert_eq!(ch.attachment, depart);
        assert_eq!(ch.tenue, Some(tenue::Tenue::ResterAccroche));
    });
}

#[test]
fn redescendre_met_fin_a_grimper_tenu() {
    use crate::menu_perso::Commande;
    let m = monde();
    let mut ch = perso_au_mur(&m, tenue::Tenue::Grimper);
    let mut rng = XorShift32::seeded(23);

    commander(&mut ch, &m, Commande::Redescendre, Duration::from_secs(2), &mut rng);
    assert_eq!(ch.tenue, None);

    // Et il arrive bien au sol : ~730 px à 16 px/s, soit 46 s — une minute
    // suffit.
    jouer_images(&mut ch, &m, &entrees(true, 1.0), &mut rng, Duration::from_secs(2), 3600, |_| {});
    assert!(matches!(ch.attachment, Attachment::On { face: Face::Top, .. }));
}
```

- [ ] **Step 2 : Vérifier qu'ils échouent**

Run : `cargo test --quiet grimper_tenu rester_accroche_tenu 2>&1 | Select-Object -Last 15`
Expected : `grimper_tenu_ne_repose_jamais_le_pied_au_sol` FAIL (il redescend au sol ou se lâche) et `rester_accroche_tenu_ne_bouge_plus` FAIL (il repart après sa pause d'accroche).

- [ ] **Step 3 : Modifier `grimper`**

En tête de `src-tauri/src/behavior/intention.rs`, avec les autres `use` :

```rust
use super::tenue::Tenue;
```

**Phase `Paroi`**, remplacer la condition :

```rust
                if cible >= longueur - 1.0 {
```

par :

```rust
                // `Grimper au mur` tenu ne repasse JAMAIS au sol de lui-même
                // (spec §2.4) : arrivé en bas, il s'accroche comme ailleurs.
                if cible >= longueur - 1.0 && ch.tenue != Some(Tenue::Grimper) {
```

**Phase `Accroche`**, remplacer :

```rust
            if maintenant < jusqu_a {
```

par :

```rust
            // « Rester accroché » tenu : la pause ne finit jamais. Il reste
            // là jusqu'à ce qu'on le décoche, l'attrape, ou le fasse lâcher.
            if maintenant < jusqu_a || ch.tenue == Some(Tenue::ResterAccroche) {
```

Plus bas dans la même phase, remplacer le tirage :

```rust
            let lache = match rng.weighted(&[e.poids_lacher, e.poids_redescendre]) {
```

par :

```rust
            // « Grimper au mur » tenu ne se lâche jamais de lui-même : seul
            // « Se lâcher » au menu le fait tomber. `if` AVANT le tirage :
            // on ne consomme pas l'aléatoire pour une issue interdite.
            let lache = if ch.tenue == Some(Tenue::Grimper) {
                false
            } else {
                match rng.weighted(&[e.poids_lacher, e.poids_redescendre]) {
```

et fermer ce `else` après le `};` du `match`, avec un `}` suivi de `;`. Vérifier que le bloc donne :

```rust
            let lache = if ch.tenue == Some(Tenue::Grimper) {
                false
            } else {
                match rng.weighted(&[e.poids_lacher, e.poids_redescendre]) {
                    Some(0) => true,
                    // (commentaire existant conservé)
                    _ => false,
                }
            };
```

Toujours dans `Accroche`, remplacer la cible de redescente :

```rust
                PhaseGrimpe::Paroi {
                    cible: plat.rect.face_length(face),
                }
```

par :

```rust
                // Tenu : un point au hasard de la paroi, en haut comme en
                // bas — c'est ce qui le fait « vivre » sur le mur. Sinon,
                // le bas, d'où il repasse au sol.
                PhaseGrimpe::Paroi {
                    cible: if ch.tenue == Some(Tenue::Grimper) {
                        rng.range(0.0, plat.rect.face_length(face))
                    } else {
                        plat.rect.face_length(face)
                    },
                }
```

> **Limite connue, assumée** : du plafond, aucune phase ne mène à un mur. Un personnage arrivé au plafond y reste et le traverse. Écrire ce passage demanderait de la navigation calculée, que la décision n° 4 exclut.

- [ ] **Step 4 : Vérifier**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 5`
Expected : `test result: ok.`

- [ ] **Step 5 : Commit**

```bash
git add src-tauri/src/behavior/intention.rs src-tauri/src/behavior/tests.rs
git commit -m "feat(escalade): grimper et rester accroche tenus"
```

---

### Task 4 : Le contenu du menu : coches, section « Tout le monde », et le tray

**Files :**
- Modify : `src-tauri/src/menu_perso.rs` (`ENVIES`, nouvelle table `TOUS`, `Ligne`, `lignes`, `commande_de_tous`, `resoudre_pour_tous`)
- Modify : `src-tauri/src/menu_perso_tests.rs`
- Modify : `src-tauri/src/actions.rs` (routage des `tous.*`, retrait d'`ID_TOUS_AU_MUR`)
- Modify : `src-tauri/src/tray.rs` (retrait de l'entrée)
- Modify : `src-tauri/src/main.rs` (tenues de tous, résolution, appel de `lignes`)
- Modify : `src-tauri/src/menu_natif.rs` (coche et titre, provisoirement)

**Interfaces :**
- Consomme : `Commande::{Basculer, Tenir, Relacher}` (tâche 2), `Tenue` (tâche 1).
- Produit : `Ligne::{Entree { id: &'static str, libelle: &'static str, coche: bool }, Titre { texte: &'static str }, Separateur}` ; `lignes(manifeste: &Manifest, table: &TableEnvies, ou: Ou, tenue: Option<Tenue>, tenues_de_tous: &[Option<Tenue>]) -> Vec<Ligne>` ; `commande_de_tous(id: &str) -> Option<Commande>` ; `resoudre_pour_tous(c: Commande, tenues: &[Option<Tenue>]) -> Commande`.

- [ ] **Step 1 : Écrire les tests**

Dans `src-tauri/src/menu_perso_tests.rs` :

1. Dans `ids_proposes`, l'appel devient `lignes(manifeste, table, ou, None, &[])`, et le `filter_map` a pour bras : `Ligne::Entree { id, .. } => Some(id), Ligne::Titre { .. } | Ligne::Separateur => None`.
2. Dans tous les tests existants, chaque appel à `lignes(&blob, &table, ou)` devient `lignes(&blob, &table, ou, None, &[])`, et le motif `Ligne::Entree { id, libelle } if` devient `Ligne::Entree { id, libelle, .. } if`.
3. Mettre à jour les trois tests d'endroit :
   - `le_menu_au_sol_ne_propose_aucune_action_d_accroche` : retirer `"perso.monter"` de la liste des interdits ; `ids.len()` reste 5.
   - `le_menu_sur_un_mur_ne_propose_aucune_envie_de_sol` : retirer `"perso.grimper"` des interdits ; remplacer `assert!(ids.contains(&"perso.monter"));` par `assert!(ids.contains(&"perso.grimper"));` ; `ids.len()` reste 4, et le commentaire devient « Grimper au mur, Rester accroché, Redescendre, Se lâcher ».
   - `le_menu_au_plafond_ne_propose_ni_envie_de_sol_ni_redescendre` : retirer `"perso.grimper"` et `"perso.monter"` des interdits ; `ids.len()` passe à **3**, et ajouter `assert!(ids.contains(&"perso.grimper"));`.
4. Remplacer le test `tous_au_mur_n_est_pas_une_envie_du_demandeur` par ceux-ci, ajoutés à la fin :

```rust
// ── Les coches et la section « Tout le monde » (spec §3) ────────────────

use crate::behavior::tenue::Tenue;

fn coche_de(lignes: &[Ligne], cherche: &str) -> Option<bool> {
    lignes.iter().find_map(|l| match l {
        Ligne::Entree { id, coche, .. } if *id == cherche => Some(*coche),
        _ => None,
    })
}

#[test]
fn la_coche_du_personnage_suit_sa_tenue() {
    let (table, blob) = table_et_blob();
    let l = lignes(&blob, &table, Ou::Sol, Some(Tenue::Asseoir), &[Some(Tenue::Asseoir)]);
    assert_eq!(coche_de(&l, "perso.asseoir"), Some(true));
    assert_eq!(coche_de(&l, "perso.flaner"), Some(false));
    // Une action ponctuelle n'a jamais de coche.
    assert_eq!(coche_de(&l, "perso.tete"), Some(false));
}

#[test]
fn tout_le_monde_n_est_coche_que_si_tous_la_tiennent() {
    let (table, blob) = table_et_blob();
    let un_seul = lignes(&blob, &table, Ou::Sol, None, &[Some(Tenue::Asseoir), None]);
    assert_eq!(coche_de(&un_seul, "tous.asseoir"), Some(false));

    let tous = lignes(&blob, &table, Ou::Sol, None, &[Some(Tenue::Asseoir), Some(Tenue::Asseoir)]);
    assert_eq!(coche_de(&tous, "tous.asseoir"), Some(true));
}

#[test]
fn la_section_tout_le_monde_suit_son_titre_avec_les_actions_du_sol() {
    let (table, blob) = table_et_blob();
    // Même au mur : la section « Tout le monde » ne propose que le sol.
    let l = lignes(&blob, &table, Ou::Mur, None, &[None]);
    let titre = l
        .iter()
        .position(|x| *x == Ligne::Titre { texte: "Tout le monde" })
        .expect("le titre de section");
    let apres: Vec<&str> = l[titre + 1..]
        .iter()
        .map_while(|x| match x {
            Ligne::Entree { id, .. } if id.starts_with("tous.") => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(apres, ["tous.flaner", "tous.asseoir", "tous.tete", "tous.jambes", "tous.grimper"]);
}

#[test]
fn monter_plus_haut_a_disparu() {
    let (table, blob) = table_et_blob();
    for ou in [Ou::Sol, Ou::Mur, Ou::Plafond] {
        assert_eq!(coche_de(&lignes(&blob, &table, ou, None, &[]), "perso.monter"), None);
    }
}

#[test]
fn les_identifiants_tous_ne_vont_pas_au_demandeur() {
    assert_eq!(commande_de("tous.grimper"), None);
    assert_eq!(
        commande_de_tous("tous.grimper"),
        Some(Commande::Basculer(Tenue::Grimper))
    );
    assert_eq!(commande_de_tous("perso.grimper"), None);
}

#[test]
fn resoudre_pour_tous_tient_si_un_seul_ne_la_tient_pas() {
    let c = resoudre_pour_tous(
        Commande::Basculer(Tenue::Asseoir),
        &[Some(Tenue::Asseoir), None],
    );
    assert_eq!(c, Commande::Tenir(Tenue::Asseoir));
}

#[test]
fn resoudre_pour_tous_relache_si_tous_la_tiennent() {
    let c = resoudre_pour_tous(
        Commande::Basculer(Tenue::Asseoir),
        &[Some(Tenue::Asseoir), Some(Tenue::Asseoir)],
    );
    assert_eq!(c, Commande::Relacher(Tenue::Asseoir));
}

#[test]
fn resoudre_pour_tous_laisse_passer_le_reste() {
    let c = Commande::Intention(Intention::Jouer(Jeu::TeteQuiTourne));
    assert_eq!(resoudre_pour_tous(c, &[None]), c);
}
```

⚠️ `Commande` doit implémenter `PartialEq` et `Debug` pour ces `assert_eq!`. Vérifier son `#[derive]`, et les ajouter s'ils manquent.

- [ ] **Step 2 : Vérifier que ça ne compile pas**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 20`
Expected : erreurs sur `lignes` (arguments) et `commande_de_tous` introuvable.

- [ ] **Step 3 : Les tables**

Dans `src-tauri/src/menu_perso.rs`, remplacer les lignes de `ENVIES` par les suivantes. **Garder** le long commentaire au-dessus de `perso.tete` (« Le libellé ne décrit PAS le dessin… »), et mettre à jour le commentaire au-dessus de la table (ligne ~192) : « Grimper au mur » est désormais **une seule ligne**, proposée au sol, au mur et au plafond.

```rust
const ENVIES: &[(&str, &str, &[Ou], Commande)] = &[
    ("perso.flaner", "Flâner", &[Ou::Sol], Commande::Basculer(Tenue::Flaner)),
    ("perso.asseoir", "S'asseoir", &[Ou::Sol], Commande::Basculer(Tenue::Asseoir)),
    // (commentaire « Le libellé ne décrit PAS le dessin… » conservé ici)
    (
        "perso.tete",
        "Faire son petit truc",
        &[Ou::Sol],
        Commande::Intention(Intention::Jouer(Jeu::TeteQuiTourne)),
    ),
    (
        "perso.jambes",
        "Balancer les jambes",
        &[Ou::Sol],
        Commande::Basculer(Tenue::BalancerLesJambes),
    ),
    // Une seule ligne pour les trois endroits : tenue, elle ne change pas
    // de sens selon qu'on la lance ou qu'on la reprend. « Monter plus
    // haut », qui la doublait au mur, a disparu (spec §2).
    (
        "perso.grimper",
        "Grimper au mur",
        &[Ou::Sol, Ou::Mur, Ou::Plafond],
        Commande::Basculer(Tenue::Grimper),
    ),
    (
        "perso.rester",
        "Rester accroché",
        &[Ou::Mur, Ou::Plafond],
        Commande::Basculer(Tenue::ResterAccroche),
    ),
    ("perso.redescendre", "Redescendre", &[Ou::Mur], Commande::Redescendre),
    ("perso.lacher", "Se lâcher", &[Ou::Mur, Ou::Plafond], Commande::SeLacher),
];

/// La section « Tout le monde » (spec §3) : les actions du SOL, données à
/// tous les présents par la boîte `Actions::pour_tous`.
///
/// Une table à part et non une colonne d'`ENVIES` : ses identifiants sont
/// DIFFÉRENTS (`tous.*`), et c'est ce qui les envoie dans l'autre boîte.
/// Réutiliser `perso.asseoir` ferait asseoir le seul demandeur.
///
/// Seulement le sol : les autres sont n'importe où, et « Se lâcher » n'a
/// de sens que pour qui est accroché.
const TOUS: &[(&str, &str, Commande)] = &[
    ("tous.flaner", "Flâner", Commande::Basculer(Tenue::Flaner)),
    ("tous.asseoir", "S'asseoir", Commande::Basculer(Tenue::Asseoir)),
    (
        "tous.tete",
        "Faire son petit truc",
        Commande::Intention(Intention::Jouer(Jeu::TeteQuiTourne)),
    ),
    ("tous.jambes", "Balancer les jambes", Commande::Basculer(Tenue::BalancerLesJambes)),
    ("tous.grimper", "Grimper au mur", Commande::Basculer(Tenue::Grimper)),
];
```

Ajouter en tête du fichier `use crate::behavior::tenue::Tenue;`, et retirer `ID_TOUS_AU_MUR` du `use crate::actions::{…}`.

Après `commande_de`, ajouter :

```rust
/// La commande d'une entrée de la section « Tout le monde », si `id` en
/// désigne une. Le pendant de `commande_de` pour la table `TOUS`.
pub fn commande_de_tous(id: &str) -> Option<Commande> {
    TOUS.iter().find(|(i, _, _)| *i == id).map(|(_, _, c)| *c)
}

/// Ce que devient une commande de la section « Tout le monde » pour les
/// personnages présents (spec §3) : coché si TOUS la tiennent, donc un clic
/// relâche chez tous ; sinon, un clic la donne à tous.
///
/// Résolue UNE fois par la boucle, avant de servir les acteurs : chaque
/// acteur résolvant son propre `Basculer`, un personnage déjà assis se
/// relèverait pendant que les autres s'assoient.
///
/// `!is_empty()` : sans personne, « tous la tiennent » serait vrai par
/// vacuité, et le clic relâcherait… personne. On tient.
pub fn resoudre_pour_tous(c: Commande, tenues: &[Option<Tenue>]) -> Commande {
    match c {
        Commande::Basculer(t) => {
            let tous = !tenues.is_empty() && tenues.iter().all(|x| *x == Some(t));
            if tous {
                Commande::Relacher(t)
            } else {
                Commande::Tenir(t)
            }
        }
        autre => autre,
    }
}
```

- [ ] **Step 4 : `Ligne` et `lignes`**

Remplacer l'enum `Ligne` :

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ligne {
    /// `id` est l'identifiant que reçoit `actions::executer` ; `coche` dit
    /// si l'action est tenue — DÉDUITE de la tenue, jamais stockée.
    Entree {
        id: &'static str,
        libelle: &'static str,
        coche: bool,
    },
    /// Un intitulé de section, non cliquable.
    Titre { texte: &'static str },
    Separateur,
}
```

Changer la signature de `lignes` et sa documentation :

```rust
/// … (documentation existante), plus :
///
/// `tenue` est celle du personnage cliqué, `tenues_de_tous` celles de tous
/// les présents — lui compris. Ce sont les seules sources des coches.
pub fn lignes(
    manifeste: &Manifest,
    table: &TableEnvies,
    ou: Ou,
    tenue: Option<Tenue>,
    tenues_de_tous: &[Option<Tenue>],
) -> Vec<Ligne> {
```

Dans la boucle sur `ENVIES`, le `push` devient :

```rust
        if jouable {
            // Seul un `Basculer` a une coche : une action ponctuelle n'a
            // pas d'état à montrer.
            let coche = matches!(commande, Commande::Basculer(t) if tenue == Some(*t));
            lignes.push(Ligne::Entree { id, libelle, coche });
        }
```

Remplacer l'entrée `ID_TOUS_AU_MUR` et son commentaire par la section :

```rust
    // ── La section « Tout le monde » (spec §3) ──────────────────────────
    //
    // Un titre plutôt que « tout le monde » répété à chaque ligne : c'est la
    // demande de l'auteur. Toujours proposée, même à un pack sans escalade :
    // elle s'adresse aux AUTRES aussi, et chacun refuse ce qu'il ne peut pas
    // exécuter (`behavior::pas`).
    lignes.push(Ligne::Titre { texte: "Tout le monde" });
    for (id, libelle, commande) in TOUS {
        let coche = match commande {
            Commande::Basculer(t) => {
                !tenues_de_tous.is_empty() && tenues_de_tous.iter().all(|x| *x == Some(*t))
            }
            _ => false,
        };
        lignes.push(Ligne::Entree { id, libelle, coche });
    }
    lignes.push(Ligne::Separateur);
```

Les autres `Ligne::Entree { id: …, libelle: … }` de la fonction (cacher, catalogue, quitter) gagnent `coche: false`.

- [ ] **Step 5 : Router les `tous.*`, retirer « Tout le monde au mur »**

Dans `src-tauri/src/actions.rs` :
- supprimer la constante `ID_TOUS_AU_MUR` et son commentaire ;
- supprimer le bras `ID_TOUS_AU_MUR => { … }` d'`executer` ;
- remplacer le bras par défaut par :

```rust
        // ── Une envie ou une action demandée par le menu du personnage ───
        //
        // Deux tables, deux boîtes : `perso.*` va au seul demandeur,
        // `tous.*` à tous les présents (spec §3). C'est l'identifiant qui
        // décide, jamais le menu d'où vient le clic.
        autre => {
            if let Some(commande) = crate::menu_perso::commande_de(autre) {
                deposer_commande(actions, commande);
            } else if let Some(commande) = crate::menu_perso::commande_de_tous(autre) {
                deposer_dans(&actions.pour_tous, commande);
            } else {
                eprintln!("entrée de menu non gérée : {autre}");
            }
        }
```

Dans `src-tauri/src/tray.rs`, supprimer la construction de `tous_au_mur` (le `MenuItem::with_id(… ID_TOUS_AU_MUR …)` et son commentaire) ainsi que `&tous_au_mur,` dans la liste des entrées.

Mettre à jour le commentaire de `Actions::pour_tous`, qui cite « Tout le monde au mur » : il porte désormais les commandes de la section « Tout le monde ».

- [ ] **Step 6 : La boucle calcule les tenues et résout**

Dans `src-tauri/src/main.rs`, juste **après** la lecture de `commande_pour_tous`, remplacer :

```rust
        let commande_pour_tous = match actions.pour_tous.try_lock() {
            Ok(mut boite) => boite.take(),
            Err(_) => None,
        };
```

par :

```rust
        let commande_pour_tous = match actions.pour_tous.try_lock() {
            Ok(mut boite) => boite.take(),
            Err(_) => None,
        };

        // Les tenues de tous les présents, pour les coches du menu et pour
        // résoudre une commande « Tout le monde ». Un acteur en départ n'est
        // plus là : il ne compte pas.
        let tenues_de_tous: Vec<Option<behavior::tenue::Tenue>> = acteurs
            .iter()
            .filter(|a| a.depart.is_none())
            .map(|a| a.ch.tenue)
            .collect();

        // Résolue UNE fois, pour tous — voir `menu_perso::resoudre_pour_tous`.
        // `map` : ne s'applique que s'il y a une commande.
        let commande_pour_tous =
            commande_pour_tous.map(|c| menu_perso::resoudre_pour_tous(c, &tenues_de_tous));
```

Au clic droit, l'appel `menu_perso::lignes(&acteur.ch.manifest, &table, ou)` devient :

```rust
                let lignes = menu_perso::lignes(
                    &acteur.ch.manifest,
                    &table,
                    ou,
                    acteur.ch.tenue,
                    &tenues_de_tous,
                );
```

- [ ] **Step 7 : Le menu natif montre coches et titre (provisoire, jusqu'à la tâche 5)**

Dans `src-tauri/src/menu_natif.rs`, ajouter `MF_CHECKED, MF_GRAYED` au `use … WindowsAndMessaging::{…}`, et remplacer le `match ligne` de la boucle par :

```rust
            match ligne {
                Ligne::Entree { libelle, coche, .. } => {
                    let texte = windows::core::HSTRING::from(*libelle);
                    // `|` : les drapeaux Win32 se combinent bit à bit.
                    let drapeaux = if *coche { MF_STRING | MF_CHECKED } else { MF_STRING };
                    let _ = AppendMenuW(menu, drapeaux, rang + 1, &texte);
                }
                Ligne::Titre { texte } => {
                    // Grisé : un titre ne se choisit pas.
                    let texte = windows::core::HSTRING::from(*texte);
                    let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, rang + 1, &texte);
                }
                Ligne::Separateur => {
                    let _ = AppendMenuW(menu, MF_SEPARATOR, 0, windows::core::PCWSTR::null());
                }
            }
```

- [ ] **Step 8 : Vérifier**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 5` puis `cargo build 2>&1 | Select-Object -Last 20`
Expected : tests OK, compilation sans erreur, et aucun nouvel avertissement.

Puis `cargo run`. Au clic droit : coches visibles, section « Tout le monde » sous un titre grisé, « Tout le monde au mur » absent du tray.

- [ ] **Step 9 : Commit**

```bash
git add src-tauri/src/menu_perso.rs src-tauri/src/menu_perso_tests.rs src-tauri/src/actions.rs src-tauri/src/tray.rs src-tauri/src/main.rs src-tauri/src/menu_natif.rs
git commit -m "feat(menu): coches des actions tenues et section Tout le monde"
```

---

### Task 5 : La fenêtre webview du menu

**Files :**
- Create : `src-tauri/src/menu_fenetre.rs`
- Create : `src-tauri/src/menu_fenetre_tests.rs`
- Create : `ui/menu.html`, `ui/menu.css`, `ui/menu.js`
- Create : `src-tauri/capabilities/menu.json`
- Modify : `src-tauri/src/menu_perso.rs` (`Ligne` sérialisable, `id_connu`)
- Modify : `src-tauri/src/commandes.rs` (trois commandes)
- Modify : `src-tauri/src/render.rs` (`appliquer_style_outil` ; retrait de `reveiller_la_file`)
- Modify : `src-tauri/src/main.rs` (module, `generate_handler!`, `manage`, création au `setup`, branchement dans la boucle)
- Delete : `src-tauri/src/menu_natif.rs`

**Interfaces :**
- Consomme : `lignes(…)` et `Ligne` (tâche 4) ; `Actions::executer_choix_du_menu(self: Arc<Self>, app: AppHandle, id: &'static str)` (déjà dans `actions.rs`) ; `render::{fenetre_au_premier_plan, prendre_le_premier_plan, rendre_le_premier_plan}`.
- Produit : `menu_fenetre::{LABEL, MARGE_CSS, EtatMenu, creer, ouvrir, placer, fermer, rect_ouvert, rectangle, hors_du_menu}` ; commandes `placer_menu(largeur: f32, hauteur: f32)`, `choisir_entree_menu(id: String)`, `fermer_menu()` ; `menu_perso::id_connu(id: &str) -> Option<&'static str>`.

- [ ] **Step 1 : Écrire les tests des calculs purs**

Créer `src-tauri/src/menu_fenetre_tests.rs` :

```rust
//! Les tests de `menu_fenetre` : le placement du menu, sans écran.

use super::*;
use crate::geom::Rect;

/// Un écran 1920×1080 à l'origine.
fn ecran() -> Rect {
    Rect::new(0.0, 0.0, 1920.0, 1080.0)
}

#[test]
fn au_milieu_le_coin_du_menu_est_sous_le_curseur() {
    // 200×300 CSS à l'échelle 1 : la fenêtre ajoute la marge de l'ombre
    // de chaque côté, et se décale d'autant pour que le MENU, lui, parte
    // du curseur.
    let (x, y, w, h) = rectangle((500, 400), (200.0, 300.0), 1.0, ecran());
    let m = MARGE_CSS as i32;
    assert_eq!((x, y), (500 - m, 400 - m));
    assert_eq!((w, h), (200 + 2 * m as u32, 300 + 2 * m as u32));
}

#[test]
fn le_menu_se_replie_dans_l_ecran() {
    // Review Focus n° 1 : près du coin bas-droit, il s'ouvre vers le haut
    // et vers la gauche, et reste entièrement dans l'écran.
    let (x, y, w, h) = rectangle((1900, 1070), (200.0, 300.0), 1.0, ecran());
    assert!(x >= 0 && y >= 0, "({x}, {y})");
    assert!(x + w as i32 <= 1920, "déborde à droite : {x} + {w}");
    assert!(y + h as i32 <= 1080, "déborde en bas : {y} + {h}");
}

#[test]
fn la_taille_suit_l_echelle_de_l_ecran() {
    // Review Focus n° 2 : 200 px CSS à 125 % = 250 px physiques.
    let (_, _, w, _) = rectangle((500, 400), (200.0, 300.0), 1.25, ecran());
    let attendu = ((200.0 + 2.0 * MARGE_CSS) * 1.25).ceil() as u32;
    assert_eq!(w, attendu);
}

#[test]
fn un_ecran_a_droite_du_premier_garde_son_origine() {
    let droite = Rect::new(1920.0, 0.0, 1920.0, 1080.0);
    let (x, _, _, _) = rectangle((2000, 400), (200.0, 300.0), 1.0, droite);
    assert!(x >= 1920, "le menu a glissé sur l'écran voisin : {x}");
}

#[test]
fn un_clic_hors_du_menu_est_detecte() {
    // Review Focus n° 3 : le filet quand `blur` ne vient jamais.
    let rect = (100, 100, 200, 300);
    assert!(!hors_du_menu((150, 150), rect));
    assert!(hors_du_menu((99, 150), rect));
    assert!(hors_du_menu((150, 401), rect));
}
```

Et dans `src-tauri/src/menu_perso_tests.rs` :

```rust
#[test]
fn chaque_entree_affichee_est_un_identifiant_connu() {
    // `choisir_entree_menu` n'accepte que ce que `id_connu` reconnaît : une
    // entrée affichée mais inconnue serait un clic sans effet.
    let (table, blob) = table_et_blob();
    for ou in [Ou::Sol, Ou::Mur, Ou::Plafond] {
        for l in lignes(&blob, &table, ou, None, &[None]) {
            if let Ligne::Entree { id, .. } = l {
                assert_eq!(id_connu(id), Some(id), "{id}");
            }
        }
    }
    assert_eq!(id_connu("n'importe.quoi"), None);
}
```

- [ ] **Step 2 : Vérifier que ça ne compile pas**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 20`
Expected : `menu_fenetre` et `id_connu` introuvables.

- [ ] **Step 3 : `Ligne` sérialisable, et `id_connu`**

Dans `src-tauri/src/menu_perso.rs`, le `derive` de `Ligne` devient :

```rust
/// `Serialize` avec `tag = "type"` : chaque ligne devient un objet JSON
/// portant son genre — `{"type":"Entree","id":…,"libelle":…,"coche":…}`,
/// `{"type":"Titre","texte":…}`, `{"type":"Separateur"}` — que `menu.js`
/// lit par `ligne.type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type")]
pub enum Ligne {
```

Ajouter :

```rust
/// L'identifiant `&'static` qui correspond à `id`, s'il est l'une de nos
/// entrées ; `None` sinon.
///
/// La fenêtre du menu renvoie une `String` : c'est ici qu'elle redevient
/// un identifiant du programme. **N'accepter que ce qu'on connaît** —
/// `actions::executer` ne recevra jamais une chaîne arbitraire venue d'un
/// webview.
pub fn id_connu(id: &str) -> Option<&'static str> {
    let communs = [ID_P_CACHER_CE, ID_P_CACHER, ID_CATALOGUE, ID_QUITTER];
    ENVIES
        .iter()
        .map(|(i, _, _, _)| *i)
        .chain(TOUS.iter().map(|(i, _, _)| *i))
        .chain(communs)
        .find(|i| *i == id)
}
```

- [ ] **Step 4 : Le module `menu_fenetre`**

Créer `src-tauri/src/menu_fenetre.rs` :

```rust
//! La fenêtre du menu du clic droit : une webview dédiée, créée UNE fois au
//! démarrage, montrée au curseur à chaque clic droit (spec « menu sur
//! mesure » §4).
//!
//! Responsabilité unique : montrer, placer et cacher cette fenêtre. Ce
//! qu'elle affiche vient de `menu_perso::lignes`, ce que fait un choix de
//! `actions::executer`.
//!
//! # Pourquoi une fenêtre et non le menu de Tauri
//!
//! Le menu de Tauri (`muda`) s'exécute dans un callback de `tao`, et `tao`
//! met en file tout événement reçu pendant ce temps (`should_buffer`,
//! `tao-0.35.3/src/platform_impl/windows/event_loop/runner.rs:143`) — dont
//! les `eval` qui portent les positions : **tous** les personnages se
//! figeaient. Une fenêtre n'a pas de boucle modale : rien n'est retenu.
//! Et elle se stylise en CSS, ce qu'aucun menu natif ne permet.
//!
//! ⚠️ **Créée une fois, jamais à la volée** : créer une fenêtre WebView2
//! est un travail lourd du thread principal — c'est le gel constaté à la
//! fermeture des fenêtres d'écran (CLAUDE.md, « Une fenêtre par ÉCRAN »).

use crate::geom::Rect;
use crate::menu_perso::Ligne;
use crate::probe::ScreenInfo;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

/// Le label de la fenêtre, repris par `capabilities/menu.json`.
pub const LABEL: &str = "menu";

/// La marge autour du menu, en pixels CSS, pour que son ombre ait de la
/// place dans la fenêtre. **Doit valoir `--marge` de `ui/menu.css`.**
pub const MARGE_CSS: f32 = 12.0;

/// Le menu en cours d'affichage.
struct MenuOuvert {
    /// Le curseur au clic droit, en pixels physiques du bureau virtuel.
    curseur: (i32, i32),
    /// L'écran du curseur : ses bornes (pour replier le menu dedans) et son
    /// échelle (pour convertir la taille CSS en pixels physiques).
    bornes: Rect,
    echelle: f32,
    /// Le drapeau qu'attend la boucle 60 Hz (`menu_en_cours`).
    ferme: Arc<AtomicBool>,
    /// La fenêtre qui avait le premier plan, à qui le rendre. Un `isize`
    /// et non un `HWND` : ce dernier enveloppe un pointeur brut, que Rust ne
    /// laisse pas passer d'un thread à l'autre dans un `Mutex` partagé.
    precedente: Option<isize>,
    /// Le rectangle de la fenêtre une fois placée (x, y, largeur, hauteur),
    /// pour le filet du « clic ailleurs ».
    rect: Option<(i32, i32, i32, i32)>,
}

/// L'état partagé entre la boucle (qui ouvre) et les commandes (qui placent
/// et ferment). Enregistré par `app.manage` au `setup`.
#[derive(Default)]
pub struct EtatMenu(Mutex<Option<MenuOuvert>>);

/// Crée la fenêtre, cachée. Appelée une fois, au `setup`.
pub fn creer(app: &AppHandle) -> Result<(), String> {
    let win = tauri::WebviewWindowBuilder::new(
        app,
        LABEL,
        tauri::WebviewUrl::App("menu.html".into()),
    )
    .title("shimeji-desktop")
    .visible(false)
    .decorations(false)
    // Transparente : c'est le CSS qui dessine le fond, les coins arrondis
    // et l'ombre.
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(false)
    .focused(false)
    .inner_size(10.0, 10.0)
    .build()
    .map_err(|e| format!("fenêtre du menu : {e}"))?;

    // Hors d'Alt+Tab, mais ACTIVABLE — contrairement aux fenêtres d'écran,
    // pas de `WS_EX_NOACTIVATE` : c'est la prise du focus qui permet de la
    // fermer quand on clique ailleurs (`blur`).
    crate::render::appliquer_style_outil(&win)
}

/// Ouvre le menu : mémorise où, puis envoie les lignes à la fenêtre. Elle
/// se mesurera, et appellera `placer_menu` pour être placée et montrée.
///
/// Appelée depuis la boucle 60 Hz : `eval` ne bloque pas, il poste le script
/// au thread principal.
pub fn ouvrir(
    app: &AppHandle,
    lignes: &[Ligne],
    curseur: (i32, i32),
    ecran: &ScreenInfo,
    ferme: Arc<AtomicBool>,
) {
    let Some(win) = app.get_webview_window(LABEL) else {
        // Sans fenêtre, pas de menu : on le dit fermé aussitôt, sinon le
        // personnage cliqué resterait figé pour toujours.
        ferme.store(true, Ordering::Release);
        return;
    };

    let etat = app.state::<EtatMenu>();
    if let Ok(mut e) = etat.0.lock() {
        *e = Some(MenuOuvert {
            curseur,
            bornes: ecran.bounds,
            echelle: ecran.scale,
            ferme: ferme.clone(),
            precedente: crate::render::fenetre_au_premier_plan().map(|h| h.0 as isize),
            rect: None,
        });
    }

    // La hauteur maximale, en pixels CSS : l'écran moins les marges. Au-delà,
    // le menu défile (`overflow-y: auto` dans `menu.css`).
    let hauteur_max = ecran.bounds.h / ecran.scale.max(0.01) - 2.0 * MARGE_CSS - 16.0;
    let charge = serde_json::json!({ "lignes": lignes, "hauteurMax": hauteur_max });
    if win.eval(format!("window.afficherMenu({charge})")).is_err() {
        ferme.store(true, Ordering::Release);
    }
}

/// Place la fenêtre à la taille mesurée par `menu.js`, puis la montre et lui
/// donne le premier plan. Appelée par la commande `placer_menu`.
pub fn placer(app: &AppHandle, largeur_css: f32, hauteur_css: f32) {
    let Some(win) = app.get_webview_window(LABEL) else {
        return;
    };
    let etat = app.state::<EtatMenu>();
    let Ok(mut e) = etat.0.lock() else {
        return;
    };
    // `as_mut` : emprunte le contenu de l'`Option` pour le modifier en place.
    let Some(ouvert) = e.as_mut() else {
        // Fermé entre-temps (Échap très rapide) : rien à montrer.
        return;
    };

    let (x, y, w, h) = rectangle(
        ouvert.curseur,
        (largeur_css, hauteur_css),
        ouvert.echelle,
        ouvert.bornes,
    );
    ouvert.rect = Some((x, y, w as i32, h as i32));

    // Position, taille, puis position encore. Arriver sur un écran de DPI
    // différent fait redimensionner la fenêtre par Windows (`WM_DPICHANGED`) :
    // la taille est donc posée APRÈS, et la position reposée au cas où ce
    // redimensionnement l'aurait déplacée.
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let _ = win.set_size(PhysicalSize::new(w, h));
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let _ = win.show();

    // Le premier plan, VÉRIFIÉ : un refus donnerait un menu qui ne se ferme
    // pas au clic ailleurs. Le filet de la boucle (`hors_du_menu`) couvre ce
    // cas, et `SHIMEJI_MENU=1` le signale.
    if let Ok(hwnd) = win.hwnd() {
        if !crate::render::prendre_le_premier_plan(hwnd) && std::env::var_os("SHIMEJI_MENU").is_some() {
            eprintln!("menu : Windows a refusé le premier plan");
        }
    }
    let _ = win.set_focus();

    if std::env::var_os("SHIMEJI_MENU").is_some() {
        println!("menu placé : {w}×{h} @ ({x}, {y})");
    }
}

/// Ferme le menu : cache la fenêtre, rend le focus, prévient la boucle.
///
/// **Idempotente** — un choix est suivi d'un `blur`, et le filet de la
/// boucle peut s'y ajouter : `take()` fait que seul le premier appel agit.
pub fn fermer(app: &AppHandle) {
    let etat = app.state::<EtatMenu>();
    let ouvert = match etat.0.lock() {
        Ok(mut e) => e.take(),
        Err(_) => None,
    };
    let Some(ouvert) = ouvert else {
        return;
    };

    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.hide();
    }
    if let Some(h) = ouvert.precedente {
        crate::render::rendre_le_premier_plan(windows::Win32::Foundation::HWND(
            h as *mut core::ffi::c_void,
        ));
    }
    // `Release` : la boucle (son `Acquire`) voit tout ce qui précède.
    ouvert.ferme.store(true, Ordering::Release);
}

/// Le rectangle du menu ouvert, s'il est déjà placé. `try_lock` : appelée
/// depuis la boucle 60 Hz, qui ne doit jamais attendre un verrou.
pub fn rect_ouvert(app: &AppHandle) -> Option<(i32, i32, i32, i32)> {
    let etat = app.state::<EtatMenu>();
    let e = etat.0.try_lock().ok()?;
    e.as_ref()?.rect
}

/// Où poser la fenêtre, en pixels physiques : `(x, y, largeur, hauteur)`.
///
/// Le coin du MENU part du curseur ; la fenêtre commence une marge plus
/// haut et plus à gauche, pour l'ombre. S'il déborde à droite ou en bas, il
/// s'ouvre de l'autre côté du curseur ; et en dernier recours il est serré
/// contre le bord — il reste toujours entièrement dans l'écran.
///
/// `taille_css` est mesurée par le webview, en pixels CSS ; `echelle` est
/// celle de l'écran, qui les convertit (CLAUDE.md, piège n° 4 des
/// coordonnées).
pub fn rectangle(
    curseur: (i32, i32),
    taille_css: (f32, f32),
    echelle: f32,
    bornes: Rect,
) -> (i32, i32, u32, u32) {
    let e = echelle.max(0.01);
    let marge = (MARGE_CSS * e).round() as i32;
    let w = ((taille_css.0 + 2.0 * MARGE_CSS) * e).ceil() as i32;
    let h = ((taille_css.1 + 2.0 * MARGE_CSS) * e).ceil() as i32;

    let (gauche, haut) = (bornes.left() as i32, bornes.top() as i32);
    let (droite, bas) = (bornes.right() as i32, bornes.bottom() as i32);

    let mut x = curseur.0 - marge;
    if x + w > droite {
        // De l'autre côté : le bord droit du MENU sur le curseur.
        x = curseur.0 - w + marge;
    }
    let mut y = curseur.1 - marge;
    if y + h > bas {
        y = curseur.1 - h + marge;
    }

    // Le dernier recours : serré contre le bord. `max` puis `min` plutôt
    // que `clamp`, qui panique si le menu est plus grand que l'écran.
    x = x.min(droite - w).max(gauche);
    y = y.min(bas - h).max(haut);

    (x, y, w as u32, h as u32)
}

/// Vrai si `p` tombe hors de `rect` `(x, y, largeur, hauteur)`.
pub fn hors_du_menu(p: (i32, i32), rect: (i32, i32, i32, i32)) -> bool {
    let (x, y, w, h) = rect;
    p.0 < x || p.0 >= x + w || p.1 < y || p.1 >= y + h
}

#[cfg(test)]
#[path = "menu_fenetre_tests.rs"]
mod tests;
```

- [ ] **Step 5 : Vérifier les tests purs**

Dans `src-tauri/src/main.rs`, ajouter `mod menu_fenetre;` à côté de `mod menu_natif;`.
Run : `cargo test --quiet 2>&1 | Select-Object -Last 5`
Expected : `test result: ok.`, avec 6 tests de plus. `ouvrir`, `placer`, `fermer` et `rect_ouvert` sont encore inutilisés, d'où des avertissements de code mort qui disparaissent au step 8.

- [ ] **Step 6 : Les fichiers de l'interface**

Créer `ui/menu.html` :

```html
<!doctype html>
<!-- Le menu du clic droit. Un afficheur, rien d'autre : Rust dit quoi
     montrer (window.afficherMenu), et exécute ce qui est choisi. -->
<html lang="fr">
<head>
  <meta charset="utf-8">
  <link rel="stylesheet" href="menu.css">
</head>
<body>
  <div id="menu" role="menu"></div>
  <script src="menu.js"></script>
</body>
</html>
```

Créer `ui/menu.css` :

```css
/* Le style du menu de l'extension Shimeji pour navigateur : fond pêche,
   coins arrondis, ombre douce, lignes aérées, séparateurs fins.
   --marge DOIT valoir MARGE_CSS de menu_fenetre.rs : c'est la place
   laissée à l'ombre autour du menu. */
:root { --marge: 12px; }

html, body {
  margin: 0;
  background: transparent;
  overflow: hidden;
}

#menu {
  position: absolute;
  left: var(--marge);
  top: var(--marge);
  /* `max-content` : la largeur suit le texte, pas la fenêtre — qui est
     minuscule tant qu'on ne l'a pas encore mesurée. */
  width: max-content;
  min-width: 190px;
  max-width: 320px;
  overflow-y: auto;
  padding: 6px 0;
  background: linear-gradient(160deg, #fff4ee 0%, #fde9df 100%);
  border-radius: 8px;
  box-shadow: 0 3px 10px rgba(60, 30, 20, 0.28);
  font-family: "Segoe UI Variable Text", "Segoe UI", sans-serif;
  font-size: 14px;
  color: #1e1a18;
  user-select: none;
}

.entree {
  display: block;
  width: 100%;
  box-sizing: border-box;
  padding: 9px 22px 9px 34px;
  border: 0;
  background: none;
  font: inherit;
  color: inherit;
  text-align: left;
  cursor: pointer;
  position: relative;
}
.entree:hover { background: rgba(120, 70, 50, 0.09); }
.entree.coche::before {
  content: "✓";
  position: absolute;
  left: 13px;
  font-weight: 600;
}

.titre {
  padding: 10px 22px 4px;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: #8a6c5e;
}

hr {
  border: 0;
  height: 1px;
  margin: 6px 12px;
  background: linear-gradient(90deg, transparent, #6f5e57, transparent);
}

#menu::-webkit-scrollbar { width: 6px; }
#menu::-webkit-scrollbar-thumb { background: #7e706a; border-radius: 3px; }
```

Créer `ui/menu.js` :

```js
// Le menu du clic droit : dessine les lignes reçues de Rust, se mesure, et
// renvoie le choix. Aucune décision ici (spec « menu sur mesure » §4).

const invoke = window.__TAURI__.core.invoke;
const menu = document.getElementById('menu');

// Appelée par Rust (menu_fenetre::ouvrir) avec { lignes, hauteurMax }.
window.afficherMenu = function (charge) {
  menu.replaceChildren();
  menu.style.maxHeight = charge.hauteurMax + 'px';

  for (const ligne of charge.lignes) {
    if (ligne.type === 'Separateur') {
      menu.appendChild(document.createElement('hr'));
    } else if (ligne.type === 'Titre') {
      const t = document.createElement('div');
      t.className = 'titre';
      t.textContent = ligne.texte;
      menu.appendChild(t);
    } else {
      const b = document.createElement('button');
      b.className = ligne.coche ? 'entree coche' : 'entree';
      b.textContent = ligne.libelle;
      b.addEventListener('click', () => invoke('choisir_entree_menu', { id: ligne.id }));
      menu.appendChild(b);
    }
  }
  menu.scrollTop = 0;

  // getBoundingClientRect force la mise en page : la mesure est juste même
  // dans une fenêtre encore cachée, où requestAnimationFrame peut ne jamais
  // se déclencher.
  const r = menu.getBoundingClientRect();
  invoke('placer_menu', { largeur: r.width, hauteur: r.height });
};

// Un clic ailleurs retire le focus à la fenêtre : le menu se ferme.
window.addEventListener('blur', () => invoke('fermer_menu'));
window.addEventListener('keydown', (e) => {
  if (e.key === 'Escape') invoke('fermer_menu');
});
```

Créer `src-tauri/capabilities/menu.json` :

```json
{
  "identifier": "menu",
  "description": "Permet a la fenetre du menu du clic droit d'appeler Rust (placer_menu, choisir_entree_menu, fermer_menu). Portee a CETTE fenetre, comme celles du catalogue et de l'assistant.",
  "windows": ["menu"],
  "permissions": ["core:default"]
}
```

- [ ] **Step 7 : Les commandes, le style, l'enregistrement**

Dans `src-tauri/src/commandes.rs`, ajouter :

```rust
// ── Le menu du clic droit (spec « menu sur mesure » §4) ─────────────────

/// `menu.js` a dessiné et mesuré le menu : le placer et le montrer.
#[tauri::command]
pub fn placer_menu(app: tauri::AppHandle, largeur: f32, hauteur: f32) {
    crate::menu_fenetre::placer(&app, largeur, hauteur);
}

/// Une entrée a été choisie. Le menu se ferme D'ABORD — son personnage
/// repart, et le focus revient à l'application d'avant — puis le choix
/// passe par le même `executer` que le tray.
///
/// `id_connu` : on n'exécute que nos identifiants, jamais une chaîne
/// arbitraire venue d'un webview.
#[tauri::command]
pub fn choisir_entree_menu(
    app: tauri::AppHandle,
    actions: tauri::State<'_, std::sync::Arc<crate::actions::Actions>>,
    id: String,
) {
    crate::menu_fenetre::fermer(&app);
    match crate::menu_perso::id_connu(&id) {
        // `inner().clone()` : un `Arc` de plus sur les mêmes actions — la
        // méthode le consomme pour l'emporter sur le thread principal.
        Some(id) => actions.inner().clone().executer_choix_du_menu(app, id),
        None => eprintln!("menu : identifiant inconnu « {id} », ignoré"),
    }
}

/// Clic ailleurs ou Échap.
#[tauri::command]
pub fn fermer_menu(app: tauri::AppHandle) {
    crate::menu_fenetre::fermer(&app);
}
```

Dans `src-tauri/src/render.rs`, ajouter après `appliquer_styles_etendus` :

```rust
/// Pose `WS_EX_TOOLWINDOW` seul : hors d'Alt+Tab, mais activable.
///
/// Pour la fenêtre du menu, qui DOIT prendre le focus — c'est ce qui lui
/// permet de se fermer au clic ailleurs. `appliquer_styles_etendus`, qui
/// pose aussi `WS_EX_NOACTIVATE`, l'en empêcherait.
pub fn appliquer_style_outil(win: &WebviewWindow) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
    };
    let hwnd = win.hwnd().map_err(|e| format!("hwnd indisponible : {e}"))?;
    // `|` : on AJOUTE le bit sans écraser ceux que Tauri a posés — même
    // raisonnement que dans `appliquer_styles_etendus`.
    unsafe {
        let actuels = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, actuels | WS_EX_TOOLWINDOW.0 as isize);
    }
    Ok(())
}
```

Dans `src-tauri/src/main.rs` :
- ajouter `commandes::placer_menu, commandes::choisir_entree_menu, commandes::fermer_menu` à `tauri::generate_handler![…]` ;
- dans le `setup`, là où `app.manage(…)` enregistre `Actions` (`rg -n "\.manage\(" src-tauri/src/main.rs`), ajouter juste après :

```rust
            // L'état du menu du clic droit, partagé entre la boucle (qui
            // l'ouvre) et les commandes (qui le placent et le ferment).
            app.manage(menu_fenetre::EtatMenu::default());
```

- dans le `setup`, juste après l'appel à `tray::installer` :

```rust
            // La fenêtre du menu, créée MAINTENANT et cachée : la créer au
            // premier clic droit gèlerait le thread principal (voir l'en-tête
            // de `menu_fenetre.rs`). Non bloquante : sans elle, le clic droit
            // ne fait rien, mais le personnage vit.
            if let Err(e) = menu_fenetre::creer(app.handle()) {
                eprintln!("{e}");
            }
```

- [ ] **Step 8 : Brancher la boucle, retirer le menu natif**

Dans `src-tauri/src/main.rs`, fonction `boucle`, remplacer tout le bloc `// ── Le menu, sur son propre thread ──` (du `let ferme = …` jusqu'au `});` du `std::thread::spawn`) par :

```rust
                // ── Le menu, dans sa fenêtre ────────────────────────────
                //
                // `ouvrir` ne bloque pas : il envoie les lignes à la fenêtre
                // du menu et rend la main. La fermeture, d'où qu'elle vienne,
                // lèvera `ferme` — et seul ce personnage attend.
                let ferme = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
                let curseur = (m.pos.x.round() as i32, m.pos.y.round() as i32);
                // `ecran_sous` rend un id ; on retrouve l'écran lui-même pour
                // ses bornes et son échelle. Sans écran sous le curseur, pas
                // de menu : on le dit fermé tout de suite.
                let ecran = ecran_sous(&ecrans_courants, m.pos)
                    .and_then(|id| ecrans_courants.iter().find(|e| e.id == id));
                match ecran {
                    Some(ecran) => menu_fenetre::ouvrir(&handle, &lignes, curseur, ecran, ferme.clone()),
                    None => ferme.store(true, std::sync::atomic::Ordering::Release),
                }
```

Juste **après** le bloc qui remet `menu_en_cours` à `None` quand `ferme` est levé, ajouter le filet :

```rust
        // ── Le filet du « clic ailleurs » (Review Focus n° 3) ───────────
        //
        // `blur` ferme le menu quand on clique ailleurs — si Windows lui a
        // donné le premier plan. Quand il l'a refusé, `blur` ne vient
        // jamais : un bouton enfoncé hors du rectangle du menu le ferme.
        if menu_en_cours.is_some() && (m.left_down || m.right_down) {
            if let Some(rect) = menu_fenetre::rect_ouvert(&handle) {
                let p = (m.pos.x.round() as i32, m.pos.y.round() as i32);
                if menu_fenetre::hors_du_menu(p, rect) {
                    menu_fenetre::fermer(&handle);
                }
            }
        }
```

⚠️ `m` (la souris) doit déjà être lue à cet endroit. Si le bloc `menu_en_cours` précède `let m = sonde.mouse();`, placer le filet juste après `let m = …`.

Supprimer `src-tauri/src/menu_natif.rs` et la ligne `mod menu_natif;`. Dans `render.rs`, supprimer `reveiller_la_file`, qui n'a plus d'appelant. Mettre à jour les commentaires de `render::prendre_le_premier_plan` et de `menu_en_cours` (dans `boucle`) qui citent `menu_natif`.

- [ ] **Step 9 : L'équivalent scriptable**

Dans `boucle`, à côté de `bouton_droit_precedent`, ajouter :

```rust
    // `SHIMEJI_MENU_OUVERT=1` : ouvre le menu du premier personnage dès qu'il
    // est posé — l'équivalent scriptable du clic droit (CLAUDE.md, « tout ce
    // qui demanderait un clic reçoit un équivalent scriptable »).
    let mut menu_de_demonstration = std::env::var_os("SHIMEJI_MENU_OUVERT").is_some();
```

La condition du clic droit devient :

```rust
            let demonstration = menu_de_demonstration
                && i == 0
                && matches!(acteur.ch.attachment, character::attach::Attachment::On { .. });
            if (front_descendant_droit && sur_le_personnage || demonstration)
                && menu_en_cours.is_none()
            {
                menu_de_demonstration = false;
```

Dans ce bloc, pour la démonstration, le curseur est le personnage lui-même :

```rust
                let point = if demonstration { acteur.ch.pos_connue } else { m.pos };
```

et `point` remplace `m.pos` dans le calcul de `curseur` et dans `ecran_sous`.

- [ ] **Step 10 : Vérifier**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 5` puis `cargo build 2>&1 | Select-Object -Last 20`
Expected : tests OK, compilation sans nouvel avertissement.

Puis :

```powershell
$env:SHIMEJI_MENU='1'; $env:SHIMEJI_MENU_OUVERT='1'; $env:SHIMEJI_QUITTER_APRES='15'
cargo run 2>&1 | Select-String -Pattern "menu"
```

Expected : une ligne `menu placé : W×H @ (x, y)`. Elle prouve que les lignes arrivent au webview, qu'il se mesure, et que l'IPC répond. Si elle manque : `capabilities/menu.json`, puis `generate_handler!`.

- [ ] **Step 11 : Commit**

```bash
git add -A src-tauri/src/menu_fenetre.rs src-tauri/src/menu_fenetre_tests.rs ui/menu.html ui/menu.css ui/menu.js src-tauri/capabilities/menu.json src-tauri/src/menu_perso.rs src-tauri/src/menu_perso_tests.rs src-tauri/src/commandes.rs src-tauri/src/render.rs src-tauri/src/main.rs src-tauri/src/menu_natif.rs
git commit -m "feat(menu): le menu du clic droit dans une fenetre webview sur mesure"
```

---

### Task 6 : Vérification à l'écran, mesure, et CLAUDE.md

**Files :**
- Modify : `CLAUDE.md`

- [ ] **Step 1 : L'auteur juge le rendu**

`cargo build`, puis `cargo run`. Demander à l'auteur de vérifier, sur ses trois écrans dont celui à 125 % :
- le style, comparé à la capture de l'extension ;
- le menu près du bas et du bord droit d'un écran ;
- la fermeture au clic ailleurs et avec Échap ;
- que les autres personnages continuent de vivre menu ouvert ;
- les coches : S'asseoir, décocher, « Tout le monde › S'asseoir ».

Corriger d'après ses retours. Le CSS se retouche sans recompiler Rust, mais il faut relancer l'application.

- [ ] **Step 2 : Mesurer le CPU**

La fenêtre cachée ne doit rien coûter :

```powershell
docs/outils/mesurer-overlay.ps1 -N 15 -Cache
```

Comparer aux **0,8 %** de CLAUDE.md (« 15 personnages, caché »). Au-delà de 1 %, chercher ce que la fenêtre du menu fait quand elle est cachée, avant d'aller plus loin.

- [ ] **Step 3 : CLAUDE.md**

- Section « Toute nouvelle action se branche au menu » : une ligne d'`ENVIES` porte une `Commande::Basculer(Tenue)` si l'action dure, une `Commande::Intention` si elle est ponctuelle, et la section « Tout le monde » a sa table `TOUS`.
- Le tableau « Où il est / Le menu propose » : « Monter plus haut » disparaît, « Grimper au mur » apparaît au mur et au plafond.
- Le bloc « Le menu du clic droit n'est plus un menu Tauri » : remplacer `menu_natif.rs` par `menu_fenetre.rs` + `ui/menu.*`, et dire pourquoi (le style, et aucune boucle modale).
- Le tableau des variables de diagnostic : ajouter `SHIMEJI_MENU_OUVERT=1`, et passer « Quatorze » à « Quinze ».
- Le tableau « Où » des documents : ajouter la spec et ce plan.

- [ ] **Step 4 : Commit**

```bash
git add CLAUDE.md
git commit -m "docs: le menu sur mesure et les actions tenues dans CLAUDE.md"
```
