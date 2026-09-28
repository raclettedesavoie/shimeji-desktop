//! L'écran qu'une application en plein écran a pris pour elle — un jeu, une
//! vidéo (demande de l'auteur, 2026-09-28, à propos de League of Legends).
//!
//! Responsabilité unique, et **fonctions pures** : dire ce qu'est « couvrir
//! un écran », et quels écrans restent habitables. La détection elle-même
//! (quelle fenêtre, sur quel moniteur) vit dans la sonde,
//! `probe::win32::Win32Probe::ecran_en_plein_ecran`.
//!
//! **Retirer l'écran du recensement suffit à tout le reste**, et c'est ce qui
//! rend ce module presque vide : sans lui dans le monde, ses plateformes
//! disparaissent (le voisin retrouve son mur de ce côté, personne n'y entre
//! plus en marchant) ; sans lui dans `ecrans_courants`, sa fenêtre d'overlay
//! se ferme — et une fenêtre transparente toujours au premier plan posée sur
//! un jeu lui coûte des images par seconde, en empêchant Windows de lui
//! donner l'écran directement.

use crate::geom::{Point, Rect};
use crate::probe::ScreenInfo;

/// La fenêtre `fenetre` recouvre-t-elle **entièrement** l'écran `ecran` ?
///
/// Comparaison des quatre bords, et non égalité stricte : certains jeux
/// déclarent une fenêtre qui déborde d'un pixel ou deux de leur écran.
///
/// On compare au rectangle **complet** de l'écran (`rcMonitor`), barre des
/// tâches comprise. C'est ce qui distingue le plein écran d'une fenêtre
/// simplement agrandie, qui s'arrête à la barre des tâches — sauf quand
/// celle-ci est masquée automatiquement : voir `est_en_plein_ecran`.
pub fn couvre(fenetre: Rect, ecran: Rect) -> bool {
    fenetre.left() <= ecran.left()
        && fenetre.top() <= ecran.top()
        && fenetre.right() >= ecran.right()
        && fenetre.bottom() >= ecran.bottom()
}

/// La fenêtre est-elle en plein écran sur `ecran` ?
///
/// `agrandie` : l'état « agrandie » de Windows (`IsZoomed`).
///
/// ⚠️ **Une fenêtre agrandie PEUT être en plein écran**, et c'est le cas de
/// Chrome : passé en plein écran depuis une fenêtre agrandie (Twitch, le
/// 2026-09-28), il GARDE cet état. La première version écartait toute fenêtre
/// agrandie, et Twitch en plein écran ne chassait personne.
///
/// Ce qui les départage, relevé sur la machine de l'auteur :
///
/// | Fenêtre | Rectangle | Écran |
/// |---|---|---|
/// | Chrome, Twitch en plein écran | (0, 0, 1920, 1080) | (0, 0, 1920, 1080) |
/// | VS Code agrandi | (1912, −8, 3848, 1040) | (1920, 0, 3840, 1080) |
///
/// Une fenêtre agrandie déborde de sa bordure invisible (~8 px, piège
/// Windows n° 1) et s'arrête à la barre des tâches. Avec une barre des
/// tâches masquée automatiquement, elle COUVRE l'écran — mais toujours en
/// débordant. D'où la règle : agrandie, elle doit coller **exactement** à
/// l'écran ; sinon, la couvrir suffit (certains jeux débordent d'un pixel).
pub fn est_en_plein_ecran(fenetre: Rect, ecran: Rect, agrandie: bool) -> bool {
    if agrandie {
        fenetre == ecran
    } else {
        couvre(fenetre, ecran)
    }
}

/// Les écrans où les personnages ont encore le droit de vivre : tous, moins
/// ceux qu'ont pris des applications en plein écran.
///
/// Peut rendre une liste **vide** — un jeu sur une machine à un seul écran,
/// ou un plein écran sur chacun. C'est à l'appelant d'en conclure qu'il faut
/// tout cacher, comme pendant un verrouillage de session.
///
/// `&[u64]` : une tranche, pour accepter un `Vec` comme un tableau littéral
/// dans les tests.
///
/// `.copied()` : `ScreenInfo` est `Copy`, on recopie donc chaque écran retenu
/// dans la nouvelle liste plutôt que d'en garder une référence — qui
/// emprunterait `tous` aussi longtemps que vit le résultat.
pub fn ecrans_habitables(tous: &[ScreenInfo], pris: &[u64]) -> Vec<ScreenInfo> {
    tous.iter().filter(|e| !pris.contains(&e.id)).copied().collect()
}

/// Ce point est-il sur l'écran `ecran` ? Sert à trouver ceux qui y étaient au
/// moment où le plein écran l'a pris, pour les faire déménager.
///
/// `bounds` et non `work_area` : un personnage porté ou en chute par-dessus
/// la barre des tâches est bien sur cet écran-là.
pub fn est_sur(ecran: &ScreenInfo, p: Point) -> bool {
    ecran.bounds.contains(p)
}

// Les tests de ce module vivent dans `plein_ecran_tests.rs`.
#[cfg(test)]
#[path = "plein_ecran_tests.rs"]
mod tests;
