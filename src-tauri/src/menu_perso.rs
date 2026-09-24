//! Le menu du **clic droit sur le personnage** (spec §3.3, §9.1).
//!
//! Responsabilité unique : **décrire** ce menu (`lignes`). L'afficher est
//! l'affaire de `menu_fenetre.rs` (et de `ui/menu.*`), et ce qu'une entrée *fait* celle
//! d'`actions.rs` — ici on ne décide rien, on propose.
//!
//! # Le menu est reconstruit à chaque clic droit
//!
//! Et non construit une fois puis réaffiché. C'est ce qui le rend
//! **toujours vrai** sans aucun code de synchronisation :
//!
//! les envies dont le pack n'a pas les poses n'apparaissent pas, d'après le
//! manifeste **courant** — donc juste après un rechargement à chaud aussi
//! (spec §8.6).
//!
//! Le coût est un petit `Vec` construit sur un clic humain : invisible.
//! Mémoriser le menu économiserait cela et coûterait toute la logique
//! « remettre les entrées d'accord avec l'état », qui est exactement la
//! classe de bugs que ce projet évite ailleurs par recalcul (décision n° 1).

use crate::actions::{ID_CATALOGUE, ID_P_CACHER, ID_P_CACHER_CE, ID_QUITTER};
use crate::behavior::desire::TableEnvies;
use crate::behavior::intention::{Intention, Jeu};
use crate::behavior::tenue::Tenue;
use crate::character::attach::Attachment;
use crate::character::manifest::Manifest;
use crate::geom::Face;

/// Ce que demande une entrée du menu — pas toujours une intention.
///
/// # Pourquoi ce n'est pas simplement `Intention`
///
/// La table d'envies (`desire.rs`) et la couche 2 (`intention.rs`) ne
/// connaissent que des intentions à part entière, tirables au hasard. Mais
/// trois actions propres au menu de l'escalade — « Rester accroché »,
/// « Redescendre », « Se lâcher » — ne sont PAS des intentions : ce sont des
/// phases d'une escalade déjà en cours, ou une absence d'intention. Les
/// forcer dans `Intention` obligerait le tirage pondéré à savoir les éviter,
/// ce qui n'a rien à y faire (elles ne se tirent jamais, on ne fait qu'y
/// entrer depuis le menu).
///
/// Ce type couvre donc les deux formes, et c'est lui — pas `Intention` —
/// que porte `Entrees::commande` et qu'exécute `behavior::pas`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Commande {
    /// Une intention comme avant : le comportement normal, tiré au sort ou
    /// choisi ici au menu.
    Intention(Intention),

    /// Tenir cette action, ou la relâcher si c'est déjà celle qu'il tient —
    /// ce que fait un clic sur une ligne du menu du personnage, dont la coche
    /// dit l'état (spec §3). Résolue par `behavior::pas`, qui seul connaît
    /// `ch.tenue` au moment où la commande arrive.
    Basculer(Tenue),

    /// La tenir, quoi qu'il tienne déjà. Ce que devient un `Basculer` de la
    /// section « Tout le monde » quand tous ne la tiennent pas encore.
    Tenir(Tenue),

    /// La relâcher s'il la tient, ne rien faire sinon. L'intention en cours
    /// continue : il reprend sa vie normale à la fin de celle-ci.
    Relacher(Tenue),

    /// Reprend l'escalade en cours pour viser le BAS du mur — jamais
    /// proposée au plafond, où « redescendre » n'a pas de sens (design
    /// §4.5, décision n° 4 : pas de navigation calculée).
    Redescendre,

    /// Efface l'intention, sans plus. La règle de sécurité du monde vertical
    /// (`behavior::pas`) fait tomber le personnage toute seule, à la MÊME
    /// image : c'est `FallFromWall` / `FallFromCeiling` de Shimeji-ee, et ça
    /// ne coûte pas une ligne de physique en plus — voir le commentaire de
    /// `behavior::pas` à l'endroit où cette commande est traitée.
    SeLacher,
}

/// À qui revient la commande choisie dans le menu, et à qui elle ne revient
/// PAS.
///
/// # Le bug que cette fonction remplace
///
/// La commande était donnée à l'acteur **sous le curseur** au moment où elle
/// arrivait. Ça marchait tant qu'il n'y avait qu'un personnage : la boîte aux
/// lettres était vidée dans ses `Entrees` sans condition. Avec plusieurs
/// acteurs, il a fallu choisir un destinataire — et « celui sous le
/// curseur » est faux, parce qu'au moment où l'utilisateur relâche le clic
/// sur une entrée, **le curseur est sur le menu**, pas sur le personnage. Il
/// n'y avait donc aucun élu, la commande était consommée par personne, et
/// aucune entrée de menu ne faisait plus rien — sans le moindre message.
///
/// Le bon destinataire est celui **qui a ouvert le menu**, mémorisé à
/// l'ouverture ; c'est ce que porte `demandeur`.
///
/// `label` et non l'index dans le `Vec` : entre l'ouverture du menu et le
/// clic, un acteur a pu partir et décaler tous les suivants. Un label n'est
/// jamais réutilisé (voir `Acteur::label`), donc au pire il ne correspond
/// plus à personne — et la commande est ignorée, ce qui est exactement ce
/// qu'on veut d'un menu dont le personnage s'en est allé.
pub fn commande_pour(
    demandeur: &mut Option<String>,
    boite: &mut Option<Commande>,
    label: &str,
) -> Option<Commande> {
    // `as_deref` : emprunte le contenu du `Option<String>` en `Option<&str>`
    // pour le comparer au label sans rien cloner.
    if demandeur.as_deref() != Some(label) {
        return None;
    }

    // `?` : la boîte est vide tant que l'utilisateur n'a pas encore choisi —
    // le menu bloque le thread de la boucle, mais le clic revient par la
    // boucle d'événements de Tauri, donc quelques images plus tard. On sort
    // en laissant `demandeur` en place, pour le retrouver à l'image suivante.
    let commande = boite.take()?;

    // Servie : le demandeur est oublié, sans quoi une commande déposée plus
    // tard (menu du tray, par exemple) lui reviendrait par erreur.
    *demandeur = None;
    Some(commande)
}

/// La commande que reçoit un acteur à cette image : la sienne d'abord, et
/// sinon celle adressée à tous (« Tout le monde grimpe au mur »).
///
/// **La personnelle gagne**, parce qu'elle est plus précise : l'utilisateur
/// qui vient de choisir « S'asseoir » pour CE personnage ne doit pas le voir
/// partir au mur parce qu'un ordre collectif est tombé dans la même image.
///
/// `or` : rend `personnelle` si elle est `Some`, sinon `pour_tous` — le
/// `match` à deux bras qu'on écrirait à la main.
pub fn commande_de_l_acteur(
    personnelle: Option<Commande>,
    pour_tous: Option<Commande>,
) -> Option<Commande> {
    personnelle.or(pour_tous)
}

/// L'endroit d'où l'on fait un clic droit, simplifié aux trois cas qui
/// changent le menu proposé (spec §4, design du plan menu).
///
/// Dérivé de `Attachment` par `ou_de` plutôt que testé à la volée dans
/// `lignes` : la correspondance face → contexte de menu ne doit vivre qu'à
/// UN endroit, sans quoi elle finirait par diverger de celle utilisée par la
/// physique.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ou {
    /// Au sol, en chute, ou porté — le menu d'aujourd'hui, inchangé.
    Sol,
    /// Accroché à un mur (face `Left` ou `Right`).
    Mur,
    /// Suspendu au plafond (face `Bottom`).
    Plafond,
}

/// Le contexte de menu qui correspond à l'endroit où est accroché le
/// personnage — ou `Sol` s'il ne l'est pas du tout.
///
/// **La seule fonction qui connaît cette correspondance.** `main.rs` l'appelle
/// juste avant `lignes`, depuis `ch.attachment` : c'est le seul endroit où la
/// boucle 60 Hz sait où en est CE personnage-là.
pub fn ou_de(attachment: &Attachment) -> Ou {
    match attachment {
        Attachment::On {
            face: Face::Left | Face::Right,
            ..
        } => Ou::Mur,
        Attachment::On {
            face: Face::Bottom, ..
        } => Ou::Plafond,
        // `Face::Top`, `Falling`, `Dragged` : au sol, en chute, ou porté.
        _ => Ou::Sol,
    }
}

/// Les envies proposées par le menu, dans l'ordre d'affichage.
///
/// # ⚠️ À TENIR À JOUR
///
/// **Toute nouvelle intention jouable s'ajoute ici, dans la même tâche que
/// son implémentation.** C'est une ligne de plus dans ce tableau et rien
/// d'autre : l'identifiant est décodé par `commande_de`, la disponibilité
/// est déduite du manifeste (pour une `Intention`) ou de l'endroit (pour les
/// trois autres), et `actions::executer` n'a pas de cas à ajouter. Une
/// intention absente de ce tableau existe pour le tirage aléatoire mais
/// reste inaccessible à l'utilisateur — un manque silencieux, qui ne casse
/// aucun test.
///
/// `&'static [(…)]` plutôt qu'un `match` en deux exemplaires : la table est
/// lue dans les deux sens — pour construire les entrées, et pour décoder un
/// identifiant reçu. Deux `match` symétriques finiraient par diverger.
///
/// # Le quatrième champ : où cette entrée apparaît
///
/// Une liste et non une seule valeur : « Rester accroché » et « Se lâcher »
/// sont pertinentes à la fois sur un mur ET au plafond (la commande qu'elles
/// posent ne connaît pas la face, elle relit `ch.attachment` à l'exécution).
/// Une seule ligne leur suffit donc, avec les DEUX contextes dans la liste —
/// dupliquer la ligne par contexte serait une seconde source de vérité pour
/// le même identifiant.
///
/// « Grimper au mur » est de même **une seule ligne**, proposée au sol, au
/// mur et au plafond : tenue, elle ne change pas de sens selon qu'on la
/// lance ou qu'on la reprend. « Monter plus haut », qui la doublait au mur
/// sous un autre identifiant, a disparu (spec « menu sur mesure » §2).
///
/// # Tenue ou ponctuelle
///
/// Une ligne `Commande::Basculer(…)` est une action **tenue** : elle a une
/// coche, et dure jusqu'à ce qu'on la décoche. Une ligne
/// `Commande::Intention(…)` est **ponctuelle** : elle se joue une fois.
const ENVIES: &[(&str, &str, &[Ou], Commande)] = &[
    ("perso.flaner", "Flâner", &[Ou::Sol], Commande::Basculer(Tenue::Flaner)),
    ("perso.asseoir", "S'asseoir", &[Ou::Sol], Commande::Basculer(Tenue::Asseoir)),
    // Le libellé ne décrit PAS le dessin, et c'est délibéré. `spinHead` est
    // un numéro de slot Shimeji (`SitAndSpinHeadAction`), pas une promesse :
    // `blob` y fait tourner sa tête, un autre pack y mange ou y joue sa pose
    // de signature. Annoncer « Faire tourner la tête » promettait donc, sur
    // presque tous les packs du catalogue, quelque chose que le pack ne
    // tenait pas. « Faire son petit truc » dit seulement que c'est SON
    // animation à lui — ce qui reste vrai quel que soit le dessin.
    //
    // L'identifiant, lui, garde le nom du slot : il n'est jamais affiché, et
    // c'est ce qui permet de retrouver la pose que la ligne déclenche.
    (
        "perso.tete",
        "Faire son petit truc",
        &[Ou::Sol],
        Commande::Intention(Intention::Jouer(Jeu::TeteQuiTourne)),
    ),
    (
        "perso.jambes",
        "Balancer les jambes",
        &[Ou::Sol],
        Commande::Basculer(Tenue::BalancerLesJambes),
    ),
    // Une seule ligne pour les trois endroits : tenue, elle ne change pas
    // de sens selon qu'on la lance ou qu'on la reprend. « Monter plus
    // haut », qui la doublait au mur, a disparu (spec §2).
    (
        "perso.grimper",
        "Grimper au mur",
        &[Ou::Sol, Ou::Mur, Ou::Plafond],
        Commande::Basculer(Tenue::Grimper),
    ),
    (
        "perso.rester",
        "Rester accroché",
        &[Ou::Mur, Ou::Plafond],
        Commande::Basculer(Tenue::ResterAccroche),
    ),
    ("perso.redescendre", "Redescendre", &[Ou::Mur], Commande::Redescendre),
    ("perso.lacher", "Se lâcher", &[Ou::Mur, Ou::Plafond], Commande::SeLacher),
];

/// La section « Tout le monde » (spec §3) : les actions du SOL, données à
/// tous les présents par la boîte `Actions::pour_tous`.
///
/// Une table à part et non une colonne d'`ENVIES` : ses identifiants sont
/// DIFFÉRENTS (`tous.*`), et c'est ce qui les envoie dans l'autre boîte.
/// Réutiliser `perso.asseoir` ferait asseoir le seul demandeur.
///
/// Seulement le sol : les autres sont n'importe où, et « Se lâcher » n'a
/// de sens que pour qui est accroché.
const TOUS: &[(&str, &str, Commande)] = &[
    ("tous.flaner", "Flâner", Commande::Basculer(Tenue::Flaner)),
    ("tous.asseoir", "S'asseoir", Commande::Basculer(Tenue::Asseoir)),
    (
        "tous.tete",
        "Faire son petit truc",
        Commande::Intention(Intention::Jouer(Jeu::TeteQuiTourne)),
    ),
    ("tous.jambes", "Balancer les jambes", Commande::Basculer(Tenue::BalancerLesJambes)),
    ("tous.grimper", "Grimper au mur", Commande::Basculer(Tenue::Grimper)),
];

/// La commande que désigne un identifiant d'entrée, s'il en désigne une.
///
/// Appelée par `actions::executer` pour le cas par défaut : tout ce qui
/// n'est pas une entrée connue est peut-être une envie du menu du personnage.
pub fn commande_de(id: &str) -> Option<Commande> {
    // `find` puis `map` plutôt qu'une boucle : on cherche la ligne dont
    // l'identifiant correspond, et on n'en garde que la commande.
    ENVIES
        .iter()
        .find(|(i, _, _, _)| *i == id)
        .map(|(_, _, _, c)| *c)
}

/// La commande d'une entrée de la section « Tout le monde », si `id` en
/// désigne une. Le pendant de `commande_de` pour la table `TOUS`.
pub fn commande_de_tous(id: &str) -> Option<Commande> {
    TOUS.iter().find(|(i, _, _)| *i == id).map(|(_, _, c)| *c)
}

/// L'identifiant `&'static` qui correspond à `id`, s'il est l'une de nos
/// entrées ; `None` sinon.
///
/// La fenêtre du menu renvoie une `String` : c'est ici qu'elle redevient
/// un identifiant du programme. **N'accepter que ce qu'on connaît** —
/// `actions::executer` ne recevra jamais une chaîne arbitraire venue d'un
/// webview.
pub fn id_connu(id: &str) -> Option<&'static str> {
    let communs = [ID_P_CACHER_CE, ID_P_CACHER, ID_CATALOGUE, ID_QUITTER];
    ENVIES
        .iter()
        .map(|(i, _, _, _)| *i)
        .chain(TOUS.iter().map(|(i, _, _)| *i))
        .chain(communs)
        .find(|i| *i == id)
}

/// Un personnage présent, vu par la section « Tout le monde » : ce qu'il
/// tient, et ce qu'il PEUT tenir là où il est (`tenue::peut_tenir`).
///
/// `peut_tenir` : sans lui, un pack sans escalade compterait parmi ceux qui
/// « ne tiennent pas encore » Grimper au mur, alors que `behavior::pas` lui
/// refuse l'ordre — et la ligne ne se décocherait jamais.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Present {
    pub tenue: Option<Tenue>,
    pub peut_tenir: Vec<Tenue>,
}

/// Vrai si TOUS les présents capables de tenir `t` la tiennent — et qu'il y
/// en a au moins un. Sans personne de capable, « tous la tiennent » serait
/// vrai par vacuité, et un clic relâcherait… personne : on tient.
///
/// `peekable` : un itérateur qui permet de regarder le premier élément sans
/// le consommer — ici, pour savoir s'il y a au moins un capable avant de
/// vérifier qu'ils la tiennent tous.
pub fn tous_la_tiennent(t: Tenue, presents: &[Present]) -> bool {
    let mut capables = presents.iter().filter(|p| p.peut_tenir.contains(&t)).peekable();
    capables.peek().is_some() && capables.all(|p| p.tenue == Some(t))
}

/// Ce que devient une commande de la section « Tout le monde » pour les
/// personnages présents (spec §3) : coché si tous ceux qui PEUVENT la tenir
/// la tiennent, donc un clic relâche chez tous ; sinon, un clic la donne à
/// tous (et chacun refuse ce qu'il ne peut pas faire).
///
/// Résolue UNE fois par la boucle, avant de servir les acteurs : chaque
/// acteur résolvant son propre `Basculer`, un personnage déjà assis se
/// relèverait pendant que les autres s'assoient.
pub fn resoudre_pour_tous(c: Commande, presents: &[Present]) -> Commande {
    match c {
        Commande::Basculer(t) => {
            if tous_la_tiennent(t, presents) {
                Commande::Relacher(t)
            } else {
                Commande::Tenir(t)
            }
        }
        autre => autre,
    }
}

/// Une ligne du menu : une entrée cliquable, un titre de section, ou un
/// séparateur.
///
/// Le menu est **décrit** ici et **affiché** ailleurs (`menu_fenetre.rs`) :
/// la description est une fonction pure, donc testable sans écran, et
/// l'affichage ne sait rien des envies ni des packs.
///
/// `Serialize` avec `tag = "type"` : chaque ligne devient un objet JSON
/// portant son genre — `{"type":"Entree","id":…,"libelle":…,"coche":…}`,
/// `{"type":"Titre","texte":…}`, `{"type":"Separateur"}` — que `menu.js`
/// lit par `ligne.type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "type")]
pub enum Ligne {
    /// `id` est l'identifiant que reçoit `actions::executer` ; `coche` dit
    /// si l'action est tenue — DÉDUITE de la tenue, jamais stockée.
    Entree {
        id: &'static str,
        libelle: &'static str,
        coche: bool,
    },
    /// Un intitulé de section, non cliquable.
    Titre { texte: &'static str },
    Separateur,
}

/// Le menu d'un personnage, ligne par ligne, **tel qu'il doit être LÀ où il
/// est**.
///
/// # Ce que `manifeste`, `table` et `ou` servent
///
/// `manifeste` et `table` retirent les envies injouables (spec §8.6) — ils
/// viennent du personnage **qui a été cliqué**, ce qui est la raison pour
/// laquelle cette fonction est appelée depuis la boucle et non depuis
/// `setup` : c'est le seul endroit où le manifeste courant est connu.
///
/// `ou` retire les entrées qui n'ont pas de sens LÀ où il est — voir `Ou` et
/// `ou_de`. C'est ce qui corrige le bug rapporté à l'écran : sans ce filtre,
/// le menu proposait « Flâner » à un personnage accroché à un mur, et le
/// choisir le faisait tomber (règle de sécurité du monde vertical).
///
/// Aucun `Actions` en paramètre, et c'est la conséquence directe du
/// gestionnaire unique : ce fichier ne déclenche **rien**, il propose. Le
/// choix atterrit dans `actions::executer`.
///
/// `tenue` est celle du personnage cliqué, `presents` décrit tous les
/// présents — lui compris. Ce sont les seules sources des coches.
pub fn lignes(
    manifeste: &Manifest,
    table: &TableEnvies,
    ou: Ou,
    tenue: Option<Tenue>,
    presents: &[Present],
) -> Vec<Ligne> {
    let mut lignes: Vec<Ligne> = Vec::new();

    // ── Les envies jouables par CE personnage, LÀ où il est ─────────────
    for (id, libelle, contextes, commande) in ENVIES {
        if !contextes.contains(&ou) {
            continue;
        }
        // Seule une `Intention` est gardée par la couverture partielle du
        // manifeste (spec §8.6) : les trois autres commandes ne demandent
        // pas de pose particulière au-delà de celles que l'escalade en cours
        // exige déjà pour être là où le menu les propose.
        let jouable = match commande {
            Commande::Intention(i) => table.jouable(manifeste, *i),
            Commande::Basculer(t) | Commande::Tenir(t) | Commande::Relacher(t) => {
                table.jouable(manifeste, t.intention())
            }
            Commande::Redescendre | Commande::SeLacher => true,
        };
        if jouable {
            // Seul un `Basculer` a une coche : une action ponctuelle n'a
            // pas d'état à montrer.
            let coche = matches!(commande, Commande::Basculer(t) if tenue == Some(*t));
            lignes.push(Ligne::Entree { id, libelle, coche });
        }
    }

    // Pas de séparateur si aucune envie n'est jouable : un menu qui
    // commencerait par une barre horizontale aurait l'air cassé.
    if !lignes.is_empty() {
        lignes.push(Ligne::Separateur);
    }

    // ── La section « Tout le monde » (spec §3) ──────────────────────────
    //
    // Un titre plutôt que « tout le monde » répété à chaque ligne : c'est la
    // demande de l'auteur. Toujours proposée, même à un pack sans escalade :
    // elle s'adresse aux AUTRES aussi, et chacun refuse ce qu'il ne peut pas
    // exécuter (`behavior::pas`).
    lignes.push(Ligne::Titre { texte: "Tout le monde" });
    for (id, libelle, commande) in TOUS {
        let coche = match commande {
            Commande::Basculer(t) => tous_la_tiennent(*t, presents),
            _ => false,
        };
        lignes.push(Ligne::Entree { id, libelle, coche });
    }
    lignes.push(Ligne::Separateur);

    // ── Les entrées communes avec le tray ───────────────────────────────
    //
    // « Démarrer avec Windows » est délibérément absent : c'est un réglage du
    // système, pas une humeur du personnage, et il n'a rien à faire au milieu
    // de « Flâner » et « S'asseoir ». Il reste dans le tray, qui est
    // justement l'endroit des réglages.

    // Deux « Cacher », et le libellé doit dire lequel est lequel : « ce
    // personnage » s'en va seul (voir `actions::ID_P_CACHER_CE`), « tous »
    // cache tout le monde jusqu'au prochain « Afficher » du tray.
    lignes.push(Ligne::Entree {
        id: ID_P_CACHER_CE,
        libelle: "Cacher ce personnage",
        coche: false,
    });
    lignes.push(Ligne::Entree {
        id: ID_P_CACHER,
        libelle: "Cacher tous les personnages",
        coche: false,
    });
    lignes.push(Ligne::Entree {
        id: ID_CATALOGUE,
        libelle: "Catalogue de personnages…",
        coche: false,
    });

    // « Quitter » en dernier, derrière son propre séparateur.
    //
    // C'est la seule action irréversible du menu, et la seule qu'on ne veut
    // surtout pas cliquer de travers en visant « Catalogue ». La
    // mettre à part et tout en bas est la convention de toutes les
    // applications, pour cette raison exacte.
    lignes.push(Ligne::Separateur);
    lignes.push(Ligne::Entree {
        id: ID_QUITTER,
        libelle: "Quitter",
        coche: false,
    });

    lignes
}

// Les tests de ce module vivent dans `menu_perso_tests.rs`
// (sortis d ici le 2026-09-14 : ils faisaient 229 des 623 lignes).
#[cfg(test)]
#[path = "menu_perso_tests.rs"]
mod tests;
