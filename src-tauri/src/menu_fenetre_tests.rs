//! Les tests de `menu_fenetre` : le placement du menu, sans écran.

use super::*;
use crate::geom::Rect;

/// Un écran 1920×1080 à l'origine.
fn ecran() -> Rect {
    Rect::new(0.0, 0.0, 1920.0, 1080.0)
}

#[test]
fn au_milieu_le_coin_du_menu_est_sous_le_curseur() {
    // 200×300 CSS à l'échelle 1 : la fenêtre ajoute la marge de l'ombre
    // de chaque côté, et se décale d'autant pour que le MENU, lui, parte
    // du curseur.
    let (x, y, w, h) = rectangle((500, 400), (200.0, 300.0), 1.0, ecran());
    let m = MARGE_CSS as i32;
    assert_eq!((x, y), (500 - m, 400 - m));
    assert_eq!((w, h), (200 + 2 * m as u32, 300 + 2 * m as u32));
}

#[test]
fn le_menu_se_replie_dans_l_ecran() {
    // Review Focus n° 1 : près du coin bas-droit, il s'ouvre vers le haut
    // et vers la gauche, et reste entièrement dans l'écran.
    let (x, y, w, h) = rectangle((1900, 1070), (200.0, 300.0), 1.0, ecran());
    assert!(x >= 0 && y >= 0, "({x}, {y})");
    assert!(x + w as i32 <= 1920, "déborde à droite : {x} + {w}");
    assert!(y + h as i32 <= 1080, "déborde en bas : {y} + {h}");
}

#[test]
fn la_taille_suit_l_echelle_de_l_ecran() {
    // Review Focus n° 2 : 200 px CSS à 125 % = 250 px physiques.
    let (_, _, w, _) = rectangle((500, 400), (200.0, 300.0), 1.25, ecran());
    let attendu = ((200.0 + 2.0 * MARGE_CSS) * 1.25).ceil() as u32;
    assert_eq!(w, attendu);
}

#[test]
fn un_ecran_a_droite_du_premier_garde_son_origine() {
    let droite = Rect::new(1920.0, 0.0, 1920.0, 1080.0);
    let (x, _, _, _) = rectangle((2000, 400), (200.0, 300.0), 1.0, droite);
    assert!(x >= 1920, "le menu a glissé sur l'écran voisin : {x}");
}

#[test]
fn un_clic_hors_du_menu_est_detecte() {
    // Review Focus n° 3 : le filet quand `blur` ne vient jamais.
    let rect = (100, 100, 200, 300);
    assert!(!hors_du_menu((150, 150), rect));
    assert!(hors_du_menu((99, 150), rect));
    assert!(hors_du_menu((150, 401), rect));
}

#[test]
fn un_menu_jamais_place_est_abandonne() {
    // Relecture finale, n° 1 : si `menu.js` ne rappelle jamais `placer_menu`
    // (page encore en chargement, exception JS), la fenêtre reste cachée —
    // ni `blur`, ni Échap, ni le filet. Sans ce délai, le personnage cliqué
    // restait figé et le clic droit mort jusqu'au redémarrage.
    use std::time::Duration;
    let ouvert = Duration::from_secs(10);
    assert!(!est_abandonne(ouvert, ouvert + Duration::from_millis(500), false));
    assert!(est_abandonne(ouvert, ouvert + DELAI_PLACEMENT + Duration::from_millis(1), false));
    // Placé : il reste ouvert tant qu'on ne le ferme pas, si longtemps
    // que l'utilisateur hésite.
    assert!(!est_abandonne(ouvert, ouvert + Duration::from_secs(600), true));
}
