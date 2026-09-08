//! Couche 2 du comportement : l'intention en cours (spec §7.1).
//! Complété à la Tâche 8 ; ce qui suit est le strict nécessaire pour que les
//! réflexes puissent annuler une intention.

use std::time::Duration;

/// Ce que le personnage est en train d'essayer de faire. **Une seule à la
/// fois** (spec §7.1). `AllerA` et `Jouer` arrivent aux étapes 4 et 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intention {
    Flaner,
    SeReposer,
}

/// Une intention en cours, avec le moment où elle a commencé — c'est de là
/// que se déduit le délai d'abandon de 20 s (décision n° 4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveIntention {
    pub kind: Intention,
    pub depuis: Duration,
}

impl ActiveIntention {
    pub fn nouvelle(kind: Intention, maintenant: Duration) -> Self {
        ActiveIntention {
            kind,
            depuis: maintenant,
        }
    }
}
