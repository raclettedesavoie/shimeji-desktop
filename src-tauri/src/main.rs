// Amorçage de l'application.
//
// Deux modes : l'application (fenêtre, boucle 60 Hz) et le mode simulation
// (`--sim`, sans écran, spec §10.3). Une sous-commande et non un second
// binaire : deux cibles binaires d'un même paquet ne partagent du code que
// par une cible `lib`, et ajouter un `lib.rs` pour cela seul réorganiserait
// tout le projet.
//
// Pas de `#![windows_subsystem = "windows"]` pour le moment : on VEUT la
// console pendant le développement (topologie des écrans, trace du
// comportement). Le plan 1b la supprimera, une fois le tray disponible pour
// quitter proprement.

mod behavior;
mod character;
mod clock;
mod geom;
mod probe;
mod rng;
mod sim;
mod world;

fn main() {
    // AVANT TOUT LE RESTE. Sans cet appel, Windows virtualise les
    // coordonnées et la sonde rendrait des pixels logiques en croyant rendre
    // des pixels physiques (spec §3.4). Voir le commentaire de la fonction.
    probe::win32::activer_conscience_dpi();

    // Aiguillage minimal, écrit à la main : une crate d'analyse d'arguments
    // pour deux drapeaux serait disproportionnée, et le plan 1b n'en ajoutera
    // pas d'autres (les réglages iront dans config.json).
    let args: Vec<String> = std::env::args().collect();

    if let Some(i) = args.iter().position(|a| a == "--sim") {
        // `get(i + 1)` puis `parse` : une valeur absente ou illisible vaut
        // 30 minutes plutôt qu'une erreur — c'est un outil de développement.
        let minutes = args
            .get(i + 1)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(30);

        let graine = args
            .iter()
            .position(|a| a == "--graine")
            .and_then(|j| args.get(j + 1))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(42);

        let dossier = dossier_personnages().join("blob");
        println!("simulation de {minutes} min, graine {graine}, personnage {}", dossier.display());

        match sim::executer(minutes, graine, &dossier) {
            Ok(r) => sim::imprimer(&r),
            Err(e) => {
                eprintln!("simulation impossible : {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    lancer_application();
}

/// Où sont les personnages.
///
/// Ordre de recherche (spec §8.1) : à côté de l'exe, puis `%APPDATA%`.
///
/// **Plus un repli de développement** au milieu : en `cargo run`, l'exe est
/// dans `src-tauri/target/debug/`, donc « à côté de l'exe » ne trouve rien et
/// l'on tomberait sur `%APPDATA%`, vide. On remonte donc les dossiers parents
/// à la recherche d'un `characters/`. Ce repli disparaîtra si un jour il
/// gêne ; pour l'instant il évite de copier 46 PNG à chaque build.
///
/// Le plan 1b déplacera cette fonction dans `config.rs`, qui résout de la
/// même façon `config.json`.
fn dossier_personnages() -> std::path::PathBuf {
    // ── À côté de l'exe ────────────────────────────────────────────────
    // `if let Ok(...)` : `current_exe` peut échouer sur des systèmes
    // exotiques. Ce n'est pas une raison de ne pas démarrer.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidat = dir.join("characters");
            if candidat.is_dir() {
                return candidat;
            }

            // ── Repli de développement ─────────────────────────────────
            // `ancestors()` énumère le dossier puis chacun de ses parents.
            // On s'arrête à 5 niveaux : assez pour sortir de target/debug/,
            // pas assez pour partir explorer tout le disque.
            for parent in dir.ancestors().take(5) {
                let candidat = parent.join("characters");
                if candidat.is_dir() {
                    return candidat;
                }
            }
        }
    }

    // ── %APPDATA% ──────────────────────────────────────────────────────
    if let Ok(appdata) = std::env::var("APPDATA") {
        return std::path::PathBuf::from(appdata)
            .join("shimeji-desktop")
            .join("characters");
    }

    // Dernier recours : le dossier courant. Ça échouera au chargement du
    // manifeste, avec un message qui nomme le chemin cherché — ce qui est
    // exactement ce qu'il faut pour diagnostiquer.
    std::path::PathBuf::from("characters")
}

fn lancer_application() {
    let sonde = probe::win32::Win32Probe::new();
    probe::win32::imprimer_diagnostic(&sonde);
    println!("personnages : {}", dossier_personnages().display());
    println!("La fenêtre arrive en Tâche 10. En attendant : cargo run -- --sim 30");
}
