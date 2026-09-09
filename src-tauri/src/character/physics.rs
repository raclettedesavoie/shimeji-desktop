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

// ── La chute, reprise de Shimeji-ee ────────────────────────────────────
//
// `src/com/group_finity/mascot/action/Fall.java` :
//
//     DEFAULT_GRAVITY     = 2      (px par tick, ajoutés à la vitesse)
//     DEFAULT_RESISTANCEY = 0.1    (fraction de la vitesse retirée par tick)
//     DEFAULT_RESISTANCEX = 0.05
//
//     vy = vy - vy * RESISTANCEY + GRAVITY
//     vx = vx - vx * RESISTANCEX
//
// **Il y a donc un frottement de l'air**, que la première version du projet
// n'avait pas — d'où une chute nettement trop rapide. La vitesse limite de
// Shimeji-ee est `GRAVITY / RESISTANCEY = 20` px/tick, soit **500 px/s**,
// contre 1 600 px/s de plafond dur ici. C'est un facteur 3.
//
// Conversion des constantes par tick (`TICK_INTERVAL = 40 ms`, donc
// 25 ticks/s) en constantes par seconde. En notant `V` la vitesse en px/s :
//
//     dV/dt = 25·GRAVITY/T − (RESISTANCEY/T)·V
//           = 1250 − 2,5·V
//
// La vitesse limite se retrouve bien : 1250 / 2,5 = 500 px/s.

/// Accélération de la pesanteur, en px/s².
///
/// `25 × GRAVITY / TICK_INTERVAL` = `25 × 2 / 0,04`.
pub const GRAVITE: f32 = 1250.0;

/// Frottement de l'air vertical, en s⁻¹. `RESISTANCEY / TICK_INTERVAL`.
///
/// C'est lui qui donne la vitesse limite, et lui qui manquait : sans
/// frottement, une chute de 1 000 px prenait 1,2 s au lieu de 2,3 s.
pub const FROTTEMENT_CHUTE_Y: f32 = 2.5;

/// Frottement horizontal, en s⁻¹. `RESISTANCEX / TICK_INTERVAL`.
///
/// L'élan horizontal d'un personnage lâché en marchant **décroît** donc.
/// La première version le conservait indéfiniment, ce qui donnait des
/// trajectoires trop plates.
pub const FROTTEMENT_CHUTE_X: f32 = 1.25;

/// Plafond dur de la vitesse de chute, en px/s.
///
/// **Garde-fou, jamais atteint en pratique** : le frottement plafonne
/// naturellement à ~500 px/s. Il ne sert que si un lâcher venait avec une
/// vitesse initiale énorme, et sa raison est technique — la détection
/// d'atterrissage teste le **segment** parcouru dans une image, et un
/// segment de plusieurs milliers de pixels enjamberait plusieurs
/// plateformes, rendant le choix arbitraire.
///
/// À 60 Hz, 900 px/s vaut 15 px par image : bien moins que la moindre
/// plateforme.
pub const VITESSE_CHUTE_MAX: f32 = 900.0;

/// Vitesse de marche.
///
/// **Reprise de Shimeji-ee, pas réglée à l'œil** : l'action `Walk` de
/// `conf/actions.xml` porte `Velocity="-2,0"`, soit 2 px par tick, et
/// `Manager.TICK_INTERVAL = 40 ms` — donc 50 px/s.
pub const VITESSE_MARCHE: f32 = 50.0;

/// Vitesse de course. `Run` porte `Velocity="-4,0"` → 100 px/s.
///
/// Le rapport à la marche est donc exactement **2×**, et c'est ce qui rend
/// le passage de l'une à l'autre lisible. Shimeji-ee a aussi un `Dash` à
/// `-8,0` (200 px/s) qu'on n'utilise pas encore.
pub const VITESSE_COURSE: f32 = 100.0;

// ── Le balancier du personnage porté ──────────────────────────────────
//
// **Ce n'est pas une animation, c'est un ressort amorti.** Découvert dans
// `action/Dragged.java` :
//
//     footDx = (footDx + (curseurX − footX) × 0,1) × 0,8
//     footX += footDx
//
// Un point « pied » poursuit le curseur avec un ressort (raideur 0,1) et un
// amortissement (facteur 0,8 par tick). Son **retard** sur le curseur choisit
// ensuite la frame, par les conditions de l'action `Pinched`.
//
// Trois propriétés en découlent, gratuitement, et ce sont exactement les
// trois qui manquaient :
//   · l'amplitude croît avec la vitesse du curseur ;
//   · le retour au repos passe par les frames intermédiaires ;
//   · le système est sous-amorti, donc il oscille un peu avant de se poser.
//
// Conversion des constantes par tick en constantes par seconde, en notant
// `x` la position du pied et `V` sa vitesse en px/s :
//
//     dV/dt = RAIDEUR·(curseur − x) − AMORTISSEMENT·V

/// Raideur du ressort du balancier, en s⁻². `0,08 / TICK_INTERVAL²`.
pub const BALANCIER_RAIDEUR: f32 = 50.0;

/// Amortissement du balancier, en s⁻¹. `0,2 / TICK_INTERVAL`.
///
/// Le rapport d'amortissement vaut `5 / (2·√50) ≈ 0,35` : **sous-amorti**.
/// C'est voulu — un système critique reviendrait au repos sans osciller, et
/// on perdrait le balancement qui fait tout l'intérêt.
pub const BALANCIER_AMORTISSEMENT: f32 = 5.0;

/// Les trois seuils de retard, en pixels, qui choisissent la frame.
///
/// Repris tels quels des conditions de l'action `Pinched` : `±10`, `±30`,
/// `±50`. Ils sont en pixels et non en vitesse, et c'est cohérent — à vitesse
/// constante `s`, le régime permanent du ressort donne un retard de `s/10`,
/// donc 300 px/s de glisser produisent 30 px de retard.
pub const BALANCIER_SEUILS: [f32; 3] = [10.0, 30.0, 50.0];

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
    // Pesanteur **moins frottement**, exactement comme `Fall.java`. Le
    // frottement est proportionnel à la vitesse : c'est lui qui plafonne
    // naturellement la chute à ~500 px/s, sans plafond dur.
    //
    // `min` et non `clamp` : seule la chute est plafonnée. Une vitesse
    // ascendante (personnage lâché vers le haut) n'a pas de raison de l'être.
    let vy = (vel.y + (GRAVITE - FROTTEMENT_CHUTE_Y * vel.y) * dt).min(VITESSE_CHUTE_MAX);

    // L'élan horizontal décroît aussi (`RESISTANCEX = 0,05`). Sans ça, un
    // personnage lâché en courant garderait sa vitesse jusqu'au sol et la
    // trajectoire paraîtrait plate.
    let vx = vel.x - FROTTEMENT_CHUTE_X * vel.x * dt;

    let nouvelle_vel = Vec2::new(vx, vy);
    let nouvelle_pos = Point::new(pos.x + nouvelle_vel.x * dt, pos.y + nouvelle_vel.y * dt);

    (nouvelle_pos, nouvelle_vel)
}

/// Un pas du ressort amorti du balancier de portage.
///
/// Rend la nouvelle position et la nouvelle vitesse du point « pied », qui
/// poursuit `curseur_x`. Voir le bandeau de constantes ci-dessus pour le
/// modèle et sa provenance.
///
/// Fonction pure, comme `integrer_chute` : c'est ce qui permet de tester le
/// balancier sans souris, sans fenêtre et sans personnage.
pub fn integrer_balancier(pied_x: f32, pied_vx: f32, curseur_x: f32, dt: f32) -> (f32, f32) {
    // Euler semi-implicite ici aussi : la vitesse avant la position. Sur un
    // ressort, l'explicite peut carrément diverger si le pas est grand.
    let acceleration = BALANCIER_RAIDEUR * (curseur_x - pied_x) - BALANCIER_AMORTISSEMENT * pied_vx;
    let nouvelle_vx = pied_vx + acceleration * dt;
    let nouvelle_x = pied_x + nouvelle_vx * dt;
    (nouvelle_x, nouvelle_vx)
}

/// Quel **niveau** de balancement afficher, d'après le retard du pied sur le
/// curseur.
///
/// Rend `(cote, niveau)` où `cote` est le signe du retard et `niveau` va de 0
/// (au repos) à 3 (balancement maximal).
///
/// Les seuils `±10`, `±30`, `±50` viennent des conditions de l'action
/// `Pinched`.
///
/// > ⚠️ **Un écart volontaire, et c'est une correction de bug.** Dans le XML
/// > de Shimeji-ee, les conditions sont évaluées dans l'ordre et
/// > `FootX < cursor.x` (soit `d < 0`) est testée **avant** la bande neutre
/// > `-10 < d < +10`. Cette bande est donc **inatteignable pour un retard
/// > négatif** : un pied immobilisé un dixième de pixel à gauche du curseur
/// > garde la pose 5 indéfiniment, au lieu de pendre droit.
/// >
/// > Ce n'est pas théorique : c'est exactement là que le ressort se pose
/// > après avoir oscillé, donc le personnage terminait **tout portage** dans
/// > une pose légèrement penchée. On teste donc la bande neutre **en
/// > premier**, et symétriquement — ce qui est manifestement l'intention de
/// > la condition `±10` d'origine.
pub fn niveau_balancier(retard: f32) -> (Cote, u8) {
    // La bande neutre d'abord, et symétrique. Voir l'avertissement ci-dessus.
    if retard.abs() < BALANCIER_SEUILS[0] {
        return (Cote::Aucun, 0);
    }

    if retard < 0.0 {
        if retard <= -BALANCIER_SEUILS[2] {
            (Cote::PiedAGauche, 3)
        } else if retard <= -BALANCIER_SEUILS[1] {
            (Cote::PiedAGauche, 2)
        } else {
            (Cote::PiedAGauche, 1)
        }
    } else if retard >= BALANCIER_SEUILS[2] {
        (Cote::PiedADroite, 3)
    } else if retard >= BALANCIER_SEUILS[1] {
        (Cote::PiedADroite, 2)
    } else {
        (Cote::PiedADroite, 1)
    }
}

/// De quel côté le pied traîne.
///
/// **Attention au sens, c'est le point qui a été inversé une fois :** le pied
/// traîne à gauche quand le curseur va à **droite**. Et comme le pied part à
/// gauche, la **tête penche à droite**. C'est un pendule, il traîne derrière.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cote {
    /// Retard négatif : `footX < curseurX`. Curseur vers la droite, tête à
    /// droite. Frames 5, 7, 9 chez Shimeji-ee.
    PiedAGauche,
    /// Retard positif. Curseur vers la gauche, tête à gauche. Frames 6, 8, 10.
    PiedADroite,
    /// Zone neutre : il pend droit. Frame 1.
    Aucun,
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
}
