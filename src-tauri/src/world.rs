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

/// Le rôle d'une plateforme issue d'un écran.
///
/// Sert **uniquement** à fabriquer quatre identités distinctes par écran :
/// ni la physique ni le comportement ne le consultent jamais, exactement
/// comme `PlatformKind`. Une plateforme se décrit par ses `faces`, pas par
/// son étiquette d'origine.
///
/// Les valeurs explicites (`= 0`, `= 1`…) ne sont pas décoratives : elles
/// entrent dans le calcul de `PlatformId::ecran`, et les changer changerait
/// toutes les identités.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleEcran {
    Sol = 0,
    MurGauche = 1,
    MurDroit = 2,
    Plafond = 3,
}

impl PlatformId {
    /// L'identité d'une des quatre plateformes d'un écran (design §2.2).
    ///
    /// `monitor << 2 | role` : les deux bits de poids faible portent le rôle,
    /// le reste porte la poignée du moniteur. `HMONITOR` et `HWND` sont des
    /// poignées en espace utilisateur, largement sous 2⁴⁷ sur Windows x64 —
    /// décaler de deux bits ne perd donc rien et ne peut pas collisionner.
    ///
    /// **L'identité reste indépendante de la géométrie** (spec §5.2), ce qui
    /// est la condition de la décision n° 1 : changer la résolution ne change
    /// pas la poignée du moniteur, donc pas l'identité.
    ///
    /// `role as u64` : un `enum` sans données et à valeurs explicites se
    /// convertit en entier par un simple `as`. C'est la seule conversion de
    /// ce genre du projet, et elle est sûre parce que les quatre valeurs
    /// tiennent sur deux bits.
    pub fn ecran(monitor: u64, role: RoleEcran) -> PlatformId {
        PlatformId(monitor << 2 | role as u64)
    }

    /// Ces deux plateformes viennent-elles du même écran ?
    ///
    /// On retire les deux bits de rôle et on compare le reste. Sert à
    /// l'intention `Grimper` (Tâche 4), qui cherche un mur **de l'écran où
    /// le personnage se trouve** — pas celui d'en face.
    pub fn meme_ecran(&self, autre: PlatformId) -> bool {
        self.0 >> 2 == autre.0 >> 2
    }
}

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

/// Épaisseur donnée au rectangle d'une plateforme.
///
/// Aucune de ces plateformes n'a d'épaisseur réelle — un sol est une ligne,
/// un mur aussi — mais un `Rect` en demande une, et une épaisseur nulle
/// rendrait `contains` toujours faux. Un pixel suffit : seule la face
/// tournée vers l'intérieur de l'écran est exposée, donc cette épaisseur
/// n'est jamais parcourue.
const EPAISSEUR_PLATEFORME: f32 = 1.0;

/// Tolérance sur la jonction entre deux écrans, en pixels.
///
/// La **même valeur** que la tolérance de `face_voisine`, et pour la même
/// raison : deux écrans côte à côte se touchent exactement, mais des
/// résolutions ou des échelles différentes peuvent laisser quelques pixels
/// de jeu. On ne veut pas d'un mur fantôme pour 2 px.
const TOLERANCE_JONCTION: f32 = 8.0;

impl World {
    /// Construit le monde : **sol, deux murs et plafond par écran**
    /// (design §2).
    ///
    /// Un mur n'est posé que si aucun autre écran ne le touche : sans cette
    /// règle, deux écrans côte à côte donneraient un mur invisible en plein
    /// milieu du bureau, que le personnage escaladerait alors qu'il traverse
    /// déjà librement le sol au même endroit (design §2.3).
    ///
    /// Tout est pris sur la **zone de travail**, jamais sur l'écran complet :
    /// c'est le piège Windows n° 3 appliqué aux trois nouvelles faces — sur
    /// l'écran complet, il grimperait derrière la barre des tâches.
    ///
    /// ⚠️ **Le sol de chaque écran est poussé en premier**, et c'est un
    /// contrat : plusieurs tests d'autres modules écrivent `platforms()[0]`
    /// en voulant dire « le sol ». Le test
    /// `le_sol_est_toujours_la_premiere_plateforme_de_son_ecran` le fige.
    pub fn from_screens(screens: &[ScreenInfo]) -> World {
        let mut platforms = Vec::with_capacity(screens.len() * 4);

        for s in screens {
            let z = s.work_area;

            // ── Le sol ──────────────────────────────────────────────────
            // Son bord SUPÉRIEUR est à hauteur du bas de la zone de travail :
            // c'est la ligne sur laquelle on marche.
            platforms.push(Platform {
                id: PlatformId::ecran(s.id, RoleEcran::Sol),
                rect: Rect::new(z.left(), z.bottom(), z.w, EPAISSEUR_PLATEFORME),
                kind: PlatformKind::Screen,
                z: 0,
                faces: vec![Face::Top],
            });

            // ── Le plafond ──────────────────────────────────────────────
            // Posé JUSTE AU-DESSUS de la zone de travail, et c'est sa face
            // `Bottom` qui est exposée : on s'y suspend par en dessous. Le
            // `- EPAISSEUR` place le rectangle de sorte que `bottom()` tombe
            // exactement sur `z.top()`.
            //
            // Jamais supprimé, lui : il n'y a rien au-dessus du bureau.
            platforms.push(Platform {
                id: PlatformId::ecran(s.id, RoleEcran::Plafond),
                rect: Rect::new(
                    z.left(),
                    z.top() - EPAISSEUR_PLATEFORME,
                    z.w,
                    EPAISSEUR_PLATEFORME,
                ),
                kind: PlatformKind::Screen,
                z: 0,
                faces: vec![Face::Bottom],
            });

            // ── Les deux murs, si personne ne les touche ────────────────
            //
            // Le mur GAUCHE expose sa face `Right` : le personnage se tient
            // à sa droite, c'est-à-dire à l'intérieur de l'écran. C'est la
            // même logique que le sol, dont la face `Top` regarde vers le
            // haut, donc vers l'intérieur (design §2.1).
            if !ecran_adjacent(screens, s, false) {
                platforms.push(Platform {
                    id: PlatformId::ecran(s.id, RoleEcran::MurGauche),
                    rect: Rect::new(
                        z.left() - EPAISSEUR_PLATEFORME,
                        z.top(),
                        EPAISSEUR_PLATEFORME,
                        z.h,
                    ),
                    kind: PlatformKind::Screen,
                    z: 0,
                    faces: vec![Face::Right],
                });
            }

            if !ecran_adjacent(screens, s, true) {
                platforms.push(Platform {
                    id: PlatformId::ecran(s.id, RoleEcran::MurDroit),
                    rect: Rect::new(z.right(), z.top(), EPAISSEUR_PLATEFORME, z.h),
                    kind: PlatformKind::Screen,
                    z: 0,
                    faces: vec![Face::Left],
                });
            }
        }

        World { platforms }
    }

    pub fn platforms(&self) -> &[Platform] {
        &self.platforms
    }

    /// Le premier sol du monde, s'il y en a un.
    ///
    /// Existe pour que les appelants qui veulent dire « le sol » cessent
    /// d'écrire `platforms()[0]`, qui n'est vrai que par convention d'ordre.
    /// Utilisé au placement initial du personnage (`main.rs`).
    ///
    /// `find` sur un `Vec` de quelques éléments : inutile d'indexer.
    pub fn premier_sol(&self) -> Option<&Platform> {
        self.platforms.iter().find(|p| p.has_face(Face::Top))
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

/// Un autre écran touche-t-il `s` de ce côté ?
///
/// **Règle binaire assumée** (design §2.3) : on ne regarde pas *quelle
/// portion* du bord est partagée, seulement s'il y a contact. Une adjacence
/// partielle — deux écrans de hauteurs différentes — fait donc perdre le mur
/// entier plutôt que sa moitié libre. C'est conservateur : on ne crée jamais
/// un mur fantôme, on en perd parfois un vrai. Le traitement rigoureux est la
/// soustraction d'intervalles 1D de la décision n° 2, réservée à l'étape 4
/// complète.
///
/// Le recouvrement vertical est exigé en plus du contact horizontal : un
/// écran placé en diagonale peut toucher la même ligne `x` sans être en face,
/// et son bord ne devrait alors rien masquer.
fn ecran_adjacent(screens: &[ScreenInfo], s: &ScreenInfo, a_droite: bool) -> bool {
    for autre in screens {
        if autre.id == s.id {
            continue;
        }

        // Les deux écrans se croisent-ils verticalement, ne serait-ce qu'un
        // peu ? `max des tops < min des bottoms` est le test d'intersection
        // d'intervalles habituel.
        let haut = s.work_area.top().max(autre.work_area.top());
        let bas = s.work_area.bottom().min(autre.work_area.bottom());
        if haut >= bas {
            continue;
        }

        let colle = if a_droite {
            (autre.work_area.left() - s.work_area.right()).abs() <= TOLERANCE_JONCTION
        } else {
            (s.work_area.left() - autre.work_area.right()).abs() <= TOLERANCE_JONCTION
        };

        if colle {
            return true;
        }
    }

    false
}

// Les tests de ce module vivent dans `world_tests.rs`
// (sortis d ici le 2026-09-14 : ils faisaient 236 des 606 lignes).
#[cfg(test)]
#[path = "world_tests.rs"]
mod tests;
