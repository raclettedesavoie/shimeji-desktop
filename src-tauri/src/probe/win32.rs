//! L'implémentation réelle de `SystemProbe`, par la crate `windows`.
//!
//! Responsabilité unique : traduire des appels Win32 en types du projet —
//! la topologie des écrans, la souris, et depuis l'étape 2 les cinq signaux
//! lents (inactivité, appli active, heure, batterie, verrouillage), soit huit
//! appels système en tout. **Aucune logique** ici — pas de filtrage, pas de
//! décision. Tout ce qui ressemble à une règle appartient à `world.rs`,
//! `signals.rs` ou au comportement, où c'est testable avec `FakeProbe`.
//!
//! Signatures vérifiées dans les sources de windows 0.61.3 :
//!   EnumDisplayMonitors  Win32/Graphics/Gdi/mod.rs:559
//!   GetMonitorInfoW      Win32/Graphics/Gdi/mod.rs:1102
//!   GetDpiForMonitor     Win32/UI/HiDpi/mod.rs:39
//!   GetCursorPos         Win32/UI/WindowsAndMessaging/mod.rs:825
//!   GetAsyncKeyState     Win32/UI/Input/KeyboardAndMouse/mod.rs:28
//!
//! Les cinq appels de l'étape 2 (signaux) sont documentés à leur emplacement,
//! plus bas dans ce fichier.

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
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
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

// ── Les signaux (étape 2) ───────────────────────────────────────────────
//
// Les cinq appels ci-dessous ont été vérifiés dans les sources de
// `windows` 0.61.3 le 2026-09-09 — signatures comprises. Les emplacements
// sont dans `docs/specs/2026-09-09-etape-2-design.md` §3.

use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::RemoteDesktop::{
    WTSFreeMemory, WTSQuerySessionInformationW, WTSSessionInfoEx, WTSINFOEXW,
    WTS_CURRENT_SESSION, WTS_SESSIONSTATE_LOCK,
};
use windows::Win32::System::SystemInformation::{GetLocalTime, GetTickCount};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

// `WTS_CURRENT_SESSION` vaut `u32::MAX` (4294967295), parce que `wtsapi32.h`
// la définit comme `(DWORD)-1` : un entier qui ressemble à une erreur est en
// réalité la façon dont l'API désigne « la session de l'appelant » sans que
// celui-ci ait besoin de connaître son propre identifiant de session.

/// Depuis combien de temps l'utilisateur n'a touché à rien.
///
/// ⚠️ **Ce compteur ne dit JAMAIS quelle touche a été pressée** — c'est
/// l'exclusion « aucune capture de frappe » du besoin, et c'est la seule
/// raison pour laquelle ce signal est acceptable.
fn inactivite() -> std::time::Duration {
    // `cbSize` doit être renseigné AVANT l'appel : c'est ainsi que Windows
    // sait quelle version de la structure on lui passe. Oublié, l'appel
    // échoue sans autre explication.
    let mut lii = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };

    let ok = unsafe { GetLastInputInfo(&mut lii) };
    if !ok.as_bool() {
        // Échec : on rend zéro, soit « l'utilisateur vient d'agir ». C'est le
        // défaut prudent — il ne s'endormira pas à cause d'une erreur de
        // sonde, alors que rendre « inactif depuis 10 min » l'endormirait à
        // tort et sans explication.
        return std::time::Duration::ZERO;
    }

    let maintenant = unsafe { GetTickCount() };

    // `wrapping_sub` et non `-` : `GetTickCount` repasse à zéro au bout de
    // 49,7 jours de fonctionnement. La soustraction qui déborde rend le bon
    // écart malgré le tour ; une soustraction ordinaire paniquerait en debug.
    let ecart_ms = maintenant.wrapping_sub(lii.dwTime);
    std::time::Duration::from_millis(ecart_ms as u64)
}

/// Le nom de fichier de l'exécutable au premier plan — `"Code.exe"`.
fn appli_active() -> Option<String> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_invalid() {
        // Aucune fenêtre au premier plan : ça arrive pendant un changement de
        // bureau, ou sur l'écran de verrouillage.
        return None;
    }

    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }

    // `PROCESS_QUERY_LIMITED_INFORMATION` et non `PROCESS_QUERY_INFORMATION` :
    // le droit limité suffit à lire le chemin de l'image, et il est accordé
    // même sur des processus d'un autre niveau d'intégrité. Le droit complet
    // échouerait sur toute application élevée.
    //
    // `ok()?` : l'échec est normal (processus protégé, processus qui vient de
    // mourir) et n'est pas une erreur à signaler — on rend `None`.
    let processus = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;

    let mut tampon = [0u16; 260]; // MAX_PATH
    let mut taille = tampon.len() as u32;

    let resultat = unsafe {
        QueryFullProcessImageNameW(
            processus,
            PROCESS_NAME_WIN32,
            windows::core::PWSTR(tampon.as_mut_ptr()),
            &mut taille,
        )
    };

    // Le handle se ferme dans TOUS les cas, y compris en cas d'échec de
    // l'appel ci-dessus. C'est pour ça qu'on ne fait pas `?` sur `resultat`
    // avant cette ligne : un `return` précoce fuirait le handle.
    unsafe {
        let _ = windows::Win32::Foundation::CloseHandle(processus);
    };
    resultat.ok()?;

    // `taille` contient maintenant la longueur écrite, sans le zéro final.
    let chemin = String::from_utf16_lossy(&tampon[..taille as usize]);

    // Le NOM seul, jamais le chemin : c'est ce que l'utilisateur écrira dans
    // `config.json`, et un chemin complet y serait indevinable.
    //
    // `rsplit('\\').next()` rend le dernier segment ; sur un chemin sans
    // antislash il rend la chaîne entière, ce qui est correct.
    chemin.rsplit('\\').next().map(|s| s.to_string())
}

/// L'heure locale, 0 à 23.
///
/// `GetLocalTime` et non l'heure UTC : « il est tard le soir » se juge à
/// l'heure de l'utilisateur. Et surtout pas la crate `chrono` — ce serait une
/// dépendance entière pour lire un `u16`.
fn heure_locale() -> u8 {
    let t = unsafe { GetLocalTime() };
    // `wHour` est un `u16` de 0 à 23 : le `as u8` ne peut pas tronquer.
    t.wHour as u8
}

fn batterie() -> super::Batterie {
    let mut etat = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut etat) }.is_err() {
        // Sonde en échec : on rend « sur secteur, pourcentage inconnu », donc
        // aucun biais. Le personnage ne doit pas fatiguer à cause d'une
        // erreur de lecture.
        return super::Batterie {
            pourcent: None,
            sur_secteur: true,
        };
    }

    // ⚠️ `BatteryLifePercent` vaut **255** quand le pourcentage est inconnu —
    // ce qui est le cas sur toute machine sans batterie. Le rendre tel quel
    // donnerait « 255 % », et le comparer à un seuil donnerait « pas de
    // batterie faible » par accident plutôt que par raison.
    let pourcent = if etat.BatteryLifePercent <= 100 {
        Some(etat.BatteryLifePercent)
    } else {
        None
    };

    // `ACLineStatus` : 0 hors secteur, 1 sur secteur, 255 inconnu. On traite
    // « inconnu » comme « sur secteur », le défaut qui ne fatigue pas.
    let sur_secteur = etat.ACLineStatus != 0;

    super::Batterie {
        pourcent,
        sur_secteur,
    }
}

/// La session est-elle verrouillée ?
fn session_verrouillee() -> bool {
    let mut tampon = windows::core::PWSTR::null();
    let mut octets: u32 = 0;

    // `None` pour le serveur = la machine locale.
    let appel = unsafe {
        WTSQuerySessionInformationW(
            None,
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &mut tampon,
            &mut octets,
        )
    };

    if appel.is_err() || tampon.is_null() {
        // On rend « déverrouillée » : le défaut qui laisse le personnage
        // vivre. Le contraire le ferait disparaître sur une erreur de sonde,
        // ce qui ressemblerait à un plantage.
        return false;
    }

    // `WTSQuerySessionInformationW` ALLOUE : il faut libérer avec
    // `WTSFreeMemory`, sinon on fuit une centaine d'octets deux fois par
    // seconde — soit ~17 Mo par jour.
    //
    // On lit d'abord, on libère ensuite, et on ne sort qu'après.
    let verrouillee = unsafe {
        let info = &*(tampon.0 as *const WTSINFOEXW);

        // `Level` doit valoir 1 pour que l'union porte un
        // `WTSInfoExLevel1`. Lire l'union sans vérifier serait une lecture
        // de mémoire non initialisée.
        if info.Level == 1 {
            info.Data.WTSInfoExLevel1.SessionFlags == WTS_SESSIONSTATE_LOCK as i32
        } else {
            false
        }
    };

    unsafe { WTSFreeMemory(tampon.0 as *mut core::ffi::c_void) };
    verrouillee
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

        // Le bouton droit, lu exactement de la même façon et dans la même
        // image : les deux doivent décrire le MÊME instant, sinon un clic
        // droit pendant un glisser donnerait un état incohérent.
        //
        // ⚠️ `VK_RBUTTON` est le bouton droit **physique**, pas « le bouton
        // secondaire ». Sur une souris inversée par les réglages Windows,
        // c'est donc le bouton gauche physique qui ouvrirait le menu. On
        // l'assume : `GetSystemMetrics(SM_SWAPBUTTON)` corrigerait le tir,
        // mais l'auteur n'a pas de souris inversée et YAGNI — la ligne à
        // ajouter le jour venu est ici et nulle part ailleurs.
        let etat_droit = unsafe { GetAsyncKeyState(VK_RBUTTON.0 as i32) };
        let right_down = (etat_droit as u16 & 0x8000) != 0;

        MouseState {
            pos: Point::new(p.x as f32, p.y as f32),
            left_down,
            right_down,
        }
    }

    fn signaux(&self) -> super::Signaux {
        super::Signaux {
            inactivite: inactivite(),
            appli_active: appli_active(),
            heure: heure_locale(),
            batterie: batterie(),
            session_verrouillee: session_verrouillee(),
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
        "souris : ({}, {}) bouton gauche={} bouton droit={}",
        m.pos.x, m.pos.y, m.left_down, m.right_down
    );
}

// Pas de `#[cfg(test)] mod tests` dans ce fichier, et c'est volontaire : il ne
// contient aucune logique à vérifier, seulement des appels au système. Un test
// n'y affirmerait que le bon fonctionnement de Windows. C'est précisément la
// raison d'être du trait — tout le testable est ailleurs. Ce fichier se
// vérifie à l'œil, une fois.
