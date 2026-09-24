//! Tests de `ne_pas_deranger.rs` : la lecture des octets de l'état WNF.
//! Les valeurs viennent de la mesure du 2026-09-24 sur la machine de l'auteur.

use super::*;

#[test]
fn zero_veut_dire_desactive() {
    assert!(!interpreter(&[0, 0, 0, 0]));
}

#[test]
fn un_veut_dire_active() {
    assert!(interpreter(&[1, 0, 0, 0]));
}

#[test]
fn un_autre_profil_compte_aussi_comme_active() {
    // Les profils « priorité seulement » et « alarmes seulement » de
    // Windows 10 avaient d'autres valeurs : tout ce qui n'est pas 0 coupe
    // les bannières.
    assert!(interpreter(&[2, 0, 0, 0]));
}

#[test]
fn des_octets_manquants_ne_font_pas_croire_au_mode() {
    assert!(!interpreter(&[]));
    assert!(!interpreter(&[1, 0]));
}
