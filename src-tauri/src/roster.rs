//! La réconciliation du roster : ce qu'il faut faire pour que les
//! personnages présents à l'écran correspondent à la liste voulue.
//!
//! Responsabilité unique, et **fonction pure** : ce module ne connaît ni
//! Tauri, ni le disque, ni l'écran. C'est ce qui rend l'étape « plusieurs
//! personnages » entièrement testable sans écran
//! (design `2026-09-15-plusieurs-personnages` §4).

use std::collections::BTreeMap;

/// Ce qu'il faut faire d'un nom pour rapprocher le présent du voulu.
///
/// `PartialEq` et `Debug` : les tests comparent des `Vec<ActionRoster>`
/// entiers, ce qui donne des messages d'échec lisibles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionRoster {
    /// Il manque un exemplaire de ce personnage : en créer un.
    Creer(String),
    /// Il y en a un de trop : en retirer un. **Lequel** est la décision de
    /// l'appelant, qui seul connaît l'ordre d'arrivée — voir la boucle,
    /// qui retire le plus récemment ajouté.
    RetirerUn(String),
}

/// Combien d'exemplaires de `nom` dans cette liste.
///
/// C'est **la seule définition du compteur** affiché par la bibliothèque :
/// `config.personnages` est un multi-ensemble depuis cette étape, et le
/// compteur en est le nombre d'occurrences (design §4). Aucune autre source
/// de vérité — un second compteur stocké à côté finirait par la contredire.
pub fn compte_de(liste: &[String], nom: &str) -> usize {
    liste.iter().filter(|n| n.as_str() == nom).count()
}

/// Ce qu'il faut faire pour passer de `presents` à `voulus`.
///
/// **L'ordre des deux listes n'a aucune importance** : ce sont des
/// multi-ensembles, pas des séquences. Deux blob et un luffy décrivent le
/// même écran quel que soit l'ordre d'écriture dans `config.json`.
///
/// Les **retraits sont rendus avant les créations**. À nombre constant de
/// personnages — remplacer zoro par luffy — l'ordre inverse ferait exister
/// brièvement un personnage de plus, donc une fenêtre en couche de plus à
/// déplacer. Le coût CPU est proportionnel au nombre de déplacements
/// (design §2), et ce pic n'a aucune raison d'être payé.
pub fn reconcilier(presents: &[String], voulus: &[String]) -> Vec<ActionRoster> {
    // `BTreeMap` et non `HashMap` : il range par ordre alphabétique, donc
    // deux appels avec les mêmes listes rendent EXACTEMENT le même `Vec`.
    // Un `HashMap` rendrait un ordre différent à chaque exécution, et les
    // tests seraient intermittents — le pire genre de test.
    //
    // La valeur est le SOLDE : négatif = il y en a trop, positif = il en
    // manque. Une seule table pour les deux sens, plutôt que deux tables à
    // tenir d'accord.
    let mut solde: BTreeMap<&str, i32> = BTreeMap::new();

    for n in presents {
        *solde.entry(n.as_str()).or_insert(0) -= 1;
    }
    for n in voulus {
        *solde.entry(n.as_str()).or_insert(0) += 1;
    }

    // Deux passes, et c'est ce qui garantit que TOUS les retraits précèdent
    // TOUTES les créations : une seule passe les entrelacerait par ordre
    // alphabétique.
    let mut actions = Vec::new();

    for (nom, delta) in &solde {
        // `delta` négatif : il y en a plus de présents que de voulus.
        // `max(0)` après la négation plutôt qu'un `if` : la boucle ne
        // s'exécute simplement pas quand il n'y a rien à retirer.
        for _ in 0..(-*delta).max(0) {
            actions.push(ActionRoster::RetirerUn(nom.to_string()));
        }
    }
    for (nom, delta) in &solde {
        for _ in 0..(*delta).max(0) {
            actions.push(ActionRoster::Creer(nom.to_string()));
        }
    }

    actions
}

// Les tests de ce module vivent dans `roster_tests.rs`.
#[cfg(test)]
#[path = "roster_tests.rs"]
mod tests;
