//! La physique : chute, atterrissage, et les vitesses de déplacement.
//!
//! Responsabilité unique : des **fonctions pures** sur des points et des
//! vitesses. Aucune notion de personnage, aucun état. C'est ce qui permet de
//! tester une chute « hauteur et durée connues » (spec §10.1) sans instancier
//! quoi que ce soit.
//!
//! Toutes les vitesses sont en **pixels physiques par seconde**, toutes les
//! accélérations en pixels par seconde carrée, et `y` croît **vers le bas**
//! (convention Windows, voir `geom::Rect`).

use crate::geom::{Face, Point, Vec2};
use crate::world::{PlatformId, World};

/// Accélération de la pesanteur.
///
/// Réglée à l'œil, pas dérivée du réel : à 9,81 m/s² et ~3 800 px/m, un
/// personnage tomberait de 1 000 px en 0,45 s — trop vif pour qu'on suive la
/// chute du regard. 1 400 px/s² donne ~1,2 s sur la même hauteur, ce qui se
/// regarde. Passera dans `config.json` au plan 1b.
pub const GRAVITE: f32 = 1400.0;

/// Vitesse de chute maximale.
///
/// Existe pour une raison technique, pas esthétique : la détection
/// d'atterrissage teste le **segment** parcouru dans une image. Sans plafond,
/// un personnage lâché très haut parcourrait plusieurs milliers de pixels par
/// image, enjamberait plusieurs plateformes d'un coup, et le choix de celle
/// sur laquelle atterrir deviendrait arbitraire.
///
/// À 60 Hz, 1 600 px/s vaut ~27 px par image : bien moins que la moindre
/// plateforme.
pub const VITESSE_CHUTE_MAX: f32 = 1600.0;

/// Vitesse de marche. Lente exprès : un pet qui se presse n'a pas l'air de
/// flâner.
pub const VITESSE_MARCHE: f32 = 55.0;

/// Vitesse de course. Le rapport à la marche (~2,7×) est ce qui rend le
/// passage de l'une à l'autre visible.
pub const VITESSE_COURSE: f32 = 150.0;

/// Un pas d'intégration de la chute libre.
///
/// **Euler semi-implicite** : on met à jour la vitesse *avant* la position.
/// C'est une ligne de différence avec Euler explicite, et c'est bien plus
/// stable — la trajectoire ne dérive pas quand le pas de temps varie un peu,
/// ce qui arrive dès que la machine est chargée.
///
/// Fonction pure : elle rend le nouvel état au lieu de modifier l'ancien.
/// C'est ce qui la rend testable en boucle dans un test, comme ci-dessous.
pub fn integrer_chute(pos: Point, vel: Vec2, dt: f32) -> (Point, Vec2) {
    // `min` et non `clamp` : seule la chute est plafonnée. Une vitesse
    // ascendante (personnage lâché vers le haut) n'a pas de raison de l'être.
    let vy = (vel.y + GRAVITE * dt).min(VITESSE_CHUTE_MAX);

    // La vitesse horizontale n'est pas amortie : le personnage garde son
    // élan. C'est ce qui rend le lâcher agréable plutôt que raide.
    let nouvelle_vel = Vec2::new(vel.x, vy);

    let nouvelle_pos = Point::new(pos.x + nouvelle_vel.x * dt, pos.y + nouvelle_vel.y * dt);

    (nouvelle_pos, nouvelle_vel)
}

/// Le personnage a-t-il touché une plateforme en passant de `avant` à
/// `apres` ?
///
/// Rend la plateforme et l'offset où se poser, ou `None` s'il continue de
/// tomber.
///
/// **Trois règles, et pas une de plus** :
///   1. on ne s'accroche qu'en **descendant** — sinon on s'agripperait au
///      sol qu'on traverse par-dessous ;
///   2. on retient la face **la plus haute** traversée — c'est la première
///      rencontrée en descendant ;
///   3. il faut être **au-dessus** de la face horizontalement.
///
/// La règle 2 ne sert à rien à l'étape 1 (les sols des écrans ne se
/// chevauchent pas) mais elle sera exactement ce qu'il faut à l'étape 4, où
/// une barre de titre flotte au-dessus du sol. L'écrire maintenant coûte deux
/// lignes ; l'oublier coûterait un diagnostic.
pub fn atterrissage(world: &World, avant: Point, apres: Point) -> Option<(PlatformId, f32)> {
    // Règle 1 : on descend ? `<` et non `<=` pour accepter le cas où le
    // personnage était déjà pile sur la ligne, sans mouvement vertical.
    if apres.y < avant.y {
        return None;
    }

    let mut meilleur: Option<(PlatformId, f32, f32)> = None; // (id, offset, y de la face)

    for plat in world.platforms() {
        if !plat.has_face(Face::Top) {
            continue;
        }

        let y_face = plat.rect.top();

        // A-t-on franchi cette ligne pendant le pas ?
        let franchie = avant.y <= y_face && apres.y >= y_face;
        if !franchie {
            continue;
        }

        // Règle 3 : est-on au-dessus de la face ?
        //
        // On teste avec `apres.x`, et non avec la position exacte du
        // croisement. C'est volontairement approximatif : à 27 px par image
        // au maximum, l'écart est invisible, et calculer l'intersection
        // exacte ajouterait de la trigonométrie pour rien. La navigation a
        // le droit d'être imparfaite (décision n° 4).
        if apres.x < plat.rect.left() || apres.x > plat.rect.right() {
            continue;
        }

        // Règle 2 : garder la face la plus haute, donc le plus petit `y`.
        let remplace = match meilleur {
            None => true,
            Some((_, _, y)) => y_face < y,
        };
        if remplace {
            meilleur = Some((plat.id, apres.x - plat.rect.left(), y_face));
        }
    }

    meilleur.map(|(id, offset, _)| (id, offset))
}

/// Le personnage est-il tombé sous le bas du bureau virtuel ?
///
/// C'est le déclencheur du **garde-fou** de la spec §6.3 : passé cette
/// limite, il est replacé sur le sol le plus proche plutôt que de tomber
/// indéfiniment. Sans ce garde-fou, lâcher un personnage à côté de l'écran
/// le perdrait pour de bon.
///
/// Rend `false` si le monde est vide : on ne peut pas être « sous » un bas
/// qui n'existe pas, et surtout il ne faut pas paniquer.
pub fn sous_le_bureau(world: &World, pos: Point) -> bool {
    // `let … else` : monde vide → pas de limite, donc pas de sortie.
    let Some(b) = world.bounds() else {
        return false;
    };

    // Une marge, pour que le garde-fou ne se déclenche pas pendant un
    // atterrissage normal juste au niveau du sol.
    const MARGE: f32 = 200.0;
    pos.y > b.bottom() + MARGE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Rect;
    use crate::probe::fake::FakeProbe;
    use crate::probe::{ScreenInfo, SystemProbe};

    fn monde_deux_ecrans() -> World {
        World::from_screens(&FakeProbe::deux_ecrans().screens())
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
    fn la_chute_conserve_la_vitesse_horizontale() {
        // Un personnage lâché en marchant garde son élan : c'est ce qui rend
        // le lâcher agréable plutôt que raide.
        let (pos, vel) = integrer_chute(Point::new(100.0, 0.0), Vec2::new(80.0, 0.0), DT);
        assert_eq!(vel.x, 80.0);
        assert!(pos.x > 100.0);
    }

    #[test]
    fn la_chute_est_deterministe_en_hauteur_et_en_duree() {
        // Le test que la spec §10.1 demande : « intégration déterministe,
        // hauteur et durée connues ».
        //
        // On intègre 1 seconde à 60 Hz depuis l'immobilité et on vérifie que
        // la distance parcourue est proche de ½·g·t² = 700 px. L'écart vient
        // du pas discret (Euler semi-implicite surestime légèrement), d'où
        // la tolérance.
        let mut pos = Point::new(0.0, 0.0);
        let mut vel = Vec2::zero();

        for _ in 0..60 {
            let (p, v) = integrer_chute(pos, vel, DT);
            pos = p;
            vel = v;
        }

        let theorique = 0.5 * GRAVITE * 1.0;
        assert!(
            (pos.y - theorique).abs() < theorique * 0.03,
            "chute de {} px, théorie {} px",
            pos.y,
            theorique
        );
    }

    #[test]
    fn la_vitesse_de_chute_est_plafonnee() {
        // Sans plafond, un personnage lâché très haut traverserait le sol
        // entre deux images : la détection d'atterrissage teste un segment,
        // mais un segment de 3 000 px enjamberait plusieurs plateformes et
        // rendrait le choix arbitraire.
        let mut vel = Vec2::new(0.0, 0.0);
        let mut pos = Point::new(0.0, 0.0);
        for _ in 0..600 {
            let (p, v) = integrer_chute(pos, vel, DT);
            pos = p;
            vel = v;
        }
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
                scale: 1.0,
            },
            ScreenInfo {
                id: 2,
                // Un écran fictif dont la zone de travail finit plus haut :
                // son sol est donc à y = 600.
                work_area: Rect::new(0.0, 0.0, 1920.0, 600.0),
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
}
