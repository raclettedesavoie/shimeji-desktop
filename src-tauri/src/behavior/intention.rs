//! Couche 2 du comportement : l'intention en cours (spec §7.1, §7.3).
//!
//! Responsabilité unique : traduire une intention en déplacement et en pose,
//! et dire quand elle est finie, échouée ou expirée. **Une seule intention à
//! la fois.**
//!
//! **Décision n° 4 — la navigation est autorisée à échouer.** Aucun calcul de
//! chemin : la carte des plateformes change 8 fois par seconde, un chemin est
//! périmé avant d'être parcouru. Décision **locale** à chaque image, et une
//! seule règle de sécurité qui remplace toute l'énumération des cas de
//! blocage :
//!
//! > **Toute intention a un délai d'abandon (~20 s)**, après quoi elle échoue
//! > et il repart flâner.
//!
//! Il se coince parfois, il prend des routes idiotes — **et c'est
//! souhaitable** : un pet qui prend un chemin bête est attachant, un qui
//! calcule l'itinéraire optimal a l'air d'un robot (spec §7.3).

use crate::character::attach::Attachment;
use crate::character::manifest::{
    POSE_CLIMB_CEILING, POSE_CLIMB_WALL, POSE_GRAB_CEILING, POSE_GRAB_WALL, POSE_RUN, POSE_SIT,
    POSE_SIT_DANGLE, POSE_SLEEP, POSE_SPIN_HEAD, POSE_STAND, POSE_WAKE, POSE_WALK,
};
use crate::config::Reglages;
use crate::character::Character;
use crate::character::Facing;
use crate::geom::{Face, Point};
use crate::rng::Rng;
use crate::world::{PlatformId, World};
use std::time::Duration;

/// Le délai au bout duquel **toute** intention échoue (spec §7.3).
///
/// Uniforme, sans exception — y compris pour `Flaner`, qui pourrait durer
/// indéfiniment. L'exception serait une deuxième règle à retenir, et la
/// flânerie qui expire produit simplement un nouveau tirage : de la variété
/// gratuite.
pub const DELAI_ABANDON: Duration = Duration::from_secs(20);

/// Délai d'abandon de 120 s pour l'escalade, à vitesse ×1 (design §4.4).
///
/// **Pourquoi pas 20 s comme le reste.** L'escalade va à 16,1 px/s : un mur
/// de 1032 px prend 64 s, et la marche jusqu'au bord en ajoute jusqu'à 38 —
/// **un écran entier**, 1920 px, et non sa moitié : sur deux écrans côte à
/// côte, chaque écran n'expose qu'UN SEUL mur (design §2.3), donc le pire
/// cas n'est pas de se trouver déjà au milieu, il est de partir de l'autre
/// bord. Avec le délai commun de 20 s, il abandonnerait toujours au tiers du
/// mur et n'atteindrait jamais le plafond.
///
/// La décision n° 4 écrit « ~20 s » avec un tilde : c'est une règle de
/// sécurité anti-blocage, pas un trait de caractère, et elle n'a pas de
/// raison d'être identique pour une intention trois fois plus lente.
///
/// ⚠️ **Cette constante seule ne suffit plus** depuis que `vitesse` est
/// réglable (`config.json`) : voir `delai_abandon`, qui la corrige par le
/// facteur de l'utilisateur.
pub const DELAI_ABANDON_GRIMPE: Duration = Duration::from_secs(120);

/// Le délai d'abandon qui s'applique à cette intention-là.
///
/// Une fonction et non une méthode de `Intention` : le délai est une règle
/// du moteur de comportement, pas une propriété de l'étiquette — la même
/// raison qui a fait de `vitesse_de(allure)` une fonction libre.
///
/// ⚠️ **Prend maintenant les réglages, et c'est une correction, pas un
/// confort** (relecture finale de l'étape 4a). `DELAI_ABANDON_GRIMPE` est une
/// CONSTANTE, mais `vitesse_escalade` (comme `vitesse_marche`) est multipliée
/// par le facteur `vitesse` de `config.json`, borné à `FACTEUR_VITESSE_MIN =
/// 0.1`. Un délai fixe face à des vitesses réglables est un piège : à ×0.5,
/// l'escalade complète calculée ci-dessus (102 s à ×1) passe à 205 s contre
/// un abandon toujours fixé à 120 s — **toute** escalade expirerait aux deux
/// tiers du mur, le personnage tomberait, et comme `Grimper` garde son poids
/// dans le tirage, il recommencerait aussitôt. Il passerait sa vie à tomber
/// des murs, sans qu'aucune ligne du code n'ait l'air fausse en la relisant
/// isolément — c'est exactement le symptôme que le design §4.4 décrit pour
/// justifier les 120 s, réintroduit par un chemin que personne n'avait
/// regardé.
///
/// Le facteur est déductible de `vitesse_marche`, déjà calculé par
/// `Reglages::depuis` : `reglages.vitesse_marche / VITESSE_MARCHE`. Diviser
/// le délai par ce même facteur garde la marge de 15 % constante quel que
/// soit le réglage, plutôt que de la faire fondre à mesure que `vitesse`
/// baisse — voir le test `une_escalade_complete_tient_dans_le_delai_d_abandon`,
/// qui le vérifie à plusieurs facteurs, dont le minimum autorisé (0.1).
pub fn delai_abandon(kind: Intention, reglages: &Reglages) -> Duration {
    match kind {
        Intention::Grimper => {
            let facteur = reglages.vitesse_marche / crate::character::physics::VITESSE_MARCHE;
            Duration::from_secs_f32(DELAI_ABANDON_GRIMPE.as_secs_f32() / facteur)
        }
        // `|` : les trois autres partagent le délai commun. Un `_` les
        // couvrirait aussi, mais il avalerait silencieusement toute
        // intention future — alors que ce `match` exhaustif obligera à se
        // poser la question.
        //
        // Elles ne sont PAS corrigées par le facteur de vitesse : leur délai
        // de 20 s est une règle de sécurité anti-blocage générique (décision
        // n° 4), pas un calcul de traversée comme celui de l'escalade — rien
        // dans leur conception n'affirme qu'il doit couvrir un trajet complet
        // à vitesse réduite.
        Intention::Flaner | Intention::SeReposer | Intention::Jouer(_) => DELAI_ABANDON,
    }
}

/// À quoi il joue.
///
/// Un `enum` et non un nom de pose libre : la table d'envies a besoin d'une
/// clé `Copy + Eq`, et une variante par animation permet à la couverture
/// partielle de les retirer **séparément** (spec §8.6).
///
/// C'est la lecture littérale de `Jouer(action)` de la spec §7.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Jeu {
    /// Assis, la tête qui tourne — 8 frames, 200 ms chacune.
    TeteQuiTourne,
    /// Assis à balancer les jambes — 4 frames, 400 ms, en boucle.
    JambesQuiBalancent,
}

impl Jeu {
    /// La pose que ce jeu demande. **Une seule** : c'est ce qui rend le
    /// retrait par couverture partielle exact — une animation absente ne
    /// retire que son propre jeu.
    pub fn pose(&self) -> &'static str {
        match self {
            Jeu::TeteQuiTourne => POSE_SPIN_HEAD,
            Jeu::JambesQuiBalancent => POSE_SIT_DANGLE,
        }
    }

    /// Les poses requises, sous la forme attendue par la table d'envies.
    ///
    /// `&'static [&'static str]` : la table stocke des tranches statiques
    /// pour n'allouer jamais. Une constante par jeu, donc, plutôt qu'un
    /// `Vec` construit à la volée.
    pub fn poses_requises(&self) -> &'static [&'static str] {
        match self {
            Jeu::TeteQuiTourne => &[POSE_SPIN_HEAD],
            Jeu::JambesQuiBalancent => &[POSE_SIT_DANGLE],
        }
    }
}

/// Ce que le personnage est en train d'essayer de faire.
///
/// **Une seule à la fois** (spec §7.1). `AllerA(surface)` arrive à l'étape 5
/// — ce sera une variante de plus, et une ligne de plus dans la table
/// d'envies.
///
/// `PartialOrd, Ord` : uniquement pour que les tests puissent ranger des
/// intentions dans un `BTreeSet` (« quelles intentions ai-je vues ? »). Le
/// comportement lui-même ne compare jamais deux intentions par ordre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Intention {
    Flaner,
    SeReposer,
    Jouer(Jeu),

    /// Aller sur un mur et y monter (étape 4a).
    ///
    /// **Cette intention possède tout le monde vertical**, et `Flaner`
    /// continue de ne connaître que le sol. L'alternative — généraliser
    /// `Flaner` à n'importe quelle face — est un piège : `avancer` déplace
    /// l'offset *le long de la face courante*, donc un `Flaner` sur un mur
    /// ferait monter et descendre le personnage en pose de marche, allure et
    /// demi-tours compris. Séparer coûte une variante ; fondre coûterait une
    /// matrice pose × face (design §4.1).
    Grimper,
}

/// À quelle vitesse il se déplace pendant une flânerie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allure {
    Arret,
    Marche,
    Course,
}

/// La vitesse d'une allure, d'après les réglages de l'utilisateur.
///
/// Fonction libre et non méthode de `Allure` : la vitesse n'est plus une
/// propriété intrinsèque de l'allure, elle dépend de la configuration.
/// Garder la méthode obligerait à donner à `Allure` une référence aux
/// réglages, ce qui n'a pas de sens pour une étiquette.
fn vitesse_de(allure: Allure, reglages: &Reglages) -> f32 {
    match allure {
        Allure::Arret => 0.0,
        Allure::Marche => reglages.vitesse_marche,
        Allure::Course => reglages.vitesse_course,
    }
}

impl Allure {
    fn pose(&self) -> &'static str {
        match self {
            Allure::Arret => POSE_STAND,
            Allure::Marche => POSE_WALK,
            Allure::Course => POSE_RUN,
        }
    }
}

/// Où en est un repos.
///
/// Deux phases et non deux intentions : « dormir » n'est pas un choix
/// distinct de « se reposer », c'est **la suite** de se reposer quand un
/// signal pousse dans ce sens. En faire deux lignes de table demanderait au
/// tirage de savoir qu'on est déjà assis, ce qui n'a rien à y faire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseRepos {
    Assis,
    Endormi,
    /// Il émerge : le sommeil finit, puis il se redresse.
    ///
    /// **Une phase et non une intention** — exactement pour la raison qui
    /// fait déjà d'`Endormi` une phase : le réveil n'est pas un choix, c'est
    /// *la suite* d'un repos. Le mettre dans la table d'envies demanderait au
    /// tirage de savoir qu'on dormait, ce qui n'a rien à y faire.
    ///
    /// ⚠️ **Elle ne se tire jamais** : `desire.rs` ne la propose pas, et
    /// `ActiveIntention::nouvelle` part toujours en `Assis`. Elle est POSÉE,
    /// depuis `main.rs`, au déverrouillage de la session — voir
    /// `ActiveIntention::reveil`.
    ///
    /// Et c'est aussi ce qui la rend **ininterruptible** sans une ligne de
    /// code : l'interruption de `behavior::mod` ne vise que `Endormi`. Le
    /// personnage vient de taper son mot de passe, il est donc « actif » au
    /// sens du signal — si le réveil était une phase `Endormi`, il serait
    /// coupé à la première image et l'on ne verrait rien.
    Selevant,
}

/// Où en est une escalade.
///
/// Même forme que `PhaseRepos`, et pour la même raison : ce ne sont pas des
/// choix distincts, c'est *la suite* d'une même intention. Les mettre dans la
/// table d'envies demanderait au tirage de savoir où le personnage est
/// accroché, ce qui n'a rien à y faire.
///
/// `PartialEq` sans `Eq` : la variante `Paroi` porte un `f32`, et `f32`
/// n'implémente pas `Eq` en Rust (NaN n'est égal à rien, pas même à
/// lui-même). C'est la même raison qui prive déjà `EtatIntention` d'`Eq`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PhaseGrimpe {
    /// Première image : choisir le mur. Existe pour que
    /// `ActiveIntention::nouvelle` reste **sans `World` ni `Rng`** — c'est
    /// déjà le parti pris des autres intentions, dont l'état initial est
    /// délibérément périmé pour que la première image décide.
    Choisir,

    /// Marcher vers le mur retenu. On mémorise **son identité**, jamais sa
    /// position : le monde est reconstruit à 8 Hz, et une position serait
    /// périmée (décision n° 1).
    Rejoindre { mur: PlatformId },

    /// Se déplacer le long de la paroi vers `cible`.
    ///
    /// La phase s'appelle `Paroi` et non `Monter` parce qu'elle sert dans les
    /// **deux sens** : monter, c'est une cible plus petite que l'offset
    /// courant ; redescendre, une cible plus grande. C'est la structure de
    /// `ClimbWall` chez Shimeji-ee, dont les deux animations sont
    /// conditionnées par `TargetY < mascot.anchor.y`.
    Paroi { cible: f32 },

    /// Accroché, immobile, le temps tiré au sort.
    Accroche,

    /// Se déplacer le long du plafond vers `cible`, un offset horizontal.
    ///
    /// Une phase distincte de `Paroi` et non un paramètre de face : les deux
    /// n'ont ni la même pose, ni le même axe de déplacement (vertical pour
    /// l'un, horizontal pour l'autre), ni la même sortie. Les fondre
    /// demanderait un `match` sur la face dans chaque ligne du corps —
    /// séparer coûte une variante, fondre coûterait une matrice pose × face
    /// (design §4.1).
    Plafond { cible: f32 },
}

/// L'état interne d'une intention en cours.
///
/// Séparé de `Intention` : celle-ci est une **étiquette** (`Copy`, `Eq`),
/// utilisable comme clé dans la table d'envies, alors que celui-ci porte des
/// données qui changent à chaque image. Fondre les deux obligerait la table
/// d'envies à connaître des durées, ce qui n'a rien à y faire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EtatIntention {
    Flanerie { allure: Allure, jusqu_a: Duration },
    Repos {
        phase: PhaseRepos,
        jusqu_a: Duration,
    },
    Jeu { jusqu_a: Duration },

    /// L'escalade en cours. `jusqu_a` n'est lu que par la phase `Accroche` —
    /// les trois autres n'ont pas de durée, elles ont un but à atteindre.
    Grimpe {
        phase: PhaseGrimpe,
        jusqu_a: Duration,
    },
}

/// Une intention en cours, avec le moment où elle a commencé — c'est de là
/// que se déduit le délai d'abandon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveIntention {
    pub kind: Intention,
    pub depuis: Duration,
    pub etat: EtatIntention,
}

impl ActiveIntention {
    /// Crée une intention fraîche.
    ///
    /// L'état initial est délibérément **déjà périmé** (`jusqu_a` à zéro) :
    /// la première image de `poursuivre` tirera donc une allure ou une durée
    /// de repos. Cela évite d'exiger un `Rng` ici — ce qui permet à
    /// `reflex.rs` et aux tests d'en construire une sans générateur.
    pub fn nouvelle(kind: Intention, maintenant: Duration) -> Self {
        let etat = match kind {
            Intention::Flaner => EtatIntention::Flanerie {
                allure: Allure::Arret,
                jusqu_a: Duration::ZERO,
            },
            Intention::SeReposer => EtatIntention::Repos {
                phase: PhaseRepos::Assis,
                jusqu_a: Duration::ZERO,
            },
            Intention::Jouer(_) => EtatIntention::Jeu {
                jusqu_a: Duration::ZERO,
            },
            // `Choisir` est l'équivalent du `jusqu_a: ZERO` des autres : un
            // état volontairement « pas encore décidé », que la première
            // image de `grimper` tranchera — c'est ce qui dispense cette
            // fonction d'un `World` et d'un `Rng`.
            Intention::Grimper => EtatIntention::Grimpe {
                phase: PhaseGrimpe::Choisir,
                jusqu_a: Duration::ZERO,
            },
        };
        ActiveIntention {
            kind,
            depuis: maintenant,
            etat,
        }
    }

    /// L'intention « il émerge », posée au déverrouillage de la session.
    ///
    /// Même principe que `nouvelle` : `jusqu_a` à zéro, donc la première
    /// image de `se_reposer` tirera la durée. C'est ce qui permet à `main.rs`
    /// de la construire **sans générateur aléatoire** et, surtout, qui garde
    /// toutes les durées d'animation dans ce fichier-ci.
    pub fn reveil(maintenant: Duration) -> Self {
        ActiveIntention {
            kind: Intention::SeReposer,
            depuis: maintenant,
            etat: EtatIntention::Repos {
                phase: PhaseRepos::Selevant,
                jusqu_a: Duration::ZERO,
            },
        }
    }

    /// L'intention posée quand un lancer vient de le coller à une paroi —
    /// un mur **ou** le plafond.
    ///
    /// **Elle est indispensable, et sa raison n'est pas évidente.** Laisser
    /// `intention = None` ferait rendre `Finie` à la couche 2, et la règle de
    /// sécurité du monde vertical le ferait tomber à l'image suivante : jeté
    /// contre un mur ou vers le plafond, il ne tiendrait qu'une image.
    ///
    /// > Rebaptisée `accroche` (elle s'appelait `accroche_au_mur`) le jour où
    /// > le plafond a appris à attraper lui aussi (design §3.2, révisé le
    /// > 2026-09-12) : son nom ne disait plus tout ce qu'elle fait. La
    /// > fonction elle-même n'a pas changé — c'est `reflex.rs` qui l'appelle
    /// > maintenant depuis deux bras (`Face::Left | Right` et `Face::Bottom`)
    /// > au lieu d'un seul.
    ///
    /// Même motif qu'`ActiveIntention::reveil` : l'état est POSÉ de
    /// l'extérieur, avec `jusqu_a` à zéro pour que la première image tire la
    /// durée — ce qui permet à `reflex.rs` de la construire **sans générateur
    /// aléatoire**, et garde toutes les durées dans ce fichier-ci.
    pub fn accroche(maintenant: Duration) -> Self {
        ActiveIntention {
            kind: Intention::Grimper,
            depuis: maintenant,
            etat: EtatIntention::Grimpe {
                phase: PhaseGrimpe::Accroche,
                jusqu_a: Duration::ZERO,
            },
        }
    }
}

/// Où en est l'intention à la fin de cette image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    EnCours,
    /// Menée à son terme normalement.
    Finie,
    /// Impossible, ou expirée au délai d'abandon. Dans les deux cas, la
    /// couche 3 en tirera une autre — c'est pourquoi les deux ne sont pas
    /// distinguées plus finement : rien n'en dépend.
    Echouee,
}

/// Fait avancer l'intention en cours d'un pas de temps.
///
/// Rend `Finie` s'il n'y a aucune intention : l'appelant (`behavior::pas`) en
/// tire alors une nouvelle. C'est plus simple qu'un `Option<Issue>`, et ça
/// évite un cas particulier au point d'appel.
///
/// `e` porte le biais d'envie (décision n° 3) — seul `se_reposer` le
/// consulte, pour décider de s'affaler plutôt que de rester assis. Il
/// descend jusqu'ici, et non directement dans `se_reposer` depuis
/// `behavior::pas`, pour que les trois intentions gardent la même signature :
/// c'est `poursuivre` qui aiguille, pas l'appelant.
pub fn poursuivre(
    ch: &mut Character,
    world: &World,
    e: &super::Entrees,
    reglages: &Reglages,
    maintenant: Duration,
    dt: f32,
    rng: &mut dyn Rng,
) -> Issue {
    // `let Some(...) else` : pas d'intention, rien à poursuivre.
    let Some(mut ai) = ch.intention else {
        return Issue::Finie;
    };

    // ── Le délai d'abandon, avant tout le reste ─────────────────────────
    // Décision n° 4. `saturating_sub` : si l'horloge de test recule (elle
    // le peut, `FakeClock::set` existe), on ne veut pas de débordement.
    //
    // ⚠️ On ne lâche PAS ici, contrairement à une version antérieure. Voir
    // le commentaire du point d'étranglement unique, en bas de cette
    // fonction, pour la raison : appeler `lacher_si_accroche` à cet endroit
    // ET après le `match` ci-dessous aurait recréé exactement le défaut que
    // cette vague de relecture corrige — la même règle vivant à deux
    // endroits, avec un risque qu'un futur point de sortie (une cinquième
    // intention, une nouvelle phase) n'en voie qu'un des deux.
    let issue = if maintenant.saturating_sub(ai.depuis) > delai_abandon(ai.kind, reglages) {
        ch.intention = None;
        Issue::Echouee
    } else {
        match ai.kind {
            Intention::Flaner => {
                let issue = flaner(ch, world, reglages, &mut ai, maintenant, dt, rng);
                // On réécrit l'intention : `ai` est une COPIE (le type est
                // `Copy`), donc modifier `ai.etat` ne touche pas `ch.intention`
                // tant qu'on ne le réaffecte pas. Oublier cette ligne donnerait
                // un personnage qui retire une allure à chaque image.
                //
                // `if` : `flaner` a pu annuler l'intention (elle est alors
                // `None`) — la réécrire l'aurait ressuscitée.
                if ch.intention.is_some() {
                    ch.intention = Some(ai);
                }
                issue
            }

            Intention::SeReposer => {
                let issue = se_reposer(ch, &mut ai, e, reglages, maintenant, rng);
                if ch.intention.is_some() {
                    ch.intention = Some(ai);
                }
                issue
            }

            Intention::Jouer(jeu) => {
                let issue = jouer(ch, jeu, &mut ai, maintenant, rng);
                if ch.intention.is_some() {
                    ch.intention = Some(ai);
                }
                issue
            }

            Intention::Grimper => {
                let issue = grimper(ch, world, reglages, &mut ai, maintenant, dt, rng);
                if ch.intention.is_some() {
                    ch.intention = Some(ai);
                }
                issue
            }
        }
    };

    // ── Le point d'étranglement unique de la règle du monde vertical ────
    //
    // ⚠️ **Bug corrigé (relecture finale de l'étape 4a).** `grimper()` a SEPT
    // points de sortie (`ch.intention = None; return Issue::…`), et deux
    // d'entre eux laissent le personnage accroché à une face non-`Top` : la
    // garde `face != Face::Top` de la phase `Choisir`, et la branche `None`
    // de `sol_au_pied_du_mur` — cette dernière deviendra SYSTÉMATIQUE dès que
    // les murs de fenêtres n'auront pas de sol au même écran. Une version
    // antérieure n'appelait `lacher_si_accroche` qu'au moment précis où le
    // délai d'abandon expirait (voir le commentaire ci-dessus, maintenant
    // supprimé de cet endroit) : c'était le bug de la Tâche 7 tel quel,
    // déplacé d'un cran — la couche 3 re-tirait aussitôt un `Grimper` neuf
    // sur un personnage toujours accroché, et la garde de `behavior::pas` ne
    // voyait jamais l'image où il aurait fallu lâcher.
    //
    // La correction : un SEUL appel, ici, qui couvre les quatre intentions et
    // toutes leurs sorties d'un coup — que l'issue vienne du délai d'abandon
    // ci-dessus ou de n'importe quel `return Issue::Echouee`/`Finie` à
    // l'intérieur de `flaner`/`se_reposer`/`jouer`/`grimper`. C'est le même
    // principe que la factorisation de `lacher_si_accroche` elle-même : une
    // règle qui vit à un seul endroit ne peut pas en oublier un second.
    //
    // `EnCours` ne déclenche rien : l'intention continue, il n'y a rien à
    // juger. C'est seulement quand elle FINIT — d'une façon ou d'une autre —
    // qu'il faut vérifier s'il reste accroché sans raison de l'être.
    if issue != Issue::EnCours {
        super::lacher_si_accroche(ch, world);
    }

    issue
}

/// Flâner : avancer, s'arrêter, courir, faire demi-tour, changer d'écran.
fn flaner(
    ch: &mut Character,
    world: &World,
    reglages: &Reglages,
    ai: &mut ActiveIntention,
    maintenant: Duration,
    dt: f32,
    rng: &mut dyn Rng,
) -> Issue {
    // On n'extrait l'état que sous la bonne variante. Une intention `Flaner`
    // avec un état `Repos` serait un bug de construction ; `else` le traite
    // comme un échec plutôt que par un `panic!`.
    let EtatIntention::Flanerie {
        mut allure,
        mut jusqu_a,
    } = ai.etat
    else {
        ch.intention = None;
        return Issue::Echouee;
    };

    // ── Renouveler l'allure quand la précédente a expiré ────────────────
    if maintenant >= jusqu_a {
        // Poids, durées et chance de demi-tour viennent maintenant des
        // réglages (décision n° 5 : « les poids vivent dans la config, on
        // règle son caractère sans recompiler »).
        //
        // Le dosage par défaut — il s'arrête souvent, marche beaucoup, et
        // **ne court pas** (`poids_course` vaut 0) — est ce qui donne
        // l'impression de flânerie plutôt que d'agitation. La course reste
        // atteignable par ce même tirage si on remonte son poids dans la
        // config, mais elle est destinée à des actions qui la demandent.
        let a = &reglages.allures;
        let poids = [a.poids_arret, a.poids_marche, a.poids_course];

        allure = match rng.weighted(&poids) {
            Some(0) => Allure::Arret,
            Some(2) => Allure::Course,
            // `Some(1)` tombe sur la marche, et le cas `None` aussi — il
            // n'arrive que si l'utilisateur a mis les trois poids à zéro,
            // auquel cas marcher est le repli le moins surprenant.
            _ => Allure::Marche,
        };

        // Une durée aléatoire, plus courte pour la course par défaut : un pet
        // qui court dix secondes d'affilée a l'air pressé, pas vivant.
        let plage = match allure {
            Allure::Arret => a.duree_arret,
            Allure::Marche => a.duree_marche,
            Allure::Course => a.duree_course,
        };
        jusqu_a = maintenant + Duration::from_secs_f32(rng.range(plage[0], plage[1]));

        // Un demi-tour de temps en temps, sans raison : c'est ce qui empêche
        // de deviner la suite. C'est LE réglage qui décide si le personnage
        // paraît décidé ou indécis.
        if rng.unit_f32() < a.chance_demi_tour {
            ch.facing = ch.facing.inverse();
        }
    }

    // ── La pose suit l'allure ───────────────────────────────────────────
    // `set_pose` ignore une pose absente du manifeste (couverture partielle,
    // spec §8.6) : un personnage sans `run` gardera donc sa pose de marche
    // en courant, plutôt que de n'afficher rien. On demande explicitement
    // `stand` en repli pour que l'arrêt reste visible.
    let pose_voulue = allure.pose();
    if ch.manifest.has_pose(pose_voulue) {
        ch.set_pose(pose_voulue, maintenant);
    } else {
        ch.set_pose(POSE_STAND, maintenant);
    }

    // ── Le déplacement, et ce qui arrive au bord ────────────────────────
    if allure != Allure::Arret {
        avancer(ch, world, vitesse_de(allure, reglages) * ch.facing.signe() * dt);
    }

    ai.etat = EtatIntention::Flanerie { allure, jusqu_a };
    Issue::EnCours
}

/// Grimper : rejoindre un mur, y monter, s'y accrocher, puis en sortir.
///
/// Les transitions de coin (sol → mur, mur → sol) vivent ici et non dans
/// `world.rs` parce que ce sont des **décisions de navigation**, pas des
/// propriétés du monde — la même raison qui place déjà `face_voisine` dans ce
/// fichier (design §4.2).
///
/// Les quatre phases s'enchaînent **une par image** : chaque appel n'en
/// exécute qu'une, et écrit la suivante dans `ai.etat`. C'est ce qui rend la
/// fonction lisible sans boucle interne, au prix d'une image de transition
/// que personne ne voit à 60 Hz.
fn grimper(
    ch: &mut Character,
    world: &World,
    reglages: &Reglages,
    ai: &mut ActiveIntention,
    maintenant: Duration,
    dt: f32,
    rng: &mut dyn Rng,
) -> Issue {
    // Même motif que `flaner` : on n'extrait l'état que sous la bonne
    // variante, et une incohérence de construction se solde par un échec
    // plutôt que par un `panic!`.
    let EtatIntention::Grimpe {
        mut phase,
        mut jusqu_a,
    } = ai.etat
    else {
        ch.intention = None;
        return Issue::Echouee;
    };

    match phase {
        // ── Choisir le mur ──────────────────────────────────────────────
        PhaseGrimpe::Choisir => {
            // `let … else` : s'il n'est pas posé quelque part, il n'y a pas
            // d'écran de référence. Les réflexes s'occupent de lui.
            let Attachment::On { platform, face, .. } = ch.attachment else {
                ch.intention = None;
                return Issue::Echouee;
            };

            // ⚠️ **Garde structurelle (Tâche 7, après le bug du délai
            // d'abandon).** `Rejoindre`, la phase suivante, est une marche
            // AU SOL : elle pose `walk` et avance sans jamais vérifier sur
            // quelle face il se trouve. Avec `lacher_si_accroche` appelée au
            // bon endroit, `Choisir` ne devrait plus jamais être atteinte
            // depuis un mur ou le plafond — mais cette garde ne DÉPEND pas
            // de ça : elle refuse ici, structurellement, plutôt que de
            // compter sur une règle lointaine (dans un autre fichier) pour
            // ne jamais être violée. Une garde redondante coûte deux lignes ;
            // son absence a coûté un bug qui a survécu trois tâches et
            // leurs relectures.
            //
            // `ActiveIntention::accroche` n'est PAS concernée : elle
            // pose directement la phase `Accroche`, jamais `Choisir` — le
            // lancer contre un mur continue de fonctionner sans passer ici.
            if face != Face::Top {
                ch.intention = None;
                return Issue::Echouee;
            }

            let Some(mur) = mur_le_plus_proche(world, platform, ch) else {
                // Aucun mur sur cet écran — l'écran du milieu d'une rangée de
                // trois. L'intention échoue, la couche 3 en tire une autre.
                // **Aucun cas particulier ailleurs** : c'est le même esprit
                // que la couverture partielle (spec §8.6).
                ch.intention = None;
                return Issue::Echouee;
            };

            phase = PhaseGrimpe::Rejoindre { mur };
        }

        // ── Marcher jusqu'au pied du mur ────────────────────────────────
        PhaseGrimpe::Rejoindre { mur } => {
            let Some(plat_mur) = world.get(mur) else {
                // Écran débranché en cours de route.
                ch.intention = None;
                return Issue::Echouee;
            };

            // La face du mur est la seule de sa liste — un mur n'en expose
            // qu'une. `copied()` transforme l'`Option<&Face>` rendue par
            // `first()` en `Option<Face>` : `Face` est `Copy`, et l'on
            // préfère la valeur à la référence pour ne pas garder d'emprunt.
            let Some(face_mur) = plat_mur.faces.first().copied() else {
                ch.intention = None;
                return Issue::Echouee;
            };

            let x_mur = plat_mur.rect.point_on(face_mur, 0.0).x;
            let Some(pos) = position_actuelle(ch, world) else {
                ch.intention = None;
                return Issue::Echouee;
            };

            // Il regarde le mur, et il marche vers lui.
            ch.facing = if x_mur < pos.x {
                Facing::Left
            } else {
                Facing::Right
            };
            ch.set_pose(POSE_WALK, maintenant);

            let pas = reglages.vitesse_marche * dt;

            if (x_mur - pos.x).abs() <= pas {
                // Arrivé : on s'accroche au BAS du mur. L'offset d'une face
                // verticale compte vers le bas depuis le haut du rectangle,
                // donc le bas du mur est à `face_length`.
                let longueur = plat_mur.rect.face_length(face_mur);
                ch.attachment = Attachment::On {
                    platform: mur,
                    face: face_mur,
                    offset: longueur,
                };

                // **La pose change dans la MÊME image que l'attache.** Sans
                // cette ligne, la face serait déjà verticale alors que la
                // pose dirait encore `walk` — un personnage qui marche dans
                // le vide pendant une image, et l'invariant de simulation de
                // la Tâche 7 se déclencherait exactement là-dessus.
                ch.set_pose(POSE_GRAB_WALL, maintenant);

                // Jusqu'où monter ? Deux comportements de Shimeji-ee :
                // `ClimbAlongWall` va jusqu'en haut, `ClimbHalfwayAlongWall`
                // s'arrête à une hauteur tirée. On tire entre les deux — la
                // marge est le produit (décision n° 3).
                let cible = if rng.unit_f32() < 0.5 {
                    0.0
                } else {
                    rng.range(0.0, longueur * 0.7)
                };
                phase = PhaseGrimpe::Paroi { cible };
            } else {
                avancer(ch, world, pas * ch.facing.signe());
            }
        }

        // ── Monter, ou redescendre ──────────────────────────────────────
        PhaseGrimpe::Paroi { cible } => {
            let Attachment::On {
                platform,
                face,
                offset,
            } = ch.attachment
            else {
                // Il a été attrapé, ou il est tombé : les réflexes ont déjà
                // tranché, on ne discute pas.
                ch.intention = None;
                return Issue::Echouee;
            };

            let Some(plat) = world.get(platform) else {
                ch.intention = None;
                return Issue::Echouee;
            };

            ch.set_pose(POSE_CLIMB_WALL, maintenant);

            let pas = reglages.vitesse_escalade * dt;
            let reste = cible - offset;

            if reste.abs() <= pas {
                // Cible atteinte. Si c'était le bas du mur, l'escalade est
                // finie et il repasse sur le sol.
                let longueur = plat.rect.face_length(face);
                if cible >= longueur - 1.0 {
                    match sol_au_pied_du_mur(world, platform) {
                        Some((sol, offset_sol)) => {
                            ch.attachment = Attachment::On {
                                platform: sol,
                                face: Face::Top,
                                offset: offset_sol,
                            };
                            ch.set_pose(POSE_STAND, maintenant);
                            ch.intention = None;
                            return Issue::Finie;
                        }
                        None => {
                            // Pas de sol retrouvé : il se lâche. La règle de
                            // sécurité l'aurait fait de toute façon, mais le
                            // dire ici évite une image de flottement.
                            ch.intention = None;
                            return Issue::Echouee;
                        }
                    }
                }

                phase = PhaseGrimpe::Accroche;
                let d = reglages.escalade.duree_accroche;
                jusqu_a = maintenant + Duration::from_secs_f32(rng.range(d[0], d[1]));
            } else {
                // `signum` donne le sens : −1 vers le haut (la cible est
                // au-dessus, donc son offset est plus petit), +1 vers le bas.
                ch.attachment = Attachment::On {
                    platform,
                    face,
                    offset: offset + pas * reste.signum(),
                };
            }
        }

        // ── Traverser le plafond ─────────────────────────────────────────
        PhaseGrimpe::Plafond { cible } => {
            let Attachment::On {
                platform,
                face,
                offset,
            } = ch.attachment
            else {
                ch.intention = None;
                return Issue::Echouee;
            };
            let Some(plat) = world.get(platform) else {
                ch.intention = None;
                return Issue::Echouee;
            };

            ch.set_pose(POSE_CLIMB_CEILING, maintenant);

            let pas = reglages.vitesse_escalade * dt;
            let reste = cible - offset;

            // Au plafond, l'orientation suit le SENS DU DÉPLACEMENT, comme
            // au sol — et non « il regarde la surface », qui n'a pas de sens
            // à l'horizontale (design §3.4). Sur un mur, `ch.facing` ne
            // varie pas pendant `Paroi` : c'est spécifique au plafond, où le
            // personnage se déplace bien horizontalement.
            ch.facing = if reste < 0.0 {
                Facing::Left
            } else {
                Facing::Right
            };

            if reste.abs() <= pas {
                phase = PhaseGrimpe::Accroche;
                let d = reglages.escalade.duree_accroche;
                jusqu_a = maintenant + Duration::from_secs_f32(rng.range(d[0], d[1]));
            } else {
                let nouveau = offset + pas * reste.signum();
                let longueur = plat.rect.face_length(face);

                // Au bout du plafond : le plafond du voisin le prolonge-
                // t-il ? C'est le MÊME mécanisme qu'au sol, et c'est pour
                // cela que `face_voisine` a été généralisée aux faces
                // `Bottom`.
                if nouveau < 0.0 || nouveau > longueur {
                    match face_voisine(world, platform, face, reste > 0.0) {
                        Some((voisine, entree)) => {
                            ch.attachment = Attachment::On {
                                platform: voisine,
                                face,
                                offset: entree,
                            };
                            // La cible appartenait à l'ancien plafond : on
                            // s'arrête là et on s'accroche, plutôt que de
                            // traduire un offset d'une plateforme à l'autre
                            // (qui n'a pas de sens si les deux plafonds ont
                            // des largeurs différentes).
                            phase = PhaseGrimpe::Accroche;
                            let d = reglages.escalade.duree_accroche;
                            jusqu_a =
                                maintenant + Duration::from_secs_f32(rng.range(d[0], d[1]));
                        }
                        None => {
                            // Bout du monde : on s'accroche sur place plutôt
                            // que de laisser l'offset déborder.
                            ch.attachment = Attachment::On {
                                platform,
                                face,
                                offset: nouveau.clamp(0.0, longueur),
                            };
                            phase = PhaseGrimpe::Accroche;
                            let d = reglages.escalade.duree_accroche;
                            jusqu_a =
                                maintenant + Duration::from_secs_f32(rng.range(d[0], d[1]));
                        }
                    }
                } else {
                    ch.attachment = Attachment::On {
                        platform,
                        face,
                        offset: nouveau,
                    };
                }
            }
        }

        // ── Accroché, puis la sortie tirée au sort ──────────────────────
        PhaseGrimpe::Accroche => {
            // La pose dépend de la FACE occupée, pas de la phase : accroché
            // à un mur ou suspendu au plafond, ce ne sont pas les mêmes
            // frames (Tâche 6). C'est la face de `ch.attachment`, pas un
            // paramètre — cette même variante `Accroche` sert aux deux cas
            // depuis que le plafond existe.
            let pose = match ch.attachment {
                Attachment::On {
                    face: Face::Bottom, ..
                } => POSE_GRAB_CEILING,
                _ => POSE_GRAB_WALL,
            };
            ch.set_pose(pose, maintenant);

            // `jusqu_a == ZERO` : première image de cette phase, sa durée
            // n'a pas encore été tirée. Ce n'est PAS le cas normal en sortie
            // de `Paroi` ou `Plafond` ci-dessus : ces deux branches posent
            // déjà `jusqu_a` avant de passer en `Accroche`. Le cas qui arrive
            // réellement ici est celui d'une intention installée de
            // l'EXTÉRIEUR par `ActiveIntention::accroche` (Tâche 5, un
            // lancer contre un mur), qui pose `jusqu_a: ZERO` précisément
            // pour que cette toute première image tire la durée d'accroche.
            //
            // ⚠️ **Correction de bug** : sans cette branche, `maintenant >=
            // Duration::ZERO` est toujours vrai, donc la toute première
            // image sautait directement au tirage lâcher/redescendre — le
            // personnage jeté contre un mur décidait de repartir 16 ms après
            // s'être accroché, sans jamais tenir la seconde promise. Même
            // motif que `se_reposer`, qui traite `jusqu_a == ZERO` comme
            // « pas encore tirée » avant de tester l'expiration.
            if jusqu_a == Duration::ZERO {
                let d = reglages.escalade.duree_accroche;
                jusqu_a = maintenant + Duration::from_secs_f32(rng.range(d[0], d[1]));
                ai.etat = EtatIntention::Grimpe { phase, jusqu_a };
                return Issue::EnCours;
            }

            if maintenant < jusqu_a {
                ai.etat = EtatIntention::Grimpe { phase, jusqu_a };
                return Issue::EnCours;
            }

            let Attachment::On {
                platform,
                face,
                offset,
            } = ch.attachment
            else {
                ch.intention = None;
                return Issue::Echouee;
            };
            let Some(plat) = world.get(platform) else {
                ch.intention = None;
                return Issue::Echouee;
            };

            // Décision n° 5 : les deux poids viennent de `config.json`, on
            // règle s'il est casse-cou ou prudent sans recompiler.
            let e = &reglages.escalade;
            let lache = match rng.weighted(&[e.poids_lacher, e.poids_redescendre]) {
                Some(0) => true,
                // `Some(1)` redescend, et `None` aussi — il n'arrive que si
                // les deux poids sont nuls, auquel cas redescendre est le
                // repli le moins surprenant.
                _ => false,
            };

            if lache {
                // On repart du rectangle COURANT pour savoir d'où il tombe
                // (décision n° 1), et la vitesse initiale est nulle : il ne
                // se jette pas, il lâche prise.
                ch.attachment = Attachment::Falling {
                    pos: plat.rect.point_on(face, offset),
                    vel: crate::geom::Vec2::zero(),
                };
                ch.intention = None;
                return Issue::Finie;
            }

            // Troisième issue, réservée au HAUT d'un mur : passer au
            // plafond. `offset <= pas_d_une_image` plutôt que `== 0.0` : on
            // ne compare jamais deux flottants pour l'égalité après une
            // accumulation de pas.
            //
            // `face != Face::Bottom` exclut le cas où l'on est DÉJÀ au
            // plafond : cette bascule n'a de sens qu'en arrivant d'un mur.
            let en_haut = face != Face::Bottom && offset <= reglages.vitesse_escalade * dt;
            if en_haut && ch.manifest.has_pose(POSE_CLIMB_CEILING) {
                // Couverture partielle (spec §8.6) : un pack sans pose de
                // plafond grimpe quand même, il s'arrête simplement en haut
                // du mur — c'est exactement pourquoi `climbCeiling` n'est
                // PAS dans les `poses_requises` de `Grimper` (desire.rs).
                if let Some((plafond, entree)) = plafond_au_sommet(world, platform, plat, face) {
                    ch.attachment = Attachment::On {
                        platform: plafond,
                        face: Face::Bottom,
                        offset: entree,
                    };
                    // ⚠️ **La pose change ICI, dans la MÊME image que
                    // l'attache.** Sans cette ligne, l'attachement dirait
                    // déjà « plafond » alors que la pose resterait
                    // `grabWall` — exactement le défaut que l'invariant de
                    // simulation de la Tâche 7 est censé détecter, et qui a
                    // déjà mordu une fois sur la transition sol → mur
                    // (`Rejoindre`, plus haut dans cette fonction).
                    ch.set_pose(POSE_CLIMB_CEILING, maintenant);

                    let longueur = world
                        .get(plafond)
                        .map(|p| p.rect.face_length(Face::Bottom))
                        .unwrap_or(0.0);
                    phase = PhaseGrimpe::Plafond {
                        cible: rng.range(0.0, longueur),
                    };
                    ai.etat = EtatIntention::Grimpe { phase, jusqu_a };
                    return Issue::EnCours;
                }
            }

            // Redescendre. Le sens dépend de la face occupée : sur un mur,
            // « redescendre » vise le bas — la même phase `Paroi`, avec une
            // cible plus GRANDE que l'offset courant. Au plafond il n'y a
            // pas de bas : redescendre n'a pas de sens, donc il reprend
            // simplement sa traversée vers un nouveau point par la phase
            // `Plafond`. Sans cette distinction, un personnage qui choisit
            // de « redescendre » depuis le plafond retomberait dans `Paroi`,
            // qui pose `climbWall` et déplace verticalement — la mauvaise
            // pose et le mauvais axe pour quelqu'un de suspendu.
            phase = if face == Face::Bottom {
                PhaseGrimpe::Plafond {
                    cible: rng.range(0.0, plat.rect.face_length(face)),
                }
            } else {
                PhaseGrimpe::Paroi {
                    cible: plat.rect.face_length(face),
                }
            };
        }
    }

    ai.etat = EtatIntention::Grimpe { phase, jusqu_a };
    Issue::EnCours
}

/// Le mur de l'écran du personnage le plus proche de lui.
///
/// « De son écran » : c'est à cela que sert `PlatformId::meme_ecran`. Sans ce
/// filtre, un personnage sur l'écran de gauche pourrait viser le mur droit de
/// l'écran de droite, à 3 000 px — une marche de 60 s pour rien.
///
/// Rend `None` quand cet écran-là n'a aucun mur : c'est le cas de l'écran du
/// milieu d'une rangée de trois, dont les deux bords sont recouverts par ses
/// voisins (design §2.3).
fn mur_le_plus_proche(world: &World, depuis: PlatformId, ch: &Character) -> Option<PlatformId> {
    // `?` : pas de position connue (plateforme disparue), pas de mur à viser.
    let pos = position_actuelle(ch, world)?;

    // (identité, distance) — la distance ne sert qu'à comparer, et on la
    // laisse tomber à la fin. Même motif que `World::nearest_floor`.
    let mut meilleur: Option<(PlatformId, f32)> = None;

    for plat in world.platforms() {
        if !plat.id.meme_ecran(depuis) {
            continue;
        }
        // Un mur, c'est-à-dire une plateforme dont l'unique face est
        // verticale. Le sol (`Top`) et le plafond (`Bottom`) sont écartés
        // par le test qui suit.
        let Some(face) = plat.faces.first().copied() else {
            continue;
        };
        if face != Face::Left && face != Face::Right {
            continue;
        }

        let d = (plat.rect.point_on(face, 0.0).x - pos.x).abs();
        // `match` explicite plutôt qu'une chaîne de combinateurs sur
        // `Option` : la comparaison se relit mieux (même choix que
        // `nearest_floor`).
        let remplace = match meilleur {
            None => true,
            Some((_, best)) => d < best,
        };
        if remplace {
            meilleur = Some((plat.id, d));
        }
    }

    meilleur.map(|(id, _)| id)
}

/// Le sol sur lequel reposer en bas d'un mur, et l'offset où y arriver.
///
/// Le mur et le sol appartiennent au même écran, donc `meme_ecran` suffit —
/// inutile de chercher géométriquement.
fn sol_au_pied_du_mur(world: &World, mur: PlatformId) -> Option<(PlatformId, f32)> {
    let plat_mur = world.get(mur)?;
    let face_mur = plat_mur.faces.first().copied()?;
    let x = plat_mur.rect.point_on(face_mur, 0.0).x;

    for plat in world.platforms() {
        if !plat.id.meme_ecran(mur) || !plat.has_face(Face::Top) {
            continue;
        }
        // `clamp` : on rabat dans les bornes du sol, le mur étant exactement
        // sur son bord à un pixel près.
        let offset = (x - plat.rect.left()).clamp(0.0, plat.rect.face_length(Face::Top));
        return Some((plat.id, offset));
    }

    None
}

/// Le plafond de l'écran de ce mur, et l'offset où y entrer.
///
/// L'offset d'entrée est l'abscisse du mur ramenée dans les bornes du
/// plafond : on arrive au plafond juste au-dessus de l'endroit où l'on
/// tenait la paroi. Même motif que `sol_au_pied_du_mur`, avec `Face::Bottom`
/// à la place de `Face::Top`.
fn plafond_au_sommet(
    world: &World,
    mur: PlatformId,
    plat_mur: &crate::world::Platform,
    face_mur: Face,
) -> Option<(PlatformId, f32)> {
    let x = plat_mur.rect.point_on(face_mur, 0.0).x;

    for plat in world.platforms() {
        if !plat.id.meme_ecran(mur) || !plat.has_face(Face::Bottom) {
            continue;
        }
        let offset = (x - plat.rect.left()).clamp(0.0, plat.rect.face_length(Face::Bottom));
        return Some((plat.id, offset));
    }

    None
}

/// La position écran actuelle, quand elle existe.
///
/// Enveloppe `attach::world_position` avec un curseur factice : le personnage
/// n'est jamais `Dragged` quand cette fonction est appelée depuis une
/// intention — les réflexes ont la priorité sur le portage, et ils rendent la
/// main avant. Le point passé n'est donc jamais lu.
fn position_actuelle(ch: &Character, world: &World) -> Option<Point> {
    crate::character::attach::world_position(&ch.attachment, world, Point::new(0.0, 0.0))
}

/// Avance de `pas` pixels le long de la face courante, et traite le bord.
///
/// **Décision locale, pas de plan** (décision n° 4) : au bord, on regarde
/// s'il existe une face voisine dans la direction du mouvement, et sinon on
/// fait demi-tour. Aucun itinéraire n'est calculé.
fn avancer(ch: &mut Character, world: &World, pas: f32) {
    let Attachment::On {
        platform,
        face,
        offset,
    } = ch.attachment
    else {
        // En chute ou porté : ce n'est pas à l'intention de décider, les
        // réflexes s'en occupent.
        return;
    };

    let Some(plat) = world.get(platform) else {
        // La plateforme a disparu entre les réflexes et ici. Improbable dans
        // une même image, mais on ne suppose rien : les réflexes le verront
        // à l'image suivante.
        return;
    };

    let longueur = plat.rect.face_length(face);
    let nouveau = offset + pas;

    // Toujours dans la face : rien de spécial.
    if nouveau >= 0.0 && nouveau <= longueur {
        ch.attachment = Attachment::On {
            platform,
            face,
            offset: nouveau,
        };
        return;
    }

    // ── Le bord est atteint ─────────────────────────────────────────────
    let vers_la_droite = pas > 0.0;

    // Y a-t-il un sol voisin qui prolonge celui-ci de ce côté ? C'est ce qui
    // fait qu'« il circule sur tous les écrans » (étape 1) — et à l'étape 4,
    // ce sera aussi ce qui le fait passer d'une barre de titre à la suivante.
    // `face` et non `Face::Top` codé en dur : `avancer` ne sert aujourd'hui
    // qu'à `Flaner`, qui ne connaît que le sol, mais `face_voisine` a été
    // généralisée pour le plafond (Tâche 6) — autant que son unique appelant
    // demande la MÊME face que celle occupée, plutôt que de supposer `Top`.
    if let Some((voisine, offset_entree)) = face_voisine(world, platform, face, vers_la_droite) {
        ch.attachment = Attachment::On {
            platform: voisine,
            face,
            offset: offset_entree,
        };
        return;
    }

    // Pas de voisin : demi-tour, et on se recale exactement sur le bord.
    //
    // La spec §6.3 autorise aussi de se laisser tomber ou de s'accrocher à
    // une face voisine. À l'étape 1 il n'y a ni murs ni plafonds exposés, et
    // se laisser tomber du bord de l'écran ne mènerait qu'au garde-fou :
    // le demi-tour est la seule issue qui ait du sens ici. Le tirage entre
    // les trois arrive à l'étape 4.
    ch.facing = ch.facing.inverse();
    ch.attachment = Attachment::On {
        platform,
        face,
        offset: nouveau.clamp(0.0, longueur),
    };
}

/// Cherche une plateforme adjacente à celle de `depuis`, exposant la MÊME
/// face, du côté demandé et à peu près à la même hauteur.
///
/// Vit ici et non dans `world.rs` parce que c'est une **décision de
/// navigation**, pas une propriété du monde : « ce sol en prolonge-t-il un
/// autre ? » n'a de sens que pour quelqu'un qui marche dessus.
///
/// Généralisée aux faces `Bottom` à l'étape 4a (Tâche 6) : le plafond d'un
/// écran prolonge celui du voisin exactement comme le sol prolonge le sol.
/// **Un seul chemin de code pour les deux** — en écrire un second finirait
/// par diverger, et c'est pourquoi la fonction prend désormais `face` en
/// paramètre plutôt que de coder `Face::Top` en dur.
///
/// Rend la plateforme voisine et l'offset auquel y entrer.
fn face_voisine(
    world: &World,
    depuis: PlatformId,
    face: Face,
    vers_la_droite: bool,
) -> Option<(PlatformId, f32)> {
    /// Tolérance sur la jonction. Deux écrans côte à côte se touchent
    /// exactement, mais des résolutions ou des échelles différentes peuvent
    /// laisser quelques pixels : on ne veut pas d'un demi-tour inexpliqué
    /// pour 2 px.
    const TOLERANCE: f32 = 8.0;

    let source = world.get(depuis)?;
    // La ligne de référence : le haut du rectangle pour un sol, le bas pour
    // un plafond. `point_on(face, 0.0).y` la donne dans les deux cas — c'est
    // la MÊME fonction que celle qui place le personnage, donc pas de risque
    // de calculer la hauteur autrement ici et là.
    let hauteur = source.rect.point_on(face, 0.0).y;

    for plat in world.platforms() {
        if plat.id == depuis || !plat.has_face(face) {
            continue;
        }

        // À peu près la même hauteur : on ne veut pas qu'il enjambe le vide
        // vers un sol (ou un plafond) 400 px plus bas.
        if (plat.rect.point_on(face, 0.0).y - hauteur).abs() > TOLERANCE {
            continue;
        }

        if vers_la_droite {
            // Le voisin commence là où celui-ci finit.
            if (plat.rect.left() - source.rect.right()).abs() <= TOLERANCE {
                return Some((plat.id, 0.0));
            }
        } else if (source.rect.left() - plat.rect.right()).abs() <= TOLERANCE {
            // On y entre par son bord droit.
            return Some((plat.id, plat.rect.face_length(face)));
        }
    }

    None
}

/// Jouer : poser une animation assise et la laisser tourner.
///
/// Plus simple que `se_reposer`, dont il ne partage pas la logique de phases :
/// un jeu n'a pas d'étape. Les deux fonctions restent séparées pour cette
/// raison — les fondre demanderait un paramètre « as-tu des phases ? », qui
/// est le signe d'une mauvaise abstraction.
fn jouer(
    ch: &mut Character,
    jeu: Jeu,
    ai: &mut ActiveIntention,
    maintenant: Duration,
    rng: &mut dyn Rng,
) -> Issue {
    let EtatIntention::Jeu { mut jusqu_a } = ai.etat else {
        ch.intention = None;
        return Issue::Echouee;
    };

    // Défense en profondeur : `desire.rs` filtre déjà sur la pose, mais une
    // config bricolée pourrait proposer ce jeu à un personnage qui n'a pas
    // l'animation — et un personnage posé sur une pose inexistante serait
    // invisible. Mieux vaut échouer et re-tirer.
    if !ch.manifest.has_pose(jeu.pose()) {
        ch.intention = None;
        return Issue::Echouee;
    }

    if jusqu_a == Duration::ZERO {
        // Première image : on tire la durée du jeu.
        //
        // 4 à 15 s, la même plage que le repos : bornée sous
        // `DELAI_ABANDON` pour que l'issue soit `Finie` et non `Echouee`.
        jusqu_a = maintenant + Duration::from_secs_f32(rng.range(4.0, 15.0));
        ai.etat = EtatIntention::Jeu { jusqu_a };
    } else if maintenant >= jusqu_a {
        ch.intention = None;
        return Issue::Finie;
    }

    ch.set_pose(jeu.pose(), maintenant);
    Issue::EnCours
}

/// Se reposer : s'asseoir, et s'endormir si un signal y pousse.
///
/// **Le sommeil n'est pas une intention à part** : c'est la seconde phase du
/// repos. Voir `PhaseRepos`.
fn se_reposer(
    ch: &mut Character,
    ai: &mut ActiveIntention,
    e: &super::Entrees,
    reglages: &Reglages,
    maintenant: Duration,
    rng: &mut dyn Rng,
) -> Issue {
    let EtatIntention::Repos {
        mut phase,
        mut jusqu_a,
    } = ai.etat
    else {
        ch.intention = None;
        return Issue::Echouee;
    };

    // Défense en profondeur : le tirage ne devrait jamais proposer cette
    // intention à un personnage sans `sit` (desire.rs le filtre). Mais une
    // config bricolée pourrait y parvenir, et un personnage assis sur une
    // pose inexistante serait invisible — mieux vaut échouer.
    //
    // ⚠️ **Sauf en phase `Selevant`** : émerger ne passe jamais par la
    // position assise. Un pack sans `sit` ne doit pas être empêché de se
    // réveiller au déverrouillage — il n'a simplement pas le droit de
    // *choisir* de se reposer, ce qui est une autre question.
    if phase != PhaseRepos::Selevant && !ch.manifest.has_pose(POSE_SIT) {
        ch.intention = None;
        return Issue::Echouee;
    }

    // La condition du sommeil, calculée ici et non plus bas seulement : elle
    // sert maintenant à DEUX endroits (la continuité juste en dessous, et la
    // bascule de phase après l'expiration), et deux calculs de la même
    // condition finiraient par diverger. C'est **la seule ligne de tout le
    // fichier qui regarde un biais** : il faut qu'un signal ait au moins
    // doublé l'envie de repos (`seuilSommeil`, 2,0 par défaut). Sans signal
    // le biais vaut 1, donc il reste assis — **une sieste ne s'improvise
    // pas.**
    //
    // Noter la forme : on ne teste PAS « est-ce que l'utilisateur est
    // parti ». On teste un poids. C'est la décision n° 3 appliquée à la
    // lettre : le comportement ne sait pas ce qu'est l'inactivité.
    //
    // ⚠️ **Sauf pour `!e.utilisateur_actif`, ajouté par la vague de
    // correction finale.** Ce terme-là consulte un FAIT et non un poids, et
    // c'est délibéré : `veut_dormir` (le poids) décide s'il VEUT dormir,
    // `utilisateur_actif` (le fait) décide si dormir est POSSIBLE. La
    // décision n° 3 n'est pas entamée — le signal ne CHOISIT toujours pas
    // l'intention, il ferme une porte, exactement comme une pose manquante en
    // ferme une (couverture partielle, spec §8.6).
    //
    // Sans ce terme, deux règles correctes séparément se contredisaient à
    // l'assemblage : le soir (22 h→6 h, ×3 par défaut) franchit `seuilSommeil`
    // à lui seul, SANS exiger d'absence — contrairement à ce que le design
    // §6 affirmait (« le sommeil n'est atteint que si un signal a poussé le
    // biais au-dessus du seuil […] parce que l'utilisateur était parti »,
    // corrigé depuis). Un utilisateur au clavier après 22 h pouvait donc
    // entrer en sommeil profond, puis `behavior::mod::pas` l'en faisait
    // aussitôt sortir (l'interruption ne s'applique qu'à `Endormi`), et la
    // continuité de pose ci-dessous le replongeait dedans : un flash de
    // quelques images à chaque fin de repos avec `blob`, et **une boucle qui
    // ne se termine jamais** avec un pack sans pose de jeu ni de marche —
    // 30 intentions tirées par seconde, personnage figé. L'invariant retenu
    // pour empêcher cela structurellement : **phase `Endormi` ⇒ utilisateur
    // absent.**
    let veut_dormir = e.biais.pour(Intention::SeReposer) >= reglages.seuil_sommeil
        && !e.utilisateur_actif;

    // ── La continuité de pose, et sa condition ──────────────────────────
    //
    // **C'est ce qui permet de ne PAS toucher au délai d'abandon**
    // (décision n° 4). Un sommeil dure 20 à 60 s, le délai coupe à 20 s, donc
    // l'intention est re-tirée — et comme un signal met ×8 sur le repos, elle
    // est presque toujours re-tirée en `SeReposer`.
    //
    // Sans cette reprise, chaque re-tirage repartirait en phase `Assis` et
    // l'on verrait le personnage se rasseoir puis se raffaler toutes les
    // 20 secondes. Avec elle, le re-tirage est **invisible**.
    //
    // ⚠️ **Mais elle est conditionnée au biais**, et pas seulement à la pose.
    // Après un réveil (Tâche 5), la pose est encore `sleep` le temps d'une
    // image : sans cette condition, un tirage qui retombe sur `SeReposer`
    // replongerait le personnage en sommeil profond au lieu de l'asseoir — le
    // réveil serait annulé une fois sur huit.
    //
    // La formule tient en une phrase : **on ne continue de dormir que si l'on
    // choisirait encore de s'endormir.** C'est la MÊME condition qui autorise
    // à entrer en sommeil, donc rien de nouveau à retenir.
    //
    // Et noter la forme : on teste un POIDS, jamais un signal. `se_reposer`
    // ne sait toujours pas ce qu'est l'inactivité (décision n° 3).
    if phase == PhaseRepos::Assis && ch.pose == POSE_SLEEP && veut_dormir {
        phase = PhaseRepos::Endormi;
        // ⚠️ Cette ligne n'a AUCUN EFFET ICI, et c'est normal : ce bloc ne se
        // déclenche que sur une intention FRAÎCHE (voir le commentaire
        // au-dessus, « comme après un re-tirage »), dont `jusqu_a` vaut déjà
        // `Duration::ZERO` depuis `ActiveIntention::nouvelle`. Elle reste
        // écrite pour que l'intention soit explicite — « ce repos tirera sa
        // propre durée à l'image suivante » — même si le compilateur ne voit
        // ici qu'une affectation redondante. Corrigé le commentaire, pas le
        // code (vague de correction finale, point 7c) : la version
        // précédente prétendait que cette ligne « repartait sur une durée
        // fraîche », ce qui laissait croire qu'elle changeait quelque chose.
        jusqu_a = Duration::ZERO;
    }

    if jusqu_a == Duration::ZERO {
        // Première image de cette phase : on tire sa durée.
        let (min, max) = match phase {
            // Assis : la plage de l'étape 1a, inchangée.
            PhaseRepos::Assis => (4.0, 15.0),

            // Endormi : 20 à 60 s. Relevé dans `actions.xml`, action
            // `LieDown` — `Sprawl` pendant `${500+Math.random()*1000}` ticks
            // à 40 ms. La leçon de l'étape 1a : chercher la constante dans le
            // source plutôt que de l'inventer.
            PhaseRepos::Endormi => (20.0, 60.0),

            // Selevant : le sommeil résiduel du déverrouillage, PLUS la
            // durée de l'animation de réveil.
            //
            // Les 2,5 à 4 s ne viennent d'aucun source Shimeji — il n'y a pas
            // de réveil chez Shimeji-ee (voir `POSE_WAKE`). C'est un réglage
            // de confort, et l'écart de 1,5 s est délibéré : à l'étape 3 il y
            // aura plusieurs personnages à l'écran, et s'ils émergeaient tous
            // à la même image on verrait une chorégraphie, pas des animaux.
            //
            // La durée de l'animation est LUE dans le manifeste et non
            // écrite ici : un pack dont le réveil dure plus longtemps doit
            // pouvoir le jouer en entier, et un pack qui n'a pas la pose
            // ajoute zéro.
            PhaseRepos::Selevant => {
                let anim = duree_reveil(ch).as_secs_f32();
                (2.5 + anim, 4.0 + anim)
            }
        };
        jusqu_a = maintenant + Duration::from_secs_f32(rng.range(min, max));
        ai.etat = EtatIntention::Repos { phase, jusqu_a };
    } else if maintenant >= jusqu_a {
        // ── La phase est écoulée : s'endormir, ou terminer ──────────────
        //
        // `veut_dormir` est calculé plus haut, avant le bloc de continuité :
        // c'est la même condition aux deux endroits, et la recalculer ici
        // aurait fini par diverger d'elle au premier réglage touché.
        if phase == PhaseRepos::Assis && veut_dormir && ch.manifest.has_pose(POSE_SLEEP) {
            phase = PhaseRepos::Endormi;
            jusqu_a = Duration::ZERO; // sera tirée à l'image suivante
            ai.etat = EtatIntention::Repos { phase, jusqu_a };
        } else {
            // Soit il n'a pas de raison de dormir, soit il n'a pas la pose
            // (couverture partielle appliquée à une PHASE), soit il vient de
            // finir sa nuit.
            ch.intention = None;
            return Issue::Finie;
        }
    }

    let pose = match phase {
        PhaseRepos::Assis => POSE_SIT,
        PhaseRepos::Endormi => POSE_SLEEP,

        // Émerger, c'est DEUX poses dans une seule phase : il dort encore,
        // puis il se redresse. On les départage sur le temps qui RESTE, et
        // non sur le temps écoulé — parce que la durée totale a été tirée au
        // hasard alors que la fin, elle, est toujours l'animation de réveil.
        //
        // Deux phases distinctes auraient demandé un second `jusqu_a`, donc
        // une seconde bascule à écrire et à tester, pour la même image à
        // l'écran.
        PhaseRepos::Selevant => {
            if jusqu_a.saturating_sub(maintenant) > duree_reveil(ch) {
                POSE_SLEEP
            } else {
                // Absente du pack : `set_pose` ne fait rien et il reste
                // affalé jusqu'au bout. C'est la couverture partielle, et
                // c'est pourquoi il n'y a pas de `has_pose` ici.
                POSE_WAKE
            }
        }
    };
    ch.set_pose(pose, maintenant);
    Issue::EnCours
}

/// Combien de temps dure l'animation de réveil de CE personnage.
///
/// Zéro si le pack n'a pas la pose : la phase `Selevant` se réduit alors à
/// son sommeil résiduel, et le personnage repart sans s'être redressé.
///
/// `match` explicite plutôt que `map_or` : le cas « pas de pose » est une
/// décision de design (couverture partielle), pas un détail à cacher dans un
/// combinateur.
fn duree_reveil(ch: &Character) -> Duration {
    match ch.manifest.pose(POSE_WAKE) {
        Some(p) => p.duree_totale(),
        None => Duration::ZERO,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::Entrees;
    use crate::character::manifest::Manifest;
    use crate::geom::Point;
    use crate::geom::Rect;
    use crate::probe::fake::FakeProbe;
    use crate::probe::{ScreenInfo, SystemProbe};
    use crate::rng::XorShift32;

    const DT: f32 = 1.0 / 60.0;

    /// Les réglages par défaut. Construits ici et non lus depuis le disque :
    /// un test qui lirait le `config.json` de la machine ne serait plus
    /// reproductible.
    fn reglages() -> Reglages {
        Reglages::depuis(&crate::config::Config::default())
    }

    // Les deux animations de jeu ajoutées ci-dessous ont des numéros de
    // frame ARBITRAIRES (9, 10, 11) : ce manifeste est un DOUBLE, il ne sert
    // qu'à dire quelles poses existent pour les tests. Les vraies frames de
    // `blob` sont déclarées dans `characters/blob/mascot.json`.
    //
    // ⚠️ Le JSON ne supporte aucun commentaire : contrairement à du Rust
    // normal, `//` à l'intérieur du `r#"..."#` ci-dessous ferait échouer
    // `serde_json` avec un message peu clair (« key must be a string »).
    // D'où ce commentaire ici, en dehors de la chaîne, plutôt qu'au milieu
    // des clés `spinHead` / `sitDangle`.
    //
    // `sleep` (frame 12, tout aussi arbitraire) est la pose de sommeil de la
    // Tâche 4. Le test `sans_la_pose_sleep_il_reste_assis_au_lieu_d_echouer`
    // construit, lui, son propre manifeste SANS elle — c'est justement ce
    // qu'il vérifie.
    //
    // `grabWall` et `climbWall` (frames 15, 16, arbitraires elles aussi)
    // sont celles de l'escalade (étape 4a, Tâche 4). Sans elles, `set_pose`
    // les ignorerait — c'est la couverture partielle (spec §8.6) — et
    // `grimper_pose_les_bonnes_animations` échouerait pour une raison qui
    // n'aurait rien à voir avec l'escalade.
    //
    // `grabCeiling` et `climbCeiling` (frames 17, 18, arbitraires) sont
    // celles du plafond (Tâche 6). Le test
    // `un_pack_sans_pose_de_plafond_grimpe_quand_meme` retire ces deux poses
    // d'une COPIE de ce manifeste (`manifeste_sans`) plutôt que d'en écrire
    // un second JSON : c'est exactement la couverture partielle qu'il
    // vérifie.
    fn manifeste() -> Manifest {
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40, 20, 48, 100],
            "poses": {
                "stand": { "frames": [1] },
                "walk":  { "frames": [2, 3], "frameMs": 120, "loop": true },
                "run":   { "frames": [4, 5], "frameMs": 80,  "loop": true },
                "sit":   { "frames": [6] },
                "fall":  { "frames": [7], "anchor": [64, 64] },
                "land":  { "frames": [8], "frameMs": 150 },
                "spinHead":  { "frames": [9, 10], "frameMs": 200 },
                "sitDangle": { "frames": [11], "anchor": [64, 112] },
                "sleep":     { "frames": [12] },
                "wake":      { "frames": [13, 14], "frameMs": 100 },
                "grabWall":  { "frames": [15] },
                "climbWall": { "frames": [15, 16], "frameMs": 150, "loop": true },
                "grabCeiling":  { "frames": [17] },
                "climbCeiling": { "frames": [17, 18], "frameMs": 150, "loop": true }
            }
        }"#;
        serde_json::from_str(json).unwrap()
    }

    fn monde() -> World {
        World::from_screens(&FakeProbe::deux_ecrans().screens())
    }

    fn perso(monde: &World, offset: f32) -> Character {
        perso_sur(monde, monde.platforms()[0].id, offset)
    }

    /// Un personnage posé sur la face `Top` de la plateforme donnée.
    ///
    /// Extraite de `perso` plutôt qu'ajoutée à côté : les tests d'escalade
    /// ont besoin de choisir LEUR plateforme (le sol de l'écran du milieu,
    /// par exemple), et deux constructeurs indépendants auraient fini par
    /// diverger sur le manifeste ou l'ancre.
    fn perso_sur(monde: &World, platform: PlatformId, offset: f32) -> Character {
        // La position passée à `Character::new` n'est qu'un point de départ
        // pour le rendu : dès la première image, `world_position` la
        // recalcule depuis la plateforme (décision n° 1).
        let depart = match monde.get(platform) {
            Some(plat) => plat.rect.point_on(Face::Top, offset),
            None => Point::new(offset, 1032.0),
        };

        Character::new(
            manifeste(),
            Attachment::On {
                platform,
                face: Face::Top,
                offset,
            },
            depart,
        )
    }

    /// Un personnage au milieu du premier sol du monde donné.
    fn perso_sur_le_sol(monde: &World) -> Character {
        let sol = monde.premier_sol().expect("un monde de test a un sol");
        perso_sur(monde, sol.id, 500.0)
    }

    fn offset_de(ch: &Character) -> f32 {
        match ch.attachment {
            Attachment::On { offset, .. } => offset,
            autre => panic!("attendu On, obtenu {autre:?}"),
        }
    }

    fn plateforme_de(ch: &Character) -> PlatformId {
        match ch.attachment {
            Attachment::On { platform, .. } => platform,
            autre => panic!("attendu On, obtenu {autre:?}"),
        }
    }

    /// Des `Entrees` inertes, avec un biais de repos choisi.
    ///
    /// Toutes les autres valeurs sont neutres : la souris est loin, aucun
    /// bouton n'est enfoncé. Un seul curseur pour tous les tests de sommeil.
    /// Des entrées où seul le poids du repos varie.
    ///
    /// ⚠️ **`utilisateur_actif` vaut `false`, et ce n'est pas un détail.**
    /// Ce helper sert à simuler « l'utilisateur est parti depuis 2 minutes »,
    /// ce que le seul biais ne suffit plus à dire : depuis l'invariant
    /// « phase `Endormi` ⇒ utilisateur absent », `veut_dormir` exige LES DEUX
    /// (le poids décide s'il VEUT dormir, le fait décide si c'est POSSIBLE).
    ///
    /// Il valait `true`, ce qui contredisait le commentaire de ses propres
    /// appelants (« comme inactif > 2 min ») et rendait deux tests
    /// inatteignables : le personnage ne pouvait plus jamais s'affaler.
    fn entrees_avec_biais_repos(x: f32) -> Entrees {
        Entrees {
            souris: Point::new(0.0, 0.0),
            echelle_affichage: 1.0,
            bouton_gauche: false,
            curseur_sur_le_personnage: false,
            biais: crate::signals::Biais {
                flaner: 1.0,
                se_reposer: x,
                jouer: 1.0,
                // Neutre : aucun de ces tests ne parle d'escalade.
                grimper: 1.0,
            },
            utilisateur_actif: false,
            commande: None,
        }
    }

    /// Des `Entrees` complètement neutres.
    ///
    /// `poursuivre` prend désormais des `Entrees` quelle que soit
    /// l'intention en cours — y compris `Flaner` et `Jouer`, qui ne les
    /// consultent jamais. Ce raccourci évite de répéter la même valeur
    /// neutre dans chacun des tests écrits avant cette tâche.
    ///
    /// Construit le biais via `signals::Biais::neutre()` plutôt qu'en
    /// recopiant `{ flaner: 1.0, se_reposer: 1.0, jouer: 1.0 }` : deux
    /// définitions du neutre auraient fini par diverger, et celle de
    /// `Biais::neutre()` sert de référence à toute la Tâche 4 (vague de
    /// correction finale, point 7b).
    fn entrees_neutres() -> Entrees {
        Entrees {
            souris: Point::new(0.0, 0.0),
            echelle_affichage: 1.0,
            bouton_gauche: false,
            curseur_sur_le_personnage: false,
            biais: crate::signals::Biais::neutre(),
            utilisateur_actif: true,
            commande: None,
        }
    }

    #[test]
    fn flaner_finit_par_faire_avancer_le_personnage() {
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(3);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Flaner, Duration::ZERO));

        let depart = offset_de(&ch);
        let mut t = Duration::ZERO;
        // 3 secondes : assez pour qu'au moins une allure de marche soit
        // tirée, quelle que soit la graine.
        for _ in 0..180 {
            poursuivre(&mut ch, &m, &entrees_neutres(), &reglages(), t, DT, &mut rng);
            t += Duration::from_micros(16_667);
        }

        assert_ne!(offset_de(&ch), depart, "il n'a pas bougé en 3 s");
    }

    #[test]
    fn flaner_alterne_les_allures_sans_jamais_courir() {
        // « jamais figé, jamais prévisible » : sur 30 s, l'arrêt et la marche
        // doivent avoir été vus tous les deux.
        //
        // La course, elle, ne doit **jamais** sortir : elle a quitté le
        // tirage de la flânerie (`poids_course` = 0 par défaut). Elle est
        // réservée à des actions qui la demanderont explicitement.
        let m = monde();
        let mut ch = perso(&m, 900.0);
        let mut rng = XorShift32::seeded(11);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Flaner, Duration::ZERO));

        let mut vues = std::collections::BTreeSet::new();
        let mut t = Duration::ZERO;
        for _ in 0..1_800 {
            poursuivre(&mut ch, &m, &entrees_neutres(), &reglages(), t, DT, &mut rng);
            vues.insert(ch.pose.clone());
            t += Duration::from_micros(16_667);
        }

        assert!(vues.contains(POSE_STAND), "jamais arrêté : {vues:?}");
        assert!(vues.contains(POSE_WALK), "jamais marché : {vues:?}");
        assert!(!vues.contains(POSE_RUN), "il a couru en flânant : {vues:?}");
    }

    #[test]
    fn remonter_le_poids_de_course_le_fait_courir_a_nouveau() {
        // Le pendant du test précédent : la course est retirée du tirage par
        // un **réglage**, pas par une suppression de code. Ce test le prouve
        // — il échouerait si `Allure::Course` devenait inatteignable.
        let m = monde();
        let mut ch = perso(&m, 900.0);
        let mut rng = XorShift32::seeded(11);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Flaner, Duration::ZERO));

        // On part des défauts et on ne change QUE le poids de la course : le
        // reste du tempérament est celui de la production.
        let mut config = crate::config::Config::default();
        config.allures.poids_course = 6.0;
        let reglages = Reglages::depuis(&config);

        let mut vues = std::collections::BTreeSet::new();
        let mut t = Duration::ZERO;
        for _ in 0..1_800 {
            poursuivre(&mut ch, &m, &entrees_neutres(), &reglages, t, DT, &mut rng);
            vues.insert(ch.pose.clone());
            t += Duration::from_micros(16_667);
        }

        assert!(vues.contains(POSE_RUN), "jamais couru : {vues:?}");
    }

    #[test]
    fn arrive_au_bord_il_fait_demi_tour_plutot_que_de_tomber() {
        // Le sol du premier écran va de 0 à 1920. On le place à 3 px du bord
        // droit, tourné à droite, en marche forcée.
        //
        // NOTE : le second écran du monde de test commence exactement à
        // x = 1920, donc `face_voisine` le trouverait. On prend donc un
        // monde à UN SEUL écran pour éprouver le demi-tour.
        let m = World::from_screens(&FakeProbe::un_ecran().screens());
        let mut ch = Character::new(
            manifeste(),
            Attachment::On {
                platform: m.platforms()[0].id,
                face: Face::Top,
                offset: 1917.0,
            },
            Point::new(1917.0, 1032.0),
        );
        ch.facing = crate::character::Facing::Right;
        let mut rng = XorShift32::seeded(5);
        ch.intention = Some(ActiveIntention {
            kind: Intention::Flaner,
            depuis: Duration::ZERO,
            etat: EtatIntention::Flanerie {
                allure: Allure::Marche,
                jusqu_a: Duration::from_secs(60),
            },
        });

        let mut t = Duration::ZERO;
        for _ in 0..30 {
            poursuivre(&mut ch, &m, &entrees_neutres(), &reglages(), t, DT, &mut rng);
            t += Duration::from_micros(16_667);
        }

        // Il est toujours accroché — il n'est pas tombé du bord du monde.
        assert!(matches!(ch.attachment, Attachment::On { .. }));
        // Et il repart vers la gauche.
        assert_eq!(ch.facing, crate::character::Facing::Left);
        assert!(offset_de(&ch) <= 1920.0);
    }

    #[test]
    fn il_passe_sur_l_ecran_voisin_quand_il_y_en_a_un() {
        // « il circule sur tous les écrans » (CLAUDE.md, étape 1). Le sol du
        // premier écran finit à x = 1920, celui du second y commence : les
        // deux faces sont adjointes, il doit enjamber la frontière.
        //
        // On force la marche vers la droite depuis tout près du bord.
        let m = monde();
        let mut ch = perso(&m, 1919.0);
        ch.facing = crate::character::Facing::Right;
        let mut rng = XorShift32::seeded(5);
        ch.intention = Some(ActiveIntention {
            kind: Intention::Flaner,
            depuis: Duration::ZERO,
            etat: EtatIntention::Flanerie {
                allure: Allure::Marche,
                jusqu_a: Duration::from_secs(60),
            },
        });

        let premier = m.platforms()[0].id;
        let mut t = Duration::ZERO;
        let mut passe = false;
        for _ in 0..60 {
            poursuivre(&mut ch, &m, &entrees_neutres(), &reglages(), t, DT, &mut rng);
            if plateforme_de(&ch) != premier {
                passe = true;
                break;
            }
            t += Duration::from_micros(16_667);
        }

        assert!(passe, "il n'a pas franchi la frontière entre les écrans");
        assert_eq!(m.get(plateforme_de(&ch)).unwrap().rect.left(), 1920.0);
        // Il entre par le bord gauche du sol voisin, donc à un offset petit.
        assert!(offset_de(&ch) < 50.0);
        // Et il continue dans le même sens : pas de demi-tour parasite.
        assert_eq!(ch.facing, crate::character::Facing::Right);
    }

    #[test]
    fn se_reposer_s_assoit_puis_se_termine() {
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::SeReposer,
            Duration::ZERO,
        ));

        // Première image : il s'assoit.
        let issue = poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages(),
            Duration::ZERO,
            DT,
            &mut rng,
        );
        assert_eq!(issue, Issue::EnCours);
        assert_eq!(ch.pose, POSE_SIT);

        // Il ne bouge pas pendant le repos.
        let ou = offset_de(&ch);
        poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages(),
            Duration::from_secs(2),
            DT,
            &mut rng,
        );
        assert_eq!(offset_de(&ch), ou);

        // Le repos dure au plus 15 s ; à 16 s il est fini.
        let issue = poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages(),
            Duration::from_secs(16),
            DT,
            &mut rng,
        );
        assert_eq!(issue, Issue::Finie);
        assert!(ch.intention.is_none());
    }

    #[test]
    fn toute_intention_expire_au_delai_d_abandon() {
        // **LE test de la décision n° 4.** Une seule règle remplace toute
        // l'énumération des cas de blocage : passé 20 s, l'intention échoue.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);

        for kind in [Intention::Flaner, Intention::SeReposer] {
            ch.intention = Some(ActiveIntention::nouvelle(kind, Duration::ZERO));

            // Juste avant le délai : l'intention n'a pas expiré. Elle peut
            // s'être terminée normalement (un repos dure au plus 15 s), donc
            // on vérifie seulement qu'elle n'a pas ÉCHOUÉ.
            let avant = poursuivre(
                &mut ch,
                &m,
                &entrees_neutres(),
                &reglages(),
                DELAI_ABANDON - Duration::from_millis(100),
                DT,
                &mut rng,
            );
            assert_ne!(avant, Issue::Echouee, "{kind:?} a expiré trop tôt");

            // Juste après : expirée. On réarme l'intention, la ligne
            // précédente ayant pu la consommer.
            ch.intention = Some(ActiveIntention::nouvelle(kind, Duration::ZERO));
            let apres = poursuivre(
                &mut ch,
                &m,
                &entrees_neutres(),
                &reglages(),
                DELAI_ABANDON + Duration::from_millis(100),
                DT,
                &mut rng,
            );
            assert_eq!(apres, Issue::Echouee, "{kind:?} n'a pas expiré");
        }
    }

    #[test]
    fn le_delai_court_depuis_le_debut_de_l_intention_pas_depuis_zero() {
        // Une intention commencée à t = 100 s doit expirer à 120 s, pas
        // immédiatement.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::Flaner,
            Duration::from_secs(100),
        ));

        let issue = poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages(),
            Duration::from_secs(110),
            DT,
            &mut rng,
        );
        assert_eq!(issue, Issue::EnCours);

        let issue = poursuivre(
            &mut ch,
            &m,
            &entrees_neutres(),
            &reglages(),
            Duration::from_secs(121),
            DT,
            &mut rng,
        );
        assert_eq!(issue, Issue::Echouee);
    }

    #[test]
    fn sans_intention_poursuivre_ne_fait_rien_et_le_dit() {
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        assert_eq!(
            poursuivre(
                &mut ch,
                &m,
                &entrees_neutres(),
                &reglages(),
                Duration::ZERO,
                DT,
                &mut rng
            ),
            Issue::Finie
        );
    }

    #[test]
    fn jouer_pose_l_animation_du_jeu_tire() {
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);

        for (jeu, pose) in [
            (Jeu::TeteQuiTourne, POSE_SPIN_HEAD),
            (Jeu::JambesQuiBalancent, POSE_SIT_DANGLE),
        ] {
            ch.intention = Some(ActiveIntention::nouvelle(
                Intention::Jouer(jeu),
                Duration::ZERO,
            ));
            let issue = poursuivre(
                &mut ch,
                &m,
                &entrees_neutres(),
                &reglages(),
                Duration::ZERO,
                DT,
                &mut rng,
            );
            assert_eq!(issue, Issue::EnCours);
            assert_eq!(ch.pose, pose, "jeu {jeu:?}");
        }
    }

    #[test]
    fn jouer_ne_deplace_pas_le_personnage() {
        // Les deux jeux sont des animations assises : `Velocity="0,0"` dans
        // `actions.xml`. Si le personnage dérivait, c'est qu'une vitesse
        // traîne quelque part.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::Jouer(Jeu::TeteQuiTourne),
            Duration::ZERO,
        ));

        let ou = offset_de(&ch);
        for i in 0..120 {
            poursuivre(
                &mut ch,
                &m,
                &entrees_neutres(),
                &reglages(),
                Duration::from_secs_f32(i as f32 * DT),
                DT,
                &mut rng,
            );
        }
        assert_eq!(offset_de(&ch), ou);
    }

    #[test]
    fn jouer_se_termine_avant_le_delai_d_abandon() {
        // Comme le repos : la durée est bornée sous `DELAI_ABANDON`, sinon
        // l'issue serait `Echouee` au lieu de `Finie` et la trace du mode
        // simulation mentirait.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::Jouer(Jeu::TeteQuiTourne),
            Duration::ZERO,
        ));

        let mut issue = Issue::EnCours;
        for i in 0..(20 * 60) {
            issue = poursuivre(
                &mut ch,
                &m,
                &entrees_neutres(),
                &reglages(),
                Duration::from_secs_f32(i as f32 * DT),
                DT,
                &mut rng,
            );
            if issue != Issue::EnCours {
                break;
            }
        }
        assert_eq!(issue, Issue::Finie);
    }

    #[test]
    fn jouer_sans_la_pose_echoue_au_lieu_de_figer() {
        // Défense en profondeur, comme `se_reposer_sans_pose_sit_echoue` :
        // le tirage filtre déjà, mais une config bricolée ne doit pas
        // produire un personnage invisible.
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "stand": { "frames": [1] }, "walk": { "frames": [2] } }
        }"#;
        let m = monde();
        let mut ch = Character::new(
            serde_json::from_str(json).unwrap(),
            Attachment::On {
                platform: m.platforms()[0].id,
                face: Face::Top,
                offset: 500.0,
            },
            Point::new(500.0, 1032.0),
        );
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::Jouer(Jeu::TeteQuiTourne),
            Duration::ZERO,
        ));

        assert_eq!(
            poursuivre(
                &mut ch,
                &m,
                &entrees_neutres(),
                &reglages(),
                Duration::ZERO,
                DT,
                &mut rng
            ),
            Issue::Echouee
        );
    }

    #[test]
    fn se_reposer_sans_pose_sit_echoue_au_lieu_de_figer() {
        // Défense en profondeur : le tirage ne devrait jamais proposer
        // `SeReposer` à un personnage sans `sit` (desire.rs). Mais si une
        // config bricolée y parvenait, l'intention doit ÉCHOUER — pas
        // asseoir un personnage sur une pose inexistante.
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "stand": { "frames": [1] }, "walk": { "frames": [2] } }
        }"#;
        let m = monde();
        let mut ch = Character::new(
            serde_json::from_str(json).unwrap(),
            Attachment::On {
                platform: m.platforms()[0].id,
                face: Face::Top,
                offset: 500.0,
            },
            Point::new(500.0, 1032.0),
        );
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::SeReposer,
            Duration::ZERO,
        ));

        assert_eq!(
            poursuivre(
                &mut ch,
                &m,
                &entrees_neutres(),
                &reglages(),
                Duration::ZERO,
                DT,
                &mut rng
            ),
            Issue::Echouee
        );
    }

    #[test]
    fn sans_signal_il_reste_assis_et_ne_s_affale_pas() {
        // **Une sieste ne s'improvise pas.** Sans signal, le biais vaut 1,
        // donc sous le seuil de 2 : il s'assoit et c'est tout. S'il
        // s'affalait de lui-même, « il dort quand tu t'en vas » perdrait tout
        // son sens — il dormirait tout le temps.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        let e = entrees_avec_biais_repos(1.0);

        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::SeReposer,
            Duration::ZERO,
        ));

        for i in 0..(14 * 60) {
            let t = Duration::from_secs_f32(i as f32 * DT);
            if poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng) != Issue::EnCours {
                break;
            }
            assert_eq!(ch.pose, POSE_SIT, "à {:.1} s il devrait être assis", t.as_secs_f32());
        }
    }

    #[test]
    fn avec_un_signal_il_s_assoit_puis_s_affale() {
        // La promesse de l'étape, dans l'ordre : 11 puis 21. C'est
        // l'ENCHAÎNEMENT qui dit « il dort », pas la frame 21 seule.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        let e = entrees_avec_biais_repos(8.0); // comme « inactif > 2 min »

        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::SeReposer,
            Duration::ZERO,
        ));

        // Première image : assis.
        poursuivre(&mut ch, &m, &e, &reglages(), Duration::ZERO, DT, &mut rng);
        assert_eq!(ch.pose, POSE_SIT);

        // Il finit par s'affaler, et en moins de 20 s (le délai d'abandon).
        let mut endormi_a = None;
        for i in 1..(20 * 60) {
            let t = Duration::from_secs_f32(i as f32 * DT);
            poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng);
            if ch.pose == POSE_SLEEP {
                endormi_a = Some(t);
                break;
            }
        }
        assert!(endormi_a.is_some(), "il ne s'est jamais affalé");
    }

    #[test]
    fn re_tirer_le_repos_pendant_le_sommeil_ne_le_fait_pas_se_rasseoir() {
        // **LE test de la continuité de pose**, et le seul qui justifie
        // qu'on n'ait PAS touché au délai d'abandon (décision n° 4).
        //
        // Un sommeil dure 20 à 60 s, le délai d'abandon coupe à 20 s, donc
        // l'intention est re-tirée. Sans continuité, on le verrait se
        // rasseoir puis se raffaler toutes les 20 secondes — un tic visible
        // à l'écran, absurde et inexplicable pour qui regarde.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        let e = entrees_avec_biais_repos(8.0);

        // On le met directement dans l'état « endormi ».
        ch.set_pose(POSE_SLEEP, Duration::ZERO);
        assert_eq!(ch.pose, POSE_SLEEP);

        // Une intention de repos FRAÎCHE, comme après un re-tirage.
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::SeReposer,
            Duration::from_secs(30),
        ));

        // La première image ne doit PAS le rasseoir.
        poursuivre(
            &mut ch,
            &m,
            &e,
            &reglages(),
            Duration::from_secs(30),
            DT,
            &mut rng,
        );
        assert_eq!(
            ch.pose, POSE_SLEEP,
            "il s'est rassis : la continuité de pose est cassée"
        );
    }

    #[test]
    fn sans_la_pose_sleep_il_reste_assis_au_lieu_d_echouer() {
        // Couverture partielle appliquée à une PHASE et non à une intention
        // (spec §8.6). Un pack sans pose de sommeil doit se reposer
        // normalement — assis — et non voir son repos échouer.
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "stand": { "frames": [1] }, "walk": { "frames": [2] },
                       "sit": { "frames": [11] } }
        }"#;
        let m = monde();
        let mut ch = Character::new(
            serde_json::from_str(json).unwrap(),
            Attachment::On {
                platform: m.platforms()[0].id,
                face: Face::Top,
                offset: 500.0,
            },
            Point::new(500.0, 1032.0),
        );
        let mut rng = XorShift32::seeded(1);
        let e = entrees_avec_biais_repos(8.0);

        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::SeReposer,
            Duration::ZERO,
        ));

        // `issue` sort de la boucle (comme dans
        // `jouer_se_termine_avant_le_delai_d_abandon`) pour pouvoir
        // l'affirmer APRÈS coup, et pas seulement à l'intérieur.
        let mut issue = Issue::EnCours;
        for i in 0..(19 * 60) {
            let t = Duration::from_secs_f32(i as f32 * DT);
            issue = poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng);
            assert_ne!(issue, Issue::Echouee, "le repos ne doit pas échouer");
            if issue != Issue::EnCours {
                break;
            }
            assert_eq!(ch.pose, POSE_SIT);
        }

        // Le repos doit se TERMINER normalement, et pas seulement « ne jamais
        // échouer ». Sans cette assertion, le test passerait même si la garde
        // `has_pose(POSE_SLEEP)` disparaissait : la phase basculerait en
        // `Endormi`, `set_pose` refuserait silencieusement la pose absente, et
        // `ch.pose` resterait figé sur `sit` par EFFET DE BORD — avec toutes
        // les assertions de la boucle encore vertes.
        assert_eq!(
            issue, Issue::Finie,
            "sans pose `sleep`, le repos doit se terminer, pas rester en cours"
        );
    }

    /// La durée de l'animation de réveil du manifeste de test : 2 × 100 ms.
    const ANIM_REVEIL: Duration = Duration::from_millis(200);

    #[test]
    fn au_reveil_il_dort_encore_un_moment_puis_se_redresse() {
        // **Le test de la demande d'origine** : au déverrouillage, le réveil
        // ne doit pas être instantané. Il dort d'abord — 2,5 à 4 s — et c'est
        // seulement à la fin qu'il se redresse.
        //
        // Noter `utilisateur_actif = true` : l'utilisateur vient de taper son
        // mot de passe, il est actif par construction. C'est ce qui rend ce
        // test intéressant — si le réveil était une phase `Endormi`,
        // l'interruption le couperait à la première image.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        let mut e = entrees_avec_biais_repos(8.0);
        e.utilisateur_actif = true;

        ch.set_pose(POSE_SLEEP, Duration::ZERO);
        ch.intention = Some(ActiveIntention::reveil(Duration::ZERO));

        let mut a_dormi = false;
        let mut redresse_a = None;
        let mut fini_a = None;

        for i in 0..(10 * 60) {
            let t = Duration::from_secs_f32(i as f32 * DT);
            let issue = poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng);

            if issue != Issue::EnCours {
                assert_eq!(issue, Issue::Finie, "le réveil ne doit pas échouer");
                fini_a = Some(t);
                break;
            }

            if ch.pose == POSE_SLEEP {
                // Une fois redressé, il ne doit PAS se raffaler : ce serait
                // le clignotement que la version précédente produisait.
                assert!(
                    redresse_a.is_none(),
                    "il s'est rendormi après s'être redressé, à {:.2} s",
                    t.as_secs_f32()
                );
                a_dormi = true;
            }
            if ch.pose == POSE_WAKE && redresse_a.is_none() {
                redresse_a = Some(t);
            }
        }

        assert!(a_dormi, "il n'a pas dormi du tout avant d'émerger");
        let redresse_a = redresse_a.expect("il ne s'est jamais redressé");
        let fini_a = fini_a.expect("le réveil ne s'est jamais terminé");

        // Le sommeil résiduel est tiré entre 2,5 et 4 s. On borne des DEUX
        // côtés : sans la borne basse, un réveil redevenu instantané
        // passerait — c'est exactement le défaut qu'on corrige ici.
        assert!(
            redresse_a >= Duration::from_secs_f32(2.5),
            "il s'est redressé au bout de {:.2} s : c'est trop tôt, le sommeil              résiduel doit durer au moins 2,5 s",
            redresse_a.as_secs_f32()
        );
        assert!(
            redresse_a <= Duration::from_secs_f32(4.0) + ANIM_REVEIL,
            "il s'est redressé au bout de {:.2} s : c'est trop tard",
            redresse_a.as_secs_f32()
        );

        // Et il se redresse pendant TOUTE l'animation, à une image près.
        let duree_redresse = fini_a.saturating_sub(redresse_a);
        assert!(
            duree_redresse + Duration::from_secs_f32(DT) >= ANIM_REVEIL,
            "l'animation de réveil n'a duré que {:.0} ms au lieu de 200",
            duree_redresse.as_secs_f32() * 1000.0
        );
    }

    #[test]
    fn deux_reveils_ne_tombent_pas_a_la_meme_image() {
        // **La marge, appliquée au réveil** (décision n° 3). À l'étape 3 il y
        // aura plusieurs personnages : s'ils émergeaient tous à la même
        // image, on verrait une chorégraphie au lieu d'animaux.
        //
        // ⚠️ On sème UNE SEULE FOIS et on laisse l'état avancer. Re-semer
        // `XorShift32::seeded(n)` avec de petits entiers séquentiels biaise
        // le premier tirage et rendrait ce test faussement vert — le piège
        // est consigné dans CLAUDE.md, il a déjà coûté un diagnostic.
        let m = monde();
        let mut rng = XorShift32::seeded(7);
        let mut e = entrees_avec_biais_repos(8.0);
        e.utilisateur_actif = true;

        let mut durees = Vec::new();
        for _ in 0..40 {
            let mut ch = perso(&m, 500.0);
            ch.set_pose(POSE_SLEEP, Duration::ZERO);
            ch.intention = Some(ActiveIntention::reveil(Duration::ZERO));

            for i in 0..(10 * 60) {
                let t = Duration::from_secs_f32(i as f32 * DT);
                if poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng) != Issue::EnCours {
                    durees.push(t);
                    break;
                }
            }
        }

        assert_eq!(durees.len(), 40, "un réveil ne s'est pas terminé");
        let min = durees.iter().min().unwrap();
        let max = durees.iter().max().unwrap();
        assert!(
            max.saturating_sub(*min) > Duration::from_secs_f32(0.8),
            "les 40 réveils tiennent dans {:.2} s : le tirage ne varie pas",
            max.saturating_sub(*min).as_secs_f32()
        );
    }

    #[test]
    fn sans_la_pose_wake_il_se_reveille_quand_meme() {
        // Couverture partielle (spec §8.6). Un pack sans animation de réveil
        // reste affalé le temps de la phase, puis repart — il ne doit NI
        // échouer, NI rester bloqué.
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "stand": { "frames": [1] }, "walk": { "frames": [2] },
                       "sit": { "frames": [11] }, "sleep": { "frames": [12] } }
        }"#;
        let m = monde();
        let mut ch = Character::new(
            serde_json::from_str(json).unwrap(),
            Attachment::On {
                platform: m.platforms()[0].id,
                face: Face::Top,
                offset: 500.0,
            },
            Point::new(500.0, 1032.0),
        );
        let mut rng = XorShift32::seeded(1);
        let mut e = entrees_avec_biais_repos(8.0);
        e.utilisateur_actif = true;

        ch.set_pose(POSE_SLEEP, Duration::ZERO);
        ch.intention = Some(ActiveIntention::reveil(Duration::ZERO));

        let mut issue = Issue::EnCours;
        for i in 0..(10 * 60) {
            let t = Duration::from_secs_f32(i as f32 * DT);
            issue = poursuivre(&mut ch, &m, &e, &reglages(), t, DT, &mut rng);
            if issue != Issue::EnCours {
                break;
            }
            assert_eq!(ch.pose, POSE_SLEEP, "sans `wake`, il reste affalé");
        }
        assert_eq!(issue, Issue::Finie, "le réveil doit se terminer");
    }

    // ── L'intention `Grimper` (Tâche 4, étape 4a) ───────────────────────

    /// Un monde d'un écran isolé — donc avec ses deux murs.
    ///
    /// `FakeProbe::deux_ecrans` ne conviendrait pas : deux écrans côte à côte
    /// se masquent mutuellement un mur (design §2.3), et l'on veut ici le cas
    /// le plus simple.
    fn monde_mure() -> World {
        World::from_screens(&FakeProbe::un_ecran().screens())
    }

    /// Fait tourner l'intention jusqu'à ce que `condition` soit vraie, ou
    /// jusqu'à `max_s` secondes simulées. Rend le temps écoulé.
    ///
    /// Écrite une fois ici plutôt que recopiée dans chaque test : les tests
    /// d'escalade durent des dizaines de secondes simulées, et la boucle est
    /// toujours la même.
    ///
    /// `impl FnMut(&Character) -> bool` plutôt qu'un `&dyn Fn` : le
    /// compilateur intègre la fermeture à l'appel, et le point d'appel reste
    /// une simple lambda. `FnMut` et non `Fn` pour qu'une condition puisse
    /// compter ce qu'elle voit passer si besoin.
    fn derouler(
        ch: &mut Character,
        world: &World,
        rng: &mut dyn Rng,
        max_s: f32,
        mut condition: impl FnMut(&Character) -> bool,
    ) -> f32 {
        let reglages = reglages();
        let mut t = Duration::ZERO;
        let mut ecoule = 0.0;

        while ecoule < max_s {
            if condition(ch) {
                return ecoule;
            }
            poursuivre(ch, world, &entrees_neutres(), &reglages, t, DT, rng);
            t += Duration::from_secs_f32(DT);
            ecoule += DT;
        }

        ecoule
    }

    #[test]
    fn choisir_depuis_un_mur_echoue_au_lieu_de_marcher_dessus() {
        // La garde structurelle de la Tâche 7 : `Choisir` refuse de partir
        // d'autre chose que `Face::Top`, même si plus rien d'autre ne devrait
        // normalement l'y amener. Une garde redondante ici coûte deux
        // lignes ; son absence a coûté un bug qui a survécu trois tâches et
        // leurs relectures — voir `expire_au_plafond_il_tombe_au_lieu_de_
        // marcher_dessus` ci-dessus pour ce bug précis.
        //
        // `ActiveIntention::accroche` n'est pas concernée par cette
        // garde : elle pose directement la phase `Accroche`, jamais
        // `Choisir` — voir `accroche_par_un_lancer_il_ne_lache_pas...` dans
        // `reflex.rs`, qui continue de passer.
        let m = monde_mure();
        let mur = m
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Right))
            .expect("mur gauche");

        let mut ch = perso_sur_le_sol(&m);
        ch.attachment = Attachment::On {
            platform: mur.id,
            face: Face::Right,
            offset: 300.0,
        };
        ch.intention = Some(ActiveIntention {
            kind: Intention::Grimper,
            depuis: Duration::ZERO,
            etat: EtatIntention::Grimpe {
                phase: PhaseGrimpe::Choisir,
                jusqu_a: Duration::ZERO,
            },
        });

        let mut rng = XorShift32::seeded(11);
        let reglages = reglages();
        let mut t = Duration::ZERO;

        // Plusieurs images, pas une seule : la garde doit tenir à chacune,
        // pas seulement à la première.
        for _ in 0..5 {
            let issue = poursuivre(&mut ch, &m, &entrees_neutres(), &reglages, t, DT, &mut rng);
            assert_ne!(ch.pose, POSE_WALK, "il ne doit pas marcher sur le mur");
            assert!(
                matches!(issue, Issue::Echouee | Issue::Finie),
                "phase Choisir depuis un mur : attendu un échec, obtenu {issue:?}"
            );
            t += Duration::from_secs_f32(DT);
        }

        assert!(
            ch.intention.is_none(),
            "l'intention doit avoir été abandonnée, elle est {:?}",
            ch.intention
        );
        // ⚠️ **Assertion corrigée (relecture finale de l'étape 4a).** La
        // garde échoue avant tout déplacement, mais elle NE laisse plus le
        // personnage pendu en attendant qu'un appel ultérieur de
        // `behavior::mod::pas` s'en aperçoive — c'était l'ancienne
        // assertion ici, et c'était précisément le bug : une image de
        // flottement, avec une dépendance à un appelant lointain pour s'en
        // sortir, exactement le motif de la Tâche 7. `poursuivre` appelle
        // maintenant `lacher_si_accroche` à son point d'étranglement unique,
        // dans la MÊME image que l'échec de cette garde — voir le
        // commentaire de ce point d'étranglement, en fin de `poursuivre`.
        assert!(
            matches!(ch.attachment, Attachment::Falling { .. }),
            "il devrait être tombé du mur dans la même image que l'échec, il est {:?}",
            ch.attachment
        );
    }

    #[test]
    fn grimper_rejoint_le_mur_puis_s_y_accroche() {
        let m = monde_mure();
        let mut ch = perso_sur_le_sol(&m);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

        let mut rng = XorShift32::seeded(7);
        derouler(&mut ch, &m, &mut rng, 60.0, |c| {
            matches!(c.attachment, Attachment::On { face, .. } if face != Face::Top)
        });

        match ch.attachment {
            Attachment::On { face, .. } => {
                assert!(face == Face::Left || face == Face::Right, "il est sur un mur");
            }
            autre => panic!("il devrait être accroché, il est {autre:?}"),
        }
    }

    #[test]
    fn grimper_monte_vraiment() {
        let m = monde_mure();
        let mut ch = perso_sur_le_sol(&m);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

        let mut rng = XorShift32::seeded(7);

        // On le laisse rejoindre le mur et grimper un moment.
        derouler(&mut ch, &m, &mut rng, 90.0, |c| {
            matches!(c.attachment, Attachment::On { face, offset, .. }
                     if face != Face::Top && offset < 900.0)
        });

        match ch.attachment {
            Attachment::On { face, offset, .. } => {
                assert_ne!(face, Face::Top);
                // L'offset d'une face verticale compte vers le bas : plus
                // petit = plus haut. Le bas du mur est à 1032.
                assert!(offset < 1000.0, "il devrait avoir quitté le bas du mur");
            }
            autre => panic!("il devrait être sur le mur, il est {autre:?}"),
        }
    }

    #[test]
    fn grimper_pose_les_bonnes_animations() {
        let m = monde_mure();
        let mut ch = perso_sur_le_sol(&m);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

        let mut rng = XorShift32::seeded(7);
        derouler(&mut ch, &m, &mut rng, 90.0, |c| c.pose == POSE_CLIMB_WALL);
        assert_eq!(ch.pose, POSE_CLIMB_WALL);
    }

    #[test]
    fn l_attache_au_mur_et_la_pose_changent_dans_la_meme_image() {
        // Sans cette garantie, la face serait déjà verticale alors que la
        // pose dirait encore `walk` — une image de marche dans le vide, que
        // l'invariant de simulation de la Tâche 7 relèverait.
        let m = monde_mure();
        let mut ch = perso_sur_le_sol(&m);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

        let mut rng = XorShift32::seeded(7);
        derouler(&mut ch, &m, &mut rng, 60.0, |c| {
            matches!(c.attachment, Attachment::On { face, .. } if face != Face::Top)
        });

        // `derouler` rend la main à l'image où la condition devient vraie,
        // c'est-à-dire AVANT d'appeler `poursuivre` une fois de plus : la
        // pose observée est donc bien celle posée par l'image de l'attache.
        assert_eq!(
            ch.pose, POSE_GRAB_WALL,
            "la pose doit basculer dans la même image que la face"
        );
    }

    #[test]
    fn grimper_regarde_le_mur() {
        // Mur gauche → il regarde à gauche ; mur droit → à droite. Sans quoi
        // il grimperait dos à la paroi.
        let m = monde_mure();
        let mut ch = perso_sur_le_sol(&m);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

        let mut rng = XorShift32::seeded(7);
        derouler(&mut ch, &m, &mut rng, 90.0, |c| {
            matches!(c.attachment, Attachment::On { face, .. } if face != Face::Top)
        });

        // ⚠️ Une face `Right` appartient au mur GAUCHE : le personnage se
        // tient à sa droite, donc à l'intérieur de l'écran (design §2.1).
        // C'est contre-intuitif la première fois.
        match ch.attachment {
            Attachment::On {
                face: Face::Right, ..
            } => {
                assert_eq!(ch.facing, Facing::Left, "mur gauche → il regarde à gauche")
            }
            Attachment::On {
                face: Face::Left, ..
            } => {
                assert_eq!(ch.facing, Facing::Right, "mur droit → il regarde à droite")
            }
            autre => panic!("il devrait être sur un mur, il est {autre:?}"),
        }
    }

    #[test]
    fn grimper_echoue_immediatement_sans_mur() {
        // L'écran du MILIEU d'une rangée de trois n'a aucun mur : ses deux
        // bords sont recouverts par ses voisins (design §2.3). L'intention
        // doit échouer tout de suite pour qu'une autre soit tirée — et
        // surtout pas figer le personnage.
        let entoure = World::from_screens(&[
            ScreenInfo {
                id: 1,
                work_area: Rect::new(-1920.0, 0.0, 1920.0, 1032.0),
                scale: 1.0,
            },
            ScreenInfo {
                id: 2,
                work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
                scale: 1.0,
            },
            ScreenInfo {
                id: 3,
                work_area: Rect::new(1920.0, 0.0, 1920.0, 1032.0),
                scale: 1.0,
            },
        ]);

        // Le personnage est sur le sol de l'écran 2, celui du milieu.
        let sol_milieu = entoure
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Top) && p.rect.left() == 0.0)
            .expect("le sol du milieu");
        let mut ch = perso_sur(&entoure, sol_milieu.id, 500.0);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Grimper, Duration::ZERO));

        let mut rng = XorShift32::seeded(3);
        let issue = poursuivre(
            &mut ch,
            &entoure,
            &entrees_neutres(),
            &reglages(),
            Duration::ZERO,
            DT,
            &mut rng,
        );

        assert_eq!(issue, Issue::Echouee);
        assert!(ch.intention.is_none());
    }

    #[test]
    fn grimper_a_un_delai_d_abandon_de_120_s() {
        let r = reglages();
        assert_eq!(delai_abandon(Intention::Grimper, &r), Duration::from_secs(120));
        assert_eq!(delai_abandon(Intention::Flaner, &r), DELAI_ABANDON);
        assert_eq!(delai_abandon(Intention::SeReposer, &r), DELAI_ABANDON);
        assert_eq!(delai_abandon(Intention::Jouer(Jeu::TeteQuiTourne), &r), DELAI_ABANDON);
    }

    #[test]
    fn le_delai_d_abandon_de_grimper_suit_le_facteur_de_vitesse() {
        // **Le test de la vague de correction finale, point 3.** Sans la
        // division par le facteur, une escalade à vitesse réduite expirerait
        // toujours à 120 s pile — le même délai qu'à vitesse normale, alors
        // qu'elle avance plus lentement. La marge doit rester la MÊME
        // proportion (~15 %) quel que soit le réglage.
        let a_vitesse = |facteur: f32| -> f32 {
            let config = crate::config::Config {
                vitesse: facteur,
                ..crate::config::Config::default()
            };
            let r = crate::config::Reglages::depuis(&config);
            delai_abandon(Intention::Grimper, &r).as_secs_f32()
        };

        assert_eq!(a_vitesse(1.0), 120.0);
        // Deux fois plus lent, deux fois plus de délai — sinon toute
        // escalade à ×0.5 expirerait aux deux tiers du mur (le bug décrit
        // dans le commentaire de `delai_abandon`).
        assert_eq!(a_vitesse(0.5), 240.0);
        // Le minimum autorisé par `FACTEUR_VITESSE_MIN` : le cas le plus
        // extrême que la config puisse produire.
        assert_eq!(a_vitesse(0.1), 1200.0);
    }

    #[test]
    fn expire_au_plafond_il_tombe_au_lieu_de_marcher_dessus() {
        // **Le test du bug trouvé à la Tâche 7**, par l'invariant du monde
        // vertical de `sim.rs`. Sans `lacher_si_accroche` appelée au point
        // précis où le délai d'abandon efface l'intention `Grimper`, le
        // personnage restait accroché au plafond (`Attachment::On { face:
        // Bottom, .. }`), intention `None` — et la couche 3, juste après,
        // lui repostait aussitôt un `Grimper` neuf (phase `Choisir`), qui le
        // faisait « marcher » (pose `walk`) le long de la face `Bottom` :
        // exactement le bug que l'invariant est fait pour attraper.
        //
        // **Plusieurs images, pas une seule** — la leçon de la Tâche 5, où
        // deux bugs ont survécu trois tâches parce que les tests
        // n'appelaient `poursuivre` qu'une fois. On continue d'appeler
        // `poursuivre` après l'expiration pour vérifier que la chute
        // s'installe VRAIMENT et ne se rattrape pas toute seule à l'image
        // suivante.
        let m = monde_mure();
        let plafond = m
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Bottom))
            .expect("plafond");

        let mut ch = perso_sur_le_sol(&m);
        ch.attachment = Attachment::On {
            platform: plafond.id,
            face: Face::Bottom,
            offset: 200.0,
        };
        // `depuis: ZERO`, et l'horloge du test démarre à 121 s : l'intention
        // est donc déjà périmée dès la première image — exactement ce qui
        // arrive à une escalade qui a trop traîné sur le plafond, sans avoir
        // à dérouler 120 s de traversée pour y arriver.
        ch.intention = Some(ActiveIntention {
            kind: Intention::Grimper,
            depuis: Duration::ZERO,
            etat: EtatIntention::Grimpe {
                phase: PhaseGrimpe::Plafond { cible: 900.0 },
                jusqu_a: Duration::ZERO,
            },
        });

        let mut rng = XorShift32::seeded(5);
        let reglages = reglages();
        let mut t = Duration::from_secs(121);

        for _ in 0..10 {
            poursuivre(&mut ch, &m, &entrees_neutres(), &reglages, t, DT, &mut rng);
            t += Duration::from_secs_f32(DT);

            // À CHAQUE image de ces dix-là, jamais la pose de marche — le
            // symptôme exact du bug.
            assert_ne!(
                ch.pose, POSE_WALK,
                "il ne doit jamais « marcher » sur le plafond"
            );
        }

        assert!(
            matches!(ch.attachment, Attachment::Falling { .. }),
            "il devrait être tombé du plafond à l'expiration, il est {:?}",
            ch.attachment
        );
    }

    #[test]
    fn une_escalade_complete_tient_dans_le_delai_d_abandon() {
        // Le calcul du design §4.4, vérifié plutôt que supposé.
        //
        // ⚠️ **Le pire cas au sol est l'écran ENTIER (1920 px), pas sa
        // moitié** (correction de la vague de relecture finale) : sur deux
        // écrans côte à côte, chaque écran n'a qu'UN SEUL mur (design §2.3),
        // et `mur_le_plus_proche` filtre par `meme_ecran` — donc rien ne
        // borne la distance à la moitié d'un écran. Une version antérieure
        // de ce test prenait 960 px et concluait à une marge de 30 % ; le
        // vrai pire cas, 1920 px, ne laisse que 15 %.
        //
        // Et on le vérifie à PLUSIEURS facteurs de vitesse, dont le minimum
        // autorisé (0.1) : `delai_abandon` divise maintenant le délai par ce
        // même facteur (point 3 de la relecture), donc la marge doit rester
        // la même proportion quel que soit le réglage — c'est ce test-ci qui
        // le démontre, plutôt que de ne vérifier que le facteur ×1 comme
        // avant.
        for facteur in [1.0f32, 0.5, 0.1] {
            let config = crate::config::Config {
                vitesse: facteur,
                ..crate::config::Config::default()
            };
            let r = crate::config::Reglages::depuis(&config);

            let marche = 1920.0 / r.vitesse_marche;
            let montee = 1032.0 / r.vitesse_escalade;
            let delai = delai_abandon(Intention::Grimper, &r).as_secs_f32();

            assert!(
                marche + montee < delai,
                "à vitesse ×{facteur}, une escalade complète dure {}s, au-dessus \
                 du délai de {delai}s",
                marche + montee
            );
        }
    }

    #[test]
    fn en_fin_d_accroche_il_lache_parfois_et_redescend_parfois() {
        // Décision n° 3 appliquée à la sortie de mur : les deux issues
        // doivent réellement sortir. Une seule graine, l'état qui avance —
        // re-semer par petits entiers biaiserait le premier tirage et
        // rendrait un faux négatif complet (piège documenté de `CLAUDE.md`).
        let m = monde_mure();
        let mut rng = XorShift32::seeded(12345);
        let reglages = reglages();

        let mut laches = 0;
        let mut descentes = 0;

        for _ in 0..200 {
            let mur = m
                .platforms()
                .iter()
                .find(|p| p.has_face(Face::Right))
                .expect("mur gauche");
            let mut ch = perso_sur_le_sol(&m);
            ch.attachment = Attachment::On {
                platform: mur.id,
                face: Face::Right,
                offset: 300.0,
            };
            // Une accroche déjà expirée : la prochaine image tire la sortie.
            //
            // ⚠️ **`jusqu_a` ne peut PAS valoir `Duration::ZERO` ici, et ce
            // n'est pas un détail.** `ZERO` ne veut PAS dire « déjà
            // expirée » mais « durée pas encore tirée » — c'est la
            // convention d'`ActiveIntention::nouvelle` et de `reveil`, et le
            // bras `PhaseGrimpe::Accroche` de `grimper()` l'applique
            // maintenant (correction du bug relevé en relecture de la
            // Tâche 5 : sans cette lecture, la toute première image
            // sautait le tirage lâcher/redescendre). Un test qui veut une
            // accroche VRAIMENT expirée doit donc donner un `jusqu_a` non
            // nul et déjà dépassé — ici 1 ms, largement avant le
            // `maintenant` d'une seconde de l'appel ci-dessous.
            ch.intention = Some(ActiveIntention {
                kind: Intention::Grimper,
                depuis: Duration::ZERO,
                etat: EtatIntention::Grimpe {
                    phase: PhaseGrimpe::Accroche,
                    jusqu_a: Duration::from_millis(1),
                },
            });

            poursuivre(
                &mut ch,
                &m,
                &entrees_neutres(),
                &reglages,
                Duration::from_secs(1),
                DT,
                &mut rng,
            );

            match ch.attachment {
                Attachment::Falling { .. } => laches += 1,
                Attachment::On { .. } => descentes += 1,
                _ => {}
            }
        }

        assert!(laches > 10, "il ne se lâche jamais ({laches} sur 200)");
        assert!(descentes > 10, "il ne redescend jamais ({descentes} sur 200)");
    }

    // ── Le plafond (Tâche 6, étape 4a) ──────────────────────────────────

    /// Le manifeste local (`manifeste()`), privé de certaines poses.
    ///
    /// Sert à éprouver la couverture partielle (spec §8.6) sans dépendre du
    /// disque : `desire.rs` a sa propre `manifeste_avec`, qui liste les poses
    /// à AJOUTER — ici on part du manifeste complet des tests d'escalade
    /// (avec `grabWall`/`climbWall` déjà déclarées) et on RETIRE celles
    /// données, ce qui est plus court pour un test qui ne veut retirer que
    /// les deux poses de plafond.
    ///
    /// `poses` est un champ public de `Manifest` (une `BTreeMap`) : pas
    /// besoin de repasser par le JSON pour le modifier.
    fn manifeste_sans(poses: &[&str]) -> Manifest {
        let mut m = manifeste();
        for p in poses {
            m.poses.remove(*p);
        }
        m
    }

    #[test]
    fn arrive_en_haut_du_mur_il_peut_basculer_au_plafond() {
        let m = monde_mure();
        let mur = m
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Right))
            .expect("mur gauche");

        // Posé en haut du mur, accroche expirée : la prochaine image tire la
        // sortie, et le plafond doit en faire partie.
        let mut rng = XorShift32::seeded(999);
        let reglages = reglages();

        let mut vus_au_plafond = 0;
        for _ in 0..200 {
            let mut ch = perso_sur_le_sol(&m);
            ch.attachment = Attachment::On {
                platform: mur.id,
                face: Face::Right,
                offset: 0.0, // tout en haut
            };
            // ⚠️ `jusqu_a` NE PEUT PAS valoir `Duration::ZERO` ici : depuis la
            // Tâche 5, `ZERO` signifie « durée pas encore tirée » (init.
            // paresseuse du bras `Accroche`), jamais « déjà expirée ». Avec
            // `ZERO`, cette toute première image se contenterait de tirer la
            // durée d'accroche et rendrait `EnCours` sans jamais tenter la
            // sortie — le personnage ne basculerait alors JAMAIS, et ce test
            // mesurerait un faux négatif complet plutôt qu'une vraie absence
            // de bascule. Un `jusqu_a` non nul et déjà dépassé (1 ms, contre
            // un `maintenant` d'une seconde) est la façon correcte d'écrire
            // « l'accroche est terminée » — le même correctif que celui déjà
            // appliqué au test `en_fin_d_accroche_il_lache_parfois…` plus
            // haut dans ce fichier.
            ch.intention = Some(ActiveIntention {
                kind: Intention::Grimper,
                depuis: Duration::ZERO,
                etat: EtatIntention::Grimpe {
                    phase: PhaseGrimpe::Accroche,
                    jusqu_a: Duration::from_millis(1),
                },
            });

            poursuivre(
                &mut ch,
                &m,
                &entrees_neutres(),
                &reglages,
                Duration::from_secs(1),
                DT,
                &mut rng,
            );

            if matches!(ch.attachment, Attachment::On { face: Face::Bottom, .. }) {
                vus_au_plafond += 1;
            }
        }

        assert!(
            vus_au_plafond > 10,
            "il ne passe jamais au plafond ({vus_au_plafond} sur 200)"
        );
    }

    #[test]
    fn au_plafond_il_traverse_avec_la_bonne_pose() {
        let m = monde_mure();
        let plafond = m
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Bottom))
            .expect("plafond");

        let mut ch = perso_sur_le_sol(&m);
        ch.attachment = Attachment::On {
            platform: plafond.id,
            face: Face::Bottom,
            offset: 200.0,
        };
        ch.intention = Some(ActiveIntention {
            kind: Intention::Grimper,
            depuis: Duration::ZERO,
            etat: EtatIntention::Grimpe {
                phase: PhaseGrimpe::Plafond { cible: 800.0 },
                // Sans effet ici : la phase `Plafond` ne LIT `jusqu_a` que
                // pour la reporter à l'identique, elle ne teste jamais son
                // expiration (contrairement à `Accroche`). `ZERO` reste
                // correct — « pas encore tirée » n'a simplement aucune
                // incidence tant qu'on n'a pas atteint la cible.
                jusqu_a: Duration::ZERO,
            },
        });

        let mut rng = XorShift32::seeded(5);
        // ⚠️ **Correction de relecture** : la condition d'arrêt initiale
        // était `c.pose == POSE_CLIMB_CEILING`, or cette pose est posée
        // **dès la première image**, en tête du bras `Plafond` — avant même
        // de calculer le déplacement. `derouler` en sortait donc au tout
        // premier tick, quel que soit le budget de 10 s passé : le test
        // prouvait qu'un instant existait, pas qu'une traversée avait lieu.
        // La condition porte maintenant sur le DÉPLACEMENT réel (au moins
        // 100 px parcourus vers la cible), qui ne peut se satisfaire qu'en
        // ayant vraiment avancé plusieurs images — même défaut, et même
        // correctif, que celui relevé sur le test du mur à la Tâche 5.
        derouler(&mut ch, &m, &mut rng, 10.0, |c| {
            (offset_de(c) - 200.0).abs() >= 100.0
        });

        assert_eq!(ch.pose, POSE_CLIMB_CEILING);

        match ch.attachment {
            Attachment::On {
                face: Face::Bottom,
                offset,
                ..
            } => {
                assert!(offset > 200.0, "il doit avoir avancé vers sa cible");
            }
            autre => panic!("il devrait être au plafond, il est {autre:?}"),
        }
    }

    #[test]
    fn le_plafond_d_un_ecran_prolonge_celui_du_voisin() {
        // La généralisation de `face_voisine` aux faces `Bottom` : au bout du
        // plafond de A, il passe sur celui de B, comme il le fait déjà au sol.
        let m = World::from_screens(&FakeProbe::deux_ecrans().screens());
        let plafond_a = m
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Bottom) && p.rect.left() == 0.0)
            .expect("plafond de gauche");

        let voisin = face_voisine(&m, plafond_a.id, Face::Bottom, true);
        assert!(voisin.is_some(), "le plafond de droite doit être trouvé");
        let (id, offset) = voisin.unwrap();
        assert_ne!(id, plafond_a.id);
        assert_eq!(offset, 0.0, "on y entre par son bord gauche");
    }

    #[test]
    fn au_bout_du_plafond_il_passe_au_plafond_du_voisin() {
        // Constat de relecture (Tâche 6) : le test ci-dessus n'appelle
        // `face_voisine` qu'À VIDE, en dehors de toute simulation. Le bloc
        // qui s'en sert réellement dans la phase `Plafond` (`if nouveau <
        // 0.0 || nouveau > longueur`) n'était donc jamais parcouru en
        // conditions réelles — exactement le défaut de la Tâche 5, où deux
        // bugs ont survécu à trois tâches parce que personne ne déroulait
        // la boucle. C'est le pendant horizontal de `grimper_monte_vraiment`
        // (qui, lui, prouve la même chose à la verticale, sur un mur).
        let m = World::from_screens(&FakeProbe::deux_ecrans().screens());
        let plafond_a = m
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Bottom) && p.rect.left() == 0.0)
            .expect("plafond de gauche");
        let id_depart = plafond_a.id;

        let mut ch = perso_sur_le_sol(&m);
        ch.attachment = Attachment::On {
            platform: plafond_a.id,
            face: Face::Bottom,
            // À 120 px du bord droit (le plafond de gauche fait 1920 px).
            offset: 1800.0,
        };
        ch.intention = Some(ActiveIntention {
            kind: Intention::Grimper,
            depuis: Duration::ZERO,
            etat: EtatIntention::Grimpe {
                // Une cible très au-delà du bord : il ne l'atteindra jamais
                // SUR ce plafond, il devra d'abord en sortir. C'est ce qui
                // force le passage par le bloc de franchissement plutôt que
                // par la sortie normale « cible atteinte ».
                phase: PhaseGrimpe::Plafond { cible: 5_000.0 },
                jusqu_a: Duration::ZERO,
            },
        });

        let mut rng = XorShift32::seeded(9);
        // 120 px à 16,1 px/s ≈ 7,5 s : 15 s de marge est largement
        // suffisant, et reste sous le délai d'abandon de 120 s.
        derouler(&mut ch, &m, &mut rng, 15.0, |c| plateforme_de(c) != id_depart);

        match ch.attachment {
            Attachment::On {
                platform,
                face: Face::Bottom,
                offset,
                ..
            } => {
                assert_ne!(platform, id_depart, "il devrait avoir changé de plafond");
                assert_eq!(
                    m.get(platform).unwrap().rect.left(),
                    1920.0,
                    "il doit être sur le plafond de l'écran voisin"
                );
                assert_eq!(offset, 0.0, "il entre par le bord gauche du plafond voisin");
            }
            autre => panic!("il devrait être au plafond voisin, il est {autre:?}"),
        }

        // La pose du plafond a été posée en tête du bras `Plafond`, AVANT le
        // calcul qui déclenche le changement de plateforme — donc dès cette
        // image-ci, jamais une pose de sol ou de mur.
        assert_eq!(ch.pose, POSE_CLIMB_CEILING);
    }

    #[test]
    fn en_fin_d_accroche_au_plafond_il_lache_parfois_et_repart_parfois() {
        // Constat de relecture (Tâche 6) : le chemin ajouté d'initiative
        // propre — depuis le plafond (`face == Face::Bottom`), un tirage
        // « redescendre » relance une traversée `Plafond` plutôt que de
        // retomber dans `Paroi` (qui poserait `climbWall` et raisonnerait
        // sur un axe vertical, tous deux faux au plafond) — n'était exercé
        // par AUCUN test. Même structure que
        // `en_fin_d_accroche_il_lache_parfois_et_redescend_parfois`, au mur.
        let m = monde_mure();
        let plafond = m
            .platforms()
            .iter()
            .find(|p| p.has_face(Face::Bottom))
            .expect("plafond");
        let longueur = plafond.rect.face_length(Face::Bottom);

        // Une seule graine, l'état qui avance : re-semer par petits entiers
        // séquentiels biaiserait le premier tirage (piège documenté de
        // `CLAUDE.md`).
        let mut rng = XorShift32::seeded(54321);
        let reglages = reglages();

        let mut laches = 0;
        let mut repartitions = 0;

        for _ in 0..200 {
            let mut ch = perso_sur_le_sol(&m);
            ch.attachment = Attachment::On {
                platform: plafond.id,
                face: Face::Bottom,
                offset: 400.0,
            };
            // Accroche déjà expirée : `Duration::ZERO` voudrait dire « pas
            // encore tirée », pas « expirée » (même piège que sur le test
            // équivalent au mur).
            ch.intention = Some(ActiveIntention {
                kind: Intention::Grimper,
                depuis: Duration::ZERO,
                etat: EtatIntention::Grimpe {
                    phase: PhaseGrimpe::Accroche,
                    jusqu_a: Duration::from_millis(1),
                },
            });

            poursuivre(
                &mut ch,
                &m,
                &entrees_neutres(),
                &reglages,
                Duration::from_secs(1),
                DT,
                &mut rng,
            );

            match ch.attachment {
                Attachment::Falling { .. } => laches += 1,
                Attachment::On {
                    face: Face::Bottom, ..
                } => {
                    repartitions += 1;

                    // Il doit avoir repris une traversée du plafond, avec
                    // une cible dans les bornes de la face — pas de
                    // `Paroi`, et pas de cible qui déborderait.
                    match ch.intention {
                        Some(ActiveIntention {
                            etat:
                                EtatIntention::Grimpe {
                                    phase: PhaseGrimpe::Plafond { cible },
                                    ..
                                },
                            ..
                        }) => {
                            assert!(
                                (0.0..=longueur).contains(&cible),
                                "cible {cible} hors des bornes [0, {longueur}]"
                            );
                        }
                        autre => panic!(
                            "attendu une nouvelle traversée du plafond (Plafond), obtenu {autre:?}"
                        ),
                    }
                }
                autre => panic!("attendu Falling ou On(Bottom), obtenu {autre:?}"),
            }
        }

        assert!(laches > 10, "il ne se lâche jamais du plafond ({laches} sur 200)");
        assert!(
            repartitions > 10,
            "il ne repart jamais en traversée ({repartitions} sur 200)"
        );
    }

    #[test]
    fn un_pack_sans_pose_de_plafond_grimpe_quand_meme() {
        // Couverture partielle (spec §8.6) : `climbCeiling` n'est PAS dans
        // les poses requises de `Grimper`. Un pack qui ne l'a pas doit
        // pouvoir grimper au mur, et simplement ne jamais passer au plafond.
        let table = crate::behavior::desire::TableEnvies::defaut();
        let sans_plafond = manifeste_sans(&[POSE_CLIMB_CEILING, POSE_GRAB_CEILING]);
        assert!(table.jouable(&sans_plafond, Intention::Grimper));
    }
}
