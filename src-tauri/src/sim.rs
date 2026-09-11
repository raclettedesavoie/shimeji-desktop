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
    /// décision n'a été prise**.
    ///
    /// ⚠️ **Ce que ce chiffre peut, et ne peut pas, détecter** (précisé par
    /// la vague de correction finale, point 7e — le commentaire précédent en
    /// promettait plus). L'empreinte comparée à chaque image inclut
    /// `intentions_tirees` ; or toute intention expire au plus tard à
    /// `DELAI_ABANDON` (décision n° 4), ce qui incrémente ce compteur et
    /// change l'empreinte. `blocage_max` ne peut donc **structurellement
    /// pas** dépasser le délai d'abandon tant qu'au moins UNE intention
    /// continue d'être tirée de temps en temps, quelle qu'elle soit. Le seul
    /// cas que ce chiffre détecte réellement est **« plus aucune intention
    /// n'est tirée du tout »** — un personnage sans aucune pose jouable
    /// (spec §8.6).
    ///
    /// Ce que ce chiffre ne détecte PAS : un tirage qui se répète très vite
    /// sans jamais laisser une intention progresser. C'est exactement ce qui
    /// s'est produit dans la boucle pathologique du point 1 de cette même
    /// vague — la couche 3 re-tirait `SeReposer` à chaque changement
    /// d'identité d'intention, ce qui change l'empreinte à CHAQUE image et
    /// maintient `blocage_max` proche de zéro, alors même que le personnage
    /// restait visuellement figé. Cette régression-là n'a été révélée que
    /// par l'assertion sur le sommeil des heures 22-23, pas par ce champ.
    pub blocage_max: Duration,

    /// Empreinte de la suite des intentions tirées.
    ///
    /// Sert à comparer deux exécutions : à graine égale elle est identique,
    /// à graine différente elle diffère presque sûrement. Comparer les
    /// simples compteurs ne suffirait pas — deux histoires différentes
    /// peuvent tirer le même nombre d'intentions.
    pub signature: u64,

    /// Combien de secondes il a passées dans la pose de sommeil.
    pub secondes_endormi: u64,

    /// Combien de fois il est passé de la pose de sommeil à autre chose.
    pub reveils: u64,

    /// Les secondes de sommeil, ventilées par heure locale.
    ///
    /// **C'est le chiffre qui prouve l'étape** : il ne suffit pas qu'il
    /// dorme, il faut qu'il dorme quand l'utilisateur n'est pas là. Un total
    /// ne le dirait pas.
    pub endormi_par_heure: [u32; 24],
}

/// La journée scriptée que la simulation joue.
///
/// **Pourquoi une chronologie et pas des signaux constants :** un biais
/// constant ne se distingue pas d'un biais absent. Ce qu'on veut prouver,
/// c'est que le personnage dort **au bon moment** — donc il faut des moments.
///
/// Déterministe et sans aléatoire : à `minute` égale, mêmes signaux. C'est ce
/// qui garde la simulation comparable d'une exécution à l'autre, comme la
/// graine du générateur.
///
/// La journée commence à **9 h** — l'heure à laquelle on lance son ordinateur,
/// donc celle qui rend la trace lisible.
pub fn signaux_de_la_journee(minute: u32) -> crate::probe::Signaux {
    const DEBUT: u32 = 9;
    let heure = ((DEBUT + minute / 60) % 24) as u8;

    // Présent : 9 h-12 h, 14 h-18 h, 20 h-minuit (il travaille tard, un soir).
    // Absent le reste du temps — pause déjeuner, début de soirée, nuit.
    //
    // Le créneau 22-23 h a été ajouté par la vague de correction finale : sans
    // lui, aucune minute de présence ne tombait dans le créneau « soir »
    // (22 h→6 h, `SignauxReglages::soir_debut/fin`), et la combinaison
    // « signal de sommeil actif + utilisateur présent » n'était donc jamais
    // jouée. C'est exactement le trou qui cachait la contradiction entre
    // l'entrée en sommeil et l'interruption du sommeil (voir `intention.rs`,
    // le commentaire sur `veut_dormir` dans `se_reposer`).
    let present = matches!(heure, 9..=11 | 14..=17 | 20..=23);

    // Combien de temps s'est-il écoulé depuis la dernière minute de présence ?
    //
    // On le calcule en remontant plutôt qu'en gardant un état : la fonction
    // reste pure, donc appelable pour n'importe quelle minute dans n'importe
    // quel ordre — ce qu'un test fait justement.
    let inactivite = if present {
        std::time::Duration::ZERO
    } else {
        let mut recul = 1u32;
        while recul <= minute {
            let h = ((DEBUT + (minute - recul) / 60) % 24) as u8;
            if matches!(h, 9..=11 | 14..=17 | 20..=21) {
                break;
            }
            recul += 1;
        }
        std::time::Duration::from_secs(recul as u64 * 60)
    };

    crate::probe::Signaux {
        inactivite,
        // Une application au premier plan pendant les heures de travail : la
        // table des modificateurs est vide par défaut, donc ça ne change rien
        // — mais un utilisateur qui ajoute une ligne le verra dans la trace.
        //
        // ⚠️ **C'est aussi le SEUL canal, dans cette simulation, par lequel un
        // modificateur d'application mal réglé ferait fuiter du sommeil aux
        // heures de présence.** Si un `config.json` d'utilisateur donnait à
        // `Code.exe` un `seReposer` démesuré (par erreur de saisie, un `80`
        // au lieu d'un `0.8`), ce serait précisément aux heures « présent »
        // ci-dessus que cette simulation le révélerait — parce que c'est la
        // seule application active jouée ici. La table par défaut étant vide,
        // rien ne l'exerce aujourd'hui ; mais si `une_journee_entiere_dort_...`
        // se mettait un jour à rougir sans qu'aucun autre changement
        // n'explique pourquoi, c'est le premier endroit à soupçonner.
        appli_active: if present {
            Some("Code.exe".to_string())
        } else {
            None
        },
        heure,
        // Sur secteur : la batterie a ses propres tests unitaires, et la
        // faire varier ici brouillerait la lecture du signal d'inactivité.
        batterie: crate::probe::Batterie {
            pourcent: None,
            sur_secteur: true,
        },
        // Le verrouillage n'est pas un signal de comportement : il porte sur
        // la fenêtre (Tâche 6), que la simulation n'a pas.
        session_verrouillee: false,
    }
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
    //
    // `biais` et `utilisateur_actif` sont recalculés dans la boucle, aux
    // mêmes 2 Hz que la vraie boucle (spec §5.5) : c'est
    // `signaux_de_la_journee` qui les fait varier au fil de la journée
    // scriptée.
    const IMAGES_PAR_SIGNAL: u64 = 30; // 60 Hz / 2 Hz

    let mut biais = crate::signals::Biais::neutre();
    let mut utilisateur_actif = true;
    let mut heure_courante: u8 = 9;

    // ── La boucle ───────────────────────────────────────────────────────
    let total_images = minutes as u64 * 60 * 60;

    let mut resume = Resume {
        images: 0,
        intentions_tirees: 0,
        reflexes: 0,
        poses_vues: BTreeSet::new(),
        blocage_max: Duration::ZERO,
        signature: 0,
        secondes_endormi: 0,
        reveils: 0,
        endormi_par_heure: [0; 24],
    };

    // Le sommeil, compté en images puis converti en secondes une seule fois
    // à la fin — accumuler des `f32` sur 5 millions d'images introduirait une
    // erreur d'arrondi qu'un compteur entier n'a pas.
    let mut images_endormi: u64 = 0;
    let mut images_endormi_par_heure = [0u64; 24];
    let mut dormait = false;

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

        // Recalcul des signaux à 2 Hz (spec §5.5) : la simulation doit
        // exercer le comportement au même rythme que la vraie boucle, sinon
        // elle vérifierait autre chose que ce qui tourne réellement.
        if i % IMAGES_PAR_SIGNAL == 0 {
            let minute = (i / (60 * 60)) as u32;
            let s = signaux_de_la_journee(minute);
            biais = crate::signals::biais_de(&s, config);
            // Même fonction que `main.rs` (`signals::utilisateur_actif`), et
            // pas recopiée ici : c'est justement la divergence que ce
            // partage empêche — voir son commentaire dans `signals.rs`.
            utilisateur_actif = crate::signals::utilisateur_actif(&s, config);
            heure_courante = s.heure;
        }

        let entrees = Entrees {
            souris: Point::new(0.0, 0.0),
            echelle_affichage: 1.0,
            bouton_gauche: false,
            curseur_sur_le_personnage: false,
            biais,
            utilisateur_actif,
        };

        let r = behavior::pas(&mut ch, &monde, &entrees, &table, &reglages, maintenant, DT, &mut rng);

        if r != behavior::reflex::Reflexe::Aucun {
            resume.reflexes += 1;
        }

        // ── Le sommeil, compté par heure ────────────────────────────────
        let dort = ch.pose == crate::character::manifest::POSE_SLEEP;
        if dort {
            // Une image vaut DT seconde ; on compte en images et on convertit
            // à la fin pour ne pas accumuler d'erreur de flottant sur
            // 5 millions d'images.
            images_endormi += 1;
            images_endormi_par_heure[heure_courante as usize] += 1;
        }
        if dormait && !dort {
            resume.reveils += 1;
        }
        dormait = dort;

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
                    // Deux jetons distincts : deux histoires qui jouent à des
                    // choses différentes doivent donner des signatures
                    // différentes.
                    behavior::intention::Intention::Jouer(
                        behavior::intention::Jeu::TeteQuiTourne,
                    ) => 3u64,
                    behavior::intention::Intention::Jouer(
                        behavior::intention::Jeu::JambesQuiBalancent,
                    ) => 4u64,
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

    // Images → secondes, une seule fois.
    resume.secondes_endormi = (images_endormi as f32 * DT) as u64;
    for h in 0..24 {
        resume.endormi_par_heure[h] = (images_endormi_par_heure[h] as f32 * DT) as u32;
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
    println!("endormi           : {} s au total", r.secondes_endormi);
    println!("réveils           : {}", r.reveils);
    println!("sommeil par heure :");
    for h in 0..24 {
        let s = r.endormi_par_heure[h];
        if s == 0 {
            continue;
        }
        // Une barre par tranche de 2 minutes, pour que 24 lignes tiennent
        // dans un terminal.
        let barre = "#".repeat((s / 120).min(60) as usize);
        println!("  {h:02} h {s:5} s {barre}");
    }

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
        // Et il ne court **jamais** : la course a quitté le tirage de la
        // flânerie (`poids_course` = 0). Ce test la verrouille dehors — si
        // quelqu'un remettait un poids par défaut, il le dirait.
        assert!(
            !r.poses_vues.contains("run"),
            "il a couru alors que la course a quitté la flânerie"
        );
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

    #[test]
    fn la_chronologie_couvre_une_journee_entiere() {
        // L'heure doit avancer, faire le tour, et rester dans 0..24.
        let h = |min| signaux_de_la_journee(min).heure;
        assert_eq!(h(0), 9, "la journée commence à 9 h");
        assert_eq!(h(60), 10);
        assert_eq!(h(15 * 60), 0, "9 h + 15 h = minuit");
        for min in 0..(24 * 60) {
            assert!(signaux_de_la_journee(min).heure < 24);
        }
    }

    #[test]
    fn la_chronologie_alterne_presence_et_absence() {
        // Aux heures de travail il est là ; la nuit il est parti. Sans cette
        // alternance, la simulation ne prouverait rien : un biais constant ne
        // se distingue pas d'un biais absent.
        let inactif = |min| signaux_de_la_journee(min).inactivite;
        assert_eq!(inactif(30), std::time::Duration::ZERO, "10 h : il travaille");
        assert!(
            inactif(15 * 60) > std::time::Duration::from_secs(120),
            "minuit : il est parti depuis longtemps"
        );
    }

    #[test]
    fn une_journee_entiere_dort_au_bon_moment() {
        // **LE test de l'étape**, et il vérifie quatre choses d'un coup.
        //
        // Une seule simulation pour les quatre : dérouler 24 h fait
        // 5,2 millions d'images, soit une à trois secondes en debug. La
        // lancer quatre fois multiplierait par quatre le temps de la suite
        // entière, qui tient aujourd'hui en 0,43 s.
        //
        // > Si ce test dépasse ~10 s sur la machine, le marquer `#[ignore]`
        // > et s'appuyer sur `--sim 1440` (Step 8), qui est de toute façon
        // > l'artefact qu'on lit.
        let r = executer(24 * 60, 42, blob(), &defauts()).expect("la simulation doit aboutir");

        // ── 1. Il dort ──────────────────────────────────────────────────
        // 2 h, 3 h, 4 h : personne devant la machine.
        let nuit: u32 = (2..=4).map(|h| r.endormi_par_heure[h]).sum();
        assert!(nuit > 0, "il n'a pas dormi de la nuit");

        // ── 2. Il dort AU BON MOMENT ────────────────────────────────────
        // C'est la différence entre un signal branché et un signal qui
        // marche : un total de sommeil ne dirait rien, seule la ventilation
        // par heure le dit. 9 h-11 h, il est au clavier.
        let matin: u32 = (9..=11).map(|h| r.endormi_par_heure[h]).sum();
        assert!(
            nuit > matin * 5,
            "il dort autant le matin que la nuit : le signal ne mord pas              (matin {matin} s, nuit {nuit} s)"
        );

        // ── 2 bis. Présent le soir, il ne dort PAS profondément ──────────
        //
        // Aux heures 22-23, la chronologie le dit PRÉSENT (voir le
        // commentaire de `signaux_de_la_journee`) alors que le signal du soir
        // (22 h→6 h, ×3 sur le repos) est déjà actif. Sans l'invariant
        // « phase Endormi ⇒ utilisateur absent » (`intention.rs`), le sommeil
        // profond resterait atteignable : le ×3 du soir suffit à franchir
        // `seuilSommeil = 2.0` à lui seul, présence ou pas.
        //
        // Ce n'est PAS une redite de l'assertion précédente : `matin` ne
        // couvre que 9 h-11 h, où aucun signal de sommeil ne mord — elle ne
        // pouvait donc rien dire de la contradiction entre le signal du soir
        // et la présence.
        let soir_present: u32 = (22..=23).map(|h| r.endormi_par_heure[h]).sum();
        assert_eq!(
            soir_present, 0,
            "il dort profondément à 22-23 h alors que l'utilisateur est présent : \
             l'entrée en sommeil ne doit pas être possible utilisateur actif"
        );

        // ── 3. Il sort du sommeil, par un moyen ou un autre ─────────────
        //
        // La chronologie compte DEUX retours de l'utilisateur : à 14 h
        // (après la pause déjeuner) et à 20 h (après la soirée). 9 h est le
        // DÉBUT de la journée simulée, pas un retour — il n'y a personne
        // avant. Vérifié en rejouant `signaux_de_la_journee` sur les 1440
        // minutes et en comptant les transitions absent → présent (vague de
        // correction finale, point 4 : l'ancien commentaire disait « cinq »,
        // ce qui était faux).
        //
        // ⚠️ **Cette assertion ne teste PAS le mécanisme d'interruption.**
        // `resume.reveils` compte TOUTE transition de la pose `sleep` vers
        // autre chose — y compris la fin naturelle d'un sommeil (20-60 s,
        // `PhaseRepos::Endormi`) ou l'expiration au délai d'abandon (20 s).
        // L'assertion resterait verte même si le bloc d'interruption de
        // `behavior::mod::pas` disparaissait entièrement : un sommeil finit
        // toujours par se terminer tout seul. Le vrai mécanisme du réveil
        // (« redevenir actif termine le sommeil, sans le choisir ») est
        // couvert ailleurs, par les tests unitaires de `behavior/mod.rs`
        // (`redevenir_actif_reveille_le_personnage_endormi`,
        // `le_reveil_ne_choisit_pas_la_suite`,
        // `etre_actif_n_empeche_pas_de_s_asseoir`). Ici, on vérifie
        // seulement qu'il ne reste pas coincé en sommeil pour toujours — et
        // la vraie propriété de l'étape, « il ne dort pas profondément
        // pendant que l'utilisateur travaille », est déjà couverte par
        // l'assertion sur les heures 22-23 ci-dessus.
        assert!(r.reveils > 0, "il ne s'est jamais réveillé");

        // ── 4. LA MARGE SURVIT — le test de la décision n° 3 ────────────
        //
        // Même avec ×8 sur le repos pendant toute la nuit, il ne doit PAS
        // avoir dormi 100 % du temps. « Cette marge est le produit. » Si
        // elle disparaissait, le signal COMMANDERAIT au lieu de biaiser, et
        // aucun autre test ne s'en apercevrait.
        let nuit_complete: u32 = (0..6).map(|h| r.endormi_par_heure[h]).sum();
        let six_heures: u32 = 6 * 3600;
        assert!(
            nuit_complete < (six_heures as f32 * 0.95) as u32,
            "il a dormi {nuit_complete} s sur {six_heures} : la marge a disparu"
        );

        // ── Et la décision n° 4 tient toujours ──────────────────────────
        // Le sommeil est la plus longue immobilité du programme : c'est ici
        // qu'un blocage se verrait.
        assert!(
            r.blocage_max < crate::behavior::intention::DELAI_ABANDON,
            "blocage de {:?}, au-delà du délai d'abandon",
            r.blocage_max
        );
    }
}
