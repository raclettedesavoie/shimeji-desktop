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
        let ai = intention_pour(tenue, &e, &r, &t, &m, false, Duration::from_secs(1));
        assert_eq!(ai.kind, tenue.intention(), "{tenue:?}");
    }
}

#[test]
fn utilisateur_parti_une_tenue_de_sol_s_assoupit() {
    // ×8 : ce que produit « inactif depuis 2 min » (décision n° 3).
    let (r, t, m) = (reglages(), TableEnvies::defaut(), blob());
    let e = entrees(false, 8.0);
    for tenue in [Tenue::Asseoir, Tenue::BalancerLesJambes, Tenue::Flaner] {
        let ai = intention_pour(tenue, &e, &r, &t, &m, false, Duration::from_secs(1));
        assert_eq!(ai.kind, Intention::SeReposer, "{tenue:?}");
    }
}

#[test]
fn au_mur_l_absence_ne_change_rien() {
    // Décision de l'auteur : au mur, rien n'interrompt une action tenue.
    let (r, t, m) = (reglages(), TableEnvies::defaut(), blob());
    let e = entrees(false, 8.0);
    let ai = intention_pour(Tenue::Grimper, &e, &r, &t, &m, true, Duration::from_secs(1));
    assert_eq!(ai.kind, Intention::Grimper);
    let ai = intention_pour(Tenue::ResterAccroche, &e, &r, &t, &m, true, Duration::from_secs(1));
    assert!(matches!(ai.etat, EtatIntention::Grimpe { phase: PhaseGrimpe::Accroche, .. }));
}

#[test]
fn rester_accroche_au_sol_part_d_abord_au_mur() {
    let (r, t, m) = (reglages(), TableEnvies::defaut(), blob());
    let ai = intention_pour(Tenue::ResterAccroche, &entrees(true, 1.0), &r, &t, &m, false, Duration::from_secs(1));
    assert!(matches!(ai.etat, EtatIntention::Grimpe { phase: PhaseGrimpe::Choisir { presse: true }, .. }));
}
