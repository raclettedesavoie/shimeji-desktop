// Amorçage de l'application. Pour l'instant : il ne fait que déclarer les
// modules, afin que `cargo test` les compile et exécute leurs tests.
// La fenêtre et la boucle 60 Hz arrivent en Tâche 10.
//
// Pas de `#![windows_subsystem = "windows"]` pour le moment : on VEUT la
// console pendant le développement (topologie des écrans, trace du
// comportement). Le plan 1b la supprimera, une fois le tray disponible pour
// quitter proprement.

mod character;
mod clock;
mod geom;
mod probe;
mod rng;
mod world;

fn main() {
    // AVANT TOUT LE RESTE. Sans cet appel, Windows virtualise les
    // coordonnées et la sonde rendrait des pixels logiques en croyant rendre
    // des pixels physiques (spec §3.4). Voir le commentaire de la fonction.
    probe::win32::activer_conscience_dpi();

    let sonde = probe::win32::Win32Probe::new();
    probe::win32::imprimer_diagnostic(&sonde);
    println!("Monde, personnage et fenêtre : Tâches 3 à 11.");
}
