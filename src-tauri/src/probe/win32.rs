//! L'implémentation réelle de `SystemProbe`, par la crate `windows`.
//!
//! Responsabilité unique : traduire trois appels Win32 en types du projet.
//! **Aucune logique** ici — pas de filtrage, pas de décision. Tout ce qui
//! ressemble à une règle appartient à `world.rs` ou au comportement, où c'est
//! testable avec `FakeProbe`.
//!
//! Signatures vérifiées dans les sources de windows 0.61.3 :
//!   EnumDisplayMonitors  Win32/Graphics/Gdi/mod.rs:559
//!   GetMonitorInfoW      Win32/Graphics/Gdi/mod.rs:1102
//!   GetDpiForMonitor     Win32/UI/HiDpi/mod.rs:39
//!   GetCursorPos         Win32/UI/WindowsAndMessaging/mod.rs:825
//!   GetAsyncKeyState     Win32/UI/Input/KeyboardAndMouse/mod.rs:28

use super::{MouseState, ScreenInfo, SystemProbe};
use crate::geom::{Point, Rect};

// `BOOL` ne vit PAS dans `Win32::Foundation` : c'est un type de
// `windows-result`, réexporté par `windows::core`. Le chercher dans
// Foundation avec les autres types win32 est l'erreur naturelle, et elle
// donne un `unresolved import` qui ne dit pas où regarder.
// `BOOL(pub i32)`, avec une méthode `.as_bool()`.
use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// Déclare le processus **conscient du DPI par moniteur (v2)**.
///
/// ⚠️ **À appeler tout au début de `main`, avant absolument tout le reste.**
///
/// Sans cet appel, Windows considère le processus comme « non conscient du
/// DPI » et lui **ment** : sur un écran à 125 %, `GetMonitorInfoW` rend
/// 1536×816 au lieu de 1920×1032, et `GetDpiForMonitor` rend 96 ppp au lieu
/// de 120. On travaillerait alors en pixels *logiques* virtualisés en croyant
/// être en pixels physiques — exactement ce que la spec §3.4 interdit, et
/// qu'elle annonce comme « un enfer à diagnostiquer ».
///
/// Deux raisons de le faire nous-mêmes plutôt que de laisser Tauri s'en
/// charger :
///
/// 1. **Tauri ne le fait qu'à la création de sa boucle d'événements.** Notre
///    sonde est utilisée avant (diagnostic de démarrage) et après (boucle
///    60 Hz). Sans cet appel, les deux ne verraient pas le même bureau, ce
///    qui est la pire forme du bug : reproductible seulement à moitié.
/// 2. Un appel explicite se lit et se vérifie ; une dépendance à l'ordre
///    d'initialisation d'une bibliothèque, non.
///
/// L'appel échoue si la conscience DPI est **déjà** fixée (Tauri est passé
/// avant, ou un manifeste la déclare). C'est sans conséquence : on voulait
/// justement ce réglage. D'où le `let _ =`.
pub fn activer_conscience_dpi() {
    use windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

pub struct Win32Probe;

impl Win32Probe {
    pub fn new() -> Self {
        Win32Probe
    }
}

// `Default` : clippy le réclame dès qu'un `new()` sans argument existe.
impl Default for Win32Probe {
    fn default() -> Self {
        Self::new()
    }
}

/// Convertit un `RECT` Win32 (bornes gauche/haut/droite/bas) en `Rect` du
/// projet (coin + taille). Les deux formes décrivent la même chose ; confondre
/// « bas » et « hauteur » est l'erreur classique, isolée ici une fois pour
/// toutes.
fn rect_depuis_win32(r: RECT) -> Rect {
    Rect::new(
        r.left as f32,
        r.top as f32,
        (r.right - r.left) as f32,
        (r.bottom - r.top) as f32,
    )
}

/// Le rappel appelé par Windows une fois par moniteur.
///
/// `unsafe extern "system"` : c'est Windows qui l'appelle, avec la convention
/// d'appel de l'OS. Rust ne peut rien vérifier de ce côté de la frontière.
///
/// `lparam` transporte un pointeur vers notre `Vec` — c'est le mécanisme
/// habituel de win32 pour passer un contexte à un rappel, faute de fermetures
/// en C. On le remet en `&mut Vec` ci-dessous, et c'est la seule ligne
/// réellement délicate de ce fichier.
unsafe extern "system" fn collecte_moniteur(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _clip: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    // `cbSize` doit être renseigné AVANT l'appel : c'est ainsi que Windows
    // sait quelle version de la structure on lui passe. L'oublier fait
    // échouer `GetMonitorInfoW` sans autre explication.
    //
    // `..Default::default()` remplit les champs restants (les deux RECT et
    // dwFlags) — MONITORINFO dérive `Default` dans windows 0.61.3.
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
        // L'échelle. `GetDpiForMonitor` peut échouer (moniteur en cours de
        // débranchement) : on garde alors 96 ppp, soit l'échelle 1. Un
        // personnage à la mauvaise taille vaut mieux qu'un plantage.
        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        let _ = GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);

        // `&mut *(...)` : on transforme l'entier du LPARAM en pointeur, puis
        // on le déréférence. Sûr ici parce que `screens()` garantit que le
        // Vec est vivant pendant toute l'énumération — `EnumDisplayMonitors`
        // est synchrone.
        let ecrans = &mut *(lparam.0 as *mut Vec<ScreenInfo>);

        ecrans.push(ScreenInfo {
            // `HMONITOR` est un pointeur opaque ; on ne s'en sert que comme
            // identité, jamais pour déréférencer. Double conversion parce que
            // le champ est un `*mut c_void`.
            id: hmonitor.0 as usize as u64,

            // `rcWork` et non `rcMonitor` : la zone de travail exclut la
            // barre des tâches (piège Windows n° 3).
            work_area: rect_depuis_win32(info.rcWork),

            scale: dpi_x as f32 / 96.0,
        });
    }

    // TRUE = continuer l'énumération. Rendre FALSE l'arrêterait au premier
    // moniteur, et le second écran n'existerait jamais pour le programme.
    BOOL(1)
}

impl SystemProbe for Win32Probe {
    fn screens(&self) -> Vec<ScreenInfo> {
        let mut ecrans: Vec<ScreenInfo> = Vec::new();

        // SÉCURITÉ : `ecrans` vit jusqu'à la fin de la fonction, et
        // `EnumDisplayMonitors` est synchrone — le rappel a donc fini de s'en
        // servir quand l'appel rend la main. Aucun pointeur ne lui survit.
        unsafe {
            let _ = EnumDisplayMonitors(
                None, // tout le bureau virtuel
                None, // aucun rectangle de découpe
                Some(collecte_moniteur),
                LPARAM(&mut ecrans as *mut Vec<ScreenInfo> as isize),
            );
        }

        ecrans
    }

    fn mouse(&self) -> MouseState {
        let mut p = POINT { x: 0, y: 0 };

        // `GetCursorPos` rend un `Result` dans ce binding. En cas d'échec
        // (bureau sécurisé, session verrouillée), on garde (0, 0) : le
        // personnage croira la souris dans un coin, ce qui est inoffensif.
        unsafe {
            let _ = GetCursorPos(&mut p);
        }

        // `GetAsyncKeyState` : le bit de POIDS FORT dit « enfoncé
        // maintenant ». Le bit de poids faible dirait « pressé depuis le
        // dernier appel », qu'on ne veut surtout pas — il se consomme à la
        // lecture, donc deux appels dans la même image se voleraient
        // l'information.
        let etat = unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) };
        let left_down = (etat as u16 & 0x8000) != 0;

        MouseState {
            pos: Point::new(p.x as f32, p.y as f32),
            left_down,
        }
    }
}

/// Imprime ce que la sonde voit. Diagnostic de développement, l'équivalent de
/// la topologie que le spike de l'étape 0 imprimait.
///
/// Prend `&dyn SystemProbe` et non `&Win32Probe` : on peut ainsi l'appeler
/// sur la sonde factice, ce qui est pratique en mode simulation (Tâche 9).
pub fn imprimer_diagnostic(sonde: &dyn SystemProbe) {
    let ecrans = sonde.screens();
    if ecrans.is_empty() {
        println!("AUCUN écran rapporté — le monde sera vide.");
        return;
    }
    for e in &ecrans {
        println!(
            "écran {:#x} : zone de travail x={} y={} l={} h={} échelle={}",
            e.id, e.work_area.x, e.work_area.y, e.work_area.w, e.work_area.h, e.scale
        );
    }
    let m = sonde.mouse();
    println!(
        "souris : ({}, {}) bouton gauche={}",
        m.pos.x, m.pos.y, m.left_down
    );
}

// Pas de `#[cfg(test)] mod tests` dans ce fichier, et c'est volontaire : il ne
// contient aucune logique à vérifier, seulement des appels au système. Un test
// n'y affirmerait que le bon fonctionnement de Windows. C'est précisément la
// raison d'être du trait — tout le testable est ailleurs. Ce fichier se
// vérifie à l'œil, une fois.
