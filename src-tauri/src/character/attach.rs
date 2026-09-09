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

use super::manifest::{Manifest, Pose};
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

/// Le coin supérieur gauche où placer la **fenêtre** de 128×128, pour que
/// l'ancre de la pose tombe exactement sur `pos`.
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
pub fn window_top_left(
    pos: Point,
    pose: &Pose,
    manifest: &Manifest,
    scale_affichage: f32,
    facing: Facing,
) -> Point {
    // L'échelle totale : celle du manifeste combinée à celle du moniteur
    // (spec §3.4, §8.5). C'est le SEUL usage légitime du facteur d'échelle —
    // il ne convertit jamais une coordonnée.
    let echelle = manifest.scale * scale_affichage;

    let largeur_boite = manifest.frame_size[0] as f32;

    // Quand le sprite est retourné, l'ancre l'est aussi : une ancre à 100 px
    // du bord gauche se retrouve à `largeur - 100` du bord gauche.
    //
    // Sans cette symétrie, un personnage dont l'ancre est décentrée sauterait
    // latéralement à chaque demi-tour. C'est du miroir, pas de la
    // compensation : on retourne l'ancre avec l'image, on ne corrige pas
    // après coup.
    let ancre_x = if facing.flipped() {
        largeur_boite - pose.anchor[0]
    } else {
        pose.anchor[0]
    };

    // Le miroir est horizontal : `y` n'est pas concerné.
    let ancre_y = pose.anchor[1];

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
    pose_avant: &Pose,
    pose_apres: &Pose,
    manifest: &Manifest,
    scale_affichage: f32,
    facing: Facing,
) -> Point {
    let coin = window_top_left(pos, pose_avant, manifest, scale_affichage, facing);

    // L'inverse de `window_top_left`, avec la pose d'arrivée.
    let echelle = manifest.scale * scale_affichage;
    let largeur_boite = manifest.frame_size[0] as f32;
    let ancre_x = if facing.flipped() {
        largeur_boite - pose_apres.anchor[0]
    } else {
        pose_apres.anchor[0]
    };

    Point::new(
        coin.x + ancre_x * echelle,
        coin.y + pose_apres.anchor[1] * echelle,
    )
}

/// La taille en pixels physiques de la fenêtre d'un personnage.
///
/// Séparée de `window_top_left` parce qu'elle ne change qu'au chargement du
/// manifeste ou au changement d'écran, alors que le coin change 60 fois par
/// seconde.
pub fn window_size(manifest: &Manifest, scale_affichage: f32) -> (u32, u32) {
    let echelle = manifest.scale * scale_affichage;
    (
        (manifest.frame_size[0] as f32 * echelle).round() as u32,
        (manifest.frame_size[1] as f32 * echelle).round() as u32,
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
    pose_nom: &str,
    pose: &Pose,
    manifest: &Manifest,
    scale_affichage: f32,
    facing: Facing,
) -> Rect {
    let echelle = manifest.scale * scale_affichage;
    let coin = window_top_left(pos, pose, manifest, scale_affichage, facing);
    let hb = manifest.hitbox_de(pose_nom);
    let largeur_boite = manifest.frame_size[0] as f32;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::manifest::POSE_STAND;
    use crate::geom::Rect;
    use crate::probe::fake::FakeProbe;
    use crate::probe::{ScreenInfo, SystemProbe};

    fn monde_un_ecran() -> World {
        World::from_screens(&FakeProbe::un_ecran().screens())
    }

    fn id_du_sol(monde: &World) -> PlatformId {
        monde.platforms()[0].id
    }

    /// La souris, quand le test ne s'y intéresse pas.
    const SOURIS_AILLEURS: Point = Point { x: 0.0, y: 0.0 };

    /// Un manifeste construit en mémoire, sans toucher au disque : ces tests
    /// portent sur la géométrie, pas sur le chargement (déjà couvert en
    /// Tâche 4).
    fn manifeste_de_test() -> Manifest {
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128, 128], "scale": 1,
            "hitbox": [40, 20, 48, 100],
            "poses": {
                "stand":    { "frames": [1], "anchor": [64, 120] },
                "decentre": { "frames": [1], "anchor": [100, 120] }
            }
        }"#;
        serde_json::from_str(json).expect("manifeste de test valide")
    }

    #[test]
    fn la_position_est_derivee_de_la_plateforme() {
        let monde = monde_un_ecran();
        let att = Attachment::On {
            platform: id_du_sol(&monde),
            face: Face::Top,
            offset: 300.0,
        };

        let p = world_position(&att, &monde, SOURIS_AILLEURS).expect("plateforme présente");
        // Le sol du FakeProbe::un_ecran est à y = 1032, x de 0 à 1920.
        assert_eq!(p, Point::new(300.0, 1032.0));
    }

    #[test]
    fn deplacer_la_plateforme_deplace_le_personnage_sans_code() {
        // LE test de la décision n° 1. On ne touche pas au personnage : on
        // change la géométrie de sa plateforme, sous la même identité, et sa
        // position doit avoir suivi.
        let avant = World::from_screens(&[ScreenInfo {
            id: 42,
            work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
            scale: 1.0,
        }]);
        let apres = World::from_screens(&[ScreenInfo {
            id: 42,
            // L'écran a « bougé » de 500 px vers la droite et 100 vers le bas.
            work_area: Rect::new(500.0, 100.0, 1920.0, 1032.0),
            scale: 1.0,
        }]);

        let att = Attachment::On {
            platform: PlatformId(42),
            face: Face::Top,
            offset: 300.0,
        };

        let p_avant = world_position(&att, &avant, SOURIS_AILLEURS).unwrap();
        let p_apres = world_position(&att, &apres, SOURIS_AILLEURS).unwrap();

        assert_eq!(p_avant, Point::new(300.0, 1032.0));
        assert_eq!(p_apres, Point::new(800.0, 1132.0));
    }

    #[test]
    fn redimensionner_la_plateforme_conserve_la_distance_au_bord() {
        // Second bénéfice gratuit de la décision n° 1 : l'offset est une
        // distance au bord, donc il ne bouge pas quand la face s'allonge.
        let etroit = World::from_screens(&[ScreenInfo {
            id: 42,
            work_area: Rect::new(0.0, 0.0, 800.0, 1032.0),
            scale: 1.0,
        }]);
        let large = World::from_screens(&[ScreenInfo {
            id: 42,
            work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
            scale: 1.0,
        }]);

        let att = Attachment::On {
            platform: PlatformId(42),
            face: Face::Top,
            offset: 100.0,
        };

        assert_eq!(
            world_position(&att, &etroit, SOURIS_AILLEURS).unwrap().x,
            world_position(&att, &large, SOURIS_AILLEURS).unwrap().x
        );
    }

    #[test]
    fn une_plateforme_absente_ne_donne_aucune_position() {
        // C'est le test unique qui remplace tout le traitement du cas
        // « la fenêtre s'est fermée » (spec §6.2).
        let monde = monde_un_ecran();
        let att = Attachment::On {
            platform: PlatformId(999_999),
            face: Face::Top,
            offset: 300.0,
        };
        assert_eq!(world_position(&att, &monde, SOURIS_AILLEURS), None);
    }

    #[test]
    fn une_plateforme_absente_est_hors_bornes() {
        let monde = monde_un_ecran();
        let att = Attachment::On {
            platform: PlatformId(999_999),
            face: Face::Top,
            offset: 300.0,
        };
        assert!(hors_bornes(&att, &monde));
    }

    #[test]
    fn un_offset_au_dela_de_la_face_est_hors_bornes() {
        // Le cas « la plateforme a rétréci sous lui ». Un seul test, comme
        // la spec §6.2 le promet.
        let monde = monde_un_ecran();
        let sol = id_du_sol(&monde);

        let dedans = Attachment::On {
            platform: sol,
            face: Face::Top,
            offset: 1900.0,
        };
        let dehors = Attachment::On {
            platform: sol,
            face: Face::Top,
            offset: 1930.0,
        };
        let negatif = Attachment::On {
            platform: sol,
            face: Face::Top,
            offset: -5.0,
        };

        assert!(!hors_bornes(&dedans, &monde));
        assert!(hors_bornes(&dehors, &monde));
        assert!(hors_bornes(&negatif, &monde));
    }

    #[test]
    fn falling_est_le_seul_etat_a_position_absolue() {
        let monde = monde_un_ecran();
        let att = Attachment::Falling {
            pos: Point::new(700.0, 200.0),
            vel: Vec2::new(0.0, 300.0),
        };
        // La position ne dépend d'aucune plateforme : c'est cohérent, en
        // chute il n'est attaché à rien (spec §6.2).
        assert_eq!(
            world_position(&att, &monde, SOURIS_AILLEURS),
            Some(Point::new(700.0, 200.0))
        );
        assert!(!hors_bornes(&att, &monde));
    }

    #[test]
    fn dragged_suit_la_souris_et_ne_stocke_rien() {
        // `Dragged` n'a AUCUNE donnée : sa position se dérive de la souris,
        // comme `On` se dérive de sa plateforme. Même principe.
        let monde = monde_un_ecran();
        let att = Attachment::Dragged;
        assert_eq!(
            world_position(&att, &monde, Point::new(640.0, 480.0)),
            Some(Point::new(640.0, 480.0))
        );
        assert!(!hors_bornes(&att, &monde));
    }

    #[test]
    fn window_top_left_pose_l_ancre_sur_la_position() {
        // L'ancre `stand` est [64, 120] : la fenêtre de 128×128 doit donc
        // être placée 64 px à gauche et 120 px au-dessus de la position.
        let m = manifeste_de_test();
        let pose = m.pose(POSE_STAND).unwrap();

        let coin = window_top_left(Point::new(300.0, 1032.0), pose, &m, 1.0, Facing::Right);
        assert_eq!(coin, Point::new(300.0 - 64.0, 1032.0 - 120.0));
    }

    #[test]
    fn window_top_left_reflete_l_ancre_quand_le_sprite_est_retourne() {
        // Une ancre décentrée doit être MIROITÉE avec le sprite, sinon le
        // personnage se décale d'un coup en faisant demi-tour.
        //
        // C'est le remplaçant principiel du bricolage du prototype VSCode,
        // qui compensait le décalage à la main, asymétriquement selon le
        // sens de marche (spec §8.3).
        let m = manifeste_de_test();
        let pose = m.pose("decentre").unwrap(); // ancre [100, 120]

        let a_droite = window_top_left(Point::new(500.0, 1032.0), pose, &m, 1.0, Facing::Right);
        let a_gauche = window_top_left(Point::new(500.0, 1032.0), pose, &m, 1.0, Facing::Left);

        // Les sprites étant dessinés vers la GAUCHE (voir `Facing::flipped`),
        // c'est le sens non miroité : l'ancre est à 100 du bord gauche.
        assert_eq!(a_gauche.x, 500.0 - 100.0);
        // Vers la droite, le sprite est miroité : l'ancre se retrouve à
        // 128 - 100 = 28 du bord gauche.
        assert_eq!(a_droite.x, 500.0 - 28.0);
        // La hauteur, elle, ne bouge pas : le miroir est horizontal.
        assert_eq!(a_droite.y, a_gauche.y);
    }

    #[test]
    fn window_top_left_applique_l_echelle_a_l_ancre() {
        // Spec §3.4 : l'échelle ne sert QU'au dimensionnement du sprite. La
        // position, elle, reste en pixels physiques — mais l'ancre est
        // exprimée dans la boîte du sprite, donc elle se met à l'échelle
        // avec lui.
        let m = manifeste_de_test();
        let pose = m.pose(POSE_STAND).unwrap();

        let coin = window_top_left(Point::new(300.0, 1032.0), pose, &m, 2.0, Facing::Right);
        assert_eq!(coin, Point::new(300.0 - 128.0, 1032.0 - 240.0));
    }

    #[test]
    fn window_size_combine_les_deux_echelles() {
        let m = manifeste_de_test(); // scale = 1, frameSize = [128, 128]
        assert_eq!(window_size(&m, 1.0), (128, 128));
        assert_eq!(window_size(&m, 1.5), (192, 192));
    }

    #[test]
    fn hitbox_ecran_se_place_dans_la_fenetre() {
        // hitbox du manifeste : [40, 20, 48, 100], ancre stand : [64, 120].
        // Le coin de la fenêtre est donc à (300-64, 1032-120) = (236, 912),
        // et la hitbox à 40/20 de là.
        let m = manifeste_de_test();
        let pose = m.pose(POSE_STAND).unwrap();

        let r = hitbox_ecran(
            Point::new(300.0, 1032.0),
            POSE_STAND,
            pose,
            &m,
            1.0,
            Facing::Right,
        );
        assert_eq!(r, Rect::new(236.0 + 40.0, 912.0 + 20.0, 48.0, 100.0));
    }

    #[test]
    fn hitbox_ecran_est_miroitee_avec_le_sprite() {
        // On prend une hitbox franchement décentrée pour que le test prouve
        // quelque chose : [10, 20, 30, 100] va de 10 à 40 dans la boîte,
        // donc retournée elle va de 128-40 = 88 à 128-10 = 118.
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128, 128], "scale": 1,
            "hitbox": [10, 20, 30, 100],
            "poses": { "stand": { "frames": [1] } }
        }"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let pose = m.pose(POSE_STAND).unwrap();

        let droite = hitbox_ecran(
            Point::new(500.0, 1000.0),
            POSE_STAND,
            pose,
            &m,
            1.0,
            Facing::Right,
        );
        let gauche = hitbox_ecran(
            Point::new(500.0, 1000.0),
            POSE_STAND,
            pose,
            &m,
            1.0,
            Facing::Left,
        );

        // Sens non miroité (gauche) : la hitbox commence à 10 dans la boîte.
        // Miroitée (droite) : elle commence à 128 - (10 + 30) = 88.
        assert_eq!(droite.w, gauche.w);
        assert_ne!(droite.x, gauche.x);
        assert_eq!(droite.x - gauche.x, 88.0 - 10.0);
    }
}
