//! Les commandes appelées par la fenêtre du catalogue (spec §9).
//!
//! Responsabilité unique : exposer trois gestes au webview. Toute la logique
//! est ailleurs — ici on ne fait que traduire un appel JS en appel Rust.
//!
//! ⚠️ Ce module est le SEUL du projet à utiliser l'IPC de Tauri. Le
//! personnage, lui, passe par `eval` (voir `render.rs`), qui ne va que de
//! Rust vers JS et n'exige aucune permission. Le catalogue, devant appeler
//! Rust, a besoin de l'IPC — donc d'une capacité, déclarée dans
//! `capabilities/catalogue.json` et **portée à cette seule fenêtre**.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Un pack présent sur le disque, tel que la fenêtre a besoin de le connaître.
///
/// `Serialize` et non `Deserialize` : la donnée ne circule que dans un sens,
/// de Rust vers le webview.
#[derive(Serialize)]
pub struct PackInstalle {
    pub nom: String,
    /// Est-ce celui qui s'affiche en ce moment ?
    pub actif: bool,
}

/// Installe un pack depuis le catalogue.
///
/// `async` parce que 46 téléchargements prennent environ 3 s : une commande
/// bloquante figerait la fenêtre du catalogue pendant tout ce temps
/// (spec §9).
///
/// L'événement `installation` porte la progression. Un seul événement, une
/// seule forme — `{slug, fait, total}`.
#[tauri::command]
pub async fn installer(app: AppHandle, slug: String) -> Result<(), String> {
    // `spawn_blocking` : le téléchargement est du travail BLOQUANT (WinHttp
    // n'est pas asynchrone). Le laisser sur l'exécuteur async figerait les
    // autres commandes.
    //
    // Les deux `clone` sont indispensables : la closure est `move`, donc
    // elle PREND ce qu'elle utilise. Sans eux, `slug` partirait dans la
    // tâche et le message d'erreur final ne pourrait plus le nommer.
    let slug_pour_tache = slug.clone();
    let app_pour_tache = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        crate::catalogue::installer(&slug_pour_tache, &mut |fait, total| {
            // L'échec d'émission est ignoré : la fenêtre a pu être fermée
            // pendant le téléchargement, ce qui n'est pas une raison
            // d'interrompre celui-ci.
            let _ = app_pour_tache.emit(
                "installation",
                serde_json::json!({ "slug": slug_pour_tache, "fait": fait, "total": total }),
            );
        })
    })
    .await
    // Deux erreurs différentes se succèdent ici : la tâche a pu être
    // annulée (le premier `?`), puis l'installation a pu échouer (le
    // `map` porte sur son Result à elle).
    .map_err(|e| format!("tâche d'installation interrompue : {e}"))?
    .map(|_chemin| ())
}

/// Ce que contient la bibliothèque, plus le dossier livré.
///
/// Les deux racines sont listées, et la bibliothèque gagne en cas d'homonyme
/// — le même ordre qu'à la résolution (spec §5), sans quoi la liste
/// mentirait sur ce qui s'afficherait réellement.
#[tauri::command]
pub fn bibliotheque() -> Vec<PackInstalle> {
    let actif = crate::config::charger()
        .personnages
        .first()
        .cloned()
        .unwrap_or_else(|| "blob".to_string());

    // Un `BTreeSet` : il dédoublonne les homonymes des deux racines ET
    // range par ordre alphabétique, ce qui donne une liste stable d'un
    // affichage à l'autre. Un `HashSet` rendrait un ordre différent à
    // chaque ouverture de la fenêtre.
    let mut noms: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    // `into_iter().flatten()` : on parcourt les racines existantes et on
    // ignore silencieusement celles qui manquent — une bibliothèque vide
    // n'est pas une erreur. `flatten` sur un itérateur d'`Option` laisse
    // tomber les `None`.
    let racines = [
        crate::config::dossier_bibliotheque(),
        Some(crate::config::dossier_personnages()),
    ];
    for racine in racines.into_iter().flatten() {
        let Ok(entrees) = std::fs::read_dir(&racine) else {
            continue;
        };
        for entree in entrees.flatten() {
            // Un personnage est un dossier contenant un mascot.json. Ce test
            // écarte d'un coup les dossiers `.partiel` d'une installation
            // interrompue et tout fichier égaré.
            if entree.path().join("mascot.json").is_file() {
                if let Some(nom) = entree.file_name().to_str() {
                    noms.insert(nom.to_string());
                }
            }
        }
    }

    noms.into_iter()
        .map(|nom| PackInstalle {
            actif: nom == actif,
            nom,
        })
        .collect()
}

/// Affiche ce personnage — le SECOND geste (décision de cadrage n° 2).
///
/// Le changement est **immédiat**, sans redémarrage : `changer_personnage`
/// dépose un rechargement que la boucle 60 Hz ramasse à l'image suivante.
///
/// `State<…>` : Tauri injecte ici ce que `main.rs` a confié à `manage`. Le
/// type demandé doit correspondre **exactement** à celui qui a été confié
/// (`Arc<Actions>`), sinon la commande échoue à l'exécution et non à la
/// compilation.
#[tauri::command]
pub fn choisir(
    actions: tauri::State<'_, std::sync::Arc<crate::actions::Actions>>,
    nom: String,
) -> Result<(), String> {
    // On refuse un personnage introuvable AVANT tout le reste : sinon
    // l'application ne redémarrerait plus, le chargement échouant sur un nom
    // qui ne résout pas.
    let Some(dossier) = crate::config::dossier_du_personnage(&nom) else {
        return Err(format!("personnage « {nom} » introuvable"));
    };

    // L'ordre compte : on charge D'ABORD, on enregistre ENSUITE. Si le
    // manifeste est illisible, rien n'a changé — ni à l'écran, ni dans la
    // config, qui aurait sinon nommé un personnage qui ne charge pas.
    actions.changer_personnage(&nom, dossier)?;
    crate::config::definir_personnages(&[nom.clone()])?;

    println!("personnage choisi : {nom}");
    Ok(())
}
