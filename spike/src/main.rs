// Spike étape 0 — JETABLE. Son produit est une réponse écrite, pas ce code.
// Voir docs/plans/2026-09-08-etape-0-spike-overlay.md

use std::time::{Duration, Instant};
use tauri::{Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

const LABEL: &str = "pet";
const SIZE: f64 = 128.0;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // ── Topologie des écrans ────────────────────────────────
            // Information dont l'étape 1 a besoin de toute façon (spec §3.4) :
            // on la relève ici pendant qu'on y est.
            //
            // `available_monitors` vit dans `impl App<Wry>` : l'appel sur le
            // `app` de `setup` est donc correct (vérifié sur tauri 2.11.5).
            let monitors = app.available_monitors()?;
            let mut min_x = i32::MAX;
            let mut max_x = i32::MIN;

            for (i, m) in monitors.iter().enumerate() {
                let p = m.position();
                let s = m.size();
                println!(
                    "écran {} : position=({}, {}) taille=({} × {}) échelle={}",
                    i,
                    p.x,
                    p.y,
                    s.width,
                    s.height,
                    m.scale_factor()
                );
                min_x = min_x.min(p.x);
                max_x = max_x.max(p.x + s.width as i32);
            }

            if monitors.is_empty() {
                // Aucun moniteur rapporté : on se rabat sur une plage sûre
                min_x = 0;
                max_x = 1920;
            }
            println!("bureau virtuel : x de {} à {}", min_x, max_x);

            // ── La fenêtre du personnage ────────────────────────────
            // Tous les attributs sont explicites : c'est exactement la
            // combinaison que le spike doit valider (spec §3.2).
            let win = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
                .title("spike-overlay")
                .inner_size(SIZE, SIZE)
                .decorations(false)
                .transparent(true)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .shadow(false)
                .focused(false)
                .build()?;

            // Les clics traversent en permanence (spec §3.3).
            win.set_ignore_cursor_events(true)?;

            // ── Déplacement à 60 Hz sur tout le bureau virtuel ──────
            // On passe par `AppHandle`, le point d'entrée multi-thread de
            // Tauri, et on retrouve la fenêtre par son label à chaque image.
            // Un `HashMap` par image est négligeable, et ça évite de parier
            // sur `WebviewWindow: Send` — non vérifiable sans compilateur.
            let handle = app.handle().clone();
            let span = (max_x - min_x - SIZE as i32).max(1) as f64;

            std::thread::spawn(move || {
                let start = Instant::now();
                loop {
                    let Some(win) = handle.get_webview_window(LABEL) else {
                        // Fenêtre fermée : plus rien à déplacer.
                        return;
                    };

                    let t = start.elapsed().as_secs_f64();

                    // Aller-retour horizontal à 300 px/s : traverse les écrans.
                    let phase = (t * 300.0 / span) % 2.0;
                    let progress = if phase < 1.0 { phase } else { 2.0 - phase };
                    let x = min_x as f64 + progress * span;

                    // Ondulation verticale, pour rendre visible toute saccade.
                    let y = 300.0 + (t * 1.5).sin() * 150.0;

                    if win
                        .set_position(PhysicalPosition::new(x as i32, y as i32))
                        .is_err()
                    {
                        return;
                    }

                    std::thread::sleep(Duration::from_millis(16));
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("échec au lancement de l'application Tauri");
}
