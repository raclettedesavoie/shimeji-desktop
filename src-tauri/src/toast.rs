//! La notification de fin de première configuration (spec §4).
//!
//! Responsabilité unique : émettre **une** notification Windows, sans jamais
//! faire échouer ce qui l'appelle.

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Annonce que l'application continue en arrière-plan.
///
/// # L'AppUserModelID — une hypothèse mesurée, et démentie
///
/// Sur Windows 10/11, un toast exige un **AppUserModelID enregistré**, que
/// fournit normalement le raccourci du menu Démarrer posé par l'installateur
/// NSIS. La conception prévoyait donc que le toast **échoue** en
/// développement, l'exe de `target\debug\` n'ayant aucun raccourci.
///
/// **Mesuré le 2026-09-20 : il réussit quand même** (`[toast] affiché`,
/// lancé directement depuis `target\debug\`). `tauri-winrt-notification`
/// pourvoit l'AppUserModelID lui-même. Une hypothèse de plus formulée puis
/// démentie par la mesure — le motif récurrent du dossier CPU.
///
/// ⚠️ Nuance qui reste vraie : `show()` rendant `Ok` veut dire que Windows a
/// **accepté** la notification, pas qu'elle a été **vue**. L'assistant de
/// concentration ou les réglages de notifications peuvent l'avaler sans que
/// nous en sachions rien.
///
/// D'où le traitement, inchangé : *best-effort*, jamais fatal, et
/// `SHIMEJI_TOAST=1` imprime le `Result` — pour que ni le succès ni l'échec
/// ne soient muets (règle du projet : tout ce qui demanderait un clic reçoit
/// un équivalent scriptable).
pub fn arriere_plan(app: &AppHandle) {
    let resultat = app
        .notification()
        .builder()
        .title("🐱 Shimeji Desktop est actif")
        .body(
            "L'application continue de fonctionner en arrière-plan.\n\
             Vous pouvez la contrôler depuis l'icône dans la zone de notification.",
        )
        .show();

    let trace = std::env::var("SHIMEJI_TOAST").is_ok();

    match resultat {
        Ok(()) => {
            if trace {
                println!("[toast] affiché");
            }
        }
        Err(e) => {
            // Toujours imprimé, même sans la variable : un toast qui n'arrive
            // pas est une information, pas un incident.
            eprintln!("[toast] non affiché : {e}");
            if trace {
                eprintln!(
                    "[toast] pistes : AppUserModelID absent, ou notifications \
                     désactivées pour cette application"
                );
            }
        }
    }
}

/// Un toast de TEST, émis par l'entrée « Tester une notification » du tray —
/// présente en build debug seulement (demande de l'auteur, 2026-09-24).
///
/// Le résultat est imprimé **toujours**, sans attendre `SHIMEJI_TOAST` :
/// c'est tout l'intérêt de l'entrée. `Ok` veut dire que Windows l'a
/// ACCEPTÉ ; s'il n'apparaît pas à l'écran alors, c'est l'assistant de
/// concentration ou les réglages de notifications de Windows qui l'avalent
/// (voir `arriere_plan`).
#[cfg(debug_assertions)]
pub fn test(app: &AppHandle) {
    let resultat = app
        .notification()
        .builder()
        .title("Shimeji Desktop — notification de test")
        .body("Si vous lisez ceci, les notifications Windows fonctionnent.")
        .show();
    match resultat {
        Ok(()) => println!("[toast] test accepté par Windows"),
        Err(e) => eprintln!("[toast] test refusé : {e}"),
    }
}

/// Annonce qu'une nouvelle version est disponible (mise à jour automatique).
///
/// ⚠️ **Ce toast s'écarte d'une décision du projet** — « il ne notifie rien,
/// ne rappelle rien ». Il a été demandé explicitement, et il se défend : il
/// relève de la maintenance de l'application, pas du comportement du
/// personnage. La règle reste vraie pour tout ce qui touche au pet.
///
/// **Aucun bouton, et c'est délibéré.** Cliquer le toast ne déclenche rien :
/// une installation lancée par un clic distrait fermerait l'application au
/// milieu d'une session. C'est l'entrée du menu du tray qui agit, quand
/// l'utilisateur l'a décidé.
///
/// L'appelant est responsable de ne l'émettre **qu'une fois par version**
/// (voir `config::derniere_version_signalee`) : sans ça, il reviendrait à
/// chaque lancement — le harcèlement exact que la règle voulait éviter.
pub fn maj_disponible(app: &AppHandle, version: &str) -> bool {
    let resultat = app
        .notification()
        .builder()
        .title(format!("Shimeji Desktop v{version} est disponible"))
        .body(
            "Ouvrez le menu de l'icône dans la zone de notification              pour l'installer.",
        )
        .show();

    let trace = std::env::var("SHIMEJI_TOAST").is_ok();

    match resultat {
        Ok(()) => {
            if trace {
                println!("[toast] mise à jour v{version} signalée");
            }
            true
        }
        Err(e) => {
            // On rend `false` : l'appelant n'enregistrera donc PAS la version
            // comme signalée, et réessaiera au prochain lancement. Un toast
            // perdu ne doit pas l'être définitivement.
            eprintln!("[toast] mise à jour non signalée : {e}");
            false
        }
    }
}
