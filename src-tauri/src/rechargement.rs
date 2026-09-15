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
use std::sync::{Arc, Mutex};

/// Un pack chargé, prêt à être instancié autant de fois qu'il le faut.
///
/// `Manifest` dérive déjà `Clone` : chaque acteur possède sa copie. Quelques
/// Ko par personnage, ce qui ne justifie pas d'introduire un `Arc` et
/// l'emprunt partagé qui va avec (design §4).
pub struct PersonnageCharge {
    pub nom: String,
    pub manifeste: Manifest,
}

/// Ce qu'un rechargement apporte.
pub struct Rechargement {
    /// Les packs chargés, **un par nom distinct** du roster voulu.
    ///
    /// Lus par le thread de la commande, jamais par la boucle : c'est ce qui
    /// garantit qu'il n'y a aucune entrée-sortie à 60 Hz.
    pub personnages: Vec<PersonnageCharge>,

    /// Le roster voulu, **avec ses doublons** : `["blob", "blob", "luffy"]`
    /// veut dire deux blob et un luffy (design §4).
    ///
    /// Distinct de `personnages`, qui dédoublonne : celui-ci dit COMBIEN,
    /// celui-là dit QUOI charger.
    pub voulus: Vec<String>,

    /// Les retraits doivent-ils sauter l'animation de départ ?
    ///
    /// `true` uniquement pour la suppression d'un pack du disque : effacer
    /// les PNG pendant qu'une fenêtre les réclame encore par `shime://`
    /// donnerait un personnage à moitié dessiné en pleine chute (design §7).
    ///
    /// **Un booléen, pas un second chemin de réconciliation** — deux chemins
    /// divergeraient à la première correction.
    pub sans_animation: bool,

    pub reglages: Reglages,

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

/// Lit ce qu'il faut pour afficher exactement `voulus`, et le dépose.
///
/// ⚠️ **Les entrées-sorties D'ABORD, verrou non tenu.** Un manifeste
/// illisible fait sortir ici sans rien avoir touché, et les personnages
/// continuent avec ce qu'ils avaient. C'est le point le plus important de ce
/// fichier — on va éditer ces JSON des dizaines de fois. Tenir le verrou
/// pendant une lecture de fichier bloquerait en plus la boucle 60 Hz.
///
/// Un nom **introuvable ou illisible** est signalé bruyamment et **ignoré** :
/// les autres personnages doivent vivre. Échouer ici sur un seul pack effacé
/// à la main rendrait toute la bibliothèque inutilisable.
///
/// Une liste entièrement vide est un **succès** — zéro personnage est un
/// état normal (design §4).
pub fn preparer_roster(
    demande: &Demande,
    voulus: &[String],
    sans_animation: bool,
) -> Result<u64, String> {
    // ── Les entrées-sorties D'ABORD, verrou non tenu ────────────────────
    //
    // Un `BTreeSet` : on ne lit le manifeste d'un pack QU'UNE FOIS, même si
    // trois exemplaires en sont voulus — et l'ordre est stable d'un appel à
    // l'autre, contrairement à un `HashSet`.
    let distincts: std::collections::BTreeSet<&String> = voulus.iter().collect();

    let mut personnages = Vec::new();
    for nom in distincts {
        let Some(dossier) = crate::config::dossier_du_personnage(nom) else {
            eprintln!("personnage « {nom} » introuvable : ignoré");
            continue;
        };
        match Manifest::load(&dossier) {
            Ok(manifeste) => personnages.push(PersonnageCharge {
                nom: nom.clone(),
                manifeste,
            }),
            Err(e) => eprintln!("personnage « {nom} » illisible, ignoré : {e}"),
        }
    }

    // Ne garder que les noms réellement chargés. Sans ce filtre, la
    // réconciliation redemanderait la création d'un personnage qu'elle ne
    // peut pas créer — à chaque passage à 8 Hz, indéfiniment.
    let voulus: Vec<String> = voulus
        .iter()
        .filter(|n| personnages.iter().any(|p| &p.nom == *n))
        .cloned()
        .collect();

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
    // les poses neuves et gardait l'ancien dessin pour les poses connues.
    //
    // `Relaxed` : on ne demande qu'une chose à cet atomique, que deux appels
    // ne rendent jamais le même nombre.
    static COMPTEUR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let version = COMPTEUR.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;

    *boite = Some(Rechargement {
        personnages,
        voulus,
        sans_animation,
        reglages,
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
        let demande: Demande = nouvelle_demande();
        let roster = vec!["blob".to_string()];

        let v1 = preparer_roster(&demande, &roster, false).expect("premier rechargement");
        // La boucle consomme la demande — c'est exactement ce que fait
        // `boite.take()` à 8 Hz.
        demande.lock().unwrap().take();

        let v2 = preparer_roster(&demande, &roster, false).expect("second rechargement");

        assert_ne!(
            v1, v2,
            "deux rechargements consommés ont rendu la même version :              le cache du webview servirait l'ancien personnage"
        );
        assert!(v2 > v1, "les versions doivent croître ({v1} puis {v2})");
    }

    /// Et deux demandes NON consommées gardent la propriété d'origine.
    #[test]
    fn deux_rechargements_rapproches_croissent_aussi() {
        let demande: Demande = nouvelle_demande();
        let roster = vec!["blob".to_string()];

        let v1 = preparer_roster(&demande, &roster, false).expect("premier");
        let v2 = preparer_roster(&demande, &roster, false).expect("second");
        assert!(v2 > v1);
    }

    /// Un pack demandé trois fois n'est LU qu'une fois.
    ///
    /// C'est ce qui rend les doublons gratuits : trois blob à l'écran, c'est
    /// un seul `mascot.json` lu et trois `Manifest::clone`.
    #[test]
    fn un_pack_en_triple_n_est_charge_qu_une_fois() {
        let demande: Demande = nouvelle_demande();
        let roster = vec!["blob".to_string(), "blob".to_string(), "blob".to_string()];

        preparer_roster(&demande, &roster, false).expect("rechargement");

        let boite = demande.lock().unwrap();
        let r = boite.as_ref().expect("une demande déposée");
        assert_eq!(r.personnages.len(), 1, "un seul manifeste lu");
        assert_eq!(r.voulus.len(), 3, "mais trois exemplaires voulus");
    }

    /// Un nom introuvable est ignoré, et les autres vivent.
    ///
    /// Échouer sur un seul pack effacé à la main rendrait toute la
    /// bibliothèque inutilisable.
    #[test]
    fn un_nom_introuvable_est_ignore_et_les_autres_vivent() {
        let demande: Demande = nouvelle_demande();
        let roster = vec![
            "blob".to_string(),
            "ce-pack-n-existe-pas-du-tout".to_string(),
        ];

        preparer_roster(&demande, &roster, false).expect("rechargement");

        let boite = demande.lock().unwrap();
        let r = boite.as_ref().expect("une demande déposée");
        assert_eq!(r.personnages.len(), 1);
        assert_eq!(r.personnages[0].nom, "blob");
        // Le nom mort est retiré du roster voulu : sans ce filtre, la
        // réconciliation redemanderait sa création à chaque passage à 8 Hz,
        // indéfiniment.
        assert_eq!(r.voulus, vec!["blob".to_string()]);
    }

    /// Une liste vide est un SUCCÈS, pas une erreur.
    #[test]
    fn un_roster_vide_est_un_succes() {
        let demande: Demande = nouvelle_demande();
        preparer_roster(&demande, &[], false).expect("un roster vide est valide");

        let boite = demande.lock().unwrap();
        let r = boite.as_ref().expect("une demande déposée");
        assert!(r.personnages.is_empty());
        assert!(r.voulus.is_empty());
    }
}
