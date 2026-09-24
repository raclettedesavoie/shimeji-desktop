//! Les tests de `physics` — sortis du fichier source le 2026-09-14.
//!
//! **Pourquoi ce fichier existe** : `character/physics.rs` faisait 1138 lignes dont 488 de tests,
//! soit 43 %. Toute lecture du module en payait le double, pour rien la plupart
//! du temps. Les tests n'ont pas changé d'une ligne : ils ont seulement déménagé, et
//! se désindentent d'un cran puisqu'ils ne sont plus enfermés dans un `mod tests { }`.

use super::*;
use crate::geom::Rect;
use crate::probe::fake::FakeProbe;
use crate::probe::{ScreenInfo, SystemProbe};

fn monde_deux_ecrans() -> World {
    World::from_screens(&FakeProbe::deux_ecrans().screens())
}

/// Un monde d'un seul écran isolé : sol à y = 1032, mur gauche à x = 0,
/// mur droit à x = 1920, plafond à y = 0.
fn monde_isole() -> World {
    World::from_screens(&crate::probe::fake::FakeProbe::un_ecran().screens())
}

#[test]
fn contact_rend_le_sol_comme_avant() {
    // La première moitié de `contact` est l'ancien `atterrissage`, et
    // elle ne doit rien changer.
    let m = monde_isole();
    let (_, face, offset) =
        contact(&m, Point::new(500.0, 1020.0), Point::new(500.0, 1040.0))
            .expect("il traverse la ligne du sol");
    assert_eq!(face, Face::Top);
    assert_eq!(offset, 500.0);
}

#[test]
fn un_lancer_vers_la_gauche_s_accroche_au_mur_gauche() {
    let m = monde_isole();
    let (_, face, offset) =
        contact(&m, Point::new(20.0, 400.0), Point::new(-10.0, 420.0))
            .expect("il traverse la ligne x = 0 vers la gauche");

    // Le mur GAUCHE de l'écran expose sa face `Right` : le personnage se
    // tient à sa droite, donc à l'intérieur de l'écran.
    assert_eq!(face, Face::Right);
    // L'offset compte vers le BAS depuis le haut de la zone de travail.
    assert_eq!(offset, 420.0);
}

#[test]
fn un_lancer_vers_la_droite_s_accroche_au_mur_droit() {
    let m = monde_isole();
    let (_, face, offset) =
        contact(&m, Point::new(1900.0, 300.0), Point::new(1930.0, 310.0))
            .expect("il traverse la ligne x = 1920 vers la droite");
    assert_eq!(face, Face::Left);
    assert_eq!(offset, 310.0);
}

#[test]
fn on_ne_s_accroche_pas_a_un_mur_qu_on_quitte() {
    // Le sens compte : partir du mur vers l'intérieur ne doit PAS
    // s'accrocher, sinon un personnage qui se lâche se rattraperait
    // aussitôt.
    let m = monde_isole();
    assert_eq!(contact(&m, Point::new(-10.0, 400.0), Point::new(20.0, 420.0)), None);
}

#[test]
fn le_sol_gagne_sur_le_mur_dans_un_coin() {
    // Repris de `Fall.java`, dont la boucle de sous-pas fait `break OUTER`
    // sur le sol AVANT de tester le mur. Sans cette priorité, un lancer
    // dans le coin s'accrocherait au mur trois pixels au-dessus du sol au
    // lieu d'atterrir — visiblement bête.
    //
    // ⚠️ **Les coordonnées d'arrivée sont exactement sur les deux bords, et
    // ce n'est pas de la coquetterie.** `atterrissage` teste `apres.x`
    // contre les bornes du sol, et `contact_mur` teste `apres.y` contre
    // celles du mur : le seul point qui satisfait les deux à la fois est le
    // coin lui-même. C'est une conséquence de l'approximation « on teste
    // avec le point d'arrivée » — voir le test suivant, qui la documente.
    //
    // La comparaison de flottants est ici exacte et sûre : ce sont des
    // littéraux, aucune accumulation ne les a arrondis.
    let m = monde_isole();
    let (_, face, _) = contact(&m, Point::new(20.0, 1020.0), Point::new(0.0, 1032.0))
        .expect("il franchit le sol ET le mur dans le même pas");
    assert_eq!(face, Face::Top);
}

#[test]
fn un_lancer_violent_dans_le_coin_ne_rattrape_rien_et_c_est_assume() {
    // Le trou de l'approximation, écrit ici pour qu'il soit CONNU et non
    // découvert deux fois : un segment qui sort par le coin bas-gauche
    // rate le sol (son `apres.x` est négatif, hors des bornes du sol) ET
    // le mur (son `apres.y` passe sous le bas du mur).
    //
    // On l'accepte au lieu de calculer le croisement exact, pour deux
    // raisons : la décision n° 4 autorise une navigation imparfaite, et le
    // garde-fou de la spec §6.3 (`sous_le_bureau` puis `nearest_floor`)
    // replace de toute façon le personnage sur le sol le plus proche. Le
    // coût est une fraction de seconde de chute en trop, dans un coin, sur
    // un lancer violent.
    //
    // Si ce test se met un jour à rendre `Some`, ce n'est PAS une
    // régression : c'est que quelqu'un a amélioré l'approximation, et il
    // faut alors supprimer ce test plutôt que le « réparer ».
    let m = monde_isole();
    assert_eq!(
        contact(&m, Point::new(20.0, 1020.0), Point::new(-10.0, 1040.0)),
        None
    );
}

#[test]
fn un_lancer_vers_le_haut_s_accroche_au_plafond() {
    // **Le renversement de la règle 3.** `Fall.java::hasNext()` ne
    // teste que le sol et le mur, mais l'auteur a essayé « il passe
    // devant » à l'écran et lui a préféré l'inverse (design §3.2,
    // révisé le 2026-09-12) : lancé vers le haut, il s'accroche au
    // plafond exactement comme il s'accroche à un mur.
    let m = monde_isole();
    let (_, face, offset) =
        contact(&m, Point::new(500.0, 20.0), Point::new(500.0, -10.0))
            .expect("il traverse la ligne y = 0 en montant");
    assert_eq!(face, Face::Bottom);
    // L'offset d'une face `Bottom` compte vers la DROITE, comme au sol.
    assert_eq!(offset, 500.0);
}

#[test]
fn on_ne_s_accroche_pas_a_un_plafond_qu_on_quitte() {
    // Symétrique de `on_ne_s_accroche_pas_a_un_mur_qu_on_quitte` : un
    // personnage déjà pile sur la ligne du plafond, qui DESCEND, ne doit
    // pas s'y raccrocher — sinon un personnage qui vient de se lâcher du
    // plafond se rattraperait aussitôt, à chaque image, pour toujours.
    // C'est l'asymétrie strict/large de `contact_plafond`, testée ici
    // directement.
    let m = monde_isole();
    assert_eq!(contact(&m, Point::new(500.0, 0.0), Point::new(500.0, 20.0)), None);
}

#[test]
fn un_mur_hors_de_la_hauteur_du_pas_n_attrape_pas() {
    // Franchir la ligne x = 0 SOUS le bas de la zone de travail ne doit
    // pas s'accrocher : il n'y a plus de mur à cette hauteur.
    let m = monde_isole();
    assert_eq!(
        contact(&m, Point::new(20.0, 2000.0), Point::new(-10.0, 2020.0)),
        None
    );
}

#[test]
fn aucun_seuil_de_vitesse_pour_s_accrocher() {
    // `Fall.java` ne teste qu'un `isOn`, sans aucune condition de
    // vitesse : un contact d'un pixel suffit. Ce test fige cette absence
    // de seuil, pour qu'on ne la « corrige » pas plus tard.
    let m = monde_isole();
    assert!(contact(&m, Point::new(0.5, 400.0), Point::new(-0.5, 400.1)).is_some());
}

/// Un pas d'intégration à 60 Hz.
const DT: f32 = 1.0 / 60.0;

#[test]
fn la_chute_accelere_vers_le_bas() {
    let (pos, vel) = integrer_chute(Point::new(100.0, 0.0), Vec2::zero(), DT);
    // y croît vers le bas : la vitesse et la position augmentent toutes
    // deux.
    assert!(vel.y > 0.0);
    assert!(pos.y > 0.0);
    // Aucune accélération horizontale : la gravité est verticale.
    assert_eq!(vel.x, 0.0);
    assert_eq!(pos.x, 100.0);
}

#[test]
fn l_elan_horizontal_decroit_sans_s_annuler() {
    // Un personnage lâché en marchant garde son élan, mais **amorti** :
    // `Fall.java` retire `RESISTANCEX = 0,05` de la vitesse à chaque
    // tick. La première version du projet ne l'amortissait pas du tout,
    // ce qui donnait des trajectoires trop plates.
    let (pos, vel) = integrer_chute(Point::new(100.0, 0.0), Vec2::new(80.0, 0.0), DT);

    assert!(vel.x < 80.0, "l'élan doit décroître");
    assert!(vel.x > 70.0, "mais pas s'effondrer en une image");
    assert!(pos.x > 100.0, "il avance quand même");
}

#[test]
fn l_elan_horizontal_finit_par_s_eteindre() {
    // Sur plusieurs secondes, le frottement doit l'avoir presque annulé.
    let mut pos = Point::new(0.0, 0.0);
    let mut vel = Vec2::new(200.0, 0.0);
    for _ in 0..(60 * 4) {
        let (p, v) = integrer_chute(pos, vel, DT);
        pos = p;
        vel = v;
    }
    assert!(vel.x.abs() < 5.0, "élan résiduel {} px/s", vel.x);
}

#[test]
fn la_chute_est_deterministe_en_hauteur_et_en_duree() {
    // Le test que la spec §10.1 demande : « intégration déterministe,
    // hauteur et durée connues ».
    //
    // **Avec frottement, ce n'est plus ½·g·t².** La solution de
    // `dV/dt = G − k·V` depuis l'immobilité donne, en une seconde :
    //
    //     y(t) = (G/k)·(t − (1 − e^{−k·t})/k)
    //          = 500 · (1 − 0,918/2,5) ≈ 316 px
    //
    // Sans frottement on aurait 625 px : la chute est donc **deux fois
    // plus lente**, et c'est exactement ce que Shimeji-ee fait.
    let mut pos = Point::new(0.0, 0.0);
    let mut vel = Vec2::zero();

    for _ in 0..60 {
        let (p, v) = integrer_chute(pos, vel, DT);
        pos = p;
        vel = v;
    }

    let vitesse_limite = GRAVITE / FROTTEMENT_CHUTE_Y;
    let theorique =
        vitesse_limite * (1.0 - (1.0 - (-FROTTEMENT_CHUTE_Y).exp()) / FROTTEMENT_CHUTE_Y);

    assert!(
        (pos.y - theorique).abs() < theorique * 0.03,
        "chute de {} px en 1 s, théorie {} px",
        pos.y,
        theorique
    );

    // Et le point qui compte pour l'œil : c'est bien plus lent que sans
    // frottement.
    assert!(
        pos.y < 0.5 * GRAVITE * 0.7,
        "la chute devrait être nettement plus lente que ½·g·t²"
    );
}

#[test]
fn la_chute_atteint_sa_vitesse_limite_par_frottement() {
    // **C'est le frottement qui plafonne, pas le plafond dur.** La
    // vitesse limite est `GRAVITE / FROTTEMENT_CHUTE_Y` = 500 px/s, ce
    // qui correspond aux 20 px/tick de Shimeji-ee.
    //
    // Le plafond dur `VITESSE_CHUTE_MAX` n'est donc jamais atteint en
    // chute libre : il ne sert que de garde-fou si un lâcher venait avec
    // une vitesse initiale énorme.
    let mut vel = Vec2::new(0.0, 0.0);
    let mut pos = Point::new(0.0, 0.0);
    for _ in 0..600 {
        let (p, v) = integrer_chute(pos, vel, DT);
        pos = p;
        vel = v;
    }

    let limite = GRAVITE / FROTTEMENT_CHUTE_Y;
    assert!(
        (vel.y - limite).abs() < 1.0,
        "vitesse limite {} px/s, attendu {}",
        vel.y,
        limite
    );
    assert!(vel.y < VITESSE_CHUTE_MAX, "le plafond dur ne doit pas mordre");
}

#[test]
fn un_lancer_vertical_monte_haut() {
    // **Le jet vers le haut paraissait mou** (rapporté à l'écran le
    // 2026-09-12) : le frottement de Shimeji-ee, appliqué aussi en montée,
    // s'AJOUTAIT à la pesanteur et coupait l'élan — ~235 px seulement.
    //
    // Depuis `FROTTEMENT_MONTEE_Y`, la montée est six fois moins freinée que
    // la chute, et l'apex vaut ~450 px au plafond de lancer. Le chiffre a été
    // choisi à l'écran, en deux essais : 576 px (aucun frottement du tout)
    // était trop haut.
    let mut pos = Point::new(0.0, 0.0);
    let mut vel = Vec2::new(0.0, -VITESSE_LANCER_MAX);

    // Le plus haut point atteint, c'est le plus PETIT y (l'axe descend).
    let mut apex = 0.0_f32;
    while vel.y < 0.0 {
        let (p, v) = integrer_chute(pos, vel, DT);
        pos = p;
        vel = v;
        apex = apex.min(pos.y);
    }

    // Une fourchette, et pas une égalité : c'est le RESSENTI qui est
    // verrouillé ici, pas une valeur au pixel. Les deux bornes sont les deux
    // essais écartés — 235 px d'un côté, 576 px de l'autre.
    assert!(
        (-apex - 450.0).abs() < 25.0,
        "monté de {} px, attendu ~450 px",
        -apex
    );
}

#[test]
fn la_montee_est_moins_freinee_que_la_chute_mais_l_est_encore() {
    // Le garde-fou de `FROTTEMENT_MONTEE_Y` : ni zéro (l'apex repartirait à
    // 576 px), ni la valeur de la chute (il retomberait à 235 px). Le test
    // porte sur la constante elle-même, pour qu'un futur réglage « rond »
    // ne la ramène pas silencieusement à l'un des deux extrêmes.
    assert!(FROTTEMENT_MONTEE_Y > 0.0);
    assert!(FROTTEMENT_MONTEE_Y < FROTTEMENT_CHUTE_Y);
}

#[test]
fn un_lancer_en_diagonale_ne_se_colle_pas_au_plafond() {
    // **La régression du 2026-09-14**, et la raison de `PENTE_MIN_PLAFOND`.
    // Lancé à 60° au-dessus de l'horizontale depuis le haut de l'écran, il
    // frôlait le plafond, s'y collait net, et sa portée tombait de 453 px à
    // 204 px. Il doit désormais passer devant et finir sa course au SOL.
    let m = monde_isole();
    let angle = 60.0_f32.to_radians();
    let mut pos = Point::new(300.0, 300.0);
    let mut vel = Vec2::new(
        VITESSE_LANCER_MAX * angle.cos(),
        -VITESSE_LANCER_MAX * angle.sin(),
    );

    let mut face_finale = None;
    for _ in 0..900 {
        let (np, nv) = integrer_chute(pos, vel, DT);
        if let Some((_, face, _)) = contact(&m, pos, np) {
            face_finale = Some(face);
            pos = np;
            break;
        }
        pos = np;
        vel = nv;
    }

    assert_eq!(face_finale, Some(Face::Top), "il devrait finir au sol");
    assert!(
        pos.x - 300.0 > 400.0,
        "portée de {} px, il s'est fait arrêter en route",
        pos.x - 300.0
    );
}

#[test]
fn un_lancer_bien_vertical_s_accroche_toujours_au_plafond() {
    // Le pendant du test précédent : `PENTE_MIN_PLAFOND` ne doit pas avoir
    // ANNULÉ la divergence assumée du 2026-09-12, seulement l'avoir
    // restreinte. À 75° il s'accroche encore, et à 90° évidemment.
    let m = monde_isole();
    for angle_deg in [75.0_f32, 90.0] {
        let angle = angle_deg.to_radians();
        let mut pos = Point::new(300.0, 300.0);
        let mut vel = Vec2::new(
            VITESSE_LANCER_MAX * angle.cos(),
            -VITESSE_LANCER_MAX * angle.sin(),
        );

        let mut face_finale = None;
        for _ in 0..900 {
            let (np, nv) = integrer_chute(pos, vel, DT);
            if let Some((_, face, _)) = contact(&m, pos, np) {
                face_finale = Some(face);
                break;
            }
            pos = np;
            vel = nv;
        }

        assert_eq!(
            face_finale,
            Some(Face::Bottom),
            "à {angle_deg}° il devrait s'accrocher au plafond"
        );
    }
}

#[test]
fn le_plafond_dur_borne_une_vitesse_initiale_absurde() {
    // Le garde-fou existe pour ça, et pour rien d'autre.
    let (_, vel) = integrer_chute(Point::new(0.0, 0.0), Vec2::new(0.0, 50_000.0), DT);
    assert_eq!(vel.y, VITESSE_CHUTE_MAX);
}

#[test]
fn atterrit_en_traversant_le_sol() {
    let monde = monde_deux_ecrans();
    // Le sol du premier écran est à y = 1032.
    let avant = Point::new(300.0, 1020.0);
    let apres = Point::new(300.0, 1040.0);

    let (id, offset) = atterrissage(&monde, avant, apres).expect("doit atterrir");
    assert_eq!(monde.get(id).unwrap().rect.top(), 1032.0);
    assert_eq!(offset, 300.0);
}

#[test]
fn n_atterrit_pas_en_montant() {
    // Un personnage qui monte (lâché vers le haut, ou plus tard un saut)
    // ne doit pas s'accrocher au sol qu'il traverse par-dessous.
    let monde = monde_deux_ecrans();
    let avant = Point::new(300.0, 1040.0);
    let apres = Point::new(300.0, 1020.0);
    assert_eq!(atterrissage(&monde, avant, apres), None);
}

#[test]
fn n_atterrit_pas_a_cote_de_la_plateforme() {
    // Entre les deux écrans il n'y a rien à x = 5000 : il continue de
    // tomber, et le garde-fou le récupérera.
    let monde = monde_deux_ecrans();
    let avant = Point::new(5000.0, 1020.0);
    let apres = Point::new(5000.0, 1040.0);
    assert_eq!(atterrissage(&monde, avant, apres), None);
}

#[test]
fn atterrit_sur_le_sol_du_bon_ecran() {
    let monde = monde_deux_ecrans();
    let (id, offset) = atterrissage(
        &monde,
        Point::new(2500.0, 1020.0),
        Point::new(2500.0, 1040.0),
    )
    .expect("doit atterrir");

    let plat = monde.get(id).unwrap();
    assert_eq!(plat.rect.left(), 1920.0);
    // L'offset est relatif au bord GAUCHE de cette plateforme.
    assert_eq!(offset, 580.0);
}

#[test]
fn atterrit_sur_la_plateforme_la_plus_haute_traversee() {
    // Deux faces traversées dans le même pas : il doit s'arrêter sur la
    // PREMIÈRE rencontrée en descendant, donc la plus haute (plus petit
    // y). À l'étape 4, ce sera le cas d'une barre de titre au-dessus du
    // sol — la règle est écrite maintenant pour ne pas avoir à y revenir.
    let monde = World::from_screens(&[
        ScreenInfo {
            id: 1,
            work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
            bounds: Rect::new(0.0, 0.0, 1920.0, 1032.0),
            scale: 1.0,
        },
        ScreenInfo {
            id: 2,
            // Un écran fictif dont la zone de travail finit plus haut :
            // son sol est donc à y = 600.
            work_area: Rect::new(0.0, 0.0, 1920.0, 600.0),
            bounds: Rect::new(0.0, 0.0, 1920.0, 600.0),
            scale: 1.0,
        },
    ]);

    let (id, _) = atterrissage(&monde, Point::new(300.0, 500.0), Point::new(300.0, 1100.0))
        .expect("doit atterrir");

    assert_eq!(monde.get(id).unwrap().rect.top(), 600.0);
}

#[test]
fn atterrit_pile_sur_la_ligne_du_sol() {
    // Cas limite : `apres.y` vaut exactement la hauteur du sol. Il doit
    // atterrir, pas passer à travers.
    let monde = monde_deux_ecrans();
    assert!(atterrissage(
        &monde,
        Point::new(300.0, 1000.0),
        Point::new(300.0, 1032.0)
    )
    .is_some());
}

#[test]
fn la_bande_neutre_du_balancier_est_symetrique() {
    // **Le test qui garde la correction du bug d'ordre de Shimeji-ee.**
    // Un retard négatif minuscule doit donner le repos, pas un
    // balancement — sinon le personnage termine chaque portage dans une
    // pose penchée, puisque c'est là que le ressort se pose.
    assert_eq!(niveau_balancier(-0.001), (Cote::Aucun, 0));
    assert_eq!(niveau_balancier(0.0), (Cote::Aucun, 0));
    assert_eq!(niveau_balancier(9.9), (Cote::Aucun, 0));
    assert_eq!(niveau_balancier(-9.9), (Cote::Aucun, 0));
}

#[test]
fn les_niveaux_du_balancier_suivent_les_seuils_de_shimeji_ee() {
    // Retard négatif = pied à gauche = curseur parti à droite.
    assert_eq!(niveau_balancier(-15.0), (Cote::PiedAGauche, 1));
    assert_eq!(niveau_balancier(-35.0), (Cote::PiedAGauche, 2));
    assert_eq!(niveau_balancier(-80.0), (Cote::PiedAGauche, 3));

    assert_eq!(niveau_balancier(15.0), (Cote::PiedADroite, 1));
    assert_eq!(niveau_balancier(35.0), (Cote::PiedADroite, 2));
    assert_eq!(niveau_balancier(80.0), (Cote::PiedADroite, 3));
}

#[test]
fn le_balancier_rattrape_le_curseur_et_s_y_arrete() {
    // Curseur fixe : le ressort doit converger, et ne pas osciller
    // éternellement.
    let (mut x, mut v) = (0.0f32, 0.0f32);
    for _ in 0..(60 * 3) {
        let (nx, nv) = integrer_balancier(x, v, 500.0, DT);
        x = nx;
        v = nv;
    }
    assert!((x - 500.0).abs() < 1.0, "pied à {x}, curseur à 500");
    assert!(v.abs() < 5.0, "il devrait s'être arrêté, v = {v}");
}

#[test]
fn le_retard_du_balancier_est_proportionnel_a_la_vitesse() {
    // C'est ce qui donne « plus c'est rapide, plus il est balancé ».
    // En régime permanent, retard = vitesse / (RAIDEUR/AMORTISSEMENT)
    // = vitesse / 10.
    let retard_a = |vitesse: f32| {
        let (mut x, mut v) = (0.0f32, 0.0f32);
        let mut curseur = 0.0f32;
        for _ in 0..(60 * 3) {
            curseur += vitesse * DT;
            let (nx, nv) = integrer_balancier(x, v, curseur, DT);
            x = nx;
            v = nv;
        }
        x - curseur
    };

    let lent = retard_a(100.0).abs();
    let vif = retard_a(500.0).abs();

    assert!(lent < vif, "retard lent {lent}, vif {vif}");

    // Le régime permanent théorique vaut `AMORTISSEMENT/RAIDEUR × v`,
    // soit `v/10` — 50 px à 500 px/s. Le mesuré est plus petit d'un pas
    // de curseur (`v × dt` = 8,3 px à 60 Hz), parce que la comparaison
    // se fait APRÈS avoir avancé le curseur — et c'est exactement l'ordre
    // dans lequel `avancer_balancier` opère.
    //
    // Conséquence assumée : à vitesse égale, le balancement est ~17 %
    // moins ample que celui de Shimeji-ee. Si cela paraissait mou à
    // l'usage, la correction propre serait de baisser les seuils de
    // `BALANCIER_SEUILS`, pas de tripoter le ressort.
    let theorique = 500.0 / 10.0 - 500.0 * DT;
    assert!(
        (vif - theorique).abs() < 3.0,
        "retard à 500 px/s = {vif}, attendu ~{theorique}"
    );
}

#[test]
fn sous_le_bureau_detecte_la_sortie_par_le_bas() {
    let monde = monde_deux_ecrans();
    assert!(!sous_le_bureau(&monde, Point::new(300.0, 500.0)));
    assert!(sous_le_bureau(&monde, Point::new(300.0, 5000.0)));
}

#[test]
fn sous_le_bureau_est_faux_dans_un_monde_vide() {
    // Pas de plateforme, donc pas de bas du bureau : on ne peut pas être
    // « sous » quelque chose qui n'existe pas. Surtout, ça ne doit pas
    // paniquer.
    let monde = World::from_screens(&[]);
    assert!(!sous_le_bureau(&monde, Point::new(0.0, 99_999.0)));
}


/// `HoldOntoWall` : `Duration="${500+Math.random()*1000}"`, en TICKS de 40 ms
/// comme toutes les durées de Shimeji-ee — donc 20 à 60 s. Lue en
/// millisecondes (0,5 à 1,5 s), il ne s'arrêtait jamais au mur (spec « menu
/// sur mesure » §6, défaut n° 1).
#[test]
fn la_duree_d_accroche_est_en_ticks_de_40_ms() {
    // `abs() < 1e-3` : 0,04 n'est pas exact en flottant.
    let ticks_en_s = |t: f32| t * 0.040;
    assert!((DUREE_ACCROCHE[0] - ticks_en_s(500.0)).abs() < 1e-3, "{DUREE_ACCROCHE:?}");
    assert!((DUREE_ACCROCHE[1] - ticks_en_s(1500.0)).abs() < 1e-3, "{DUREE_ACCROCHE:?}");
}
