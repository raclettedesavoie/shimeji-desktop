//! Les tests de `attach` — sortis du fichier source le 2026-09-14.
//!
//! **Pourquoi ce fichier existe** : `character/attach.rs` faisait 566 lignes dont 306 de tests,
//! soit 54 %. Toute lecture du module en payait le double, pour rien la plupart
//! du temps. Les tests n'ont pas changé d'une ligne : ils ont seulement déménagé, et
//! se désindentent d'un cran puisqu'ils ne sont plus enfermés dans un `mod tests { }`.

use super::*;
use crate::character::manifest::POSE_STAND;
use crate::geom::Rect;
use crate::probe::fake::FakeProbe;
use crate::probe::{ScreenInfo, SystemProbe};
use crate::world::RoleEcran;

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

/// Un pack dont les frames n'ont **pas toutes la même taille** — le cas qui
/// a révélé le défaut le 2026-09-20.
///
/// Calqué sur `the-simba-cub-05bf62` : `frameSize` déclare la plus GRANDE
/// frame (213×191), mais la frame 1 ne fait que 128×128. Dimensionner la
/// fenêtre sur `frameSize` la rendait 63 px trop haute, et comme l'ancre est
/// exprimée dans la boîte de la frame, le personnage s'enfonçait d'autant
/// sous le sol — sous la barre des tâches, donc à moitié invisible.
fn manifeste_heterogene() -> Manifest {
    let json = r#"{
        "id": "h", "name": "H", "frameSize": [213, 191], "scale": 1,
        "hitbox": [10, 20, 30, 100],
        "frames": {
            "1":  { "size": [128, 128] },
            "2":  { "size": [213, 191], "anchor": [106, 191] },
            "12": { "size": [128, 191] }
        },
        "poses": {
            "stand": { "frames": [1], "anchor": [64, 128] },
            "geste": { "frames": [2], "anchor": [64, 128] },
            "haute": { "frames": [12], "anchor": [64, 191] }
        }
    }"#;
    serde_json::from_str(json).expect("manifeste hétérogène valide")
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
        bounds: Rect::new(0.0, 0.0, 1920.0, 1032.0),
        scale: 1.0,
    }]);
    let apres = World::from_screens(&[ScreenInfo {
        id: 42,
        // L'écran a « bougé » de 500 px vers la droite et 100 vers le bas.
        work_area: Rect::new(500.0, 100.0, 1920.0, 1032.0),
        bounds: Rect::new(500.0, 100.0, 1920.0, 1032.0),
        scale: 1.0,
    }]);

    let att = Attachment::On {
        // `PlatformId::ecran` : depuis la Tâche 1 de l'étape 4a, l'identité
        // d'une plateforme n'est plus l'id brut du moniteur mais un
        // encodage qui y ajoute le rôle (sol/mur/plafond) sur deux bits.
        // Écrire `PlatformId(42)` à la main ne désignerait plus le sol.
        platform: PlatformId::ecran(42, RoleEcran::Sol),
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
        bounds: Rect::new(0.0, 0.0, 800.0, 1032.0),
        scale: 1.0,
    }]);
    let large = World::from_screens(&[ScreenInfo {
        id: 42,
        work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
        bounds: Rect::new(0.0, 0.0, 1920.0, 1032.0),
        scale: 1.0,
    }]);

    let att = Attachment::On {
        // `PlatformId::ecran` : depuis la Tâche 1 de l'étape 4a, l'identité
        // d'une plateforme n'est plus l'id brut du moniteur mais un
        // encodage qui y ajoute le rôle (sol/mur/plafond) sur deux bits.
        // Écrire `PlatformId(42)` à la main ne désignerait plus le sol.
        platform: PlatformId::ecran(42, RoleEcran::Sol),
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

    let coin = window_top_left(Point::new(300.0, 1032.0), 1, POSE_STAND, &m, 1.0, Facing::Right);
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
    let m = manifeste_de_test(); // pose « decentre » : ancre [100, 120]

    let a_droite = window_top_left(Point::new(500.0, 1032.0), 1, "decentre", &m, 1.0, Facing::Right);
    let a_gauche = window_top_left(Point::new(500.0, 1032.0), 1, "decentre", &m, 1.0, Facing::Left);

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

    let coin = window_top_left(Point::new(300.0, 1032.0), 1, POSE_STAND, &m, 2.0, Facing::Right);
    assert_eq!(coin, Point::new(300.0 - 128.0, 1032.0 - 240.0));
}

#[test]
fn window_size_combine_les_deux_echelles() {
    let m = manifeste_de_test(); // scale = 1, frameSize = [128, 128]
    assert_eq!(window_size(&m, 1, 1.0), (128, 128));
    assert_eq!(window_size(&m, 1, 1.5), (192, 192));
}

#[test]
fn hitbox_ecran_se_place_dans_la_fenetre() {
    // hitbox du manifeste : [40, 20, 48, 100], ancre stand : [64, 120].
    // Le coin de la fenêtre est donc à (300-64, 1032-120) = (236, 912),
    // et la hitbox à 40/20 de là.
    let m = manifeste_de_test();

    let r = hitbox_ecran(
        Point::new(300.0, 1032.0),
        1,
        POSE_STAND,
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

    let droite = hitbox_ecran(
        Point::new(500.0, 1000.0),
        1,
        POSE_STAND,
        &m,
        1.0,
        Facing::Right,
    );
    let gauche = hitbox_ecran(
        Point::new(500.0, 1000.0),
        1,
        POSE_STAND,
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

// ── La fenêtre adaptative (2026-09-20) ──────────────────────────────────
//
// Ces quatre tests portent tous sur le même défaut, constaté à l'écran puis
// MESURÉ sur les rectangles réels des fenêtres : un pack dont les frames ne
// font pas toutes la taille déclarée s'affichait déformé et enfoncé sous le
// sol. `the-simba-cub` avait 79 px de fenêtre sous la zone de travail, donc
// le bas du personnage derrière la barre des tâches.

#[test]
fn window_size_suit_la_frame_affichee_et_non_la_plus_grande() {
    // LE test du défaut. `frameSize` vaut [213, 191] — la plus grande frame
    // du pack — mais la frame 1 ne fait que 128×128. C'est la frame qu'on
    // affiche qui doit dimensionner la fenêtre, sans quoi le webview étire
    // l'image (`width: 100%` dans index.html) et l'ancre ne désigne plus
    // rien.
    let m = manifeste_heterogene();

    assert_eq!(window_size(&m, 1, 1.0), (128, 128));
    assert_eq!(window_size(&m, 2, 1.0), (213, 191));
    assert_eq!(window_size(&m, 12, 1.0), (128, 191));

    // L'échelle de l'écran se combine comme avant.
    assert_eq!(window_size(&m, 1, 1.25), (160, 160));
}

#[test]
fn window_size_se_replie_sur_frame_size_pour_une_frame_inconnue() {
    // Le repli n'est pas un cas d'erreur : c'est le cas NORMAL de tout pack
    // écrit à la main, `blob` compris, qui n'a pas de table `frames`.
    let m = manifeste_heterogene();
    assert_eq!(window_size(&m, 99, 1.0), (213, 191));
}

#[test]
fn les_pieds_tombent_sur_le_sol_quelle_que_soit_la_frame() {
    // La conséquence visible, et celle qu'on a mesurée à l'écran : le BAS de
    // la fenêtre doit coïncider avec le sol quand l'ancre est au bas de la
    // frame. Avant le correctif, la fenêtre de la frame 1 (128 de haut)
    // était dimensionnée à 191 : 63 px de trop, tous sous le sol.
    let m = manifeste_heterogene();
    let sol = 1032.0;

    // Pose « stand » : frame 1, 128×128, ancre [64, 128] — le bas de la
    // boîte.
    let coin = window_top_left(Point::new(300.0, sol), 1, POSE_STAND, &m, 1.0, Facing::Right);
    let (_, hauteur) = window_size(&m, 1, 1.0);
    assert_eq!(coin.y + hauteur as f32, sol);

    // Pose « haute » : frame 12, 128×191, ancre [64, 191] — même exigence,
    // avec une frame d'une tout autre hauteur.
    let coin = window_top_left(Point::new(300.0, sol), 12, "haute", &m, 1.0, Facing::Right);
    let (_, hauteur) = window_size(&m, 12, 1.0);
    assert_eq!(coin.y + hauteur as f32, sol);
}

#[test]
fn window_top_left_miroite_sur_la_largeur_de_la_frame() {
    // Le miroir doit se faire sur la largeur de la frame AFFICHÉE, pas sur
    // celle de la plus grande du pack : sinon le personnage saute
    // latéralement de la différence (85 px ici) à chaque demi-tour.
    let m = manifeste_heterogene();

    let a_gauche = window_top_left(Point::new(500.0, 1032.0), 1, POSE_STAND, &m, 1.0, Facing::Left);
    let a_droite =
        window_top_left(Point::new(500.0, 1032.0), 1, POSE_STAND, &m, 1.0, Facing::Right);

    // Ancre [64, 128] dans une frame de 128 de large : elle est au milieu,
    // donc le miroir ne doit RIEN changer. Avec `frameSize` (213) on
    // obtenait 213 - 64 = 149, soit 85 px d'écart.
    assert_eq!(a_gauche.x, 500.0 - 64.0);
    assert_eq!(a_droite.x, a_gauche.x);
}

#[test]
fn l_ancre_de_la_frame_l_emporte_sur_celle_de_la_pose() {
    // La table `frames` porte une ancre par IMAGE (lue dans l'actions.xml du
    // pack à l'installation) : c'est la plus précise des trois, et
    // `ancre_de_frame` la sert déjà. Il fallait encore que la géométrie la
    // consulte.
    let m = manifeste_heterogene();

    // Pose « geste » : ancre de pose [64, 128], mais la frame 2 déclare
    // [106, 191]. C'est celle de la frame qui gagne.
    let coin = window_top_left(Point::new(500.0, 1032.0), 2, "geste", &m, 1.0, Facing::Left);
    assert_eq!(coin, Point::new(500.0 - 106.0, 1032.0 - 191.0));
}

// ── L'échelle entière (2026-09-20) ──────────────────────────────────────

#[test]
fn l_echelle_de_l_ecran_est_arrondie_a_l_entier() {
    // Du pixel-art agrandi d'un facteur FRACTIONNAIRE au plus proche voisin
    // (`image-rendering: pixelated` dans index.html) double un pixel source
    // sur quatre et laisse les trois autres : les contours deviennent des
    // escaliers inégaux. Seul un facteur entier est net.
    //
    // Constaté en passant d'un écran 27 pouces à 100 % à celui du portable
    // à 125 % : le même `blob`, soudain crénelé.
    assert_eq!(echelle_ecran_entiere(1.0), 1.0);
    assert_eq!(echelle_ecran_entiere(1.25), 1.0);
    assert_eq!(echelle_ecran_entiere(1.75), 2.0);
    assert_eq!(echelle_ecran_entiere(2.0), 2.0);

    // Jamais sous 1 : réduire du pixel-art au plus proche voisin JETTE des
    // pixels, ce qui est pire encore que d'en doubler.
    assert_eq!(echelle_ecran_entiere(0.5), 1.0);

    // Une sonde muette (0.0) ne doit pas faire disparaître le personnage.
    assert_eq!(echelle_ecran_entiere(0.0), 1.0);
}

#[test]
fn l_echelle_entiere_ne_touche_pas_au_reglage_de_l_auteur() {
    // `manifest.scale` et l'`echelle` de la config sont des choix
    // DÉLIBÉRÉS — « je veux ce personnage à moitié moins gros ». Les
    // arrondir les annulerait. Seul le facteur du MONITEUR est arrondi, et
    // il l'est en amont : `window_size` reçoit déjà le produit et ne
    // réarrondit rien.
    let json = r#"{
        "id": "t", "name": "T", "frameSize": [128, 128], "scale": 0.5,
        "hitbox": [0, 0, 128, 128],
        "poses": { "stand": { "frames": [1] } }
    }"#;
    let m: Manifest = serde_json::from_str(json).unwrap();
    assert_eq!(window_size(&m, 1, 1.0), (64, 64));
}

/// « Petite taille » (tray, demande de l'auteur 2026-09-24) : la taille
/// qu'a un personnage sur l'écran du portable à 125 %, où 128 px réels
/// paraissent 1,25 fois plus petits — d'où ×0,8, soit 1 / 1,25.
#[test]
fn la_petite_taille_est_celle_de_l_ecran_a_125() {
    assert_eq!(facteur_de_taille(false), 1.0);
    assert!((facteur_de_taille(true) * 1.25 - 1.0).abs() < 1e-6);
}
