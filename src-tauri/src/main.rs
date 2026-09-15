// Pas de console en release, mais on la garde en debug.
//
// `cfg_attr(not(debug_assertions), â€¦)` plutÃ´t que l'attribut nu : le mode
// simulation (`--sim`), la sous-commande `--demarrage` et les traces de
// diagnostic (`SHIMEJI_CADENCE`, `SHIMEJI_TRACE`) Ã©crivent tous sur la sortie
// standard. Les priver de console en debug les rendrait muets.
//
// **Possible seulement depuis la TÃ¢che 1** : sans Â« Quitter Â» dans le tray,
// une application sans console ne se fermerait plus du tout.
//
// âš ï¸ Un attribut `#![â€¦]` de niveau *crate* doit Ãªtre la PREMIÃˆRE chose du
// fichier â€” avant les commentaires de module et les `mod`. Le placer aprÃ¨s
// donne Â« inner attribute is not permitted following an outer attribute Â».
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// AmorÃ§age de l'application.
//
// Deux modes : l'application (fenÃªtre, boucle 60 Hz) et le mode simulation
// (`--sim`, sans Ã©cran, spec Â§10.3). Une sous-commande et non un second
// binaire : deux cibles binaires d'un mÃªme paquet ne partagent du code que
// par une cible `lib`, et ajouter un `lib.rs` pour cela seul rÃ©organiserait
// tout le projet.
//
// Pas de `#![windows_subsystem = "windows"]` pour le moment : on VEUT la
// console pendant le dÃ©veloppement (topologie des Ã©crans, trace du
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

// En Rust, **les mÃ©thodes d'un trait ne sont visibles que si le trait est
// dans la portÃ©e** â€” mÃªme quand on dÃ©tient le type concret qui l'implÃ©mente.
// Sans ces deux lignes, `sonde.screens()` et `horloge.elapsed()` donnent
// Â« no method named â€¦ found for struct Win32Probe Â», ce qui laisse croire
// que la mÃ©thode n'existe pas alors qu'il manque seulement un `use`.
//
// C'est le pendant de Â« les traits sont ouverts Â» : n'importe qui peut
// ajouter un trait Ã  un type, donc le compilateur ne cherche que parmi ceux
// qu'on a explicitement fait entrer.
use clock::Clock;
use probe::SystemProbe;

fn main() {
    // AVANT TOUT LE RESTE. Sans cet appel, Windows virtualise les
    // coordonnÃ©es et la sonde rendrait des pixels logiques en croyant rendre
    // des pixels physiques (spec Â§3.4). Voir le commentaire de la fonction.
    probe::win32::activer_conscience_dpi();

    // Aiguillage minimal, Ã©crit Ã  la main : une crate d'analyse d'arguments
    // pour deux drapeaux serait disproportionnÃ©e, et le plan 1b n'en ajoutera
    // pas d'autres (les rÃ©glages iront dans config.json).
    let args: Vec<String> = std::env::args().collect();

    // â”€â”€ `--installer <slug>` â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    //
    // L'Ã©quivalent scriptable du clic dans la grille du catalogue, selon la
    // rÃ¨gle du projet : tout ce qui demanderait un clic reÃ§oit un Ã©quivalent
    // en ligne de commande.
    //
    // Il vÃ©rifie en prime le seul morceau que les tests ne couvrent pas â€”
    // **que le CDN rÃ©ponde bien ce qu'on croit**, et que `ReseauWinHttp`
    // sache lui parler. Les tests, eux, ne voient que le faux rÃ©seau.
    if let Some(pos) = args.iter().position(|a| a == "--installer") {
        let Some(slug) = args.get(pos + 1) else {
            eprintln!("usage : --installer <slug>");
            std::process::exit(2);
        };

        println!("installation de Â« {slug} Â»â€¦");
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
                println!("installÃ© : {}", chemin.display());
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Ã©chec : {e}");
                std::process::exit(1);
            }
        }
    }

    if let Some(i) = args.iter().position(|a| a == "--sim") {
        // `get(i + 1)` puis `parse` : une valeur absente ou illisible vaut
        // 30 minutes plutÃ´t qu'une erreur â€” c'est un outil de dÃ©veloppement.
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

        // La MÃŠME rÃ©solution que l'application : la simulation doit jouer
        // avec le `blob` que le programme afficherait rÃ©ellement, sinon elle
        // mesurerait le comportement d'un autre personnage que celui qu'on
        // observe Ã  l'Ã©cran.
        let Some(dossier) = config::dossier_du_personnage("blob") else {
            eprintln!("simulation impossible : personnage Â« blob Â» introuvable");
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

    // Sous-commande de diagnostic du dÃ©marrage automatique.
    //
    // Elle existe pour une raison prÃ©cise : le basculement se fait par une
    // case du tray, donc **un clic humain**, et le code du registre ne
    // serait autrement vÃ©rifiable qu'Ã  la main. Un test unitaire ne convient
    // pas non plus â€” il modifierait le registre de la machine qui exÃ©cute la
    // suite.
    //
    // Effet de bord utile : elle rend le rÃ©glage scriptable.
    if let Some(i) = args.iter().position(|a| a == "--demarrage") {
        match args.get(i + 1).map(|s| s.as_str()) {
            Some("on") => match autostart::activer() {
                Ok(()) => println!("dÃ©marrage avec Windows : activÃ©"),
                Err(e) => {
                    eprintln!("Ã©chec : {e}");
                    std::process::exit(1);
                }
            },
            Some("off") => match autostart::desactiver() {
                Ok(()) => println!("dÃ©marrage avec Windows : dÃ©sactivÃ©"),
                Err(e) => {
                    eprintln!("Ã©chec : {e}");
                    std::process::exit(1);
                }
            },
            _ => println!(
                "dÃ©marrage avec Windows : {}",
                if autostart::est_actif() {
                    "actif"
                } else {
                    "inactif"
                }
            ),
        }
        return;
    }

    // `--signaux` : imprime l'instantanÃ© de la sonde et sort.
    //
    // C'est la seule vÃ©rification possible des cinq appels Windows : aucun
    // test ne peut savoir depuis combien de temps l'utilisateur n'a rien
    // touchÃ©. Rendue scriptable plutÃ´t que laissÃ©e Ã  l'Å“il, comme le reste
    // du projet â€” on peut la lancer deux fois Ã  5 s d'intervalle et vÃ©rifier
    // que l'inactivitÃ© a bien augmentÃ© de 5 s.
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

/// Le label de la fenÃªtre d'un personnage. Un seul personnage Ã  l'Ã©tape 1a ;
/// l'Ã©tape 3 en instanciera plusieurs, d'oÃ¹ l'index dÃ¨s maintenant.
fn label_de(index: usize) -> String {
    format!("pet-{index}")
}

/// CrÃ©e la fenÃªtre d'UN personnage, avec toutes ses propriÃ©tÃ©s.
///
/// Extraite de `setup` : les fenÃªtres naissent dÃ©sormais **en cours
/// d'exÃ©cution**, depuis le thread de la boucle, et plus seulement au
/// dÃ©marrage (design Â« plusieurs personnages Â» Â§3).
///
/// C'est lÃ©gitime : `RuntimeHandle` de Tauri poste un message au thread
/// principal quand on l'appelle depuis un autre thread. Ã€ l'inverse,
/// `tauri-runtime-wry` panique explicitement si `WindowMessage::Close` est
/// traitÃ© *sur* le thread principal (`lib.rs:3492`) â€” c'est-Ã -dire si l'on
/// ferme une fenÃªtre depuis un gestionnaire d'Ã©vÃ©nements. Notre boucle Ã©tant
/// un thread Ã  part, elle est du bon cÃ´tÃ©. **VÃ©rifiÃ© par un spike jetable**
/// (20 cycles crÃ©ation/destruction) et pas seulement dÃ©duit des sources.
///
/// âš ï¸ **Les deux styles Ã©tendus sont posÃ©s ICI et nulle part ailleurs.** Les
/// oublier sur les fenÃªtres nÂ° 2 et suivantes donnerait des personnages qui
/// volent le focus et apparaissent dans Alt+Tab â€” un dÃ©faut qui ne se verrait
/// que sur le deuxiÃ¨me personnage, donc jamais pendant une mise au point Ã 
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
        // Le fragment dit Ã  `pet.js` quel personnage servir, et il doit
        // porter le nom **du pack**, pas `blob` en dur.
        //
        // Le bug que Ã§a corrige est sournois : avec `#blob` fixe, le
        // manifeste chargÃ© Ã©tait bien celui du personnage demandÃ© (bonnes
        // poses, bonnes ancres, bonne hitbox) mais le webview rÃ©clamait
        // `shime:///blob/N` â€” donc les **images** de blob. Rien ne le
        // signalait : aucune erreur, aucune trace, un personnage
        // parfaitement animÃ©â€¦ avec le mauvais dessin.
        //
        // Invisible tant que `blob` Ã©tait le seul pack livrÃ©. ConstatÃ© Ã 
        // l'Å“il en ajoutant `luffy`, et par aucun autre moyen.
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
    .map_err(|e| format!("fenÃªtre Â« {label} Â» : {e}"))?;

    // Les clics traversent en permanence ; la boucle ne les rÃ©active que
    // dans la hitbox de la pose courante (spec Â§3.3).
    win.set_ignore_cursor_events(true)
        .map_err(|e| format!("clics traversants sur Â« {label} Â» : {e}"))?;

    // **Les deux dÃ©couvertes de l'Ã©tape 0.** Ã€ faire avant que la hitbox
    // n'existe : sinon le vol de focus apparaÃ®trait en mÃªme temps que
    // l'attrapabilitÃ©, et les deux se diagnostiqueraient ensemble, pour rien.
    match render::appliquer_styles_etendus(&win) {
        Ok(()) => println!("styles Ã©tendus posÃ©s sur {label} (NOACTIVATE, TOOLWINDOW)"),
        // Non bloquant : la fenÃªtre marche sans, elle est seulement moins
        // polie. Mieux vaut un personnage qui vole le focus qu'aucun
        // personnage.
        Err(e) => eprintln!("styles Ã©tendus NON appliquÃ©s sur {label} : {e}"),
    }

    Ok(win)
}

fn lancer_application() {
    let dossier = config::dossier_personnages();
    println!("personnages : {}", dossier.display());

    // â”€â”€ La configuration â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    // `charger()` ne peut pas Ã©chouer : ni l'absence de fichier, ni un
    // fichier partiel, ni mÃªme un JSON cassÃ© n'empÃªchent le personnage de
    // vivre (spec Â§9.3). Un fichier malformÃ© est signalÃ© bruyamment par
    // `config`, pas ici.
    let configuration = config::charger();
    let reglages = config::Reglages::depuis(&configuration);
    println!(
        "config : Ã©chelle {}, vitesse Ã—{} (marche {} px/s)",
        configuration.echelle, configuration.vitesse, reglages.vitesse_marche
    );

    // â”€â”€ Le roster voulu, tel que la config le dit â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    //
    // `personnages` est un **multi-ensemble** : une rÃ©pÃ©tition vaut un
    // exemplaire de plus (design Â§4). La liste vide est permise â€” zÃ©ro
    // personnage est un Ã©tat normal, pas une erreur de configuration : on
    // dÃ©marre, le tray vit, et un clic dans la bibliothÃ¨que ramÃ¨ne quelqu'un.
    let mut roster: Vec<String> = configuration.personnages.clone();

    // â”€â”€ Les manifestes, avant tout le reste â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    //
    // Un par nom DISTINCT : trois blob, c'est un seul `mascot.json` lu et
    // trois `Manifest::clone`. `BTreeSet` plutÃ´t que `HashSet` pour que
    // l'ordre de lecture â€” et donc l'ordre des messages â€” soit le mÃªme d'un
    // dÃ©marrage Ã  l'autre.
    //
    // Un nom introuvable ou illisible est signalÃ© **bruyamment** et retirÃ© du
    // roster : les autres personnages doivent vivre. Ã‰chouer ici sur un seul
    // pack effacÃ© Ã  la main rendrait toute l'application inutilisable.
    let mut manifestes: std::collections::BTreeMap<String, character::manifest::Manifest> =
        std::collections::BTreeMap::new();

    for nom in roster.iter().collect::<std::collections::BTreeSet<_>>() {
        let Some(dossier) = config::dossier_du_personnage(nom) else {
            eprintln!("personnage Â« {nom} Â» introuvable (ni bibliothÃ¨que, ni dossier livrÃ©)");
            continue;
        };
        match character::manifest::Manifest::load(&dossier) {
            Ok(m) => {
                println!("personnage chargÃ© : {} ({} poses)", m.name, m.poses.len());
                manifestes.insert(nom.clone(), m);
            }
            Err(e) => eprintln!("personnage Â« {nom} Â» illisible, ignorÃ© : {e}"),
        }
    }

    // Les noms qui n'ont pas chargÃ© sortent du roster : sans ce filtre, la
    // rÃ©conciliation redemanderait leur crÃ©ation Ã  chaque passage Ã  8 Hz,
    // indÃ©finiment.
    roster.retain(|n| manifestes.contains_key(n));

    if roster.is_empty() {
        println!("aucun personnage actif â€” ils s'activent depuis Â« Ma bibliothÃ¨que Â»");
    }

    // La table d'envies, rÃ©glÃ©e par la config (dÃ©cision nÂ° 5). Construite
    // une fois : elle ne change qu'au rechargement Ã  chaud (TÃ¢che 5).
    let table = behavior::desire::TableEnvies::depuis_config(&configuration);
    let echelle_config = configuration.echelle;

    tauri::Builder::default()
        // â”€â”€ Les trois commandes de la fenÃªtre du catalogue (spec Â§9) â”€â”€â”€â”€
        // `generate_handler!` engendre la table de routage Ã  la
        // compilation. Attention : une commande oubliÃ©e ici est
        // introuvable cÃ´tÃ© JS **sans erreur de compilation** â€” d'oÃ¹ la
        // sonde d'IPC qui a validÃ© le tuyau avant qu'on bÃ¢tisse dessus.
        .invoke_handler(tauri::generate_handler![
            commandes::installer,
            commandes::bibliotheque,
            commandes::definir_compte,
            commandes::supprimer
        ])
        // â”€â”€ Le schÃ©ma URI qui sert les PNG externes â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        // Les personnages sont des fichiers externes au binaire (spec Â§8.1),
        // donc aucun chemin relatif du webview ne peut les atteindre. Rust
        // les sert lui-mÃªme. Sur Windows, ce schÃ©ma est accessible sous
        // http://shime.localhost/<personnage>/<numÃ©ro>.
        //
        // VÃ©rifiÃ© : `Builder::register_uri_scheme_protocol`
        // (tauri-2.11.5/src/app.rs:2130) ; l'hÃ´te `.localhost` sur Windows
        // (src/manager/mod.rs:342).
        // Plus de dossier capturÃ© : `servir_frame` rÃ©sout lui-mÃªme le
        // personnage nommÃ© dans l'URL, par la MÃŠME rÃ¨gle que le chargement
        // (bibliothÃ¨que puis dossier livrÃ©). Capturer un dossier unique
        // reviendrait Ã  ne pouvoir servir qu'une seule des deux racines.
        .register_uri_scheme_protocol("shime", move |_ctx, requete| {
            servir_frame(requete.uri().path())
        })
        .setup(move |app| {
            // â”€â”€ La topologie, et le monde â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
            let sonde = probe::win32::Win32Probe::new();
            probe::win32::imprimer_diagnostic(&sonde);

            let ecrans = sonde.screens();
            let monde = world::World::from_screens(&ecrans);
            if monde.platforms().is_empty() {
                eprintln!("aucun Ã©cran : rien Ã  faire.");
                return Ok(());
            }

            // â”€â”€ L'alÃ©atoire, semÃ© UNE SEULE FOIS â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
            //
            // SemÃ© ici et non dans la boucle, parce que l'apparition en a
            // besoin avant que la boucle n'existe â€” et surtout parce qu'il ne
            // doit y en avoir **qu'un**. Un `XorShift32::seeded(index)` par
            // personnage retomberait en plein dans le piÃ¨ge des graines
            // sÃ©quentielles (CLAUDE.md) : tous apparaÃ®traient au mÃªme `x` et
            // tireraient la mÃªme premiÃ¨re envie.
            //
            // Graine issue de l'horloge systÃ¨me : deux lancements ne doivent
            // pas donner la mÃªme histoire. C'est le seul endroit du programme
            // oÃ¹ l'alÃ©atoire n'est pas reproductible, et c'est voulu â€” le
            // mode simulation, lui, prend une graine explicite.
            let graine = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(12345);
            let mut rng = rng::XorShift32::seeded(graine);

            // L'Ã©chelle d'AFFICHAGE : celle du moniteur, multipliÃ©e par le
            // rÃ©glage de l'utilisateur. Les fonctions de `attach` n'ont pas Ã 
            // savoir que le second existe â€” elles reÃ§oivent un seul facteur.
            let echelle_affichage = ecrans[0].scale * configuration.echelle;

            // â”€â”€ Un acteur, et une fenÃªtre, par personnage du roster â”€â”€â”€â”€â”€â”€
            //
            // Ils TOMBENT tous du haut de l'Ã©cran, Ã  des `x` tirÃ©s au sort.
            // Presque rien Ã  Ã©crire, et c'est la dÃ©cision nÂ° 1 qui le
            // permet : `Attachment::Falling` existe dÃ©jÃ , et les rÃ©flexes Ã 
            // 60 Hz gÃ¨rent chute puis atterrissage depuis l'Ã©tape 4a.
            let mut acteurs: Vec<Acteur> = Vec::new();

            for (index, nom) in roster.iter().enumerate() {
                // `expect` impossible Ã  dÃ©clencher : `roster` a Ã©tÃ© filtrÃ©
                // sur `manifestes` juste avant. On prÃ©fÃ¨re quand mÃªme sauter
                // plutÃ´t que paniquer dans `setup`.
                let Some(manifeste) = manifestes.get(nom) else {
                    continue;
                };

                let taille = character::attach::window_size(manifeste, echelle_affichage);

                // `let â€¦ else` : sans Ã©cran, il n'y a nulle part oÃ¹ le faire
                // apparaÃ®tre. Un monde vide est un cas normal (session
                // distante en cours d'Ã©tablissement).
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
                    // Inatteignable â€” `point_de_chute` ne rend que `Falling`.
                    _ => geom::Point::new(0.0, 0.0),
                };

                let label = label_de(index);
                if let Err(e) =
                    creer_fenetre_personnage(&app.handle().clone(), &label, nom, taille)
                {
                    eprintln!("Â« {nom} Â» n'a pas pu apparaÃ®tre : {e}");
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
                    // incliquable jusqu'au prochain changement d'Ã©tat.
                    clics_traversent: true,
                    derniere_trace_grimpe: None,
                    depart: None,
                });
            }

            // â”€â”€ Le tray â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
            // InstallÃ© AVANT la boucle : si le tray Ã©choue, on veut le savoir
            // tout de suite, pas aprÃ¨s avoir dÃ©marrÃ© un thread.
            //
            // Non bloquant si Ã§a Ã©choue : une application sans tray reste
            // utilisable (au prix d'un `Stop-Process`), alors qu'aucune
            // application ne l'est du tout.
            let visibilite = tray::nouvelle_visibilite();
            let demande = rechargement::nouvelle_demande();

            // La boÃ®te aux lettres du menu contextuel, vers la boucle 60 Hz.
            let commande = actions::nouvelle_commande();

            // Tout ce dont les DEUX menus ont besoin pour agir, en un seul
            // objet partagÃ© â€” voir l'en-tÃªte d'`actions.rs`.
            //
            // Le ROSTER et non plus un personnage unique : Â« choisir Â» n'est
            // plus Â« remplacer Â» mais Â« ajouter ou retirer Â» (design Â§4).
            let actions = actions::Actions::nouvelles(
                visibilite.clone(),
                demande.clone(),
                roster.clone(),
                commande.clone(),
            );

            // `manage` met la valeur Ã  disposition des commandes, qui la
            // reÃ§oivent par un paramÃ¨tre `State<â€¦>`. C'est le mÃ©canisme
            // d'injection de Tauri â€” il Ã©vite une variable globale, et c'est
            // ainsi que les commandes de la bibliotheque atteignent le roster.
            tauri::Manager::manage(app, actions.clone());

            // â”€â”€ Deux Ã©quivalents scriptables des clics de la bibliothÃ¨que â”€
            //
            // RÃ¨gle du projet : tout ce qui demanderait un clic en reÃ§oit un.
            // Ces deux variables sont ce qui permet de vÃ©rifier l'Ã©tape
            // Â« plusieurs personnages Â» **sans humain**.

            // `SHIMEJI_PERSONNAGES=blob,blob,luffy` : le roster de dÃ©part.
            // Les doublons sont CONSERVÃ‰S â€” deux blob veut dire deux blob.
            // Il est appliquÃ© comme un changement ordinaire, donc les
            // personnages en trop tombent du haut de l'Ã©cran comme si on
            // venait de les activer.
            if let Ok(liste) = std::env::var("SHIMEJI_PERSONNAGES") {
                let voulus: Vec<String> = liste
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                match actions.definir_roster(&voulus, false) {
                    Ok(v) => println!("[diag] roster de dÃ©part {voulus:?} -> version {v}"),
                    // Bruyant et non fatal : on veut voir POURQUOI le roster
                    // demandÃ© n'a pas pris, plutÃ´t que dÃ©marrer sur un Ã©cran
                    // vide sans explication.
                    Err(e) => eprintln!("[diag] roster de dÃ©part refusÃ© : {e}"),
                }
            }

            // `SHIMEJI_ROSTER=8:blob,luffy` : un changement de roster aprÃ¨s
            // 8 secondes â€” le clic dans la bibliothÃ¨que, **y compris une
            // dÃ©sactivation**. C'est le seul moyen d'observer le DÃ‰PART sans
            // qu'un humain clique.
            if let Ok(valeur) = std::env::var("SHIMEJI_ROSTER") {
                // `split_once` : la forme est `<secondes>:<liste>`. Une
                // valeur mal formÃ©e est signalÃ©e et ignorÃ©e, plutÃ´t que
                // d'agir tout de suite â€” ce qui ressemblerait Ã  un succÃ¨s.
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
                                    Err(e) => eprintln!("[diag] roster refusÃ© : {e}"),
                                }
                            });
                        }
                        Err(_) => eprintln!("SHIMEJI_ROSTER : Â« {secondes} Â» n'est pas un dÃ©lai"),
                    },
                    None => eprintln!("SHIMEJI_ROSTER : forme attendue <secondes>:<liste>"),
                }
            }

            if let Err(e) = tray::installer(
                &app.handle().clone(),
                // Le REGISTRE et non la config : les deux divergent dÃ¨s que
                // l'utilisateur retire l'entrÃ©e Ã  la main ou par le
                // gestionnaire des tÃ¢ches, et c'est le registre qui dit la
                // vÃ©ritÃ©.
                autostart::est_actif(),
                actions.clone(),
            ) {
                eprintln!("tray non installÃ© : {e}");
            }

            // â”€â”€ Deux crochets pour les vÃ©rifications qui demandent un clic â”€â”€
            //
            // Le tray a deux entrÃ©es dont l'effet ne se constate qu'en
            // cliquant : Â« Afficher Â» et Â« Quitter Â». Chacune reÃ§oit ici son
            // Ã©quivalent scriptable â€” mÃªme code appelÃ©, sans humain. C'est la
            // mÃªme intention que le fichier tÃ©moin de rechargement juste
            // en dessous.

            // `SHIMEJI_CACHE=1` : dÃ©marre cachÃ©, comme si l'on avait dÃ©cochÃ©
            // Â« Afficher Â». Sans Ã§a, le gain de l'optimisation Â« ne rien
            // dessiner quand c'est cachÃ© Â» ne se mesure pas â€” et une
            // optimisation non mesurÃ©e est une croyance (voir CLAUDE.md).
            if std::env::var("SHIMEJI_CACHE").is_ok() {
                visibilite.store(false, std::sync::atomic::Ordering::Relaxed);
                tray::basculer_visibilite(&app.handle().clone(), false);
                println!("SHIMEJI_CACHE : dÃ©marrÃ© cachÃ©");
            }

            // `SHIMEJI_QUITTER_APRES=<secondes>` : appelle `exit(0)` â€” la
            // ligne exacte de l'entrÃ©e Â« Quitter Â» â€” au bout du dÃ©lai.
            //
            // C'est la vÃ©rification qui **valide le retrait de la console** :
            // sans console, `Ctrl+C` n'existe plus, et une fenÃªtre sans
            // bordure, non focalisable, hors taskbar et hors Alt+Tab ne se
            // ferme par aucun moyen normal. Il faut donc prouver, et pas
            // supposer, qu'un processus GUI dans cet Ã©tat sait bien se
            // terminer sur `exit`.
            if let Ok(valeur) = std::env::var("SHIMEJI_QUITTER_APRES") {
                // `parse` rend un `Result` : une valeur illisible ne doit pas
                // faire quitter tout de suite, ce qui ressemblerait Ã  un
                // succÃ¨s de la vÃ©rification alors qu'on n'a rien vÃ©rifiÃ©.
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
                        "SHIMEJI_QUITTER_APRES : Â« {valeur} Â» n'est pas un nombre de secondes"
                    ),
                }
            }

            // â”€â”€ Rechargement dÃ©clenchÃ© par un fichier tÃ©moin â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
            //
            // Le rechargement se fait normalement par le tray, donc par un
            // clic. Pour qu'il soit **vÃ©rifiable sans humain** â€” et
            // scriptable â€”, on surveille aussi l'apparition d'un fichier
            // `recharger.txt` Ã  cÃ´tÃ© des personnages : sa prÃ©sence dÃ©clenche
            // un rechargement, puis il est supprimÃ©.
            //
            // Trois lignes de plus dans la boucle, et c'est ce qui permet de
            // prouver que le rechargement Ã  chaud marche vraiment plutÃ´t que
            // de l'affirmer.
            let temoin = dossier.join("recharger.txt");

            // â”€â”€ Les horloges â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
            let handle = app.handle().clone();
            // Diagnostic de performance : `SHIMEJI_SANS_BOUCLE=1` crÃ©e la
            // fenÃªtre et n'anime rien. C'est la seule faÃ§on de sÃ©parer ce que
            // coÃ»te NOTRE boucle de ce que coÃ»te l'existence d'une fenÃªtre
            // transparente toujours au premier plan â€” et sans cette mesure,
            // toute optimisation de la boucle est une croyance.
            if std::env::var("SHIMEJI_SANS_BOUCLE").is_ok() {
                println!("SHIMEJI_SANS_BOUCLE : aucune animation, fenÃªtre seule");
            } else {
                // La config complÃ¨te est clonÃ©e pour la boucle : `setup`
                // continue de s'en servir plus haut (le tray, notamment), et
                // la boucle a besoin de sa propre copie pour la remplacer
                // au rechargement Ã  chaud (TÃ¢che 6).
                let configuration_boucle = configuration.clone();
                // La boucle y publie ce qui vit rÃ©ellement, pour que la
                // suppression d'un pack sache quand les fenÃªtres ont disparu.
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

            // `SHIMEJI_CATALOGUE=1` ouvre la fenÃªtre du catalogue au
            // dÃ©marrage â€” l'Ã©quivalent scriptable de l'entrÃ©e de menu que la
            // tÃ¢che 11 ajoutera, et ce qui a prouvÃ© que l'IPC rÃ©pondait
            // avant qu'on bÃ¢tisse une interface dessus (tÃ¢che 8).
            if std::env::var("SHIMEJI_CATALOGUE").is_ok() {
                actions::ouvrir_catalogue(&tauri::Manager::app_handle(app).clone());
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Ã©chec au lancement de l'application Tauri");
}

/// Sert un PNG de personnage pour le schÃ©ma `shime`.
///
/// `chemin` est de la forme `/blob/12`. On refuse tout ce qui n'a pas cette
/// forme exacte plutÃ´t que de composer un chemin de fichier depuis une
/// chaÃ®ne arbitraire : un `..` dans l'URL ne doit pas pouvoir dÃ©signer un
/// fichier hors du dossier des personnages.
fn servir_frame(chemin: &str) -> tauri::http::Response<Vec<u8>> {
    let refus = |code: u16| {
        tauri::http::Response::builder()
            .status(code)
            .body(Vec::new())
            .expect("rÃ©ponse vide toujours constructible")
    };

    // `trim_start_matches('/')` puis dÃ©coupage : on attend exactement deux
    // segments.
    let segments: Vec<&str> = chemin.trim_start_matches('/').split('/').collect();
    if segments.len() != 2 {
        return refus(404);
    }

    let (personnage, numero) = (segments[0], segments[1]);

    // Le nom du personnage ne peut contenir que des caractÃ¨res anodins, et
    // le numÃ©ro doit Ãªtre un entier. Ces deux tests suffisent Ã  interdire
    // tout `..` ou sÃ©parateur de chemin.
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

    // MÃªme rÃ©solution que le chargement : bibliothÃ¨que puis dossier livrÃ©.
    // La validation du nom ci-dessus protÃ¨ge dÃ©sormais DEUX racines, ce qui
    // la rend d'autant plus nÃ©cessaire.
    let Some(dossier_perso) = crate::config::dossier_du_personnage(personnage) else {
        return refus(404);
    };
    let fichier = dossier_perso.join("img").join(format!("shime{n}.png"));

    // On ne trace que les Ã‰CHECS. Tracer les succÃ¨s a servi une fois â€” c'est
    // ainsi qu'on a diagnostiquÃ© le sprite invisible (voir `render::pousser`)
    // â€” mais Ã  8 requÃªtes par seconde la console devient illisible, et une
    // console illisible ne sert plus Ã  diagnostiquer quoi que ce soit.
    //
    // Un 404 en revanche est toujours anormal : il signale un numÃ©ro de frame
    // du manifeste qui ne correspond Ã  aucun fichier.
    if !fichier.is_file() {
        eprintln!("shime:// {chemin} -> 404 ({})", fichier.display());
    } else if std::env::var("SHIMEJI_TRACE").is_ok() {
        // Trace des succÃ¨s, activÃ©e par SHIMEJI_TRACE=1. C'est ce qui a
        // permis de diagnostiquer le sprite invisible ; on la garde derriÃ¨re
        // une variable d'environnement plutÃ´t que de la supprimer, parce que
        // c'est le seul moyen de voir quelles frames sont VRAIMENT demandÃ©es.
        println!("shime:// {chemin} -> 200");
    }

    match std::fs::read(&fichier) {
        Ok(octets) => tauri::http::Response::builder()
            .status(200)
            .header("Content-Type", "image/png")
            // Les images ne changent pas pendant une exÃ©cution ; le cache du
            // webview Ã©vite de relire 46 fichiers en boucle. Le rechargement
            // Ã  chaud du plan 1b changera le numÃ©ro de version dans l'URL
            // pour contourner ce cache.
            .header("Cache-Control", "max-age=3600")
            .body(octets)
            .expect("rÃ©ponse constructible"),
        Err(_) => refus(404),
    }
}

/// La boucle 60 Hz : physique, comportement, rendu (spec Â§5.5).
///
/// Les trois horloges de la spec, dont deux sont ici :
///   Â· **60 Hz** â€” comportement, rendu, position
///   Â· **~8 Hz** â€” recensement du monde
///   Â· **~2 Hz** â€” les signaux (inactivitÃ©, appli active, heure, batterie,
///     verrouillage) â€” spec Â§5.5, design Ã©tape 2 Â§9
/// Le personnage est-il dÃ©jÃ  en train d'Ã©merger ?
///
/// Sert d'anti-rebond au dÃ©verrouillage (voir le point d'appel). Rendre vrai
/// empÃªche de relancer un rÃ©veil dÃ©jÃ  commencÃ©.
///
/// `matches!` : on ne veut lire que la phase, sans dÃ©monter l'intention ni la
/// reconstruire. C'est la mÃªme forme que l'interruption de `behavior::mod`.
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

/// Un personnage Ã  l'Ã©cran : sa fenÃªtre, son Ã©tat, et le peu de mÃ©moire de
/// rendu qu'il faut pour n'appeler Windows que sur changement.
///
/// **Ce qui est ici est ce qui doit exister N fois.** Tout ce qui est
/// partagÃ© â€” le monde, les signaux, le biais, le RNG, les horloges â€” reste
/// une variable locale de `boucle` et n'est calculÃ© qu'UNE fois par image
/// (design Â« plusieurs personnages Â» Â§3). C'est ce partage qui fait que N
/// personnages ne coÃ»tent pas N fois notre calcul.
struct Acteur {
    /// Le label de sa fenÃªtre Tauri.
    ///
    /// âš ï¸ **Jamais rÃ©utilisÃ©** : il vient d'un compteur monotone, pas de
    /// l'index dans le `Vec`. Retirer `pet-1` puis en ajouter un
    /// rÃ©attribuerait `pet-1` pendant que Windows dÃ©truit encore la fenÃªtre
    /// prÃ©cÃ©dente (design Â§3, piÃ¨ge nÂ° 3).
    label: String,

    /// Le pack dont il est une instance. **Plusieurs acteurs peuvent
    /// partager le mÃªme nom** : c'est tout l'objet des doublons.
    nom: String,

    ch: character::Character,

    // â”€â”€ La mÃ©moire de rendu â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    // Ces quatre champs existent pour une seule raison : n'appeler Windows
    // que quand quelque chose a changÃ©. Le coÃ»t Ã©tant proportionnel au
    // nombre de dÃ©placements (design Â§2), s'en priver multiplierait la
    // consommation par ~2 â€” c'est mesurÃ©, 21 % contre 12,3 %.
    dernier_rendu: Option<render::Rendu>,
    derniere_taille: Option<(u32, u32)>,
    dernier_coin: Option<(i32, i32)>,
    clics_traversent: bool,

    /// Diagnostic `SHIMEJI_ESCALADE=1` : le dernier triplet tracÃ©, pour ne
    /// tracer qu'au changement de phase.
    derniere_trace_grimpe: Option<(String, String, String)>,

    /// Non `None` quand il est en train de quitter la scÃ¨ne (design Â§6).
    ///
    /// Sa prÃ©sence **court-circuite `behavior::pas`** : un acteur en dÃ©part
    /// n'est plus un personnage vivant, il s'en va, et la scÃ¨ne n'a plus son
    /// mot Ã  dire.
    depart: Option<Depart>,
}

/// Les trois temps d'un dÃ©part (design Â§6).
///
/// âš ï¸ **Il n'existe AUCUNE frame Â« plier les jambes Â» dans le vocabulaire
/// Shimeji** â€” vÃ©rifiÃ© dans `docs/specs/2026-09-09-frames-shimeji.md`, tirÃ©
/// des sources de Shimeji-ee. Le standard n'a que `jump`, la frame 22, une
/// seule image. Ces trois temps composent la lecture cherchÃ©e avec ce qui
/// existe rÃ©ellement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhaseDepart {
    /// `sit` ~120 ms : il se ramasse.
    SeRamasse,
    /// `jump` ~150 ms : il se dÃ©tend.
    Saute,
    /// `fall` : il tombe, et il TRAVERSE le sol.
    Tombe,
}

/// Un acteur en train de quitter la scÃ¨ne.
///
/// **Pourquoi court-circuiter `behavior::pas` plutÃ´t qu'ajouter un Ã©tat au
/// comportement :** le rÃ©flexe d'atterrissage est non nÃ©gociable par
/// dÃ©finition (spec Â§7.1, couche 1). Y introduire une exception Â« sauf si je
/// suis en train de partir Â» le rendrait nÃ©gociable, et ce serait le premier
/// pas vers un rÃ©flexe plein de cas particuliers.
struct Depart {
    phase: PhaseDepart,
    /// Le temps de l'horloge injectÃ©e au dÃ©but de la **phase courante**.
    depuis: std::time::Duration,
    /// Le dÃ©but du dÃ©part **entier**.
    ///
    /// Distinct de `depuis`, et c'est nÃ©cessaire : le garde-fou des 3 s porte
    /// sur le dÃ©part complet, pas sur sa derniÃ¨re phase. Les confondre le
    /// rendrait inopÃ©rant, puisqu'il repartirait de zÃ©ro Ã  chaque changement
    /// de pose.
    debut: std::time::Duration,
    pos: geom::Point,
    vel: geom::Vec2,
}

/// Les durÃ©es des deux premiÃ¨res phases du dÃ©part.
///
/// **Points de dÃ©part Ã  rÃ©gler Ã  l'Å“il**, comme toutes les constantes
/// d'animation de ce projet â€” et comme elles, Ã  chercher d'abord dans les
/// sources de Shimeji-ee avant d'en inventer une (leÃ§on transverse de
/// l'Ã©tape 1a, oÃ¹ les quatre rÃ©glages faits Ã  l'Å“il Ã©taient faux).
const DUREE_SE_RAMASSE: std::time::Duration = std::time::Duration::from_millis(120);
const DUREE_SAUTE: std::time::Duration = std::time::Duration::from_millis(150);

/// La vitesse initiale vers le haut, en px/s. Sans elle, Â« il saute Â» se lit
/// comme Â« il glisse Â».
const IMPULSION_DEPART: f32 = 350.0;

/// De combien il faut dÃ©passer le bas du bureau pour Ãªtre hors de vue. Une
/// hauteur de fenÃªtre suffit largement.
const MARGE_SORTIE: f32 = 200.0;

/// Au-delÃ , l'acteur est retirÃ© quoi qu'il arrive. Un acteur qui ne partirait
/// jamais ferait fuir une fenÃªtre Ã  chaque dÃ©sactivation â€” et c'est le genre
/// de fuite qu'on ne voit qu'aprÃ¨s une heure d'usage.
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

/// Fait avancer un dÃ©part d'une image. Rend `true` quand l'acteur doit Ãªtre
/// retirÃ©.
///
/// **La couverture partielle s'applique toute seule** : `set_pose` ignore une
/// pose absente du manifeste, donc un pack sans `sit` ou sans `jump` traverse
/// simplement la phase correspondante sans changer d'image. Aucun cas
/// particulier Ã  coder â€” c'est la rÃ¨gle Â§8.6, qui retire du jeu ce qui n'est
/// pas dessinÃ©.
fn avancer_le_depart(
    d: &mut Depart,
    ch: &mut character::Character,
    monde: &world::World,
    maintenant: std::time::Duration,
    dt: f32,
) -> bool {
    // Le garde-fou de durÃ©e, testÃ© en PREMIER.
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
                // Une petite impulsion vers le haut : sans elle, Â« il saute Â»
                // se lit comme Â« il glisse Â».
                d.vel = geom::Vec2::new(0.0, -IMPULSION_DEPART);
            }
        }

        PhaseDepart::Tombe => {
            ch.set_pose("fall", maintenant);

            // La mÃªme gravitÃ© que la physique normale, mais **sans son test
            // de contact** : c'est trÃ¨s exactement ce que Â« il traverse le
            // sol Â» veut dire, et la raison d'Ãªtre de ce court-circuit.
            d.vel.y += character::physics::GRAVITE * dt;
            d.pos.y += d.vel.y * dt;

            // `bounds()` couvre TOUS les Ã©crans : sur deux moniteurs de
            // hauteurs diffÃ©rentes, ne regarder que l'Ã©cran de dÃ©part
            // retirerait le personnage trop tÃ´t sur l'un des deux.
            //
            // `map_or(true, â€¦)` : sans aucun Ã©cran il n'y a plus rien Ã 
            // montrer, donc on retire â€” plutÃ´t que de le laisser tomber
            // indÃ©finiment.
            let sorti = monde
                .bounds()
                .map_or(true, |b| d.pos.y > b.bottom() + MARGE_SORTIE);
            if sorti {
                return true;
            }
        }
    }

    // La position est poussÃ©e au webview par le chemin de rendu habituel :
    // `pos_connue` est ce que la boucle lit pour placer la fenÃªtre d'un
    // acteur en dÃ©part.
    ch.pos_connue = d.pos;
    false
}

/// Quel personnage le curseur dÃ©signe-t-il ? **Au plus un.**
///
/// Rend son index dans le `Vec`. Il n'y a qu'un curseur, donc au plus un
/// personnage concernÃ© : sans cette rÃ¨gle, deux personnages superposÃ©s
/// seraient attrapÃ©s ENSEMBLE par un mÃªme clic et se suivraient jusqu'au
/// relÃ¢chement (design Â§3, piÃ¨ge nÂ° 2).
///
/// L'ordre de dÃ©partage est celui du `Vec` : arbitraire, mais **stable** â€”
/// le mÃªme personnage gagne tant que rien ne change. Deux fenÃªtres toujours
/// au premier plan n'ont de toute faÃ§on pas de z-order que nous
/// contrÃ´lions.
fn elire_sous_le_curseur(
    acteurs: &[Acteur],
    monde: &world::World,
    souris: geom::Point,
    echelle: f32,
) -> Option<usize> {
    // Un personnage dÃ©jÃ  portÃ© garde la main, oÃ¹ que soit le curseur : un
    // glisser rapide fait sortir le sprite de sa propre hitbox, et le
    // relÃ¢cher tout seul serait prÃ©cisÃ©ment le bug que la traversÃ©e des
    // clics Ã©vite dÃ©jÃ  cÃ´tÃ© fenÃªtre.
    for (i, a) in acteurs.iter().enumerate() {
        if matches!(a.ch.attachment, character::attach::Attachment::Dragged) {
            return Some(i);
        }
    }

    for (i, a) in acteurs.iter().enumerate() {
        // Position indÃ©rivable (plateforme disparue) ou pose absente du
        // manifeste : on ne peut pas savoir. On passe au suivant plutÃ´t que
        // de dÃ©cider Ã  sa place â€” les clics continuent de traverser, ce qui
        // ne gÃªne personne.
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
    // Le `Vec` remplace le couple `(label, ch)` du temps oÃ¹ il n'y avait
    // qu'un personnage. `mut` : il grandit et rÃ©trÃ©cit en cours
    // d'exÃ©cution, c'est tout l'objet de cette Ã©tape.
    mut acteurs: Vec<Acteur>,
    mut monde: world::World,
    mut echelle_affichage: f32,
    visibilite: tray::Visibilite,
    mut reglages: config::Reglages,
    mut table: behavior::desire::TableEnvies,
    // Le rÃ©glage `echelle` de la config, gardÃ© Ã  part pour le recombiner Ã 
    // l'Ã©chelle du moniteur au recensement â€” celle-ci peut changer si le
    // personnage passe sur un Ã©cran de DPI diffÃ©rent.
    mut echelle_config: f32,
    demande: rechargement::Demande,
    temoin: std::path::PathBuf,
    // La Config complÃ¨te (option 1 du brief de la TÃ¢che 6) : `signals::biais_de`
    // a besoin de la table des applications, que `Reglages` n'expose pas.
    // Une variable globale aurait Ã©tÃ© plus courte Ã  Ã©crire, mais la spec
    // Â§10.2 l'interdit â€” et Ã§a rendrait `biais_de` intestable en dehors de
    // cette boucle.
    mut config_courante: config::Config,
    // La boÃ®te aux lettres par laquelle l'envie choisie au menu contextuel
    // revient jusqu'ici. La boucle n'a PAS besoin d'`Actions` : construire le
    // menu ne dÃ©clenche rien, et le clic part dans la boucle d'Ã©vÃ©nements de
    // Tauri jusqu'Ã  l'unique gestionnaire installÃ© par `tray.rs`.
    commande: actions::BoiteCommande,
    // **Le RNG, semÃ© une seule fois par `setup`.** Il arrive dÃ©jÃ  amorcÃ©
    // parce que l'apparition du premier personnage s'en est servie avant que
    // ce thread n'existe. Le re-semer ici rejouerait la mÃªme sÃ©quence, et en
    // crÃ©er un par personnage retomberait dans le piÃ¨ge des graines
    // sÃ©quentielles (CLAUDE.md).
    mut rng: rng::XorShift32,
    // PartagÃ© avec les deux menus. La boucle n'y touche que pour PUBLIER ce
    // qui vit rÃ©ellement (design Â§7) â€” elle ne lit jamais le roster voulu,
    // qui lui arrive par la boÃ®te de rechargement.
    actions: std::sync::Arc<actions::Actions>,
) {
    use behavior::Entrees;
    use std::time::{Duration, Instant};
    // `get_webview_window` est une mÃ©thode du trait `Manager` : sans cet
    // import, l'`AppHandle` ne l'expose pas.
    use tauri::Manager;

    let sonde = probe::win32::Win32Probe::new();
    let horloge = clock::SystemClock::new();

    // `SHIMEJI_ESCALADE=1` : force l'intention `Grimper` dÃ¨s la premiÃ¨re
    // image, au lieu d'attendre qu'elle sorte du tirage pondÃ©rÃ© (elle partage
    // aujourd'hui le poids de `se_reposer`, donc l'attendre Ã  l'Å“il peut
    // prendre plusieurs minutes).
    //
    // AjoutÃ©e Ã  la TÃ¢che 7 pour une raison prÃ©cise : mesurer l'ancre de
    // `grabWall`/`climbWall` demande de REGARDER le personnage accrochÃ© Ã  un
    // mur, et Ã§a, aucun script ne peut le faire Ã  la place d'un humain â€” la
    // seule exception du projet Ã  Â« tout ce qui demanderait un clic reÃ§oit un
    // Ã©quivalent scriptable Â» (voir CLAUDE.md). Mais le TRAJET jusqu'Ã  ce
    // moment-lÃ , lui, se scripte trÃ¨s bien : cette variable Ã©vite Ã  l'auteur
    // d'ouvrir le menu contextuel et de choisir Â« Grimper Â» Ã  la main, et la
    // trace ci-dessous (Ã  chaque changement de phase) lui dit quand regarder
    // l'Ã©cran sans avoir Ã  fixer le personnage pendant plusieurs minutes.
    let trace_escalade = std::env::var("SHIMEJI_ESCALADE").is_ok();
    if trace_escalade {
        // Sur TOUS les acteurs : Ã  N=1 c'est l'ancien comportement, et Ã  N
        // supÃ©rieur on veut pouvoir regarder n'importe lequel d'entre eux.
        for a in acteurs.iter_mut() {
            a.ch.intention = Some(behavior::intention::ActiveIntention::nouvelle(
                behavior::intention::Intention::Grimper,
                horloge.elapsed(),
            ));
        }
        println!("SHIMEJI_ESCALADE : intention Grimper forcÃ©e au dÃ©marrage");
    }

    const PERIODE: Duration = Duration::from_micros(16_667); // 60 Hz
    const PERIODE_MONDE: Duration = Duration::from_millis(125); // 8 Hz
    const PERIODE_SIGNAUX: Duration = Duration::from_millis(500); // 2 Hz

    // Pendant ce dÃ©lai aprÃ¨s le dÃ©marrage, on pousse la frame Ã  CHAQUE
    // image, sans comparer.
    //
    // C'est une course au dÃ©marrage : `pet.js` n'a pas encore posÃ© son
    // Ã©couteur pendant le chargement de la page, et un message Ã©mis avant
    // est perdu. Si le personnage restait immobile pendant les premiÃ¨res
    // secondes, rien ne s'afficherait â€” Ã©cran vide, sans erreur, et le
    // diagnostic partirait chercher un problÃ¨me de transparence.
    const AMORCAGE: Duration = Duration::from_secs(2);

    let mut dernier_recensement = Duration::ZERO;
    let mut dernier_signal = Duration::ZERO;

    // Le biais courant, recalculÃ© Ã  2 Hz et transportÃ© Ã  60 Hz.
    //
    // Neutre au dÃ©marrage : la premiÃ¨re demi-seconde, le personnage se
    // comporte comme Ã  l'Ã©tape 1. Rien Ã  corriger â€” attendre les signaux
    // avant de bouger serait une demi-seconde de figement au lancement.
    let mut biais = signals::Biais::neutre();
    let mut utilisateur_actif = true;

    // La session est-elle verrouillÃ©e ? MÃ©morisÃ© pour ne basculer les
    // fenÃªtres que sur CHANGEMENT.
    let mut verrouille = false;

    // Diagnostic : `SHIMEJI_SIGNAUX=1`.
    let trace_signaux = std::env::var("SHIMEJI_SIGNAUX").is_ok();

    // L'Ã©chelle du moniteur, sÃ©parÃ©e du rÃ©glage de la config : le
    // rechargement Ã  chaud change le second sans redemander le premier.
    let mut ecrans_echelle = if echelle_config != 0.0 {
        echelle_affichage / echelle_config
    } else {
        1.0
    };
    // `dernier_rendu`, `derniere_taille`, `dernier_coin` et
    // `clics_traversent` ont migrÃ© dans `Acteur` : ce sont les seules
    // variables de cette boucle qui doivent exister N fois. La position
    // ENTIÃˆRE posÃ©e Ã  la derniÃ¨re image (`dernier_coin`) en fait partie â€”
    // `set_position` ne prend que des entiers, et deux positions flottantes
    // qui s'arrondissent au mÃªme pixel produisent exactement le mÃªme appel.

    // Diagnostic de cadence, voir plus bas.
    let trace_cadence = std::env::var("SHIMEJI_CADENCE").is_ok();
    let mut images_depuis_trace: u32 = 0;
    let mut placements_depuis_trace: u32 = 0;
    let mut travail_cumule = Duration::ZERO;
    let mut derniere_trace = Duration::ZERO;

    // Le prochain numÃ©ro de label libre.
    //
    // âš ï¸ **Monotone, et jamais remis Ã  zÃ©ro.** L'index dans le `Vec` ne peut
    // pas servir de label : retirer `pet-1` puis en ajouter un rÃ©attribuerait
    // `pet-1` pendant que Windows dÃ©truit encore la fenÃªtre prÃ©cÃ©dente, et
    // Tauri refuserait le label â€” ou pire, servirait l'ancienne fenÃªtre
    // (design Â§3, piÃ¨ge nÂ° 3).
    //
    // Il part aprÃ¨s les labels dÃ©jÃ  posÃ©s par `setup`.
    let mut prochain_label: usize = acteurs.len();

    // Le bouton droit Ã©tait-il enfoncÃ© Ã  l'image prÃ©cÃ©dente ?
    //
    // **PartagÃ©, et pas par acteur** : il n'y a qu'une souris, donc qu'un
    // front descendant par clic. Un drapeau par personnage ferait ouvrir N
    // menus d'affilÃ©e sur un seul clic.
    //
    // C'est ce qui transforme un Ã©tat â€” Â« le bouton est enfoncÃ© Â», vrai
    // pendant les ~15 images que dure un clic humain â€” en un **front**, qui
    // n'arrive qu'une fois. Sans lui, maintenir le bouton rouvrirait le menu
    // en boucle dÃ¨s sa fermeture.
    let mut bouton_droit_precedent = false;

    loop {
        // `Instant` ici et non l'horloge injectÃ©e : c'est la CADENCE, pas le
        // temps du comportement. La distinction compte â€” le comportement doit
        // rester pilotable par une horloge factice (spec Â§10.2).
        let debut = Instant::now();
        let maintenant = horloge.elapsed();

        // â”€â”€ ~2 Hz : les signaux (spec Â§5.5, design Ã©tape 2 Â§9) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        //
        // Cinq appels systÃ¨me toutes les 500 ms. Ã€ comparer aux 60
        // `SetWindowPos` par seconde qui coÃ»tent 11 points de CPU : c'est du
        // bruit. MesurÃ© quand mÃªme â€” voir `CLAUDE.md`.
        if maintenant.saturating_sub(dernier_signal) >= PERIODE_SIGNAUX {
            let s = sonde.signaux();

            // Le biais : de la donnÃ©e pure, calculÃ©e par une fonction pure.
            biais = signals::biais_de(&s, &config_courante);

            // Â« Actif Â» se dÃ©rive du MÃŠME seuil que le biais, pour qu'il soit
            // impossible d'Ãªtre Â« actif Â» et Â« inactif Â» dans la mÃªme image.
            // La fonction vit dans `signals.rs`, Ã  cÃ´tÃ© de `biais_de`, et pas
            // recopiÃ©e ici : c'est la SEULE dÃ©finition de Â« actif Â», partagÃ©e
            // avec `sim.rs` â€” sans quoi les deux finiraient par diverger.
            utilisateur_actif = signals::utilisateur_actif(&s, &config_courante);

            // â”€â”€ Le verrouillage : le quatriÃ¨me rÃ©flexe â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
            //
            // C'est un rÃ©flexe au sens du design (dÃ©cision nÂ° 5) : non
            // nÃ©gociable, immÃ©diat, on ne biaise pas un poids pour
            // disparaÃ®tre d'un Ã©cran de verrouillage.
            //
            // Mais il ne s'implÃ©mente PAS dans `reflex.rs`, et c'est
            // dÃ©libÃ©rÃ© : son effet porte sur la FENÃŠTRE, pas sur l'accroche
            // du personnage. `reflex.rs` ne connaÃ®t ni Tauri ni le tray, et
            // c'est ce qui le garde testable sans Ã©cran.
            if s.session_verrouillee != verrouille {
                verrouille = s.session_verrouillee;

                // â”€â”€ Au retour : il Ã©merge â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                //
                // Il a Â« dormi Â» pendant que la session Ã©tait verrouillÃ©e â€”
                // ce qui est littÃ©ralement vrai : la boucle tournait, mais
                // rien n'Ã©tait dessinÃ©.
                //
                // C'est une INTENTION qu'on pose, pas un dÃ©cor peint
                // par-dessus le rendu. La pose et sa durÃ©e vivent donc dans
                // `intention.rs`, avec toutes les autres, et **le rendu ne
                // connaÃ®t toujours aucun numÃ©ro de frame** â€” ce qui est ce
                // qui permet au rÃ©veil de marcher sur un pack tiers dont la
                // numÃ©rotation n'est pas celle du blob.
                //
                // Le garde `deja_en_reveil` est une simple prÃ©caution :
                // relancer depuis zÃ©ro un rÃ©veil dÃ©jÃ  commencÃ© n'aurait aucun
                // sens, quelle qu'en soit la raison.
                //
                // âš ï¸ Il ne compense PAS un rebond de la sonde, contrairement Ã 
                // ce qu'une premiÃ¨re hypothÃ¨se supposait. On avait cru que
                // `WTSSessionInfoEx`, sondÃ©e Ã  2 Hz, repassait transitoirement
                // par LOCK pendant la bascule de bureau et faisait voir DEUX
                // dÃ©verrouillages. **La console dit non** : un cycle complet
                // n'imprime qu'un Â« verrouillÃ©e Â» et qu'un Â« dÃ©verrouillÃ©e Â».
                // Le vrai Â« rÃ©veil jouÃ© deux fois Â» venait de l'ordre des
                // frames â€” voir `POSE_WAKE`.
                //
                // Sur TOUS les acteurs : en oublier un le laisserait figÃ©
                // dans la pose qu'il avait au verrouillage, pendant que les
                // autres Ã©mergent.
                if !verrouille {
                    for a in acteurs.iter_mut() {
                        if !deja_en_reveil(&a.ch) {
                            a.ch.intention =
                                Some(behavior::intention::ActiveIntention::reveil(maintenant));
                        }
                    }
                }

                // On ne rend visible que si l'utilisateur n'avait pas
                // lui-mÃªme dÃ©cochÃ© Â« Afficher Â» : le dÃ©verrouillage ne doit
                // pas dÃ©faire son choix.
                let voulu = visibilite.load(std::sync::atomic::Ordering::Relaxed);
                tray::basculer_visibilite(&handle, voulu && !verrouille);

                println!(
                    "session {} â€” personnage {}",
                    if verrouille { "verrouillÃ©e" } else { "dÃ©verrouillÃ©e" },
                    if verrouille { "planquÃ©" } else { "il Ã©merge" }
                );
            }

            if trace_signaux {
                println!(
                    "signaux : inactif {:.0} s Â· {} Â· {} h Â· batterie {} Â· verrouillÃ© {} \
                     â†’ flÃ¢ner Ã—{:.2} reposer Ã—{:.2} jouer Ã—{:.2}",
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

        // â”€â”€ ~8 Hz : recenser le monde â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        // Ã€ l'Ã©tape 1 c'est la liste des Ã©crans ; Ã  l'Ã©tape 4 s'y ajouteront
        // les fenÃªtres, leur filtrage et l'occlusion.
        if maintenant.saturating_sub(dernier_recensement) >= PERIODE_MONDE {
            let ecrans = sonde.screens();
            if !ecrans.is_empty() {
                monde = world::World::from_screens(&ecrans);
                // GardÃ©e Ã  part : le rechargement Ã  chaud doit pouvoir
                // recombiner l'Ã©chelle du moniteur avec la NOUVELLE Ã©chelle
                // de la config, sans redemander les Ã©crans.
                ecrans_echelle = ecrans[0].scale;
                echelle_affichage = ecrans_echelle * echelle_config;
            }

            // Le fichier tÃ©moin : prÃ©sent â†’ on demande un rechargement du
            // roster ENTIER, et on le retire pour ne pas boucler.
            //
            // **Le mÃªme chemin que la bibliothÃ¨que** : relire, recharger,
            // rÃ©concilier. Un seul chemin de code, donc pas de second qui
            // divergerait Ã  la premiÃ¨re correction (design Â§4).
            if temoin.exists() {
                let _ = std::fs::remove_file(&temoin);
                // On relit la config Ã  chaque fois plutÃ´t qu'une fois au
                // dÃ©marrage : un pack a pu Ãªtre installÃ© entre-temps, et une
                // liste mÃ©morisÃ©e ne le verrait jamais.
                let voulus = config::charger().personnages;
                match rechargement::preparer_roster(&demande, &voulus, false) {
                    Ok(v) => println!("rechargement demandÃ© par tÃ©moin (version {v})"),
                    Err(e) => eprintln!("rechargement impossible : {e}"),
                }
            }

            // â”€â”€ Une demande de rechargement en attente ? â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
            //
            // `try_lock` et non `lock` : la boucle 60 Hz ne doit JAMAIS
            // attendre. Si le thread de la commande tient le verrou Ã  cet
            // instant, on rÃ©essaiera dans 125 ms et personne ne s'en
            // apercevra.
            if let Ok(mut boite) = demande.try_lock() {
                // `take()` vide la boÃ®te en rÃ©cupÃ©rant son contenu : la
                // demande est consommÃ©e atomiquement, sans drapeau Ã 
                // remettre Ã  zÃ©ro.
                if let Some(r) = boite.take() {
                    reglages = r.reglages;
                    table = r.table;
                    echelle_config = r.echelle_config;
                    config_courante = r.config;
                    echelle_affichage = ecrans_echelle * echelle_config;

                    // â”€â”€ Les manifestes des acteurs dÃ©jÃ  lÃ  â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    //
                    // Un rechargement Ã  chaud change le `mascot.json` sous
                    // les pieds d'un personnage qui reste : il faut lui
                    // donner le nouveau, sans quoi le fichier tÃ©moin ne
                    // servirait plus Ã  rÃ©gler les animations.
                    for a in acteurs.iter_mut() {
                        let Some(charge) = r.personnages.iter().find(|p| p.nom == a.nom) else {
                            continue;
                        };

                        // La pose courante existe-t-elle encore dans le
                        // nouveau manifeste ? Si elle vient d'Ãªtre renommÃ©e
                        // ou retirÃ©e, `set_pose` refuserait tout changement
                        // et le personnage resterait figÃ© sur une clÃ© morte.
                        if !charge.manifeste.has_pose(&a.ch.pose) {
                            a.ch.pose = character::manifest::POSE_STAND.to_string();
                            a.ch.pose_depuis = maintenant;
                        }

                        a.ch.manifest = charge.manifeste.clone();

                        // Le webview doit oublier ses images, et la taille
                        // de la fenÃªtre peut avoir changÃ© (`frameSize`,
                        // `scale`).
                        let _ = render::recharger(&handle, &a.label, r.version, &a.nom);
                        a.derniere_taille = None;
                        a.dernier_rendu = None;
                        a.dernier_coin = None;
                    }

                    // â”€â”€ RÃ©concilier prÃ©sents et voulus â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
                    //
                    // Un acteur DÃ‰JÃ€ EN DÃ‰PART ne compte plus comme
                    // prÃ©sent : sinon rÃ©activer un personnage pendant que
                    // le prÃ©cÃ©dent tombe n'en crÃ©erait pas de nouveau, et
                    // le compteur de la bibliothÃ¨que mentirait.
                    let presents: Vec<String> = acteurs
                        .iter()
                        .filter(|a| a.depart.is_none())
                        .map(|a| a.nom.clone())
                        .collect();

                    for action in roster::reconcilier(&presents, &r.voulus) {
                        match action {
                            roster::ActionRoster::Creer(nom) => {
                                // Le manifeste a Ã©tÃ© lu par le thread de la
                                // commande : il n'y a AUCUNE entrÃ©e-sortie
                                // ici, sur le chemin des 60 Hz.
                                let Some(charge) = r.personnages.iter().find(|p| p.nom == nom)
                                else {
                                    eprintln!("Â« {nom} Â» voulu mais non chargÃ© : ignorÃ©");
                                    continue;
                                };

                                let taille = character::attach::window_size(
                                    &charge.manifeste,
                                    echelle_affichage,
                                );

                                // Le point d'apparition AVANT la fenÃªtre :
                                // s'il n'y a aucun Ã©cran, on n'a pas crÃ©Ã©
                                // de fenÃªtre Ã  dÃ©truire.
                                let Some(att) =
                                    apparition::point_de_chute(&monde, taille.0 as f32, &mut rng)
                                else {
                                    eprintln!("aucun Ã©cran : Â« {nom} Â» n'apparaÃ®t pas");
                                    continue;
                                };

                                // âš ï¸ Compteur MONOTONE, jamais l'index dans
                                // le `Vec` : un label rÃ©utilisÃ© se
                                // heurterait Ã  une fenÃªtre que Windows
                                // dÃ©truit encore (design Â§3, piÃ¨ge nÂ° 3).
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
                                            // lisible plutÃ´t qu'un
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
                                        println!("Â« {nom} Â» apparaÃ®t ({label})");
                                    }
                                    // Bruyant : une crÃ©ation silencieusement
                                    // ratÃ©e donnerait un compteur Ã  3 pour 2
                                    // personnages Ã  l'Ã©cran.
                                    Err(e) => {
                                        eprintln!("Â« {nom} Â» n'a pas pu apparaÃ®tre : {e}")
                                    }
                                }
                            }

                            roster::ActionRoster::RetirerUn(nom) => {
                                // `rposition` : le plus RÃ‰CEMMENT ajoutÃ©
                                // part le premier, ce qu'attend quelqu'un
                                // qui vient de cliquer Â« + Â» puis Â« âˆ’ Â»
                                // (design Â§4). On ignore ceux qui partent
                                // dÃ©jÃ , pour ne pas en marquer deux.
                                let Some(i) = acteurs
                                    .iter()
                                    .rposition(|a| a.nom == nom && a.depart.is_none())
                                else {
                                    continue;
                                };

                                if r.sans_animation {
                                    // Suppression d'un pack : on n'attend
                                    // pas l'animation, les fichiers vont
                                    // Ãªtre effacÃ©s (design Â§7).
                                    let parti = acteurs.remove(i);
                                    if let Some(w) = handle.get_webview_window(&parti.label) {
                                        let _ = w.destroy();
                                    }
                                    println!("Â« {nom} Â» retirÃ© ({})", parti.label);
                                } else {
                                    let pos = acteurs[i].ch.pos_connue;
                                    acteurs[i].depart = Some(Depart::commence(maintenant, pos));
                                    println!("Â« {nom} Â» s'en va ({})", acteurs[i].label);
                                }
                            }
                        }
                    }

                    println!("roster appliquÃ© (version {})", r.version);
                }
            }

            // Publier ce qui vit RÃ‰ELLEMENT, pour la suppression (design Â§7).
            // Ã€ 8 Hz, jamais sur le chemin des 60 Hz.
            actions.publier_presents(
                acteurs
                    .iter()
                    .filter(|a| a.depart.is_none())
                    .map(|a| a.nom.clone())
                    .collect(),
            );

            dernier_recensement = maintenant;
        }

        // â”€â”€ 60 Hz : les entrÃ©es, et le hit-testing (spec Â§3.3) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        // La spec Â§3.3 propose ~30 Hz pour `GetCursorPos`. On le lit Ã  60 Hz :
        // l'appel est effectivement quasi gratuit, et Ã  30 Hz le personnage
        // traÃ®nerait visiblement derriÃ¨re le curseur pendant un glisser.
        let m = sonde.mouse();

        // â”€â”€ L'Ã©lection : UN SEUL personnage sous le curseur â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        //
        // Le hit-testing compare le curseur Ã  la hitbox de la POSE COURANTE
        // â€” et non Ã  la boÃ®te de 128Ã—128 : sans cette distinction, le
        // personnage serait un trou noir de 128 px avalant les clics dans
        // ses zones transparentes. `elire_sous_le_curseur` le fait pour
        // chaque acteur, et n'en retient qu'un.
        let elu = elire_sous_le_curseur(&acteurs, &monde, m.pos, echelle_affichage);

        // Le front descendant du bouton droit : calculÃ© UNE fois, avant la
        // boucle, parce qu'il n'y a qu'une souris.
        //
        // C'est ce qui transforme un Ã©tat â€” Â« le bouton est enfoncÃ© Â», vrai
        // pendant les ~15 images que dure un clic humain â€” en un **front**,
        // qui n'arrive qu'une fois. Sans lui, maintenir le bouton rouvrirait
        // le menu en boucle dÃ¨s sa fermeture.
        let front_descendant_droit = !m.right_down && bouton_droit_precedent;
        bouton_droit_precedent = m.right_down;

        // La commande Ã©ventuellement dÃ©posÃ©e par le gestionnaire de menu.
        //
        // `try_lock` et non `lock` : Ã  60 Hz on ne s'autorise jamais Ã 
        // attendre un verrou. S'il est pris Ã  cet instant, la commande sera
        // lue Ã  l'image suivante, 16 ms plus tard â€” invisible.
        //
        // `take()` vide la boÃ®te en rÃ©cupÃ©rant son contenu : la commande est
        // ainsi consommÃ©e une fois et une seule. **Lue avant la boucle, et
        // donnÃ©e au seul acteur Ã©lu** : la donner Ã  tous ferait exÃ©cuter
        // l'entrÃ©e de menu par N personnages.
        let mut commande_du_menu = match commande.try_lock() {
            Ok(mut boite) => boite.take(),
            Err(_) => None,
        };

        // CachÃ© par l'utilisateur, OU session verrouillÃ©e : on calcule tout,
        // on ne dessine rien. Lu une fois, il vaut pour tous les acteurs.
        //
        // Le comportement, lui, continue de tourner : il doit avancer pour
        // qu'on le retrouve ailleurs en le rÃ©affichant, et c'est du calcul
        // pur â€” mesurÃ© Ã  100 Âµs par image quand il ne se passe rien.
        //
        // Ce qui coÃ»te, c'est `SetWindowPos` sur une fenÃªtre en couche (voir
        // Â« Mesurer le CPU Â» dans CLAUDE.md), et c'est exactement ce qu'on
        // saute. Le verrouillage emprunte EXACTEMENT ce mÃªme chemin, dÃ©jÃ 
        // mesurÃ© Ã  0,9 %.
        //
        // `Ordering::Relaxed` : il n'y a aucune autre donnÃ©e Ã  synchroniser
        // avec ce boolÃ©en, seulement sa propre valeur.
        let visible = visibilite.load(std::sync::atomic::Ordering::Relaxed) && !verrouille;

        // Un menu contextuel a-t-il Ã©tÃ© ouvert pendant cette image ? Voir
        // pourquoi ce drapeau existe, plus bas, lÃ  oÃ¹ il est posÃ©.
        let mut menu_ouvert = false;

        // Les acteurs dont le dÃ©part s'achÃ¨ve Ã  cette image.
        //
        // CollectÃ©s plutÃ´t que retirÃ©s sur place : retirer d'un `Vec` qu'on
        // parcourt par index dÃ©calerait tous les suivants, et on en sauterait
        // un sur deux. On les retire aprÃ¨s la boucle, **du dernier au
        // premier**, pour que les index restent valides.
        let mut a_retirer: Vec<usize> = Vec::new();

        let dt = PERIODE.as_secs_f32();

        // â”€â”€ 60 Hz par personnage â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        for i in 0..acteurs.len() {
            let sur_le_personnage = elu == Some(i);
            let acteur = &mut acteurs[i];

            // â”€â”€ Un acteur en dÃ©part ne vit plus : il tombe, et c'est tout â”€
            //
            // Ni comportement, ni hit-testing, ni menu : il n'est plus
            // attrapable ni cliquable (design Â§6). `elire_sous_le_curseur`
            // l'ignore dÃ©jÃ  ; ce `continue` couvre tout le reste.
            if let Some(mut d) = acteur.depart.take() {
                let fini = avancer_le_depart(&mut d, &mut acteur.ch, &monde, maintenant, dt);
                if fini {
                    a_retirer.push(i);
                } else {
                    // Remis en place : `take()` l'avait sorti pour pouvoir
                    // emprunter `acteur.ch` en mÃªme temps que lui. Sans ce
                    // va-et-vient, le vÃ©rificateur d'emprunt refuserait deux
                    // emprunts mutables du mÃªme `acteur`.
                    acteur.depart = Some(d);

                    // On le dessine quand mÃªme : c'est toute la raison
                    // d'Ãªtre de l'animation de dÃ©part.
                    if visible {
                        let coin = geom::Point::new(
                            acteur.ch.pos_connue.x - (acteur.derniere_taille.unwrap_or((0, 0)).0 as f32) / 2.0,
                            acteur.ch.pos_connue.y,
                        );
                        let _ = render::placer(&handle, &acteur.label, coin);
                        let rendu = render::Rendu {
                            image: acteur.ch.frame_courante(maintenant),
                            flip: acteur.ch.facing.flipped(),
                        };
                        if acteur.dernier_rendu != Some(rendu) {
                            let _ = render::pousser(&handle, &acteur.label, rendu);
                            acteur.dernier_rendu = Some(rendu);
                        }
                        placements_depuis_trace += 1;
                    }
                }
                continue;
            }

            // â”€â”€ Absorber le clic, mais seulement lÃ  oÃ¹ il faut â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
            //
            // Pourquoi dÃ©sactiver la traversÃ©e alors que la sonde nous dit
            // dÃ©jÃ  tout ? Parce que si les clics continuaient de traverser,
            // cliquer sur le personnage cliquerait **aussi** l'icÃ´ne du
            // bureau derriÃ¨re lui. Il faut ABSORBER le clic â€” c'est Ã  Ã§a que
            // sert le va-et-vient de `set_ignore_cursor_events` (spec Â§3.3).
            //
            // Pendant un glisser, on garde les clics absorbÃ©s mÃªme si le
            // sprite a glissÃ© hors de sa propre hitbox : sinon un
            // dÃ©placement rapide relÃ¢cherait le personnage tout seul.
            let porte = matches!(
                acteur.ch.attachment,
                character::attach::Attachment::Dragged
            );
            let doit_traverser = !sur_le_personnage && !porte;

            // On n'appelle Win32 que sur CHANGEMENT d'Ã©tat : appeler
            // `set_ignore_cursor_events` 60 fois par seconde marcherait,
            // mais c'est un appel systÃ¨me par image pour rien â€” et la
            // section Â« Mesurer le CPU Â» de CLAUDE.md dit pourquoi on y
            // regarde.
            if doit_traverser != acteur.clics_traversent {
                // âš ï¸ `continue` et non `return` : la fenÃªtre de CE
                // personnage a pu Ãªtre dÃ©truite, mais Ã§a n'est plus une
                // raison de tuer la boucle â€” les autres continuent de
                // vivre. C'est la gÃ©nÃ©ralisation qui l'impose, et c'est
                // aussi ce qui rend le retrait d'un acteur inoffensif.
                if render::traverser_les_clics(&handle, &acteur.label, doit_traverser).is_err() {
                    continue;
                }
                acteur.clics_traversent = doit_traverser;
            }

            // â”€â”€ Clic droit sur le personnage : le menu contextuel â”€â”€â”€â”€â”€â”€â”€
            //
            // Front **descendant** (le bouton vient d'Ãªtre RELÃ‚CHÃ‰) ET
            // curseur dans la hitbox : un clic droit sur le bureau Ã  cÃ´tÃ© de
            // lui ne doit rien ouvrir. Le test de hitbox est le MÃŠME que
            // celui qui absorbe les clics gauches, donc la zone cliquable
            // est exactement celle qu'on voit.
            //
            // âš ï¸ **Au relÃ¢chement et non Ã  l'enfoncement**, et pour deux
            // raisons qui pointent dans le mÃªme sens :
            //
            // 1. c'est la convention de Windows â€” l'explorateur, comme toute
            //    application, ouvre son menu contextuel sur `WM_RBUTTONUP` ;
            // 2. ouvrir au bouton encore enfoncÃ© lance `TrackPopupMenu`
            //    pendant que Windows suit toujours un clic droit en cours.
            //    Le menu hÃ©rite alors d'un suivi de souris qui ne lui
            //    appartient pas, et se referme mal.
            if front_descendant_droit && sur_le_personnage {
                // `let â€¦ else` : si la fenÃªtre a Ã©tÃ© fermÃ©e, cet acteur n'a
                // plus de menu Ã  ouvrir. On passe au suivant.
                let Some(win) = handle.get_webview_window(&acteur.label) else {
                    continue;
                };

                // **Cet appel bloque** jusqu'Ã  la fermeture du menu : le
                // personnage s'immobilise pendant ce temps, ce qui est voulu
                // (voir `menu_perso::ouvrir`).
                //
                // `ou_de` : le menu proposÃ© dÃ©pend de l'endroit oÃ¹ il est
                // accrochÃ© â€” voir `menu_perso::Ou`. C'est ce qui corrige le
                // bug rapportÃ© Ã  l'Ã©cran : un menu de sol proposÃ© Ã  un
                // personnage accrochÃ© Ã  un mur le faisait tomber au premier
                // clic, quelle que soit l'entrÃ©e choisie.
                let ou = menu_perso::ou_de(&acteur.ch.attachment);
                if let Err(e) =
                    menu_perso::ouvrir(&handle, &win, &acteur.ch.manifest, &table, ou)
                {
                    eprintln!("menu du personnage : {e}");
                }

                // âš ï¸ **On sort de la boucle `for`, et l'image entiÃ¨re est
                // abandonnÃ©e** â€” pas seulement ce personnage.
                //
                // `maintenant` a Ã©tÃ© lu AVANT le menu, il a donc plusieurs
                // secondes de retard, et `dt` vaut toujours 16,7 ms.
                // Poursuivre ferait juger toutes les Ã©chÃ©ances (dÃ©lai
                // d'abandon, durÃ©e de pose) sur un instant pÃ©rimÃ© â€” et Ã  N
                // personnages, un simple `continue` de la boucle `for`
                // Ã©tendrait ce dÃ©faut aux Nâˆ’1 autres au lieu de le corriger.
                menu_ouvert = true;
                break;
            }

            let entrees = Entrees {
            souris: m.pos,
            echelle_affichage,
            bouton_gauche: m.left_down,
            curseur_sur_le_personnage: sur_le_personnage,
            // RecalculÃ©s Ã  2 Hz ci-dessus, transportÃ©s tels quels Ã  60 Hz.
            biais,
            utilisateur_actif,
                // `take()` : la commande n'est donnÃ©e qu'Ã  l'acteur Ã‰LU, et
                // une seule fois. La donner Ã  tous ferait exÃ©cuter l'entrÃ©e
                // de menu par N personnages â€” dont Nâˆ’1 qui n'ont rien
                // demandÃ©.
                commande: if sur_le_personnage {
                    commande_du_menu.take()
                } else {
                    None
                },
            };

            // â”€â”€ 60 Hz : le comportement â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
            // La MÃŠME fonction que le mode simulation.
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

        // â”€â”€ Diagnostic : `SHIMEJI_ESCALADE=1` â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        // Rien qu'une intention `Grimper` en cours ne trace : c'est
        // exactement le moment que l'auteur doit regarder pour mesurer
        // l'ancre de `grabWall`/`climbWall` Ã  l'Å“il.
        if trace_escalade {
            if let Some(ai) = acteur.ch.intention {
                if let behavior::intention::EtatIntention::Grimpe { phase, .. } = ai.etat {
                    // Le MESSAGE affichÃ© garde l'offset â€” il aide Ã  situer le
                    // personnage sur la paroi au moment prÃ©cis oÃ¹ la trace
                    // sort. Voir plus bas pourquoi la CLÃ‰, elle, ne le
                    // contient plus.
                    let face_affichee = match acteur.ch.attachment {
                        character::attach::Attachment::On { face, offset, .. } => {
                            format!("{face:?} offset={offset:.1}")
                        }
                        character::attach::Attachment::Falling { .. } => "chute".to_string(),
                        character::attach::Attachment::Dragged => "portÃ©".to_string(),
                    };

                    // âš ï¸ **La clÃ© de dÃ©doublonnage NE CONTIENT PAS l'offset**
                    // (correction de la relecture finale, point 4) : seul le
                    // nom de la face y entre, sans sa valeur numÃ©rique.
                    //
                    // L'offset avance en continu pendant `Rejoindre` et
                    // `Paroi` (jusqu'Ã  0,83 px par image), donc l'inclure
                    // dans la clÃ© la faisait changer Ã  presque CHAQUE image :
                    // la trace sortait Ã  ~60 lignes par seconde, alors que ce
                    // commentaire promet Â« Ã  chaque changement de phase Â».
                    // Inutilisable pour ce Ã  quoi cette trace sert : dire Ã 
                    // l'auteur QUAND regarder l'Ã©cran pour mesurer l'ancre de
                    // `grabWall`/`climbWall` â€” la seule vÃ©rification de ce
                    // projet qui reste manuelle (voir plus haut, Ã  la
                    // dÃ©claration de `trace_escalade`). La clÃ© ne porte donc
                    // que ce qui identifie une PHASE, pas une position dans
                    // cette phase.
                    let face_pour_la_cle = match acteur.ch.attachment {
                        character::attach::Attachment::On { face, .. } => format!("{face:?}"),
                        character::attach::Attachment::Falling { .. } => "chute".to_string(),
                        character::attach::Attachment::Dragged => "portÃ©".to_string(),
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

            // â”€â”€ Sur changement seulement : la taille de la fenÃªtre â”€â”€â”€â”€â”€
            // Elle ne dÃ©pend que du manifeste et de l'Ã©chelle de l'Ã©cran.
            // L'appeler Ã  60 Hz coÃ»tait 8 points de pourcentage de CPU pour
            // rien (voir l'avertissement de `render::placer`).
            let taille = character::attach::window_size(&acteur.ch.manifest, echelle_affichage);
            if acteur.derniere_taille != Some(taille) {
                // `continue` et non `return` : voir la traversÃ©e des clics
                // plus haut â€” la fenÃªtre d'un acteur peut disparaÃ®tre sans
                // que les autres aient Ã  mourir avec.
                if render::dimensionner(&handle, &acteur.label, taille).is_err() {
                    continue;
                }
                acteur.derniere_taille = Some(taille);
            }

            // CachÃ© ou session verrouillÃ©e : on a fait tourner le
            // comportement ci-dessus, et on s'arrÃªte lÃ . `visible` est lu
            // une fois pour tous, avant la boucle.
            if !visible {
                // On oublie ce qu'on avait posÃ© : au retour, il faut tout
                // repousser, la fenÃªtre ayant pu Ãªtre masquÃ©e entre-temps.
                acteur.dernier_coin = None;
                acteur.dernier_rendu = None;
                continue;
            }

            // â”€â”€ 60 Hz : le rendu â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
            // La position est DÃ‰RIVÃ‰E Ã  chaque image (dÃ©cision nÂ° 1).
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

            // N'Ã©mettre que sur changement â€” sauf pendant l'amorÃ§age, oÃ¹
            // l'Ã©couteur du webview n'existe peut-Ãªtre pas encore.
            //
            // Ã€ 60 Hz, une pose de marche ne change d'image que ~8 fois par
            // seconde : on Ã©conomise ~85 % des messages, sans une ligne de
            // logique cÃ´tÃ© front.
            let amorcage = maintenant < AMORCAGE;
            if amorcage || acteur.dernier_rendu != Some(rendu) {
                if let Err(e) = render::pousser(&handle, &acteur.label, rendu) {
                    // On imprime : un acteur qui meurt en silence donne un
                    // personnage figÃ© sans explication, et c'est exactement
                    // ce qu'on a dÃ©jÃ  passÃ© du temps Ã  diagnostiquer.
                    eprintln!("rendu impossible pour {} : {e}", acteur.label);
                    continue;
                }
                acteur.dernier_rendu = Some(rendu);
            }
        }

        // â”€â”€ Les dÃ©parts achevÃ©s â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        //
        // Du dernier au premier, pour que les index collectÃ©s pendant la
        // boucle restent valides Ã  mesure qu'on retire.
        for i in a_retirer.into_iter().rev() {
            let parti = acteurs.remove(i);
            if let Some(w) = handle.get_webview_window(&parti.label) {
                let _ = w.destroy();
            }
            println!("Â« {} Â» est parti ({})", parti.nom, parti.label);
        }

        // Un menu contextuel a Ã©tÃ© ouvert : il a bloquÃ© plusieurs secondes,
        // `maintenant` est pÃ©rimÃ©. On repart sur une image neuve plutÃ´t que
        // de juger les Ã©chÃ©ances de tout le monde sur un instant faux.
        if menu_ouvert {
            continue;
        }

        // â”€â”€ Diagnostic de cadence â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        // `SHIMEJI_CADENCE=1` imprime toutes les 5 s les images par seconde
        // rÃ©ellement atteintes et le temps de travail par image. C'est la
        // seule faÃ§on de savoir si la boucle DORT ou si elle tourne Ã  plat :
        // quand le travail dÃ©passe la pÃ©riode, `checked_sub` rend `None` et
        // il n'y a aucun sommeil du tout.
        if trace_cadence {
            images_depuis_trace += 1;
            travail_cumule += debut.elapsed();
            if maintenant.saturating_sub(derniere_trace) >= Duration::from_secs(5) {
                let secondes = maintenant.saturating_sub(derniere_trace).as_secs_f64();
                let moyenne_us = travail_cumule.as_micros() as f64 / images_depuis_trace as f64;
                println!(
                    "cadence : {:.1} img/s, travail moyen {:.0} Âµs, {} placements sur {} images ({:.0} %)",
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

        // â”€â”€ Tenir la cadence â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
        // `checked_sub` : si une image a pris plus de 16,7 ms (machine
        // chargÃ©e), `PERIODE - ecoule` dÃ©borderait. On enchaÃ®ne alors
        // immÃ©diatement plutÃ´t que de dormir une Ã©ternitÃ©.
        let ecoule = debut.elapsed();
        if let Some(reste) = PERIODE.checked_sub(ecoule) {
            std::thread::sleep(reste);
        }
    }
}
