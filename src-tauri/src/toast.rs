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
