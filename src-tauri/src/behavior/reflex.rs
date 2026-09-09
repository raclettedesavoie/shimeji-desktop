//! Couche 1 du comportement : les réflexes (spec §7.1).
//!
//! **Ce ne sont pas des décisions, ce sont des conséquences physiques.** Ils
//! ne consultent ni l'envie ni l'intention — ils peuvent en revanche
//! l'annuler. Quand l'un d'eux s'impose, les couches 2 et 3 ne tournent pas
//! du tout dans cette image.
//!
//! Les quatre réflexes de la spec :
//!   · plateforme disparue   → je tombe          ✅ ici
//!   · attrapé à la souris   → je suis porté     ✅ ici
//!   · contact avec le sol   → j'atterris        ✅ ici
//!   · session verrouillée   → je disparais      ⬜ étape 2 (signal)
//!
//! Le garde-fou « tombé sous le bureau » s'y ajoute (spec §6.3). Il n'est pas
//! dans la liste de la spec parce que ce n'est pas un réflexe du personnage,
//! c'est un filet de sécurité du programme — mais sa place est ici, au même
//! rang de priorité.

use super::Entrees;
use crate::character::attach::{hors_bornes, world_position, Attachment};
use crate::character::manifest::{
    POSE_DRAGGED, POSE_DRAGGED_LEFT, POSE_DRAGGED_RIGHT, POSE_FALL, POSE_LAND, POSE_STAND,
};
use crate::character::physics::{atterrissage, integrer_chute, sous_le_bureau, VITESSE_BALANCIER};
use crate::character::Character;
use crate::geom::{Face, Vec2};
use crate::world::World;
use std::time::Duration;

/// Ce qui s'est imposé à cette image. `Aucun` laisse la main aux couches 2
/// et 3.
///
/// Rendu à l'appelant plutôt que gardé pour soi : le mode simulation
/// (Tâche 9) en fait sa trace, et c'est ce qui permet de vérifier « il n'a
/// jamais été bloqué » sans regarder l'écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reflexe {
    Chute,
    Porte,
    Atterrissage,
    /// Rattrapé par le garde-fou après être tombé sous le bureau.
    Rattrape,
    Aucun,
}

/// La pose d'un personnage porté, selon la vitesse horizontale de la souris.
///
/// C'est le **balancier** : immobile il pend droit, tiré d'un côté il penche
/// de ce côté. Shimeji-ee joue à la place un cycle unique
/// (`Pinched` = 9,7,5,1,6,8,10) qui bat indépendamment de la souris ; le
/// découper en trois poses donne l'impression de tenir une peluche plutôt
/// que de regarder une animation.
///
/// > Le sens retenu est le **direct** : on tire à gauche → il penche à
/// > gauche. Un pendule réel traînerait *derrière* et pencherait à droite.
/// > Les deux se défendent ; ce choix-ci se lit mieux à l'écran. Pour
/// > l'inverser, échanger les deux constantes ci-dessous — une ligne.
fn pose_portee(souris_vx: f32) -> &'static str {
    if souris_vx < -VITESSE_BALANCIER {
        POSE_DRAGGED_LEFT
    } else if souris_vx > VITESSE_BALANCIER {
        POSE_DRAGGED_RIGHT
    } else {
        POSE_DRAGGED
    }
}

/// Applique les réflexes, dans l'ordre de priorité.
///
/// L'ordre compte, et il n'est pas arbitraire : « porté » passe avant
/// « plateforme disparue » parce qu'un personnage tenu à la main n'a pas de
/// plateforme à perdre. L'inverse le ferait tomber de la main de
/// l'utilisateur si la fenêtre sous lui se fermait.
pub fn appliquer(
    ch: &mut Character,
    world: &World,
    e: &Entrees,
    maintenant: Duration,
    dt: f32,
) -> Reflexe {
    // ── Tenir `pos_connue` à jour ───────────────────────────────────────
    // Avant tout le reste, et à chaque image : c'est de là que partira une
    // chute si la plateforme disparaît. Voir le commentaire du champ — c'est
    // un cache, pas une source de vérité.
    if let Some(p) = world_position(&ch.attachment, world, e.souris) {
        ch.pos_connue = p;
    }

    // ── Réflexe 2 : porté ───────────────────────────────────────────────
    // Traité en premier, voir la note sur l'ordre.
    match ch.attachment {
        Attachment::Dragged => {
            if e.bouton_gauche {
                // Toujours tenu. On ne teste PAS `curseur_sur_le_personnage`
                // ici : pendant un déplacement rapide, le sprite traîne
                // derrière le curseur et sortirait de sa propre hitbox — il
                // se décrocherait tout seul.
                ch.set_pose(pose_portee(e.souris_vx), maintenant);
                return Reflexe::Porte;
            }

            // Relâché : il tombe, depuis le curseur et sans élan vertical.
            ch.attachment = Attachment::Falling {
                pos: e.souris,
                vel: Vec2::zero(),
            };
            ch.set_pose(POSE_FALL, maintenant);
            ch.intention = None;
            return Reflexe::Chute;
        }

        _ => {
            // Pas encore tenu : le devient-il ?
            if e.bouton_gauche && e.curseur_sur_le_personnage {
                ch.attachment = Attachment::Dragged;
                ch.set_pose(pose_portee(e.souris_vx), maintenant);
                // Être soulevé annule ce qu'il était en train de faire :
                // reprendre une promenade après avoir été déplacé de deux
                // écrans n'aurait aucun sens.
                ch.intention = None;
                return Reflexe::Porte;
            }
        }
    }

    // ── Réflexe 1 : plateforme disparue ou trop courte ──────────────────
    if hors_bornes(&ch.attachment, world) {
        ch.attachment = Attachment::Falling {
            // Le seul usage de `pos_connue` : `world_position` rendrait
            // `None`, il n'y a plus rien dont dériver la position.
            pos: ch.pos_connue,
            vel: Vec2::zero(),
        };
        ch.set_pose(POSE_FALL, maintenant);
        ch.intention = None;
        return Reflexe::Chute;
    }

    // ── Réflexe 3 : la chute, et son issue ──────────────────────────────
    if let Attachment::Falling { pos, vel } = ch.attachment {
        // Le garde-fou d'abord : inutile de chercher un atterrissage à
        // 50 000 px sous le bureau.
        if sous_le_bureau(world, pos) {
            // `if let Some(...)` et non `unwrap` : un monde vide ne doit pas
            // faire paniquer. Dans ce cas on le laisse tomber — il n'y a
            // nulle part où le poser, et le monde reviendra.
            if let Some((platform, offset)) = world.nearest_floor(pos) {
                ch.attachment = Attachment::On {
                    platform,
                    face: Face::Top,
                    offset,
                };
                ch.set_pose(POSE_LAND, maintenant);
                ch.intention = None;
                return Reflexe::Rattrape;
            }
        }

        let (nouvelle_pos, nouvelle_vel) = integrer_chute(pos, vel, dt);

        // A-t-on traversé une face pendant ce pas ?
        if let Some((platform, offset)) = atterrissage(world, pos, nouvelle_pos) {
            ch.attachment = Attachment::On {
                platform,
                face: Face::Top,
                offset,
            };
            ch.set_pose(POSE_LAND, maintenant);
            ch.intention = None;
            return Reflexe::Atterrissage;
        }

        ch.attachment = Attachment::Falling {
            pos: nouvelle_pos,
            vel: nouvelle_vel,
        };
        ch.set_pose(POSE_FALL, maintenant);
        return Reflexe::Chute;
    }

    // ── Fin de la pose d'atterrissage ───────────────────────────────────
    // `land` n'est pas une intention : c'est la queue d'un réflexe. On la
    // laisse se jouer, puis on rend la main.
    if ch.pose == POSE_LAND {
        if ch.pose_terminee(maintenant) {
            ch.set_pose(POSE_STAND, maintenant);
        } else {
            // Toujours en train d'atterrir : les couches 2 et 3 attendent.
            return Reflexe::Atterrissage;
        }
    }

    Reflexe::Aucun
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::manifest::{Manifest, POSE_FALL, POSE_LAND, POSE_STAND};
    use crate::geom::Point;
    use crate::probe::fake::FakeProbe;
    use crate::probe::SystemProbe;
    use crate::world::PlatformId;

    const DT: f32 = 1.0 / 60.0;

    fn manifeste() -> Manifest {
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40, 20, 48, 100],
            "poses": {
                "stand":   { "frames": [1] },
                "walk":    { "frames": [2, 3], "frameMs": 120, "loop": true },
                "run":     { "frames": [4, 5], "frameMs": 80,  "loop": true },
                "sit":     { "frames": [6] },
                "fall":    { "frames": [7], "anchor": [64, 64] },
                "land":    { "frames": [8], "frameMs": 150 },
                "dragged":      { "frames": [1] },
                "draggedLeft":  { "frames": [5, 7, 9], "frameMs": 100, "loop": true },
                "draggedRight": { "frames": [6, 8, 10], "frameMs": 100, "loop": true }
            }
        }"#;
        serde_json::from_str(json).unwrap()
    }

    fn monde() -> World {
        World::from_screens(&FakeProbe::deux_ecrans().screens())
    }

    fn sol(monde: &World) -> PlatformId {
        monde.platforms()[0].id
    }

    fn perso_pose_sur_le_sol(monde: &World) -> Character {
        Character::new(
            manifeste(),
            Attachment::On {
                platform: sol(monde),
                face: Face::Top,
                offset: 300.0,
            },
            Point::new(300.0, 1032.0),
        )
    }

    fn entrees_neutres() -> Entrees {
        Entrees {
            souris: Point::new(0.0, 0.0),
            souris_vx: 0.0,
            bouton_gauche: false,
            curseur_sur_le_personnage: false,
        }
    }

    #[test]
    fn pose_sur_le_sol_aucun_reflexe_ne_s_impose() {
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);
        assert_eq!(r, Reflexe::Aucun);
    }

    #[test]
    fn plateforme_disparue_il_tombe() {
        // Le réflexe n° 1. On lui donne une plateforme qui n'existe pas —
        // c'est l'équivalent exact d'une fenêtre qu'on ferme (étape 4).
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::On {
            platform: PlatformId(999_999),
            face: Face::Top,
            offset: 300.0,
        };

        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Chute);
        assert!(matches!(ch.attachment, Attachment::Falling { .. }));
        assert_eq!(ch.pose, POSE_FALL);
    }

    #[test]
    fn la_chute_amorcee_part_de_la_derniere_position_connue() {
        // C'est le seul usage légitime de `pos_connue` : sans lui, on ne
        // saurait pas d'où commencer à tomber.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.pos_connue = Point::new(742.0, 1032.0);
        ch.attachment = Attachment::On {
            platform: PlatformId(999_999),
            face: Face::Top,
            offset: 300.0,
        };

        appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        match ch.attachment {
            Attachment::Falling { pos, vel } => {
                assert_eq!(pos.x, 742.0);
                // Elle commence sans vitesse verticale : il ne saute pas, le
                // sol se dérobe.
                assert_eq!(vel.y, 0.0);
            }
            autre => panic!("attendu Falling, obtenu {autre:?}"),
        }
    }

    #[test]
    fn offset_hors_bornes_il_tombe() {
        // La plateforme a rétréci sous lui. Même réflexe, même code.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::On {
            platform: sol(&m),
            face: Face::Top,
            offset: 99_999.0,
        };

        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);
        assert_eq!(r, Reflexe::Chute);
    }

    #[test]
    fn curseur_dans_la_hitbox_et_bouton_enfonce_il_est_attrape() {
        // Le réflexe n° 2.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);

        let e = Entrees {
            souris: Point::new(300.0, 1000.0),
            souris_vx: 0.0,
            bouton_gauche: true,
            curseur_sur_le_personnage: true,
        };

        let r = appliquer(&mut ch, &m, &e, Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Porte);
        assert_eq!(ch.attachment, Attachment::Dragged);
        assert_eq!(ch.pose, crate::character::manifest::POSE_DRAGGED);
    }

    #[test]
    fn porte_immobile_il_pend_droit() {
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;

        let e = Entrees {
            souris: Point::new(800.0, 300.0),
            souris_vx: 0.0,
            bouton_gauche: true,
            curseur_sur_le_personnage: true,
        };
        appliquer(&mut ch, &m, &e, Duration::ZERO, DT);
        assert_eq!(ch.pose, POSE_DRAGGED);
    }

    #[test]
    fn porte_en_mouvement_il_balance_du_bon_cote() {
        // Le balancier que Shimeji-ee n'a pas : son `Pinched` bat
        // indépendamment de la souris.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;

        let mut entrees = |vx: f32| Entrees {
            souris: Point::new(800.0, 300.0),
            souris_vx: vx,
            bouton_gauche: true,
            curseur_sur_le_personnage: true,
        };

        appliquer(&mut ch, &m, &entrees(-400.0), Duration::ZERO, DT);
        assert_eq!(ch.pose, POSE_DRAGGED_LEFT, "tiré à gauche");

        appliquer(&mut ch, &m, &entrees(400.0), Duration::ZERO, DT);
        assert_eq!(ch.pose, POSE_DRAGGED_RIGHT, "tiré à droite");

        // Sous le seuil : il repend droit. Un tremblement de main ne doit pas
        // le faire battre.
        appliquer(&mut ch, &m, &entrees(30.0), Duration::ZERO, DT);
        assert_eq!(ch.pose, POSE_DRAGGED, "vitesse sous le seuil");
    }

    #[test]
    fn le_bouton_seul_ne_suffit_pas_a_l_attraper() {
        // Sinon tout clic n'importe où sur le bureau l'arracherait.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        let e = Entrees {
            souris: Point::new(50.0, 50.0),
            souris_vx: 0.0,
            bouton_gauche: true,
            curseur_sur_le_personnage: false,
        };
        assert_eq!(
            appliquer(&mut ch, &m, &e, Duration::ZERO, DT),
            Reflexe::Aucun
        );
    }

    #[test]
    fn etre_attrape_annule_l_intention_en_cours() {
        // Les réflexes ne consultent pas l'intention, mais ils l'INVALIDENT :
        // reprendre une promenade après avoir été soulevé n'aurait aucun sens
        // (spec §7.1).
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.intention = Some(crate::behavior::intention::ActiveIntention::nouvelle(
            crate::behavior::intention::Intention::Flaner,
            Duration::ZERO,
        ));

        let e = Entrees {
            souris: Point::new(300.0, 1000.0),
            souris_vx: 0.0,
            bouton_gauche: true,
            curseur_sur_le_personnage: true,
        };
        appliquer(&mut ch, &m, &e, Duration::ZERO, DT);

        assert!(ch.intention.is_none());
    }

    #[test]
    fn relacher_le_bouton_le_fait_tomber_depuis_le_curseur() {
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;

        let e = Entrees {
            souris: Point::new(1200.0, 400.0),
            souris_vx: 0.0,
            bouton_gauche: false,
            curseur_sur_le_personnage: true,
        };
        let r = appliquer(&mut ch, &m, &e, Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Chute);
        match ch.attachment {
            Attachment::Falling { pos, .. } => assert_eq!(pos, Point::new(1200.0, 400.0)),
            autre => panic!("attendu Falling, obtenu {autre:?}"),
        }
    }

    #[test]
    fn porte_il_reste_porte_tant_que_le_bouton_est_enfonce() {
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;

        let e = Entrees {
            souris: Point::new(800.0, 300.0),
            souris_vx: 0.0,
            bouton_gauche: true,
            curseur_sur_le_personnage: false, // il a glissé sous le curseur
        };
        let r = appliquer(&mut ch, &m, &e, Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Porte);
        assert_eq!(ch.attachment, Attachment::Dragged);
    }

    #[test]
    fn en_chute_il_avance_puis_atterrit() {
        // Le réflexe n° 3. On le lâche juste au-dessus du sol et on itère
        // jusqu'à l'atterrissage — au plus 2 secondes de temps simulé.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Falling {
            pos: Point::new(300.0, 900.0),
            vel: Vec2::zero(),
        };

        let mut t = Duration::ZERO;
        let mut atterri = false;
        for _ in 0..120 {
            let r = appliquer(&mut ch, &m, &entrees_neutres(), t, DT);
            if r == Reflexe::Atterrissage {
                atterri = true;
                break;
            }
            assert_eq!(r, Reflexe::Chute, "il devait encore tomber");
            t += Duration::from_micros(16_667);
        }

        assert!(atterri, "il n'a jamais atterri");
        assert_eq!(ch.pose, POSE_LAND);
        match ch.attachment {
            Attachment::On { platform, face, .. } => {
                assert_eq!(platform, sol(&m));
                assert_eq!(face, Face::Top);
            }
            autre => panic!("attendu On, obtenu {autre:?}"),
        }
    }

    #[test]
    fn tomber_sous_le_bureau_le_replace_sur_le_sol_le_plus_proche() {
        // Le garde-fou de la spec §6.3. Lâché très à droite, dans le vide
        // au-delà du second écran.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Falling {
            pos: Point::new(9_000.0, 50_000.0),
            vel: Vec2::new(0.0, 100.0),
        };

        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Rattrape);
        match ch.attachment {
            Attachment::On {
                platform, offset, ..
            } => {
                // Le sol du second écran, et un offset tenable.
                assert_eq!(m.get(platform).unwrap().rect.left(), 1920.0);
                assert!(offset >= 0.0 && offset <= 1920.0);
            }
            autre => panic!("attendu On, obtenu {autre:?}"),
        }
    }

    #[test]
    fn la_pose_land_finie_il_repasse_a_stand() {
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        // `land` dure 150 ms dans le manifeste de test.
        ch.set_pose(POSE_LAND, Duration::ZERO);

        // Juste avant la fin : il tient sa pose.
        appliquer(&mut ch, &m, &entrees_neutres(), Duration::from_millis(100), DT);
        assert_eq!(ch.pose, POSE_LAND);

        appliquer(&mut ch, &m, &entrees_neutres(), Duration::from_millis(200), DT);
        assert_eq!(ch.pose, POSE_STAND);
    }

    #[test]
    fn pos_connue_est_tenue_a_jour_a_chaque_image() {
        // Sans quoi une chute amorcée partirait d'une position périmée.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.pos_connue = Point::new(0.0, 0.0);

        appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        assert_eq!(ch.pos_connue, Point::new(300.0, 1032.0));
    }

    #[test]
    fn un_personnage_sans_pose_fall_tombe_quand_meme() {
        // Couverture partielle (spec §8.6) : l'absence d'une pose ne doit
        // jamais empêcher un RÉFLEXE. Les réflexes sont non négociables ;
        // c'est le tirage des ENVIES qui se restreint (Tâche 8).
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "stand": { "frames": [1] } }
        }"#;
        let m = monde();
        let mut ch = Character::new(
            serde_json::from_str(json).unwrap(),
            Attachment::On {
                platform: PlatformId(999_999),
                face: Face::Top,
                offset: 10.0,
            },
            Point::new(300.0, 500.0),
        );

        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Chute);
        assert!(matches!(ch.attachment, Attachment::Falling { .. }));
        // La pose demandée n'existe pas : il garde celle qu'il avait plutôt
        // que d'afficher du vide.
        assert_eq!(ch.pose, POSE_STAND);
    }
}
