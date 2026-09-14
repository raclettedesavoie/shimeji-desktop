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
use crate::character::manifest::{
    Manifest, POSE_CLIMB_CEILING, POSE_CLIMB_WALL, POSE_GRAB_CEILING, POSE_GRAB_WALL,
};
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

    /// Combien de temps il a passé accroché à une face verticale ou au
    /// plafond (étape 4a) — sol exclu, donc `Face::Top` ne compte pas.
    ///
    /// C'est le chiffre qui prouve que l'escalade a bien été EXERCÉE par
    /// cette simulation, et pas seulement rendue possible : un poids
    /// `envies.grimper` mis à zéro par erreur laisserait ce champ à zéro sans
    /// qu'aucun autre test ne le remarque.
    pub temps_accroche: Duration,

    /// Combien de fois il a atteint le plafond (transition `Paroi` →
    /// `Plafond`, comptée une fois par bascule, pas par image).
    pub plafonds_atteints: u64,
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
        temps_accroche: Duration::ZERO,
        plafonds_atteints: 0,
    };

    // Le sommeil, compté en images puis converti en secondes une seule fois
    // à la fin — accumuler des `f32` sur 5 millions d'images introduirait une
    // erreur d'arrondi qu'un compteur entier n'a pas.
    let mut images_endormi: u64 = 0;
    let mut images_endormi_par_heure = [0u64; 24];
    let mut dormait = false;

    // Même principe pour le temps accroché (étape 4a) : un compteur
    // d'images converti une seule fois à la fin, pour la même raison
    // d'arrondi.
    let mut images_accroche: u64 = 0;

    // La dernière phase de `Grimper` vue, pour détecter la TRANSITION vers
    // `Plafond` (et compter « atteint le plafond » une fois par bascule, pas
    // une fois par image passée dessus).
    let mut derniere_phase_grimpe: Option<behavior::intention::PhaseGrimpe> = None;

    // Pour la détection de blocage : ce qu'on observait au dernier
    // changement, et quand.
    let mut derniere_empreinte = (String::new(), 0i64, 0i64, 0u64);
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
            // Aucune commande : la simulation n'a ni souris ni menu. C'est
            // voulu — elle mesure la vie AUTONOME du personnage, et une
            // commande injectée fausserait l'histogramme de sommeil.
            commande: None,
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

        // ── L'invariant du monde vertical (étape 4a) ────────────────────
        //
        // Le filet de la règle de sécurité, vérifié 60 fois par seconde
        // simulée : si le personnage est accroché à une face verticale ou au
        // plafond, sa pose est forcément une pose d'escalade. C'est ce qui
        // attrape la régression « il marche sur un mur », qu'aucun test
        // unitaire ne verrait parce qu'elle demande la conjonction d'un
        // tirage et d'une transition.
        if let Attachment::On { face, .. } = ch.attachment {
            if face != Face::Top {
                // Une image de plus passée hors du sol : compté en images
                // pour la même raison que le sommeil, converti à la fin.
                images_accroche += 1;

                let attendue = matches!(
                    ch.pose.as_str(),
                    POSE_GRAB_WALL | POSE_CLIMB_WALL | POSE_GRAB_CEILING | POSE_CLIMB_CEILING
                );
                assert!(
                    attendue,
                    "pose « {} » sur une face {:?} à t = {:?}",
                    ch.pose, face, maintenant
                );
            }
        }

        // Combien de fois il a atteint le plafond : une TRANSITION de phase,
        // pas un total d'images — sinon un personnage qui traverse
        // lentement le plafond compterait des centaines d'« atteintes ».
        if let Some(behavior::intention::ActiveIntention {
            etat: behavior::intention::EtatIntention::Grimpe { phase, .. },
            ..
        }) = ch.intention
        {
            let etait_deja_au_plafond =
                matches!(derniere_phase_grimpe, Some(behavior::intention::PhaseGrimpe::Plafond { .. }));
            if matches!(phase, behavior::intention::PhaseGrimpe::Plafond { .. }) && !etait_deja_au_plafond
            {
                resume.plafonds_atteints += 1;
            }
            derniere_phase_grimpe = Some(phase);
        } else {
            // Il n'est plus en train de grimper : la prochaine escalade
            // repart d'un historique vierge, sinon une deuxième bascule sur
            // le plafond d'une AUTRE escalade ne compterait pas — l'ancienne
            // phase `Plafond` traînerait encore.
            derniere_phase_grimpe = None;
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
                    // Deux jetons distincts : deux histoires qui jouent à des
                    // choses différentes doivent donner des signatures
                    // différentes.
                    behavior::intention::Intention::Jouer(
                        behavior::intention::Jeu::TeteQuiTourne,
                    ) => 3u64,
                    behavior::intention::Intention::Jouer(
                        behavior::intention::Jeu::JambesQuiBalancent,
                    ) => 4u64,
                    // L'escalade (étape 4a) : son propre jeton, pour que deux
                    // histoires qui grimpent différemment se distinguent.
                    behavior::intention::Intention::Grimper => 5u64,
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
        // L'empreinte : la pose, la position (x ET y) arrondie au pixel, et
        // le nombre d'intentions tirées.
        //
        // · On arrondit la position parce qu'un flottant qui bouge de 1e-6
        //   par image ferait croire à un mouvement.
        // · `y` a rejoint `x` avec l'étape 4a : jusque-là tout déplacement
        //   était horizontal, et l'empreinte n'avait donc besoin que de `x`.
        //   Une escalade avance en `y` à x constant — sans ce terme, monter
        //   un mur de 1032 px à 16,1 px/s (64 s, cf. `VITESSE_ESCALADE`)
        //   ressemblait à un blocage de 64 s, largement au-dessus de la
        //   limite de 21 s, alors qu'il grimpait bel et bien. Un faux
        //   positif du détecteur, pas une régression du comportement.
        // · Le compteur d'intentions est indispensable : deux repos tirés de
        //   suite laissent la pose et la position identiques pendant 30 s,
        //   alors que rien n'est bloqué — c'est une SUITE DE CHOIX. Sans ce
        //   troisième terme, la mesure confondrait « il ne se passe rien » et
        //   « il a décidé de ne rien faire, deux fois ».
        let empreinte = (
            ch.pose.clone(),
            ch.pos_connue.x.round() as i64,
            ch.pos_connue.y.round() as i64,
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
    resume.temps_accroche = Duration::from_secs_f32(images_accroche as f32 * DT);

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
    println!(
        "accroché (mur/plafond) : {:.1} s",
        r.temps_accroche.as_secs_f32()
    );
    println!("plafonds atteints : {}", r.plafonds_atteints);
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

// Les tests de ce module vivent dans `sim_tests.rs`
// (sortis d ici le 2026-09-14 : ils faisaient 229 des 760 lignes).
#[cfg(test)]
#[path = "sim_tests.rs"]
mod tests;
