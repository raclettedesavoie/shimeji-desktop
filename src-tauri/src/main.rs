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

mod behavior;
mod character;
mod clock;
mod geom;
mod probe;
mod render;
mod rng;
mod sim;
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

        let dossier = dossier_personnages().join("blob");
        println!(
            "simulation de {minutes} min, graine {graine}, personnage {}",
            dossier.display()
        );

        match sim::executer(minutes, graine, &dossier) {
            Ok(r) => sim::imprimer(&r),
            Err(e) => {
                eprintln!("simulation impossible : {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    lancer_application();
}

/// Où sont les personnages.
///
/// Ordre de recherche (spec §8.1) : à côté de l'exe, puis `%APPDATA%`.
///
/// **Plus un repli de développement** au milieu : en `cargo run`, l'exe est
/// dans `src-tauri/target/debug/`, donc « à côté de l'exe » ne trouve rien et
/// l'on tomberait sur `%APPDATA%`, vide. On remonte donc les dossiers parents
/// à la recherche d'un `characters/`. Ce repli disparaîtra si un jour il
/// gêne ; pour l'instant il évite de copier 46 PNG à chaque build.
///
/// Le plan 1b déplacera cette fonction dans `config.rs`, qui résout de la
/// même façon `config.json`.
fn dossier_personnages() -> std::path::PathBuf {
    // ── À côté de l'exe ────────────────────────────────────────────────
    // `if let Ok(...)` : `current_exe` peut échouer sur des systèmes
    // exotiques. Ce n'est pas une raison de ne pas démarrer.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidat = dir.join("characters");
            if candidat.is_dir() {
                return candidat;
            }

            // ── Repli de développement ─────────────────────────────────
            // `ancestors()` énumère le dossier puis chacun de ses parents.
            // On s'arrête à 5 niveaux : assez pour sortir de target/debug/,
            // pas assez pour partir explorer tout le disque.
            for parent in dir.ancestors().take(5) {
                let candidat = parent.join("characters");
                if candidat.is_dir() {
                    return candidat;
                }
            }
        }
    }

    // ── %APPDATA% ──────────────────────────────────────────────────────
    if let Ok(appdata) = std::env::var("APPDATA") {
        return std::path::PathBuf::from(appdata)
            .join("shimeji-desktop")
            .join("characters");
    }

    // Dernier recours : le dossier courant. Ça échouera au chargement du
    // manifeste, avec un message qui nomme le chemin cherché — ce qui est
    // exactement ce qu'il faut pour diagnostiquer.
    std::path::PathBuf::from("characters")
}

/// Le label de la fenêtre d'un personnage. Un seul personnage à l'étape 1a ;
/// l'étape 3 en instanciera plusieurs, d'où l'index dès maintenant.
fn label_de(index: usize) -> String {
    format!("pet-{index}")
}

fn lancer_application() {
    let dossier = dossier_personnages();
    println!("personnages : {}", dossier.display());

    // ── Le manifeste, avant tout le reste ───────────────────────────────
    // Sans personnage, il n'y a rien à afficher : autant échouer tout de
    // suite avec un message clair que d'ouvrir une fenêtre vide.
    let manifeste = match character::manifest::Manifest::load(&dossier.join("blob")) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("personnage « blob » illisible : {e}");
            std::process::exit(1);
        }
    };
    println!(
        "personnage chargé : {} ({} poses)",
        manifeste.name,
        manifeste.poses.len()
    );

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
            let sol = &monde.platforms()[0];
            let offset = sol.rect.face_length(world::Face::Top) / 2.0;
            let depart = sol.rect.point_on(world::Face::Top, offset);

            let echelle_ecran = ecrans[0].scale;
            let taille = character::attach::window_size(&manifeste, echelle_ecran);

            // ── La fenêtre ──────────────────────────────────────────────
            // Exactement la combinaison validée par l'étape 0, plus les deux
            // styles étendus qu'elle a révélés manquants.
            let label = label_de(0);
            let win = tauri::WebviewWindowBuilder::new(
                app,
                &label,
                // Le fragment `#blob` dit à `pet.js` quel personnage servir.
                tauri::WebviewUrl::App("index.html#blob".into()),
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

            std::thread::spawn(move || {
                boucle(handle, label, personnage, monde, echelle_ecran);
            });

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
///   · **~2 Hz** — les signaux : étape 2, pas encore
fn boucle(
    handle: tauri::AppHandle,
    label: String,
    mut ch: character::Character,
    mut monde: world::World,
    mut echelle_ecran: f32,
) {
    use behavior::{desire::TableEnvies, Entrees};
    use std::time::{Duration, Instant};

    let sonde = probe::win32::Win32Probe::new();
    let horloge = clock::SystemClock::new();

    // Graine issue de l'horloge système : deux lancements ne doivent pas
    // donner la même histoire. C'est le seul endroit du programme où
    // l'aléatoire n'est pas reproductible, et c'est voulu — le mode
    // simulation, lui, prend une graine explicite.
    let graine = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345);
    let mut rng = rng::XorShift32::seeded(graine);

    let table = TableEnvies::defaut();

    const PERIODE: Duration = Duration::from_micros(16_667); // 60 Hz
    const PERIODE_MONDE: Duration = Duration::from_millis(125); // 8 Hz

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
    let mut dernier_rendu: Option<render::Rendu> = None;
    let mut derniere_taille: Option<(u32, u32)> = None;

    loop {
        // `Instant` ici et non l'horloge injectée : c'est la CADENCE, pas le
        // temps du comportement. La distinction compte — le comportement doit
        // rester pilotable par une horloge factice (spec §10.2).
        let debut = Instant::now();
        let maintenant = horloge.elapsed();

        // ── ~8 Hz : recenser le monde ───────────────────────────────────
        // À l'étape 1 c'est la liste des écrans ; à l'étape 4 s'y ajouteront
        // les fenêtres, leur filtrage et l'occlusion.
        if maintenant.saturating_sub(dernier_recensement) >= PERIODE_MONDE {
            let ecrans = sonde.screens();
            if !ecrans.is_empty() {
                monde = world::World::from_screens(&ecrans);
                echelle_ecran = ecrans[0].scale;
            }
            dernier_recensement = maintenant;
        }

        // ── 60 Hz : les entrées ─────────────────────────────────────────
        // La spec §3.3 propose ~30 Hz pour `GetCursorPos`. On le lit à 60 Hz :
        // l'appel est effectivement quasi gratuit, et à 30 Hz le personnage
        // traînerait visiblement derrière le curseur pendant un glisser.
        //
        // `curseur_sur_le_personnage` reste `false` : le hit-testing arrive
        // en Tâche 11.
        let m = sonde.mouse();
        let entrees = Entrees {
            souris: m.pos,
            bouton_gauche: m.left_down,
            curseur_sur_le_personnage: false,
        };

        // ── 60 Hz : le comportement ─────────────────────────────────────
        // La MÊME fonction que le mode simulation.
        let dt = PERIODE.as_secs_f32();
        behavior::pas(&mut ch, &monde, &entrees, &table, maintenant, dt, &mut rng);

        // ── Sur changement seulement : la taille de la fenêtre ──────────
        // Elle ne dépend que du manifeste et de l'échelle de l'écran.
        // L'appeler à 60 Hz coûtait 8 points de pourcentage de CPU pour
        // rien (voir l'avertissement de `render::placer`).
        let taille = character::attach::window_size(&ch.manifest, echelle_ecran);
        if derniere_taille != Some(taille) {
            if render::dimensionner(&handle, &label, taille).is_err() {
                return;
            }
            derniere_taille = Some(taille);
        }

        // ── 60 Hz : le rendu ────────────────────────────────────────────
        // La position est DÉRIVÉE à chaque image (décision n° 1).
        if let Some(pos) = character::attach::world_position(&ch.attachment, &monde, m.pos) {
            if let Some(pose) = ch.manifest.pose(&ch.pose) {
                let coin = character::attach::window_top_left(
                    pos,
                    pose,
                    &ch.manifest,
                    echelle_ecran,
                    ch.facing,
                );

                if render::placer(&handle, &label, coin).is_err() {
                    // Fenêtre fermée : plus rien à faire dans ce thread.
                    return;
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
