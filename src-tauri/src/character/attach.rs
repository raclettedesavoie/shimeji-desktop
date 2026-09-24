//! L'accroche : où le personnage se trouve, et comment on le sait.
//!
//! **Responsabilité unique, et la plus importante du projet : la position
//! d'un personnage accroché n'est jamais stockée, elle est DÉRIVÉE** de la
//! plateforme à chaque image (décision n° 1, spec §6.2).
//!
//! Ce que cette seule règle rend gratuit :
//!   · la fenêtre est déplacée      → il voyage avec elle, zéro ligne
//!   · elle est redimensionnée      → il garde sa distance au bord, zéro ligne
//!   · elle rétrécit sous lui       → un test : `hors_bornes`
//!   · elle se ferme ou se minimise → le même test
//!
//! Un personnage assis sur une barre de titre qu'on balade est *le* moment
//! qui fait sourire avec un Shimeji. Ici c'est une conséquence du modèle, pas
//! une fonctionnalité écrite.
//!
//! ⚠️ **Ne jamais ajouter un champ `pos` à `Attachment::On`**, même « pour
//! éviter de recalculer ». Le recalcul est trois additions ; le champ
//! ramènerait les quatre bugs ci-dessus d'un coup.

use super::manifest::Manifest;
use super::Facing;
use crate::geom::{Face, Point, Rect, Vec2};
use crate::world::{PlatformId, World};

/// Où est le personnage — sous la forme qui rend la décision n° 1 possible.
///
/// Noter l'asymétrie : `On` et `Dragged` n'ont **aucune** coordonnée absolue,
/// `Falling` en a. Ce n'est pas une incohérence, c'est le modèle : en chute
/// il n'est attaché à rien, donc il n'y a rien dont dériver sa position
/// (spec §6.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Attachment {
    /// Accroché à une face d'une plateforme, à `offset` du bord.
    On {
        platform: PlatformId,
        face: Face,
        /// Distance le long de la face, depuis son extrémité la plus
        /// « petite » (gauche pour les horizontales, haut pour les
        /// verticales). **Pas** une coordonnée écran.
        offset: f32,
    },

    /// En chute libre. Le seul état à position absolue.
    Falling { pos: Point, vel: Vec2 },

    /// Tenu par la souris. Aucune donnée : la position se dérive du curseur,
    /// exactement comme `On` se dérive de sa plateforme.
    Dragged,
}

/// **La fonction centrale du projet.** La position écran du point d'ancrage du
/// personnage, recalculée depuis l'état courant du monde.
///
/// `souris` sert au cas `Dragged`. Le passer toujours, plutôt que de stocker
/// la position dans la variante, garde `Dragged` sans données — et donc
/// impossible à désynchroniser du curseur.
///
/// Rend `None` quand la plateforme a disparu. **Ce `None` est le seul
/// mécanisme de détection de la fermeture d'une fenêtre** dont le projet a
/// besoin (spec §6.2) : l'appelant en déduit une chute.
pub fn world_position(att: &Attachment, world: &World, souris: Point) -> Option<Point> {
    match att {
        Attachment::On {
            platform,
            face,
            offset,
        } => {
            // `let … else` : si la plateforme n'existe plus, on sort avec
            // None. Équivalent d'un `match` dont la branche None ferait
            // `return None`, en une ligne.
            let Some(plat) = world.get(*platform) else {
                return None;
            };

            // On repart du rectangle COURANT, jamais d'une position
            // mémorisée : c'est ce qui rend gratuit le déplacement de la
            // plateforme (décision n° 1).
            Some(plat.rect.point_on(*face, *offset))
        }

        Attachment::Falling { pos, .. } => Some(*pos),

        Attachment::Dragged => Some(souris),
    }
}

/// Le personnage a-t-il perdu son appui ?
///
/// Deux causes, un seul test — c'est la promesse de la spec §6.2 :
///   · la plateforme a disparu (fenêtre fermée, écran débranché) ;
///   · l'offset est sorti de la face (la plateforme a rétréci sous lui).
///
/// `Falling` et `Dragged` rendent toujours `false` : ils n'ont pas d'appui à
/// perdre.
pub fn hors_bornes(att: &Attachment, world: &World) -> bool {
    match att {
        Attachment::On {
            platform,
            face,
            offset,
        } => {
            let Some(plat) = world.get(*platform) else {
                // Plateforme disparue : c'est le cas le plus fréquent à
                // l'étape 4, et il ne demande pas une ligne de plus.
                return true;
            };

            // La face doit exister ET l'offset y tenir. À l'étape 4, un bord
            // recouvert disparaît de `faces` (décision n° 2) : ce test
            // deviendra alors aussi celui de l'occlusion, sans changer.
            if !plat.has_face(*face) {
                return true;
            }

            *offset < 0.0 || *offset > plat.rect.face_length(*face)
        }

        Attachment::Falling { .. } | Attachment::Dragged => false,
    }
}

/// Le coin supérieur gauche où placer la **fenêtre**, pour que l'ancre de
/// l'image affichée tombe exactement sur `pos`.
///
/// « Positionner » devient ainsi « place l'ancre ici » (spec §8.3).
///
/// > ⚠️ **Ne jamais ajouter ici une compensation de décalage** pour rattraper
/// > un sprite qui « paraît trop haut » ou décalé selon le sens de marche.
/// > L'ancre existe exactement pour rendre ça inutile : si le personnage est
/// > mal posé, c'est l'ancre du manifeste qu'il faut corriger — c'est de la
/// > donnée, elle se règle sans recompiler. Le prototype VSCode contenait un
/// > tel bricolage, asymétrique selon le sens de marche ; il disparaît ici
/// > (spec §8.3).
///
/// # Pourquoi `frame` et non plus la `&Pose`
///
/// La taille et l'ancre sont des propriétés de l'**image**, pas de la pose
/// (voir `manifest::InfoFrame`) : un pack tiers a souvent des frames de
/// tailles différentes. On passe donc le numéro de l'image affichée et le
/// nom de la pose, et les deux replis — image → pose → défaut — sont faits
/// par `Manifest::ancre_de_frame`.
pub fn window_top_left(
    pos: Point,
    frame: u32,
    pose_nom: &str,
    manifest: &Manifest,
    scale_affichage: f32,
    facing: Facing,
) -> Point {
    // L'échelle totale : celle du manifeste combinée à celle du moniteur
    // (spec §3.4, §8.5). C'est le SEUL usage légitime du facteur d'échelle —
    // il ne convertit jamais une coordonnée.
    let echelle = manifest.scale * scale_affichage;

    // ⚠️ La largeur de **cette image**, et non `manifest.frame_size` qui est
    // la plus grande du pack. Miroiter sur la plus grande décalerait le
    // personnage de la différence à chaque demi-tour (mesuré le 2026-09-20 :
    // 85 px sur `the-simba-cub`).
    let largeur_boite = manifest.taille_de_frame(frame)[0] as f32;
    let ancre = manifest.ancre_de_frame(frame, pose_nom);

    // Quand le sprite est retourné, l'ancre l'est aussi : une ancre à 100 px
    // du bord gauche se retrouve à `largeur - 100` du bord gauche.
    //
    // Sans cette symétrie, un personnage dont l'ancre est décentrée sauterait
    // latéralement à chaque demi-tour. C'est du miroir, pas de la
    // compensation : on retourne l'ancre avec l'image, on ne corrige pas
    // après coup.
    let ancre_x = if facing.flipped() {
        largeur_boite - ancre[0]
    } else {
        ancre[0]
    };

    // Le miroir est horizontal : `y` n'est pas concerné.
    let ancre_y = ancre[1];

    Point::new(pos.x - ancre_x * echelle, pos.y - ancre_y * echelle)
}

/// La position à donner après un changement de pose, pour que **le sprite ne
/// bouge pas à l'écran**.
///
/// Deux poses peuvent avoir des ancres différentes — c'est même le but des
/// ancres. Mais `pos` désigne « le point de la scène sur lequel l'ancre se
/// pose » : garder le même `pos` en changeant d'ancre **téléporte le
/// sprite**.
///
/// > C'est exactement le bug qu'on a eu au relâchement d'un portage : les
/// > poses `dragged*` ont l'ancre `[64, 8]` (le curseur tient la tête) et
/// > `fall` a `[64, 128]` (les pieds). En gardant `pos = curseur`, le
/// > personnage repartait **120 px plus haut** que là où on le tenait.
///
/// La conversion passe par le coin de la fenêtre, qui lui est une vraie
/// position d'écran : on le calcule avec l'ancienne pose, puis on remonte à
/// `pos` avec la nouvelle.
///
/// `facing` est le même pour les deux poses — appeler cette fonction en
/// changeant aussi d'orientation n'aurait pas de sens, et le miroir se
/// compense donc de lui-même.
pub fn position_conservant_le_sprite(
    pos: Point,
    frame_avant: u32,
    pose_avant: &str,
    frame_apres: u32,
    pose_apres: &str,
    manifest: &Manifest,
    scale_affichage: f32,
    facing: Facing,
) -> Point {
    let coin = window_top_left(pos, frame_avant, pose_avant, manifest, scale_affichage, facing);

    // L'inverse de `window_top_left`, avec la pose d'arrivée — et donc avec
    // la boîte de SON image, qui n'a pas forcément la même largeur.
    let echelle = manifest.scale * scale_affichage;
    let largeur_boite = manifest.taille_de_frame(frame_apres)[0] as f32;
    let ancre = manifest.ancre_de_frame(frame_apres, pose_apres);
    let ancre_x = if facing.flipped() {
        largeur_boite - ancre[0]
    } else {
        ancre[0]
    };

    Point::new(coin.x + ancre_x * echelle, coin.y + ancre[1] * echelle)
}

/// Le facteur d'échelle du **moniteur**, arrondi à l'entier le plus proche
/// et jamais inférieur à 1.
///
/// # Pourquoi arrondir
///
/// Le sprite est agrandi par le webview avec `image-rendering: pixelated`
/// (index.html) : un agrandissement au **plus proche voisin**. À un facteur
/// fractionnaire comme 1,25, un pixel source sur quatre occupe deux pixels
/// écran et les trois autres un seul — les contours deviennent des escaliers
/// irréguliers, ce qui est exactement ce qu'on cherche à éviter sur du
/// pixel-art. À un facteur entier, chaque pixel source occupe le même carré :
/// c'est net.
///
/// Le défaut ne s'est vu qu'en changeant d'écran, d'un 27 pouces à 100 % à
/// celui d'un portable à 125 % (2026-09-20) — le même `blob`, soudain
/// crénelé.
///
/// # Ce qui n'est PAS arrondi, et pourquoi
///
/// Ni `manifest.scale`, ni l'`echelle` de `config.json`. Ce sont des choix
/// délibérés d'un auteur de pack ou de l'utilisateur (« je veux ce
/// personnage à moitié moins gros ») ; les arrondir les annulerait purement
/// et simplement. Seule la conversion imposée par le DPI de l'écran est
/// contrainte à l'entier, et elle l'est **en amont**, dans `main`, là où les
/// deux facteurs sont combinés. Les fonctions ci-dessous reçoivent le
/// produit et ne réarrondissent rien.
///
/// # Le minimum de 1
///
/// Réduire du pixel-art au plus proche voisin **jette** des pixels : un trait
/// d'un pixel de large peut disparaître entièrement. On préfère un
/// personnage un peu trop grand à un personnage troué. Et `0.0`, que
/// rendrait une sonde muette, ne doit pas le faire disparaître.
pub fn echelle_ecran_entiere(scale_moniteur: f32) -> f32 {
    let arrondi = scale_moniteur.round();
    if arrondi < 1.0 {
        1.0
    } else {
        arrondi
    }
}

/// Le facteur de la case « Petite taille » du tray (demande de l'auteur,
/// 2026-09-24) : ×1 décochée, ×0,8 cochée.
///
/// 0,8 = 1 / 1,25 : c'est la taille qu'a un personnage sur l'écran du
/// portable à 125 %, où ses 128 pixels réels paraissent plus petits que sur
/// un écran à 100 %. L'auteur la voulait en option, la taille normale restant
/// celle par défaut.
///
/// ⚠️ Réduire du pixel-art jette des pixels (voir « Le minimum de 1 »
/// ci-dessus) : c'est un choix de l'utilisateur, comme l'`echelle` de
/// `config.json`, et il n'est donc pas arrondi.
pub const FACTEUR_PETITE_TAILLE: f32 = 0.8;

/// Le facteur de taille voulu par l'utilisateur dans le tray.
pub fn facteur_de_taille(petite: bool) -> f32 {
    if petite {
        FACTEUR_PETITE_TAILLE
    } else {
        1.0
    }
}

/// La taille en pixels physiques de la fenêtre d'un personnage.
///
/// ⚠️ **Elle suit l'image AFFICHÉE, pas le pack.** `manifest.frame_size`
/// déclare la plus grande frame ; s'en servir pour toutes rendait la fenêtre
/// trop grande pour les autres, et comme `index.html` étire le sprite à
/// `100%` de la fenêtre, l'image était déformée ET l'ancre ne tombait plus
/// sur les pieds. Mesuré le 2026-09-20 : la fenêtre de `the-simba-cub`
/// descendait 79 px sous la zone de travail, donc tout le bas du personnage
/// était caché derrière la barre des tâches.
///
/// Elle ne change qu'au **changement d'image** (≈ 7 fois par seconde), alors
/// que le coin change 60 fois par seconde : c'est pourquoi les deux restent
/// séparées, et pourquoi la boucle ne rappelle `render::dimensionner` que
/// sur changement (voir l'avertissement de `render::placer`).
pub fn window_size(manifest: &Manifest, frame: u32, scale_affichage: f32) -> (u32, u32) {
    let echelle = manifest.scale * scale_affichage;
    let taille = manifest.taille_de_frame(frame);
    (
        (taille[0] as f32 * echelle).round() as u32,
        (taille[1] as f32 * echelle).round() as u32,
    )
}

/// Le rectangle écran de la hitbox de la pose courante — celle qui sert au
/// hit-testing (Tâche 11) et à la proximité entre personnages (étape 3).
///
/// La hitbox est déclarée **dans la boîte du sprite** (spec §8.4) ; il faut
/// donc la translater et la mettre à l'échelle comme le sprite, miroir
/// compris.
pub fn hitbox_ecran(
    pos: Point,
    frame: u32,
    pose_nom: &str,
    manifest: &Manifest,
    scale_affichage: f32,
    facing: Facing,
) -> Rect {
    let echelle = manifest.scale * scale_affichage;
    let coin = window_top_left(pos, frame, pose_nom, manifest, scale_affichage, facing);
    let hb = manifest.hitbox_de(pose_nom);
    // Même boîte que pour l'ancre : celle de l'image affichée.
    let largeur_boite = manifest.taille_de_frame(frame)[0] as f32;

    // Même miroir que pour l'ancre : le bord gauche de la hitbox retournée
    // est à `largeur - (x + l)` du bord gauche de la boîte.
    let hb_x = if facing.flipped() {
        largeur_boite - (hb.x + hb.w)
    } else {
        hb.x
    };

    Rect::new(
        coin.x + hb_x * echelle,
        coin.y + hb.y * echelle,
        hb.w * echelle,
        hb.h * echelle,
    )
}

// Les tests de ce module vivent dans `attach_tests.rs`
// (sortis d ici le 2026-09-14 : ils faisaient 306 des 566 lignes).
#[cfg(test)]
#[path = "attach_tests.rs"]
mod tests;
