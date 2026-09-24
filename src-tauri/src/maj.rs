//! La mise à jour automatique (spec « mise à jour » du 2026-09-21).
//!
//! Responsabilité unique : demander à GitHub s'il existe une version plus
//! récente, et l'installer quand l'utilisateur le demande. Aucune décision
//! d'interface ici — le libellé du menu est posé par `Actions`, le toast par
//! `toast.rs`.
//!
//! # Ce que le plugin fait, et ce qu'il ne fait pas
//!
//! `tauri-plugin-updater` télécharge le `latest.json` publié avec la release,
//! **vérifie sa signature minisign** contre la clé publique de
//! `tauri.conf.json`, puis télécharge et lance l'installateur. Un fichier
//! dont la signature ne correspond pas est refusé : c'est ce qui empêche
//! quelqu'un capable de répondre à la place de GitHub de faire installer son
//! propre exécutable.
//!
//! Il ne fait **pas** de retour arrière, et ne sait pas installer une version
//! plus ancienne. C'est assumé (hors périmètre).

use crate::actions::Actions;
use std::sync::Arc;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

/// Ce que dit l'entrée du tray. **Le libellé EST l'état** : rien d'autre ne
/// le stocke (voir `Actions::afficher_etat_maj`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EtatMaj {
    /// Rien de vérifié à la main : un clic lance la vérification.
    Repos,
    /// Une version plus récente existe : un clic l'installe.
    Disponible(String),
    /// Vérifié à la main, rien de plus récent (demande de l'auteur,
    /// 2026-09-24 — un clic sans retour ressemblait à un clic sans effet).
    AJour(String),
    /// Réseau coupé, GitHub injoignable… Un clic réessaie.
    Impossible,
}

/// Le libellé de l'entrée du tray pour cet état.
pub fn libelle(etat: &EtatMaj) -> String {
    match etat {
        EtatMaj::Repos => "Vérifier les mises à jour…".to_string(),
        EtatMaj::Disponible(v) => format!("Mettre à jour vers la v{v}"),
        EtatMaj::AJour(v) => format!("À jour (v{v})"),
        EtatMaj::Impossible => "Vérification impossible — réessayer".to_string(),
    }
}

/// Une mise à jour vient-elle de s'installer ? Vrai si la version retenue
/// au dernier lancement existe et diffère de la courante.
///
/// Rien de retenu (`""`) : tout premier lancement — l'assistant a son propre
/// toast —, ou lancement d'une version antérieure à la clé. On ne peut alors
/// rien affirmer, et l'on se tait.
pub fn vient_d_etre_mis_a_jour(derniere_lancee: &str, courante: &str) -> bool {
    !derniere_lancee.is_empty() && derniere_lancee != courante
}

/// Au démarrage, en RELEASE seulement : annonce la mise à jour qui vient de
/// s'installer, puis retient la version de ce lancement. Voir
/// `config::derniere_version_lancee` pour la raison du « release seulement ».
#[cfg(not(debug_assertions))]
pub fn constater_au_demarrage(app: &AppHandle, derniere_lancee: &str) {
    let courante = app.package_info().version.to_string();
    if vient_d_etre_mis_a_jour(derniere_lancee, &courante) {
        crate::toast::mis_a_jour(app, &courante);
    }
    if derniere_lancee != courante {
        if let Err(e) = crate::config::definir_version_lancee(&courante) {
            eprintln!("[maj] version lancée non enregistrée : {e}");
        }
    }
}

/// Vérifie, sans bloquer, s'il existe une version plus récente.
///
/// Appelée une fois au démarrage. **Jamais depuis la boucle 60 Hz** : un
/// appel réseau y coûterait des dizaines d'images perdues. `spawn` la met sur
/// l'exécuteur asynchrone de Tauri, qui tourne sur ses propres threads.
///
/// Silencieuse par construction : si le réseau est coupé, si GitHub répond
/// une erreur, ou si aucune version n'est plus récente, **il ne se passe
/// rien** — aucune fenêtre, aucun message, et l'entrée du menu garde son
/// libellé de repos.
pub fn verifier_en_arriere_plan(app: AppHandle, actions: Arc<Actions>) {
    // `SHIMEJI_MAJ=1` trace le résultat. Sans elle, une vérification qui
    // échoue est parfaitement muette — ce qui est le bon comportement pour
    // l'utilisateur, et le pire pour qui met au point.
    let trace = std::env::var("SHIMEJI_MAJ").is_ok();

    // `async move` : la closure prend la propriété de `app` et `actions`.
    // Elle survit à la fonction qui l'a créée, donc elle ne peut rien
    // emprunter.
    tauri::async_runtime::spawn(async move {
        if trace {
            println!("[maj] vérification…");
        }

        // `updater()` construit le client depuis `tauri.conf.json` ; il
        // échoue si la clé publique ou l'endpoint manquent.
        let client = match app.updater() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[maj] updater indisponible : {e}");
                return;
            }
        };

        // `check()` rend `Option<Update>` : `None` veut dire « vous êtes à
        // jour », ce qui n'est pas une erreur.
        match client.check().await {
            Ok(Some(mise_a_jour)) => {
                let version = mise_a_jour.version.clone();
                if trace {
                    println!("[maj] version {version} disponible");
                }
                signaler(&app, &actions, &version);
            }
            Ok(None) => {
                if trace {
                    println!("[maj] déjà à jour");
                }
            }
            Err(e) => {
                // Non fatal, et volontairement discret : être hors ligne est
                // un état normal, pas un incident.
                if trace {
                    eprintln!("[maj] vérification impossible : {e}");
                }
            }
        }
    });
}

/// Note la version trouvée, et n'émet le toast qu'une fois par version.
///
/// Séparée de la vérification pour une raison de lisibilité : la logique du
/// « une seule fois » n'a rien à voir avec le réseau, et la mêler à la
/// fonction ci-dessus rendrait les deux plus difficiles à suivre.
fn signaler(app: &AppHandle, actions: &Actions, version: &str) {
    // Le menu, TOUJOURS : c'est le rappel permanent et discret, et il doit
    // refléter la réalité même si le toast a déjà été vu.
    actions.afficher_etat_maj(&EtatMaj::Disponible(version.to_string()));

    // Le toast, une seule fois par version. On relit la configuration au lieu
    // de se fier à un état en mémoire : l'utilisateur a pu être notifié lors
    // d'un lancement précédent.
    let deja = crate::config::charger().derniere_version_signalee;
    if deja == version {
        if std::env::var("SHIMEJI_MAJ").is_ok() {
            println!("[maj] v{version} déjà signalée, pas de toast");
        }
        return;
    }

    // On n'enregistre QUE si le toast est bien parti : un toast avalé par
    // Windows doit pouvoir être retenté au prochain lancement.
    if crate::toast::maj_disponible(app, version) {
        if let Err(e) = crate::config::definir_version_signalee(version) {
            eprintln!("[maj] version signalée non enregistrée : {e}");
        }
    }
}

/// Le clic sur l'entrée du tray : vérifie, DIT le résultat, et installe
/// s'il y a quelque chose à installer.
///
/// Déclenchée par l'entrée du menu du tray, jamais toute seule : sur Windows,
/// appliquer une mise à jour **relance l'installateur NSIS**, qui ferme
/// l'application. Le faire sans que l'utilisateur l'ait demandé ferait
/// disparaître ses personnages au milieu d'une session.
///
/// On **revérifie** au lieu de conserver l'objet `Update` trouvé au
/// démarrage : c'est un aller-retour réseau de plus sur un clic — négligeable
/// — et ça évite de garder un objet dont la validité se périme (une release
/// peut avoir été retirée entre-temps).
///
/// Ce que l'utilisateur voit (demande de l'auteur, 2026-09-24) : à jour, le
/// libellé « À jour (vX) » et un toast ; une version trouvée, un toast
/// « Mise à jour vers la vX… » avant le téléchargement ; un échec, le
/// libellé « Vérification impossible — réessayer » et un toast qui le dit.
///
/// ⚠️ **En debug, on n'installe JAMAIS** : la branche porte souvent une
/// version plus ancienne que celle installée, et le clic réinstallait la
/// release par-dessus depuis `cargo run` (constaté le 2026-09-24).
pub fn verifier_puis_installer(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // `state` : les `Actions` confiées à Tauri par `main` (`manage`). Un
        // `Arc` cloné, pour l'emporter dans cette tâche.
        let actions = tauri::Manager::state::<Arc<Actions>>(&app).inner().clone();

        let client = match app.updater() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[maj] updater indisponible : {e}");
                actions.afficher_etat_maj(&EtatMaj::Impossible);
                crate::toast::maj_impossible(&app, "mise à jour mal configurée");
                return;
            }
        };

        let mise_a_jour = match client.check().await {
            Ok(Some(m)) => m,
            Ok(None) => {
                let courante = app.package_info().version.to_string();
                println!("[maj] déjà à jour (v{courante})");
                actions.afficher_etat_maj(&EtatMaj::AJour(courante.clone()));
                crate::toast::a_jour(&app, &courante);
                return;
            }
            Err(e) => {
                eprintln!("[maj] vérification impossible : {e}");
                actions.afficher_etat_maj(&EtatMaj::Impossible);
                // Le message du plugin est technique et en anglais : la
                // raison affichée est la nôtre, le détail reste dans la trace.
                crate::toast::maj_impossible(&app, "serveur de mise à jour injoignable");
                return;
            }
        };

        let version = mise_a_jour.version.clone();
        actions.afficher_etat_maj(&EtatMaj::Disponible(version.clone()));

        if cfg!(debug_assertions) {
            // Pas de toast « mise à jour en cours » : il mentirait. Le
            // libellé du tray dit déjà ce qui a été trouvé.
            println!("[maj] v{version} disponible — installation désactivée en debug");
            return;
        }

        crate::toast::installation_en_cours(&app, &version);
        println!("[maj] installation de la v{}…", mise_a_jour.version);

        // Les deux closures sont les rapports de progression du plugin : une
        // par morceau téléchargé, une à la fin. On ne s'en sert que pour
        // tracer — il n'y a aucune barre de progression à nourrir, l'écran
        // appartient au personnage.
        let resultat = mise_a_jour
            .download_and_install(|_recu, _total| {}, || println!("[maj] téléchargement terminé"))
            .await;

        match resultat {
            Ok(()) => {
                // `restart` ne rend jamais la main : il termine le processus.
                // Tout ce qui suit serait mort.
                println!("[maj] redémarrage");
                app.restart();
            }
            Err(e) => {
                // Non fatal : l'application continue de tourner dans la
                // version qu'elle a. L'utilisateur pourra réessayer, ou
                // télécharger l'installateur à la main.
                eprintln!("[maj] installation échouée : {e}");
                actions.afficher_etat_maj(&EtatMaj::Impossible);
                crate::toast::maj_impossible(&app, "l'installation a échoué");
            }
        }
    });
}

#[cfg(test)]
#[path = "maj_tests.rs"]
mod tests;
