//! Le monde : la liste de tout ce sur quoi un personnage peut se tenir.
//!
//! Responsabilité unique : transformer ce que la sonde rapporte en
//! plateformes, et permettre de retrouver l'une d'elles par son identité.
//!
//! **Aucune notion de personnage ici.** C'est la clé du §5.1 : la physique et
//! le comportement ne manipulent que des `Platform`, et ne savent donc jamais
//! si celle sous leurs pieds est le sol d'un écran ou la barre de titre de
//! VSCode. C'est ce qui permet de livrer le sol maintenant (étape 1) et de
//! brancher les fenêtres plus tard (étape 4) sans réécrire une ligne.

use crate::geom::{Point, Rect};
use crate::probe::ScreenInfo;

// Réexport sous son vrai nom : les appelants écrivent `use crate::world::Face`
// sans avoir à savoir que le type vit dans `geom` pour éviter un cycle de
// dépendances (voir « écarts assumés » en tête de plan). `Face` est donc dans
// la portée de ce fichier par ce réexport, et ne figure PAS dans le `use
// crate::geom::{…}` ci-dessus — l'y mettre serait un conflit de noms.
pub use crate::geom::Face;

/// Identité stable d'une plateforme (spec §5.2).
///
/// Dérivée du `HMONITOR` pour un écran, et du `HWND` pour une fenêtre à
/// l'étape 4. **Elle ne dépend pas de la géométrie** : « la plateforme sur
/// laquelle je suis » survit donc au déplacement, au redimensionnement et au
/// changement de résolution. C'est la condition qui rend la décision n° 1
/// possible.
///
/// `Hash` et `Eq` pour servir de clé de table à l'étape 4, où l'on suivra la
/// seule plateforme occupée à 60 Hz (spec §5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlatformId(pub u64);

/// D'où vient la plateforme. Le comportement n'a pas à le consulter — c'est
/// là pour le diagnostic et le mode simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformKind {
    Screen,
    Window,
}

/// Un rectangle, plus les faces réellement utilisables.
///
/// `faces` est une liste et non quatre booléens : à l'étape 4, un bord
/// partiellement recouvert disparaîtra de cette liste (décision n° 2), et une
/// liste rend l'absence naturelle à exprimer.
#[derive(Debug, Clone, PartialEq)]
pub struct Platform {
    pub id: PlatformId,
    pub rect: Rect,
    pub kind: PlatformKind,
    /// 0 = au-dessus de tout. Sert à l'occlusion de l'étape 4 ; à l'étape 1,
    /// tous les sols sont au même rang.
    pub z: u32,
    pub faces: Vec<Face>,
}

impl Platform {
    /// Recherche linéaire sur un Vec de quatre éléments au maximum : plus
    /// rapide qu'un ensemble, et plus lisible.
    pub fn has_face(&self, face: Face) -> bool {
        self.faces.contains(&face)
    }
}

/// Tout ce sur quoi on peut se tenir, à un instant donné.
///
/// Reconstruit à ~8 Hz (spec §5.5). Le personnage n'en garde qu'un
/// `PlatformId`, jamais une référence — c'est pour ça qu'on peut reconstruire
/// le monde entier sans rien invalider. En Rust, ce détail-là n'est pas un
/// détail : garder une `&Platform` dans le personnage obligerait à annoter des
/// durées de vie partout, et interdirait de reconstruire le monde.
#[derive(Debug, Clone, Default)]
pub struct World {
    platforms: Vec<Platform>,
}

/// Hauteur donnée au rectangle d'une plateforme de sol.
///
/// Le sol n'a pas d'épaisseur réelle, mais un `Rect` en demande une, et une
/// hauteur nulle rendrait `contains` toujours faux. Un pixel suffit : seule la
/// face `Top` est exposée, donc cette hauteur n'est jamais parcourue.
const EPAISSEUR_DU_SOL: f32 = 1.0;

impl World {
    /// Construit le monde de l'étape 1 : **un sol par écran, et rien d'autre.**
    ///
    /// La spec §5.1 décrit qu'un écran offre aussi deux murs et un plafond.
    /// L'étape 1 ne les expose délibérément pas (spec §11) : sans images
    /// d'escalade branchées ni comportement de grimpe, un mur exposé serait
    /// une plateforme sur laquelle le personnage pourrait s'accrocher sans
    /// savoir en redescendre. C'est une ligne à ajouter à l'étape 4, pas une
    /// omission à rattraper.
    pub fn from_screens(screens: &[ScreenInfo]) -> World {
        let mut platforms = Vec::with_capacity(screens.len());

        for s in screens {
            // Le sol : un rectangle posé au BAS de la zone de travail. Sa
            // face Top est donc à `work_area.bottom()` — la ligne sur
            // laquelle le personnage marche, juste au-dessus de la barre des
            // tâches (piège Windows n° 3).
            platforms.push(Platform {
                id: PlatformId(s.id),
                rect: Rect::new(
                    s.work_area.left(),
                    s.work_area.bottom(),
                    s.work_area.w,
                    EPAISSEUR_DU_SOL,
                ),
                kind: PlatformKind::Screen,
                z: 0,
                faces: vec![Face::Top],
            });
        }

        World { platforms }
    }

    pub fn platforms(&self) -> &[Platform] {
        &self.platforms
    }

    /// Retrouve une plateforme par son identité.
    ///
    /// Rend `Option` et ne panique pas : « la plateforme a disparu » est un
    /// cas NORMAL et fréquent — c'est même l'événement central de la décision
    /// n° 1 (fenêtre fermée → le personnage tombe). L'appelant doit le gérer,
    /// et le type le lui rappelle à chaque usage.
    pub fn get(&self, id: PlatformId) -> Option<&Platform> {
        self.platforms.iter().find(|p| p.id == id)
    }

    /// Le sol le plus proche d'un point, et l'offset correspondant le long de
    /// sa face `Top`.
    ///
    /// Deux usages :
    ///   · placer un personnage au démarrage ;
    ///   · le **garde-fou** de la spec §6.3 — un personnage tombé sous le bas
    ///     du bureau virtuel est replacé sur le sol le plus proche, et n'est
    ///     donc jamais perdu définitivement.
    ///
    /// L'offset est **rabattu dans les bornes de la face** : le but est de
    /// produire une position tenable, pas de reporter le problème.
    pub fn nearest_floor(&self, p: Point) -> Option<(PlatformId, f32)> {
        // (id, offset, distance) — la distance ne sert qu'à comparer, et on
        // la laisse tomber à la fin.
        let mut meilleur: Option<(PlatformId, f32, f32)> = None;

        for plat in &self.platforms {
            if !plat.has_face(Face::Top) {
                continue;
            }

            // Le point de la face le plus proche horizontalement : on rabat
            // `p.x` entre les deux bords. `clamp` panique si min > max, ce
            // qui ne peut pas arriver ici — `from_screens` ne construit
            // jamais un rectangle de largeur négative.
            let x_rabattu = p.x.clamp(plat.rect.left(), plat.rect.right());
            let point_face = Point::new(x_rabattu, plat.rect.top());
            let distance = p.distance_to(point_face);

            // `match` explicite plutôt qu'une chaîne de combinateurs sur
            // Option : la comparaison se relit mieux.
            let remplace = match meilleur {
                None => true,
                Some((_, _, d)) => distance < d,
            };
            if remplace {
                meilleur = Some((plat.id, x_rabattu - plat.rect.left(), distance));
            }
        }

        meilleur.map(|(id, offset, _)| (id, offset))
    }

    /// Le rectangle englobant toutes les plateformes. Le « bas du bureau
    /// virtuel » de la spec §6.3 s'en déduit.
    ///
    /// `None` si le monde est vide, ce qui n'est pas une erreur.
    pub fn bounds(&self) -> Option<Rect> {
        // `first()?` : si la liste est vide, on rend None immédiatement. Le
        // `?` sur une Option, dans une fonction qui rend une Option, est la
        // façon courte d'écrire ce `match`.
        let premier = self.platforms.first()?;

        let mut min_x = premier.rect.left();
        let mut max_x = premier.rect.right();
        let mut min_y = premier.rect.top();
        let mut max_y = premier.rect.bottom();

        // `[1..]` : on a déjà pris le premier ci-dessus. Sûr même avec un seul
        // élément — la tranche est alors vide, et la boucle ne tourne pas.
        for p in &self.platforms[1..] {
            min_x = min_x.min(p.rect.left());
            max_x = max_x.max(p.rect.right());
            min_y = min_y.min(p.rect.top());
            max_y = max_y.max(p.rect.bottom());
        }

        Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::fake::FakeProbe;
    use crate::probe::{ScreenInfo, SystemProbe};

    #[test]
    fn un_ecran_donne_une_plateforme_de_sol() {
        let monde = World::from_screens(&FakeProbe::un_ecran().screens());
        assert_eq!(monde.platforms().len(), 1);

        let p = &monde.platforms()[0];
        assert_eq!(p.kind, PlatformKind::Screen);
        // À l'étape 1, SEULE la face Top est exposée : le sol. Murs et
        // plafond arrivent à l'étape 4 (spec §11).
        assert!(p.has_face(Face::Top));
        assert!(!p.has_face(Face::Left));
        assert!(!p.has_face(Face::Bottom));
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
    fn deux_ecrans_donnent_deux_plateformes_distinctes() {
        let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());
        assert_eq!(monde.platforms().len(), 2);
        assert_ne!(monde.platforms()[0].id, monde.platforms()[1].id);
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
        assert_eq!(b.left(), 0.0);
        assert_eq!(b.right(), 3840.0);
    }
}
