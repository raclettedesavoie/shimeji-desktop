//! La fenêtre du menu du clic droit : une webview dédiée, créée UNE fois au
//! démarrage, montrée au curseur à chaque clic droit (spec « menu sur
//! mesure » §4).
//!
//! Responsabilité unique : montrer, placer et cacher cette fenêtre. Ce
//! qu'elle affiche vient de `menu_perso::lignes`, ce que fait un choix de
//! `actions::executer`.
//!
//! # Pourquoi une fenêtre et non le menu de Tauri
//!
//! Le menu de Tauri (`muda`) s'exécute dans un callback de `tao`, et `tao`
//! met en file tout événement reçu pendant ce temps (`should_buffer`,
//! `tao-0.35.3/src/platform_impl/windows/event_loop/runner.rs:143`) — dont
//! les `eval` qui portent les positions : **tous** les personnages se
//! figeaient. Une fenêtre n'a pas de boucle modale : rien n'est retenu.
//! Et elle se stylise en CSS, ce qu'aucun menu natif ne permet.
//!
//! ⚠️ **Créée une fois, jamais à la volée** : créer une fenêtre WebView2
//! est un travail lourd du thread principal — c'est le gel constaté à la
//! fermeture des fenêtres d'écran (CLAUDE.md, « Une fenêtre par ÉCRAN »).

use crate::geom::Rect;
use crate::menu_perso::Ligne;
use crate::probe::ScreenInfo;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize};

/// Le label de la fenêtre, repris par `capabilities/menu.json`.
pub const LABEL: &str = "menu";

/// La marge autour du menu, en pixels CSS, pour que son ombre ait de la
/// place dans la fenêtre. **Doit valoir `--marge` de `ui/menu.css`.**
pub const MARGE_CSS: f32 = 12.0;

/// Le temps laissé à `menu.js` pour dessiner, mesurer et rappeler
/// `placer_menu`. Il lui faut quelques millisecondes ; au-delà d'une seconde,
/// il ne le fera plus (page pas encore chargée, exception JS), et le menu est
/// abandonné — voir `est_abandonne`.
pub const DELAI_PLACEMENT: std::time::Duration = std::time::Duration::from_secs(1);

/// Le menu en cours d'affichage.
struct MenuOuvert {
    /// Le curseur au clic droit, en pixels physiques du bureau virtuel.
    curseur: (i32, i32),
    /// L'écran du curseur : ses bornes (pour replier le menu dedans) et son
    /// échelle (pour convertir la taille CSS en pixels physiques).
    bornes: Rect,
    echelle: f32,
    /// Le drapeau qu'attend la boucle 60 Hz (`menu_en_cours`).
    ferme: Arc<AtomicBool>,
    /// La fenêtre qui avait le premier plan, à qui le rendre. Un `isize`
    /// et non un `HWND` : ce dernier enveloppe un pointeur brut, que Rust ne
    /// laisse pas passer d'un thread à l'autre dans un `Mutex` partagé.
    precedente: Option<isize>,
    /// Le rectangle de la fenêtre une fois placée (x, y, largeur, hauteur),
    /// pour le filet du « clic ailleurs ».
    rect: Option<(i32, i32, i32, i32)>,
}

/// L'état partagé entre la boucle (qui ouvre) et les commandes (qui placent
/// et ferment). Enregistré par `app.manage` au `setup`.
#[derive(Default)]
pub struct EtatMenu(Mutex<Option<MenuOuvert>>);

/// Crée la fenêtre, cachée. Appelée une fois, au `setup`.
pub fn creer(app: &AppHandle) -> Result<(), String> {
    let win = tauri::WebviewWindowBuilder::new(
        app,
        LABEL,
        tauri::WebviewUrl::App("menu.html".into()),
    )
    .title("shimeji-desktop")
    .visible(false)
    .decorations(false)
    // Transparente : c'est le CSS qui dessine le fond, les coins arrondis
    // et l'ombre.
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(false)
    .focused(false)
    .inner_size(10.0, 10.0)
    .build()
    .map_err(|e| format!("fenêtre du menu : {e}"))?;

    // Hors d'Alt+Tab, mais ACTIVABLE — contrairement aux fenêtres d'écran,
    // pas de `WS_EX_NOACTIVATE` : c'est la prise du focus qui permet de la
    // fermer quand on clique ailleurs (`blur`).
    crate::render::appliquer_style_outil(&win)
}

/// Ouvre le menu : mémorise où, puis envoie les lignes à la fenêtre. Elle
/// se mesurera, et appellera `placer_menu` pour être placée et montrée.
///
/// Appelée depuis la boucle 60 Hz : `eval` ne bloque pas, il poste le script
/// au thread principal.
pub fn ouvrir(
    app: &AppHandle,
    lignes: &[Ligne],
    curseur: (i32, i32),
    ecran: &ScreenInfo,
    ferme: Arc<AtomicBool>,
) {
    let Some(win) = app.get_webview_window(LABEL) else {
        // Sans fenêtre, pas de menu : on le dit fermé aussitôt, sinon le
        // personnage cliqué resterait figé pour toujours.
        ferme.store(true, Ordering::Release);
        return;
    };

    let etat = app.state::<EtatMenu>();
    if let Ok(mut e) = etat.0.lock() {
        *e = Some(MenuOuvert {
            curseur,
            bornes: ecran.bounds,
            echelle: ecran.scale,
            ferme: ferme.clone(),
            precedente: crate::render::fenetre_au_premier_plan().map(|h| h.0 as isize),
            rect: None,
        });
    }

    // La hauteur maximale, en pixels CSS : l'écran moins les marges. Au-delà,
    // le menu défile (`overflow-y: auto` dans `menu.css`).
    let hauteur_max = ecran.bounds.h / ecran.scale.max(0.01) - 2.0 * MARGE_CSS - 16.0;
    let charge = serde_json::json!({ "lignes": lignes, "hauteurMax": hauteur_max });
    if win.eval(format!("window.afficherMenu({charge})")).is_err() {
        ferme.store(true, Ordering::Release);
    }
}

/// Place la fenêtre à la taille mesurée par `menu.js`, puis la montre et lui
/// donne le premier plan. Appelée par la commande `placer_menu`.
pub fn placer(app: &AppHandle, largeur_css: f32, hauteur_css: f32) {
    let Some(win) = app.get_webview_window(LABEL) else {
        return;
    };
    let etat = app.state::<EtatMenu>();
    let Ok(mut e) = etat.0.lock() else {
        return;
    };
    // `as_mut` : emprunte le contenu de l'`Option` pour le modifier en place.
    let Some(ouvert) = e.as_mut() else {
        // Fermé entre-temps (Échap très rapide) : rien à montrer.
        return;
    };

    let (x, y, w, h) = rectangle(
        ouvert.curseur,
        (largeur_css, hauteur_css),
        ouvert.echelle,
        ouvert.bornes,
    );
    ouvert.rect = Some((x, y, w as i32, h as i32));

    // Position, taille, puis position encore. Arriver sur un écran de DPI
    // différent fait redimensionner la fenêtre par Windows (`WM_DPICHANGED`) :
    // la taille est donc posée APRÈS, et la position reposée au cas où ce
    // redimensionnement l'aurait déplacée.
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let _ = win.set_size(PhysicalSize::new(w, h));
    let _ = win.set_position(PhysicalPosition::new(x, y));
    let _ = win.show();

    // Le premier plan, VÉRIFIÉ : un refus donnerait un menu qui ne se ferme
    // pas au clic ailleurs. Le filet de la boucle (`hors_du_menu`) couvre ce
    // cas, et `SHIMEJI_MENU=1` le signale.
    if let Ok(hwnd) = win.hwnd() {
        if !crate::render::prendre_le_premier_plan(hwnd) && std::env::var_os("SHIMEJI_MENU").is_some() {
            eprintln!("menu : Windows a refusé le premier plan");
        }
    }
    let _ = win.set_focus();

    if std::env::var_os("SHIMEJI_MENU").is_some() {
        println!("menu placé : {w}×{h} @ ({x}, {y})");
    }
}

/// Ferme le menu : cache la fenêtre, rend le focus, prévient la boucle.
///
/// **Idempotente** — un choix est suivi d'un `blur`, et le filet de la
/// boucle peut s'y ajouter : `take()` fait que seul le premier appel agit.
pub fn fermer(app: &AppHandle) {
    let etat = app.state::<EtatMenu>();
    let ouvert = match etat.0.lock() {
        Ok(mut e) => e.take(),
        Err(_) => None,
    };
    let Some(ouvert) = ouvert else {
        return;
    };

    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.hide();
    }
    if let Some(h) = ouvert.precedente {
        crate::render::rendre_le_premier_plan(windows::Win32::Foundation::HWND(
            h as *mut core::ffi::c_void,
        ));
    }
    // `Release` : la boucle (son `Acquire`) voit tout ce qui précède.
    ouvert.ferme.store(true, Ordering::Release);
}

/// Le rectangle du menu ouvert, s'il est déjà placé. `try_lock` : appelée
/// depuis la boucle 60 Hz, qui ne doit jamais attendre un verrou.
pub fn rect_ouvert(app: &AppHandle) -> Option<(i32, i32, i32, i32)> {
    let etat = app.state::<EtatMenu>();
    let e = etat.0.try_lock().ok()?;
    e.as_ref()?.rect
}

/// Le menu est-il placé ? `None` si l'état est verrouillé en ce moment
/// (`placer` ou `fermer` en cours) : la boucle ne décide rien cette image-là,
/// elle réessaiera à la suivante. `Some(false)` s'il n'y a aucun menu.
pub fn est_place(app: &AppHandle) -> Option<bool> {
    let etat = app.state::<EtatMenu>();
    let e = etat.0.try_lock().ok()?;
    // `as_ref` puis `is_some_and` : « il y a un menu, et son rectangle est
    // connu » — faux dans les deux autres cas.
    Some(e.as_ref().is_some_and(|m| m.rect.is_some()))
}

/// Un menu ouvert à `ouvert_a` et toujours pas placé à `maintenant` est-il à
/// abandonner ?
///
/// Le filet de la boucle (`hors_du_menu`) a besoin du rectangle ; `blur` et
/// Échap ont besoin d'une fenêtre visible. Un menu jamais placé n'a ni l'un
/// ni l'autre : sans ce délai, rien ne le fermerait, le personnage cliqué
/// resterait figé et tout clic droit suivant serait refusé (relecture finale).
pub fn est_abandonne(
    ouvert_a: std::time::Duration,
    maintenant: std::time::Duration,
    place: bool,
) -> bool {
    !place && maintenant.saturating_sub(ouvert_a) > DELAI_PLACEMENT
}

/// Où poser la fenêtre, en pixels physiques : `(x, y, largeur, hauteur)`.
///
/// Le coin du MENU part du curseur ; la fenêtre commence une marge plus
/// haut et plus à gauche, pour l'ombre. S'il déborde à droite ou en bas, il
/// s'ouvre de l'autre côté du curseur ; et en dernier recours il est serré
/// contre le bord — il reste toujours entièrement dans l'écran.
///
/// `taille_css` est mesurée par le webview, en pixels CSS ; `echelle` est
/// celle de l'écran, qui les convertit (CLAUDE.md, piège n° 4 des
/// coordonnées).
pub fn rectangle(
    curseur: (i32, i32),
    taille_css: (f32, f32),
    echelle: f32,
    bornes: Rect,
) -> (i32, i32, u32, u32) {
    let e = echelle.max(0.01);
    let marge = (MARGE_CSS * e).round() as i32;
    let w = ((taille_css.0 + 2.0 * MARGE_CSS) * e).ceil() as i32;
    let h = ((taille_css.1 + 2.0 * MARGE_CSS) * e).ceil() as i32;

    let (gauche, haut) = (bornes.left() as i32, bornes.top() as i32);
    let (droite, bas) = (bornes.right() as i32, bornes.bottom() as i32);

    let mut x = curseur.0 - marge;
    if x + w > droite {
        // De l'autre côté : le bord droit du MENU sur le curseur.
        x = curseur.0 - w + marge;
    }
    let mut y = curseur.1 - marge;
    if y + h > bas {
        y = curseur.1 - h + marge;
    }

    // Le dernier recours : serré contre le bord. `max` puis `min` plutôt
    // que `clamp`, qui panique si le menu est plus grand que l'écran.
    x = x.min(droite - w).max(gauche);
    y = y.min(bas - h).max(haut);

    (x, y, w as u32, h as u32)
}

/// Vrai si `p` tombe hors de `rect` `(x, y, largeur, hauteur)`.
pub fn hors_du_menu(p: (i32, i32), rect: (i32, i32, i32, i32)) -> bool {
    let (x, y, w, h) = rect;
    p.0 < x || p.0 >= x + w || p.1 < y || p.1 >= y + h
}

#[cfg(test)]
#[path = "menu_fenetre_tests.rs"]
mod tests;
