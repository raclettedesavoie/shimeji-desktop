//! Le tray et son menu (spec §9.1).
//!
//! Responsabilité unique : construire le menu, et traduire un clic en action.
//! **Aucune logique de personnage ici** — les actions se contentent
//! d'appeler ailleurs.
//!
//! Fichier à part plutôt que dans `main.rs` : le menu, ses identifiants et
//! ses actions font une centaine de lignes qui n'ont rien à voir avec
//! l'amorçage, et `main.rs` en fait déjà 400.
//!
//! Signatures vérifiées dans tauri 2.11.5 :
//!   TrayIconBuilder                src/tray/mod.rs:216
//!   MenuItem::with_id              src/menu/normal.rs:48
//!   CheckMenuItem::with_id         src/menu/check.rs:49
//!   Menu::with_items               src/menu/menu.rs:120
//!   PredefinedMenuItem::separator  src/menu/predefined.rs:15
//!   Manager::webview_windows       src/lib.rs:588
//!   AppHandle::exit                src/app.rs:574

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

// Les identifiants des entrées. Des constantes plutôt que des littéraux :
// l'identifiant est écrit à la construction du menu ET lu dans le
// gestionnaire d'événements, donc une faute de frappe donnerait une entrée
// qui ne fait silencieusement rien.
pub const ID_AFFICHER: &str = "afficher";
pub const ID_RECHARGER: &str = "recharger";
pub const ID_DEMARRAGE: &str = "demarrage";
pub const ID_DOSSIER: &str = "dossier";
pub const ID_QUITTER: &str = "quitter";

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
    for (_label, win) in app.webview_windows() {
        let _ = if visible { win.show() } else { win.hide() };
    }
}

/// Installe l'icône du tray et son menu.
///
/// `dossier_personnages` est capturé par la fermeture du menu, pour l'entrée
/// « ouvrir le dossier ». `demarrage_actif` initialise la case à cocher
/// d'après ce que dit vraiment le registre — pas d'après la config, qui
/// pourrait mentir si l'utilisateur a retiré l'entrée à la main.
pub fn installer(
    app: &AppHandle,
    dossier_personnages: &Path,
    demarrage_actif: bool,
    visibilite: Visibilite,
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

    let recharger = MenuItem::with_id(
        app,
        ID_RECHARGER,
        "Recharger les personnages",
        true,
        None::<&str>,
    )
    .map_err(|e| format!("entrée « recharger » : {e}"))?;

    let demarrage = CheckMenuItem::with_id(
        app,
        ID_DEMARRAGE,
        "Démarrer avec Windows",
        true,
        demarrage_actif,
        None::<&str>,
    )
    .map_err(|e| format!("entrée « démarrage » : {e}"))?;

    let dossier = MenuItem::with_id(
        app,
        ID_DOSSIER,
        "Ouvrir le dossier des personnages",
        true,
        None::<&str>,
    )
    .map_err(|e| format!("entrée « dossier » : {e}"))?;

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
            &recharger,
            &demarrage,
            &dossier,
            &separateur,
            &quitter,
        ],
    )
    .map_err(|e| format!("menu : {e}"))?;

    // ── Le gestionnaire d'événements ────────────────────────────────────
    // `move` : la fermeture doit posséder ce qu'elle utilise, puisqu'elle
    // survit à cette fonction. D'où le `to_path_buf` — on ne peut pas
    // capturer un `&Path` emprunté.
    let dossier_a_ouvrir = dossier_personnages.to_path_buf();

    // Les deux entrées à cocher sont capturées pour pouvoir lire leur état :
    // `is_checked()` dit si l'utilisateur vient de cocher ou de décocher.
    let afficher_pour_evenement = afficher.clone();
    let demarrage_pour_evenement = demarrage.clone();

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
        .on_menu_event(move |app, evenement| {
            match evenement.id().as_ref() {
                ID_AFFICHER => {
                    // `is_checked` rend l'état APRÈS le clic : c'est
                    // directement la visibilité voulue.
                    let visible = afficher_pour_evenement.is_checked().unwrap_or(true);

                    // L'ordre compte : on prévient d'abord la boucle, pour
                    // qu'elle arrête de dessiner, puis on cache. L'inverse
                    // laisserait une image poussée à une fenêtre déjà
                    // masquée — inoffensif, mais gratuit.
                    visibilite.store(visible, Ordering::Relaxed);
                    basculer_visibilite(app, visible);
                }

                ID_RECHARGER => {
                    // Tâche 5. On le dit plutôt que de ne rien faire : une
                    // entrée de menu muette se diagnostique mal.
                    println!("rechargement : Tâche 5 du plan 1b");
                }

                ID_DEMARRAGE => {
                    // Tâche 4.
                    let _voulu = demarrage_pour_evenement.is_checked().unwrap_or(false);
                    println!("démarrage automatique : Tâche 4 du plan 1b");
                }

                ID_DOSSIER => {
                    // `explorer` plutôt qu'un plugin Tauri : c'est une ligne,
                    // ça n'ajoute aucune dépendance, et l'échec (dossier
                    // absent) n'a pas de conséquence.
                    let _ = std::process::Command::new("explorer")
                        .arg(&dossier_a_ouvrir)
                        .spawn();
                }

                ID_QUITTER => {
                    // `exit` termine le processus. Les threads des boucles
                    // 60 Hz meurent avec lui — ils ne détiennent aucune
                    // ressource à libérer proprement, seulement un
                    // `AppHandle`.
                    app.exit(0);
                }

                // Un identifiant inconnu ne peut venir que d'une entrée
                // ajoutée sans son cas ici. On le signale plutôt que de
                // l'ignorer.
                autre => eprintln!("entrée de tray non gérée : {autre}"),
            }
        })
        .build(app)
        .map_err(|e| format!("tray : {e}"))?;

    Ok(())
}
