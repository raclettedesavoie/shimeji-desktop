//! Le rechargement à chaud des personnages (spec §9.1).
//!
//! Responsabilité unique : porter un manifeste fraîchement lu depuis le
//! thread du tray jusqu'au thread de la boucle 60 Hz.
//!
//! **Cette pièce pèse plus qu'il n'y paraît** : c'est elle qui rend
//! supportable le réglage des animations. Sans elle, chaque ajustement de
//! timing dans `mascot.json` coûte un redémarrage — et le réglage du
//! manifeste sera l'essentiel du travail de finition.
//!
//! # Pourquoi un `Mutex` alors que tout le reste s'en passe
//!
//! Le manifeste est **possédé** par le `Character`, qui vit dans le thread de
//! la boucle. Le clic de tray, lui, arrive sur le thread principal de Tauri.
//! Deux threads, une donnée à transmettre : il faut une synchronisation.
//!
//! `Arc<Mutex<Option<…>>>` est le plus simple qui marche :
//!   · `Arc` — plusieurs propriétaires (le tray et la boucle) ;
//!   · `Mutex` — un seul écrit à la fois ;
//!   · `Option` — la boîte est vide la plupart du temps, et `take()` la vide
//!     en récupérant son contenu, ce qui rend « consommer la demande »
//!     atomique et évite d'avoir à remettre un drapeau à zéro.
//!
//! **Le verrou n'est jamais tenu pendant une entrée-sortie** : c'est le tray
//! qui lit le disque, *puis* pose le résultat. La boucle 60 Hz ne fait que
//! `try_lock` et `take` — jamais de blocage à 60 Hz sur un accès disque.

use crate::character::manifest::Manifest;
use crate::config::Reglages;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Ce qu'un rechargement apporte.
pub struct Rechargement {
    pub manifeste: Manifest,
    pub reglages: Reglages,

    /// La table d'envies, reconstruite depuis la config relue.
    pub table: crate::behavior::desire::TableEnvies,

    /// Le réglage `echelle`, à recombiner avec celui du moniteur.
    pub echelle_config: f32,

    /// Numéro de version, incrémenté à chaque rechargement.
    ///
    /// Sert **uniquement** à contourner le cache du webview : les images sont
    /// servies avec `Cache-Control: max-age=3600`, donc une image modifiée
    /// sur le disque ne serait pas relue. On change l'URL plutôt que le
    /// cache — `?v=3` au lieu de `?v=2` — ce qui est le contournement usuel
    /// et ne demande **aucun changement côté serveur** : le gestionnaire du
    /// schéma URI lit `uri().path()`, qui ignore la requête.
    pub version: u64,
}

/// La boîte partagée entre le tray et la boucle.
pub type Demande = Arc<Mutex<Option<Rechargement>>>;

pub fn nouvelle_demande() -> Demande {
    Arc::new(Mutex::new(None))
}

/// Relit le manifeste et la config depuis le disque, et dépose le résultat.
///
/// Appelée depuis le thread du tray. Rend la nouvelle version, ou l'erreur —
/// que l'appelant affiche, parce qu'un rechargement silencieusement raté est
/// le pire des cas : on croit tester son nouveau timing et on regarde
/// l'ancien.
pub fn preparer(demande: &Demande, dossier: &Path, personnage: &str) -> Result<u64, String> {
    // ── Les entrées-sorties D'ABORD, verrou non tenu ────────────────────
    // Si le manifeste est illisible on sort ici, **sans avoir rien touché** :
    // le personnage continue avec ce qu'il avait. C'est le point le plus
    // important de ce fichier — on va éditer ce JSON des dizaines de fois.
    let manifeste = Manifest::load(&dossier.join(personnage))
        .map_err(|e| format!("manifeste illisible, rien n'a changé : {e}"))?;

    let config = crate::config::charger();
    let reglages = Reglages::depuis(&config);
    let table = crate::behavior::desire::TableEnvies::depuis_config(&config);

    // ── Puis on pose, brièvement ────────────────────────────────────────
    let mut boite = demande
        .lock()
        // `PoisonError` : un autre thread a paniqué en tenant le verrou. Ça
        // ne peut arriver que si la boucle a paniqué, auquel cas le
        // rechargement est le moindre des soucis.
        .map_err(|_| "verrou de rechargement empoisonné".to_string())?;

    // La version repart de celle en attente s'il y en avait une, pour que
    // deux clics rapprochés ne rendent pas le même numéro.
    let version = boite.as_ref().map(|r| r.version).unwrap_or(0) + 1;

    *boite = Some(Rechargement {
        manifeste,
        reglages,
        table,
        echelle_config: config.echelle,
        version,
    });

    Ok(version)
}
