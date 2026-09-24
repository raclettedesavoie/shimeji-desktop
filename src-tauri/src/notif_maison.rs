//! La notification MAISON : une petite fenêtre en bas à droite, qui remplace
//! le toast quand Windows est en « Ne pas déranger » (demande de l'auteur,
//! 2026-09-24 — le mode rangeait nos toasts sans bannière, et l'on a cru à
//! un défaut).
//!
//! Responsabilité unique : montrer un titre et un texte quelques secondes,
//! puis se cacher. QUAND la montrer, c'est `toast.rs` qui le décide.
//!
//! ⚠️ **Créée une fois au démarrage, jamais à la volée** — la même règle que
//! le menu (`menu_fenetre.rs`) : créer une fenêtre WebView2 est un travail
//! lourd du thread principal.
//!
//! Elle ne prend jamais le focus (`WS_EX_NOACTIVATE`) et laisse passer les
//! clics : une notification ne doit rien interrompre, c'est tout l'esprit du
//! mode que l'utilisateur a choisi. Elle disparaît seule.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

/// Le label de la fenêtre.
pub const LABEL: &str = "notif";

/// Le temps d'affichage — celui d'une bannière de Windows.
const DUREE: Duration = Duration::from_secs(6);

/// La taille de la fenêtre en pixels CSS, ombre comprise. **Doit suivre
/// `ui/notif.css`** (360 + 2 × 12 de marge, et 96 + 2 × 12).
const LARGEUR_CSS: f64 = 384.0;
const HAUTEUR_CSS: f64 = 120.0;

/// L'écart au bord de la zone de travail, en pixels physiques.
const ECART_AU_BORD: i32 = 8;

/// Le numéro de la dernière notification montrée. Le minuteur d'une
/// notification ne cache la fenêtre que si aucune autre ne l'a remplacée
/// entre-temps — sinon la seconde disparaîtrait au bout de quelques instants.
static DERNIERE: AtomicU64 = AtomicU64::new(0);

/// Crée la fenêtre, cachée. Appelée une fois, au `setup`.
pub fn creer(app: &AppHandle) -> Result<(), String> {
    let win = tauri::WebviewWindowBuilder::new(
        app,
        LABEL,
        tauri::WebviewUrl::App("notif.html".into()),
    )
    .title("shimeji-desktop")
    .visible(false)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(false)
    .focused(false)
    .inner_size(LARGEUR_CSS, HAUTEUR_CSS)
    .build()
    .map_err(|e| format!("fenêtre de notification : {e}"))?;

    // Hors d'Alt+Tab ET jamais activée — contrairement au menu, qui a besoin
    // du focus pour se fermer au clic ailleurs.
    crate::render::appliquer_styles_etendus(&win)?;
    win.set_ignore_cursor_events(true)
        .map_err(|e| format!("notification traversante : {e}"))
}

/// Où poser une fenêtre de `taille` dans le coin bas-droit de `zone`
/// (x, y, largeur, hauteur), à `ecart` du bord.
fn coin_bas_droit(zone: (i32, i32, i32, i32), taille: (i32, i32), ecart: i32) -> (i32, i32) {
    let (x, y, l, h) = zone;
    (x + l - taille.0 - ecart, y + h - taille.1 - ecart)
}

/// Montre la notification. Rend `false` si la fenêtre manque ou si l'écran
/// principal est introuvable : l'appelant retombe alors sur le toast.
pub fn afficher(app: &AppHandle, titre: &str, corps: &str) -> bool {
    let Some(win) = app.get_webview_window(LABEL) else {
        return false;
    };
    // `let … else` : sans écran principal, on ne sait pas où la poser.
    let Ok(Some(ecran)) = app.primary_monitor() else {
        return false;
    };

    // ── Taille et place, en pixels physiques ────────────────────────────
    let echelle = ecran.scale_factor();
    let taille = (
        (LARGEUR_CSS * echelle).round() as i32,
        (HAUTEUR_CSS * echelle).round() as i32,
    );
    let zone = ecran.work_area();
    let (x, y) = coin_bas_droit(
        (
            zone.position.x,
            zone.position.y,
            zone.size.width as i32,
            zone.size.height as i32,
        ),
        taille,
        ECART_AU_BORD,
    );
    let _ = win.set_size(PhysicalSize::new(taille.0 as u32, taille.1 as u32));
    let _ = win.set_position(PhysicalPosition::new(x, y));

    // ── Le contenu ──────────────────────────────────────────────────────
    // `json!` échappe les guillemets et les retours à la ligne : le texte
    // ne peut pas casser le script.
    let charge = serde_json::json!({ "titre": titre, "corps": corps });
    if win.eval(format!("window.afficherNotif({charge})")).is_err() {
        return false;
    }
    let _ = win.show();

    // ── Le minuteur ─────────────────────────────────────────────────────
    // `fetch_add` rend l'ancienne valeur : `numero` est donc celui de CETTE
    // notification. Un thread qui dort 6 s ne coûte rien, et il y en a au
    // plus un par notification — quelques-unes par session.
    let numero = DERNIERE.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(DUREE);
        if DERNIERE.load(Ordering::SeqCst) == numero {
            if let Some(win) = app.get_webview_window(LABEL) {
                let _ = win.hide();
            }
        }
    });
    true
}

#[cfg(test)]
#[path = "notif_maison_tests.rs"]
mod tests;
