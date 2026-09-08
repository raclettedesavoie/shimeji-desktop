//! L'horloge, **injectée** — première des trois contraintes de la spec §10.2.
//!
//! Responsabilité unique : dire quel temps s'est écoulé. Rien dans le projet
//! n'appelle `Instant::now()` en dehors de `SystemClock` ci-dessous.
//!
//! Sans cette injection, rien de temporel n'est testable : le délai d'abandon
//! de 20 s (spec §7.3) demanderait un test de 20 secondes, et la durée
//! d'affichage d'une frame ne se vérifierait pas du tout.

use std::cell::Cell;
use std::time::{Duration, Instant};

/// Ce que le reste du programme sait du temps : une durée depuis le démarrage.
///
/// **Volontairement monotone et relative**, pas une date. L'heure du jour
/// (pour « il mange à midi ») est un SIGNAL, qui arrive à l'étape 2 ; elle
/// s'ajoutera comme une seconde méthode, sans rien changer d'ici.
///
/// `&self` et non `&mut self` : lire l'heure ne modifie rien de l'extérieur.
/// C'est ce qui permet de partager une horloge sans emprunt mutable.
pub trait Clock {
    fn elapsed(&self) -> Duration;
}

/// L'horloge réelle. Le **seul** endroit du projet où `Instant::now()` est
/// appelé — hormis le point de mesure de la boucle 60 Hz (Tâche 10), qui
/// mesure la CADENCE et non le temps du comportement.
pub struct SystemClock {
    start: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        SystemClock {
            start: Instant::now(),
        }
    }
}

// `Default` : demandé par clippy dès qu'un `new()` sans argument existe, et
// utile pour écrire `SystemClock::default()`.
impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

/// L'horloge des tests : elle n'avance que quand on le lui demande.
///
/// `Cell<Duration>` donne la « mutabilité intérieure » : on modifie la valeur
/// à travers un `&self` (non mutable). C'est nécessaire parce que
/// `Clock::elapsed` prend `&self` — un test qui détiendrait un `&FakeClock`
/// ne pourrait sinon pas le faire avancer. `Cell` convient ici parce que
/// `Duration` est `Copy` et qu'on reste sur un seul thread.
pub struct FakeClock {
    now: Cell<Duration>,
}

impl FakeClock {
    pub fn new() -> Self {
        FakeClock {
            now: Cell::new(Duration::ZERO),
        }
    }

    /// Avance de `d`. C'est la façon normale de tester un délai.
    pub fn advance(&self, d: Duration) {
        self.now.set(self.now.get() + d);
    }

    /// Positionne le temps absolu. Utile pour aller droit au bord d'un délai.
    pub fn set(&self, d: Duration) {
        self.now.set(d);
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for FakeClock {
    fn elapsed(&self) -> Duration {
        self.now.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_demarre_a_zero() {
        let c = FakeClock::new();
        assert_eq!(c.elapsed(), Duration::ZERO);
    }

    #[test]
    fn fake_clock_avance_par_cumul() {
        let c = FakeClock::new();
        c.advance(Duration::from_millis(500));
        c.advance(Duration::from_millis(300));
        assert_eq!(c.elapsed(), Duration::from_millis(800));
    }

    #[test]
    fn fake_clock_avance_a_travers_une_reference_non_mutable() {
        // C'est tout l'intérêt du Cell : le trait expose `&self`, donc un
        // test qui ne détient qu'un `&dyn Clock` doit pouvoir faire avancer
        // le temps par ailleurs.
        let c = FakeClock::new();
        let vue: &dyn Clock = &c;
        c.advance(Duration::from_secs(20));
        assert_eq!(vue.elapsed(), Duration::from_secs(20));
    }

    #[test]
    fn fake_clock_peut_sauter_a_une_date() {
        let c = FakeClock::new();
        c.advance(Duration::from_secs(5));
        c.set(Duration::from_secs(1));
        assert_eq!(c.elapsed(), Duration::from_secs(1));
    }
}
