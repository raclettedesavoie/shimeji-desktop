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
use crate::probe::{ScreenInfo, WindowInfo};

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

/// Le rôle d'une plateforme au sein de son support — un écran **ou une
/// fenêtre**.
///
/// Sert **uniquement** à fabriquer quatre identités distinctes par support :
/// ni la physique ni le comportement ne le consultent jamais, exactement
/// comme `PlatformKind`. Une plateforme se décrit par ses `faces`, pas par
/// son étiquette d'origine.
///
/// Les noms restent ceux de l'écran, et les fenêtres les réemploient tels
/// quels : le `Sol` d'une fenêtre est sa **barre de titre**, son `Plafond`
/// est son **dessous** (où l'on se suspend). C'est volontairement le même
/// vocabulaire — la physique ne doit jamais pouvoir distinguer les deux
/// sources (design §5.1), et deux jeux de noms l'inviteraient à essayer.
///
/// Les valeurs explicites (`= 0`, `= 1`…) ne sont pas décoratives : elles
/// entrent dans le calcul des identités, et les changer changerait toutes
/// les plateformes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Sol = 0,
    MurGauche = 1,
    MurDroit = 2,
    Plafond = 3,
}

impl PlatformId {
    /// Le nombre de bits réservés, en bas de l'identifiant, à ce qui n'est
    /// pas la poignée : **deux pour le rôle, un pour la source.**
    ///
    /// ⚠️ **Le bit de source a été ajouté à l'étape 4b, et il n'est pas
    /// facultatif.** `HMONITOR` et `HWND` sont deux espaces de poignées
    /// *distincts*, mais ce sont tous deux des pointeurs en espace
    /// utilisateur : rien n'interdit qu'un moniteur et une fenêtre portent la
    /// même valeur numérique. Sans ce bit, les deux plateformes
    /// collisionneraient — et le symptôme serait un personnage qui « change
    /// de support » sans raison, au hasard des lancements. Introuvable.
    const BITS_BAS: u32 = 3;

    /// Le bit qui distingue une fenêtre d'un écran. 0 = écran, 1 = fenêtre.
    const BIT_FENETRE: u64 = 1;

    /// L'identité d'une des quatre plateformes d'un **écran** (design §5.2).
    ///
    /// **L'identité reste indépendante de la géométrie**, ce qui est la
    /// condition de la décision n° 1 : changer la résolution ne change pas la
    /// poignée du moniteur, donc pas l'identité.
    ///
    /// `role as u64` : un `enum` sans données et à valeurs explicites se
    /// convertit en entier par un simple `as`. C'est sûr parce que les quatre
    /// valeurs tiennent sur deux bits.
    pub fn ecran(monitor: u64, role: Role) -> PlatformId {
        PlatformId(monitor << Self::BITS_BAS | (role as u64) << 1)
    }

    /// L'identité d'une des quatre plateformes d'une **fenêtre**
    /// (design §5.2, étape 4b).
    ///
    /// Dérivée du `HWND`, donc **stable au déplacement ET au
    /// redimensionnement** de la fenêtre. C'est ce qui fait qu'un personnage
    /// assis sur une barre de titre qu'on balade voyage avec elle sans une
    /// ligne de code dédiée (décision n° 1).
    ///
    /// ⚠️ **Le numéro de segment n'entre PAS dans l'identité**, alors qu'un
    /// bord partiellement recouvert en produit plusieurs (décision n° 2).
    /// C'était la tentation naturelle, et elle aurait cassé la décision n° 1 :
    /// les segments se renumérotent dès qu'une fenêtre au-dessus bouge, donc
    /// « la plateforme où je suis » changerait d'identité et le personnage
    /// tomberait sans raison, par intermittence. Les segments vivent donc
    /// dans `Platform::libre`, qui varie — pas dans l'identité, qui ne doit
    /// pas.
    pub fn fenetre(hwnd: u64, role: Role) -> PlatformId {
        PlatformId(hwnd << Self::BITS_BAS | (role as u64) << 1 | Self::BIT_FENETRE)
    }

    /// Ces deux plateformes viennent-elles du **même support** — le même
    /// écran, ou la même fenêtre ?
    ///
    /// On retire les bits de rôle et de source, et on compare le reste.
    ///
    /// ⚠️ La source est comparée **séparément**, et ce n'est pas du zèle :
    /// deux poignées d'espaces différents peuvent coïncider numériquement
    /// (c'est la raison d'être de `BIT_FENETRE`), et un décalage seul rendrait
    /// alors `true` pour un écran et une fenêtre sans rapport.
    ///
    /// Sert à l'intention `Grimper`, qui cherche un mur **du support où le
    /// personnage se trouve** — pas celui d'en face. Renommée depuis
    /// `meme_ecran` à l'étape 4b : un personnage debout sur une barre de
    /// titre qui grimpe le flanc de SA fenêtre est exactement le comportement
    /// voulu, et l'ancien nom l'aurait fait lire comme un bug.
    pub fn meme_support(&self, autre: PlatformId) -> bool {
        (self.0 & Self::BIT_FENETRE) == (autre.0 & Self::BIT_FENETRE)
            && self.0 >> Self::BITS_BAS == autre.0 >> Self::BITS_BAS
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
    /// 0 = au-dessus de tout. C'est le rang d'énumération de `EnumWindows`
    /// pour une fenêtre, et 0 pour un écran — un écran est derrière tout.
    pub z: u32,
    pub faces: Vec<Face>,

    /// Les portions **réellement praticables** de la face, en distances au
    /// bord (la même coordonnée que l'`offset` d'un `Attachment`).
    ///
    /// # C'est ici que vit la décision n° 2
    ///
    /// Un bord est un segment ; on lui retire les fenêtres de z-order
    /// supérieur, et il ne reste que des morceaux. Le personnage devient
    /// alors **physiquement incapable** de se tenir sur du vide, plutôt
    /// qu'on ait à détecter puis corriger le cas.
    ///
    /// ```text
    /// barre de titre    ├────────────────────────────┤
    /// fenêtre au-dessus          ▓▓▓▓▓▓▓▓▓▓▓
    ///                                  ↓
    /// libre             ├────────┤           ├───────┤
    /// ```
    ///
    /// # Pourquoi une liste ici, et non plusieurs plateformes
    ///
    /// Parce que l'identité doit rester stable (décision n° 1). Faire une
    /// plateforme par morceau obligerait à numéroter les morceaux, et ils se
    /// renumérotent dès qu'une fenêtre au-dessus bouge : « la plateforme où
    /// je suis » changerait d'identité et le personnage tomberait sans
    /// raison, par intermittence. Ici l'identité ne bouge pas ; c'est
    /// l'étendue praticable qui varie, et il tombe exactement quand
    /// l'endroit **qu'il occupe** se fait recouvrir.
    ///
    /// Une plateforme entièrement libre porte un seul intervalle couvrant
    /// toute la face — jamais une liste vide, qui voudrait dire « rien n'est
    /// praticable ».
    pub libre: Vec<(f32, f32)>,
}

impl Platform {
    /// Recherche linéaire sur un Vec de quatre éléments au maximum : plus
    /// rapide qu'un ensemble, et plus lisible.
    pub fn has_face(&self, face: Face) -> bool {
        self.faces.contains(&face)
    }

    /// Une plateforme à **une seule face, entièrement praticable**.
    ///
    /// Les huit plateformes (quatre par écran, quatre par fenêtre) se
    /// construisent toutes ainsi, et l'occlusion vient ensuite restreindre
    /// `libre`. Un constructeur plutôt que le champ recopié huit fois : le
    /// jour où `libre` changera de forme, il y aura un seul endroit à
    /// corriger — et surtout, on ne peut pas oublier de le remplir.
    ///
    /// `face_length` donne la longueur de la face concernée : la largeur du
    /// rectangle pour `Top`/`Bottom`, sa hauteur pour `Left`/`Right`. C'est
    /// la MÊME fonction qui borne les offsets ailleurs, donc les deux ne
    /// peuvent pas se désaccorder.
    pub fn entiere(id: PlatformId, rect: Rect, kind: PlatformKind, z: u32, face: Face) -> Platform {
        Platform {
            id,
            rect,
            kind,
            z,
            faces: vec![face],
            libre: vec![(0.0, rect.face_length(face))],
        }
    }

    /// Cet endroit de la face est-il praticable, ou recouvert ?
    ///
    /// `offset` est la distance au bord de la face, exactement celle que
    /// stocke un `Attachment::On` — c'est ce qui permet de poser la question
    /// sans rien convertir.
    ///
    /// Bornes **inclusives** : un personnage pile au bout d'un segment libre
    /// y tient encore. Les exclure le ferait tomber en arrivant exactement
    /// sur le bord, ce qui est le cas le plus fréquent quand il marche vers
    /// une zone recouverte — il tomberait un pixel trop tôt, à chaque fois.
    pub fn est_libre(&self, offset: f32) -> bool {
        self.libre
            .iter()
            .any(|(debut, fin)| offset >= *debut && offset <= *fin)
    }
}

/// La plus petite portion de bord sur laquelle il vaille la peine de se
/// tenir, en pixels.
///
/// En dessous, le personnage déborderait largement du morceau et aurait
/// l'air posé dans le vide — ce que la décision n° 2 cherche justement à
/// rendre impossible. La valeur est de l'ordre de la moitié d'une frame
/// Shimeji (128 px) : assez pour qu'on le voie tenir, assez petit pour que
/// les rebords étroits restent utilisables.
const LONGUEUR_MINIMALE_SEGMENT: f32 = 64.0;

/// Retire d'un segment `[0, longueur]` tous les intervalles `occultants`, et
/// rend ce qui reste.
///
/// **C'est toute la décision n° 2, et c'est de l'arithmétique 1D** — pas de
/// la géométrie. C'est précisément ce qui la rend bon marché et testable
/// sans le moindre écran.
///
/// Les intervalles trop courts pour qu'on s'y tienne sont écartés
/// (`LONGUEUR_MINIMALE_SEGMENT`), ce qui évite de semer le monde de miettes
/// de plateformes entre deux fenêtres presque jointives.
///
/// # L'algorithme, et pourquoi celui-là
///
/// On trie les occultants par début, puis on balaye une fois en gardant le
/// point le plus à droite déjà couvert. C'est la méthode classique de fusion
/// d'intervalles, en `O(n log n)` dominé par le tri — avec une poignée de
/// fenêtres, le tri coûte moins que son propre commentaire.
///
/// L'alternative naïve — soustraire les occultants un par un d'une liste de
/// morceaux — est plus courte à écrire et **fausse dès que deux occultants
/// se chevauchent**, ce qui est le cas normal sur un bureau encombré.
fn soustraire_intervalles(longueur: f32, occultants: &[(f32, f32)]) -> Vec<(f32, f32)> {
    // Rien ne recouvre : toute la face est praticable. Le cas de loin le plus
    // fréquent, et il évite une allocation de tri.
    if occultants.is_empty() {
        return vec![(0.0, longueur)];
    }

    // On borne les occultants à la face et on jette ceux qui n'y touchent
    // pas : la suite du balayage suppose des intervalles à l'intérieur.
    let mut bornes: Vec<(f32, f32)> = occultants
        .iter()
        .map(|(a, b)| (a.max(0.0), b.min(longueur)))
        .filter(|(a, b)| b > a)
        .collect();

    // `partial_cmp` et non `cmp` : `f32` n'est que `PartialOrd`, à cause de
    // `NaN`. `unwrap_or(Equal)` traite un éventuel `NaN` comme « à égalité »
    // plutôt que de paniquer — un rectangle dégénéré ne doit pas faire
    // tomber l'application.
    bornes.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut libres = Vec::new();

    // `curseur` = le point à partir duquel la face est encore libre.
    let mut curseur = 0.0f32;

    for (debut, fin) in bornes {
        // Le trou entre la fin du dernier occultant et le début de celui-ci.
        // `debut > curseur` et non `>=` : deux occultants jointifs ne laissent
        // pas de trou de largeur nulle.
        if debut > curseur {
            let morceau = (curseur, debut);
            if morceau.1 - morceau.0 >= LONGUEUR_MINIMALE_SEGMENT {
                libres.push(morceau);
            }
        }

        // `max` et non une affectation : un occultant entièrement contenu
        // dans un précédent ne doit pas faire RECULER le curseur, ce qui
        // rouvrirait une portion déjà couverte.
        curseur = curseur.max(fin);
    }

    // La queue, après le dernier occultant.
    if longueur - curseur >= LONGUEUR_MINIMALE_SEGMENT {
        libres.push((curseur, longueur));
    }

    libres
}


/// Les quatre plateformes qu'une fenêtre expose, et la face praticable de
/// chacune.
///
/// ⚠️ **L'association rôle → face est INVERSÉE par rapport à un écran**, sur
/// les murs, et c'est la chose la plus facile à lire de travers de ce
/// fichier. La raison est purement géométrique : sur un écran, le personnage
/// est **dedans**, donc le mur gauche lui présente sa face `Right` ; sur une
/// fenêtre, il est **dehors**, donc le bord gauche lui présente sa face
/// `Left`.
///
/// Les deux faces horizontales, elles, ne s'inversent pas : on marche sur le
/// dessus (`Top`, la barre de titre) et on se suspend sous le dessous
/// (`Bottom`), comme pour le sol et le plafond d'un écran.
const ROLES_DE_FENETRE: &[(Role, Face)] = &[
    (Role::Sol, Face::Top),
    (Role::Plafond, Face::Bottom),
    (Role::MurGauche, Face::Left),
    (Role::MurDroit, Face::Right),
];

/// Le rectangle fin d'une des quatre plateformes d'une fenêtre.
///
/// Chaque rectangle est placé pour que `point_on` de sa face tombe **sur le
/// bord visuel** de la fenêtre : c'est la seule propriété qui compte, et
/// c'est elle qui fait que le personnage a l'air posé dessus et non dedans.
fn rect_de_face(f: Rect, role: Role) -> Rect {
    match role {
        // `top()` du rectangle = haut de la fenêtre : on marche dessus.
        Role::Sol => Rect::new(f.left(), f.top(), f.w, EPAISSEUR_PLATEFORME),

        // `bottom()` du rectangle = bas de la fenêtre : on s'y suspend.
        Role::Plafond => Rect::new(
            f.left(),
            f.bottom() - EPAISSEUR_PLATEFORME,
            f.w,
            EPAISSEUR_PLATEFORME,
        ),

        // `left()` du rectangle = bord gauche de la fenêtre.
        Role::MurGauche => Rect::new(f.left(), f.top(), EPAISSEUR_PLATEFORME, f.h),

        // `right()` du rectangle = bord droit de la fenêtre.
        Role::MurDroit => Rect::new(
            f.right() - EPAISSEUR_PLATEFORME,
            f.top(),
            EPAISSEUR_PLATEFORME,
            f.h,
        ),
    }
}

/// L'ombre que la fenêtre `devant` porte sur une face de la fenêtre `f`, en
/// distances au bord de cette face.
///
/// Rend `None` si `devant` ne touche pas la ligne de la face — le cas le plus
/// fréquent, et c'est pour ça que la fonction rend une `Option` plutôt qu'un
/// intervalle vide : `filter_map` écarte alors le cas sans allouer.
///
/// **Une face est une LIGNE**, pas une bande : on teste donc si `devant`
/// contient cette ligne (une coordonnée), puis on projette son étendue sur
/// l'autre axe. C'est tout ce que « soustraction d'intervalles 1D » veut
/// dire, et c'est ce qui rend la décision n° 2 si bon marché.
fn ombre_sur(f: Rect, role: Role, devant: Rect) -> Option<(f32, f32)> {
    match role {
        // ── Les deux faces horizontales ─────────────────────────────────
        // La ligne est à une hauteur `y` ; `devant` la coupe s'il l'encadre
        // verticalement. On projette alors son étendue horizontale.
        Role::Sol | Role::Plafond => {
            let y = if role == Role::Sol { f.top() } else { f.bottom() };
            if devant.top() > y || devant.bottom() < y {
                return None;
            }
            Some((devant.left() - f.left(), devant.right() - f.left()))
        }

        // ── Les deux faces verticales ───────────────────────────────────
        // Symétrique : la ligne est à une abscisse `x`, et on projette
        // l'étendue verticale. L'offset d'une face `Left`/`Right` compte
        // vers le BAS depuis le haut du rectangle (convention de
        // `Rect::point_on`), d'où le `- f.top()`.
        Role::MurGauche | Role::MurDroit => {
            let x = if role == Role::MurGauche {
                f.left()
            } else {
                f.right()
            };
            if devant.left() > x || devant.right() < x {
                return None;
            }
            Some((devant.top() - f.top(), devant.bottom() - f.top()))
        }
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
            platforms.push(Platform::entiere(
                PlatformId::ecran(s.id, Role::Sol),
                Rect::new(z.left(), z.bottom(), z.w, EPAISSEUR_PLATEFORME),
                PlatformKind::Screen,
                0,
                Face::Top,
            ));

            // ── Le plafond ──────────────────────────────────────────────
            // Posé JUSTE AU-DESSUS de la zone de travail, et c'est sa face
            // `Bottom` qui est exposée : on s'y suspend par en dessous. Le
            // `- EPAISSEUR` place le rectangle de sorte que `bottom()` tombe
            // exactement sur `z.top()`.
            //
            // Jamais supprimé, lui : il n'y a rien au-dessus du bureau.
            platforms.push(Platform::entiere(
                PlatformId::ecran(s.id, Role::Plafond),
                Rect::new(
                    z.left(),
                    z.top() - EPAISSEUR_PLATEFORME,
                    z.w,
                    EPAISSEUR_PLATEFORME,
                ),
                PlatformKind::Screen,
                0,
                Face::Bottom,
            ));

            // ── Les deux murs, si personne ne les touche ────────────────
            //
            // Le mur GAUCHE expose sa face `Right` : le personnage se tient
            // à sa droite, c'est-à-dire à l'intérieur de l'écran. C'est la
            // même logique que le sol, dont la face `Top` regarde vers le
            // haut, donc vers l'intérieur (design §2.1).
            if !ecran_adjacent(screens, s, false) {
                platforms.push(Platform::entiere(
                    PlatformId::ecran(s.id, Role::MurGauche),
                    Rect::new(
                        z.left() - EPAISSEUR_PLATEFORME,
                        z.top(),
                        EPAISSEUR_PLATEFORME,
                        z.h,
                    ),
                    PlatformKind::Screen,
                    0,
                    Face::Right,
                ));
            }

            if !ecran_adjacent(screens, s, true) {
                platforms.push(Platform::entiere(
                    PlatformId::ecran(s.id, Role::MurDroit),
                    Rect::new(z.right(), z.top(), EPAISSEUR_PLATEFORME, z.h),
                    PlatformKind::Screen,
                    0,
                    Face::Left,
                ));
            }
        }

        World { platforms }
    }


    /// Le monde complet : les écrans **et les fenêtres** (design §5.1,
    /// étape 4b).
    ///
    /// `fenetres` arrive déjà filtrée par la sonde (design §5.3) et rangée
    /// **du premier plan vers l'arrière** — c'est ce qui rend l'occlusion
    /// possible sans interroger quoi que ce soit de plus.
    ///
    /// # ⚠️ L'occlusion ne s'applique qu'entre FENÊTRES — précision de
    /// périmètre
    ///
    /// La décision n° 2 dit « un bord recouvert n'est pas exposé ». Appliquée
    /// littéralement aux plateformes d'**écran**, elle supprimerait le sol du
    /// bureau dès qu'une fenêtre le recouvre — c'est-à-dire presque toujours,
    /// et complètement dès qu'une fenêtre est maximisée. Le personnage
    /// n'aurait alors nulle part où marcher sur un bureau ordinaire, ce qui
    /// n'est évidemment pas ce que la décision cherchait.
    ///
    /// Ce que la décision cherchait, c'est qu'il ne s'assoie pas sur une
    /// barre de titre **cachée derrière une autre fenêtre** — un bord qu'on
    /// ne voit pas. Le sol de l'écran, lui, n'est jamais « caché » en ce
    /// sens : c'est le bas du bureau, et un personnage qui y marche se lit
    /// comme un personnage au bas de l'écran, exactement comme dans
    /// Shimeji-ee.
    ///
    /// Les quatre plateformes de chaque écran restent donc entières.
    pub fn from_screens_and_windows(screens: &[ScreenInfo], fenetres: &[WindowInfo]) -> World {
        let mut monde = World::from_screens(screens);

        for (rang, f) in fenetres.iter().enumerate() {
            // Les fenêtres **devant celle-ci** sont les seules à pouvoir la
            // masquer. La liste étant rangée du premier plan vers l'arrière,
            // ce sont exactement celles qui précèdent — d'où le `&[..rang]`,
            // qui est toute la gestion du z-order de ce fichier.
            let devant = &fenetres[..rang];

            for (role, face) in ROLES_DE_FENETRE {
                let rect = rect_de_face(f.rect, *role);

                // Les intervalles que les fenêtres de devant retirent à cette
                // face, en distances au bord de la face.
                let occultants: Vec<(f32, f32)> = devant
                    .iter()
                    .filter_map(|o| ombre_sur(f.rect, *role, o.rect))
                    .collect();

                let libre = soustraire_intervalles(rect.face_length(*face), &occultants);

                // Entièrement recouverte : on ne l'expose pas du tout. Une
                // plateforme sans un seul morceau praticable serait un piège —
                // `nearest_floor` et les intentions la verraient comme une
                // cible valable, et il s'y rendrait pour rien.
                if libre.is_empty() {
                    continue;
                }

                monde.platforms.push(Platform {
                    id: PlatformId::fenetre(f.hwnd, *role),
                    rect,
                    kind: PlatformKind::Window,
                    z: f.z,
                    faces: vec![*face],
                    libre,
                });
            }
        }

        monde
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
