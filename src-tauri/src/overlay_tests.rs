//! Les tests de `overlay` — la répartition des sprites par écran et la
//! fabrication du JavaScript.
//!
//! Ils tournent **sans écran et sans Tauri** : c'est tout l'intérêt d'avoir
//! sorti cette logique dans un module qui ne connaît ni fenêtre ni `eval`.
use super::*;
use crate::geom::Rect;
use crate::probe::ScreenInfo;

/// Deux écrans côte à côte, 1920×1080 chacun. On prend la zone de travail
/// égale à l'écran entier : la barre des tâches ne change rien à la
/// répartition, et l'ôter ici rendrait chaque attente illisible.
fn deux_ecrans() -> Vec<ScreenInfo> {
    vec![
        ScreenInfo {
            id: 1,
            work_area: Rect::new(0.0, 0.0, 1920.0, 1080.0),
            scale: 1.0,
        },
        ScreenInfo {
            id: 2,
            work_area: Rect::new(1920.0, 0.0, 1920.0, 1080.0),
            scale: 1.0,
        },
    ]
}

fn sprite(id: u32, x: i32, y: i32) -> SpriteRendu {
    SpriteRendu {
        id,
        x,
        y,
        w: 128,
        h: 128,
        image: 1,
        flip: false,
    }
}

#[test]
fn un_sprite_va_sur_l_ecran_qui_le_contient() {
    let charges = repartir(&[sprite(7, 100, 200)], &deux_ecrans());
    assert_eq!(charges.len(), 1);
    assert_eq!(charges[0].ecran, 1);
    assert_eq!(charges[0].sprites.len(), 1);
    assert_eq!(charges[0].sprites[0].id, 7);
}

#[test]
fn les_coordonnees_sont_relatives_a_l_ecran() {
    // Sur le second écran, qui commence à x = 1920 : un sprite posé à 2000
    // doit être dessiné à 80 dans SA fenêtre, pas à 2000. C'est la seule
    // conversion de repère du module, donc la seule occasion de se tromper.
    let charges = repartir(&[sprite(1, 2000, 300)], &deux_ecrans());
    assert_eq!(charges[0].ecran, 2);
    assert_eq!(charges[0].sprites[0].x, 80);
    assert_eq!(charges[0].sprites[0].y, 300);
}

#[test]
fn un_sprite_a_cheval_est_emis_dans_les_deux_ecrans() {
    // Conception §5.2 : posé à x = 1860, il déborde de 68 px sur le second
    // écran. Sans cette règle il serait COUPÉ net au bord — une régression
    // sur le multi-écran, qui est l'un des trois arguments ayant fait choisir
    // cette architecture.
    let charges = repartir(&[sprite(3, 1860, 100)], &deux_ecrans());
    assert_eq!(charges.len(), 2);

    let gauche = charges.iter().find(|c| c.ecran == 1).expect("écran 1 absent");
    let droite = charges.iter().find(|c| c.ecran == 2).expect("écran 2 absent");

    assert_eq!(gauche.sprites[0].x, 1860);
    // Relatif au second écran : 1860 - 1920 = -60. La coordonnée est
    // NÉGATIVE, et c'est voulu : le sprite entre par la gauche, sa moitié
    // gauche étant dessinée par la fenêtre voisine.
    assert_eq!(droite.sprites[0].x, -60);
    assert_eq!(droite.sprites[0].id, 3);
}

#[test]
fn un_ecran_sans_sprite_n_a_pas_de_charge() {
    // Conception §5.1 : un écran vide ne doit pas exister comme fenêtre, donc
    // pas apparaître ici non plus. C'est ce qui évite de payer le péage de
    // ~34 % sur un écran où il ne se passe rien.
    let charges = repartir(&[sprite(1, 10, 10)], &deux_ecrans());
    assert!(charges.iter().all(|c| c.ecran != 2));
}

#[test]
fn un_sprite_hors_de_tout_ecran_n_est_emis_nulle_part() {
    // Peut arriver le temps d'une image, quand une plateforme disparaît sous
    // un personnage : on ne veut ni panique, ni charge fantôme.
    let charges = repartir(&[sprite(1, 9000, 9000)], &deux_ecrans());
    assert!(charges.is_empty());
}

// ── `js_de` : la charge utile envoyée au webview ────────────────────────

#[test]
fn le_js_appelle_poser_tous_avec_un_tableau_de_tableaux() {
    let charge = ChargeEcran {
        ecran: 1,
        sprites: vec![SpriteRelatif {
            id: 4,
            x: 10,
            y: 20,
            w: 128,
            h: 128,
            image: 7,
            flip: false,
        }],
    };
    assert_eq!(js_de(&charge), "window.poserTous([[4,10,20,128,128,7,0]])");
}

#[test]
fn le_flip_est_un_entier_et_non_un_booleen() {
    // `0`/`1` au lieu de `false`/`true` : quatre caractères de moins par
    // sprite, sur une chaîne refabriquee 45 fois par seconde.
    let charge = ChargeEcran {
        ecran: 1,
        sprites: vec![SpriteRelatif {
            id: 1,
            x: 0,
            y: 0,
            w: 64,
            h: 64,
            image: 2,
            flip: true,
        }],
    };
    assert!(js_de(&charge).ends_with(",1]])"));
}

#[test]
fn plusieurs_sprites_sont_separes_par_une_virgule() {
    let charge = ChargeEcran {
        ecran: 1,
        sprites: vec![
            SpriteRelatif {
                id: 1,
                x: 0,
                y: 0,
                w: 1,
                h: 2,
                image: 3,
                flip: false,
            },
            SpriteRelatif {
                id: 2,
                x: -5,
                y: 6,
                w: 7,
                h: 8,
                image: 9,
                flip: true,
            },
        ],
    };
    assert_eq!(
        js_de(&charge),
        "window.poserTous([[1,0,0,1,2,3,0],[2,-5,6,7,8,9,1]])"
    );
}

#[test]
fn une_charge_vide_ne_produit_pas_de_javascript_casse() {
    // `repartir` n'en produit jamais, mais `js_de` est publique : une charge
    // vide doit rendre un appel VALIDE, pas `window.poserTous([)`.
    let charge = ChargeEcran {
        ecran: 1,
        sprites: Vec::new(),
    };
    assert_eq!(js_de(&charge), "window.poserTous([])");
}
