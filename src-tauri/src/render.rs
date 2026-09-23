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

use crate::overlay::ChargeEcran;
use crate::probe::ScreenInfo;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

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

/// Retient la fenêtre actuellement au premier plan, pour pouvoir la lui
/// rendre.
///
/// # Pourquoi c'est indispensable, et pas une politesse
///
/// `prendre_le_premier_plan` met la fenêtre du menu au premier plan — c'est
/// tout son but. Conséquence immédiate : ouvrir le menu
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

/// Met la fenêtre propriétaire du menu au premier plan, avant d'ouvrir le
/// menu. Rend `true` si Windows a accepté.
///
/// # Pourquoi vérifier le résultat
///
/// `SetForegroundWindow` est régulièrement refusé : Windows ne le concède
/// qu'au processus qui a reçu le dernier événement d'entrée, et le nôtre ne
/// voit le clic droit que par `GetCursorPos` — il ne reçoit aucun message.
///
/// Et un refus ne se voit pas : le menu s'affiche quand même. C'est le piège
/// Win32 classique du **menu qui ne se referme pas** quand on clique ailleurs
/// — `TrackPopupMenu` termine sa boucle modale sur la perte d'activation de
/// son propriétaire, et un propriétaire qui n'a jamais été activé n'en perd
/// jamais.
///
/// # Le repli par `AttachThreadInput`
///
/// La parade documentée (KB135788) : attacher temporairement la file
/// d'entrée du thread propriétaire de notre fenêtre à celle du thread qui
/// détient le premier plan. Pendant cet attachement les deux threads
/// partagent la notion de « qui a le focus », et `SetForegroundWindow`
/// redevient autorisé.
///
/// ⚠️ **C'est le thread PROPRIÉTAIRE de la fenêtre qu'on attache.** Depuis
/// que le menu a son propre thread (`menu_natif`), c'est aussi celui qui
/// appelle — mais le demander à la fenêtre reste juste quel que soit
/// l'appelant. D'où `GetWindowThreadProcessId(hwnd)` plutôt que
/// `GetCurrentThreadId()` — c'est l'erreur silencieuse classique de cette
/// recette, et elle rendrait le repli inopérant sans le moindre message.
///
/// L'attachement est **toujours défait**, y compris si `SetForegroundWindow`
/// échoue encore : le laisser en place lierait durablement notre file
/// d'entrée à celle d'une autre application.
pub fn prendre_le_premier_plan(hwnd: windows::Win32::Foundation::HWND) -> bool {
    // `AttachThreadInput` vit dans `System::Threading` et non dans
    // `WindowsAndMessaging` comme les trois autres — c'est une fonction de
    // *thread*, pas de fenêtre. D'où la feature `Win32_System_Threading`
    // dans `Cargo.toml`.
    use windows::Win32::System::Threading::AttachThreadInput;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    };

    unsafe {
        // Le cas normal : Windows accepte, il n'y a rien de plus à faire.
        // `.as_bool()` : le binding rend un `BOOL`, pas un `bool` de Rust.
        if SetForegroundWindow(hwnd).as_bool() {
            return true;
        }

        // ── Le repli ────────────────────────────────────────────────────
        let devant = GetForegroundWindow();
        if devant.is_invalid() {
            // Aucun premier plan à qui s'attacher (bureau sécurisé,
            // transition) : il n'y a pas de repli possible, et ce n'est pas
            // une anomalie.
            return false;
        }

        // `None` pour le second paramètre : on ne veut que l'identifiant du
        // thread, pas celui du processus. Le binding l'expose en
        // `Option<*mut u32>` précisément pour ça.
        let thread_devant = GetWindowThreadProcessId(devant, None);
        let thread_nous = GetWindowThreadProcessId(hwnd, None);

        // Deux threads déjà identiques : rien à attacher, et l'appel
        // échouerait. Ce serait le cas si le premier plan était… nous.
        if thread_devant == 0 || thread_nous == 0 || thread_devant == thread_nous {
            return false;
        }

        let _ = AttachThreadInput(thread_nous, thread_devant, true);
        let ok = SetForegroundWindow(hwnd).as_bool();
        let _ = AttachThreadInput(thread_nous, thread_devant, false);

        ok
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
pub fn reveiller_la_file(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_NULL};

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

/// Crée la fenêtre d'un écran : à la taille de l'écran COMPLET,
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
    //
    // ⚠️ **`bounds` et non `work_area`** (2026-09-23) : la zone de travail
    // exclut la barre des tâches, et la fenêtre qui s'y limitait coupait net
    // tout personnage porté ou en chute par-dessus — il disparaissait. Le
    // SOL, lui, reste la zone de travail : c'est `World` qui le décide, pas
    // la taille de cette fenêtre. Les clics traversent, donc couvrir la
    // barre des tâches ne la rend pas moins cliquable.
    win.set_position(PhysicalPosition::new(
        ecran.bounds.x as i32,
        ecran.bounds.y as i32,
    ))
    .map_err(|e| format!("set_position sur « {label} » : {e}"))?;

    win.set_size(PhysicalSize::new(
        ecran.bounds.w as u32,
        ecran.bounds.h as u32,
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
        ecran.bounds.w, ecran.bounds.h, ecran.bounds.x, ecran.bounds.y
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
