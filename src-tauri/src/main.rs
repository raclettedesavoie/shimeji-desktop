// Pas de console en release, mais on la garde en debug.
//
// `cfg_attr(not(debug_assertions), …)` plutôt que l'attribut nu : le mode
// simulation (`--sim`), la sous-commande `--demarrage` et les traces de
// diagnostic (`SHIMEJI_CADENCE`, `SHIMEJI_TRACE`) écrivent tous sur la sortie
// standard. Les priver de console en debug les rendrait muets.
//
// **Possible seulement depuis la Tâche 1** : sans « Quitter » dans le tray,
// une application sans console ne se fermerait plus du tout.
//
// ⚠️ Un attribut `#![…]` de niveau *crate* doit être la PREMIÈRE chose du
// fichier — avant les commentaires de module et les `mod`. Le placer après
// donne « inner attribute is not permitted following an outer attribute ».
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Amorçage de l'application.
//
// Deux modes : l'application (fenêtre, boucle 60 Hz) et le mode simulation
// (`--sim`, sans écran, spec §10.3). Une sous-commande et non un second
// binaire : deux cibles binaires d'un même paquet ne partagent du code que
// par une cible `lib`, et ajouter un `lib.rs` pour cela seul réorganiserait
// tout le projet.
//
// Pas de `#![windows_subsystem = "windows"]` pour le moment : on VEUT la
// console pendant le développement (topologie des écrans, trace du
// comportement). Le plan 1b la supprimera, une fois le tray disponible pour
// quitter proprement.

mod actions;
mod autostart;
mod behavior;
mod character;
mod clock;
mod config;
mod geom;
mod menu_perso;
mod probe;
mod rechargement;
mod render;
mod rng;
mod signals;
mod sim;
mod tray;
mod world;

// En Rust, **les méthodes d'un trait ne sont visibles que si le trait est
// dans la portée** — même quand on détient le type concret qui l'implémente.
// Sans ces deux lignes, `sonde.screens()` et `horloge.elapsed()` donnent
// « no method named … found for struct Win32Probe », ce qui laisse croire
// que la méthode n'existe pas alors qu'il manque seulement un `use`.
//
// C'est le pendant de « les traits sont ouverts » : n'importe qui peut
// ajouter un trait à un type, donc le compilateur ne cherche que parmi ceux
// qu'on a explicitement fait entrer.
use clock::Clock;
use probe::SystemProbe;

fn main() {
    // AVANT TOUT LE RESTE. Sans cet appel, Windows virtualise les
    // coordonnées et la sonde rendrait des pixels logiques en croyant rendre
    // des pixels physiques (spec §3.4). Voir le commentaire de la fonction.
    probe::win32::activer_conscience_dpi();

    // Aiguillage minimal, écrit à la main : une crate d'analyse d'arguments
    // pour deux drapeaux serait disproportionnée, et le plan 1b n'en ajoutera
    // pas d'autres (les réglages iront dans config.json).
    let args: Vec<String> = std::env::args().collect();

    if let Some(i) = args.iter().position(|a| a == "--sim") {
        // `get(i + 1)` puis `parse` : une valeur absente ou illisible vaut
        // 30 minutes plutôt qu'une erreur — c'est un outil de développement.
        let minutes = args
            .get(i + 1)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(30);

        let graine = args
            .iter()
            .position(|a| a == "--graine")
            .and_then(|j| args.get(j + 1))
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(42);

        let dossier = config::dossier_personnages().join("blob");
        println!(
            "simulation de {minutes} min, graine {graine}, personnage {}",
            dossier.display()
        );

        match sim::executer(minutes, graine, &dossier, &config::charger()) {
            Ok(r) => sim::imprimer(&r),
            Err(e) => {
                eprintln!("simulation impossible : {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // Sous-commande de diagnostic du démarrage automatique.
    //
    // Elle existe pour une raison précise : le basculement se fait par une
    // case du tray, donc **un clic humain**, et le code du registre ne
    // serait autrement vérifiable qu'à la main. Un test unitaire ne convient
    // pas non plus — il modifierait le registre de la machine qui exécute la
    // suite.
    //
    // Effet de bord utile : elle rend le réglage scriptable.
    if let Some(i) = args.iter().position(|a| a == "--demarrage") {
        match args.get(i + 1).map(|s| s.as_str()) {
            Some("on") => match autostart::activer() {
                Ok(()) => println!("démarrage avec Windows : activé"),
                Err(e) => {
                    eprintln!("échec : {e}");
                    std::process::exit(1);
                }
            },
            Some("off") => match autostart::desactiver() {
                Ok(()) => println!("démarrage avec Windows : désactivé"),
                Err(e) => {
                    eprintln!("échec : {e}");
                    std::process::exit(1);
                }
            },
            _ => println!(
                "démarrage avec Windows : {}",
                if autostart::est_actif() {
                    "actif"
                } else {
                    "inactif"
                }
            ),
        }
        return;
    }

    // `--signaux` : imprime l'instantané de la sonde et sort.
    //
    // C'est la seule vérification possible des cinq appels Windows : aucun
    // test ne peut savoir depuis combien de temps l'utilisateur n'a rien
    // touché. Rendue scriptable plutôt que laissée à l'œil, comme le reste
    // du projet — on peut la lancer deux fois à 5 s d'intervalle et vérifier
    // que l'inactivité a bien augmenté de 5 s.
    if args.iter().any(|a| a == "--signaux") {
        probe::win32::activer_conscience_dpi();
        let sonde = probe::win32::Win32Probe::new();
        let s = sonde.signaux();
        println!("inactivite        : {:.1} s", s.inactivite.as_secs_f32());
        println!("appli active      : {}", s.appli_active.as_deref().unwrap_or("(aucune)"));
        println!("heure locale      : {} h", s.heure);
        match s.batterie.pourcent {
            Some(p) => println!("batterie          : {p} %"),
            None => println!("batterie          : (aucune, ou inconnue)"),
        }
        println!("sur secteur       : {}", s.batterie.sur_secteur);
        println!("session verrouillee : {}", s.session_verrouillee);
        return;
    }

    lancer_application();
}

/// Le label de la fenêtre d'un personnage. Un seul personnage à l'étape 1a ;
/// l'étape 3 en instanciera plusieurs, d'où l'index dès maintenant.
fn label_de(index: usize) -> String {
    format!("pet-{index}")
}

fn lancer_application() {
    let dossier = config::dossier_personnages();
    println!("personnages : {}", dossier.display());

    // ── La configuration ────────────────────────────────────────────────
    // `charger()` ne peut pas échouer : ni l'absence de fichier, ni un
    // fichier partiel, ni même un JSON cassé n'empêchent le personnage de
    // vivre (spec §9.3). Un fichier malformé est signalé bruyamment par
    // `config`, pas ici.
    let configuration = config::charger();
    let reglages = config::Reglages::depuis(&configuration);
    println!(
        "config : échelle {}, vitesse ×{} (marche {} px/s)",
        configuration.echelle, configuration.vitesse, reglages.vitesse_marche
    );

    // Le nom du dossier du personnage. `first()` : l'étape 1 n'en instancie
    // qu'un ; l'étape 3 bouclera sur la liste. `unwrap_or_else` plutôt qu'un
    // échec : une liste vide dans la config ne doit pas empêcher de démarrer
    // (spec §9.3), et `blob` est le personnage livré.
    let nom_personnage = configuration
        .personnages
        .first()
        .cloned()
        .unwrap_or_else(|| "blob".to_string());

    // ── Le manifeste, avant tout le reste ───────────────────────────────
    // Sans personnage, il n'y a rien à afficher : autant échouer tout de
    // suite avec un message clair que d'ouvrir une fenêtre vide.
    let manifeste = match character::manifest::Manifest::load(&dossier.join(&nom_personnage)) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("personnage « {nom_personnage} » illisible : {e}");
            std::process::exit(1);
        }
    };
    println!(
        "personnage chargé : {} ({} poses)",
        manifeste.name,
        manifeste.poses.len()
    );

    // La table d'envies, réglée par la config (décision n° 5). Construite
    // une fois : elle ne change qu'au rechargement à chaud (Tâche 5).
    let table = behavior::desire::TableEnvies::depuis_config(&configuration);
    let echelle_config = configuration.echelle;

    // Le dossier est déplacé dans la fermeture du schéma URI ci-dessous ;
    // on en garde une copie pour la suite.
    let dossier_pour_protocole = dossier.clone();

    tauri::Builder::default()
        // ── Le schéma URI qui sert les PNG externes ─────────────────────
        // Les personnages sont des fichiers externes au binaire (spec §8.1),
        // donc aucun chemin relatif du webview ne peut les atteindre. Rust
        // les sert lui-même. Sur Windows, ce schéma est accessible sous
        // http://shime.localhost/<personnage>/<numéro>.
        //
        // Vérifié : `Builder::register_uri_scheme_protocol`
        // (tauri-2.11.5/src/app.rs:2130) ; l'hôte `.localhost` sur Windows
        // (src/manager/mod.rs:342).
        .register_uri_scheme_protocol("shime", move |_ctx, requete| {
            servir_frame(&dossier_pour_protocole, requete.uri().path())
        })
        .setup(move |app| {
            // ── La topologie, et le monde ───────────────────────────────
            let sonde = probe::win32::Win32Probe::new();
            probe::win32::imprimer_diagnostic(&sonde);

            let ecrans = sonde.screens();
            let monde = world::World::from_screens(&ecrans);
            if monde.platforms().is_empty() {
                eprintln!("aucun écran : rien à faire.");
                return Ok(());
            }

            // ── Le personnage, posé au milieu du premier sol ────────────
            // `let … else` : sans écran, il n'y a nulle part où poser le
            // personnage. On sort du bloc de placement plutôt que de paniquer
            // — un monde vide est un cas normal (session distante en cours
            // d'établissement), voir `World::from_screens`. Le bloc englobant
            // rend déjà `Ok(())` juste au-dessus pour ce même cas.
            let Some(sol) = monde.premier_sol() else {
                return Ok(());
            };
            let offset = sol.rect.face_length(world::Face::Top) / 2.0;
            let depart = sol.rect.point_on(world::Face::Top, offset);

            // L'échelle d'AFFICHAGE : celle du moniteur, multipliée par le
            // réglage de l'utilisateur. Les fonctions de `attach` n'ont pas à
            // savoir que le second existe — elles reçoivent un seul facteur,
            // et c'est tout ce dont elles ont besoin.
            let echelle_affichage = ecrans[0].scale * configuration.echelle;
            let taille = character::attach::window_size(&manifeste, echelle_affichage);

            // ── La fenêtre ──────────────────────────────────────────────
            // Exactement la combinaison validée par l'étape 0, plus les deux
            // styles étendus qu'elle a révélés manquants.
            let label = label_de(0);
            let win = tauri::WebviewWindowBuilder::new(
                app,
                &label,
                // Le fragment dit à `pet.js` quel personnage servir, et il
                // doit porter le nom **lu dans la config**, pas `blob` en dur.
                //
                // Le bug que ça corrige est sournois : avec `#blob` fixe, le
                // manifeste chargé était bien celui du personnage demandé
                // (bonnes poses, bonnes ancres, bonne hitbox) mais le webview
                // réclamait `shime:///blob/N` — donc les **images** de blob.
                // Rien ne le signalait : aucune erreur, aucune trace, un
                // personnage parfaitement animé… avec le mauvais dessin.
                //
                // Invisible tant que `blob` était le seul pack livré. Constaté
                // à l'œil en ajoutant `luffy`, et par aucun autre moyen.
                tauri::WebviewUrl::App(format!("index.html#{nom_personnage}").into()),
            )
            .title("shimeji-desktop")
            .inner_size(taille.0 as f64, taille.1 as f64)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false)
            .focused(false)
            .build()?;

            // Les clics traversent en permanence ; la Tâche 11 ne les
            // réactive que dans la hitbox de la pose courante (spec §3.3).
            win.set_ignore_cursor_events(true)?;

            // **Les deux découvertes de l'étape 0.** À faire ici, avant que
            // la hitbox n'existe : sinon le vol de focus apparaîtrait en même
            // temps que l'attrapabilité, et les deux se diagnostiqueraient
            // ensemble, pour rien.
            match render::appliquer_styles_etendus(&win) {
                Ok(()) => println!("styles étendus posés (NOACTIVATE, TOOLWINDOW)"),
                // Non bloquant : la fenêtre marche sans, elle est seulement
                // moins polie. Mieux vaut un personnage qui vole le focus
                // qu'aucun personnage.
                Err(e) => eprintln!("styles étendus NON appliqués : {e}"),
            }

            // ── Le tray ─────────────────────────────────────────────────
            // Installé AVANT la boucle : si le tray échoue, on veut le savoir
            // tout de suite, pas après avoir démarré un thread.
            //
            // Non bloquant si ça échoue : une application sans tray reste
            // utilisable (au prix d'un `Stop-Process`), alors qu'aucune
            // application ne l'est du tout.
            let visibilite = tray::nouvelle_visibilite();
            let demande = rechargement::nouvelle_demande();

            // La boîte aux lettres du menu contextuel, vers la boucle 60 Hz.
            let commande = actions::nouvelle_commande();

            // Tout ce dont les DEUX menus ont besoin pour agir, en un seul
            // objet partagé — voir l'en-tête d'`actions.rs`.
            let actions = actions::Actions::nouvelles(
                visibilite.clone(),
                demande.clone(),
                dossier.clone(),
                nom_personnage.clone(),
                commande.clone(),
            );

            if let Err(e) = tray::installer(
                &app.handle().clone(),
                // Le REGISTRE et non la config : les deux divergent dès que
                // l'utilisateur retire l'entrée à la main ou par le
                // gestionnaire des tâches, et c'est le registre qui dit la
                // vérité.
                autostart::est_actif(),
                actions.clone(),
            ) {
                eprintln!("tray non installé : {e}");
            }

            // ── Deux crochets pour les vérifications qui demandent un clic ──
            //
            // Le tray a deux entrées dont l'effet ne se constate qu'en
            // cliquant : « Afficher » et « Quitter ». Chacune reçoit ici son
            // équivalent scriptable — même code appelé, sans humain. C'est la
            // même intention que le fichier témoin de rechargement juste
            // en dessous.

            // `SHIMEJI_CACHE=1` : démarre caché, comme si l'on avait décoché
            // « Afficher ». Sans ça, le gain de l'optimisation « ne rien
            // dessiner quand c'est caché » ne se mesure pas — et une
            // optimisation non mesurée est une croyance (voir CLAUDE.md).
            if std::env::var("SHIMEJI_CACHE").is_ok() {
                visibilite.store(false, std::sync::atomic::Ordering::Relaxed);
                tray::basculer_visibilite(&app.handle().clone(), false);
                println!("SHIMEJI_CACHE : démarré caché");
            }

            // `SHIMEJI_QUITTER_APRES=<secondes>` : appelle `exit(0)` — la
            // ligne exacte de l'entrée « Quitter » — au bout du délai.
            //
            // C'est la vérification qui **valide le retrait de la console** :
            // sans console, `Ctrl+C` n'existe plus, et une fenêtre sans
            // bordure, non focalisable, hors taskbar et hors Alt+Tab ne se
            // ferme par aucun moyen normal. Il faut donc prouver, et pas
            // supposer, qu'un processus GUI dans cet état sait bien se
            // terminer sur `exit`.
            if let Ok(valeur) = std::env::var("SHIMEJI_QUITTER_APRES") {
                // `parse` rend un `Result` : une valeur illisible ne doit pas
                // faire quitter tout de suite, ce qui ressemblerait à un
                // succès de la vérification alors qu'on n'a rien vérifié.
                match valeur.trim().parse::<u64>() {
                    Ok(secondes) => {
                        let handle_quitter = app.handle().clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_secs(secondes));
                            println!("SHIMEJI_QUITTER_APRES : exit(0)");
                            handle_quitter.exit(0);
                        });
                    }
                    Err(_) => eprintln!(
                        "SHIMEJI_QUITTER_APRES : « {valeur} » n'est pas un nombre de secondes"
                    ),
                }
            }

            // ── Rechargement déclenché par un fichier témoin ────────────
            //
            // Le rechargement se fait normalement par le tray, donc par un
            // clic. Pour qu'il soit **vérifiable sans humain** — et
            // scriptable —, on surveille aussi l'apparition d'un fichier
            // `recharger.txt` à côté des personnages : sa présence déclenche
            // un rechargement, puis il est supprimé.
            //
            // Trois lignes de plus dans la boucle, et c'est ce qui permet de
            // prouver que le rechargement à chaud marche vraiment plutôt que
            // de l'affirmer.
            let temoin = dossier.join("recharger.txt");

            // ── Les horloges ────────────────────────────────────────────
            let handle = app.handle().clone();
            let personnage = character::Character::new(
                manifeste,
                character::attach::Attachment::On {
                    platform: sol.id,
                    face: world::Face::Top,
                    offset,
                },
                depart,
            );

            // Diagnostic de performance : `SHIMEJI_SANS_BOUCLE=1` crée la
            // fenêtre et n'anime rien. C'est la seule façon de séparer ce que
            // coûte NOTRE boucle de ce que coûte l'existence d'une fenêtre
            // transparente toujours au premier plan — et sans cette mesure,
            // toute optimisation de la boucle est une croyance.
            if std::env::var("SHIMEJI_SANS_BOUCLE").is_ok() {
                println!("SHIMEJI_SANS_BOUCLE : aucune animation, fenêtre seule");
            } else {
                // La config complète est clonée pour la boucle : `setup`
                // continue de s'en servir plus haut (le tray, notamment), et
                // la boucle a besoin de sa propre copie pour la remplacer
                // au rechargement à chaud (Tâche 6).
                let configuration_boucle = configuration.clone();
                std::thread::spawn(move || {
                    boucle(
                    handle,
                    label,
                    personnage,
                    monde,
                    echelle_affichage,
                    visibilite,
                    reglages,
                    table,
                    echelle_config,
                    demande,
                    temoin,
                    nom_personnage,
                    configuration_boucle,
                    commande,
                );
                });
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("échec au lancement de l'application Tauri");
}

/// Sert un PNG de personnage pour le schéma `shime`.
///
/// `chemin` est de la forme `/blob/12`. On refuse tout ce qui n'a pas cette
/// forme exacte plutôt que de composer un chemin de fichier depuis une
/// chaîne arbitraire : un `..` dans l'URL ne doit pas pouvoir désigner un
/// fichier hors du dossier des personnages.
fn servir_frame(dossier: &std::path::Path, chemin: &str) -> tauri::http::Response<Vec<u8>> {
    let refus = |code: u16| {
        tauri::http::Response::builder()
            .status(code)
            .body(Vec::new())
            .expect("réponse vide toujours constructible")
    };

    // `trim_start_matches('/')` puis découpage : on attend exactement deux
    // segments.
    let segments: Vec<&str> = chemin.trim_start_matches('/').split('/').collect();
    if segments.len() != 2 {
        return refus(404);
    }

    let (personnage, numero) = (segments[0], segments[1]);

    // Le nom du personnage ne peut contenir que des caractères anodins, et
    // le numéro doit être un entier. Ces deux tests suffisent à interdire
    // tout `..` ou séparateur de chemin.
    let nom_sain = !personnage.is_empty()
        && personnage
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    let Ok(n) = numero.parse::<u32>() else {
        return refus(404);
    };
    if !nom_sain {
        return refus(404);
    }

    let fichier = dossier
        .join(personnage)
        .join("img")
        .join(format!("shime{n}.png"));

    // On ne trace que les ÉCHECS. Tracer les succès a servi une fois — c'est
    // ainsi qu'on a diagnostiqué le sprite invisible (voir `render::pousser`)
    // — mais à 8 requêtes par seconde la console devient illisible, et une
    // console illisible ne sert plus à diagnostiquer quoi que ce soit.
    //
    // Un 404 en revanche est toujours anormal : il signale un numéro de frame
    // du manifeste qui ne correspond à aucun fichier.
    if !fichier.is_file() {
        eprintln!("shime:// {chemin} -> 404 ({})", fichier.display());
    } else if std::env::var("SHIMEJI_TRACE").is_ok() {
        // Trace des succès, activée par SHIMEJI_TRACE=1. C'est ce qui a
        // permis de diagnostiquer le sprite invisible ; on la garde derrière
        // une variable d'environnement plutôt que de la supprimer, parce que
        // c'est le seul moyen de voir quelles frames sont VRAIMENT demandées.
        println!("shime:// {chemin} -> 200");
    }

    match std::fs::read(&fichier) {
        Ok(octets) => tauri::http::Response::builder()
            .status(200)
            .header("Content-Type", "image/png")
            // Les images ne changent pas pendant une exécution ; le cache du
            // webview évite de relire 46 fichiers en boucle. Le rechargement
            // à chaud du plan 1b changera le numéro de version dans l'URL
            // pour contourner ce cache.
            .header("Cache-Control", "max-age=3600")
            .body(octets)
            .expect("réponse constructible"),
        Err(_) => refus(404),
    }
}

/// La boucle 60 Hz : physique, comportement, rendu (spec §5.5).
///
/// Les trois horloges de la spec, dont deux sont ici :
///   · **60 Hz** — comportement, rendu, position
///   · **~8 Hz** — recensement du monde
///   · **~2 Hz** — les signaux (inactivité, appli active, heure, batterie,
///     verrouillage) — spec §5.5, design étape 2 §9
/// Le personnage est-il déjà en train d'émerger ?
///
/// Sert d'anti-rebond au déverrouillage (voir le point d'appel). Rendre vrai
/// empêche de relancer un réveil déjà commencé.
///
/// `matches!` : on ne veut lire que la phase, sans démonter l'intention ni la
/// reconstruire. C'est la même forme que l'interruption de `behavior::mod`.
fn deja_en_reveil(ch: &character::Character) -> bool {
    matches!(
        ch.intention,
        Some(behavior::intention::ActiveIntention {
            etat: behavior::intention::EtatIntention::Repos {
                phase: behavior::intention::PhaseRepos::Selevant,
                ..
            },
            ..
        })
    )
}

fn boucle(
    handle: tauri::AppHandle,
    label: String,
    mut ch: character::Character,
    mut monde: world::World,
    mut echelle_affichage: f32,
    visibilite: tray::Visibilite,
    mut reglages: config::Reglages,
    mut table: behavior::desire::TableEnvies,
    // Le réglage `echelle` de la config, gardé à part pour le recombiner à
    // l'échelle du moniteur au recensement — celle-ci peut changer si le
    // personnage passe sur un écran de DPI différent.
    mut echelle_config: f32,
    demande: rechargement::Demande,
    temoin: std::path::PathBuf,
    nom_personnage: String,
    // La Config complète (option 1 du brief de la Tâche 6) : `signals::biais_de`
    // a besoin de la table des applications, que `Reglages` n'expose pas.
    // Une variable globale aurait été plus courte à écrire, mais la spec
    // §10.2 l'interdit — et ça rendrait `biais_de` intestable en dehors de
    // cette boucle.
    mut config_courante: config::Config,
    // La boîte aux lettres par laquelle l'envie choisie au menu contextuel
    // revient jusqu'ici. La boucle n'a PAS besoin d'`Actions` : construire le
    // menu ne déclenche rien, et le clic part dans la boucle d'événements de
    // Tauri jusqu'à l'unique gestionnaire installé par `tray.rs`.
    commande: actions::Commande,
) {
    // Le dossier des personnages, pour le rechargement par témoin.
    let dossier_boucle = config::dossier_personnages();
    use behavior::Entrees;
    use std::time::{Duration, Instant};
    // `get_webview_window` est une méthode du trait `Manager` : sans cet
    // import, l'`AppHandle` ne l'expose pas.
    use tauri::Manager;

    let sonde = probe::win32::Win32Probe::new();
    let horloge = clock::SystemClock::new();

    // `SHIMEJI_ESCALADE=1` : force l'intention `Grimper` dès la première
    // image, au lieu d'attendre qu'elle sorte du tirage pondéré (elle partage
    // aujourd'hui le poids de `se_reposer`, donc l'attendre à l'œil peut
    // prendre plusieurs minutes).
    //
    // Ajoutée à la Tâche 7 pour une raison précise : mesurer l'ancre de
    // `grabWall`/`climbWall` demande de REGARDER le personnage accroché à un
    // mur, et ça, aucun script ne peut le faire à la place d'un humain — la
    // seule exception du projet à « tout ce qui demanderait un clic reçoit un
    // équivalent scriptable » (voir CLAUDE.md). Mais le TRAJET jusqu'à ce
    // moment-là, lui, se scripte très bien : cette variable évite à l'auteur
    // d'ouvrir le menu contextuel et de choisir « Grimper » à la main, et la
    // trace ci-dessous (à chaque changement de phase) lui dit quand regarder
    // l'écran sans avoir à fixer le personnage pendant plusieurs minutes.
    let trace_escalade = std::env::var("SHIMEJI_ESCALADE").is_ok();
    if trace_escalade {
        ch.intention = Some(behavior::intention::ActiveIntention::nouvelle(
            behavior::intention::Intention::Grimper,
            horloge.elapsed(),
        ));
        println!("SHIMEJI_ESCALADE : intention Grimper forcée au démarrage");
    }
    // Le dernier triplet (phase, face, pose) imprimé : on ne retrace qu'au
    // CHANGEMENT, sinon la console serait inondée à 60 Hz pour une
    // information qui ne bouge qu'à la transition.
    let mut derniere_trace_grimpe: Option<(String, String, String)> = None;

    // Graine issue de l'horloge système : deux lancements ne doivent pas
    // donner la même histoire. C'est le seul endroit du programme où
    // l'aléatoire n'est pas reproductible, et c'est voulu — le mode
    // simulation, lui, prend une graine explicite.
    let graine = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345);
    let mut rng = rng::XorShift32::seeded(graine);

    const PERIODE: Duration = Duration::from_micros(16_667); // 60 Hz
    const PERIODE_MONDE: Duration = Duration::from_millis(125); // 8 Hz
    const PERIODE_SIGNAUX: Duration = Duration::from_millis(500); // 2 Hz

    // Pendant ce délai après le démarrage, on pousse la frame à CHAQUE
    // image, sans comparer.
    //
    // C'est une course au démarrage : `pet.js` n'a pas encore posé son
    // écouteur pendant le chargement de la page, et un message émis avant
    // est perdu. Si le personnage restait immobile pendant les premières
    // secondes, rien ne s'afficherait — écran vide, sans erreur, et le
    // diagnostic partirait chercher un problème de transparence.
    const AMORCAGE: Duration = Duration::from_secs(2);

    let mut dernier_recensement = Duration::ZERO;
    let mut dernier_signal = Duration::ZERO;

    // Le biais courant, recalculé à 2 Hz et transporté à 60 Hz.
    //
    // Neutre au démarrage : la première demi-seconde, le personnage se
    // comporte comme à l'étape 1. Rien à corriger — attendre les signaux
    // avant de bouger serait une demi-seconde de figement au lancement.
    let mut biais = signals::Biais::neutre();
    let mut utilisateur_actif = true;

    // La session est-elle verrouillée ? Mémorisé pour ne basculer les
    // fenêtres que sur CHANGEMENT.
    let mut verrouille = false;

    // Diagnostic : `SHIMEJI_SIGNAUX=1`.
    let trace_signaux = std::env::var("SHIMEJI_SIGNAUX").is_ok();

    // L'échelle du moniteur, séparée du réglage de la config : le
    // rechargement à chaud change le second sans redemander le premier.
    let mut ecrans_echelle = if echelle_config != 0.0 {
        echelle_affichage / echelle_config
    } else {
        1.0
    };
    let mut dernier_rendu: Option<render::Rendu> = None;
    let mut derniere_taille: Option<(u32, u32)> = None;

    // La position ENTIÈRE effectivement posée à la dernière image.
    //
    // `set_position` ne prend que des entiers : deux positions flottantes qui
    // s'arrondissent au même pixel produisent exactement le même appel. À
    // l'arrêt — environ trois tirages d'allure sur dix — la position ne
    // change pas du tout, et on épargne 60 appels système par seconde.
    let mut dernier_coin: Option<(i32, i32)> = None;

    // Diagnostic de cadence, voir plus bas.
    let trace_cadence = std::env::var("SHIMEJI_CADENCE").is_ok();
    let mut images_depuis_trace: u32 = 0;
    let mut placements_depuis_trace: u32 = 0;
    let mut travail_cumule = Duration::ZERO;
    let mut derniere_trace = Duration::ZERO;

    // L'état courant de la traversée des clics. Initialisé à `true` parce
    // que c'est ce que `setup` a posé juste avant de lancer ce thread.
    let mut clics_traversent = true;

    // Le bouton droit était-il enfoncé à l'image précédente ?
    //
    // C'est ce qui transforme un état — « le bouton est enfoncé », vrai
    // pendant les ~15 images que dure un clic humain — en un **front**, qui
    // n'arrive qu'une fois. Sans lui, maintenir le bouton rouvrirait le menu
    // en boucle dès sa fermeture.
    let mut bouton_droit_precedent = false;

    loop {
        // `Instant` ici et non l'horloge injectée : c'est la CADENCE, pas le
        // temps du comportement. La distinction compte — le comportement doit
        // rester pilotable par une horloge factice (spec §10.2).
        let debut = Instant::now();
        let maintenant = horloge.elapsed();

        // ── ~2 Hz : les signaux (spec §5.5, design étape 2 §9) ──────────
        //
        // Cinq appels système toutes les 500 ms. À comparer aux 60
        // `SetWindowPos` par seconde qui coûtent 11 points de CPU : c'est du
        // bruit. Mesuré quand même — voir `CLAUDE.md`.
        if maintenant.saturating_sub(dernier_signal) >= PERIODE_SIGNAUX {
            let s = sonde.signaux();

            // Le biais : de la donnée pure, calculée par une fonction pure.
            biais = signals::biais_de(&s, &config_courante);

            // « Actif » se dérive du MÊME seuil que le biais, pour qu'il soit
            // impossible d'être « actif » et « inactif » dans la même image.
            // La fonction vit dans `signals.rs`, à côté de `biais_de`, et pas
            // recopiée ici : c'est la SEULE définition de « actif », partagée
            // avec `sim.rs` — sans quoi les deux finiraient par diverger.
            utilisateur_actif = signals::utilisateur_actif(&s, &config_courante);

            // ── Le verrouillage : le quatrième réflexe ──────────────────
            //
            // C'est un réflexe au sens du design (décision n° 5) : non
            // négociable, immédiat, on ne biaise pas un poids pour
            // disparaître d'un écran de verrouillage.
            //
            // Mais il ne s'implémente PAS dans `reflex.rs`, et c'est
            // délibéré : son effet porte sur la FENÊTRE, pas sur l'accroche
            // du personnage. `reflex.rs` ne connaît ni Tauri ni le tray, et
            // c'est ce qui le garde testable sans écran.
            if s.session_verrouillee != verrouille {
                verrouille = s.session_verrouillee;

                // ── Au retour : il émerge ───────────────────────────
                //
                // Il a « dormi » pendant que la session était verrouillée —
                // ce qui est littéralement vrai : la boucle tournait, mais
                // rien n'était dessiné.
                //
                // C'est une INTENTION qu'on pose, pas un décor peint
                // par-dessus le rendu. La pose et sa durée vivent donc dans
                // `intention.rs`, avec toutes les autres, et **le rendu ne
                // connaît toujours aucun numéro de frame** — ce qui est ce
                // qui permet au réveil de marcher sur un pack tiers dont la
                // numérotation n'est pas celle du blob.
                //
                // Le garde `deja_en_reveil` est une simple précaution :
                // relancer depuis zéro un réveil déjà commencé n'aurait aucun
                // sens, quelle qu'en soit la raison.
                //
                // ⚠️ Il ne compense PAS un rebond de la sonde, contrairement à
                // ce qu'une première hypothèse supposait. On avait cru que
                // `WTSSessionInfoEx`, sondée à 2 Hz, repassait transitoirement
                // par LOCK pendant la bascule de bureau et faisait voir DEUX
                // déverrouillages. **La console dit non** : un cycle complet
                // n'imprime qu'un « verrouillée » et qu'un « déverrouillée ».
                // Le vrai « réveil joué deux fois » venait de l'ordre des
                // frames — voir `POSE_WAKE`.
                if !verrouille && !deja_en_reveil(&ch) {
                    ch.intention = Some(behavior::intention::ActiveIntention::reveil(maintenant));
                }

                // On ne rend visible que si l'utilisateur n'avait pas
                // lui-même décoché « Afficher » : le déverrouillage ne doit
                // pas défaire son choix.
                let voulu = visibilite.load(std::sync::atomic::Ordering::Relaxed);
                tray::basculer_visibilite(&handle, voulu && !verrouille);

                println!(
                    "session {} — personnage {}",
                    if verrouille { "verrouillée" } else { "déverrouillée" },
                    if verrouille { "planqué" } else { "il émerge" }
                );
            }

            if trace_signaux {
                println!(
                    "signaux : inactif {:.0} s · {} · {} h · batterie {} · verrouillé {} \
                     → flâner ×{:.2} reposer ×{:.2} jouer ×{:.2}",
                    s.inactivite.as_secs_f32(),
                    s.appli_active.as_deref().unwrap_or("-"),
                    s.heure,
                    match s.batterie.pourcent {
                        Some(p) => format!("{p} %"),
                        None => "-".to_string(),
                    },
                    s.session_verrouillee,
                    biais.flaner,
                    biais.se_reposer,
                    biais.jouer,
                );
            }

            dernier_signal = maintenant;
        }

        // ── ~8 Hz : recenser le monde ───────────────────────────────────
        // À l'étape 1 c'est la liste des écrans ; à l'étape 4 s'y ajouteront
        // les fenêtres, leur filtrage et l'occlusion.
        if maintenant.saturating_sub(dernier_recensement) >= PERIODE_MONDE {
            let ecrans = sonde.screens();
            if !ecrans.is_empty() {
                monde = world::World::from_screens(&ecrans);
                // Gardée à part : le rechargement à chaud doit pouvoir
                // recombiner l'échelle du moniteur avec la NOUVELLE échelle
                // de la config, sans redemander les écrans.
                ecrans_echelle = ecrans[0].scale;
                echelle_affichage = ecrans_echelle * echelle_config;
            }

            // Le fichier témoin : présent → on demande un rechargement, et
            // on le retire pour ne pas boucler.
            if temoin.exists() {
                let _ = std::fs::remove_file(&temoin);
                match rechargement::preparer(&demande, &dossier_boucle, &nom_personnage) {
                    Ok(v) => println!("rechargement demandé par témoin (version {v})"),
                    Err(e) => eprintln!("rechargement impossible : {e}"),
                }
            }

            // ── Une demande de rechargement en attente ? ────────────────
            //
            // `try_lock` et non `lock` : la boucle 60 Hz ne doit JAMAIS
            // attendre. Si le tray tient le verrou à cet instant, on
            // réessaiera dans 125 ms et personne ne s'en apercevra.
            if let Ok(mut boite) = demande.try_lock() {
                // `take()` vide la boîte en récupérant son contenu : la
                // demande est consommée atomiquement, sans drapeau à
                // remettre à zéro.
                if let Some(r) = boite.take() {
                    // La pose courante existe-t-elle encore dans le nouveau
                    // manifeste ? Si l'utilisateur vient de la renommer ou de
                    // la retirer, `set_pose` refuserait tout changement et le
                    // personnage resterait figé sur une clé morte.
                    if !r.manifeste.has_pose(&ch.pose) {
                        ch.pose = character::manifest::POSE_STAND.to_string();
                        ch.pose_depuis = maintenant;
                    }

                    ch.manifest = r.manifeste;
                    reglages = r.reglages;
                    table = r.table;
                    echelle_config = r.echelle_config;
                    config_courante = r.config;
                    echelle_affichage = ecrans_echelle * echelle_config;

                    // Le webview doit oublier ses images, et la taille de la
                    // fenêtre peut avoir changé (`frameSize`, `scale`).
                    let _ = render::recharger(&handle, &label, r.version);
                    derniere_taille = None;
                    dernier_rendu = None;
                    dernier_coin = None;

                    println!("personnage rechargé (version {})", r.version);
                }
            }

            dernier_recensement = maintenant;
        }

        // ── 60 Hz : les entrées, et le hit-testing (spec §3.3) ──────────
        // La spec §3.3 propose ~30 Hz pour `GetCursorPos`. On le lit à 60 Hz :
        // l'appel est effectivement quasi gratuit, et à 30 Hz le personnage
        // traînerait visiblement derrière le curseur pendant un glisser.
        let m = sonde.mouse();

        // Le curseur est-il dans la hitbox de la POSE COURANTE — et non dans
        // la boîte de 128×128 ? Sans cette distinction, le personnage serait
        // un trou noir de 128 px avalant les clics dans ses zones
        // transparentes.
        let sur_le_personnage = match (
            character::attach::world_position(&ch.attachment, &monde, m.pos),
            ch.manifest.pose(&ch.pose),
        ) {
            (Some(pos), Some(pose)) => character::attach::hitbox_ecran(
                pos,
                &ch.pose,
                pose,
                &ch.manifest,
                echelle_affichage,
                ch.facing,
            )
            .contains(m.pos),

            // Position indérivable (plateforme disparue) ou pose absente :
            // on ne peut pas savoir. `false` est le bon défaut — les clics
            // continuent de traverser, ce qui ne gêne personne.
            _ => false,
        };

        // ── Absorber le clic, mais seulement là où il faut ──────────────
        //
        // Pourquoi désactiver la traversée alors que la sonde nous dit déjà
        // tout ? Parce que si les clics continuaient de traverser, cliquer
        // sur le personnage cliquerait **aussi** l'icône du bureau derrière
        // lui. Il faut ABSORBER le clic — c'est à ça que sert le va-et-vient
        // de `set_ignore_cursor_events` (spec §3.3).
        //
        // Pendant un glisser, on garde les clics absorbés même si le sprite
        // a glissé hors de sa propre hitbox : sinon un déplacement rapide
        // relâcherait le personnage tout seul.
        let porte = matches!(ch.attachment, character::attach::Attachment::Dragged);
        let doit_traverser = !sur_le_personnage && !porte;

        // On n'appelle Win32 que sur CHANGEMENT d'état : appeler
        // `set_ignore_cursor_events` 60 fois par seconde marcherait, mais
        // c'est un appel système par image pour rien — et la section
        // « Mesurer le CPU » de CLAUDE.md dit pourquoi on y regarde.
        if doit_traverser != clics_traversent {
            if render::traverser_les_clics(&handle, &label, doit_traverser).is_err() {
                return;
            }
            clics_traversent = doit_traverser;
        }

        // ── Clic droit sur le personnage : le menu contextuel ───────────
        //
        // Front **descendant** (le bouton vient d'être RELÂCHÉ) ET curseur
        // dans la hitbox : un clic droit sur le bureau à côté de lui ne doit
        // rien ouvrir. Le test de hitbox est le MÊME que celui qui absorbe
        // les clics gauches, donc la zone cliquable est exactement celle
        // qu'on voit.
        //
        // ⚠️ **Au relâchement et non à l'enfoncement**, et pour deux raisons
        // qui pointent dans le même sens :
        //
        // 1. c'est la convention de Windows — l'explorateur, comme toute
        //    application, ouvre son menu contextuel sur `WM_RBUTTONUP` ;
        // 2. ouvrir au bouton encore enfoncé lance `TrackPopupMenu` pendant
        //    que Windows suit toujours un clic droit en cours. Le menu hérite
        //    alors d'un suivi de souris qui ne lui appartient pas, et se
        //    referme mal — ce qu'on a justement cherché à corriger ici.
        let front_descendant_droit = !m.right_down && bouton_droit_precedent;
        bouton_droit_precedent = m.right_down;

        if front_descendant_droit && sur_le_personnage {
            // `let … else` : si la fenêtre a été fermée, on sort du thread.
            // Équivalent d'un `match` dont la branche `None` ferait `return`.
            let Some(win) = handle.get_webview_window(&label) else {
                return;
            };

            // **Cet appel bloque** jusqu'à la fermeture du menu : le
            // personnage s'immobilise pendant ce temps, ce qui est voulu
            // (voir `menu_perso::ouvrir`).
            if let Err(e) = menu_perso::ouvrir(&handle, &win, &ch.manifest, &table) {
                eprintln!("menu du personnage : {e}");
            }

            // On repart sur une image neuve plutôt que de finir celle-ci :
            // `maintenant` a été lu AVANT le menu, il a donc plusieurs
            // secondes de retard, et `dt` vaut toujours 16,7 ms. Poursuivre
            // ferait juger toutes les échéances (délai d'abandon, durée de
            // pose) sur un instant périmé.
            continue;
        }

        // La commande éventuellement déposée par le gestionnaire de menu.
        //
        // `try_lock` et non `lock` : à 60 Hz on ne s'autorise jamais à
        // attendre un verrou. S'il est pris à cet instant, la commande sera
        // lue à l'image suivante, 16 ms plus tard — invisible.
        //
        // `take()` vide la boîte en récupérant son contenu : la commande est
        // ainsi consommée une fois et une seule.
        let commande_du_menu = match commande.try_lock() {
            Ok(mut boite) => boite.take(),
            Err(_) => None,
        };

        let entrees = Entrees {
            souris: m.pos,
            echelle_affichage,
            bouton_gauche: m.left_down,
            curseur_sur_le_personnage: sur_le_personnage,
            // Recalculés à 2 Hz ci-dessus, transportés tels quels à 60 Hz.
            biais,
            utilisateur_actif,
            commande: commande_du_menu,
        };

        // ── 60 Hz : le comportement ─────────────────────────────────────
        // La MÊME fonction que le mode simulation.
        let dt = PERIODE.as_secs_f32();
        behavior::pas(&mut ch, &monde, &entrees, &table, &reglages, maintenant, dt, &mut rng);

        // ── Diagnostic : `SHIMEJI_ESCALADE=1` ───────────────────────────
        // Rien qu'une intention `Grimper` en cours ne trace : c'est
        // exactement le moment que l'auteur doit regarder pour mesurer
        // l'ancre de `grabWall`/`climbWall` à l'œil.
        if trace_escalade {
            if let Some(ai) = ch.intention {
                if let behavior::intention::EtatIntention::Grimpe { phase, .. } = ai.etat {
                    // Le MESSAGE affiché garde l'offset — il aide à situer le
                    // personnage sur la paroi au moment précis où la trace
                    // sort. Voir plus bas pourquoi la CLÉ, elle, ne le
                    // contient plus.
                    let face_affichee = match ch.attachment {
                        character::attach::Attachment::On { face, offset, .. } => {
                            format!("{face:?} offset={offset:.1}")
                        }
                        character::attach::Attachment::Falling { .. } => "chute".to_string(),
                        character::attach::Attachment::Dragged => "porté".to_string(),
                    };

                    // ⚠️ **La clé de dédoublonnage NE CONTIENT PAS l'offset**
                    // (correction de la relecture finale, point 4) : seul le
                    // nom de la face y entre, sans sa valeur numérique.
                    //
                    // L'offset avance en continu pendant `Rejoindre` et
                    // `Paroi` (jusqu'à 0,83 px par image), donc l'inclure
                    // dans la clé la faisait changer à presque CHAQUE image :
                    // la trace sortait à ~60 lignes par seconde, alors que ce
                    // commentaire promet « à chaque changement de phase ».
                    // Inutilisable pour ce à quoi cette trace sert : dire à
                    // l'auteur QUAND regarder l'écran pour mesurer l'ancre de
                    // `grabWall`/`climbWall` — la seule vérification de ce
                    // projet qui reste manuelle (voir plus haut, à la
                    // déclaration de `trace_escalade`). La clé ne porte donc
                    // que ce qui identifie une PHASE, pas une position dans
                    // cette phase.
                    let face_pour_la_cle = match ch.attachment {
                        character::attach::Attachment::On { face, .. } => format!("{face:?}"),
                        character::attach::Attachment::Falling { .. } => "chute".to_string(),
                        character::attach::Attachment::Dragged => "porté".to_string(),
                    };
                    let cle = (format!("{phase:?}"), face_pour_la_cle, ch.pose.clone());
                    if derniere_trace_grimpe.as_ref() != Some(&cle) {
                        println!(
                            "SHIMEJI_ESCALADE : phase={:?} {} pose={}",
                            phase, face_affichee, ch.pose
                        );
                        derniere_trace_grimpe = Some(cle);
                    }
                }
            }
        }

        // ── Sur changement seulement : la taille de la fenêtre ──────────
        // Elle ne dépend que du manifeste et de l'échelle de l'écran.
        // L'appeler à 60 Hz coûtait 8 points de pourcentage de CPU pour
        // rien (voir l'avertissement de `render::placer`).
        let taille = character::attach::window_size(&ch.manifest, echelle_affichage);
        if derniere_taille != Some(taille) {
            if render::dimensionner(&handle, &label, taille).is_err() {
                return;
            }
            derniere_taille = Some(taille);
        }

        // ── Caché par l'utilisateur, OU session verrouillée : rien à dessiner ──
        //
        // Le comportement, lui, continue de tourner : il doit avancer pour
        // qu'on le retrouve ailleurs en le réaffichant, et c'est du calcul
        // pur — mesuré à 100 µs par image quand il ne se passe rien.
        //
        // Ce qui coûte, c'est `SetWindowPos` sur une fenêtre en couche (voir
        // la section « Mesurer le CPU » de CLAUDE.md), et c'est exactement ce
        // qu'on saute ici. Le verrouillage emprunte EXACTEMENT ce même chemin,
        // déjà mesuré à 0,9 % : c'est donc aussi la première des pistes CPU
        // restantes, et elle se referme ici.
        //
        // `Ordering::Relaxed` : il n'y a aucune autre donnée à synchroniser
        // avec ce booléen, seulement sa propre valeur. Un ordre plus fort
        // n'apporterait qu'un coût.
        let visible = visibilite.load(std::sync::atomic::Ordering::Relaxed) && !verrouille;

        if !visible {
            // On oublie ce qu'on avait posé : au retour, il faut tout
            // repousser, la fenêtre ayant pu être masquée entre-temps.
            dernier_coin = None;
            dernier_rendu = None;

            let ecoule = debut.elapsed();
            if let Some(reste) = PERIODE.checked_sub(ecoule) {
                std::thread::sleep(reste);
            }
            continue;
        }

        // ── 60 Hz : le rendu ────────────────────────────────────────────
        // La position est DÉRIVÉE à chaque image (décision n° 1).
        if let Some(pos) = character::attach::world_position(&ch.attachment, &monde, m.pos) {
            if let Some(pose) = ch.manifest.pose(&ch.pose) {
                let coin = character::attach::window_top_left(
                    pos,
                    pose,
                    &ch.manifest,
                    echelle_affichage,
                    ch.facing,
                );

                // Arrondi ici et non dans `placer` : c'est cet entier qu'on
                // compare, et le calculer deux fois serait deux occasions de
                // divergence.
                let coin_entier = (coin.x.round() as i32, coin.y.round() as i32);

                if dernier_coin != Some(coin_entier) {
                    if render::placer(&handle, &label, coin).is_err() {
                        // Fenêtre fermée : plus rien à faire dans ce thread.
                        return;
                    }
                    dernier_coin = Some(coin_entier);
                    placements_depuis_trace += 1;
                }
            }
        }

        let rendu = render::Rendu {
            image: ch.frame_courante(maintenant),
            flip: ch.facing.flipped(),
        };

        // N'émettre que sur changement — sauf pendant l'amorçage, où
        // l'écouteur du webview n'existe peut-être pas encore.
        //
        // À 60 Hz, une pose de marche ne change d'image que ~8 fois par
        // seconde : on économise ~85 % des messages, sans une ligne de
        // logique côté front.
        let amorcage = maintenant < AMORCAGE;
        if amorcage || dernier_rendu != Some(rendu) {
            if let Err(e) = render::pousser(&handle, &label, rendu) {
                // On imprime avant de sortir : un thread qui meurt en silence
                // donne un personnage figé sans explication, et c'est
                // exactement ce qu'on vient de passer du temps à diagnostiquer.
                eprintln!("rendu impossible, arrêt de la boucle : {e}");
                return;
            }
            dernier_rendu = Some(rendu);
        }

        // ── Diagnostic de cadence ───────────────────────────────────────
        // `SHIMEJI_CADENCE=1` imprime toutes les 5 s les images par seconde
        // réellement atteintes et le temps de travail par image. C'est la
        // seule façon de savoir si la boucle DORT ou si elle tourne à plat :
        // quand le travail dépasse la période, `checked_sub` rend `None` et
        // il n'y a aucun sommeil du tout.
        if trace_cadence {
            images_depuis_trace += 1;
            travail_cumule += debut.elapsed();
            if maintenant.saturating_sub(derniere_trace) >= Duration::from_secs(5) {
                let secondes = maintenant.saturating_sub(derniere_trace).as_secs_f64();
                let moyenne_us = travail_cumule.as_micros() as f64 / images_depuis_trace as f64;
                println!(
                    "cadence : {:.1} img/s, travail moyen {:.0} µs, {} placements sur {} images ({:.0} %)",
                    images_depuis_trace as f64 / secondes,
                    moyenne_us,
                    placements_depuis_trace,
                    images_depuis_trace,
                    placements_depuis_trace as f64 * 100.0 / images_depuis_trace as f64
                );
                placements_depuis_trace = 0;
                images_depuis_trace = 0;
                travail_cumule = Duration::ZERO;
                derniere_trace = maintenant;
            }
        }

        // ── Tenir la cadence ────────────────────────────────────────────
        // `checked_sub` : si une image a pris plus de 16,7 ms (machine
        // chargée), `PERIODE - ecoule` déborderait. On enchaîne alors
        // immédiatement plutôt que de dormir une éternité.
        let ecoule = debut.elapsed();
        if let Some(reste) = PERIODE.checked_sub(ecoule) {
            std::thread::sleep(reste);
        }
    }
}
