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

use super::intention::{Intention, Jeu};
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
    /// Les valeurs de départ de la spec §7.2.
    ///
    /// **Délègue à `depuis_config`** plutôt que de recopier les poids : deux
    /// listes de nombres finiraient par diverger à la première modification,
    /// et un test le vérifie.
    pub fn defaut() -> TableEnvies {
        Self::depuis_config(&crate::config::Config::default())
    }

    /// La table telle que l'utilisateur l'a réglée (décision n° 5).
    ///
    /// Restreinte aux deux intentions de l'étape 1a. Les quatre autres lignes
    /// de la spec §7.2 — manger, aller à la fenêtre active, aller voir
    /// l'autre personnage, s'amuser — arrivent aux étapes 2, 3 et 5. **Ce
    /// seront littéralement des lignes de plus dans ce `vec!`.**
    ///
    /// **Les `poses_requises` ne viennent PAS de la config**, et c'est
    /// délibéré : ce ne sont pas des préférences mais des faits — flâner
    /// exige de savoir marcher. Les laisser configurer permettrait de
    /// demander une intention injouable, ce que la couverture partielle
    /// (spec §8.6) est justement là pour rendre impossible.
    pub fn depuis_config(config: &crate::config::Config) -> TableEnvies {
        TableEnvies {
            entrees: vec![
                EntreeEnvie {
                    intention: Intention::Flaner,
                    base: config.envies.flaner,
                    // Flâner sans savoir marcher n'a pas de sens.
                    poses_requises: &[POSE_WALK],
                },
                EntreeEnvie {
                    intention: Intention::SeReposer,
                    base: config.envies.se_reposer,
                    poses_requises: &[POSE_SIT],
                },
                // **Deux lignes et non une**, pour que la couverture partielle
                // joue par animation (spec §8.6) : un pack qui n'a que l'une
                // des deux joue quand même.
                //
                // Les deux partagent le poids `envies.jouer` : « jouer ×3 »
                // dans un modificateur d'application ne distingue pas les
                // animations, et devoir régler chaque jeu séparément serait
                // du réglage pour rien.
                EntreeEnvie {
                    intention: Intention::Jouer(Jeu::TeteQuiTourne),
                    base: config.envies.jouer,
                    poses_requises: Jeu::TeteQuiTourne.poses_requises(),
                },
                EntreeEnvie {
                    intention: Intention::Jouer(Jeu::JambesQuiBalancent),
                    base: config.envies.jouer,
                    poses_requises: Jeu::JambesQuiBalancent.poses_requises(),
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

    /// Ce personnage-là sait-il faire cette intention-là ?
    ///
    /// Même règle que la couverture partielle du tirage (spec §8.6), mais
    /// posée en question plutôt qu'appliquée à un poids. **Deux appelants,
    /// et c'est la raison de l'extraire** :
    ///
    /// · le menu contextuel, pour ne proposer que ce qui est jouable — une
    ///   entrée grisée « Balancer les jambes » sur un pack qui n'a pas la
    ///   pose serait un mensonge affiché ;
    /// · `behavior::pas`, pour refuser une commande devenue injouable entre
    ///   le clic et l'image suivante. Ce n'est pas théorique : un
    ///   rechargement à chaud vers un pack plus pauvre peut se produire
    ///   pendant que le menu est ouvert.
    ///
    /// Une intention absente de la table rend `false` : on ne peut pas jouer
    /// ce qu'on ne sait pas décrire.
    pub fn jouable(&self, manifest: &Manifest, intention: Intention) -> bool {
        match self.entrees.iter().find(|e| e.intention == intention) {
            Some(e) => e.poses_requises.iter().all(|p| manifest.has_pose(p)),
            None => false,
        }
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
                // `compter` ne sert qu'aux tests d'étape 1a, dont les
                // manifestes (via `manifeste_avec`) ne déclarent jamais
                // `spinHead` ni `sitDangle` : le poids des deux lignes
                // `Jouer` est donc TOUJOURS nul, et cette branche ne peut
                // pas être atteinte. `unreachable!` plutôt qu'un compteur
                // muet : si un jour un manifeste de test gagnait ces poses
                // par erreur, on veut un panic bruyant, pas un total qui ne
                // correspond plus à `n`.
                Some(Intention::Jouer(_)) => unreachable!("aucune pose de jeu dans ce manifeste"),
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
                // Même raison que dans `compter` : ce manifeste n'a ni
                // `spinHead` ni `sitDangle`, le poids des lignes `Jouer` est
                // nul, la branche est inatteignable.
                Some(Intention::Jouer(_)) => unreachable!("aucune pose de jeu dans ce manifeste"),
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
    fn la_table_suit_les_poids_de_la_config() {
        // Le test qui prouve qu'on règle le caractère sans recompiler.
        let m = manifeste_avec(&["stand", "walk", "sit"]);

        let c = crate::config::Config {
            envies: crate::config::Envies {
                flaner: 1.0,
                se_reposer: 9.0,
                // `..Default::default()` : la Tâche 3 a ajouté `jouer` à
                // cette structure, et un littéral exhaustif ferait alors
                // échouer la compilation de CE test — pour rien (même
                // raison que le commentaire sur `ModifsAppli` dans
                // `signals.rs`).
                ..crate::config::Envies::default()
            },
            ..crate::config::Config::default()
        };

        let table = TableEnvies::depuis_config(&c);
        let mut rng = XorShift32::seeded(42);

        let (mut flaner, mut reposer) = (0, 0);
        for _ in 0..10_000 {
            match table.tirer(&m, &mut rng) {
                Some(Intention::Flaner) => flaner += 1,
                Some(Intention::SeReposer) => reposer += 1,
                // Même raison que dans `compter` : ce manifeste n'a ni
                // `spinHead` ni `sitDangle`, le poids des lignes `Jouer` est
                // nul, la branche est inatteignable.
                Some(Intention::Jouer(_)) => unreachable!("aucune pose de jeu dans ce manifeste"),
                None => {}
            }
        }

        // Rapport inversé par rapport au défaut : il se repose maintenant
        // neuf fois sur dix.
        assert!(reposer > flaner * 5, "reposer {reposer}, flâner {flaner}");
    }

    #[test]
    fn la_table_par_defaut_est_celle_de_la_config_par_defaut() {
        // Deux chemins vers les mêmes poids : ils ne doivent pas diverger par
        // inadvertance. `defaut()` doit être exactement
        // `depuis_config(&Config::default())`.
        let a = TableEnvies::defaut();
        let b = TableEnvies::depuis_config(&crate::config::Config::default());

        assert_eq!(a.entrees.len(), b.entrees.len());
        for (ea, eb) in a.entrees.iter().zip(b.entrees.iter()) {
            assert_eq!(ea.intention, eb.intention);
            assert_eq!(ea.base, eb.base);
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

    #[test]
    fn chaque_jeu_est_retire_separement_du_tirage() {
        // **LE test des deux lignes.** Un pack qui n'a que `spinHead` doit
        // jouer quand même — avec cette animation seulement. Une intention
        // `Jouer` unique exigeant les deux poses ne jouerait pas du tout.
        use crate::behavior::intention::Jeu;

        let m = manifeste_avec(&["stand", "walk", "sit", "spinHead"]);
        let table = TableEnvies::defaut();
        let mut rng = XorShift32::seeded(3);

        let mut vus = std::collections::BTreeSet::new();
        for _ in 0..2_000 {
            if let Some(i) = table.tirer(&m, &mut rng) {
                vus.insert(i);
            }
        }

        assert!(
            vus.contains(&Intention::Jouer(Jeu::TeteQuiTourne)),
            "il a spinHead : il doit pouvoir se tourner la tête"
        );
        assert!(
            !vus.contains(&Intention::Jouer(Jeu::JambesQuiBalancent)),
            "il n'a pas sitDangle : cette animation doit être retirée"
        );
    }

    #[test]
    fn le_biais_de_jeu_porte_sur_les_deux_animations() {
        // Le modificateur par application dit « jouer ×3 » sans distinguer
        // les animations : les deux lignes doivent donc en profiter.
        //
        // Pas d'import de `Jeu` ici : contrairement à
        // `chaque_jeu_est_retire_separement_du_tirage`, ce test ne nomme
        // jamais une variante précise — il ne regarde que
        // `Intention::Jouer(_)` — et un import inutilisé laissé en place est
        // un avertissement permanent, qui est exactement ce qui masque le
        // prochain avertissement réel.
        let m = manifeste_avec(&["stand", "walk", "sit", "spinHead", "sitDangle"]);
        let table = TableEnvies::defaut();
        let mut rng = XorShift32::seeded(11);

        let (mut jeux, mut autres) = (0, 0);
        for _ in 0..10_000 {
            let mult = |i: Intention| match i {
                Intention::Jouer(_) => 3.0,
                _ => 1.0,
            };
            match table.tirer_avec(&m, &mut rng, mult) {
                Some(Intention::Jouer(_)) => jeux += 1,
                Some(_) => autres += 1,
                None => {}
            }
        }

        // Poids : flâner 5, reposer 1, deux jeux à 1 × 3 = 6. Donc 6 / 12.
        let part = jeux as f32 / (jeux + autres) as f32;
        assert!((part - 0.5).abs() < 0.03, "part des jeux : {part}");
    }
}
