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
use crate::character::manifest::{
    Manifest, POSE_CLIMB_WALL, POSE_GRAB_WALL, POSE_SIT, POSE_WALK,
};
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
                // L'escalade (étape 4a). Les deux poses suffisent : sans
                // `grabWall` il ne saurait pas tenir, sans `climbWall` il ne
                // saurait pas monter. Un pack qui n'a ni l'une ni l'autre ne
                // grimpera JAMAIS, et il n'y a aucun cas particulier ailleurs
                // (spec §8.6).
                //
                // `climbCeiling` n'y figure pas volontairement : un pack qui
                // sait grimper mais pas se suspendre grimpe quand même, et
                // s'arrête en haut du mur (Tâche 6).
                EntreeEnvie {
                    intention: Intention::Grimper,
                    base: config.envies.grimper,
                    poses_requises: &[POSE_GRAB_WALL, POSE_CLIMB_WALL],
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

// Les tests de ce module vivent dans `desire_tests.rs`
// (sortis d ici le 2026-09-14 : ils faisaient 270 des 467 lignes).
#[cfg(test)]
#[path = "desire_tests.rs"]
mod tests;
