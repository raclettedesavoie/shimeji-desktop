# Étape 1a — « Il vit sur le sol » : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Un personnage `blob` vit sur le sol des deux écrans — il marche, court, s'arrête, fait demi-tour, on peut l'attraper à la souris, le lâcher, il tombe et il atterrit.

**Architecture:** Toute la logique en Rust, dans `src-tauri/`. Le cœur est **pur et testable sans écran** : géométrie, monde, accroche, chute, comportement, tous alimentés par une horloge, un générateur aléatoire et une sonde système **injectés** (spec §10.2). La couche visible est mince : une fenêtre de 128×128 par personnage, déplacée par Rust à 60 Hz, et un webview qui ne fait que poser un `src` d'image. Un **mode simulation** déroule le comportement sans écran.

**Tech Stack:** Rust (cible `x86_64-pc-windows-msvc`), Tauri 2.11.5, crate `windows` 0.61, `serde`/`serde_json`. Aucun Node, aucun bundler, aucun framework front, aucun canvas.

**Spec:** `docs/specs/2026-09-08-design.md` — §3 (architecture), §5 (le monde), §6 (le personnage), §7 (le comportement), §8 (format de personnage), §10 (vérification).

**Résultat de l'étape 0 :** `docs/specs/2026-09-08-spike-0-resultat.md` — la stack est validée, et ce document impose quatre choix repris dans les contraintes globales.

---

## Périmètre — ce que ce plan fait, et ne fait pas

L'étape 1 de la spec §11 couvre « marche, court, s'arrête, demi-tour ; tous les écrans ;
attrapable, il tombe et atterrit ; tray, démarrage auto, config ». Ces deux moitiés sont
de nature trop différente pour un seul plan : **ce plan est la première.**

| | Dans ce plan (1a) | Dans le plan 1b |
|---|---|---|
| ✅ | géométrie, monde, accroche, chute, comportement | |
| ✅ | la fenêtre, les styles étendus, le rendu | |
| ✅ | le manifeste `mascot.json` et son chargement | |
| ✅ | attrapable à la souris, hit-testing | |
| ✅ | le mode simulation sans écran | |
| ⬜ | | le tray et l'entrée « Quitter » |
| ⬜ | | `config.json` et sa résolution de chemins |
| ⬜ | | le démarrage automatique (clé `Run`) |
| ⬜ | | le rechargement à chaud des personnages |

> ⚠️ **Conséquence à connaître avant de lancer l'application de ce plan :** il n'y a pas
> encore de tray, donc **pas d'entrée « Quitter »**. La fenêtre est sans bordure, non
> focalisable, hors taskbar et hors Alt+Tab : elle ne peut pas se fermer normalement.
> **On arrête par `Ctrl+C` dans le terminal**, ou `Stop-Process -Name shimeji-desktop`.
> C'est exactement le désagrément que le plan 1b supprime.

**Ce que ce plan ne contient pas non plus, et c'est voulu** (spec §11, dividende du §5.1) :
aucune plateforme de fenêtre. Le monde n'expose que **le sol de chaque écran**. Donc
**pas de soustraction d'intervalles 1D**, **pas de filtrage de fenêtres**, pas d'escalade,
pas de murs, pas de plafond. Ces morceaux appartiennent à l'étape 4, et le modèle de monde
est fait pour qu'ils s'ajoutent sans toucher ni la physique ni le comportement. YAGNI.

Les **signaux** (inactivité, heure, batterie, verrouillage) appartiennent à l'étape 2. Ce
plan met en place les trois couches de comportement et une table d'envies à deux entrées,
pour que l'étape 2 soit littéralement « ajouter des lignes » (décision n° 5).

---

## Global Constraints

Valables pour **toutes** les tâches. Les quatre premières viennent du résultat de
l'étape 0 et ne sont pas rediscutables : chacune a été payée par un diagnostic.

- **60 Hz, sans repli.** L'étape 0 a mesuré que déplacer une fenêtre à 60 Hz est fluide
  sur cette machine. Le repli « 30 Hz + interpolation » de la spec §12 est **abandonné** —
  ne pas le réintroduire.
- **`WS_EX_NOACTIVATE` et `WS_EX_TOOLWINDOW` posés dès la création de la fenêtre.** Tauri
  ne les pose pas. Sans le premier, réactiver les clics dans la hitbox (Tâche 11) ferait
  qu'attraper le personnage **volerait le focus de l'éditeur**. Sans le second, l'exclusion
  d'Alt+Tab ne repose que sur une heuristique de Windows 11.
- **`windows` épinglé en `0.61`** — la version dont dépend Tauri 2.11.5. Deux versions
  majeures de cette crate donnent deux types `HWND` **distincts**, et le message du
  compilateur parle alors de deux types de même nom, ce qui est déroutant.
- **Ne jamais supposer `x ≥ 0`** sur le bureau virtuel. C'est vrai sur cette machine
  (écran secondaire à droite), et faux dès qu'on branche un écran à gauche.
- **Cible Rust** : `x86_64-pc-windows-msvc`. Pas de GNU.
- **Compiler depuis PowerShell**, jamais depuis Git Bash — sinon rustc peut pêcher le
  `link.exe` de Git for Windows et rendre une erreur `extra operand` opaque.
- **Aucun Node, aucun npm, aucun `package.json`.** Front statique. Pas de bundler, pas de
  TypeScript, pas de framework, pas de canvas. (Spec §13)
- **Tout en pixels physiques du bureau virtuel**, sans exception. Le facteur d'échelle d'un
  moniteur ne sert **qu'**à dimensionner le sprite à l'affichage. (Spec §3.4)
- **Le sol est la zone de travail, pas l'écran** — sinon il marche sous la barre des
  tâches. C'est `rcWork` de `MONITORINFO`, pas `rcMonitor`.
- **Les trois contraintes de testabilité de la spec §10.2 sont en place dès la Tâche 1** :
  temps injecté, aléatoire injecté et graînable, couche win32 derrière une interface.
  Aucun `Instant::now()` ni `rand::random()` ailleurs que dans les implémentations
  réelles de `clock.rs` et `rng.rs`.
- **Code abondamment commenté en français**, expliquant le *pourquoi* et les constructions
  Rust non évidentes. C'est une exigence explicite de l'auteur (`CLAUDE.md`), pas un style.
- **Taille de fenêtre** : 128×128, comme les frames.
- **Runtime Visual C++ lié statiquement** pour l'exe final (spec §4). Réglé en Tâche 1.

---

## Structure de fichiers

Reprise de l'**annexe A du plan de l'étape 0**, qui la verrouille. Les fichiers de 1b sont
listés en gris pour situer, mais **ne sont pas créés par ce plan**.

```
shimeji-desktop/
├── characters/blob/
│   ├── mascot.json         ← CRÉÉ en Tâche 4 (n'existe pas encore ; les 46 PNG, oui)
│   └── img/shime1..46.png     déjà là
├── ui/
│   ├── index.html          afficheur de sprite, délibérément bête
│   └── pet.js              reçoit { image, flip }, pose un src — rien d'autre
└── src-tauri/
    ├── Cargo.toml
    ├── build.rs
    ├── tauri.conf.json
    ├── icons/icon.ico      OBLIGATOIRE — son absence fait échouer tauri-build
    └── src/
        ├── main.rs         amorçage, les trois horloges, instanciation
        ├── geom.rs         Point, Vec2, Rect, Face
        ├── clock.rs        horloge INJECTABLE — spec §10.2
        ├── rng.rs          aléatoire INJECTABLE et graînable — spec §10.2
        ├── probe/
        │   ├── mod.rs      trait SystemProbe — la frontière testable, spec §10.2
        │   ├── win32.rs    implémentation réelle (crate `windows`)
        │   └── fake.rs     implémentation de test
        ├── world.rs        Platform, PlatformId, World, construction depuis les écrans
        ├── character/
        │   ├── mod.rs      Character, Facing, l'animation des poses
        │   ├── manifest.rs chargement et validation de mascot.json
        │   ├── attach.rs   Attachment, position DÉRIVÉE, ancre — décision n° 1
        │   └── physics.rs  chute, atterrissage
        ├── behavior/
        │   ├── mod.rs      l'enchaînement des trois couches
        │   ├── reflex.rs   couche 1 — non négociable
        │   ├── intention.rs couche 2 — une seule à la fois, délai d'abandon
        │   └── desire.rs   couche 3 — tirage pondéré
        ├── render.rs       pousse la frame vers le webview, déplace la fenêtre
        └── sim.rs          mode simulation sans écran — spec §10.3
                            [1b] config.rs, tray (dans main.rs), autostart
```

### Deux écarts assumés par rapport à l'annexe A

L'annexe listait `Face` sous `world.rs` et un `character/mod.rs` limité à « état du
personnage ». Deux ajustements, chacun pour une raison mécanique :

1. **`Face` vit dans `geom.rs`**, pas dans `world.rs`. `Rect::point_on(face, offset)` en a
   besoin : laisser `Face` dans `world.rs` créerait un cycle `geom → world → geom`. `Face`
   désigne un côté de rectangle — c'est de la géométrie. `world.rs` le réexporte, donc les
   `use` restent lisibles.
2. **`character/mod.rs` porte aussi l'avance des frames d'animation** (quelle image
   afficher, depuis combien de temps). C'est de l'état du personnage, et l'isoler dans un
   cinquième fichier séparerait deux choses qui changent toujours ensemble.

Les trois fichiers déclarés **non négociables** par l'annexe — `clock.rs`, `rng.rs`,
`probe/mod.rs` — sont créés en **Tâche 1 et Tâche 2**, avant toute logique qui en dépend.

### Comment le webview obtient les images

Les personnages sont des **fichiers externes** au binaire (spec §8.1), donc le webview ne
peut pas les charger par un chemin relatif : rien ne les embarque.

**Retenu : un schéma URI custom servi par Rust.** `Builder::register_uri_scheme_protocol`
(vérifié dans `tauri-2.11.5/src/app.rs:2130`) fait de Rust le serveur des PNG. Le webview
écrit `<img src="http://shime.localhost/blob/1">` — sur Windows, Tauri sert les schémas
custom sous `http://<scheme>.localhost` (`manager/mod.rs:342`).

| Écarté | Pourquoi |
|---|---|
| protocole `asset` de Tauri | exige une *scope* de chemins absolus dans `tauri.conf.json`, alors que le dossier des personnages est résolu **à l'exécution** (à côté de l'exe, sinon `%APPDATA%`) |
| PNG encodés en base64 poussés par IPC | ajoute une dépendance base64 et ~500 Ko d'IPC au démarrage, pour éviter un appel qui existe déjà |

Bénéfice non cherché : le rechargement à chaud du plan 1b devient **gratuit** — les images
n'étant jamais mises en cache par un bundler, il suffit de changer le numéro de version
dans l'URL.

---

## Tâche 1 : Le projet, et les trois contraintes de testabilité

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/icons/icon.ico`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/geom.rs`
- Create: `src-tauri/src/clock.rs`
- Create: `src-tauri/src/rng.rs`
- Create: `.cargo/config.toml`

**Interfaces:**
- Consomme : rien. C'est la première tâche.
- Produit :
  - `geom::Point { x: f32, y: f32 }`, `geom::Vec2 { x: f32, y: f32 }`
  - `geom::Rect { x: f32, y: f32, w: f32, h: f32 }` avec
    `left()`, `right()`, `top()`, `bottom()`, `contains(Point) -> bool`,
    `face_length(Face) -> f32`, `point_on(Face, f32) -> Point`
  - `geom::Face` (`Top | Left | Right | Bottom`), `Copy`
  - `clock::Clock` (trait) avec `elapsed(&self) -> Duration` ;
    `clock::SystemClock::new()` ; `clock::FakeClock::new()` avec
    `advance(Duration)` et `set(Duration)`
  - `rng::Rng` (trait) avec `next_u32`, `unit_f32`, `range`, `weighted(&[f32]) -> Option<usize>` ;
    `rng::XorShift32::seeded(u32)`
- Ces trois modules ne dépendent de **rien** dans le projet : ce sont les feuilles de
  l'arbre de dépendances, et c'est pour ça qu'ils viennent en premier.

> **Pourquoi cette tâche mélange l'échafaudage Tauri et trois modules de logique.**
> Un projet Tauri qui ne compile pas ne permet pas de lancer `cargo test` — les scripts de
> build de Tauri sont des exécutables, qui doivent être liés pour s'exécuter (constaté à
> l'étape 0 : le linker manquant interdisait aussi `cargo check`). L'échafaudage n'est donc
> pas séparable du premier test. Il est replié dans la tâche dont le livrable en a besoin.

- [ ] **Step 1 : Créer l'arborescence**

```bash
mkdir -p src-tauri/src/probe src-tauri/src/character src-tauri/src/behavior src-tauri/icons ui .cargo
```

`probe/`, `character/` et `behavior/` sont créés maintenant même s'ils se remplissent plus
tard : ça évite d'y penser à chaque tâche.

- [ ] **Step 2 : Écrire `src-tauri/Cargo.toml`**

```toml
[package]
name = "shimeji-desktop"
version = "0.1.0"
edition = "2021"
publish = false

# Un SEUL binaire, délibérément. Le mode simulation (Tâche 9) est une
# sous-commande (`--sim`) et non une seconde cible : deux cibles binaires
# d'un même paquet ne partagent du code que par une cible `lib`, et ajouter
# un `lib.rs` pour cela seul réorganiserait tout le projet.

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }

# `derive` engendre le code de (dé)sérialisation à la compilation, à partir
# des annotations sur les structs — c'est ce qui évite d'écrire le parsing
# de mascot.json à la main (Tâche 4).
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Épinglé en 0.61 : c'est la version dont dépend Tauri 2.11.5, et deux versions
# majeures de cette crate donnent deux types `HWND` DISTINCTS. Prendre 0.62
# ferait que le HWND rendu par `window.hwnd()` ne serait pas celui qu'attend
# notre `SetWindowLongPtrW` (contrainte globale, résultat de l'étape 0).
#
# Les `features` sont à activer une par une : la crate `windows` couvre toute
# l'API Windows et ne compile que ce qu'on demande.
[dependencies.windows]
version = "0.61"
features = [
    "Win32_Foundation",                 # HWND, RECT, POINT, BOOL
    "Win32_Graphics_Gdi",               # EnumDisplayMonitors, MONITORINFO, HMONITOR
    "Win32_UI_WindowsAndMessaging",     # Get/SetWindowLongPtrW, GWL_EXSTYLE, GetCursorPos
    "Win32_UI_Input_KeyboardAndMouse",  # GetAsyncKeyState, VK_LBUTTON
    "Win32_UI_HiDpi",                   # GetDpiForMonitor, MDT_EFFECTIVE_DPI
]

# En release, on veut un exe autonome et petit.
[profile.release]
opt-level = "z"     # optimiser la taille : l'exe est distribué, pas un calculateur
lto = true          # optimisation inter-modules
codegen-units = 1   # laisse plus de latitude à LTO, au prix du temps de compilation
strip = true        # retire les symboles de debug
panic = "abort"     # pas de déroulement de pile : plus petit, et on n'attrape aucun panic
```

- [ ] **Step 3 : Écrire `.cargo/config.toml` — le runtime C++ lié statiquement**

Spec §4 : l'exe doit être autonome, la machine cible n'ayant à fournir que WebView2. Sans
ceci, il réclame le redistribuable Visual C++.

```toml
# Lie statiquement le runtime Visual C++ pour que l'exe soit totalement
# autonome (spec §4). Sans ça, la machine cible doit avoir le redistribuable
# VC++ installé — une dépendance d'exécution qu'on ne veut pas.
#
# Ce fichier est à la RACINE du dépôt, pas dans src-tauri/ : cargo remonte
# l'arborescence pour le trouver, et le mettre à la racine le rend valable
# pour tous les paquets qu'on ajouterait plus tard.
[target.x86_64-pc-windows-msvc]
rustflags = ["-C", "target-feature=+crt-static"]
```

- [ ] **Step 4 : Écrire `src-tauri/build.rs`**

```rust
// Déclenche la génération de code de Tauri (contexte, capacités, ressource
// Windows). Deux exigences découvertes à l'étape 0 : `icons/icon.ico` doit
// exister, et `frontendDist` est résolu relativement au tauri.conf.json.
fn main() {
    tauri_build::build()
}
```

- [ ] **Step 5 : Écrire `src-tauri/tauri.conf.json`**

`"windows": []` est délibéré : les fenêtres de personnages sont créées **par code**, une
par personnage, avec des attributs qui doivent tous être explicites (spec §3.2). Ici
`frontendDist` vaut `"../ui"` — la configuration est dans `src-tauri/`, donc le chemin
conventionnel est cette fois le bon (l'étape 0 avait dû écrire `"./ui"` faute d'une
arborescence en `src-tauri/`).

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "shimeji-desktop",
  "version": "0.1.0",
  "identifier": "dev.local.shimeji-desktop",
  "build": {
    "frontendDist": "../ui"
  },
  "app": {
    "windows": [],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "icon": ["icons/icon.ico"]
  }
}
```

- [ ] **Step 6 : Créer `src-tauri/icons/icon.ico`**

Obligatoire : son absence fait échouer `tauri-build` (constaté à l'étape 0). On réutilise
celui du spike, déjà généré depuis `shime1.png`.

```bash
cp docs/spike-etape-0/icons/icon.ico src-tauri/icons/icon.ico
```

- [ ] **Step 7 : Écrire `src-tauri/src/geom.rs`**

```rust
//! Géométrie du bureau virtuel : points, vecteurs, rectangles, faces.
//!
//! Responsabilité unique : les primitives spatiales, sans aucune notion de
//! personnage, de fenêtre ou d'écran. Tout est en PIXELS PHYSIQUES du bureau
//! virtuel (spec §3.4) — le facteur d'échelle d'un moniteur ne sert qu'au
//! dimensionnement du sprite, et n'entre jamais ici.

/// Un côté utilisable d'un rectangle.
///
/// Vit dans `geom` et non dans `world` parce que `Rect::point_on` en a besoin :
/// le placer dans `world` créerait un cycle geom → world → geom. C'est de la
/// géométrie — un côté de rectangle.
///
/// `Copy` : quatre variantes sans données, donc copier coûte moins que de
/// raisonner sur qui la possède. `PartialEq` pour les comparaisons de tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    /// Le dessus — on marche dessus. Le sol d'un écran, la barre de titre d'une fenêtre.
    Top,
    /// Le bord gauche — on s'y agrippe (étape 4).
    Left,
    /// Le bord droit — idem.
    Right,
    /// Le dessous — on s'y suspend (étape 4).
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Point { x, y }
    }

    /// Distance euclidienne. Sert à la proximité entre personnages (étape 3)
    /// et au choix du sol le plus proche quand un personnage tombe hors du
    /// bureau (Tâche 6).
    pub fn distance_to(&self, other: Point) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Un déplacement, ou une vitesse. Même forme qu'un `Point`, mais un type
/// distinct : additionner une position à une position n'a pas de sens, alors
/// qu'additionner un vecteur à une position en a. Le compilateur le fait
/// respecter gratuitement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    pub fn zero() -> Self {
        Vec2 { x: 0.0, y: 0.0 }
    }
}

/// Un rectangle, défini par son coin supérieur gauche et sa taille.
///
/// Convention d'axes de Windows : `y` croît **vers le bas**. `top()` est donc
/// la plus PETITE valeur de `y`, et `bottom()` la plus grande. C'est
/// contre-intuitif si l'on vient des mathématiques, et c'est la source d'erreur
/// de signe la plus courante dans ce genre de code — d'où ces accesseurs
/// nommés, plutôt que des comparaisons écrites à la main partout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }

    pub fn left(&self) -> f32 {
        self.x
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    /// Le plus petit `y` — voir la note sur la convention d'axes.
    pub fn top(&self) -> f32 {
        self.y
    }

    /// Le plus grand `y`.
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    /// Bornes incluses à gauche/en haut, exclues à droite/en bas. Cette
    /// asymétrie est volontaire : deux rectangles adjacents ne se recouvrent
    /// alors jamais sur leur frontière commune, et un point n'appartient donc
    /// jamais à deux écrans à la fois.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.left() && p.x < self.right() && p.y >= self.top() && p.y < self.bottom()
    }

    /// Longueur parcourable d'une face : horizontale pour `Top`/`Bottom`,
    /// verticale pour `Left`/`Right`.
    ///
    /// C'est l'unité de l'`offset` de la décision n° 1 : un personnage
    /// accroché stocke sa distance le long de cette longueur, et non une
    /// position absolue (spec §6.2).
    pub fn face_length(&self, face: Face) -> f32 {
        match face {
            Face::Top | Face::Bottom => self.w,
            Face::Left | Face::Right => self.h,
        }
    }

    /// Le point situé à `offset` le long de `face`, en partant du coin le plus
    /// « petit » de cette face (gauche pour les horizontales, haut pour les
    /// verticales).
    ///
    /// **C'est la fonction qui rend la décision n° 1 possible.** On repart du
    /// rectangle COURANT à chaque image : si la plateforme a bougé ou changé
    /// de taille, la position suit sans une ligne de code de plus (spec §6.2).
    ///
    /// L'offset n'est volontairement pas borné ici : un offset hors bornes est
    /// une information utile — c'est le signe que la plateforme a rétréci sous
    /// le personnage, et c'est `attach.rs` qui en déduit la chute (Tâche 5).
    pub fn point_on(&self, face: Face, offset: f32) -> Point {
        match face {
            Face::Top => Point::new(self.left() + offset, self.top()),
            Face::Bottom => Point::new(self.left() + offset, self.bottom()),
            Face::Left => Point::new(self.left(), self.top() + offset),
            Face::Right => Point::new(self.right(), self.top() + offset),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_on_top_part_du_bord_gauche() {
        let r = Rect::new(100.0, 50.0, 400.0, 200.0);
        let p = r.point_on(Face::Top, 30.0);
        assert_eq!(p, Point::new(130.0, 50.0));
    }

    #[test]
    fn point_on_right_descend_depuis_le_haut() {
        let r = Rect::new(100.0, 50.0, 400.0, 200.0);
        let p = r.point_on(Face::Right, 30.0);
        assert_eq!(p, Point::new(500.0, 80.0));
    }

    #[test]
    fn point_on_bottom_est_bien_en_bas() {
        // Vérifie la convention d'axes : bottom() > top().
        let r = Rect::new(0.0, 0.0, 100.0, 10.0);
        assert_eq!(r.point_on(Face::Bottom, 0.0), Point::new(0.0, 10.0));
    }

    #[test]
    fn face_length_distingue_horizontal_et_vertical() {
        let r = Rect::new(0.0, 0.0, 400.0, 200.0);
        assert_eq!(r.face_length(Face::Top), 400.0);
        assert_eq!(r.face_length(Face::Bottom), 400.0);
        assert_eq!(r.face_length(Face::Left), 200.0);
        assert_eq!(r.face_length(Face::Right), 200.0);
    }

    #[test]
    fn deux_rects_adjacents_ne_partagent_aucun_point() {
        // La borne droite exclusive garantit qu'un point n'est jamais dans
        // deux écrans à la fois — cas réel : x = 1920 sur cette machine.
        let gauche = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let droite = Rect::new(1920.0, 0.0, 1920.0, 1080.0);
        let frontiere = Point::new(1920.0, 500.0);
        assert!(!gauche.contains(frontiere));
        assert!(droite.contains(frontiere));
    }

    #[test]
    fn contains_accepte_les_coordonnees_negatives() {
        // Contrainte globale : ne jamais supposer x >= 0. Un écran branché à
        // gauche donne un rectangle à x négatif.
        let r = Rect::new(-1920.0, 0.0, 1920.0, 1080.0);
        assert!(r.contains(Point::new(-1000.0, 500.0)));
        assert!(!r.contains(Point::new(10.0, 500.0)));
    }

    #[test]
    fn point_on_accepte_un_offset_hors_bornes() {
        // Volontaire : c'est le signal d'une plateforme qui a rétréci, et
        // attach.rs en déduira la chute. Borner ici masquerait l'information.
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(r.point_on(Face::Top, 250.0), Point::new(250.0, 0.0));
    }
}
```

- [ ] **Step 8 : Lancer les tests de géométrie et les voir passer**

```powershell
cd src-tauri
cargo test geom
```

Attendu : `7 passed`. Le premier build est long (Tauri tire beaucoup de dépendances,
~2–3 min à froid).

**Si l'erreur mentionne `link.exe` et `extra operand`** : ce n'est pas une erreur MSVC mais
une formulation GNU coreutils — le `link.exe` de Git for Windows a été pêché à la place du
linker Visual Studio. Compiler depuis **PowerShell** et non depuis Git Bash.

- [ ] **Step 9 : Écrire `src-tauri/src/clock.rs`**

```rust
//! L'horloge, **injectée** — première des trois contraintes de la spec §10.2.
//!
//! Responsabilité unique : dire quel temps s'est écoulé. Rien dans le projet
//! n'appelle `Instant::now()` en dehors de `SystemClock` ci-dessous.
//!
//! Sans cette injection, rien de temporel n'est testable : le délai d'abandon
//! de 20 s (spec §7.3) demanderait un test de 20 secondes, et la durée
//! d'affichage d'une frame ne se vérifierait pas du tout.

use std::cell::Cell;
use std::time::{Duration, Instant};

/// Ce que le reste du programme sait du temps : une durée depuis le démarrage.
///
/// **Volontairement monotone et relative**, pas une date. L'heure du jour
/// (pour « il mange à midi ») est un SIGNAL, qui arrive à l'étape 2 ; elle
/// s'ajoutera comme une seconde méthode, sans rien changer d'ici.
///
/// `&self` et non `&mut self` : lire l'heure ne modifie rien de l'extérieur.
/// C'est ce qui permet de partager une horloge sans emprunt mutable.
pub trait Clock {
    fn elapsed(&self) -> Duration;
}

/// L'horloge réelle. Le **seul** endroit du projet où `Instant::now()` est
/// appelé — hormis le point de mesure de la boucle 60 Hz (Tâche 10).
pub struct SystemClock {
    start: Instant,
}

impl SystemClock {
    pub fn new() -> Self {
        SystemClock {
            start: Instant::now(),
        }
    }
}

// `Default` : demandé par clippy dès qu'un `new()` sans argument existe, et
// utile pour écrire `SystemClock::default()`.
impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }
}

/// L'horloge des tests : elle n'avance que quand on le lui demande.
///
/// `Cell<Duration>` donne la « mutabilité intérieure » : on modifie la valeur
/// à travers un `&self` (non mutable). C'est nécessaire parce que
/// `Clock::elapsed` prend `&self` — un test qui détiendrait un `&FakeClock`
/// ne pourrait sinon pas le faire avancer. `Cell` convient ici parce que
/// `Duration` est `Copy` et qu'on reste sur un seul thread.
pub struct FakeClock {
    now: Cell<Duration>,
}

impl FakeClock {
    pub fn new() -> Self {
        FakeClock {
            now: Cell::new(Duration::ZERO),
        }
    }

    /// Avance de `d`. C'est la façon normale de tester un délai.
    pub fn advance(&self, d: Duration) {
        self.now.set(self.now.get() + d);
    }

    /// Positionne le temps absolu. Utile pour aller droit au bord d'un délai.
    pub fn set(&self, d: Duration) {
        self.now.set(d);
    }
}

impl Default for FakeClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for FakeClock {
    fn elapsed(&self) -> Duration {
        self.now.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clock_demarre_a_zero() {
        let c = FakeClock::new();
        assert_eq!(c.elapsed(), Duration::ZERO);
    }

    #[test]
    fn fake_clock_avance_par_cumul() {
        let c = FakeClock::new();
        c.advance(Duration::from_millis(500));
        c.advance(Duration::from_millis(300));
        assert_eq!(c.elapsed(), Duration::from_millis(800));
    }

    #[test]
    fn fake_clock_avance_a_travers_une_reference_non_mutable() {
        // C'est tout l'intérêt du Cell : le trait expose `&self`, donc un
        // test qui ne détient qu'un `&dyn Clock` doit pouvoir faire avancer
        // le temps par ailleurs.
        let c = FakeClock::new();
        let vue: &dyn Clock = &c;
        c.advance(Duration::from_secs(20));
        assert_eq!(vue.elapsed(), Duration::from_secs(20));
    }

    #[test]
    fn fake_clock_peut_sauter_a_une_date() {
        let c = FakeClock::new();
        c.advance(Duration::from_secs(5));
        c.set(Duration::from_secs(1));
        assert_eq!(c.elapsed(), Duration::from_secs(1));
    }
}
```

- [ ] **Step 10 : Écrire `src-tauri/src/rng.rs`**

```rust
//! L'aléatoire, **injecté et graînable** — deuxième contrainte de la spec §10.2.
//!
//! Responsabilité unique : produire des nombres pseudo-aléatoires reproductibles.
//! Rien dans le projet n'appelle un générateur global.
//!
//! Sans cette injection, le tirage pondéré des envies (spec §7.2) n'est pas
//! testable : on ne peut pas vérifier « 10 000 tirages à graine fixe donnent
//! la distribution attendue » si la graine n'existe pas.
//!
//! **Générateur écrit à la main plutôt qu'une crate** : c'est 15 lignes, ça
//! supprime une dépendance, et surtout le déterminisme devient une propriété
//! qu'on peut lire dans ce fichier au lieu de la déduire de la documentation
//! d'un tiers. Un mode simulation (Tâche 9) qui rejoue exactement la même
//! séquence est à ce prix.
//!
//! ⚠️ Xorshift n'est **pas** cryptographique. C'est parfaitement indifférent
//! ici : on décide si un personnage s'assoit, pas si une clé est sûre.

/// Ce que le reste du programme sait de l'aléatoire.
///
/// `&mut self` — contrairement à `Clock` : tirer un nombre **fait avancer
/// l'état** du générateur. Le type dit donc la vérité, et le compilateur
/// interdit deux tirages simultanés depuis deux endroits, ce qui casserait
/// la reproductibilité.
pub trait Rng {
    /// Le tirage primitif. Tout le reste en découle.
    fn next_u32(&mut self) -> u32;

    /// Un flottant dans `[0, 1)`.
    ///
    /// On divise par `2^32` et non par `u32::MAX`, pour que 1.0 soit
    /// strictement exclu — un `1.0` inattendu ferait sortir d'un tableau
    /// dans les tirages par index.
    fn unit_f32(&mut self) -> f32 {
        self.next_u32() as f32 / 4_294_967_296.0
    }

    /// Un flottant dans `[min, max)`. Si `max <= min`, rend `min` : une plage
    /// vide n'est pas une erreur, c'est une valeur unique — ça évite d'avoir
    /// à valider les plages qui viennent de la config (plan 1b).
    fn range(&mut self, min: f32, max: f32) -> f32 {
        if max <= min {
            return min;
        }
        min + self.unit_f32() * (max - min)
    }

    /// Tirage pondéré : rend l'index choisi, la probabilité de chaque index
    /// étant proportionnelle à son poids.
    ///
    /// **C'est le cœur de la décision n° 3** : les signaux ne commandent pas,
    /// ils multiplient des poids que l'on passe ici (spec §7.2).
    ///
    /// Rend `None` si la liste est vide ou si tous les poids sont nuls —
    /// ce qui est le cas d'un personnage à couverture partielle dont aucune
    /// intention n'est jouable (spec §8.6). `None` n'est donc pas une erreur,
    /// c'est « rien à faire », et l'appelant le traite comme tel.
    ///
    /// Les poids négatifs sont ignorés (traités comme nuls) plutôt que
    /// rejetés : les poids viennent d'un fichier de config éditable à la main,
    /// et une coquille ne doit pas tuer le personnage.
    fn weighted(&mut self, weights: &[f32]) -> Option<usize> {
        let total: f32 = weights.iter().filter(|w| **w > 0.0).sum();
        if total <= 0.0 {
            return None;
        }

        // On tire un point sur [0, total) et on avance dans les poids jusqu'à
        // le dépasser : la « roue de loterie », où chaque poids occupe un arc
        // proportionnel à sa valeur.
        let mut cible = self.unit_f32() * total;
        for (i, w) in weights.iter().enumerate() {
            if *w <= 0.0 {
                continue;
            }
            cible -= *w;
            if cible < 0.0 {
                return Some(i);
            }
        }

        // Atteint seulement par accumulation d'erreurs d'arrondi sur les
        // flottants. On rend le dernier index de poids non nul : c'est le
        // choix le plus proche de l'intention, et ça évite un `unreachable!()`
        // qui ferait paniquer le programme pour une erreur de 1e-7.
        weights.iter().rposition(|w| *w > 0.0)
    }
}

/// Xorshift 32 bits. Trois décalages-xor, et c'est tout.
pub struct XorShift32 {
    state: u32,
}

impl XorShift32 {
    /// `seed` de 0 est remplacé par une constante : xorshift reste bloqué à
    /// zéro pour toujours si son état est nul. Le corriger silencieusement
    /// vaut mieux qu'un `panic!` — une graine de 0 est ce qu'on écrit
    /// spontanément dans un test.
    pub fn seeded(seed: u32) -> Self {
        XorShift32 {
            state: if seed == 0 { 0x9E37_79B9 } else { seed },
        }
    }
}

impl Rng for XorShift32 {
    fn next_u32(&mut self) -> u32 {
        // `^=` et `<<`/`>>` sur un u32 : aucun débordement possible, les bits
        // sortis sont perdus. Pas besoin de `wrapping_*` ici.
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meme_graine_meme_sequence() {
        // La propriété qui rend le mode simulation utile : rejouable.
        let mut a = XorShift32::seeded(42);
        let mut b = XorShift32::seeded(42);
        for _ in 0..100 {
            assert_eq!(a.next_u32(), b.next_u32());
        }
    }

    #[test]
    fn graines_differentes_sequences_differentes() {
        let mut a = XorShift32::seeded(1);
        let mut b = XorShift32::seeded(2);
        assert_ne!(a.next_u32(), b.next_u32());
    }

    #[test]
    fn graine_zero_ne_reste_pas_bloquee() {
        let mut r = XorShift32::seeded(0);
        let premier = r.next_u32();
        let second = r.next_u32();
        assert_ne!(premier, 0);
        assert_ne!(premier, second);
    }

    #[test]
    fn unit_f32_reste_dans_zero_un() {
        let mut r = XorShift32::seeded(7);
        for _ in 0..10_000 {
            let v = r.unit_f32();
            assert!((0.0..1.0).contains(&v), "valeur hors bornes : {v}");
        }
    }

    #[test]
    fn range_inverse_rend_le_minimum() {
        let mut r = XorShift32::seeded(7);
        assert_eq!(r.range(5.0, 5.0), 5.0);
        assert_eq!(r.range(9.0, 2.0), 9.0);
    }

    #[test]
    fn weighted_respecte_les_proportions() {
        // Le test que la spec §10.1 demande : 10 000 tirages à graine fixe,
        // distribution attendue. Poids 1 / 3 / 6 → environ 10 / 30 / 60 %.
        let mut r = XorShift32::seeded(12345);
        let poids = [1.0, 3.0, 6.0];
        let mut comptes = [0usize; 3];

        for _ in 0..10_000 {
            let i = r.weighted(&poids).expect("poids non nuls");
            comptes[i] += 1;
        }

        // Marge de 2 points de pourcentage : large devant l'erreur
        // d'échantillonnage sur 10 000 tirages, serrée devant une vraie
        // erreur de proportion.
        let pct = |n: usize| n as f32 / 100.0;
        assert!((pct(comptes[0]) - 10.0).abs() < 2.0, "{:?}", comptes);
        assert!((pct(comptes[1]) - 30.0).abs() < 2.0, "{:?}", comptes);
        assert!((pct(comptes[2]) - 60.0).abs() < 2.0, "{:?}", comptes);
    }

    #[test]
    fn weighted_ignore_les_poids_nuls_et_negatifs() {
        let mut r = XorShift32::seeded(99);
        // Seul l'index 2 est jouable : c'est le cas « couverture partielle ».
        let poids = [0.0, -5.0, 1.0];
        for _ in 0..1_000 {
            assert_eq!(r.weighted(&poids), Some(2));
        }
    }

    #[test]
    fn weighted_rend_none_quand_rien_nest_jouable() {
        let mut r = XorShift32::seeded(99);
        assert_eq!(r.weighted(&[]), None);
        assert_eq!(r.weighted(&[0.0, 0.0]), None);
    }
}
```

- [ ] **Step 11 : Écrire `src-tauri/src/main.rs` — minimal, mais qui déclare les modules**

À ce stade il ne crée aucune fenêtre : la Tâche 10 s'en charge. Son rôle ici est de
**déclarer les modules** pour que `cargo test` les compile.

```rust
// Amorçage de l'application. Pour l'instant : il ne fait que déclarer les
// modules, afin que `cargo test` les compile et exécute leurs tests.
// La fenêtre et la boucle 60 Hz arrivent en Tâche 10.
//
// Pas de `#![windows_subsystem = "windows"]` pour le moment : on VEUT la
// console pendant le développement (topologie des écrans, trace du
// comportement). Le plan 1b la supprimera, une fois le tray disponible pour
// quitter proprement.

mod clock;
mod geom;
mod rng;

fn main() {
    println!("shimeji-desktop — squelette. Fenêtre et boucle : Tâche 10.");
}
```

- [ ] **Step 12 : Lancer toute la suite et la voir passer**

```powershell
cd src-tauri
cargo test
```

Attendu : `19 passed` — 7 pour `geom`, 4 pour `clock`, 8 pour `rng`.

- [ ] **Step 13 : Vérifier que le binaire se construit et se lance**

```powershell
cd src-tauri
cargo run
```

Attendu : la ligne `shimeji-desktop — squelette…`, puis sortie immédiate. Aucune fenêtre :
c'est normal, `tauri.conf.json` n'en déclare aucune et le code n'en crée pas encore.

- [ ] **Step 14 : Ajouter les artefacts de build au `.gitignore`**

Le `.gitignore` a déjà `target/`, `src-tauri/target/` et `**/gen/schemas/`. Vérifier que
rien de généré n'est suivi :

```bash
git status --short
```

Attendu : uniquement les fichiers écrits dans cette tâche. **Si `src-tauri/gen/` apparaît**,
c'est que le motif ne l'attrape pas — le corriger avant de commiter.

- [ ] **Step 15 : Commit**

```bash
git add .cargo src-tauri ui .gitignore
git commit -m "feat(etape-1a): squelette Tauri et les trois contraintes de testabilité

geom.rs, clock.rs et rng.rs — les trois feuilles de l'arbre de dépendances,
donc écrites en premier. clock et rng sont deux des trois contraintes de la
spec §10.2 : elles ne se rattrapent pas après coup.

Face vit dans geom et non dans world : Rect::point_on en a besoin, et
l'inverse créerait un cycle geom → world → geom.

windows épinglé en 0.61, la version de Tauri 2.11.5 — sinon les HWND sont
deux types distincts (résultat de l'étape 0). crt-static pour que l'exe
soit autonome (spec §4)."
```

---

## Tâche 2 : La sonde système — la frontière testable

**Files:**
- Create: `src-tauri/src/probe/mod.rs`
- Create: `src-tauri/src/probe/fake.rs`
- Create: `src-tauri/src/probe/win32.rs`
- Modify: `src-tauri/src/main.rs` (ajouter `mod probe;`)

**Interfaces:**
- Consomme : `geom::{Point, Rect}`.
- Produit :
  - `probe::ScreenInfo { id: u64, work_area: Rect, scale: f32 }`
  - `probe::MouseState { pos: Point, left_down: bool }`
  - `probe::SystemProbe` (trait) avec `screens(&self) -> Vec<ScreenInfo>` et
    `mouse(&self) -> MouseState`
  - `probe::win32::Win32Probe::new()` et
    `probe::win32::imprimer_diagnostic(&dyn SystemProbe)`
  - `probe::fake::FakeProbe::new(Vec<ScreenInfo>)` avec `set_mouse(Point, bool)`,
    et les raccourcis `un_ecran()`, `deux_ecrans()`, `ecran_a_gauche_hidpi()`

> **Troisième contrainte de la spec §10.2**, et la plus structurante : sans cette
> interface, aucun test ne peut fournir de faux écrans, et toute la suite — monde,
> accroche, chute, comportement — devient dépendante de la machine qui la fait tourner.
>
> À l'étape 4, `SystemProbe` gagnera une méthode `windows()`. **Rien d'autre ne changera**,
> et c'est le but : le monde, la physique et le comportement ne sauront jamais si une
> plateforme vient d'un écran ou d'une fenêtre (spec §5.1).

- [ ] **Step 1 : Écrire `src-tauri/src/probe/mod.rs`**

```rust
//! La frontière entre le programme et Windows — troisième contrainte de la
//! spec §10.2.
//!
//! Responsabilité unique : décrire ce que le programme a besoin de savoir du
//! système, sans dire comment on l'apprend. Deux implémentations : `win32`
//! (la vraie) et `fake` (les tests).
//!
//! Tout ce qui sort d'ici est en PIXELS PHYSIQUES du bureau virtuel
//! (spec §3.4). `scale` est transporté pour dimensionner le sprite, et pour
//! rien d'autre — surtout pas pour convertir des coordonnées.

use crate::geom::{Point, Rect};

pub mod fake;
pub mod win32;

/// Un écran, tel que la physique a besoin de le connaître.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenInfo {
    /// Identité stable dans le temps, dérivée du `HMONITOR` (spec §5.2).
    /// C'est ce qui fait que « l'écran sur lequel je suis » survit à un
    /// changement de résolution.
    pub id: u64,

    /// **La zone de travail, pas l'écran** — elle exclut la barre des tâches.
    /// Utiliser le rectangle de l'écran ferait marcher le personnage SOUS la
    /// barre des tâches (piège Windows n° 3).
    pub work_area: Rect,

    /// Facteur d'échelle du moniteur (1.0 à 96 ppp, 1.5 à 144, 2.0 à 192).
    /// Sert **uniquement** au dimensionnement du sprite (spec §3.4).
    pub scale: f32,
}

/// L'état de la souris. Deux informations, et pas une de plus : on ne capture
/// aucune frappe (décision n° 8 du journal).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MouseState {
    pub pos: Point,
    pub left_down: bool,
}

/// Ce que le programme sait du système.
///
/// `&self` partout : interroger le système ne modifie rien côté programme.
///
/// À l'étape 4 s'ajoutera `fn windows(&self) -> Vec<WindowInfo>`. Le monde,
/// la physique et le comportement n'en sauront rien — ils ne manipulent que
/// des `Platform` (spec §5.1). C'est ce découplage qui permet de livrer le
/// sol maintenant et les fenêtres plus tard sans rien réécrire.
pub trait SystemProbe {
    /// Tous les écrans, dans un ordre non garanti. Peut être **vide** si aucun
    /// moniteur n'est rapporté (session distante en cours d'établissement) —
    /// l'appelant doit traiter ce cas, pas paniquer.
    fn screens(&self) -> Vec<ScreenInfo>;

    fn mouse(&self) -> MouseState;
}
```

- [ ] **Step 2 : Écrire `src-tauri/src/probe/fake.rs`, tests inclus**

C'est la sonde qui pilotera tous les tests des tâches suivantes. Ses trois constructeurs
de commodité sont ce qui rendra ces tests lisibles.

```rust
//! Sonde de test : des écrans et une souris que le test décide.
//!
//! Existe pour satisfaire la troisième contrainte de la spec §10.2. Les
//! constructeurs de commodité reproduisent des topologies nommées, ce qui
//! rend les attentes des tests lisibles sans commentaire.

use super::{MouseState, ScreenInfo, SystemProbe};
use crate::geom::{Point, Rect};
use std::cell::Cell;

pub struct FakeProbe {
    screens: Vec<ScreenInfo>,
    // `Cell` pour la même raison que dans `FakeClock` : le trait expose
    // `&self`, donc un test qui n'a qu'une référence partagée doit pouvoir
    // bouger la souris. `MouseState` est `Copy`, ce que `Cell` exige.
    mouse: Cell<MouseState>,
}

impl FakeProbe {
    pub fn new(screens: Vec<ScreenInfo>) -> Self {
        FakeProbe {
            screens,
            mouse: Cell::new(MouseState {
                pos: Point::new(0.0, 0.0),
                left_down: false,
            }),
        }
    }

    /// Un seul écran 1920×1080 dont la zone de travail exclut 48 px de barre
    /// des tâches. Le sol est donc à y = 1032, et non 1080 : c'est le piège
    /// Windows n° 3, rendu explicite dans les tests.
    pub fn un_ecran() -> Self {
        Self::new(vec![ScreenInfo {
            id: 1,
            work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
            scale: 1.0,
        }])
    }

    /// La topologie réelle relevée à l'étape 0 : deux 1920×1080 côte à côte,
    /// échelle 1, le second à x = 1920.
    pub fn deux_ecrans() -> Self {
        Self::new(vec![
            ScreenInfo {
                id: 1,
                work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
                scale: 1.0,
            },
            ScreenInfo {
                id: 2,
                work_area: Rect::new(1920.0, 0.0, 1920.0, 1032.0),
                scale: 1.0,
            },
        ])
    }

    /// Deux écrans dont le second est **à gauche**, donc à `x` négatif, et à
    /// l'**échelle 2**.
    ///
    /// C'est la topologie que la machine de développement ne peut pas
    /// produire : elle n'a que des écrans à l'échelle 1, rangés vers la
    /// droite. Le multi-DPI est la seule inconnue laissée ouverte par
    /// l'étape 0 — on ne peut pas l'observer, mais ce constructeur permet au
    /// moins de ne pas coder contre elle.
    pub fn ecran_a_gauche_hidpi() -> Self {
        Self::new(vec![
            ScreenInfo {
                id: 1,
                work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
                scale: 1.0,
            },
            ScreenInfo {
                id: 2,
                work_area: Rect::new(-2560.0, 0.0, 2560.0, 1392.0),
                scale: 2.0,
            },
        ])
    }

    pub fn set_mouse(&self, pos: Point, left_down: bool) {
        self.mouse.set(MouseState { pos, left_down });
    }
}

impl SystemProbe for FakeProbe {
    fn screens(&self) -> Vec<ScreenInfo> {
        self.screens.clone()
    }

    fn mouse(&self) -> MouseState {
        self.mouse.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deux_ecrans_reproduit_la_topologie_de_l_etape_0() {
        let p = FakeProbe::deux_ecrans();
        let e = p.screens();
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].work_area.left(), 0.0);
        assert_eq!(e[1].work_area.left(), 1920.0);
        // Le sol est la zone de travail, pas l'écran : 1032 et non 1080.
        assert_eq!(e[0].work_area.bottom(), 1032.0);
    }

    #[test]
    fn la_souris_se_deplace_a_travers_une_reference_partagee() {
        let p = FakeProbe::un_ecran();
        let vue: &dyn SystemProbe = &p;
        p.set_mouse(Point::new(300.0, 400.0), true);
        let m = vue.mouse();
        assert_eq!(m.pos, Point::new(300.0, 400.0));
        assert!(m.left_down);
    }

    #[test]
    fn une_topologie_a_x_negatif_est_representable() {
        // Contrainte globale : ne jamais supposer x >= 0.
        let p = FakeProbe::ecran_a_gauche_hidpi();
        let e = p.screens();
        assert!(e[1].work_area.left() < 0.0);
        assert_eq!(e[1].scale, 2.0);
    }
}
```

- [ ] **Step 3 : Écrire `src-tauri/src/probe/win32.rs`**

```rust
//! L'implémentation réelle de `SystemProbe`, par la crate `windows`.
//!
//! Responsabilité unique : traduire trois appels Win32 en types du projet.
//! **Aucune logique** ici — pas de filtrage, pas de décision. Tout ce qui
//! ressemble à une règle appartient à `world.rs` ou au comportement, où c'est
//! testable avec `FakeProbe`.
//!
//! Signatures vérifiées dans les sources de windows 0.61.3 :
//!   EnumDisplayMonitors  Win32/Graphics/Gdi/mod.rs:559
//!   GetMonitorInfoW      Win32/Graphics/Gdi/mod.rs:1102
//!   GetDpiForMonitor     Win32/UI/HiDpi/mod.rs:39
//!   GetCursorPos         Win32/UI/WindowsAndMessaging/mod.rs:825
//!   GetAsyncKeyState     Win32/UI/Input/KeyboardAndMouse/mod.rs:28

use super::{MouseState, ScreenInfo, SystemProbe};
use crate::geom::{Point, Rect};

// `BOOL` ne vit PAS dans `Win32::Foundation` : c'est un type de
// `windows-result`, réexporté par `windows::core`. Le chercher dans
// Foundation avec les autres types win32 est l'erreur naturelle, et elle
// donne un `unresolved import` qui ne dit pas où regarder.
// `BOOL(pub i32)`, avec une méthode `.as_bool()`.
use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// Déclare le processus **conscient du DPI par moniteur (v2)**.
///
/// ⚠️ **À appeler tout au début de `main`, avant absolument tout le reste.**
///
/// Sans cet appel, Windows considère le processus comme « non conscient du
/// DPI » et lui **ment** : sur un écran à 125 %, `GetMonitorInfoW` rend
/// 1536×816 au lieu de 1920×1020, et `GetDpiForMonitor` rend 96 ppp au lieu
/// de 120. On travaillerait alors en pixels *logiques* virtualisés en croyant
/// être en pixels physiques — exactement ce que la spec §3.4 interdit, et
/// qu'elle annonce comme « un enfer à diagnostiquer ».
///
/// Deux raisons de le faire nous-mêmes plutôt que de laisser Tauri s'en
/// charger :
///
/// 1. **Tauri ne le fait qu'à la création de sa boucle d'événements.** Notre
///    sonde est utilisée avant (diagnostic de démarrage) et après (boucle
///    60 Hz). Sans cet appel, les deux ne verraient pas le même bureau, ce
///    qui est la pire forme du bug : reproductible seulement à moitié.
/// 2. Un appel explicite se lit et se vérifie ; une dépendance à l'ordre
///    d'initialisation d'une bibliothèque, non.
///
/// L'appel échoue si la conscience DPI est **déjà** fixée (Tauri est passé
/// avant, ou un manifeste la déclare). C'est sans conséquence : on voulait
/// justement ce réglage. D'où le `let _ =`.
pub fn activer_conscience_dpi() {
    use windows::Win32::UI::HiDpi::{
        SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    };

    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

pub struct Win32Probe;

impl Win32Probe {
    pub fn new() -> Self {
        Win32Probe
    }
}

// `Default` : clippy le réclame dès qu'un `new()` sans argument existe.
impl Default for Win32Probe {
    fn default() -> Self {
        Self::new()
    }
}

/// Convertit un `RECT` Win32 (bornes gauche/haut/droite/bas) en `Rect` du
/// projet (coin + taille). Les deux formes décrivent la même chose ; confondre
/// « bas » et « hauteur » est l'erreur classique, isolée ici une fois pour
/// toutes.
fn rect_depuis_win32(r: RECT) -> Rect {
    Rect::new(
        r.left as f32,
        r.top as f32,
        (r.right - r.left) as f32,
        (r.bottom - r.top) as f32,
    )
}

/// Le rappel appelé par Windows une fois par moniteur.
///
/// `unsafe extern "system"` : c'est Windows qui l'appelle, avec la convention
/// d'appel de l'OS. Rust ne peut rien vérifier de ce côté de la frontière.
///
/// `lparam` transporte un pointeur vers notre `Vec` — c'est le mécanisme
/// habituel de win32 pour passer un contexte à un rappel, faute de fermetures
/// en C. On le remet en `&mut Vec` ci-dessous, et c'est la seule ligne
/// réellement délicate de ce fichier.
unsafe extern "system" fn collecte_moniteur(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _clip: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    // `cbSize` doit être renseigné AVANT l'appel : c'est ainsi que Windows
    // sait quelle version de la structure on lui passe. L'oublier fait
    // échouer `GetMonitorInfoW` sans autre explication.
    //
    // `..Default::default()` remplit les champs restants (les deux RECT et
    // dwFlags) — MONITORINFO dérive `Default` dans windows 0.61.3.
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
        // L'échelle. `GetDpiForMonitor` peut échouer (moniteur en cours de
        // débranchement) : on garde alors 96 ppp, soit l'échelle 1. Un
        // personnage à la mauvaise taille vaut mieux qu'un plantage.
        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        let _ = GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);

        // `&mut *(...)` : on transforme l'entier du LPARAM en pointeur, puis
        // on le déréférence. Sûr ici parce que `screens()` garantit que le
        // Vec est vivant pendant toute l'énumération — `EnumDisplayMonitors`
        // est synchrone.
        let ecrans = &mut *(lparam.0 as *mut Vec<ScreenInfo>);

        ecrans.push(ScreenInfo {
            // `HMONITOR` est un pointeur opaque ; on ne s'en sert que comme
            // identité, jamais pour déréférencer. Double conversion parce que
            // le champ est un `*mut c_void`.
            id: hmonitor.0 as usize as u64,

            // `rcWork` et non `rcMonitor` : la zone de travail exclut la
            // barre des tâches (piège Windows n° 3).
            work_area: rect_depuis_win32(info.rcWork),

            scale: dpi_x as f32 / 96.0,
        });
    }

    // TRUE = continuer l'énumération. Rendre FALSE l'arrêterait au premier
    // moniteur, et le second écran n'existerait jamais pour le programme.
    BOOL(1)
}

impl SystemProbe for Win32Probe {
    fn screens(&self) -> Vec<ScreenInfo> {
        let mut ecrans: Vec<ScreenInfo> = Vec::new();

        // SÉCURITÉ : `ecrans` vit jusqu'à la fin de la fonction, et
        // `EnumDisplayMonitors` est synchrone — le rappel a donc fini de s'en
        // servir quand l'appel rend la main. Aucun pointeur ne lui survit.
        unsafe {
            let _ = EnumDisplayMonitors(
                None, // tout le bureau virtuel
                None, // aucun rectangle de découpe
                Some(collecte_moniteur),
                LPARAM(&mut ecrans as *mut Vec<ScreenInfo> as isize),
            );
        }

        ecrans
    }

    fn mouse(&self) -> MouseState {
        let mut p = POINT { x: 0, y: 0 };

        // `GetCursorPos` rend un `Result` dans ce binding. En cas d'échec
        // (bureau sécurisé, session verrouillée), on garde (0, 0) : le
        // personnage croira la souris dans un coin, ce qui est inoffensif.
        unsafe {
            let _ = GetCursorPos(&mut p);
        }

        // `GetAsyncKeyState` : le bit de POIDS FORT dit « enfoncé
        // maintenant ». Le bit de poids faible dirait « pressé depuis le
        // dernier appel », qu'on ne veut surtout pas — il se consomme à la
        // lecture, donc deux appels dans la même image se voleraient
        // l'information.
        let etat = unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) };
        let left_down = (etat as u16 & 0x8000) != 0;

        MouseState {
            pos: Point::new(p.x as f32, p.y as f32),
            left_down,
        }
    }
}

/// Imprime ce que la sonde voit. Diagnostic de développement, l'équivalent de
/// la topologie que le spike de l'étape 0 imprimait.
///
/// Prend `&dyn SystemProbe` et non `&Win32Probe` : on peut ainsi l'appeler
/// sur la sonde factice, ce qui est pratique en mode simulation (Tâche 9).
pub fn imprimer_diagnostic(sonde: &dyn SystemProbe) {
    let ecrans = sonde.screens();
    if ecrans.is_empty() {
        println!("AUCUN écran rapporté — le monde sera vide.");
        return;
    }
    for e in &ecrans {
        println!(
            "écran {:#x} : zone de travail x={} y={} l={} h={} échelle={}",
            e.id, e.work_area.x, e.work_area.y, e.work_area.w, e.work_area.h, e.scale
        );
    }
    let m = sonde.mouse();
    println!(
        "souris : ({}, {}) bouton gauche={}",
        m.pos.x, m.pos.y, m.left_down
    );
}

// Pas de `#[cfg(test)] mod tests` dans ce fichier, et c'est volontaire : il ne
// contient aucune logique à vérifier, seulement des appels au système. Un test
// n'y affirmerait que le bon fonctionnement de Windows. C'est précisément la
// raison d'être du trait — tout le testable est ailleurs. Ce fichier se
// vérifie à l'œil, une fois, au Step 5.
```

- [ ] **Step 4 : Déclarer le module et compiler**

Dans `main.rs`, ajouter `mod probe;` — les déclarations restent en ordre alphabétique :

```rust
mod clock;
mod geom;
mod probe;
mod rng;
```

```powershell
cd src-tauri
cargo test probe
```

Attendu : `3 passed`, et **aucun avertissement**. Si un import est signalé inutilisé dans
`win32.rs`, le retirer plutôt que de l'annoter.

- [ ] **Step 5 : Vérifier la sonde réelle à l'œil, une fois**

Remplacer le corps de `main` :

```rust
fn main() {
    // AVANT TOUT LE RESTE. Sans cet appel, Windows virtualise les
    // coordonnées et la sonde rendrait des pixels logiques en croyant rendre
    // des pixels physiques (spec §3.4). Voir le commentaire de la fonction.
    probe::win32::activer_conscience_dpi();

    let sonde = probe::win32::Win32Probe::new();
    probe::win32::imprimer_diagnostic(&sonde);
    println!("Monde, personnage et fenêtre : Tâches 3 à 11.");
}
```

```powershell
cd src-tauri
cargo run
```

Attendu : une ligne par écran **actif**, en pixels physiques. Relevé réel du
2026-09-08 sur cette machine, un seul écran étant alors allumé :

```
écran 0x6008c : zone de travail x=0 y=0 l=1920 h=1020 échelle=1.25
souris : (1205, 500) bouton gauche=false
```

**Quatre contrôles, chacun correspondant à un piège connu :**

| À vérifier | Pourquoi | Si c'est faux |
|---|---|---|
| **la largeur est celle du panneau** (1920), pas une valeur divisée | sans `activer_conscience_dpi`, Windows virtualise : on obtient 1536 sur un écran à 125 % | l'appel manque, ou n'est pas la **première** instruction de `main` |
| **`échelle` vaut le vrai facteur** (1.25 ici), pas 1 | même cause : `GetDpiForMonitor` rend 96 ppp à un processus non conscient | idem |
| `h` est inférieure à la hauteur du panneau | c'est `rcWork`, pas `rcMonitor` (piège n° 3). Ici 1020 = 1080 − 60, la barre des tâches faisant 48 px logiques × 1,25 | on a lu `rcMonitor` — corriger le champ |
| **autant de lignes que d'écrans allumés** | l'énumération traverse tous les moniteurs | le rappel rend `FALSE` trop tôt, ou `cbSize` n'est pas renseigné. **Vérifier d'abord combien d'écrans sont réellement actifs** — un écran éteint n'est pas énuméré, et ce n'est pas un bug |

> ℹ️ **Contrôle croisé indépendant**, si le compte d'écrans surprend :
>
> ```powershell
> Add-Type -AssemblyName System.Windows.Forms
> [System.Windows.Forms.Screen]::AllScreens | ForEach-Object { "$($_.DeviceName) $($_.Bounds) $($_.WorkingArea)" }
> ```
>
> PowerShell n'étant pas conscient du DPI, il rend des pixels **logiques** — la
> comparaison des deux sorties est d'ailleurs la façon la plus directe de voir la
> virtualisation à l'œuvre.

Bouger la souris et relancer doit changer les coordonnées ; maintenir le bouton gauche
pendant le lancement doit donner `bouton gauche=true`.

- [ ] **Step 6 : Commit**

```bash
git add src-tauri/src/probe src-tauri/src/main.rs
git commit -m "feat(etape-1a): SystemProbe, la troisième contrainte de testabilité

Le trait, l'implémentation win32 et la sonde factice. Dernière des trois
contraintes de la spec §10.2 : sans elle, aucun test ne peut fournir de faux
écrans, et tout le reste dépendrait de la machine qui l'exécute.

rcWork et non rcMonitor : le sol est la zone de travail, sinon le personnage
marche sous la barre des tâches (piège Windows n° 3).

FakeProbe::ecran_a_gauche_hidpi() existe pour éprouver les deux hypothèses
que cette machine ne peut pas produire — x négatif et échelle 2. Le multi-DPI
est la seule inconnue laissée ouverte par l'étape 0 : on ne peut pas
l'observer, on peut au moins ne pas coder contre elle.

win32.rs n'a volontairement pas de tests, et c'est le but du trait : sans
logique dedans, un test n'y affirmerait que le bon fonctionnement de Windows."
```

---

## Tâche 3 : Le monde — une plateforme par écran

**Files:**
- Create: `src-tauri/src/world.rs`
- Modify: `src-tauri/src/main.rs` (ajouter `mod world;`)

**Interfaces:**
- Consomme : `geom::{Face, Point, Rect}`, `probe::ScreenInfo`.
- Produit :
  - `world::PlatformId(pub u64)` — `Copy`, `PartialEq`, `Eq`, `Hash`
  - `world::PlatformKind { Screen, Window }`
  - `world::Platform { id, rect, kind, z, faces: Vec<Face> }` avec `has_face(Face) -> bool`
  - `world::World` avec `from_screens(&[ScreenInfo]) -> World`,
    `get(PlatformId) -> Option<&Platform>`, `platforms() -> &[Platform]`,
    `nearest_floor(Point) -> Option<(PlatformId, f32)>`, `bounds() -> Option<Rect>`
  - `world::Face` — réexport de `geom::Face`, pour que `use crate::world::Face` marche

> **Le fichier qui encaissera le dividende du §5.1.** À l'étape 1, `from_screens` est la
> seule source de plateformes. À l'étape 4 s'ajoutera une seconde source, et **aucun
> appelant ne changera** : la physique et le comportement ne consomment que `Platform`.

- [ ] **Step 1 : Écrire le test d'abord**

Créer `src-tauri/src/world.rs` avec **seulement** ce bloc de tests. Il ne compile pas —
c'est le point de départ voulu : les tests décrivent une interface qui n'existe pas encore.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::fake::FakeProbe;
    use crate::probe::{ScreenInfo, SystemProbe};

    #[test]
    fn un_ecran_donne_une_plateforme_de_sol() {
        let monde = World::from_screens(&FakeProbe::un_ecran().screens());
        assert_eq!(monde.platforms().len(), 1);

        let p = &monde.platforms()[0];
        assert_eq!(p.kind, PlatformKind::Screen);
        // À l'étape 1, SEULE la face Top est exposée : le sol. Murs et
        // plafond arrivent à l'étape 4 (spec §11).
        assert!(p.has_face(Face::Top));
        assert!(!p.has_face(Face::Left));
        assert!(!p.has_face(Face::Bottom));
    }

    #[test]
    fn le_sol_est_en_bas_de_la_zone_de_travail() {
        // Le point le plus important de cette tâche. Le rectangle du sol doit
        // avoir son bord SUPÉRIEUR à hauteur du bas de la zone de travail :
        // c'est là qu'on marche.
        let monde = World::from_screens(&FakeProbe::un_ecran().screens());
        let p = &monde.platforms()[0];
        assert_eq!(p.rect.top(), 1032.0);
        assert_eq!(p.rect.left(), 0.0);
        assert_eq!(p.rect.w, 1920.0);
    }

    #[test]
    fn deux_ecrans_donnent_deux_plateformes_distinctes() {
        let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());
        assert_eq!(monde.platforms().len(), 2);
        assert_ne!(monde.platforms()[0].id, monde.platforms()[1].id);
    }

    #[test]
    fn l_identite_survit_a_un_changement_de_resolution() {
        // Spec §5.2 : PlatformId dérive de l'identité de l'écran, pas de sa
        // géométrie. C'est la condition de la décision n° 1.
        let avant = vec![ScreenInfo {
            id: 77,
            work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
            scale: 1.0,
        }];
        let apres = vec![ScreenInfo {
            id: 77,
            work_area: Rect::new(0.0, 0.0, 1280.0, 672.0),
            scale: 1.0,
        }];

        let id_avant = World::from_screens(&avant).platforms()[0].id;
        let id_apres = World::from_screens(&apres).platforms()[0].id;
        assert_eq!(id_avant, id_apres);
    }

    #[test]
    fn get_retrouve_une_plateforme_et_rend_none_sinon() {
        let monde = World::from_screens(&FakeProbe::un_ecran().screens());
        let id = monde.platforms()[0].id;
        assert!(monde.get(id).is_some());
        assert!(monde.get(PlatformId(999_999)).is_none());
    }

    #[test]
    fn nearest_floor_choisit_l_ecran_sous_le_point() {
        let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());

        // Un point au-dessus de l'écran de droite doit retenir SON sol, et
        // l'offset doit être la distance depuis le bord gauche de ce sol.
        let (id, offset) = monde
            .nearest_floor(Point::new(2000.0, 300.0))
            .expect("un sol existe");

        assert_eq!(monde.get(id).unwrap().rect.left(), 1920.0);
        assert_eq!(offset, 80.0);
    }

    #[test]
    fn nearest_floor_rabat_un_point_hors_bureau_sur_le_sol_le_plus_proche() {
        // Le garde-fou de la spec §6.3 : un personnage lâché hors écran ne
        // doit jamais être perdu.
        let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());
        let (id, offset) = monde
            .nearest_floor(Point::new(99_999.0, 500.0))
            .expect("un sol existe");

        let p = monde.get(id).unwrap();
        assert_eq!(p.rect.left(), 1920.0);
        // Rabattu dans les bornes de la face, pas laissé à 98 079.
        assert!(offset >= 0.0 && offset <= p.rect.face_length(Face::Top));
    }

    #[test]
    fn nearest_floor_fonctionne_avec_un_ecran_a_x_negatif() {
        let monde = World::from_screens(&FakeProbe::ecran_a_gauche_hidpi().screens());
        let (id, _) = monde
            .nearest_floor(Point::new(-1000.0, 200.0))
            .expect("un sol existe");
        assert!(monde.get(id).unwrap().rect.left() < 0.0);
    }

    #[test]
    fn un_monde_sans_ecran_est_vide_mais_pas_une_erreur() {
        // `screens()` peut rendre une liste vide (session distante en cours
        // d'établissement). Ça ne doit pas paniquer.
        let monde = World::from_screens(&[]);
        assert!(monde.platforms().is_empty());
        assert_eq!(monde.nearest_floor(Point::new(0.0, 0.0)), None);
        assert_eq!(monde.bounds(), None);
    }

    #[test]
    fn bounds_englobe_tous_les_ecrans() {
        let monde = World::from_screens(&FakeProbe::deux_ecrans().screens());
        let b = monde.bounds().expect("deux écrans");
        assert_eq!(b.left(), 0.0);
        assert_eq!(b.right(), 3840.0);
    }
}
```

- [ ] **Step 2 : Lancer les tests et les voir échouer**

Ajouter `mod world;` dans `main.rs`, puis :

```powershell
cd src-tauri
cargo test world
```

Attendu : **échec de compilation** — `cannot find type World in this scope`, idem pour
`Platform`, `PlatformId`, `PlatformKind`, `Face`, `Rect`, `Point`.

- [ ] **Step 3 : Écrire l'implémentation, au-dessus du bloc de tests**

```rust
//! Le monde : la liste de tout ce sur quoi un personnage peut se tenir.
//!
//! Responsabilité unique : transformer ce que la sonde rapporte en
//! plateformes, et permettre de retrouver l'une d'elles par son identité.
//!
//! **Aucune notion de personnage ici.** C'est la clé du §5.1 : la physique et
//! le comportement ne manipulent que des `Platform`, et ne savent donc jamais
//! si celle sous leurs pieds est le sol d'un écran ou la barre de titre de
//! VSCode. C'est ce qui permet de livrer le sol maintenant (étape 1) et de
//! brancher les fenêtres plus tard (étape 4) sans réécrire une ligne.

use crate::geom::{Point, Rect};
use crate::probe::ScreenInfo;

// Réexport sous son vrai nom : les appelants écrivent `use crate::world::Face`
// sans avoir à savoir que le type vit dans `geom` pour éviter un cycle de
// dépendances (voir « écarts assumés » en tête de plan). `Face` est donc dans
// la portée de ce fichier par ce réexport, et ne figure PAS dans le `use
// crate::geom::{…}` ci-dessus — l'y mettre serait un conflit de noms.
pub use crate::geom::Face;

/// Identité stable d'une plateforme (spec §5.2).
///
/// Dérivée du `HMONITOR` pour un écran, et du `HWND` pour une fenêtre à
/// l'étape 4. **Elle ne dépend pas de la géométrie** : « la plateforme sur
/// laquelle je suis » survit donc au déplacement, au redimensionnement et au
/// changement de résolution. C'est la condition qui rend la décision n° 1
/// possible.
///
/// `Hash` et `Eq` pour servir de clé de table à l'étape 4, où l'on suivra la
/// seule plateforme occupée à 60 Hz (spec §5.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlatformId(pub u64);

/// D'où vient la plateforme. Le comportement n'a pas à le consulter — c'est
/// là pour le diagnostic et le mode simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformKind {
    Screen,
    Window,
}

/// Un rectangle, plus les faces réellement utilisables.
///
/// `faces` est une liste et non quatre booléens : à l'étape 4, un bord
/// partiellement recouvert disparaîtra de cette liste (décision n° 2), et une
/// liste rend l'absence naturelle à exprimer.
#[derive(Debug, Clone, PartialEq)]
pub struct Platform {
    pub id: PlatformId,
    pub rect: Rect,
    pub kind: PlatformKind,
    /// 0 = au-dessus de tout. Sert à l'occlusion de l'étape 4 ; à l'étape 1,
    /// tous les sols sont au même rang.
    pub z: u32,
    pub faces: Vec<Face>,
}

impl Platform {
    /// Recherche linéaire sur un Vec de quatre éléments au maximum : plus
    /// rapide qu'un ensemble, et plus lisible.
    pub fn has_face(&self, face: Face) -> bool {
        self.faces.contains(&face)
    }
}

/// Tout ce sur quoi on peut se tenir, à un instant donné.
///
/// Reconstruit à ~8 Hz (spec §5.5). Le personnage n'en garde qu'un
/// `PlatformId`, jamais une référence — c'est pour ça qu'on peut reconstruire
/// le monde entier sans rien invalider. En Rust, ce détail-là n'est pas un
/// détail : garder une `&Platform` dans le personnage obligerait à annoter des
/// durées de vie partout, et interdirait de reconstruire le monde.
#[derive(Debug, Clone, Default)]
pub struct World {
    platforms: Vec<Platform>,
}

/// Hauteur donnée au rectangle d'une plateforme de sol.
///
/// Le sol n'a pas d'épaisseur réelle, mais un `Rect` en demande une, et une
/// hauteur nulle rendrait `contains` toujours faux. Un pixel suffit : seule la
/// face `Top` est exposée, donc cette hauteur n'est jamais parcourue.
const EPAISSEUR_DU_SOL: f32 = 1.0;

impl World {
    /// Construit le monde de l'étape 1 : **un sol par écran, et rien d'autre.**
    ///
    /// La spec §5.1 décrit qu'un écran offre aussi deux murs et un plafond.
    /// L'étape 1 ne les expose délibérément pas (spec §11) : sans images
    /// d'escalade branchées ni comportement de grimpe, un mur exposé serait
    /// une plateforme sur laquelle le personnage pourrait s'accrocher sans
    /// savoir en redescendre. C'est une ligne à ajouter à l'étape 4, pas une
    /// omission à rattraper.
    pub fn from_screens(screens: &[ScreenInfo]) -> World {
        let mut platforms = Vec::with_capacity(screens.len());

        for s in screens {
            // Le sol : un rectangle posé au BAS de la zone de travail. Sa
            // face Top est donc à `work_area.bottom()` — la ligne sur
            // laquelle le personnage marche, juste au-dessus de la barre des
            // tâches (piège Windows n° 3).
            platforms.push(Platform {
                id: PlatformId(s.id),
                rect: Rect::new(
                    s.work_area.left(),
                    s.work_area.bottom(),
                    s.work_area.w,
                    EPAISSEUR_DU_SOL,
                ),
                kind: PlatformKind::Screen,
                z: 0,
                faces: vec![Face::Top],
            });
        }

        World { platforms }
    }

    pub fn platforms(&self) -> &[Platform] {
        &self.platforms
    }

    /// Retrouve une plateforme par son identité.
    ///
    /// Rend `Option` et ne panique pas : « la plateforme a disparu » est un
    /// cas NORMAL et fréquent — c'est même l'événement central de la décision
    /// n° 1 (fenêtre fermée → le personnage tombe). L'appelant doit le gérer,
    /// et le type le lui rappelle à chaque usage.
    pub fn get(&self, id: PlatformId) -> Option<&Platform> {
        self.platforms.iter().find(|p| p.id == id)
    }

    /// Le sol le plus proche d'un point, et l'offset correspondant le long de
    /// sa face `Top`.
    ///
    /// Deux usages :
    ///   · placer un personnage au démarrage ;
    ///   · le **garde-fou** de la spec §6.3 — un personnage tombé sous le bas
    ///     du bureau virtuel est replacé sur le sol le plus proche, et n'est
    ///     donc jamais perdu définitivement.
    ///
    /// L'offset est **rabattu dans les bornes de la face** : le but est de
    /// produire une position tenable, pas de reporter le problème.
    pub fn nearest_floor(&self, p: Point) -> Option<(PlatformId, f32)> {
        // (id, offset, distance) — la distance ne sert qu'à comparer, et on
        // la laisse tomber à la fin.
        let mut meilleur: Option<(PlatformId, f32, f32)> = None;

        for plat in &self.platforms {
            if !plat.has_face(Face::Top) {
                continue;
            }

            // Le point de la face le plus proche horizontalement : on rabat
            // `p.x` entre les deux bords. `clamp` panique si min > max, ce
            // qui ne peut pas arriver ici — `from_screens` ne construit
            // jamais un rectangle de largeur négative.
            let x_rabattu = p.x.clamp(plat.rect.left(), plat.rect.right());
            let point_face = Point::new(x_rabattu, plat.rect.top());
            let distance = p.distance_to(point_face);

            // `match` explicite plutôt qu'une chaîne de combinateurs sur
            // Option : la comparaison se relit mieux.
            let remplace = match meilleur {
                None => true,
                Some((_, _, d)) => distance < d,
            };
            if remplace {
                meilleur = Some((plat.id, x_rabattu - plat.rect.left(), distance));
            }
        }

        meilleur.map(|(id, offset, _)| (id, offset))
    }

    /// Le rectangle englobant toutes les plateformes. Le « bas du bureau
    /// virtuel » de la spec §6.3 s'en déduit.
    ///
    /// `None` si le monde est vide, ce qui n'est pas une erreur.
    pub fn bounds(&self) -> Option<Rect> {
        // `first()?` : si la liste est vide, on rend None immédiatement. Le
        // `?` sur une Option, dans une fonction qui rend une Option, est la
        // façon courte d'écrire ce `match`.
        let premier = self.platforms.first()?;

        let mut min_x = premier.rect.left();
        let mut max_x = premier.rect.right();
        let mut min_y = premier.rect.top();
        let mut max_y = premier.rect.bottom();

        // `[1..]` : on a déjà pris le premier ci-dessus. Sûr même avec un seul
        // élément — la tranche est alors vide, et la boucle ne tourne pas.
        for p in &self.platforms[1..] {
            min_x = min_x.min(p.rect.left());
            max_x = max_x.max(p.rect.right());
            min_y = min_y.min(p.rect.top());
            max_y = max_y.max(p.rect.bottom());
        }

        Some(Rect::new(min_x, min_y, max_x - min_x, max_y - min_y))
    }
}
```

- [ ] **Step 4 : Lancer les tests et les voir passer**

```powershell
cd src-tauri
cargo test
```

Attendu : `32 passed` — 19 de la Tâche 1, 3 de la Tâche 2, 10 ici. Aucun avertissement.

**Si `l_identite_survit_a_un_changement_de_resolution` échoue**, c'est que `PlatformId` a
été dérivé de la géométrie et non de `ScreenInfo::id`. C'est la seule erreur possible dans
cette tâche qui casserait la décision n° 1 : la corriger, ne pas assouplir le test.

- [ ] **Step 5 : Commit**

```bash
git add src-tauri/src/world.rs src-tauri/src/main.rs
git commit -m "feat(etape-1a): le monde, un sol par écran

Le fichier qui encaissera le dividende du §5.1 : la physique et le
comportement ne manipuleront que des Platform, et ne sauront jamais si celle
sous leurs pieds est le sol d'un écran ou une barre de titre. L'étape 4
ajoutera une source de plateformes, aucun appelant ne changera.

Seule la face Top est exposée à cette étape. Les murs et le plafond que
décrit la spec §5.1 attendent l'étape 4 : sans comportement de grimpe, un mur
exposé serait une plateforme dont le personnage ne saurait pas redescendre.

PlatformId dérive de ScreenInfo::id, jamais de la géométrie — c'est la
condition de la décision n° 1, et un test la verrouille en changeant la
résolution sous une identité constante.

nearest_floor rabat l'offset dans les bornes de la face : c'est le garde-fou
de la spec §6.3, un personnage lâché hors écran ne doit jamais être perdu."
```

---

## Tâche 4 : Le manifeste — et la couverture partielle

**Files:**
- Create: `characters/blob/mascot.json`
- Create: `src-tauri/src/character/mod.rs`
- Create: `src-tauri/src/character/manifest.rs`
- Modify: `src-tauri/src/main.rs` (ajouter `mod character;`)

**Interfaces:**
- Consomme : rien du projet (seulement `serde`, `serde_json`, `std::fs`).
- Produit :
  - `character::Facing { Left, Right }` avec `flipped(&self) -> bool`
  - `character::manifest::Hitbox { x, y, w, h }`
  - `character::manifest::Pose { frames: Vec<u32>, frame_ms: u32, looping: bool,
    hold: Option<u32>, anchor: [f32; 2], hitbox: Option<Hitbox> }` avec
    `duree_totale() -> Duration`, `frame_a(Duration) -> u32`
  - `character::manifest::Manifest { id, name, frame_size: [u32; 2], scale, poses, hitbox }`
    avec `load(&Path) -> Result<Manifest, ManifestError>`,
    `pose(&str) -> Option<&Pose>`, `has_pose(&str) -> bool`,
    `hitbox_de(&str) -> Hitbox`
  - `character::manifest::ManifestError` — implémente `Display`
  - les constantes de noms de poses : `POSE_STAND`, `POSE_WALK`, `POSE_RUN`,
    `POSE_SIT`, `POSE_FALL`, `POSE_LAND`, `POSE_DRAGGED`

> **Le fichier où la couverture partielle devient réelle** (spec §8.6). `load` **retire**
> silencieusement toute pose dont les images manquent, au lieu d'échouer. C'est ce qui
> rend utilisable un pack tiers incomplet — et c'est aussi ce qui permettra, à la
> Tâche 8, de retirer une intention du tirage sans un seul cas particulier.

- [ ] **Step 1 : Écrire `characters/blob/mascot.json`**

Le fichier n'existe pas encore ; les 46 PNG, oui. La correspondance frames → poses vient
de la spec §8.5, établie à l'œil sur `blob`. **C'est un point de départ à raffiner en
éditant ce JSON** — c'est précisément le bénéfice du format (spec §8.5).

Ne sont déclarées ici que les poses **utilisées par l'étape 1a**, plus celles des étapes
suivantes : les déclarer maintenant ne coûte rien, et `has_pose` fera le tri.

```json
{
  "id": "blob",
  "name": "Shimeji (mascotte par défaut)",
  "frameSize": [128, 128],
  "scale": 1,
  "hitbox": [40, 20, 48, 100],
  "poses": {
    "stand":     { "frames": [1],                "anchor": [64, 120] },
    "walk":      { "frames": [2, 3, 1, 4],       "frameMs": 120, "loop": true },
    "run":       { "frames": [30, 31, 32, 33],   "frameMs": 80,  "loop": true },
    "sit":       { "frames": [39] },
    "sleep":     { "frames": [39, 40, 41],       "frameMs": 400, "hold": 41 },
    "fall":      { "frames": [10],               "anchor": [64, 64] },
    "land":      { "frames": [4],                "frameMs": 150 },
    "dragged":   { "frames": [10],               "anchor": [64, 64] },
    "crawl":     { "frames": [18, 20, 21],       "frameMs": 200, "loop": true },
    "cling":     { "frames": [23, 24, 25],       "frameMs": 150, "loop": true, "anchor": [110, 64] },
    "climbOver": { "frames": [34, 35, 36],       "frameMs": 180 },
    "hang":      { "frames": [22],               "anchor": [64, 10] },
    "split":     { "frames": [44, 45, 46],       "frameMs": 200 }
  }
}
```

**Deux ancres méritent leur explication**, parce qu'elles sont la raison d'être du champ :

| Pose | Ancre | Pourquoi |
|---|---|---|
| `stand`, `walk`, `run`, `sit`, `land` | `[64, 120]` (le défaut) | le sol sous les pieds : bas de la boîte, centré |
| `fall`, `dragged` | `[64, 64]` | en chute il n'y a pas de sol sous les pieds ; le centre du corps est le point qui suit la trajectoire, et c'est aussi le point que la souris tient |
| `cling` | `[110, 64]` | la main qui agrippe, sur le côté droit de la boîte (étape 4) |
| `hang` | `[64, 10]` | les mains, en haut de la boîte (étape 4) |

- [ ] **Step 2 : Écrire le test d'abord**

Créer `src-tauri/src/character/manifest.rs` avec **seulement** ce bloc.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Écrit un mascot.json et les PNG demandés dans un dossier temporaire,
    /// et rend son chemin.
    ///
    /// Les PNG sont des fichiers **vides** : `load` ne vérifie que leur
    /// EXISTENCE, pas leur contenu. Décoder l'image serait le travail du
    /// webview, et l'exiger ici rendrait le test lent pour rien.
    fn dossier_de_test(json: &str, frames_presentes: &[u32]) -> std::path::PathBuf {
        // `std::env::temp_dir()` plus un nom unique : pas de dépendance à une
        // crate de fichiers temporaires pour trois tests.
        let base = std::env::temp_dir().join(format!(
            "shimeji-test-{}-{}",
            std::process::id(),
            // Un compteur croissant : deux appels dans le même test ne
            // doivent pas se marcher dessus.
            COMPTEUR.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(base.join("img")).unwrap();
        std::fs::write(base.join("mascot.json"), json).unwrap();
        for n in frames_presentes {
            std::fs::write(base.join("img").join(format!("shime{n}.png")), b"").unwrap();
        }
        base
    }

    static COMPTEUR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    const MINIMAL: &str = r#"{
        "id": "t",
        "name": "Test",
        "frameSize": [128, 128],
        "scale": 1,
        "hitbox": [40, 20, 48, 100],
        "poses": {
            "stand": { "frames": [1] },
            "walk":  { "frames": [2, 3], "frameMs": 120, "loop": true }
        }
    }"#;

    #[test]
    fn charge_un_manifeste_correct() {
        let d = dossier_de_test(MINIMAL, &[1, 2, 3]);
        let m = Manifest::load(&d).expect("manifeste valide");

        assert_eq!(m.id, "t");
        assert_eq!(m.frame_size, [128, 128]);
        assert!(m.has_pose(POSE_STAND));
        assert!(m.has_pose(POSE_WALK));
    }

    #[test]
    fn les_defauts_s_appliquent_aux_champs_absents() {
        let d = dossier_de_test(MINIMAL, &[1, 2, 3]);
        let m = Manifest::load(&d).unwrap();

        let stand = m.pose(POSE_STAND).unwrap();
        // `frameMs` absent → 150 (spec §8.5)
        assert_eq!(stand.frame_ms, 150);
        // `loop` absent → false
        assert!(!stand.looping);
        // `anchor` absent → [64, 120], le sol centré
        assert_eq!(stand.anchor, [64.0, 120.0]);
        // `hitbox` de pose absente → celle du manifeste
        assert_eq!(m.hitbox_de(POSE_STAND), m.hitbox);
    }

    #[test]
    fn une_pose_dont_les_images_manquent_est_retiree_sans_erreur() {
        // LE test de la couverture partielle (spec §8.6). `walk` demande les
        // frames 2 et 3 ; on ne fournit que la 1. Le chargement doit
        // RÉUSSIR, et `walk` doit avoir disparu.
        let d = dossier_de_test(MINIMAL, &[1]);
        let m = Manifest::load(&d).expect("le chargement doit réussir malgré tout");

        assert!(m.has_pose(POSE_STAND));
        assert!(!m.has_pose(POSE_WALK), "walk devait être retirée");
    }

    #[test]
    fn une_pose_partiellement_couverte_est_retiree_entierement() {
        // La frame 2 est là, la 3 non. On ne joue pas une animation trouée :
        // c'est tout ou rien par pose.
        let d = dossier_de_test(MINIMAL, &[1, 2]);
        let m = Manifest::load(&d).unwrap();
        assert!(!m.has_pose(POSE_WALK));
    }

    #[test]
    fn un_manifeste_sans_aucune_pose_jouable_est_une_erreur() {
        // La limite de la tolérance : un personnage dont AUCUNE pose n'a
        // d'image n'est pas un personnage à couverture partielle, c'est un
        // dossier vide. Là, il faut le dire.
        let d = dossier_de_test(MINIMAL, &[]);
        match Manifest::load(&d) {
            Err(ManifestError::AucunePoseJouable) => {}
            autre => panic!("attendu AucunePoseJouable, obtenu {autre:?}"),
        }
    }

    #[test]
    fn un_json_malforme_donne_une_erreur_explicite_sans_paniquer() {
        // Spec §10.1 : « Manifeste malformé → erreur explicite, aucune
        // panique. »
        let d = dossier_de_test("{ ceci n'est pas du JSON", &[1]);
        match Manifest::load(&d) {
            Err(ManifestError::JsonInvalide(msg)) => {
                assert!(!msg.is_empty(), "le message doit dire où ça casse");
            }
            autre => panic!("attendu JsonInvalide, obtenu {autre:?}"),
        }
    }

    #[test]
    fn un_mascot_json_absent_donne_une_erreur_explicite() {
        let d = std::env::temp_dir().join("shimeji-test-dossier-inexistant-xyz");
        match Manifest::load(&d) {
            Err(ManifestError::FichierIllisible { .. }) => {}
            autre => panic!("attendu FichierIllisible, obtenu {autre:?}"),
        }
    }

    #[test]
    fn une_pose_sans_frame_est_rejetee_a_la_validation() {
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "stand": { "frames": [1] }, "vide": { "frames": [] } }
        }"#;
        let d = dossier_de_test(json, &[1]);
        let m = Manifest::load(&d).unwrap();
        // Une pose à zéro frame ne peut rien afficher : retirée comme une
        // pose sans images, pour la même raison.
        assert!(!m.has_pose("vide"));
        assert!(m.has_pose(POSE_STAND));
    }

    #[test]
    fn frame_a_avance_dans_une_animation_en_boucle() {
        let d = dossier_de_test(MINIMAL, &[1, 2, 3]);
        let m = Manifest::load(&d).unwrap();
        let walk = m.pose(POSE_WALK).unwrap();

        // frames [2, 3], 120 ms chacune, en boucle
        assert_eq!(walk.frame_a(Duration::from_millis(0)), 2);
        assert_eq!(walk.frame_a(Duration::from_millis(119)), 2);
        assert_eq!(walk.frame_a(Duration::from_millis(120)), 3);
        // 240 ms : retour au début
        assert_eq!(walk.frame_a(Duration::from_millis(240)), 2);
        assert_eq!(walk.frame_a(Duration::from_millis(361)), 3);
    }

    #[test]
    fn frame_a_tient_la_derniere_image_quand_loop_est_faux() {
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "seq": { "frames": [1, 2, 3], "frameMs": 100 } }
        }"#;
        let d = dossier_de_test(json, &[1, 2, 3]);
        let m = Manifest::load(&d).unwrap();
        let seq = m.pose("seq").unwrap();

        assert_eq!(seq.frame_a(Duration::from_millis(0)), 1);
        assert_eq!(seq.frame_a(Duration::from_millis(250)), 3);
        // Au-delà de la séquence : on TIENT la dernière, on ne reboucle pas.
        assert_eq!(seq.frame_a(Duration::from_secs(10)), 3);
    }

    #[test]
    fn hold_choisit_l_image_tenue_a_la_fin() {
        // C'est ce qui permet de RESTER endormi après la séquence
        // d'endormissement (spec §8.5). Ici, la séquence finit sur 3 mais
        // c'est 2 qui doit être tenue.
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "seq": { "frames": [1, 2, 3], "frameMs": 100, "hold": 2 } }
        }"#;
        let d = dossier_de_test(json, &[1, 2, 3]);
        let m = Manifest::load(&d).unwrap();
        let seq = m.pose("seq").unwrap();

        assert_eq!(seq.frame_a(Duration::from_millis(250)), 3);
        assert_eq!(seq.frame_a(Duration::from_secs(10)), 2);
    }

    #[test]
    fn le_manifeste_reel_de_blob_se_charge() {
        // Le seul test qui touche au dépôt : il vérifie que le mascot.json
        // écrit au Step 1 est cohérent avec les 46 PNG réellement présents.
        // Un chemin relatif depuis src-tauri/, où cargo exécute les tests.
        let d = std::path::Path::new("../characters/blob");
        let m = Manifest::load(d).expect("blob doit se charger");

        assert_eq!(m.id, "blob");
        // blob possède TOUTES les poses (spec §8.7) : aucune ne doit avoir
        // été retirée. Si ce test échoue, c'est qu'un numéro de frame du
        // mascot.json ne correspond à aucun fichier.
        for pose in [
            POSE_STAND, POSE_WALK, POSE_RUN, POSE_SIT, POSE_FALL, POSE_LAND, POSE_DRAGGED,
        ] {
            assert!(m.has_pose(pose), "pose manquante : {pose}");
        }
    }
}
```

- [ ] **Step 3 : Lancer les tests et les voir échouer**

```powershell
cd src-tauri
cargo test manifest
```

Attendu : **échec de compilation** — `Manifest`, `ManifestError`, `POSE_STAND` n'existent
pas.

- [ ] **Step 4 : Écrire `src-tauri/src/character/mod.rs`**

Pour l'instant, il ne porte que `Facing` et déclare son sous-module. `Character` arrive à
la Tâche 7, quand le comportement en a besoin.

```rust
//! Le personnage : son état, son manifeste, son accroche, sa physique.
//!
//! Ce module regroupe ce qui change ensemble. `Character` lui-même arrive à
//! la Tâche 7, avec le comportement qui le fait vivre.

pub mod manifest;

/// Le sens dans lequel le personnage regarde.
///
/// **Il n'y a pas de frames dédiées à chaque sens** : l'orientation est
/// obtenue par miroir horizontal (spec §8.5). Ce type dit donc s'il faut
/// retourner le sprite, et rien d'autre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Facing {
    Left,
    Right,
}

impl Facing {
    /// `true` s'il faut retourner le sprite horizontalement.
    ///
    /// Convention : les sprites sont dessinés **tournés vers la droite**.
    /// Regarder à gauche demande donc un miroir. Si un pack tiers est dessiné
    /// dans l'autre sens, il paraîtra à l'envers — c'est un défaut de contenu
    /// qui se corrige dans le manifeste, pas ici.
    pub fn flipped(&self) -> bool {
        matches!(self, Facing::Left)
    }

    /// L'autre sens. Sert au demi-tour en bout de plateforme.
    pub fn inverse(&self) -> Facing {
        match self {
            Facing::Left => Facing::Right,
            Facing::Right => Facing::Left,
        }
    }

    /// `-1.0` vers la gauche, `+1.0` vers la droite. Multiplier une vitesse
    /// par ce signe évite un `match` à chaque déplacement.
    pub fn signe(&self) -> f32 {
        match self {
            Facing::Left => -1.0,
            Facing::Right => 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facing_gauche_demande_un_miroir() {
        assert!(Facing::Left.flipped());
        assert!(!Facing::Right.flipped());
    }

    #[test]
    fn inverse_fait_un_aller_retour() {
        assert_eq!(Facing::Left.inverse(), Facing::Right);
        assert_eq!(Facing::Left.inverse().inverse(), Facing::Left);
    }

    #[test]
    fn le_signe_correspond_au_sens() {
        assert_eq!(Facing::Right.signe(), 1.0);
        assert_eq!(Facing::Left.signe(), -1.0);
    }
}
```

- [ ] **Step 5 : Écrire l'implémentation dans `manifest.rs`, au-dessus des tests**

```rust
//! Chargement et validation de `mascot.json` (spec §8.5).
//!
//! Responsabilité unique : transformer un dossier de personnage en `Manifest`
//! utilisable, ou en une erreur explicite.
//!
//! **Le principe qui gouverne ce fichier : une pose dont les images manquent
//! est RETIRÉE, pas une erreur** (spec §8.6). C'est ce qui rend utilisable
//! n'importe quel pack Shimeji trouvé sur internet, même incomplet — et c'est
//! ce qui, à la Tâche 8, retirera une intention du tirage sans un seul cas
//! particulier à coder.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::time::Duration;

// ── Les noms de poses connus du code ───────────────────────────────────
//
// Le vocabulaire vient des slots Shimeji (spec §8.2). Des constantes plutôt
// que des littéraux dispersés : une faute de frappe devient une erreur de
// compilation au lieu d'une pose silencieusement absente.
//
// Ces sept-là sont celles dont l'étape 1a a besoin. Les autres poses du
// manifeste (`sleep`, `crawl`, `cling`, `climbOver`, `hang`, `split`) sont
// chargées mais encore inutilisées : elles servent aux étapes 2 et 4.

pub const POSE_STAND: &str = "stand";
pub const POSE_WALK: &str = "walk";
pub const POSE_RUN: &str = "run";
pub const POSE_SIT: &str = "sit";
pub const POSE_FALL: &str = "fall";
pub const POSE_LAND: &str = "land";
pub const POSE_DRAGGED: &str = "dragged";

/// Le rectangle réellement occupé par le personnage dans la boîte de 128×128
/// (spec §8.4). Sert au hit-testing (Tâche 11) et à la proximité entre
/// personnages (étape 3).
///
/// `#[serde(from = "[f32; 4]")]` : dans le JSON c'est un tableau
/// `[x, y, l, h]`, mais on veut des champs nommés dans le code — se souvenir
/// que `.2` est la largeur est exactement le genre de détail qui produit des
/// bugs silencieux. Serde désérialise le tableau, puis appelle le `From`
/// ci-dessous.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(from = "[f32; 4]")]
pub struct Hitbox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl From<[f32; 4]> for Hitbox {
    fn from(v: [f32; 4]) -> Self {
        Hitbox {
            x: v[0],
            y: v[1],
            w: v[2],
            h: v[3],
        }
    }
}

/// L'ancre par défaut : le sol sous les pieds, bas de la boîte, centré
/// (spec §8.5).
fn ancre_par_defaut() -> [f32; 2] {
    [64.0, 120.0]
}

/// La durée d'affichage par défaut d'une frame (spec §8.5).
fn frame_ms_par_defaut() -> u32 {
    150
}

fn scale_par_defaut() -> f32 {
    1.0
}

fn frame_size_par_defaut() -> [u32; 2] {
    [128, 128]
}

/// Une pose : une suite d'images, un rythme, une ancre.
///
/// `rename_all = "camelCase"` : le JSON écrit `frameMs`, le Rust `frame_ms`.
/// Une seule annotation évite un `rename` par champ.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pose {
    /// Numéros de `shime<n>.png`, dans l'ordre de lecture.
    pub frames: Vec<u32>,

    #[serde(default = "frame_ms_par_defaut")]
    pub frame_ms: u32,

    /// `loop` est un **mot-clé Rust** : impossible de nommer le champ ainsi.
    /// `rename` fait le pont avec le JSON, et `default` donne `false`.
    #[serde(rename = "loop", default)]
    pub looping: bool,

    /// Frame conservée à l'arrêt quand `looping` est faux. C'est ce qui
    /// permet de *rester* endormi après la séquence d'endormissement
    /// (spec §8.5). Absent → la dernière frame de la séquence.
    #[serde(default)]
    pub hold: Option<u32>,

    /// `[x, y]` dans la boîte : le point du sprite à faire coïncider avec la
    /// position sur la plateforme.
    ///
    /// Gardé en tableau brut plutôt qu'en `geom::Point` : `geom` n'a aucune
    /// dépendance, et lui en ajouter une sur `serde` pour ce seul champ
    /// coûterait plus que la conversion au point d'usage (Tâche 5).
    #[serde(default = "ancre_par_defaut")]
    pub anchor: [f32; 2],

    /// Hitbox propre à cette pose. Absent → celle du manifeste (spec §8.5).
    #[serde(default)]
    pub hitbox: Option<Hitbox>,
}

impl Pose {
    /// Durée d'un cycle complet de l'animation.
    pub fn duree_totale(&self) -> Duration {
        Duration::from_millis(self.frame_ms as u64 * self.frames.len() as u64)
    }

    /// Quelle image afficher après `ecoule` passé dans cette pose.
    ///
    /// C'est une **fonction pure du temps écoulé**, et non un compteur qu'on
    /// incrémente. Deux bénéfices : elle est testable sans faire tourner de
    /// boucle, et un décalage d'image ne peut pas s'accumuler.
    pub fn frame_a(&self, ecoule: Duration) -> u32 {
        // Une pose sans frame ne devrait pas exister — `load` les retire.
        // Mais `frame_a` peut être appelée sur une `Pose` construite à la
        // main dans un test, donc on ne suppose rien.
        if self.frames.is_empty() {
            return 1;
        }

        let index = if self.frame_ms == 0 {
            // frameMs à 0 : on tiendrait la première image indéfiniment.
            // Une division par zéro plus loin serait un panic.
            0
        } else {
            (ecoule.as_millis() / self.frame_ms as u128) as usize
        };

        if index < self.frames.len() {
            return self.frames[index];
        }

        // La séquence est finie.
        if self.looping {
            // `%` sur l'index et non sur le temps : une seule opération, et
            // pas de perte de précision sur les longues durées.
            self.frames[index % self.frames.len()]
        } else {
            // `hold` s'il est donné, la dernière frame sinon.
            //
            // `unwrap_or_else` et non `unwrap_or` : on ne veut pas évaluer
            // l'accès au dernier élément si `hold` est présent.
            self.hold
                .unwrap_or_else(|| *self.frames.last().expect("non vide, testé plus haut"))
        }
    }
}

/// Le manifeste complet d'un personnage.
///
/// `BTreeMap` et non `HashMap` : l'ordre des poses devient déterministe, ce
/// qui rend la trace du mode simulation (Tâche 9) reproductible d'une
/// exécution à l'autre. Le coût est nul à cette taille.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,

    #[serde(default = "frame_size_par_defaut")]
    pub frame_size: [u32; 2],

    /// Multiplie la taille d'affichage. Le facteur d'échelle du moniteur s'y
    /// combine (spec §3.4, §8.5).
    #[serde(default = "scale_par_defaut")]
    pub scale: f32,

    pub poses: BTreeMap<String, Pose>,

    /// La hitbox par défaut, pour les poses qui n'en déclarent pas.
    pub hitbox: Hitbox,
}

/// Ce qui peut mal se passer au chargement.
///
/// Un `enum` et non une chaîne : l'appelant peut distinguer « ce dossier
/// n'est pas un personnage » (on l'ignore) de « ce personnage est cassé »
/// (on le signale). Et les tests peuvent vérifier *laquelle* des erreurs
/// s'est produite.
#[derive(Debug)]
pub enum ManifestError {
    FichierIllisible { chemin: String, cause: String },
    JsonInvalide(String),
    /// Aucune pose ne possède ses images. Ce n'est plus de la couverture
    /// partielle, c'est un dossier vide — et là, il faut le dire.
    AucunePoseJouable,
}

// `Display` plutôt que la crate `thiserror` : trois variantes ne justifient
// pas une dépendance, et écrire le message à la main le rend lisible en
// français, ce que le projet exige.
impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::FichierIllisible { chemin, cause } => {
                write!(f, "impossible de lire « {chemin} » : {cause}")
            }
            ManifestError::JsonInvalide(msg) => {
                write!(f, "mascot.json invalide : {msg}")
            }
            ManifestError::AucunePoseJouable => write!(
                f,
                "aucune pose n'a ses images — le dossier img/ est-il vide \
                 ou les numéros de frames faux ?"
            ),
        }
    }
}

// `std::error::Error` : rend l'erreur utilisable avec `Box<dyn Error>` et
// `?`. L'implémentation par défaut suffit, `Display` et `Debug` étant là.
impl std::error::Error for ManifestError {}

impl Manifest {
    /// Charge `<dir>/mascot.json` et **retire les poses dont les images
    /// manquent** dans `<dir>/img/`.
    ///
    /// Le retrait est le cœur de la spec §8.6 : il n'échoue pas, il réduit.
    /// Un personnage sans images d'escalade ne grimpera jamais, et ça se
    /// règle tout seul au tirage des envies (Tâche 8).
    pub fn load(dir: &Path) -> Result<Manifest, ManifestError> {
        let chemin = dir.join("mascot.json");

        let texte = std::fs::read_to_string(&chemin).map_err(|e| {
            // `map_err` : on remplace l'erreur d'E/S par la nôtre, en gardant
            // son message. C'est ce qui permet de dire QUEL fichier manque.
            ManifestError::FichierIllisible {
                chemin: chemin.display().to_string(),
                cause: e.to_string(),
            }
        })?;

        let mut manifeste: Manifest = serde_json::from_str(&texte)
            .map_err(|e| ManifestError::JsonInvalide(e.to_string()))?;

        // ── Retrait des poses injouables ───────────────────────────────
        let dossier_img = dir.join("img");
        let mut retirees: Vec<String> = Vec::new();

        // `retain` garde les éléments pour lesquels la fermeture rend `true`.
        // On collecte au passage les noms retirés, pour pouvoir les
        // annoncer : une pose qui disparaît en silence rendrait le réglage
        // d'un manifeste très pénible à diagnostiquer.
        manifeste.poses.retain(|nom, pose| {
            // Une pose sans frame ne peut rien afficher : même traitement
            // qu'une pose sans images.
            if pose.frames.is_empty() {
                retirees.push(nom.clone());
                return false;
            }

            let toutes_presentes = pose
                .frames
                .iter()
                .all(|n| dossier_img.join(format!("shime{n}.png")).is_file());

            if !toutes_presentes {
                retirees.push(nom.clone());
            }
            toutes_presentes
        });

        if !retirees.is_empty() {
            // `eprintln!` et non `println!` : c'est un diagnostic, il n'a
            // rien à faire dans la trace du mode simulation.
            eprintln!(
                "personnage « {} » : {} pose(s) retirée(s) faute d'images — {}",
                manifeste.id,
                retirees.len(),
                retirees.join(", ")
            );
        }

        if manifeste.poses.is_empty() {
            return Err(ManifestError::AucunePoseJouable);
        }

        Ok(manifeste)
    }

    pub fn pose(&self, nom: &str) -> Option<&Pose> {
        self.poses.get(nom)
    }

    /// **La fonction qui rend la couverture partielle gratuite.**
    ///
    /// Le tirage des envies (Tâche 8) l'appelle pour retirer les options
    /// injouables. Aucun cas particulier ailleurs.
    pub fn has_pose(&self, nom: &str) -> bool {
        self.poses.contains_key(nom)
    }

    /// La hitbox effective d'une pose : la sienne si elle en déclare une,
    /// celle du manifeste sinon (spec §8.5).
    ///
    /// Une pose inconnue rend la hitbox du manifeste plutôt que `None` :
    /// l'appelant (le hit-testing, Tâche 11) a toujours besoin d'un
    /// rectangle, et celui du manifeste est le repli sensé.
    pub fn hitbox_de(&self, nom: &str) -> Hitbox {
        match self.pose(nom) {
            Some(p) => p.hitbox.unwrap_or(self.hitbox),
            None => self.hitbox,
        }
    }
}
```

- [ ] **Step 6 : Lancer les tests et les voir passer**

Ajouter `mod character;` dans `main.rs`.

```powershell
cd src-tauri
cargo test
```

Attendu : `47 passed` — 32 des tâches précédentes, 3 de `Facing`, 12 de `manifest`.

**Si `le_manifeste_reel_de_blob_se_charge` échoue** en signalant des poses retirées, lire
le message `eprintln!` : il nomme les poses fautives. Cela veut dire qu'un numéro de frame
du `mascot.json` du Step 1 ne correspond à aucun fichier de `characters/blob/img/`.
Corriger le JSON, pas le test.

- [ ] **Step 7 : Vérifier à l'œil que la couverture partielle fonctionne pour de vrai**

Le test le prouve sur un dossier factice. Une vérification sur le vrai personnage vaut
d'être faite une fois, parce que c'est la promesse « n'importe quel pack du net
fonctionne » :

```powershell
cd C:\Users\alri\Documents\shimeji-desktop
# On cache une frame dont dépend `run`
Rename-Item characters\blob\img\shime30.png shime30.png.bak
cd src-tauri
cargo test le_manifeste_reel_de_blob -- --nocapture
```

Attendu : le test **échoue** (il exige `run`), et la sortie contient
`1 pose(s) retirée(s) faute d'images — run`. C'est la preuve que le retrait fonctionne et
qu'il est annoncé.

Remettre le fichier :

```powershell
cd C:\Users\alri\Documents\shimeji-desktop
Rename-Item characters\blob\img\shime30.png.bak shime30.png
cd src-tauri
cargo test
```

Attendu : `47 passed` de nouveau.

- [ ] **Step 8 : Commit**

```bash
git add characters/blob/mascot.json src-tauri/src/character src-tauri/src/main.rs
git commit -m "feat(etape-1a): le manifeste, et la couverture partielle

mascot.json pour blob, d'après la correspondance frames → poses de la spec
§8.5. C'est un point de départ à raffiner en éditant ce JSON — précisément le
bénéfice du format.

Le principe qui gouverne manifest.rs : une pose dont les images manquent est
RETIRÉE, pas une erreur (spec §8.6). C'est ce qui rendra utilisable un pack
Shimeji tiers incomplet, et ce qui retirera une intention du tirage sans un
seul cas particulier à coder. La limite : un manifeste dont AUCUNE pose n'est
jouable est une erreur — ce n'est plus de la couverture partielle, c'est un
dossier vide.

Pose::frame_a est une fonction pure du temps écoulé, pas un compteur qu'on
incrémente : testable sans boucle, et aucun décalage ne peut s'accumuler.

BTreeMap et non HashMap pour que l'ordre des poses soit déterministe — la
trace du mode simulation doit être reproductible."
```

---

## Tâche 5 : L'accroche et la position dérivée — décision n° 1

**Files:**
- Create: `src-tauri/src/character/attach.rs`
- Modify: `src-tauri/src/character/mod.rs` (ajouter `pub mod attach;`)

**Interfaces:**
- Consomme : `geom::{Face, Point, Vec2}`, `world::{PlatformId, World}`,
  `character::Facing`, `character::manifest::{Manifest, Pose}`.
- Produit :
  - `character::attach::Attachment` — `On { platform, face, offset }`,
    `Falling { pos, vel }`, `Dragged`
  - `character::attach::world_position(&Attachment, &World, Point) -> Option<Point>`
  - `character::attach::hors_bornes(&Attachment, &World) -> bool`
  - `character::attach::window_top_left(Point, &Pose, &Manifest, f32, Facing) -> Point`

> **La tâche la plus importante du plan.** Tout ce que la décision n° 1 rend gratuit
> (fenêtre déplacée, redimensionnée, fermée) passe par les trente lignes de
> `world_position`. Si cette tâche est faite de travers — en mémorisant la position au
> lieu de la dériver — l'étape 4 devra être réécrite entièrement.

- [ ] **Step 1 : Écrire le test d'abord**

Créer `src-tauri/src/character/attach.rs` avec **seulement** ce bloc.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{ScreenInfo, SystemProbe};
    use crate::probe::fake::FakeProbe;

    fn monde_un_ecran() -> World {
        World::from_screens(&FakeProbe::un_ecran().screens())
    }

    fn id_du_sol(monde: &World) -> PlatformId {
        monde.platforms()[0].id
    }

    /// La souris, quand le test ne s'y intéresse pas.
    const SOURIS_AILLEURS: Point = Point { x: 0.0, y: 0.0 };

    #[test]
    fn la_position_est_derivee_de_la_plateforme() {
        let monde = monde_un_ecran();
        let att = Attachment::On {
            platform: id_du_sol(&monde),
            face: Face::Top,
            offset: 300.0,
        };

        let p = world_position(&att, &monde, SOURIS_AILLEURS).expect("plateforme présente");
        // Le sol du FakeProbe::un_ecran est à y = 1032, x de 0 à 1920.
        assert_eq!(p, Point::new(300.0, 1032.0));
    }

    #[test]
    fn deplacer_la_plateforme_deplace_le_personnage_sans_code() {
        // LE test de la décision n° 1. On ne touche pas au personnage : on
        // change la géométrie de sa plateforme, sous la même identité, et sa
        // position doit avoir suivi.
        let avant = World::from_screens(&[ScreenInfo {
            id: 42,
            work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
            scale: 1.0,
        }]);
        let apres = World::from_screens(&[ScreenInfo {
            id: 42,
            // L'écran a « bougé » de 500 px vers la droite et 100 vers le bas.
            work_area: Rect::new(500.0, 100.0, 1920.0, 1032.0),
            scale: 1.0,
        }]);

        let att = Attachment::On {
            platform: PlatformId(42),
            face: Face::Top,
            offset: 300.0,
        };

        let p_avant = world_position(&att, &avant, SOURIS_AILLEURS).unwrap();
        let p_apres = world_position(&att, &apres, SOURIS_AILLEURS).unwrap();

        assert_eq!(p_avant, Point::new(300.0, 1032.0));
        assert_eq!(p_apres, Point::new(800.0, 1132.0));
    }

    #[test]
    fn redimensionner_la_plateforme_conserve_la_distance_au_bord() {
        // Second bénéfice gratuit de la décision n° 1 : l'offset est une
        // distance au bord, donc il ne bouge pas quand la face s'allonge.
        let etroit = World::from_screens(&[ScreenInfo {
            id: 42,
            work_area: Rect::new(0.0, 0.0, 800.0, 1032.0),
            scale: 1.0,
        }]);
        let large = World::from_screens(&[ScreenInfo {
            id: 42,
            work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
            scale: 1.0,
        }]);

        let att = Attachment::On {
            platform: PlatformId(42),
            face: Face::Top,
            offset: 100.0,
        };

        assert_eq!(
            world_position(&att, &etroit, SOURIS_AILLEURS).unwrap().x,
            world_position(&att, &large, SOURIS_AILLEURS).unwrap().x
        );
    }

    #[test]
    fn une_plateforme_absente_ne_donne_aucune_position() {
        // C'est le test unique qui remplace tout le traitement du cas
        // « la fenêtre s'est fermée » (spec §6.2).
        let monde = monde_un_ecran();
        let att = Attachment::On {
            platform: PlatformId(999_999),
            face: Face::Top,
            offset: 300.0,
        };
        assert_eq!(world_position(&att, &monde, SOURIS_AILLEURS), None);
    }

    #[test]
    fn une_plateforme_absente_est_hors_bornes() {
        let monde = monde_un_ecran();
        let att = Attachment::On {
            platform: PlatformId(999_999),
            face: Face::Top,
            offset: 300.0,
        };
        assert!(hors_bornes(&att, &monde));
    }

    #[test]
    fn un_offset_au_dela_de_la_face_est_hors_bornes() {
        // Le cas « la plateforme a rétréci sous lui ». Un seul test, comme
        // la spec §6.2 le promet.
        let monde = monde_un_ecran();
        let sol = id_du_sol(&monde);

        let dedans = Attachment::On {
            platform: sol,
            face: Face::Top,
            offset: 1900.0,
        };
        let dehors = Attachment::On {
            platform: sol,
            face: Face::Top,
            offset: 1930.0,
        };
        let negatif = Attachment::On {
            platform: sol,
            face: Face::Top,
            offset: -5.0,
        };

        assert!(!hors_bornes(&dedans, &monde));
        assert!(hors_bornes(&dehors, &monde));
        assert!(hors_bornes(&negatif, &monde));
    }

    #[test]
    fn falling_est_le_seul_etat_a_position_absolue() {
        let monde = monde_un_ecran();
        let att = Attachment::Falling {
            pos: Point::new(700.0, 200.0),
            vel: Vec2::new(0.0, 300.0),
        };
        // La position ne dépend d'aucune plateforme : c'est cohérent, en
        // chute il n'est attaché à rien (spec §6.2).
        assert_eq!(
            world_position(&att, &monde, SOURIS_AILLEURS),
            Some(Point::new(700.0, 200.0))
        );
        assert!(!hors_bornes(&att, &monde));
    }

    #[test]
    fn dragged_suit_la_souris_et_ne_stocke_rien() {
        // `Dragged` n'a AUCUNE donnée : sa position se dérive de la souris,
        // comme `On` se dérive de sa plateforme. Même principe.
        let monde = monde_un_ecran();
        let att = Attachment::Dragged;
        assert_eq!(
            world_position(&att, &monde, Point::new(640.0, 480.0)),
            Some(Point::new(640.0, 480.0))
        );
        assert!(!hors_bornes(&att, &monde));
    }

    #[test]
    fn window_top_left_pose_l_ancre_sur_la_position() {
        // L'ancre `stand` est [64, 120] : la fenêtre de 128×128 doit donc
        // être placée 64 px à gauche et 120 px au-dessus de la position.
        let m = manifeste_de_test();
        let pose = m.pose(POSE_STAND).unwrap();

        let coin = window_top_left(
            Point::new(300.0, 1032.0),
            pose,
            &m,
            1.0,
            Facing::Right,
        );
        assert_eq!(coin, Point::new(300.0 - 64.0, 1032.0 - 120.0));
    }

    #[test]
    fn window_top_left_reflete_l_ancre_quand_le_sprite_est_retourne() {
        // Une ancre décentrée doit être MIROITÉE avec le sprite, sinon le
        // personnage se décale d'un coup en faisant demi-tour.
        //
        // C'est le remplaçant principiel du bricolage du prototype VSCode,
        // qui compensait le décalage à la main, asymétriquement selon le
        // sens de marche (spec §8.3).
        let m = manifeste_de_test();
        let pose = m.pose("decentre").unwrap(); // ancre [100, 120]

        let a_droite = window_top_left(Point::new(500.0, 1032.0), pose, &m, 1.0, Facing::Right);
        let a_gauche = window_top_left(Point::new(500.0, 1032.0), pose, &m, 1.0, Facing::Left);

        // Vers la droite : l'ancre est à 100 depuis le bord gauche.
        assert_eq!(a_droite.x, 500.0 - 100.0);
        // Vers la gauche : le sprite est miroité, l'ancre se retrouve à
        // 128 - 100 = 28 depuis le bord gauche.
        assert_eq!(a_gauche.x, 500.0 - 28.0);
        // La hauteur, elle, ne bouge pas : le miroir est horizontal.
        assert_eq!(a_droite.y, a_gauche.y);
    }

    #[test]
    fn window_top_left_applique_l_echelle_a_l_ancre() {
        // Spec §3.4 : l'échelle ne sert QU'au dimensionnement du sprite. La
        // position, elle, reste en pixels physiques — mais l'ancre est
        // exprimée dans la boîte du sprite, donc elle se met à l'échelle
        // avec lui.
        let m = manifeste_de_test();
        let pose = m.pose(POSE_STAND).unwrap();

        let coin = window_top_left(Point::new(300.0, 1032.0), pose, &m, 2.0, Facing::Right);
        assert_eq!(coin, Point::new(300.0 - 128.0, 1032.0 - 240.0));
    }

    /// Un manifeste construit en mémoire, sans toucher au disque : ces tests
    /// portent sur la géométrie, pas sur le chargement (déjà couvert en
    /// Tâche 4).
    fn manifeste_de_test() -> Manifest {
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128, 128], "scale": 1,
            "hitbox": [40, 20, 48, 100],
            "poses": {
                "stand":    { "frames": [1] },
                "decentre": { "frames": [1], "anchor": [100, 120] }
            }
        }"#;
        serde_json::from_str(json).expect("manifeste de test valide")
    }
}
```

- [ ] **Step 2 : Lancer les tests et les voir échouer**

```powershell
cd src-tauri
cargo test attach
```

Attendu : **échec de compilation** — `Attachment`, `world_position`, `hors_bornes`,
`window_top_left` n'existent pas.

- [ ] **Step 3 : Écrire l'implémentation, au-dessus des tests**

```rust
//! L'accroche : où le personnage se trouve, et comment on le sait.
//!
//! **Responsabilité unique, et la plus importante du projet : la position
//! d'un personnage accroché n'est jamais stockée, elle est DÉRIVÉE** de la
//! plateforme à chaque image (décision n° 1, spec §6.2).
//!
//! Ce que cette seule règle rend gratuit :
//!   · la fenêtre est déplacée      → il voyage avec elle, zéro ligne
//!   · elle est redimensionnée      → il garde sa distance au bord, zéro ligne
//!   · elle rétrécit sous lui       → un test : `hors_bornes`
//!   · elle se ferme ou se minimise → le même test
//!
//! Un personnage assis sur une barre de titre qu'on balade est *le* moment
//! qui fait sourire avec un Shimeji. Ici c'est une conséquence du modèle, pas
//! une fonctionnalité écrite.
//!
//! ⚠️ **Ne jamais ajouter un champ `pos` à `Attachment::On`**, même « pour
//! éviter de recalculer ». Le recalcul est trois additions ; le champ
//! ramènerait les quatre bugs ci-dessus d'un coup.

use super::manifest::{Manifest, Pose};
use super::Facing;
use crate::geom::{Face, Point, Rect, Vec2};
use crate::world::{PlatformId, World};

/// Où est le personnage — sous la forme qui rend la décision n° 1 possible.
///
/// Noter l'asymétrie : `On` et `Dragged` n'ont **aucune** coordonnée absolue,
/// `Falling` en a. Ce n'est pas une incohérence, c'est le modèle : en chute
/// il n'est attaché à rien, donc il n'y a rien dont dériver sa position
/// (spec §6.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Attachment {
    /// Accroché à une face d'une plateforme, à `offset` du bord.
    On {
        platform: PlatformId,
        face: Face,
        /// Distance le long de la face, depuis son extrémité la plus
        /// « petite » (gauche pour les horizontales, haut pour les
        /// verticales). **Pas** une coordonnée écran.
        offset: f32,
    },

    /// En chute libre. Le seul état à position absolue.
    Falling { pos: Point, vel: Vec2 },

    /// Tenu par la souris. Aucune donnée : la position se dérive du curseur,
    /// exactement comme `On` se dérive de sa plateforme.
    Dragged,
}

/// **La fonction centrale du projet.** La position écran du point d'ancrage du
/// personnage, recalculée depuis l'état courant du monde.
///
/// `souris` sert au cas `Dragged`. Le passer toujours, plutôt que de stocker
/// la position dans la variante, garde `Dragged` sans données — et donc
/// impossible à désynchroniser du curseur.
///
/// Rend `None` quand la plateforme a disparu. **Ce `None` est le seul
/// mécanisme de détection de la fermeture d'une fenêtre** dont le projet a
/// besoin (spec §6.2) : l'appelant en déduit une chute.
pub fn world_position(att: &Attachment, world: &World, souris: Point) -> Option<Point> {
    match att {
        Attachment::On {
            platform,
            face,
            offset,
        } => {
            // `let … else` : si la plateforme n'existe plus, on sort avec
            // None. Équivalent d'un `match` dont la branche None ferait
            // `return None`, en une ligne.
            let Some(plat) = world.get(*platform) else {
                return None;
            };

            // On repart du rectangle COURANT, jamais d'une position
            // mémorisée : c'est ce qui rend gratuit le déplacement de la
            // plateforme (décision n° 1).
            Some(plat.rect.point_on(*face, *offset))
        }

        Attachment::Falling { pos, .. } => Some(*pos),

        Attachment::Dragged => Some(souris),
    }
}

/// Le personnage a-t-il perdu son appui ?
///
/// Deux causes, un seul test — c'est la promesse de la spec §6.2 :
///   · la plateforme a disparu (fenêtre fermée, écran débranché) ;
///   · l'offset est sorti de la face (la plateforme a rétréci sous lui).
///
/// `Falling` et `Dragged` rendent toujours `false` : ils n'ont pas d'appui à
/// perdre.
pub fn hors_bornes(att: &Attachment, world: &World) -> bool {
    match att {
        Attachment::On {
            platform,
            face,
            offset,
        } => {
            let Some(plat) = world.get(*platform) else {
                // Plateforme disparue : c'est le cas le plus fréquent à
                // l'étape 4, et il ne demande pas une ligne de plus.
                return true;
            };

            // La face doit exister ET l'offset y tenir. À l'étape 4, un bord
            // recouvert disparaît de `faces` (décision n° 2) : ce test
            // deviendra alors aussi celui de l'occlusion, sans changer.
            if !plat.has_face(*face) {
                return true;
            }

            *offset < 0.0 || *offset > plat.rect.face_length(*face)
        }

        Attachment::Falling { .. } | Attachment::Dragged => false,
    }
}

/// Le coin supérieur gauche où placer la **fenêtre** de 128×128, pour que
/// l'ancre de la pose tombe exactement sur `pos`.
///
/// « Positionner » devient ainsi « place l'ancre ici » (spec §8.3).
///
/// > ⚠️ **Ne jamais ajouter ici une compensation de décalage** pour rattraper
/// > un sprite qui « paraît trop haut » ou décalé selon le sens de marche.
/// > L'ancre existe exactement pour rendre ça inutile : si le personnage est
/// > mal posé, c'est l'ancre du manifeste qu'il faut corriger — c'est de la
/// > donnée, elle se règle sans recompiler. Le prototype VSCode contenait un
/// > tel bricolage, asymétrique selon le sens de marche ; il disparaît ici
/// > (spec §8.3).
pub fn window_top_left(
    pos: Point,
    pose: &Pose,
    manifest: &Manifest,
    scale_ecran: f32,
    facing: Facing,
) -> Point {
    // L'échelle totale : celle du manifeste combinée à celle du moniteur
    // (spec §3.4, §8.5). C'est le SEUL usage légitime du facteur d'échelle —
    // il ne convertit jamais une coordonnée.
    let echelle = manifest.scale * scale_ecran;

    let largeur_boite = manifest.frame_size[0] as f32;

    // Quand le sprite est retourné, l'ancre l'est aussi : une ancre à 100 px
    // du bord gauche se retrouve à `largeur - 100` du bord gauche.
    //
    // Sans cette symétrie, un personnage dont l'ancre est décentrée sauterait
    // latéralement à chaque demi-tour. C'est du miroir, pas de la
    // compensation : on retourne l'ancre avec l'image, on ne corrige pas
    // après coup.
    let ancre_x = if facing.flipped() {
        largeur_boite - pose.anchor[0]
    } else {
        pose.anchor[0]
    };

    // Le miroir est horizontal : `y` n'est pas concerné.
    let ancre_y = pose.anchor[1];

    Point::new(
        pos.x - ancre_x * echelle,
        pos.y - ancre_y * echelle,
    )
}

/// La taille en pixels physiques de la fenêtre d'un personnage.
///
/// Séparée de `window_top_left` parce qu'elle ne change qu'au chargement du
/// manifeste ou au changement d'écran, alors que le coin change 60 fois par
/// seconde.
pub fn window_size(manifest: &Manifest, scale_ecran: f32) -> (u32, u32) {
    let echelle = manifest.scale * scale_ecran;
    (
        (manifest.frame_size[0] as f32 * echelle).round() as u32,
        (manifest.frame_size[1] as f32 * echelle).round() as u32,
    )
}

/// Le rectangle écran de la hitbox de la pose courante — celle qui sert au
/// hit-testing (Tâche 11) et à la proximité entre personnages (étape 3).
///
/// La hitbox est déclarée **dans la boîte du sprite** (spec §8.4) ; il faut
/// donc la translater et la mettre à l'échelle comme le sprite, miroir
/// compris.
pub fn hitbox_ecran(
    pos: Point,
    pose_nom: &str,
    pose: &Pose,
    manifest: &Manifest,
    scale_ecran: f32,
    facing: Facing,
) -> Rect {
    let echelle = manifest.scale * scale_ecran;
    let coin = window_top_left(pos, pose, manifest, scale_ecran, facing);
    let hb = manifest.hitbox_de(pose_nom);
    let largeur_boite = manifest.frame_size[0] as f32;

    // Même miroir que pour l'ancre : le bord gauche de la hitbox retournée
    // est à `largeur - (x + l)` du bord gauche de la boîte.
    let hb_x = if facing.flipped() {
        largeur_boite - (hb.x + hb.w)
    } else {
        hb.x
    };

    Rect::new(
        coin.x + hb_x * echelle,
        coin.y + hb.y * echelle,
        hb.w * echelle,
        hb.h * echelle,
    )
}
```

- [ ] **Step 4 : Ajouter deux tests pour les fonctions découvertes en écrivant**

`window_size` et `hitbox_ecran` n'étaient pas dans les tests du Step 1 — elles sont
apparues en écrivant l'implémentation. Les couvrir maintenant, dans le même bloc `tests` :

```rust
    #[test]
    fn window_size_combine_les_deux_echelles() {
        let m = manifeste_de_test(); // scale = 1, frameSize = [128, 128]
        assert_eq!(window_size(&m, 1.0), (128, 128));
        assert_eq!(window_size(&m, 1.5), (192, 192));
    }

    #[test]
    fn hitbox_ecran_se_place_dans_la_fenetre() {
        // hitbox du manifeste : [40, 20, 48, 100], ancre stand : [64, 120].
        // Le coin de la fenêtre est donc à (300-64, 1032-120) = (236, 912),
        // et la hitbox à 40/20 de là.
        let m = manifeste_de_test();
        let pose = m.pose(POSE_STAND).unwrap();

        let r = hitbox_ecran(
            Point::new(300.0, 1032.0),
            POSE_STAND,
            pose,
            &m,
            1.0,
            Facing::Right,
        );
        assert_eq!(r, Rect::new(236.0 + 40.0, 912.0 + 20.0, 48.0, 100.0));
    }

    #[test]
    fn hitbox_ecran_est_miroitee_avec_le_sprite() {
        // La hitbox [40, 20, 48, 100] n'est pas centrée dans 128 : elle va de
        // 40 à 88, donc il reste 40 à droite. Retournée, elle doit aller de
        // 128-88 = 40 à 128-40 = 88 — ici, symétrique par coïncidence.
        // On prend donc une hitbox franchement décentrée pour que le test
        // prouve quelque chose.
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128, 128], "scale": 1,
            "hitbox": [10, 20, 30, 100],
            "poses": { "stand": { "frames": [1] } }
        }"#;
        let m: Manifest = serde_json::from_str(json).unwrap();
        let pose = m.pose(POSE_STAND).unwrap();

        let droite = hitbox_ecran(Point::new(500.0, 1000.0), POSE_STAND, pose, &m, 1.0, Facing::Right);
        let gauche = hitbox_ecran(Point::new(500.0, 1000.0), POSE_STAND, pose, &m, 1.0, Facing::Left);

        // Vers la droite, la hitbox commence à 10 dans la boîte ; vers la
        // gauche, elle commence à 128 - (10 + 30) = 88.
        assert_eq!(droite.w, gauche.w);
        assert_ne!(droite.x, gauche.x);
        assert_eq!(gauche.x - droite.x, 88.0 - 10.0);
    }
```

Il faut alors importer `POSE_STAND` et `Rect` dans le bloc de tests :

```rust
    use super::*;
    use crate::character::manifest::POSE_STAND;
    use crate::geom::Rect;
```

- [ ] **Step 5 : Lancer les tests et les voir passer**

Ajouter `pub mod attach;` dans `character/mod.rs`.

```powershell
cd src-tauri
cargo test
```

Attendu : `61 passed` — 47 des tâches précédentes, 14 ici.

**Si `deplacer_la_plateforme_deplace_le_personnage_sans_code` échoue**, arrêter et relire
`world_position` : c'est le test qui garde la décision n° 1. La seule façon de le faire
échouer est d'avoir mémorisé une position quelque part.

- [ ] **Step 6 : Commit**

```bash
git add src-tauri/src/character
git commit -m "feat(etape-1a): l'accroche et la position dérivée — décision n° 1

La tâche la plus importante du plan. La position d'un personnage accroché
n'est jamais stockée : elle est dérivée de sa plateforme à chaque image
(spec §6.2).

Ce que ces trente lignes rendent gratuit, sans une ligne de plus : plateforme
déplacée (il voyage avec), redimensionnée (il garde sa distance au bord),
rétrécie sous lui ou fermée (hors_bornes le dit). Un test verrouille chacun
des quatre cas — celui qui déplace l'écran sous une identité constante est
celui à ne jamais laisser échouer.

Dragged n'a AUCUNE donnée : sa position se dérive du curseur, comme On se
dérive de sa plateforme. Falling est le seul état à position absolue, ce qui
est cohérent — en chute, il n'est attaché à rien.

L'ancre est MIROITÉE avec le sprite plutôt que compensée après coup : c'est
le remplaçant principiel du bricolage asymétrique du prototype VSCode
(spec §8.3)."
```

---

## Tâche 6 : La chute et l'atterrissage

**Files:**
- Create: `src-tauri/src/character/physics.rs`
- Modify: `src-tauri/src/character/mod.rs` (ajouter `pub mod physics;`)

**Interfaces:**
- Consomme : `geom::{Face, Point, Vec2}`, `world::{PlatformId, World}`.
- Produit :
  - constantes `physics::GRAVITE`, `physics::VITESSE_CHUTE_MAX`,
    `physics::VITESSE_MARCHE`, `physics::VITESSE_COURSE`
  - `physics::integrer_chute(Point, Vec2, f32) -> (Point, Vec2)`
  - `physics::atterrissage(&World, Point, Point) -> Option<(PlatformId, f32)>`
  - `physics::sous_le_bureau(&World, Point) -> bool`

> Aucune notion de personnage ici non plus : ce sont des fonctions **pures** sur des
> points et des vitesses. C'est ce qui permet de tester une chute « hauteur et durée
> connues » (spec §10.1) sans instancier quoi que ce soit.

- [ ] **Step 1 : Écrire le test d'abord**

Créer `src-tauri/src/character/physics.rs` avec **seulement** ce bloc.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::fake::FakeProbe;
    use crate::probe::{ScreenInfo, SystemProbe};

    fn monde_deux_ecrans() -> World {
        World::from_screens(&FakeProbe::deux_ecrans().screens())
    }

    /// Un pas d'intégration à 60 Hz.
    const DT: f32 = 1.0 / 60.0;

    #[test]
    fn la_chute_accelere_vers_le_bas() {
        let (pos, vel) = integrer_chute(Point::new(100.0, 0.0), Vec2::zero(), DT);
        // y croît vers le bas : la vitesse et la position augmentent toutes
        // deux.
        assert!(vel.y > 0.0);
        assert!(pos.y > 0.0);
        // Aucune accélération horizontale : la gravité est verticale.
        assert_eq!(vel.x, 0.0);
        assert_eq!(pos.x, 100.0);
    }

    #[test]
    fn la_chute_conserve_la_vitesse_horizontale() {
        // Un personnage lâché en marchant garde son élan : c'est ce qui rend
        // le lâcher agréable plutôt que raide.
        let (pos, vel) = integrer_chute(Point::new(100.0, 0.0), Vec2::new(80.0, 0.0), DT);
        assert_eq!(vel.x, 80.0);
        assert!(pos.x > 100.0);
    }

    #[test]
    fn la_chute_est_deterministe_en_hauteur_et_en_duree() {
        // Le test que la spec §10.1 demande : « intégration déterministe,
        // hauteur et durée connues ».
        //
        // On intègre 1 seconde à 60 Hz depuis l'immobilité et on vérifie que
        // la distance parcourue est proche de ½·g·t² = 700 px. L'écart vient
        // du pas discret (Euler semi-implicite surestime légèrement), d'où
        // la tolérance.
        let mut pos = Point::new(0.0, 0.0);
        let mut vel = Vec2::zero();

        for _ in 0..60 {
            let (p, v) = integrer_chute(pos, vel, DT);
            pos = p;
            vel = v;
        }

        let theorique = 0.5 * GRAVITE * 1.0;
        assert!(
            (pos.y - theorique).abs() < theorique * 0.03,
            "chute de {} px, théorie {} px",
            pos.y,
            theorique
        );
    }

    #[test]
    fn la_vitesse_de_chute_est_plafonnee() {
        // Sans plafond, un personnage lâché très haut traverserait le sol
        // entre deux images : la détection d'atterrissage teste un segment,
        // mais un segment de 3 000 px enjamberait plusieurs plateformes et
        // rendrait le choix arbitraire.
        let mut vel = Vec2::new(0.0, 0.0);
        let mut pos = Point::new(0.0, 0.0);
        for _ in 0..600 {
            let (p, v) = integrer_chute(pos, vel, DT);
            pos = p;
            vel = v;
        }
        assert_eq!(vel.y, VITESSE_CHUTE_MAX);
    }

    #[test]
    fn atterrit_en_traversant_le_sol() {
        let monde = monde_deux_ecrans();
        // Le sol du premier écran est à y = 1032.
        let avant = Point::new(300.0, 1020.0);
        let apres = Point::new(300.0, 1040.0);

        let (id, offset) = atterrissage(&monde, avant, apres).expect("doit atterrir");
        assert_eq!(monde.get(id).unwrap().rect.top(), 1032.0);
        assert_eq!(offset, 300.0);
    }

    #[test]
    fn n_atterrit_pas_en_montant() {
        // Un personnage qui monte (lâché vers le haut, ou plus tard un saut)
        // ne doit pas s'accrocher au sol qu'il traverse par-dessous.
        let monde = monde_deux_ecrans();
        let avant = Point::new(300.0, 1040.0);
        let apres = Point::new(300.0, 1020.0);
        assert_eq!(atterrissage(&monde, avant, apres), None);
    }

    #[test]
    fn n_atterrit_pas_a_cote_de_la_plateforme() {
        // Entre les deux écrans il n'y a rien à x = 5000 : il continue de
        // tomber, et le garde-fou le récupérera.
        let monde = monde_deux_ecrans();
        let avant = Point::new(5000.0, 1020.0);
        let apres = Point::new(5000.0, 1040.0);
        assert_eq!(atterrissage(&monde, avant, apres), None);
    }

    #[test]
    fn atterrit_sur_le_sol_du_bon_ecran() {
        let monde = monde_deux_ecrans();
        let (id, offset) = atterrissage(
            &monde,
            Point::new(2500.0, 1020.0),
            Point::new(2500.0, 1040.0),
        )
        .expect("doit atterrir");

        let plat = monde.get(id).unwrap();
        assert_eq!(plat.rect.left(), 1920.0);
        // L'offset est relatif au bord GAUCHE de cette plateforme.
        assert_eq!(offset, 580.0);
    }

    #[test]
    fn atterrit_sur_la_plateforme_la_plus_haute_traversee() {
        // Deux faces traversées dans le même pas : il doit s'arrêter sur la
        // PREMIÈRE rencontrée en descendant, donc la plus haute (plus petit
        // y). À l'étape 4, ce sera le cas d'une barre de titre au-dessus du
        // sol — la règle est écrite maintenant pour ne pas avoir à y revenir.
        let monde = World::from_screens(&[
            ScreenInfo {
                id: 1,
                work_area: Rect::new(0.0, 0.0, 1920.0, 1032.0),
                scale: 1.0,
            },
            ScreenInfo {
                id: 2,
                // Un écran fictif dont la zone de travail finit plus haut :
                // son sol est donc à y = 600.
                work_area: Rect::new(0.0, 0.0, 1920.0, 600.0),
                scale: 1.0,
            },
        ]);

        let (id, _) = atterrissage(
            &monde,
            Point::new(300.0, 500.0),
            Point::new(300.0, 1100.0),
        )
        .expect("doit atterrir");

        assert_eq!(monde.get(id).unwrap().rect.top(), 600.0);
    }

    #[test]
    fn atterrit_pile_sur_la_ligne_du_sol() {
        // Cas limite : `apres.y` vaut exactement la hauteur du sol. Il doit
        // atterrir, pas passer à travers.
        let monde = monde_deux_ecrans();
        assert!(atterrissage(
            &monde,
            Point::new(300.0, 1000.0),
            Point::new(300.0, 1032.0)
        )
        .is_some());
    }

    #[test]
    fn sous_le_bureau_detecte_la_sortie_par_le_bas() {
        let monde = monde_deux_ecrans();
        assert!(!sous_le_bureau(&monde, Point::new(300.0, 500.0)));
        assert!(sous_le_bureau(&monde, Point::new(300.0, 5000.0)));
    }

    #[test]
    fn sous_le_bureau_est_faux_dans_un_monde_vide() {
        // Pas de plateforme, donc pas de bas du bureau : on ne peut pas être
        // « sous » quelque chose qui n'existe pas. Surtout, ça ne doit pas
        // paniquer.
        let monde = World::from_screens(&[]);
        assert!(!sous_le_bureau(&monde, Point::new(0.0, 99_999.0)));
    }
}
```

- [ ] **Step 2 : Lancer les tests et les voir échouer**

```powershell
cd src-tauri
cargo test physics
```

Attendu : **échec de compilation** — `integrer_chute`, `atterrissage`, `sous_le_bureau`,
`GRAVITE`, `VITESSE_CHUTE_MAX` n'existent pas.

- [ ] **Step 3 : Écrire l'implémentation, au-dessus des tests**

```rust
//! La physique : chute, atterrissage, et les vitesses de déplacement.
//!
//! Responsabilité unique : des **fonctions pures** sur des points et des
//! vitesses. Aucune notion de personnage, aucun état. C'est ce qui permet de
//! tester une chute « hauteur et durée connues » (spec §10.1) sans instancier
//! quoi que ce soit.
//!
//! Toutes les vitesses sont en **pixels physiques par seconde**, toutes les
//! accélérations en pixels par seconde carrée, et `y` croît **vers le bas**
//! (convention Windows, voir `geom::Rect`).

use crate::geom::{Face, Point, Vec2};
use crate::world::{PlatformId, World};

/// Accélération de la pesanteur.
///
/// Réglée à l'œil, pas dérivée du réel : à 9,81 m/s² et ~3 800 px/m, un
/// personnage tomberait de 1 000 px en 0,45 s — trop vif pour qu'on suive la
/// chute du regard. 1 400 px/s² donne ~1,2 s sur la même hauteur, ce qui se
/// regarde. Passera dans `config.json` au plan 1b.
pub const GRAVITE: f32 = 1400.0;

/// Vitesse de chute maximale.
///
/// Existe pour une raison technique, pas esthétique : la détection
/// d'atterrissage teste le **segment** parcouru dans une image. Sans plafond,
/// un personnage lâché très haut parcourrait plusieurs milliers de pixels par
/// image, enjamberait plusieurs plateformes d'un coup, et le choix de celle
/// sur laquelle atterrir deviendrait arbitraire.
///
/// À 60 Hz, 1 600 px/s vaut ~27 px par image : bien moins que la moindre
/// plateforme.
pub const VITESSE_CHUTE_MAX: f32 = 1600.0;

/// Vitesse de marche. Lente exprès : un pet qui se presse n'a pas l'air de
/// flâner.
pub const VITESSE_MARCHE: f32 = 55.0;

/// Vitesse de course. Le rapport à la marche (~2,7×) est ce qui rend le
/// passage de l'une à l'autre visible.
pub const VITESSE_COURSE: f32 = 150.0;

/// Un pas d'intégration de la chute libre.
///
/// **Euler semi-implicite** : on met à jour la vitesse *avant* la position.
/// C'est une ligne de différence avec Euler explicite, et c'est bien plus
/// stable — la trajectoire ne dérive pas quand le pas de temps varie un peu,
/// ce qui arrive dès que la machine est chargée.
///
/// Fonction pure : elle rend le nouvel état au lieu de modifier l'ancien.
/// C'est ce qui la rend testable en boucle dans un test, comme ci-dessus.
pub fn integrer_chute(pos: Point, vel: Vec2, dt: f32) -> (Point, Vec2) {
    // `min` et non `clamp` : seule la chute est plafonnée. Une vitesse
    // ascendante (personnage lâché vers le haut) n'a pas de raison de l'être.
    let vy = (vel.y + GRAVITE * dt).min(VITESSE_CHUTE_MAX);

    // La vitesse horizontale n'est pas amortie : le personnage garde son
    // élan. C'est ce qui rend le lâcher agréable plutôt que raide.
    let nouvelle_vel = Vec2::new(vel.x, vy);

    let nouvelle_pos = Point::new(pos.x + nouvelle_vel.x * dt, pos.y + nouvelle_vel.y * dt);

    (nouvelle_pos, nouvelle_vel)
}

/// Le personnage a-t-il touché une plateforme en passant de `avant` à
/// `apres` ?
///
/// Rend la plateforme et l'offset où se poser, ou `None` s'il continue de
/// tomber.
///
/// **Trois règles, et pas une de plus** :
///   1. on ne s'accroche qu'en **descendant** — sinon on s'agripperait au
///      sol qu'on traverse par-dessous ;
///   2. on retient la face **la plus haute** traversée — c'est la première
///      rencontrée en descendant ;
///   3. il faut être **au-dessus** de la face horizontalement.
///
/// La règle 2 ne sert à rien à l'étape 1 (les sols des écrans ne se
/// chevauchent pas) mais elle sera exactement ce qu'il faut à l'étape 4, où
/// une barre de titre flotte au-dessus du sol. L'écrire maintenant coûte deux
/// lignes ; l'oublier coûterait un diagnostic.
pub fn atterrissage(world: &World, avant: Point, apres: Point) -> Option<(PlatformId, f32)> {
    // Règle 1 : on descend ? `<=` et non `<` pour accepter le cas où le
    // personnage était déjà pile sur la ligne.
    if apres.y < avant.y {
        return None;
    }

    let mut meilleur: Option<(PlatformId, f32, f32)> = None; // (id, offset, y de la face)

    for plat in world.platforms() {
        if !plat.has_face(Face::Top) {
            continue;
        }

        let y_face = plat.rect.top();

        // A-t-on franchi cette ligne pendant le pas ?
        let franchie = avant.y <= y_face && apres.y >= y_face;
        if !franchie {
            continue;
        }

        // Règle 3 : est-on au-dessus de la face ?
        //
        // On teste avec `apres.x`, et non avec la position exacte du
        // croisement. C'est volontairement approximatif : à 27 px par image
        // au maximum, l'écart est invisible, et calculer l'intersection
        // exacte ajouterait de la trigonométrie pour rien. La navigation a
        // le droit d'être imparfaite (décision n° 4).
        if apres.x < plat.rect.left() || apres.x > plat.rect.right() {
            continue;
        }

        // Règle 2 : garder la face la plus haute, donc le plus petit `y`.
        let remplace = match meilleur {
            None => true,
            Some((_, _, y)) => y_face < y,
        };
        if remplace {
            meilleur = Some((plat.id, apres.x - plat.rect.left(), y_face));
        }
    }

    meilleur.map(|(id, offset, _)| (id, offset))
}

/// Le personnage est-il tombé sous le bas du bureau virtuel ?
///
/// C'est le déclencheur du **garde-fou** de la spec §6.3 : passé cette
/// limite, il est replacé sur le sol le plus proche plutôt que de tomber
/// indéfiniment. Sans ce garde-fou, lâcher un personnage à côté de l'écran
/// le perdrait pour de bon.
///
/// Rend `false` si le monde est vide : on ne peut pas être « sous » un bas
/// qui n'existe pas, et surtout il ne faut pas paniquer.
pub fn sous_le_bureau(world: &World, pos: Point) -> bool {
    // `let … else` : monde vide → pas de limite, donc pas de sortie.
    let Some(b) = world.bounds() else {
        return false;
    };

    // Une marge, pour que le garde-fou ne se déclenche pas pendant un
    // atterrissage normal juste au niveau du sol.
    const MARGE: f32 = 200.0;
    pos.y > b.bottom() + MARGE
}
```

- [ ] **Step 4 : Lancer les tests et les voir passer**

Ajouter `pub mod physics;` dans `character/mod.rs`. Le bloc de tests a besoin de `Rect` :

```rust
    use super::*;
    use crate::geom::Rect;
    use crate::probe::fake::FakeProbe;
    use crate::probe::{ScreenInfo, SystemProbe};
```

```powershell
cd src-tauri
cargo test
```

Attendu : `73 passed` — 61 des tâches précédentes, 12 ici.

- [ ] **Step 5 : Commit**

```bash
git add src-tauri/src/character
git commit -m "feat(etape-1a): la chute et l'atterrissage

Des fonctions pures sur des points et des vitesses, sans aucune notion de
personnage : c'est ce qui permet de tester une chute « hauteur et durée
connues » (spec §10.1) sans instancier quoi que ce soit.

Euler semi-implicite — la vitesse avant la position. Une ligne de différence
avec Euler explicite, et la trajectoire ne dérive plus quand le pas de temps
varie, ce qui arrive dès que la machine est chargée.

VITESSE_CHUTE_MAX existe pour une raison technique et non esthétique : la
détection d'atterrissage teste le segment parcouru dans une image. Sans
plafond, un lâcher depuis très haut enjamberait plusieurs plateformes et le
choix deviendrait arbitraire.

atterrissage retient la face la plus haute traversée. Inutile à cette étape
— les sols des écrans ne se chevauchent pas — mais exactement ce qu'il faut à
l'étape 4, où une barre de titre flotte au-dessus du sol. Deux lignes
maintenant contre un diagnostic plus tard."
```

---

## Tâche 7 : Le personnage et les réflexes — couche 1

**Files:**
- Modify: `src-tauri/src/character/mod.rs` (ajouter `Character`)
- Create: `src-tauri/src/behavior/mod.rs`
- Create: `src-tauri/src/behavior/reflex.rs`
- Modify: `src-tauri/src/main.rs` (ajouter `mod behavior;`)

**Interfaces:**
- Consomme : tout ce qui précède.
- Produit :
  - `character::Character { manifest, attachment, facing, pose, pose_depuis, pos_connue, intention }`
    avec `new(Manifest, Attachment, Point) -> Character`,
    `set_pose(&mut self, &str, Duration)`, `frame_courante(Duration) -> u32`,
    `pose_terminee(Duration) -> bool`
  - `behavior::Entrees { souris: Point, bouton_gauche: bool, curseur_sur_le_personnage: bool }`
  - `behavior::reflex::Reflexe { Chute, Porte, Atterrissage, Rattrape, Aucun }`
  - `behavior::reflex::appliquer(&mut Character, &World, &Entrees, Duration, f32) -> Reflexe`

> **Les réflexes ne sont pas des décisions, ce sont des conséquences physiques** — ils ne
> consultent ni l'envie ni l'intention (spec §7.1). Quand l'un d'eux s'impose, les couches
> 2 et 3 ne tournent pas du tout dans cette image.

- [ ] **Step 1 : Ajouter `Character` dans `character/mod.rs`**

À écrire sous `Facing`, avant le bloc de tests existant.

```rust
pub mod attach;
pub mod manifest;
pub mod physics;

use attach::Attachment;
use manifest::Manifest;
use crate::geom::Point;
use std::time::Duration;

/// L'état complet d'un personnage (spec §6.1).
pub struct Character {
    pub manifest: Manifest,
    pub attachment: Attachment,
    pub facing: Facing,

    /// Le nom de la pose courante — une clé du manifeste.
    ///
    /// Une `String` et non un `enum` : le vocabulaire de poses est de la
    /// DONNÉE (spec §8.2, décision n° 6). Un `enum` obligerait à recompiler
    /// pour qu'un pack tiers déclare une pose de plus.
    pub pose: String,

    /// Le moment (temps de l'horloge injectée) où la pose courante a
    /// commencé. C'est de là que `frame_courante` déduit l'image à afficher.
    pub pose_depuis: Duration,

    /// **La dernière position dérivée**, mise à jour à chaque image.
    ///
    /// ⚠️ **Ce n'est pas une entorse à la décision n° 1.** La source de
    /// vérité reste `attachment` ; ce champ n'est qu'un *cache de la dernière
    /// valeur calculée*, et il ne sert qu'à **un** endroit : amorcer une
    /// chute quand la plateforme vient de disparaître. À cet instant précis,
    /// `world_position` rend `None` — il n'y a plus rien dont dériver — et il
    /// faut bien un point d'où commencer à tomber.
    ///
    /// Ne jamais le lire ailleurs : toute lecture supplémentaire serait le
    /// premier pas vers la position stockée, et ramènerait les quatre bugs
    /// que la décision n° 1 supprime.
    pub pos_connue: Point,

    /// L'intention en cours. `None` = il faut en tirer une (couche 3).
    /// Le type arrive à la Tâche 8 ; ici le champ existe pour que les
    /// réflexes puissent l'annuler.
    pub intention: Option<crate::behavior::intention::ActiveIntention>,
}

impl Character {
    pub fn new(manifest: Manifest, attachment: Attachment, pos_connue: Point) -> Character {
        Character {
            manifest,
            attachment,
            facing: Facing::Right,
            pose: manifest::POSE_STAND.to_string(),
            pose_depuis: Duration::ZERO,
            pos_connue,
            intention: None,
        }
    }

    /// Change de pose — **et ne remet le chronomètre à zéro que si la pose
    /// change réellement**.
    ///
    /// C'est le détail qui fait toute la différence : appelée à 60 Hz avec le
    /// même nom, une version naïve redémarrerait l'animation à chaque image
    /// et le personnage resterait figé sur sa première frame. Bug typique,
    /// et difficile à voir puisque « ça affiche bien quelque chose ».
    ///
    /// **Une pose absente du manifeste est ignorée** : le personnage garde
    /// celle qu'il avait. C'est la couverture partielle appliquée aux
    /// RÉFLEXES (spec §8.6) — un pack sans `fall` doit quand même pouvoir
    /// tomber, il le fera dans sa pose courante. Les réflexes sont non
    /// négociables ; seul le tirage des envies se restreint (`desire.rs`).
    ///
    /// Sans ce garde, `ch.pose` désignerait une clé inexistante et
    /// `frame_courante` se rabattrait sur la frame 1 — le personnage
    /// changerait d'apparence sans raison visible. C'est le test
    /// `un_personnage_sans_pose_fall_tombe_quand_meme` qui l'exige.
    pub fn set_pose(&mut self, nom: &str, maintenant: Duration) {
        if !self.manifest.has_pose(nom) {
            return;
        }
        if self.pose != nom {
            self.pose = nom.to_string();
            self.pose_depuis = maintenant;
        }
    }

    /// L'image à afficher maintenant.
    ///
    /// Si la pose courante a disparu du manifeste (personnage rechargé à
    /// chaud au plan 1b avec un JSON amputé), on rend la frame 1 plutôt que
    /// de paniquer : un personnage figé sur une mauvaise image se voit et se
    /// corrige, un plantage perd la session.
    pub fn frame_courante(&self, maintenant: Duration) -> u32 {
        match self.manifest.pose(&self.pose) {
            Some(p) => p.frame_a(maintenant.saturating_sub(self.pose_depuis)),
            None => 1,
        }
    }

    /// La séquence de la pose courante est-elle arrivée à son terme ?
    ///
    /// Toujours `false` pour une pose en boucle : une boucle ne se termine
    /// pas. Sert à enchaîner après un atterrissage (la pose `land` finie, on
    /// repasse à `stand`).
    pub fn pose_terminee(&self, maintenant: Duration) -> bool {
        match self.manifest.pose(&self.pose) {
            Some(p) if !p.looping => {
                maintenant.saturating_sub(self.pose_depuis) >= p.duree_totale()
            }
            _ => false,
        }
    }
}
```

> ⚠️ **`Character::new` telle qu'écrite ne compile pas** : `pose:
> manifest::POSE_STAND.to_string()` est évalué après que `manifest` a été déplacé dans la
> structure. Le compilateur le signalera (`borrow of moved value`). C'est une erreur
> d'emprunt classique et instructive — la corriger en calculant la pose **avant** de
> construire :
>
> ```rust
> pub fn new(manifest: Manifest, attachment: Attachment, pos_connue: Point) -> Character {
>     // Calculé AVANT le déplacement de `manifest` dans la structure.
>     // `POSE_STAND` est une constante du module, pas un champ de `manifest` :
>     // le préfixe `manifest::` désigne le MODULE, homonyme du paramètre.
>     // Renommer le paramètre lève l'ambiguïté à la lecture.
>     let pose = manifest::POSE_STAND.to_string();
>     Character {
>         manifest,
>         attachment,
>         facing: Facing::Right,
>         pose,
>         pose_depuis: Duration::ZERO,
>         pos_connue,
>         intention: None,
>     }
> }
> ```
>
> En réalité `POSE_STAND` étant une constante de module, l'ordre n'a pas d'importance pour
> le compilateur — mais le module `manifest` et le paramètre `manifest` sont homonymes, ce
> qui rend la ligne pénible à relire. **Écrire la version ci-dessus.**

- [ ] **Step 2 : Écrire `src-tauri/src/behavior/mod.rs`**

```rust
//! Le comportement, en trois couches qui ne communiquent que vers le bas
//! (décision n° 5, spec §7.1).
//!
//!   1. RÉFLEXES   — non négociables, 60 Hz  → `reflex.rs`
//!   2. INTENTION  — une seule à la fois     → `intention.rs`
//!   3. ENVIE      — tirage pondéré          → `desire.rs`
//!
//! Responsabilité de ce fichier : les types partagés par les trois couches,
//! et l'enchaînement lui-même (`pas`, Tâche 8).

pub mod desire;
pub mod intention;
pub mod reflex;

use crate::geom::Point;

/// Ce que le monde extérieur dit au personnage à cette image.
///
/// Regroupé dans une structure plutôt que passé en trois paramètres : à
/// l'étape 2 s'y ajouteront l'inactivité, l'heure et la batterie, et les
/// signatures des trois couches n'auront pas à changer. C'est le pendant, du
/// côté des entrées, de « ajouter un signal = ajouter une ligne ».
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entrees {
    pub souris: Point,
    pub bouton_gauche: bool,

    /// Le curseur est-il dans la **hitbox de la pose courante** ?
    ///
    /// Calculé par l'appelant (Tâche 11) et non ici : la hitbox dépend de la
    /// pose et de l'échelle de l'écran, que le hit-testing connaît déjà.
    /// Le passer tout cuit garde les réflexes purs et testables sans
    /// manifeste.
    pub curseur_sur_le_personnage: bool,
}
```

- [ ] **Step 3 : Écrire le test des réflexes**

Créer `src-tauri/src/behavior/reflex.rs` avec **seulement** ce bloc.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::attach::Attachment;
    use crate::character::manifest::{Manifest, POSE_FALL, POSE_LAND, POSE_STAND, POSE_DRAGGED};
    use crate::character::Character;
    use crate::geom::{Face, Point, Vec2};
    use crate::probe::fake::FakeProbe;
    use crate::probe::SystemProbe;
    use crate::world::{PlatformId, World};
    use std::time::Duration;

    const DT: f32 = 1.0 / 60.0;

    fn manifeste() -> Manifest {
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40, 20, 48, 100],
            "poses": {
                "stand":   { "frames": [1] },
                "walk":    { "frames": [2, 3], "frameMs": 120, "loop": true },
                "run":     { "frames": [4, 5], "frameMs": 80,  "loop": true },
                "sit":     { "frames": [6] },
                "fall":    { "frames": [7], "anchor": [64, 64] },
                "land":    { "frames": [8], "frameMs": 150 },
                "dragged": { "frames": [7], "anchor": [64, 64] }
            }
        }"#;
        serde_json::from_str(json).unwrap()
    }

    fn monde() -> World {
        World::from_screens(&FakeProbe::deux_ecrans().screens())
    }

    fn sol(monde: &World) -> PlatformId {
        monde.platforms()[0].id
    }

    fn perso_pose_sur_le_sol(monde: &World) -> Character {
        Character::new(
            manifeste(),
            Attachment::On {
                platform: sol(monde),
                face: Face::Top,
                offset: 300.0,
            },
            Point::new(300.0, 1032.0),
        )
    }

    fn entrees_neutres() -> Entrees {
        Entrees {
            souris: Point::new(0.0, 0.0),
            bouton_gauche: false,
            curseur_sur_le_personnage: false,
        }
    }

    #[test]
    fn pose_sur_le_sol_aucun_reflexe_ne_s_impose() {
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);
        assert_eq!(r, Reflexe::Aucun);
    }

    #[test]
    fn plateforme_disparue_il_tombe() {
        // Le réflexe n° 1. On lui donne une plateforme qui n'existe pas —
        // c'est l'équivalent exact d'une fenêtre qu'on ferme (étape 4).
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::On {
            platform: PlatformId(999_999),
            face: Face::Top,
            offset: 300.0,
        };

        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Chute);
        assert!(matches!(ch.attachment, Attachment::Falling { .. }));
        assert_eq!(ch.pose, POSE_FALL);
    }

    #[test]
    fn la_chute_amorcee_part_de_la_derniere_position_connue() {
        // C'est le seul usage légitime de `pos_connue` : sans lui, on ne
        // saurait pas d'où commencer à tomber.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.pos_connue = Point::new(742.0, 1032.0);
        ch.attachment = Attachment::On {
            platform: PlatformId(999_999),
            face: Face::Top,
            offset: 300.0,
        };

        appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        match ch.attachment {
            Attachment::Falling { pos, vel } => {
                assert_eq!(pos.x, 742.0);
                // Elle commence sans vitesse verticale : il ne saute pas, le
                // sol se dérobe.
                assert_eq!(vel.y, 0.0);
            }
            autre => panic!("attendu Falling, obtenu {autre:?}"),
        }
    }

    #[test]
    fn offset_hors_bornes_il_tombe() {
        // La plateforme a rétréci sous lui. Même réflexe, même code.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::On {
            platform: sol(&m),
            face: Face::Top,
            offset: 99_999.0,
        };

        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);
        assert_eq!(r, Reflexe::Chute);
    }

    #[test]
    fn curseur_dans_la_hitbox_et_bouton_enfonce_il_est_attrape() {
        // Le réflexe n° 2.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);

        let e = Entrees {
            souris: Point::new(300.0, 1000.0),
            bouton_gauche: true,
            curseur_sur_le_personnage: true,
        };

        let r = appliquer(&mut ch, &m, &e, Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Porte);
        assert_eq!(ch.attachment, Attachment::Dragged);
        assert_eq!(ch.pose, POSE_DRAGGED);
    }

    #[test]
    fn le_bouton_seul_ne_suffit_pas_a_l_attraper() {
        // Sinon tout clic n'importe où sur le bureau l'arracherait.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        let e = Entrees {
            souris: Point::new(50.0, 50.0),
            bouton_gauche: true,
            curseur_sur_le_personnage: false,
        };
        assert_eq!(appliquer(&mut ch, &m, &e, Duration::ZERO, DT), Reflexe::Aucun);
    }

    #[test]
    fn etre_attrape_annule_l_intention_en_cours() {
        // Les réflexes ne consultent pas l'intention, mais ils l'INVALIDENT :
        // reprendre une promenade après avoir été soulevé n'aurait aucun sens
        // (spec §7.1).
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.intention = Some(crate::behavior::intention::ActiveIntention::nouvelle(
            crate::behavior::intention::Intention::Flaner,
            Duration::ZERO,
        ));

        let e = Entrees {
            souris: Point::new(300.0, 1000.0),
            bouton_gauche: true,
            curseur_sur_le_personnage: true,
        };
        appliquer(&mut ch, &m, &e, Duration::ZERO, DT);

        assert!(ch.intention.is_none());
    }

    #[test]
    fn relacher_le_bouton_le_fait_tomber_depuis_le_curseur() {
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;

        let e = Entrees {
            souris: Point::new(1200.0, 400.0),
            bouton_gauche: false,
            curseur_sur_le_personnage: true,
        };
        let r = appliquer(&mut ch, &m, &e, Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Chute);
        match ch.attachment {
            Attachment::Falling { pos, .. } => assert_eq!(pos, Point::new(1200.0, 400.0)),
            autre => panic!("attendu Falling, obtenu {autre:?}"),
        }
    }

    #[test]
    fn porte_il_reste_porte_tant_que_le_bouton_est_enfonce() {
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;

        let e = Entrees {
            souris: Point::new(800.0, 300.0),
            bouton_gauche: true,
            curseur_sur_le_personnage: false, // il a glissé sous le curseur
        };
        let r = appliquer(&mut ch, &m, &e, Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Porte);
        assert_eq!(ch.attachment, Attachment::Dragged);
    }

    #[test]
    fn en_chute_il_avance_puis_atterrit() {
        // Le réflexe n° 3. On le lâche juste au-dessus du sol et on itère
        // jusqu'à l'atterrissage — au plus 2 secondes de temps simulé.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Falling {
            pos: Point::new(300.0, 900.0),
            vel: Vec2::zero(),
        };

        let mut t = Duration::ZERO;
        let mut atterri = false;
        for _ in 0..120 {
            let r = appliquer(&mut ch, &m, &entrees_neutres(), t, DT);
            if r == Reflexe::Atterrissage {
                atterri = true;
                break;
            }
            assert_eq!(r, Reflexe::Chute, "il devait encore tomber");
            t += Duration::from_micros(16_667);
        }

        assert!(atterri, "il n'a jamais atterri");
        assert_eq!(ch.pose, POSE_LAND);
        match ch.attachment {
            Attachment::On { platform, face, .. } => {
                assert_eq!(platform, sol(&m));
                assert_eq!(face, Face::Top);
            }
            autre => panic!("attendu On, obtenu {autre:?}"),
        }
    }

    #[test]
    fn tomber_sous_le_bureau_le_replace_sur_le_sol_le_plus_proche() {
        // Le garde-fou de la spec §6.3. Lâché très à droite, dans le vide
        // au-delà du second écran.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Falling {
            pos: Point::new(9_000.0, 50_000.0),
            vel: Vec2::new(0.0, 100.0),
        };

        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Rattrape);
        match ch.attachment {
            Attachment::On { platform, offset, .. } => {
                // Le sol du second écran, et un offset tenable.
                assert_eq!(m.get(platform).unwrap().rect.left(), 1920.0);
                assert!(offset >= 0.0 && offset <= 1920.0);
            }
            autre => panic!("attendu On, obtenu {autre:?}"),
        }
    }

    #[test]
    fn la_pose_land_finie_il_repasse_a_stand() {
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        // `land` dure 150 ms dans le manifeste de test.
        ch.set_pose(POSE_LAND, Duration::ZERO);

        // Juste avant la fin : il tient sa pose, et aucun réflexe ne
        // s'impose — mais l'atterrissage n'est pas « fini ».
        appliquer(&mut ch, &m, &entrees_neutres(), Duration::from_millis(100), DT);
        assert_eq!(ch.pose, POSE_LAND);

        appliquer(&mut ch, &m, &entrees_neutres(), Duration::from_millis(200), DT);
        assert_eq!(ch.pose, POSE_STAND);
    }

    #[test]
    fn pos_connue_est_tenue_a_jour_a_chaque_image() {
        // Sans quoi une chute amorcée partirait d'une position périmée.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.pos_connue = Point::new(0.0, 0.0);

        appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        assert_eq!(ch.pos_connue, Point::new(300.0, 1032.0));
    }

    #[test]
    fn un_personnage_sans_pose_fall_tombe_quand_meme() {
        // Couverture partielle (spec §8.6) : l'absence d'une pose ne doit
        // jamais empêcher un RÉFLEXE. Les réflexes sont non négociables ;
        // c'est le tirage des ENVIES qui se restreint (Tâche 8).
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "stand": { "frames": [1] } }
        }"#;
        let m = monde();
        let mut ch = Character::new(
            serde_json::from_str(json).unwrap(),
            Attachment::On {
                platform: PlatformId(999_999),
                face: Face::Top,
                offset: 10.0,
            },
            Point::new(300.0, 500.0),
        );

        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Chute);
        assert!(matches!(ch.attachment, Attachment::Falling { .. }));
        // La pose demandée n'existe pas : il garde celle qu'il avait plutôt
        // que d'afficher du vide.
        assert_eq!(ch.pose, POSE_STAND);
    }
}
```

- [ ] **Step 4 : Lancer les tests et les voir échouer**

```powershell
cd src-tauri
cargo test reflex
```

Attendu : **échec de compilation** — `appliquer`, `Reflexe`, ainsi que
`intention::ActiveIntention` et `intention::Intention`, qui n'arrivent qu'à la Tâche 8.

**Créer dès maintenant un `behavior/intention.rs` minimal** pour débloquer la
compilation — il sera complété à la Tâche 8 :

```rust
//! Couche 2 du comportement : l'intention en cours (spec §7.1).
//! Complété à la Tâche 8 ; ce qui suit est le strict nécessaire pour que les
//! réflexes puissent annuler une intention.

use std::time::Duration;

/// Ce que le personnage est en train d'essayer de faire. **Une seule à la
/// fois** (spec §7.1). `AllerA` et `Jouer` arrivent aux étapes 4 et 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intention {
    Flaner,
    SeReposer,
}

/// Une intention en cours, avec le moment où elle a commencé — c'est de là
/// que se déduit le délai d'abandon de 20 s (décision n° 4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveIntention {
    pub kind: Intention,
    pub depuis: Duration,
}

impl ActiveIntention {
    pub fn nouvelle(kind: Intention, maintenant: Duration) -> Self {
        ActiveIntention {
            kind,
            depuis: maintenant,
        }
    }
}
```

Et un `behavior/desire.rs` vide (`//! Couche 3 — Tâche 8.`), pour que
`pub mod desire;` compile.

- [ ] **Step 5 : Écrire `reflex.rs`, au-dessus de son bloc de tests**

```rust
//! Couche 1 du comportement : les réflexes (spec §7.1).
//!
//! **Ce ne sont pas des décisions, ce sont des conséquences physiques.** Ils
//! ne consultent ni l'envie ni l'intention — ils peuvent en revanche
//! l'annuler. Quand l'un d'eux s'impose, les couches 2 et 3 ne tournent pas
//! du tout dans cette image.
//!
//! Les quatre réflexes de la spec :
//!   · plateforme disparue   → je tombe          ✅ ici
//!   · attrapé à la souris   → je suis porté     ✅ ici
//!   · contact avec le sol   → j'atterris        ✅ ici
//!   · session verrouillée   → je disparais      ⬜ étape 2 (signal)
//!
//! Le garde-fou « tombé sous le bureau » s'y ajoute (spec §6.3). Il n'est pas
//! dans la liste de la spec parce que ce n'est pas un réflexe du personnage,
//! c'est un filet de sécurité du programme — mais sa place est ici, au même
//! rang de priorité.

use super::Entrees;
use crate::character::attach::{hors_bornes, world_position, Attachment};
use crate::character::manifest::{POSE_DRAGGED, POSE_FALL, POSE_LAND, POSE_STAND};
use crate::character::physics::{atterrissage, integrer_chute, sous_le_bureau};
use crate::character::Character;
use crate::geom::{Face, Vec2};
use crate::world::World;
use std::time::Duration;

/// Ce qui s'est imposé à cette image. `Aucun` laisse la main aux couches 2
/// et 3.
///
/// Rendu à l'appelant plutôt que gardé pour soi : le mode simulation
/// (Tâche 9) en fait sa trace, et c'est ce qui permet de vérifier « il n'a
/// jamais été bloqué » sans regarder l'écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reflexe {
    Chute,
    Porte,
    Atterrissage,
    /// Rattrapé par le garde-fou après être tombé sous le bureau.
    Rattrape,
    Aucun,
}

/// Applique les réflexes, dans l'ordre de priorité.
///
/// L'ordre compte, et il n'est pas arbitraire : « porté » passe avant
/// « plateforme disparue » parce qu'un personnage tenu à la main n'a pas de
/// plateforme à perdre. L'inverse le ferait tomber de la main de
/// l'utilisateur si la fenêtre sous lui se fermait.
pub fn appliquer(
    ch: &mut Character,
    world: &World,
    e: &Entrees,
    maintenant: Duration,
    dt: f32,
) -> Reflexe {
    // ── Tenir `pos_connue` à jour ───────────────────────────────────────
    // Avant tout le reste, et à chaque image : c'est de là que partira une
    // chute si la plateforme disparaît. Voir le commentaire du champ — c'est
    // un cache, pas une source de vérité.
    if let Some(p) = world_position(&ch.attachment, world, e.souris) {
        ch.pos_connue = p;
    }

    // ── Réflexe 2 : porté ───────────────────────────────────────────────
    // Traité en premier, voir la note sur l'ordre.
    match ch.attachment {
        Attachment::Dragged => {
            if e.bouton_gauche {
                // Toujours tenu. On ne teste PAS `curseur_sur_le_personnage`
                // ici : pendant un déplacement rapide, le sprite traîne
                // derrière le curseur et sortirait de sa propre hitbox — il
                // se décrocherait tout seul.
                ch.set_pose(POSE_DRAGGED, maintenant);
                return Reflexe::Porte;
            }

            // Relâché : il tombe, depuis le curseur et sans élan vertical.
            ch.attachment = Attachment::Falling {
                pos: e.souris,
                vel: Vec2::zero(),
            };
            ch.set_pose(POSE_FALL, maintenant);
            ch.intention = None;
            return Reflexe::Chute;
        }

        _ => {
            // Pas encore tenu : le devient-il ?
            if e.bouton_gauche && e.curseur_sur_le_personnage {
                ch.attachment = Attachment::Dragged;
                ch.set_pose(POSE_DRAGGED, maintenant);
                // Être soulevé annule ce qu'il était en train de faire :
                // reprendre une promenade après avoir été déplacé de deux
                // écrans n'aurait aucun sens.
                ch.intention = None;
                return Reflexe::Porte;
            }
        }
    }

    // ── Réflexe 1 : plateforme disparue ou trop courte ──────────────────
    if hors_bornes(&ch.attachment, world) {
        ch.attachment = Attachment::Falling {
            // Le seul usage de `pos_connue` : `world_position` rendrait
            // `None`, il n'y a plus rien dont dériver la position.
            pos: ch.pos_connue,
            vel: Vec2::zero(),
        };
        ch.set_pose(POSE_FALL, maintenant);
        ch.intention = None;
        return Reflexe::Chute;
    }

    // ── Réflexe 3 : la chute, et son issue ──────────────────────────────
    if let Attachment::Falling { pos, vel } = ch.attachment {
        // Le garde-fou d'abord : inutile de chercher un atterrissage à
        // 50 000 px sous le bureau.
        if sous_le_bureau(world, pos) {
            // `if let Some(...)` et non `unwrap` : un monde vide ne doit pas
            // faire paniquer. Dans ce cas on le laisse tomber — il n'y a
            // nulle part où le poser, et le monde reviendra.
            if let Some((platform, offset)) = world.nearest_floor(pos) {
                ch.attachment = Attachment::On {
                    platform,
                    face: Face::Top,
                    offset,
                };
                ch.set_pose(POSE_LAND, maintenant);
                ch.intention = None;
                return Reflexe::Rattrape;
            }
        }

        let (nouvelle_pos, nouvelle_vel) = integrer_chute(pos, vel, dt);

        // A-t-on traversé une face pendant ce pas ?
        if let Some((platform, offset)) = atterrissage(world, pos, nouvelle_pos) {
            ch.attachment = Attachment::On {
                platform,
                face: Face::Top,
                offset,
            };
            ch.set_pose(POSE_LAND, maintenant);
            ch.intention = None;
            return Reflexe::Atterrissage;
        }

        ch.attachment = Attachment::Falling {
            pos: nouvelle_pos,
            vel: nouvelle_vel,
        };
        ch.set_pose(POSE_FALL, maintenant);
        return Reflexe::Chute;
    }

    // ── Fin de la pose d'atterrissage ───────────────────────────────────
    // `land` n'est pas une intention : c'est la queue d'un réflexe. On la
    // laisse se jouer, puis on rend la main.
    if ch.pose == POSE_LAND {
        if ch.pose_terminee(maintenant) {
            ch.set_pose(POSE_STAND, maintenant);
        } else {
            // Toujours en train d'atterrir : les couches 2 et 3 attendent.
            return Reflexe::Atterrissage;
        }
    }

    Reflexe::Aucun
}
```

- [ ] **Step 6 : Lancer les tests et les voir passer**

Ajouter `mod behavior;` dans `main.rs`.

```powershell
cd src-tauri
cargo test
```

Attendu : `87 passed` — 73 des tâches précédentes, 14 ici.

**Si `la_pose_land_finie_il_repasse_a_stand` échoue** en restant sur `POSE_LAND`, vérifier
que `set_pose` ne remet pas le chronomètre à zéro quand la pose est inchangée : c'est le
piège annoncé au Step 1, et il se manifeste exactement ici.

- [ ] **Step 7 : Commit**

```bash
git add src-tauri/src/behavior src-tauri/src/character/mod.rs src-tauri/src/main.rs
git commit -m "feat(etape-1a): Character et les réflexes — couche 1

Les réflexes ne sont pas des décisions, ce sont des conséquences physiques :
ils ne consultent ni l'envie ni l'intention (spec §7.1). Ils l'annulent en
revanche — reprendre une promenade après avoir été soulevé de deux écrans
n'aurait aucun sens.

L'ordre de priorité n'est pas arbitraire : « porté » passe avant « plateforme
disparue », parce qu'un personnage tenu à la main n'a pas de plateforme à
perdre. L'inverse le ferait tomber de la main si la fenêtre sous lui se
fermait.

Character::set_pose ne remet le chronomètre à zéro que si la pose CHANGE
réellement. Appelée à 60 Hz avec le même nom, une version naïve redémarrerait
l'animation à chaque image et le personnage resterait figé sur sa première
frame — bug typique, et difficile à voir puisque ça affiche bien quelque
chose.

Character::pos_connue est un cache de la dernière position dérivée, pas une
entorse à la décision n° 1 : il ne sert qu'à amorcer une chute à l'instant où
la plateforme disparaît, moment où world_position rend None et où il n'y a
plus rien dont dériver.

Pendant un glisser rapide, on ne teste pas que le curseur est encore dans la
hitbox : le sprite traîne derrière le curseur et le personnage se
décrocherait tout seul."
```

---

## Tâche 8 : Les intentions et les envies — couches 2 et 3

**Files:**
- Modify: `src-tauri/src/behavior/intention.rs` (compléter le fichier minimal de la Tâche 7)
- Modify: `src-tauri/src/behavior/desire.rs` (remplacer le fichier vide)
- Modify: `src-tauri/src/behavior/mod.rs` (ajouter `pas`)

**Interfaces:**
- Consomme : tout ce qui précède, plus `rng::Rng` et `clock::Clock`.
- Produit :
  - `behavior::intention::Allure { Arret, Marche, Course }`
  - `behavior::intention::EtatIntention { Flanerie { allure, jusqu_a }, Repos { jusqu_a } }`
  - `behavior::intention::ActiveIntention { kind, depuis, etat }` avec
    `nouvelle(Intention, Duration) -> ActiveIntention`
  - `behavior::intention::DELAI_ABANDON: Duration`
  - `behavior::intention::Issue { EnCours, Finie, Echouee }`
  - `behavior::intention::poursuivre(&mut Character, &World, Duration, f32, &mut dyn Rng) -> Issue`
  - `behavior::desire::EntreeEnvie { intention, base, poses_requises }`
  - `behavior::desire::TableEnvies` avec `defaut() -> TableEnvies`,
    `tirer(&Manifest, &mut dyn Rng) -> Option<Intention>`,
    `tirer_avec(&Manifest, &mut dyn Rng, impl Fn(Intention) -> f32) -> Option<Intention>`
  - `behavior::pas(&mut Character, &World, &Entrees, &TableEnvies, Duration, f32, &mut dyn Rng) -> reflex::Reflexe`

> **Deux décisions vivent dans cette tâche, et ce sont les deux qui font le produit.**
>
> **Décision n° 3 — les signaux biaisent, ils ne commandent pas.** `tirer_avec` prend un
> multiplicateur par intention. À l'étape 1 il vaut toujours 1 ; à l'étape 2, « inactif
> 2 min » rendra 8 pour `SeReposer`. **La signature ne changera pas** : ajouter un signal
> sera ajouter une ligne à une table de multiplicateurs.
>
> **Décision n° 4 — la navigation est autorisée à échouer.** Aucun calcul de chemin, une
> décision locale par image, et un **délai d'abandon uniforme de 20 s** qui remplace toute
> l'énumération des cas de blocage.

- [ ] **Step 1 : Écrire le test des envies d'abord**

Dans `src-tauri/src/behavior/desire.rs`, **seulement** ce bloc.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::manifest::Manifest;
    use crate::rng::XorShift32;

    /// Un manifeste où l'on choisit les poses présentes, pour éprouver la
    /// couverture partielle sans toucher au disque.
    fn manifeste_avec(poses: &[&str]) -> Manifest {
        let corps: Vec<String> = poses
            .iter()
            .map(|p| format!(r#""{p}": {{ "frames": [1] }}"#))
            .collect();
        let json = format!(
            r#"{{ "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
                  "hitbox": [40,20,48,100], "poses": {{ {} }} }}"#,
            corps.join(",")
        );
        serde_json::from_str(&json).expect("manifeste de test valide")
    }

    fn compter(
        table: &TableEnvies,
        m: &Manifest,
        graine: u32,
        n: usize,
    ) -> (usize, usize, usize) {
        let mut rng = XorShift32::seeded(graine);
        let (mut flaner, mut reposer, mut rien) = (0, 0, 0);
        for _ in 0..n {
            match table.tirer(m, &mut rng) {
                Some(Intention::Flaner) => flaner += 1,
                Some(Intention::SeReposer) => reposer += 1,
                None => rien += 1,
            }
        }
        (flaner, reposer, rien)
    }

    #[test]
    fn la_table_par_defaut_privilegie_la_flanerie() {
        // Poids de la spec §7.2 : Flâner 5, Se reposer 1. Donc ~83 / 17 %.
        let m = manifeste_avec(&["stand", "walk", "sit"]);
        let (flaner, reposer, rien) = compter(&TableEnvies::defaut(), &m, 42, 10_000);

        assert_eq!(rien, 0);
        let pct = |n: usize| n as f32 / 100.0;
        assert!((pct(flaner) - 83.3).abs() < 2.0, "flâner {flaner}");
        assert!((pct(reposer) - 16.7).abs() < 2.0, "reposer {reposer}");
    }

    #[test]
    fn une_pose_manquante_retire_l_intention_du_tirage() {
        // **LE test de la couverture partielle appliquée au comportement**
        // (spec §8.6). Pas de `sit` → il ne se repose JAMAIS, et il n'y a
        // aucun cas particulier dans le code pour ça.
        let m = manifeste_avec(&["stand", "walk"]);
        let (flaner, reposer, rien) = compter(&TableEnvies::defaut(), &m, 42, 1_000);

        assert_eq!(reposer, 0, "il ne devrait jamais se reposer sans pose sit");
        assert_eq!(flaner, 1_000);
        assert_eq!(rien, 0);
    }

    #[test]
    fn sans_aucune_pose_jouable_le_tirage_rend_none() {
        // Un personnage qui n'a que `stand` ne peut ni flâner (pas de walk)
        // ni se reposer (pas de sit). `None` = « rien à faire », et
        // l'appelant le traite comme tel — ce n'est pas une erreur.
        let m = manifeste_avec(&["stand"]);
        let (_, _, rien) = compter(&TableEnvies::defaut(), &m, 42, 100);
        assert_eq!(rien, 100);
    }

    #[test]
    fn un_multiplicateur_biaise_sans_commander() {
        // **LE test de la décision n° 3.** On multiplie par 8 l'envie de se
        // reposer, comme le fera « inactif depuis 2 min » à l'étape 2.
        //
        // Poids : Flâner 5, Se reposer 1 × 8 = 8. Donc ~38 / 62 %.
        //
        // Le point n'est pas qu'il se repose : c'est qu'il flâne ENCORE
        // parfois. « Cette marge est le produit. » Un déclenchement
        // donnerait 0 / 100 et un personnage prévisible.
        let m = manifeste_avec(&["stand", "walk", "sit"]);
        let table = TableEnvies::defaut();
        let mut rng = XorShift32::seeded(7);

        let (mut flaner, mut reposer) = (0, 0);
        for _ in 0..10_000 {
            let mult = |i: Intention| match i {
                Intention::SeReposer => 8.0,
                _ => 1.0,
            };
            match table.tirer_avec(&m, &mut rng, mult) {
                Some(Intention::Flaner) => flaner += 1,
                Some(Intention::SeReposer) => reposer += 1,
                None => {}
            }
        }

        let pct = |n: usize| n as f32 / 100.0;
        assert!((pct(reposer) - 61.5).abs() < 2.0, "reposer {reposer}");
        // La marge : il flâne encore dans plus d'un tiers des cas.
        assert!(flaner > 3_000, "la marge a disparu : flâner {flaner}");
    }

    #[test]
    fn un_multiplicateur_nul_retire_l_option_completement() {
        // C'est ainsi qu'un signal pourra interdire une intention sans qu'on
        // ajoute un chemin de code (étape 2 : pas de sieste juste après
        // s'être réveillé, par exemple).
        let m = manifeste_avec(&["stand", "walk", "sit"]);
        let table = TableEnvies::defaut();
        let mut rng = XorShift32::seeded(7);

        for _ in 0..500 {
            let mult = |i: Intention| match i {
                Intention::SeReposer => 0.0,
                _ => 1.0,
            };
            assert_eq!(
                table.tirer_avec(&m, &mut rng, mult),
                Some(Intention::Flaner)
            );
        }
    }

    #[test]
    fn le_tirage_est_reproductible_a_graine_fixe() {
        // Ce qui rend la trace du mode simulation (Tâche 9) comparable d'une
        // exécution à l'autre.
        let m = manifeste_avec(&["stand", "walk", "sit"]);
        let t = TableEnvies::defaut();
        assert_eq!(compter(&t, &m, 999, 500), compter(&t, &m, 999, 500));
    }
}
```

- [ ] **Step 2 : Écrire `desire.rs`, au-dessus des tests**

```rust
//! Couche 3 du comportement : l'envie (spec §7.2, décision n° 3).
//!
//! Responsabilité unique : choisir la prochaine intention par **tirage
//! pondéré**, où les signaux modifient les poids et les poses manquantes
//! retirent les options.
//!
//! > Si « inactif 2 min » *déclenchait* le sommeil, on aurait un afficheur
//! > d'état système déguisé en personnage : parfaitement prévisible, abandonné
//! > en trois jours. « Inactif 2 min » **multiplie par 8** l'envie de dormir —
//! > il s'endort presque toujours, mais parfois il s'assoit, parfois il traîne
//! > encore. **Cette marge est le produit.**
//!
//! Trois propriétés découlent de cette forme, et ce sont elles qu'il faut
//! préserver :
//!   · ajouter un signal = ajouter une ligne, aucun nouveau chemin de code ;
//!   · un personnage à couverture partielle marche sans cas particulier ;
//!   · les poids étant de la donnée, on règle son caractère sans recompiler
//!     (le plan 1b les sortira dans `config.json`).

use super::intention::Intention;
use crate::character::manifest::{Manifest, POSE_SIT, POSE_WALK};
use crate::rng::Rng;

/// Une ligne de la table d'envies.
#[derive(Debug, Clone)]
pub struct EntreeEnvie {
    pub intention: Intention,

    /// Le poids de base, avant modification par les signaux (spec §7.2).
    pub base: f32,

    /// Les poses sans lesquelles cette intention est injouable.
    ///
    /// **C'est ici que la couverture partielle devient gratuite** (spec
    /// §8.6) : une intention dont une pose manque est retirée du tirage, et
    /// il n'y a aucun cas particulier ailleurs. Un personnage sans images
    /// d'escalade ne grimpera jamais, simplement parce que l'option n'est
    /// jamais tirée.
    ///
    /// `&'static [&'static str]` : ces listes sont écrites dans le code et
    /// vivent aussi longtemps que le programme, donc aucune allocation.
    pub poses_requises: &'static [&'static str],
}

/// La table complète. Un `Vec` et non un tableau de taille fixe : le plan 1b
/// la chargera depuis `config.json`.
#[derive(Debug, Clone)]
pub struct TableEnvies {
    pub entrees: Vec<EntreeEnvie>,
}

impl TableEnvies {
    /// Les valeurs de départ de la spec §7.2, restreintes aux deux intentions
    /// de l'étape 1a.
    ///
    /// Les quatre autres lignes de la spec — manger, aller à la fenêtre
    /// active, aller voir l'autre personnage, s'amuser — arrivent aux étapes
    /// 2, 3 et 5. **Ce seront littéralement des lignes de plus dans ce
    /// `vec!`.**
    pub fn defaut() -> TableEnvies {
        TableEnvies {
            entrees: vec![
                EntreeEnvie {
                    intention: Intention::Flaner,
                    base: 5.0,
                    // Flâner sans savoir marcher n'a pas de sens.
                    poses_requises: &[POSE_WALK],
                },
                EntreeEnvie {
                    intention: Intention::SeReposer,
                    base: 1.0,
                    poses_requises: &[POSE_SIT],
                },
            ],
        }
    }

    /// Tire une intention, sans aucun biais. C'est le cas de l'étape 1a :
    /// il n'y a pas encore de signaux.
    pub fn tirer(&self, manifest: &Manifest, rng: &mut dyn Rng) -> Option<Intention> {
        self.tirer_avec(manifest, rng, |_| 1.0)
    }

    /// Tire une intention en appliquant un multiplicateur par intention.
    ///
    /// **C'est le point d'entrée des signaux** (décision n° 3). À l'étape 2,
    /// l'appelant passera une fermeture qui consulte l'inactivité, l'heure et
    /// la batterie. **Cette signature ne changera pas** — c'est tout l'enjeu :
    /// ajouter un signal ne doit toucher ni ce fichier, ni les intentions,
    /// ni les réflexes.
    ///
    /// `impl Fn(Intention) -> f32` plutôt qu'un `&dyn Fn` : le compilateur
    /// peut alors intégrer la fermeture à l'appel, et l'écriture au point
    /// d'appel reste une simple lambda.
    pub fn tirer_avec(
        &self,
        manifest: &Manifest,
        rng: &mut dyn Rng,
        multiplicateur: impl Fn(Intention) -> f32,
    ) -> Option<Intention> {
        // Les poids, dans le même ordre que `self.entrees` — c'est ce qui
        // permet de remonter de l'index tiré à l'intention.
        let poids: Vec<f32> = self
            .entrees
            .iter()
            .map(|e| {
                // Couverture partielle : une pose manquante annule le poids.
                // `all` sur une liste vide rend `true`, donc une intention
                // sans exigence est toujours jouable — ce qui est correct.
                let jouable = e.poses_requises.iter().all(|p| manifest.has_pose(p));
                if !jouable {
                    return 0.0;
                }
                e.base * multiplicateur(e.intention)
            })
            .collect();

        // `weighted` rend `None` si tous les poids sont nuls — c'est le cas
        // d'un personnage dont aucune intention n'est jouable. Ce n'est pas
        // une erreur : l'appelant le traite comme « rien à faire » et
        // réessaiera à l'image suivante.
        let index = rng.weighted(&poids)?;
        Some(self.entrees[index].intention)
    }
}
```

- [ ] **Step 3 : Lancer les tests des envies et les voir passer**

```powershell
cd src-tauri
cargo test desire
```

Attendu : `6 passed`.

**Si `un_multiplicateur_biaise_sans_commander` échoue** avec `flaner` proche de 0, c'est
que le multiplicateur a été appliqué comme un filtre au lieu d'un facteur. C'est
exactement la décision n° 3 qui serait défaite : relire, ne pas assouplir le test.

- [ ] **Step 4 : Écrire le test des intentions**

Dans `src-tauri/src/behavior/intention.rs`, ajouter ce bloc **après** le contenu minimal
de la Tâche 7.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::attach::Attachment;
    use crate::character::manifest::{Manifest, POSE_RUN, POSE_SIT, POSE_STAND, POSE_WALK};
    use crate::character::Character;
    use crate::geom::{Face, Point};
    use crate::probe::fake::FakeProbe;
    use crate::probe::SystemProbe;
    use crate::rng::XorShift32;
    use crate::world::{PlatformId, World};

    const DT: f32 = 1.0 / 60.0;

    fn manifeste() -> Manifest {
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40, 20, 48, 100],
            "poses": {
                "stand": { "frames": [1] },
                "walk":  { "frames": [2, 3], "frameMs": 120, "loop": true },
                "run":   { "frames": [4, 5], "frameMs": 80,  "loop": true },
                "sit":   { "frames": [6] },
                "fall":  { "frames": [7], "anchor": [64, 64] },
                "land":  { "frames": [8], "frameMs": 150 }
            }
        }"#;
        serde_json::from_str(json).unwrap()
    }

    fn monde() -> World {
        World::from_screens(&FakeProbe::deux_ecrans().screens())
    }

    fn perso(monde: &World, offset: f32) -> Character {
        Character::new(
            manifeste(),
            Attachment::On {
                platform: monde.platforms()[0].id,
                face: Face::Top,
                offset,
            },
            Point::new(offset, 1032.0),
        )
    }

    fn offset_de(ch: &Character) -> f32 {
        match ch.attachment {
            Attachment::On { offset, .. } => offset,
            autre => panic!("attendu On, obtenu {autre:?}"),
        }
    }

    fn plateforme_de(ch: &Character) -> PlatformId {
        match ch.attachment {
            Attachment::On { platform, .. } => platform,
            autre => panic!("attendu On, obtenu {autre:?}"),
        }
    }

    #[test]
    fn flaner_finit_par_faire_avancer_le_personnage() {
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(3);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Flaner, Duration::ZERO));

        let depart = offset_de(&ch);
        let mut t = Duration::ZERO;
        // 3 secondes : assez pour qu'au moins une allure de marche soit
        // tirée, quelle que soit la graine.
        for _ in 0..180 {
            poursuivre(&mut ch, &m, t, DT, &mut rng);
            t += Duration::from_micros(16_667);
        }

        assert_ne!(offset_de(&ch), depart, "il n'a pas bougé en 3 s");
    }

    #[test]
    fn flaner_alterne_les_allures_et_les_poses() {
        // « jamais figé, jamais prévisible » : sur 30 s, les trois allures
        // doivent avoir été vues.
        let m = monde();
        let mut ch = perso(&m, 900.0);
        let mut rng = XorShift32::seeded(11);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Flaner, Duration::ZERO));

        let mut vues = std::collections::BTreeSet::new();
        let mut t = Duration::ZERO;
        for _ in 0..1_800 {
            poursuivre(&mut ch, &m, t, DT, &mut rng);
            vues.insert(ch.pose.clone());
            t += Duration::from_micros(16_667);
        }

        assert!(vues.contains(POSE_STAND), "jamais arrêté : {vues:?}");
        assert!(vues.contains(POSE_WALK), "jamais marché : {vues:?}");
        assert!(vues.contains(POSE_RUN), "jamais couru : {vues:?}");
    }

    #[test]
    fn arrive_au_bord_il_fait_demi_tour_plutot_que_de_tomber() {
        // Le sol du premier écran va de 0 à 1920. On le place à 3 px du bord
        // droit, tourné à droite, en marche forcée.
        let m = monde();
        let mut ch = perso(&m, 1917.0);
        ch.facing = crate::character::Facing::Right;
        let mut rng = XorShift32::seeded(5);
        ch.intention = Some(ActiveIntention {
            kind: Intention::Flaner,
            depuis: Duration::ZERO,
            etat: EtatIntention::Flanerie {
                allure: Allure::Marche,
                jusqu_a: Duration::from_secs(60),
            },
        });

        let mut t = Duration::ZERO;
        for _ in 0..30 {
            poursuivre(&mut ch, &m, t, DT, &mut rng);
            t += Duration::from_micros(16_667);
        }

        // Il est toujours accroché — il n'est pas tombé du bord du monde.
        assert!(matches!(ch.attachment, Attachment::On { .. }));
        // Et il repart vers la gauche.
        assert_eq!(ch.facing, crate::character::Facing::Left);
        assert!(offset_de(&ch) <= 1920.0);
    }

    #[test]
    fn il_passe_sur_l_ecran_voisin_quand_il_y_en_a_un() {
        // « il circule sur tous les écrans » (CLAUDE.md, étape 1). Le sol du
        // premier écran finit à x = 1920, celui du second y commence : les
        // deux faces sont adjointes, il doit enjamber la frontière.
        //
        // On force la marche vers la droite depuis tout près du bord.
        let m = monde();
        let mut ch = perso(&m, 1919.0);
        ch.facing = crate::character::Facing::Right;
        let mut rng = XorShift32::seeded(5);
        ch.intention = Some(ActiveIntention {
            kind: Intention::Flaner,
            depuis: Duration::ZERO,
            etat: EtatIntention::Flanerie {
                allure: Allure::Marche,
                jusqu_a: Duration::from_secs(60),
            },
        });

        let premier = m.platforms()[0].id;
        let mut t = Duration::ZERO;
        let mut passe = false;
        for _ in 0..60 {
            poursuivre(&mut ch, &m, t, DT, &mut rng);
            if plateforme_de(&ch) != premier {
                passe = true;
                break;
            }
            t += Duration::from_micros(16_667);
        }

        assert!(passe, "il n'a pas franchi la frontière entre les écrans");
        assert_eq!(m.get(plateforme_de(&ch)).unwrap().rect.left(), 1920.0);
        // Il entre par le bord gauche du sol voisin, donc à un offset petit.
        assert!(offset_de(&ch) < 50.0);
        // Et il continue dans le même sens : pas de demi-tour parasite.
        assert_eq!(ch.facing, crate::character::Facing::Right);
    }

    #[test]
    fn se_reposer_s_assoit_puis_se_termine() {
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::SeReposer,
            Duration::ZERO,
        ));

        // Première image : il s'assoit.
        let issue = poursuivre(&mut ch, &m, Duration::ZERO, DT, &mut rng);
        assert_eq!(issue, Issue::EnCours);
        assert_eq!(ch.pose, POSE_SIT);

        // Il ne bouge pas pendant le repos.
        let ou = offset_de(&ch);
        poursuivre(&mut ch, &m, Duration::from_secs(2), DT, &mut rng);
        assert_eq!(offset_de(&ch), ou);

        // Le repos dure au plus DELAI_ABANDON ; passé ce délai, l'intention
        // se termine d'une façon ou d'une autre.
        let issue = poursuivre(&mut ch, &m, Duration::from_secs(25), DT, &mut rng);
        assert_ne!(issue, Issue::EnCours);
    }

    #[test]
    fn toute_intention_expire_au_delai_d_abandon() {
        // **LE test de la décision n° 4.** Une seule règle remplace toute
        // l'énumération des cas de blocage : passé 20 s, l'intention échoue.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);

        for kind in [Intention::Flaner, Intention::SeReposer] {
            ch.intention = Some(ActiveIntention::nouvelle(kind, Duration::ZERO));

            // Juste avant le délai : toujours en cours.
            let avant = poursuivre(
                &mut ch,
                &m,
                DELAI_ABANDON - Duration::from_millis(100),
                DT,
                &mut rng,
            );
            assert_eq!(avant, Issue::EnCours, "{kind:?} a expiré trop tôt");

            // Juste après : expirée.
            let apres = poursuivre(
                &mut ch,
                &m,
                DELAI_ABANDON + Duration::from_millis(100),
                DT,
                &mut rng,
            );
            assert_eq!(apres, Issue::Echouee, "{kind:?} n'a pas expiré");
        }
    }

    #[test]
    fn le_delai_court_depuis_le_debut_de_l_intention_pas_depuis_zero() {
        // Une intention commencée à t = 100 s doit expirer à 120 s, pas
        // immédiatement.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::Flaner,
            Duration::from_secs(100),
        ));

        let issue = poursuivre(&mut ch, &m, Duration::from_secs(110), DT, &mut rng);
        assert_eq!(issue, Issue::EnCours);

        let issue = poursuivre(&mut ch, &m, Duration::from_secs(121), DT, &mut rng);
        assert_eq!(issue, Issue::Echouee);
    }

    #[test]
    fn sans_intention_poursuivre_ne_fait_rien_et_le_dit() {
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        assert_eq!(
            poursuivre(&mut ch, &m, Duration::ZERO, DT, &mut rng),
            Issue::Finie
        );
    }

    #[test]
    fn se_reposer_sans_pose_sit_echoue_au_lieu_de_figer() {
        // Défense en profondeur : le tirage ne devrait jamais proposer
        // `SeReposer` à un personnage sans `sit` (Tâche 8, desire.rs). Mais
        // si une config bricolée y parvenait, l'intention doit ÉCHOUER — pas
        // asseoir un personnage sur une pose inexistante.
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "stand": { "frames": [1] }, "walk": { "frames": [2] } }
        }"#;
        let m = monde();
        let mut ch = Character::new(
            serde_json::from_str(json).unwrap(),
            Attachment::On {
                platform: m.platforms()[0].id,
                face: Face::Top,
                offset: 500.0,
            },
            Point::new(500.0, 1032.0),
        );
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::SeReposer,
            Duration::ZERO,
        ));

        assert_eq!(
            poursuivre(&mut ch, &m, Duration::ZERO, DT, &mut rng),
            Issue::Echouee
        );
    }
}
```

- [ ] **Step 5 : Compléter `intention.rs`**

Remplacer le contenu minimal de la Tâche 7 par celui-ci, **en gardant `Intention` et
`ActiveIntention` compatibles** avec ce que `reflex.rs` utilise déjà.

```rust
//! Couche 2 du comportement : l'intention en cours (spec §7.1, §7.3).
//!
//! Responsabilité unique : traduire une intention en déplacement et en pose,
//! et dire quand elle est finie, échouée ou expirée. **Une seule intention à
//! la fois.**
//!
//! **Décision n° 4 — la navigation est autorisée à échouer.** Aucun calcul de
//! chemin : la carte des plateformes change 8 fois par seconde, un chemin est
//! périmé avant d'être parcouru. Décision **locale** à chaque image, et une
//! seule règle de sécurité qui remplace toute l'énumération des cas de
//! blocage :
//!
//! > **Toute intention a un délai d'abandon (~20 s)**, après quoi elle échoue
//! > et il repart flâner.
//!
//! Il se coince parfois, il prend des routes idiotes — **et c'est
//! souhaitable** : un pet qui prend un chemin bête est attachant, un qui
//! calcule l'itinéraire optimal a l'air d'un robot (spec §7.3).

use crate::character::attach::Attachment;
use crate::character::manifest::{POSE_RUN, POSE_SIT, POSE_STAND, POSE_WALK};
use crate::character::physics::{VITESSE_COURSE, VITESSE_MARCHE};
use crate::character::{Character, Facing};
use crate::geom::Face;
use crate::rng::Rng;
use crate::world::{PlatformId, World};
use std::time::Duration;

/// Le délai au bout duquel **toute** intention échoue (spec §7.3).
///
/// Uniforme, sans exception — y compris pour `Flaner`, qui pourrait durer
/// indéfiniment. L'exception serait une deuxième règle à retenir, et la
/// flânerie qui expire produit simplement un nouveau tirage : de la variété
/// gratuite.
pub const DELAI_ABANDON: Duration = Duration::from_secs(20);

/// Ce que le personnage est en train d'essayer de faire.
///
/// **Une seule à la fois** (spec §7.1). `AllerA(surface)` arrive à l'étape 5
/// et `Jouer(action)` à l'étape 2 — ce seront deux variantes de plus, et deux
/// lignes de plus dans la table d'envies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intention {
    Flaner,
    SeReposer,
}

/// À quelle vitesse il se déplace pendant une flânerie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allure {
    Arret,
    Marche,
    Course,
}

impl Allure {
    fn vitesse(&self) -> f32 {
        match self {
            Allure::Arret => 0.0,
            Allure::Marche => VITESSE_MARCHE,
            Allure::Course => VITESSE_COURSE,
        }
    }

    fn pose(&self) -> &'static str {
        match self {
            Allure::Arret => POSE_STAND,
            Allure::Marche => POSE_WALK,
            Allure::Course => POSE_RUN,
        }
    }
}

/// L'état interne d'une intention en cours.
///
/// Séparé de `Intention` : celle-ci est une **étiquette** (`Copy`, `Eq`),
/// utilisable comme clé dans la table d'envies, alors que celui-ci porte des
/// données qui changent à chaque image. Fondre les deux obligerait la table
/// d'envies à connaître des durées, ce qui n'a rien à y faire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EtatIntention {
    Flanerie { allure: Allure, jusqu_a: Duration },
    Repos { jusqu_a: Duration },
}

/// Une intention en cours, avec le moment où elle a commencé — c'est de là
/// que se déduit le délai d'abandon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveIntention {
    pub kind: Intention,
    pub depuis: Duration,
    pub etat: EtatIntention,
}

impl ActiveIntention {
    /// Crée une intention fraîche.
    ///
    /// L'état initial est délibérément **déjà périmé** (`jusqu_a` à zéro) :
    /// la première image de `poursuivre` tirera donc une allure ou une durée
    /// de repos. Cela évite d'exiger un `Rng` ici — ce qui permet à
    /// `reflex.rs` et aux tests d'en construire une sans générateur.
    pub fn nouvelle(kind: Intention, maintenant: Duration) -> Self {
        let etat = match kind {
            Intention::Flaner => EtatIntention::Flanerie {
                allure: Allure::Arret,
                jusqu_a: Duration::ZERO,
            },
            Intention::SeReposer => EtatIntention::Repos {
                jusqu_a: Duration::ZERO,
            },
        };
        ActiveIntention {
            kind,
            depuis: maintenant,
            etat,
        }
    }
}

/// Où en est l'intention à la fin de cette image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    EnCours,
    /// Menée à son terme normalement.
    Finie,
    /// Impossible, ou expirée au délai d'abandon. Dans les deux cas, la
    /// couche 3 en tirera une autre — c'est pourquoi les deux ne sont pas
    /// distinguées plus finement : rien n'en dépend.
    Echouee,
}

/// Fait avancer l'intention en cours d'un pas de temps.
///
/// Rend `Finie` s'il n'y a aucune intention : l'appelant (`behavior::pas`) en
/// tire alors une nouvelle. C'est plus simple qu'un `Option<Issue>`, et ça
/// évite un cas particulier au point d'appel.
pub fn poursuivre(
    ch: &mut Character,
    world: &World,
    maintenant: Duration,
    dt: f32,
    rng: &mut dyn Rng,
) -> Issue {
    // `let Some(...) else` : pas d'intention, rien à poursuivre.
    let Some(mut ai) = ch.intention else {
        return Issue::Finie;
    };

    // ── Le délai d'abandon, avant tout le reste ─────────────────────────
    // Décision n° 4. `saturating_sub` : si l'horloge de test recule (elle
    // le peut, `FakeClock::set` existe), on ne veut pas de débordement.
    if maintenant.saturating_sub(ai.depuis) > DELAI_ABANDON {
        ch.intention = None;
        return Issue::Echouee;
    }

    match ai.kind {
        Intention::Flaner => {
            let issue = flaner(ch, world, &mut ai, maintenant, dt, rng);
            // On réécrit l'intention : `ai` est une COPIE (le type est
            // `Copy`), donc modifier `ai.etat` ne touche pas `ch.intention`
            // tant qu'on ne le réaffecte pas. Oublier cette ligne donnerait
            // un personnage qui retire une allure à chaque image.
            ch.intention = Some(ai);
            issue
        }

        Intention::SeReposer => {
            let issue = se_reposer(ch, &mut ai, maintenant, rng);
            ch.intention = Some(ai);
            issue
        }
    }
}

/// Flâner : avancer, s'arrêter, courir, faire demi-tour, changer d'écran.
fn flaner(
    ch: &mut Character,
    world: &World,
    ai: &mut ActiveIntention,
    maintenant: Duration,
    dt: f32,
    rng: &mut dyn Rng,
) -> Issue {
    // On n'extrait l'état que sous la bonne variante. Une intention `Flaner`
    // avec un état `Repos` serait un bug de construction ; `else` le traite
    // comme un échec plutôt que par un `panic!`.
    let EtatIntention::Flanerie {
        mut allure,
        mut jusqu_a,
    } = ai.etat
    else {
        return Issue::Echouee;
    };

    // ── Renouveler l'allure quand la précédente a expiré ────────────────
    if maintenant >= jusqu_a {
        // Poids : il s'arrête souvent, marche beaucoup, court rarement.
        // C'est ce dosage qui donne l'impression de flânerie plutôt que
        // d'agitation. Passera dans `config.json` au plan 1b.
        let poids = [3.0, 6.0, 1.0];
        allure = match rng.weighted(&poids) {
            Some(0) => Allure::Arret,
            Some(2) => Allure::Course,
            // `Some(1)` et le cas `None` (impossible ici, les poids sont
            // constants et non nuls) tombent sur la marche.
            _ => Allure::Marche,
        };

        // Une durée aléatoire, plus courte pour la course : un pet qui court
        // dix secondes d'affilée a l'air pressé, pas vivant.
        let (min, max) = match allure {
            Allure::Arret => (0.8, 3.0),
            Allure::Marche => (1.5, 5.0),
            Allure::Course => (0.6, 1.8),
        };
        jusqu_a = maintenant + Duration::from_secs_f32(rng.range(min, max));

        // Un demi-tour de temps en temps, sans raison : c'est ce qui empêche
        // de deviner la suite.
        if rng.unit_f32() < 0.25 {
            ch.facing = ch.facing.inverse();
        }
    }

    // ── La pose suit l'allure ───────────────────────────────────────────
    // Si la pose de l'allure manque au manifeste, on se rabat sur `stand` :
    // un personnage sans `run` marchera au lieu de courir, plutôt que de ne
    // rien afficher (couverture partielle, spec §8.6).
    let pose_voulue = allure.pose();
    if ch.manifest.has_pose(pose_voulue) {
        ch.set_pose(pose_voulue, maintenant);
    } else if ch.manifest.has_pose(POSE_STAND) {
        ch.set_pose(POSE_STAND, maintenant);
    }

    // ── Le déplacement, et ce qui arrive au bord ────────────────────────
    if allure != Allure::Arret {
        avancer(ch, world, allure.vitesse() * ch.facing.signe() * dt, rng);
    }

    ai.etat = EtatIntention::Flanerie { allure, jusqu_a };
    Issue::EnCours
}

/// Avance de `pas` pixels le long de la face courante, et traite le bord.
///
/// **Décision locale, pas de plan** (décision n° 4) : au bord, on regarde
/// s'il existe une face voisine dans la direction du mouvement, et sinon on
/// fait demi-tour. Aucun itinéraire n'est calculé.
fn avancer(ch: &mut Character, world: &World, pas: f32, rng: &mut dyn Rng) {
    let Attachment::On {
        platform,
        face,
        offset,
    } = ch.attachment
    else {
        // En chute ou porté : ce n'est pas à l'intention de décider, les
        // réflexes s'en occupent.
        return;
    };

    let Some(plat) = world.get(platform) else {
        // La plateforme a disparu entre les réflexes et ici. Improbable dans
        // une même image, mais on ne suppose rien : les réflexes le verront
        // à l'image suivante.
        return;
    };

    let longueur = plat.rect.face_length(face);
    let nouveau = offset + pas;

    // Toujours dans la face : rien de spécial.
    if nouveau >= 0.0 && nouveau <= longueur {
        ch.attachment = Attachment::On {
            platform,
            face,
            offset: nouveau,
        };
        return;
    }

    // ── Le bord est atteint ─────────────────────────────────────────────
    let vers_la_droite = pas > 0.0;

    // Y a-t-il un sol voisin qui prolonge celui-ci de ce côté ? C'est ce qui
    // fait qu'« il circule sur tous les écrans » (étape 1) — et à l'étape 4,
    // ce sera aussi ce qui le fait passer d'une barre de titre à la suivante.
    if let Some((voisine, offset_entree)) =
        face_voisine(world, platform, plat.rect.top(), vers_la_droite)
    {
        ch.attachment = Attachment::On {
            platform: voisine,
            face: Face::Top,
            offset: offset_entree,
        };
        return;
    }

    // Pas de voisin : demi-tour, et on se recale exactement sur le bord.
    //
    // La spec §6.3 autorise aussi de se laisser tomber ou de s'accrocher à
    // une face voisine. À l'étape 1 il n'y a ni murs ni plafonds exposés, et
    // se laisser tomber du bord de l'écran ne mènerait qu'au garde-fou :
    // le demi-tour est la seule issue qui ait du sens ici. Le tirage entre
    // les trois arrive à l'étape 4.
    let _ = rng; // gardé pour ce tirage à venir
    ch.facing = ch.facing.inverse();
    ch.attachment = Attachment::On {
        platform,
        face,
        offset: offset.clamp(0.0, longueur),
    };
}

/// Cherche un sol adjacent à celui de `depuis`, du côté demandé et à peu près
/// à la même hauteur.
///
/// Vit ici et non dans `world.rs` parce que c'est une **décision de
/// navigation**, pas une propriété du monde : « ce sol en prolonge-t-il un
/// autre ? » n'a de sens que pour quelqu'un qui marche dessus.
///
/// Rend la plateforme voisine et l'offset auquel y entrer.
fn face_voisine(
    world: &World,
    depuis: PlatformId,
    hauteur: f32,
    vers_la_droite: bool,
) -> Option<(PlatformId, f32)> {
    /// Tolérance sur la jonction. Deux écrans côte à côte se touchent
    /// exactement, mais des résolutions ou des échelles différentes peuvent
    /// laisser quelques pixels : on ne veut pas d'un demi-tour inexpliqué
    /// pour 2 px.
    const TOLERANCE: f32 = 8.0;

    let source = world.get(depuis)?;

    for plat in world.platforms() {
        if plat.id == depuis || !plat.has_face(Face::Top) {
            continue;
        }

        // À peu près la même hauteur : on ne veut pas qu'il enjambe le vide
        // vers un sol 400 px plus bas.
        if (plat.rect.top() - hauteur).abs() > TOLERANCE {
            continue;
        }

        if vers_la_droite {
            // Le voisin commence là où celui-ci finit.
            if (plat.rect.left() - source.rect.right()).abs() <= TOLERANCE {
                return Some((plat.id, 0.0));
            }
        } else if (source.rect.left() - plat.rect.right()).abs() <= TOLERANCE {
            // On y entre par son bord droit.
            return Some((plat.id, plat.rect.face_length(Face::Top)));
        }
    }

    None
}

/// Se reposer : s'asseoir, et ne rien faire pendant un moment.
///
/// À l'étape 2, l'inactivité prolongée enchaînera sur `sleep` — ce sera une
/// pose de plus et une transition, pas un nouveau chemin de code.
fn se_reposer(
    ch: &mut Character,
    ai: &mut ActiveIntention,
    maintenant: Duration,
    rng: &mut dyn Rng,
) -> Issue {
    let EtatIntention::Repos { mut jusqu_a } = ai.etat else {
        return Issue::Echouee;
    };

    // Défense en profondeur : le tirage ne devrait jamais proposer cette
    // intention à un personnage sans `sit` (desire.rs le filtre). Mais une
    // config bricolée pourrait y parvenir, et un personnage assis sur une
    // pose inexistante serait invisible — mieux vaut échouer.
    if !ch.manifest.has_pose(POSE_SIT) {
        ch.intention = None;
        return Issue::Echouee;
    }

    if maintenant >= jusqu_a {
        // Première image de l'intention : on tire sa durée.
        //
        // Bornée sous `DELAI_ABANDON` : au-delà, le délai d'abandon
        // couperait le repos avant son terme et l'`Issue` serait `Echouee`
        // au lieu de `Finie`. Rien n'en dépend fonctionnellement, mais la
        // trace du mode simulation serait trompeuse.
        if jusqu_a == Duration::ZERO {
            jusqu_a = maintenant + Duration::from_secs_f32(rng.range(4.0, 15.0));
            ai.etat = EtatIntention::Repos { jusqu_a };
        } else {
            // Le repos est arrivé à son terme.
            ch.intention = None;
            return Issue::Finie;
        }
    }

    ch.set_pose(POSE_SIT, maintenant);
    Issue::EnCours
}
```

- [ ] **Step 6 : Ajouter l'enchaînement des trois couches dans `behavior/mod.rs`**

```rust
/// Un pas de comportement : les trois couches, dans l'ordre, une fois.
///
/// C'est la seule fonction que la boucle 60 Hz (Tâche 10) et le mode
/// simulation (Tâche 9) appellent. Les deux partagent donc **exactement** le
/// même comportement — c'est ce qui rend la simulation représentative.
///
/// Rend le réflexe qui s'est éventuellement imposé, pour la trace.
pub fn pas(
    ch: &mut crate::character::Character,
    world: &crate::world::World,
    e: &Entrees,
    table: &desire::TableEnvies,
    maintenant: std::time::Duration,
    dt: f32,
    rng: &mut dyn crate::rng::Rng,
) -> reflex::Reflexe {
    // ── Couche 1 : les réflexes ─────────────────────────────────────────
    // S'ils s'imposent, les couches 2 et 3 ne tournent pas du tout dans
    // cette image (spec §7.1).
    let r = reflex::appliquer(ch, world, e, maintenant, dt);
    if r != reflex::Reflexe::Aucun {
        return r;
    }

    // ── Couche 2 : poursuivre l'intention en cours ──────────────────────
    match intention::poursuivre(ch, world, maintenant, dt, rng) {
        intention::Issue::EnCours => return r,
        // Finie ou échouée : on passe à la couche 3.
        intention::Issue::Finie | intention::Issue::Echouee => {}
    }

    // ── Couche 3 : tirer une nouvelle envie ─────────────────────────────
    //
    // À l'étape 2, le `|_| 1.0` deviendra une fermeture qui consulte les
    // signaux. C'est le seul endroit à toucher — d'où « ajouter un signal =
    // ajouter une ligne » (décision n° 5).
    if let Some(kind) = table.tirer(&ch.manifest, rng) {
        ch.intention = Some(intention::ActiveIntention::nouvelle(kind, maintenant));
    }
    // `None` = aucune intention jouable (personnage très incomplet). On ne
    // fait rien : il reste dans sa pose, et on réessaiera à l'image
    // suivante. Ce n'est pas une erreur.

    r
}
```

- [ ] **Step 7 : Lancer toute la suite et la voir passer**

```powershell
cd src-tauri
cargo test
```

Attendu : `102 passed` — 87 des tâches précédentes, 6 de `desire`, 9 de `intention`.

**Si `il_passe_sur_l_ecran_voisin_quand_il_y_en_a_un` échoue**, c'est la seule exigence de
l'étape 1 qui serait perdue (« il circule sur tous les écrans »). Vérifier la tolérance de
`face_voisine` : les deux sols se touchent exactement à x = 1920 dans le monde de test.

- [ ] **Step 8 : Commit**

```bash
git add src-tauri/src/behavior
git commit -m "feat(etape-1a): les intentions et les envies — couches 2 et 3

Les deux décisions qui font le produit.

Décision n° 3 — les signaux biaisent, ils ne commandent pas. tirer_avec prend
un multiplicateur par intention ; à l'étape 2, « inactif 2 min » rendra 8
pour SeReposer et la signature ne changera pas. Un test vérifie qu'à ×8 il
flâne ENCORE dans plus d'un tiers des cas : c'est cette marge qui est le
produit, un déclenchement donnerait 0/100 et un personnage prévisible.

Décision n° 4 — la navigation est autorisée à échouer. Aucun calcul de
chemin, une décision locale par image, et un délai d'abandon uniforme de 20 s
qui remplace toute l'énumération des cas de blocage. Uniforme sans exception,
y compris pour Flaner : l'exception serait une règle de plus à retenir, et la
flânerie qui expire produit un nouveau tirage, donc de la variété gratuite.

Intention reste une étiquette Copy/Eq utilisable comme clé de la table
d'envies ; l'état qui change à chaque image vit dans EtatIntention. Fondre
les deux obligerait la table d'envies à connaître des durées.

face_voisine vit dans intention.rs et non dans world.rs : « ce sol en
prolonge-t-il un autre ? » est une décision de navigation, pas une propriété
du monde. C'est elle qui fait qu'il circule sur tous les écrans."
```

---

## Tâche 9 : Le mode simulation — vérifier sans écran

**Files:**
- Create: `src-tauri/src/sim.rs`
- Modify: `src-tauri/src/main.rs` (ajouter `mod sim;` et l'aiguillage en ligne de commande)

**Interfaces:**
- Consomme : `behavior::pas`, `world::World`, `character::Character`,
  `probe::fake::FakeProbe`, `clock::FakeClock`, `rng::XorShift32`.
- Produit :
  - `sim::Resume { images, intentions_tirees, reflexes, poses_vues, blocage_max }`
  - `sim::executer(minutes: u32, graine: u32, dossier: &Path) -> Result<Resume, String>`
  - `sim::imprimer(&Resume)`

> **Ce que ni les tests unitaires ni l'œil ne couvrent** (spec §10.3) : « il a flâné, il
> s'est reposé, il n'est jamais resté bloqué plus de 20 s » — sur des heures de temps
> simulé, en quelques secondes de calcul, et de façon reproductible.
>
> Le mode simulation appelle **exactement la même `behavior::pas`** que la boucle 60 Hz.
> C'est ce qui le rend représentatif : il n'y a pas deux comportements à maintenir.

- [ ] **Step 1 : Choisir la forme — un sous-commande, pas un second binaire**

Un second binaire (`src/bin/sim.rs`) **ne peut pas** utiliser les modules du premier : deux
cibles binaires d'un même paquet ne partagent du code que par une cible `lib`. Ajouter un
`lib.rs` uniquement pour ça réorganiserait tout le projet.

**Retenu : une sous-commande du binaire principal.**

```powershell
cargo run -- --sim 30           # 30 minutes de temps simulé, graine par défaut
cargo run -- --sim 30 --graine 7
```

C'est la raison pour laquelle le `Cargo.toml` de la Tâche 1 ne déclare **qu'un** binaire :
la décision est prise là-bas, elle est justifiée ici. Rien à modifier.

- [ ] **Step 2 : Écrire le test d'abord**

Dans `src-tauri/src/sim.rs`, **seulement** ce bloc.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// Le dossier du personnage de test, relatif à `src-tauri/` où cargo
    /// exécute les tests.
    fn blob() -> &'static Path {
        Path::new("../characters/blob")
    }

    #[test]
    fn une_simulation_courte_produit_un_resume_coherent() {
        let r = executer(2, 42, blob()).expect("la simulation doit aboutir");

        // 2 minutes à 60 Hz.
        assert_eq!(r.images, 2 * 60 * 60);
        assert!(r.intentions_tirees > 0, "aucune intention tirée");
    }

    #[test]
    fn il_flane_et_il_se_repose_sur_une_longue_duree() {
        // Ce que la spec §10.3 demande de pouvoir vérifier sans écran.
        // 30 minutes : assez pour que les deux intentions sortent, même avec
        // un rapport de poids de 5 contre 1.
        let r = executer(30, 42, blob()).expect("la simulation doit aboutir");

        assert!(r.poses_vues.contains("walk"), "il n'a jamais marché");
        assert!(r.poses_vues.contains("run"), "il n'a jamais couru");
        assert!(r.poses_vues.contains("stand"), "il ne s'est jamais arrêté");
        assert!(r.poses_vues.contains("sit"), "il ne s'est jamais reposé");
    }

    #[test]
    fn il_n_est_jamais_bloque_plus_que_le_delai_d_abandon() {
        // **La régression de comportement que rien d'autre ne détecte.**
        // « Bloqué » = la même intention et la même pose sans avoir bougé.
        let r = executer(30, 42, blob()).expect("la simulation doit aboutir");

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
        let a = executer(5, 999, blob()).unwrap();
        let b = executer(5, 999, blob()).unwrap();

        assert_eq!(a.images, b.images);
        assert_eq!(a.intentions_tirees, b.intentions_tirees);
        assert_eq!(a.poses_vues, b.poses_vues);
        assert_eq!(a.blocage_max, b.blocage_max);
    }

    #[test]
    fn deux_graines_donnent_deux_histoires() {
        // Sinon l'aléatoire ne sert à rien, et « jamais prévisible » est
        // faux.
        let a = executer(5, 1, blob()).unwrap();
        let b = executer(5, 2, blob()).unwrap();
        assert_ne!(a.intentions_tirees, b.intentions_tirees);
    }

    #[test]
    fn un_dossier_de_personnage_invalide_donne_une_erreur_lisible() {
        let e = executer(1, 1, Path::new("../characters/inexistant"))
            .expect_err("doit échouer");
        assert!(e.contains("mascot.json"), "message peu clair : {e}");
    }
}
```

- [ ] **Step 3 : Écrire `sim.rs`, au-dessus des tests**

```rust
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
    /// Combien de fois chaque réflexe s'est imposé — chutes, atterrissages,
    /// rattrapages.
    pub reflexes: u64,
    pub poses_vues: BTreeSet<String>,
    /// La plus longue période sans changer ni d'intention ni de position.
    /// **C'est le chiffre qui compte** : il doit rester sous le délai
    /// d'abandon (décision n° 4).
    pub blocage_max: Duration,
}

/// Déroule `minutes` de comportement et rend le résumé.
///
/// Rend `Err(String)` et non une erreur typée : le seul appelant est la
/// ligne de commande, qui va l'imprimer. Un `enum` d'erreurs n'apporterait
/// rien ici.
pub fn executer(minutes: u32, graine: u32, dossier: &Path) -> Result<Resume, String> {
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
    let table = TableEnvies::defaut();

    // La souris ne bouge pas et le bouton reste relâché : on simule le
    // comportement autonome, pas l'interaction. L'attrapage est vérifié par
    // les tests de `reflex.rs`, et à l'œil en Tâche 11.
    let entrees = Entrees {
        souris: Point::new(0.0, 0.0),
        bouton_gauche: false,
        curseur_sur_le_personnage: false,
    };

    // ── La boucle ───────────────────────────────────────────────────────
    let total_images = minutes as u64 * 60 * 60;

    let mut resume = Resume {
        images: 0,
        intentions_tirees: 0,
        reflexes: 0,
        poses_vues: BTreeSet::new(),
        blocage_max: Duration::ZERO,
    };

    // Pour la détection de blocage : ce qu'on observait au dernier
    // changement, et quand.
    let mut derniere_empreinte = (String::new(), 0i64);
    let mut depuis_changement = Duration::ZERO;

    let mut intention_precedente = None;

    for i in 0..total_images {
        let maintenant = Duration::from_secs_f64(i as f64 * DT as f64);

        let r = behavior::pas(&mut ch, &monde, &entrees, &table, maintenant, DT, &mut rng);

        if r != behavior::reflex::Reflexe::Aucun {
            resume.reflexes += 1;
        }

        // Une intention tirée = l'intention a changé d'identité.
        let intention_actuelle = ch.intention.map(|ai| ai.kind);
        if intention_actuelle != intention_precedente {
            if intention_actuelle.is_some() {
                resume.intentions_tirees += 1;
            }
            intention_precedente = intention_actuelle;
        }

        resume.poses_vues.insert(ch.pose.clone());

        // ── Détection de blocage ────────────────────────────────────────
        // L'empreinte : la pose, plus la position arrondie au pixel. On
        // arrondit parce qu'un flottant qui bouge de 1e-6 par image
        // ferait croire à un mouvement.
        let empreinte = (ch.pose.clone(), ch.pos_connue.x.round() as i64);
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

    Ok(resume)
}

/// Imprime le résumé, en français et lisible d'un coup d'œil.
pub fn imprimer(r: &Resume) {
    println!("── simulation ────────────────────────────────");
    println!("images            : {}", r.images);
    println!(
        "temps simulé      : {:.1} min",
        r.images as f64 * DT as f64 / 60.0
    );
    println!("intentions tirées : {}", r.intentions_tirees);
    println!("réflexes          : {}", r.reflexes);
    println!(
        "poses vues        : {}",
        r.poses_vues
            .iter()
            .cloned()
            .collect::<Vec<String>>()
            .join(", ")
    );
    println!("blocage le plus long : {:.1} s", r.blocage_max.as_secs_f32());

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
```

- [ ] **Step 4 : Aiguiller depuis `main.rs`**

```rust
mod behavior;
mod character;
mod clock;
mod geom;
mod probe;
mod render;
mod rng;
mod sim;
mod world;

fn main() {
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
```

`dossier_personnages` et `lancer_application` sont écrits à la Tâche 10. Pour compiler dès
maintenant, en écrire des versions provisoires :

```rust
fn dossier_personnages() -> std::path::PathBuf {
    // Version provisoire — la vraie résolution arrive en Tâche 10.
    std::path::PathBuf::from("../characters")
}

fn lancer_application() {
    println!("La fenêtre arrive en Tâche 10. En attendant : cargo run -- --sim 30");
}
```

- [ ] **Step 5 : Lancer les tests, puis la simulation**

```powershell
cd src-tauri
cargo test sim
```

Attendu : `6 passed`. Les deux tests de 30 minutes prennent quelques secondes chacun.

```powershell
cd src-tauri
cargo run -- --sim 30
```

Attendu, quelque chose comme :

```
── simulation ────────────────────────────────
images            : 108000
temps simulé      : 30.0 min
intentions tirées : ~150
réflexes          : 0
poses vues        : run, sit, stand, walk
blocage le plus long : ~15.0 s
✅ jamais bloqué au-delà du délai d'abandon
```

**Deux choses à regarder, et elles disent le caractère du personnage :**

| Observation | Ce qu'elle signifie |
|---|---|
| `réflexes : 0` | normal ici — sans interaction ni fenêtre qui se ferme, il ne tombe jamais |
| `blocage le plus long` proche de 20 s | il passe de longs moments assis ou arrêté. Si ça déplaît, ce sont les **poids** qu'on règle (`TableEnvies::defaut`, et les durées d'allure), pas le code |

- [ ] **Step 6 : Commit**

```bash
git add src-tauri/src/sim.rs src-tauri/src/main.rs
git commit -m "feat(etape-1a): le mode simulation, vérifier sans écran

Ce que ni les tests unitaires ni l'œil ne couvrent (spec §10.3) : « il a
flâné, il s'est reposé, il n'est jamais resté bloqué plus de 20 s », sur des
heures de temps simulé, en quelques secondes, et de façon reproductible.
C'est ce qui rend les régressions de comportement détectables — un réglage de
poids qui rendrait le personnage catatonique se verra ici avant d'être livré.

Une sous-commande (--sim) et non un second binaire : deux cibles binaires
d'un même paquet ne partagent du code que par une cible lib, et ajouter un
lib.rs pour ça seul réorganiserait tout le projet.

Le mode simulation appelle EXACTEMENT la même behavior::pas que la boucle
60 Hz. Il n'y a pas deux comportements à maintenir, et c'est ce qui le rend
représentatif."
```

---

## Tâche 10 : La fenêtre, le rendu, la boucle — **le moment visible**

**Files:**
- Create: `ui/index.html`
- Create: `ui/pet.js`
- Create: `src-tauri/src/render.rs`
- Modify: `src-tauri/src/main.rs` (`dossier_personnages`, `lancer_application`)

**Interfaces:**
- Consomme : tout ce qui précède.
- Produit :
  - `render::Rendu { image: u32, flip: bool }` — `Serialize`, `PartialEq`, `Copy`
  - `render::appliquer_styles_etendus(&WebviewWindow) -> Result<(), String>`
  - `render::pousser(&AppHandle, &str, Rendu) -> Result<(), String>`
  - `render::placer(&AppHandle, &str, Point, (u32, u32)) -> Result<(), String>`
  - `main::dossier_personnages() -> PathBuf`
  - `main::lancer_application()`
- Événement webview : `"frame"`, charge utile `{ "image": u32, "flip": bool }`

> **La tâche où le personnage devient visible.** Tout ce qui précède est vrai et testé ;
> ici on le regarde. C'est aussi la tâche qui applique les **deux découvertes de
> l'étape 0** — sans `WS_EX_NOACTIVATE`, la Tâche 11 volerait le focus.

- [ ] **Step 1 : Écrire `ui/index.html`**

`background: transparent` sur `html` **et** `body` : si l'un des deux garde un fond,
WebView2 peint un rectangle opaque et la transparence de la fenêtre ne se voit pas. C'est
la cause de faux négatifs la plus courante, et le spike de l'étape 0 l'a confirmée.

```html
<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>shimeji</title>
<style>
  /* Les deux doivent être transparents : voir la note du plan. */
  html, body {
    margin: 0;
    padding: 0;
    background: transparent;
    overflow: hidden;
  }

  #pet {
    /* La fenêtre est déjà à la bonne taille : le sprite la remplit. */
    width: 100%;
    height: 100%;
    /* Le pixel-art doit rester net : sans ça, la mise à l'échelle sur un
       écran HiDPI le rend flou. */
    image-rendering: pixelated;
    /* Aucune sélection, aucun glisser natif de l'image : c'est Rust qui
       gère le déplacement (spec §3.1). */
    user-select: none;
    -webkit-user-drag: none;
    pointer-events: none;
  }
</style>
</head>
<body>
  <img id="pet" alt="">
  <script src="pet.js"></script>
</body>
</html>
```

- [ ] **Step 2 : Écrire `ui/pet.js`**

**Délibérément bête** (spec §3.1) : il reçoit `{ image, flip }` et pose un `src` et une
transformation. Aucune physique, aucun état, aucune décision. Une trentaine de lignes.

```js
// Afficheur de sprite. Délibérément bête : il reçoit { image, flip } et
// n'en fait rien d'autre que l'afficher (spec §3.1).
//
// Toute la logique — physique, comportement, position de la fenêtre — vit en
// Rust. Faire calculer quoi que ce soit ici coûterait un aller-retour IPC
// 60 fois par seconde et par personnage.

const pet = document.getElementById('pet');

// Le personnage à afficher est passé dans le fragment de l'URL par Rust, à
// la création de la fenêtre : #blob. Un fragment plutôt qu'un paramètre de
// requête, pour ne pas interférer avec le chargement de la page.
const personnage = window.location.hash.slice(1) || 'blob';

// Les images viennent d'un schéma URI servi par Rust : les PNG sont des
// fichiers EXTERNES au binaire (spec §8.1), donc aucun chemin relatif ne
// peut les atteindre. Sur Windows, Tauri sert les schémas custom sous
// http://<scheme>.localhost.
const BASE = `http://shime.localhost/${personnage}/`;

// On précharge : sans ça, la première apparition de chaque pose clignote le
// temps du chargement. Les images restent dans le cache du webview.
const cache = new Map();
function urlDe(n) {
  if (!cache.has(n)) {
    const img = new Image();
    img.src = BASE + n;
    cache.set(n, img.src);
  }
  return cache.get(n);
}

let derniere = null;

// `window.__TAURI__` n'existe pas : `withGlobalTauri` n'est pas activé, et on
// ne veut pas de bundler. On écoute donc l'événement par le pont natif que
// Tauri installe dans chaque webview.
window.__TAURI_INTERNALS__.invoke('plugin:event|listen', {
  event: 'frame',
  target: { kind: 'Any' },
  handler: window.__TAURI_INTERNALS__.transformCallback((message) => {
    const { image, flip } = message.payload;

    // Ne toucher au DOM que si quelque chose a changé : Rust n'émet déjà
    // que sur changement, mais une double sécurité coûte deux comparaisons.
    if (derniere === null || derniere.image !== image) {
      pet.src = urlDe(image);
    }
    if (derniere === null || derniere.flip !== flip) {
      // Miroir horizontal : l'orientation n'a pas de frames dédiées
      // (spec §8.5).
      pet.style.transform = flip ? 'scaleX(-1)' : 'none';
    }
    derniere = { image, flip };
  }),
});
```

> ⚠️ **Le pont d'événements est le point fragile de ce fichier.** `window.__TAURI_INTERNALS__`
> est une interface interne, et son nom peut changer d'une version de Tauri à l'autre.
> **Si l'écoute ne fonctionne pas**, deux replis, dans cet ordre :
>
> 1. activer `"withGlobalTauri": true` dans `app` de `tauri.conf.json`, puis utiliser
>    `window.__TAURI__.event.listen('frame', cb)` — API publique, mais elle injecte un
>    script supplémentaire dans la page ;
> 2. se passer d'événements : faire pousser l'image par Rust en évaluant du JavaScript
>    dans le webview (`WebviewWindow::eval`), ce qui supprime tout code d'écoute côté
>    front. Moins élégant, mais **le plus robuste** — et cohérent avec « le webview est un
>    afficheur bête ».
>
> Vérifier ce point **au Step 7**, avant d'aller plus loin : c'est le seul endroit du plan
> dont l'API n'a pas pu être vérifiée dans les sources.

- [ ] **Step 3 : Écrire `src-tauri/src/render.rs`**

```rust
//! Le rendu : pousser la frame vers le webview, et déplacer la fenêtre.
//!
//! Responsabilité unique : la frontière entre l'état du personnage (calculé
//! en Rust) et son affichage. **Aucune décision** ici — pas de physique, pas
//! de comportement.
//!
//! Le motif `AppHandle` + recherche de la fenêtre par label est **retenu de
//! l'étape 0** : `WebviewWindow: Send` n'est pas garanti explicitement, et ce
//! motif gère en prime le cas de la fenêtre fermée. Un accès à une table de
//! hachage par image est négligeable.

use crate::geom::Point;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

/// Ce que le webview a besoin de savoir. **Deux champs, et pas un de plus.**
///
/// `PartialEq` : l'appelant compare avec la valeur précédente et n'émet que
/// sur changement. À 60 Hz, une pose de marche ne change d'image que ~8 fois
/// par seconde — on économise ainsi ~85 % des messages IPC, sans aucune
/// logique côté front.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Rendu {
    /// Le numéro de `shime<n>.png`.
    pub image: u32,
    /// Faut-il retourner le sprite horizontalement (spec §8.5) ?
    pub flip: bool,
}

/// Pose `WS_EX_NOACTIVATE` et `WS_EX_TOOLWINDOW` sur la fenêtre.
///
/// **Les deux découvertes de l'étape 0.** Tauri ne les pose pas :
///
/// · **`WS_EX_NOACTIVATE`** — sans lui, la Tâche 11 (qui réactive les clics
///   dans la hitbox) ferait qu'attraper le personnage **volerait le focus de
///   l'éditeur** en cours de frappe. `.focused(false)` de Tauri ne couvre pas
///   ce cas : il ne concerne que l'affichage initial, pas l'activation par
///   clic.
///
/// · **`WS_EX_TOOLWINDOW`** — `.skip_taskbar(true)` passe par
///   `ITaskbarList::DeleteTab`, qui retire de la barre des tâches ; l'exclusion
///   d'Alt+Tab n'en découle pas, Windows 11 l'accompagne par heuristique.
///
/// Signatures vérifiées : `WebviewWindow::hwnd`
/// (`tauri-2.11.5/src/webview/webview_window.rs:1847`),
/// `Get/SetWindowLongPtrW` (`windows-0.61.3/…/WindowsAndMessaging/mod.rs:1132`
/// et `:2262`).
pub fn appliquer_styles_etendus(win: &WebviewWindow) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };

    let hwnd = win.hwnd().map_err(|e| format!("hwnd indisponible : {e}"))?;

    // `unsafe` : ces deux appels franchissent la frontière FFI vers Win32.
    // Rust ne peut pas garantir que `hwnd` est un handle valide — c'est nous
    // qui l'affirmons, ce qui est légitime : il sort de `build()` juste
    // avant, et la fenêtre n'a pas pu être fermée entre-temps.
    unsafe {
        let actuels = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);

        // `.0` extrait le u32 du newtype `WINDOW_EX_STYLE` ; `as isize`
        // l'aligne sur le type de Get/SetWindowLongPtrW. Sans ces deux
        // conversions, le `|` ne compile pas.
        let ajout = (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) as isize;

        // `|` et non une affectation : on AJOUTE nos bits sans écraser ceux
        // que Tauri a posés (LAYERED, TOPMOST, TRANSPARENT). Écrire
        // `SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ajout)` retirerait la
        // transparence et le premier plan — les deux propriétés que
        // l'étape 0 vient d'établir.
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, actuels | ajout);
    }

    Ok(())
}

/// Envoie la frame au webview.
///
/// Rend `Err` si la fenêtre a disparu — l'appelant en déduit qu'il faut
/// arrêter la boucle de ce personnage.
pub fn pousser(app: &AppHandle, label: &str, r: Rendu) -> Result<(), String> {
    // `let … else` : la fenêtre a été fermée, il n'y a plus rien à afficher.
    let Some(win) = app.get_webview_window(label) else {
        return Err(format!("fenêtre « {label} » absente"));
    };

    // `emit` sur la fenêtre et non sur l'AppHandle : avec plusieurs
    // personnages, chacun ne doit recevoir que SA frame. Émettre depuis
    // l'AppHandle enverrait à tous les webviews, et chaque personnage
    // afficherait la pose du dernier émetteur.
    win.emit("frame", r).map_err(|e| format!("emit : {e}"))
}

/// Déplace et dimensionne la fenêtre.
///
/// La position est en **pixels physiques du bureau virtuel** (spec §3.4),
/// d'où `PhysicalPosition` et `PhysicalSize` — utiliser les variantes
/// logiques ferait dériver la position sur un écran non standard, et ce
/// genre de décalage ne se reproduit que sur un seul écran, ce qui est un
/// enfer à diagnostiquer.
pub fn placer(
    app: &AppHandle,
    label: &str,
    coin: Point,
    taille: (u32, u32),
) -> Result<(), String> {
    let Some(win) = app.get_webview_window(label) else {
        return Err(format!("fenêtre « {label} » absente"));
    };

    // `round()` avant la conversion : `as i32` tronque vers zéro, ce qui
    // décalerait d'un pixel la moitié du temps et donnerait un tremblement
    // visible sur du pixel-art.
    win.set_position(PhysicalPosition::new(
        coin.x.round() as i32,
        coin.y.round() as i32,
    ))
    .map_err(|e| format!("set_position : {e}"))?;

    win.set_size(PhysicalSize::new(taille.0, taille.1))
        .map_err(|e| format!("set_size : {e}"))
}

/// Active ou désactive la traversée des clics (spec §3.3).
///
/// Utilisé par la Tâche 11. Séparé de `placer` parce qu'il ne change que
/// lorsque le curseur entre ou sort de la hitbox, pas à chaque image.
pub fn traverser_les_clics(app: &AppHandle, label: &str, traverser: bool) -> Result<(), String> {
    let Some(win) = app.get_webview_window(label) else {
        return Err(format!("fenêtre « {label} » absente"));
    };
    win.set_ignore_cursor_events(traverser)
        .map_err(|e| format!("set_ignore_cursor_events : {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ce fichier n'a presque rien à tester : tout y est un appel à Tauri ou
    // à Win32. Le seul comportement propre est la comparaison de `Rendu`,
    // dont dépend l'économie de messages IPC — et une régression y serait
    // invisible à l'œil, puisque l'affichage resterait correct.

    #[test]
    fn deux_rendus_identiques_sont_egaux() {
        let a = Rendu { image: 3, flip: false };
        let b = Rendu { image: 3, flip: false };
        assert_eq!(a, b);
    }

    #[test]
    fn un_changement_de_flip_seul_rend_les_rendus_differents() {
        // Sans ce test, une implémentation qui ne comparerait que `image`
        // passerait : le personnage regarderait alors toujours du même côté.
        let a = Rendu { image: 3, flip: false };
        let b = Rendu { image: 3, flip: true };
        assert_ne!(a, b);
    }

    #[test]
    fn le_rendu_se_serialise_avec_les_noms_attendus_du_front() {
        // `pet.js` lit `message.payload.image` et `.flip`. Un renommage
        // côté Rust casserait l'affichage sans casser la compilation.
        let json = serde_json::to_string(&Rendu { image: 7, flip: true }).unwrap();
        assert_eq!(json, r#"{"image":7,"flip":true}"#);
    }
}
```

- [ ] **Step 4 : Écrire la résolution du dossier des personnages dans `main.rs`**

```rust
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
    // `if let Ok(...)` : `current_exe` peut échouer sur des systèmes exotiques.
    // Ce n'est pas une raison de ne pas démarrer.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidat = dir.join("characters");
            if candidat.is_dir() {
                return candidat;
            }

            // ── Repli de développement ─────────────────────────────────
            // `ancestors()` énumère le dossier puis chacun de ses parents.
            // On s'arrête à 5 niveaux : assez pour sortir de
            // target/debug/, pas assez pour partir explorer tout le disque.
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
```

- [ ] **Step 5 : Écrire `lancer_application` dans `main.rs`**

```rust
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
    let manifeste = match crate::character::manifest::Manifest::load(&dossier.join("blob")) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("personnage « blob » illisible : {e}");
            std::process::exit(1);
        }
    };
    println!("personnage chargé : {} ({} poses)", manifeste.name, manifeste.poses.len());

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
            let sonde = crate::probe::win32::Win32Probe::new();
            crate::probe::win32::imprimer_diagnostic(&sonde);

            let ecrans = sonde.screens();
            let monde = crate::world::World::from_screens(&ecrans);
            if monde.platforms().is_empty() {
                eprintln!("aucun écran : rien à faire.");
                return Ok(());
            }

            // ── Le personnage, posé sur le premier sol ──────────────────
            let sol = &monde.platforms()[0];
            let offset = sol.rect.face_length(crate::world::Face::Top) / 2.0;
            let depart = sol.rect.point_on(crate::world::Face::Top, offset);

            let echelle_ecran = ecrans[0].scale;
            let taille = crate::character::attach::window_size(&manifeste, echelle_ecran);

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
            if let Err(e) = crate::render::appliquer_styles_etendus(&win) {
                // Non bloquant : la fenêtre marche sans, elle est seulement
                // moins polie. Mieux vaut un personnage qui vole le focus
                // qu'aucun personnage.
                eprintln!("styles étendus non appliqués : {e}");
            }

            // ── Les horloges ────────────────────────────────────────────
            let handle = app.handle().clone();
            let personnage = crate::character::Character::new(
                manifeste,
                crate::character::attach::Attachment::On {
                    platform: sol.id,
                    face: crate::world::Face::Top,
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
fn servir_frame(
    dossier: &std::path::Path,
    chemin: &str,
) -> tauri::http::Response<Vec<u8>> {
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
    mut ch: crate::character::Character,
    mut monde: crate::world::World,
    mut echelle_ecran: f32,
) {
    use crate::behavior::{self, desire::TableEnvies, Entrees};
    use std::time::{Duration, Instant};

    let sonde = crate::probe::win32::Win32Probe::new();
    let horloge = crate::clock::SystemClock::new();

    // Graine issue de l'horloge système : deux lancements ne doivent pas
    // donner la même histoire. C'est le seul endroit du programme où
    // l'aléatoire n'est pas reproductible, et c'est voulu — le mode
    // simulation, lui, prend une graine explicite.
    let graine = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345);
    let mut rng = crate::rng::XorShift32::seeded(graine);

    let table = TableEnvies::defaut();

    const PERIODE: Duration = Duration::from_micros(16_667); // 60 Hz
    const PERIODE_MONDE: Duration = Duration::from_millis(125); // 8 Hz

    let mut dernier_recensement = Duration::ZERO;
    let mut dernier_rendu: Option<crate::render::Rendu> = None;

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
                monde = crate::world::World::from_screens(&ecrans);
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

        // ── 60 Hz : le rendu ────────────────────────────────────────────
        // La position est DÉRIVÉE à chaque image (décision n° 1).
        if let Some(pos) =
            crate::character::attach::world_position(&ch.attachment, &monde, m.pos)
        {
            if let Some(pose) = ch.manifest.pose(&ch.pose) {
                let coin = crate::character::attach::window_top_left(
                    pos,
                    pose,
                    &ch.manifest,
                    echelle_ecran,
                    ch.facing,
                );
                let taille =
                    crate::character::attach::window_size(&ch.manifest, echelle_ecran);

                if crate::render::placer(&handle, &label, coin, taille).is_err() {
                    // Fenêtre fermée : plus rien à faire dans ce thread.
                    return;
                }
            }
        }

        let rendu = crate::render::Rendu {
            image: ch.frame_courante(maintenant),
            flip: ch.facing.flipped(),
        };

        // N'émettre que sur changement : à 60 Hz, une pose de marche ne
        // change d'image que ~8 fois par seconde. On économise ~85 % des
        // messages, sans une ligne de logique côté front.
        if dernier_rendu != Some(rendu) {
            if crate::render::pousser(&handle, &label, rendu).is_err() {
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
```

- [ ] **Step 6 : Compiler**

```powershell
cd src-tauri
cargo build
```

Attendu : compilation réussie. Corriger les imports manquants que le compilateur signale —
ils sont explicites.

- [ ] **Step 7 : Vérifier le pont d'événements AVANT tout le reste**

C'est le seul point du plan dont l'API n'a pas pu être vérifiée dans les sources.

```powershell
cd src-tauri
cargo run
```

**Attendu : une silhouette blanche debout sur le bureau, au-dessus de la barre des tâches,
qui se met à marcher.**

| Symptôme | Diagnostic | Remède |
|---|---|---|
| rien ne s'affiche, mais la fenêtre existe | le pont d'événements ne fonctionne pas | appliquer le repli n° 1 puis n° 2 de la note du Step 2 |
| une image apparaît puis se fige | l'écoute marche, mais `pousser` n'est appelé qu'une fois | vérifier la comparaison `dernier_rendu` |
| carré blanc ou noir de 128 px | transparence perdue | `background: transparent` sur `html` **et** `body` |
| il marche **sous** la barre des tâches | c'est `rcMonitor` et non `rcWork` | Tâche 2, `probe/win32.rs` |
| il flotte au-dessus du sol | l'ancre du manifeste | corriger `anchor` dans `mascot.json` — **jamais** une compensation dans le code (spec §8.3) |

> ⚠️ **Il n'y a pas encore de tray, donc pas de « Quitter ».** Arrêter par `Ctrl+C` dans le
> terminal, ou `Stop-Process -Name shimeji-desktop`.

- [ ] **Step 8 : Vérifier les sept propriétés de l'étape 0 sur la vraie application**

L'étape 0 les a établies sur le spike. Les revérifier ici est rapide et vérifie surtout
les **deux styles ajoutés**, que le spike n'avait pas.

Rejouer la sonde de l'étape 0, en l'adaptant au nom du processus :

```powershell
# copier docs\spike-etape-0\probe-styles.ps1, y remplacer
#   Get-Process -Name spike-overlay   par   Get-Process -Name shimeji-desktop
```

**Attendu, et c'est le point de cette étape** — les cinq styles présents, là où le spike
n'en avait que trois :

```
WS_EX_LAYERED     (composition alpha)    OUI
WS_EX_TOPMOST     (premier plan)         OUI
WS_EX_TRANSPARENT (clics traversants)    OUI
WS_EX_TOOLWINDOW  (hors Alt+Tab)         OUI   ← nouveau
WS_EX_NOACTIVATE  (pas de vol focus)     OUI   ← nouveau
```

**Si les deux nouveaux sont `non`**, `appliquer_styles_etendus` a échoué silencieusement :
son `eprintln!` doit être dans la console. La Tâche 11 ne doit **pas** être commencée avant
que `WS_EX_NOACTIVATE` soit posé — sinon on introduit le vol de focus en même temps que
l'attrapabilité, et les deux se diagnostiqueront ensemble.

- [ ] **Step 9 : Vérifier le multi-écran et la marche à l'œil**

| À observer | Attendu |
|---|---|
| il marche, court, s'arrête, fait demi-tour | les trois allures alternent sans régularité devinable |
| il atteint le bord droit de l'écran 1 | il **passe sur l'écran 2** sans disparaître ni sauter |
| il atteint le bord droit de l'écran 2 | il fait **demi-tour** (pas de voisin) |
| une fenêtre maximisée | il reste visible par-dessus |
| Alt+Tab, barre des tâches | `shimeji-desktop` n'apparaît nulle part |
| taper dans un éditeur pendant qu'il marche | la frappe n'est jamais interrompue |

- [ ] **Step 10 : Commit**

```bash
git add ui src-tauri/src/render.rs src-tauri/src/main.rs
git commit -m "feat(etape-1a): la fenêtre, le rendu, la boucle 60 Hz — il vit

Le moment visible. Tout ce qui précédait était vrai et testé ; ici on le
regarde.

Les deux découvertes de l'étape 0 sont appliquées : WS_EX_NOACTIVATE et
WS_EX_TOOLWINDOW, que Tauri ne pose pas. Le premier AVANT que la hitbox
n'existe (Tâche 11) — sinon le vol de focus apparaîtrait en même temps que
l'attrapabilité et les deux se diagnostiqueraient ensemble, pour rien. Les
bits sont ajoutés par | et non affectés : écraser l'exstyle retirerait
LAYERED et TOPMOST, donc la transparence et le premier plan.

Les PNG externes sont servis par un schéma URI custom que Rust implémente
(http://shime.localhost/blob/12 sur Windows). Écarté : le protocole asset,
qui exige une scope de chemins absolus alors que le dossier est résolu à
l'exécution ; et le base64 par IPC, qui ajoutait une dépendance et 500 Ko au
démarrage pour éviter un appel qui existe déjà.

On n'émet la frame que sur changement : à 60 Hz une pose de marche ne change
d'image que ~8 fois par seconde, soit ~85 % des messages IPC économisés sans
une ligne de logique côté front.

La souris est lue à 60 Hz et non aux ~30 Hz que suggère la spec §3.3 :
l'appel est effectivement quasi gratuit, et à 30 Hz le personnage traînerait
visiblement derrière le curseur pendant un glisser.

pet.js fait 30 lignes et ne décide de rien : il reçoit { image, flip } et
pose un src. Toute la logique est en Rust (spec §3.1)."
```

---

## Tâche 11 : Attrapable — le hit-testing

**Files:**
- Modify: `src-tauri/src/main.rs` (le hit-testing dans `boucle`)

**Interfaces:**
- Consomme : `character::attach::hitbox_ecran`, `render::traverser_les_clics`,
  `behavior::Entrees::curseur_sur_le_personnage`.
- Produit : aucune interface nouvelle — la Tâche 11 **remplit un champ** que la Tâche 7
  avait déjà prévu. C'est le signe que le découpage était juste.

> **Pourquoi désactiver la traversée des clics et pas seulement lire la souris.** On sait
> déjà tout ce qu'il faut par la sonde : `GetCursorPos` et `GetAsyncKeyState` répondent
> sans que la fenêtre reçoive quoi que ce soit. Mais si les clics continuaient de
> traverser, cliquer sur le personnage cliquerait **aussi** l'icône du bureau derrière lui.
> Il faut donc **absorber** le clic — c'est exactement à quoi sert le va-et-vient de
> `set_ignore_cursor_events` de la spec §3.3.
>
> Et sans la boîte de 128×128 réduite à sa hitbox serrée, le personnage serait un **trou
> noir de 128 px** sur le bureau, avalant les clics dans ses zones transparentes.

- [ ] **Step 1 : Remplacer la ligne du hit-testing dans `boucle`**

Ajouter avant la lecture des entrées, et remplacer le `curseur_sur_le_personnage: false` :

```rust
    // Hors de la boucle, avec les autres états persistants :
    let mut clics_traversent = true;
```

```rust
        // ── 60 Hz : les entrées, et le hit-testing (spec §3.3) ──────────
        let m = sonde.mouse();

        // Le curseur est-il dans la hitbox de la POSE COURANTE — et non dans
        // la boîte de 128×128 ? Sans cette distinction, le personnage serait
        // un trou noir de 128 px avalant les clics dans ses zones
        // transparentes.
        let sur_le_personnage = match (
            crate::character::attach::world_position(&ch.attachment, &monde, m.pos),
            ch.manifest.pose(&ch.pose),
        ) {
            (Some(pos), Some(pose)) => crate::character::attach::hitbox_ecran(
                pos,
                &ch.pose,
                pose,
                &ch.manifest,
                echelle_ecran,
                ch.facing,
            )
            .contains(m.pos),

            // Position indérivable (plateforme disparue) ou pose absente :
            // on ne peut pas savoir. `false` est le bon défaut — les clics
            // continuent de traverser, ce qui ne gêne personne.
            _ => false,
        };

        // ── Absorber le clic, mais seulement là où il faut ──────────────
        // Pendant un glisser, on garde les clics absorbés même si le sprite
        // a glissé hors de sa propre hitbox : sinon un déplacement rapide
        // relâcherait le personnage tout seul.
        let porte = matches!(
            ch.attachment,
            crate::character::attach::Attachment::Dragged
        );
        let doit_traverser = !sur_le_personnage && !porte;

        // On n'appelle Win32 que sur CHANGEMENT d'état : appeler
        // `set_ignore_cursor_events` 60 fois par seconde marcherait, mais
        // c'est un appel système par image pour rien.
        if doit_traverser != clics_traversent {
            if crate::render::traverser_les_clics(&handle, &label, doit_traverser).is_err() {
                return;
            }
            clics_traversent = doit_traverser;
        }

        let entrees = Entrees {
            souris: m.pos,
            bouton_gauche: m.left_down,
            curseur_sur_le_personnage: sur_le_personnage,
        };
```

- [ ] **Step 2 : Compiler et lancer**

```powershell
cd src-tauri
cargo build
cargo run
```

- [ ] **Step 3 : Vérifier l'attrapage à l'œil — la seule vérification non automatisable**

Les réflexes d'attrapage, de lâcher, de chute et d'atterrissage sont **déjà couverts par
les tests** de la Tâche 7. Ce qui reste à l'œil, c'est le hit-testing lui-même : la
géométrie de la hitbox à l'écran (spec §10.4).

| À faire | Attendu |
|---|---|
| passer le curseur **à côté** du personnage, cliquer | le clic atteint ce qui est dessous |
| passer le curseur **sur** le personnage, cliquer et maintenir | il est soulevé, il suit le curseur |
| le promener sur les deux écrans, puis relâcher | **il tombe**, puis il atterrit et repart |
| le relâcher **au-dessus** de la barre des tâches | il atterrit sur le sol de cet écran |
| le relâcher **très à côté** des deux écrans | le garde-fou le replace sur le sol le plus proche |
| cliquer sur le personnage pendant une frappe dans un éditeur | **la frappe n'est pas interrompue** — c'est `WS_EX_NOACTIVATE` ; si elle l'est, revenir au Step 8 de la Tâche 10 |
| cliquer dans un **coin transparent** de sa boîte de 128 px | le clic passe à travers — la hitbox est serrée |

**Si l'attrapage rate d'un décalage constant**, c'est la hitbox du manifeste qu'il faut
corriger (`"hitbox": [40, 20, 48, 100]`), pas le code : la hitbox est de la donnée, elle se
règle sans recompiler. Un décalage **qui s'inverse selon le sens de marche** signale en
revanche un bug dans le miroir de `hitbox_ecran` — que ses tests couvrent.

- [ ] **Step 4 : Lancer toute la suite une dernière fois**

```powershell
cd src-tauri
cargo test
```

Attendu : `111 passed` — 102 des tâches précédentes, 6 de `sim`, 3 de `render`.

- [ ] **Step 5 : Commit**

```bash
git add src-tauri/src/main.rs
git commit -m "feat(etape-1a): attrapable — le hit-testing

La Tâche 11 ne crée aucune interface : elle remplit le champ
curseur_sur_le_personnage que la Tâche 7 avait déjà prévu. C'est le signe que
le découpage des couches était juste.

On teste la hitbox de la POSE COURANTE, pas la boîte de 128×128 : sinon le
personnage serait un trou noir de 128 px avalant les clics dans ses zones
transparentes (spec §3.3).

La traversée des clics n'est désactivée que dans la hitbox, et seulement sur
CHANGEMENT d'état — un appel système par image serait gratuit mais inutile.
Sa raison d'être est d'ABSORBER le clic : la sonde sait déjà tout ce qu'il
faut sans que la fenêtre reçoive rien, mais si les clics traversaient
toujours, cliquer le personnage cliquerait aussi l'icône du bureau derrière.

Pendant un glisser, on garde les clics absorbés même si le sprite a glissé
hors de sa propre hitbox, sinon un déplacement rapide le relâcherait tout
seul."
```

---

## Auto-revue

Menée après avoir écrit le plan, en le relisant contre la spec.

### Couverture de la spec

| Section de la spec | Où c'est traité |
|---|---|
| §3.1 répartition Rust / webview | Tâche 10 — `pet.js` fait 30 lignes et ne décide de rien |
| §3.2 une fenêtre par personnage, 128×128 | Tâche 10 |
| §3.3 hit-testing par hitbox | Tâche 11 |
| §3.4 pixels physiques, échelle au sprite seulement | Tâches 1, 5, 10 |
| §4 distribution, runtime C++ statique | Tâche 1 Step 3 |
| §5.1 le monde = plateformes | Tâche 3 |
| §5.2 identité stable | Tâche 3, test dédié |
| §5.3 filtrage des fenêtres | ⬜ **étape 4** — hors périmètre, annoncé en tête |
| §5.4 occlusion, soustraction 1D | ⬜ **étape 4** — hors périmètre, annoncé en tête |
| §5.5 les trois horloges | Tâche 10 — 60 Hz et 8 Hz ; le 2 Hz est l'étape 2 |
| §6.1 état du personnage | Tâche 7 |
| §6.2 position dérivée | **Tâche 5**, quatre tests |
| §6.3 transitions | Tâche 7 (réflexes) et Tâche 8 (bord de face) |
| §7.1 trois couches | Tâches 7 et 8, enchaînées par `behavior::pas` |
| §7.2 les signaux biaisent | Tâche 8 — `tirer_avec`, test de la marge |
| §7.3 navigation, délai d'abandon | Tâche 8 |
| §7.4 social | ⬜ **étape 3** |
| §8.1 personnages externes | Tâches 4 et 10 (schéma URI) |
| §8.2 vocabulaire Shimeji | Tâche 4 — constantes de poses |
| §8.3 une frame = un fichier + une ancre | Tâche 5 — `window_top_left` |
| §8.4 hitbox | Tâches 4 et 11 |
| §8.5 manifeste | Tâche 4 |
| §8.6 couverture partielle | Tâches 4 et 8 — un test dans chacune |
| §8.7 le personnage `blob` | Tâche 4 Step 1 |
| §9 tray, démarrage auto, config | ⬜ **plan 1b** — annoncé en tête |
| §10.1 testable sans écran | les dix sujets du tableau ont un test, sauf les deux de l'étape 4 |
| §10.2 les trois contraintes | Tâches 1 et 2, **avant** toute logique |
| §10.3 mode simulation | Tâche 9 |
| §10.4 ce qui reste à l'œil | Tâche 10 Step 9, Tâche 11 Step 3 |

**Deux écarts assumés par rapport à la spec, tous deux documentés au point d'usage :**

1. **La souris est lue à 60 Hz**, là où §3.3 suggère ~30 Hz. À 30 Hz le personnage
   traînerait derrière le curseur pendant un glisser, et l'appel est effectivement quasi
   gratuit.
2. **Le repli « 30 Hz + interpolation » de §12 est abandonné**, l'étape 0 ayant mesuré que
   60 Hz est fluide. C'est une contrainte globale de ce plan.

### Cohérence des types

Vérifiée en relisant les blocs `Interfaces` dans l'ordre :

- `Intention` reste l'étiquette `Copy`/`Eq` de la Tâche 7 à la Tâche 8 ; l'état mutable
  vit dans `EtatIntention`. Le fichier minimal de la Tâche 7 Step 4 est **remplacé**, pas
  complété, par celui de la Tâche 8 Step 5 — et il garde les mêmes noms, donc `reflex.rs`
  ne change pas.
- `Face` est défini dans `geom` et réexporté par `world`. Les deux orthographes
  (`geom::Face`, `world::Face`) désignent le même type.
- `ActiveIntention::nouvelle(Intention, Duration)` a la même signature dans les deux
  tâches.
- `Rendu` a exactement les deux champs que `pet.js` lit, et un test vérifie le JSON produit.
- `Entrees::curseur_sur_le_personnage` est déclaré en Tâche 7, laissé à `false` en
  Tâche 10, rempli en Tâche 11.

### Le point qui reste incertain

**Un seul, et il est isolé** : le pont d'événements de `pet.js`.
`window.__TAURI_INTERNALS__` est une interface interne dont le nom peut changer d'une
version de Tauri à l'autre, et c'est le seul endroit du plan dont l'API n'a pas pu être
vérifiée dans les sources — tout le reste l'a été.

Deux replis sont écrits, dans l'ordre, à la Tâche 10 Step 2, et le Step 7 les met à
l'épreuve **avant** que le reste de la tâche n'en dépende.

---

## Annexe — ce que le plan 1b contiendra

Écrit ici pour que la frontière soit nette, et pour que rien de 1b ne fuite dans 1a.

| | Contenu | Fichiers |
|---|---|---|
| 1 | **Le tray et « Quitter »** — c'est ce qui rend l'application utilisable au quotidien : aujourd'hui elle ne se ferme que par `Ctrl+C` | `main.rs` |
| 2 | **`config.json`** — personnages à instancier, échelle, vitesses, **table d'envies et durées d'allure**. Toutes les valeurs ont un défaut dans le code : l'absence de fichier n'est pas une erreur, un fichier partiel non davantage (spec §9.3) | `config.rs` |
| 3 | **Démarrage automatique** — clé `Run` de l'utilisateur courant, case à cocher dans le tray. Au démarrage : aucune fenêtre, aucune notification, aucun vol de focus (spec §9.2) | `main.rs` |
| 4 | **Rechargement à chaud** — relire `mascot.json` et les images sans redémarrer. Pèse plus qu'il n'y paraît : c'est lui qui rend supportable le réglage des animations, qui sera l'essentiel du travail de finition (spec §9.1). Le schéma URI de la Tâche 10 le rend presque gratuit — il suffit d'un numéro de version dans l'URL |
| 5 | **`#![windows_subsystem = "windows"]`** — supprimer la console, une fois que « Quitter » existe dans le tray |

**Ce que 1b ne contient pas** : rien de nouveau côté comportement. Il déplace des
constantes vers un fichier et ajoute une interface. Si une tâche de 1b demande de toucher
à `behavior/` ou à `character/`, c'est le signe qu'elle appartenait à 1a.
