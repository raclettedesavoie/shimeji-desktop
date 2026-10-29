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

pub struct Win32Probe {
    /// Les fenêtres vues en plein écran AU PREMIER PLAN, et qui couvrent
    /// encore leur écran — chacune en entier (`HWND` converti). Voir
    /// `ecrans_en_plein_ecran`.
    ///
    /// `Mutex` : le trait n'expose que `&self`, il faut donc une mutabilité
    /// intérieure. `RefCell` ferait l'affaire aujourd'hui, mais rendrait la
    /// sonde non partageable entre threads (`!Sync`) sans que rien ne le
    /// signale à l'endroit où on la crée. Le verrou n'est jamais disputé :
    /// seule la boucle interroge la sonde.
    fenetres_plein_ecran: Mutex<Vec<isize>>,
}

impl Win32Probe {
    pub fn new() -> Self {
        Win32Probe {
            fenetres_plein_ecran: Mutex::new(Vec::new()),
        }
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

            // `rcMonitor` : l'écran entier, pour la seule fenêtre d'affichage
            // — un personnage porté au-dessus de la barre des tâches doit
            // rester visible (voir `ScreenInfo::bounds`).
            bounds: rect_depuis_win32(info.rcMonitor),

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
            // La sonde ne sait pas la mesurer : c'est la boucle qui la
            // renseigne (voir le commentaire du champ).
            latence_file: std::time::Duration::ZERO,
        }
    }

    /// Deux temps, et c'est le second qui compte à l'usage :
    ///
    /// 1. **La fenêtre au premier plan est en plein écran** → elle rejoint
    ///    les fenêtres mémorisées.
    /// 2. **Chaque fenêtre mémorisée couvre-t-elle toujours son écran ?** Si
    ///    oui, cet écran est pris ; sinon elle est oubliée.
    ///
    /// Les fenêtres s'ADDITIONNENT : avec un plein écran sur chaque écran,
    /// les deux sont pris, et les personnages se cachent. La première
    /// version n'en retenait qu'une — le premier temps REMPLAÇAIT la
    /// mémoire —, et chaque clic faisait passer les personnages d'un écran à
    /// l'autre, par-dessus la vidéo qu'on venait de quitter.
    ///
    /// Sans le second temps, un Alt-Tab du jeu vers une fenêtre de l'AUTRE
    /// écran — pendant un chargement, par exemple — rendrait l'écran du jeu
    /// aux personnages, rouvrirait sa fenêtre d'overlay (un gel bref, voir
    /// CLAUDE.md « Une fenêtre par écran »), puis la refermerait au retour
    /// dans le jeu. L'écran ne se libère donc que quand le jeu se ferme, se
    /// réduit (Alt-Tab d'un plein écran exclusif), ou cesse de couvrir l'écran.
    ///
    /// Assumé : un navigateur ouvert PAR-DESSUS le jeu, sur le même écran,
    /// ne libère pas l'écran — le jeu le couvre toujours derrière lui.
    fn ecrans_en_plein_ecran(&self) -> Vec<u64> {
        // `unwrap_or_else(|e| e.into_inner())` : si un thread avait paniqué
        // en tenant le verrou, il serait « empoisonné ». On reprend quand
        // même la liste — au pire périmée, et le second temps la nettoie —
        // plutôt que de faire paniquer la boucle à son tour.
        let mut memoire = self
            .fenetres_plein_ecran
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        // ── 1. Le premier plan ──────────────────────────────────────────
        let premier_plan = unsafe { GetForegroundWindow() };
        let entier = premier_plan.0 as isize;
        if ecran_couvert_par(premier_plan).is_some() && !memoire.contains(&entier) {
            memoire.push(entier);
        }

        // ── 2. Celles qui couvrent encore leur écran ────────────────────
        //
        // `retain` garde les éléments pour lesquels la fermeture rend
        // `true`, et retire les autres — sur place. La fermeture en profite
        // pour noter l'écran de chaque fenêtre gardée.
        //
        // `HWND(... as *mut c_void)` : on refait un handle depuis l'entier
        // mémorisé. Aucun risque même si la fenêtre a été détruite
        // entre-temps : les fonctions interrogées échouent proprement sur un
        // handle mort, et `ecran_couvert_par` rend `None`.
        let mut ecrans: Vec<u64> = Vec::new();
        memoire.retain(|&h| match ecran_couvert_par(HWND(h as *mut core::ffi::c_void)) {
            Some(id) => {
                // Deux fenêtres plein écran sur le même écran : un seul id.
                if !ecrans.contains(&id) {
                    ecrans.push(id);
                }
                true
            }
            None => false,
        });
        ecrans
    }
}

// ── Le plein écran (demande de l'auteur, 2026-09-28) ────────────────────
//
// Toutes les fonctions ci-dessous sont dans `Win32_UI_WindowsAndMessaging`,
// `Win32_Graphics_Gdi` et `Win32_Graphics_Dwm`, activées dans `Cargo.toml`.
//
// ⚠️ `DWMWA_CLOAKED` (fenêtres fantômes, piège Windows n° 2) est testé, et
// ce n'était pas le cas de la première version, qui supposait qu'« une
// fenêtre au premier plan qui couvre un écran n'est pas un fantôme ». Faux,
// constaté le 2026-10-29 : « Expérience d'entrée Windows » (`TextInputHost`,
// le panneau de Win+V et Win+.) fait EXACTEMENT la taille de l'écran
// principal, se dit visible, prend le premier plan quand on l'ouvre — puis
// reste là, masquée par le compositeur, une fois refermée. Mémorisée au
// passage, elle « couvrait » son écran pour toujours : l'écran de gauche
// n'était plus jamais rendu aux personnages.

use std::sync::Mutex;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONULL};
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetShellWindow, GetWindowRect, IsIconic, IsWindowVisible, IsZoomed,
};

/// L'écran que `hwnd` recouvre entièrement, s'il en recouvre un et qu'il
/// ressemble à une application en plein écran. `None` dans tous les autres
/// cas, y compris un handle nul ou mort.
fn ecran_couvert_par(hwnd: HWND) -> Option<u64> {
    if hwnd.is_invalid() {
        return None;
    }

    // SÉCURITÉ : on ne fait qu'INTERROGER des fenêtres par leur handle.
    // Aucun pointeur ne sort de ce bloc, et un handle périmé fait échouer
    // les appels au lieu de corrompre quoi que ce soit.
    unsafe {
        // ── Les fenêtres qu'on écarte d'emblée ──────────────────────────
        //
        // Invisible ou réduite : elle ne couvre rien.
        //
        // ⚠️ Agrandie (`IsZoomed`), elle n'est PAS écartée ici : Chrome en
        // plein écran garde cet état. C'est `est_en_plein_ecran`, plus bas,
        // qui départage — voir là-bas le relevé qui l'a montré.
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() {
            return None;
        }

        // Masquée par le compositeur (piège Windows n° 2) : elle se dit
        // visible, mais rien n'est dessiné. Voir le bandeau plus haut pour
        // la fenêtre qui l'a montré.
        //
        // `DwmGetWindowAttribute` écrit dans un tampon dont on lui donne
        // l'adresse et la taille : d'où le `*mut u32` converti en pointeur
        // générique (`c_void`). 0 = affichée, toute autre valeur = masquée
        // (par l'application, par le shell, ou par un bureau virtuel). Si
        // l'appel échoue, on ne conclut rien et on continue.
        let mut masquee: u32 = 0;
        let lue = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut masquee as *mut u32 as *mut core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
        if lue.is_ok() && masquee != 0 {
            return None;
        }

        // Le bureau lui-même couvre l'écran : cliquer sur le fond d'écran le
        // met au premier plan. Sans ce test, cliquer sur le bureau
        // chasserait les personnages de l'écran… où ils vivent.
        if hwnd == GetShellWindow() || est_le_bureau(hwnd) {
            return None;
        }

        // Nos propres fenêtres : celles des écrans font la taille de leur
        // écran. Elles ne prennent jamais le premier plan
        // (`WS_EX_NOACTIVATE`), mais la fenêtre mémorisée n'est pas forcément
        // celle du premier plan — on préfère ne pas dépendre de ce détail.
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == GetCurrentProcessId() {
            return None;
        }

        // ── La fenêtre couvre-t-elle son écran ? ────────────────────────
        //
        // `GetWindowRect` et non les bornes DWM (piège Windows n° 1) : une
        // fenêtre plein écran n'a pas de bordure invisible, et un
        // éventuel débordement va dans le bon sens pour `couvre`.
        let mut r = RECT::default();
        if GetWindowRect(hwnd, &mut r).is_err() {
            return None;
        }

        // `MONITOR_DEFAULTTONULL` : une fenêtre hors de tout écran n'en
        // prend aucun, plutôt que de se voir attribuer le plus proche.
        let moniteur = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL);
        if moniteur.is_invalid() {
            return None;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(moniteur, &mut info).as_bool() {
            return None;
        }

        let agrandie = IsZoomed(hwnd).as_bool();
        if !crate::plein_ecran::est_en_plein_ecran(
            rect_depuis_win32(r),
            rect_depuis_win32(info.rcMonitor),
            agrandie,
        ) {
            return None;
        }

        // Même identité que dans `collecte_moniteur` : c'est ce qui permet
        // de retrouver l'écran dans la liste de `screens()`.
        Some(moniteur.0 as usize as u64)
    }
}

/// La fenêtre est-elle le fond du bureau ? `Progman` est le bureau classique ;
/// `WorkerW` est la fenêtre que l'explorateur glisse derrière les icônes
/// (fonds d'écran animés, transition de fond d'écran). Les deux font la
/// taille de l'écran, et prennent le premier plan quand on clique dessus.
fn est_le_bureau(hwnd: HWND) -> bool {
    let mut tampon = [0u16; 32];
    // `GetClassNameW` rend le nombre de caractères écrits, 0 en cas d'échec
    // — dont un handle mort. SÉCURITÉ : Windows écrit dans `tampon`, dont la
    // tranche lui transmet la taille ; il ne peut pas déborder.
    let n = unsafe { GetClassNameW(hwnd, &mut tampon) };
    if n <= 0 {
        return false;
    }
    let classe = String::from_utf16_lossy(&tampon[..n as usize]);
    classe == "Progman" || classe == "WorkerW"
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
