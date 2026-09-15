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
mod apparition;
mod autostart;
mod behavior;
mod catalogue;
mod character;
mod clock;
mod commandes;
mod config;
mod geom;
mod menu_perso;
mod probe;
mod rechargement;
mod render;
mod rng;
mod roster;
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

    // ── `--installer <slug>` ────────────────────────────────────────────
    //
    // L'équivalent scriptable du clic dans la grille du catalogue, selon la
    // règle du projet : tout ce qui demanderait un clic reçoit un équivalent
    // en ligne de commande.
    //
    // Il vérifie en prime le seul morceau que les tests ne couvrent pas —
    // **que le CDN réponde bien ce qu'on croit**, et que `ReseauWinHttp`
    // sache lui parler. Les tests, eux, ne voient que le faux réseau.
    if let Some(pos) = args.iter().position(|a| a == "--installer") {
        let Some(slug) = args.get(pos + 1) else {
            eprintln!("usage : --installer <slug>");
            std::process::exit(2);
        };

        println!("installation de « {slug} »…");
        let mut dernier = 0;
        let resultat = catalogue::installer(slug, &mut |fait, total| {
            // On n'imprime qu'au changement : 46 lignes identiques ne
            // renseignent personne.
            if fait != dernier {
                dernier = fait;
                println!("  {fait}/{total}");
            }
        });

        match resultat {
            Ok(chemin) => {
                println!("installé : {}", chemin.display());
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("échec : {e}");
                std::process::exit(1);
            }
        }
    }

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

        // La MÊME résolution que l'application : la simulation doit jouer
        // avec le `blob` que le programme afficherait réellement, sinon elle
        // mesurerait le comportement d'un autre personnage que celui qu'on
        // observe à l'écran.
        let Some(dossier) = config::dossier_du_personnage("blob") else {
            eprintln!("simulation impossible : personnage « blob » introuvable");
            std::process::exit(1);
        };
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

/// Crée la fenêtre d'UN personnage, avec toutes ses propriétés.
///
/// Extraite de `setup` : les fenêtres naissent désormais **en cours
/// d'exécution**, depuis le thread de la boucle, et plus seulement au
/// démarrage (design « plusieurs personnages » §3).
///
/// C'est légitime : `RuntimeHandle` de Tauri poste un message au thread
/// principal quand on l'appelle depuis un autre thread. À l'inverse,
/// `tauri-runtime-wry` panique explicitement si `WindowMessage::Close` est
/// traité *sur* le thread principal (`lib.rs:3492`) — c'est-à-dire si l'on
/// ferme une fenêtre depuis un gestionnaire d'événements. Notre boucle étant
/// un thread à part, elle est du bon côté. **Vérifié par un spike jetable**
/// (20 cycles création/destruction) et pas seulement déduit des sources.
///
/// ⚠️ **Les deux styles étendus sont posés ICI et nulle part ailleurs.** Les
/// oublier sur les fenêtres n° 2 et suivantes donnerait des personnages qui
/// volent le focus et apparaissent dans Alt+Tab — un défaut qui ne se verrait
/// que sur le deuxième personnage, donc jamais pendant une mise au point à
/// N=1.
fn creer_fenetre_personnage(
    app: &tauri::AppHandle,
    label: &str,
    nom: &str,
    taille: (u32, u32),
) -> Result<tauri::WebviewWindow, String> {
    let win = tauri::WebviewWindowBuilder::new(
        app,
        label,
        // Le fragment dit à `pet.js` quel personnage servir, et il doit
        // porter le nom **du pack**, pas `blob` en dur.
        //
        // Le bug que ça corrige est sournois : avec `#blob` fixe, le
        // manifeste chargé était bien celui du personnage demandé (bonnes
        // poses, bonnes ancres, bonne hitbox) mais le webview réclamait
        // `shime:///blob/N` — donc les **images** de blob. Rien ne le
        // signalait : aucune erreur, aucune trace, un personnage
        // parfaitement animé… avec le mauvais dessin.
        //
        // Invisible tant que `blob` était le seul pack livré. Constaté à
        // l'œil en ajoutant `luffy`, et par aucun autre moyen.
        tauri::WebviewUrl::App(format!("index.html#{nom}").into()),
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
    .build()
    .map_err(|e| format!("fenêtre « {label} » : {e}"))?;

    // Les clics traversent en permanence ; la boucle ne les réactive que
    // dans la hitbox de la pose courante (spec §3.3).
    win.set_ignore_cursor_events(true)
        .map_err(|e| format!("clics traversants sur « {label} » : {e}"))?;

    // **Les deux découvertes de l'étape 0.** À faire avant que la hitbox
    // n'existe : sinon le vol de focus apparaîtrait en même temps que
    // l'attrapabilité, et les deux se diagnostiqueraient ensemble, pour rien.
    match render::appliquer_styles_etendus(&win) {
        Ok(()) => println!("styles étendus posés sur {label} (NOACTIVATE, TOOLWINDOW)"),
        // Non bloquant : la fenêtre marche sans, elle est seulement moins
        // polie. Mieux vaut un personnage qui vole le focus qu'aucun
        // personnage.
        Err(e) => eprintln!("styles étendus NON appliqués sur {label} : {e}"),
    }

    Ok(win)
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

    // ── Le roster voulu, tel que la config le dit ───────────────────────
    //
    // `personnages` est un **multi-ensemble** : une répétition vaut un
    // exemplaire de plus (design §4). La liste vide est permise — zéro
    // personnage est un état normal, pas une erreur de configuration : on
    // démarre, le tray vit, et un clic dans la bibliothèque ramène quelqu'un.
    let mut roster: Vec<String> = configuration.personnages.clone();

    // ── Les manifestes, avant tout le reste ─────────────────────────────
    //
    // Un par nom DISTINCT : trois blob, c'est un seul `mascot.json` lu et
    // trois `Manifest::clone`. `BTreeSet` plutôt que `HashSet` pour que
    // l'ordre de lecture — et donc l'ordre des messages — soit le même d'un
    // démarrage à l'autre.
    //
    // Un nom introuvable ou illisible est signalé **bruyamment** et retiré du
    // roster : les autres personnages doivent vivre. Échouer ici sur un seul
    // pack effacé à la main rendrait toute l'application inutilisable.
    let mut manifestes: std::collections::BTreeMap<String, character::manifest::Manifest> =
        std::collections::BTreeMap::new();

    for nom in roster.iter().collect::<std::collections::BTreeSet<_>>() {
        let Some(dossier) = config::dossier_du_personnage(nom) else {
            eprintln!("personnage « {nom} » introuvable (ni bibliothèque, ni dossier livré)");
            continue;
        };
        match character::manifest::Manifest::load(&dossier) {
            Ok(m) => {
                println!("personnage chargé : {} ({} poses)", m.name, m.poses.len());
                manifestes.insert(nom.clone(), m);
            }
            Err(e) => eprintln!("personnage « {nom} » illisible, ignoré : {e}"),
        }
    }

    // Les noms qui n'ont pas chargé sortent du roster : sans ce filtre, la
    // réconciliation redemanderait leur création à chaque passage à 8 Hz,
    // indéfiniment.
    roster.retain(|n| manifestes.contains_key(n));

    if roster.is_empty() {
        println!("aucun personnage actif — ils s'activent depuis « Ma bibliothèque »");
    }

    // La table d'envies, réglée par la config (décision n° 5). Construite
    // une fois : elle ne change qu'au rechargement à chaud (Tâche 5).
    let table = behavior::desire::TableEnvies::depuis_config(&configuration);
    let echelle_config = configuration.echelle;

    tauri::Builder::default()
        // ── Les trois commandes de la fenêtre du catalogue (spec §9) ────
        // `generate_handler!` engendre la table de routage à la
        // compilation. Attention : une commande oubliée ici est
        // introuvable côté JS **sans erreur de compilation** — d'où la
        // sonde d'IPC qui a validé le tuyau avant qu'on bâtisse dessus.
        .invoke_handler(tauri::generate_handler![
            commandes::installer,
            commandes::bibliotheque,
            commandes::definir_compte,
            commandes::supprimer
        ])
        // ── Le schéma URI qui sert les PNG externes ─────────────────────
        // Les personnages sont des fichiers externes au binaire (spec §8.1),
        // donc aucun chemin relatif du webview ne peut les atteindre. Rust
        // les sert lui-même. Sur Windows, ce schéma est accessible sous
        // http://shime.localhost/<personnage>/<numéro>.
        //
        // Vérifié : `Builder::register_uri_scheme_protocol`
        // (tauri-2.11.5/src/app.rs:2130) ; l'hôte `.localhost` sur Windows
        // (src/manager/mod.rs:342).
        // Plus de dossier capturé : `servir_frame` résout lui-même le
        // personnage nommé dans l'URL, par la MÊME règle que le chargement
        // (bibliothèque puis dossier livré). Capturer un dossier unique
        // reviendrait à ne pouvoir servir qu'une seule des deux racines.
        .register_uri_scheme_protocol("shime", move |_ctx, requete| {
            servir_frame(requete.uri().path())
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

            // ── L'aléatoire, semé UNE SEULE FOIS ────────────────────────
            //
            // Semé ici et non dans la boucle, parce que l'apparition en a
            // besoin avant que la boucle n'existe — et surtout parce qu'il ne
            // doit y en avoir **qu'un**. Un `XorShift32::seeded(index)` par
            // personnage retomberait en plein dans le piège des graines
            // séquentielles (CLAUDE.md) : tous apparaîtraient au même `x` et
            // tireraient la même première envie.
            //
            // Graine issue de l'horloge système : deux lancements ne doivent
            // pas donner la même histoire. C'est le seul endroit du programme
            // où l'aléatoire n'est pas reproductible, et c'est voulu — le
            // mode simulation, lui, prend une graine explicite.
            let graine = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(12345);
            let mut rng = rng::XorShift32::seeded(graine);

            // L'échelle d'AFFICHAGE : celle du moniteur, multipliée par le
            // réglage de l'utilisateur. Les fonctions de `attach` n'ont pas à
            // savoir que le second existe — elles reçoivent un seul facteur.
            let echelle_affichage = ecrans[0].scale * configuration.echelle;

            // ── Un acteur, et une fenêtre, par personnage du roster ──────
            //
            // Ils TOMBENT tous du haut de l'écran, à des `x` tirés au sort.
            // Presque rien à écrire, et c'est la décision n° 1 qui le
            // permet : `Attachment::Falling` existe déjà, et les réflexes à
            // 60 Hz gèrent chute puis atterrissage depuis l'étape 4a.
            let mut acteurs: Vec<Acteur> = Vec::new();

            for (index, nom) in roster.iter().enumerate() {
                // `expect` impossible à déclencher : `roster` a été filtré
                // sur `manifestes` juste avant. On préfère quand même sauter
                // plutôt que paniquer dans `setup`.
                let Some(manifeste) = manifestes.get(nom) else {
                    continue;
                };

                let taille = character::attach::window_size(manifeste, echelle_affichage);

                // `let … else` : sans écran, il n'y a nulle part où le faire
                // apparaître. Un monde vide est un cas normal (session
                // distante en cours d'établissement).
                let Some(attachement) =
                    apparition::point_de_chute(&monde, taille.0 as f32, &mut rng)
                else {
                    break;
                };

                // `pos_connue` doit valoir le point de chute : c'est le champ
                // qui amorce une chute qui commence (voir son commentaire
                // dans `character/mod.rs`).
                let depart = match attachement {
                    character::attach::Attachment::Falling { pos, .. } => pos,
                    // Inatteignable — `point_de_chute` ne rend que `Falling`.
                    _ => geom::Point::new(0.0, 0.0),
                };

                let label = label_de(index);
                if let Err(e) =
                    creer_fenetre_personnage(&app.handle().clone(), &label, nom, taille)
                {
                    eprintln!("« {nom} » n'a pas pu apparaître : {e}");
                    continue;
                }

                acteurs.push(Acteur {
                    label,
                    nom: nom.clone(),
                    ch: character::Character::new(manifeste.clone(), attachement, depart),
                    dernier_rendu: None,
                    derniere_taille: None,
                    dernier_coin: None,
                    // `true` : c'est ce que `creer_fenetre_personnage` vient
                    // de poser. Mentir ici ferait sauter le premier appel de
                    // `traverser_les_clics`, et le personnage serait
                    // incliquable jusqu'au prochain changement d'état.
                    clics_traversent: true,
                    derniere_trace_grimpe: None,
                    depart: None,
                });
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
            //
            // Le ROSTER et non plus un personnage unique : « choisir » n'est
            // plus « remplacer » mais « ajouter ou retirer » (design §4).
            let actions = actions::Actions::nouvelles(
                visibilite.clone(),
                demande.clone(),
                roster.clone(),
                commande.clone(),
            );

            // `manage` met la valeur à disposition des commandes, qui la
            // reçoivent par un paramètre `State<…>`. C'est le mécanisme
            // d'injection de Tauri — il évite une variable globale, et c'est
            // ainsi que les commandes de la bibliotheque atteignent le roster.
            tauri::Manager::manage(app, actions.clone());

            // ── Deux équivalents scriptables des clics de la bibliothèque ─
            //
            // Règle du projet : tout ce qui demanderait un clic en reçoit un.
            // Ces deux variables sont ce qui permet de vérifier l'étape
            // « plusieurs personnages » **sans humain**.

            // `SHIMEJI_PERSONNAGES=blob,blob,luffy` : le roster de départ.
            // Les doublons sont CONSERVÉS — deux blob veut dire deux blob.
            // Il est appliqué comme un changement ordinaire, donc les
            // personnages en trop tombent du haut de l'écran comme si on
            // venait de les activer.
            if let Ok(liste) = std::env::var("SHIMEJI_PERSONNAGES") {
                let voulus: Vec<String> = liste
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                match actions.definir_roster(&voulus, false) {
                    Ok(v) => println!("[diag] roster de départ {voulus:?} -> version {v}"),
                    // Bruyant et non fatal : on veut voir POURQUOI le roster
                    // demandé n'a pas pris, plutôt que démarrer sur un écran
                    // vide sans explication.
                    Err(e) => eprintln!("[diag] roster de départ refusé : {e}"),
                }
            }

            // `SHIMEJI_ROSTER=8:blob,luffy` : un changement de roster après
            // 8 secondes — le clic dans la bibliothèque, **y compris une
            // désactivation**. C'est le seul moyen d'observer le DÉPART sans
            // qu'un humain clique.
            if let Ok(valeur) = std::env::var("SHIMEJI_ROSTER") {
                // `split_once` : la forme est `<secondes>:<liste>`. Une
                // valeur mal formée est signalée et ignorée, plutôt que
                // d'agir tout de suite — ce qui ressemblerait à un succès.
                match valeur.split_once(':') {
                    Some((secondes, liste)) => match secondes.trim().parse::<u64>() {
                        Ok(delai) => {
                            let voulus: Vec<String> = liste
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            let actions_diag = actions.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_secs(delai));
                                match actions_diag.definir_roster(&voulus, false) {
                                    Ok(v) => println!("[diag] roster {voulus:?} -> version {v}"),
                                    Err(e) => eprintln!("[diag] roster refusé : {e}"),
                                }
                            });
                        }
                        Err(_) => eprintln!("SHIMEJI_ROSTER : « {secondes} » n'est pas un délai"),
                    },
                    None => eprintln!("SHIMEJI_ROSTER : forme attendue <secondes>:<liste>"),
                }
            }

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
                // La boucle y publie ce qui vit réellement, pour que la
                // suppression d'un pack sache quand les fenêtres ont disparu.
                let actions_boucle = actions.clone();
                std::thread::spawn(move || {
                    boucle(
                        handle,
                        acteurs,
                        monde,
                        echelle_affichage,
                        visibilite,
                        reglages,
                        table,
                        echelle_config,
                        demande,
                        temoin,
                        configuration_boucle,
                        commande,
                        rng,
                        actions_boucle,
                    );
                });
            }

            // `SHIMEJI_CATALOGUE=1` ouvre la fenêtre du catalogue au
            // démarrage — l'équivalent scriptable de l'entrée de menu que la
            // tâche 11 ajoutera, et ce qui a prouvé que l'IPC répondait
            // avant qu'on bâtisse une interface dessus (tâche 8).
            if std::env::var("SHIMEJI_CATALOGUE").is_ok() {
                actions::ouvrir_catalogue(&tauri::Manager::app_handle(app).clone());
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
fn servir_frame(chemin: &str) -> tauri::http::Response<Vec<u8>> {
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

    // Même résolution que le chargement : bibliothèque puis dossier livré.
    // La validation du nom ci-dessus protège désormais DEUX racines, ce qui
    // la rend d'autant plus nécessaire.
    let Some(dossier_perso) = crate::config::dossier_du_personnage(personnage) else {
        return refus(404);
    };
    let fichier = dossier_perso.join("img").join(format!("shime{n}.png"));

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

/// Un personnage à l'écran : sa fenêtre, son état, et le peu de mémoire de
/// rendu qu'il faut pour n'appeler Windows que sur changement.
///
/// **Ce qui est ici est ce qui doit exister N fois.** Tout ce qui est
/// partagé — le monde, les signaux, le biais, le RNG, les horloges — reste
/// une variable locale de `boucle` et n'est calculé qu'UNE fois par image
/// (design « plusieurs personnages » §3). C'est ce partage qui fait que N
/// personnages ne coûtent pas N fois notre calcul.
struct Acteur {
    /// Le label de sa fenêtre Tauri.
    ///
    /// ⚠️ **Jamais réutilisé** : il vient d'un compteur monotone, pas de
    /// l'index dans le `Vec`. Retirer `pet-1` puis en ajouter un
    /// réattribuerait `pet-1` pendant que Windows détruit encore la fenêtre
    /// précédente (design §3, piège n° 3).
    label: String,

    /// Le pack dont il est une instance. **Plusieurs acteurs peuvent
    /// partager le même nom** : c'est tout l'objet des doublons.
    nom: String,

    ch: character::Character,

    // ── La mémoire de rendu ─────────────────────────────────────────────
    // Ces quatre champs existent pour une seule raison : n'appeler Windows
    // que quand quelque chose a changé. Le coût étant proportionnel au
    // nombre de déplacements (design §2), s'en priver multiplierait la
    // consommation par ~2 — c'est mesuré, 21 % contre 12,3 %.
    dernier_rendu: Option<render::Rendu>,
    derniere_taille: Option<(u32, u32)>,
    dernier_coin: Option<(i32, i32)>,
    clics_traversent: bool,

    /// Diagnostic `SHIMEJI_ESCALADE=1` : le dernier triplet tracé, pour ne
    /// tracer qu'au changement de phase.
    derniere_trace_grimpe: Option<(String, String, String)>,

    /// Non `None` quand il est en train de quitter la scène (design §6).
    ///
    /// Sa présence **court-circuite `behavior::pas`** : un acteur en départ
    /// n'est plus un personnage vivant, il s'en va, et la scène n'a plus son
    /// mot à dire.
    depart: Option<Depart>,
}

/// Les trois temps d'un départ (design §6).
///
/// ⚠️ **Il n'existe AUCUNE frame « plier les jambes » dans le vocabulaire
/// Shimeji** — vérifié dans `docs/specs/2026-09-09-frames-shimeji.md`, tiré
/// des sources de Shimeji-ee. Le standard n'a que `jump`, la frame 22, une
/// seule image. Ces trois temps composent la lecture cherchée avec ce qui
/// existe réellement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhaseDepart {
    /// `sit` ~120 ms : il se ramasse.
    SeRamasse,
    /// `jump` ~150 ms : il se détend.
    Saute,
    /// `fall` : il tombe, et il TRAVERSE le sol.
    Tombe,
}

/// Un acteur en train de quitter la scène.
///
/// **Pourquoi court-circuiter `behavior::pas` plutôt qu'ajouter un état au
/// comportement :** le réflexe d'atterrissage est non négociable par
/// définition (spec §7.1, couche 1). Y introduire une exception « sauf si je
/// suis en train de partir » le rendrait négociable, et ce serait le premier
/// pas vers un réflexe plein de cas particuliers.
struct Depart {
    phase: PhaseDepart,
    /// Le temps de l'horloge injectée au début de la **phase courante**.
    depuis: std::time::Duration,
    /// Le début du départ **entier**.
    ///
    /// Distinct de `depuis`, et c'est nécessaire : le garde-fou des 3 s porte
    /// sur le départ complet, pas sur sa dernière phase. Les confondre le
    /// rendrait inopérant, puisqu'il repartirait de zéro à chaque changement
    /// de pose.
    debut: std::time::Duration,
    pos: geom::Point,
    vel: geom::Vec2,
}

/// Les durées des deux premières phases du départ.
///
/// **Points de départ à régler à l'œil**, comme toutes les constantes
/// d'animation de ce projet — et comme elles, à chercher d'abord dans les
/// sources de Shimeji-ee avant d'en inventer une (leçon transverse de
/// l'étape 1a, où les quatre réglages faits à l'œil étaient faux).
const DUREE_SE_RAMASSE: std::time::Duration = std::time::Duration::from_millis(120);
const DUREE_SAUTE: std::time::Duration = std::time::Duration::from_millis(150);

/// La vitesse initiale vers le haut, en px/s. Sans elle, « il saute » se lit
/// comme « il glisse ».
const IMPULSION_DEPART: f32 = 350.0;

/// De combien il faut dépasser le bas du bureau pour être hors de vue. Une
/// hauteur de fenêtre suffit largement.
const MARGE_SORTIE: f32 = 200.0;

/// Au-delà, l'acteur est retiré quoi qu'il arrive. Un acteur qui ne partirait
/// jamais ferait fuir une fenêtre à chaque désactivation — et c'est le genre
/// de fuite qu'on ne voit qu'après une heure d'usage.
const DUREE_MAX_DEPART: std::time::Duration = std::time::Duration::from_secs(3);

impl Depart {
    fn commence(maintenant: std::time::Duration, pos: geom::Point) -> Depart {
        Depart {
            phase: PhaseDepart::SeRamasse,
            depuis: maintenant,
            debut: maintenant,
            pos,
            vel: geom::Vec2::new(0.0, 0.0),
        }
    }
}

/// Fait avancer un départ d'une image. Rend `true` quand l'acteur doit être
/// retiré.
///
/// **La couverture partielle s'applique toute seule** : `set_pose` ignore une
/// pose absente du manifeste, donc un pack sans `sit` ou sans `jump` traverse
/// simplement la phase correspondante sans changer d'image. Aucun cas
/// particulier à coder — c'est la règle §8.6, qui retire du jeu ce qui n'est
/// pas dessiné.
fn avancer_le_depart(
    d: &mut Depart,
    ch: &mut character::Character,
    monde: &world::World,
    maintenant: std::time::Duration,
    dt: f32,
) -> bool {
    // Le garde-fou de durée, testé en PREMIER.
    if maintenant.saturating_sub(d.debut) > DUREE_MAX_DEPART {
        return true;
    }

    match d.phase {
        PhaseDepart::SeRamasse => {
            ch.set_pose("sit", maintenant);
            if maintenant.saturating_sub(d.depuis) >= DUREE_SE_RAMASSE {
                d.phase = PhaseDepart::Saute;
                d.depuis = maintenant;
            }
        }

        PhaseDepart::Saute => {
            ch.set_pose("jump", maintenant);
            if maintenant.saturating_sub(d.depuis) >= DUREE_SAUTE {
                d.phase = PhaseDepart::Tombe;
                d.depuis = maintenant;
                // Une petite impulsion vers le haut : sans elle, « il saute »
                // se lit comme « il glisse ».
                d.vel = geom::Vec2::new(0.0, -IMPULSION_DEPART);
            }
        }

        PhaseDepart::Tombe => {
            ch.set_pose("fall", maintenant);

            // La même gravité que la physique normale, mais **sans son test
            // de contact** : c'est très exactement ce que « il traverse le
            // sol » veut dire, et la raison d'être de ce court-circuit.
            d.vel.y += character::physics::GRAVITE * dt;
            d.pos.y += d.vel.y * dt;

            // `bounds()` couvre TOUS les écrans : sur deux moniteurs de
            // hauteurs différentes, ne regarder que l'écran de départ
            // retirerait le personnage trop tôt sur l'un des deux.
            //
            // `map_or(true, …)` : sans aucun écran il n'y a plus rien à
            // montrer, donc on retire — plutôt que de le laisser tomber
            // indéfiniment.
            let sorti = monde
                .bounds()
                .map_or(true, |b| d.pos.y > b.bottom() + MARGE_SORTIE);
            if sorti {
                return true;
            }
        }
    }

    // La position est poussée au webview par le chemin de rendu habituel :
    // `pos_connue` est ce que la boucle lit pour placer la fenêtre d'un
    // acteur en départ.
    ch.pos_connue = d.pos;
    false
}

/// Quel personnage le curseur désigne-t-il ? **Au plus un.**
///
/// Rend son index dans le `Vec`. Il n'y a qu'un curseur, donc au plus un
/// personnage concerné : sans cette règle, deux personnages superposés
/// seraient attrapés ENSEMBLE par un même clic et se suivraient jusqu'au
/// relâchement (design §3, piège n° 2).
///
/// L'ordre de départage est celui du `Vec` : arbitraire, mais **stable** —
/// le même personnage gagne tant que rien ne change. Deux fenêtres toujours
/// au premier plan n'ont de toute façon pas de z-order que nous
/// contrôlions.
fn elire_sous_le_curseur(
    acteurs: &[Acteur],
    monde: &world::World,
    souris: geom::Point,
    echelle: f32,
) -> Option<usize> {
    // Un personnage déjà porté garde la main, où que soit le curseur : un
    // glisser rapide fait sortir le sprite de sa propre hitbox, et le
    // relâcher tout seul serait précisément le bug que la traversée des
    // clics évite déjà côté fenêtre.
    for (i, a) in acteurs.iter().enumerate() {
        if matches!(a.ch.attachment, character::attach::Attachment::Dragged) {
            return Some(i);
        }
    }

    for (i, a) in acteurs.iter().enumerate() {
        // Position indérivable (plateforme disparue) ou pose absente du
        // manifeste : on ne peut pas savoir. On passe au suivant plutôt que
        // de décider à sa place — les clics continuent de traverser, ce qui
        // ne gêne personne.
        let (Some(pos), Some(pose)) = (
            character::attach::world_position(&a.ch.attachment, monde, souris),
            a.ch.manifest.pose(&a.ch.pose),
        ) else {
            continue;
        };

        if character::attach::hitbox_ecran(
            pos,
            &a.ch.pose,
            pose,
            &a.ch.manifest,
            echelle,
            a.ch.facing,
        )
        .contains(souris)
        {
            return Some(i);
        }
    }

    None
}

fn boucle(
    handle: tauri::AppHandle,
    // Le `Vec` remplace le couple `(label, ch)` du temps où il n'y avait
    // qu'un personnage. `mut` : il grandit et rétrécit en cours
    // d'exécution, c'est tout l'objet de cette étape.
    mut acteurs: Vec<Acteur>,
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
    commande: actions::BoiteCommande,
    // **Le RNG, semé une seule fois par `setup`.** Il arrive déjà amorcé
    // parce que l'apparition du premier personnage s'en est servie avant que
    // ce thread n'existe. Le re-semer ici rejouerait la même séquence, et en
    // créer un par personnage retomberait dans le piège des graines
    // séquentielles (CLAUDE.md).
    mut rng: rng::XorShift32,
    // Partagé avec les deux menus. La boucle n'y touche que pour PUBLIER ce
    // qui vit réellement (design §7) — elle ne lit jamais le roster voulu,
    // qui lui arrive par la boîte de rechargement.
    actions: std::sync::Arc<actions::Actions>,
) {
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
        // Sur TOUS les acteurs : à N=1 c'est l'ancien comportement, et à N
        // supérieur on veut pouvoir regarder n'importe lequel d'entre eux.
        for a in acteurs.iter_mut() {
            a.ch.intention = Some(behavior::intention::ActiveIntention::nouvelle(
                behavior::intention::Intention::Grimper,
                horloge.elapsed(),
            ));
        }
        println!("SHIMEJI_ESCALADE : intention Grimper forcée au démarrage");
    }

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
    // `dernier_rendu`, `derniere_taille`, `dernier_coin` et
    // `clics_traversent` ont migré dans `Acteur` : ce sont les seules
    // variables de cette boucle qui doivent exister N fois. La position
    // ENTIÈRE posée à la dernière image (`dernier_coin`) en fait partie —
    // `set_position` ne prend que des entiers, et deux positions flottantes
    // qui s'arrondissent au même pixel produisent exactement le même appel.

    // Diagnostic de cadence, voir plus bas.
    let trace_cadence = std::env::var("SHIMEJI_CADENCE").is_ok();
    let mut images_depuis_trace: u32 = 0;
    let mut placements_depuis_trace: u32 = 0;
    let mut travail_cumule = Duration::ZERO;
    let mut derniere_trace = Duration::ZERO;

    // Le prochain numéro de label libre.
    //
    // ⚠️ **Monotone, et jamais remis à zéro.** L'index dans le `Vec` ne peut
    // pas servir de label : retirer `pet-1` puis en ajouter un réattribuerait
    // `pet-1` pendant que Windows détruit encore la fenêtre précédente, et
    // Tauri refuserait le label — ou pire, servirait l'ancienne fenêtre
    // (design §3, piège n° 3).
    //
    // Il part après les labels déjà posés par `setup`.
    let mut prochain_label: usize = acteurs.len();

    // Le bouton droit était-il enfoncé à l'image précédente ?
    //
    // **Partagé, et pas par acteur** : il n'y a qu'une souris, donc qu'un
    // front descendant par clic. Un drapeau par personnage ferait ouvrir N
    // menus d'affilée sur un seul clic.
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
                //
                // Sur TOUS les acteurs : en oublier un le laisserait figé
                // dans la pose qu'il avait au verrouillage, pendant que les
                // autres émergent.
                if !verrouille {
                    for a in acteurs.iter_mut() {
                        if !deja_en_reveil(&a.ch) {
                            a.ch.intention =
                                Some(behavior::intention::ActiveIntention::reveil(maintenant));
                        }
                    }
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

            // Le fichier témoin : présent → on demande un rechargement du
            // roster ENTIER, et on le retire pour ne pas boucler.
            //
            // **Le même chemin que la bibliothèque** : relire, recharger,
            // réconcilier. Un seul chemin de code, donc pas de second qui
            // divergerait à la première correction (design §4).
            if temoin.exists() {
                let _ = std::fs::remove_file(&temoin);
                // On relit la config à chaque fois plutôt qu'une fois au
                // démarrage : un pack a pu être installé entre-temps, et une
                // liste mémorisée ne le verrait jamais.
                let voulus = config::charger().personnages;
                match rechargement::preparer_roster(&demande, &voulus, false) {
                    Ok(v) => println!("rechargement demandé par témoin (version {v})"),
                    Err(e) => eprintln!("rechargement impossible : {e}"),
                }
            }

            // ── Une demande de rechargement en attente ? ────────────────
            //
            // `try_lock` et non `lock` : la boucle 60 Hz ne doit JAMAIS
            // attendre. Si le thread de la commande tient le verrou à cet
            // instant, on réessaiera dans 125 ms et personne ne s'en
            // apercevra.
            if let Ok(mut boite) = demande.try_lock() {
                // `take()` vide la boîte en récupérant son contenu : la
                // demande est consommée atomiquement, sans drapeau à
                // remettre à zéro.
                if let Some(r) = boite.take() {
                    reglages = r.reglages;
                    table = r.table;
                    echelle_config = r.echelle_config;
                    config_courante = r.config;
                    echelle_affichage = ecrans_echelle * echelle_config;

                    // ── Les manifestes des acteurs déjà là ─────────────
                    //
                    // Un rechargement à chaud change le `mascot.json` sous
                    // les pieds d'un personnage qui reste : il faut lui
                    // donner le nouveau, sans quoi le fichier témoin ne
                    // servirait plus à régler les animations.
                    for a in acteurs.iter_mut() {
                        let Some(charge) = r.personnages.iter().find(|p| p.nom == a.nom) else {
                            continue;
                        };

                        // La pose courante existe-t-elle encore dans le
                        // nouveau manifeste ? Si elle vient d'être renommée
                        // ou retirée, `set_pose` refuserait tout changement
                        // et le personnage resterait figé sur une clé morte.
                        if !charge.manifeste.has_pose(&a.ch.pose) {
                            a.ch.pose = character::manifest::POSE_STAND.to_string();
                            a.ch.pose_depuis = maintenant;
                        }

                        a.ch.manifest = charge.manifeste.clone();

                        // Le webview doit oublier ses images, et la taille
                        // de la fenêtre peut avoir changé (`frameSize`,
                        // `scale`).
                        let _ = render::recharger(&handle, &a.label, r.version, &a.nom);
                        a.derniere_taille = None;
                        a.dernier_rendu = None;
                        a.dernier_coin = None;
                    }

                    // ── Réconcilier présents et voulus ─────────────────
                    //
                    // Un acteur DÉJÀ EN DÉPART ne compte plus comme
                    // présent : sinon réactiver un personnage pendant que
                    // le précédent tombe n'en créerait pas de nouveau, et
                    // le compteur de la bibliothèque mentirait.
                    let presents: Vec<String> = acteurs
                        .iter()
                        .filter(|a| a.depart.is_none())
                        .map(|a| a.nom.clone())
                        .collect();

                    for action in roster::reconcilier(&presents, &r.voulus) {
                        match action {
                            roster::ActionRoster::Creer(nom) => {
                                // Le manifeste a été lu par le thread de la
                                // commande : il n'y a AUCUNE entrée-sortie
                                // ici, sur le chemin des 60 Hz.
                                let Some(charge) = r.personnages.iter().find(|p| p.nom == nom)
                                else {
                                    eprintln!("« {nom} » voulu mais non chargé : ignoré");
                                    continue;
                                };

                                let taille = character::attach::window_size(
                                    &charge.manifeste,
                                    echelle_affichage,
                                );

                                // Le point d'apparition AVANT la fenêtre :
                                // s'il n'y a aucun écran, on n'a pas créé
                                // de fenêtre à détruire.
                                let Some(att) =
                                    apparition::point_de_chute(&monde, taille.0 as f32, &mut rng)
                                else {
                                    eprintln!("aucun écran : « {nom} » n'apparaît pas");
                                    continue;
                                };

                                // ⚠️ Compteur MONOTONE, jamais l'index dans
                                // le `Vec` : un label réutilisé se
                                // heurterait à une fenêtre que Windows
                                // détruit encore (design §3, piège n° 3).
                                let label = format!("pet-{prochain_label}");
                                prochain_label += 1;

                                match creer_fenetre_personnage(&handle, &label, &nom, taille) {
                                    Ok(_) => {
                                        let depart_pos = match att {
                                            character::attach::Attachment::Falling {
                                                pos, ..
                                            } => pos,
                                            // Inatteignable : `point_de_chute`
                                            // ne rend que `Falling`. Un repli
                                            // lisible plutôt qu'un
                                            // `unreachable!()` qui tuerait la
                                            // boucle si ce module changeait.
                                            _ => geom::Point::new(0.0, 0.0),
                                        };

                                        acteurs.push(Acteur {
                                            label: label.clone(),
                                            nom: nom.clone(),
                                            ch: character::Character::new(
                                                charge.manifeste.clone(),
                                                att,
                                                depart_pos,
                                            ),
                                            dernier_rendu: None,
                                            derniere_taille: None,
                                            dernier_coin: None,
                                            // `true` : c'est ce que
                                            // `creer_fenetre_personnage` vient
                                            // de poser. Mentir ici ferait
                                            // sauter le premier appel de
                                            // `traverser_les_clics`, et le
                                            // personnage serait incliquable.
                                            clics_traversent: true,
                                            derniere_trace_grimpe: None,
                                            depart: None,
                                        });
                                        println!("« {nom} » apparaît ({label})");
                                    }
                                    // Bruyant : une création silencieusement
                                    // ratée donnerait un compteur à 3 pour 2
                                    // personnages à l'écran.
                                    Err(e) => {
                                        eprintln!("« {nom} » n'a pas pu apparaître : {e}")
                                    }
                                }
                            }

                            roster::ActionRoster::RetirerUn(nom) => {
                                // `rposition` : le plus RÉCEMMENT ajouté
                                // part le premier, ce qu'attend quelqu'un
                                // qui vient de cliquer « + » puis « − »
                                // (design §4). On ignore ceux qui partent
                                // déjà, pour ne pas en marquer deux.
                                let Some(i) = acteurs
                                    .iter()
                                    .rposition(|a| a.nom == nom && a.depart.is_none())
                                else {
                                    continue;
                                };

                                if r.sans_animation {
                                    // Suppression d'un pack : on n'attend
                                    // pas l'animation, les fichiers vont
                                    // être effacés (design §7).
                                    let parti = acteurs.remove(i);
                                    if let Some(w) = handle.get_webview_window(&parti.label) {
                                        let _ = w.destroy();
                                    }
                                    println!("« {nom} » retiré ({})", parti.label);
                                } else {
                                    let pos = acteurs[i].ch.pos_connue;
                                    acteurs[i].depart = Some(Depart::commence(maintenant, pos));
                                    println!("« {nom} » s'en va ({})", acteurs[i].label);
                                }
                            }
                        }
                    }

                    println!("roster appliqué (version {})", r.version);
                }
            }

            // Publier ce qui vit RÉELLEMENT, pour la suppression (design §7).
            // À 8 Hz, jamais sur le chemin des 60 Hz.
            actions.publier_presents(
                acteurs
                    .iter()
                    .filter(|a| a.depart.is_none())
                    .map(|a| a.nom.clone())
                    .collect(),
            );

            dernier_recensement = maintenant;
        }

        // ── 60 Hz : les entrées, et le hit-testing (spec §3.3) ──────────
        // La spec §3.3 propose ~30 Hz pour `GetCursorPos`. On le lit à 60 Hz :
        // l'appel est effectivement quasi gratuit, et à 30 Hz le personnage
        // traînerait visiblement derrière le curseur pendant un glisser.
        let m = sonde.mouse();

        // ── L'élection : UN SEUL personnage sous le curseur ─────────────
        //
        // Le hit-testing compare le curseur à la hitbox de la POSE COURANTE
        // — et non à la boîte de 128×128 : sans cette distinction, le
        // personnage serait un trou noir de 128 px avalant les clics dans
        // ses zones transparentes. `elire_sous_le_curseur` le fait pour
        // chaque acteur, et n'en retient qu'un.
        let elu = elire_sous_le_curseur(&acteurs, &monde, m.pos, echelle_affichage);

        // Le front descendant du bouton droit : calculé UNE fois, avant la
        // boucle, parce qu'il n'y a qu'une souris.
        //
        // C'est ce qui transforme un état — « le bouton est enfoncé », vrai
        // pendant les ~15 images que dure un clic humain — en un **front**,
        // qui n'arrive qu'une fois. Sans lui, maintenir le bouton rouvrirait
        // le menu en boucle dès sa fermeture.
        let front_descendant_droit = !m.right_down && bouton_droit_precedent;
        bouton_droit_precedent = m.right_down;

        // La commande éventuellement déposée par le gestionnaire de menu.
        //
        // `try_lock` et non `lock` : à 60 Hz on ne s'autorise jamais à
        // attendre un verrou. S'il est pris à cet instant, la commande sera
        // lue à l'image suivante, 16 ms plus tard — invisible.
        //
        // `take()` vide la boîte en récupérant son contenu : la commande est
        // ainsi consommée une fois et une seule. **Lue avant la boucle, et
        // donnée au seul acteur élu** : la donner à tous ferait exécuter
        // l'entrée de menu par N personnages.
        let mut commande_du_menu = match commande.try_lock() {
            Ok(mut boite) => boite.take(),
            Err(_) => None,
        };

        // Caché par l'utilisateur, OU session verrouillée : on calcule tout,
        // on ne dessine rien. Lu une fois, il vaut pour tous les acteurs.
        //
        // Le comportement, lui, continue de tourner : il doit avancer pour
        // qu'on le retrouve ailleurs en le réaffichant, et c'est du calcul
        // pur — mesuré à 100 µs par image quand il ne se passe rien.
        //
        // Ce qui coûte, c'est `SetWindowPos` sur une fenêtre en couche (voir
        // « Mesurer le CPU » dans CLAUDE.md), et c'est exactement ce qu'on
        // saute. Le verrouillage emprunte EXACTEMENT ce même chemin, déjà
        // mesuré à 0,9 %.
        //
        // `Ordering::Relaxed` : il n'y a aucune autre donnée à synchroniser
        // avec ce booléen, seulement sa propre valeur.
        let visible = visibilite.load(std::sync::atomic::Ordering::Relaxed) && !verrouille;

        // Un menu contextuel a-t-il été ouvert pendant cette image ? Voir
        // pourquoi ce drapeau existe, plus bas, là où il est posé.
        let mut menu_ouvert = false;

        // Les acteurs dont le départ s'achève à cette image.
        //
        // Collectés plutôt que retirés sur place : retirer d'un `Vec` qu'on
        // parcourt par index décalerait tous les suivants, et on en sauterait
        // un sur deux. On les retire après la boucle, **du dernier au
        // premier**, pour que les index restent valides.
        let mut a_retirer: Vec<usize> = Vec::new();

        let dt = PERIODE.as_secs_f32();

        // ── 60 Hz par personnage ────────────────────────────────────────
        for i in 0..acteurs.len() {
            let sur_le_personnage = elu == Some(i);
            let acteur = &mut acteurs[i];

            // ── Un acteur en départ ne vit plus : il tombe, et c'est tout ─
            //
            // Ni comportement, ni hit-testing, ni menu : il n'est plus
            // attrapable ni cliquable (design §6). `elire_sous_le_curseur`
            // l'ignore déjà ; ce `continue` couvre tout le reste.
            if let Some(mut d) = acteur.depart.take() {
                let fini = avancer_le_depart(&mut d, &mut acteur.ch, &monde, maintenant, dt);
                if fini {
                    a_retirer.push(i);
                } else {
                    // Remis en place : `take()` l'avait sorti pour pouvoir
                    // emprunter `acteur.ch` en même temps que lui. Sans ce
                    // va-et-vient, le vérificateur d'emprunt refuserait deux
                    // emprunts mutables du même `acteur`.
                    acteur.depart = Some(d);

                    // On le dessine quand même : c'est toute la raison
                    // d'être de l'animation de départ.
                    //
                    // ⚠️ **`window_top_left` et pas un calcul à la main.**
                    // L'ancre d'une pose n'est pas le centre du sprite —
                    // c'est le sol sous ses pieds, et elle diffère d'une
                    // pose à l'autre (`sit`, `jump` et `fall` n'ont pas la
                    // même). Soustraire une demi-largeur poserait le
                    // personnage de travers pendant sa chute, et c'est très
                    // exactement la compensation que l'ancre existe pour
                    // rendre inutile (spec §8.3).
                    if visible {
                        if let Some(pose) = acteur.ch.manifest.pose(&acteur.ch.pose) {
                            let coin = character::attach::window_top_left(
                                acteur.ch.pos_connue,
                                pose,
                                &acteur.ch.manifest,
                                echelle_affichage,
                                acteur.ch.facing,
                            );
                            let _ = render::placer(&handle, &acteur.label, coin);
                            placements_depuis_trace += 1;
                        }

                        let rendu = render::Rendu {
                            image: acteur.ch.frame_courante(maintenant),
                            flip: acteur.ch.facing.flipped(),
                        };
                        if acteur.dernier_rendu != Some(rendu) {
                            let _ = render::pousser(&handle, &acteur.label, rendu);
                            acteur.dernier_rendu = Some(rendu);
                        }
                    }
                }
                continue;
            }

            // ── Absorber le clic, mais seulement là où il faut ──────────
            //
            // Pourquoi désactiver la traversée alors que la sonde nous dit
            // déjà tout ? Parce que si les clics continuaient de traverser,
            // cliquer sur le personnage cliquerait **aussi** l'icône du
            // bureau derrière lui. Il faut ABSORBER le clic — c'est à ça que
            // sert le va-et-vient de `set_ignore_cursor_events` (spec §3.3).
            //
            // Pendant un glisser, on garde les clics absorbés même si le
            // sprite a glissé hors de sa propre hitbox : sinon un
            // déplacement rapide relâcherait le personnage tout seul.
            let porte = matches!(
                acteur.ch.attachment,
                character::attach::Attachment::Dragged
            );
            let doit_traverser = !sur_le_personnage && !porte;

            // On n'appelle Win32 que sur CHANGEMENT d'état : appeler
            // `set_ignore_cursor_events` 60 fois par seconde marcherait,
            // mais c'est un appel système par image pour rien — et la
            // section « Mesurer le CPU » de CLAUDE.md dit pourquoi on y
            // regarde.
            if doit_traverser != acteur.clics_traversent {
                // ⚠️ `continue` et non `return` : la fenêtre de CE
                // personnage a pu être détruite, mais ça n'est plus une
                // raison de tuer la boucle — les autres continuent de
                // vivre. C'est la généralisation qui l'impose, et c'est
                // aussi ce qui rend le retrait d'un acteur inoffensif.
                if render::traverser_les_clics(&handle, &acteur.label, doit_traverser).is_err() {
                    continue;
                }
                acteur.clics_traversent = doit_traverser;
            }

            // ── Clic droit sur le personnage : le menu contextuel ───────
            //
            // Front **descendant** (le bouton vient d'être RELÂCHÉ) ET
            // curseur dans la hitbox : un clic droit sur le bureau à côté de
            // lui ne doit rien ouvrir. Le test de hitbox est le MÊME que
            // celui qui absorbe les clics gauches, donc la zone cliquable
            // est exactement celle qu'on voit.
            //
            // ⚠️ **Au relâchement et non à l'enfoncement**, et pour deux
            // raisons qui pointent dans le même sens :
            //
            // 1. c'est la convention de Windows — l'explorateur, comme toute
            //    application, ouvre son menu contextuel sur `WM_RBUTTONUP` ;
            // 2. ouvrir au bouton encore enfoncé lance `TrackPopupMenu`
            //    pendant que Windows suit toujours un clic droit en cours.
            //    Le menu hérite alors d'un suivi de souris qui ne lui
            //    appartient pas, et se referme mal.
            if front_descendant_droit && sur_le_personnage {
                // `let … else` : si la fenêtre a été fermée, cet acteur n'a
                // plus de menu à ouvrir. On passe au suivant.
                let Some(win) = handle.get_webview_window(&acteur.label) else {
                    continue;
                };

                // **Cet appel bloque** jusqu'à la fermeture du menu : le
                // personnage s'immobilise pendant ce temps, ce qui est voulu
                // (voir `menu_perso::ouvrir`).
                //
                // `ou_de` : le menu proposé dépend de l'endroit où il est
                // accroché — voir `menu_perso::Ou`. C'est ce qui corrige le
                // bug rapporté à l'écran : un menu de sol proposé à un
                // personnage accroché à un mur le faisait tomber au premier
                // clic, quelle que soit l'entrée choisie.
                let ou = menu_perso::ou_de(&acteur.ch.attachment);
                if let Err(e) =
                    menu_perso::ouvrir(&handle, &win, &acteur.ch.manifest, &table, ou)
                {
                    eprintln!("menu du personnage : {e}");
                }

                // ⚠️ **On sort de la boucle `for`, et l'image entière est
                // abandonnée** — pas seulement ce personnage.
                //
                // `maintenant` a été lu AVANT le menu, il a donc plusieurs
                // secondes de retard, et `dt` vaut toujours 16,7 ms.
                // Poursuivre ferait juger toutes les échéances (délai
                // d'abandon, durée de pose) sur un instant périmé — et à N
                // personnages, un simple `continue` de la boucle `for`
                // étendrait ce défaut aux N−1 autres au lieu de le corriger.
                menu_ouvert = true;
                break;
            }

            let entrees = Entrees {
            souris: m.pos,
            echelle_affichage,
            bouton_gauche: m.left_down,
            curseur_sur_le_personnage: sur_le_personnage,
            // Recalculés à 2 Hz ci-dessus, transportés tels quels à 60 Hz.
            biais,
            utilisateur_actif,
                // `take()` : la commande n'est donnée qu'à l'acteur ÉLU, et
                // une seule fois. La donner à tous ferait exécuter l'entrée
                // de menu par N personnages — dont N−1 qui n'ont rien
                // demandé.
                commande: if sur_le_personnage {
                    commande_du_menu.take()
                } else {
                    None
                },
            };

            // ── 60 Hz : le comportement ────────────────────────────────
            // La MÊME fonction que le mode simulation.
            behavior::pas(
                &mut acteur.ch,
                &monde,
                &entrees,
                &table,
                &reglages,
                maintenant,
                dt,
                &mut rng,
            );

        // ── Diagnostic : `SHIMEJI_ESCALADE=1` ───────────────────────────
        // Rien qu'une intention `Grimper` en cours ne trace : c'est
        // exactement le moment que l'auteur doit regarder pour mesurer
        // l'ancre de `grabWall`/`climbWall` à l'œil.
        if trace_escalade {
            if let Some(ai) = acteur.ch.intention {
                if let behavior::intention::EtatIntention::Grimpe { phase, .. } = ai.etat {
                    // Le MESSAGE affiché garde l'offset — il aide à situer le
                    // personnage sur la paroi au moment précis où la trace
                    // sort. Voir plus bas pourquoi la CLÉ, elle, ne le
                    // contient plus.
                    let face_affichee = match acteur.ch.attachment {
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
                    let face_pour_la_cle = match acteur.ch.attachment {
                        character::attach::Attachment::On { face, .. } => format!("{face:?}"),
                        character::attach::Attachment::Falling { .. } => "chute".to_string(),
                        character::attach::Attachment::Dragged => "porté".to_string(),
                    };
                    let cle = (format!("{phase:?}"), face_pour_la_cle, acteur.ch.pose.clone());
                    if acteur.derniere_trace_grimpe.as_ref() != Some(&cle) {
                        println!(
                            "SHIMEJI_ESCALADE : phase={:?} {} pose={}",
                            phase, face_affichee, acteur.ch.pose
                        );
                        acteur.derniere_trace_grimpe = Some(cle);
                    }
                }
            }
        }

            // ── Sur changement seulement : la taille de la fenêtre ─────
            // Elle ne dépend que du manifeste et de l'échelle de l'écran.
            // L'appeler à 60 Hz coûtait 8 points de pourcentage de CPU pour
            // rien (voir l'avertissement de `render::placer`).
            let taille = character::attach::window_size(&acteur.ch.manifest, echelle_affichage);
            if acteur.derniere_taille != Some(taille) {
                // `continue` et non `return` : voir la traversée des clics
                // plus haut — la fenêtre d'un acteur peut disparaître sans
                // que les autres aient à mourir avec.
                if render::dimensionner(&handle, &acteur.label, taille).is_err() {
                    continue;
                }
                acteur.derniere_taille = Some(taille);
            }

            // Caché ou session verrouillée : on a fait tourner le
            // comportement ci-dessus, et on s'arrête là. `visible` est lu
            // une fois pour tous, avant la boucle.
            if !visible {
                // On oublie ce qu'on avait posé : au retour, il faut tout
                // repousser, la fenêtre ayant pu être masquée entre-temps.
                acteur.dernier_coin = None;
                acteur.dernier_rendu = None;
                continue;
            }

            // ── 60 Hz : le rendu ───────────────────────────────────────
            // La position est DÉRIVÉE à chaque image (décision n° 1).
            if let Some(pos) =
                character::attach::world_position(&acteur.ch.attachment, &monde, m.pos)
            {
                if let Some(pose) = acteur.ch.manifest.pose(&acteur.ch.pose) {
                    let coin = character::attach::window_top_left(
                        pos,
                        pose,
                        &acteur.ch.manifest,
                        echelle_affichage,
                        acteur.ch.facing,
                    );

                    // Arrondi ici et non dans `placer` : c'est cet entier
                    // qu'on compare, et le calculer deux fois serait deux
                    // occasions de divergence.
                    let coin_entier = (coin.x.round() as i32, coin.y.round() as i32);

                    if acteur.dernier_coin != Some(coin_entier) {
                        if render::placer(&handle, &acteur.label, coin).is_err() {
                            continue;
                        }
                        acteur.dernier_coin = Some(coin_entier);
                        placements_depuis_trace += 1;
                    }
                }
            }

            let rendu = render::Rendu {
                image: acteur.ch.frame_courante(maintenant),
                flip: acteur.ch.facing.flipped(),
            };

            // N'émettre que sur changement — sauf pendant l'amorçage, où
            // l'écouteur du webview n'existe peut-être pas encore.
            //
            // À 60 Hz, une pose de marche ne change d'image que ~8 fois par
            // seconde : on économise ~85 % des messages, sans une ligne de
            // logique côté front.
            let amorcage = maintenant < AMORCAGE;
            if amorcage || acteur.dernier_rendu != Some(rendu) {
                if let Err(e) = render::pousser(&handle, &acteur.label, rendu) {
                    // On imprime : un acteur qui meurt en silence donne un
                    // personnage figé sans explication, et c'est exactement
                    // ce qu'on a déjà passé du temps à diagnostiquer.
                    eprintln!("rendu impossible pour {} : {e}", acteur.label);
                    continue;
                }
                acteur.dernier_rendu = Some(rendu);
            }
        }

        // ── Les départs achevés ─────────────────────────────────────────
        //
        // Du dernier au premier, pour que les index collectés pendant la
        // boucle restent valides à mesure qu'on retire.
        for i in a_retirer.into_iter().rev() {
            let parti = acteurs.remove(i);
            if let Some(w) = handle.get_webview_window(&parti.label) {
                let _ = w.destroy();
            }
            println!("« {} » est parti ({})", parti.nom, parti.label);
        }

        // Un menu contextuel a été ouvert : il a bloqué plusieurs secondes,
        // `maintenant` est périmé. On repart sur une image neuve plutôt que
        // de juger les échéances de tout le monde sur un instant faux.
        if menu_ouvert {
            continue;
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
