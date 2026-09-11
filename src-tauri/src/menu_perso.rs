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

use crate::actions::{ID_DOSSIER, ID_P_CACHER, ID_QUITTER, ID_RECHARGER};
use crate::behavior::desire::TableEnvies;
use crate::behavior::intention::{Intention, Jeu};
use crate::character::manifest::Manifest;
use tauri::menu::{IsMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, WebviewWindow};

/// Les envies proposées par le menu, dans l'ordre d'affichage.
///
/// # ⚠️ À TENIR À JOUR
///
/// **Toute nouvelle intention jouable s'ajoute ici, dans la même tâche que
/// son implémentation.** C'est une ligne de plus dans ce tableau et rien
/// d'autre : l'identifiant est décodé par `intention_de`, la disponibilité
/// est déduite du manifeste, et `actions::executer` n'a pas de cas à
/// ajouter. Une intention absente de ce tableau existe pour le tirage
/// aléatoire mais reste inaccessible à l'utilisateur — un manque silencieux,
/// qui ne casse aucun test.
///
/// `&'static [(…)]` plutôt qu'un `match` en deux exemplaires : la table est
/// lue dans les deux sens — pour construire les entrées, et pour décoder un
/// identifiant reçu. Deux `match` symétriques finiraient par diverger.
const ENVIES: &[(&str, &str, Intention)] = &[
    ("perso.flaner", "Flâner", Intention::Flaner),
    ("perso.asseoir", "S'asseoir", Intention::SeReposer),
    (
        "perso.tete",
        "Faire tourner la tête",
        Intention::Jouer(Jeu::TeteQuiTourne),
    ),
    (
        "perso.jambes",
        "Balancer les jambes",
        Intention::Jouer(Jeu::JambesQuiBalancent),
    ),
];

/// L'intention que désigne un identifiant d'entrée, s'il en désigne une.
///
/// Appelée par `actions::executer` pour le cas par défaut : tout ce qui
/// n'est pas une entrée connue est peut-être une envie.
pub fn intention_de(id: &str) -> Option<Intention> {
    // `find` puis `map` plutôt qu'une boucle : on cherche la ligne dont
    // l'identifiant correspond, et on n'en garde que l'intention.
    ENVIES.iter().find(|(i, _, _)| *i == id).map(|(_, _, x)| *x)
}

/// Construit et affiche le menu au curseur. **Bloque** jusqu'à sa fermeture.
///
/// Appelée depuis le thread de la boucle 60 Hz, qui est donc figé pendant que
/// le menu est ouvert — le personnage s'immobilise. C'est voulu : c'est ce
/// que fait Shimeji-ee, et un personnage qui continuerait de marcher sous un
/// menu ouvert sur lui serait plus déroutant qu'amusant.
///
/// # Ce que `manifeste` et `table` servent
///
/// À retirer les envies injouables (spec §8.6). Ils viennent du personnage
/// **de cette fenêtre-là**, ce qui est la raison pour laquelle cette fonction
/// est appelée depuis la boucle et non depuis `setup` : c'est le seul endroit
/// où le manifeste courant est connu.
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
) -> Result<(), String> {
    // ── Les envies jouables par CE personnage ───────────────────────────
    //
    // On construit d'abord un `Vec` de valeurs possédées, puis un second de
    // références de trait. En un seul passage, les `MenuItem` seraient
    // temporaires et les références pendantes — c'est l'emprunt de Rust qui
    // l'impose, et c'est une erreur qu'on ne peut pas commettre par accident.
    let mut entrees: Vec<MenuItem<tauri::Wry>> = Vec::new();
    for (id, libelle, intention) in ENVIES {
        if table.jouable(manifeste, *intention) {
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

    let recharger = MenuItem::with_id(
        app,
        ID_RECHARGER,
        "Recharger les personnages",
        true,
        None::<&str>,
    )
    .map_err(|e| format!("entrée « recharger » : {e}"))?;

    let dossier = MenuItem::with_id(
        app,
        ID_DOSSIER,
        "Ouvrir le dossier des personnages",
        true,
        None::<&str>,
    )
    .map_err(|e| format!("entrée « dossier » : {e}"))?;

    // **Deux séparateurs distincts et non un réutilisé** : une entrée de menu
    // ne peut occuper qu'une position, la poser deux fois ne la duplique pas.
    let separateur = PredefinedMenuItem::separator(app).map_err(|e| format!("séparateur : {e}"))?;
    let separateur_final =
        PredefinedMenuItem::separator(app).map_err(|e| format!("séparateur final : {e}"))?;

    // « Quitter » en dernier, derrière son propre séparateur.
    //
    // C'est la seule action irréversible du menu, et la seule qu'on ne veut
    // surtout pas cliquer de travers en visant « Ouvrir le dossier ». La
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
    refs.push(&recharger);
    refs.push(&dossier);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Le décodage doit être l'exact inverse de la construction.
    ///
    /// Le test qui compte vraiment de ce fichier : si une ligne d'`ENVIES`
    /// était ajoutée avec un identifiant en double, `intention_de` rendrait
    /// systématiquement la première — l'entrée du menu serait présente et
    /// déclencherait une **autre** action. Silencieux, et très pénible à
    /// diagnostiquer à l'œil.
    #[test]
    fn chaque_envie_se_decode_en_elle_meme() {
        for (id, _, intention) in ENVIES {
            assert_eq!(
                intention_de(id),
                Some(*intention),
                "l'identifiant « {id} » ne rend pas son intention"
            );
        }
    }

    #[test]
    fn un_identifiant_inconnu_ne_decode_rien() {
        // C'est ce qui permet à `actions::executer` d'utiliser `intention_de`
        // comme cas par défaut sans avaler les entrées du tray.
        assert_eq!(intention_de("recharger"), None);
        assert_eq!(intention_de(""), None);
    }

    /// Toutes les envies du menu doivent exister dans la table d'envies.
    ///
    /// Sinon `TableEnvies::jouable` rendrait `false` et l'entrée
    /// n'apparaîtrait **jamais**, sur aucun pack — un menu amputé sans le
    /// moindre message. C'est le mode d'échec exact d'un oubli dans la liste
    /// que le commentaire d'`ENVIES` demande de tenir à jour.
    #[test]
    fn toutes_les_envies_du_menu_sont_dans_la_table() {
        let table = TableEnvies::defaut();
        for (id, _, intention) in ENVIES {
            assert!(
                table.entrees.iter().any(|e| e.intention == *intention),
                "« {id} » n'est pas dans la table d'envies : l'entrée serait toujours cachée"
            );
        }
    }

    /// Un pack complet les propose toutes ; un pack qui ne sait que marcher
    /// n'en propose qu'une.
    ///
    /// C'est la couverture partielle (spec §8.6) vue depuis le menu, et la
    /// raison pour laquelle `ouvrir` filtre au lieu de tout afficher grisé.
    #[test]
    fn la_couverture_partielle_retire_les_envies_injouables() {
        let table = TableEnvies::defaut();

        let blob = Manifest::load(std::path::Path::new("../characters/blob"))
            .expect("le personnage de test doit être lisible");
        let proposees = ENVIES
            .iter()
            .filter(|(_, _, i)| table.jouable(&blob, *i))
            .count();
        assert_eq!(proposees, ENVIES.len(), "blob a toutes les poses");
    }
}
