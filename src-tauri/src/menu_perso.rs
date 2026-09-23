//! Le menu du **clic droit sur le personnage** (spec §3.3, §9.1).
//!
//! Responsabilité unique : **décrire** ce menu (`lignes`). L'afficher est
//! l'affaire de `menu_natif.rs`, et ce qu'une entrée *fait* celle
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

use crate::actions::{ID_CATALOGUE, ID_P_CACHER, ID_P_CACHER_CE, ID_QUITTER, ID_TOUS_AU_MUR};
use crate::behavior::desire::TableEnvies;
use crate::behavior::intention::{Intention, Jeu};
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

    /// Reprend l'accroche là où il est — mur ou plafond, peu importe : c'est
    /// exactement l'intention que pose déjà un lancer contre une paroi
    /// (`ActiveIntention::accroche`). `HoldOntoWall` / `HoldOntoCeiling` de
    /// Shimeji-ee.
    ResterAccroche,

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
/// « Grimper au mur » et « Monter plus haut » sont en revanche deux LIGNES
/// distinctes — deux identifiants, deux libellés — qui partagent la MÊME
/// commande (`Commande::Intention(Intention::Grimper)`) : c'est la même
/// action de fond (grimper), seul son libellé change selon qu'on la propose
/// pour la déclencher ou pour la reprendre. Deux identifiants gardent le
/// décodage sans ambiguïté — un seul identifiant affiché avec deux libellés
/// différents selon le contexte aurait, lui, demandé au décodage de
/// connaître le contexte, ce qui n'a rien à y faire.
const ENVIES: &[(&str, &str, &[Ou], Commande)] = &[
    ("perso.flaner", "Flâner", &[Ou::Sol], Commande::Intention(Intention::Flaner)),
    (
        "perso.asseoir",
        "S'asseoir",
        &[Ou::Sol],
        Commande::Intention(Intention::SeReposer),
    ),
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
        Commande::Intention(Intention::Jouer(Jeu::JambesQuiBalancent)),
    ),
    (
        "perso.grimper",
        "Grimper au mur",
        &[Ou::Sol],
        Commande::Intention(Intention::Grimper),
    ),
    (
        "perso.monter",
        "Monter plus haut",
        &[Ou::Mur],
        Commande::Intention(Intention::Grimper),
    ),
    (
        "perso.rester",
        "Rester accroché",
        &[Ou::Mur, Ou::Plafond],
        Commande::ResterAccroche,
    ),
    (
        "perso.redescendre",
        "Redescendre",
        &[Ou::Mur],
        Commande::Redescendre,
    ),
    (
        "perso.lacher",
        "Se lâcher",
        &[Ou::Mur, Ou::Plafond],
        Commande::SeLacher,
    ),
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

/// Une ligne du menu : une entrée cliquable, ou un séparateur.
///
/// Le menu est **décrit** ici et **affiché** ailleurs (`menu_natif.rs`) :
/// la description est une fonction pure, donc testable sans écran, et
/// l'affichage ne sait rien des envies ni des packs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ligne {
    /// `id` est l'identifiant que reçoit `actions::executer`, exactement
    /// comme s'il venait d'un menu Tauri ; `libelle` est le texte affiché.
    Entree {
        id: &'static str,
        libelle: &'static str,
    },
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
pub fn lignes(manifeste: &Manifest, table: &TableEnvies, ou: Ou) -> Vec<Ligne> {
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
            Commande::ResterAccroche | Commande::Redescendre | Commande::SeLacher => true,
        };
        if jouable {
            lignes.push(Ligne::Entree { id, libelle });
        }
    }

    // Pas de séparateur si aucune envie n'est jouable : un menu qui
    // commencerait par une barre horizontale aurait l'air cassé.
    if !lignes.is_empty() {
        lignes.push(Ligne::Separateur);
    }

    // ── Les entrées communes avec le tray ───────────────────────────────
    //
    // « Démarrer avec Windows » est délibérément absent : c'est un réglage du
    // système, pas une humeur du personnage, et il n'a rien à faire au milieu
    // de « Flâner » et « S'asseoir ». Il reste dans le tray, qui est
    // justement l'endroit des réglages.
    //
    // « Tout le monde grimpe au mur » : un ordre à TOUS, donc rangé avec les
    // entrées communes et non parmi les envies de CE personnage. Toujours
    // proposé, même à un pack sans escalade : il s'adresse aux AUTRES
    // aussi, et chacun le refuse s'il ne peut pas l'exécuter.
    lignes.push(Ligne::Entree {
        id: ID_TOUS_AU_MUR,
        libelle: "Tout le monde grimpe au mur",
    });

    // Deux « Cacher », et le libellé doit dire lequel est lequel : « ce
    // personnage » s'en va seul (voir `actions::ID_P_CACHER_CE`), « tous »
    // cache tout le monde jusqu'au prochain « Afficher » du tray.
    lignes.push(Ligne::Entree {
        id: ID_P_CACHER_CE,
        libelle: "Cacher ce personnage",
    });
    lignes.push(Ligne::Entree {
        id: ID_P_CACHER,
        libelle: "Cacher tous les personnages",
    });
    lignes.push(Ligne::Entree {
        id: ID_CATALOGUE,
        libelle: "Catalogue de personnages…",
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
    });

    lignes
}

// Les tests de ce module vivent dans `menu_perso_tests.rs`
// (sortis d ici le 2026-09-14 : ils faisaient 229 des 623 lignes).
#[cfg(test)]
#[path = "menu_perso_tests.rs"]
mod tests;
