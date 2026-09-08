// Amorçage de l'application. Pour l'instant : il ne fait que déclarer les
// modules, afin que `cargo test` les compile et exécute leurs tests.
// La fenêtre et la boucle 60 Hz arrivent en Tâche 10.
//
// Pas de `#![windows_subsystem = "windows"]` pour le moment : on VEUT la
// console pendant le développement (topologie des écrans, trace du
// comportement). Le plan 1b la supprimera, une fois le tray disponible pour
// quitter proprement.

mod clock;
mod geom;
mod rng;

fn main() {
    println!("shimeji-desktop — squelette. Fenêtre et boucle : Tâche 10.");
}
