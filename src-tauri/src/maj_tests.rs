//! Tests de `maj.rs` : les libellés de l'entrée du tray, et « vient-on
//! d'être mis à jour ? ». Le réseau et le plugin n'y apparaissent pas.

use super::*;

#[test]
fn chaque_etat_a_son_libelle() {
    assert_eq!(libelle(&EtatMaj::Repos), "Vérifier les mises à jour…");
    assert_eq!(libelle(&EtatMaj::Disponible("0.4.0".into())), "Mettre à jour vers la v0.4.0");
    assert_eq!(libelle(&EtatMaj::AJour("0.3.0".into())), "À jour (v0.3.0)");
    assert_eq!(libelle(&EtatMaj::Impossible), "Vérification impossible — réessayer");
}

#[test]
fn une_version_changee_depuis_le_dernier_lancement_est_une_mise_a_jour() {
    assert!(vient_d_etre_mis_a_jour("0.3.0", "0.4.0"));
}

#[test]
fn meme_version_pas_de_mise_a_jour() {
    assert!(!vient_d_etre_mis_a_jour("0.4.0", "0.4.0"));
}

#[test]
fn sans_version_retenue_on_ne_sait_pas_donc_on_ne_dit_rien() {
    // Tout premier lancement (l'assistant a son propre toast), ou version
    // antérieure à cette clé : on ne peut rien affirmer.
    assert!(!vient_d_etre_mis_a_jour("", "0.4.0"));
}
