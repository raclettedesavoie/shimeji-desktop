//! Sonde de test : des écrans et une souris que le test décide.
//!
//! Existe pour satisfaire la troisième contrainte de la spec §10.2. Les
//! constructeurs de commodité reproduisent des topologies nommées, ce qui
//! rend les attentes des tests lisibles sans commentaire.

use super::{MouseState, ScreenInfo, SystemProbe};
use crate::geom::{Point, Rect};
use std::cell::Cell;

pub struct FakeProbe {
    screens: Vec<ScreenInfo>,
    // `Cell` pour la même raison que dans `FakeClock` : le trait expose
    // `&self`, donc un test qui n'a qu'une référence partagée doit pouvoir
    // bouger la souris. `MouseState` est `Copy`, ce que `Cell` exige.
    mouse: Cell<MouseState>,
}

impl FakeProbe {
    pub fn new(screens: Vec<ScreenInfo>) -> Self {
        FakeProbe {
            screens,
            mouse: Cell::new(MouseState {
                pos: Point::new(0.0, 0.0),
                left_down: false,
            }),
        }
    }

    /// Un seul écran 1920×1080 dont la zone de travail exclut 48 px de barre
    /// des tâches. Le sol est donc à y = 1032, et non 1080 : c'est le piège
    /// Windows n° 3, rendu explicite dans les tests.
    pub fn un_ecran() -> Self {
        Self::new(vec![ScreenInfo {
            id: 1,
            work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
            scale: 1.0,
        }])
    }

    /// La topologie réelle relevée à l'étape 0 : deux 1920×1080 côte à côte,
    /// échelle 1, le second à x = 1920.
    pub fn deux_ecrans() -> Self {
        Self::new(vec![
            ScreenInfo {
                id: 1,
                work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
                scale: 1.0,
            },
            ScreenInfo {
                id: 2,
                work_area: Rect::new(1920.0, 0.0, 1920.0, 1032.0),
                scale: 1.0,
            },
        ])
    }

    /// Deux écrans dont le second est **à gauche**, donc à `x` négatif, et à
    /// l'**échelle 2**.
    ///
    /// C'est la topologie que la machine de développement ne peut pas
    /// produire : elle n'a que des écrans à l'échelle 1, rangés vers la
    /// droite. Le multi-DPI est la seule inconnue laissée ouverte par
    /// l'étape 0 — on ne peut pas l'observer, mais ce constructeur permet au
    /// moins de ne pas coder contre elle.
    pub fn ecran_a_gauche_hidpi() -> Self {
        Self::new(vec![
            ScreenInfo {
                id: 1,
                work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
                scale: 1.0,
            },
            ScreenInfo {
                id: 2,
                work_area: Rect::new(-2560.0, 0.0, 2560.0, 1392.0),
                scale: 2.0,
            },
        ])
    }

    pub fn set_mouse(&self, pos: Point, left_down: bool) {
        self.mouse.set(MouseState { pos, left_down });
    }
}

impl SystemProbe for FakeProbe {
    fn screens(&self) -> Vec<ScreenInfo> {
        self.screens.clone()
    }

    fn mouse(&self) -> MouseState {
        self.mouse.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deux_ecrans_reproduit_la_topologie_de_l_etape_0() {
        let p = FakeProbe::deux_ecrans();
        let e = p.screens();
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].work_area.left(), 0.0);
        assert_eq!(e[1].work_area.left(), 1920.0);
        // Le sol est la zone de travail, pas l'écran : 1032 et non 1080.
        assert_eq!(e[0].work_area.bottom(), 1032.0);
    }

    #[test]
    fn la_souris_se_deplace_a_travers_une_reference_partagee() {
        let p = FakeProbe::un_ecran();
        let vue: &dyn SystemProbe = &p;
        p.set_mouse(Point::new(300.0, 400.0), true);
        let m = vue.mouse();
        assert_eq!(m.pos, Point::new(300.0, 400.0));
        assert!(m.left_down);
    }

    #[test]
    fn une_topologie_a_x_negatif_est_representable() {
        // Contrainte globale : ne jamais supposer x >= 0.
        let p = FakeProbe::ecran_a_gauche_hidpi();
        let e = p.screens();
        assert!(e[1].work_area.left() < 0.0);
        assert_eq!(e[1].scale, 2.0);
    }
}
