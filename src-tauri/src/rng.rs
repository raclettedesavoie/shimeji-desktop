//! L'aléatoire, **injecté et graînable** — deuxième contrainte de la spec §10.2.
//!
//! Responsabilité unique : produire des nombres pseudo-aléatoires reproductibles.
//! Rien dans le projet n'appelle un générateur global.
//!
//! Sans cette injection, le tirage pondéré des envies (spec §7.2) n'est pas
//! testable : on ne peut pas vérifier « 10 000 tirages à graine fixe donnent
//! la distribution attendue » si la graine n'existe pas.
//!
//! **Générateur écrit à la main plutôt qu'une crate** : c'est 15 lignes, ça
//! supprime une dépendance, et surtout le déterminisme devient une propriété
//! qu'on peut lire dans ce fichier au lieu de la déduire de la documentation
//! d'un tiers. Un mode simulation (Tâche 9) qui rejoue exactement la même
//! séquence est à ce prix.
//!
//! ⚠️ Xorshift n'est **pas** cryptographique. C'est parfaitement indifférent
//! ici : on décide si un personnage s'assoit, pas si une clé est sûre.

/// Ce que le reste du programme sait de l'aléatoire.
///
/// `&mut self` — contrairement à `Clock` : tirer un nombre **fait avancer
/// l'état** du générateur. Le type dit donc la vérité, et le compilateur
/// interdit deux tirages simultanés depuis deux endroits, ce qui casserait
/// la reproductibilité.
pub trait Rng {
    /// Le tirage primitif. Tout le reste en découle.
    fn next_u32(&mut self) -> u32;

    /// Un flottant dans `[0, 1)`.
    ///
    /// On divise par `2^32` et non par `u32::MAX`, pour que 1.0 soit
    /// strictement exclu — un `1.0` inattendu ferait sortir d'un tableau
    /// dans les tirages par index.
    fn unit_f32(&mut self) -> f32 {
        self.next_u32() as f32 / 4_294_967_296.0
    }

    /// Un flottant dans `[min, max)`. Si `max <= min`, rend `min` : une plage
    /// vide n'est pas une erreur, c'est une valeur unique — ça évite d'avoir
    /// à valider les plages qui viennent de la config (plan 1b).
    fn range(&mut self, min: f32, max: f32) -> f32 {
        if max <= min {
            return min;
        }
        min + self.unit_f32() * (max - min)
    }

    /// Tirage pondéré : rend l'index choisi, la probabilité de chaque index
    /// étant proportionnelle à son poids.
    ///
    /// **C'est le cœur de la décision n° 3** : les signaux ne commandent pas,
    /// ils multiplient des poids que l'on passe ici (spec §7.2).
    ///
    /// Rend `None` si la liste est vide ou si tous les poids sont nuls —
    /// ce qui est le cas d'un personnage à couverture partielle dont aucune
    /// intention n'est jouable (spec §8.6). `None` n'est donc pas une erreur,
    /// c'est « rien à faire », et l'appelant le traite comme tel.
    ///
    /// Les poids négatifs sont ignorés (traités comme nuls) plutôt que
    /// rejetés : les poids viennent d'un fichier de config éditable à la main,
    /// et une coquille ne doit pas tuer le personnage.
    fn weighted(&mut self, weights: &[f32]) -> Option<usize> {
        let total: f32 = weights.iter().filter(|w| **w > 0.0).sum();
        if total <= 0.0 {
            return None;
        }

        // On tire un point sur [0, total) et on avance dans les poids jusqu'à
        // le dépasser : la « roue de loterie », où chaque poids occupe un arc
        // proportionnel à sa valeur.
        let mut cible = self.unit_f32() * total;
        for (i, w) in weights.iter().enumerate() {
            if *w <= 0.0 {
                continue;
            }
            cible -= *w;
            if cible < 0.0 {
                return Some(i);
            }
        }

        // Atteint seulement par accumulation d'erreurs d'arrondi sur les
        // flottants. On rend le dernier index de poids non nul : c'est le
        // choix le plus proche de l'intention, et ça évite un `unreachable!()`
        // qui ferait paniquer le programme pour une erreur de 1e-7.
        weights.iter().rposition(|w| *w > 0.0)
    }
}

/// Xorshift 32 bits. Trois décalages-xor, et c'est tout.
pub struct XorShift32 {
    state: u32,
}

impl XorShift32 {
    /// `seed` de 0 est remplacé par une constante : xorshift reste bloqué à
    /// zéro pour toujours si son état est nul. Le corriger silencieusement
    /// vaut mieux qu'un `panic!` — une graine de 0 est ce qu'on écrit
    /// spontanément dans un test.
    pub fn seeded(seed: u32) -> Self {
        XorShift32 {
            state: if seed == 0 { 0x9E37_79B9 } else { seed },
        }
    }
}

impl Rng for XorShift32 {
    fn next_u32(&mut self) -> u32 {
        // `^=` et `<<`/`>>` sur un u32 : aucun débordement possible, les bits
        // sortis sont perdus. Pas besoin de `wrapping_*` ici.
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meme_graine_meme_sequence() {
        // La propriété qui rend le mode simulation utile : rejouable.
        let mut a = XorShift32::seeded(42);
        let mut b = XorShift32::seeded(42);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn graines_differentes_sequences_differentes() {
        let mut a = XorShift32::seeded(1);
        let mut b = XorShift32::seeded(2);
        assert_ne!(a.next_u32(), b.next_u32());
    }

    #[test]
    fn graine_zero_ne_reste_pas_bloquee() {
        let mut r = XorShift32::seeded(0);
        let premier = r.next_u32();
        let second = r.next_u32();
        assert_ne!(premier, 0);
        assert_ne!(premier, second);
    }

    #[test]
    fn unit_f32_reste_dans_zero_un() {
        let mut r = XorShift32::seeded(7);
        for _ in 0..10_000 {
            let v = r.unit_f32();
            assert!((0.0..1.0).contains(&v), "valeur hors bornes : {v}");
        }
    }

    #[test]
    fn range_inverse_rend_le_minimum() {
        let mut r = XorShift32::seeded(7);
        assert_eq!(r.range(5.0, 5.0), 5.0);
        assert_eq!(r.range(9.0, 2.0), 9.0);
    }

    #[test]
    fn weighted_respecte_les_proportions() {
        // Le test que la spec §10.1 demande : 10 000 tirages à graine fixe,
        // distribution attendue. Poids 1 / 3 / 6 → environ 10 / 30 / 60 %.
        let mut r = XorShift32::seeded(12345);
        let poids = [1.0, 3.0, 6.0];
        let mut comptes = [0usize; 3];

        for _ in 0..10_000 {
            let i = r.weighted(&poids).expect("poids non nuls");
            comptes[i] += 1;
        }

        // Marge de 2 points de pourcentage : large devant l'erreur
        // d'échantillonnage sur 10 000 tirages, serrée devant une vraie
        // erreur de proportion.
        let pct = |n: usize| n as f32 / 100.0;
        assert!((pct(comptes[0]) - 10.0).abs() < 2.0, "{:?}", comptes);
        assert!((pct(comptes[1]) - 30.0).abs() < 2.0, "{:?}", comptes);
        assert!((pct(comptes[2]) - 60.0).abs() < 2.0, "{:?}", comptes);
    }

    #[test]
    fn weighted_ignore_les_poids_nuls_et_negatifs() {
        let mut r = XorShift32::seeded(99);
        // Seul l'index 2 est jouable : c'est le cas « couverture partielle ».
        let poids = [0.0, -5.0, 1.0];
        for _ in 0..1_000 {
            assert_eq!(r.weighted(&poids), Some(2));
        }
    }

    #[test]
    fn weighted_rend_none_quand_rien_nest_jouable() {
        let mut r = XorShift32::seeded(99);
        assert_eq!(r.weighted(&[]), None);
        assert_eq!(r.weighted(&[0.0, 0.0]), None);
    }
}
