//! Les tests de `world` — sortis du fichier source le 2026-09-14.
//!
//! **Pourquoi ce fichier existe** : `world.rs` faisait 606 lignes dont 236 de tests,
//! soit 39 %. Toute lecture du module en payait le double, pour rien la plupart
//! du temps. Les tests n'ont pas changé d'une ligne : ils ont seulement déménagé, et
//! se désindentent d'un cran puisqu'ils ne sont plus enfermés dans un `mod tests { }`.

use super::*;
use crate::geom::{Face, Point};
use crate::probe::fake::FakeProbe;
use crate::probe::{ScreenInfo, SystemProbe, WindowInfo};

/// Compte les plateformes qui exposent une face donnée. Rendue locale au
/// module de test : c'est une question de test, pas une propriété du monde.
fn combien_de_face(monde: &World, face: Face) -> usize {
    monde.platforms().iter().filter(|p| p.has_face(face)).count()
}

#[test]
fn un_ecran_isole_donne_sol_plafond_et_deux_murs() {
    let monde = World::from_screens(&FakeProbe::un_ecran().screens());
    assert_eq!(monde.platforms().len(), 4);
    assert_eq!(combien_de_face(&monde, Face::Top), 1);
    assert_eq!(combien_de_face(&monde, Face::Bottom), 1);
    assert_eq!(combien_de_face(&monde, Face::Right), 1); // le mur GAUCHE
    assert_eq!(combien_de_face(&monde, Face::Left), 1); // le mur DROIT
}

#[test]
fn le_sol_est_toujours_la_premiere_plateforme_de_son_ecran() {
    // Plusieurs tests d'autres modules écrivent `platforms()[0]` en
    // voulant dire « le sol ». Cet ordre est donc un contrat, pas un
    // hasard — d'où ce test qui le fige.
    let monde = World::from_screens(&FakeProbe::un_ecran().screens());
    assert!(monde.platforms()[0].has_face(Face::Top));
    assert_eq!(monde.premier_sol().map(|p| p.id), Some(monde.platforms()[0].id));
}

#[test]
fn le_mur_gauche_expose_sa_face_droite_sur_le_bord_de_la_zone_de_travail() {
    // La vérification la plus importante de la tâche : le POINT rendu
    // par point_on doit tomber exactement sur le bord, pas un pixel à
    // côté — c'est là que le personnage sera dessiné.
    let monde = World::from_screens(&FakeProbe::un_ecran().screens());
    let mur = monde
        .platforms()
        .iter()
        .find(|p| p.has_face(Face::Right))
        .expect("un écran isolé a un mur gauche");

    // FakeProbe::un_ecran : zone de travail (0, 0, 1920, 1032).
    assert_eq!(mur.rect.point_on(Face::Right, 0.0), Point::new(0.0, 0.0));
    assert_eq!(mur.rect.point_on(Face::Right, 100.0), Point::new(0.0, 100.0));
    assert_eq!(mur.rect.face_length(Face::Right), 1032.0);
}

#[test]
fn le_mur_droit_et_le_plafond_tombent_aussi_sur_le_bord() {
    let monde = World::from_screens(&FakeProbe::un_ecran().screens());

    let droit = monde
        .platforms()
        .iter()
        .find(|p| p.has_face(Face::Left))
        .expect("un écran isolé a un mur droit");
    assert_eq!(droit.rect.point_on(Face::Left, 50.0), Point::new(1920.0, 50.0));

    let plafond = monde
        .platforms()
        .iter()
        .find(|p| p.has_face(Face::Bottom))
        .expect("tout écran a un plafond");
    assert_eq!(plafond.rect.point_on(Face::Bottom, 30.0), Point::new(30.0, 0.0));
}

#[test]
fn deux_ecrans_ne_posent_que_les_deux_murs_exterieurs() {
    // Le point décidé avec l'auteur : vu de l'utilisateur, le bureau est
    // UNE boîte. Six plateformes : 2 sols + 2 plafonds + 2 murs.
    let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());
    assert_eq!(monde.platforms().len(), 6);
    assert_eq!(combien_de_face(&monde, Face::Top), 2);
    assert_eq!(combien_de_face(&monde, Face::Bottom), 2);

    // Un seul mur gauche (celui de l'écran de gauche, à x = 0) et un seul
    // mur droit (celui de l'écran de droite, à x = 3840).
    let gauches: Vec<&Platform> = monde
        .platforms()
        .iter()
        .filter(|p| p.has_face(Face::Right))
        .collect();
    assert_eq!(gauches.len(), 1);
    assert_eq!(gauches[0].rect.point_on(Face::Right, 0.0).x, 0.0);

    let droits: Vec<&Platform> = monde
        .platforms()
        .iter()
        .filter(|p| p.has_face(Face::Left))
        .collect();
    assert_eq!(droits.len(), 1);
    assert_eq!(droits[0].rect.point_on(Face::Left, 0.0).x, 3840.0);
}

#[test]
fn deux_ecrans_donnent_des_identites_toutes_distinctes() {
    // Remplace `deux_ecrans_donnent_deux_plateformes_distinctes`.
    let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());
    let mut ids: Vec<u64> = monde.platforms().iter().map(|p| p.id.0).collect();
    ids.sort_unstable();
    let avant = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), avant, "deux plateformes partagent une identité");
}

#[test]
fn meme_support_reconnait_les_quatre_plateformes_d_un_ecran() {
    let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());
    let sol_a = monde.platforms()[0].id;

    let memes = monde
        .platforms()
        .iter()
        .filter(|p| p.id.meme_support(sol_a))
        .count();
    assert_eq!(memes, 3, "sol + plafond + un mur pour l'écran de gauche");
}

#[test]
fn un_ecran_a_gauche_a_bien_son_mur_a_x_negatif() {
    // Contrainte globale du projet : ne jamais supposer x >= 0.
    let monde = World::from_screens(&FakeProbe::ecran_a_gauche_hidpi().screens());
    let mur = monde
        .platforms()
        .iter()
        .find(|p| p.has_face(Face::Right))
        .expect("le mur gauche de l'écran de gauche existe");
    assert!(mur.rect.point_on(Face::Right, 0.0).x < 0.0);
}

#[test]
fn le_sol_est_en_bas_de_la_zone_de_travail() {
    // Le point le plus important de cette tâche. Le rectangle du sol doit
    // avoir son bord SUPÉRIEUR à hauteur du bas de la zone de travail :
    // c'est là qu'on marche.
    let monde = World::from_screens(&FakeProbe::un_ecran().screens());
    let p = &monde.platforms()[0];
    assert_eq!(p.rect.top(), 1032.0);
    assert_eq!(p.rect.left(), 0.0);
    assert_eq!(p.rect.w, 1920.0);
}

#[test]
fn l_identite_survit_a_un_changement_de_resolution() {
    // Spec §5.2 : PlatformId dérive de l'identité de l'écran, pas de sa
    // géométrie. C'est la condition de la décision n° 1.
    let avant = vec![ScreenInfo {
        id: 77,
        work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
        scale: 1.0,
    }];
    let apres = vec![ScreenInfo {
        id: 77,
        work_area: Rect::new(0.0, 0.0, 1280.0, 672.0),
        scale: 1.0,
    }];

    let id_avant = World::from_screens(&avant).platforms()[0].id;
    let id_apres = World::from_screens(&apres).platforms()[0].id;
    assert_eq!(id_avant, id_apres);
}

#[test]
fn get_retrouve_une_plateforme_et_rend_none_sinon() {
    let monde = World::from_screens(&FakeProbe::un_ecran().screens());
    let id = monde.platforms()[0].id;
    assert!(monde.get(id).is_some());
    assert!(monde.get(PlatformId(999_999)).is_none());
}

#[test]
fn nearest_floor_choisit_l_ecran_sous_le_point() {
    let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());

    // Un point au-dessus de l'écran de droite doit retenir SON sol, et
    // l'offset doit être la distance depuis le bord gauche de ce sol.
    let (id, offset) = monde
        .nearest_floor(Point::new(2000.0, 300.0))
        .expect("un sol existe");

    assert_eq!(monde.get(id).unwrap().rect.left(), 1920.0);
    assert_eq!(offset, 80.0);
}

#[test]
fn nearest_floor_rabat_un_point_hors_bureau_sur_le_sol_le_plus_proche() {
    // Le garde-fou de la spec §6.3 : un personnage lâché hors écran ne
    // doit jamais être perdu.
    let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());
    let (id, offset) = monde
        .nearest_floor(Point::new(99_999.0, 500.0))
        .expect("un sol existe");

    let p = monde.get(id).unwrap();
    assert_eq!(p.rect.left(), 1920.0);
    // Rabattu dans les bornes de la face, pas laissé à 98 079.
    assert!(offset >= 0.0 && offset <= p.rect.face_length(Face::Top));
}

#[test]
fn nearest_floor_fonctionne_avec_un_ecran_a_x_negatif() {
    let monde = World::from_screens(&FakeProbe::ecran_a_gauche_hidpi().screens());
    let (id, _) = monde
        .nearest_floor(Point::new(-1000.0, 200.0))
        .expect("un sol existe");
    assert!(monde.get(id).unwrap().rect.left() < 0.0);
}

#[test]
fn un_monde_sans_ecran_est_vide_mais_pas_une_erreur() {
    // `screens()` peut rendre une liste vide (session distante en cours
    // d'établissement). Ça ne doit pas paniquer.
    let monde = World::from_screens(&[]);
    assert!(monde.platforms().is_empty());
    assert_eq!(monde.nearest_floor(Point::new(0.0, 0.0)), None);
    assert_eq!(monde.bounds(), None);
}

#[test]
fn bounds_englobe_tous_les_ecrans() {
    let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());
    let b = monde.bounds().expect("deux écrans");
    // Depuis la Tâche 1, les murs extérieurs débordent d'un pixel de
    // chaque côté de la zone de travail (0..3840) : le mur gauche de
    // l'écran de gauche est posé à `z.left() - EPAISSEUR_PLATEFORME`, et
    // le mur droit de l'écran de droite a sa propre épaisseur au-delà de
    // `z.right()`. `bounds()` en tient compte, ce qui est correct — c'est
    // bien le rectangle englobant réel.
    assert_eq!(b.left(), -1.0);
    assert_eq!(b.right(), 3841.0);
}


// ══════════════════════════════════════════════════════════════════════
// L'occlusion — décision n° 2 (étape 4b)
// ══════════════════════════════════════════════════════════════════════
//
// La soustraction d'intervalles 1D est le cœur de l'étape, et le seul
// endroit où une erreur donnerait un personnage assis dans le vide sans
// qu'aucun message ne le dise. Elle est de l'arithmétique pure : tout se
// vérifie ici, sans écran et sans Windows.

/// Une fenêtre de test, avec un `z` explicite.
///
/// `z` = le rang dans la liste donnée à `from_screens_and_windows`, donc
/// l'appelant doit garder les deux cohérents — comme le fait la vraie sonde.
fn fen(hwnd: u64, x: f32, y: f32, w: f32, h: f32, z: u32) -> WindowInfo {
    WindowInfo {
        hwnd,
        rect: Rect::new(x, y, w, h),
        z,
    }
}

/// Les plateformes issues de fenêtres, dans l'ordre où le monde les range.
fn plateformes_de_fenetres(monde: &World) -> Vec<&Platform> {
    monde
        .platforms()
        .iter()
        .filter(|p| p.kind == PlatformKind::Window)
        .collect()
}

#[test]
fn sans_rien_devant_toute_la_face_est_libre() {
    assert_eq!(soustraire_intervalles(800.0, &[]), vec![(0.0, 800.0)]);
}

#[test]
fn une_fenetre_au_milieu_coupe_le_bord_en_deux() {
    // C'est le schéma du design §5.4, en nombres.
    let libres = soustraire_intervalles(800.0, &[(300.0, 500.0)]);
    assert_eq!(libres, vec![(0.0, 300.0), (500.0, 800.0)]);
}

/// **Le test qui condamne l'algorithme naïf.**
///
/// Soustraire les occultants un par un d'une liste de morceaux est plus
/// court à écrire et donne un résultat FAUX dès que deux occultants se
/// chevauchent — ce qui est le cas normal sur un bureau encombré. Ici les
/// deux fenêtres se recouvrent de 300 à 400, et la zone couverte doit être
/// la RÉUNION `[100, 500]` — un seul bloc, pas deux morceaux qui se marchent
/// dessus et laisseraient reparaître du libre entre eux.
#[test]
fn deux_occultants_qui_se_chevauchent_ne_laissent_qu_un_trou() {
    let libres = soustraire_intervalles(800.0, &[(100.0, 400.0), (300.0, 500.0)]);
    assert_eq!(libres, vec![(0.0, 100.0), (500.0, 800.0)]);

    // Ce que le test dit vraiment : rien n'est libre entre 100 et 500.
    let couvert = |x: f32| !libres.iter().any(|(a, b)| x >= *a && x <= *b);
    assert!(couvert(350.0), "le chevauchement reste couvert");
}

/// Un occultant entièrement contenu dans un précédent ne doit pas ROUVRIR
/// la portion déjà couverte.
///
/// C'est le piège du balayage : sans le `max` sur le curseur, le second
/// occultant le ferait reculer de 600 à 300, et la portion de 300 à 600
/// redeviendrait praticable alors qu'elle est couverte.
#[test]
fn un_occultant_inclus_dans_un_autre_ne_rouvre_rien() {
    let libres = soustraire_intervalles(800.0, &[(100.0, 600.0), (200.0, 300.0)]);
    assert_eq!(libres, vec![(0.0, 100.0), (600.0, 800.0)]);

    // Le point du test : la portion que le second occultant aurait pu
    // rouvrir reste couverte.
    let couvert = |x: f32| !libres.iter().any(|(a, b)| x >= *a && x <= *b);
    assert!(couvert(450.0));
}

/// Les miettes sont jetées : un rebord de 10 px n'est pas une plateforme.
///
/// Sans ce filtre, deux fenêtres presque jointives sèmeraient le monde de
/// morceaux sur lesquels le personnage déborderait complètement — et il
/// aurait l'air posé dans le vide, ce que la décision n° 2 cherche
/// justement à rendre impossible.
#[test]
fn les_morceaux_trop_courts_sont_jetes() {
    // Un trou de 10 px entre deux occultants, et une queue de 10 px.
    let libres = soustraire_intervalles(510.0, &[(0.0, 250.0), (260.0, 500.0)]);
    assert!(
        libres.is_empty(),
        "aucun de ces morceaux n'est assez long pour qu'on s'y tienne : {libres:?}"
    );
}

#[test]
fn une_fenetre_expose_ses_quatre_bords() {
    let ecrans = FakeProbe::un_ecran().screens();
    let monde = World::from_screens_and_windows(&ecrans, &[fen(7, 400.0, 300.0, 600.0, 400.0, 0)]);

    let f = plateformes_de_fenetres(&monde);
    assert_eq!(f.len(), 4, "dessus, dessous et les deux bords");

    // Toutes portent la même poignée, donc le MÊME support — c'est ce qui
    // permettra à `Grimper` de passer de la barre de titre au flanc.
    let dessus = f[0];
    assert!(f.iter().all(|p| p.id.meme_support(dessus.id)));
}

/// **Le test qui empêche le personnage de flotter.**
///
/// Chaque plateforme doit poser son `point_on` exactement sur le bord
/// VISUEL de la fenêtre. C'est la seule propriété qui compte
/// géométriquement : une erreur d'un `EPAISSEUR_PLATEFORME` ici, et il est
/// assis 1 px dans la barre de titre — invisible en test, très visible sur
/// du pixel-art.
#[test]
fn chaque_bord_tombe_sur_le_bord_visuel_de_la_fenetre() {
    let ecrans = FakeProbe::un_ecran().screens();
    let monde = World::from_screens_and_windows(&ecrans, &[fen(7, 400.0, 300.0, 600.0, 400.0, 0)]);

    for p in plateformes_de_fenetres(&monde) {
        let face = p.faces[0];
        let point = p.rect.point_on(face, 0.0);
        match face {
            // La barre de titre est en haut de la fenêtre.
            Face::Top => assert_eq!(point.y, 300.0),
            // Le dessous est en bas : 300 + 400.
            Face::Bottom => assert_eq!(point.y, 700.0),
            Face::Left => assert_eq!(point.x, 400.0),
            // Le bord droit : 400 + 600.
            Face::Right => assert_eq!(point.x, 1000.0),
        }
    }
}

/// Une fenêtre entièrement recouverte ne doit exposer AUCUNE plateforme.
///
/// Pas même une plateforme « vide » : `nearest_floor` et les intentions la
/// verraient comme une cible valable, et il s'y rendrait pour rien avant
/// d'échouer au délai d'abandon. Un piège silencieux.
#[test]
fn une_fenetre_entierement_couverte_n_expose_rien() {
    let ecrans = FakeProbe::un_ecran().screens();
    let monde = World::from_screens_and_windows(
        &ecrans,
        &[
            // Devant, et strictement plus grande de tous les côtés.
            fen(1, 300.0, 200.0, 900.0, 700.0, 0),
            fen(2, 400.0, 300.0, 600.0, 400.0, 1),
        ],
    );

    let cachee = PlatformId::fenetre(2, Role::Sol);
    assert!(
        monde.get(cachee).is_none(),
        "la fenêtre du dessous est invisible : elle n'est pas une plateforme"
    );
}

/// **Le sens du z-order, et il est facile de l'inverser sans s'en rendre
/// compte.**
///
/// Une fenêtre ne peut être masquée que par celles qui sont DEVANT elle. Si
/// on inversait le sens, la fenêtre au premier plan serait masquée par tout
/// ce qui est derrière — exactement le contraire, et le symptôme serait « il
/// ne s'assoit jamais sur la fenêtre active », ce qui ressemble beaucoup à
/// un problème de filtrage.
#[test]
fn la_fenetre_de_devant_n_est_pas_masquee_par_celle_de_derriere() {
    let ecrans = FakeProbe::un_ecran().screens();
    let monde = World::from_screens_and_windows(
        &ecrans,
        &[
            fen(1, 400.0, 300.0, 600.0, 400.0, 0), // devant
            fen(2, 300.0, 200.0, 900.0, 700.0, 1), // derrière, et plus grande
        ],
    );

    let devant = monde
        .get(PlatformId::fenetre(1, Role::Sol))
        .expect("la fenêtre de devant reste praticable");
    assert_eq!(
        devant.libre,
        vec![(0.0, 600.0)],
        "rien de ce qui est derrière ne peut la masquer"
    );
}

/// La barre de titre partiellement recouverte garde ses morceaux, et
/// `est_libre` répond juste de part et d'autre.
#[test]
fn une_barre_de_titre_a_moitie_couverte_garde_ses_morceaux() {
    let ecrans = FakeProbe::un_ecran().screens();
    let monde = World::from_screens_and_windows(
        &ecrans,
        &[
            // Devant : couvre la moitié droite de la barre de titre de #2.
            fen(1, 700.0, 200.0, 600.0, 400.0, 0),
            fen(2, 400.0, 300.0, 600.0, 400.0, 1),
        ],
    );

    let barre = monde
        .get(PlatformId::fenetre(2, Role::Sol))
        .expect("la moitié gauche reste praticable");

    // La fenêtre #2 va de x=400 à x=1000 ; #1 la couvre à partir de x=700,
    // soit un offset de 300.
    assert_eq!(barre.libre, vec![(0.0, 300.0)]);

    assert!(barre.est_libre(150.0), "à gauche, il tient");
    assert!(!barre.est_libre(450.0), "à droite, c'est recouvert");
}

/// **La précision de périmètre, verrouillée par un test.**
///
/// Le sol de l'écran n'est jamais occulté, même sous une fenêtre maximisée.
/// Appliquer l'occlusion aux plateformes d'écran priverait le personnage de
/// tout endroit où marcher sur un bureau ordinaire — ce que la décision n° 2
/// ne cherchait pas. Sans ce test, quelqu'un « corrigerait » l'incohérence
/// apparente un jour, et le personnage disparaîtrait sous la première
/// fenêtre maximisée.
#[test]
fn le_sol_de_l_ecran_survit_a_une_fenetre_maximisee() {
    let ecrans = FakeProbe::un_ecran().screens();

    // Une fenêtre qui couvre toute la zone de travail.
    let monde = World::from_screens_and_windows(&ecrans, &[fen(1, 0.0, 0.0, 1920.0, 1032.0, 0)]);

    let sol = monde.premier_sol().expect("le sol existe toujours");
    assert_eq!(sol.kind, PlatformKind::Screen);
    assert!(
        sol.est_libre(960.0),
        "on marche au bas de l'écran quoi qu'il y ait derrière"
    );
}

/// Sans aucune fenêtre, le monde est **exactement** celui d'avant l'étape 4b.
///
/// C'est ce qui garantit que les 240 tests écrits avant cette étape
/// continuent de décrire le même monde : `from_screens` n'est pas une
/// abréviation approximative de `from_screens_and_windows`, c'en est le cas
/// particulier exact.
#[test]
fn sans_fenetre_le_monde_est_celui_des_ecrans_seuls() {
    let ecrans = FakeProbe::deux_ecrans().screens();
    let avec = World::from_screens_and_windows(&ecrans, &[]);
    let sans = World::from_screens(&ecrans);

    assert_eq!(avec.platforms(), sans.platforms());
}

/// **La preuve que l'occlusion MORD.**
///
/// Tous les tests ci-dessus vérifient un calcul ; celui-ci vérifie qu'il
/// change quelque chose. Un personnage assis sur une barre de titre, une
/// fenêtre passe devant et recouvre l'endroit exact où il est : il doit
/// tomber. Sans ce test, `libre` pourrait être parfaitement calculé et
/// n'être consulté nulle part — l'étape entière serait morte, et verte.
#[test]
fn recouvert_pendant_qu_il_est_assis_il_tombe() {
    use crate::character::attach::{hors_bornes, Attachment};

    let ecrans = FakeProbe::un_ecran().screens();
    let barre = PlatformId::fenetre(2, Role::Sol);

    // Il est posé à 450 px du bord gauche de la fenêtre #2 (x = 400),
    // donc à x = 850 à l'écran.
    let assis = Attachment::On {
        platform: barre,
        face: Face::Top,
        offset: 450.0,
    };

    // Avant : la fenêtre #2 est seule, toute sa barre de titre est libre.
    let avant = World::from_screens_and_windows(&ecrans, &[fen(2, 400.0, 300.0, 600.0, 400.0, 0)]);
    assert!(!hors_bornes(&assis, &avant), "rien ne le recouvre encore");

    // Après : une fenêtre passe DEVANT et couvre à partir de x = 700,
    // c'est-à-dire à partir de l'offset 300 — donc sous ses pieds.
    let apres = World::from_screens_and_windows(
        &ecrans,
        &[
            fen(1, 700.0, 200.0, 600.0, 400.0, 0),
            fen(2, 400.0, 300.0, 600.0, 400.0, 1),
        ],
    );
    assert!(
        hors_bornes(&assis, &apres),
        "sa place vient d'être recouverte : il n'a plus rien sous les pieds"
    );

    // ⚠️ Et l'identité, elle, n'a PAS bougé — c'est la décision n° 1.
    // Si le numéro de segment était entré dans `PlatformId`, la plateforme
    // aurait changé d'identité et il serait tombé pour la mauvaise raison,
    // ce que ce test ne distinguerait pas sans cette ligne.
    assert!(
        apres.get(barre).is_some(),
        "la barre de titre existe toujours, c'est SA place qui est prise"
    );
}
