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
mod charge;
mod clock;
mod commandes;
mod config;
mod geom;
mod maj;
mod menu_perso;
mod overlay;
mod probe;
mod rechargement;
mod render;
mod rng;
mod roster;
mod signals;
mod sim;
mod toast;
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
/// L'écran sur lequel se trouve un point du bureau virtuel.
///
/// Sert à savoir à quelle fenêtre accrocher le menu contextuel d'un
/// personnage, et laquelle doit absorber ses clics. Rend `None` si le point
/// n'est sur aucun écran — ce qui arrive le temps d'une image quand une
/// plateforme disparaît sous lui.
///
/// ⚠️ **Les bords BAS et DROIT appartiennent à l'écran** (`<=` et non `<`), et
/// ce n'est pas un détail de confort : `pos_connue` est l'**ancre** du
/// personnage, c'est-à-dire le sol sous ses pieds. Un personnage debout sur le
/// plancher a donc `y` **exactement égal** à `work_area.bottom`.
///
/// Avec une comparaison stricte, `ecran_sous` rendait `None` pour tout
/// personnage au sol — donc **aucun menu contextuel au sol**, alors qu'il
/// marchait sur les murs, où `y` tombe au milieu de l'écran. Constaté à
/// l'écran le 2026-09-23, et invisible autrement : aucun message, le clic
/// droit ne faisait simplement rien.
///
/// Le risque symétrique — deux écrans empilés dont l'un a `bottom` égal au
/// `top` de l'autre — est sans conséquence : `find` rend le premier, et les
/// deux fenêtres couvrent ce pixel de toute façon.
fn ecran_sous(ecrans: &[probe::ScreenInfo], p: geom::Point) -> Option<u64> {
    ecrans
        .iter()
        .find(|e| {
            let z = e.work_area;
            p.x >= z.left() && p.x <= z.right() && p.y >= z.top() && p.y <= z.bottom()
        })
        .map(|e| e.id)
}

/// La table `id -> pack` en littéral JavaScript, pour `window.declarer`.
///
/// Les clés sont des chaînes parce qu'un objet JavaScript n'a pas de clés
/// numériques : `{3:"blob"}` est relu `{"3":"blob"}`. Les guillemets sont donc
/// posés ici, et `overlay.js` interroge `packs[id]` — la conversion implicite
/// de JavaScript fait le reste.
///
/// ⚠️ **Aucun échappement**, et c'est délibéré : un nom de pack est un nom de
/// dossier, validé par le catalogue à l'installation. Si cette garantie devait
/// tomber un jour, c'est ICI qu'il faudrait échapper — et nulle part ailleurs,
/// puisque c'est la seule chaîne non numérique qui traverse vers le webview.
fn table_packs_js(acteurs: &[Acteur]) -> String {
    let mut s = String::from("{");
    for (i, a) in acteurs.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&format!("\"{}\":\"{}\"", a.id, a.nom));
    }
    s.push('}');
    s
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
        // Le plugin de notification (spec §4). Enregistré même si le toast
        // n'est émis qu'une fois dans la vie de l'application : sans lui,
        // `app.notification()` échoue au lieu de rendre une erreur utile.
        //
        // Aucune permission à déclarer : le système de capabilities gouverne
        // l'API JAVASCRIPT du plugin. Ici l'émission vient de Rust, où rien
        // ne la filtre.
        .plugin(tauri_plugin_notification::init())
        // La mise à jour automatique. Le plugin lit `plugins.updater` de
        // `tauri.conf.json` : l'endpoint, et surtout la clé PUBLIQUE contre
        // laquelle il vérifie la signature de tout ce qu'il télécharge.
        .plugin(tauri_plugin_updater::Builder::new().build())
        // ── Les commandes des fenêtres (spec §9) ────────────────────────
        // `generate_handler!` engendre la table de routage à la
        // compilation. Attention : une commande oubliée ici est
        // introuvable côté JS **sans erreur de compilation** — d'où la
        // sonde d'IPC qui a validé le tuyau avant qu'on bâtisse dessus.
        .invoke_handler(tauri::generate_handler![
            commandes::installer,
            commandes::bibliotheque,
            commandes::definir_compte,
            commandes::supprimer,
            commandes::onboarding_etat,
            commandes::onboarding_terminer
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
            //
            // ⚠️ Le facteur du moniteur passe par `echelle_ecran_entiere` :
            // du pixel-art agrandi d'un facteur fractionnaire est crénelé.
            // Le réglage de l'utilisateur, lui, n'est PAS arrondi — voir le
            // pourquoi sur cette fonction.
            let echelle_affichage =
                character::attach::echelle_ecran_entiere(ecrans[0].scale) * configuration.echelle;

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

                // La taille de départ se prend sur l'image de départ : les
                // frames d'un pack n'ont pas toutes la même taille, et c'est
                // désormais celle AFFICHÉE qui dimensionne la fenêtre.
                let taille = character::attach::window_size(
                    manifeste,
                    manifeste.frame_initiale(),
                    echelle_affichage,
                );

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

                // Aucune fenêtre à créer ici : les fenêtres sont celles des
                // ÉCRANS, et la boucle les ouvre au fur et à mesure que des
                // personnages y arrivent (conception §5.1).
                acteurs.push(Acteur {
                    id: index as u32,
                    nom: nom.clone(),
                    ch: character::Character::new(manifeste.clone(), attachement, depart),
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

            // ── La vérification de mise à jour ──────────────────────────
            //
            // APRÈS `tray::installer`, et ce n'est pas un détail : c'est lui
            // qui enregistre les poignées du menu dans `Actions`. Lancée
            // avant, la vérification n'aurait rien où écrire son libellé —
            // et `signaler_maj` sortirait en silence.
            //
            // Elle ne bloque rien : tout se passe sur l'exécuteur asynchrone
            // de Tauri. Hors ligne, il ne se passe simplement rien.
            maj::verifier_en_arriere_plan(app.handle().clone(), actions.clone());

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

            // ── L'écran de démarrage, et l'assistant (spec §2, §3) ───────
            //
            // L'ordre est celui de la spec : au PREMIER lancement l'assistant
            // prime, et l'écran enregistré n'est appliqué qu'ensuite, par
            // `onboarding_terminer`. Appliquer les deux ouvrirait deux
            // fenêtres au tout premier démarrage.
            //
            // Les personnages, eux, vivent DÉJÀ : la boucle 60 Hz est lancée
            // juste au-dessus. C'est délibéré — l'utilisateur voit
            // immédiatement ce qu'il a installé, au lieu d'un bureau vide en
            // se demandant si ça marche.
            //
            // `configuration` est toujours vivante ici : la boucle n'en a
            // pris qu'un CLONE (`configuration_boucle`), pas la propriété.
            let poignee = tauri::Manager::app_handle(app).clone();

            // `SHIMEJI_ONBOARDING=1` force l'assistant sans toucher au
            // fichier : l'équivalent scriptable du premier lancement, qui
            // évite d'avoir à supprimer `config.json` entre deux essais.
            let force_assistant = std::env::var("SHIMEJI_ONBOARDING").is_ok();

            if force_assistant || !configuration.premiere_configuration_faite {
                actions::ouvrir_onboarding(&poignee);
            } else {
                // `&actions` : `actions` est un `Arc<Actions>`, et
                // `&Arc<Actions>` se déréférence tout seul en `&Actions`
                // (coercition de déréférencement).
                actions::appliquer_ecran(&actions, &poignee, configuration.ecran_au_demarrage);
            }

            // `SHIMEJI_CATALOGUE=1` ouvre la fenêtre du gestionnaire au
            // démarrage — l'équivalent scriptable de l'entrée de menu, et ce
            // qui a prouvé que l'IPC répondait avant qu'on bâtisse une
            // interface dessus (tâche 8 du catalogue).
            if std::env::var("SHIMEJI_CATALOGUE").is_ok() {
                actions::ouvrir_catalogue(&poignee);
            }

            Ok(())
        })
        // `.build()` puis `.run(closure)` au lieu du `.run(context)` d'avant :
        // c'est la seule façon d'intercepter les événements de la boucle.
        .build(tauri::generate_context!())
        .expect("échec au lancement de l'application Tauri")
        .run(|_app, evenement| {
            // Tauri termine le processus quand la DERNIÈRE fenêtre se ferme.
            // Nous vivons dans le tray : avec un roster vide — un état normal,
            // décocher le dernier personnage est permis — fermer le
            // gestionnaire tuait l'application entière, tray compris.
            //
            // ⚠️ `code: None` est ESSENTIEL. Tauri documente que le code vaut
            // `None` quand la sortie vient d'une interaction utilisateur, et
            // `Some` quand elle est demandée par `AppHandle::exit`
            // (tauri-2.11.5/src/app.rs:226-229). Le « Quitter » des deux menus
            // appelle `app.exit(0)` : il porte donc un code, et TRAVERSE ce
            // filtre. Sans lui, on rendrait l'application impossible à
            // quitter — un défaut bien pire que celui qu'on corrige.
            if let tauri::RunEvent::ExitRequested {
                api, code: None, ..
            } = evenement
            {
                api.prevent_exit();
            }
        });
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
    /// Son identité, telle que le webview la connaît.
    ///
    /// ⚠️ **Jamais réutilisée** : elle vient d'un compteur monotone, pas de
    /// l'index dans le `Vec`. Un index se réutilise quand un acteur part, et
    /// le suivant hériterait alors de la position INTERPOLÉE du précédent —
    /// il traverserait l'écran en glissant, sans qu'aucune erreur ne le
    /// signale (conception « une fenêtre par écran », `SpriteRendu::id`).
    ///
    /// Elle a remplacé le label de fenêtre le 2026-09-23 : un personnage
    /// n'est plus une fenêtre, c'est un `<img>` dans celle de son écran.
    id: u32,

    /// Le pack dont il est une instance. **Plusieurs acteurs peuvent
    /// partager le même nom** : c'est tout l'objet des doublons.
    nom: String,

    ch: character::Character,

    // ⚠️ **La mémoire de rendu a disparu le 2026-09-23.** Les quatre champs
    // `dernier_rendu`, `derniere_taille`, `dernier_coin` et
    // `clics_traversent` n'existaient que pour n'appeler Windows qu'au
    // changement. Il n'y a plus d'appel Windows par personnage : la boucle
    // empile de la donnée, et la comparaison « est-ce que ça a changé » se
    // fait une fois par ÉCRAN, sur la charge entière (`derniere_charge`).

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
    // L'instant courant, pour dériver l'image affichée — la hitbox se lit
    // désormais dans la boîte de CETTE image, dont la taille varie d'une
    // frame à l'autre sur les packs tiers.
    maintenant: std::time::Duration,
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
        let Some(pos) = character::attach::world_position(&a.ch.attachment, monde, souris) else {
            continue;
        };
        // Pose absente du manifeste : on ne peut rien situer. On passe au
        // suivant plutôt que de décider à sa place — les clics continuent de
        // traverser, ce qui ne gêne personne.
        if !a.ch.manifest.has_pose(&a.ch.pose) {
            continue;
        }

        if character::attach::hitbox_ecran(
            pos,
            a.ch.frame_courante(maintenant),
            &a.ch.pose,
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
    //
    // ⚠️ **Le risque a EMPIRÉ avec l'overlay, pas disparu.** `eval` rend `Ok`
    // dès que le message est posté : si `overlay.js` n'a pas encore défini
    // `window.poserTous`, le JavaScript lève une exception que **personne
    // n'observe**, et la charge est perdue en silence. Comme
    // `derniere_charge` l'aurait enregistrée comme envoyée, l'écran resterait
    // vide jusqu'au prochain changement — c'est-à-dire, pour un personnage
    // endormi, indéfiniment.
    //
    // D'où l'amorçage PAR ÉCRAN ci-dessous : pendant deux secondes après la
    // création d'une fenêtre, on renvoie tout à chaque tour sans se fier au
    // dédoublonnage. Trente envois de plus, une fois par fenêtre.
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

    // Le moniteur de la file du thread principal (spec « régulation de
    // charge » §5.1). `Arc` parce que la fermeture postée sur l'autre
    // thread doit en posséder une référence comptée.
    let moniteur_charge = std::sync::Arc::new(charge::Moniteur::nouveau());

    // Combien de fois `pousser` a échoué depuis la dernière trace.
    // Remplace un `eprintln!` par échec, qui sortait des centaines de fois
    // par seconde à onze personnages et noyait tout le reste.
    let mut echecs_de_rendu: u32 = 0;

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

    // La prochaine identité libre.
    //
    // ⚠️ **Monotone, et jamais remise à zéro.** L'index dans le `Vec` ne
    // peut pas servir d'identité : un index réutilisé ferait hériter le
    // nouveau venu de la position INTERPOLÉE de celui qu'il remplace, et il
    // traverserait l'écran en glissant.
    //
    // Elle part après les identités déjà posées par `setup`.
    let mut prochain_id: u32 = acteurs.len() as u32;

    // La liste des écrans, gardée d'un recensement à l'autre.
    //
    // Recensée à 8 Hz comme `monde`, mais lue à 60 Hz : l'overlay en a besoin
    // à chaque image pour répartir les sprites, et redemander `screens()` à
    // 60 Hz serait un appel système par image pour rien.
    let mut ecrans_courants: Vec<probe::ScreenInfo> = sonde.screens();

    // ── L'état de l'overlay ─────────────────────────────────────────────
    //
    // Les écrans pour lesquels une fenêtre existe. On ne les redemande pas à
    // Tauri à chaque image : `get_webview_window` est un ALLER-RETOUR vers le
    // thread principal — découverte du spike `spike-deplacements-groupes`, où
    // l'appeler 15 fois par image coûtait 66 ms par image.
    let mut ecrans_ouverts: std::collections::HashSet<u64> = std::collections::HashSet::new();

    // La dernière charge envoyée à chaque écran, pour ne rien réenvoyer
    // d'identique (conception §5.4). Un `eval` coûte ~2,9 ms de CPU : 44 par
    // seconde payés pour rien, c'est ~13 % de CPU quand tout le monde dort.
    let mut derniere_charge: std::collections::HashMap<u64, overlay::ChargeEcran> =
        std::collections::HashMap::new();

    // Qui absorbe les clics, par écran. Remplace le `clics_traversent` qui
    // vivait sur chaque acteur : c'est la FENÊTRE qui absorbe, et elle en
    // porte désormais plusieurs.
    let mut clics_traversent: std::collections::HashMap<u64, bool> =
        std::collections::HashMap::new();

    // Jusqu'à quand chaque fenêtre est en amorçage. Voir `AMORCAGE`.
    let mut amorcage_ecran: std::collections::HashMap<u64, std::time::Instant> =
        std::collections::HashMap::new();

    // Depuis quand l'écran est vide, pour ne pas fermer sa fenêtre aussitôt.
    //
    // ⚠️ **Un délai de grâce, et il n'est pas cosmétique.** Créer une fenêtre
    // WebView2 coûte des centaines de millisecondes, pendant lesquelles les
    // personnages de cet écran ne sont PAS dessinés. Sans ce délai, un
    // personnage qui fait des allers-retours sur un bord d'écran ferait
    // battre la fenêtre voisine — et clignoter les personnages qu'elle porte.
    //
    // Le coût de l'attente est nul ou presque : une fenêtre sans sprite
    // n'anime rien, et le péage de ~34 % se paie au CONTENU qui change, pas à
    // l'existence de la fenêtre (mesuré : 2,9 % pour trois fenêtres
    // immobiles).
    let mut vide_depuis: std::collections::HashMap<u64, std::time::Instant> =
        std::collections::HashMap::new();
    const GRACE_ECRAN_VIDE: Duration = Duration::from_secs(3);

    // Le prochain instant d'envoi. 15 Hz, mesuré comme le meilleur compromis
    // (spike du 2026-09-22) : 44 eval/s tiennent 3 ms de latence, là où 174
    // en coûtaient 157 % de CPU.
    let mut prochain_envoi = std::time::Instant::now();
    const PERIODE_ENVOI: Duration = Duration::from_millis(66);

    // La version de contenu courante, portée par `window.declarer`. Elle
    // change au rechargement à chaud, et sert à invalider le cache d'images
    // du webview.
    let mut version_contenu: u32 = 0;

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

    // Le label de l'acteur **qui a ouvert le dernier menu contextuel**.
    //
    // ⚠️ **C'est lui, et pas l'acteur sous le curseur, qui reçoit la
    // commande choisie.** Au moment où l'utilisateur relâche le clic sur une
    // entrée, le curseur est sur le MENU : aucun personnage n'est élu, et
    // router la commande sur l'élu revenait à la jeter — plus aucune entrée
    // de menu ne faisait quoi que ce soit. Voir `menu_perso::commande_pour`.
    //
    // Persistant d'une image à l'autre parce que le clic revient par la
    // boucle d'événements de Tauri, donc quelques images après la fermeture
    // du menu.
    let mut demandeur_du_menu: Option<String> = None;

    loop {
        // `Instant` ici et non l'horloge injectée : c'est la CADENCE, pas le
        // temps du comportement. La distinction compte — le comportement doit
        // rester pilotable par une horloge factice (spec §10.2).
        let debut = Instant::now();
        let maintenant = horloge.elapsed();

        // Le roster a-t-il changé pendant cette image ? Si oui, la table
        // `id -> pack` est republiée dans toutes les fenêtres d'écran avant
        // la prochaine charge — sans quoi `urlDe` construirait « undefined »
        // pour un nouveau venu, et un partant resterait affiché à jamais.
        //
        // Déclaré ICI, tout en haut : le bloc qui applique un changement de
        // roster est plus haut dans le corps de boucle que l'overlay qui le
        // consomme.
        let mut roster_a_declarer = false;

        // ── ~2 Hz : les signaux (spec §5.5, design étape 2 §9) ──────────
        //
        // Cinq appels système toutes les 500 ms. À comparer aux 60
        // `SetWindowPos` par seconde qui coûtent 11 points de CPU : c'est du
        // bruit. Mesuré quand même — voir `CLAUDE.md`.
        if maintenant.saturating_sub(dernier_signal) >= PERIODE_SIGNAUX {
            let mut s = sonde.signaux();

            // La sonde Win32 ne connaît pas ce signal-là : il parle de
            // NOTRE file, pas du système. On le renseigne ici, juste avant
            // `biais_de` — c'est le seul endroit du programme qui tient
            // les deux moitiés.
            s.latence_file = moniteur_charge.latence();

            // Et on relance un jeton pour la prochaine fois. Posté APRÈS
            // la lecture : le jeton en vol n'est pas celui qu'on vient de
            // lire, et attendre son retour ici bloquerait la boucle — à
            // 2 Hz, la valeur précédente est parfaitement suffisante.
            charge::sonder(&handle, &moniteur_charge);

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
                     · latence {:.0} ms → flâner ×{:.2} reposer ×{:.2} jouer ×{:.2}",
                    s.inactivite.as_secs_f32(),
                    s.appli_active.as_deref().unwrap_or("-"),
                    s.heure,
                    match s.batterie.pourcent {
                        Some(p) => format!("{p} %"),
                        None => "-".to_string(),
                    },
                    s.session_verrouillee,
                    s.latence_file.as_secs_f32() * 1000.0,
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
                // Gardée pour les 60 Hz : c'est elle que l'overlay répartit.
                ecrans_courants = ecrans.clone();
                // Gardée à part : le rechargement à chaud doit pouvoir
                // recombiner l'échelle du moniteur avec la NOUVELLE échelle
                // de la config, sans redemander les écrans.
                // Même arrondi qu'au démarrage : brancher un écran d'un
                // autre DPI ne doit pas rendre le personnage crénelé.
                ecrans_echelle = character::attach::echelle_ecran_entiere(ecrans[0].scale);
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

                        // Le webview doit oublier ses images : c'est
                        // `window.declarer` qui porte la version, et il est
                        // republié plus bas, une fois pour TOUTES les
                        // fenêtres — au lieu d'un appel par personnage.
                        //
                        // `as u32` : la version de rechargement est un `u64`,
                        // mais elle compte des rechargements à chaud d'une
                        // session — elle ne débordera jamais 32 bits.
                        version_contenu = r.version as u32;
                        roster_a_declarer = true;
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
                                    charge.manifeste.frame_initiale(),
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
                                // le `Vec` : une identité réutilisée ferait
                                // hériter le nouveau venu de la position
                                // interpolée du précédent, et il traverserait
                                // l'écran en glissant.
                                let id = prochain_id;
                                prochain_id += 1;

                                {
                                    {
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
                                            id,
                                            nom: nom.clone(),
                                            ch: character::Character::new(
                                                charge.manifeste.clone(),
                                                att,
                                                depart_pos,
                                            ),
                                            derniere_trace_grimpe: None,
                                            depart: None,
                                        });
                                        // Le webview doit connaître ce nouvel
                                        // id AVANT la prochaine charge, sinon
                                        // `urlDe` construirait « undefined »
                                        // dans l'URL de son image.
                                        roster_a_declarer = true;
                                        println!("« {nom} » apparaît (id {id})");
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
                                    // Aucune fenêtre à détruire : le sprite
                                    // disparaît parce que `window.declarer`
                                    // ne cite plus son id, et `overlay.js`
                                    // retire du DOM ce qui n'y est plus.
                                    roster_a_declarer = true;
                                    println!("« {nom} » retiré (id {})", parti.id);
                                } else {
                                    let pos = acteurs[i].ch.pos_connue;
                                    acteurs[i].depart = Some(Depart::commence(maintenant, pos));
                                    println!("« {nom} » s'en va (id {})", acteurs[i].id);
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
        let elu = elire_sous_le_curseur(&acteurs, maintenant, &monde, m.pos, echelle_affichage);

        // Le front descendant du bouton droit : calculé UNE fois, avant la
        // boucle, parce qu'il n'y a qu'une souris.
        //
        // C'est ce qui transforme un état — « le bouton est enfoncé », vrai
        // pendant les ~15 images que dure un clic humain — en un **front**,
        // qui n'arrive qu'une fois. Sans lui, maintenir le bouton rouvrirait
        // le menu en boucle dès sa fermeture.
        let front_descendant_droit = !m.right_down && bouton_droit_precedent;
        bouton_droit_precedent = m.right_down;

        // ── L'absorption des clics, par écran ───────────────────────────
        //
        // Avant le 2026-09-23, chaque personnage était une fenêtre et
        // absorbait pour lui-même. Maintenant une fenêtre porte N personnages :
        // elle absorbe si le curseur est sur **l'un** d'eux, ou si l'un d'eux
        // est porté. Le test lui-même (`elu`, calculé sur les hitbox) n'a pas
        // changé — seule la fenêtre à qui on l'applique.
        //
        // ⚠️ **Décidé ICI, avant la boucle, et pas après.** Deux raisons, et
        // la seconde a été un bug réel :
        //
        // 1. Le menu contextuel s'ouvre DANS la boucle. L'absorption doit être
        //    déjà posée quand il s'ouvre, sinon le clic droit atteint aussi
        //    l'application derrière — et l'utilisateur voit **deux** menus.
        // 2. La version précédente cherchait l'écran dans `charges`, calculé
        //    APRÈS la boucle. Or la boucle **`break`** quand un menu s'ouvre :
        //    la liste des sprites était alors partielle, l'écran introuvable,
        //    et l'absorption relâchée au pire moment.
        //
        // Elle ne dépend donc plus que de `elu` et des positions, tous deux
        // connus avant que quoi que ce soit ne bouge.
        //
        // Pendant un glisser on garde les clics absorbés même si le sprite a
        // quitté sa propre hitbox : sinon un déplacement rapide relâcherait le
        // personnage tout seul.
        let acteur_actif = elu.or_else(|| {
            acteurs
                .iter()
                .position(|a| matches!(a.ch.attachment, character::attach::Attachment::Dragged))
        });

        let ecran_absorbant =
            acteur_actif.and_then(|i| ecran_sous(&ecrans_courants, acteurs[i].ch.pos_connue));

        for id in ecrans_ouverts.iter().copied().collect::<Vec<_>>() {
            let doit_traverser = Some(id) != ecran_absorbant;
            // On n'appelle Win32 que sur CHANGEMENT : l'appeler 60 fois par
            // seconde marcherait, mais c'est un appel système par image pour
            // rien (CLAUDE.md, « Mesurer le CPU »).
            if clics_traversent.get(&id) != Some(&doit_traverser) {
                let label = render::label_ecran(id);
                if render::traverser_les_clics(&handle, &label, doit_traverser).is_ok() {
                    clics_traversent.insert(id, doit_traverser);
                }
            }
        }

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

        // Les sprites de cette image, tous acteurs confondus. Remplace les
        // trois appels Windows par personnage (`dimensionner`, `placer`,
        // `pousser`) qui bouchaient la file du thread principal.
        let mut sprites: Vec<overlay::SpriteRendu> = Vec::new();

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
                    if visible && acteur.ch.manifest.has_pose(&acteur.ch.pose) {
                        let image = acteur.ch.frame_courante(maintenant);
                        let coin = character::attach::window_top_left(
                            acteur.ch.pos_connue,
                            image,
                            &acteur.ch.pose,
                            &acteur.ch.manifest,
                            echelle_affichage,
                            acteur.ch.facing,
                        );
                        sprites.push(overlay::SpriteRendu {
                            id: acteur.id,
                            x: coin.x.round() as i32,
                            y: coin.y.round() as i32,
                            w: character::attach::window_size(
                                &acteur.ch.manifest,
                                image,
                                echelle_affichage,
                            )
                            .0,
                            h: character::attach::window_size(
                                &acteur.ch.manifest,
                                image,
                                echelle_affichage,
                            )
                            .1,
                            image,
                            flip: acteur.ch.facing.flipped(),
                        });
                        placements_depuis_trace += 1;
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
            // ⚠️ **L'absorption ne se décide plus ici.** Un personnage
            // n'est plus une fenêtre : c'est la fenêtre de son ÉCRAN qui
            // absorbe, et elle en porte plusieurs. La décision est donc
            // prise une fois par écran, après cette boucle — le test lui
            // même (`sur_le_personnage`, `porte`) est rigoureusement le
            // même, seule la fenêtre à qui on l'applique a changé.
            //
            // `porte` reste calculé ici parce que l'acteur est sous la main ;
            // il sert plus bas, via `acteur_actif`.
            let _ = porte;

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
                // La fenêtre à qui le menu s'accroche est celle de l'ÉCRAN
                // du personnage, pas la sienne — il n'en a plus.
                //
                // `let … else` : si l'écran n'a pas (ou plus) de fenêtre,
                // il n'y a pas de menu à ouvrir. On passe au suivant plutôt
                // que de chercher un repli : `label_ecran(0)` désignerait
                // une fenêtre qui n'existe pas, donc un menu muet.
                let Some(id_ecran) = ecran_sous(&ecrans_courants, acteur.ch.pos_connue) else {
                    continue;
                };
                let Some(win) = handle.get_webview_window(&render::label_ecran(id_ecran)) else {
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
                // Mémorisé MAINTENANT : c'est la seule image où l'on sait
                // encore qui a fait le clic droit.
                // L'identité, puisqu'il n'y a plus de label de fenêtre.
                // C'est une clé opaque pour `menu_perso` : il ne fait que la
                // comparer, il n'en tire rien.
                demandeur_du_menu = Some(acteur.id.to_string());

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
                // La commande n'est donnée qu'à l'acteur qui a OUVERT le
                // menu, et une seule fois. La donner à tous ferait exécuter
                // l'entrée par N personnages — dont N−1 qui n'ont rien
                // demandé ; la donner à l'acteur sous le curseur ne la
                // donnait à personne, le curseur étant sur le menu.
                commande: menu_perso::commande_pour(
                    &mut demandeur_du_menu,
                    &mut commande_du_menu,
                    &acteur.id.to_string(),
                ),
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

            // Caché ou session verrouillée : le comportement a tourné
            // ci-dessus, et on s'arrête là. `visible` est lu une fois pour
            // tous, avant la boucle. N'empiler aucun sprite suffit : toutes
            // les fenêtres se fermeront plus bas, faute d'écran occupé — ce
            // qui rend le mode caché réellement gratuit.
            if !visible {
                continue;
            }

            // ── 60 Hz : empiler, au lieu d'appeler Windows ──────────────
            //
            // La position est DÉRIVÉE à chaque image (décision n° 1) : c'est
            // elle qui rend gratuit le déplacement d'une plateforme sous les
            // pieds du personnage, et elle n'a pas changé d'un iota.
            //
            // Ce qui a changé, c'est ce qu'on en fait. Avant : jusqu'à trois
            // appels Windows par acteur et par image (`dimensionner`,
            // `placer`, `pousser`), soit jusqu'à 900 messages par seconde à
            // 15 personnages — ce qui bouchait la file du thread principal et
            // lui donnait 8 à 14 SECONDES de retard. Maintenant : un
            // `Vec::push`, et un seul `eval` par écran à 15 Hz.
            let Some(pos) = character::attach::world_position(&acteur.ch.attachment, &monde, m.pos)
            else {
                continue;
            };
            if !acteur.ch.manifest.has_pose(&acteur.ch.pose) {
                continue;
            }

            let image = acteur.ch.frame_courante(maintenant);

            // La taille dépend du manifeste, de l'échelle de l'écran — et
            // depuis le 2026-09-20 de l'IMAGE affichée, les frames d'un pack
            // tiers n'ayant pas toutes la même taille. Elle n'est plus testée
            // « au changement » : elle voyage dans la charge utile, et c'est
            // `overlay.js` qui ne réécrit le style que si elle a bougé.
            let taille =
                character::attach::window_size(&acteur.ch.manifest, image, echelle_affichage);

            let coin = character::attach::window_top_left(
                pos,
                image,
                &acteur.ch.pose,
                &acteur.ch.manifest,
                echelle_affichage,
                acteur.ch.facing,
            );

            sprites.push(overlay::SpriteRendu {
                id: acteur.id,
                // Arrondi ICI et nulle part ailleurs : c'est cet entier que
                // `overlay.js` compare pour savoir s'il doit réécrire, et le
                // calculer deux fois serait deux occasions de divergence.
                x: coin.x.round() as i32,
                y: coin.y.round() as i32,
                w: taille.0,
                h: taille.1,
                image,
                flip: acteur.ch.facing.flipped(),
            });
            placements_depuis_trace += 1;
        }

        // ── Les départs achevés ─────────────────────────────────────────
        //
        // Du dernier au premier, pour que les index collectés pendant la
        // boucle restent valides à mesure qu'on retire.
        for i in a_retirer.into_iter().rev() {
            let parti = acteurs.remove(i);
            // Plus de fenêtre à détruire : le sprite disparaît parce que
            // `window.declarer` ne cite plus son id.
            roster_a_declarer = true;
            println!("« {} » est parti (id {})", parti.nom, parti.id);
        }

        // ── L'overlay : une fenêtre par écran occupé ────────────────────
        //
        // Tout ce qui suit remplace les appels Windows que la boucle par
        // acteur faisait jusqu'au 2026-09-23. Conception :
        // `docs/specs/2026-09-23-fenetre-par-ecran-design.md`.
        //
        // `repartir` émet un sprite à cheval dans les DEUX écrans qu'il
        // touche (§5.2), et n'émet aucune charge pour un écran vide (§5.1).
        let charges = overlay::repartir(&sprites, &ecrans_courants);

        let occupes: std::collections::HashSet<u64> = charges.iter().map(|c| c.ecran).collect();

        // Fermer les fenêtres des écrans que plus personne n'habite. Le péage
        // mesuré est de ~34 % par fenêtre ANIMÉE : un écran vide doit être
        // gratuit, et une fenêtre fermée l'est tout à fait.
        //
        // `collect` en `Vec` d'abord : on ne peut pas modifier
        // `ecrans_ouverts` pendant qu'on l'itère.
        // Un écran de nouveau occupé n'est plus candidat à la fermeture.
        for id in &occupes {
            vide_depuis.remove(id);
        }

        let candidats: Vec<u64> = ecrans_ouverts.difference(&occupes).copied().collect();
        for id in candidats {
            // Premier tour où il est vide : on note l'heure et on attend.
            let depuis = *vide_depuis
                .entry(id)
                .or_insert_with(std::time::Instant::now);
            if depuis.elapsed() < GRACE_ECRAN_VIDE {
                continue;
            }
            render::detruire_fenetre_ecran(&handle, id);
            ecrans_ouverts.remove(&id);
            derniere_charge.remove(&id);
            clics_traversent.remove(&id);
            amorcage_ecran.remove(&id);
            vide_depuis.remove(&id);
        }

        // Ouvrir celles qui viennent d'être occupées.
        for c in &charges {
            if ecrans_ouverts.contains(&c.ecran) {
                continue;
            }
            // `let … else` : l'écran a disparu entre la répartition et ici
            // (débranchement). On saute, la prochaine image s'en occupera.
            let Some(e) = ecrans_courants.iter().find(|e| e.id == c.ecran) else {
                continue;
            };
            match render::creer_fenetre_ecran(&handle, e) {
                Ok(()) => {
                    ecrans_ouverts.insert(c.ecran);
                    amorcage_ecran.insert(c.ecran, std::time::Instant::now() + AMORCAGE);
                    // La table des packs doit arriver AVANT la première
                    // charge : sans elle, `urlDe` construirait « undefined »
                    // dans l'URL de l'image, et l'on verrait un personnage
                    // parfaitement animé… sans aucun dessin.
                    roster_a_declarer = true;
                }
                Err(msg) => eprintln!("écran {} : {msg}", c.ecran),
            }
        }

        // Une fenêtre encore en amorçage doit tout recevoir à chaque tour :
        // son premier `declarer` a pu être perdu (voir `AMORCAGE`).
        let en_amorcage: std::collections::HashSet<u64> = amorcage_ecran
            .iter()
            .filter(|(_, fin)| std::time::Instant::now() < **fin)
            .map(|(id, _)| *id)
            .collect();

        // Republier la table `id -> pack` quand le roster a bougé.
        if (roster_a_declarer || !en_amorcage.is_empty()) && !ecrans_ouverts.is_empty() {
            let table = table_packs_js(&acteurs);
            for id in &ecrans_ouverts {
                let _ = render::declarer_packs(&handle, *id, &table, version_contenu);
            }
            // Les charges mémorisées ne valent plus rien : le webview vient
            // de remettre à zéro ce qu'il sait. Sans cet oubli, §5.4
            // sauterait le prochain envoi et l'écran resterait vide.
            derniere_charge.clear();
        }

        // ── L'envoi, à 15 Hz et seulement sur changement ────────────────
        if std::time::Instant::now() >= prochain_envoi {
            prochain_envoi = std::time::Instant::now() + PERIODE_ENVOI;

            for c in &charges {
                // §5.4 : un `eval` coûte ~2,9 ms de CPU. Ne rien envoyer
                // quand rien n'a changé rend gratuit le cas « tout le monde
                // dort », qui est celui de la nuit et de l'utilisateur parti.
                // Le dédoublonnage (§5.4) ne s'applique PAS pendant
                // l'amorçage : c'est précisément là que l'envoi précédent a
                // pu se perdre sans le dire.
                if !en_amorcage.contains(&c.ecran) && derniere_charge.get(&c.ecran) == Some(c) {
                    continue;
                }
                if render::pousser_ecran(&handle, c).is_ok() {
                    derniere_charge.insert(c.ecran, c.clone());
                } else {
                    // ⚠️ **Compté, pas imprimé.** Un message par échec noyait
                    // tout le reste à onze personnages. Et l'échec n'est PAS
                    // anodin : il veut dire que la file du thread principal a
                    // débordé (spec « régulation de charge » §2).
                    echecs_de_rendu += 1;
                }
            }
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
                // ⚠️ **Le taux est rapporté au nombre d'ACTEURS.**
                //
                // `placements_depuis_trace` compte tous les acteurs
                // confondus : à 10 personnages, il vaut jusqu'à 10 fois le
                // nombre d'images, et le pourcentage brut donnait « 550 % »
                // — un chiffre qui ne veut rien dire et qu'on croirait à une
                // anomalie. Divisé par le nombre d'acteurs, il redevient ce
                // qu'il a toujours été : la proportion d'images où UN
                // personnage a bougé.
                //
                // `max(1)` : sans aucun personnage à l'écran (cas normal
                // depuis cette étape), on ne divise pas par zéro.
                let acteurs_ici = acteurs.len().max(1) as f64;
                println!(
                    "cadence : {:.1} img/s, travail moyen {:.0} µs, \
                     {} placements sur {} images × {} acteurs ({:.0} % par acteur)",
                    images_depuis_trace as f64 / secondes,
                    moyenne_us,
                    placements_depuis_trace,
                    images_depuis_trace,
                    acteurs.len(),
                    placements_depuis_trace as f64 * 100.0
                        / (images_depuis_trace as f64 * acteurs_ici)
                );
                // N'afficher que si ce n'est pas zéro : une ligne
                // « 0 échec » toutes les cinq secondes serait du bruit, et
                // c'est l'APPARITION du chiffre qui doit sauter aux yeux.
                if echecs_de_rendu > 0 {
                    println!(
                        "  {echecs_de_rendu} images perdues : la file du thread principal a débordé (latence {:.0} ms)",
                        moniteur_charge.latence().as_secs_f32() * 1000.0
                    );
                }
                echecs_de_rendu = 0;

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

// ─────────────────────────────────────────────────────────────────────────
// Les rares fonctions de `main.rs` qui se testent sans écran.
//
// Le reste du fichier est une boucle qui pilote Windows : il ne se vérifie
// qu'à l'œil, ou par les modules qu'il appelle. `ecran_sous`, elle, est
// purement géométrique — et son bord bas a déjà coûté un bug.
// ─────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests_main {
    use super::*;
    use crate::geom::Rect;

    fn un_ecran() -> Vec<probe::ScreenInfo> {
        vec![probe::ScreenInfo {
            id: 42,
            // Zone de travail : 1032 et non 1080, la barre des tâches prenant
            // les 48 derniers pixels.
            work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
            scale: 1.0,
        }]
    }

    #[test]
    fn un_personnage_au_sol_appartient_a_son_ecran() {
        // ⚠️ **Le test qui compte.** `pos_connue` est l'ANCRE du personnage,
        // c'est-à-dire le sol sous ses pieds : debout sur le plancher, son `y`
        // vaut EXACTEMENT `work_area.bottom`.
        //
        // Avec une comparaison stricte (`y < bottom`), `ecran_sous` rendait
        // `None` — et le clic droit ne faisait alors strictement rien au sol,
        // sans le moindre message, alors qu'il marchait sur les murs.
        let p = geom::Point::new(500.0, 1032.0);
        assert_eq!(ecran_sous(&un_ecran(), p), Some(42));
    }

    #[test]
    fn un_personnage_contre_le_bord_droit_appartient_a_son_ecran() {
        // Même raisonnement pour un personnage accroché au mur de droite :
        // son ancre est la main qui agrippe, donc pile sur le bord.
        let p = geom::Point::new(1920.0, 400.0);
        assert_eq!(ecran_sous(&un_ecran(), p), Some(42));
    }

    #[test]
    fn un_point_hors_de_tout_ecran_ne_rend_aucun_ecran() {
        let p = geom::Point::new(9000.0, 9000.0);
        assert_eq!(ecran_sous(&un_ecran(), p), None);
    }

    #[test]
    fn le_coin_haut_gauche_appartient_a_l_ecran() {
        assert_eq!(ecran_sous(&un_ecran(), geom::Point::new(0.0, 0.0)), Some(42));
    }
}
