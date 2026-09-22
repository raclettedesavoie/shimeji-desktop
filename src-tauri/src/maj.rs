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
    actions.signaler_maj(version);

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

/// Télécharge et installe la mise à jour, puis relance l'application.
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
pub fn installer(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let client = match app.updater() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[maj] updater indisponible : {e}");
                return;
            }
        };

        let mise_a_jour = match client.check().await {
            Ok(Some(m)) => m,
            Ok(None) => {
                // La release a pu être retirée, ou l'utilisateur a déjà mis à
                // jour par ailleurs. Ce n'est pas une erreur.
                println!("[maj] plus rien à installer");
                return;
            }
            Err(e) => {
                eprintln!("[maj] vérification impossible : {e}");
                return;
            }
        };

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
            }
        }
    });
}
