//! Sonde de test : des écrans, une souris et des fenêtres que le test décide.
//!
//! Existe pour satisfaire la troisième contrainte de la spec §10.2. Les
//! constructeurs de commodité reproduisent des topologies nommées, ce qui
//! rend les attentes des tests lisibles sans commentaire.

use super::{Batterie, MouseState, ScreenInfo, Signaux, SystemProbe, WindowInfo};
use crate::geom::{Point, Rect};
use std::cell::{Cell, RefCell};

pub struct FakeProbe {
    screens: Vec<ScreenInfo>,
    // `Cell` pour la même raison que dans `FakeClock` : le trait expose
    // `&self`, donc un test qui n'a qu'une référence partagée doit pouvoir
    // bouger la souris. `MouseState` est `Copy`, ce que `Cell` exige.
    mouse: Cell<MouseState>,

    // `RefCell` et non `Cell` : `Signaux` n'est pas `Copy`, il porte le nom
    // de l'application active. `Cell::get` exige `Copy` ; `RefCell` prête à
    // la place, au prix d'un compteur d'emprunts vérifié à l'exécution.
    signaux: RefCell<Signaux>,

    // Les fenêtres que ce faux système déclare. `RefCell` comme `signaux` :
    // un `Vec` n'est pas `Copy`, et les tests doivent pouvoir le remplacer à
    // travers une référence partagée (« et maintenant cette fenêtre se
    // ferme »).
    //
    // **Vide par défaut**, et c'est délibéré : tous les tests écrits avant
    // l'étape 4b décrivent un monde sans fenêtres, et doivent continuer de
    // décrire exactement le même monde. Une fenêtre par défaut changerait
    // silencieusement ce que des dizaines de tests mesurent.
    fenetres: RefCell<Vec<WindowInfo>>,
}

// `allow(dead_code)` : ces éléments SONT utilisés — par les tests. Mais un
// build normal ne compile pas `#[cfg(test)]`, donc le compilateur les voit
// morts et le signale à chaque fois. Ce sont les doubles de test exigés par
// la spec §10.2 ; ils ont leur place dans le binaire, et l'avertissement est
// ici du bruit, pas un signal.
#[allow(dead_code)]
impl FakeProbe {
    pub fn new(screens: Vec<ScreenInfo>) -> Self {
        FakeProbe {
            screens,
            mouse: Cell::new(MouseState {
                pos: Point::new(0.0, 0.0),
                left_down: false,
                right_down: false,
            }),

            // Le défaut est délibérément « rien de spécial » : aucun signal
            // ne mord, donc un test d'étape 1 qui ignore les signaux garde
            // exactement le comportement qu'il avait.
            fenetres: RefCell::new(Vec::new()),

            signaux: RefCell::new(Signaux {
                inactivite: std::time::Duration::ZERO,
                appli_active: None,
                heure: 15,
                batterie: Batterie {
                    pourcent: None,
                    sur_secteur: true,
                },
                session_verrouillee: false,
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

    /// Le cas courant des tests : seule la position et le bouton gauche
    /// comptent. Le bouton droit n'intéresse que la boucle 60 Hz, qui n'est
    /// pas testable hors Windows — l'imposer ici alourdirait des dizaines
    /// d'appels pour rien.
    pub fn set_mouse(&self, pos: Point, left_down: bool) {
        self.mouse.set(MouseState {
            pos,
            left_down,
            right_down: false,
        });
    }

    pub fn set_mouse_droit(&self, pos: Point, right_down: bool) {
        self.mouse.set(MouseState {
            pos,
            left_down: false,
            right_down,
        });
    }

    /// Remplace la liste des fenêtres. `z` est le rang dans la liste — donc
    /// l'ordre d'écriture du test EST le z-order, du premier plan vers
    /// l'arrière, exactement comme `EnumWindows`.
    ///
    /// Prend des `(hwnd, Rect)` plutôt que des `WindowInfo` tout faits :
    /// écrire le `z` à la main dans chaque test inviterait à se tromper, et
    /// un z incohérent avec l'ordre donnerait une occlusion fausse sans
    /// qu'aucune assertion ne le dise.
    pub fn set_fenetres(&self, fenetres: &[(u64, Rect)]) {
        *self.fenetres.borrow_mut() = fenetres
            .iter()
            .enumerate()
            .map(|(i, (hwnd, rect))| WindowInfo {
                hwnd: *hwnd,
                rect: *rect,
                z: i as u32,
            })
            .collect();
    }

    pub fn set_signaux(&self, s: Signaux) {
        *self.signaux.borrow_mut() = s;
    }

    /// Raccourci pour le cas le plus fréquent des tests : « il est parti
    /// depuis N secondes ». Écrire les cinq champs à chaque fois noierait
    /// l'intention du test dans du remplissage.
    pub fn set_inactivite(&self, d: std::time::Duration) {
        self.signaux.borrow_mut().inactivite = d;
    }
}

impl SystemProbe for FakeProbe {
    fn screens(&self) -> Vec<ScreenInfo> {
        self.screens.clone()
    }

    fn mouse(&self) -> MouseState {
        self.mouse.get()
    }

    fn windows(&self) -> Vec<WindowInfo> {
        self.fenetres.borrow().clone()
    }

    fn rect_de_fenetre(&self, hwnd: u64) -> Option<Rect> {
        // La vraie sonde ré-interroge le système ; la fausse relit sa propre
        // liste. Les deux répondent donc `None` pour une fenêtre fermée, ce
        // qui est **la** propriété que les tests doivent pouvoir exercer :
        // c'est elle qui déclenche « plateforme disparue → je tombe ».
        self.fenetres
            .borrow()
            .iter()
            .find(|f| f.hwnd == hwnd)
            .map(|f| f.rect)
    }

    fn signaux(&self) -> Signaux {
        // `clone` : le trait rend une valeur possédée, et `Signaux` n'est pas
        // `Copy`. Deux fois par seconde, une chaîne de vingt caractères —
        // sans importance.
        self.signaux.borrow().clone()
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
        // Le bouton droit reste au repos : `set_mouse` ne parle que du
        // gauche, et les deux ne doivent pas se contaminer.
        assert!(!m.right_down);

        p.set_mouse_droit(Point::new(10.0, 20.0), true);
        let m = vue.mouse();
        assert!(m.right_down);
        assert!(!m.left_down);
    }

    #[test]
    fn une_topologie_a_x_negatif_est_representable() {
        // Contrainte globale : ne jamais supposer x >= 0.
        let p = FakeProbe::ecran_a_gauche_hidpi();
        let e = p.screens();
        assert!(e[1].work_area.left() < 0.0);
        assert_eq!(e[1].scale, 2.0);
    }

    #[test]
    fn les_signaux_se_reglent_a_travers_une_reference_partagee() {
        // Même contrainte que pour la souris : le trait expose `&self`, donc
        // un test qui n'a qu'une référence partagée doit pouvoir changer les
        // signaux. `Signaux` n'étant pas `Copy` (il porte une `String`),
        // c'est un `RefCell` et non un `Cell`.
        let p = FakeProbe::un_ecran();
        let vue: &dyn SystemProbe = &p;

        // Le défaut : personne n'est parti, il est 15 h, sur secteur, session
        // ouverte. C'est « rien de spécial », comme dans les tests de
        // `signals.rs`.
        let d = vue.signaux();
        assert_eq!(d.inactivite, std::time::Duration::ZERO);
        assert!(!d.session_verrouillee);
        assert_eq!(d.heure, 15);

        p.set_signaux(Signaux {
            inactivite: std::time::Duration::from_secs(300),
            appli_active: Some("Code.exe".to_string()),
            heure: 23,
            batterie: Batterie {
                pourcent: Some(7),
                sur_secteur: false,
            },
            session_verrouillee: true,
        });

        let s = vue.signaux();
        assert_eq!(s.inactivite, std::time::Duration::from_secs(300));
        assert_eq!(s.appli_active.as_deref(), Some("Code.exe"));
        assert_eq!(s.heure, 23);
        assert_eq!(s.batterie.pourcent, Some(7));
        assert!(s.session_verrouillee);
    }
}
