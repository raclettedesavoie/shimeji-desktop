//! L'action TENUE d'un personnage : un ordre du menu qui dure tant qu'on ne
//! l'arrête pas (spec « menu sur mesure et actions tenues » §2).
//!
//! Responsabilité unique : dire quelle intention sert une tenue, et quand
//! l'assoupissement la remplace. Fonctions pures — ni Tauri, ni écran.

use super::desire::TableEnvies;
use super::intention::{ActiveIntention, Intention, Jeu};
use super::Entrees;
use crate::character::manifest::Manifest;
use crate::character::Character;
use crate::config::Reglages;
use std::time::Duration;

/// Ce qu'un personnage fait tant qu'on ne l'arrête pas.
///
/// **Aucune coche n'est stockée ailleurs** : le menu la DÉDUIT de ce champ
/// (spec §2.1), la même règle que l'interrupteur de la bibliothèque — une
/// seule vérité, rien à tenir d'accord.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tenue {
    Asseoir,
    BalancerLesJambes,
    Flaner,
    /// Il vit sur les murs et le plafond, sans jamais revenir au sol de
    /// lui-même (spec §2.4).
    Grimper,
    /// Immobile sur sa paroi.
    ResterAccroche,
}

impl Tenue {
    /// Toutes les tenues, pour qui doit les passer en revue — la boucle,
    /// quand elle demande à chacun ce qu'il peut tenir.
    pub const TOUTES: [Tenue; 5] = [
        Tenue::Asseoir,
        Tenue::BalancerLesJambes,
        Tenue::Flaner,
        Tenue::Grimper,
        Tenue::ResterAccroche,
    ];

    /// L'intention qui sert cette tenue.
    ///
    /// `ResterAccroche` rend `Grimper` : c'est une escalade arrêtée, et la
    /// règle de sécurité du monde vertical (`behavior::pas`) n'exempte que
    /// `Grimper` — une autre intention le ferait tomber.
    pub fn intention(self) -> Intention {
        match self {
            Tenue::Asseoir => Intention::SeReposer,
            Tenue::BalancerLesJambes => Intention::Jouer(Jeu::JambesQuiBalancent),
            Tenue::Flaner => Intention::Flaner,
            Tenue::Grimper | Tenue::ResterAccroche => Intention::Grimper,
        }
    }

    /// Vrai pour une tenue qui se joue au sol. C'est elle, et elle seule,
    /// que l'inactivité fait s'assoupir (spec §2.3).
    pub fn au_sol(self) -> bool {
        matches!(self, Tenue::Asseoir | Tenue::BalancerLesJambes | Tenue::Flaner)
    }
}

/// L'intention à poser pour servir `t`, maintenant.
///
/// # L'assoupissement (spec §2.3)
///
/// Une tenue de sol, quand l'utilisateur est parti, pose `SeReposer` à la
/// place : il s'assoit, puis `se_reposer` l'endort par sa propre règle — la
/// MÊME condition que le sommeil ordinaire, recopiée ici sans rien y
/// ajouter. À son retour, l'interruption de `behavior::pas` le réveille, et
/// la tenue relance sa propre intention. Elle ne s'est jamais décochée.
///
/// Le test porte sur un POIDS et sur un FAIT, exactement comme
/// `se_reposer` : il n'est pas un déclenchement (décision n° 3).
///
/// `table.jouable(…, SeReposer)` : un pack sans pose assise ne s'assoupit
/// pas, sans quoi `se_reposer` échouerait à chaque image, et la tenue
/// relancerait l'échec 60 fois par seconde.
pub fn intention_pour(
    t: Tenue,
    e: &Entrees,
    reglages: &Reglages,
    table: &TableEnvies,
    manifeste: &Manifest,
    sur_paroi: bool,
    maintenant: Duration,
) -> ActiveIntention {
    let assoupi = t.au_sol()
        && !e.utilisateur_actif
        && e.biais.pour(Intention::SeReposer) >= reglages.seuil_sommeil
        && table.jouable(manifeste, Intention::SeReposer);
    if assoupi {
        return ActiveIntention::nouvelle(Intention::SeReposer, maintenant);
    }

    match t {
        // Un ORDRE : il court jusqu'au mur, et sur un mur il reprend
        // l'escalade là où il est (phase `Choisir`).
        Tenue::Grimper => ActiveIntention::grimper_sur_ordre(maintenant),
        // Déjà sur une paroi : il se fige là — l'intention qu'un lancer
        // contre une paroi installe déjà. Au sol : il part au mur (file
        // comprise), monte, et s'y figera — la pause `Accroche` d'un
        // `ResterAccroche` ne finit jamais (`intention::grimper`).
        Tenue::ResterAccroche if sur_paroi => ActiveIntention::accroche(maintenant),
        Tenue::ResterAccroche => ActiveIntention::grimper_sur_ordre(maintenant),
        autre => ActiveIntention::nouvelle(autre.intention(), maintenant),
    }
}

/// Ce personnage peut-il tenir `t`, LÀ où il est, avec SON pack ?
///
/// La règle unique de `Commande::Tenir` dans `behavior::pas`, et celle que
/// la section « Tout le monde » applique pour ne compter que les présents
/// capables (sans quoi une ligne cochée par tous ceux qui PEUVENT ne se
/// décocherait jamais).
///
/// - **Posé** (`Attachment::On`) : porté ou en chute, les réflexes passent
///   avant et `pas` ne verrait même pas la commande.
/// - **À sa place** : une tenue de sol sur une paroi le ferait tomber.
///   `Grimper` et `ResterAccroche` valent partout — au sol il part au mur ;
///   sur un mur il reprend, ou se fige (« Rester accroché » au sol depuis le
///   2026-09-24, pour « Tout le monde › Rester accroché »).
/// - **Jouable** : son pack a les poses (couverture partielle, spec §8.6).
pub fn peut_tenir(t: Tenue, ch: &Character, table: &TableEnvies) -> bool {
    use crate::character::attach::Attachment;
    use crate::geom::Face;

    // `let … else` : pas posé, pas de tenue possible — on sort tout de suite.
    let Attachment::On { face, .. } = ch.attachment else {
        return false;
    };
    let sur_une_paroi = face != Face::Top;
    let a_sa_place = match t {
        Tenue::Grimper | Tenue::ResterAccroche => true,
        _ => !sur_une_paroi,
    };
    a_sa_place && table.jouable(&ch.manifest, t.intention())
}

/// Son intention en cours sert-elle une tenue AU MUR ?
///
/// C'est ce qui l'exempte du délai d'abandon (`intention::poursuivre`) : au
/// mur, rien n'interrompt une tenue (spec §2.3). Au sol, pas d'exemption, et
/// c'est voulu — une flânerie ne se termine QUE par ce délai. C'est donc lui
/// qui la relance toutes les 20 s, et qui laisse l'assoupissement s'y
/// brancher.
///
/// `matches!` sur un couple : les deux `Option` doivent être `Some`, et la
/// garde `if` pose la condition sur leur contenu.
pub fn sert_une_tenue_au_mur(ch: &Character) -> bool {
    matches!(
        (ch.tenue, ch.intention),
        (Some(t), Some(ai)) if !t.au_sol() && ai.kind == Intention::Grimper
    )
}

#[cfg(test)]
#[path = "tenue_tests.rs"]
mod tests;
