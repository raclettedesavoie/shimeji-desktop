//! Les tests de `plein_ecran` — l'écran qu'un jeu a pris pour lui.
use super::*;
use crate::probe::fake::FakeProbe;
use crate::probe::SystemProbe;

#[test]
fn une_fenetre_exactement_de_la_taille_de_l_ecran_le_couvre() {
    let ecran = Rect::new(1920.0, 0.0, 1920.0, 1080.0);
    assert!(couvre(ecran, ecran));
}

#[test]
fn une_fenetre_qui_deborde_couvre_encore() {
    // Certains jeux débordent d'un pixel : ce n'est pas une raison pour
    // laisser les personnages se promener par-dessus.
    let ecran = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let fenetre = Rect::new(-1.0, -1.0, 1922.0, 1082.0);
    assert!(couvre(fenetre, ecran));
}

#[test]
fn une_fenetre_agrandie_ne_couvre_pas_la_barre_des_taches() {
    // Une fenêtre agrandie s'arrête à la zone de travail : il manque les
    // 48 px de la barre des tâches, ce n'est PAS du plein écran.
    let ecran = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let agrandie = Rect::new(0.0, 0.0, 1920.0, 1032.0);
    assert!(!couvre(agrandie, ecran));
}

#[test]
fn une_fenetre_d_un_autre_ecran_ne_le_couvre_pas() {
    let ecran = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let voisine = Rect::new(1920.0, 0.0, 1920.0, 1080.0);
    assert!(!couvre(voisine, ecran));
}

// Les trois tests suivants reprennent les rectangles RELEVÉS sur la machine
// de l'auteur le 2026-09-28 (voir `est_en_plein_ecran`). `Rect::new` prend
// (x, y, largeur, hauteur), d'où les soustractions.

#[test]
fn chrome_en_plein_ecran_compte_bien_qu_agrandi() {
    // Twitch en plein écran : Chrome garde l'état « agrandie ».
    let ecran = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let chrome = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    assert!(est_en_plein_ecran(chrome, ecran, true));
}

#[test]
fn une_fenetre_agrandie_ordinaire_ne_compte_pas() {
    // VS Code agrandi sur l'écran de droite.
    let ecran = Rect::new(1920.0, 0.0, 1920.0, 1080.0);
    let vscode = Rect::new(1912.0, -8.0, 3848.0 - 1912.0, 1040.0 + 8.0);
    assert!(!est_en_plein_ecran(vscode, ecran, true));
}

#[test]
fn agrandie_avec_barre_des_taches_masquee_ne_compte_pas() {
    // La barre masquée automatiquement : la fenêtre agrandie COUVRE l'écran,
    // mais en débordant de sa bordure invisible. Ce n'est pas du plein écran.
    let ecran = Rect::new(0.0, 0.0, 1920.0, 1080.0);
    let agrandie = Rect::new(-8.0, -8.0, 1936.0, 1096.0);
    assert!(couvre(agrandie, ecran));
    assert!(!est_en_plein_ecran(agrandie, ecran, true));
    // Le même rectangle, SANS l'état « agrandie » : un jeu qui déborde.
    assert!(est_en_plein_ecran(agrandie, ecran, false));
}

#[test]
fn l_ecran_pris_disparait_des_ecrans_habitables() {
    let tous = FakeProbe::deux_ecrans().screens();
    let restants = ecrans_habitables(&tous, &[2]);
    assert_eq!(restants.len(), 1);
    assert_eq!(restants[0].id, 1);
}

#[test]
fn sans_plein_ecran_tous_les_ecrans_restent() {
    let tous = FakeProbe::deux_ecrans().screens();
    assert_eq!(ecrans_habitables(&tous, &[]), tous);
}

#[test]
fn un_plein_ecran_sur_chaque_ecran_ne_laisse_nulle_part_ou_aller() {
    // Le bug du 2026-09-28 : un seul écran était retenu, et cliquer d'une
    // vidéo à l'autre les faisait tomber sur l'écran qu'on venait de quitter.
    // Les deux pris, c'est la liste vide — donc tout le monde caché.
    let tous = FakeProbe::deux_ecrans().screens();
    assert!(ecrans_habitables(&tous, &[1, 2]).is_empty());
}

#[test]
fn un_seul_ecran_pris_ne_laisse_nulle_part_ou_aller() {
    // C'est à l'appelant de tout cacher : la liste vide est le signal.
    let tous = FakeProbe::un_ecran().screens();
    assert!(ecrans_habitables(&tous, &[1]).is_empty());
}

#[test]
fn le_monde_sans_l_ecran_pris_n_y_garde_aucun_sol() {
    // La propriété qui fait tout le travail : retirer l'écran du recensement
    // retire son sol, donc personne ne peut plus y marcher, et
    // `point_de_chute` ne peut plus y faire tomber personne.
    //
    // On ne regarde que les SOLS (face `Top`) : les murs sont posés à
    // l'EXTÉRIEUR de leur écran (`from_screens`), donc le mur droit de
    // l'écran 1 déborde légitimement sur l'écran 2 — et c'est lui qui
    // arrête désormais les personnages à la frontière.
    let tous = FakeProbe::deux_ecrans().screens();
    let monde = crate::world::World::from_screens(&ecrans_habitables(&tous, &[2]));
    let ecran_pris = tous[1];
    for p in monde.platforms().iter().filter(|p| p.has_face(crate::geom::Face::Top)) {
        let milieu = p.rect.x + p.rect.w / 2.0;
        assert!(
            milieu < ecran_pris.work_area.left(),
            "un sol survit sur l'écran pris : {:?}",
            p.rect
        );
    }
}

#[test]
fn celui_qui_tombe_apres_l_eviction_atterrit_sur_un_autre_ecran() {
    // Ce que fait la boucle au moment où le jeu prend l'écran 2 : ceux qui y
    // étaient retombent par `point_de_chute`, sur le monde SANS lui.
    //
    // Semé UNE fois, l'état avance d'un tirage à l'autre (CLAUDE.md,
    // « Semer l'aléatoire une seule fois »).
    let tous = FakeProbe::deux_ecrans().screens();
    let monde = crate::world::World::from_screens(&ecrans_habitables(&tous, &[2]));
    let mut rng = crate::rng::XorShift32::seeded(2026);
    for _ in 0..500 {
        let att = crate::apparition::point_de_chute(&monde, 128.0, &mut rng).expect("écran 1");
        let crate::character::attach::Attachment::Falling { pos, .. } = att else {
            panic!("point_de_chute ne rend que Falling");
        };
        assert!(!est_sur(&tous[1], pos), "il retombe sur l'écran du jeu : {pos:?}");
    }
}

#[test]
fn est_sur_departage_deux_ecrans_voisins() {
    let tous = FakeProbe::deux_ecrans().screens();
    let p = Point::new(2500.0, 500.0);
    assert!(!est_sur(&tous[0], p));
    assert!(est_sur(&tous[1], p));
    // La frontière appartient à l'écran de droite (`Rect::contains` exclut
    // le bord droit) : un point n'est jamais sur deux écrans à la fois.
    let frontiere = Point::new(1920.0, 500.0);
    assert!(!est_sur(&tous[0], frontiere));
    assert!(est_sur(&tous[1], frontiere));
}
