//! La frontière entre le programme et Windows — troisième contrainte de la
//! spec §10.2.
//!
//! Responsabilité unique : décrire ce que le programme a besoin de savoir du
//! système, sans dire comment on l'apprend. Deux implémentations : `win32`
//! (la vraie) et `fake` (les tests).
//!
//! Tout ce qui sort d'ici est en PIXELS PHYSIQUES du bureau virtuel
//! (spec §3.4). `scale` est transporté pour dimensionner le sprite, et pour
//! rien d'autre — surtout pas pour convertir des coordonnées.

use crate::geom::{Point, Rect};

pub mod fake;
pub mod win32;

/// Un écran, tel que la physique a besoin de le connaître.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenInfo {
    /// Identité stable dans le temps, dérivée du `HMONITOR` (spec §5.2).
    /// C'est ce qui fait que « l'écran sur lequel je suis » survit à un
    /// changement de résolution.
    pub id: u64,

    /// **La zone de travail, pas l'écran** — elle exclut la barre des tâches.
    /// Utiliser le rectangle de l'écran ferait marcher le personnage SOUS la
    /// barre des tâches (piège Windows n° 3).
    pub work_area: Rect,

    /// Facteur d'échelle du moniteur (1.0 à 96 ppp, 1.5 à 144, 2.0 à 192).
    /// Sert **uniquement** au dimensionnement du sprite (spec §3.4).
    pub scale: f32,
}

/// L'état de la souris. Deux informations, et pas une de plus : on ne capture
/// aucune frappe (décision n° 8 du journal).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MouseState {
    pub pos: Point,
    pub left_down: bool,
}

/// L'état du système à un instant, tel que le comportement a besoin de le
/// connaître (design de l'étape 2, §3).
///
/// **Un instantané et non cinq accesseurs.** La raison est la cohérence :
/// lues à cinq instants différents, ces valeurs pourraient montrer « session
/// verrouillée » et « actif il y a 10 ms » dans la même image du
/// comportement. Un instantané rend cet état impossible par construction.
///
/// Pas `Copy` : `appli_active` est une `String`. C'est exactement pourquoi le
/// comportement ne reçoit PAS cette structure mais un `signals::Biais`, qui
/// est `Copy` — voir `Entrees`.
#[derive(Debug, Clone, PartialEq)]
pub struct Signaux {
    /// Depuis combien de temps l'utilisateur n'a touché à rien.
    ///
    /// ⚠️ `GetLastInputInfo` rend un **compteur de millisecondes**, jamais
    /// une touche. C'est la seule voie compatible avec « aucune capture de
    /// frappe », qui est une exclusion explicite du besoin.
    pub inactivite: std::time::Duration,

    /// Le nom de fichier de l'exécutable au premier plan — `"Code.exe"`.
    ///
    /// Le nom seul, jamais le chemin complet : c'est ce que l'utilisateur
    /// écrira dans `config.json`, et un chemin serait impossible à deviner.
    /// `None` quand la fenêtre au premier plan n'appartient à aucun processus
    /// interrogeable — écran de connexion, fenêtre d'élévation UAC.
    pub appli_active: Option<String>,

    /// L'heure locale, de 0 à 23. Rien de plus fin : aucun signal du projet
    /// ne dépend de la minute.
    pub heure: u8,

    pub batterie: Batterie,

    /// Vrai pendant que la session est verrouillée (Win+L, veille avec mot de
    /// passe, changement d'utilisateur).
    pub session_verrouillee: bool,
}

/// L'état de la batterie.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Batterie {
    /// `None` sur une machine **sans** batterie, ou quand Windows dit ne pas
    /// savoir.
    ///
    /// ⚠️ **Ce n'est pas `Some(100)`.** `GetSystemPowerStatus` rend 255 quand
    /// il n'y a pas de batterie ; confondre les deux ferait fatiguer un pet
    /// sur une tour de bureau, en permanence et sans raison visible.
    pub pourcent: Option<u8>,

    pub sur_secteur: bool,
}

/// Ce que le programme sait du système.
///
/// `&self` partout : interroger le système ne modifie rien côté programme.
///
/// À l'étape 4 s'ajoutera `fn windows(&self) -> Vec<WindowInfo>`. Le monde,
/// la physique et le comportement n'en sauront rien — ils ne manipulent que
/// des `Platform` (spec §5.1). C'est ce découplage qui permet de livrer le
/// sol maintenant et les fenêtres plus tard sans rien réécrire.
pub trait SystemProbe {
    /// Tous les écrans, dans un ordre non garanti. Peut être **vide** si aucun
    /// moniteur n'est rapporté (session distante en cours d'établissement) —
    /// l'appelant doit traiter ce cas, pas paniquer.
    fn screens(&self) -> Vec<ScreenInfo>;

    fn mouse(&self) -> MouseState;

    /// Tout ce qui change lentement, lu **d'un coup** (design §5.5 : ~2 Hz).
    ///
    /// Appelée deux fois par seconde et pas davantage : aucun de ces signaux
    /// ne bouge vite, et cinq appels système à 60 Hz seraient 300 appels par
    /// seconde pour des valeurs qui changent toutes les minutes.
    fn signaux(&self) -> Signaux;
}
