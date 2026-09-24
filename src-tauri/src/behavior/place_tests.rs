//! Tests de `place.rs` — des cas écrits à la main, sans monde ni boucle.

use super::*;
use crate::world::{PlatformId, RoleEcran};
use std::time::Duration;

fn sol() -> PlatformId {
    PlatformId::ecran(1, RoleEcran::Sol)
}
fn mur() -> PlatformId {
    PlatformId::ecran(1, RoleEcran::MurGauche)
}

/// Un occupant assis sur le sol, arrêté depuis `depuis` secondes.
fn assis(acteur: u32, offset: f32, depuis: u64) -> Occupant {
    Occupant {
        acteur,
        platform: sol(),
        face: Face::Top,
        offset,
        demi_largeur: 45.0,
        arrete_depuis: Some(Duration::from_secs(depuis)),
        attend_le_mur: None,
    }
}

#[test]
fn une_place_libre_est_rendue_telle_quelle() {
    let autres = [assis(1, 100.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    assert_eq!(place_libre(&v, sol(), 500.0, 45.0, 1920.0), Some(500.0));
}

#[test]
fn sur_une_place_prise_il_va_a_cote_du_cote_le_plus_proche() {
    // Assis à 500 (couvre 455..545). Moi à 510 : la place libre la plus
    // proche est collée à droite, 545 + 45 = 590 (côte à côte).
    let autres = [assis(1, 500.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    assert_eq!(place_libre(&v, sol(), 510.0, 45.0, 1920.0), Some(590.0));
    // Moi à 490 : collé à gauche, 455 − 45 = 410.
    assert_eq!(place_libre(&v, sol(), 490.0, 45.0, 1920.0), Some(410.0));
}

#[test]
fn il_saute_par_dessus_une_rangee_entiere() {
    let autres = [assis(1, 500.0, 1), assis(2, 590.0, 1), assis(3, 680.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    // Au milieu de la rangée (590) : 410 et 770 sont à égale distance
    // (180) ; on garde le premier trouvé — peu importe, pourvu que ce soit
    // hors de la rangée.
    let p = place_libre(&v, sol(), 600.0, 45.0, 1920.0).unwrap();
    assert!(p <= 410.0 || p >= 770.0, "{p}");
}

#[test]
fn un_marcheur_n_occupe_rien() {
    let mut o = assis(1, 500.0, 1);
    o.arrete_depuis = None;
    let autres = [o];
    let v = Voisinage { moi: 9, autres: &autres };
    assert_eq!(place_libre(&v, sol(), 500.0, 45.0, 1920.0), Some(500.0));
}

#[test]
fn il_s_ignore_lui_meme() {
    let autres = [assis(9, 500.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    assert_eq!(place_libre(&v, sol(), 500.0, 45.0, 1920.0), Some(500.0));
}

#[test]
fn le_bord_du_sol_borne_la_place() {
    // Assis à 45 (couvre 0..90). Moi à 20 : à gauche il n'y a plus de sol,
    // la place est à droite, 135.
    let autres = [assis(1, 45.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    assert_eq!(place_libre(&v, sol(), 20.0, 45.0, 1920.0), Some(135.0));
}

#[test]
fn ecran_plein_aucune_place() {
    // Un sol de 200 px, déjà occupé au milieu : aucune place de 90 px.
    let autres = [assis(1, 100.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    assert_eq!(place_libre(&v, sol(), 100.0, 45.0, 200.0), None);
}

#[test]
fn le_dernier_arrive_cede() {
    let autres = [assis(1, 500.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    // Moi arrêté à 3 s, lui à 1 s : c'est moi qui cède.
    assert!(doit_ceder(&v, sol(), 510.0, 45.0, Some(Duration::from_secs(3))));
    // Moi arrêté à 0 s : je suis le plus ancien, je reste.
    assert!(!doit_ceder(&v, sol(), 510.0, 45.0, Some(Duration::from_secs(0))));
}

#[test]
fn egalite_le_plus_grand_numero_cede() {
    let autres = [assis(1, 500.0, 2)];
    let v9 = Voisinage { moi: 9, autres: &autres };
    assert!(doit_ceder(&v9, sol(), 510.0, 45.0, Some(Duration::from_secs(2))));
    let autres = [assis(9, 510.0, 2)];
    let v1 = Voisinage { moi: 1, autres: &autres };
    assert!(!doit_ceder(&v1, sol(), 500.0, 45.0, Some(Duration::from_secs(2))));
}

#[test]
fn cote_a_cote_n_est_pas_un_chevauchement() {
    let autres = [assis(1, 500.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    assert!(!doit_ceder(&v, sol(), 590.0, 45.0, Some(Duration::from_secs(3))));
}

#[test]
fn le_bas_du_mur_est_pris_tant_que_le_grimpeur_n_a_pas_monte_d_un_corps() {
    // Mur de 1000 px, offset compté vers le BAS depuis le haut : le bas est
    // à 1000. Hauteur de corps 100 : la zone de départ est 900..1000.
    let grimpeur = |offset| Occupant {
        acteur: 1,
        platform: mur(),
        face: Face::Right,
        offset,
        demi_largeur: 45.0,
        arrete_depuis: None, // en mouvement : compte quand même sur le mur
        attend_le_mur: None,
    };
    let autres = [grimpeur(950.0)];
    assert!(!bas_du_mur_libre(&Voisinage { moi: 9, autres: &autres }, mur(), 1000.0, 100.0));
    let autres = [grimpeur(880.0)];
    assert!(bas_du_mur_libre(&Voisinage { moi: 9, autres: &autres }, mur(), 1000.0, 100.0));
}

#[test]
fn seul_le_plus_proche_du_mur_part_le_premier() {
    let en_file = |acteur, offset| Occupant {
        attend_le_mur: Some(mur()),
        ..assis(acteur, offset, 1)
    };
    // Pied du mur à 0. Lui à 45, moi à 135 : il passe avant moi.
    let autres = [en_file(1, 45.0)];
    let v = Voisinage { moi: 9, autres: &autres };
    assert!(!premier_de_la_file(&v, mur(), 0.0, 135.0));
    // Seul en file : je suis le premier.
    let v = Voisinage { moi: 9, autres: &[] };
    assert!(premier_de_la_file(&v, mur(), 0.0, 135.0));
}
