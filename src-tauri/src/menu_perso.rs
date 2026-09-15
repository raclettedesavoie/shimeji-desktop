//! Le menu du **clic droit sur le personnage** (spec §3.3, §9.1).
//!
//! Responsabilité unique : construire ce menu et l'afficher au curseur. Ce
//! qu'une entrée *fait* est dans `actions.rs` — ici on ne décide rien, on
//! propose.
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
//! Le coût est une poignée d'objets créés sur un clic humain : invisible.
//! Mémoriser le menu économiserait cela et coûterait toute la logique
//! « remettre les entrées d'accord avec l'état », qui est exactement la
//! classe de bugs que ce projet évite ailleurs par recalcul (décision n° 1).

use crate::actions::{ID_CATALOGUE, ID_P_CACHER, ID_QUITTER};
use crate::behavior::desire::TableEnvies;
use crate::behavior::intention::{Intention, Jeu};
use crate::character::attach::Attachment;
use crate::character::manifest::Manifest;
use crate::geom::Face;
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, WebviewWindow};

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

/// L'endroit d'où l'on fait un clic droit, simplifié aux trois cas qui
/// changent le menu proposé (spec §4, design du plan menu).
///
/// Dérivé de `Attachment` par `ou_de` plutôt que testé à la volée dans
/// `ouvrir` : la correspondance face → contexte de menu ne doit vivre qu'à
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
/// juste avant `ouvrir`, depuis `ch.attachment` : c'est le seul endroit où la
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
    (
        "perso.tete",
        "Faire tourner la tête",
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

/// Construit et affiche le menu au curseur. **Bloque** jusqu'à sa fermeture.
///
/// Appelée depuis le thread de la boucle 60 Hz, qui est donc figé pendant que
/// le menu est ouvert — le personnage s'immobilise. C'est voulu : c'est ce
/// que fait Shimeji-ee, et un personnage qui continuerait de marcher sous un
/// menu ouvert sur lui serait plus déroutant qu'amusant.
///
/// # Ce que `manifeste`, `table` et `ou` servent
///
/// `manifeste` et `table` retirent les envies injouables (spec §8.6) — ils
/// viennent du personnage **de cette fenêtre-là**, ce qui est la raison pour
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
/// clic repart dans la boucle d'événements de Tauri et atterrit dans
/// `actions::executer`.
///
/// Rend `Err` si la construction ou l'affichage échoue. L'appelant se
/// contente de le signaler — un menu qui ne s'ouvre pas n'empêche pas le
/// personnage de vivre.
pub fn ouvrir(
    app: &AppHandle,
    win: &WebviewWindow,
    manifeste: &Manifest,
    table: &TableEnvies,
    ou: Ou,
) -> Result<(), String> {
    // ── Les envies jouables par CE personnage, LÀ où il est ─────────────
    //
    // On construit d'abord un `Vec` de valeurs possédées, puis un second de
    // références de trait. En un seul passage, les `MenuItem` seraient
    // temporaires et les références pendantes — c'est l'emprunt de Rust qui
    // l'impose, et c'est une erreur qu'on ne peut pas commettre par accident.
    let mut entrees: Vec<MenuItem<tauri::Wry>> = Vec::new();
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
            entrees.push(
                MenuItem::with_id(app, *id, *libelle, true, None::<&str>)
                    .map_err(|e| format!("entrée « {libelle} » : {e}"))?,
            );
        }
    }

    // ── Les entrées communes avec le tray ───────────────────────────────
    //
    // « Démarrer avec Windows » est délibérément absent : c'est un réglage du
    // système, pas une humeur du personnage, et il n'a rien à faire au milieu
    // de « Flâner » et « S'asseoir ». Il reste dans le tray, qui est
    // justement l'endroit des réglages.
    let cacher = MenuItem::with_id(
        app,
        ID_P_CACHER,
        "Cacher les personnages",
        true,
        None::<&str>,
    )
    .map_err(|e| format!("entrée « cacher » : {e}"))?;

    let catalogue = MenuItem::with_id(
        app,
        ID_CATALOGUE,
        "Catalogue de personnages…",
        true,
        None::<&str>,
    )
    .map_err(|e| format!("entrée « catalogue » : {e}"))?;

    // **Deux séparateurs distincts et non un réutilisé** : une entrée de menu
    // ne peut occuper qu'une position, la poser deux fois ne la duplique pas.
    let separateur = PredefinedMenuItem::separator(app).map_err(|e| format!("séparateur : {e}"))?;
    let separateur_final =
        PredefinedMenuItem::separator(app).map_err(|e| format!("séparateur final : {e}"))?;

    // « Quitter » en dernier, derrière son propre séparateur.
    //
    // C'est la seule action irréversible du menu, et la seule qu'on ne veut
    // surtout pas cliquer de travers en visant « Catalogue ». La
    // mettre à part et tout en bas est la convention de toutes les
    // applications, pour cette raison exacte.
    let quitter = MenuItem::with_id(app, ID_QUITTER, "Quitter", true, None::<&str>)
        .map_err(|e| format!("entrée « quitter » : {e}"))?;

    // `&[&dyn IsMenuItem<R>]` : les entrées n'ont pas le même type concret
    // (`MenuItem`, `CheckMenuItem`, `PredefinedMenuItem`), donc on passe par
    // des références de trait. C'est la raison du `&` devant chacune.
    let mut refs: Vec<&dyn IsMenuItem<tauri::Wry>> = Vec::new();
    for e in &entrees {
        refs.push(e);
    }
    // Pas de séparateur si aucune envie n'est jouable : un menu qui
    // commencerait par une barre horizontale aurait l'air cassé.
    if !entrees.is_empty() {
        refs.push(&separateur);
    }
    refs.push(&cacher);
    refs.push(&catalogue);
    refs.push(&separateur_final);
    refs.push(&quitter);

    let menu = Menu::with_items(app, &refs).map_err(|e| format!("menu : {e}"))?;

    // ── L'affichage ─────────────────────────────────────────────────────
    //
    // `autoriser_activation` retire `WS_EX_NOACTIVATE` le temps du menu :
    // sans ça le menu resterait collé à l'écran. Le pourquoi complet est dans
    // le commentaire de cette fonction — il n'est pas devinable.
    // À qui rendre le focus après le menu — retenu AVANT de l'avoir pris.
    // Voir `fenetre_au_premier_plan` : sans cette restitution, un clic droit
    // sur le personnage laisserait l'éditeur muet.
    let precedente = crate::render::fenetre_au_premier_plan();

    let _ = crate::render::autoriser_activation(win, true);

    // Puis on prend RÉELLEMENT le premier plan, et on vérifie que Windows a
    // accepté — `muda` le demande aussi mais ignore son refus, et un refus
    // donne précisément le menu qui ne se referme pas quand on clique
    // ailleurs. Tout le raisonnement est dans `prendre_le_premier_plan`.
    let devant = crate::render::prendre_le_premier_plan(win).unwrap_or(false);

    // Un diagnostic plutôt qu'un `if` : on ne peut RIEN faire d'utile d'un
    // refus ici — afficher quand même vaut mieux que ne rien afficher. Mais
    // si le menu se recolle un jour à l'écran, cette ligne dit en une seconde
    // si la cause est là ou ailleurs, au lieu de relire trois crates.
    if !devant && std::env::var_os("SHIMEJI_MENU").is_some() {
        eprintln!(
            "menu : Windows a refusé le premier plan — le menu risque de ne pas se refermer au clic"
        );
    }

    // `popup_menu` place le menu au curseur et **bloque** jusqu'au choix.
    // Vérifié : `WebviewWindow::popup_menu`
    // (`tauri-2.11.5/src/webview/webview_window.rs:1681`), qui délègue à
    // `Window::popup_menu` (`src/window/mod.rs:1454`).
    let resultat = win
        .popup_menu(&menu)
        .map_err(|e| format!("affichage du menu : {e}"));

    // **Remis quoi qu'il arrive**, y compris si l'affichage a échoué : voir
    // l'avertissement de `autoriser_activation`. C'est la raison pour
    // laquelle le résultat est mis de côté au lieu d'être propagé par `?`.
    let _ = crate::render::autoriser_activation(win, false);

    // La seconde moitié de la recette : sans ce message vide, c'est le menu
    // SUIVANT qui se comporte mal. Voir `reveiller_la_file`.
    crate::render::reveiller_la_file(win);

    // Puis on rend le focus. Après avoir remis `WS_EX_NOACTIVATE`, pour que
    // notre fenêtre ne puisse plus le reprendre entre les deux appels.
    //
    // `if let Some` : il n'y avait pas forcément de premier plan à l'ouverture
    // (bureau sécurisé), auquel cas il n'y a rien à restaurer.
    if let Some(hwnd) = precedente {
        crate::render::rendre_le_premier_plan(hwnd);
    }

    // L'action, elle, ne s'exécute pas ici : le clic est parti dans la boucle
    // d'événements de Tauri et atterrira dans l'unique gestionnaire installé
    // par `tray.rs`. Voir l'avertissement en tête d'`actions.rs`.
    resultat
}

// Les tests de ce module vivent dans `menu_perso_tests.rs`
// (sortis d ici le 2026-09-14 : ils faisaient 229 des 623 lignes).
#[cfg(test)]
#[path = "menu_perso_tests.rs"]
mod tests;
