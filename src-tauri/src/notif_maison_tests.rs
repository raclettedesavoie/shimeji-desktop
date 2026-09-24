//! Tests de `notif_maison.rs` : où se pose la fenêtre.

use super::*;

#[test]
fn elle_se_pose_dans_le_coin_bas_droit_de_la_zone_de_travail() {
    // Un écran 1920×1080 dont la barre des tâches prend 48 px en bas.
    let (x, y) = coin_bas_droit((0, 0, 1920, 1032), (400, 120), 8);
    assert_eq!((x, y), (1920 - 400 - 8, 1032 - 120 - 8));
}

#[test]
fn elle_suit_un_ecran_qui_ne_commence_pas_en_zero() {
    // Écran principal à droite d'un autre : ses coordonnées commencent loin.
    let (x, y) = coin_bas_droit((2560, 100, 1920, 1000), (400, 120), 8);
    assert_eq!((x, y), (2560 + 1920 - 400 - 8, 100 + 1000 - 120 - 8));
}
