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

/// L'état de la souris. Trois informations, et pas une de plus : on ne
/// capture aucune frappe (décision n° 8 du journal).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MouseState {
    pub pos: Point,

    /// Le bouton gauche : c'est lui qui **attrape** le personnage (§3.3).
    pub left_down: bool,

    /// Le bouton droit : c'est lui qui **ouvre le menu contextuel**.
    ///
    /// Séparé du gauche plutôt que d'être un `enum Bouton` : les deux
    /// peuvent être enfoncés en même temps, et surtout ils ne s'adressent pas
    /// à la même couche — le gauche est lu par les **réflexes** (portage), le
    /// droit ne l'est par personne dans `behavior/`, il est consommé par la
    /// boucle 60 Hz qui ouvre le menu. Les fondre obligerait les réflexes à
    /// connaître un bouton qui ne les concerne pas.
    pub right_down: bool,
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

/// Une fenêtre retenue par le filtrage, telle que le monde a besoin de la
/// connaître (design §5.1, §5.3).
///
/// **Trois champs, et pas un de plus.** Ni titre, ni nom de processus, ni
/// classe : le monde ne construit que des rectangles, et tout champ
/// supplémentaire finirait par tenter quelqu'un d'écrire « si c'est VSCode
/// alors… », ce qui trahirait « la physique ne sait jamais d'où vient une
/// plateforme » (design §5.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowInfo {
    /// Le `HWND`, qui donne son identité à la plateforme (design §5.2).
    ///
    /// C'est lui qui fait que « la plateforme sur laquelle je suis » survit
    /// au déplacement ET au redimensionnement de la fenêtre — la condition
    /// de la décision n° 1.
    pub hwnd: u64,

    /// Les bornes **VISUELLES**, pas celles de `GetWindowRect`.
    ///
    /// ⚠️ Piège Windows n° 1 : une fenêtre Win10/11 déclare ~7 px de bordure
    /// de redimensionnement invisible de chaque côté. Utilisé tel quel, le
    /// personnage est assis 7 px au-dessus de la barre de titre, **dans le
    /// vide** — subtilement faux, et très visible sur du pixel-art.
    /// `probe::win32` les obtient par `DwmGetWindowAttribute`.
    pub rect: Rect,

    /// Rang dans le z-order : **0 = au premier plan**.
    ///
    /// Gratuit : `EnumWindows` énumère du premier plan vers l'arrière, donc
    /// le rang d'énumération *est* le z. Aucune API supplémentaire, et c'est
    /// ce qui rend l'occlusion (décision n° 2) bon marché.
    pub z: u32,
}

/// Ce que le programme sait du système.
///
/// `&self` partout : interroger le système ne modifie rien côté programme.
///
/// `windows()` a été ajoutée à l'étape 4b, exactement comme annoncé. Le monde,
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

    /// Les fenêtres praticables, **déjà filtrées** (design §5.3), rangées du
    /// premier plan vers l'arrière.
    ///
    /// Le filtrage est fait ici et non par l'appelant : il est entièrement
    /// fait de questions Win32 (visible ? masquée ? outil ? minimisée ?) que
    /// `world.rs` n'a aucun moyen de poser, et qu'il n'a surtout pas à
    /// connaître.
    ///
    /// Appelée à **~8 Hz** et pas davantage (design §5.5) : une fenêtre qui
    /// apparaît est vue en ~125 ms, ce qui est imperceptible, alors qu'un
    /// recensement complet à 60 Hz coûterait ~2 400 appels système par
    /// seconde.
    ///
    /// Peut être **vide**, et ce n'est pas une erreur : un bureau sans aucune
    /// fenêtre praticable est un cas normal. Le monde n'expose alors que les
    /// plateformes d'écran, comme avant l'étape 4b.
    fn windows(&self) -> Vec<WindowInfo>;

    /// Le rectangle visuel d'**une seule** fenêtre, ré-interrogé à 60 Hz.
    ///
    /// # Pourquoi cette méthode existe alors que `windows()` rend déjà tout
    ///
    /// Parce que sans elle l'étape rate son moment. Un personnage assis sur
    /// une barre de titre que l'on **balade à la souris** est *le* geste qui
    /// fait sourire avec un Shimeji ; recalculé à 8 Hz seulement, il
    /// avancerait par sauts de 125 ms — saccadé, et raté.
    ///
    /// Le design §5.5 le prévoit explicitement : « interroger *une* fenêtre
    /// est quasi gratuit → il colle parfaitement à la fenêtre qu'on
    /// déplace ». C'est **un** appel système par image, contre la quarantaine
    /// d'un recensement complet.
    ///
    /// Rend `None` si la fenêtre a disparu, n'est plus praticable, ou n'est
    /// tout simplement pas une fenêtre. L'appelant en déduit que la
    /// plateforme n'existe plus — et le réflexe « plateforme disparue → je
    /// tombe » fait le reste, sans cas particulier.
    fn rect_de_fenetre(&self, hwnd: u64) -> Option<Rect>;
}
