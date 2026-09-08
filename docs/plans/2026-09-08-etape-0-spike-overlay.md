# Étape 0 — Spike overlay transparent : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prouver sur cette machine qu'une fenêtre Tauri v2 peut être simultanément transparente, sans bordure, au premier plan, hors taskbar, traversée par les clics, et déplacée à 60 Hz à travers deux écrans de DPI différents — avant d'écrire une ligne de physique.

**Architecture:** Un projet Tauri autonome et **jetable**, dans `spike/`, hors du code du produit. Il crée une fenêtre de 128×128 par code (pas par configuration) pour contrôler chaque attribut, affiche un PNG, et la déplace en boucle sur toute l'étendue du bureau virtuel. Il imprime aussi la topologie des écrans, information dont l'étape 1 aura besoin de toute façon.

**Tech Stack:** Rust (cible MSVC), Tauri v2, `cargo-tauri`. Aucun Node, aucun bundler, aucune dépendance npm.

**Spec:** `docs/specs/2026-09-08-design.md` — voir §3.2 (modèle de fenêtre), §3.3 (hit-testing), §3.4 (coordonnées), §11 (étapes), §12 (risques).

---

## Contexte d'exécution — à lire avant de commencer

**Rust n'est pas installé sur cette machine.** Constaté le 2026-09-08 :

| Outil | État |
|---|---|
| `rustc` / `cargo` / `rustup` | ❌ absents |
| WebView2 | ✅ présent (152.0.4191.66) |
| Node | ⚠️ v14.17.0 — **non utilisé**, voir contraintes |
| Visual Studio Installer | présent, composants C++ à confirmer |

**Aucune commande de ce plan n'a été exécutée ni vérifiée par son auteur** : la chaîne
d'outils manquait. Chaque étape « Lancer » est à exécuter, et son résultat à comparer à
l'attendu. Un écart n'est pas un échec du spike — c'est une information à consigner en
Tâche 3.

---

## Global Constraints

Exigences valables pour toutes les tâches, reprises de la spec.

- **Cible Rust** : `x86_64-pc-windows-msvc`. Pas de GNU.
- **Aucun Node, aucun npm, aucun `package.json`.** On passe par `cargo-tauri`. La voie
  npm exigerait Node 18+ ; le Node 14 de la machine la ferme, et s'en passer supprime la
  contrainte. (Spec §4)
- **Front statique** : HTML/CSS servi depuis un dossier. Pas de bundler, pas de
  TypeScript, pas de framework, pas de canvas. (Spec §13)
- **Tout en pixels physiques du bureau virtuel.** Le facteur d'échelle d'un moniteur ne
  sert qu'au dimensionnement du sprite. Ne jamais mélanger coordonnées logiques et
  physiques. (Spec §3.4)
- **Taille de fenêtre** : 128×128, comme les frames. (Spec §3.2)
- **Le code de `spike/` est jetable.** Son produit est une réponse écrite, pas du code à
  faire vivre. Ne rien y soigner au-delà de ce qui sert à répondre. (Spec §11)
- **Runtime Visual C++ lié statiquement** pour l'exe final — hors sujet pour le spike,
  qui tourne en développement. (Spec §4)

---

## Structure de fichiers

Créés par ce plan, tous sous `spike/` :

| Fichier | Responsabilité |
|---|---|
| `spike/Cargo.toml` | dépendances du spike |
| `spike/build.rs` | génération Tauri |
| `spike/tauri.conf.json` | configuration Tauri — **aucune fenêtre déclarée** |
| `spike/src/main.rs` | création de la fenêtre par code, topologie des écrans, boucle de déplacement |
| `spike/ui/index.html` | affichage du PNG sur fond transparent |
| `spike/ui/shime1.png` | une frame, copiée depuis `characters/blob/img/` |
| `docs/specs/2026-09-08-spike-0-resultat.md` | **le livrable réel** — la réponse |

Rien hors de `spike/` et `docs/` n'est touché. Le code du produit n'existe pas encore.

---

## Tâche 1 : Installer la chaîne d'outils

**Files:** aucun fichier du dépôt modifié.

**Interfaces:**
- Consomme : rien.
- Produit : `cargo`, `rustc`, `cargo tauri` disponibles dans le `PATH`, cible MSVC installée.

> **À faire par l'utilisateur, pas par un agent.** L'installation des composants C++ de
> Visual Studio ouvre une interface graphique et pèse plusieurs Go. Un agent qui tenterait
> de l'automatiser bloquerait sur une fenêtre invisible.

- [ ] **Step 1 : Installer les composants C++ de Visual Studio**

La cible MSVC de Rust a besoin du linker et du SDK Windows. Vérifier d'abord s'ils sont
déjà là :

```powershell
& "C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe" -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
```

Attendu : un chemin d'installation. **Si la sortie est vide**, installer la charge de
travail « Développement Desktop en C++ » via le Visual Studio Installer, ou :

```powershell
winget install --id Microsoft.VisualStudio.2022.BuildTools --override "--quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

- [ ] **Step 2 : Installer Rust**

```powershell
winget install --id Rustlang.Rustup
```

Puis **ouvrir un nouveau terminal** — le `PATH` de la session courante ne contient pas
encore `cargo`.

- [ ] **Step 3 : Vérifier Rust et la cible**

```powershell
rustc --version; cargo --version; rustup target list --installed
```

Attendu : deux numéros de version, et `x86_64-pc-windows-msvc` dans la liste. Si la cible
manque :

```powershell
rustup target add x86_64-pc-windows-msvc
```

- [ ] **Step 4 : Installer le CLI Tauri par cargo**

```powershell
cargo install tauri-cli --version "^2"
```

Compte plusieurs minutes. **Ne pas** utiliser `npm install @tauri-apps/cli` : c'est la
voie que le Node 14 de cette machine ferme, et on n'en a pas besoin.

- [ ] **Step 5 : Vérifier le CLI**

```powershell
cargo tauri --version
```

Attendu : une version `2.x`.

- [ ] **Step 6 : Consigner les versions obtenues**

Créer `docs/specs/2026-09-08-spike-0-resultat.md` avec les versions relevées :

```markdown
# Étape 0 — résultat du spike

## Chaîne d'outils

| Outil | Version |
|---|---|
| rustc | <coller la sortie> |
| cargo | <coller la sortie> |
| cargo-tauri | <coller la sortie> |
| WebView2 | 152.0.4191.66 |
| Composants C++ VS | <chemin de vswhere, ou "installés le <date>"> |

## Résultat des vérifications

_(rempli en Tâche 3)_
```

- [ ] **Step 7 : Commit**

```bash
git add docs/specs/2026-09-08-spike-0-resultat.md
git commit -m "chore: consigner la chaîne d'outils installée pour le spike"
```

---

## Tâche 2 : Le spike

**Files:**
- Create: `spike/Cargo.toml`
- Create: `spike/build.rs`
- Create: `spike/tauri.conf.json`
- Create: `spike/src/main.rs`
- Create: `spike/ui/index.html`
- Create: `spike/ui/shime1.png` (copie)

**Interfaces:**
- Consomme : la chaîne d'outils de la Tâche 1.
- Produit : un exécutable qui, lancé, affiche une frame de 128×128 se déplaçant sur le
  bureau virtuel, et imprime sur la sortie standard la topologie des écrans au format
  `écran <i> : position=(x, y) taille=(w × h) échelle=<f>`.

> Pas de test automatisé ici, et c'est volontaire : ce qu'on vérifie — transparence,
> premier plan, clics traversants, fluidité — n'est observable qu'à l'œil. La spec §10.4
> liste précisément ces quatre points comme les seuls non automatisables. La Tâche 3 est
> le test.

- [ ] **Step 1 : Créer l'arborescence et copier une frame**

```bash
mkdir -p spike/src spike/ui
cp characters/blob/img/shime1.png spike/ui/shime1.png
```

- [ ] **Step 2 : Écrire `spike/Cargo.toml`**

```toml
[package]
name = "spike-overlay"
version = "0.0.0"
edition = "2021"
publish = false

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
```

- [ ] **Step 3 : Écrire `spike/build.rs`**

```rust
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 4 : Écrire `spike/tauri.conf.json`**

`"windows": []` est délibéré : la fenêtre est créée par code en Step 5, pour que chaque
attribut soit explicite et modifiable pendant le diagnostic. `"csp": null` évite qu'une
politique de sécurité bloque le chargement du PNG local pendant un spike.

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "spike-overlay",
  "version": "0.0.0",
  "identifier": "dev.local.spike-overlay",
  "build": {
    "frontendDist": "../ui"
  },
  "app": {
    "windows": [],
    "security": {
      "csp": null
    }
  }
}
```

- [ ] **Step 5 : Écrire `spike/src/main.rs`**

```rust
use std::time::{Duration, Instant};
use tauri::{Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // ── Topologie des écrans ────────────────────────────────
            // Information dont l'étape 1 a besoin de toute façon (spec §3.4) :
            // on la relève ici pendant qu'on y est.
            let monitors = app.available_monitors()?;
            let mut min_x = i32::MAX;
            let mut max_x = i32::MIN;

            for (i, m) in monitors.iter().enumerate() {
                let p = m.position();
                let s = m.size();
                println!(
                    "écran {} : position=({}, {}) taille=({} × {}) échelle={}",
                    i, p.x, p.y, s.width, s.height, m.scale_factor()
                );
                min_x = min_x.min(p.x);
                max_x = max_x.max(p.x + s.width as i32);
            }

            if min_x == i32::MAX {
                // Aucun moniteur rapporté : on se rabat sur une plage sûre
                min_x = 0;
                max_x = 1920;
            }
            println!("bureau virtuel : x de {} à {}", min_x, max_x);

            // ── La fenêtre du personnage ────────────────────────────
            // Tous les attributs sont explicites : c'est la combinaison
            // exacte que le spike doit valider (spec §3.2).
            let win = WebviewWindowBuilder::new(
                app,
                "pet",
                WebviewUrl::App("index.html".into()),
            )
            .title("spike-overlay")
            .inner_size(128.0, 128.0)
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
            let span = (max_x - min_x - 128).max(1) as f64;
            std::thread::spawn(move || {
                let start = Instant::now();
                loop {
                    let t = start.elapsed().as_secs_f64();

                    // Aller-retour horizontal à 300 px/s : traverse les écrans.
                    let phase = (t * 300.0 / span) % 2.0;
                    let progress = if phase < 1.0 { phase } else { 2.0 - phase };
                    let x = min_x as f64 + progress * span;

                    // Ondulation verticale, pour rendre visible toute saccade.
                    let y = 300.0 + (t * 1.5).sin() * 150.0;

                    let _ = win.set_position(PhysicalPosition::new(x as i32, y as i32));
                    std::thread::sleep(Duration::from_millis(16));
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("échec au lancement de l'application Tauri");
}
```

Note : pas de `windows_subsystem = "windows"` — on **veut** la console pour lire la
topologie des écrans. L'application finale la supprimera.

- [ ] **Step 6 : Écrire `spike/ui/index.html`**

`background: transparent` sur `html` **et** `body` : si l'un des deux garde un fond,
WebView2 peint un rectangle opaque et la transparence de la fenêtre ne se voit pas —
c'est la cause de faux négatifs la plus courante sur ce test.

```html
<!doctype html>
<html>
<head>
<meta charset="utf-8">
<style>
  html, body {
    margin: 0;
    padding: 0;
    width: 128px;
    height: 128px;
    background: transparent;
    overflow: hidden;
  }
  #pet {
    width: 128px;
    height: 128px;
    background: url('shime1.png') no-repeat center;
    image-rendering: pixelated;
  }
</style>
</head>
<body>
  <div id="pet"></div>
</body>
</html>
```

- [ ] **Step 7 : Compiler**

```powershell
cd spike
cargo build
```

Attendu : compilation réussie. Premier build long (plusieurs minutes, Tauri tire beaucoup
de dépendances).

**Si le linker échoue** — c'est le symptôme de composants C++ manquants : reprendre la
Tâche 1 Step 1.

**Si le compilateur refuse un appel.** Ce code n'a pas pu être compilé par son auteur
(Rust absent, voir « Contexte d'exécution »). Deux appels sont les candidats les plus
probables à un écart de signature selon la version exacte de Tauri v2 résolue par cargo :

| Symptôme | Correction |
|---|---|
| `available_monitors` inconnu sur `app` | l'appeler sur la fenêtre après `build()` : déplacer le bloc topologie après la création de `win` et utiliser `win.available_monitors()?` |
| `shadow` inconnu sur le builder | retirer `.shadow(false)` — c'est un confort visuel, pas une des sept propriétés testées |

Toute autre erreur : lire le message, il est en général explicite, et consigner la
correction en Tâche 3 Step 2 — elle vaudra pour l'étape 1.

- [ ] **Step 8 : Lancer**

```powershell
cd spike
cargo tauri dev
```

Attendu : la console imprime un `écran <i> : …` par moniteur, puis `bureau virtuel : …`,
et une petite silhouette blanche traverse l'écran de gauche à droite en ondulant.

- [ ] **Step 9 : Commit**

```bash
git add spike/
git commit -m "spike: fenêtre overlay transparente Tauri, jetable"
```

---

## Tâche 3 : Vérifier les sept propriétés et trancher

**Files:**
- Modify: `docs/specs/2026-09-08-spike-0-resultat.md`

**Interfaces:**
- Consomme : le spike lancé de la Tâche 2.
- Produit : **la décision** — poursuivre en Tauri, ou basculer sur Electron. Aucune ligne
  de physique n'est écrite avant celle-ci (spec §11, §12).

> C'est le livrable réel de l'étape 0. Le code de la Tâche 2 n'était qu'un instrument.

- [ ] **Step 1 : Vérifier les sept propriétés, spike en cours d'exécution**

Cocher chacune. Une seule case rouge ne condamne pas le projet — elle envoie en Step 3.

| # | Propriété | Comment vérifier | Attendu |
|---|---|---|---|
| 1 | **Transparence réelle** | regarder autour de la silhouette | le bureau est visible ; **aucun** carré blanc ou noir de 128 px |
| 2 | **Premier plan** | maximiser une fenêtre (VSCode, navigateur) | la silhouette reste visible par-dessus |
| 3 | **Hors taskbar et hors Alt+Tab** | regarder la barre des tâches, faire Alt+Tab | `spike-overlay` n'apparaît nulle part |
| 4 | **Clics traversants** | cliquer un bouton ou une icône **sous** la silhouette | le clic atteint ce qui est dessous, la silhouette est ignorée |
| 5 | **Pas de vol de focus** | taper dans un éditeur pendant le passage de la silhouette | la frappe n'est jamais interrompue |
| 6 | **Fluidité à 60 Hz** | suivre le mouvement des yeux | déplacement régulier ; pas de saccade, pas de traînée, pas de rémanence du fond |
| 7 | **Multi-écran** | suivre la traversée d'un écran à l'autre | franchit la frontière sans disparaître ni sauter ; si les écrans ont des DPI différents, la taille apparente peut varier — **le noter, ce n'est pas un échec** |

- [ ] **Step 2 : Consigner la topologie et les résultats**

Remplir la section « Résultat des vérifications » de
`docs/specs/2026-09-08-spike-0-resultat.md` :

```markdown
## Résultat des vérifications

Date : <date>

### Topologie des écrans relevée

<coller la sortie console : les lignes "écran <i> : …" et "bureau virtuel : …">

### Les sept propriétés

| # | Propriété | Résultat | Observations |
|---|---|---|---|
| 1 | Transparence réelle | ✅ / ❌ | |
| 2 | Premier plan | ✅ / ❌ | |
| 3 | Hors taskbar et Alt+Tab | ✅ / ❌ | |
| 4 | Clics traversants | ✅ / ❌ | |
| 5 | Pas de vol de focus | ✅ / ❌ | |
| 6 | Fluidité à 60 Hz | ✅ / ❌ | |
| 7 | Multi-écran | ✅ / ❌ | |

### Décision

<Poursuivre en Tauri | Poursuivre avec la réserve suivante : … | Basculer sur Electron>

### Conséquences pour l'étape 1

<ce que ces résultats imposent ou permettent — par exemple : cadence de déplacement
retenue, attribut de fenêtre à ajouter, comportement DPI à gérer>
```

- [ ] **Step 3 : En cas d'échec — la marche à suivre**

Ne pas improviser. Chaque échec a un traitement établi ; l'ordre compte.

| Échec | À essayer, dans cet ordre |
|---|---|
| **1 — carré opaque** | ① vérifier `background: transparent` sur `html` *et* `body` ② supprimer tout `background-color` hérité ③ confirmer `.transparent(true)` **et** `.decorations(false)` — sur Windows, la transparence exige l'absence de décoration |
| **2 — passe derrière** | rappeler `set_always_on_top(true)` après l'affichage ; certains gestionnaires réinitialisent le rang à la première présentation |
| **3 — visible en Alt+Tab** | `.skip_taskbar(true)` couvre la barre des tâches, pas toujours Alt+Tab ; noter comme réserve mineure, à traiter par style de fenêtre étendu à l'étape 1 |
| **4 — clics bloqués** | vérifier que `set_ignore_cursor_events(true)` est appelé **après** `build()` ; si l'appel échoue, lire l'erreur retournée |
| **6 — saccades** | passer la boucle à 33 ms (30 Hz) et réobserver. Si 30 Hz est fluide, **c'est la réponse** : l'étape 1 déplacera à 30 Hz avec interpolation (spec §12), et ce n'est pas un échec du modèle |
| **7 — saute entre écrans** | relever les positions imprimées : un bureau virtuel à coordonnées négatives (écran secondaire à gauche) est le cas le plus probable — le vérifier avant toute conclusion |

**Bascule sur Electron** uniquement si 1, 2 ou 4 résiste à tous les remèdes ci-dessus :
ce sont les trois propriétés sans lesquelles le produit n'existe pas. La spec reste alors
valable à ~90 % — seuls §3.1 (la logique repasserait en JS) et §4 (distribution) changent.
Consigner précisément ce qui a résisté : c'est ce qui justifiera le changement.

- [ ] **Step 4 : Commit**

```bash
git add docs/specs/2026-09-08-spike-0-resultat.md
git commit -m "docs: résultat du spike étape 0 et décision de stack"
```

- [ ] **Step 5 : Décider du sort de `spike/`**

Le code est jetable, mais il a une valeur résiduelle : c'est le plus petit reproducteur
d'un éventuel problème d'affichage. Deux options légitimes :

```bash
# le conserver comme référence de diagnostic
git mv spike docs/spike-etape-0
git commit -m "chore: archiver le spike de l'étape 0 comme référence"

# ou s'en débarrasser, tout étant consigné
git rm -r spike && git commit -m "chore: retirer le spike de l'étape 0, résultat consigné"
```

**Ne pas** faire évoluer `spike/` vers l'application : l'étape 1 repart d'une structure
propre (voir annexe).

---

## Annexe A — Structure verrouillée pour l'étape 1

Décidée maintenant pour que les tâches de l'étape 1 s'y accrochent sans rediscussion.
Chaque fichier a une responsabilité unique (spec §3.1, §10.2).

```
shimeji-desktop/
├── ui/
│   ├── index.html          afficheur de sprite, délibérément bête
│   └── pet.js              reçoit { image, flip }, pose un background — rien d'autre
└── src-tauri/
    ├── Cargo.toml
    ├── build.rs
    ├── tauri.conf.json
    └── src/
        ├── main.rs         amorçage, tray, instanciation des personnages
        ├── config.rs       chargement config + résolution des chemins (exe puis %APPDATA%)
        ├── geom.rs         Rect, Point, Vec2
        ├── clock.rs        horloge INJECTABLE — contrainte spec §10.2
        ├── rng.rs          aléatoire INJECTABLE et graînable — contrainte spec §10.2
        ├── probe/
        │   ├── mod.rs      trait SystemProbe — la frontière testable, contrainte spec §10.2
        │   ├── win32.rs    implémentation réelle (crate `windows`)
        │   └── fake.rs     implémentation de test
        ├── world.rs        Platform, PlatformId, Face, construction du monde
        ├── character/
        │   ├── mod.rs      état du personnage
        │   ├── manifest.rs chargement et validation de mascot.json
        │   ├── attach.rs   Attachment, position DÉRIVÉE, transitions — décision n° 1
        │   └── physics.rs  chute, atterrissage
        ├── behavior/
        │   ├── mod.rs      les trois couches
        │   ├── reflex.rs   couche 1
        │   ├── intention.rs couche 2
        │   └── desire.rs   couche 3, tirage pondéré
        ├── render.rs       pousse la frame vers le webview, déplace la fenêtre
        └── sim.rs          mode simulation sans écran — spec §10.3
```

**Trois fichiers sont non négociables et doivent exister dès la première tâche de
l'étape 1** — `clock.rs`, `rng.rs`, `probe/mod.rs`. Ce sont les trois contraintes de
testabilité de la spec §10.2 : elles ne se rattrapent pas après coup. Un `Instant::now()`
ou un `rand::random()` appelé directement quelque part rend intestable tout ce qui en
dépend.

**Ce que l'étape 1 ne contient pas**, malgré la structure : aucune plateforme de fenêtre
(le monde n'expose que le sol de chaque écran), donc **pas de soustraction d'intervalles**
et **pas de filtrage de fenêtres**. Ces deux morceaux appartiennent à l'étape 4. YAGNI.

---

## Annexe B — Contour des étapes 2 à 5

Chacune fera son propre plan, écrit à son tour. Résumé pour situer l'étape 1.

| Étape | Contenu | Nouveaux fichiers | Dépend de |
|---|---|---|---|
| **1** | il vit sur le sol : marche, court, demi-tour, tous les écrans, attrapable, tombe, atterrit ; tray, démarrage auto, config | toute l'annexe A | résultat de l'étape 0 |
| **2** | il réagit : inactivité → sommeil, heure → repas, batterie, verrouillage | `signals.rs` + lignes dans `desire.rs` | étape 1 |
| **3** | deuxième personnage : coexistence et rencontres | `behavior/social.rs` | étape 1 |
| **4** | il grimpe : plateformes de fenêtres, escalade, occlusion | `probe/win32.rs` étendu, `geom.rs` + soustraction 1D, `world.rs` étendu | étapes 1 et 2 |
| **5** | il suit : navigation vers la fenêtre active | `behavior/navigate.rs` | étape 4 |

L'étape 4 n'ajoute que des plateformes : ni la physique ni le comportement ne changent.
C'est le dividende du modèle de monde de la spec §5.1, et c'est ce qui justifie de livrer
le sol d'abord.
