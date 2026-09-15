//! Les tests de `apparition` — où naît un personnage qui s'active.
use super::*;
use crate::character::physics;
use crate::probe::fake::FakeProbe;
use crate::probe::SystemProbe;
use crate::world;

fn monde_d_un_ecran() -> world::World {
    let sonde = FakeProbe::un_ecran();
    world::World::from_screens(&sonde.screens())
}

#[test]
fn il_nait_en_chute_libre_et_pas_accroche() {
    // C'est TOUT le point : les réflexes à 60 Hz gèrent déjà la chute et
    // l'atterrissage, donc naître en `Falling` suffit (design §5).
    let monde = monde_d_un_ecran();
    let mut rng = XorShift32::seeded(12345);
    let att = point_de_chute(&monde, 128.0, &mut rng).expect("un écran existe");
    assert!(matches!(att, Attachment::Falling { .. }));
}

#[test]
fn il_nait_sans_vitesse() {
    // Une vitesse initiale non nulle le ferait apparaître en train de
    // filer : la gravité des réflexes suffit à l'accélérer.
    let monde = monde_d_un_ecran();
    let mut rng = XorShift32::seeded(999);
    let Attachment::Falling { vel, .. } = point_de_chute(&monde, 128.0, &mut rng).expect("un écran")
    else {
        panic!("attendu Falling");
    };
    assert_eq!(vel.x, 0.0);
    assert_eq!(vel.y, 0.0);
}

#[test]
fn il_nait_en_haut_de_la_zone_de_travail() {
    // Pas au milieu, pas en bas : « il tombe du HAUT de l'écran ».
    let monde = monde_d_un_ecran();
    let mut rng = XorShift32::seeded(55);
    let sol = monde.premier_sol().expect("un sol");

    let Attachment::Falling { pos, .. } = point_de_chute(&monde, 128.0, &mut rng).expect("un écran")
    else {
        panic!("attendu Falling");
    };

    // Le sol est en BAS (son `top()` est la ligne de marche) : apparaître
    // au-dessus veut dire un `y` strictement plus petit, et de beaucoup.
    assert!(
        pos.y < sol.rect.top() - 500.0,
        "apparu en y = {}, alors que le sol est en y = {}",
        pos.y,
        sol.rect.top()
    );
}

#[test]
fn le_x_reste_dans_la_zone_de_travail_marges_comprises() {
    // Sans marge il apparaîtrait à moitié hors champ sur les bords.
    //
    // ⚠️ Le RNG est semé UNE FOIS et on le laisse avancer sur les 500
    // tirages : le re-semer avec 0, 1, 2… biaiserait chaque premier tirage
    // vers les bits de poids faible, et cette boucle mesurerait 500 fois la
    // même chose (piège documenté dans CLAUDE.md).
    let monde = monde_d_un_ecran();
    let mut rng = XorShift32::seeded(7);
    let sol = monde.premier_sol().expect("un sol");
    let demi = 64.0;

    for _ in 0..500 {
        let Attachment::Falling { pos, .. } =
            point_de_chute(&monde, 128.0, &mut rng).expect("un écran")
        else {
            panic!("attendu Falling");
        };
        assert!(pos.x >= sol.rect.left() + demi, "x = {} trop à gauche", pos.x);
        assert!(pos.x <= sol.rect.right() - demi, "x = {} trop à droite", pos.x);
    }
}

#[test]
fn le_x_varie_reellement_d_une_apparition_a_l_autre() {
    // « à un x aléatoire » : si la fonction rendait toujours le même point,
    // les trois tests ci-dessus passeraient quand même.
    let monde = monde_d_un_ecran();
    let mut rng = XorShift32::seeded(4242);
    let mut vus = std::collections::BTreeSet::new();

    for _ in 0..50 {
        let Attachment::Falling { pos, .. } =
            point_de_chute(&monde, 128.0, &mut rng).expect("un écran")
        else {
            panic!("attendu Falling");
        };
        vus.insert(pos.x as i32);
    }
    assert!(vus.len() > 20, "seulement {} x distincts sur 50", vus.len());
}

#[test]
fn il_ne_s_agrippe_pas_au_plafond_en_apparaissant() {
    // ⚠️ LE VRAI RISQUE de ce module (design §5).
    //
    // La plateforme « plafond » est posée JUSTE au-dessus de la zone de
    // travail et expose sa face `Bottom`, celle à laquelle on se suspend.
    // Un personnage qui apparaît au sommet naît donc à quelques pixels
    // d'une surface accrochable — et depuis le 2026-09-12, le plafond
    // ATTRAPE, par divergence assumée de Shimeji-ee. S'il s'y accrochait,
    // tous les personnages apparaîtraient collés au plafond au lieu de
    // tomber.
    //
    // Ce qui nous sauve est que `contact_plafond` ne s'accroche qu'en
    // MONTANT. Ce test verrouille cette propriété : quelqu'un qui
    // assouplirait un jour cette condition casserait l'apparition, et
    // l'apprendrait ici plutôt qu'à l'écran.
    //
    // ⚠️ `contact` prend **deux positions** — l'avant et l'après d'une
    // image — et non une position et une vitesse : c'est un test de
    // trajectoire balayée, pas de point.
    let monde = monde_d_un_ecran();
    let mut rng = XorShift32::seeded(31337);

    // 200 apparitions, parce qu'un seul tirage tomberait peut-être sur un
    // x que rien ne couvre.
    for _ in 0..200 {
        let Attachment::Falling { pos, .. } =
            point_de_chute(&monde, 128.0, &mut rng).expect("un écran")
        else {
            panic!("attendu Falling");
        };

        // La première image de chute : la gravité appliquée sur 1/60 s, puis
        // le déplacement sur 1/60 s. Exactement ce que fait la boucle.
        let dt = 1.0 / 60.0;
        let apres = Point::new(pos.x, pos.y + physics::GRAVITE * dt * dt);
        assert!(
            physics::contact(&monde, pos, apres).is_none(),
            "il s'accroche dès l'apparition en ({}, {}) au lieu de tomber",
            pos.x,
            pos.y
        );
    }
}

#[test]
fn un_monde_sans_ecran_rend_none_sans_paniquer() {
    // Cas réel : session distante en cours d'établissement.
    let monde = world::World::from_screens(&[]);
    let mut rng = XorShift32::seeded(1);
    assert!(point_de_chute(&monde, 128.0, &mut rng).is_none());
}

#[test]
fn les_deux_ecrans_sont_tous_deux_atteignables() {
    // « il apparaît sur un écran tiré au sort » : avec deux écrans, les
    // deux doivent sortir. Sinon le second moniteur ne verrait jamais
    // personne apparaître — un défaut qu'aucun test à un écran ne voit.
    let sonde = FakeProbe::deux_ecrans();
    let monde = world::World::from_screens(&sonde.screens());
    let mut rng = XorShift32::seeded(2024);

    let mut gauche = 0;
    let mut droite = 0;
    for _ in 0..200 {
        let Attachment::Falling { pos, .. } =
            point_de_chute(&monde, 128.0, &mut rng).expect("deux écrans")
        else {
            panic!("attendu Falling");
        };
        if pos.x < 1920.0 {
            gauche += 1;
        } else {
            droite += 1;
        }
    }
    assert!(gauche > 10, "seulement {gauche} apparitions à gauche sur 200");
    assert!(droite > 10, "seulement {droite} apparitions à droite sur 200");
}
