//! Le personnage : son état, son manifeste, son accroche, sa physique.
//!
//! Ce module regroupe ce qui change ensemble. `Character` lui-même arrive à
//! la Tâche 7, avec le comportement qui le fait vivre.

pub mod attach;
pub mod manifest;
pub mod physics;

/// Le sens dans lequel le personnage regarde.
///
/// **Il n'y a pas de frames dédiées à chaque sens** : l'orientation est
/// obtenue par miroir horizontal (spec §8.5). Ce type dit donc s'il faut
/// retourner le sprite, et rien d'autre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    Left,
    Right,
}

impl Facing {
    /// `true` s'il faut retourner le sprite horizontalement.
    ///
    /// **Convention : les sprites Shimeji sont dessinés tournés vers la
    /// GAUCHE.** Regarder à droite demande donc un miroir.
    ///
    /// Ce n'est pas une supposition : dans `conf/actions.xml` de Shimeji-ee,
    /// l'action `Walk` porte `Velocity="-2,0"` — une vitesse **négative**,
    /// donc vers la gauche, avec le sprite non miroité. Idem pour `Run`
    /// (`-4,0`), `Dash` (`-8,0`) et `Creep`.
    ///
    /// La convention inverse avait été retenue au départ, par défaut plutôt
    /// que par vérification, et le personnage marchait à reculons — les
    /// pattes allaient dans un sens, le déplacement dans l'autre. C'est
    /// visible en une seconde à l'œil et invisible pour un test.
    pub fn flipped(&self) -> bool {
        matches!(self, Facing::Right)
    }

    /// L'autre sens. Sert au demi-tour en bout de plateforme.
    pub fn inverse(&self) -> Facing {
        match self {
            Facing::Left => Facing::Right,
            Facing::Right => Facing::Left,
        }
    }

    /// `-1.0` vers la gauche, `+1.0` vers la droite. Multiplier une vitesse
    /// par ce signe évite un `match` à chaque déplacement.
    pub fn signe(&self) -> f32 {
        match self {
            Facing::Left => -1.0,
            Facing::Right => 1.0,
        }
    }
}

use attach::Attachment;
use manifest::Manifest;
use crate::geom::Point;
use std::time::Duration;

/// L'état complet d'un personnage (spec §6.1).
pub struct Character {
    pub manifest: Manifest,
    pub attachment: Attachment,
    pub facing: Facing,

    /// Le nom de la pose courante — une clé du manifeste.
    ///
    /// Une `String` et non un `enum` : le vocabulaire de poses est de la
    /// DONNÉE (spec §8.2, décision n° 6). Un `enum` obligerait à recompiler
    /// pour qu'un pack tiers déclare une pose de plus.
    pub pose: String,

    /// Le moment (temps de l'horloge injectée) où la pose courante a
    /// commencé. C'est de là que `frame_courante` déduit l'image à afficher.
    pub pose_depuis: Duration,

    /// **La dernière position dérivée**, mise à jour à chaque image.
    ///
    /// ⚠️ **Ce n'est pas une entorse à la décision n° 1.** La source de
    /// vérité reste `attachment` ; ce champ n'est qu'un *cache de la dernière
    /// valeur calculée*, et il ne sert qu'à **un** endroit : amorcer une
    /// chute quand la plateforme vient de disparaître. À cet instant précis,
    /// `world_position` rend `None` — il n'y a plus rien dont dériver — et il
    /// faut bien un point d'où commencer à tomber.
    ///
    /// Ne jamais le lire ailleurs : toute lecture supplémentaire serait le
    /// premier pas vers la position stockée, et ramènerait les quatre bugs
    /// que la décision n° 1 supprime.
    pub pos_connue: Point,

    /// L'intention en cours. `None` = il faut en tirer une (couche 3).
    pub intention: Option<crate::behavior::intention::ActiveIntention>,

    /// Le point « pied » du balancier de portage, et sa vitesse.
    ///
    /// Repris de `Dragged.java` : pendant qu'on le porte, un point poursuit
    /// le curseur par un **ressort amorti**, et son retard sur le curseur
    /// choisit la pose. Ce n'est donc pas une animation qui se déroule mais
    /// un état physique — d'où l'amplitude qui suit la vitesse de la main, et
    /// le retour au repos qui repasse par les poses intermédiaires.
    ///
    /// Seulement significatif quand `attachment` vaut `Dragged` ; réinitialisé
    /// sur le curseur au moment où on l'attrape.
    pub pied_x: f32,
    pub pied_vx: f32,
}

impl Character {
    pub fn new(manifest: Manifest, attachment: Attachment, pos_connue: Point) -> Character {
        // Calculée AVANT la construction : le module `manifest` et le
        // paramètre `manifest` sont homonymes, et lire `manifest::POSE_STAND`
        // au milieu de l'initialisation d'un champ `manifest` est pénible.
        let pose = manifest::POSE_STAND.to_string();

        Character {
            manifest,
            attachment,
            facing: Facing::Right,
            pose,
            pose_depuis: Duration::ZERO,
            pos_connue,
            intention: None,
            // Sans objet tant qu'il n'est pas porté ; `reflex` les
            // réinitialise sur le curseur à l'instant de l'attrapage.
            pied_x: 0.0,
            pied_vx: 0.0,
        }
    }

    /// Change de pose — **et ne remet le chronomètre à zéro que si la pose
    /// change réellement**.
    ///
    /// C'est le détail qui fait toute la différence : appelée à 60 Hz avec le
    /// même nom, une version naïve redémarrerait l'animation à chaque image
    /// et le personnage resterait figé sur sa première frame. Bug typique,
    /// et difficile à voir puisque « ça affiche bien quelque chose ».
    ///
    /// **Une pose absente du manifeste est ignorée** : le personnage garde
    /// celle qu'il avait. C'est la couverture partielle appliquée aux
    /// RÉFLEXES (spec §8.6) — un pack sans `fall` doit quand même pouvoir
    /// tomber, il le fera dans sa pose courante. Les réflexes sont non
    /// négociables ; seul le tirage des envies se restreint (`desire.rs`).
    ///
    /// Sans ce garde, `ch.pose` désignerait une clé inexistante et
    /// `frame_courante` se rabattrait sur la frame 1 — le personnage
    /// changerait d'apparence sans raison visible.
    pub fn set_pose(&mut self, nom: &str, maintenant: Duration) {
        if !self.manifest.has_pose(nom) {
            return;
        }
        if self.pose != nom {
            self.pose = nom.to_string();
            self.pose_depuis = maintenant;
        }
    }

    /// L'image à afficher maintenant.
    ///
    /// Si la pose courante a disparu du manifeste (personnage rechargé à
    /// chaud au plan 1b avec un JSON amputé), on rend la frame 1 plutôt que
    /// de paniquer : un personnage figé sur une mauvaise image se voit et se
    /// corrige, un plantage perd la session.
    pub fn frame_courante(&self, maintenant: Duration) -> u32 {
        match self.manifest.pose(&self.pose) {
            Some(p) => p.frame_a(maintenant.saturating_sub(self.pose_depuis)),
            None => 1,
        }
    }

    /// La séquence de la pose courante est-elle arrivée à son terme ?
    ///
    /// Toujours `false` pour une pose en boucle : une boucle ne se termine
    /// pas. Sert à enchaîner après un atterrissage (la pose `land` finie, on
    /// repasse à `stand`).
    pub fn pose_terminee(&self, maintenant: Duration) -> bool {
        match self.manifest.pose(&self.pose) {
            Some(p) if !p.looping => {
                maintenant.saturating_sub(self.pose_depuis) >= p.duree_totale()
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facing_droite_demande_un_miroir() {
        // **C'est ce test qui garde la convention.** Les sprites Shimeji sont
        // dessinés tournés vers la GAUCHE — `Walk` porte `Velocity="-2,0"`
        // dans `conf/actions.xml`. Donc c'est regarder à DROITE qui demande
        // un miroir.
        //
        // L'inverse avait été supposé au départ, et le personnage marchait à
        // reculons : les pattes dans un sens, le déplacement dans l'autre.
        assert!(Facing::Right.flipped());
        assert!(!Facing::Left.flipped());
    }

    #[test]
    fn inverse_fait_un_aller_retour() {
        assert_eq!(Facing::Left.inverse(), Facing::Right);
        assert_eq!(Facing::Left.inverse().inverse(), Facing::Left);
    }

    #[test]
    fn le_signe_correspond_au_sens() {
        assert_eq!(Facing::Right.signe(), 1.0);
        assert_eq!(Facing::Left.signe(), -1.0);
    }
}
