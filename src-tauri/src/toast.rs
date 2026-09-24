//! Les notifications Windows de l'application — toutes de maintenance, jamais
//! du personnage (voir « Hors sujet » dans CLAUDE.md).
//!
//! Responsabilité unique : émettre **une** notification Windows, sans jamais
//! faire échouer ce qui l'appelle.

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Remet la notification au plugin. Rend `false` si le plugin l'a refusée
/// d'emblée, `true` sinon.
///
/// # ⚠️ `true` ne veut PAS dire « affichée » — ni même « acceptée »
///
/// Lu dans `tauri-plugin-notification` 2.4.0 (`src/desktop.rs:216`) :
/// `show()` lance l'affichage dans une tâche à part et **jette son
/// résultat** (`let _ = notification.show()`), puis rend `Ok` aussitôt.
/// Nous ne pouvons donc pas savoir si Windows l'a acceptée, et encore moins
/// si elle a été vue — le mode « Ne pas déranger » la range dans le centre
/// de notifications sans bannière (constaté le 2026-09-24, où l'on a cru à
/// un défaut). L'ancienne note « mesuré le 2026-09-20 : il réussit » reposait
/// sur ce `Ok`, et ne prouvait rien.
///
/// # L'AppUserModelID
///
/// En debug (exe sous `target\debug\`), le plugin n'en pose aucun, et
/// `notify-rust` emprunte celui de PowerShell. Installée, l'application
/// utilise son `identifier` (`dev.local.shimeji-desktop`), que porte le
/// raccourci du menu Démarrer posé par NSIS — vérifié le 2026-09-24.
///
/// `SHIMEJI_TOAST=1` trace la remise au plugin.
fn emettre(app: &AppHandle, titre: &str, corps: &str) -> bool {
    // En « Ne pas déranger », Windows range le toast sans bannière : on
    // montre la notification maison à la place (demande de l'auteur,
    // 2026-09-24). Si elle ne peut pas s'afficher, on retombe sur le toast.
    if crate::ne_pas_deranger::actif() && crate::notif_maison::afficher(app, titre, corps) {
        if std::env::var("SHIMEJI_TOAST").is_ok() {
            println!("[toast] Ne pas déranger : notification maison — {titre}");
        }
        return true;
    }

    let resultat = app.notification().builder().title(titre).body(corps).show();
    let trace = std::env::var("SHIMEJI_TOAST").is_ok();
    match resultat {
        Ok(()) => {
            if trace {
                println!("[toast] remis au plugin : {titre}");
            }
            true
        }
        Err(e) => {
            // Toujours imprimé, même sans la variable : un toast qui n'arrive
            // pas est une information, pas un incident.
            eprintln!("[toast] refusé par le plugin : {titre} — {e}");
            false
        }
    }
}

/// Annonce que l'application continue en arrière-plan (fin de la première
/// configuration, spec §4).
pub fn arriere_plan(app: &AppHandle) {
    emettre(
        app,
        "🐱 Shimeji Desktop est actif",
        "L'application continue de fonctionner en arrière-plan.\n\
         Vous pouvez la contrôler depuis l'icône dans la zone de notification.",
    );
}

/// Un toast de TEST, émis par l'entrée « Tester une notification » du tray —
/// présente en build debug seulement (demande de l'auteur, 2026-09-24).
/// S'il n'apparaît pas, regarder d'abord le mode « Ne pas déranger ».
#[cfg(debug_assertions)]
pub fn test(app: &AppHandle) {
    emettre(
        app,
        "Shimeji Desktop — notification de test",
        "Si vous lisez ceci, les notifications de Shimeji Desktop s'affichent.",
    );
    println!(
        "[toast] test émis — Ne pas déranger : {}",
        if crate::ne_pas_deranger::actif() { "actif, notification maison" } else { "inactif, toast Windows" }
    );
}

/// Annonce qu'une nouvelle version est disponible (vérification au
/// démarrage).
///
/// ⚠️ **Les toasts de mise à jour s'écartent d'une décision du projet** —
/// « il ne notifie rien, ne rappelle rien ». Ils ont été demandés
/// explicitement, et se défendent : ils relèvent de la maintenance de
/// l'application, pas du comportement du personnage.
///
/// **Aucun bouton, et c'est délibéré.** Cliquer le toast ne déclenche rien :
/// une installation lancée par un clic distrait fermerait l'application au
/// milieu d'une session. C'est l'entrée du menu du tray qui agit.
///
/// L'appelant est responsable de ne l'émettre **qu'une fois par version**
/// (voir `config::derniere_version_signalee`). Rend `false` si le plugin l'a
/// refusée, pour qu'elle soit retentée au prochain lancement.
pub fn maj_disponible(app: &AppHandle, version: &str) -> bool {
    emettre(
        app,
        &format!("Shimeji Desktop v{version} est disponible"),
        "Ouvrez le menu de l'icône dans la zone de notification pour l'installer.",
    )
}

/// « Vérifier les mises à jour » au clic : rien de plus récent (demande de
/// l'auteur, 2026-09-24). Seulement sur un clic — la vérification du
/// démarrage, elle, reste muette quand on est à jour.
pub fn a_jour(app: &AppHandle, version: &str) {
    emettre(
        app,
        &format!("Shimeji Desktop est à jour (v{version})"),
        "Aucune version plus récente n'est disponible.",
    );
}

/// « Vérifier les mises à jour » au clic : la vérification ou l'installation
/// a échoué (demande de l'auteur, 2026-09-24 — le libellé du tray seul ne se
/// voyait qu'en rouvrant le menu). Seulement sur un clic : au démarrage,
/// être hors ligne reste un état normal, et muet.
pub fn maj_impossible(app: &AppHandle, raison: &str) {
    emettre(
        app,
        "Impossible de vérifier les mises à jour",
        &format!("Réessayez depuis le menu de l'icône. ({raison})"),
    );
}

/// Le téléchargement commence : quelques secondes où rien d'autre ne se
/// voit, avant que l'installateur ferme l'application.
pub fn installation_en_cours(app: &AppHandle, version: &str) {
    emettre(
        app,
        &format!("Mise à jour vers la v{version}…"),
        "Shimeji Desktop va se fermer quelques instants, puis revenir.",
    );
}

/// En debug, un clic sur « Mettre à jour » trouve une version mais
/// n'installe rien (voir `maj::verifier_puis_installer`) : on le dit.
/// `cfg_attr` : en release, rien ne l'appelle.
#[cfg_attr(not(debug_assertions), allow(dead_code))]
pub fn installation_desactivee_en_debug(app: &AppHandle, version: &str) {
    emettre(
        app,
        &format!("v{version} disponible — rien n'est installé en debug"),
        "Depuis cargo run, l'installation est désactivée : elle écraserait la version installée.",
    );
}

/// Au premier lancement après une mise à jour (`maj::constater_au_demarrage`,
/// en release seulement — d'où le `allow` en debug).
#[cfg_attr(debug_assertions, allow(dead_code))]
pub fn mis_a_jour(app: &AppHandle, version: &str) {
    emettre(
        app,
        &format!("Shimeji Desktop a été mis à jour en v{version}"),
        "Vos personnages sont de retour.",
    );
}
