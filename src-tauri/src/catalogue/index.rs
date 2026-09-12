//! L'index du catalogue : la liste des packs disponibles (spec §7).
//!
//! Responsabilité unique : lire `catalogue.json`. Ne télécharge rien et ne
//! sait rien des URL — le slug suffit, l'URL s'en déduit dans `mod.rs`.
