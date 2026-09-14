//! Les tests de `reflex` — sortis du fichier source le 2026-09-14.
//!
//! **Pourquoi ce fichier existe** : `behavior/reflex.rs` faisait 1418 lignes dont 1038 de tests,
//! soit 73 %. Toute lecture du module en payait le double, pour rien la plupart
//! du temps. Les tests n'ont pas changé d'une ligne : ils ont seulement déménagé, et
//! se désindentent d'un cran puisqu'ils ne sont plus enfermés dans un `mod tests { }`.

use super::*;
use crate::character::manifest::{Manifest, POSE_FALL, POSE_GRAB_WALL, POSE_LAND, POSE_STAND};
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
            "dragged":       { "frames": [1],  "anchor": [64, 8] },
            "draggedRight1": { "frames": [5],  "anchor": [64, 8] },
            "draggedRight2": { "frames": [7],  "anchor": [64, 8] },
            "draggedRight3": { "frames": [9],  "anchor": [64, 8] },
            "draggedLeft1":  { "frames": [6],  "anchor": [64, 8] },
            "draggedLeft2":  { "frames": [8],  "anchor": [64, 8] },
            "draggedLeft3":  { "frames": [10], "anchor": [64, 8] }
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
        echelle_affichage: 1.0,
        bouton_gauche: false,
        curseur_sur_le_personnage: false,
        biais: crate::signals::Biais::neutre(),
        utilisateur_actif: true,
        commande: None,
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
        echelle_affichage: 1.0,
        bouton_gauche: true,
        curseur_sur_le_personnage: true,
        biais: crate::signals::Biais::neutre(),
        utilisateur_actif: true,
        commande: None,
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
    // Le pied déjà sur le curseur : c'est l'état « au repos ». Sans cette
    // ligne il partirait de 0, à 800 px du curseur, donc en balancement
    // maximal — ce qui teste autre chose.
    ch.portage.pied_x = 800.0;

    let e = Entrees {
        souris: Point::new(800.0, 300.0),
        echelle_affichage: 1.0,
        bouton_gauche: true,
        curseur_sur_le_personnage: true,
        biais: crate::signals::Biais::neutre(),
        utilisateur_actif: true,
        commande: None,
    };
    appliquer(&mut ch, &m, &e, Duration::ZERO, DT);
    assert_eq!(ch.pose, POSE_DRAGGED);
}

/// Porte le personnage et déplace le curseur à `vitesse` px/s pendant
/// `secondes`, en pas de 1/60 s. Rend la pose atteinte à la fin.
///
/// C'est le seul moyen honnête d'éprouver un ressort : lui donner une
/// entrée réaliste et regarder son régime, plutôt que de vérifier un
/// seuil sur une seule image.
fn glisser(ch: &mut Character, m: &World, vitesse: f32, secondes: f32) -> String {
    let mut x = 800.0f32;
    let mut t = Duration::ZERO;
    let pas = (secondes / DT) as usize;

    for _ in 0..pas {
        x += vitesse * DT;
        let e = Entrees {
            souris: Point::new(x, 300.0),
            echelle_affichage: 1.0,
            bouton_gauche: true,
            curseur_sur_le_personnage: true,
            biais: crate::signals::Biais::neutre(),
            utilisateur_actif: true,
            commande: None,
        };
        appliquer(ch, m, &e, t, DT);
        t += Duration::from_micros(16_667);
    }
    ch.pose.clone()
}

#[test]
fn le_balancier_traine_derriere_le_curseur() {
    // **LE test du sens**, celui qui avait été inversé. Un pendule traîne
    // derrière : curseur vers la GAUCHE → tête à GAUCHE.
    let m = monde();

    let mut ch = perso_pose_sur_le_sol(&m);
    ch.attachment = Attachment::Dragged;
    ch.portage.pied_x = 800.0;
    let pose = glisser(&mut ch, &m, -400.0, 0.5);
    assert!(
        POSES_DRAGGED_LEFT.contains(&pose.as_str()),
        "curseur à gauche : attendu une pose tête à gauche, obtenu {pose}"
    );

    let mut ch = perso_pose_sur_le_sol(&m);
    ch.attachment = Attachment::Dragged;
    ch.portage.pied_x = 800.0;
    let pose = glisser(&mut ch, &m, 400.0, 0.5);
    assert!(
        POSES_DRAGGED_RIGHT.contains(&pose.as_str()),
        "curseur à droite : attendu une pose tête à droite, obtenu {pose}"
    );
}

#[test]
fn plus_la_main_va_vite_plus_il_balance() {
    // L'amplitude doit suivre la vitesse. En régime permanent le retard
    // vaut `vitesse / 10`, et les seuils sont à 10, 30 et 50 px — donc
    // 150 px/s, 400 px/s et 700 px/s tombent dans trois niveaux distincts.
    let m = monde();

    let niveau = |vitesse: f32| -> usize {
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;
        ch.portage.pied_x = 800.0;
        let pose = glisser(&mut ch, &m, vitesse, 1.0);
        POSES_DRAGGED_LEFT
            .iter()
            .position(|p| *p == pose.as_str())
            .map(|i| i + 1)
            .unwrap_or(0)
    };

    let lent = niveau(-150.0);
    let moyen = niveau(-400.0);
    let vif = niveau(-700.0);

    assert!(lent < moyen, "lent {lent} devrait balancer moins que {moyen}");
    assert!(moyen < vif, "moyen {moyen} devrait balancer moins que {vif}");
}

#[test]
fn le_retour_au_repos_passe_par_les_niveaux_intermediaires() {
    // Il ne doit pas SAUTER au repos quand la main s'arrête : le ressort
    // le ramène en repassant par les poses de moindre amplitude.
    let m = monde();
    let mut ch = perso_pose_sur_le_sol(&m);
    ch.attachment = Attachment::Dragged;
    ch.portage.pied_x = 800.0;

    // On le lance fort, puis on immobilise le curseur.
    let ample = glisser(&mut ch, &m, -700.0, 1.0);
    assert!(POSES_DRAGGED_LEFT.contains(&ample.as_str()));

    // Curseur figé : on collecte les poses traversées.
    let mut vues = std::collections::BTreeSet::new();
    let x_final = 800.0 - 700.0 * 1.0;
    let mut t = Duration::from_secs(1);
    for _ in 0..120 {
        let e = Entrees {
            souris: Point::new(x_final, 300.0),
            echelle_affichage: 1.0,
            bouton_gauche: true,
            curseur_sur_le_personnage: true,
            biais: crate::signals::Biais::neutre(),
            utilisateur_actif: true,
            commande: None,
        };
        appliquer(&mut ch, &m, &e, t, DT);
        vues.insert(ch.pose.clone());
        t += Duration::from_micros(16_667);
    }

    // Il finit au repos…
    assert_eq!(ch.pose, POSE_DRAGGED, "il devrait avoir fini de balancer");
    // …et il est passé par au moins un niveau intermédiaire en chemin,
    // au lieu de sauter directement de l'amplitude maximale au repos.
    assert!(
        vues.len() >= 3,
        "retour trop brusque : seulement {:?}",
        vues
    );
}

#[test]
fn attraper_repart_du_repos_sur_le_curseur() {
    // Sans réinitialisation, il hériterait du retard d'un portage
    // précédent et balancerait violemment à l'instant de la saisie.
    let m = monde();
    let mut ch = perso_pose_sur_le_sol(&m);
    ch.portage.pied_x = -99_999.0;
    ch.portage.pied_vx = 12_345.0;

    let e = Entrees {
        souris: Point::new(640.0, 480.0),
        echelle_affichage: 1.0,
        bouton_gauche: true,
        curseur_sur_le_personnage: true,
        biais: crate::signals::Biais::neutre(),
        utilisateur_actif: true,
        commande: None,
    };
    appliquer(&mut ch, &m, &e, Duration::ZERO, DT);

    assert_eq!(ch.portage.pied_x, 640.0);
    assert_eq!(ch.portage.pied_vx, 0.0);
    assert_eq!(ch.pose, POSE_DRAGGED, "il pend droit au moment de la saisie");
    // Et le sprite n'est pas miroité pendant le portage.
    assert_eq!(ch.facing, crate::character::Facing::Left);
}

#[test]
fn le_bouton_seul_ne_suffit_pas_a_l_attraper() {
    // Sinon tout clic n'importe où sur le bureau l'arracherait.
    let m = monde();
    let mut ch = perso_pose_sur_le_sol(&m);
    let e = Entrees {
        souris: Point::new(50.0, 50.0),
        echelle_affichage: 1.0,
        bouton_gauche: true,
        curseur_sur_le_personnage: false,
        biais: crate::signals::Biais::neutre(),
        utilisateur_actif: true,
        commande: None,
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
        echelle_affichage: 1.0,
        bouton_gauche: true,
        curseur_sur_le_personnage: true,
        biais: crate::signals::Biais::neutre(),
        utilisateur_actif: true,
        commande: None,
    };
    appliquer(&mut ch, &m, &e, Duration::ZERO, DT);

    assert!(ch.intention.is_none());
}

#[test]
fn relacher_le_bouton_ne_fait_pas_sauter_le_sprite() {
    // **L'invariante, et non une coordonnée.** Les poses de portage ont
    // l'ancre sur la tête et `fall` sous les pieds : garder `pos =
    // curseur` faisait repartir le personnage 120 px plus haut que là où
    // on le tenait, ce qui se voyait très bien.
    //
    // On vérifie donc que le COIN DE LA FENÊTRE est inchangé — c'est la
    // seule vraie position d'écran, et l'assertion reste juste si l'on
    // change une ancre plus tard.
    use crate::character::attach::window_top_left;

    let m = monde();
    let mut ch = perso_pose_sur_le_sol(&m);
    ch.attachment = Attachment::Dragged;
    ch.portage = crate::character::Portage::neuf(Point::new(1200.0, 400.0));
    ch.set_pose(POSE_DRAGGED, Duration::ZERO);

    let coin_avant = window_top_left(
        Point::new(1200.0, 400.0),
        ch.manifest.pose(POSE_DRAGGED).unwrap(),
        &ch.manifest,
        1.0,
        ch.facing,
    );

    let e = Entrees {
        souris: Point::new(1200.0, 400.0),
        echelle_affichage: 1.0,
        bouton_gauche: false,
        curseur_sur_le_personnage: true,
        biais: crate::signals::Biais::neutre(),
        utilisateur_actif: true,
        commande: None,
    };
    let r = appliquer(&mut ch, &m, &e, Duration::ZERO, DT);
    assert_eq!(r, Reflexe::Chute);

    let pos = match ch.attachment {
        Attachment::Falling { pos, .. } => pos,
        autre => panic!("attendu Falling, obtenu {autre:?}"),
    };
    let coin_apres = window_top_left(
        pos,
        ch.manifest.pose(POSE_FALL).unwrap(),
        &ch.manifest,
        1.0,
        ch.facing,
    );

    assert!(
        (coin_apres.x - coin_avant.x).abs() < 0.01
            && (coin_apres.y - coin_avant.y).abs() < 0.01,
        "le sprite a sauté : coin {coin_avant:?} -> {coin_apres:?}"
    );
}

#[test]
fn relacher_apres_un_glisser_le_lance() {
    // **Le lancer.** Il ne doit pas tomber à la verticale après un
    // glisser : il part avec la vitesse de la main, comme l'action
    // `Thrown` de Shimeji-ee.
    let m = monde();
    let mut ch = perso_pose_sur_le_sol(&m);
    ch.attachment = Attachment::Dragged;
    ch.portage = crate::character::Portage::neuf(Point::new(800.0, 300.0));

    // On le glisse vers la droite pendant une demi-seconde…
    glisser(&mut ch, &m, 600.0, 0.5);

    // …puis on lâche, curseur à la dernière position atteinte.
    let x_final = 800.0 + 600.0 * 0.5;
    let e = Entrees {
        souris: Point::new(x_final, 300.0),
        echelle_affichage: 1.0,
        bouton_gauche: false,
        curseur_sur_le_personnage: true,
        biais: crate::signals::Biais::neutre(),
        utilisateur_actif: true,
        commande: None,
    };
    appliquer(&mut ch, &m, &e, Duration::from_millis(500), DT);

    match ch.attachment {
        Attachment::Falling { vel, .. } => {
            assert!(
                vel.x > 300.0,
                "il devrait partir vers la droite avec de l'élan, vx = {}",
                vel.x
            );
        }
        autre => panic!("attendu Falling, obtenu {autre:?}"),
    }
}

#[test]
fn lance_a_droite_il_tombe_la_tete_a_droite() {
    // Repris de `Fall.java` : l'orientation suit la vitesse horizontale.
    // Sans ça, il partait toujours dans l'orientation du portage (forcée
    // à `Left`), donc un jet vers la droite montrait un sprite de chute
    // tête à gauche.
    let m = monde();

    let lancer = |vitesse: f32| -> crate::character::Facing {
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;
        ch.portage = crate::character::Portage::neuf(Point::new(800.0, 300.0));
        glisser(&mut ch, &m, vitesse, 0.5);

        let x_final = 800.0 + vitesse * 0.5;
        let e = Entrees {
            souris: Point::new(x_final, 300.0),
            echelle_affichage: 1.0,
            bouton_gauche: false,
            curseur_sur_le_personnage: true,
            biais: crate::signals::Biais::neutre(),
            utilisateur_actif: true,
            commande: None,
        };
        appliquer(&mut ch, &m, &e, Duration::from_millis(500), DT);
        ch.facing
    };

    assert_eq!(lancer(600.0), crate::character::Facing::Right, "jet à droite");
    assert_eq!(lancer(-600.0), crate::character::Facing::Left, "jet à gauche");
}

#[test]
fn une_chute_verticale_ne_retourne_pas_le_sprite() {
    // La zone morte : un résidu de lissage ne doit pas retourner le
    // sprite. On le fait tomber d'une plateforme disparue en regardant à
    // droite — il doit continuer à regarder à droite.
    let m = monde();
    let mut ch = perso_pose_sur_le_sol(&m);
    ch.facing = crate::character::Facing::Right;
    ch.attachment = Attachment::Falling {
        pos: Point::new(300.0, 500.0),
        vel: Vec2::new(1.0, 0.0), // un pixel par seconde : du bruit
    };

    appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);
    assert_eq!(ch.facing, crate::character::Facing::Right);
}

#[test]
fn relacher_sans_avoir_bouge_le_laisse_tomber_droit() {
    // Le pendant du test précédent : un simple clic-relâche ne doit pas
    // le catapulter.
    let m = monde();
    let mut ch = perso_pose_sur_le_sol(&m);
    ch.attachment = Attachment::Dragged;
    ch.portage = crate::character::Portage::neuf(Point::new(800.0, 300.0));

    // Curseur immobile pendant une demi-seconde.
    glisser(&mut ch, &m, 0.0, 0.5);

    let e = Entrees {
        souris: Point::new(800.0, 300.0),
        echelle_affichage: 1.0,
        bouton_gauche: false,
        curseur_sur_le_personnage: true,
        biais: crate::signals::Biais::neutre(),
        utilisateur_actif: true,
        commande: None,
    };
    appliquer(&mut ch, &m, &e, Duration::from_millis(500), DT);

    match ch.attachment {
        Attachment::Falling { vel, .. } => {
            assert!(vel.x.abs() < 10.0, "élan parasite vx = {}", vel.x);
            assert!(vel.y.abs() < 10.0, "élan parasite vy = {}", vel.y);
        }
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
        echelle_affichage: 1.0,
        bouton_gauche: true,
        curseur_sur_le_personnage: false, // il a glissé sous le curseur
        biais: crate::signals::Biais::neutre(),
        utilisateur_actif: true,
        commande: None,
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

// ── Tâche 5, étape 4a : le lancer qui s'accroche ────────────────────
//
// Ces trois tests couvrent : le mur qui attrape un lancer, le POINT NON
// ÉVIDENT de la tâche (sans l'intention posée en phase `Accroche`, la
// règle de sécurité de la Tâche 3 le ferait tomber dès l'image
// suivante), et le contre-exemple du coin, où le sol doit gagner.

/// Un `blob` complet, posé sur le sol — chargé depuis le vrai manifeste
/// plutôt que le manifeste minimal `manifeste()` d'en haut : celui-ci
/// n'a pas de pose `grabWall`, or ces tests-ci doivent la vérifier.
fn perso(m: &World) -> Character {
    // `&…[0]` : `Platform` n'implémente pas `Copy` (voir `world.rs`), on
    // emprunte donc plutôt que de tenter de le sortir du slice — même
    // motif que le `perso` de `behavior/mod.rs`.
    let sol = &m.platforms()[0];
    Character::new(
        Manifest::load(std::path::Path::new("../characters/blob"))
            .expect("le personnage de test doit être lisible"),
        Attachment::On {
            platform: sol.id,
            face: Face::Top,
            offset: 500.0,
        },
        sol.rect.point_on(Face::Top, 500.0),
    )
}

#[test]
fn jete_contre_un_mur_il_s_y_accroche() {
    let m = World::from_screens(&FakeProbe::un_ecran().screens());
    let mut ch = perso(&m);

    // Lancé vers la gauche, à mi-hauteur.
    ch.attachment = Attachment::Falling {
        pos: Point::new(30.0, 400.0),
        vel: crate::geom::Vec2::new(-600.0, 0.0),
    };

    // Quelques images suffisent pour parcourir les 30 px.
    let mut accroche = false;
    for i in 0..20 {
        let r = appliquer(
            &mut ch,
            &m,
            &entrees_neutres(),
            Duration::from_secs_f32(i as f32 * DT),
            DT,
        );
        if r == Reflexe::Accroche {
            accroche = true;
            break;
        }
    }

    assert!(accroche, "il devrait s'accrocher au mur gauche");
    assert!(matches!(
        ch.attachment,
        Attachment::On { face: Face::Right, .. }
    ));
    assert_eq!(ch.pose, POSE_GRAB_WALL);
    assert_eq!(ch.facing, Facing::Left, "il regarde le mur");
}

#[test]
fn accroche_par_un_lancer_il_ne_lache_pas_a_l_image_suivante() {
    // LE test de la tâche. Sans l'intention posée en phase `Accroche`,
    // la couche 2 rend `Finie` et la règle de sécurité le fait tomber :
    // jeté contre un mur, il ne tiendrait qu'une image.
    let m = World::from_screens(&FakeProbe::un_ecran().screens());
    let mut ch = perso(&m);
    ch.attachment = Attachment::Falling {
        pos: Point::new(30.0, 400.0),
        vel: crate::geom::Vec2::new(-600.0, 0.0),
    };

    for i in 0..20 {
        let r = appliquer(
            &mut ch,
            &m,
            &entrees_neutres(),
            Duration::from_secs_f32(i as f32 * DT),
            DT,
        );
        if r == Reflexe::Accroche {
            break;
        }
    }

    assert!(
        ch.intention.is_some(),
        "une intention doit avoir été posée, sinon la règle de sécurité le lâche"
    );

    // Et on le vérifie réellement, en faisant tourner le comportement
    // complet plusieurs images.
    let mut rng = crate::rng::XorShift32::seeded(4);
    let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
    for i in 0..10 {
        crate::behavior::pas(
            &mut ch,
            &m,
            &entrees_neutres(),
            &crate::behavior::desire::TableEnvies::defaut(),
            &reglages,
            Duration::from_secs_f32(1.0 + i as f32 * DT),
            DT,
            &mut rng,
        );
    }

    assert!(
        matches!(ch.attachment, Attachment::On { face: Face::Right, .. }),
        "il doit tenir le mur, il est {:?}",
        ch.attachment
    );

    // ── La bonne raison, et pas seulement le bon résultat ───────────
    //
    // Bug corrigé (relecture de la Tâche 5) : `PhaseGrimpe::Accroche`
    // n'avait aucune initialisation paresseuse de `jusqu_a`, donc la
    // toute première image sautait directement au tirage
    // lâcher/redescendre — il ne tenait jamais la seconde promise. Ce
    // test passait quand même, mais pour la MAUVAISE raison : un bug
    // séparé dans `contact_mur` (le franchissement large des deux
    // côtés) faisait se raccrocher IMMÉDIATEMENT tout personnage lâché
    // pile sur le plan du mur, ce qui reproduisait accidentellement
    // `Attachment::On`. On vérifie donc maintenant l'ÉTAT INTERNE :
    // l'intention doit être `Grimper`, en phase `Accroche`, avec un
    // `jusqu_a` déjà tiré (donc non nul) — la preuve qu'il tient
    // vraiment le mur pendant sa durée d'accroche, et non qu'il
    // s'est lâché puis instantanément rattrapé.
    let tient_vraiment = matches!(
        ch.intention,
        Some(crate::behavior::intention::ActiveIntention {
            kind: crate::behavior::intention::Intention::Grimper,
            etat: crate::behavior::intention::EtatIntention::Grimpe {
                phase: crate::behavior::intention::PhaseGrimpe::Accroche,
                jusqu_a,
            },
            ..
        }) if jusqu_a != Duration::ZERO
    );
    assert!(
        tient_vraiment,
        "il devrait être en train de tenir le mur (Grimper/Accroche, \
         jusqu_a tiré), il est {:?}",
        ch.intention
    );
}

#[test]
fn jete_dans_un_coin_il_atterrit_au_lieu_de_s_accrocher() {
    let m = World::from_screens(&FakeProbe::un_ecran().screens());
    let mut ch = perso(&m);
    ch.attachment = Attachment::Falling {
        pos: Point::new(30.0, 1020.0),
        vel: crate::geom::Vec2::new(-600.0, 400.0),
    };

    for i in 0..20 {
        let r = appliquer(
            &mut ch,
            &m,
            &entrees_neutres(),
            Duration::from_secs_f32(i as f32 * DT),
            DT,
        );
        if r == Reflexe::Atterrissage || r == Reflexe::Accroche {
            break;
        }
    }

    assert!(
        matches!(ch.attachment, Attachment::On { face: Face::Top, .. }),
        "le sol gagne sur le mur"
    );
}

// ── Le plafond attrape aussi : révision du 2026-09-12 de la design §3.2
// ──────────────────────────────────────────────────────────────────────
//
// Trois tests, sur le même modèle que ceux du mur ci-dessus : l'accroche
// elle-même, le point non évident (sans l'intention, il retomberait dès
// l'image suivante), et — nouveau ici — la preuve que le contraire est
// aussi vrai : une fois l'intention terminée, il tombe VRAIMENT, mesuré
// sur plusieurs images plutôt que sur un seul appel.

#[test]
fn jete_vers_le_haut_il_s_accroche_au_plafond() {
    let m = World::from_screens(&FakeProbe::un_ecran().screens());
    let mut ch = perso(&m);

    // Lancé vers le haut ET vers la droite, pour vérifier au passage que
    // l'orientation suit la vitesse HORIZONTALE (comme au sol), et non
    // une notion de « regarder la surface » qui n'a pas de sens au
    // plafond.
    ch.attachment = Attachment::Falling {
        pos: Point::new(300.0, 30.0),
        vel: crate::geom::Vec2::new(300.0, -600.0),
    };

    let mut accroche = false;
    for i in 0..20 {
        let r = appliquer(
            &mut ch,
            &m,
            &entrees_neutres(),
            Duration::from_secs_f32(i as f32 * DT),
            DT,
        );
        if r == Reflexe::Accroche {
            accroche = true;
            break;
        }
    }

    assert!(accroche, "il devrait s'accrocher au plafond");
    assert!(matches!(
        ch.attachment,
        Attachment::On { face: Face::Bottom, .. }
    ));
    assert_eq!(ch.pose, crate::character::manifest::POSE_GRAB_CEILING);
    assert_eq!(
        ch.facing,
        Facing::Right,
        "l'orientation suit la vitesse horizontale, pas la surface"
    );
}

#[test]
fn accroche_au_plafond_par_un_lancer_il_ne_lache_pas_a_l_image_suivante() {
    // Même raison que pour le mur : sans l'intention posée par
    // `ActiveIntention::accroche`, la règle de sécurité du monde
    // vertical (`behavior::pas`) le ferait tomber dès l'image suivante.
    let m = World::from_screens(&FakeProbe::un_ecran().screens());
    let mut ch = perso(&m);
    ch.attachment = Attachment::Falling {
        pos: Point::new(300.0, 30.0),
        vel: crate::geom::Vec2::new(0.0, -600.0),
    };

    for i in 0..20 {
        let r = appliquer(
            &mut ch,
            &m,
            &entrees_neutres(),
            Duration::from_secs_f32(i as f32 * DT),
            DT,
        );
        if r == Reflexe::Accroche {
            break;
        }
    }

    assert!(
        ch.intention.is_some(),
        "une intention doit avoir été posée, sinon la règle de sécurité le lâche"
    );

    // Et on le vérifie réellement, en faisant tourner le comportement
    // complet plusieurs images — pas seulement `reflex::appliquer`.
    let mut rng = crate::rng::XorShift32::seeded(4);
    let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
    for i in 0..10 {
        crate::behavior::pas(
            &mut ch,
            &m,
            &entrees_neutres(),
            &crate::behavior::desire::TableEnvies::defaut(),
            &reglages,
            Duration::from_secs_f32(1.0 + i as f32 * DT),
            DT,
            &mut rng,
        );
    }

    assert!(
        matches!(ch.attachment, Attachment::On { face: Face::Bottom, .. }),
        "il doit tenir le plafond, il est {:?}",
        ch.attachment
    );
}

#[test]
fn accroche_au_plafond_sans_intention_il_tombe_vraiment() {
    // **Le test qui compte le plus dans ce lot.** Deux bugs de cette
    // branche ont survécu à trois tâches parce qu'un test n'appelait la
    // boucle qu'une seule fois — voir le commentaire de
    // `accroche_par_un_lancer_il_ne_lache_pas_a_l_image_suivante` plus
    // haut. On fait donc ici l'inverse de ce test : intention absente
    // dès le départ, et on fait tourner `behavior::pas` sur une demi-
    // seconde simulée (30 images à 60 Hz) pour vérifier que la vitesse
    // verticale de chute grandit réellement — la preuve qu'il ne reste
    // pas figé au plafond avec la mauvaise pose.
    let m = World::from_screens(&FakeProbe::un_ecran().screens());
    let mut ch = perso(&m);

    let plafond = m
        .platforms()
        .iter()
        .find(|p| p.has_face(Face::Bottom))
        .expect("un écran isolé a un plafond")
        .id;
    ch.attachment = Attachment::On {
        platform: plafond,
        face: Face::Bottom,
        offset: 300.0,
    };
    ch.pos_connue = Point::new(300.0, 0.0);
    // Pas d'intention `Grimper` : c'est exactement le cas que la règle
    // de sécurité du monde vertical (design §4.5) doit détecter.
    ch.intention = None;

    let mut rng = crate::rng::XorShift32::seeded(7);
    let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
    let mut t = Duration::ZERO;
    for _ in 0..30 {
        crate::behavior::pas(
            &mut ch,
            &m,
            &entrees_neutres(),
            &crate::behavior::desire::TableEnvies::defaut(),
            &reglages,
            t,
            DT,
            &mut rng,
        );
        t += Duration::from_secs_f32(DT);
    }

    match ch.attachment {
        Attachment::Falling { vel, .. } => {
            // La gravité doit l'avoir fait accélérer vers le bas de
            // manière significative sur une demi-seconde — pas juste
            // franchi zéro d'un résidu de flottant.
            assert!(
                vel.y > 100.0,
                "il devrait tomber franchement après 0,5 s, vy = {}",
                vel.y
            );
        }
        autre => panic!(
            "il devrait être en train de tomber après s'être lâché du \
             plafond, il est {autre:?}"
        ),
    }
}
