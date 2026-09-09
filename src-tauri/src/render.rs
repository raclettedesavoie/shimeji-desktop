//! Le rendu : pousser la frame vers le webview, et déplacer la fenêtre.
//!
//! Responsabilité unique : la frontière entre l'état du personnage (calculé
//! en Rust) et son affichage. **Aucune décision** ici — pas de physique, pas
//! de comportement.
//!
//! Le motif `AppHandle` + recherche de la fenêtre par label est **retenu de
//! l'étape 0** : `WebviewWindow: Send` n'est pas garanti explicitement, et ce
//! motif gère en prime le cas de la fenêtre fermée. Un accès à une table de
//! hachage par image est négligeable.

use crate::geom::Point;
use serde::Serialize;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

/// Ce que le webview a besoin de savoir. **Deux champs, et pas un de plus.**
///
/// `PartialEq` : l'appelant compare avec la valeur précédente et n'émet que
/// sur changement. À 60 Hz, une pose de marche ne change d'image que ~8 fois
/// par seconde — on économise ainsi ~85 % des messages IPC, sans aucune
/// logique côté front.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Rendu {
    /// Le numéro de `shime<n>.png`.
    pub image: u32,
    /// Faut-il retourner le sprite horizontalement (spec §8.5) ?
    pub flip: bool,
}

/// Pose `WS_EX_NOACTIVATE` et `WS_EX_TOOLWINDOW` sur la fenêtre.
///
/// **Les deux découvertes de l'étape 0.** Tauri ne les pose pas :
///
/// · **`WS_EX_NOACTIVATE`** — sans lui, la Tâche 11 (qui réactive les clics
///   dans la hitbox) ferait qu'attraper le personnage **volerait le focus de
///   l'éditeur** en cours de frappe. `.focused(false)` de Tauri ne couvre pas
///   ce cas : il ne concerne que l'affichage initial, pas l'activation par
///   clic.
///
/// · **`WS_EX_TOOLWINDOW`** — `.skip_taskbar(true)` passe par
///   `ITaskbarList::DeleteTab`, qui retire de la barre des tâches ;
///   l'exclusion d'Alt+Tab n'en découle pas, Windows 11 l'accompagne par
///   heuristique.
///
/// Signatures vérifiées : `WebviewWindow::hwnd`
/// (`tauri-2.11.5/src/webview/webview_window.rs:1847`),
/// `Get/SetWindowLongPtrW` (`windows-0.61.3/…/WindowsAndMessaging/mod.rs:1132`
/// et `:2262`).
pub fn appliquer_styles_etendus(win: &WebviewWindow) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    let hwnd = win.hwnd().map_err(|e| format!("hwnd indisponible : {e}"))?;

    // `unsafe` : ces deux appels franchissent la frontière FFI vers Win32.
    // Rust ne peut pas garantir que `hwnd` est un handle valide — c'est nous
    // qui l'affirmons, ce qui est légitime : il sort de `build()` juste
    // avant, et la fenêtre n'a pas pu être fermée entre-temps.
    unsafe {
        let actuels = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);

        // `.0` extrait le u32 du newtype `WINDOW_EX_STYLE` ; `as isize`
        // l'aligne sur le type de Get/SetWindowLongPtrW. Sans ces deux
        // conversions, le `|` ne compile pas.
        let ajout = (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) as isize;

        // `|` et non une affectation : on AJOUTE nos bits sans écraser ceux
        // que Tauri a posés (LAYERED, TOPMOST, TRANSPARENT). Écrire
        // `SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ajout)` retirerait la
        // transparence et le premier plan — les deux propriétés que
        // l'étape 0 vient d'établir.
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, actuels | ajout);
    }

    Ok(())
}

/// Envoie la frame au webview d'un personnage, en **appelant directement une
/// fonction JavaScript**.
///
/// Rend `Err` si la fenêtre a disparu — l'appelant en déduit qu'il faut
/// arrêter la boucle de ce personnage.
///
/// # Pourquoi `eval` et non le système d'événements
///
/// **C'est une découverte de l'exécution, pas un choix initial.** La voie
/// des événements (`emit_to` + `window.__TAURI__.event.listen`) a été
/// écrite, lancée, et **ne fonctionnait pas** : le sprite restait invisible.
///
/// Le diagnostic est venu d'une trace sur le schéma URI — une seule requête
/// `shime:///blob/1`, celle de l'affichage initial de `pet.js`, et aucune
/// ensuite. Donc les images étaient bien servies, mais `poser` n'était
/// jamais rappelée : l'écouteur ne recevait rien.
///
/// La cause : **Tauri v2 a une liste de contrôle d'accès**, et
/// `event.listen` exige la permission `core:event:allow-listen`. Le projet
/// n'a aucun fichier de capacités, donc l'appel était refusé — et
/// *silencieusement*, la promesse rejetée n'étant attendue par personne.
///
/// Deux issues possibles : déclarer les capacités, ou se passer du système
/// d'événements. **On se passe** — et ce n'est pas par paresse :
///
/// | | Événements | `eval` |
/// |---|---|---|
/// | fichier de capacités à maintenir | oui | non |
/// | API Tauri injectée dans la page | oui (`withGlobalTauri`) | non |
/// | dépend d'une ACL qui peut refuser en silence | oui | non |
/// | ciblage d'un webview précis | `emit_to` (`emit` diffuse à TOUS) | intrinsèque |
///
/// La dernière ligne compte pour l'étape 3 : `Emitter::emit` diffuse à tous
/// les webviews (`tauri/src/lib.rs:934`), donc chaque personnage aurait
/// affiché la pose du dernier émetteur. Avec `eval`, la fenêtre visée est
/// celle sur laquelle on appelle la méthode — l'erreur n'est pas
/// exprimable.
///
/// Et c'est cohérent avec « le webview est un afficheur délibérément bête »
/// (spec §3.1) : il ne reste plus une ligne de code d'écoute côté JS.
///
/// Vérifié : `WebviewWindow::eval`
/// (`tauri-2.11.5/src/webview/webview_window.rs:2403`).
pub fn pousser(app: &AppHandle, label: &str, r: Rendu) -> Result<(), String> {
    let Some(win) = app.get_webview_window(label) else {
        return Err(format!("fenêtre « {label} » absente"));
    };

    // `{}` sur un `bool` rend « true » ou « false », qui sont exactement les
    // littéraux JavaScript attendus. Les deux valeurs étant un entier et un
    // booléen produits par nous, il n'y a rien à échapper — aucune chaîne
    // venue de l'extérieur n'entre dans ce JavaScript.
    let js = format!("window.poser({}, {})", r.image, r.flip);

    win.eval(js).map_err(|e| format!("eval : {e}"))
}

/// Prévient le webview qu'il doit oublier ses images.
///
/// Séparée de `pousser` parce qu'elle n'arrive que sur action de
/// l'utilisateur, jamais dans la boucle.
pub fn recharger(app: &AppHandle, label: &str, version: u64) -> Result<(), String> {
    let Some(win) = app.get_webview_window(label) else {
        return Err(format!("fenêtre « {label} » absente"));
    };
    win.eval(format!("window.recharger({version})"))
        .map_err(|e| format!("eval : {e}"))
}

/// Déplace la fenêtre. **Appelée 60 fois par seconde.**
///
/// La position est en **pixels physiques du bureau virtuel** (spec §3.4),
/// d'où `PhysicalPosition` — utiliser la variante logique ferait dériver la
/// position sur un écran non standard, et ce genre de décalage ne se
/// reproduit que sur un seul écran, ce qui est un enfer à diagnostiquer.
///
/// > **Ne pas y remettre `set_size`.** La taille ne change qu'au chargement
/// > du manifeste ou au changement d'échelle d'écran — quelques fois par
/// > heure au grand maximum. Appeler une API du système 60 fois par seconde
/// > pour une valeur constante est une faute par principe. D'où
/// > `dimensionner`, séparée.
/// >
/// > ⚠️ **Mais ce n'est PAS ce qui coûte le CPU**, contrairement à ce qu'on a
/// > d'abord cru. Mesuré sur cette machine, en build *debug* :
/// >
/// > | | CPU |
/// > |---|---|
/// > | `set_position` + `set_size` à 60 Hz | 14,8 % |
/// > | `set_position` seul à 60 Hz | 14,1 % |
/// > | **spike de l'étape 0**, qui ne fait *que* `set_position` à 60 Hz | **18 %** |
/// >
/// > Le spike consomme plus que l'application complète : le coût est donc
/// > **inhérent au déplacement d'une fenêtre en couche à 60 Hz**, pas dans
/// > notre logique. Chercher l'optimisation dans le comportement ou dans le
/// > rendu serait chercher au mauvais endroit.
pub fn placer(app: &AppHandle, label: &str, coin: Point) -> Result<(), String> {
    // `let … else` : la fenêtre a été fermée, il n'y a plus rien à placer.
    let Some(win) = app.get_webview_window(label) else {
        return Err(format!("fenêtre « {label} » absente"));
    };

    // `round()` avant la conversion : `as i32` tronque vers zéro, ce qui
    // décalerait d'un pixel la moitié du temps et donnerait un tremblement
    // visible sur du pixel-art.
    win.set_position(PhysicalPosition::new(
        coin.x.round() as i32,
        coin.y.round() as i32,
    ))
    .map_err(|e| format!("set_position : {e}"))
}

/// Redimensionne la fenêtre. **À n'appeler que sur changement.**
///
/// Séparée de `placer` pour une raison mesurée, pas esthétique : voir
/// l'avertissement ci-dessus.
pub fn dimensionner(app: &AppHandle, label: &str, taille: (u32, u32)) -> Result<(), String> {
    let Some(win) = app.get_webview_window(label) else {
        return Err(format!("fenêtre « {label} » absente"));
    };

    win.set_size(PhysicalSize::new(taille.0, taille.1))
        .map_err(|e| format!("set_size : {e}"))
}

/// Active ou désactive la traversée des clics (spec §3.3).
///
/// Utilisé par la Tâche 11. Séparé de `placer` parce qu'il ne change que
/// lorsque le curseur entre ou sort de la hitbox, pas à chaque image.
pub fn traverser_les_clics(app: &AppHandle, label: &str, traverser: bool) -> Result<(), String> {
    let Some(win) = app.get_webview_window(label) else {
        return Err(format!("fenêtre « {label} » absente"));
    };
    win.set_ignore_cursor_events(traverser)
        .map_err(|e| format!("set_ignore_cursor_events : {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ce fichier n'a presque rien à tester : tout y est un appel à Tauri ou
    // à Win32. Le seul comportement propre est la comparaison de `Rendu`,
    // dont dépend l'économie de messages IPC — et une régression y serait
    // invisible à l'œil, puisque l'affichage resterait correct.

    #[test]
    fn deux_rendus_identiques_sont_egaux() {
        let a = Rendu {
            image: 3,
            flip: false,
        };
        let b = Rendu {
            image: 3,
            flip: false,
        };
        assert_eq!(a, b);
    }

    #[test]
    fn un_changement_de_flip_seul_rend_les_rendus_differents() {
        // Sans ce test, une implémentation qui ne comparerait que `image`
        // passerait : le personnage regarderait alors toujours du même côté.
        let a = Rendu {
            image: 3,
            flip: false,
        };
        let b = Rendu {
            image: 3,
            flip: true,
        };
        assert_ne!(a, b);
    }

    #[test]
    fn le_rendu_se_serialise_avec_les_noms_attendus_du_front() {
        // `pet.js` lit `event.payload.image` et `.flip`. Un renommage côté
        // Rust casserait l'affichage sans casser la compilation.
        let json = serde_json::to_string(&Rendu {
            image: 7,
            flip: true,
        })
        .unwrap();
        assert_eq!(json, r#"{"image":7,"flip":true}"#);
    }
}
