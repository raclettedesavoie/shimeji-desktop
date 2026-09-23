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
use crate::overlay::ChargeEcran;
use crate::probe::ScreenInfo;
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

/// Retire ou remet `WS_EX_NOACTIVATE`, le temps d'afficher un menu.
///
/// # Pourquoi ce va-et-vient est nécessaire
///
/// `muda` affiche un menu contextuel en appelant `SetForegroundWindow(hwnd)`
/// **puis** `TrackPopupMenu` (`muda-0.19.3`,
/// `src/platform_impl/windows/mod.rs:1038`). Or `SetForegroundWindow` échoue
/// sur une fenêtre `WS_EX_NOACTIVATE` — silencieusement, en rendant `FALSE`
/// que muda n'examine pas.
///
/// Le menu s'affiche quand même, mais son propriétaire n'est pas au premier
/// plan : c'est le piège Win32 classique du **menu qui ne se referme pas**
/// quand on clique ailleurs. `TrackPopupMenu` ne reçoit jamais le message de
/// perte d'activation qui le termine, et l'utilisateur se retrouve avec un
/// menu collé à l'écran.
///
/// D'où : on autorise l'activation juste avant le menu, on la réinterdit
/// juste après. La fenêtre reste non activable **tout le reste du temps** —
/// c'est-à-dire pendant les glissers, qui sont la raison d'être du style
/// (voir `appliquer_styles_etendus`). Attraper le personnage ne vole donc
/// toujours pas le focus ; seul un menu que l'utilisateur a explicitement
/// ouvert le prend, ce que fait n'importe quelle application.
///
/// ⚠️ **Toujours remettre le style**, y compris si l'affichage du menu
/// échoue. L'appelant s'en charge ; l'oublier laisserait le personnage
/// voleur de focus pour le reste de la session, et le diagnostic partirait
/// chercher très loin de ce fichier.
pub fn autoriser_activation(win: &WebviewWindow, autoriser: bool) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE,
    };

    let hwnd = win.hwnd().map_err(|e| format!("hwnd indisponible : {e}"))?;
    let bit = WS_EX_NOACTIVATE.0 as isize;

    unsafe {
        let actuels = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);

        // `& !bit` retire le bit, `| bit` le remet — et on ne touche qu'à
        // celui-là : les autres styles (LAYERED, TOPMOST, TRANSPARENT,
        // TOOLWINDOW) doivent survivre intacts, comme dans
        // `appliquer_styles_etendus`.
        let nouveaux = if autoriser {
            actuels & !bit
        } else {
            actuels | bit
        };

        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, nouveaux);
    }

    Ok(())
}

/// Retient la fenêtre actuellement au premier plan, pour pouvoir la lui
/// rendre.
///
/// # Pourquoi c'est indispensable, et pas une politesse
///
/// `autoriser_activation(true)` rend `SetForegroundWindow` **efficace** sur
/// notre fenêtre — c'est tout son but. Conséquence immédiate : ouvrir le menu
/// prend le focus à l'éditeur, et Windows ne le rend à personne quand le menu
/// se ferme. L'utilisateur se retrouve à taper dans le vide, sur un webview
/// qui n'attend rien.
///
/// Ce serait une violation directe du besoin — « il ne gêne jamais : pas de
/// vol de focus » — et la plus sournoise, parce qu'elle n'apparaît qu'après
/// avoir utilisé le menu, donc jamais pendant qu'on teste le reste.
///
/// Rend `None` s'il n'y a pas de premier plan (bureau sécurisé, transition) ;
/// il n'y a alors rien à restaurer, et c'est un cas normal.
///
/// Vérifié : `GetForegroundWindow`
/// (`windows-0.61.3/…/WindowsAndMessaging/mod.rs`), déjà employée par
/// `signals`/`probe::win32` pour l'application active.
pub fn fenetre_au_premier_plan() -> Option<windows::Win32::Foundation::HWND> {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    // `unsafe` : franchissement de la frontière FFI. Aucun pointeur ne nous
    // est confié, l'appel est sans condition préalable.
    let hwnd = unsafe { GetForegroundWindow() };

    // `is_invalid()` : le binding rend un `HWND` nul plutôt qu'un `Option`
    // quand il n'y a pas de fenêtre au premier plan.
    if hwnd.is_invalid() {
        None
    } else {
        Some(hwnd)
    }
}

/// Met **notre** fenêtre au premier plan, avant d'ouvrir le menu. Rend `true`
/// si Windows a accepté.
///
/// # Pourquoi la refaire alors que `muda` l'appelle déjà
///
/// `muda` appelle bien `SetForegroundWindow(hwnd)` juste avant
/// `TrackPopupMenu` (`muda-0.19.3`, `src/platform_impl/windows/mod.rs:1038`)
/// — **mais il ne regarde pas son résultat**. Or cet appel est régulièrement
/// refusé : Windows ne le concède qu'au processus qui a reçu le dernier
/// événement d'entrée, et notre fenêtre — en couche, `WS_EX_TOOLWINDOW`,
/// jamais activée de la session — n'est pas dans la position la plus
/// favorable pour le demander.
///
/// Et un refus ne se voit pas : le menu s'affiche quand même. C'est
/// **exactement** le piège Win32 du menu qui ne se referme pas quand on
/// clique ailleurs, décrit dans `autoriser_activation` — `TrackPopupMenu`
/// termine sa boucle modale sur la perte d'activation de son propriétaire, et
/// un propriétaire qui n'a jamais été activé n'en perd jamais.
///
/// # Le repli par `AttachThreadInput`
///
/// La parade documentée (KB135788) : attacher temporairement la file
/// d'entrée du thread propriétaire de notre fenêtre à celle du thread qui
/// détient le premier plan. Pendant cet attachement les deux threads
/// partagent la notion de « qui a le focus », et `SetForegroundWindow`
/// redevient autorisé.
///
/// ⚠️ **C'est le thread PROPRIÉTAIRE de la fenêtre qu'on attache**, pas le
/// nôtre : notre boucle 60 Hz n'a pas de fenêtre, donc pas de file d'entrée
/// qui intéresse Windows. D'où `GetWindowThreadProcessId(hwnd)` plutôt que
/// `GetCurrentThreadId()` — c'est l'erreur silencieuse classique de cette
/// recette, et elle rendrait le repli inopérant sans le moindre message.
///
/// L'attachement est **toujours défait**, y compris si `SetForegroundWindow`
/// échoue encore : le laisser en place lierait durablement notre file
/// d'entrée à celle d'une autre application.
pub fn prendre_le_premier_plan(win: &WebviewWindow) -> Result<bool, String> {
    // `AttachThreadInput` vit dans `System::Threading` et non dans
    // `WindowsAndMessaging` comme les trois autres — c'est une fonction de
    // *thread*, pas de fenêtre. D'où la feature `Win32_System_Threading`
    // dans `Cargo.toml`.
    use windows::Win32::System::Threading::AttachThreadInput;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    };

    let hwnd = win.hwnd().map_err(|e| format!("hwnd indisponible : {e}"))?;

    unsafe {
        // Le cas normal : Windows accepte, il n'y a rien de plus à faire.
        // `.as_bool()` : le binding rend un `BOOL`, pas un `bool` de Rust.
        if SetForegroundWindow(hwnd).as_bool() {
            return Ok(true);
        }

        // ── Le repli ────────────────────────────────────────────────────
        let devant = GetForegroundWindow();
        if devant.is_invalid() {
            // Aucun premier plan à qui s'attacher (bureau sécurisé,
            // transition) : il n'y a pas de repli possible, et ce n'est pas
            // une anomalie.
            return Ok(false);
        }

        // `None` pour le second paramètre : on ne veut que l'identifiant du
        // thread, pas celui du processus. Le binding l'expose en
        // `Option<*mut u32>` précisément pour ça.
        let thread_devant = GetWindowThreadProcessId(devant, None);
        let thread_nous = GetWindowThreadProcessId(hwnd, None);

        // Deux threads déjà identiques : rien à attacher, et l'appel
        // échouerait. Ce serait le cas si le premier plan était… nous.
        if thread_devant == 0 || thread_nous == 0 || thread_devant == thread_nous {
            return Ok(false);
        }

        let _ = AttachThreadInput(thread_nous, thread_devant, true);
        let ok = SetForegroundWindow(hwnd).as_bool();
        let _ = AttachThreadInput(thread_nous, thread_devant, false);

        Ok(ok)
    }
}

/// Poste un message vide dans la file de la fenêtre, **après** la fermeture
/// du menu.
///
/// La seconde moitié de la recette de KB135788, et celle qu'on oublie
/// toujours parce qu'elle n'a aucun effet visible le premier coup :
/// `TrackPopupMenu` laisse sa fenêtre propriétaire dans un état où le menu
/// **suivant** peut refuser de s'afficher ou de se refermer, tant qu'un
/// message quelconque n'est pas passé dans sa file. `WM_NULL` est le message
/// qui ne fait rien, choisi exactement pour cet usage.
///
/// L'échec est ignoré : si la fenêtre vient de disparaître, il n'y a plus de
/// file, et plus de menu à débloquer non plus.
pub fn reveiller_la_file(win: &WebviewWindow) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_NULL};

    let Ok(hwnd) = win.hwnd() else {
        return;
    };

    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
    }
}

/// Rend le premier plan à la fenêtre qui l'avait.
///
/// L'échec est **ignoré volontairement** : Windows refuse `SetForegroundWindow`
/// dans plusieurs situations légitimes (la fenêtre a été fermée pendant que
/// le menu était ouvert, une autre application a pris le focus entre-temps).
/// Insister n'aurait pas de sens — et se battre contre le gestionnaire de
/// fenêtres pour reprendre un focus que l'utilisateur a peut-être déplacé
/// lui-même serait pire que le problème.
pub fn rendre_le_premier_plan(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
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
pub fn recharger(
    app: &AppHandle,
    label: &str,
    version: u64,
    personnage: &str,
) -> Result<(), String> {
    let Some(win) = app.get_webview_window(label) else {
        return Err(format!("fenêtre « {label} » absente"));
    };
    // Le second argument est le personnage courant : il peut avoir changé
    // (fenêtre du catalogue), auquel cas `pet.js` refait sa base d'URL.
    win.eval(format!("window.recharger({version}, \"{personnage}\")"))
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

// ─────────────────────────────────────────────────────────────────────────
// L'overlay : une fenêtre par écran occupé
//
// Conception : `docs/specs/2026-09-23-fenetre-par-ecran-design.md`
// ─────────────────────────────────────────────────────────────────────────

/// Le label Tauri de la fenêtre d'un écran.
///
/// Dérivé de l'identité **stable** de l'écran (`ScreenInfo::id`, issue du
/// `HMONITOR`) et non de son index : un écran débranché puis rebranché
/// changerait d'index, et la fenêtre suivante hériterait du label de la
/// précédente pendant que Windows détruit encore celle-ci. C'est le même
/// piège que celui des labels d'acteurs (design « plusieurs personnages »
/// §3, piège n° 3).
pub fn label_ecran(id: u64) -> String {
    format!("ecran-{id}")
}

/// Crée la fenêtre d'un écran : à la taille de sa zone de travail,
/// transparente, au premier plan, traversante — et qui **ne bougera jamais**.
///
/// C'est tout l'objet de l'architecture : cette fenêtre ne reçoit aucun
/// `SetWindowPos` après sa création. Les personnages se déplacent en CSS à
/// l'intérieur, ce qui retire de la file du thread principal les 900 messages
/// par seconde qui la bouchaient.
pub fn creer_fenetre_ecran(app: &AppHandle, ecran: &ScreenInfo) -> Result<(), String> {
    let label = label_ecran(ecran.id);

    let win = tauri::WebviewWindowBuilder::new(
        app,
        &label,
        tauri::WebviewUrl::App("overlay.html".into()),
    )
    .title("shimeji-desktop")
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(false)
    .focused(false)
    .build()
    .map_err(|e| format!("fenêtre « {label} » : {e}"))?;

    // Position et taille en pixels PHYSIQUES. Les passer au builder les ferait
    // interpréter en pixels LOGIQUES, donc faux sur un écran à 125 % — la
    // fenêtre couvrirait 80 % de l'écran et les personnages seraient coupés.
    // C'est le piège n° 4 des « coordonnées » de CLAUDE.md, et il serait ici
    // beaucoup plus visible qu'avec une fenêtre de 128 px.
    win.set_position(PhysicalPosition::new(
        ecran.work_area.x as i32,
        ecran.work_area.y as i32,
    ))
    .map_err(|e| format!("set_position sur « {label} » : {e}"))?;

    win.set_size(PhysicalSize::new(
        ecran.work_area.w as u32,
        ecran.work_area.h as u32,
    ))
    .map_err(|e| format!("set_size sur « {label} » : {e}"))?;

    // Les clics traversent par défaut. La boucle ne les absorbe que quand le
    // curseur est sur un personnage de CET écran (conception §4).
    //
    // ⚠️ C'est LA propriété qui rend l'idée vivable : sans elle, une fenêtre
    // plein écran au premier plan rendrait la machine inutilisable.
    win.set_ignore_cursor_events(true)
        .map_err(|e| format!("clics traversants sur « {label} » : {e}"))?;

    // **Les deux découvertes de l'étape 0.** Sans elles, la fenêtre volerait
    // le focus de l'éditeur et apparaîtrait dans Alt+Tab.
    match appliquer_styles_etendus(&win) {
        Ok(()) => println!("styles étendus posés sur {label} (NOACTIVATE, TOOLWINDOW)"),
        // Non bloquant : la fenêtre marche sans, elle est seulement moins
        // polie. Mieux vaut un overlay impoli qu'aucun personnage.
        Err(e) => eprintln!("styles étendus NON appliqués sur {label} : {e}"),
    }

    println!(
        "fenêtre {label} : {}×{} @ ({}, {})",
        ecran.work_area.w, ecran.work_area.h, ecran.work_area.x, ecran.work_area.y
    );
    Ok(())
}

/// Ferme la fenêtre d'un écran devenu vide (conception §5.1).
///
/// Silencieuse si la fenêtre n'existe pas : l'appelant peut le demander deux
/// fois sans que ce soit une erreur.
pub fn detruire_fenetre_ecran(app: &AppHandle, id: u64) {
    // `let … else` : rien à fermer, on sort. Équivalent d'un `match` dont la
    // branche `None` ferait `return`.
    let Some(win) = app.get_webview_window(&label_ecran(id)) else {
        return;
    };
    let _ = win.close();
}

/// Envoie toute la charge d'un écran, en un seul `eval`.
///
/// ⚠️ **C'est l'appel dont le spike a mesuré le coût : ~2,9 ms de CPU
/// chacun.** Il ne doit être émis que lorsque quelque chose a changé
/// (conception §5.4), et jamais plus de ~15 fois par seconde et par écran.
/// L'appeler à 60 Hz coûtait 157 % de CPU au lieu de 30.
pub fn pousser_ecran(app: &AppHandle, charge: &ChargeEcran) -> Result<(), String> {
    let label = label_ecran(charge.ecran);
    let Some(win) = app.get_webview_window(&label) else {
        return Err(format!("fenêtre « {label} » absente"));
    };
    win.eval(crate::overlay::js_de(charge))
        .map_err(|e| format!("eval sur « {label} » : {e}"))
}

/// Publie la table `id -> pack` dans la fenêtre d'un écran.
///
/// `table_js` est un littéral objet JavaScript déjà formé par l'appelant
/// (`{"3":"blob","4":"naruto-kakashi"}`), parce que c'est lui qui connaît le
/// roster. Appelée **au changement de roster seulement**, jamais dans la
/// boucle : c'est la seule donnée non numérique qui traverse.
pub fn declarer_packs(
    app: &AppHandle,
    id_ecran: u64,
    table_js: &str,
    version: u32,
) -> Result<(), String> {
    let label = label_ecran(id_ecran);
    let Some(win) = app.get_webview_window(&label) else {
        return Err(format!("fenêtre « {label} » absente"));
    };
    win.eval(format!("window.declarer({table_js}, {version})"))
        .map_err(|e| format!("eval sur « {label} » : {e}"))
}
