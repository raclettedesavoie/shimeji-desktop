//! Spike jetable : **une fenêtre plein écran par écran**, au lieu d'une
//! fenêtre par personnage.
//!
//! Responsabilité unique : produire des **chiffres**, et rien d'autre. Il ne
//! dessine aucun comportement, ne lit aucun manifeste, et ne doit jamais être
//! promu tel quel — comme `spike-deplacements-groupes` et
//! `spike-deplacement-30hz` avant lui, c'est de la **mesure, pas une
//! promotion**.
//!
//! # Ce que les trois premières phases ont déjà rendu (2026-09-22)
//!
//! | Phase | CPU total | Latence médiane / max |
//! |---|---|---|
//! | 0 — témoin, aucune fenêtre | 0,7 % | 0 / 2 ms |
//! | 1 — 3 fenêtres plein écran **immobiles** | 2,9 % | 0 / 5 ms |
//! | 2 — + 170 `eval` groupés/s, 15 figurants | **156,8 %** | **0 / 51 ms** |
//! | 2 — idem avec **un seul** figurant | 53,0 % | 0 / 14 ms |
//!
//! **Deux conclusions, opposées.**
//!
//! 1. **La file n'est plus le problème.** `eval` groupé tient 0 ms médian là
//!    où l'application actuelle mesure 8 à 14 **secondes** à 15 personnages.
//!    La question que l'auteur avait soulevée — « `eval` n'emprunte-t-il pas
//!    le même canal que `set_position` ? » — a donc sa réponse : oui, mais
//!    170 messages/s y coûtent ce que 900 `SetWindowPos`/s rendaient
//!    intenable.
//! 2. **Le coût s'est déplacé dans les renderers**, et il est **par fenêtre
//!    animée, pas par sprite** : un figurant coûte 42 %, quinze répartis sur
//!    trois fenêtres en coûtent 140 (~46 % par fenêtre). Une fenêtre plein
//!    écran transparente semble donc repeindre **toute sa surface** à chaque
//!    image.
//!
//! # Ce que les leviers ci-dessous servent à trancher
//!
//! Si le coût suit la **surface**, une fenêtre plus petite doit le faire
//! chuter proportionnellement — et la « tuile » (fenêtre par personnage, mais
//! nettement plus grande que le sprite) redevient la bonne piste. Si le coût
//! suit la **transparence**, c'est la composition logicielle de WebView2 qu'il
//! faut attaquer, et la taille n'y changera rien.
//!
//! # Lancement
//!
//! ```powershell
//! $env:SHIMEJI_SPIKE_OVERLAY=0; cargo run   # témoin : aucune fenêtre
//! $env:SHIMEJI_SPIKE_OVERLAY=1; cargo run   # les fenêtres, immobiles
//! $env:SHIMEJI_SPIKE_OVERLAY=2; cargo run   # + eval groupés à 60 Hz
//! ```
//!
//! | Variable | Défaut | Effet |
//! |---|---|---|
//! | `SHIMEJI_SPIKE_N` | 15 | nombre de figurants simulés |
//! | `SHIMEJI_SPIKE_SECONDES` | 60 | durée (**60 s minimum** : le dossier CPU rappelle qu'une mesure sur 10 s ne veut rien dire) |
//! | `SHIMEJI_SPIKE_TAILLE` | plein écran | côté, en pixels physiques, d'une fenêtre carrée — le levier « surface » |
//! | `SHIMEJI_SPIKE_OPAQUE` | absent | fenêtres **non** transparentes — le levier « transparence » |
//! | `SHIMEJI_SPIKE_PLAT` | absent | retire `will-change: transform` du CSS — le levier « couches du compositeur » |

use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::charge;
use crate::geom::Rect;
use crate::probe::SystemProbe;

/// Un personnage simulé : une position qui avance et rebondit sur les bords
/// de sa zone.
///
/// Aucune physique réelle — on ne mesure pas le comportement, on mesure le
/// coût d'afficher *quelque chose qui bouge tout le temps*. Un mouvement
/// constant est le **pire cas**, donc le bon cas à mesurer : dans
/// l'application réelle, un personnage assis ne coûte rien.
struct Figurant {
    zone: usize,
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

/// Les réglages du spike, lus une fois dans l'environnement.
///
/// Un `struct` plutôt que six paramètres : la liste s'allonge à chaque
/// question qu'on veut trancher, et six arguments positionnels finiraient par
/// s'inverser silencieusement.
#[derive(Clone, Copy)]
struct Reglages {
    phase: u8,
    n: usize,
    secondes: u64,
    /// `None` = plein écran ; `Some(c)` = une fenêtre carrée de `c` pixels.
    taille: Option<u32>,
    opaque: bool,
    plat: bool,
    /// Fréquence des `eval`, en hertz. **Découplée de la cadence de la
    /// boucle**, qui reste à 60 Hz : c'est tout l'objet de la mesure. Un
    /// `eval` coûtant ~6,8 ms de CPU (mesuré), la seule question qui reste
    /// est combien on peut s'en payer par seconde.
    hz: f32,
}

/// Point d'entrée du spike. Ne rend jamais la main : il appelle `exit(0)`
/// quand la durée est écoulée.
pub fn lancer(phase: u8) {
    fn var_num<T: std::str::FromStr>(nom: &str) -> Option<T> {
        std::env::var(nom).ok().and_then(|s| s.trim().parse().ok())
    }

    let r = Reglages {
        phase,
        n: var_num("SHIMEJI_SPIKE_N").unwrap_or(15),
        secondes: var_num("SHIMEJI_SPIKE_SECONDES").unwrap_or(60),
        taille: var_num("SHIMEJI_SPIKE_TAILLE"),
        opaque: std::env::var("SHIMEJI_SPIKE_OPAQUE").is_ok(),
        plat: std::env::var("SHIMEJI_SPIKE_PLAT").is_ok(),
        hz: var_num("SHIMEJI_SPIKE_HZ").unwrap_or(60.0),
    };

    println!("── spike « une fenêtre par écran » ──");
    println!(
        "phase {} · {} figurants · {} s · fenêtre {} · {} · {}",
        r.phase,
        r.n,
        r.secondes,
        match r.taille {
            Some(c) => format!("{c}×{c}"),
            None => "plein écran".to_string(),
        },
        if r.opaque { "OPAQUE" } else { "transparente" },
        if r.plat { "sans will-change" } else { "will-change" },
    );
    println!("eval à {} Hz (la boucle reste à 60 Hz)", r.hz);
    match phase {
        0 => println!("témoin : aucune fenêtre créée, seule la sonde tourne"),
        1 => println!("fenêtres créées, IMMOBILES, aucun eval"),
        2 => println!("fenêtres + un eval groupé par fenêtre et par image"),
        _ => {
            eprintln!("phase inconnue : 0, 1 ou 2");
            std::process::exit(2);
        }
    }

    tauri::Builder::default()
        .setup(move |app| {
            let handle = app.handle().clone();

            // ── Topologie des écrans ────────────────────────────────────
            let sonde = crate::probe::win32::Win32Probe::new();
            let ecrans = sonde.screens();
            println!("{} écran(s) :", ecrans.len());
            for (i, e) in ecrans.iter().enumerate() {
                println!(
                    "  [{i}] {}×{} @ ({}, {})  échelle {}",
                    e.work_area.w, e.work_area.h, e.work_area.x, e.work_area.y, e.scale
                );
            }

            // ── Les zones : une par écran ───────────────────────────────
            //
            // Une « zone » est le rectangle que couvre une fenêtre du spike.
            // Plein écran par défaut ; réduite à un carré quand on veut
            // mesurer l'effet de la SURFACE. Le même rectangle sert aussi de
            // bornes de rebond aux figurants, ce qui garantit qu'aucun sprite
            // ne sort de sa fenêtre — donc qu'on mesure bien ce qu'on croit.
            let zones: Vec<Rect> = ecrans
                .iter()
                .map(|e| match r.taille {
                    None => e.work_area,
                    // Décalé de 50 px du coin pour que la fenêtre soit bien
                    // visible à l'œil pendant la mesure, et pas collée au bord.
                    Some(c) => Rect::new(
                        e.work_area.x + 50.0,
                        e.work_area.y + 50.0,
                        (c as f32).min(e.work_area.w),
                        (c as f32).min(e.work_area.h),
                    ),
                })
                .collect();

            // ── Les fenêtres ────────────────────────────────────────────
            if r.phase >= 1 {
                for (i, z) in zones.iter().enumerate() {
                    if let Err(msg) = creer_fenetre(&handle, i, z, &r) {
                        eprintln!("zone {i} : {msg}");
                    }
                }
            }

            // ── La boucle de mesure ─────────────────────────────────────
            // Un thread à part, comme la boucle de l'application : mesurer
            // depuis le thread principal mesurerait le thread principal en
            // train de s'attendre lui-même.
            std::thread::spawn(move || boucle(handle, r, zones));

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("le spike n'a pas pu démarrer");
}

/// Crée la fenêtre d'une zone : transparente (ou non), au premier plan,
/// traversante, et qui ne bougera plus jamais.
fn creer_fenetre(
    app: &tauri::AppHandle,
    index: usize,
    zone: &Rect,
    r: &Reglages,
) -> Result<(), String> {
    use tauri::{PhysicalPosition, PhysicalSize};

    let label = format!("spike-ecran-{index}");

    // Le fragment porte le réglage CSS jusqu'à la page, exactement comme
    // `index.html#<personnage>` le fait pour le vrai afficheur.
    let url = if r.plat {
        "spike-overlay.html#plat"
    } else {
        "spike-overlay.html"
    };

    let win = tauri::WebviewWindowBuilder::new(app, &label, tauri::WebviewUrl::App(url.into()))
        .title("spike overlay")
        .decorations(false)
        .transparent(!r.opaque)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .focused(false)
        .build()
        .map_err(|e| format!("création : {e}"))?;

    // Position et taille en pixels PHYSIQUES. Passer par le builder les
    // interpréterait en pixels logiques, donc faux sur l'écran à 125 % —
    // exactement le piège n° 4 des « coordonnées » de CLAUDE.md.
    win.set_position(PhysicalPosition::new(zone.x as i32, zone.y as i32))
        .map_err(|e| format!("set_position : {e}"))?;

    win.set_size(PhysicalSize::new(zone.w as u32, zone.h as u32))
        .map_err(|e| format!("set_size : {e}"))?;

    // Les clics traversent. Sans ça, une fenêtre plein écran au premier plan
    // rendrait la machine inutilisable — c'est LA propriété qui rend l'idée
    // acceptable, donc celle qu'il faut vérifier à l'œil pendant la mesure.
    win.set_ignore_cursor_events(true)
        .map_err(|e| format!("set_ignore_cursor_events : {e}"))?;

    // Les deux styles étendus de l'étape 0, pour être dans les mêmes
    // conditions que l'application réelle (pas de vol de focus, pas d'Alt+Tab).
    if let Err(e) = crate::render::appliquer_styles_etendus(&win) {
        eprintln!("styles étendus NON appliqués sur {label} : {e}");
    }

    println!("fenêtre {label} : {}×{} @ ({}, {})", zone.w, zone.h, zone.x, zone.y);
    Ok(())
}

/// La boucle à 60 Hz : fait avancer les figurants, envoie (ou non) les `eval`,
/// et relève la latence de la file deux fois par seconde.
fn boucle(app: tauri::AppHandle, r: Reglages, zones: Vec<Rect>) {
    if zones.is_empty() {
        eprintln!("aucun écran : rien à mesurer");
        std::process::exit(1);
    }

    let moniteur = Arc::new(charge::Moniteur::nouveau());

    // Les figurants, répartis en tourniquet sur les zones — comme le seraient
    // des personnages posés un peu partout.
    let mut figurants: Vec<Figurant> = (0..r.n)
        .map(|i| Figurant {
            zone: i % zones.len(),
            x: (i * 97 % 400) as f32,
            y: (i * 53 % 300) as f32,
            // Des vitesses volontairement différentes : si toutes les
            // positions changeaient à l'identique, le compositeur pourrait
            // optimiser un cas que la réalité n'offrira pas.
            vx: if i % 2 == 0 { 2.0 } else { -1.5 },
            vy: if i % 3 == 0 { 1.0 } else { -0.5 },
        })
        .collect();

    let debut = Instant::now();
    let fin = debut + Duration::from_secs(r.secondes);
    let periode = Duration::from_secs_f32(1.0 / 60.0);

    let mut images: u64 = 0;
    let mut evals: u64 = 0;
    let mut latences: Vec<u128> = Vec::new();
    let mut prochain_releve = Instant::now() + Duration::from_millis(500);
    let mut prochain_eval = Instant::now();

    // Tampon réutilisé pour construire le JavaScript. Le réallouer 180 fois
    // par seconde ferait mesurer l'allocateur en même temps que la file.
    let mut js = String::with_capacity(4096);

    while Instant::now() < fin {
        let image_debut = Instant::now();

        // ── Faire avancer les figurants ─────────────────────────────────
        for f in figurants.iter_mut() {
            let z = &zones[f.zone];
            // Bornes de rebond : la zone moins le sprite, jamais négatives
            // (une fenêtre plus petite que 128 px rendrait `max` nul).
            let bx = (z.w - 128.0).max(0.0);
            let by = (z.h - 128.0).max(0.0);
            f.x += f.vx;
            f.y += f.vy;
            if f.x < 0.0 || f.x > bx {
                f.vx = -f.vx;
                f.x = f.x.clamp(0.0, bx);
            }
            if f.y < 0.0 || f.y > by {
                f.vy = -f.vy;
                f.y = f.y.clamp(0.0, by);
            }
        }

        // ── Un eval groupé par fenêtre ──────────────────────────────────
        // Le débit d'`eval` est découplé de la cadence de la boucle : la
        // physique reste à 60 Hz, seul l'envoi est espacé. C'est exactement
        // ce que ferait l'application réelle si le webview interpolait entre
        // deux positions reçues.
        let envoyer = r.phase == 2 && Instant::now() >= prochain_eval;
        if envoyer {
            prochain_eval = Instant::now() + Duration::from_secs_f32(1.0 / r.hz);
        }

        if envoyer {
            for i in 0..zones.len() {
                js.clear();
                js.push_str("window.poserTous([");
                let mut premier = true;
                for (idf, f) in figurants.iter().enumerate() {
                    if f.zone != i {
                        continue;
                    }
                    if !premier {
                        js.push(',');
                    }
                    premier = false;
                    // Entiers : un pixel fractionnaire ferait lisser le
                    // pixel-art, et allongerait la chaîne pour rien.
                    js.push_str(&format!(
                        "[{},{},{},{}]",
                        idf,
                        f.x as i32,
                        f.y as i32,
                        if f.vx < 0.0 { "true" } else { "false" }
                    ));
                }
                js.push_str("])");

                // Une zone sans figurant ne doit pas coûter un aller-retour.
                if premier {
                    continue;
                }

                let label = format!("spike-ecran-{i}");
                // `let … else` : si la fenêtre a disparu, on saute cette zone
                // au lieu de paniquer. Équivalent d'un `match` dont la
                // branche `None` ferait `continue`.
                let Some(win) = tauri::Manager::get_webview_window(&app, &label) else {
                    continue;
                };
                // `as_str()` et non `&js` : `eval` prend `impl Into<String>`,
                // et l'on reste sur l'implémentation la plus directe.
                if win.eval(js.as_str()).is_ok() {
                    evals += 1;
                }
            }
        }

        // ── Le relevé, 2 fois par seconde ───────────────────────────────
        if Instant::now() >= prochain_releve {
            prochain_releve = Instant::now() + Duration::from_millis(500);
            let l = moniteur.latence();
            latences.push(l.as_millis());
            println!(
                "t={:>5.1}s  latence {:>7} ms  images {:>6}  evals {:>6}",
                debut.elapsed().as_secs_f32(),
                l.as_millis(),
                images,
                evals
            );
            // Relancé APRÈS la lecture, comme dans l'application : le jeton
            // en vol n'est pas celui qu'on vient de lire.
            charge::sonder(&app, &moniteur);
        }

        images += 1;

        // ── Tenir la cadence ────────────────────────────────────────────
        // `checked_sub` : si l'image a déjà dépassé son budget, il n'y a rien
        // à attendre — et une soustraction de `Duration` qui passerait sous
        // zéro paniquerait.
        if let Some(reste) = periode.checked_sub(image_debut.elapsed()) {
            std::thread::sleep(reste);
        }
    }

    // ── Le verdict ──────────────────────────────────────────────────────
    let duree = debut.elapsed().as_secs_f32();
    latences.sort_unstable();
    let mediane = latences.get(latences.len() / 2).copied().unwrap_or(0);
    let max = latences.last().copied().unwrap_or(0);

    println!("\n── résultat, phase {} ──", r.phase);
    println!("durée             : {duree:.1} s");
    println!("cadence           : {:.1} img/s", images as f32 / duree);
    println!("evals envoyés     : {evals}  ({:.0}/s)", evals as f32 / duree);
    println!("latence médiane   : {mediane} ms");
    println!("latence max       : {max} ms");
    println!("relevés           : {}", latences.len());

    std::process::exit(0);
}
