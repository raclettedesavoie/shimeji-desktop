//! Les tests de `world` — sortis du fichier source le 2026-09-14.
//!
//! **Pourquoi ce fichier existe** : `world.rs` faisait 606 lignes dont 236 de tests,
//! soit 39 %. Toute lecture du module en payait le double, pour rien la plupart
//! du temps. Les tests n'ont pas changé d'une ligne : ils ont seulement déménagé, et
//! se désindentent d'un cran puisqu'ils ne sont plus enfermés dans un `mod tests { }`.

use super::*;
use crate::geom::{Face, Point};
use crate::probe::fake::FakeProbe;
use crate::probe::{ScreenInfo, SystemProbe};

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
fn deux_ecrans_cote_a_cote_ne_posent_que_les_deux_murs_exterieurs() {
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
fn meme_ecran_reconnait_les_quatre_plateformes_d_un_ecran() {
    let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());
    let sol_a = monde.platforms()[0].id;

    let memes = monde
        .platforms()
        .iter()
        .filter(|p| p.id.meme_ecran(sol_a))
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
