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
    /// Convention : les sprites sont dessinés **tournés vers la droite**.
    /// Regarder à gauche demande donc un miroir. Si un pack tiers est dessiné
    /// dans l'autre sens, il paraîtra à l'envers — c'est un défaut de contenu
    /// qui se corrige dans le manifeste, pas ici.
    pub fn flipped(&self) -> bool {
        matches!(self, Facing::Left)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facing_gauche_demande_un_miroir() {
        assert!(Facing::Left.flipped());
        assert!(!Facing::Right.flipped());
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
