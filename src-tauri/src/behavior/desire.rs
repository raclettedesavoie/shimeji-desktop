//! Couche 3 du comportement : l'envie (spec §7.2, décision n° 3).
//!
//! Responsabilité unique : choisir la prochaine intention par **tirage
//! pondéré**, où les signaux modifient les poids et les poses manquantes
//! retirent les options.
//!
//! > Si « inactif 2 min » *déclenchait* le sommeil, on aurait un afficheur
//! > d'état système déguisé en personnage : parfaitement prévisible, abandonné
//! > en trois jours. « Inactif 2 min » **multiplie par 8** l'envie de dormir —
//! > il s'endort presque toujours, mais parfois il s'assoit, parfois il traîne
//! > encore. **Cette marge est le produit.**
//!
//! Trois propriétés découlent de cette forme, et ce sont elles qu'il faut
//! préserver :
//!   · ajouter un signal = ajouter une ligne, aucun nouveau chemin de code ;
//!   · un personnage à couverture partielle marche sans cas particulier ;
//!   · les poids étant de la donnée, on règle son caractère sans recompiler
//!     (le plan 1b les sortira dans `config.json`).

use super::intention::Intention;
use crate::character::manifest::{Manifest, POSE_SIT, POSE_WALK};
use crate::rng::Rng;

/// Une ligne de la table d'envies.
#[derive(Debug, Clone)]
pub struct EntreeEnvie {
    pub intention: Intention,

    /// Le poids de base, avant modification par les signaux (spec §7.2).
    pub base: f32,

    /// Les poses sans lesquelles cette intention est injouable.
    ///
    /// **C'est ici que la couverture partielle devient gratuite** (spec
    /// §8.6) : une intention dont une pose manque est retirée du tirage, et
    /// il n'y a aucun cas particulier ailleurs. Un personnage sans images
    /// d'escalade ne grimpera jamais, simplement parce que l'option n'est
    /// jamais tirée.
    ///
    /// `&'static [&'static str]` : ces listes sont écrites dans le code et
    /// vivent aussi longtemps que le programme, donc aucune allocation.
    pub poses_requises: &'static [&'static str],
}

/// La table complète. Un `Vec` et non un tableau de taille fixe : le plan 1b
/// la chargera depuis `config.json`.
#[derive(Debug, Clone)]
pub struct TableEnvies {
    pub entrees: Vec<EntreeEnvie>,
}

impl TableEnvies {
    /// Les valeurs de départ de la spec §7.2, restreintes aux deux intentions
    /// de l'étape 1a.
    ///
    /// Les quatre autres lignes de la spec — manger, aller à la fenêtre
    /// active, aller voir l'autre personnage, s'amuser — arrivent aux étapes
    /// 2, 3 et 5. **Ce seront littéralement des lignes de plus dans ce
    /// `vec!`.**
    pub fn defaut() -> TableEnvies {
        TableEnvies {
            entrees: vec![
                EntreeEnvie {
                    intention: Intention::Flaner,
                    base: 5.0,
                    // Flâner sans savoir marcher n'a pas de sens.
                    poses_requises: &[POSE_WALK],
                },
                EntreeEnvie {
                    intention: Intention::SeReposer,
                    base: 1.0,
                    poses_requises: &[POSE_SIT],
                },
            ],
        }
    }

    /// Tire une intention, sans aucun biais. C'est le cas de l'étape 1a :
    /// il n'y a pas encore de signaux.
    pub fn tirer(&self, manifest: &Manifest, rng: &mut dyn Rng) -> Option<Intention> {
        self.tirer_avec(manifest, rng, |_| 1.0)
    }

    /// Tire une intention en appliquant un multiplicateur par intention.
    ///
    /// **C'est le point d'entrée des signaux** (décision n° 3). À l'étape 2,
    /// l'appelant passera une fermeture qui consulte l'inactivité, l'heure et
    /// la batterie. **Cette signature ne changera pas** — c'est tout l'enjeu :
    /// ajouter un signal ne doit toucher ni ce fichier, ni les intentions,
    /// ni les réflexes.
    ///
    /// `impl Fn(Intention) -> f32` plutôt qu'un `&dyn Fn` : le compilateur
    /// peut alors intégrer la fermeture à l'appel, et l'écriture au point
    /// d'appel reste une simple lambda.
    pub fn tirer_avec(
        &self,
        manifest: &Manifest,
        rng: &mut dyn Rng,
        multiplicateur: impl Fn(Intention) -> f32,
    ) -> Option<Intention> {
        // Les poids, dans le même ordre que `self.entrees` — c'est ce qui
        // permet de remonter de l'index tiré à l'intention.
        let poids: Vec<f32> = self
            .entrees
            .iter()
            .map(|e| {
                // Couverture partielle : une pose manquante annule le poids.
                // `all` sur une liste vide rend `true`, donc une intention
                // sans exigence est toujours jouable — ce qui est correct.
                let jouable = e.poses_requises.iter().all(|p| manifest.has_pose(p));
                if !jouable {
                    return 0.0;
                }
                e.base * multiplicateur(e.intention)
            })
            .collect();

        // `weighted` rend `None` si tous les poids sont nuls — c'est le cas
        // d'un personnage dont aucune intention n'est jouable. Ce n'est pas
        // une erreur : l'appelant le traite comme « rien à faire » et
        // réessaiera à l'image suivante.
        let index = rng.weighted(&poids)?;
        Some(self.entrees[index].intention)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::manifest::Manifest;
    use crate::rng::XorShift32;

    /// Un manifeste où l'on choisit les poses présentes, pour éprouver la
    /// couverture partielle sans toucher au disque.
    fn manifeste_avec(poses: &[&str]) -> Manifest {
        let corps: Vec<String> = poses
            .iter()
            .map(|p| format!(r#""{p}": {{ "frames": [1] }}"#))
            .collect();
        let json = format!(
            r#"{{ "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
                  "hitbox": [40,20,48,100], "poses": {{ {} }} }}"#,
            corps.join(",")
        );
        serde_json::from_str(&json).expect("manifeste de test valide")
    }

    fn compter(table: &TableEnvies, m: &Manifest, graine: u32, n: usize) -> (usize, usize, usize) {
        let mut rng = XorShift32::seeded(graine);
        let (mut flaner, mut reposer, mut rien) = (0, 0, 0);
        for _ in 0..n {
            match table.tirer(m, &mut rng) {
                Some(Intention::Flaner) => flaner += 1,
                Some(Intention::SeReposer) => reposer += 1,
                None => rien += 1,
            }
        }
        (flaner, reposer, rien)
    }

    #[test]
    fn la_table_par_defaut_privilegie_la_flanerie() {
        // Poids de la spec §7.2 : Flâner 5, Se reposer 1. Donc ~83 / 17 %.
        let m = manifeste_avec(&["stand", "walk", "sit"]);
        let (flaner, reposer, rien) = compter(&TableEnvies::defaut(), &m, 42, 10_000);

        assert_eq!(rien, 0);
        let pct = |n: usize| n as f32 / 100.0;
        assert!((pct(flaner) - 83.3).abs() < 2.0, "flâner {flaner}");
        assert!((pct(reposer) - 16.7).abs() < 2.0, "reposer {reposer}");
    }

    #[test]
    fn une_pose_manquante_retire_l_intention_du_tirage() {
        // **LE test de la couverture partielle appliquée au comportement**
        // (spec §8.6). Pas de `sit` → il ne se repose JAMAIS, et il n'y a
        // aucun cas particulier dans le code pour ça.
        let m = manifeste_avec(&["stand", "walk"]);
        let (flaner, reposer, rien) = compter(&TableEnvies::defaut(), &m, 42, 1_000);

        assert_eq!(reposer, 0, "il ne devrait jamais se reposer sans pose sit");
        assert_eq!(flaner, 1_000);
        assert_eq!(rien, 0);
    }

    #[test]
    fn sans_aucune_pose_jouable_le_tirage_rend_none() {
        // Un personnage qui n'a que `stand` ne peut ni flâner (pas de walk)
        // ni se reposer (pas de sit). `None` = « rien à faire », et
        // l'appelant le traite comme tel — ce n'est pas une erreur.
        let m = manifeste_avec(&["stand"]);
        let (_, _, rien) = compter(&TableEnvies::defaut(), &m, 42, 100);
        assert_eq!(rien, 100);
    }

    #[test]
    fn un_multiplicateur_biaise_sans_commander() {
        // **LE test de la décision n° 3.** On multiplie par 8 l'envie de se
        // reposer, comme le fera « inactif depuis 2 min » à l'étape 2.
        //
        // Poids : Flâner 5, Se reposer 1 × 8 = 8. Donc ~38 / 62 %.
        //
        // Le point n'est pas qu'il se repose : c'est qu'il flâne ENCORE
        // parfois. « Cette marge est le produit. » Un déclenchement
        // donnerait 0 / 100 et un personnage prévisible.
        let m = manifeste_avec(&["stand", "walk", "sit"]);
        let table = TableEnvies::defaut();
        let mut rng = XorShift32::seeded(7);

        let (mut flaner, mut reposer) = (0, 0);
        for _ in 0..10_000 {
            let mult = |i: Intention| match i {
                Intention::SeReposer => 8.0,
                _ => 1.0,
            };
            match table.tirer_avec(&m, &mut rng, mult) {
                Some(Intention::Flaner) => flaner += 1,
                Some(Intention::SeReposer) => reposer += 1,
                None => {}
            }
        }

        let pct = |n: usize| n as f32 / 100.0;
        assert!((pct(reposer) - 61.5).abs() < 2.0, "reposer {reposer}");
        // La marge : il flâne encore dans plus d'un tiers des cas.
        assert!(flaner > 3_000, "la marge a disparu : flâner {flaner}");
    }

    #[test]
    fn un_multiplicateur_nul_retire_l_option_completement() {
        // C'est ainsi qu'un signal pourra interdire une intention sans qu'on
        // ajoute un chemin de code (étape 2 : pas de sieste juste après
        // s'être réveillé, par exemple).
        let m = manifeste_avec(&["stand", "walk", "sit"]);
        let table = TableEnvies::defaut();
        let mut rng = XorShift32::seeded(7);

        for _ in 0..500 {
            let mult = |i: Intention| match i {
                Intention::SeReposer => 0.0,
                _ => 1.0,
            };
            assert_eq!(
                table.tirer_avec(&m, &mut rng, mult),
                Some(Intention::Flaner)
            );
        }
    }

    #[test]
    fn le_tirage_est_reproductible_a_graine_fixe() {
        // Ce qui rend la trace du mode simulation (Tâche 9) comparable d'une
        // exécution à l'autre.
        let m = manifeste_avec(&["stand", "walk", "sit"]);
        let t = TableEnvies::defaut();
        assert_eq!(compter(&t, &m, 999, 500), compter(&t, &m, 999, 500));
    }
}
