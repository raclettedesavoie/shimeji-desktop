//! Les tests de `manifest` — sortis du fichier source le 2026-09-14.
//!
//! **Pourquoi ce fichier existe** : `character/manifest.rs` faisait 693 lignes dont 259 de tests,
//! soit 37 %. Toute lecture du module en payait le double, pour rien la plupart
//! du temps. Les tests n'ont pas changé d'une ligne : ils ont seulement déménagé, et
//! se désindentent d'un cran puisqu'ils ne sont plus enfermés dans un `mod tests { }`.

use super::*;
use std::time::Duration;

static COMPTEUR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Écrit un mascot.json et les PNG demandés dans un dossier temporaire,
/// et rend son chemin.
///
/// Les PNG sont des fichiers **vides** : `load` ne vérifie que leur
/// EXISTENCE, pas leur contenu. Décoder l'image serait le travail du
/// webview, et l'exiger ici rendrait le test lent pour rien.
fn dossier_de_test(json: &str, frames_presentes: &[u32]) -> std::path::PathBuf {
    // `std::env::temp_dir()` plus un nom unique : pas de dépendance à une
    // crate de fichiers temporaires pour douze tests.
    //
    // Le nom était shimeji-test-<pid>-<compteur>, mais Windows RECYCLE les
    // PID. Conséquence : une exécution qui retombe sur un PID déjà utilisé
    // réutilisait le dossier laissé par une exécution PRÉCÉDENTE. Le
    // compteur n'y changeait rien : il était distribué dans un ordre NON
    // DÉTERMINISTE entre les threads de test parallèles, donc plusieurs
    // tests sur la même exécution partageaient le même dossier. Tous les
    // tests d'une exécution N héritaient de la poubelle de la PREMIÈRE
    // exécution qui avait eu ce PID.
    //
    // Symptôme : un_manifeste_sans_aucune_pose_jouable_est_une_erreur
    // échouait environ une fois sur quatre — assez rare pour être attribué
    // au test lui-même plutôt qu'à son helper, ce qui aurait envoyé
    // chercher un bug inexistant.
    //
    // La morale : partager un dossier, c'est se partager aussi SES FICHIERS.
    // Si le test A crée shime1.png et ferme le dossier, le test B qui le
    // réutilise y trouve shime1.png dont il attend précisément l'ABSENCE.
    //
    // Deux corrections :
    // 1. Ajouter un grain UNIQUE par exécution (nanosecondes depuis
    //    l'époque). Un PID recyclé ne peut plus collisionner sur le même
    //    nom de dossier.
    // 2. SUPPRIMER le dossier s'il existe avant de le créer, comme défense
    //    en profondeur. Cette ligne garantit qu'un test PART TOUJOURS d'un
    //    dossier vide, même si le grain échouait un jour.
    //
    // La distinction : laisser des dossiers dans %TEMP% est acceptable
    // (c'est un artefact), les RÉUTILISER ne l'est pas (c'est la source du
    // bug). Le nettoyage à la sortie du test n'aurait rien changé — une
    // autre exécution aurait pu écrire dans le dossier entre sa création et
    // son nettoyage. C'est la SÉPARATION (un nom unique) qui répare le bug.
    let nanos_unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let base = std::env::temp_dir().join(format!(
        "shimeji-test-{}-{}-{}",
        std::process::id(),
        COMPTEUR.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        nanos_unique
    ));

    // Défense en profondeur : si le dossier existait, le supprimer.
    // L'échec est sans importance (le dossier n'existait pas).
    let _ = std::fs::remove_dir_all(&base);

    std::fs::create_dir_all(base.join("img")).unwrap();
    std::fs::write(base.join("mascot.json"), json).unwrap();
    for n in frames_presentes {
        std::fs::write(base.join("img").join(format!("shime{n}.png")), b"").unwrap();
    }
    base
}

const MINIMAL: &str = r#"{
    "id": "t",
    "name": "Test",
    "frameSize": [128, 128],
    "scale": 1,
    "hitbox": [40, 20, 48, 100],
    "poses": {
        "stand": { "frames": [1] },
        "walk":  { "frames": [2, 3], "frameMs": 120, "loop": true }
    }
}"#;

#[test]
fn charge_un_manifeste_correct() {
    let d = dossier_de_test(MINIMAL, &[1, 2, 3]);
    let m = Manifest::load(&d).expect("manifeste valide");

    assert_eq!(m.id, "t");
    assert_eq!(m.frame_size, [128, 128]);
    assert!(m.has_pose(POSE_STAND));
    assert!(m.has_pose(POSE_WALK));
}

#[test]
fn les_defauts_s_appliquent_aux_champs_absents() {
    let d = dossier_de_test(MINIMAL, &[1, 2, 3]);
    let m = Manifest::load(&d).unwrap();

    let stand = m.pose(POSE_STAND).unwrap();
    // `frameMs` absent → 150 (spec §8.5)
    assert_eq!(stand.frame_ms, 150);
    // `loop` absent → false
    assert!(!stand.looping);
    // `anchor` absent → [64, 128], le sol tout en bas de la boîte
    assert_eq!(stand.anchor, [64.0, 128.0]);
    // `hitbox` de pose absente → celle du manifeste
    assert_eq!(m.hitbox_de(POSE_STAND), m.hitbox);
}

#[test]
fn une_pose_dont_les_images_manquent_est_retiree_sans_erreur() {
    // LE test de la couverture partielle (spec §8.6). `walk` demande les
    // frames 2 et 3 ; on ne fournit que la 1. Le chargement doit
    // RÉUSSIR, et `walk` doit avoir disparu.
    let d = dossier_de_test(MINIMAL, &[1]);
    let m = Manifest::load(&d).expect("le chargement doit réussir malgré tout");

    assert!(m.has_pose(POSE_STAND));
    assert!(!m.has_pose(POSE_WALK), "walk devait être retirée");
}

#[test]
fn une_pose_partiellement_couverte_est_retiree_entierement() {
    // La frame 2 est là, la 3 non. On ne joue pas une animation trouée :
    // c'est tout ou rien par pose.
    let d = dossier_de_test(MINIMAL, &[1, 2]);
    let m = Manifest::load(&d).unwrap();
    assert!(!m.has_pose(POSE_WALK));
}

#[test]
fn un_manifeste_sans_aucune_pose_jouable_est_une_erreur() {
    // La limite de la tolérance : un personnage dont AUCUNE pose n'a
    // d'image n'est pas un personnage à couverture partielle, c'est un
    // dossier vide. Là, il faut le dire.
    let d = dossier_de_test(MINIMAL, &[]);
    match Manifest::load(&d) {
        Err(ManifestError::AucunePoseJouable) => {}
        autre => panic!("attendu AucunePoseJouable, obtenu {autre:?}"),
    }
}

#[test]
fn un_manifeste_avec_bom_se_charge() {
    // Même piège que pour `config.json`, et plus probable encore : le
    // manifeste est fait pour être édité à la main.
    let avec_bom = format!("{}{}", '\u{feff}', MINIMAL);
    let d = dossier_de_test(&avec_bom, &[1, 2, 3]);
    let m = Manifest::load(&d).expect("un BOM ne doit pas empêcher le chargement");
    assert_eq!(m.id, "t");
}

#[test]
fn un_json_malforme_donne_une_erreur_explicite_sans_paniquer() {
    // Spec §10.1 : « Manifeste malformé → erreur explicite, aucune
    // panique. »
    let d = dossier_de_test("{ ceci n'est pas du JSON", &[1]);
    match Manifest::load(&d) {
        Err(ManifestError::JsonInvalide(msg)) => {
            assert!(!msg.is_empty(), "le message doit dire où ça casse");
        }
        autre => panic!("attendu JsonInvalide, obtenu {autre:?}"),
    }
}

#[test]
fn un_mascot_json_absent_donne_une_erreur_explicite() {
    let d = std::env::temp_dir().join("shimeji-test-dossier-inexistant-xyz");
    match Manifest::load(&d) {
        Err(ManifestError::FichierIllisible { .. }) => {}
        autre => panic!("attendu FichierIllisible, obtenu {autre:?}"),
    }
}

#[test]
fn une_pose_sans_frame_est_rejetee_a_la_validation() {
    let json = r#"{
        "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
        "hitbox": [40,20,48,100],
        "poses": { "stand": { "frames": [1] }, "vide": { "frames": [] } }
    }"#;
    let d = dossier_de_test(json, &[1]);
    let m = Manifest::load(&d).unwrap();
    // Une pose à zéro frame ne peut rien afficher : retirée comme une
    // pose sans images, pour la même raison.
    assert!(!m.has_pose("vide"));
    assert!(m.has_pose(POSE_STAND));
}

#[test]
fn frame_a_avance_dans_une_animation_en_boucle() {
    let d = dossier_de_test(MINIMAL, &[1, 2, 3]);
    let m = Manifest::load(&d).unwrap();
    let walk = m.pose(POSE_WALK).unwrap();

    // frames [2, 3], 120 ms chacune, en boucle
    assert_eq!(walk.frame_a(Duration::from_millis(0)), 2);
    assert_eq!(walk.frame_a(Duration::from_millis(119)), 2);
    assert_eq!(walk.frame_a(Duration::from_millis(120)), 3);
    // 240 ms : retour au début
    assert_eq!(walk.frame_a(Duration::from_millis(240)), 2);
    assert_eq!(walk.frame_a(Duration::from_millis(361)), 3);
}

#[test]
fn frame_a_tient_la_derniere_image_quand_loop_est_faux() {
    let json = r#"{
        "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
        "hitbox": [40,20,48,100],
        "poses": { "seq": { "frames": [1, 2, 3], "frameMs": 100 } }
    }"#;
    let d = dossier_de_test(json, &[1, 2, 3]);
    let m = Manifest::load(&d).unwrap();
    let seq = m.pose("seq").unwrap();

    assert_eq!(seq.frame_a(Duration::from_millis(0)), 1);
    assert_eq!(seq.frame_a(Duration::from_millis(250)), 3);
    // Au-delà de la séquence : on TIENT la dernière, on ne reboucle pas.
    assert_eq!(seq.frame_a(Duration::from_secs(10)), 3);
}

#[test]
fn hold_choisit_l_image_tenue_a_la_fin() {
    // C'est ce qui permet de RESTER endormi après la séquence
    // d'endormissement (spec §8.5). Ici, la séquence finit sur 3 mais
    // c'est 2 qui doit être tenue.
    let json = r#"{
        "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
        "hitbox": [40,20,48,100],
        "poses": { "seq": { "frames": [1, 2, 3], "frameMs": 100, "hold": 2 } }
    }"#;
    let d = dossier_de_test(json, &[1, 2, 3]);
    let m = Manifest::load(&d).unwrap();
    let seq = m.pose("seq").unwrap();

    assert_eq!(seq.frame_a(Duration::from_millis(250)), 3);
    assert_eq!(seq.frame_a(Duration::from_secs(10)), 2);
}

#[test]
fn le_manifeste_reel_de_blob_se_charge() {
    // Le seul test qui touche au dépôt : il vérifie que le mascot.json
    // est cohérent avec les 46 PNG réellement présents.
    // Un chemin relatif depuis src-tauri/, où cargo exécute les tests.
    let d = std::path::Path::new("../characters/blob");
    let m = Manifest::load(d).expect("blob doit se charger");

    assert_eq!(m.id, "blob");
    // blob possède TOUTES les poses (spec §8.7) : aucune ne doit avoir
    // été retirée. Si ce test échoue, c'est qu'un numéro de frame du
    // mascot.json ne correspond à aucun fichier.
    for pose in [
        POSE_STAND, POSE_WALK, POSE_RUN, POSE_SIT, POSE_FALL, POSE_LAND, POSE_DRAGGED,
    ] {
        assert!(m.has_pose(pose), "pose manquante : {pose}");
    }
}

/// La table `frames` renseigne taille et ancre par image, et son absence
/// laisse EXACTEMENT le comportement d'avant.
///
/// C'est cette seconde moitié qui compte : `blob` n'a pas de table
/// `frames` et ne doit pas être retouché. Le changement est purement
/// additif côté données, et c'est ce qui le rend sûr.
#[test]
fn la_table_frames_est_facultative_et_se_replie() {
    let avec = r#"{
        "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
        "hitbox": [40,20,48,100],
        "frames": {
            "23": { "size": [96,140], "anchor": [48,4] },
            "24": { "size": [90,138] }
        },
        "poses": { "stand": { "frames": [1], "anchor": [64,128] } }
    }"#;
    let m: Manifest = serde_json::from_str(avec).expect("manifeste valide");

    // Déclarée → on la lit.
    assert_eq!(m.taille_de_frame(23), [96, 140]);
    assert_eq!(m.ancre_de_frame(23, "stand"), [48.0, 4.0]);

    // Taille déclarée, ancre absente → repli sur l'ancre de la POSE.
    assert_eq!(m.taille_de_frame(24), [90, 138]);
    assert_eq!(m.ancre_de_frame(24, "stand"), [64.0, 128.0]);

    // Image absente de la table → repli complet sur le manifeste.
    assert_eq!(m.taille_de_frame(1), [128, 128]);
    assert_eq!(m.ancre_de_frame(1, "stand"), [64.0, 128.0]);

    // Pose inconnue → l'ancre par défaut, jamais un panic : l'appelant
    // est dans la boucle 60 Hz et n'a rien à faire d'un cas d'erreur.
    assert_eq!(m.ancre_de_frame(1, "pose-qui-n-existe-pas"), [64.0, 128.0]);

    // Et sans table du tout : le comportement d'avant, à l'identique.
    let sans = r#"{
        "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
        "hitbox": [40,20,48,100],
        "poses": { "stand": { "frames": [1], "anchor": [64,128] } }
    }"#;
    let m2: Manifest = serde_json::from_str(sans).expect("manifeste valide");
    assert!(m2.frames.is_empty());
    assert_eq!(m2.taille_de_frame(1), [128, 128]);
    assert_eq!(m2.ancre_de_frame(1, "stand"), [64.0, 128.0]);
}

/// ⚠️ **Porté, un personnage pend par le HAUT — quelle que soit l'ancre de
/// l'image.**
///
/// Constaté à l'écran le 2026-09-23 : `blob` pendait sous le curseur, mais
/// tous les packs installés par le catalogue flottaient AU-DESSUS.
///
/// La cause est un aplatissement. Dans Shimeji, une ancre dépend du couple
/// *(action, image)* ; notre table `frames` n'en garde qu'une par NUMÉRO
/// d'image. Or l'image 1 sert à la fois à « debout » (ancre = les pieds) et à
/// « porté » (ancre = le point de pincement). Le catalogue y écrit celle de
/// debout, et comme l'ancre d'image l'emportait sur celle de la pose, le
/// curseur tenait le personnage… par les pieds.
///
/// `blob` y échappait par accident : écrit à la main, il n'a pas de table
/// `frames`.
#[test]
fn une_pose_portee_ignore_l_ancre_de_l_image() {
    // Exactement ce qu'écrit le catalogue : l'image 1 porte l'ancre des
    // pieds, la pose `dragged` déclare le point de pincement.
    let json = r#"{
        "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
        "hitbox": [40,20,48,100],
        "frames": {
            "1": { "size": [128,128], "anchor": [64,128] },
            "5": { "size": [128,128], "anchor": [70,120] }
        },
        "poses": {
            "stand":         { "frames": [1], "anchor": [64,128] },
            "dragged":       { "frames": [1], "anchor": [64,8] },
            "draggedRight1": { "frames": [5], "anchor": [64,8] }
        }
    }"#;
    let m: Manifest = serde_json::from_str(json).expect("manifeste valide");

    // La même image, deux sens : debout, on tient par les pieds…
    assert_eq!(m.ancre_de_frame(1, "stand"), [64.0, 128.0]);
    // … porté, par le haut de la tête.
    assert_eq!(m.ancre_de_frame(1, "dragged"), [64.0, 8.0]);

    // Et pendant le balancement aussi : sans ça, le point tenu par le
    // curseur SAUTERAIT d'une image de balancement à l'autre.
    assert_eq!(m.ancre_de_frame(5, "draggedRight1"), [64.0, 8.0]);
}

#[test]
fn les_poses_portees_sont_reconnues_toutes_les_sept() {
    // La pose au repos, et les six de balancement. En oublier une ferait
    // sauter le personnage sous le curseur à chaque passage par elle.
    assert!(est_pose_portee(POSE_DRAGGED));
    for p in POSES_DRAGGED_LEFT.iter().chain(POSES_DRAGGED_RIGHT.iter()) {
        assert!(est_pose_portee(p), "pose portée non reconnue : {p}");
    }
    // Et rien d'autre : une pose debout doit garder l'ancre de son image.
    assert!(!est_pose_portee(POSE_STAND));
    assert!(!est_pose_portee(POSE_FALL));
}
