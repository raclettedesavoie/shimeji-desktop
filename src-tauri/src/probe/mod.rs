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
}
