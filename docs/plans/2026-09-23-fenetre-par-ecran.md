# Une fenêtre par écran — le plan

> **Pour un agent exécutant :** SOUS-COMPÉTENCE REQUISE — utiliser
> `superpowers:subagent-driven-development` (recommandé) ou
> `superpowers:executing-plans` pour exécuter tâche par tâche. Les étapes sont
> des cases à cocher (`- [ ]`).

**But :** remplacer « une fenêtre Windows par personnage » par « une fenêtre par
écran occupé », pour que la file du thread principal cesse de se boucher.

**Architecture :** les personnages ne sont plus des fenêtres mais des `<img>`
dans une fenêtre par écran. Rust garde **toute** la physique à 60 Hz, envoie
les positions groupées à 15 Hz par un `eval`, et le webview interpole pour
dessiner à 60 Hz.

**Pile :** Rust, Tauri 2.11.5, crate `windows` 0.61, HTML/CSS/JS sans bundler.

**Conception :** `docs/specs/2026-09-23-fenetre-par-ecran-design.md`
**Mesures fondatrices :** `docs/specs/2026-09-22-spike-fenetre-par-ecran.md`

## Contraintes globales

- **Compiler depuis PowerShell**, jamais Git Bash (piège n° 3 de CLAUDE.md).
- **Ne jamais éditer une source par `Get-Content`/`Set-Content`** : UTF-8
  accentué, PowerShell 5.1 double-encode tout le fichier. Outil d'édition ou
  Python en UTF-8 explicite.
- **Commentaires abondants, en français**, expliquant le *pourquoi*. Citer la
  section de la conception qu'un bloc applique.
- **`cargo test --quiet`**, et `cargo build 2>&1 | Select-Object -Last 40` —
  borner les sorties (CLAUDE.md, coût des sessions).
- **`cargo test` ne reconstruit pas l'exe** : `cargo build` avant de relancer
  l'application.
- Les tests d'un module `x.rs` vivent dans `x_tests.rs`, inclus par
  `#[cfg(test)] #[path = "x_tests.rs"] mod tests;`.
- **La physique, le monde, le comportement et les signaux ne sont PAS
  touchés.** Toute tâche qui aurait besoin de les modifier est le signe d'une
  erreur de conception — s'arrêter et le signaler.

> ⚠️ **Deux noms de ce plan sont repris de l'existant sans avoir été
> revérifiés, et doivent l'être à la tâche 5 :** `echecs_pousser` (le compteur
> d'échecs tracé dans la cadence) et `version_contenu` (la version du contenu,
> incrémentée au rechargement à chaud). S'ils portent un autre nom dans
> `main.rs`, utiliser le vrai — ne pas en créer un second, ce serait une
> seconde vérité à tenir d'accord avec la première.

---

### Tâche 1 : la répartition des sprites par écran

**Fichiers :**
- Créer : `src-tauri/src/overlay.rs`
- Créer : `src-tauri/src/overlay_tests.rs`
- Modifier : `src-tauri/src/main.rs` (ajouter `mod overlay;` après `mod menu_perso;`)

**Interfaces :**
- Consomme : `crate::geom::Rect`, `crate::probe::ScreenInfo`
- Produit :
  - `pub struct SpriteRendu { pub id: u32, pub x: i32, pub y: i32, pub w: u32, pub h: u32, pub image: u32, pub flip: bool }`
  - `pub struct SpriteRelatif { pub id: u32, pub x: i32, pub y: i32, pub w: u32, pub h: u32, pub image: u32, pub flip: bool }`
  - `pub struct ChargeEcran { pub ecran: u64, pub sprites: Vec<SpriteRelatif> }`
  - `pub fn repartir(sprites: &[SpriteRendu], ecrans: &[ScreenInfo]) -> Vec<ChargeEcran>`

- [ ] **Étape 1 : écrire les tests qui échouent**

Créer `src-tauri/src/overlay_tests.rs` :

```rust
//! Les tests de `overlay` — la répartition des sprites par écran et la
//! fabrication du JavaScript.
use super::*;
use crate::geom::Rect;
use crate::probe::ScreenInfo;

/// Deux écrans côte à côte, 1920×1080 chacun, sans barre des tâches pour
/// simplifier : la zone de travail vaut l'écran entier.
fn deux_ecrans() -> Vec<ScreenInfo> {
    vec![
        ScreenInfo { id: 1, work_area: Rect::new(0.0, 0.0, 1920.0, 1080.0), scale: 1.0 },
        ScreenInfo { id: 2, work_area: Rect::new(1920.0, 0.0, 1920.0, 1080.0), scale: 1.0 },
    ]
}

fn sprite(id: u32, x: i32, y: i32) -> SpriteRendu {
    SpriteRendu { id, x, y, w: 128, h: 128, image: 1, flip: false }
}

#[test]
fn un_sprite_va_sur_l_ecran_qui_le_contient() {
    let charges = repartir(&[sprite(7, 100, 200)], &deux_ecrans());
    assert_eq!(charges.len(), 1);
    assert_eq!(charges[0].ecran, 1);
    assert_eq!(charges[0].sprites.len(), 1);
    assert_eq!(charges[0].sprites[0].id, 7);
}

#[test]
fn les_coordonnees_sont_relatives_a_l_ecran() {
    // Sur le second écran, qui commence à x = 1920 : un sprite posé à 2000
    // doit être dessiné à 80 dans SA fenêtre, pas à 2000.
    let charges = repartir(&[sprite(1, 2000, 300)], &deux_ecrans());
    assert_eq!(charges[0].ecran, 2);
    assert_eq!(charges[0].sprites[0].x, 80);
    assert_eq!(charges[0].sprites[0].y, 300);
}

#[test]
fn un_sprite_a_cheval_est_emis_dans_les_deux_ecrans() {
    // Décision 5.2 : posé à x = 1860, il déborde de 68 px sur le second
    // écran. Sans ça il serait COUPÉ au bord.
    let charges = repartir(&[sprite(3, 1860, 100)], &deux_ecrans());
    assert_eq!(charges.len(), 2);

    let gauche = charges.iter().find(|c| c.ecran == 1).expect("écran 1 absent");
    let droite = charges.iter().find(|c| c.ecran == 2).expect("écran 2 absent");

    assert_eq!(gauche.sprites[0].x, 1860);
    // Relatif au second écran : 1860 - 1920 = -60, donc il entre par la
    // gauche, à cheval sur le bord.
    assert_eq!(droite.sprites[0].x, -60);
    assert_eq!(droite.sprites[0].id, 3);
}

#[test]
fn un_ecran_sans_sprite_n_a_pas_de_charge() {
    // Décision 5.1 : un écran vide ne doit pas exister comme fenêtre, donc
    // pas apparaître ici non plus.
    let charges = repartir(&[sprite(1, 10, 10)], &deux_ecrans());
    assert!(charges.iter().all(|c| c.ecran != 2));
}

#[test]
fn un_sprite_hors_de_tout_ecran_n_est_emis_nulle_part() {
    // Peut arriver une image, le temps qu'une plateforme disparaisse : on ne
    // veut ni panique, ni charge fantôme.
    let charges = repartir(&[sprite(1, 9000, 9000)], &deux_ecrans());
    assert!(charges.is_empty());
}
```

- [ ] **Étape 2 : lancer les tests et vérifier qu'ils échouent**

Depuis PowerShell :
```powershell
cd C:\Users\alri\Documents\shimeji-desktop\src-tauri
cargo test --quiet overlay 2>&1 | Select-Object -Last 20
```
Attendu : ÉCHEC de compilation, `unresolved module` ou `cannot find function repartir`.

- [ ] **Étape 3 : écrire `overlay.rs`**

Créer `src-tauri/src/overlay.rs` :

```rust
//! La couche d'affichage de l'overlay : répartir les sprites par écran, et
//! fabriquer le JavaScript qui les porte.
//!
//! Responsabilité unique, et **aucune dépendance à Tauri** : ce module ne
//! connaît ni fenêtre, ni `eval`, ni thread. Il transforme de la donnée en
//! donnée, ce qui est précisément ce qui le rend testable sans écran.
//!
//! Conception : `docs/specs/2026-09-23-fenetre-par-ecran-design.md` §5.

use crate::probe::ScreenInfo;

/// Un sprite tel que la boucle le calcule : en pixels du **bureau virtuel**,
/// coin haut-gauche.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteRendu {
    /// Identité stable dans le temps, pour que le webview retrouve le même
    /// élément `<img>` d'une image à l'autre. C'est le compteur monotone des
    /// acteurs, jamais leur index dans le `Vec` — un index se réutilise, et
    /// un sprite hériterait alors de la position interpolée du précédent.
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub image: u32,
    pub flip: bool,
}

/// Le même sprite, mais en pixels **relatifs à la fenêtre d'un écran**.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteRelatif {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub image: u32,
    pub flip: bool,
}

/// Ce qu'un écran doit afficher.
#[derive(Debug, Clone, PartialEq)]
pub struct ChargeEcran {
    /// L'identité de l'écran (`ScreenInfo::id`), stable dans le temps.
    pub ecran: u64,
    pub sprites: Vec<SpriteRelatif>,
}

/// Répartit les sprites sur les écrans qu'ils touchent.
///
/// **Un sprite à cheval est émis dans les DEUX écrans** (conception §5.2),
/// avec des coordonnées relatives à chacun — donc négatives dans celui où il
/// entre par la gauche ou par le haut. Chaque fenêtre le coupe à son bord, et
/// les deux moitiés se rejoignent à l'écran.
///
/// **Un écran sans sprite ne produit aucune charge** (§5.1) : c'est ce qui
/// permet à l'appelant de ne pas créer sa fenêtre, et donc de ne pas payer le
/// péage de ~34 %.
pub fn repartir(sprites: &[SpriteRendu], ecrans: &[ScreenInfo]) -> Vec<ChargeEcran> {
    let mut charges: Vec<ChargeEcran> = Vec::new();

    for e in ecrans {
        let z = e.work_area;
        let mut dedans: Vec<SpriteRelatif> = Vec::new();

        for s in sprites {
            // Intersection de deux rectangles. On compare en flottants parce
            // que `work_area` est en flottants ; les sprites, eux, sont déjà
            // arrondis par la boucle.
            let sx = s.x as f32;
            let sy = s.y as f32;
            let sw = s.w as f32;
            let sh = s.h as f32;

            let touche = sx < z.right() && sx + sw > z.left() && sy < z.bottom() && sy + sh > z.top();

            if !touche {
                continue;
            }

            dedans.push(SpriteRelatif {
                id: s.id,
                // Le passage en coordonnées de fenêtre. C'est la SEULE
                // conversion du module, et la seule occasion de se tromper
                // de repère.
                x: s.x - z.x as i32,
                y: s.y - z.y as i32,
                w: s.w,
                h: s.h,
                image: s.image,
                flip: s.flip,
            });
        }

        // `is_empty` et non « pousser quand même une charge vide » : une
        // charge vide ferait croire à l'appelant que l'écran est occupé.
        if !dedans.is_empty() {
            charges.push(ChargeEcran { ecran: e.id, sprites: dedans });
        }
    }

    charges
}

// Les tests de ce module vivent dans `overlay_tests.rs`, selon la convention
// du projet (voir `world.rs`).
#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
```

Puis ajouter dans `src-tauri/src/main.rs`, dans la liste des `mod` (ordre
alphabétique, après `mod menu_perso;`) :

```rust
mod overlay;
```

- [ ] **Étape 4 : lancer les tests et vérifier qu'ils passent**

```powershell
cargo test --quiet overlay 2>&1 | Select-Object -Last 20
```
Attendu : les 5 tests passent.

- [ ] **Étape 5 : commit**

```bash
git add src-tauri/src/overlay.rs src-tauri/src/overlay_tests.rs src-tauri/src/main.rs
git commit -m "feat(overlay): repartir les sprites par ecran, a cheval compris"
```

---

### Tâche 2 : le JavaScript de la charge utile

**Fichiers :**
- Modifier : `src-tauri/src/overlay.rs`
- Modifier : `src-tauri/src/overlay_tests.rs`

**Interfaces :**
- Consomme : `ChargeEcran`, `SpriteRelatif` (tâche 1)
- Produit : `pub fn js_de(charge: &ChargeEcran) -> String`

- [ ] **Étape 1 : écrire les tests qui échouent**

Ajouter à la fin de `src-tauri/src/overlay_tests.rs` :

```rust
#[test]
fn le_js_appelle_poser_tous_avec_un_tableau_de_tableaux() {
    let charge = ChargeEcran {
        ecran: 1,
        sprites: vec![SpriteRelatif { id: 4, x: 10, y: 20, w: 128, h: 128, image: 7, flip: false }],
    };
    assert_eq!(js_de(&charge), "window.poserTous([[4,10,20,128,128,7,0]])");
}

#[test]
fn le_flip_est_un_entier_et_non_un_booleen() {
    // Un entier plutôt que `true`/`false` : la chaîne est construite 44 fois
    // par seconde, et `0`/`1` la raccourcit de quatre caractères par sprite.
    let charge = ChargeEcran {
        ecran: 1,
        sprites: vec![SpriteRelatif { id: 1, x: 0, y: 0, w: 64, h: 64, image: 2, flip: true }],
    };
    assert!(js_de(&charge).ends_with(",1]])"));
}

#[test]
fn plusieurs_sprites_sont_separes_par_une_virgule() {
    let charge = ChargeEcran {
        ecran: 1,
        sprites: vec![
            SpriteRelatif { id: 1, x: 0, y: 0, w: 1, h: 2, image: 3, flip: false },
            SpriteRelatif { id: 2, x: -5, y: 6, w: 7, h: 8, image: 9, flip: true },
        ],
    };
    assert_eq!(
        js_de(&charge),
        "window.poserTous([[1,0,0,1,2,3,0],[2,-5,6,7,8,9,1]])"
    );
}
```

- [ ] **Étape 2 : lancer les tests et vérifier qu'ils échouent**

```powershell
cargo test --quiet overlay 2>&1 | Select-Object -Last 20
```
Attendu : `cannot find function js_de`.

- [ ] **Étape 3 : écrire `js_de`**

Ajouter dans `src-tauri/src/overlay.rs`, avant le bloc `#[cfg(test)]` :

```rust
/// Fabrique l'appel JavaScript qui porte toute la charge d'un écran.
///
/// # Pourquoi un tableau de tableaux, et pas des objets
///
/// `[4,10,20,128,128,7,0]` fait 21 caractères ; l'objet équivalent en fait
/// plus du double. Cette chaîne est construite **44 fois par seconde** et
/// traverse la frontière de processus à chaque fois : c'est Rust qui paie sa
/// construction, et le webview son analyse.
///
/// L'ordre des champs est `[id, x, y, w, h, image, flip]` — le MÊME que celui
/// que lit `overlay.js`. Les deux se corrompent silencieusement s'ils
/// divergent : un `w` lu comme un `y` ne produit aucune erreur, juste un
/// sprite au mauvais endroit.
///
/// # Pourquoi rien n'est échappé
///
/// Il n'entre dans cette chaîne que des entiers et un booléen, tous produits
/// par nous. Aucun nom de fichier, aucune chaîne venue d'un pack tiers — le
/// personnage à afficher est fixé à la création de la fenêtre. C'est ce qui
/// rend l'absence d'échappement sûre, et c'est pourquoi il ne faut **jamais**
/// ajouter de texte à cette charge utile sans revoir ce point.
pub fn js_de(charge: &ChargeEcran) -> String {
    // 32 caractères par sprite : une estimation large, pour que le `String`
    // ne se réalloue pas en cours de route.
    let mut js = String::with_capacity(24 + charge.sprites.len() * 32);
    js.push_str("window.poserTous([");

    for (i, s) in charge.sprites.iter().enumerate() {
        if i > 0 {
            js.push(',');
        }
        js.push_str(&format!(
            "[{},{},{},{},{},{},{}]",
            s.id,
            s.x,
            s.y,
            s.w,
            s.h,
            s.image,
            // `u8` plutôt que `bool` : voir le commentaire de l'en-tête.
            u8::from(s.flip)
        ));
    }

    js.push_str("])");
    js
}
```

- [ ] **Étape 4 : lancer les tests et vérifier qu'ils passent**

```powershell
cargo test --quiet overlay 2>&1 | Select-Object -Last 20
```
Attendu : les 8 tests passent.

- [ ] **Étape 5 : commit**

```bash
git add src-tauri/src/overlay.rs src-tauri/src/overlay_tests.rs
git commit -m "feat(overlay): le JavaScript de la charge utile d'un ecran"
```

---

### Tâche 3 : l'afficheur multi-sprites

**Fichiers :**
- Créer : `ui/overlay.html`
- Créer : `ui/overlay.js`

**Interfaces :**
- Consomme : l'appel `window.poserTous([[id,x,y,w,h,image,flip], …])` de la tâche 2
- Produit : la page que la tâche 4 chargera dans chaque fenêtre d'écran. Elle
  lit le nom du pack par sprite dans le fragment d'URL ? **Non** : voir la
  note ci-dessous.

> ⚠️ **Le personnage n'est plus fixé par fenêtre.** Une fenêtre d'écran porte
> des personnages de packs **différents**. L'URL de l'image ne peut donc plus
> venir du fragment comme dans `pet.js` : elle est déduite de l'`id` du
> sprite, dont Rust publie la table par un second appel, `window.declarer`.
> C'est la seule information non numérique qui traverse, et elle ne change
> qu'à un changement de roster — pas 44 fois par seconde.

- [ ] **Étape 1 : écrire `ui/overlay.html`**

```html
<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>shimeji overlay</title>
<style>
  /* `background: transparent` sur `html` ET `body` : si l'un des deux garde
     un fond, WebView2 peint un rectangle opaque et la transparence de la
     fenêtre ne se voit pas. Cause de faux négatif n° 1, confirmée au spike
     de l'étape 0. */
  html, body {
    margin: 0;
    padding: 0;
    background: transparent;
    overflow: hidden;
  }

  .perso {
    position: absolute;
    /* `left/top` restent à 0 : TOUT le déplacement passe par `transform`,
       composé par le GPU sans refaire la mise en page. C'est le mécanisme
       qui remplace `SetWindowPos`. */
    left: 0;
    top: 0;
    image-rendering: pixelated;
    user-select: none;
    -webkit-user-drag: none;
    /* Le webview n'intercepte jamais rien : c'est Rust qui décide, par
       `set_ignore_cursor_events` sur la fenêtre entière (conception §4). */
    pointer-events: none;
    will-change: transform;
  }
</style>
</head>
<body>
<script src="overlay.js"></script>
</body>
</html>
```

- [ ] **Étape 2 : écrire `ui/overlay.js`**

```js
// Afficheur de l'overlay : N sprites dans une fenêtre d'écran.
//
// Il ne calcule RIEN — ni trajectoire, ni décision, ni rien qu'il renvoie à
// Rust. Il fait deux choses :
//   1. placer chaque sprite là où Rust le dit (15 fois par seconde) ;
//   2. GLISSER entre deux positions reçues, pour dessiner à 60 images/s.
//
// Le point 2 est du LISSAGE, pas de la logique (conception §2.2) : le facteur
// est borné à 1, donc on n'invente jamais une position que Rust n'a pas
// calculée. Si l'envoi suivant tarde, le sprite s'arrête sur la dernière
// position connue au lieu de la dépasser.

// ── La table des personnages ────────────────────────────────────────────
//
// `id -> nom de pack`, publiée par Rust à chaque changement de roster. Elle
// ne peut pas venir du fragment d'URL comme dans l'ancien `pet.js` : une
// fenêtre d'écran porte des packs différents.
let packs = {};

// Version du contenu, changée à chaque rechargement à chaud. Les images sont
// servies avec `Cache-Control: max-age=3600` : sans ce paramètre, une image
// modifiée sur le disque ne serait jamais relue.
let version = 0;

// Le cache des URL déjà construites, pour ne pas reformer la même chaîne
// 60 fois par seconde.
const urls = new Map();

function urlDe(id, image) {
  const cle = version + '/' + id + '/' + image;
  let u = urls.get(cle);
  if (u === undefined) {
    // Le gestionnaire du schéma URI lit `uri().path()`, qui IGNORE la
    // requête : `?v=3` ne change donc rien côté Rust, seulement la clé de
    // cache du webview.
    u = 'http://shime.localhost/' + packs[id] + '/' + image + '?v=' + version;
    urls.set(cle, u);
  }
  return u;
}

// ── Les sprites vivants ─────────────────────────────────────────────────
//
// `id -> { el, dx, dy, ax, ay, t, w, h, image, flip, px, py, pimage, pflip }`
// `d*` = départ du segment courant, `a*` = arrivée, `t` = instant de départ.
// `p*` = ce qui est réellement POSÉ dans le DOM, pour ne rien réécrire
// d'identique.
const sprites = new Map();
let intervalle = 66;   // durée mesurée entre deux envois, remplacée dès le 2e
let dernierEnvoi = 0;

function elementDe(id) {
  let s = sprites.get(id);
  if (s === undefined) {
    const el = document.createElement('img');
    el.className = 'perso';
    document.body.appendChild(el);
    s = { el: el, px: null, py: null, pimage: null, pflip: null, pw: null, ph: null };
    sprites.set(id, s);
  }
  return s;
}

// ── Ce que Rust appelle ─────────────────────────────────────────────────

// Publie la table `id -> pack`. Appelée à chaque changement de roster, et une
// fois au démarrage. Les sprites dont l'id a disparu sont retirés du DOM :
// sans ça, un personnage supprimé resterait affiché pour toujours.
window.declarer = function (table, v) {
  packs = table;
  version = v;
  urls.clear();
  sprites.forEach(function (s, id) {
    if (!(id in packs)) {
      s.el.remove();
      sprites.delete(id);
    }
  });
};

// Reçoit toute la charge de CET écran, 15 fois par seconde.
// Format d'un sprite : [id, x, y, w, h, image, flip] — le MÊME ordre que
// `overlay::js_de` côté Rust. Les deux divergent en silence s'ils ne sont pas
// modifiés ensemble.
window.poserTous = function (liste) {
  const maintenant = performance.now();
  if (dernierEnvoi !== 0) {
    // L'intervalle RÉEL, mesuré et non supposé : Rust envoie à 15 Hz nominal
    // et dérive. Une durée codée en dur ferait arriver l'interpolation trop
    // tôt (saccade) ou trop tard (glissement).
    intervalle = maintenant - dernierEnvoi;
  }
  dernierEnvoi = maintenant;

  const vus = new Set();

  for (let i = 0; i < liste.length; i++) {
    const l = liste[i];
    const id = l[0];
    vus.add(id);
    const s = elementDe(id);

    if (s.ax === undefined) {
      // Première position connue : on part d'elle, sinon le sprite
      // traverserait l'écran depuis (0,0) à son apparition.
      s.dx = l[1]; s.dy = l[2];
    } else {
      // Le départ du nouveau segment est l'ARRIVÉE du précédent, jamais la
      // position dessinée : partir du dessin accumulerait l'erreur d'arrondi
      // de chaque image.
      s.dx = s.ax; s.dy = s.ay;
    }
    s.ax = l[1]; s.ay = l[2];
    s.w = l[3]; s.h = l[4];
    s.image = l[5]; s.flip = l[6];
    s.t = maintenant;
  }

  // Un sprite absent de la charge a quitté cet écran (il est passé sur le
  // voisin, ou il est parti). On le retire : le laisser le figerait là.
  sprites.forEach(function (s, id) {
    if (!vus.has(id)) {
      s.el.remove();
      sprites.delete(id);
    }
  });
};

// ── La boucle de dessin, à la cadence de l'écran ────────────────────────

function dessiner() {
  const maintenant = performance.now();

  sprites.forEach(function (s, id) {
    if (s.ax === undefined) {
      return;
    }
    // Borné à 1 : on ne DEVINE jamais une position que Rust n'a pas calculée
    // (conception §2.2).
    let alpha = (maintenant - s.t) / intervalle;
    if (alpha > 1) alpha = 1;

    // Arrondi : le pixel-art doit tomber sur des pixels entiers, sinon le
    // compositeur le lisse et la netteté promise par `image-rendering:
    // pixelated` est perdue.
    const x = Math.round(s.dx + (s.ax - s.dx) * alpha);
    const y = Math.round(s.dy + (s.ay - s.dy) * alpha);

    // ⚠️ **Ne rien écrire quand rien n'a changé** (conception §5.3). Le péage
    // de ~34 % par fenêtre est payé quand le CONTENU change : un personnage
    // endormi doit être gratuit. Mesuré : 24 % contre 101 %.
    if (s.px !== x || s.py !== y || s.pflip !== s.flip) {
      s.px = x; s.py = y; s.pflip = s.flip;
      s.el.style.transform =
        'translate(' + x + 'px,' + y + 'px)' + (s.flip ? ' scaleX(-1)' : '');
    }
    if (s.pimage !== s.image) {
      s.pimage = s.image;
      s.el.src = urlDe(id, s.image);
    }
    if (s.pw !== s.w || s.ph !== s.h) {
      s.pw = s.w; s.ph = s.h;
      s.el.style.width = s.w + 'px';
      s.el.style.height = s.h + 'px';
    }
  });

  requestAnimationFrame(dessiner);
}

requestAnimationFrame(dessiner);
```

- [ ] **Étape 3 : vérifier que la page se charge sans erreur**

Il n'y a rien à tester automatiquement ici (aucun harnais JS dans ce projet,
délibérément). La vérification vient à la tâche 5, quand une fenêtre la
chargera. Contrôler seulement que les deux fichiers existent et que
`overlay.html` référence bien `overlay.js` :

```powershell
Select-String ui\overlay.html -Pattern "overlay.js"
```
Attendu : une occurrence.

- [ ] **Étape 4 : commit**

```bash
git add ui/overlay.html ui/overlay.js
git commit -m "feat(overlay): l'afficheur multi-sprites, avec lissage et saut des dessins inutiles"
```

---

### Tâche 4 : créer, détruire et alimenter une fenêtre d'écran

**Fichiers :**
- Modifier : `src-tauri/src/render.rs`

**Interfaces :**
- Consomme : `crate::overlay::ChargeEcran`, `crate::overlay::js_de` (tâches 1-2)
- Produit :
  - `pub fn label_ecran(id: u64) -> String`
  - `pub fn creer_fenetre_ecran(app: &AppHandle, ecran: &ScreenInfo) -> Result<(), String>`
  - `pub fn detruire_fenetre_ecran(app: &AppHandle, id: u64)`
  - `pub fn pousser_ecran(app: &AppHandle, charge: &ChargeEcran) -> Result<(), String>`
  - `pub fn declarer_packs(app: &AppHandle, id_ecran: u64, table_js: &str, version: u32) -> Result<(), String>`

- [ ] **Étape 1 : écrire les fonctions**

Ajouter dans `src-tauri/src/render.rs` :

```rust
/// Le label Tauri de la fenêtre d'un écran.
///
/// Dérivé de l'identité **stable** de l'écran (`ScreenInfo::id`, issue du
/// `HMONITOR`) et non de son index : un écran débranché puis rebranché
/// changerait d'index, et la fenêtre suivante hériterait du label de la
/// précédente pendant que Windows détruit encore celle-ci.
pub fn label_ecran(id: u64) -> String {
    format!("ecran-{id}")
}

/// Crée la fenêtre d'un écran : à la taille de sa zone de travail,
/// transparente, au premier plan, traversante, et qui **ne bougera jamais**.
///
/// C'est tout l'objet de l'architecture : cette fenêtre ne reçoit aucun
/// `SetWindowPos` après sa création (conception §1). Les personnages se
/// déplacent en CSS à l'intérieur.
pub fn creer_fenetre_ecran(app: &AppHandle, ecran: &ScreenInfo) -> Result<(), String> {
    use tauri::{PhysicalPosition, PhysicalSize};

    let label = label_ecran(ecran.id);

    let win = tauri::WebviewWindowBuilder::new(
        app,
        &label,
        tauri::WebviewUrl::App("overlay.html".into()),
    )
    .title("shimeji-desktop")
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(false)
    .focused(false)
    .build()
    .map_err(|e| format!("fenêtre « {label} » : {e}"))?;

    // Position et taille en pixels PHYSIQUES. Les passer au builder les
    // ferait interpréter en pixels logiques, donc faux sur un écran à 125 %
    // (piège n° 4 des « coordonnées », CLAUDE.md).
    win.set_position(PhysicalPosition::new(
        ecran.work_area.x as i32,
        ecran.work_area.y as i32,
    ))
    .map_err(|e| format!("set_position sur « {label} » : {e}"))?;

    win.set_size(PhysicalSize::new(
        ecran.work_area.w as u32,
        ecran.work_area.h as u32,
    ))
    .map_err(|e| format!("set_size sur « {label} » : {e}"))?;

    // Les clics traversent par défaut. La boucle ne les absorbe que quand le
    // curseur est sur un personnage de CET écran (conception §4).
    win.set_ignore_cursor_events(true)
        .map_err(|e| format!("clics traversants sur « {label} » : {e}"))?;

    // **Les deux découvertes de l'étape 0.** Sans elles, la fenêtre volerait
    // le focus de l'éditeur et apparaîtrait dans Alt+Tab.
    match appliquer_styles_etendus(&win) {
        Ok(()) => println!("styles étendus posés sur {label} (NOACTIVATE, TOOLWINDOW)"),
        Err(e) => eprintln!("styles étendus NON appliqués sur {label} : {e}"),
    }

    Ok(())
}

/// Ferme la fenêtre d'un écran devenu vide (conception §5.1).
///
/// Silencieuse si la fenêtre n'existe pas : l'appelant peut le demander deux
/// fois sans que ce soit une erreur.
pub fn detruire_fenetre_ecran(app: &AppHandle, id: u64) {
    if let Some(win) = app.get_webview_window(&label_ecran(id)) {
        let _ = win.close();
    }
}

/// Envoie toute la charge d'un écran, en un seul `eval`.
///
/// ⚠️ **C'est l'appel dont le spike a mesuré le coût : ~2,9 ms de CPU
/// chacun.** Il ne doit être émis que lorsque quelque chose a changé
/// (conception §5.4), et jamais plus de 15 fois par seconde et par écran.
pub fn pousser_ecran(app: &AppHandle, charge: &crate::overlay::ChargeEcran) -> Result<(), String> {
    let label = label_ecran(charge.ecran);
    let Some(win) = app.get_webview_window(&label) else {
        return Err(format!("fenêtre « {label} » absente"));
    };
    win.eval(crate::overlay::js_de(charge))
        .map_err(|e| format!("eval sur « {label} » : {e}"))
}

/// Publie la table `id -> pack` dans la fenêtre d'un écran.
///
/// `table_js` est un littéral objet JavaScript déjà formé par l'appelant
/// (`{"3":"blob","4":"naruto-kakashi"}`), parce que c'est lui qui connaît le
/// roster. Appelée **au changement de roster seulement**, jamais dans la
/// boucle : c'est la seule donnée non numérique qui traverse.
pub fn declarer_packs(
    app: &AppHandle,
    id_ecran: u64,
    table_js: &str,
    version: u32,
) -> Result<(), String> {
    let label = label_ecran(id_ecran);
    let Some(win) = app.get_webview_window(&label) else {
        return Err(format!("fenêtre « {label} » absente"));
    };
    win.eval(format!("window.declarer({table_js}, {version})"))
        .map_err(|e| format!("eval sur « {label} » : {e}"))
}
```

Ajouter en haut de `render.rs`, s'il n'y est pas déjà :

```rust
use crate::probe::ScreenInfo;
```

- [ ] **Étape 2 : vérifier que ça compile**

```powershell
cargo build 2>&1 | Select-Object -Last 40
```
Attendu : compile. Des avertissements « never used » sur les nouvelles
fonctions sont normaux tant que la tâche 5 ne les appelle pas.

- [ ] **Étape 3 : commit**

```bash
git add src-tauri/src/render.rs
git commit -m "feat(overlay): creer, detruire et alimenter la fenetre d'un ecran"
```

---

### Tâche 5 : brancher l'overlay dans la boucle

**Fichiers :**
- Modifier : `src-tauri/src/main.rs` — le bloc de rendu par acteur (~lignes 1988-2075) et la création de fenêtres au changement de roster (~lignes 508, 1589)

**Interfaces :**
- Consomme : tout ce qui précède
- Produit : l'application qui affiche ses personnages dans des fenêtres d'écran

- [ ] **Étape 1 : ajouter l'état de l'overlay à la boucle**

Avant la boucle à 60 Hz, à côté de `moniteur_charge` (~ligne 1252) :

```rust
// ── L'état de l'overlay ─────────────────────────────────────────────
//
// Les écrans pour lesquels une fenêtre existe actuellement. On ne les
// déduit pas de Tauri à chaque image : `get_webview_window` est un
// aller-retour vers le thread principal (découverte du spike
// `spike-deplacements-groupes`, qui coûtait 66 ms par image).
let mut ecrans_ouverts: std::collections::HashSet<u64> = std::collections::HashSet::new();

// La dernière charge envoyée à chaque écran. Sert au §5.4 : ne rien
// envoyer quand rien n'a changé.
let mut derniere_charge: std::collections::HashMap<u64, overlay::ChargeEcran> =
    std::collections::HashMap::new();

// Le prochain instant d'envoi. 15 Hz — mesuré comme le meilleur compromis
// (spike du 2026-09-22, §4) : 44 eval/s tiennent 3 ms de latence.
let mut prochain_envoi = std::time::Instant::now();
const PERIODE_ENVOI: std::time::Duration = std::time::Duration::from_millis(66);
```

- [ ] **Étape 2 : remplacer le bloc de rendu par acteur**

Dans la boucle, **supprimer** les trois appels `render::dimensionner`,
`render::placer` et `render::pousser` (et les champs `derniere_taille`,
`dernier_coin`, `dernier_rendu` qui ne servaient qu'à eux), et **accumuler** à
la place. Le corps de la boucle par acteur se termine désormais par :

```rust
            // ── L'accumulation, au lieu de trois appels Windows ─────────
            //
            // Avant, chaque acteur appelait `dimensionner`, `placer` et
            // `pousser` — donc jusqu'à trois messages sur la file du thread
            // principal, par acteur et par image. C'est ce qui la bouchait.
            // Maintenant on empile de la donnée, et un seul `eval` par écran
            // partira plus bas, à 15 Hz (conception §3).
            if let Some(pos) =
                character::attach::world_position(&acteur.ch.attachment, &monde, m.pos)
            {
                if acteur.ch.manifest.has_pose(&acteur.ch.pose) {
                    let image = acteur.ch.frame_courante(maintenant);
                    let taille = character::attach::window_size(
                        &acteur.ch.manifest,
                        image,
                        echelle_affichage,
                    );
                    let coin = character::attach::window_top_left(
                        pos,
                        image,
                        &acteur.ch.pose,
                        &acteur.ch.manifest,
                        echelle_affichage,
                        acteur.ch.facing,
                    );

                    sprites.push(overlay::SpriteRendu {
                        // `acteur.id` et non `i` : un index se réutilise
                        // quand un acteur part, et le suivant hériterait de
                        // la position interpolée du précédent — il
                        // traverserait l'écran en glissant.
                        id: acteur.id,
                        x: coin.x.round() as i32,
                        y: coin.y.round() as i32,
                        w: taille.0,
                        h: taille.1,
                        image,
                        flip: acteur.ch.facing.flipped(),
                    });
                }
            }
```

Déclarer `let mut sprites: Vec<overlay::SpriteRendu> = Vec::new();` juste
avant la boucle par acteur, à chaque image.

> ⚠️ **`Acteur` gagne un champ `id: u32`**, alimenté par le même compteur
> monotone que `label`. Le `label` d'acteur disparaîtra à la tâche 7, mais
> l'identité, elle, reste nécessaire — c'est elle que le webview utilise pour
> retrouver le bon `<img>`.

- [ ] **Étape 3 : ouvrir, fermer et alimenter les fenêtres, après la boucle par acteur**

```rust
            // ── Les fenêtres d'écran ────────────────────────────────────
            //
            // Une fenêtre n'existe que si son écran porte un personnage
            // (conception §5.1) : le péage de ~34 % est par fenêtre ANIMÉE,
            // donc un écran vide doit être gratuit.
            let charges = if visible {
                overlay::repartir(&sprites, &ecrans)
            } else {
                // Caché ou session verrouillée : aucune charge, donc toutes
                // les fenêtres se ferment juste en dessous. C'est ce qui rend
                // le mode caché réellement gratuit.
                Vec::new()
            };

            let occupes: std::collections::HashSet<u64> =
                charges.iter().map(|c| c.ecran).collect();

            // Fermer ce qui n'est plus occupé.
            for id in ecrans_ouverts.difference(&occupes).copied().collect::<Vec<_>>() {
                render::detruire_fenetre_ecran(&handle, id);
                ecrans_ouverts.remove(&id);
                derniere_charge.remove(&id);
            }

            // Ouvrir ce qui vient de l'être.
            for c in &charges {
                if ecrans_ouverts.contains(&c.ecran) {
                    continue;
                }
                let Some(e) = ecrans.iter().find(|e| e.id == c.ecran) else {
                    continue;
                };
                match render::creer_fenetre_ecran(&handle, e) {
                    Ok(()) => {
                        ecrans_ouverts.insert(c.ecran);
                        // La table des packs doit arriver AVANT la première
                        // charge : sans elle, `urlDe` construirait
                        // « undefined » dans l'URL de l'image.
                        let table = table_packs_js(&acteurs);
                        let _ = render::declarer_packs(&handle, c.ecran, &table, version_contenu);
                    }
                    Err(msg) => eprintln!("écran {} : {msg}", c.ecran),
                }
            }

            // ── L'envoi, à 15 Hz et seulement sur changement ────────────
            if std::time::Instant::now() >= prochain_envoi {
                prochain_envoi = std::time::Instant::now() + PERIODE_ENVOI;

                for c in &charges {
                    // §5.4 : un `eval` coûte ~2,9 ms de CPU. Ne rien envoyer
                    // quand rien n'a changé rend gratuit le cas « tout le
                    // monde dort », qui est le cas de la nuit.
                    if derniere_charge.get(&c.ecran) == Some(c) {
                        continue;
                    }
                    if render::pousser_ecran(&handle, c).is_ok() {
                        derniere_charge.insert(c.ecran, c.clone());
                    } else {
                        echecs_pousser += 1;
                    }
                }
            }
```

Et la fonction utilitaire, à côté de `creer_fenetre_personnage` :

```rust
/// La table `id -> pack` en littéral JavaScript, pour `window.declarer`.
///
/// Les clés sont des chaînes parce qu'un objet JavaScript n'a pas de clés
/// numériques : `{3:"blob"}` est relu `{"3":"blob"}`. Les guillemets sont
/// donc posés ici, et `overlay.js` interroge `packs[id]` — la conversion
/// implicite de JavaScript fait le reste.
///
/// Aucun échappement : un nom de pack est un nom de dossier validé par le
/// catalogue. Si cette garantie devait tomber, c'est ICI qu'il faudrait
/// échapper, pas ailleurs.
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
```

- [ ] **Étape 4 : republier la table des packs à chaque changement de roster**

Là où le roster change (~ligne 1589, et le rechargement à chaud), après avoir
ajouté ou retiré des acteurs :

```rust
                        // Le webview doit connaître les nouveaux `id` avant
                        // la prochaine charge, et oublier les anciens — c'est
                        // `window.declarer` qui retire du DOM les sprites
                        // disparus. L'omettre laisserait un personnage
                        // supprimé affiché pour toujours.
                        let table = table_packs_js(&acteurs);
                        for id in &ecrans_ouverts {
                            let _ = render::declarer_packs(&handle, *id, &table, version_contenu);
                        }
```

- [ ] **Étape 5 : compiler et lancer**

```powershell
cargo build 2>&1 | Select-Object -Last 40
$env:SHIMEJI_PERSONNAGES="blob,blob"
cargo run
```
Attendu : deux `blob` marchent sur le sol, dans une seule fenêtre d'écran.
Vérifier **à l'œil** : ils sont visibles, nets, ils marchent, les clics
traversent vers le bureau.

- [ ] **Étape 6 : commit**

```bash
git add src-tauri/src/main.rs
git commit -m "feat(overlay): la boucle alimente une fenetre par ecran occupe"
```

---

### Tâche 6 : les clics, le glisser et le menu contextuel

**Fichiers :**
- Modifier : `src-tauri/src/main.rs` (~lignes 1815-1870)

**Interfaces :**
- Consomme : `ecrans_ouverts`, `charges` (tâche 5), `elu` (existant)
- Produit : l'absorption des clics par écran

- [ ] **Étape 1 : remplacer l'absorption par acteur par une absorption par écran**

`render::traverser_les_clics` était appelée avec le label d'un personnage.
Elle prend maintenant le label d'un écran — sa signature ne change pas, seul
l'appelant change. Supprimer le champ `clics_traversent` de `Acteur` et le
porter par écran :

```rust
// À déclarer avec `ecrans_ouverts` :
let mut clics_traversent: std::collections::HashMap<u64, bool> =
    std::collections::HashMap::new();
```

Puis, **après** le calcul de `charges` :

```rust
            // ── L'absorption des clics, par écran ──────────────────────
            //
            // Avant, chaque personnage était une fenêtre et absorbait pour
            // lui-même. Maintenant une fenêtre porte N personnages : elle
            // absorbe si le curseur est sur **l'un** d'eux, ou si l'un d'eux
            // est porté.
            //
            // Le test d'appartenance reste EXACTEMENT le même (`elu`, calculé
            // plus haut sur les hitbox) : on ne change que la fenêtre à qui
            // on l'applique. C'est ce qui rend ce remplacement sûr.
            let acteur_actif = elu.or_else(|| {
                acteurs.iter().position(|a| {
                    matches!(a.ch.attachment, character::attach::Attachment::Dragged)
                })
            });

            // L'écran qui doit absorber : celui qui porte l'acteur actif.
            let ecran_absorbant = acteur_actif.and_then(|i| {
                let id = acteurs[i].id;
                charges
                    .iter()
                    .find(|c| c.sprites.iter().any(|s| s.id == id))
                    .map(|c| c.ecran)
            });

            for id in ecrans_ouverts.iter().copied().collect::<Vec<_>>() {
                let doit_traverser = Some(id) != ecran_absorbant;
                // On n'appelle Win32 que sur CHANGEMENT : l'appeler 60 fois
                // par seconde marcherait, mais c'est un appel système par
                // image pour rien (CLAUDE.md, « Mesurer le CPU »).
                if clics_traversent.get(&id) != Some(&doit_traverser) {
                    let label = render::label_ecran(id);
                    if render::traverser_les_clics(&handle, &label, doit_traverser).is_ok() {
                        clics_traversent.insert(id, doit_traverser);
                    }
                }
            }
```

> ⚠️ **Un personnage à cheval sur deux écrans n'est absorbé que par un seul.**
> `find` rend le premier. C'est accepté : l'attraper marche, seul le bord
> lointain du sprite laisse passer le clic. Le corriger demanderait d'absorber
> sur les deux écrans, donc de laisser une fenêtre non traversante alors que
> le curseur n'est pas dessus.

- [ ] **Étape 2 : vérifier que le menu contextuel et le glisser marchent toujours**

Le clic droit (`front_descendant_droit && sur_le_personnage`) et le glisser
n'utilisent que `elu` et la position du curseur, pas le label de fenêtre : ils
ne changent pas. Le seul point à vérifier est que `menu_perso::ouvrir` reçoit
encore une fenêtre valide — lui passer la fenêtre de l'écran absorbant :

```rust
                // `let … else` plutôt qu'un `unwrap_or_default()` : l'identité
                // d'un écran vient du `HMONITOR`, et **zéro n'est pas une
                // valeur de repli valide** — ce serait le label d'une fenêtre
                // qui n'existe pas, donc un menu qui ne s'ouvre jamais, sans
                // le moindre message.
                //
                // Si aucun écran n'absorbe, c'est qu'aucun personnage n'est
                // sous le curseur : il n'y a pas de menu à ouvrir.
                let Some(id_ecran) = ecran_absorbant else {
                    continue;
                };
                let label = render::label_ecran(id_ecran);
```

- [ ] **Étape 3 : compiler et vérifier à l'œil**

```powershell
cargo build 2>&1 | Select-Object -Last 40
$env:SHIMEJI_PERSONNAGES="blob,blob"
cargo run
```
Vérifier, dans cet ordre : (1) le curseur hors des personnages laisse cliquer
le bureau ; (2) le curseur sur un personnage permet de l'attraper et de le
lâcher ; (3) le clic droit dessus ouvre le menu ; (4) le clic droit à côté
n'ouvre rien.

- [ ] **Étape 4 : commit**

```bash
git add src-tauri/src/main.rs
git commit -m "feat(overlay): absorber les clics par ecran, pas par personnage"
```

---

### Tâche 7 : retirer l'ancien chemin

**Fichiers :**
- Modifier : `src-tauri/src/main.rs`, `src-tauri/src/render.rs`
- Supprimer : `ui/index.html`, `ui/pet.js`

- [ ] **Étape 1 : supprimer ce qui n'a plus d'appelant**

- `main.rs` : `fn creer_fenetre_personnage`, le champ `label` d'`Acteur`, et
  les champs `derniere_taille`, `dernier_coin`, `dernier_rendu`,
  `clics_traversent`.
- `render.rs` : `pub fn placer`, `pub fn dimensionner`, `pub fn pousser`,
  `pub struct Rendu`, `pub fn recharger` (remplacée par `declarer_packs`).
- `ui/` : `index.html` et `pet.js`.

> ⚠️ **Ne PAS supprimer** `appliquer_styles_etendus`, `autoriser_activation`,
> `prendre_le_premier_plan`, `reveiller_la_file`, `traverser_les_clics` : elles
> servent toutes à l'overlay ou au menu.

- [ ] **Étape 2 : vérifier qu'il ne reste aucun avertissement de code mort nouveau**

```powershell
cargo build 2>&1 | Select-String "never used|never read" | Select-Object -Last 20
```
Attendu : seulement les avertissements qui existaient déjà avant cette branche
(`ReseauFake`, `PlatformKind::Window`, `World::premier_sol`).

- [ ] **Étape 3 : la suite complète**

```powershell
cargo test --quiet 2>&1 | Select-Object -Last 20
```
Attendu : tous les tests passent. Le compte doit être **316 + 8** = 324.

- [ ] **Étape 4 : commit**

```bash
git add -A
git commit -m "refactor(overlay): retirer une fenetre par personnage"
```

---

### Tâche 8 : mesurer, et décider

**Fichiers :**
- Créer : `docs/specs/2026-09-23-fenetre-par-ecran-mesure.md`
- Modifier : `CLAUDE.md`

- [ ] **Étape 1 : mesurer les trois cas**

```powershell
cargo build --release 2>&1 | Select-Object -Last 10
# 15 personnages, la configuration qui a declenche tout ceci
.\docs\outils\mesurer-roster.ps1 -N 15
.\docs\outils\mesurer-roster.ps1 -N 15 -Cache
.\docs\outils\mesurer-roster.ps1 -N 1
```

Relever pour chacun : CPU du processus, CPU de l'arbre WebView2, latence de la
file (`SHIMEJI_SIGNAUX=1`).

> ⚠️ **`mesurer-roster.ps1` ne relève que le processus principal.** Depuis la
> mesure du 2026-09-22, ça ne suffit plus : tout le coût est dans les
> renderers. Reprendre la fonction `Arbre-Webview` de
> `docs/outils/mesurer-spike-overlay.ps1`.

- [ ] **Étape 2 : appliquer le critère de retour en arrière**

Conception §7 : si la latence à 15 personnages n'est **pas** sous 100 ms, ou si
le CPU dépasse **120 %** dans le cas « 15 personnages sur 3 écrans », la
migration a échoué et **la branche n'est pas fusionnée**. Écrire le verdict,
quel qu'il soit.

- [ ] **Étape 3 : écrire le document de mesure**

Mêmes sections que `docs/specs/2026-09-22-mesure-regulation.md` : le défaut, ce
que la migration change, les pièges de mesure rencontrés.

- [ ] **Étape 4 : mettre CLAUDE.md à jour**

Sections à corriger, faute de quoi le fichier décrira une architecture qui
n'existe plus :
- « Une fenêtre par personnage » → « Une fenêtre par écran », en gardant la
  trace de l'ancienne décision et de **pourquoi** elle a été renversée.
- « Où vit la logique » : ajouter le lissage et dire pourquoi ce n'est pas de
  la logique.
- « Décision n° 2 » et les trois horloges : ajouter la cadence d'envoi.
- « État actuel » : les nouveaux chiffres.
- « La prochaine action » : l'étape 4b redevient la suite.

- [ ] **Étape 5 : commit**

```bash
git add -A
git commit -m "docs(overlay): la mesure de la migration, et CLAUDE.md remis d'aplomb"
```
