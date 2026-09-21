//! La santé de la file de messages du thread principal (spec
//! « régulation de charge » §5.1).
//!
//! Responsabilité unique : publier **une durée** — le temps qu'un jeton met à
//! être traité par le thread principal. Ce module ne décide rien : il ne sait
//! pas ce qu'on fera de cette durée, et surtout pas qu'elle finira en envie de
//! se reposer.
//!
//! # Pourquoi mesurer plutôt que compter les personnages
//!
//! Le nombre de personnages qu'une machine encaisse dépend de la machine — un
//! cœur plus rapide vide la file plus vite. Un plafond en dur coderait donc en
//! dur une valeur qui n'est vraie que sur la machine où on l'a écrite. La
//! latence, elle, se mesure dans l'unité qui compte, partout.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// La dernière latence mesurée, et le drapeau du jeton en vol.
///
/// `Atomic*` et non `Mutex` : les deux accès sont une lecture et une écriture
/// de valeur simple, et le thread principal ne doit jamais attendre un verrou
/// que nous tiendrions — ce serait ajouter à l'embouteillage qu'on mesure.
pub struct Moniteur {
    /// En **microsecondes**. `u64` parce qu'il n'existe pas d'`AtomicDuration`,
    /// et que µs donne largement la finesse utile (on parle de dizaines de ms).
    derniere_latence_us: AtomicU64,

    /// Vrai pendant qu'un jeton attend son tour dans la file.
    en_vol: AtomicBool,
}

impl Moniteur {
    pub fn nouveau() -> Moniteur {
        Moniteur {
            derniere_latence_us: AtomicU64::new(0),
            en_vol: AtomicBool::new(false),
        }
    }

    /// La dernière latence connue. Zéro tant que rien n'a été mesuré.
    pub fn latence(&self) -> Duration {
        Duration::from_micros(self.derniere_latence_us.load(Ordering::Relaxed))
    }

    /// Le jeton est revenu : on publie sa durée et on libère la place.
    pub fn enregistrer(&self, d: Duration) {
        // `try_from` plutôt qu'un `as u64` : `as` sur un `u128` tronquerait en
        // silence. Le cas ne se produira jamais (il faudrait 584 000 ans de
        // latence), mais on préfère saturer que mentir.
        let us = u64::try_from(d.as_micros()).unwrap_or(u64::MAX);
        self.derniere_latence_us.store(us, Ordering::Relaxed);
        self.en_vol.store(false, Ordering::Relaxed);
    }

    /// Réserve la place du jeton. Rend `false` si un jeton attend déjà.
    ///
    /// `swap` et non `load` puis `store` : les deux en une seule opération
    /// atomique, sinon deux appels simultanés pourraient tous deux voir
    /// `false` et décoller ensemble.
    pub fn prendre_le_vol(&self) -> bool {
        !self.en_vol.swap(true, Ordering::Relaxed)
    }
}

/// Poste un jeton horodaté dans la file du thread principal.
///
/// C'est **le même canal** que `eval` et `set_position` empruntent
/// (`run_on_main_thread` → `Message::Task` → `PostMessageW`) : le temps que met
/// ce jeton EST la latence que subissent les déplacements et les menus. Y
/// mesurer autre chose — un `GetTickCount`, une charge CPU — ne dirait rien de
/// ce qui nous intéresse.
///
/// Ne poste rien si un jeton est déjà en vol : sous saturation, en ajouter
/// aggraverait ce qu'on mesure.
pub fn sonder(app: &tauri::AppHandle, moniteur: &Arc<Moniteur>) {
    if !moniteur.prendre_le_vol() {
        return;
    }

    let depart = Instant::now();
    // `clone` de l'`Arc` : la fermeture doit vivre jusqu'à son exécution sur
    // l'autre thread, donc elle possède sa propre référence comptée.
    let m = moniteur.clone();

    // `move` : la fermeture emporte `depart` et `m`. Elle est `FnOnce + Send`,
    // ce qu'exige `run_on_main_thread`.
    let poste = app.run_on_main_thread(move || {
        m.enregistrer(depart.elapsed());
    });

    // La file a refusé le jeton (elle est pleine, ou l'application se ferme).
    // On libère le drapeau, sinon un unique refus suspendrait la mesure pour
    // toujours — et c'est précisément quand ça sature qu'on veut la voir.
    //
    // `Duration::MAX` et non zéro : un refus est le pire état possible de la
    // file, pas le meilleur. L'écrire zéro ferait taire la régulation à
    // l'instant exact où elle doit agir.
    if poste.is_err() {
        moniteur.enregistrer(Duration::MAX);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_latence_non_mesuree_vaut_zero() {
        let m = Moniteur::nouveau();
        assert_eq!(m.latence(), Duration::ZERO);
    }

    #[test]
    fn la_derniere_mesure_est_celle_qui_est_lue() {
        let m = Moniteur::nouveau();
        m.enregistrer(Duration::from_millis(42));
        assert_eq!(m.latence(), Duration::from_millis(42));
        m.enregistrer(Duration::from_millis(7));
        assert_eq!(m.latence(), Duration::from_millis(7));
    }

    /// Le cœur du drapeau : un seul jeton en vol à la fois.
    #[test]
    fn un_seul_jeton_en_vol_a_la_fois() {
        let m = Moniteur::nouveau();
        // Le premier décollage est accordé…
        assert!(m.prendre_le_vol());
        // …le second est refusé tant que rien n'est revenu.
        assert!(!m.prendre_le_vol());

        // Le retour du jeton libère la place.
        m.enregistrer(Duration::from_millis(3));
        assert!(m.prendre_le_vol());
    }
}
