//! Mode simulation : dérouler le comportement sans écran (spec §10.3).
//!
//! Responsabilité unique : faire tourner `behavior::pas` contre un monde
//! factice à graine fixe, et résumer ce qui s'est passé.
//!
//! **Ce que ni les tests unitaires ni l'œil ne couvrent** : « il a flâné, il
//! s'est reposé, il n'est jamais resté bloqué plus de 20 s », sur des heures
//! de temps simulé, en quelques secondes de calcul, et de façon
//! reproductible. C'est ce qui rend les **régressions de comportement**
//! détectables — un réglage de poids qui rendrait le personnage catatonique
//! se verrait ici avant d'être livré.
//!
//! Il appelle **exactement la même `behavior::pas`** que la boucle 60 Hz :
//! il n'y a pas deux comportements à maintenir, et c'est ce qui rend la
//! simulation représentative.

use crate::behavior::{self, desire::TableEnvies, Entrees};
use crate::character::attach::Attachment;
use crate::character::manifest::Manifest;
use crate::character::Character;
use crate::geom::{Face, Point};
use crate::probe::fake::FakeProbe;
use crate::probe::SystemProbe;
use crate::rng::XorShift32;
use crate::world::World;
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

/// Un pas de simulation : 60 Hz, comme la vraie boucle.
const DT: f32 = 1.0 / 60.0;

/// Ce qu'on retient d'une simulation.
///
/// `BTreeSet` et non `HashSet` : l'ordre d'affichage devient déterministe,
/// donc deux traces se comparent à l'œil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resume {
    pub images: u64,
    pub intentions_tirees: u64,
    /// Combien de fois un réflexe s'est imposé — chutes, atterrissages,
    /// rattrapages.
    pub reflexes: u64,
    pub poses_vues: BTreeSet<String>,

    /// La plus longue période pendant laquelle **rien n'a bougé et aucune
    /// décision n'a été prise**. **C'est le chiffre qui compte** : il doit
    /// rester sous le délai d'abandon (décision n° 4).
    pub blocage_max: Duration,

    /// Empreinte de la suite des intentions tirées.
    ///
    /// Sert à comparer deux exécutions : à graine égale elle est identique,
    /// à graine différente elle diffère presque sûrement. Comparer les
    /// simples compteurs ne suffirait pas — deux histoires différentes
    /// peuvent tirer le même nombre d'intentions.
    pub signature: u64,
}

/// Déroule `minutes` de comportement et rend le résumé.
///
/// Rend `Err(String)` et non une erreur typée : le seul appelant est la
/// ligne de commande, qui va l'imprimer. Un `enum` d'erreurs n'apporterait
/// rien ici.
pub fn executer(
    minutes: u32,
    graine: u32,
    dossier: &Path,
    config: &crate::config::Config,
) -> Result<Resume, String> {
    // ── Le monde et le personnage ───────────────────────────────────────
    let sonde = FakeProbe::deux_ecrans();
    let monde = World::from_screens(&sonde.screens());

    let manifeste: Manifest =
        Manifest::load(dossier).map_err(|e| format!("personnage illisible : {e}"))?;

    let sol = monde
        .platforms()
        .first()
        .ok_or_else(|| "monde sans plateforme".to_string())?;

    let depart = sol.rect.point_on(Face::Top, 500.0);
    let mut ch = Character::new(
        manifeste,
        Attachment::On {
            platform: sol.id,
            face: Face::Top,
            offset: 500.0,
        },
        depart,
    );

    // ── Les sources injectées, toutes déterministes ─────────────────────
    // Pas de `FakeClock` ici : on calcule directement le temps depuis le
    // numéro d'image, ce qui est plus simple et strictement équivalent.
    // `FakeClock` sert aux tests qui doivent faire des sauts dans le temps.
    let mut rng = XorShift32::seeded(graine);

    // La table ET les réglages viennent de la config : sinon la simulation
    // vérifierait un comportement que l'utilisateur n'a pas.
    let table = TableEnvies::depuis_config(config);
    let reglages = crate::config::Reglages::depuis(config);

    // La souris ne bouge pas et le bouton reste relâché : on simule le
    // comportement autonome, pas l'interaction. L'attrapage est vérifié par
    // les tests de `reflex.rs`, et à l'œil en Tâche 11.
    let entrees = Entrees {
        souris: Point::new(0.0, 0.0),
        echelle_affichage: 1.0,
        bouton_gauche: false,
        curseur_sur_le_personnage: false,
        // Neutre jusqu'à la Tâche 7, qui jouera une journée entière.
        biais: crate::signals::Biais::neutre(),
        utilisateur_actif: true,
    };

    // ── La boucle ───────────────────────────────────────────────────────
    let total_images = minutes as u64 * 60 * 60;

    let mut resume = Resume {
        images: 0,
        intentions_tirees: 0,
        reflexes: 0,
        poses_vues: BTreeSet::new(),
        blocage_max: Duration::ZERO,
        signature: 0,
    };

    // Pour la détection de blocage : ce qu'on observait au dernier
    // changement, et quand.
    let mut derniere_empreinte = (String::new(), 0i64, 0u64);
    let mut depuis_changement = Duration::ZERO;

    // On identifie une intention par `(type, depuis)` et non par son seul
    // type : `depuis` est l'instant où elle a commencé, donc deux Flâner
    // consécutifs sont bien deux intentions distinctes.
    //
    // Comparer les seuls types sous-comptait d'un facteur ~5 : cinq tirages
    // sur six donnent Flâner, et un Flâner qui expire et redonne Flâner
    // passait totalement inaperçu.
    let mut intention_precedente: Option<(behavior::intention::Intention, Duration)> = None;

    for i in 0..total_images {
        let maintenant = Duration::from_secs_f64(i as f64 * DT as f64);

        let r = behavior::pas(&mut ch, &monde, &entrees, &table, &reglages, maintenant, DT, &mut rng);

        if r != behavior::reflex::Reflexe::Aucun {
            resume.reflexes += 1;
        }

        // Une intention tirée = l'identité `(type, depuis)` a changé.
        let intention_actuelle = ch.intention.map(|ai| (ai.kind, ai.depuis));
        if intention_actuelle != intention_precedente {
            if let Some((kind, _)) = intention_actuelle {
                resume.intentions_tirees += 1;

                // Une empreinte de la SUITE, pas seulement du compte : deux
                // histoires différentes peuvent tirer autant d'intentions.
                // Mélange multiplicatif banal, sans prétention
                // cryptographique — il ne sert qu'à comparer deux traces.
                let jeton = match kind {
                    behavior::intention::Intention::Flaner => 1u64,
                    behavior::intention::Intention::SeReposer => 2u64,
                };
                resume.signature = resume
                    .signature
                    .wrapping_mul(0x100_0000_01b3)
                    .wrapping_add(jeton);
            }
            intention_precedente = intention_actuelle;
        }

        resume.poses_vues.insert(ch.pose.clone());

        // ── Détection de blocage ────────────────────────────────────────
        // L'empreinte : la pose, la position arrondie au pixel, et le nombre
        // d'intentions tirées.
        //
        // · On arrondit la position parce qu'un flottant qui bouge de 1e-6
        //   par image ferait croire à un mouvement.
        // · Le compteur d'intentions est indispensable : deux repos tirés de
        //   suite laissent la pose et la position identiques pendant 30 s,
        //   alors que rien n'est bloqué — c'est une SUITE DE CHOIX. Sans ce
        //   troisième terme, la mesure confondrait « il ne se passe rien » et
        //   « il a décidé de ne rien faire, deux fois ».
        let empreinte = (
            ch.pose.clone(),
            ch.pos_connue.x.round() as i64,
            resume.intentions_tirees,
        );
        if empreinte != derniere_empreinte {
            derniere_empreinte = empreinte;
            depuis_changement = maintenant;
        } else {
            let immobile = maintenant.saturating_sub(depuis_changement);
            if immobile > resume.blocage_max {
                resume.blocage_max = immobile;
            }
        }

        resume.images += 1;
    }

    Ok(resume)
}

/// Imprime le résumé, en français et lisible d'un coup d'œil.
pub fn imprimer(r: &Resume) {
    println!("── simulation ────────────────────────────────");
    println!("images               : {}", r.images);
    println!(
        "temps simulé         : {:.1} min",
        r.images as f64 * DT as f64 / 60.0
    );
    println!("intentions tirées    : {}", r.intentions_tirees);
    println!("réflexes             : {}", r.reflexes);
    println!(
        "poses vues           : {}",
        r.poses_vues
            .iter()
            .cloned()
            .collect::<Vec<String>>()
            .join(", ")
    );
    println!("signature            : {:#018x}", r.signature);
    println!(
        "blocage le plus long : {:.1} s",
        r.blocage_max.as_secs_f32()
    );

    // Le verdict, plutôt que de laisser le lecteur comparer à 20 s.
    let limite = crate::behavior::intention::DELAI_ABANDON;
    if r.blocage_max > limite {
        println!(
            "⚠️  BLOCAGE au-delà du délai d'abandon ({:.0} s) — régression de comportement",
            limite.as_secs_f32()
        );
    } else {
        println!("✅ jamais bloqué au-delà du délai d'abandon");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le dossier du personnage de test, relatif à `src-tauri/` où cargo
    /// exécute les tests.
    fn blob() -> &'static Path {
        Path::new("../characters/blob")
    }

    /// La config par défaut, construite et non lue depuis le disque : un test
    /// qui lirait le `config.json` de la machine ne serait plus
    /// reproductible.
    fn defauts() -> crate::config::Config {
        crate::config::Config::default()
    }

    #[test]
    fn une_simulation_courte_produit_un_resume_coherent() {
        let r = executer(2, 42, blob(), &defauts()).expect("la simulation doit aboutir");

        // 2 minutes à 60 Hz.
        assert_eq!(r.images, 2 * 60 * 60);
        assert!(r.intentions_tirees > 0, "aucune intention tirée");
    }

    #[test]
    fn il_flane_et_il_se_repose_sur_une_longue_duree() {
        // Ce que la spec §10.3 demande de pouvoir vérifier sans écran.
        // 30 minutes : assez pour que les deux intentions sortent, même avec
        // un rapport de poids de 5 contre 1.
        let r = executer(30, 42, blob(), &defauts()).expect("la simulation doit aboutir");

        assert!(r.poses_vues.contains("walk"), "il n'a jamais marché");
        assert!(r.poses_vues.contains("run"), "il n'a jamais couru");
        assert!(r.poses_vues.contains("stand"), "il ne s'est jamais arrêté");
        assert!(r.poses_vues.contains("sit"), "il ne s'est jamais reposé");
    }

    #[test]
    fn il_n_est_jamais_bloque_plus_que_le_delai_d_abandon() {
        // **La régression de comportement que rien d'autre ne détecte.**
        // « Bloqué » = ni la pose, ni la position, ni l'intention n'ont
        // changé (voir le commentaire de l'empreinte).
        let r = executer(30, 42, blob(), &defauts()).expect("la simulation doit aboutir");

        let limite = crate::behavior::intention::DELAI_ABANDON + Duration::from_secs(1);
        assert!(
            r.blocage_max <= limite,
            "bloqué {:?}, limite {:?}",
            r.blocage_max,
            limite
        );
    }

    #[test]
    fn la_simulation_est_reproductible_a_graine_fixe() {
        let a = executer(5, 999, blob(), &defauts()).unwrap();
        let b = executer(5, 999, blob(), &defauts()).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn deux_graines_donnent_deux_histoires() {
        // Sinon l'aléatoire ne sert à rien, et « jamais prévisible » est
        // faux. On compare la SIGNATURE et non les compteurs : deux
        // histoires différentes peuvent tirer autant d'intentions.
        let a = executer(5, 1, blob(), &defauts()).unwrap();
        let b = executer(5, 2, blob(), &defauts()).unwrap();
        assert_ne!(a.signature, b.signature);
    }

    #[test]
    fn un_dossier_de_personnage_invalide_donne_une_erreur_lisible() {
        let e =
            executer(1, 1, Path::new("../characters/inexistant"), &defauts()).expect_err("doit échouer");
        assert!(e.contains("mascot.json"), "message peu clair : {e}");
    }
}
