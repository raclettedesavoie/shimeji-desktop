//! Les tests de `roster` — la réconciliation « présents vs voulus ».
use super::*;

/// Un raccourci : les tests manipulent des listes de noms, et
/// `vec!["blob"]` ne se convertit pas tout seul en `Vec<String>`.
fn noms(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn rien_a_faire_quand_les_deux_listes_concordent() {
    let actions = reconcilier(&noms(&["blob", "luffy"]), &noms(&["blob", "luffy"]));
    assert!(actions.is_empty());
}

#[test]
fn l_ordre_des_listes_n_a_aucune_importance() {
    // Un multi-ensemble n'est pas une liste ordonnée : « blob puis luffy »
    // et « luffy puis blob » décrivent le même écran.
    let actions = reconcilier(&noms(&["blob", "luffy"]), &noms(&["luffy", "blob"]));
    assert!(actions.is_empty());
}

#[test]
fn un_nom_voulu_et_absent_se_cree() {
    let actions = reconcilier(&noms(&[]), &noms(&["blob"]));
    assert_eq!(actions, vec![ActionRoster::Creer("blob".to_string())]);
}

#[test]
fn un_nom_present_et_non_voulu_se_retire() {
    let actions = reconcilier(&noms(&["blob"]), &noms(&[]));
    assert_eq!(actions, vec![ActionRoster::RetirerUn("blob".to_string())]);
}

#[test]
fn les_doublons_sont_comptes_et_non_dedoublonnes() {
    // Le cœur du multi-ensemble : deux blob voulus quand un seul existe
    // donne UNE création, pas zéro.
    let actions = reconcilier(&noms(&["blob"]), &noms(&["blob", "blob"]));
    assert_eq!(actions, vec![ActionRoster::Creer("blob".to_string())]);
}

#[test]
fn trois_voulus_pour_un_present_donnent_deux_creations() {
    let actions = reconcilier(&noms(&["blob"]), &noms(&["blob", "blob", "blob"]));
    assert_eq!(
        actions,
        vec![
            ActionRoster::Creer("blob".to_string()),
            ActionRoster::Creer("blob".to_string()),
        ]
    );
}

#[test]
fn un_de_trois_se_retire_une_seule_fois() {
    let actions = reconcilier(&noms(&["blob", "blob", "blob"]), &noms(&["blob", "blob"]));
    assert_eq!(actions, vec![ActionRoster::RetirerUn("blob".to_string())]);
}

#[test]
fn creations_et_retraits_coexistent_dans_une_meme_reconciliation() {
    let actions = reconcilier(&noms(&["blob", "zoro"]), &noms(&["blob", "luffy"]));
    // Les retraits AVANT les créations : à nombre constant de personnages,
    // on ne veut pas de pic transitoire — le coût CPU est proportionnel au
    // nombre de fenêtres déplacées (design §2).
    assert_eq!(
        actions,
        vec![
            ActionRoster::RetirerUn("zoro".to_string()),
            ActionRoster::Creer("luffy".to_string()),
        ]
    );
}

#[test]
fn tout_retirer_est_permis() {
    // La liste vide est un état NORMAL, pas une erreur (design §4).
    let actions = reconcilier(&noms(&["blob", "luffy"]), &noms(&[]));
    assert_eq!(actions.len(), 2);
    assert!(actions.contains(&ActionRoster::RetirerUn("blob".to_string())));
    assert!(actions.contains(&ActionRoster::RetirerUn("luffy".to_string())));
}

#[test]
fn deux_appels_identiques_rendent_exactement_la_meme_liste() {
    // C'est ce que garantit le `BTreeMap`, et ce qu'un `HashMap` ne
    // garantirait pas : sans ordre stable, les tests seraient
    // intermittents — le pire genre de test.
    let presents = noms(&["blob", "zoro", "blob"]);
    let voulus = noms(&["luffy", "pierrot", "blob"]);
    assert_eq!(
        reconcilier(&presents, &voulus),
        reconcilier(&presents, &voulus)
    );
}

#[test]
fn compte_de_compte_les_occurrences() {
    let l = noms(&["blob", "luffy", "blob"]);
    assert_eq!(compte_de(&l, "blob"), 2);
    assert_eq!(compte_de(&l, "luffy"), 1);
    assert_eq!(compte_de(&l, "zoro"), 0);
}
