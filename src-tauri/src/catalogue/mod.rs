//! Le catalogue de personnages : parcourir, installer (spec §4).
//!
//! Responsabilité unique : transformer un slug du catalogue shimejis.xyz en
//! un dossier de personnage utilisable par le moteur. Ne sait rien de
//! l'affichage — la fenêtre du catalogue, elle, est en HTML.

pub mod index;
pub mod installation;
pub mod reseau;
