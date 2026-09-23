//! Les tests de `intention` — sortis du fichier source le 2026-09-14.
//!
//! **Pourquoi ce fichier existe** : `behavior/intention.rs` faisait 3394 lignes dont 1776 de tests,
//! soit 52 %. Toute lecture du module en payait le double, pour rien la plupart
//! du temps. Les tests n'ont pas changé d'une ligne : ils ont seulement déménagé, et
//! se désindentent d'un cran puisqu'ils ne sont plus enfermés dans un `mod tests { }`.

use super::*;
use crate::behavior::Entrees;
use crate::character::manifest::Manifest;
use crate::geom::Point;
use crate::geom::Rect;
use crate::probe::fake::FakeProbe;
use crate::probe::{ScreenInfo, SystemProbe};
use crate::rng::XorShift32;

const DT: f32 = 1.0 / 60.0;

/// Les réglages par défaut. Construits ici et non lus depuis le disque :
/// un test qui lirait le `config.json` de la machine ne serait plus
/// reproductible.
fn reglages() -> Reglages {
    Reglages::depuis(&crate::config::Config::default())
}

// Les deux animations de jeu ajoutées ci-dessous ont des numéros de
// frame ARBITRAIRES (9, 10, 11) : ce manifeste est un DOUBLE, il ne sert
// qu'à dire quelles poses existent pour les tests. Les vraies frames de
// `blob` sont déclarées dans `characters/blob/mascot.json`.
//
// ⚠️ Le JSON ne supporte aucun commentaire : contrairement à du Rust
// normal, `//` à l'intérieur du `r#"..."#` ci-dessous ferait échouer
// `serde_json` avec un message peu clair (« key must be a string »).
// D'où ce commentaire ici, en dehors de la chaîne, plutôt qu'au milieu
// des clés `spinHead` / `sitDangle`.
//
// `sleep` (frame 12, tout aussi arbitraire) est la pose de sommeil de la
// Tâche 4. Le test `sans_la_pose_sleep_il_reste_assis_au_lieu_d_echouer`
// construit, lui, son propre manifeste SANS elle — c'est justement ce
// qu'il vérifie.
//
// `grabWall` et `climbWall` (frames 15, 16, arbitraires elles aussi)
// sont celles de l'escalade (étape 4a, Tâche 4). Sans elles, `set_pose`
// les ignorerait — c'est la couverture partielle (spec §8.6) — et
// `grimper_pose_les_bonnes_animations` échouerait pour une raison qui
// n'aurait rien à voir avec l'escalade.
//
// `grabCeiling` et `climbCeiling` (frames 17, 18, arbitraires) sont
// celles du plafond (Tâche 6). Le test
// `un_pack_sans_pose_de_plafond_grimpe_quand_meme` retire ces deux poses
// d'une COPIE de ce manifeste (`manifeste_sans`) plutôt que d'en écrire
// un second JSON : c'est exactement la couverture partielle qu'il
// vérifie.
fn manifeste() -> Manifest {
    let json = r#"{
        "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
        "hitbox": [40, 20, 48, 100],
        "poses": {
            "stand": { "frames": [1] },
            "walk":  { "frames": [2, 3], "frameMs": 120, "loop": true },
            "run":   { "frames": [4, 5], "frameMs": 80,  "loop": true },
            "sit":   { "frames": [6] },
            "fall":  { "frames": [7], "anchor": [64, 64] },
            "land":  { "frames": [8], "frameMs": 150 },
            "spinHead":  { "frames": [9, 10], "frameMs": 200 },
            "sitDangle": { "frames": [11], "anchor": [64, 112] },
            "sleep":     { "frames": [12] },
            "wake":      { "frames": [13, 14], "frameMs": 100 },
            "sprawl":    { "frames": [19] },
            "grabWall":  { "frames": [15] },
            "climbWall": { "frames": [15, 16], "frameMs": 150, "loop": true },
            "grabCeiling":  { "frames": [17] },
            "climbCeiling": { "frames": [17, 18], "frameMs": 150, "loop": true }
        }
    }"#;
    serde_json::from_str(json).unwrap()
}

fn monde() -> World {
    World::from_screens(&FakeProbe::deux_ecrans().screens())
}

fn perso(monde: &World, offset: f32) -> Character {
    perso_sur(monde, monde.platforms()[0].id, offset)
}

/// Un personnage posé sur la face `Top` de la plateforme donnée.
///
/// Extraite de `perso` plutôt qu'ajoutée à côté : les tests d'escalade
/// ont besoin de choisir LEUR plateforme (le sol de l'écran du milieu,
/// par exemple), et deux constructeurs indépendants auraient fini par
/// diverger sur le manifeste ou l'ancre.
fn perso_sur(monde: &World, platform: PlatformId, offset: f32) -> Character {
    // La position passée à `Character::new` n'est qu'un point de départ
    // pour le rendu : dès la première image, `world_position` la
    // recalcule depuis la plateforme (décision n° 1).
    let depart = match monde.get(platform) {
        Some(plat) => plat.rect.point_on(Face::Top, offset),
        None => Point::new(offset, 1032.0),
    };

    Character::new(
        manifeste(),
        Attachment::On {
            platform,
            face: Face::Top,
            offset,
        },
        depart,
    )
}

/// Un personnage au milieu du premier sol du monde donné.
fn perso_sur_le_sol(monde: &World) -> Character {
    let sol = monde.premier_sol().expect("un monde de test a un sol");
    perso_sur(monde, sol.id, 500.0)
}

fn offset_de(ch: &Character) -> f32 {
    match ch.attachment {
        Attachment::On { offset, .. } => offset,
        autre => panic!("attendu On, obtenu {autre:?}"),
    }
}

fn plateforme_de(ch: &Character) -> PlatformId {
    match ch.attachment {
        Attachment::On { platform, .. } => platform,
        autre => panic!("attendu On, obtenu {autre:?}"),
    }
}

/// Des `Entrees` inertes, avec un biais de repos choisi.
///
/// Toutes les autres valeurs sont neutres : la souris est loin, aucun
/// bouton n'est enfoncé. Un seul curseur pour tous les tests de sommeil.
/// Des entrées où seul le poids du repos varie.
///
/// ⚠️ **`utilisateur_actif` vaut `false`, et ce n'est pas un détail.**
/// Ce helper sert à simuler « l'utilisateur est parti depuis 2 minutes »,
/// ce que le seul biais ne suffit plus à dire : depuis l'invariant
/// « phase `Endormi` ⇒ utilisateur absent », `veut_dormir` exige LES DEUX
/// (le poids décide s'il VEUT dormir, le fait décide si c'est POSSIBLE).
///
/// Il valait `true`, ce qui contredisait le commentaire de ses propres
/// appelants (« comme inactif > 2 min ») et rendait deux tests
/// inatteignables : le personnage ne pouvait plus jamais s'affaler.
fn entrees_avec_biais_repos(x: f32) -> Entrees {
    Entrees {
        souris: Point::new(0.0, 0.0),
        echelle_affichage: 1.0,
        bouton_gauche: false,
        curseur_sur_le_personnage: false,
        biais: crate::signals::Biais {
            flaner: 1.0,
            se_reposer: x,
            jouer: 1.0,
            // Neutre : aucun de ces tests ne parle d'escalade.
            grimper: 1.0,
        },
        utilisateur_actif: false,
        commande: None,
    }
}

/// Des `Entrees` complètement neutres.
///
/// `poursuivre` prend désormais des `Entrees` quelle que soit
/// l'intention en cours — y compris `Flaner` et `Jouer`, qui ne les
/// consultent jamais. Ce raccourci évite de répéter la même valeur
/// neutre dans chacun des tests écrits avant cette tâche.
///
/// Construit le biais via `signals::Biais::neutre()` plutôt qu'en
/// recopiant `{ flaner: 1.0, se_reposer: 1.0, jouer: 1.0 }` : deux
/// définitions du neutre auraient fini par diverger, et celle de
/// `Biais::neutre()` sert de référence à toute la Tâche 4 (vague de
/// correction finale, point 7b).
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
fn flaner_finit_par_faire_avancer_le_personnage() {
    let m = monde();
    let mut ch = perso(&m, 500.0);
    let mut rng = XorShift32::seeded(3);
    ch.intention = Some(ActiveIntention::nouvelle(Intention::Flaner, Duration::ZERO));

    let depart = offset_de(&ch);
    let mut t = Duration::ZERO;
    // 3 secondes : assez pour qu'au moins une allure de marche soit
    // tirée, quelle que soit la graine.
    for _ in 0..180 {
        poursuivre(&mut ch, &m, &entrees_neutres(), &reglages(), t, DT, &mut rng);
        t += Duration::from_micros(16_667);
    }

    assert_ne!(offset_de(&ch), depart, "il n'a pas bougé en 3 s");
}

#[test]
fn flaner_alterne_les_allures_sans_jamais_courir() {
    // « jamais figé, jamais prévisible » : sur 30 s, l'arrêt et la marche
    // doivent avoir été vus tous les deux.
    //
    // La course, elle, ne doit **jamais** sortir : elle a quitté le
    // tirage de la flânerie (`poids_course` = 0 par défaut). Elle est
    // réservée à des actions qui la demanderont explicitement.
    let m = monde();
    let mut ch = perso(&m, 900.0);
    let mut rng = XorShift32::seeded(11);
    ch.intention = Some(ActiveIntention::nouvelle(Intention::Flaner, Duration::ZERO));

    let mut vues = std::collections::BTreeSet::new();
    let mut t = Duration::ZERO;
    for _ in 0..1_800 {
        poursuivre(&mut ch, &m, &entrees_neutres(), &reglages(), t, DT, &mut rng);
        vues.insert(ch.pose.clone());
        t += Duration::from_micros(16_667);
    }

    assert!(vues.contains(POSE_STAND), "jamais arrêté : {vues:?}");
    assert!(vues.contains(POSE_WALK), "jamais marché : {vues:?}");
    assert!(!vues.contains(POSE_RUN), "il a couru en flânant : {vues:?}");
}

#[test]
fn remonter_le_poids_de_course_le_fait_courir_a_nouveau() {
    // Le pendant du test précédent : la course est retirée du tirage par
    // un **réglage**, pas par une suppression de code. Ce test le prouve
    // — il échouerait si `Allure::Course` devenait inatteignable.
    let m = monde();
    let mut ch = perso(&m, 900.0);
    let mut rng = XorShift32::seeded(11);
    ch.intention = Some(ActiveIntention::nouvelle(Intention::Flaner, Duration::ZERO));

    // On part des défauts et on ne change QUE le poids de la course : le
    // reste du tempérament est celui de la production.
    let mut config = crate::config::Config::default();
    config.allures.poids_course = 6.0;
    let reglages = Reglages::depuis(&config);

    let mut vues = std::collections::BTreeSet::new();
    let mut t = Duration::ZERO;
    for _ in 0..1_800 {
        poursuivre(&mut ch, &m, &entrees_neutres(), &reglages, t, DT, &mut rng);
        vues.insert(ch.pose.clone());
        t += Duration::from_micros(16_667);
    }

    assert!(vues.contains(POSE_RUN), "jamais couru : {vues:?}");
}

#[test]
fn arrive_au_bord_il_fait_demi_tour_plutot_que_de_tomber() {
    // Le sol du premier écran va de 0 à 1920. On le place à 3 px du bord
    // droit, tourné à droite, en marche forcée.
    //
    // NOTE : le second écran du monde de test commence exactement à
    // x = 1920, donc `face_voisine` le trouverait. On prend donc un
    // monde à UN SEUL écran pour éprouver le demi-tour.
    let m = World::from_screens(&FakeProbe::un_ecran().screens());
    let mut ch = Character::new(
        manifeste(),
        Attachment::On {
            platform: m.platforms()[0].id,
            face: Face::Top,
            offset: 1917.0,
        },
        Point::new(1917.0, 1032.0),
    );
    ch.facing = crate::character::Facing::Right;
    let mut rng = XorShift32::seeded(5);
    ch.intention = Some(ActiveIntention {
        kind: Intention::Flaner,
        depuis: Duration::ZERO,
        etat: EtatIntention::Flanerie {
            allure: Allure::Marche,
            jusqu_a: Duration::from_secs(60),
        },
    });

    let mut t = Duration::ZERO;
    for _ in 0..30 {
        poursuivre(&mut ch, &m, &entrees_neutres(), &reglages(), t, DT, &mut rng);
        t += Duration::from_micros(16_667);
    }

    // Il est toujours accroché — il n'est pas tombé du bord du monde.
    assert!(matches!(ch.attachment, Attachment::On { .. }));
    // Et il repart vers la gauche.
    assert_eq!(ch.facing, crate::character::Facing::Left);
    assert!(offset_de(&ch) <= 1920.0);
}

#[test]
fn il_passe_sur_l_ecran_voisin_quand_il_y_en_a_un() {
    // « il circule sur tous les écrans » (CLAUDE.md, étape 1). Le sol du
    // premier écran finit à x = 1920, celui du second y commence : les
    // deux faces sont adjointes, il doit enjamber la frontière.
    //
    // On force la marche vers la droite depuis tout près du bord.
    let m = monde();
    let mut ch = perso(&m, 1919.0);
    ch.facing = crate::character::Facing::Right;
    let mut rng = XorShift32::seeded(5);
    ch.intention = Some(ActiveIntention {
        kind: Intention::Flaner,
        depuis: Duration::ZERO,
        etat: EtatIntention::Flanerie {
            allure: Allure::Marche,
            jusqu_a: Duration::from_secs(60),
        },
    });

    let premier = m.platforms()[0].id;
    let mut t = Duration::ZERO;
    let mut passe = false;
    for _ in 0..60 {
        poursuivre(&mut ch, &m, &entrees_neutres(), &reglages(), t, DT, &mut rng);
        if plateforme_de(&ch) != premier {
            passe = true;
            break;
        }
        t += Duration::from_micros(16_667);
    }

    assert!(passe, "il n'a pas franchi la frontière entre les écrans");
    assert_eq!(m.get(plateforme_de(&ch)).unwrap().rect.left(), 1920.0);
    // Il entre par le bord gauche du sol voisin, donc à un offset petit.
    assert!(offset_de(&ch) < 50.0);
    // Et il continue dans le même sens : pas de demi-tour parasite.
    assert_eq!(ch.facing, crate::character::Facing::Right);
}

#[test]
fn se_reposer_s_assoit_puis_se_termine() {
    let m = monde();
    let mut ch = perso(&m, 500.0);
    let mut rng = XorShift32::seeded(1);
    ch.intention = Some(ActiveIntention::nouvelle(
        Intention::SeReposer,
        Duration::ZERO,
    ));

    // Première image : il s'assoit.
    let issue = poursuivre(
        &mut ch,
        &m,
        &entrees_neutres(),
        &reglages(),
        Duration::ZERO,
        DT,
        &mut rng,
    );
    assert_eq!(issue, Issue::EnCours);
    assert_eq!(ch.pose, POSE_SIT);

    // Il ne bouge pas pendant le repos.
    let ou = offset_de(&ch);
    poursuivre(
        &mut ch,
        &m,
        &entrees_neutres(),
        &reglages(),
        Duration::from_secs(2),
        DT,
        &mut rng,
    );
    assert_eq!(offset_de(&ch), ou);

    // Le repos dure au plus 15 s ; à 16 s il est fini.
    let issue = poursuivre(
        &mut ch,
        &m,
        &entrees_neutres(),
        &reglages(),
        Duration::from_secs(16),
        DT,
        &mut rng,
    );
    assert_eq!(issue, Issue::Finie);
    assert!(ch.intention.is_none());
}

#[test]
fn toute_intention_expire_au_delai_d_abandon() {
    // **LE test de la décision n° 4.** Une seule règle remplace toute
    // l'énumération des cas de blocage : passé 20 s, l'intention échoue.
    let m = monde();
    let mut ch = perso(&m, 500.0);
    let mut rng = XorShift32::seeded(1);

    for kind in [Intention::Flaner, Intention::SeReposer] {
        ch.intention = Some(ActiveIntention::nouvelle(kind, Duration::ZERO));

        // Juste avant le délai : l'intention n'a pas expiré. Elle peut
        // s'être terminée normalement (un repos dure au plus 15 s), donc
        // on vérifie seulement qu'elle n'a pas ÉCHOUÉ.
        let avant = poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages(),
            DELAI_ABANDON - Duration::from_millis(100),
            DT,
            &mut rng,
        );
        assert_ne!(avant, Issue::Echouee, "{kind:?} a expiré trop tôt");

        // Juste après : expirée. On réarme l'intention, la ligne
        // précédente ayant pu la consommer.
        ch.intention = Some(ActiveIntention::nouvelle(kind, Duration::ZERO));
        let apres = poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages(),
            DELAI_ABANDON + Duration::from_millis(100),
            DT,
            &mut rng,
        );
        assert_eq!(apres, Issue::Echouee, "{kind:?} n'a pas expiré");
    }
}

#[test]
fn le_delai_court_depuis_le_debut_de_l_intention_pas_depuis_zero() {
    // Une intention commencée à t = 100 s doit expirer à 120 s, pas
    // immédiatement.
    let m = monde();
    let mut ch = perso(&m, 500.0);
    let mut rng = XorShift32::seeded(1);
    ch.intention = Some(ActiveIntention::nouvelle(
        Intention::Flaner,
        Duration::from_secs(100),
    ));

    let issue = poursuivre(
        &mut ch,
        &m,
        &entrees_neutres(),
        &reglages(),
        Duration::from_secs(110),
        DT,
        &mut rng,
    );
    assert_eq!(issue, Issue::EnCours);

    let issue = poursuivre(
        &mut ch,
        &m,
        &entrees_neutres(),
        &reglages(),
        Duration::from_secs(121),
        DT,
        &mut rng,
    );
    assert_eq!(issue, Issue::Echouee);
}

#[test]
fn sans_intention_poursuivre_ne_fait_rien_et_le_dit() {
    let m = monde();
    let mut ch = perso(&m, 500.0);
    let mut rng = XorShift32::seeded(1);
    assert_eq!(
        poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages(),
            Duration::ZERO,
            DT,
            &mut rng
        ),
        Issue::Finie
    );
}

#[test]
fn jouer_pose_l_animation_du_jeu_tire() {
    let m = monde();
    let mut ch = perso(&m, 500.0);
    let mut rng = XorShift32::seeded(1);

    for (jeu, pose) in [
        (Jeu::TeteQuiTourne, POSE_SPIN_HEAD),
        (Jeu::JambesQuiBalancent, POSE_SIT_DANGLE),
    ] {
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::Jouer(jeu),
            Duration::ZERO,
        ));
        let issue = poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages(),
            Duration::ZERO,
            DT,
            &mut rng,
        );
        assert_eq!(issue, Issue::EnCours);
        assert_eq!(ch.pose, pose, "jeu {jeu:?}");
    }
}

#[test]
fn jouer_ne_deplace_pas_le_personnage() {
    // Les deux jeux sont des animations assises : `Velocity="0,0"` dans
    // `actions.xml`. Si le personnage dérivait, c'est qu'une vitesse
    // traîne quelque part.
    let m = monde();
    let mut ch = perso(&m, 500.0);
    let mut rng = XorShift32::seeded(1);
    ch.intention = Some(ActiveIntention::nouvelle(
        Intention::Jouer(Jeu::TeteQuiTourne),
        Duration::ZERO,
    ));

    let ou = offset_de(&ch);
    for i in 0..120 {
        poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages(),
            Duration::from_secs_f32(i as f32 * DT),
            DT,
            &mut rng,
        );
    }
    assert_eq!(offset_de(&ch), ou);
}

#[test]
fn jouer_se_termine_avant_le_delai_d_abandon() {
    // Comme le repos : la durée est bornée sous `DELAI_ABANDON`, sinon
    // l'issue serait `Echouee` au lieu de `Finie` et la trace du mode
    // simulation mentirait.
    let m = monde();
    let mut ch = perso(&m, 500.0);
    let mut rng = XorShift32::seeded(1);
    ch.intention = Some(ActiveIntention::nouvelle(
        Intention::Jouer(Jeu::TeteQuiTourne),
        Duration::ZERO,
    ));

    let mut issue = Issue::EnCours;
    for i in 0..(20 * 60) {
        issue = poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages(),
            Duration::from_secs_f32(i as f32 * DT),
            DT,
            &mut rng,
        );
        if issue != Issue::EnCours {
            break;
        }
    }
    assert_eq!(issue, Issue::Finie);
}

#[test]
fn jouer_sans_la_pose_echoue_au_lieu_de_figer() {
    // Défense en profondeur, comme `se_reposer_sans_pose_sit_echoue` :
    // le tirage filtre déjà, mais une config bricolée ne doit pas
    // produire un personnage invisible.
    let json = r#"{
        "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
        "hitbox": [40,20,48,100],
        "poses": { "stand": { "frames": [1] }, "walk": { "frames": [2] } }
    }"#;
    let m = monde();
    let mut ch = Character::new(
        serde_json::from_str(json).unwrap(),
        Attachment::On {
            platform: m.platforms()[0].id,
            face: Face::Top,
            offset: 500.0,
        },
        Point::new(500.0, 1032.0),
    );
    let mut rng = XorShift32::seeded(1);
    ch.intention = Some(ActiveIntention::nouvelle(
        Intention::Jouer(Jeu::TeteQuiTourne),
        Duration::ZERO,
    ));

    assert_eq!(
        poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages(),
            Duration::ZERO,
            DT,
            &mut rng
        ),
        Issue::Echouee
    );
}

#[test]
fn se_reposer_sans_pose_sit_echoue_au_lieu_de_figer() {
    // Défense en profondeur : le tirage ne devrait jamais proposer
    // `SeReposer` à un personnage sans `sit` (desire.rs). Mais si une
    // config bricolée y parvenait, l'intention doit ÉCHOUER — pas
    // asseoir un personnage sur une pose inexistante.
    let json = r#"{
        "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
        "hitbox": [40,20,48,100],
        "poses": { "stand": { "frames": [1] }, "walk": { "frames": [2] } }
    }"#;
    let m = monde();
    let mut ch = Character::new(
        serde_json::from_str(json).unwrap(),
        Attachment::On {
            platform: m.platforms()[0].id,
            face: Face::Top,
            offset: 500.0,
        },
        Point::new(500.0, 1032.0),
    );
    let mut rng = XorShift32::seeded(1);
    ch.intention = Some(ActiveIntention::nouvelle(
        Intention::SeReposer,
        Duration::ZERO,
    ));

    assert_eq!(
        poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages(),
            Duration::ZERO,
            DT,
            &mut rng
        ),
        Issue::Echouee
    );
}

#[test]
fn sans_signal_il_reste_assis_et_ne_s_affale_pas() {
    // **Une sieste ne s'improvise pas.** Sans signal, le biais vaut 1,
    // donc sous le seuil de 2 : il s'assoit et c'est tout. S'il
    // s'affalait de lui-même, « il dort quand tu t'en vas » perdrait tout
    // son sens — il dormirait tout le temps.
    let m = monde();
    let mut ch = perso(&m, 500.0);
    let mut rng = XorShift32::seeded(1);
    let e = entrees_avec_biais_repos(1.0);

    ch.intention = Some(ActiveIntention::nouvelle(
        Intention::SeReposer,
        Duration::ZERO,
    ));

    for i in 0..(14 * 60) {
        let t = Duration::from_secs_f32(i as f32 * DT);
        if poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng) != Issue::EnCours {
            break;
        }
        assert_eq!(ch.pose, POSE_SIT, "à {:.1} s il devrait être assis", t.as_secs_f32());
    }
}

#[test]
fn avec_un_signal_il_s_assoit_puis_s_affale() {
    // La promesse de l'étape, dans l'ordre : 11 puis 21. C'est
    // l'ENCHAÎNEMENT qui dit « il dort », pas la frame 21 seule.
    let m = monde();
    let mut ch = perso(&m, 500.0);
    let mut rng = XorShift32::seeded(1);
    let e = entrees_avec_biais_repos(8.0); // comme « inactif > 2 min »

    ch.intention = Some(ActiveIntention::nouvelle(
        Intention::SeReposer,
        Duration::ZERO,
    ));

    // Première image : assis.
    poursuivre(&mut ch, &m, &e, &reglages(), Duration::ZERO, DT, &mut rng);
    assert_eq!(ch.pose, POSE_SIT);

    // Il finit par s'affaler, et en moins de 20 s (le délai d'abandon).
    let mut endormi_a = None;
    for i in 1..(20 * 60) {
        let t = Duration::from_secs_f32(i as f32 * DT);
        poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng);
        if ch.pose == POSE_SLEEP {
            endormi_a = Some(t);
            break;
        }
    }
    assert!(endormi_a.is_some(), "il ne s'est jamais affalé");
}

#[test]
fn re_tirer_le_repos_pendant_le_sommeil_ne_le_fait_pas_se_rasseoir() {
    // **LE test de la continuité de pose**, et le seul qui justifie
    // qu'on n'ait PAS touché au délai d'abandon (décision n° 4).
    //
    // Un sommeil dure 20 à 60 s, le délai d'abandon coupe à 20 s, donc
    // l'intention est re-tirée. Sans continuité, on le verrait se
    // rasseoir puis se raffaler toutes les 20 secondes — un tic visible
    // à l'écran, absurde et inexplicable pour qui regarde.
    let m = monde();
    let mut ch = perso(&m, 500.0);
    let mut rng = XorShift32::seeded(1);
    let e = entrees_avec_biais_repos(8.0);

    // On le met directement dans l'état « endormi ».
    ch.set_pose(POSE_SLEEP, Duration::ZERO);
    assert_eq!(ch.pose, POSE_SLEEP);

    // Une intention de repos FRAÎCHE, comme après un re-tirage.
    ch.intention = Some(ActiveIntention::nouvelle(
        Intention::SeReposer,
        Duration::from_secs(30),
    ));

    // La première image ne doit PAS le rasseoir.
    poursuivre(
        &mut ch,
        &m,
        &e,
        &reglages(),
        Duration::from_secs(30),
        DT,
        &mut rng,
    );
    assert_eq!(
        ch.pose, POSE_SLEEP,
        "il s'est rassis : la continuité de pose est cassée"
    );
}

#[test]
fn sans_la_pose_sleep_il_reste_assis_au_lieu_d_echouer() {
    // Couverture partielle appliquée à une PHASE et non à une intention
    // (spec §8.6). Un pack sans pose de sommeil doit se reposer
    // normalement — assis — et non voir son repos échouer.
    let json = r#"{
        "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
        "hitbox": [40,20,48,100],
        "poses": { "stand": { "frames": [1] }, "walk": { "frames": [2] },
                   "sit": { "frames": [11] } }
    }"#;
    let m = monde();
    let mut ch = Character::new(
        serde_json::from_str(json).unwrap(),
        Attachment::On {
            platform: m.platforms()[0].id,
            face: Face::Top,
            offset: 500.0,
        },
        Point::new(500.0, 1032.0),
    );
    let mut rng = XorShift32::seeded(1);
    let e = entrees_avec_biais_repos(8.0);

    ch.intention = Some(ActiveIntention::nouvelle(
        Intention::SeReposer,
        Duration::ZERO,
    ));

    // `issue` sort de la boucle (comme dans
    // `jouer_se_termine_avant_le_delai_d_abandon`) pour pouvoir
    // l'affirmer APRÈS coup, et pas seulement à l'intérieur.
    let mut issue = Issue::EnCours;
    for i in 0..(19 * 60) {
        let t = Duration::from_secs_f32(i as f32 * DT);
        issue = poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng);
        assert_ne!(issue, Issue::Echouee, "le repos ne doit pas échouer");
        if issue != Issue::EnCours {
            break;
        }
        assert_eq!(ch.pose, POSE_SIT);
    }

    // Le repos doit se TERMINER normalement, et pas seulement « ne jamais
    // échouer ». Sans cette assertion, le test passerait même si la garde
    // `has_pose(POSE_SLEEP)` disparaissait : la phase basculerait en
    // `Endormi`, `set_pose` refuserait silencieusement la pose absente, et
    // `ch.pose` resterait figé sur `sit` par EFFET DE BORD — avec toutes
    // les assertions de la boucle encore vertes.
    assert_eq!(
        issue, Issue::Finie,
        "sans pose `sleep`, le repos doit se terminer, pas rester en cours"
    );
}

/// La durée de l'animation de réveil du manifeste de test : 2 × 100 ms.
const ANIM_REVEIL: Duration = Duration::from_millis(200);

#[test]
fn au_reveil_il_dort_encore_un_moment_puis_se_redresse() {
    // **Le test de la demande d'origine** : au déverrouillage, le réveil
    // ne doit pas être instantané. Il dort d'abord — 2,5 à 4 s — et c'est
    // seulement à la fin qu'il se redresse.
    //
    // Noter `utilisateur_actif = true` : l'utilisateur vient de taper son
    // mot de passe, il est actif par construction. C'est ce qui rend ce
    // test intéressant — si le réveil était une phase `Endormi`,
    // l'interruption le couperait à la première image.
    let m = monde();
    let mut ch = perso(&m, 500.0);
    let mut rng = XorShift32::seeded(1);
    let mut e = entrees_avec_biais_repos(8.0);
    e.utilisateur_actif = true;

    ch.set_pose(POSE_SLEEP, Duration::ZERO);
    ch.intention = Some(ActiveIntention::reveil(Duration::ZERO));

    let mut a_dormi = false;
    let mut redresse_a = None;
    let mut fini_a = None;

    for i in 0..(10 * 60) {
        let t = Duration::from_secs_f32(i as f32 * DT);
        let issue = poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng);

        if issue != Issue::EnCours {
            assert_eq!(issue, Issue::Finie, "le réveil ne doit pas échouer");
            fini_a = Some(t);
            break;
        }

        if ch.pose == POSE_SLEEP {
            // Une fois redressé, il ne doit PAS se raffaler : ce serait
            // le clignotement que la version précédente produisait.
            assert!(
                redresse_a.is_none(),
                "il s'est rendormi après s'être redressé, à {:.2} s",
                t.as_secs_f32()
            );
            a_dormi = true;
        }
        if ch.pose == POSE_WAKE && redresse_a.is_none() {
            redresse_a = Some(t);
        }
    }

    assert!(a_dormi, "il n'a pas dormi du tout avant d'émerger");
    let redresse_a = redresse_a.expect("il ne s'est jamais redressé");
    let fini_a = fini_a.expect("le réveil ne s'est jamais terminé");

    // Le sommeil résiduel est tiré entre 2,5 et 4 s. On borne des DEUX
    // côtés : sans la borne basse, un réveil redevenu instantané
    // passerait — c'est exactement le défaut qu'on corrige ici.
    assert!(
        redresse_a >= Duration::from_secs_f32(2.5),
        "il s'est redressé au bout de {:.2} s : c'est trop tôt, le sommeil              résiduel doit durer au moins 2,5 s",
        redresse_a.as_secs_f32()
    );
    assert!(
        redresse_a <= Duration::from_secs_f32(4.0) + ANIM_REVEIL,
        "il s'est redressé au bout de {:.2} s : c'est trop tard",
        redresse_a.as_secs_f32()
    );

    // Et il se redresse pendant TOUTE l'animation, à une image près.
    let duree_redresse = fini_a.saturating_sub(redresse_a);
    assert!(
        duree_redresse + Duration::from_secs_f32(DT) >= ANIM_REVEIL,
        "l'animation de réveil n'a duré que {:.0} ms au lieu de 200",
        duree_redresse.as_secs_f32() * 1000.0
    );
}

#[test]
fn deux_reveils_ne_tombent_pas_a_la_meme_image() {
    // **La marge, appliquée au réveil** (décision n° 3). À l'étape 3 il y
    // aura plusieurs personnages : s'ils émergeaient tous à la même
    // image, on verrait une chorégraphie au lieu d'animaux.
    //
    // ⚠️ On sème UNE SEULE FOIS et on laisse l'état avancer. Re-semer
    // `XorShift32::seeded(n)` avec de petits entiers séquentiels biaise
    // le premier tirage et rendrait ce test faussement vert — le piège
    // est consigné dans CLAUDE.md, il a déjà coûté un diagnostic.
    let m = monde();
    let mut rng = XorShift32::seeded(7);
    let mut e = entrees_avec_biais_repos(8.0);
    e.utilisateur_actif = true;

    let mut durees = Vec::new();
    for _ in 0..40 {
        let mut ch = perso(&m, 500.0);
        ch.set_pose(POSE_SLEEP, Duration::ZERO);
        ch.intention = Some(ActiveIntention::reveil(Duration::ZERO));

        for i in 0..(10 * 60) {
            let t = Duration::from_secs_f32(i as f32 * DT);
            if poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng) != Issue::EnCours {
                durees.push(t);
                break;
            }
        }
    }

    assert_eq!(durees.len(), 40, "un réveil ne s'est pas terminé");
    let min = durees.iter().min().unwrap();
    let max = durees.iter().max().unwrap();
    assert!(
        max.saturating_sub(*min) > Duration::from_secs_f32(0.8),
        "les 40 réveils tiennent dans {:.2} s : le tirage ne varie pas",
        max.saturating_sub(*min).as_secs_f32()
    );
}

#[test]
fn sans_la_pose_wake_il_se_reveille_quand_meme() {
    // Couverture partielle (spec §8.6). Un pack sans animation de réveil
    // reste affalé le temps de la phase, puis repart — il ne doit NI
    // échouer, NI rester bloqué.
    let json = r#"{
        "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
        "hitbox": [40,20,48,100],
        "poses": { "stand": { "frames": [1] }, "walk": { "frames": [2] },
                   "sit": { "frames": [11] }, "sleep": { "frames": [12] } }
    }"#;
    let m = monde();
    let mut ch = Character::new(
        serde_json::from_str(json).unwrap(),
        Attachment::On {
            platform: m.platforms()[0].id,
            face: Face::Top,
            offset: 500.0,
        },
        Point::new(500.0, 1032.0),
    );
    let mut rng = XorShift32::seeded(1);
    let mut e = entrees_avec_biais_repos(8.0);
    e.utilisateur_actif = true;

    ch.set_pose(POSE_SLEEP, Duration::ZERO);
    ch.intention = Some(ActiveIntention::reveil(Duration::ZERO));

    let mut issue = Issue::EnCours;
    for i in 0..(10 * 60) {
        let t = Duration::from_secs_f32(i as f32 * DT);
        issue = poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng);
        if issue != Issue::EnCours {
            break;
        }
        assert_eq!(ch.pose, POSE_SLEEP, "sans `wake`, il reste affalé");
    }
    assert_eq!(issue, Issue::Finie, "le réveil doit se terminer");
}

// ── L'intention `Grimper` (Tâche 4, étape 4a) ───────────────────────

/// Un monde d'un écran isolé — donc avec ses deux murs.
///
/// `FakeProbe::deux_ecrans` ne conviendrait pas : deux écrans côte à côte
/// se masquent mutuellement un mur (design §2.3), et l'on veut ici le cas
/// le plus simple.
fn monde_mure() -> World {
    World::from_screens(&FakeProbe::un_ecran().screens())
}

/// Fait tourner l'intention jusqu'à ce que `condition` soit vraie, ou
/// jusqu'à `max_s` secondes simulées. Rend le temps écoulé.
///
/// Écrite une fois ici plutôt que recopiée dans chaque test : les tests
/// d'escalade durent des dizaines de secondes simulées, et la boucle est
/// toujours la même.
///
/// `impl FnMut(&Character) -> bool` plutôt qu'un `&dyn Fn` : le
/// compilateur intègre la fermeture à l'appel, et le point d'appel reste
/// une simple lambda. `FnMut` et non `Fn` pour qu'une condition puisse
/// compter ce qu'elle voit passer si besoin.
fn derouler(
    ch: &mut Character,
    world: &World,
    rng: &mut dyn Rng,
    max_s: f32,
    mut condition: impl FnMut(&Character) -> bool,
) -> f32 {
    let reglages = reglages();
    let mut t = Duration::ZERO;
    let mut ecoule = 0.0;

    while ecoule < max_s {
        if condition(ch) {
            return ecoule;
        }
        poursuivre(ch, world, &entrees_neutres(), &reglages, t, DT, rng);
        t += Duration::from_secs_f32(DT);
        ecoule += DT;
    }

    ecoule
}

#[test]
fn choisir_depuis_un_mur_reprend_l_escalade_au_lieu_d_echouer() {
    // ⚠️ **Ce test verrouillait l'ANCIEN comportement, et c'était très
    // exactement le bug rapporté à l'écran** (menu contextuel de
    // l'escalade) : la garde structurelle de la Tâche 7 faisait ÉCHOUER
    // `Choisir` sur toute face non-`Top`, et un personnage sur qui l'on
    // choisissait « Grimper au mur » au menu — donc déjà accroché à un
    // mur — tombait au lieu de continuer à monter.
    //
    // La garde protégeait un vrai risque, qui reste vérifié ci-dessous
    // (`assert_ne!(ch.pose, POSE_WALK, …)`) : `Rejoindre`, la phase qui
    // suit `Choisir` sur `Face::Top`, est une marche AU SOL. La
    // correction ne supprime pas la protection, elle change la réponse :
    // au lieu d'échouer sur une face verticale, `Choisir` saute
    // directement dans la phase d'ESCALADE qui correspond (`Paroi`),
    // sans jamais passer par `Rejoindre` — voir le commentaire de
    // `Choisir` dans `grimper`.
    //
    // `ActiveIntention::accroche` n'est pas concernée par cette phase :
    // elle pose directement `Accroche`, jamais `Choisir` — voir
    // `accroche_par_un_lancer_il_ne_lache_pas...` dans `reflex.rs`, qui
    // continue de passer.
    let m = monde_mure();
    let mur = m
        .platforms()
        .iter()
        .find(|p| p.has_face(Face::Right))
        .expect("mur gauche");

    let mut ch = perso_sur_le_sol(&m);
    ch.attachment = Attachment::On {
        platform: mur.id,
        face: Face::Right,
        offset: 300.0,
    };
    ch.intention = Some(ActiveIntention {
        kind: Intention::Grimper,
        depuis: Duration::ZERO,
        etat: EtatIntention::Grimpe {
            phase: PhaseGrimpe::Choisir { presse: false },
            jusqu_a: Duration::ZERO,
        },
    });

    let mut rng = XorShift32::seeded(11);
    let reglages = reglages();
    let mut t = Duration::ZERO;

    // Plusieurs images, pas une seule : l'escalade reprise doit tenir
    // sur la durée, pas seulement à la première image.
    for _ in 0..5 {
        let issue = poursuivre(&mut ch, &m, &entrees_neutres(), &reglages, t, DT, &mut rng);
        assert_ne!(ch.pose, POSE_WALK, "il ne doit jamais marcher sur le mur");
        assert_eq!(
            issue,
            Issue::EnCours,
            "l'escalade reprise doit continuer, pas échouer"
        );
        t += Duration::from_secs_f32(DT);
    }

    // Il reste accroché au MUR, avec une intention `Grimper` toujours
    // vivante : c'est la propriété qui corrige le bug — plus de chute.
    assert!(
        matches!(ch.attachment, Attachment::On { face: Face::Right, .. }),
        "il devrait toujours être sur le mur, en train de monter, il est {:?}",
        ch.attachment
    );
    assert_eq!(
        ch.intention.map(|ai| ai.kind),
        Some(Intention::Grimper),
        "l'escalade reprise ne doit pas avoir été abandonnée"
    );
}

#[test]
fn grimper_rejoint_le_mur_puis_s_y_accroche() {
    let m = monde_mure();
    let mut ch = perso_sur_le_sol(&m);
    ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

    let mut rng = XorShift32::seeded(7);
    derouler(&mut ch, &m, &mut rng, 60.0, |c| {
        matches!(c.attachment, Attachment::On { face, .. } if face != Face::Top)
    });

    match ch.attachment {
        Attachment::On { face, .. } => {
            assert!(face == Face::Left || face == Face::Right, "il est sur un mur");
        }
        autre => panic!("il devrait être accroché, il est {autre:?}"),
    }
}

#[test]
fn grimper_monte_vraiment() {
    let m = monde_mure();
    let mut ch = perso_sur_le_sol(&m);
    ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

    let mut rng = XorShift32::seeded(7);

    // On le laisse rejoindre le mur et grimper un moment.
    derouler(&mut ch, &m, &mut rng, 90.0, |c| {
        matches!(c.attachment, Attachment::On { face, offset, .. }
                 if face != Face::Top && offset < 900.0)
    });

    match ch.attachment {
        Attachment::On { face, offset, .. } => {
            assert_ne!(face, Face::Top);
            // L'offset d'une face verticale compte vers le bas : plus
            // petit = plus haut. Le bas du mur est à 1032.
            assert!(offset < 1000.0, "il devrait avoir quitté le bas du mur");
        }
        autre => panic!("il devrait être sur le mur, il est {autre:?}"),
    }
}

#[test]
fn grimper_pose_les_bonnes_animations() {
    let m = monde_mure();
    let mut ch = perso_sur_le_sol(&m);
    ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

    let mut rng = XorShift32::seeded(7);
    derouler(&mut ch, &m, &mut rng, 90.0, |c| c.pose == POSE_CLIMB_WALL);
    assert_eq!(ch.pose, POSE_CLIMB_WALL);
}

#[test]
fn l_attache_au_mur_et_la_pose_changent_dans_la_meme_image() {
    // Sans cette garantie, la face serait déjà verticale alors que la
    // pose dirait encore `walk` — une image de marche dans le vide, que
    // l'invariant de simulation de la Tâche 7 relèverait.
    let m = monde_mure();
    let mut ch = perso_sur_le_sol(&m);
    ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

    let mut rng = XorShift32::seeded(7);
    derouler(&mut ch, &m, &mut rng, 60.0, |c| {
        matches!(c.attachment, Attachment::On { face, .. } if face != Face::Top)
    });

    // `derouler` rend la main à l'image où la condition devient vraie,
    // c'est-à-dire AVANT d'appeler `poursuivre` une fois de plus : la
    // pose observée est donc bien celle posée par l'image de l'attache.
    assert_eq!(
        ch.pose, POSE_GRAB_WALL,
        "la pose doit basculer dans la même image que la face"
    );
}

#[test]
fn grimper_regarde_le_mur() {
    // Mur gauche → il regarde à gauche ; mur droit → à droite. Sans quoi
    // il grimperait dos à la paroi.
    let m = monde_mure();
    let mut ch = perso_sur_le_sol(&m);
    ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

    let mut rng = XorShift32::seeded(7);
    derouler(&mut ch, &m, &mut rng, 90.0, |c| {
        matches!(c.attachment, Attachment::On { face, .. } if face != Face::Top)
    });

    // ⚠️ Une face `Right` appartient au mur GAUCHE : le personnage se
    // tient à sa droite, donc à l'intérieur de l'écran (design §2.1).
    // C'est contre-intuitif la première fois.
    match ch.attachment {
        Attachment::On {
            face: Face::Right, ..
        } => {
            assert_eq!(ch.facing, Facing::Left, "mur gauche → il regarde à gauche")
        }
        Attachment::On {
            face: Face::Left, ..
        } => {
            assert_eq!(ch.facing, Facing::Right, "mur droit → il regarde à droite")
        }
        autre => panic!("il devrait être sur un mur, il est {autre:?}"),
    }
}

#[test]
fn grimper_echoue_immediatement_sans_mur() {
    // L'écran du MILIEU d'une rangée de trois n'a aucun mur : ses deux
    // bords sont recouverts par ses voisins (design §2.3). L'intention
    // doit échouer tout de suite pour qu'une autre soit tirée — et
    // surtout pas figer le personnage.
    let entoure = World::from_screens(&[
        ScreenInfo {
            id: 1,
            work_area: Rect::new(-1920.0, 0.0, 1920.0, 1032.0),
            bounds: Rect::new(-1920.0, 0.0, 1920.0, 1032.0),
            scale: 1.0,
        },
        ScreenInfo {
            id: 2,
            work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
            bounds: Rect::new(0.0, 0.0, 1920.0, 1032.0),
            scale: 1.0,
        },
        ScreenInfo {
            id: 3,
            work_area: Rect::new(1920.0, 0.0, 1920.0, 1032.0),
            bounds: Rect::new(1920.0, 0.0, 1920.0, 1032.0),
            scale: 1.0,
        },
    ]);

    // Le personnage est sur le sol de l'écran 2, celui du milieu.
    let sol_milieu = entoure
        .platforms()
        .iter()
        .find(|p| p.has_face(Face::Top) && p.rect.left() == 0.0)
        .expect("le sol du milieu");
    let mut ch = perso_sur(&entoure, sol_milieu.id, 500.0);
    ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

    let mut rng = XorShift32::seeded(3);
    let issue = poursuivre(
        &mut ch,
        &entoure,
        &entrees_neutres(),
        &reglages(),
        Duration::ZERO,
        DT,
        &mut rng,
    );

    assert_eq!(issue, Issue::Echouee);
    assert!(ch.intention.is_none());
}

#[test]
fn grimper_a_un_delai_d_abandon_de_120_s() {
    let r = reglages();
    assert_eq!(delai_abandon(Intention::Grimper, &r), Duration::from_secs(120));
    assert_eq!(delai_abandon(Intention::Flaner, &r), DELAI_ABANDON);
    assert_eq!(delai_abandon(Intention::SeReposer, &r), DELAI_ABANDON);
    assert_eq!(delai_abandon(Intention::Jouer(Jeu::TeteQuiTourne), &r), DELAI_ABANDON);
}

#[test]
fn le_delai_d_abandon_de_grimper_suit_le_facteur_de_vitesse() {
    // **Le test de la vague de correction finale, point 3.** Sans la
    // division par le facteur, une escalade à vitesse réduite expirerait
    // toujours à 120 s pile — le même délai qu'à vitesse normale, alors
    // qu'elle avance plus lentement. La marge doit rester la MÊME
    // proportion (~15 %) quel que soit le réglage.
    let a_vitesse = |facteur: f32| -> f32 {
        let config = crate::config::Config {
            vitesse: facteur,
            ..crate::config::Config::default()
        };
        let r = crate::config::Reglages::depuis(&config);
        delai_abandon(Intention::Grimper, &r).as_secs_f32()
    };

    assert_eq!(a_vitesse(1.0), 120.0);
    // Deux fois plus lent, deux fois plus de délai — sinon toute
    // escalade à ×0.5 expirerait aux deux tiers du mur (le bug décrit
    // dans le commentaire de `delai_abandon`).
    assert_eq!(a_vitesse(0.5), 240.0);
    // Le minimum autorisé par `FACTEUR_VITESSE_MIN` : le cas le plus
    // extrême que la config puisse produire.
    assert_eq!(a_vitesse(0.1), 1200.0);
}

#[test]
fn expire_au_plafond_il_tombe_au_lieu_de_marcher_dessus() {
    // **Le test du bug trouvé à la Tâche 7**, par l'invariant du monde
    // vertical de `sim.rs`. Sans `lacher_si_accroche` appelée au point
    // précis où le délai d'abandon efface l'intention `Grimper`, le
    // personnage restait accroché au plafond (`Attachment::On { face:
    // Bottom, .. }`), intention `None` — et la couche 3, juste après,
    // lui repostait aussitôt un `Grimper` neuf (phase `Choisir`), qui le
    // faisait « marcher » (pose `walk`) le long de la face `Bottom` :
    // exactement le bug que l'invariant est fait pour attraper.
    //
    // **Plusieurs images, pas une seule** — la leçon de la Tâche 5, où
    // deux bugs ont survécu trois tâches parce que les tests
    // n'appelaient `poursuivre` qu'une fois. On continue d'appeler
    // `poursuivre` après l'expiration pour vérifier que la chute
    // s'installe VRAIMENT et ne se rattrape pas toute seule à l'image
    // suivante.
    let m = monde_mure();
    let plafond = m
        .platforms()
        .iter()
        .find(|p| p.has_face(Face::Bottom))
        .expect("plafond");

    let mut ch = perso_sur_le_sol(&m);
    ch.attachment = Attachment::On {
        platform: plafond.id,
        face: Face::Bottom,
        offset: 200.0,
    };
    // `depuis: ZERO`, et l'horloge du test démarre à 121 s : l'intention
    // est donc déjà périmée dès la première image — exactement ce qui
    // arrive à une escalade qui a trop traîné sur le plafond, sans avoir
    // à dérouler 120 s de traversée pour y arriver.
    ch.intention = Some(ActiveIntention {
        kind: Intention::Grimper,
        depuis: Duration::ZERO,
        etat: EtatIntention::Grimpe {
            phase: PhaseGrimpe::Plafond { cible: 900.0 },
            jusqu_a: Duration::ZERO,
        },
    });

    let mut rng = XorShift32::seeded(5);
    let reglages = reglages();
    let mut t = Duration::from_secs(121);

    for _ in 0..10 {
        poursuivre(&mut ch, &m, &entrees_neutres(), &reglages, t, DT, &mut rng);
        t += Duration::from_secs_f32(DT);

        // À CHAQUE image de ces dix-là, jamais la pose de marche — le
        // symptôme exact du bug.
        assert_ne!(
            ch.pose, POSE_WALK,
            "il ne doit jamais « marcher » sur le plafond"
        );
    }

    assert!(
        matches!(ch.attachment, Attachment::Falling { .. }),
        "il devrait être tombé du plafond à l'expiration, il est {:?}",
        ch.attachment
    );
}

#[test]
fn une_escalade_complete_tient_dans_le_delai_d_abandon() {
    // Le calcul du design §4.4, vérifié plutôt que supposé.
    //
    // ⚠️ **Le pire cas au sol est l'écran ENTIER (1920 px), pas sa
    // moitié** (correction de la vague de relecture finale) : sur deux
    // écrans côte à côte, chaque écran n'a qu'UN SEUL mur (design §2.3),
    // et `mur_le_plus_proche` filtre par `meme_ecran` — donc rien ne
    // borne la distance à la moitié d'un écran. Une version antérieure
    // de ce test prenait 960 px et concluait à une marge de 30 % ; le
    // vrai pire cas, 1920 px, ne laisse que 15 %.
    //
    // Et on le vérifie à PLUSIEURS facteurs de vitesse, dont le minimum
    // autorisé (0.1) : `delai_abandon` divise maintenant le délai par ce
    // même facteur (point 3 de la relecture), donc la marge doit rester
    // la même proportion quel que soit le réglage — c'est ce test-ci qui
    // le démontre, plutôt que de ne vérifier que le facteur ×1 comme
    // avant.
    for facteur in [1.0f32, 0.5, 0.1] {
        let config = crate::config::Config {
            vitesse: facteur,
            ..crate::config::Config::default()
        };
        let r = crate::config::Reglages::depuis(&config);

        let marche = 1920.0 / r.vitesse_marche;
        let montee = 1032.0 / r.vitesse_escalade;
        let delai = delai_abandon(Intention::Grimper, &r).as_secs_f32();

        assert!(
            marche + montee < delai,
            "à vitesse ×{facteur}, une escalade complète dure {}s, au-dessus \
             du délai de {delai}s",
            marche + montee
        );
    }
}

#[test]
fn en_fin_d_accroche_il_lache_parfois_et_redescend_parfois() {
    // Décision n° 3 appliquée à la sortie de mur : les deux issues
    // doivent réellement sortir. Une seule graine, l'état qui avance —
    // re-semer par petits entiers biaiserait le premier tirage et
    // rendrait un faux négatif complet (piège documenté de `CLAUDE.md`).
    let m = monde_mure();
    let mut rng = XorShift32::seeded(12345);
    let reglages = reglages();

    let mut laches = 0;
    let mut descentes = 0;

    for _ in 0..200 {
        let mur = m
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Right))
            .expect("mur gauche");
        let mut ch = perso_sur_le_sol(&m);
        ch.attachment = Attachment::On {
            platform: mur.id,
            face: Face::Right,
            offset: 300.0,
        };
        // Une accroche déjà expirée : la prochaine image tire la sortie.
        //
        // ⚠️ **`jusqu_a` ne peut PAS valoir `Duration::ZERO` ici, et ce
        // n'est pas un détail.** `ZERO` ne veut PAS dire « déjà
        // expirée » mais « durée pas encore tirée » — c'est la
        // convention d'`ActiveIntention::nouvelle` et de `reveil`, et le
        // bras `PhaseGrimpe::Accroche` de `grimper()` l'applique
        // maintenant (correction du bug relevé en relecture de la
        // Tâche 5 : sans cette lecture, la toute première image
        // sautait le tirage lâcher/redescendre). Un test qui veut une
        // accroche VRAIMENT expirée doit donc donner un `jusqu_a` non
        // nul et déjà dépassé — ici 1 ms, largement avant le
        // `maintenant` d'une seconde de l'appel ci-dessous.
        ch.intention = Some(ActiveIntention {
            kind: Intention::Grimper,
            depuis: Duration::ZERO,
            etat: EtatIntention::Grimpe {
                phase: PhaseGrimpe::Accroche,
                jusqu_a: Duration::from_millis(1),
            },
        });

        poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages,
            Duration::from_secs(1),
            DT,
            &mut rng,
        );

        match ch.attachment {
            Attachment::Falling { .. } => laches += 1,
            Attachment::On { .. } => descentes += 1,
            _ => {}
        }
    }

    assert!(laches > 10, "il ne se lâche jamais ({laches} sur 200)");
    assert!(descentes > 10, "il ne redescend jamais ({descentes} sur 200)");
}

// ── Le plafond (Tâche 6, étape 4a) ──────────────────────────────────

/// Le manifeste local (`manifeste()`), privé de certaines poses.
///
/// Sert à éprouver la couverture partielle (spec §8.6) sans dépendre du
/// disque : `desire.rs` a sa propre `manifeste_avec`, qui liste les poses
/// à AJOUTER — ici on part du manifeste complet des tests d'escalade
/// (avec `grabWall`/`climbWall` déjà déclarées) et on RETIRE celles
/// données, ce qui est plus court pour un test qui ne veut retirer que
/// les deux poses de plafond.
///
/// `poses` est un champ public de `Manifest` (une `BTreeMap`) : pas
/// besoin de repasser par le JSON pour le modifier.
fn manifeste_sans(poses: &[&str]) -> Manifest {
    let mut m = manifeste();
    for p in poses {
        m.poses.remove(*p);
    }
    m
}

#[test]
fn arrive_en_haut_du_mur_il_peut_basculer_au_plafond() {
    let m = monde_mure();
    let mur = m
        .platforms()
        .iter()
        .find(|p| p.has_face(Face::Right))
        .expect("mur gauche");

    // Posé en haut du mur, accroche expirée : la prochaine image tire la
    // sortie, et le plafond doit en faire partie.
    let mut rng = XorShift32::seeded(999);
    let reglages = reglages();

    let mut vus_au_plafond = 0;
    for _ in 0..200 {
        let mut ch = perso_sur_le_sol(&m);
        ch.attachment = Attachment::On {
            platform: mur.id,
            face: Face::Right,
            offset: 0.0, // tout en haut
        };
        // ⚠️ `jusqu_a` NE PEUT PAS valoir `Duration::ZERO` ici : depuis la
        // Tâche 5, `ZERO` signifie « durée pas encore tirée » (init.
        // paresseuse du bras `Accroche`), jamais « déjà expirée ». Avec
        // `ZERO`, cette toute première image se contenterait de tirer la
        // durée d'accroche et rendrait `EnCours` sans jamais tenter la
        // sortie — le personnage ne basculerait alors JAMAIS, et ce test
        // mesurerait un faux négatif complet plutôt qu'une vraie absence
        // de bascule. Un `jusqu_a` non nul et déjà dépassé (1 ms, contre
        // un `maintenant` d'une seconde) est la façon correcte d'écrire
        // « l'accroche est terminée » — le même correctif que celui déjà
        // appliqué au test `en_fin_d_accroche_il_lache_parfois…` plus
        // haut dans ce fichier.
        ch.intention = Some(ActiveIntention {
            kind: Intention::Grimper,
            depuis: Duration::ZERO,
            etat: EtatIntention::Grimpe {
                phase: PhaseGrimpe::Accroche,
                jusqu_a: Duration::from_millis(1),
            },
        });

        poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages,
            Duration::from_secs(1),
            DT,
            &mut rng,
        );

        if matches!(ch.attachment, Attachment::On { face: Face::Bottom, .. }) {
            vus_au_plafond += 1;
        }
    }

    assert!(
        vus_au_plafond > 10,
        "il ne passe jamais au plafond ({vus_au_plafond} sur 200)"
    );
}

#[test]
fn au_plafond_il_traverse_avec_la_bonne_pose() {
    let m = monde_mure();
    let plafond = m
        .platforms()
        .iter()
        .find(|p| p.has_face(Face::Bottom))
        .expect("plafond");

    let mut ch = perso_sur_le_sol(&m);
    ch.attachment = Attachment::On {
        platform: plafond.id,
        face: Face::Bottom,
        offset: 200.0,
    };
    ch.intention = Some(ActiveIntention {
        kind: Intention::Grimper,
        depuis: Duration::ZERO,
        etat: EtatIntention::Grimpe {
            phase: PhaseGrimpe::Plafond { cible: 800.0 },
            // Sans effet ici : la phase `Plafond` ne LIT `jusqu_a` que
            // pour la reporter à l'identique, elle ne teste jamais son
            // expiration (contrairement à `Accroche`). `ZERO` reste
            // correct — « pas encore tirée » n'a simplement aucune
            // incidence tant qu'on n'a pas atteint la cible.
            jusqu_a: Duration::ZERO,
        },
    });

    let mut rng = XorShift32::seeded(5);
    // ⚠️ **Correction de relecture** : la condition d'arrêt initiale
    // était `c.pose == POSE_CLIMB_CEILING`, or cette pose est posée
    // **dès la première image**, en tête du bras `Plafond` — avant même
    // de calculer le déplacement. `derouler` en sortait donc au tout
    // premier tick, quel que soit le budget de 10 s passé : le test
    // prouvait qu'un instant existait, pas qu'une traversée avait lieu.
    // La condition porte maintenant sur le DÉPLACEMENT réel (au moins
    // 100 px parcourus vers la cible), qui ne peut se satisfaire qu'en
    // ayant vraiment avancé plusieurs images — même défaut, et même
    // correctif, que celui relevé sur le test du mur à la Tâche 5.
    derouler(&mut ch, &m, &mut rng, 10.0, |c| {
        (offset_de(c) - 200.0).abs() >= 100.0
    });

    assert_eq!(ch.pose, POSE_CLIMB_CEILING);

    match ch.attachment {
        Attachment::On {
            face: Face::Bottom,
            offset,
            ..
        } => {
            assert!(offset > 200.0, "il doit avoir avancé vers sa cible");
        }
        autre => panic!("il devrait être au plafond, il est {autre:?}"),
    }
}

#[test]
fn le_plafond_d_un_ecran_prolonge_celui_du_voisin() {
    // La généralisation de `face_voisine` aux faces `Bottom` : au bout du
    // plafond de A, il passe sur celui de B, comme il le fait déjà au sol.
    let m = World::from_screens(&FakeProbe::deux_ecrans().screens());
    let plafond_a = m
        .platforms()
        .iter()
        .find(|p| p.has_face(Face::Bottom) && p.rect.left() == 0.0)
        .expect("plafond de gauche");

    let voisin = face_voisine(&m, plafond_a.id, Face::Bottom, true);
    assert!(voisin.is_some(), "le plafond de droite doit être trouvé");
    let (id, offset) = voisin.unwrap();
    assert_ne!(id, plafond_a.id);
    assert_eq!(offset, 0.0, "on y entre par son bord gauche");
}

#[test]
fn au_bout_du_plafond_il_passe_au_plafond_du_voisin() {
    // Constat de relecture (Tâche 6) : le test ci-dessus n'appelle
    // `face_voisine` qu'À VIDE, en dehors de toute simulation. Le bloc
    // qui s'en sert réellement dans la phase `Plafond` (`if nouveau <
    // 0.0 || nouveau > longueur`) n'était donc jamais parcouru en
    // conditions réelles — exactement le défaut de la Tâche 5, où deux
    // bugs ont survécu à trois tâches parce que personne ne déroulait
    // la boucle. C'est le pendant horizontal de `grimper_monte_vraiment`
    // (qui, lui, prouve la même chose à la verticale, sur un mur).
    let m = World::from_screens(&FakeProbe::deux_ecrans().screens());
    let plafond_a = m
        .platforms()
        .iter()
        .find(|p| p.has_face(Face::Bottom) && p.rect.left() == 0.0)
        .expect("plafond de gauche");
    let id_depart = plafond_a.id;

    let mut ch = perso_sur_le_sol(&m);
    ch.attachment = Attachment::On {
        platform: plafond_a.id,
        face: Face::Bottom,
        // À 120 px du bord droit (le plafond de gauche fait 1920 px).
        offset: 1800.0,
    };
    ch.intention = Some(ActiveIntention {
        kind: Intention::Grimper,
        depuis: Duration::ZERO,
        etat: EtatIntention::Grimpe {
            // Une cible très au-delà du bord : il ne l'atteindra jamais
            // SUR ce plafond, il devra d'abord en sortir. C'est ce qui
            // force le passage par le bloc de franchissement plutôt que
            // par la sortie normale « cible atteinte ».
            phase: PhaseGrimpe::Plafond { cible: 5_000.0 },
            jusqu_a: Duration::ZERO,
        },
    });

    let mut rng = XorShift32::seeded(9);
    // 120 px à 16,1 px/s ≈ 7,5 s : 15 s de marge est largement
    // suffisant, et reste sous le délai d'abandon de 120 s.
    derouler(&mut ch, &m, &mut rng, 15.0, |c| plateforme_de(c) != id_depart);

    match ch.attachment {
        Attachment::On {
            platform,
            face: Face::Bottom,
            offset,
            ..
        } => {
            assert_ne!(platform, id_depart, "il devrait avoir changé de plafond");
            assert_eq!(
                m.get(platform).unwrap().rect.left(),
                1920.0,
                "il doit être sur le plafond de l'écran voisin"
            );
            assert_eq!(offset, 0.0, "il entre par le bord gauche du plafond voisin");
        }
        autre => panic!("il devrait être au plafond voisin, il est {autre:?}"),
    }

    // La pose du plafond a été posée en tête du bras `Plafond`, AVANT le
    // calcul qui déclenche le changement de plateforme — donc dès cette
    // image-ci, jamais une pose de sol ou de mur.
    assert_eq!(ch.pose, POSE_CLIMB_CEILING);
}

#[test]
fn en_fin_d_accroche_au_plafond_il_lache_parfois_et_repart_parfois() {
    // Constat de relecture (Tâche 6) : le chemin ajouté d'initiative
    // propre — depuis le plafond (`face == Face::Bottom`), un tirage
    // « redescendre » relance une traversée `Plafond` plutôt que de
    // retomber dans `Paroi` (qui poserait `climbWall` et raisonnerait
    // sur un axe vertical, tous deux faux au plafond) — n'était exercé
    // par AUCUN test. Même structure que
    // `en_fin_d_accroche_il_lache_parfois_et_redescend_parfois`, au mur.
    let m = monde_mure();
    let plafond = m
        .platforms()
        .iter()
        .find(|p| p.has_face(Face::Bottom))
        .expect("plafond");
    let longueur = plafond.rect.face_length(Face::Bottom);

    // Une seule graine, l'état qui avance : re-semer par petits entiers
    // séquentiels biaiserait le premier tirage (piège documenté de
    // `CLAUDE.md`).
    let mut rng = XorShift32::seeded(54321);
    let reglages = reglages();

    let mut laches = 0;
    let mut repartitions = 0;

    for _ in 0..200 {
        let mut ch = perso_sur_le_sol(&m);
        ch.attachment = Attachment::On {
            platform: plafond.id,
            face: Face::Bottom,
            offset: 400.0,
        };
        // Accroche déjà expirée : `Duration::ZERO` voudrait dire « pas
        // encore tirée », pas « expirée » (même piège que sur le test
        // équivalent au mur).
        ch.intention = Some(ActiveIntention {
            kind: Intention::Grimper,
            depuis: Duration::ZERO,
            etat: EtatIntention::Grimpe {
                phase: PhaseGrimpe::Accroche,
                jusqu_a: Duration::from_millis(1),
            },
        });

        poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages,
            Duration::from_secs(1),
            DT,
            &mut rng,
        );

        match ch.attachment {
            Attachment::Falling { .. } => laches += 1,
            Attachment::On {
                face: Face::Bottom, ..
            } => {
                repartitions += 1;

                // Il doit avoir repris une traversée du plafond, avec
                // une cible dans les bornes de la face — pas de
                // `Paroi`, et pas de cible qui déborderait.
                match ch.intention {
                    Some(ActiveIntention {
                        etat:
                            EtatIntention::Grimpe {
                                phase: PhaseGrimpe::Plafond { cible },
                                ..
                            },
                        ..
                    }) => {
                        assert!(
                            (0.0..=longueur).contains(&cible),
                            "cible {cible} hors des bornes [0, {longueur}]"
                        );
                    }
                    autre => panic!(
                        "attendu une nouvelle traversée du plafond (Plafond), obtenu {autre:?}"
                    ),
                }
            }
            autre => panic!("attendu Falling ou On(Bottom), obtenu {autre:?}"),
        }
    }

    assert!(laches > 10, "il ne se lâche jamais du plafond ({laches} sur 200)");
    assert!(
        repartitions > 10,
        "il ne repart jamais en traversée ({repartitions} sur 200)"
    );
}

#[test]
fn un_pack_sans_pose_de_plafond_grimpe_quand_meme() {
    // Couverture partielle (spec §8.6) : `climbCeiling` n'est PAS dans
    // les poses requises de `Grimper`. Un pack qui ne l'a pas doit
    // pouvoir grimper au mur, et simplement ne jamais passer au plafond.
    let table = crate::behavior::desire::TableEnvies::defaut();
    let sans_plafond = manifeste_sans(&[POSE_CLIMB_CEILING, POSE_GRAB_CEILING]);
    assert!(table.jouable(&sans_plafond, Intention::Grimper));
}

// ── Sur ordre, il court jusqu'au mur ────────────────────────────────────

/// L'offset parcouru par `Rejoindre` pendant une seconde, et la pose
/// observée à la fin — ce que les deux tests suivants comparent.
///
/// On attend d'abord que la phase `Choisir` soit passée (une image), puis
/// on mesure sur 60 images : assez pour que l'écart marche/course soit
/// net, trop peu pour atteindre le mur depuis l'offset 500.
fn une_seconde_de_rejoindre(intention: ActiveIntention) -> (f32, String) {
    let m = monde_mure();
    let mut ch = perso_sur_le_sol(&m);
    ch.intention = Some(intention);
    let mut rng = XorShift32::seeded(7);
    let reglages = reglages();

    // Première image : `Choisir` tranche le mur, sans bouger.
    poursuivre(&mut ch, &m, &entrees_neutres(), &reglages, Duration::ZERO, DT, &mut rng);
    let depart = offset_de(&ch);

    let mut t = Duration::ZERO;
    for _ in 0..60 {
        t += Duration::from_secs_f32(DT);
        poursuivre(&mut ch, &m, &entrees_neutres(), &reglages, t, DT, &mut rng);
    }
    ((offset_de(&ch) - depart).abs(), ch.pose.clone())
}

#[test]
fn sur_ordre_il_court_jusqu_au_mur() {
    let (parcouru, pose) =
        une_seconde_de_rejoindre(ActiveIntention::grimper_sur_ordre(Duration::ZERO));
    let r = reglages();

    assert_eq!(pose, POSE_RUN, "sur ordre, il doit courir vers le mur");
    // À une image près : la mesure compte 60 pas de `vitesse_course * DT`.
    assert!(
        (parcouru - r.vitesse_course).abs() < r.vitesse_course * DT * 2.0,
        "parcouru {parcouru:.1} px en 1 s, attendu ~{:.1}",
        r.vitesse_course
    );
}

#[test]
fn quand_il_decide_seul_il_marche_jusqu_au_mur() {
    // Le tirage aléatoire garde la marche : l'ordre a l'air obéi, la
    // flânerie reste calme (décision de l'auteur, 2026-09-23).
    let (parcouru, pose) =
        une_seconde_de_rejoindre(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));
    let r = reglages();

    assert_eq!(pose, POSE_WALK);
    assert!(
        (parcouru - r.vitesse_marche).abs() < r.vitesse_marche * DT * 2.0,
        "parcouru {parcouru:.1} px en 1 s, attendu ~{:.1}",
        r.vitesse_marche
    );
}

#[test]
fn sur_ordre_sans_pose_run_il_marche_a_la_vitesse_de_marche() {
    // Couverture partielle (spec §8.6) : courir en pose de marche aurait
    // l'air d'un glissement. Sans `run`, l'ordre est obéi à pied.
    let m = monde_mure();
    let mut ch = perso_sur_le_sol(&m);
    ch.manifest = manifeste_sans(&[POSE_RUN]);
    ch.intention = Some(ActiveIntention::grimper_sur_ordre(Duration::ZERO));
    let mut rng = XorShift32::seeded(7);
    let reglages = reglages();

    poursuivre(&mut ch, &m, &entrees_neutres(), &reglages, Duration::ZERO, DT, &mut rng);
    let depart = offset_de(&ch);
    let mut t = Duration::ZERO;
    for _ in 0..60 {
        t += Duration::from_secs_f32(DT);
        poursuivre(&mut ch, &m, &entrees_neutres(), &reglages, t, DT, &mut rng);
    }

    assert_eq!(ch.pose, POSE_WALK);
    let parcouru = (offset_de(&ch) - depart).abs();
    assert!((parcouru - reglages.vitesse_marche).abs() < reglages.vitesse_marche * DT * 2.0);
}

// ── Étalé au sol après une chute ────────────────────────────────────────

#[test]
fn etale_il_reste_au_sol_puis_se_releve() {
    // La demande de l'auteur (2026-09-23) : après une chute, il ne repart
    // pas aussitôt. Il reste étalé 1 à 2 s (`dureeAuSol`), puis se relève
    // par l'animation de réveil, et seulement alors l'intention finit.
    let m = monde();
    let mut ch = perso(&m, 500.0);
    let mut rng = XorShift32::seeded(3);
    let e = entrees_neutres();
    let r = reglages();
    ch.intention = Some(ActiveIntention::etale(Duration::ZERO));

    let mut releve_a = None;
    let mut fini_a = None;
    for i in 0..(10 * 60) {
        let t = Duration::from_secs_f32(i as f32 * DT);
        let issue = poursuivre(&mut ch, &m, &e, &r, t, DT, &mut rng);
        if issue != Issue::EnCours {
            assert_eq!(issue, Issue::Finie, "se relever ne doit pas échouer");
            fini_a = Some(t);
            break;
        }
        if releve_a.is_none() {
            if ch.pose == POSE_WAKE {
                releve_a = Some(t);
            } else {
                assert_eq!(ch.pose, POSE_SPRAWL, "avant de se relever, il est étalé");
            }
        }
        // Il ne bouge pas d'un pixel tant qu'il est au sol.
        assert_eq!(offset_de(&ch), 500.0);
    }

    let releve_a = releve_a.expect("il ne s'est jamais relevé");
    let fini_a = fini_a.expect("il est resté au sol pour toujours");
    let [min, max] = r.duree_au_sol;
    assert!(releve_a >= Duration::from_secs_f32(min), "relevé trop tôt : {releve_a:?}");
    assert!(releve_a <= Duration::from_secs_f32(max) + ANIM_REVEIL, "trop tard : {releve_a:?}");
    assert!(fini_a.saturating_sub(releve_a) + Duration::from_secs_f32(DT) >= ANIM_REVEIL);
}

#[test]
fn etale_n_exige_pas_la_pose_sit() {
    // Même exemption que `Selevant` : tomber n'est pas choisir de se
    // reposer, un pack sans `sit` doit pouvoir rester étalé.
    let m = monde();
    let mut ch = perso(&m, 500.0);
    ch.manifest = manifeste_sans(&[POSE_SIT]);
    ch.intention = Some(ActiveIntention::etale(Duration::ZERO));
    let mut rng = XorShift32::seeded(3);

    let issue = poursuivre(&mut ch, &m, &entrees_neutres(), &reglages(), Duration::ZERO, DT, &mut rng);
    assert_eq!(issue, Issue::EnCours);
    assert_eq!(ch.pose, POSE_SPRAWL);
}
