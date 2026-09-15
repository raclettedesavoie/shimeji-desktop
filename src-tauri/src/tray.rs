//! Le tray et son menu (spec §9.1).
//!
//! Responsabilité unique : construire le menu du tray. Ce qu'une entrée
//! **fait** est dans `actions.rs`, partagé avec le menu du clic droit sur le
//! personnage (`menu_perso.rs`).
//!
//! Fichier à part plutôt que dans `main.rs` : le menu et sa construction
//! n'ont rien à voir avec l'amorçage, et `main.rs` en fait déjà 400.
//!
//! # ⚠️ C'est ici qu'est installé l'UNIQUE gestionnaire d'événements de menu
//!
//! Tauri livre tout événement de menu à **tous** les gestionnaires, quel que
//! soit le menu d'origine (`tauri-2.11.5`, `src/tray/mod.rs:326`). Un second
//! `on_menu_event` ailleurs exécuterait donc chaque action deux fois — et
//! deux bascules s'annulent. Le menu du personnage n'a volontairement pas de
//! gestionnaire : ses clics arrivent ici. Voir l'en-tête d'`actions.rs`.
//!
//! Signatures vérifiées dans tauri 2.11.5 :
//!   TrayIconBuilder                src/tray/mod.rs:216
//!   MenuItem::with_id              src/menu/normal.rs:48
//!   CheckMenuItem::with_id         src/menu/check.rs:49
//!   Menu::with_items               src/menu/menu.rs:120
//!   PredefinedMenuItem::separator  src/menu/predefined.rs:15
//!   Manager::webview_windows       src/lib.rs:588
//!   AppHandle::exit                src/app.rs:574

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

// Les identifiants vivent dans `actions.rs` avec ceux du menu du personnage :
// c'est là qu'ils sont lus, et les tenir à deux endroits inviterait à en
// ajouter un sans son cas de traitement.
use crate::actions::{ID_AFFICHER, ID_CATALOGUE, ID_DEMARRAGE, ID_QUITTER};

/// Partagé entre le tray et les boucles : les personnages sont-ils visibles ?
///
/// `AtomicBool` et non `Mutex<bool>` : c'est un seul booléen, lu 60 fois par
/// seconde et écrit à la main. Un atomique se lit sans verrou et ne peut pas
/// être empoisonné par le panic d'un autre thread.
///
/// C'est la Tâche 6 du plan qui s'en servira pour **ne rien dessiner quand
/// c'est caché** — le plus gros gain de CPU qui reste. On le pose dès
/// maintenant parce que c'est ici qu'il est écrit.
pub type Visibilite = Arc<AtomicBool>;

pub fn nouvelle_visibilite() -> Visibilite {
    Arc::new(AtomicBool::new(true))
}

/// Montre ou cache toutes les fenêtres de personnages.
///
/// `webview_windows()` rend une table de toutes les fenêtres : on n'a donc
/// pas à tenir une liste de labels en parallèle, ce qui serait une seconde
/// source de vérité à maintenir synchronisée.
///
/// Les erreurs sont ignorées volontairement : une fenêtre déjà fermée n'est
/// pas un problème, et il n'y a rien à faire de plus que continuer avec les
/// autres.
pub fn basculer_visibilite(app: &AppHandle, visible: bool) {
    // ⚠️ **Les fenêtres de PERSONNAGES seulement**, reconnues à leur label.
    //
    // La boucle parcourait auparavant toutes les fenêtres du programme, ce
    // qui était sans conséquence tant qu'il n'y en avait qu'une. Depuis le
    // catalogue il y en a deux sortes, et « Cacher les personnages » faisait
    // aussi disparaître la fenêtre du catalogue — y compris quand c'est
    // depuis elle qu'on venait de désactiver quelqu'un.
    //
    // Le préfixe `pet-` est posé par `label_de` et par la réconciliation du
    // roster : c'est la seule convention de nommage du programme, et elle
    // est vérifiée par `libelle_de_personnage`.
    for (label, win) in app.webview_windows() {
        if !est_un_personnage(&label) {
            continue;
        }
        let _ = if visible { win.show() } else { win.hide() };
    }
}

/// Ce label est-il celui d'une fenêtre de personnage ?
///
/// Une fonction plutôt qu'un `starts_with` recopié à trois endroits : le
/// jour où le préfixe change, il ne doit y avoir qu'un seul endroit à
/// corriger — et surtout un seul endroit à oublier.
pub fn est_un_personnage(label: &str) -> bool {
    label.starts_with("pet-")
}

/// Installe l'icône du tray et son menu.
///
/// `actions` porte tout ce dont les entrées ont besoin pour agir, et sert
/// aussi au menu du personnage. `demarrage_actif` initialise la case à cocher
/// d'après ce que dit vraiment le registre — pas d'après la config, qui
/// pourrait mentir si l'utilisateur a retiré l'entrée à la main.
pub fn installer(
    app: &AppHandle,
    demarrage_actif: bool,
    actions: std::sync::Arc<crate::actions::Actions>,
) -> Result<(), String> {
    // ── Les entrées ─────────────────────────────────────────────────────
    // `with_id` et non `new` : c'est l'identifiant qui reviendra dans
    // l'événement, et le laisser engendrer automatiquement rendrait le
    // `match` du gestionnaire impossible à écrire.
    //
    // Le dernier paramètre est un accélérateur clavier (`Option<&str>`) : on
    // n'en veut aucun — un raccourci global sur un pet serait envahissant.
    // `None::<&str>` et non `None` : rien ne permet d'inférer le type `A`.
    let afficher = CheckMenuItem::with_id(
        app,
        ID_AFFICHER,
        "Afficher les personnages",
        true,
        true, // coché au démarrage : ils sont visibles
        None::<&str>,
    )
    .map_err(|e| format!("entrée « afficher » : {e}"))?;

    let demarrage = CheckMenuItem::with_id(
        app,
        ID_DEMARRAGE,
        "Démarrer avec Windows",
        true,
        demarrage_actif,
        None::<&str>,
    )
    .map_err(|e| format!("entrée « démarrage » : {e}"))?;

    let catalogue = MenuItem::with_id(
        app,
        ID_CATALOGUE,
        "Catalogue de personnages…",
        true,
        None::<&str>,
    )
    .map_err(|e| format!("entrée « catalogue » : {e}"))?;

    let separateur =
        PredefinedMenuItem::separator(app).map_err(|e| format!("séparateur : {e}"))?;

    let quitter = MenuItem::with_id(app, ID_QUITTER, "Quitter", true, None::<&str>)
        .map_err(|e| format!("entrée « quitter » : {e}"))?;

    // `&[&dyn IsMenuItem<R>]` : les entrées n'ont pas le même type concret
    // (`MenuItem`, `CheckMenuItem`, `PredefinedMenuItem`), donc on passe par
    // des références de trait. C'est la raison du `&` devant chacune.
    let menu = Menu::with_items(
        app,
        &[
            &afficher,
            &demarrage,
            &catalogue,
            &separateur,
            &quitter,
        ],
    )
    .map_err(|e| format!("menu : {e}"))?;

    // ── Le gestionnaire d'événements — le SEUL du programme ─────────────
    //
    // `move` : la fermeture doit posséder ce qu'elle utilise, puisqu'elle
    // survit à cette fonction. D'où les clones — un emprunt ne pourrait pas
    // en sortir.
    //
    // Les deux entrées à cocher sont capturées pour pouvoir lire leur état :
    // `is_checked()` dit si l'utilisateur vient de cocher ou de décocher.
    let cases = crate::actions::CasesTray {
        afficher: afficher.clone(),
        demarrage: demarrage.clone(),
    };

    // Les mêmes cases sont confiées à `actions`, pour que le menu du
    // personnage puisse les remettre d'accord après avoir agi.
    actions.enregistrer_cases(cases.clone());

    TrayIconBuilder::new()
        .menu(&menu)
        // L'icône du bundle, celle de `icons/icon.ico`. `Option` parce qu'un
        // projet peut n'en avoir aucune — ici elle est obligatoire pour
        // `tauri-build`, donc elle est toujours là.
        .icon(
            app.default_window_icon()
                .ok_or("aucune icône par défaut")?
                .clone(),
        )
        .tooltip("shimeji-desktop")
        // Reçoit AUSSI les clics du menu du personnage : voir l'avertissement
        // en tête de ce fichier. `executer` répartit sur l'identifiant.
        .on_menu_event(move |app, evenement| {
            crate::actions::executer(&actions, app, evenement.id().as_ref(), &cases);
        })
        .build(app)
        .map_err(|e| format!("tray : {e}"))?;

    Ok(())
}
