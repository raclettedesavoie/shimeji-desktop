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

    /// Le personnage rechargé.
    ///
    /// Transporté jusqu'au webview parce qu'il peut avoir CHANGÉ : la fenêtre
    /// du catalogue en choisit un autre, et `pet.js` doit alors refaire sa
    /// base d'URL. Sans ce nom, il réclamerait encore les images de l'ancien
    /// — un personnage parfaitement animé avec le mauvais dessin.
    pub personnage: String,

    /// La table d'envies, reconstruite depuis la config relue.
    pub table: crate::behavior::desire::TableEnvies,

    /// Le réglage `echelle`, à recombiner avec celui du moniteur.
    pub echelle_config: f32,

    /// La config complète, pour `signals::biais_de` — la table des
    /// applications en fait partie.
    ///
    /// Redondante avec `reglages` et `table`, qui en sont dérivés. On la
    /// transporte quand même plutôt que de reconstruire : `preparer` l'a déjà
    /// lue, et la relire dans la boucle serait une entrée-sortie à 8 Hz.
    pub config: crate::config::Config,

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
/// `dossier_perso` est le dossier **du personnage** et non son parent : il est
/// désormais résolu par `config::dossier_du_personnage`, qui consulte la
/// bibliothèque puis le dossier livré. Recevoir le chemin déjà résolu évite
/// que cette fonction ait à connaître cette règle — et lui permet de charger
/// un personnage de la bibliothèque comme un autre, sans le savoir.
/// Le nom d'un personnage, c'est le nom de son dossier.
///
/// `file_name` rend une `Option` (un chemin peut finir par `..`) et un
/// `OsStr` (Windows tolère des noms qui ne sont pas de l'UTF-8 valide) :
/// d'où les deux conversions en cascade. Le repli sur `blob` n'est atteint
/// que par un chemin absurde, et vaut mieux qu'un `unwrap`.
fn nom_du_dossier(dossier: &Path) -> String {
    dossier
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("blob")
        .to_string()
}

pub fn preparer(demande: &Demande, dossier_perso: &Path) -> Result<u64, String> {
    // ── Les entrées-sorties D'ABORD, verrou non tenu ────────────────────
    // Si le manifeste est illisible on sort ici, **sans avoir rien touché** :
    // le personnage continue avec ce qu'il avait. C'est le point le plus
    // important de ce fichier — on va éditer ce JSON des dizaines de fois.
    let manifeste = Manifest::load(dossier_perso)
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

    // ⚠️ Un compteur MONOTONE, et non la version de la demande en attente.
    //
    // La version se déduisait de `boite.as_ref()`, ce qui paraissait suffire
    // : « deux clics rapprochés ne rendent pas le même numéro ». Mais la
    // boucle 60 Hz vide la boîte par `take()` — la demande suivante repartait
    // donc de `0 + 1`, et **tout rechargement consommé rendait 1**.
    //
    // Sans conséquence tant qu'un seul personnage existait : la version ne
    // servait qu'à contourner le cache d'images du webview, et le contenu
    // rechargé était le même personnage. Depuis le catalogue, `pet.js` indexe
    // ce cache par `version + '/' + frame` : deux rechargements de même
    // version font de chaque frame **déjà vue** un succès de cache, qui rend
    // l'URL de l'ANCIEN personnage. À l'écran, le personnage changeait pour
    // les poses neuves et gardait l'ancien dessin pour les poses connues —
    // d'où « il repasse sur blob dès qu'il fait autre chose que flâner ».
    //
    // `Relaxed` : on ne demande qu'une chose à cet atomique, que deux appels
    // ne rendent jamais le même nombre. Aucun autre accès mémoire n'a besoin
    // d'être ordonné par rapport à lui, donc l'ordonnancement le moins cher
    // convient.
    static COMPTEUR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let version = COMPTEUR.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

    *boite = Some(Rechargement {
        manifeste,
        reglages,
        personnage: nom_du_dossier(dossier_perso),
        table,
        echelle_config: config.echelle,
        config,
        version,
    });

    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deux rechargements CONSOMMÉS doivent porter deux versions distinctes.
    ///
    /// C'est le défaut du 2026-09-15, et il ne se voyait qu'à l'écran. La
    /// version ne servait qu'à contourner le cache du webview tant qu'un seul
    /// personnage existait ; depuis le catalogue, `pet.js` indexe ce cache par
    /// `version + '/' + frame`. Deux rechargements qui rendent le MÊME numéro
    /// font donc de chaque frame déjà vue un succès de cache — qui sert l'URL
    /// de l'**ancien** personnage. Résultat à l'écran : le personnage change
    /// pour les poses neuves et reste l'ancien pour les poses connues.
    ///
    /// Le piège tenait à ce que la version se déduisait de la demande EN
    /// ATTENTE. La boucle 60 Hz la vide par `take()` : la suivante repartait
    /// donc systématiquement de 0 + 1.
    #[test]
    fn deux_rechargements_consommes_ne_partagent_pas_leur_version() {
        let dossier = crate::config::dossier_du_personnage("blob")
            .expect("le personnage de référence doit exister");

        let demande: Demande = nouvelle_demande();

        let v1 = preparer(&demande, &dossier).expect("premier rechargement");
        // La boucle consomme la demande — c'est exactement ce que fait
        // `boite.take()` à 8 Hz.
        demande.lock().unwrap().take();

        let v2 = preparer(&demande, &dossier).expect("second rechargement");

        assert_ne!(
            v1, v2,
            "deux rechargements consommés ont rendu la même version : \
             le cache du webview servirait l'ancien personnage"
        );
        assert!(v2 > v1, "les versions doivent croître ({v1} puis {v2})");
    }

    /// Et deux demandes NON consommées gardent la propriété d'origine.
    #[test]
    fn deux_rechargements_rapproches_croissent_aussi() {
        let dossier = crate::config::dossier_du_personnage("blob")
            .expect("le personnage de référence doit exister");

        let demande: Demande = nouvelle_demande();
        let v1 = preparer(&demande, &dossier).expect("premier");
        // Sans `take()` : deux clics plus rapides que la boucle.
        let v2 = preparer(&demande, &dossier).expect("second");

        assert!(v2 > v1, "deux clics rapprochés doivent différer");
    }
}
