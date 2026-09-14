# Le catalogue de personnages — plan d'implémentation

> **Pour les agents :** SOUS-COMPÉTENCE REQUISE — utiliser
> `superpowers:subagent-driven-development` (recommandé) ou
> `superpowers:executing-plans` pour exécuter ce plan tâche par tâche. Les
> étapes sont des cases à cocher (`- [ ]`).

**But :** remplacer les packs de personnages versionnés par un catalogue
parcouru et installé depuis l'application, dans une bibliothèque sous
`%APPDATA%`.

**Architecture :** parcourir est de la présentation (le webview tire les
vignettes du CDN, la CSP étant nulle) ; installer est de la logique (Rust,
réseau derrière un trait, fonctions pures testées sans réseau ni écran). Un
nouveau dossier `catalogue/` porte l'essentiel ; l'existant n'est touché que
par ajouts.

**Stack :** Rust, Tauri 2.11.5, crate `windows` 0.61 (WinHttp), crate `png`
(décodage seul), HTML/CSS/JS statique.

**Spec :** [`docs/specs/2026-09-11-catalogue-de-personnages-design.md`](../specs/2026-09-11-catalogue-de-personnages-design.md)
— le plan argumente depuis elle, lire les deux.

---

## Contraintes globales

Elles s'appliquent à **toutes** les tâches, sans être répétées dans chacune.

- **Branche : `catalogue-de-personnages`.** Vérifier `git branch` **avant chaque
  commit** : une session parallèle travaille sur `etape-4a-il-grimpe`, et le
  contexte de démarrage d'une session est un instantané, pas une vérité
  courante. Un commit qui atterrit sur la mauvaise branche est déjà arrivé.
- **Ne pas toucher `attach.rs`, `render.rs`, ni la boucle 60 Hz de `main.rs`.**
  C'est le terrain des étapes 4 et 5. Le diff doit le montrer d'un coup d'œil.
- **Compiler et tester depuis PowerShell**, jamais depuis Git Bash : rustc y
  pêche le `link.exe` de Git for Windows et rend une erreur `extra operand`
  opaque.
- `cargo test` **ne reconstruit pas l'exe** — faire `cargo build` avant de
  relancer l'application.
- **Tout JSON écrit l'est sans BOM** : `serde_json` refuse le BOM UTF-8 avec le
  message trompeur `expected value at line 1 column 1`.
- **Commentaires abondants, en français**, expliquant le *pourquoi* et citant la
  section de spec appliquée. L'auteur apprend Rust : préférer un `match`
  explicite à une chaîne de combinateurs, et expliquer toute construction non
  élémentaire (`let … else`, `Arc`/`Mutex`, durées de vie).
- **Chaque fichier s'ouvre sur deux ou trois lignes** disant sa responsabilité
  unique et la section de spec dont il relève.
- **Crate `windows` épinglée en `0.61`** — ne pas monter de version : deux
  versions majeures donnent deux types `HWND` distincts.
- **Une seule dépendance nouvelle autorisée : `png`.** Pas `image`, pas de client
  HTTP, pas de parseur XML. L'exe release fait 2,7 Mo et c'est un objectif tenu
  de la spec §4.
- **Un seul `on_menu_event` dans tout le programme**, installé par `tray.rs`.
- URL du CDN : `https://sprites.shimejis.xyz/directory/<slug>/img/shime<n>.png`
  et `https://sprites.shimejis.xyz/directory/<slug>/actions.xml`.

  > ⚠️ **Corrigé le 2026-09-14, à la première installation réelle.** Ce plan
  > annonçait `<slug>/conf/actions.xml` — le CDN y rend **404**, vérifié sur
  > quatre slugs. L'erreur était invisible : les tests servent un faux réseau,
  > et le repli sur l'ancre de convention est silencieux, si bien que les packs
  > s'installaient tous avec la même ancre. Et **deux schémas coexistent** :
  > `one-piece-luffy-01` est en balises japonaises (`画像`, `基準座標`),
  > `pierrot-54acb5` en anglais (`Image`, `ImageAnchor`). Le parseur lit les
  > deux depuis `installation.rs::ancres_avec`.
- `cargo test` doit rester **vert à chaque commit**.

---

## Structure des fichiers

| Fichier | Responsabilité |
|---|---|
| `src-tauri/src/catalogue/mod.rs` | **neuf** — orchestre : slug → dossier installé |
| `src-tauri/src/catalogue/reseau.rs` | **neuf** — le trait `Reseau`, WinHttp, et le faux |
| `src-tauri/src/catalogue/installation.rs` | **neuf** — fonctions pures : en-tête PNG, boîte opaque, `actions.xml`, poses, `mascot.json` |
| `src-tauri/src/catalogue/index.rs` | **neuf** — lire `catalogue.json` |
| `src-tauri/src/commandes.rs` | **neuf** — les trois commandes Tauri |
| `ui/catalogue.html`, `ui/catalogue.js` | **neufs** — la fenêtre du catalogue |
| `ui/catalogue.json` | **neuf** — l'index pré-généré |
| `src-tauri/src/config.rs` | +2 fonctions ; `resoudre` **intact** |
| `src-tauri/src/character/manifest.rs` | +1 champ facultatif `frames` |
| `src-tauri/src/{tray,menu_perso,actions}.rs` | +1 entrée de menu, +1 cas |
| `src-tauri/src/rechargement.rs` | signature simplifiée |
| `src-tauri/src/main.rs` | substitutions d'appel + enregistrement des commandes |
| `ui/pet.js` | `recharger` recalcule `BASE` |
| `tools/recuperer-packs.ps1` | réduit au mode `-Index` |

---

## Tâche 1 : la résolution bibliothèque → dépôt

**Fichiers :**
- Modifier : `src-tauri/src/config.rs` (après `dossier_personnages`, ~ligne 376)
- Modifier : `src-tauri/src/main.rs:198`, `src-tauri/src/main.rs:488`
- Modifier : `src-tauri/src/rechargement.rs` (signature de `preparer`)
- Modifier : `src-tauri/src/actions.rs` (le champ `dossier` et l'appel)

**Interfaces :**
- Consomme : rien.
- Produit :
  - `config::dossier_bibliotheque() -> Option<PathBuf>`
  - `config::dossier_du_personnage(nom: &str) -> Option<PathBuf>`
  - `config::personnage_dans(bibliotheque: Option<&Path>, livre: &Path, nom: &str) -> Option<PathBuf>`
  - `rechargement::preparer(demande: &Demande, dossier_perso: &Path) -> Result<u64, String>`

- [ ] **Étape 1 : écrire le test qui échoue**

Dans `src-tauri/src/config.rs`, module `tests` :

```rust
/// La bibliothèque gagne sur le dossier livré.
///
/// C'est le seul ordre défendable (spec §5) : un pack installé par
/// l'utilisateur qui porte le nom d'un pack livré doit gagner, sinon on
/// obtient un « je l'ai installé et il ne se passe rien » indébogable.
///
/// On teste la fonction PARAMÉTRÉE et jamais la publique : celle-ci lit
/// `%APPDATA%` par `env::var`, qui est global au processus et donc instable
/// quand `cargo test` tourne en parallèle (spec §11).
#[test]
fn la_bibliotheque_gagne_sur_le_dossier_livre() {
    let base = std::env::temp_dir().join("shimeji-test-resolution");
    let biblio = base.join("biblio");
    let livre = base.join("livre");
    // `all` : crée aussi les parents. L'erreur « existe déjà » est ignorée,
    // un test relancé retrouvant ses dossiers.
    let _ = std::fs::create_dir_all(biblio.join("blob"));
    let _ = std::fs::create_dir_all(livre.join("blob"));
    let _ = std::fs::create_dir_all(livre.join("seulement-livre"));

    assert_eq!(
        personnage_dans(Some(&biblio), &livre, "blob"),
        Some(biblio.join("blob")),
        "la bibliothèque doit gagner"
    );
    assert_eq!(
        personnage_dans(Some(&biblio), &livre, "seulement-livre"),
        Some(livre.join("seulement-livre")),
        "à défaut, le dossier livré"
    );
    assert_eq!(
        personnage_dans(Some(&biblio), &livre, "inexistant"),
        None,
        "introuvable partout → None"
    );
    assert_eq!(
        personnage_dans(None, &livre, "blob"),
        Some(livre.join("blob")),
        "sans bibliothèque, le dossier livré suffit"
    );
}
```

- [ ] **Étape 2 : lancer le test et vérifier qu'il échoue**

```powershell
cd C:\Users\alri\Documents\shimeji-desktop\src-tauri
cargo test la_bibliotheque_gagne
```
Attendu : ÉCHEC de compilation, `cannot find function personnage_dans`.

- [ ] **Étape 3 : écrire l'implémentation**

Dans `src-tauri/src/config.rs`, après `dossier_personnages` :

```rust
/// La bibliothèque : là où le catalogue INSTALLE (spec §5).
///
/// Contrairement à `resoudre`, elle ne CHERCHE pas : c'est une destination
/// d'écriture, toujours au même endroit. Rendre un chemin qui n'existe pas
/// encore est normal — c'est à l'installation de le créer.
///
/// `Option` parce que `%APPDATA%` peut manquer sur un système exotique, et
/// que l'absence de bibliothèque n'est pas une erreur : on retombe alors sur
/// le seul dossier livré.
pub fn dossier_bibliotheque() -> Option<PathBuf> {
    match std::env::var("APPDATA") {
        Ok(appdata) => Some(
            PathBuf::from(appdata)
                .join("shimeji-desktop")
                .join("characters"),
        ),
        Err(_) => None,
    }
}

/// Où trouver le personnage nommé `nom` : la bibliothèque D'ABORD, le
/// dossier livré ENSUITE (spec §5).
///
/// `resoudre` n'est volontairement PAS modifiée : ses appelants gardent
/// exactement le comportement qu'ils ont, ce qui rend ce changement sans
/// risque de régression.
pub fn dossier_du_personnage(nom: &str) -> Option<PathBuf> {
    // `as_deref` : `Option<PathBuf>` → `Option<&Path>`, sans copier le
    // chemin ni prendre la propriété de l'`Option` locale.
    let biblio = dossier_bibliotheque();
    personnage_dans(biblio.as_deref(), &dossier_personnages(), nom)
}

/// Le cœur de `dossier_du_personnage`, **paramétré par ses deux racines**.
///
/// Séparée pour une raison de test et non d'esthétique : la version publique
/// lit `%APPDATA%` par `env::var`, variable GLOBALE au processus. La modifier
/// dans un test la modifierait pour tous les tests tournant en parallèle —
/// le genre d'échec qui n'arrive qu'une fois sur dix (spec §11).
pub fn personnage_dans(
    bibliotheque: Option<&Path>,
    livre: &Path,
    nom: &str,
) -> Option<PathBuf> {
    // `if let Some(b)` : la bibliothèque peut ne pas exister, ce n'est pas
    // une erreur — on passe simplement au dossier livré.
    if let Some(b) = bibliotheque {
        let candidat = b.join(nom);
        if candidat.is_dir() {
            return Some(candidat);
        }
    }

    let candidat = livre.join(nom);
    if candidat.is_dir() {
        return Some(candidat);
    }

    None
}
```

- [ ] **Étape 4 : lancer le test et vérifier qu'il passe**

```powershell
cargo test la_bibliotheque_gagne
```
Attendu : PASS.

- [ ] **Étape 5 : brancher les trois appelants**

Dans `main.rs:198`, remplacer :

```rust
let manifeste = match character::manifest::Manifest::load(&dossier.join(&nom_personnage)) {
```

par :

```rust
    // Le personnage se cherche dans la bibliothèque PUIS dans le dossier
    // livré (spec §5). `let … else` : sans dossier, il n'y a rien à
    // afficher — autant échouer tout de suite avec un message qui NOMME le
    // personnage cherché.
    let Some(dossier_perso) = config::dossier_du_personnage(&nom_personnage) else {
        eprintln!("personnage « {nom_personnage} » introuvable (ni bibliothèque, ni dossier livré)");
        std::process::exit(1);
    };
    let manifeste = match character::manifest::Manifest::load(&dossier_perso) {
```

Dans `main.rs`, `servir_frame` (~ligne 488), remplacer :

```rust
    let fichier = dossier
        .join(personnage)
        .join("img")
        .join(format!("shime{n}.png"));
```

par :

```rust
    // Même résolution que le chargement : sinon un personnage installé dans
    // la bibliothèque s'animerait correctement en réclamant des images
    // introuvables. La validation du nom ci-dessus protège DEUX racines
    // désormais, ce qui la rend d'autant plus nécessaire.
    let Some(dossier_perso) = crate::config::dossier_du_personnage(personnage) else {
        return refus(404);
    };
    let fichier = dossier_perso.join("img").join(format!("shime{n}.png"));
```

⚠️ `servir_frame` prend aujourd'hui `dossier: &std::path::Path` en premier
paramètre. Il devient inutile : le retirer de la signature **et de son appel**
dans le `register_uri_scheme_protocol` de `setup`.

Dans `rechargement.rs`, `preparer` :

```rust
pub fn preparer(demande: &Demande, dossier_perso: &Path) -> Result<u64, String> {
    let manifeste = Manifest::load(dossier_perso)
        .map_err(|e| format!("manifeste illisible, rien n'a changé : {e}"))?;
    // … le reste inchangé
```

Dans `actions.rs`, le champ `dossier: PathBuf` d'`Actions` **désigne désormais
le dossier du personnage** (et non le dossier parent). Adapter `ID_RECHARGER` :

```rust
        ID_RECHARGER => {
            match crate::rechargement::preparer(&actions.demande, &actions.dossier) {
```

⚠️ `ID_DOSSIER` ouvre `actions.dossier` dans l'explorateur : il ouvrira
maintenant le dossier **du personnage**, ce qui est plus utile, pas moins.
Le commentaire de ce cas doit le dire.

Adapter enfin la construction d'`Actions::nouvelles` dans `main.rs` pour lui
passer `dossier_perso.clone()`.

- [ ] **Étape 6 : lancer toute la suite**

```powershell
cargo test
```
Attendu : PASS, aucun test en échec.

- [ ] **Étape 7 : vérifier que l'application démarre encore**

```powershell
cargo build
$env:SHIMEJI_QUITTER_APRES=5; cargo run
```
Attendu : `personnage chargé : Shimeji (mascotte par défaut) (… poses)`, une
fenêtre, puis sortie après 5 s sans message d'erreur.

- [ ] **Étape 8 : commit**

```powershell
git branch --show-current   # DOIT afficher catalogue-de-personnages
git add src-tauri/src/config.rs src-tauri/src/main.rs src-tauri/src/rechargement.rs src-tauri/src/actions.rs
git commit -m @'
feat(config): le personnage se cherche dans la bibliotheque puis le depot

Ajoute dossier_bibliotheque() et dossier_du_personnage(), sans toucher a
resoudre() : ses appelants gardent exactement leur comportement, ce qui rend
le changement sans risque de regression.

La bibliotheque gagne sur le dossier livre. C'est le seul ordre defendable :
un pack installe qui porte le nom d'un pack livre doit gagner, sinon on
obtient un « je l'ai installe et il ne se passe rien » indebogable.

Le coeur est parametre par ses deux racines et c'est LUI qui est teste : la
version publique lit %APPDATA% par env::var, global au processus, donc
instable quand cargo test tourne en parallele.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
'@
```

---

## Tâche 2 : la table `frames` du manifeste

Purement **additive** : le moteur ne la consomme pas encore (spec §6). Elle
est écrite dès maintenant pour que les packs installés deviennent corrects le
jour venu **sans être retéléchargés**.

**Fichiers :**
- Modifier : `src-tauri/src/character/manifest.rs`

**Interfaces :**
- Consomme : rien.
- Produit :
  - `manifest::InfoFrame { size: [u32; 2], anchor: Option<[f32; 2]> }`
  - `Manifest.frames: BTreeMap<u32, InfoFrame>`
  - `Manifest::taille_de_frame(&self, n: u32) -> [u32; 2]`
  - `Manifest::ancre_de_frame(&self, n: u32, pose: &str) -> [f32; 2]`

- [ ] **Étape 1 : écrire le test qui échoue**

Dans le module `tests` de `manifest.rs` :

```rust
/// La table `frames` renseigne taille et ancre par image ; son absence
/// laisse EXACTEMENT le comportement d'avant (spec §6).
///
/// C'est cette seconde moitié qui compte : `blob` n'a pas de table `frames`
/// et ne doit pas être retouché.
#[test]
fn la_table_frames_est_facultative_et_se_replie() {
    let avec = r#"{
        "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
        "hitbox": [40,20,48,100],
        "frames": {
            "23": { "size": [96,140], "anchor": [48,4] },
            "24": { "size": [90,138] }
        },
        "poses": { "stand": { "frames": [1], "anchor": [64,128] } }
    }"#;
    let m: Manifest = serde_json::from_str(avec).expect("manifeste valide");

    // Déclarée → on la lit.
    assert_eq!(m.taille_de_frame(23), [96, 140]);
    assert_eq!(m.ancre_de_frame(23, "stand"), [48.0, 4.0]);

    // Taille déclarée, ancre absente → repli sur l'ancre de la POSE.
    assert_eq!(m.taille_de_frame(24), [90, 138]);
    assert_eq!(m.ancre_de_frame(24, "stand"), [64.0, 128.0]);

    // Image absente de la table → repli complet sur le manifeste.
    assert_eq!(m.taille_de_frame(1), [128, 128]);
    assert_eq!(m.ancre_de_frame(1, "stand"), [64.0, 128.0]);

    // Pose inconnue → l'ancre par défaut, jamais un panic.
    assert_eq!(m.ancre_de_frame(1, "pose-qui-n-existe-pas"), [64.0, 128.0]);

    // Et sans table du tout : le comportement d'avant, à l'identique.
    let sans = r#"{
        "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
        "hitbox": [40,20,48,100],
        "poses": { "stand": { "frames": [1], "anchor": [64,128] } }
    }"#;
    let m2: Manifest = serde_json::from_str(sans).expect("manifeste valide");
    assert!(m2.frames.is_empty());
    assert_eq!(m2.taille_de_frame(1), [128, 128]);
    assert_eq!(m2.ancre_de_frame(1, "stand"), [64.0, 128.0]);
}
```

- [ ] **Étape 2 : lancer le test et vérifier qu'il échoue**

```powershell
cargo test la_table_frames_est_facultative
```
Attendu : ÉCHEC, `no method named taille_de_frame`.

- [ ] **Étape 3 : écrire l'implémentation**

Dans `manifest.rs`, à côté de `Pose` :

```rust
/// Ce qu'on sait d'UNE image, indépendamment des poses qui l'emploient.
///
/// Pourquoi une table séparée plutôt que des champs sur `Pose` : la taille et
/// l'ancre sont des propriétés de l'IMAGE, et une même image sert dans
/// plusieurs poses (spec §6). Les porter sur la pose les dupliquerait, donc
/// les ferait diverger.
#[derive(Debug, Clone, Deserialize)]
pub struct InfoFrame {
    /// Dimensions réelles du PNG, lues à l'installation dans son en-tête.
    pub size: [u32; 2],

    /// Ancre propre à cette image. Absente → celle de la pose.
    ///
    /// `Option` et non une valeur par défaut : « non déclarée » et « déclarée
    /// à l'ancre de la pose » doivent rester distinguables, sans quoi on ne
    /// saurait pas s'il faut se replier.
    #[serde(default)]
    pub anchor: Option<[f32; 2]>,
}
```

Dans la struct `Manifest`, après `frame_size` :

```rust
    /// Taille et ancre par image (spec §6).
    ///
    /// **Facultative, et pas encore consommée par le moteur.** Elle est
    /// écrite dès maintenant par l'installation pour que les packs déjà
    /// installés deviennent corrects sans être retéléchargés, le jour où la
    /// fenêtre adaptative arrivera.
    ///
    /// `BTreeMap<u32, …>` et non un `Vec` : les numéros de frames sont épars
    /// (un pack peut avoir 1..25 puis 42..46), un `Vec` serait donc troué.
    #[serde(default)]
    pub frames: BTreeMap<u32, InfoFrame>,
```

Dans `impl Manifest` :

```rust
    /// La taille de l'image `n`, ou celle du manifeste à défaut.
    ///
    /// Le repli n'est pas un cas d'erreur : c'est le cas NORMAL pour tout
    /// pack écrit à la main, `blob` compris.
    pub fn taille_de_frame(&self, n: u32) -> [u32; 2] {
        match self.frames.get(&n) {
            Some(info) => info.size,
            None => self.frame_size,
        }
    }

    /// L'ancre de l'image `n` dans le contexte de la pose `pose`.
    ///
    /// Trois niveaux de repli, du plus précis au plus général : l'ancre de
    /// l'image, puis celle de la pose, puis celle par défaut. Une pose
    /// inconnue rend l'ancre par défaut plutôt que `None` — comme
    /// `hitbox_de`, et pour la même raison : l'appelant est dans la boucle
    /// 60 Hz et n'a rien à faire d'un cas d'erreur.
    pub fn ancre_de_frame(&self, n: u32, pose: &str) -> [f32; 2] {
        // `and_then` : la table peut ne pas connaître l'image, ET l'image
        // peut ne pas déclarer d'ancre. Deux `Option` à traverser.
        if let Some(a) = self.frames.get(&n).and_then(|info| info.anchor) {
            return a;
        }

        match self.poses.get(pose) {
            Some(p) => p.anchor,
            None => ancre_par_defaut(),
        }
    }
```

- [ ] **Étape 4 : lancer les tests**

```powershell
cargo test
```
Attendu : PASS, y compris tous les tests existants de `manifest.rs` — le
champ étant `#[serde(default)]`, aucun manifeste existant n'est invalidé.

- [ ] **Étape 5 : vérifier que `blob` charge toujours**

```powershell
cargo build
$env:SHIMEJI_QUITTER_APRES=5; cargo run
```
Attendu : `personnage chargé : Shimeji (mascotte par défaut)`.

- [ ] **Étape 6 : commit**

```powershell
git add src-tauri/src/character/manifest.rs
git commit -m @'
feat(manifest): la table `frames` — taille et ancre par image

Purement additive : le moteur ne la consomme pas encore. Elle est ecrite
des maintenant par l'installation pour que les packs installes deviennent
corrects sans etre retelecharges, le jour ou la fenetre adaptative arrivera
(spec section 6).

La table est separee des poses parce que la taille et l'ancre sont des
proprietes de l'IMAGE, et qu'une meme image sert dans plusieurs poses : les
porter sur la pose les dupliquerait, donc les ferait diverger.

Tout est facultatif et se replie sur le comportement actuel exact, ce qui
laisse blob intact — c'est ce qui rend le changement sur.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
'@
```

---

## Tâche 3 : le réseau derrière un trait

**Fichiers :**
- Créer : `src-tauri/src/catalogue/mod.rs` (déclaration des sous-modules)
- Créer : `src-tauri/src/catalogue/reseau.rs`
- Modifier : `src-tauri/Cargo.toml` (feature `Win32_Networking_WinHttp`)
- Modifier : `src-tauri/src/main.rs` (ajouter `mod catalogue;`)

**Interfaces :**
- Consomme : rien.
- Produit :
  - `catalogue::reseau::Reseau` — trait : `fn get(&self, url: &str) -> Resultat`
  - `type Resultat = Result<Option<Vec<u8>>, String>` — `Ok(None)` = 404 (absence normale), `Err` = panne
  - `catalogue::reseau::ReseauWinHttp::new() -> ReseauWinHttp`
  - `catalogue::reseau::ReseauFake::new(reponses: Vec<(String, Option<Vec<u8>>)>) -> ReseauFake`
  - `ReseauFake::echouer_apres(&self, n: usize)`

- [ ] **Étape 1 : écrire le test qui échoue**

Dans `src-tauri/src/catalogue/reseau.rs`, module `tests` :

```rust
/// Le faux réseau rend ce qu'on lui a dit, et sait tomber en panne.
///
/// `echouer_apres` existe pour LE cas de test qui compte : la coupure à
/// mi-installation (spec §11). Sans lui, on ne testerait que le chemin
/// heureux, qui n'est pas celui qui casse.
#[test]
fn le_faux_reseau_rend_et_echoue_a_la_demande() {
    let r = ReseauFake::new(vec![
        ("https://x/1".to_string(), Some(vec![1, 2, 3])),
        ("https://x/2".to_string(), None), // 404 : absence NORMALE
    ]);

    assert_eq!(r.get("https://x/1"), Ok(Some(vec![1, 2, 3])));
    assert_eq!(r.get("https://x/2"), Ok(None), "404 n'est pas une panne");
    assert_eq!(r.get("https://x/inconnue"), Ok(None), "non déclarée = absente");

    // Après deux requêtes réussies, tout échoue.
    let r2 = ReseauFake::new(vec![("https://x/1".to_string(), Some(vec![9]))]);
    r2.echouer_apres(1);
    assert_eq!(r2.get("https://x/1"), Ok(Some(vec![9])));
    assert!(r2.get("https://x/1").is_err(), "la 2e requête doit tomber");
}
```

- [ ] **Étape 2 : lancer et vérifier l'échec**

```powershell
cargo test le_faux_reseau_rend
```
Attendu : ÉCHEC, module introuvable.

- [ ] **Étape 3 : créer le module et le trait**

`src-tauri/src/catalogue/mod.rs` :

```rust
//! Le catalogue de personnages : parcourir, installer (spec §4).
//!
//! Responsabilité unique : transformer un slug du catalogue shimejis.xyz en
//! un dossier de personnage utilisable. Ne sait rien de l'affichage.

pub mod index;
pub mod installation;
pub mod reseau;
```

⚠️ `index` et `installation` n'existent pas encore : créer deux fichiers vides
avec seulement leur en-tête de module, sinon la compilation échoue.

`src-tauri/src/catalogue/reseau.rs` :

```rust
//! La SEULE pièce du catalogue qui touche au réseau (spec §4, §11).
//!
//! Responsabilité unique : rendre les octets d'une URL. Aucune connaissance
//! des PNG, des manifestes ni des slugs.
//!
//! # Pourquoi un trait
//!
//! Pour que l'installation entière se teste **sans sortir de la machine**
//! (spec §11) — y compris le cas qui compte vraiment : la coupure à
//! mi-parcours. C'est le même motif que `probe::SystemProbe`, qui sépare
//! déjà « ce qu'on a besoin de savoir » de « comment on l'apprend ».

/// Ce que rend une requête.
///
/// Trois issues et non deux, et la distinction est essentielle :
///   · `Ok(Some(octets))` — la ressource existe ;
///   · `Ok(None)` — 404. **Une absence NORMALE**, pas une erreur : c'est
///     ainsi qu'on découvre qu'un pack n'a que 30 frames ;
///   · `Err(message)` — une panne. Elle doit interrompre l'installation.
///
/// Les confondre ferait qu'une coupure réseau serait prise pour « le pack
/// s'arrête ici », et produirait un personnage amputé présenté comme complet.
pub type Resultat = Result<Option<Vec<u8>>, String>;

pub trait Reseau {
    fn get(&self, url: &str) -> Resultat;
}
```

- [ ] **Étape 4 : écrire le faux réseau**

À la suite, dans `reseau.rs` :

```rust
use std::cell::Cell;

/// Le réseau des tests : une table d'URL connues, et une panne à la demande.
pub struct ReseauFake {
    reponses: Vec<(String, Option<Vec<u8>>)>,

    /// `Cell` pour la même raison que dans `FakeProbe` : le trait expose
    /// `&self`, donc un test qui n'a qu'une référence partagée doit pouvoir
    /// faire avancer le compteur. `usize` est `Copy`, ce que `Cell` exige.
    servies: Cell<usize>,
    echec_apres: Cell<Option<usize>>,
}

impl ReseauFake {
    pub fn new(reponses: Vec<(String, Option<Vec<u8>>)>) -> Self {
        ReseauFake {
            reponses,
            servies: Cell::new(0),
            echec_apres: Cell::new(None),
        }
    }

    /// Après `n` requêtes servies, toutes les suivantes échouent.
    pub fn echouer_apres(&self, n: usize) {
        self.echec_apres.set(Some(n));
    }
}

impl Reseau for ReseauFake {
    fn get(&self, url: &str) -> Resultat {
        if let Some(seuil) = self.echec_apres.get() {
            if self.servies.get() >= seuil {
                return Err(format!("panne simulée sur {url}"));
            }
        }
        self.servies.set(self.servies.get() + 1);

        // Une URL non déclarée est traitée comme un 404 : c'est ce qui rend
        // les tests courts — on ne déclare que ce qui existe.
        match self.reponses.iter().find(|(u, _)| u == url) {
            Some((_, corps)) => Ok(corps.clone()),
            None => Ok(None),
        }
    }
}
```

- [ ] **Étape 5 : lancer le test et vérifier qu'il passe**

```powershell
cargo test le_faux_reseau_rend
```
Attendu : PASS.

- [ ] **Étape 6 : ajouter la feature WinHttp**

Dans `src-tauri/Cargo.toml`, section `[dependencies.windows]`, ajouter à la
liste `features` :

```toml
    "Win32_Networking_WinHttp",         # WinHttpOpen… — le telechargement du catalogue
```

- [ ] **Étape 7 : écrire l'implémentation WinHttp**

À la suite de `reseau.rs` :

```rust
// ── Le vrai réseau : WinHttp ────────────────────────────────────────────
//
// Pourquoi l'API de Windows plutôt qu'une crate HTTP : **aucune dépendance
// nouvelle** (la crate `windows` est déjà épinglée en 0.61) et **aucune pile
// TLS embarquée** dans l'exe, qui fait 2,7 Mo et doit le rester (spec §4).
// C'est aussi la logique du reste du projet : l'API Windows typée plutôt
// qu'un portage.

use std::ffi::c_void;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Networking::WinHttp::*;

pub struct ReseauWinHttp;

impl ReseauWinHttp {
    pub fn new() -> Self {
        ReseauWinHttp
    }
}

/// Un handle WinHttp qui se ferme tout seul.
///
/// `Drop` plutôt qu'un `WinHttpCloseHandle` à la main à chaque sortie : il y
/// a six chemins de sortie dans `get`, et en oublier un fuirait un handle à
/// chaque image manquante. C'est exactement ce à quoi sert `Drop` en Rust —
/// le destructeur est appelé quoi qu'il arrive, y compris sur un `return`
/// anticipé.
struct Handle(*mut c_void);

impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // `unsafe` : on rend un handle au système. L'erreur éventuelle
            // n'a aucun recours utile.
            unsafe {
                let _ = WinHttpCloseHandle(self.0);
            }
        }
    }
}

impl Reseau for ReseauWinHttp {
    fn get(&self, url: &str) -> Resultat {
        // ── Découper l'URL ──────────────────────────────────────────────
        // On n'accepte QUE https, et on découpe à la main plutôt que
        // d'ajouter une crate d'URL pour deux `split`.
        let reste = match url.strip_prefix("https://") {
            Some(r) => r,
            None => return Err(format!("URL non https : {url}")),
        };
        let (hote, chemin) = match reste.find('/') {
            Some(i) => (&reste[..i], &reste[i..]),
            None => (reste, "/"),
        };

        // `HSTRING` convertit en UTF-16 terminé par un nul, ce qu'attendent
        // toutes les API W de Windows. La variable doit VIVRE aussi
        // longtemps que le `PCWSTR` qui la pointe — d'où les `let` séparés
        // plutôt que des temporaires dans l'appel.
        let hote_w = HSTRING::from(hote);
        let chemin_w = HSTRING::from(chemin);
        let agent_w = HSTRING::from("shimeji-desktop/0.1");

        unsafe {
            let session = Handle(WinHttpOpen(
                PCWSTR(agent_w.as_ptr()),
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            ));
            if session.0.is_null() {
                return Err("WinHttpOpen a échoué".to_string());
            }

            let connexion = Handle(WinHttpConnect(
                session.0,
                PCWSTR(hote_w.as_ptr()),
                INTERNET_DEFAULT_HTTPS_PORT,
                0,
            ));
            if connexion.0.is_null() {
                return Err(format!("connexion à {hote} impossible"));
            }

            let requete = Handle(WinHttpOpenRequest(
                connexion.0,
                PCWSTR::null(), // verbe nul = GET
                PCWSTR(chemin_w.as_ptr()),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null_mut(),
                WINHTTP_FLAG_SECURE,
            ));
            if requete.0.is_null() {
                return Err(format!("requête sur {chemin} impossible"));
            }

            WinHttpSendRequest(requete.0, PCWSTR::null(), 0, None, 0, 0, 0)
                .map_err(|e| format!("envoi : {e}"))?;
            WinHttpReceiveResponse(requete.0, std::ptr::null_mut())
                .map_err(|e| format!("réponse : {e}"))?;

            // ── Le code de statut ───────────────────────────────────────
            // 404 n'est PAS une erreur : c'est ainsi qu'on découvre la fin
            // d'un pack. Tout autre code hors 200 l'est.
            let mut statut: u32 = 0;
            let mut taille = std::mem::size_of::<u32>() as u32;
            WinHttpQueryHeaders(
                requete.0,
                WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                PCWSTR::null(),
                Some(&mut statut as *mut u32 as *mut c_void),
                &mut taille,
                std::ptr::null_mut(),
            )
            .map_err(|e| format!("statut : {e}"))?;

            if statut == 404 {
                return Ok(None);
            }
            if statut != 200 {
                return Err(format!("statut {statut} sur {url}"));
            }

            // ── Lire le corps ───────────────────────────────────────────
            // Boucle en deux temps, imposée par l'API : demander combien
            // d'octets sont disponibles, puis les lire.
            let mut corps: Vec<u8> = Vec::new();
            loop {
                let mut dispo: u32 = 0;
                WinHttpQueryDataAvailable(requete.0, &mut dispo)
                    .map_err(|e| format!("lecture : {e}"))?;
                if dispo == 0 {
                    break;
                }

                let debut = corps.len();
                corps.resize(debut + dispo as usize, 0);
                let mut lus: u32 = 0;
                WinHttpReadData(
                    requete.0,
                    corps[debut..].as_mut_ptr() as *mut c_void,
                    dispo,
                    &mut lus,
                )
                .map_err(|e| format!("lecture : {e}"))?;

                // On tronque à ce qui a RÉELLEMENT été lu : `dispo` est un
                // majorant, pas une promesse.
                corps.truncate(debut + lus as usize);
            }

            Ok(Some(corps))
        }
    }
}
```

⚠️ **Les signatures exactes de `windows` 0.61 doivent être vérifiées à la
compilation** — cette crate change de conventions entre versions (certains
paramètres passent de pointeurs bruts à `Option<&…>`). Si un appel ne compile
pas, consulter `cargo doc -p windows --open` et adapter **l'appel seul** : la
structure ci-dessus (session → connexion → requête → statut → corps) est celle
de WinHttp et ne change pas.

- [ ] **Étape 8 : ajouter le module à `main.rs`**

Auprès des autres `mod` :

```rust
mod catalogue;
```

- [ ] **Étape 9 : compiler et lancer la suite**

```powershell
cargo test
```
Attendu : compilation réussie, tous les tests PASS.

- [ ] **Étape 10 : commit**

```powershell
git add src-tauri/Cargo.toml src-tauri/src/catalogue/ src-tauri/src/main.rs
git commit -m @'
feat(catalogue): le reseau derriere un trait, WinHttp et son faux

WinHttp via la crate windows deja epinglee en 0.61 : aucune dependance
nouvelle, aucune pile TLS embarquee dans un exe de 2,7 Mo qui doit le
rester. C'est aussi la logique du reste du projet — l'API Windows typee
plutot qu'un portage.

Le trait existe pour que l'installation entiere se teste sans sortir de la
machine, y compris le cas qui compte : la coupure a mi-parcours, d'ou
`echouer_apres` sur le faux.

Trois issues et non deux : Ok(None) est un 404, donc une absence NORMALE —
c'est ainsi qu'on decouvre qu'un pack n'a que 30 frames. Les confondre avec
une panne produirait un personnage ampute presente comme complet.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
'@
```

---

## Tâche 4 : l'en-tête PNG et la hitbox mesurée

**Fichiers :**
- Modifier : `src-tauri/src/catalogue/installation.rs`
- Modifier : `src-tauri/Cargo.toml` (dépendance `png`)

**Interfaces :**
- Consomme : rien.
- Produit :
  - `installation::taille_png(octets: &[u8]) -> Option<[u32; 2]>`
  - `installation::boite_opaque(rgba: &[u8], largeur: u32, hauteur: u32) -> Option<[u32; 4]>` — `[x0, y0, x1, y1]` inclusifs
  - `installation::hitbox_depuis(rgba: &[u8], largeur: u32, hauteur: u32) -> [u32; 4]` — `[x, y, w, h]`
  - `installation::decoder_png(octets: &[u8]) -> Option<(Vec<u8>, u32, u32)>` — RGBA

- [ ] **Étape 1 : écrire les tests qui échouent**

```rust
/// La taille se lit dans l'en-tête, SANS décoder l'image.
///
/// Un PNG commence par 8 octets de signature, puis un chunk IHDR dont les
/// deux premiers champs sont largeur et hauteur en gros-boutiste. C'est tout
/// ce dont on a besoin, et ça évite de décoder 46 images par pack (spec §6).
#[test]
fn la_taille_se_lit_dans_l_en_tete_png() {
    // Signature PNG, longueur de chunk (13), "IHDR", largeur 185, hauteur 155.
    let mut octets = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    octets.extend_from_slice(&13u32.to_be_bytes());
    octets.extend_from_slice(b"IHDR");
    octets.extend_from_slice(&185u32.to_be_bytes());
    octets.extend_from_slice(&155u32.to_be_bytes());

    assert_eq!(taille_png(&octets), Some([185, 155]));

    // Ce qui n'est pas un PNG est REFUSÉ, jamais deviné : le CDN pourrait
    // servir une page d'erreur HTML avec un statut 200.
    assert_eq!(taille_png(b"<!doctype html><html>"), None);
    assert_eq!(taille_png(&[]), None);
    assert_eq!(taille_png(&octets[..12]), None, "tronqué → refus");
}

/// La hitbox est MESURÉE sur les pixels opaques, jamais recopiée sur blob.
///
/// C'est la leçon de l'étape 1a, où les quatre réglages faits à l'œil se sont
/// tous révélés faux — et celle de Luffy, chibi dont le chapeau touche le
/// bord haut de la boîte, que le `y = 20` de blob aurait amputé.
#[test]
fn la_hitbox_est_mesuree_sur_les_pixels_opaques() {
    // Une image 4×4 dont seul le carré (1,1)-(2,2) est opaque.
    let mut rgba = vec![0u8; 4 * 4 * 4];
    for y in 1..=2u32 {
        for x in 1..=2u32 {
            let i = ((y * 4 + x) * 4) as usize;
            rgba[i + 3] = 255; // alpha
        }
    }

    assert_eq!(boite_opaque(&rgba, 4, 4), Some([1, 1, 2, 2]));
    // [x, y, largeur, hauteur] — les bornes étant inclusives, 2-1+1 = 2.
    assert_eq!(hitbox_depuis(&rgba, 4, 4), [1, 1, 2, 2]);

    // Une image entièrement transparente n'a pas de boîte. La hitbox se
    // replie alors sur l'image entière : mieux vaut une hitbox trop large
    // qu'un personnage impossible à attraper.
    let vide = vec![0u8; 4 * 4 * 4];
    assert_eq!(boite_opaque(&vide, 4, 4), None);
    assert_eq!(hitbox_depuis(&vide, 4, 4), [0, 0, 4, 4]);
}
```

- [ ] **Étape 2 : lancer et vérifier l'échec**

```powershell
cargo test la_taille_se_lit; cargo test la_hitbox_est_mesuree
```
Attendu : ÉCHEC, fonctions introuvables.

- [ ] **Étape 3 : ajouter la dépendance `png`**

Dans `Cargo.toml`, section `[dependencies]` :

```toml
# Décodage PNG, pour la SEULE image dont on mesure la boîte opaque (la
# hitbox). Pas `image` : on ne veut que le PNG, et l'exe release fait 2,7 Mo,
# un objectif tenu de la spec §4. Les 45 autres frames ne sont jamais
# décodées — leur taille se lit dans l'en-tête (spec §6).
png = "0.17"
```

- [ ] **Étape 4 : écrire l'implémentation**

`src-tauri/src/catalogue/installation.rs` :

```rust
//! Les fonctions pures de l'installation (spec §8, §11).
//!
//! Responsabilité unique : transformer des octets en données de manifeste.
//! Aucun réseau, aucun disque — c'est ce qui les rend testables sur des
//! images fabriquées en mémoire.

/// La signature qui ouvre tout fichier PNG.
const SIGNATURE_PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Largeur et hauteur, lues dans l'en-tête **sans décoder l'image**.
///
/// Structure d'un PNG : 8 octets de signature, puis des chunks. Le premier
/// est toujours IHDR, dont les deux premiers champs sont largeur et hauteur
/// sur 4 octets en gros-boutiste. On n'a besoin que des 24 premiers octets.
///
/// Rend `None` sur tout ce qui n'est pas un PNG : le CDN pourrait servir une
/// page d'erreur HTML avec un statut 200, et la prendre pour une image
/// écrirait une taille absurde dans le manifeste.
pub fn taille_png(octets: &[u8]) -> Option<[u32; 2]> {
    if octets.len() < 24 {
        return None;
    }
    if octets[..8] != SIGNATURE_PNG {
        return None;
    }
    if &octets[12..16] != b"IHDR" {
        return None;
    }

    // `try_into` rend un `Result` parce que la tranche pourrait ne pas faire
    // 4 octets ; ici la longueur est déjà vérifiée, d'où `ok()?` qui est
    // inatteignable mais évite un `unwrap`.
    let l = u32::from_be_bytes(octets[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(octets[20..24].try_into().ok()?);

    // Une dimension nulle n'est pas une image.
    if l == 0 || h == 0 {
        return None;
    }

    Some([l, h])
}

/// Décode un PNG en RGBA. **Une seule image par installation.**
pub fn decoder_png(octets: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let decodeur = png::Decoder::new(octets);
    // `ok()?` partout : un PNG illisible n'est pas un cas d'erreur à
    // remonter, c'est un pack dont on mesurera la hitbox autrement.
    let mut lecteur = decodeur.read_info().ok()?;
    let mut tampon = vec![0u8; lecteur.output_buffer_size()];
    let info = lecteur.next_frame(&mut tampon).ok()?;

    // On n'accepte que le RGBA 8 bits : c'est ce que servent tous les packs
    // Shimeji, et convertir les autres formats coûterait la crate `image`
    // entière pour un cas qui ne se présente pas.
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }

    tampon.truncate(info.buffer_size());
    Some((tampon, info.width, info.height))
}

/// La boîte englobante des pixels non transparents, bornes **inclusives**.
///
/// Rend `None` si l'image est entièrement transparente — ce qui arrive sur
/// certaines frames de packs incomplets.
pub fn boite_opaque(rgba: &[u8], largeur: u32, hauteur: u32) -> Option<[u32; 4]> {
    // Un pixel est jugé opaque au-delà de ce seuil. Pas `> 0` : les bords
    // anticrénelés portent un alpha de 1 ou 2 qui gonflerait la boîte de
    // plusieurs pixels sans qu'on y voie rien.
    const SEUIL_ALPHA: u8 = 16;

    let mut x0 = u32::MAX;
    let mut y0 = u32::MAX;
    let mut x1 = 0u32;
    let mut y1 = 0u32;
    let mut trouve = false;

    for y in 0..hauteur {
        for x in 0..largeur {
            let i = ((y * largeur + x) * 4 + 3) as usize;
            // Garde de longueur : une image tronquée ne doit pas paniquer.
            if i >= rgba.len() {
                continue;
            }
            if rgba[i] > SEUIL_ALPHA {
                trouve = true;
                if x < x0 { x0 = x; }
                if y < y0 { y0 = y; }
                if x > x1 { x1 = x; }
                if y > y1 { y1 = y; }
            }
        }
    }

    if trouve {
        Some([x0, y0, x1, y1])
    } else {
        None
    }
}

/// La hitbox `[x, y, largeur, hauteur]` du manifeste.
///
/// Repli sur l'image entière quand rien n'est opaque : mieux vaut une hitbox
/// trop large qu'un personnage impossible à attraper (spec §3.3).
pub fn hitbox_depuis(rgba: &[u8], largeur: u32, hauteur: u32) -> [u32; 4] {
    match boite_opaque(rgba, largeur, hauteur) {
        // Bornes inclusives → `+ 1` pour obtenir une dimension.
        Some([x0, y0, x1, y1]) => [x0, y0, x1 - x0 + 1, y1 - y0 + 1],
        None => [0, 0, largeur, hauteur],
    }
}
```

- [ ] **Étape 5 : lancer les tests**

```powershell
cargo test taille_png; cargo test hitbox
```
Attendu : PASS.

- [ ] **Étape 6 : vérifier sur un vrai PNG**

Ajouter ce test, qui lit un fichier **réellement** présent dans le dépôt :

```rust
/// Un vrai PNG du dépôt, pour que le test ne prouve pas seulement que notre
/// fabrication d'octets est cohérente avec notre lecture.
#[test]
fn la_taille_se_lit_sur_une_vraie_frame() {
    // Chemin relatif au dossier du CRATE (`src-tauri/`), d'où le `..`.
    let chemin = std::path::Path::new("../characters/blob/img/shime1.png");
    let octets = std::fs::read(chemin).expect("blob/shime1.png doit exister");
    assert_eq!(taille_png(&octets), Some([128, 128]));

    let (rgba, l, h) = decoder_png(&octets).expect("PNG RGBA 8 bits");
    assert_eq!((l, h), (128, 128));
    let hb = hitbox_depuis(&rgba, l, h);
    assert!(hb[2] > 0 && hb[3] > 0, "blob n'est pas transparent : {hb:?}");
}
```

```powershell
cargo test la_taille_se_lit_sur_une_vraie_frame
```
Attendu : PASS.

- [ ] **Étape 7 : commit**

```powershell
git add src-tauri/Cargo.toml src-tauri/src/catalogue/installation.rs
git commit -m @'
feat(catalogue): l'en-tete PNG et la hitbox mesuree

La taille de chaque frame se lit dans les 24 premiers octets du fichier,
sans decoder : c'est ce qui evite 46 decodages par pack. Une seule image est
decodee, pour mesurer la boite opaque dont sort la hitbox.

La hitbox est MESUREE et non recopiee sur blob — la lecon de l'etape 1a, ou
les quatre reglages faits a l'oeil se sont tous reveles faux, et celle de
Luffy dont le chapeau touche le bord haut de la boite.

Ce qui n'est pas un PNG est refuse et jamais devine : le CDN peut servir une
page d'erreur HTML avec un statut 200, et la prendre pour une image ecrirait
une taille absurde dans le manifeste.

Seuil d'alpha a 16 et non a 0 : les bords anticreneles portent un alpha de 1
ou 2 qui gonflerait la boite sans qu'on y voie rien.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
'@
```

---

## Tâche 5 : les ancres, les poses retenues, le `mascot.json`

**Fichiers :**
- Modifier : `src-tauri/src/catalogue/installation.rs`

**Interfaces :**
- Consomme : `taille_png`, `hitbox_depuis` (Tâche 4).
- Produit :
  - `installation::POSES: &[DefPose]` avec `struct DefPose { nom, frames, frame_ms, looping, ancre }`
  - `installation::ancres_de_actions_xml(xml: &str) -> BTreeMap<u32, [f32; 2]>`
  - `installation::poses_retenues(presentes: &BTreeSet<u32>) -> Vec<&'static DefPose>`
  - `installation::ecrire_mascot_json(slug, presentes, tailles, ancres, hitbox) -> Result<String, String>`

- [ ] **Étape 1 : écrire les tests qui échouent**

```rust
use std::collections::BTreeSet;

/// Les ancres se lisent dans l'`actions.xml` du pack, servi par le CDN.
///
/// C'est LA source de vérité : les proportions d'un dessin ne se transposent
/// pas d'un pack à l'autre, seule la numérotation des poses le fait.
#[test]
fn les_ancres_se_lisent_dans_actions_xml() {
    let xml = r#"<?xml version="1.0"?>
    <Mascot>
      <Pose Image="/shime1.png" ImageAnchor="64,128" Duration="1"/>
      <Pose Image="/shime23.png" ImageAnchor="48,4" Duration="1"/>
      <Pose Image="/pas-un-numero.png" ImageAnchor="1,2" Duration="1"/>
    </Mascot>"#;

    let a = ancres_de_actions_xml(xml);
    assert_eq!(a.get(&1), Some(&[64.0, 128.0]));
    assert_eq!(a.get(&23), Some(&[48.0, 4.0]));
    assert_eq!(a.len(), 2, "les images non numérotées sont ignorées");

    // Un XML absent ou illisible ne casse rien : table vide, et l'appelant
    // se repliera sur la convention de Shimeji-ee (milieu-bas).
    assert!(ancres_de_actions_xml("").is_empty());
    assert!(ancres_de_actions_xml("<html>erreur 500</html>").is_empty());
}

/// On ne déclare QUE les poses dont TOUTES les frames existent (spec §8.7).
///
/// Un pack incomplet n'est pas une erreur : l'option est simplement retirée
/// du tirage d'envies. Aucun cas particulier à coder — il suffit de ne pas
/// déclarer la pose.
#[test]
fn seules_les_poses_completes_sont_retenues() {
    // Un pack qui n'a que 1, 2, 3 : de quoi tenir debout et marcher, rien
    // de plus.
    let presentes: BTreeSet<u32> = [1, 2, 3].into_iter().collect();
    let retenues = poses_retenues(&presentes);
    let noms: Vec<&str> = retenues.iter().map(|p| p.nom).collect();

    assert!(noms.contains(&"stand"), "stand n'a besoin que de la frame 1");
    assert!(noms.contains(&"walk"), "walk a besoin de 1, 2, 3");
    assert!(!noms.contains(&"climbWall"), "climbWall a besoin de 12, 13, 14");
    assert!(!noms.contains(&"sleep"), "sleep a besoin de la frame 21");
}

/// Le manifeste écrit est du JSON valide, sans BOM, et relisible par le
/// moteur — c'est la seule vérification qui compte vraiment.
#[test]
fn le_mascot_json_ecrit_est_relisible_par_le_moteur() {
    let presentes: BTreeSet<u32> = (1..=46).collect();
    let mut tailles = std::collections::BTreeMap::new();
    let mut ancres = std::collections::BTreeMap::new();
    for n in 1..=46u32 {
        tailles.insert(n, [128u32, 128u32]);
        ancres.insert(n, [64.0f32, 128.0f32]);
    }

    let json = ecrire_mascot_json("un-slug", &presentes, &tailles, &ancres, [40, 20, 48, 108])
        .expect("génération");

    assert!(!json.starts_with('\u{feff}'), "jamais de BOM");

    // LA vérification : le moteur doit savoir le relire.
    let m: crate::character::manifest::Manifest =
        serde_json::from_str(&json).expect("le moteur doit relire ce qu'on écrit");
    assert_eq!(m.id, "un-slug");
    assert!(m.poses.contains_key("stand"));
    assert!(m.poses.contains_key("walk"));
    assert_eq!(m.taille_de_frame(1), [128, 128]);
    assert_eq!(m.frames.len(), 46, "la table frames est écrite (spec §6)");
}
```

- [ ] **Étape 2 : lancer et vérifier l'échec**

```powershell
cargo test les_ancres_se_lisent; cargo test seules_les_poses; cargo test le_mascot_json
```
Attendu : ÉCHEC, fonctions introuvables.

- [ ] **Étape 3 : écrire la table des poses**

Dans `installation.rs` :

```rust
use std::collections::{BTreeMap, BTreeSet};

/// La définition d'une pose du vocabulaire Shimeji.
///
/// `&'static str` et tableaux statiques : la table entière est connue à la
/// compilation, donc rien n'est alloué.
pub struct DefPose {
    pub nom: &'static str,
    pub frames: &'static [u32],
    pub frame_ms: u32,
    pub looping: bool,
    /// Ancre imposée par la pose, quand elle ne se déduit pas du dessin.
    pub ancre: Option<[f32; 2]>,
}

/// Le vocabulaire de poses = les slots Shimeji.
///
/// La numérotation `shime1..46` est un **standard de fait** : tous les packs
/// utilisent les mêmes numéros pour les mêmes poses. En visant ce
/// vocabulaire, tout pack du catalogue fonctionne sans code (spec §8).
///
/// Correspondance tirée de `conf/actions.xml` de Shimeji-ee, relevée dans
/// `docs/specs/2026-09-09-frames-shimeji.md` — **pas devinée à l'œil**.
pub const POSES: &[DefPose] = &[
    DefPose { nom: "stand", frames: &[1], frame_ms: 150, looping: false, ancre: None },
    DefPose { nom: "walk", frames: &[1, 2, 1, 3], frame_ms: 240, looping: true, ancre: None },
    DefPose { nom: "run", frames: &[1, 2, 1, 3], frame_ms: 80, looping: true, ancre: None },
    DefPose { nom: "sit", frames: &[11], frame_ms: 150, looping: false, ancre: None },
    // `sleep` est la MÊME frame que `sprawl` : Shimeji-ee n'a aucune
    // animation de sommeil. Déclarée sous le nom `sleep` pour que le code
    // ignore qu'il s'agit d'un substitut — un pack tiers avec une vraie pose
    // de sommeil la déclarerait au même nom, sans changement côté Rust.
    DefPose { nom: "sleep", frames: &[21], frame_ms: 150, looping: false, ancre: None },
    DefPose { nom: "fall", frames: &[4], frame_ms: 150, looping: false, ancre: None },
    DefPose { nom: "land", frames: &[18, 19], frame_ms: 160, looping: false, ancre: None },

    // Les poses de glisser portent leur ancre remontée : `Dragged.java`
    // place l'ancre à curseur + (0,120) avec ImageAnchor 64,128 — le haut du
    // sprite est donc 8 px AU-DESSUS du curseur, il le tient par la tête.
    DefPose { nom: "dragged", frames: &[1], frame_ms: 150, looping: false, ancre: Some([64.0, 8.0]) },
    DefPose { nom: "draggedRight1", frames: &[5], frame_ms: 150, looping: false, ancre: Some([64.0, 8.0]) },
    DefPose { nom: "draggedRight2", frames: &[7], frame_ms: 150, looping: false, ancre: Some([64.0, 8.0]) },
    DefPose { nom: "draggedRight3", frames: &[9], frame_ms: 150, looping: false, ancre: Some([64.0, 8.0]) },
    DefPose { nom: "draggedLeft1", frames: &[6], frame_ms: 150, looping: false, ancre: Some([64.0, 8.0]) },
    DefPose { nom: "draggedLeft2", frames: &[8], frame_ms: 150, looping: false, ancre: Some([64.0, 8.0]) },
    DefPose { nom: "draggedLeft3", frames: &[10], frame_ms: 150, looping: false, ancre: Some([64.0, 8.0]) },

    DefPose { nom: "sprawl", frames: &[21], frame_ms: 150, looping: false, ancre: None },
    DefPose { nom: "tripping", frames: &[19, 18, 20, 20, 19], frame_ms: 160, looping: false, ancre: None },
    DefPose { nom: "creep", frames: &[20, 20, 21, 21, 21], frame_ms: 160, looping: true, ancre: None },
    DefPose { nom: "sitDangle", frames: &[31, 32, 31, 33], frame_ms: 400, looping: true, ancre: Some([64.0, 112.0]) },
    DefPose { nom: "sitLegsUp", frames: &[30], frame_ms: 150, looping: false, ancre: Some([64.0, 112.0]) },
    DefPose { nom: "sitLookUp", frames: &[26], frame_ms: 150, looping: false, ancre: None },
    DefPose { nom: "spinHead", frames: &[26, 15, 27, 16, 28, 17, 29, 11], frame_ms: 200, looping: false, ancre: None },
    DefPose { nom: "jump", frames: &[22], frame_ms: 150, looping: false, ancre: None },

    DefPose { nom: "grabWall", frames: &[13], frame_ms: 150, looping: false, ancre: None },
    DefPose { nom: "climbWall", frames: &[14, 12, 13], frame_ms: 160, looping: true, ancre: None },
    DefPose { nom: "grabCeiling", frames: &[23], frame_ms: 150, looping: false, ancre: Some([64.0, 48.0]) },
    DefPose { nom: "climbCeiling", frames: &[23, 24, 25], frame_ms: 160, looping: true, ancre: Some([64.0, 48.0]) },

    DefPose { nom: "split", frames: &[42, 43, 44, 45, 46], frame_ms: 160, looping: false, ancre: None },
];

/// Les poses sans lesquelles un personnage ne peut pas vivre.
///
/// Un pack qui n'a pas de quoi tenir debout et marcher n'est pas installable :
/// mieux vaut le refuser bruyamment que livrer un dossier qui fera échouer le
/// chargement, ce qui ressemblerait à un bug du moteur.
pub const POSES_VITALES: &[&str] = &["stand", "walk"];
```

- [ ] **Étape 4 : écrire les trois fonctions**

```rust
/// Les ancres déclarées par le pack lui-même, dans son `actions.xml`.
///
/// On lit au motif plutôt qu'avec un parseur XML : deux attributs à extraire
/// ne justifient pas une dépendance (spec §4). Le format est stable depuis
/// Shimeji-ee, et un XML inattendu rend simplement une table vide, ce qui
/// déclenche le repli documenté.
pub fn ancres_de_actions_xml(xml: &str) -> BTreeMap<u32, [f32; 2]> {
    let mut ancres = BTreeMap::new();

    // On découpe sur `Image="` et on lit, dans chaque morceau, le numéro
    // puis l'`ImageAnchor` qui le suit. `skip(1)` : le texte avant la
    // première occurrence n'est pas une pose.
    for morceau in xml.split("Image=\"").skip(1) {
        // ── Le numéro de la frame ───────────────────────────────────────
        let Some(fin_chemin) = morceau.find('"') else {
            continue;
        };
        let chemin = &morceau[..fin_chemin];
        let Some(pos) = chemin.rfind("shime") else {
            continue;
        };
        let numero_txt: String = chemin[pos + 5..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let Ok(n) = numero_txt.parse::<u32>() else {
            continue;
        };

        // ── L'ancre, cherchée dans le MÊME morceau ──────────────────────
        // Donc avant le prochain `Image="`, ce qui garantit qu'on ne prend
        // pas l'ancre de la pose suivante.
        let Some(pos_ancre) = morceau.find("ImageAnchor=\"") else {
            continue;
        };
        let apres = &morceau[pos_ancre + 13..];
        let Some(fin) = apres.find('"') else {
            continue;
        };
        let mut parts = apres[..fin].split(',');
        let (Some(x), Some(y)) = (parts.next(), parts.next()) else {
            continue;
        };
        let (Ok(x), Ok(y)) = (x.trim().parse::<f32>(), y.trim().parse::<f32>()) else {
            continue;
        };

        ancres.insert(n, [x, y]);
    }

    ancres
}

/// Les poses dont TOUTES les frames sont présentes (spec §8.7).
pub fn poses_retenues(presentes: &BTreeSet<u32>) -> Vec<&'static DefPose> {
    POSES
        .iter()
        .filter(|def| def.frames.iter().all(|n| presentes.contains(n)))
        .collect()
}

/// Engendre le `mascot.json` du pack.
///
/// Écrit à la main plutôt que sérialisé depuis une struct : le manifeste
/// porte des champs de documentation (`_source`, `_mesures`) que le moteur
/// ignore mais qu'un humain lira dans six mois, et `Manifest` ne les a pas.
pub fn ecrire_mascot_json(
    slug: &str,
    presentes: &BTreeSet<u32>,
    tailles: &BTreeMap<u32, [u32; 2]>,
    ancres: &BTreeMap<u32, [f32; 2]>,
    hitbox: [u32; 4],
) -> Result<String, String> {
    let retenues = poses_retenues(presentes);

    for vitale in POSES_VITALES {
        if !retenues.iter().any(|p| p.nom == *vitale) {
            return Err(format!(
                "pack inutilisable : la pose « {vitale} » manque ({} frames présentes)",
                presentes.len()
            ));
        }
    }

    // La toile déclarée est la PLUS GRANDE des frames : c'est le repli pour
    // les images absentes de la table, et tant que la fenêtre adaptative
    // n'est pas branchée (spec §6), c'est elle que le moteur emploie.
    let mut tl = 0u32;
    let mut th = 0u32;
    for [l, h] in tailles.values() {
        if *l > tl { tl = *l; }
        if *h > th { th = *h; }
    }
    if tl == 0 || th == 0 {
        return Err("aucune frame mesurable".to_string());
    }

    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(&format!(
        "  \"_source\": \"Installe par shimeji-desktop depuis le catalogue shimejis.xyz, slug '{slug}' (CDN https://sprites.shimejis.xyz/directory/{slug}/). L'art n'est PAS de nous : voir l'avertissement du CLAUDE.md avant toute publication du depot.\",\n"
    ));
    s.push_str(&format!(
        "  \"_mesures\": \"Tailles lues dans l'en-tete PNG de chaque frame, sans decodage. Ancres lues dans l'actions.xml du pack quand il est servi, sinon supposees au milieu-bas (convention Shimeji-ee). Hitbox MESUREE sur la boite opaque de shime1, non recopiee sur blob : la numerotation des poses se transpose d'un pack a l'autre, les proportions du dessin non. Frames presentes : {}.\",\n",
        presentes.len()
    ));
    s.push_str(&format!("  \"id\": \"{slug}\",\n"));
    s.push_str(&format!("  \"name\": \"{slug}\",\n"));
    s.push_str(&format!("  \"frameSize\": [{tl}, {th}],\n"));
    s.push_str("  \"scale\": 1,\n");
    s.push_str(&format!(
        "  \"hitbox\": [{}, {}, {}, {}],\n",
        hitbox[0], hitbox[1], hitbox[2], hitbox[3]
    ));

    // ── La table `frames` (spec §6) ─────────────────────────────────────
    s.push_str("  \"frames\": {\n");
    let mut premiere = true;
    for (n, [l, h]) in tailles {
        if !premiere {
            s.push_str(",\n");
        }
        premiere = false;
        let ancre = match ancres.get(n) {
            Some([x, y]) => format!(", \"anchor\": [{x}, {y}]"),
            // Pas d'ancre déclarée : on n'en invente pas ici. Le repli sur
            // l'ancre de la pose se fait à la lecture (`ancre_de_frame`).
            None => String::new(),
        };
        s.push_str(&format!("    \"{n}\": {{ \"size\": [{l}, {h}]{ancre} }}"));
    }
    s.push_str("\n  },\n");

    // ── Les poses ───────────────────────────────────────────────────────
    s.push_str("  \"poses\": {\n");
    let mut premiere = true;
    for def in &retenues {
        if !premiere {
            s.push_str(",\n");
        }
        premiere = false;

        let frames: Vec<String> = def.frames.iter().map(|n| n.to_string()).collect();
        let ancre = match def.ancre {
            Some([x, y]) => format!(", \"anchor\": [{x}, {y}]"),
            // Sans ancre imposée, on prend celle de la première frame lue
            // dans l'actions.xml, et à défaut le milieu-bas de la toile.
            None => {
                let [x, y] = def
                    .frames
                    .first()
                    .and_then(|n| ancres.get(n))
                    .copied()
                    .unwrap_or([tl as f32 / 2.0, th as f32]);
                format!(", \"anchor\": [{x}, {y}]")
            }
        };
        s.push_str(&format!(
            "    \"{}\": {{ \"frames\": [{}], \"frameMs\": {}, \"loop\": {}{} }}",
            def.nom,
            frames.join(", "),
            def.frame_ms,
            def.looping,
            ancre
        ));
    }
    s.push_str("\n  }\n}\n");

    Ok(s)
}
```

- [ ] **Étape 5 : lancer les tests**

```powershell
cargo test les_ancres_se_lisent; cargo test seules_les_poses; cargo test le_mascot_json
```
Attendu : PASS pour les trois.

- [ ] **Étape 6 : lancer toute la suite**

```powershell
cargo test
```
Attendu : PASS.

- [ ] **Étape 7 : commit**

```powershell
git add src-tauri/src/catalogue/installation.rs
git commit -m @'
feat(catalogue): les ancres, les poses retenues, le mascot.json

La table des poses est le vocabulaire Shimeji, tire de conf/actions.xml de
Shimeji-ee et releve dans docs/specs/2026-09-09-frames-shimeji.md — pas
devine a l'oeil. C'est ce qui fait que tout pack du catalogue fonctionne
sans code.

Les ancres se lisent au motif dans l'actions.xml du pack : deux attributs a
extraire ne justifient pas une dependance XML, et un XML inattendu rend une
table vide, ce qui declenche le repli documente plutot qu'une erreur.

On ne declare QUE les poses dont toutes les frames existent : un pack
incomplet n'est pas une erreur, l'option est simplement retiree du tirage
d'envies. Mais stand et walk sont exigees — un pack sans elles ferait
echouer le chargement, ce qui ressemblerait a un bug du moteur.

Le test qui compte relit le JSON produit AVEC LE MOTEUR : c'est la seule
verification qui prouve que ce qu'on ecrit est utilisable.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
'@
```

---

## Tâche 6 : l'orchestration de l'installation, et `--installer`

**Fichiers :**
- Modifier : `src-tauri/src/catalogue/mod.rs`
- Modifier : `src-tauri/src/main.rs` (analyse des arguments)

**Interfaces :**
- Consomme : `reseau::Reseau` (T3), tout `installation::*` (T4, T5), `config::dossier_bibliotheque` (T1).
- Produit :
  - `catalogue::slug_valide(slug: &str) -> bool`
  - `catalogue::installer_avec<R: Reseau>(reseau: &R, racine: &Path, slug: &str, progres: &mut dyn FnMut(u32, u32)) -> Result<PathBuf, String>`
  - `catalogue::installer(slug: &str, progres: &mut dyn FnMut(u32, u32)) -> Result<PathBuf, String>`

- [ ] **Étape 1 : écrire les tests qui échouent**

Dans `catalogue/mod.rs`, module `tests` :

```rust
use super::*;
use crate::catalogue::reseau::ReseauFake;

/// Fabrique les octets d'un PNG 128×128 minimal mais VALIDE en en-tête.
///
/// Suffisant pour `taille_png` ; `decoder_png` échouera dessus, ce qui
/// exerce précisément le repli de la hitbox.
fn png_factice(l: u32, h: u32) -> Vec<u8> {
    let mut o = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    o.extend_from_slice(&13u32.to_be_bytes());
    o.extend_from_slice(b"IHDR");
    o.extend_from_slice(&l.to_be_bytes());
    o.extend_from_slice(&h.to_be_bytes());
    o
}

fn reseau_d_un_pack_complet(slug: &str) -> ReseauFake {
    let mut r = Vec::new();
    for n in 1..=46u32 {
        r.push((
            format!("https://sprites.shimejis.xyz/directory/{slug}/img/shime{n}.png"),
            Some(png_factice(128, 128)),
        ));
    }
    r.push((
        format!("https://sprites.shimejis.xyz/directory/{slug}/conf/actions.xml"),
        Some(br#"<Mascot><Pose Image="/shime1.png" ImageAnchor="64,128"/></Mascot>"#.to_vec()),
    ));
    ReseauFake::new(r)
}

/// Le slug construit un CHEMIN et une URL : il est validé avant les deux.
#[test]
fn le_slug_est_valide_avant_tout() {
    assert!(slug_valide("one-piece-luffy-01"));
    assert!(slug_valide("zoro-8bd774"));
    assert!(!slug_valide(""));
    assert!(!slug_valide(".."), "traversée de chemin");
    assert!(!slug_valide("a/b"), "séparateur de chemin");
    assert!(!slug_valide("a\\b"));
    assert!(!slug_valide("a b"), "espace");
}

/// Une installation complète, de bout en bout, sans réseau réel.
#[test]
fn une_installation_complete_produit_un_dossier_lisible() {
    let racine = std::env::temp_dir().join("shimeji-test-install-ok");
    let _ = std::fs::remove_dir_all(&racine);
    let reseau = reseau_d_un_pack_complet("pack-test");

    let mut vus = Vec::new();
    let dossier = installer_avec(&reseau, &racine, "pack-test", &mut |fait, total| {
        vus.push((fait, total));
    })
    .expect("l'installation doit réussir");

    assert!(dossier.join("mascot.json").is_file());
    assert!(dossier.join("img").join("shime1.png").is_file());
    assert!(!vus.is_empty(), "la progression doit être rapportée");

    // Le manifeste écrit doit être relisible par le moteur.
    let texte = crate::config::lire_json(&dossier.join("mascot.json")).expect("lecture");
    let m: crate::character::manifest::Manifest =
        serde_json::from_str(&texte).expect("le moteur doit relire");
    assert!(m.poses.contains_key("walk"));
}

/// LE cas qui compte : une coupure à mi-parcours ne laisse JAMAIS un
/// personnage à moitié installé (spec §8).
#[test]
fn une_coupure_ne_laisse_pas_de_pack_a_moitie() {
    let racine = std::env::temp_dir().join("shimeji-test-install-coupe");
    let _ = std::fs::remove_dir_all(&racine);
    let reseau = reseau_d_un_pack_complet("pack-coupe");
    reseau.echouer_apres(20);

    let r = installer_avec(&reseau, &racine, "pack-coupe", &mut |_, _| {});
    assert!(r.is_err(), "la coupure doit faire échouer l'installation");
    assert!(
        !racine.join("pack-coupe").exists(),
        "le dossier final ne doit pas exister"
    );
}

/// Un pack trop pauvre est refusé bruyamment (spec §8).
#[test]
fn un_pack_sans_pose_vitale_est_refuse() {
    let racine = std::env::temp_dir().join("shimeji-test-install-pauvre");
    let _ = std::fs::remove_dir_all(&racine);
    // Seule shime1 existe : de quoi tenir debout, pas de quoi marcher.
    let reseau = ReseauFake::new(vec![(
        "https://sprites.shimejis.xyz/directory/pauvre/img/shime1.png".to_string(),
        Some(png_factice(128, 128)),
    )]);

    let r = installer_avec(&reseau, &racine, "pauvre", &mut |_, _| {});
    assert!(r.is_err());
    assert!(!racine.join("pauvre").exists());
}
```

- [ ] **Étape 2 : lancer et vérifier l'échec**

```powershell
cargo test une_installation_complete
```
Attendu : ÉCHEC, `installer_avec` introuvable.

- [ ] **Étape 3 : écrire l'orchestration**

Dans `catalogue/mod.rs`, après les `pub mod` :

```rust
use crate::catalogue::installation::*;
use crate::catalogue::reseau::{Reseau, ReseauWinHttp};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const CDN: &str = "https://sprites.shimejis.xyz/directory";

/// Le slug construit un CHEMIN et une URL : il est validé avant les deux.
///
/// Même modèle que la validation du nom dans le schéma `shime://`
/// (`main.rs:476`) : caractères anodins seulement, ce qui suffit à interdire
/// tout `..` ou séparateur.
pub fn slug_valide(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 100
        && slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Installe le pack `slug` sous `racine`, et rend son dossier.
///
/// `progres` est appelé après chaque frame téléchargée — c'est ce qui
/// alimente la barre de progression de la fenêtre du catalogue.
/// `&mut dyn FnMut` plutôt qu'un générique : la fonction est déjà générique
/// sur le réseau, et un second paramètre de type alourdirait chaque appel
/// sans rien apporter.
pub fn installer_avec<R: Reseau>(
    reseau: &R,
    racine: &Path,
    slug: &str,
    progres: &mut dyn FnMut(u32, u32),
) -> Result<PathBuf, String> {
    if !slug_valide(slug) {
        return Err(format!("slug refusé : « {slug} »"));
    }

    let final_ = racine.join(slug);
    // Le dossier de travail porte un nom POINTÉ : s'il survit à une
    // coupure, il est ignoré au chargement (aucun personnage ne commence
    // par un point) et écrasé au prochain essai (spec §8).
    let partiel = racine.join(format!(".{slug}.partiel"));

    let _ = std::fs::remove_dir_all(&partiel);
    std::fs::create_dir_all(partiel.join("img"))
        .map_err(|e| format!("création de {} : {e}", partiel.display()))?;

    // ── Les frames ──────────────────────────────────────────────────────
    let mut presentes: BTreeSet<u32> = BTreeSet::new();
    let mut tailles: BTreeMap<u32, [u32; 2]> = BTreeMap::new();
    let mut premiere_image: Option<Vec<u8>> = None;

    // On continue au-delà de 46 tant que les fichiers existent, et on
    // s'arrête après deux absences consécutives : certains packs ont des
    // trous, mais aucun n'a deux trous de suite suivis de contenu.
    let mut absences = 0;
    let mut n = 1u32;
    // 120 est une borne de sûreté : sans elle, un CDN qui répondrait 200 à
    // tout ferait boucler l'installation jusqu'à remplir le disque.
    while n <= 120 {
        let url = format!("{CDN}/{slug}/img/shime{n}.png");
        // Le `?` propage la PANNE — et seulement elle. Un 404 rend
        // `Ok(None)` et continue : c'est la distinction de `reseau::Resultat`.
        let octets = match reseau.get(&url) {
            Ok(Some(o)) => o,
            Ok(None) => {
                absences += 1;
                if n > 46 && absences >= 2 {
                    break;
                }
                n += 1;
                continue;
            }
            Err(e) => {
                // Ménage AVANT de rendre l'erreur : on ne laisse pas le
                // dossier partiel encombrer la bibliothèque.
                let _ = std::fs::remove_dir_all(&partiel);
                return Err(format!("téléchargement interrompu : {e}"));
            }
        };
        absences = 0;

        let Some(taille) = taille_png(&octets) else {
            // Ce n'est pas un PNG : on l'ignore plutôt que d'échouer. Le CDN
            // sert parfois une page d'erreur avec un statut 200.
            n += 1;
            continue;
        };

        std::fs::write(partiel.join("img").join(format!("shime{n}.png")), &octets)
            .map_err(|e| format!("écriture de shime{n}.png : {e}"))?;

        if n == 1 {
            premiere_image = Some(octets);
        }
        presentes.insert(n);
        tailles.insert(n, taille);
        progres(presentes.len() as u32, 46);
        n += 1;
    }

    // ── Les ancres ──────────────────────────────────────────────────────
    // Une panne ici n'est PAS fatale : l'absence d'actions.xml a un repli
    // documenté (la convention Shimeji-ee), contrairement à l'absence de
    // frames.
    let ancres = match reseau.get(&format!("{CDN}/{slug}/conf/actions.xml")) {
        Ok(Some(o)) => {
            let texte = String::from_utf8_lossy(&o);
            ancres_de_actions_xml(&texte)
        }
        _ => BTreeMap::new(),
    };

    // ── La hitbox : UNE image décodée ───────────────────────────────────
    let hitbox = match premiere_image.as_ref().and_then(|o| decoder_png(o)) {
        Some((rgba, l, h)) => hitbox_depuis(&rgba, l, h),
        // shime1 illisible : la toile entière. Mieux vaut une hitbox trop
        // large qu'un personnage impossible à attraper (spec §3.3).
        None => {
            let [l, h] = tailles.get(&1).copied().unwrap_or([128, 128]);
            [0, 0, l, h]
        }
    };

    // ── Le manifeste ────────────────────────────────────────────────────
    let json = match ecrire_mascot_json(slug, &presentes, &tailles, &ancres, hitbox) {
        Ok(j) => j,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&partiel);
            return Err(e);
        }
    };
    std::fs::write(partiel.join("mascot.json"), json)
        .map_err(|e| format!("écriture du manifeste : {e}"))?;

    // ── Le renommage : c'est LUI qui rend l'installation atomique ───────
    let _ = std::fs::remove_dir_all(&final_);
    std::fs::rename(&partiel, &final_)
        .map_err(|e| format!("renommage final : {e}"))?;

    Ok(final_)
}

/// L'installation réelle : vrai réseau, vraie bibliothèque.
pub fn installer(slug: &str, progres: &mut dyn FnMut(u32, u32)) -> Result<PathBuf, String> {
    let Some(racine) = crate::config::dossier_bibliotheque() else {
        return Err("%APPDATA% introuvable : pas de bibliothèque".to_string());
    };
    installer_avec(&ReseauWinHttp::new(), &racine, slug, progres)
}
```

- [ ] **Étape 4 : lancer les quatre tests**

```powershell
cargo test le_slug_est_valide
cargo test une_installation_complete
cargo test une_coupure_ne_laisse_pas
cargo test un_pack_sans_pose_vitale
```
Attendu : PASS pour les quatre.

- [ ] **Étape 5 : brancher `--installer <slug>`**

Dans `main.rs`, auprès du traitement de `--sim` et `--demarrage` :

```rust
    // ── `--installer <slug>` ────────────────────────────────────────────
    // L'équivalent scriptable du clic dans la grille du catalogue, selon la
    // règle du projet : tout ce qui demanderait un clic reçoit un équivalent
    // en ligne de commande. Il vérifie en prime le seul morceau que les
    // tests ne couvrent pas — que le CDN réponde bien ce qu'on croit.
    if let Some(pos) = args.iter().position(|a| a == "--installer") {
        let Some(slug) = args.get(pos + 1) else {
            eprintln!("usage : --installer <slug>");
            std::process::exit(2);
        };

        println!("installation de « {slug} »…");
        let mut dernier = 0;
        match catalogue::installer(slug, &mut |fait, total| {
            // On n'imprime qu'au changement : 46 lignes identiques ne
            // renseignent personne.
            if fait != dernier {
                dernier = fait;
                println!("  {fait}/{total}");
            }
        }) {
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
```

- [ ] **Étape 6 : vérifier sur le vrai CDN**

```powershell
cargo build
cargo run -- --installer one-piece-zoro-01
```
Attendu : une progression jusqu'à ~46, puis
`installé : C:\Users\...\AppData\Roaming\shimeji-desktop\characters\one-piece-zoro-01`.

Puis vérifier que le manifeste produit est cohérent :

```powershell
Get-Content "$env:APPDATA\shimeji-desktop\characters\one-piece-zoro-01\mascot.json" -TotalCount 12
(Get-ChildItem "$env:APPDATA\shimeji-desktop\characters\one-piece-zoro-01\img").Count
```
Attendu : un JSON lisible dont `frameSize` vaut `[185, 155]` (la mesure de la
spec §3), et ~46 fichiers.

⚠️ Si le CDN ne répond pas, la cause est presque toujours dans `ReseauWinHttp`
(Tâche 3) et non ici : le tester d'abord avec un slug connu.

- [ ] **Étape 7 : lancer toute la suite et commiter**

```powershell
cargo test
git add src-tauri/src/catalogue/mod.rs src-tauri/src/main.rs
git commit -m @'
feat(catalogue): l'installation de bout en bout, et --installer

Telecharge les frames, lit leur taille dans l'en-tete, lit les ancres de
l'actions.xml, mesure la hitbox sur UNE image decodee, ecrit le mascot.json.

L'installation est atomique par renommage : le travail se fait dans un
dossier au nom POINTE, renomme seulement quand tout est ecrit. Une coupure
ne laisse donc jamais un personnage a moitie installe — le genre de panne
qui se diagnostique tres mal, parce qu'elle ressemble a un bug du moteur.
C'est le cas que le test principal exerce.

Un 404 fait avancer, une panne interrompt : sans cette distinction, une
coupure reseau serait prise pour « le pack s'arrete ici » et produirait un
personnage ampute presente comme complet.

--installer <slug> est l'equivalent scriptable du clic dans la grille, selon
la regle du projet, et verifie le seul morceau que les tests ne couvrent
pas : que le CDN reponde bien ce qu'on croit.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
'@
```

---

## Tâche 7 : l'index `catalogue.json`

**Fichiers :**
- Modifier : `src-tauri/src/catalogue/index.rs`
- Créer : `ui/catalogue.json`
- Modifier : `tools/recuperer-packs.ps1`

**Interfaces :**
- Consomme : rien.
- Produit :
  - `index::Pack { slug: String, nom: String, franchise: String }`
  - `index::Index { packs: Vec<Pack> }`
  - `index::lire(texte: &str) -> Result<Index, String>`
  - `index::nom_lisible(slug: &str) -> String`

- [ ] **Étape 1 : écrire les tests qui échouent**

```rust
/// L'index se lit, BOM compris.
///
/// Le BOM a déjà coûté un diagnostic sur ce projet : `serde_json` le refuse
/// avec le message trompeur « expected value at line 1 column 1 ».
#[test]
fn l_index_se_lit_avec_ou_sans_bom() {
    let json = r#"{
        "genere_le": "2026-09-11",
        "packs": [
            { "slug": "one-piece-luffy-01", "nom": "Luffy", "franchise": "One Piece" }
        ]
    }"#;

    let i = lire(json).expect("index valide");
    assert_eq!(i.packs.len(), 1);
    assert_eq!(i.packs[0].slug, "one-piece-luffy-01");

    let avec_bom = format!("\u{feff}{json}");
    assert!(lire(&avec_bom).is_ok(), "le BOM ne doit pas faire échouer");

    assert!(lire("pas du json").is_err());
}

/// Le nom lisible se déduit du slug pour les dépôts communautaires.
#[test]
fn le_nom_lisible_retire_le_suffixe_hexadecimal() {
    // 1636 des 2063 slugs sont de cette forme.
    assert_eq!(nom_lisible("zoro-8bd774"), "Zoro");
    assert_eq!(nom_lisible("pierrot-54acb5"), "Pierrot");
    // Un suffixe qui n'est pas hexadécimal de 6 caractères reste.
    assert_eq!(nom_lisible("one-piece-luffy-01"), "One piece luffy 01");
    assert_eq!(nom_lisible(""), "");
}
```

- [ ] **Étape 2 : lancer et vérifier l'échec**

```powershell
cargo test l_index_se_lit; cargo test le_nom_lisible
```
Attendu : ÉCHEC.

- [ ] **Étape 3 : écrire l'implémentation**

`src-tauri/src/catalogue/index.rs` :

```rust
//! L'index du catalogue : la liste des packs disponibles (spec §7).
//!
//! Responsabilité unique : lire `catalogue.json`. Ne télécharge rien, ne
//! sait rien des URL — le slug suffit, l'URL s'en déduit dans `mod.rs`.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Pack {
    pub slug: String,
    pub nom: String,
    pub franchise: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Index {
    pub packs: Vec<Pack>,
}

/// Lit l'index, BOM compris.
///
/// Le BOM a coûté un diagnostic sur ce projet : `serde_json` le refuse avec
/// le message trompeur « expected value at line 1 column 1 ». On le retire
/// ici comme `config::lire_json` le fait déjà pour les fichiers.
pub fn lire(texte: &str) -> Result<Index, String> {
    const BOM: char = '\u{feff}';
    let propre = texte.strip_prefix(BOM).unwrap_or(texte);
    serde_json::from_str(propre).map_err(|e| format!("catalogue.json illisible : {e}"))
}

/// Un nom présentable, déduit du slug.
///
/// 1636 des 2063 slugs du catalogue sont des dépôts communautaires portant un
/// suffixe hexadécimal de 6 caractères (`zoro-8bd774`) : on le retire. Les
/// autres gardent leur slug, simplement mis en forme.
pub fn nom_lisible(slug: &str) -> String {
    let sans_suffixe = match slug.rsplit_once('-') {
        // `len() == 6` ET tout hexadécimal : les deux conditions, sinon on
        // amputerait « luffy-01 » de son numéro.
        Some((debut, fin))
            if fin.len() == 6 && fin.chars().all(|c| c.is_ascii_hexdigit()) =>
        {
            debut
        }
        _ => slug,
    };

    let avec_espaces = sans_suffixe.replace('-', " ");

    // Première lettre en capitale. `chars().next()` plutôt qu'un index : un
    // caractère UTF-8 peut faire plusieurs octets, et `&s[0..1]` paniquerait
    // au milieu de l'un d'eux.
    let mut c = avec_espaces.chars();
    match c.next() {
        Some(premiere) => premiere.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}
```

- [ ] **Étape 4 : lancer les tests**

```powershell
cargo test l_index_se_lit; cargo test le_nom_lisible
```
Attendu : PASS.

- [ ] **Étape 5 : ajouter le mode `-Index` au script**

Dans `tools/recuperer-packs.ps1`, ajouter le paramètre `-Index` et cette
fonction, puis la brancher en tête du script (avant tout téléchargement) :

```powershell
# --- Le mode -Index : engendrer ui/catalogue.json -------------------------
#
# C'est un geste de MAINTENANCE, joue a la main pour rafraichir le catalogue,
# jamais un chemin d'execution. Si le markup de shimejis.xyz change, c'est ce
# script qui casse, sur la machine du developpeur, avec un message — pas le
# catalogue chez l'utilisateur.
function New-Index {
    Write-Host "recuperation de $CATALOGUE ..."
    $html = (Invoke-WebRequest -Uri $CATALOGUE -UseBasicParsing).Content

    # Les slugs se lisent dans les URL de vignettes : chaque carte du
    # repertoire pointe la shime1.png de son pack.
    $motif = 'directory/([a-z0-9\-_]+)/img/shime1\.png'
    $slugs = [regex]::Matches($html, $motif) |
             ForEach-Object { $_.Groups[1].Value } |
             Select-Object -Unique |
             Sort-Object

    Write-Host "$($slugs.Count) slugs distincts"

    $packs = foreach ($s in $slugs) {
        # Le suffixe hexadecimal de 6 caracteres marque un depot
        # communautaire : il n'a pas de franchise.
        if ($s -match '^(.*)-([0-9a-f]{6})$') {
            $nom = (Get-Culture).TextInfo.ToTitleCase($Matches[1] -replace '-', ' ')
            $franchise = 'Communaute'
        } else {
            $nom = (Get-Culture).TextInfo.ToTitleCase($s -replace '-', ' ')
            $franchise = 'Catalogue'
        }
        [pscustomobject]@{ slug = $s; nom = $nom; franchise = $franchise }
    }

    $index = [pscustomobject]@{
        genere_le = (Get-Date -Format 'yyyy-MM-dd')
        source    = $CATALOGUE
        packs     = @($packs)
    }

    # -Depth : sans lui, ConvertTo-Json aplatit les objets imbriques a partir
    # du niveau 2 et ecrit « System.Object[] ».
    $json = $index | ConvertTo-Json -Depth 4

    # SANS BOM, imperativement : serde_json le refuse avec un message
    # trompeur. UTF8Encoding($false) est la seule facon fiable en PS 5.1 —
    # Out-File -Encoding utf8 ecrit un BOM.
    $sortie = Join-Path $PSScriptRoot '..\ui\catalogue.json'
    [System.IO.File]::WriteAllText(
        [System.IO.Path]::GetFullPath($sortie),
        $json,
        (New-Object System.Text.UTF8Encoding($false))
    )
    Write-Host "ecrit : $sortie"
}
```

⚠️ **La franchise reste une incertitude assumée** (spec §7) : le titre de
section du HTML n'a pas été vérifié. Le repli ci-dessus met tout le monde dans
`Catalogue` ou `Communaute`, ce qui rend le catalogue utilisable — juste moins
joli. Si, en lançant le script, les titres de franchise se révèlent
exploitables, les extraire ici ; sinon, garder ce repli et **le noter dans la
spec**.

- [ ] **Étape 6 : engendrer le vrai `catalogue.json`**

```powershell
cd C:\Users\alri\Documents\shimeji-desktop
.\tools\recuperer-packs.ps1 -Index
Get-Item ui\catalogue.json | Select-Object Length
```
Attendu : ~2000 slugs, un fichier de l'ordre de 100 à 200 Ko.

Vérifier l'absence de BOM :

```powershell
$o = [System.IO.File]::ReadAllBytes("ui\catalogue.json")[0..2]
"$o"   # NE DOIT PAS commencer par 239 187 191
```

- [ ] **Étape 7 : vérifier que Rust relit ce fichier réel**

Ajouter ce test dans `index.rs` :

```rust
/// L'index RÉEL du dépôt se lit. Ce test attrape ce qu'aucun JSON fabriqué
/// à la main n'attrape : un BOM, un champ manquant, une fuite d'encodage.
#[test]
fn l_index_du_depot_se_lit() {
    let texte = std::fs::read_to_string("../ui/catalogue.json")
        .expect("ui/catalogue.json doit exister — le régénérer par tools\\recuperer-packs.ps1 -Index");
    let i = lire(&texte).expect("l'index du dépôt doit être lisible");
    assert!(i.packs.len() > 500, "seulement {} packs ?", i.packs.len());
}
```

```powershell
cd src-tauri; cargo test l_index_du_depot_se_lit
```
Attendu : PASS.

- [ ] **Étape 8 : commit**

```powershell
git add ui/catalogue.json src-tauri/src/catalogue/index.rs tools/recuperer-packs.ps1
git commit -m @'
feat(catalogue): l'index pre-genere, et le mode -Index du script

L'index est versionne plutot qu'analyse a l'execution : si le markup de
shimejis.xyz change, c'est le script qui casse, sur la machine du
developpeur, avec un message — pas le catalogue chez l'utilisateur.

Trois champs par pack, pas plus. Pas de nombre de frames : le HTML n'expose
que les shime1.png, donc le compte demanderait 2000 sondages du CDN pour une
information dont l'installation a besoin de toute facon. Pas d'URL : elle se
deduit du slug par une regle unique, ecrite une fois dans le code — un champ
url par pack serait 2000 occasions de diverger.

Le BOM est retire a la lecture et jamais ecrit : il a deja coute un
diagnostic sur ce projet, serde_json le refusant avec le message trompeur
« expected value at line 1 column 1 ».

Un test relit l'index REEL du depot : c'est lui qui attrape ce qu'aucun JSON
fabrique a la main n'attrape.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
'@
```

---

## Tâche 8 : les trois commandes, et l'écriture chirurgicale de `config.json`

**Fichiers :**
- Créer : `src-tauri/src/commandes.rs`
- Créer : `src-tauri/capabilities/catalogue.json`
- Modifier : `src-tauri/src/config.rs` (mémoriser le chemin chargé, écrire)
- Modifier : `src-tauri/src/main.rs` (`invoke_handler`)
- Modifier : `src-tauri/tauri.conf.json` (`withGlobalTauri`)

> ### ⚠️⚠️ Lire ceci avant d'écrire une ligne — le piège a déjà été payé
>
> **Tauri v2 a une liste de contrôle d'accès, et elle refuse en SILENCE.**
> `render.rs:195-225` raconte l'histoire : une première version poussait les
> images par `emit` + `window.__TAURI__.event.listen`, elle a été écrite,
> lancée, et **le sprite restait invisible**. Cause : `event.listen` exige la
> permission `core:event:allow-listen`, le projet n'avait aucun fichier de
> capacités, l'appel était refusé — et la promesse rejetée n'étant attendue
> par personne, **rien ne s'affichait nulle part**.
>
> Vérifié au moment d'écrire ce plan : **`src-tauri/capabilities/` n'existe
> toujours pas**, et `withGlobalTauri` n'est **pas** activé dans
> `tauri.conf.json`. Donc en l'état, `window.__TAURI__` est `undefined` et
> tout `invoke` échouerait exactement de la même façon.
>
> **Le personnage continue de passer par `eval`** — on ne touche pas à ce
> chemin, qui n'a besoin d'aucune permission et vise une fenêtre précise.
> Mais `eval` ne va que de Rust vers JS : le catalogue, lui, doit appeler
> Rust. Il lui faut donc l'IPC, donc une capacité — **portée à la seule
> fenêtre `catalogue`**, pour que le chemin du personnage reste intact.

**Interfaces :**
- Consomme : `catalogue::installer` (T6), `config::dossier_bibliotheque` (T1).
- Produit :
  - `config::chemin_charge() -> Option<PathBuf>`
  - `config::ecrire_personnage(chemin: &Path, nom: &str) -> Result<(), String>`
  - `config::definir_personnage(nom: &str) -> Result<(), String>`
  - `commandes::{installer, bibliotheque, choisir}` — commandes Tauri
  - `commandes::PackInstalle { nom: String, actif: bool }`

- [ ] **Étape 1 : écrire le test qui échoue**

Dans `config.rs`, module `tests` :

```rust
/// On modifie la SEULE clé `personnages`, en laissant tout le reste intact.
///
/// Une sérialisation depuis `Config` perdrait les clés inconnues et
/// remettrait les valeurs par défaut partout : l'utilisateur verrait son
/// fichier regle a la main ecrase par un clic dans une autre fenetre
/// (spec §10).
#[test]
fn ecrire_le_personnage_preserve_le_reste_du_fichier() {
    let chemin = std::env::temp_dir().join("shimeji-test-config-chirurgie.json");
    std::fs::write(
        &chemin,
        r#"{
  "echelle": 1.5,
  "vitesse": 2,
  "personnages": ["blob"],
  "une_cle_que_le_code_ne_connait_pas": { "a": 1 }
}"#,
    )
    .expect("écriture du fichier de test");

    ecrire_personnage(&chemin, "luffy").expect("l'écriture doit réussir");

    let texte = lire_json(&chemin).expect("relecture");
    let v: serde_json::Value = serde_json::from_str(&texte).expect("JSON valide");

    assert_eq!(v["personnages"][0], "luffy", "la clé visée est changée");
    assert_eq!(v["echelle"], 1.5, "les autres réglages survivent");
    assert_eq!(v["vitesse"], 2);
    assert_eq!(
        v["une_cle_que_le_code_ne_connait_pas"]["a"], 1,
        "les clés inconnues survivent"
    );
    assert!(!texte.starts_with('\u{feff}'), "jamais de BOM");
}

/// Un fichier absent est CRÉÉ, avec la seule clé qu'on sait devoir y mettre.
#[test]
fn ecrire_le_personnage_cree_le_fichier_absent() {
    let chemin = std::env::temp_dir().join("shimeji-test-config-neuve.json");
    let _ = std::fs::remove_file(&chemin);

    ecrire_personnage(&chemin, "blob").expect("création");

    let texte = lire_json(&chemin).expect("relecture");
    let v: serde_json::Value = serde_json::from_str(&texte).expect("JSON valide");
    assert_eq!(v["personnages"][0], "blob");
}
```

- [ ] **Étape 2 : lancer et vérifier l'échec**

```powershell
cargo test ecrire_le_personnage
```
Attendu : ÉCHEC, `ecrire_personnage` introuvable.

- [ ] **Étape 3 : écrire l'implémentation dans `config.rs`**

```rust
/// Le chemin du `config.json` réellement chargé.
///
/// ⚠️ **Mémorisé et non recalculé.** `resoudre` trouve d'abord celui du
/// dépôt ; si l'écriture allait dans `%APPDATA%` pendant que la lecture vient
/// du dépôt, le réglage paraîtrait sans effet et le diagnostic serait long
/// (spec §10).
///
/// `OnceLock` : écrit une fois au démarrage, lu de plusieurs threads,
/// sans verrou. C'est le type de la bibliothèque standard fait pour ça.
static CHEMIN_CHARGE: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();

pub fn chemin_charge() -> Option<PathBuf> {
    // `get_or_init` : à la première interrogation, on résout ; ensuite on
    // rend la même valeur. `clone` parce que l'appelant veut posséder.
    CHEMIN_CHARGE.get_or_init(|| resoudre("config.json")).clone()
}

/// Remplace la clé `personnages` de `chemin`, en laissant tout le reste.
///
/// Édition **chirurgicale** : on relit en `serde_json::Value`, on ne touche
/// qu'à une clé, on réécrit. Sérialiser depuis `Config` perdrait toutes les
/// clés inconnues et remettrait les valeurs par défaut partout (spec §10).
pub fn ecrire_personnage(chemin: &Path, nom: &str) -> Result<(), String> {
    // Un fichier absent n'est pas une erreur : c'est le cas normal au
    // premier choix, et on le crée.
    let mut valeur: serde_json::Value = match lire_json(chemin) {
        Ok(texte) => serde_json::from_str(&texte)
            .map_err(|e| format!("config.json illisible, rien n'est écrit : {e}"))?,
        Err(_) => serde_json::json!({}),
    };

    // Un objet JSON, sinon on n'a rien à modifier. `as_object_mut` rend
    // `None` si la racine est un tableau ou un scalaire.
    let Some(objet) = valeur.as_object_mut() else {
        return Err("config.json n'est pas un objet JSON".to_string());
    };
    objet.insert(
        "personnages".to_string(),
        serde_json::json!([nom]),
    );

    // `to_string_pretty` : le fichier est édité à la main par l'auteur, une
    // seule ligne le rendrait pénible.
    let texte = serde_json::to_string_pretty(&valeur)
        .map_err(|e| format!("sérialisation : {e}"))?;

    // `write` écrit en UTF-8 SANS BOM — c'est ce qu'il faut (spec §10).
    std::fs::write(chemin, texte).map_err(|e| format!("écriture de config.json : {e}"))
}

/// Enregistre le personnage choisi dans le `config.json` réellement chargé,
/// ou en crée un dans `%APPDATA%` s'il n'y en avait aucun.
pub fn definir_personnage(nom: &str) -> Result<(), String> {
    match chemin_charge() {
        Some(c) => ecrire_personnage(&c, nom),
        None => {
            // Aucun config.json nulle part : on en crée un à côté de la
            // bibliothèque, jamais dans le dépôt.
            let Ok(appdata) = std::env::var("APPDATA") else {
                return Err("%APPDATA% introuvable".to_string());
            };
            let dossier = PathBuf::from(appdata).join("shimeji-desktop");
            std::fs::create_dir_all(&dossier)
                .map_err(|e| format!("création de {} : {e}", dossier.display()))?;
            ecrire_personnage(&dossier.join("config.json"), nom)
        }
    }
}
```

- [ ] **Étape 4 : lancer les deux tests**

```powershell
cargo test ecrire_le_personnage
```
Attendu : PASS pour les deux.

- [ ] **Étape 5 : écrire les commandes**

`src-tauri/src/commandes.rs` :

```rust
//! Les commandes appelées par la fenêtre du catalogue (spec §9).
//!
//! Responsabilité unique : exposer trois gestes au webview. Toute la logique
//! est ailleurs — ici on ne fait que traduire un appel JS en appel Rust.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Un pack présent sur le disque, tel que la fenêtre a besoin de le connaître.
#[derive(Serialize)]
pub struct PackInstalle {
    pub nom: String,
    /// Est-ce celui qui s'affiche en ce moment ?
    pub actif: bool,
}

/// Installe un pack depuis le catalogue.
///
/// `async` parce que 46 téléchargements prennent environ 3 s : une commande
/// bloquante figerait la fenêtre du catalogue pendant tout ce temps
/// (spec §9).
///
/// L'événement `installation` porte la progression. Un seul événement, une
/// seule forme — `{slug, fait, total}`.
#[tauri::command]
pub async fn installer(app: AppHandle, slug: String) -> Result<(), String> {
    // `spawn_blocking` : le téléchargement est du travail BLOQUANT (WinHttp
    // n'est pas asynchrone). Le laisser sur l'exécuteur async figerait les
    // autres commandes.
    let slug_pour_tache = slug.clone();
    let app_pour_tache = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        crate::catalogue::installer(&slug_pour_tache, &mut |fait, total| {
            // L'échec d'émission est ignoré : la fenêtre a pu être fermée
            // pendant le téléchargement, ce qui n'est pas une raison de
            // l'interrompre.
            let _ = app_pour_tache.emit(
                "installation",
                serde_json::json!({ "slug": slug_pour_tache, "fait": fait, "total": total }),
            );
        })
    })
    .await
    .map_err(|e| format!("tâche d'installation interrompue : {e}"))?
    .map(|_chemin| ())
}

/// Ce que contient la bibliothèque, plus le dossier livré.
///
/// Les deux racines sont listées, et la bibliothèque gagne en cas d'homonyme
/// — le même ordre qu'à la résolution (spec §5), sans quoi la liste
/// mentirait sur ce qui s'afficherait réellement.
#[tauri::command]
pub fn bibliotheque() -> Vec<PackInstalle> {
    let actif = crate::config::charger()
        .personnages
        .first()
        .cloned()
        .unwrap_or_else(|| "blob".to_string());

    let mut noms: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    // `into_iter().flatten()` : on parcourt les racines existantes et on
    // ignore silencieusement celles qui manquent — une bibliothèque vide
    // n'est pas une erreur.
    let racines = [
        crate::config::dossier_bibliotheque(),
        Some(crate::config::dossier_personnages()),
    ];
    for racine in racines.into_iter().flatten() {
        let Ok(entrees) = std::fs::read_dir(&racine) else {
            continue;
        };
        for entree in entrees.flatten() {
            // Un personnage est un dossier contenant un mascot.json. Ce test
            // écarte d'un coup les dossiers `.partiel` d'une installation
            // interrompue et tout fichier égaré.
            if entree.path().join("mascot.json").is_file() {
                if let Some(nom) = entree.file_name().to_str() {
                    noms.insert(nom.to_string());
                }
            }
        }
    }

    noms.into_iter()
        .map(|nom| PackInstalle {
            actif: nom == actif,
            nom,
        })
        .collect()
}

/// Affiche ce personnage — le SECOND geste (décision de cadrage n° 2).
///
/// ⚠️ **Cette version n'écrit que la config : le changement ne prend effet
/// qu'au redémarrage.** La Tâche 9 REMPLACE ce corps pour déclencher aussi
/// le rechargement à chaud, une fois `Actions::changer_personnage` écrite.
/// L'ordre est volontaire : on livre d'abord un geste qui marche, même
/// imparfaitement, plutôt qu'une tâche qui ne se teste qu'à la fin.
#[tauri::command]
pub fn choisir(nom: String) -> Result<(), String> {
    // On refuse un personnage introuvable AVANT d'écrire la config : sinon
    // l'application ne redémarrerait plus, le chargement échouant sur un nom
    // qui ne résout pas.
    let Some(_) = crate::config::dossier_du_personnage(&nom) else {
        return Err(format!("personnage « {nom} » introuvable"));
    };

    crate::config::definir_personnage(&nom)?;
    println!("personnage choisi : {nom}");
    Ok(())
}
```

- [ ] **Étape 6 : enregistrer les commandes**

Dans `main.rs`, ajouter `mod commandes;` et, sur le `tauri::Builder` :

```rust
        // Les trois commandes de la fenêtre du catalogue (spec §9).
        // `generate_handler!` engendre la table de routage à la compilation :
        // une commande oubliée ici est introuvable côté JS, sans erreur de
        // compilation — d'où le test manuel de l'étape suivante.
        .invoke_handler(tauri::generate_handler![
            commandes::installer,
            commandes::bibliotheque,
            commandes::choisir
        ])
```

- [ ] **Étape 7 : déclarer la capacité de la fenêtre du catalogue**

Créer `src-tauri/capabilities/catalogue.json` — **sans BOM** :

```json
{
  "identifier": "catalogue",
  "description": "Permet a la fenetre du catalogue d'appeler Rust et d'ecouter la progression d'installation. Portee a CETTE fenetre : le personnage continue de passer par eval, sans aucune permission.",
  "windows": ["catalogue"],
  "permissions": ["core:default", "core:event:allow-listen"]
}
```

⚠️ **`"windows": ["catalogue"]` n'est pas décoratif.** Sans cette portée, la
capacité s'appliquerait aussi à la fenêtre du personnage, qui n'en a pas
besoin et dont le chemin `eval` est délibérément sans permission. Le label
doit correspondre **exactement** à celui de `ouvrir_catalogue` (Tâche 10).

Puis activer l'injection de l'API dans `src-tauri/tauri.conf.json` :

```json
  "app": {
    "withGlobalTauri": true,
    "windows": [],
    "security": {
      "csp": null
    }
  }
```

⚠️ `withGlobalTauri` est **global**, pas par fenêtre : il injecte
`window.__TAURI__` dans *toutes* les pages, y compris celle du personnage.
C'est sans conséquence — `pet.js` ne s'en sert pas et garde son chemin
`eval` — mais il faut le savoir plutôt que de le découvrir.

On ne passe **pas** par `window.__TAURI_INTERNALS__`, qui éviterait ce
drapeau : c'est une interface **interne**, dont le nom peut changer d'une
version de Tauri à l'autre. `pet.js` l'écarte déjà pour cette raison, et il
n'y a pas de raison de trancher autrement ici.

- [ ] **Étape 8 : PROUVER que l'IPC répond, avant de bâtir dessus**

C'est l'étape que la première tentative d'événements n'avait pas faite, et
c'est ce qui lui a coûté un diagnostic complet. On vérifie le tuyau **avant**
d'y brancher une interface.

Créer temporairement `ui/catalogue.html` réduit à ceci :

```html
<!doctype html>
<meta charset="utf-8">
<title>sonde</title>
<body style="font: 14px sans-serif; background:#242424; color:#c9c9c9">
<pre id="sortie">appel en cours…</pre>
<script>
const sortie = document.getElementById('sortie');
if (!window.__TAURI__) {
  sortie.textContent = 'ÉCHEC : window.__TAURI__ absent → withGlobalTauri non activé';
} else {
  window.__TAURI__.core.invoke('bibliotheque')
    .then((r) => { sortie.textContent = 'OK invoke : ' + JSON.stringify(r); })
    .catch((e) => { sortie.textContent = 'ÉCHEC invoke (ACL ?) : ' + e; });

  window.__TAURI__.event.listen('sonde', () => {})
    .then(() => { sortie.textContent += '\nOK listen'; })
    .catch((e) => { sortie.textContent += '\nÉCHEC listen (ACL ?) : ' + e; });
}
</script>
```

Ajouter temporairement, à la fin du `setup` de `main.rs`, un appel à
`actions::ouvrir_catalogue(&handle)` — la fonction n'existant qu'en Tâche 10,
créer ici une version minimale qui ouvre `catalogue.html` avec le label
`catalogue`, et que la Tâche 10 étoffera.

```powershell
cargo build
cargo run
```

Attendu, **écrit noir sur blanc dans la fenêtre** :
```
OK invoke : [{"nom":"blob","actif":true}]
OK listen
```

⚠️ **Ne pas continuer tant que ces deux lignes ne s'affichent pas.**
- « `window.__TAURI__` absent » → `withGlobalTauri` n'est pas pris en compte ;
  vérifier qu'il est bien sous `app` et non à la racine.
- « ÉCHEC invoke » ou « ÉCHEC listen » → la capacité n'est pas appliquée ;
  vérifier le nom du dossier (`capabilities`, au pluriel, à côté de
  `tauri.conf.json`), l'absence de BOM, et que le label de la fenêtre est
  bien `catalogue`.

Retirer ensuite l'appel temporaire du `setup`.

- [ ] **Étape 9 : compiler et lancer la suite**

```powershell
cargo test
cargo build
```
Attendu : PASS, compilation réussie.

- [ ] **Étape 10 : commit**

```powershell
git add src-tauri/src/commandes.rs src-tauri/src/config.rs src-tauri/src/main.rs src-tauri/capabilities/ src-tauri/tauri.conf.json ui/catalogue.html
git commit -m @'
feat(commandes): installer, bibliotheque, choisir

Trois commandes et un evenement de progression. installer est async et passe
par spawn_blocking : 46 telechargements prennent ~3 s, et WinHttp n'est pas
asynchrone — le laisser sur l'executeur figerait les autres commandes.

L'ecriture de config.json est CHIRURGICALE : on relit en Value, on ne touche
qu'a la cle personnages, on reecrit. Serialiser depuis Config perdrait
toutes les cles inconnues et remettrait les valeurs par defaut partout —
l'utilisateur verrait son fichier regle a la main ecrase par un clic dans
une autre fenetre.

Et on reecrit le fichier REELLEMENT charge, memorise et non recalcule : sans
ca, resoudre trouve celui du depot pendant que l'ecriture va dans %APPDATA%,
et le reglage parait sans effet.

choisir refuse un personnage introuvable AVANT d'ecrire : sinon
l'application ne redemarrerait plus.

Et une capacite, portee a la SEULE fenetre du catalogue. Tauri v2 refuse
l'IPC en silence sans elle — c'est ce qui avait rendu le sprite invisible a
la premiere tentative d'evenements (render.rs:195). Le personnage, lui,
continue de passer par eval, sans aucune permission : son chemin n'est pas
touche.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
'@
```

---

## Tâche 9 : le changement de personnage à chaud

**Fichiers :**
- Modifier : `src-tauri/src/actions.rs` (`Actions.personnage` devient modifiable)
- Modifier : `src-tauri/src/commandes.rs` (`choisir` déclenche le rechargement)
- Modifier : `ui/pet.js` (`recharger` recalcule `BASE`)
- Modifier : `src-tauri/src/render.rs` — **interdit**, voir la note

**Interfaces :**
- Consomme : `rechargement::preparer` (T1), `config::definir_personnage` (T8).
- Produit : `Actions::changer_personnage(&self, nom: &str, dossier: PathBuf)`

> ⚠️ `render.rs` est **hors périmètre** (contraintes globales). L'appel
> `win.eval("window.recharger(v)")` qu'il contient doit passer un second
> argument. C'est **une chaîne de format à modifier**, pas de la logique :
> c'est la seule exception tolérée, elle doit tenir en une ligne, et le diff
> de `render.rs` doit se limiter à cette ligne.

- [ ] **Étape 1 : rendre le personnage d'`Actions` modifiable**

Dans `actions.rs`, remplacer les deux champs :

```rust
    pub dossier: PathBuf,
    pub personnage: String,
```

par :

```rust
    /// Le personnage courant et son dossier.
    ///
    /// `Mutex` parce qu'ils changent maintenant en cours d'exécution : la
    /// fenêtre du catalogue peut en choisir un autre. Ils étaient constants
    /// tant qu'un seul personnage existait au démarrage.
    ///
    /// Les deux ensemble dans UN verrou et non deux : ils doivent changer
    /// d'un coup, sinon un rechargement pourrait lire le nouveau nom avec
    /// l'ancien dossier.
    perso: Mutex<(String, PathBuf)>,
```

Et ajouter :

```rust
impl Actions {
    /// Le dossier du personnage courant.
    pub fn dossier_courant(&self) -> Option<PathBuf> {
        // `ok()` : un verrou empoisonné rend `None`, et l'appelant se
        // contentera de ne rien faire — mieux qu'un panic dans un
        // gestionnaire de menu.
        self.perso.lock().ok().map(|p| p.1.clone())
    }

    /// Change le personnage courant et demande son chargement.
    pub fn changer_personnage(&self, nom: &str, dossier: PathBuf) -> Result<u64, String> {
        // Les entrées-sorties D'ABORD, verrou non tenu : si le manifeste est
        // illisible, on sort sans avoir rien touché et le personnage courant
        // continue avec ce qu'il avait. Même principe que `preparer`.
        let version = crate::rechargement::preparer(&self.demande, &dossier)?;

        match self.perso.lock() {
            Ok(mut p) => {
                *p = (nom.to_string(), dossier);
                Ok(version)
            }
            Err(_) => Err("verrou du personnage empoisonné".to_string()),
        }
    }
}
```

Adapter `nouvelles`, le cas `ID_RECHARGER` et le cas `ID_DOSSIER` pour passer
par `dossier_courant()`.

- [ ] **Étape 2 : brancher `choisir`**

`commandes::choisir` doit maintenant déclencher le rechargement. Elle a besoin
de l'`Actions`, que Tauri peut lui fournir par son état managé :

Dans `main.rs`, après la création d'`Actions` :

```rust
    // `manage` met la valeur à disposition des commandes, qui la reçoivent
    // par un paramètre `State<…>`. C'est le mécanisme d'injection de Tauri —
    // il évite une variable globale.
    app.manage(actions.clone());
```

Dans `commandes.rs`, **remplacer entièrement** le corps de `choisir` écrit en
Tâche 8 (qui n'écrivait que la config) par celui-ci, qui déclenche en plus le
rechargement à chaud :

```rust
#[tauri::command]
pub fn choisir(
    actions: tauri::State<'_, std::sync::Arc<crate::actions::Actions>>,
    nom: String,
) -> Result<(), String> {
    let Some(dossier) = crate::config::dossier_du_personnage(&nom) else {
        return Err(format!("personnage « {nom} » introuvable"));
    };

    // L'ordre compte : on charge D'ABORD, on enregistre ENSUITE. Si le
    // manifeste est illisible, rien n'a changé — ni à l'écran, ni dans la
    // config, qui aurait sinon nommé un personnage qui ne charge pas.
    actions.changer_personnage(&nom, dossier)?;
    crate::config::definir_personnage(&nom)?;

    println!("personnage choisi : {nom}");
    Ok(())
}
```

- [ ] **Étape 3 : faire suivre le webview**

Dans `ui/pet.js`, remplacer :

```js
const personnage = window.location.hash.slice(1) || 'blob';
const BASE = `http://shime.localhost/${personnage}/`;
```

par :

```js
// Le personnage à afficher. `let` et non `const` : il CHANGE désormais, la
// fenêtre du catalogue pouvant en choisir un autre sans redémarrage.
let personnage = window.location.hash.slice(1) || 'blob';

// Recalculée à chaque changement. Sans ça, on demanderait encore les images
// de l'ancien personnage — un personnage parfaitement animé avec le mauvais
// dessin, exactement ce que le commentaire de main.rs:272 redoutait.
let BASE = `http://shime.localhost/${personnage}/`;
```

Et remplacer la fonction `recharger` :

```js
// Appelée par Rust après un rechargement à chaud. `nom` est le personnage
// courant : il peut avoir changé (fenêtre du catalogue), auquel cas c'est
// toute la base d'URL qu'il faut refaire.
window.recharger = (v, nom) => {
  version = v;
  if (nom && nom !== personnage) {
    personnage = nom;
    BASE = `http://shime.localhost/${personnage}/`;
  }
  // On force le prochain `poser` à réécrire le `src`, même si l'image
  // demandée porte le même numéro qu'avant : c'est son CONTENU qui a pu
  // changer, pas son numéro.
  derniere = null;
};
```

- [ ] **Étape 4 : passer le nom depuis Rust**

Dans `render.rs`, **cette ligne et elle seule** :

```rust
    win.eval(format!("window.recharger({version})"))
```

devient :

```rust
    // Le second argument est le personnage courant : il peut avoir changé
    // (fenêtre du catalogue), auquel cas `pet.js` refait sa base d'URL.
    win.eval(format!("window.recharger({version}, \"{personnage}\")"))
```

Ajouter le paramètre `personnage: &str` à cette fonction et à son appel dans
la boucle, qui le lit du `Rechargement` reçu.

⚠️ Cela implique d'ajouter un champ `personnage: String` à la struct
`rechargement::Rechargement` et de le renseigner dans `preparer` — qui reçoit
désormais le dossier, et en tire le nom par `file_name()`.

- [ ] **Étape 5 : vérifier à la main**

```powershell
cargo build
cargo run
```

Dans une autre console :

```powershell
cargo run -- --installer naruto-kakashi
```

Puis, une fois la fenêtre du catalogue disponible (Tâche 10), choisir
`naruto-kakashi` et vérifier que le personnage **change à l'écran sans
redémarrage**.

⚠️ Tant que la Tâche 10 n'est pas faite, vérifier par un raccourci : modifier
temporairement `config.json` pour nommer `naruto-kakashi`, relancer, et
constater qu'il s'affiche. Le changement **à chaud**, lui, ne se vérifie qu'en
Tâche 10 — le noter et y revenir.

- [ ] **Étape 6 : lancer la suite et commiter**

```powershell
cargo test
git add src-tauri/src/actions.rs src-tauri/src/commandes.rs src-tauri/src/rechargement.rs src-tauri/src/render.rs src-tauri/src/main.rs ui/pet.js
git commit -m @'
feat(catalogue): changer de personnage sans redemarrer

Le chemin existait deja a 95 % : rechargement::preparer designait deja le
personnage par parametre et transportait un Manifest complet jusqu'a la
boucle ; main.rs lui passait simplement toujours le meme.

Le seul point dur etait cote webview : pet.js lisait index.html#<perso> une
SEULE fois pour construire BASE. Apres un changement, il demanderait encore
les images de l'ancien personnage — un personnage parfaitement anime avec le
mauvais dessin. window.recharger prend donc un second argument.

choisir charge D'ABORD et enregistre ENSUITE : si le manifeste est
illisible, rien n'a change, ni a l'ecran ni dans la config, qui aurait sinon
nomme un personnage qui ne charge pas.

Le diff de render.rs se limite volontairement a une chaine de format : ce
fichier est le terrain des etapes 4 et 5.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
'@
```

---

## Tâche 10 : la fenêtre du catalogue

**Fichiers :**
- Créer : `ui/catalogue.html`, `ui/catalogue.js`
- Modifier : `src-tauri/src/actions.rs` (créer la fenêtre)

**Interfaces :**
- Consomme : `commandes::{installer, bibliotheque, choisir}` (T8), `ui/catalogue.json` (T7).
- Produit : `actions::ouvrir_catalogue(app: &AppHandle)`

- [ ] **Étape 1 : écrire la page**

`ui/catalogue.html` — **remplace entièrement** la sonde minimale écrite en
Tâche 8, étape 8, qui avait servi à prouver que l'IPC répondait. Jetons
mesurés dans le DOM de shimejis.xyz (spec §9) :

```html
<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>Catalogue de personnages</title>
<style>
  /* Jetons MESURÉS dans le DOM de shimejis.xyz, pas approchés (spec §9). */
  :root {
    --fond: rgb(36, 36, 36);
    --texte: rgb(201, 201, 201);
  }

  body {
    margin: 0;
    background: var(--fond);
    color: var(--texte);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,
                 "Helvetica Neue", Arial, sans-serif;
  }

  header {
    position: sticky;   /* la recherche reste atteignable sur 2000 cartes */
    top: 0;
    background: var(--fond);
    padding: 12px 16px;
    display: flex;
    gap: 12px;
    align-items: center;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
  }

  nav button {
    background: transparent;
    color: var(--texte);
    border: 1px solid rgba(255, 255, 255, 0.2);
    padding: 6px 12px;
    cursor: pointer;
    font: inherit;
  }
  nav button[aria-selected="true"] { background: rgba(255, 255, 255, 0.12); }

  #recherche {
    flex: 1;
    background: rgba(255, 255, 255, 0.06);
    color: var(--texte);
    border: 1px solid rgba(255, 255, 255, 0.2);
    padding: 6px 10px;
    font: inherit;
  }

  h2 { font-size: 15px; font-weight: 600; margin: 20px 16px 4px; }

  /* Flex wrap, PAS une CSS grid : c'est ce que fait shimejis.xyz, et les
     cartes ont une largeur fixe. */
  .grille { display: flex; flex-wrap: wrap; padding: 0 8px; }

  .carte {
    width: 223px;
    height: 144px;
    padding: 8px;
    background: transparent;   /* sans bordure ni ombre : mesuré */
    border: none;
    color: inherit;
    font: inherit;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .carte:hover { background: rgba(255, 255, 255, 0.06); }
  .carte[disabled] { cursor: default; opacity: 0.55; }

  .carte img {
    width: 128px;
    height: 128px;
    object-fit: contain;
    border-radius: 0;
    /* Pixel-art : la mise à l'échelle doit rester nette. */
    image-rendering: pixelated;
  }

  .carte span { font-size: 12px; text-align: center; }
  .pastille { color: #7fd67f; }

  #etat {
    position: fixed;
    bottom: 0; left: 0; right: 0;
    padding: 8px 16px;
    background: rgba(0, 0, 0, 0.85);
    font-size: 13px;
  }
  #etat:empty { display: none; }
</style>
</head>
<body>
  <header>
    <nav>
      <button id="onglet-catalogue" aria-selected="true">Catalogue</button>
      <button id="onglet-biblio" aria-selected="false">Ma bibliothèque</button>
    </nav>
    <input id="recherche" type="search" placeholder="Rechercher un personnage…">
  </header>

  <main id="contenu"></main>
  <div id="etat"></div>

  <script src="catalogue.js"></script>
</body>
</html>
```

- [ ] **Étape 2 : écrire le script**

`ui/catalogue.js` :

```js
// La fenêtre du catalogue (spec §9).
//
// Deux écrans, parce que DEUX GESTES : le catalogue installe, la
// bibliothèque choisit qui s'affiche. Personne ne doit pouvoir confondre
// « je le télécharge » et « je l'affiche ».

const CDN = 'https://sprites.shimejis.xyz/directory';

const contenu = document.getElementById('contenu');
const etat = document.getElementById('etat');
const recherche = document.getElementById('recherche');
const ongletCatalogue = document.getElementById('onglet-catalogue');
const ongletBiblio = document.getElementById('onglet-biblio');

let packs = [];        // l'index, lu une fois
let installes = new Set();
let ecran = 'catalogue';

function dire(message) {
  etat.textContent = message || '';
}

// ── Le catalogue ────────────────────────────────────────────────────────
//
// Les vignettes SONT les shime1.png, tirées du CDN. La CSP est nulle, rien
// ne s'y oppose (spec §3).
//
// `loading="lazy"` : WebView2 est Chromium, il ne charge que ce qui approche
// du viewport. Zéro ligne de défilement virtuel à écrire (spec §9).
function carteCatalogue(pack) {
  const bouton = document.createElement('button');
  bouton.className = 'carte';

  const img = document.createElement('img');
  img.loading = 'lazy';
  img.src = `${CDN}/${pack.slug}/img/shime1.png`;
  img.alt = '';

  const nom = document.createElement('span');
  const deja = installes.has(pack.slug);
  nom.textContent = deja ? `✓ ${pack.nom}` : pack.nom;
  if (deja) {
    nom.className = 'pastille';
    bouton.disabled = true;
  }

  bouton.append(img, nom);
  bouton.addEventListener('click', () => installer(pack));
  return bouton;
}

async function installer(pack) {
  dire(`installation de ${pack.nom}…`);
  try {
    await window.__TAURI__.core.invoke('installer', { slug: pack.slug });
    installes.add(pack.slug);
    dire(`${pack.nom} installé.`);
    rendre();
  } catch (e) {
    // Bruyant : une installation silencieusement ratée laisserait croire
    // que le clic n'a rien fait.
    dire(`échec : ${e}`);
  }
}

// ── La bibliothèque : le SECOND geste ───────────────────────────────────
function carteBibliotheque(pack) {
  const bouton = document.createElement('button');
  bouton.className = 'carte';

  const img = document.createElement('img');
  img.loading = 'lazy';
  // Le schéma servi par Rust : les images sont des fichiers EXTERNES au
  // binaire, aucun chemin relatif ne peut les atteindre.
  img.src = `http://shime.localhost/${pack.nom}/1`;
  img.alt = '';

  const nom = document.createElement('span');
  nom.textContent = pack.actif ? `● ${pack.nom}` : pack.nom;
  if (pack.actif) nom.className = 'pastille';

  bouton.append(img, nom);
  bouton.addEventListener('click', async () => {
    dire(`affichage de ${pack.nom}…`);
    try {
      await window.__TAURI__.core.invoke('choisir', { nom: pack.nom });
      dire(`${pack.nom} s'affiche.`);
      rendre();
    } catch (e) {
      dire(`échec : ${e}`);
    }
  });
  return bouton;
}

// ── Le rendu ────────────────────────────────────────────────────────────
async function rendre() {
  contenu.textContent = '';
  const filtre = recherche.value.trim().toLowerCase();

  if (ecran === 'biblio') {
    const liste = await window.__TAURI__.core.invoke('bibliotheque');
    const grille = document.createElement('div');
    grille.className = 'grille';
    for (const p of liste) {
      if (!filtre || p.nom.toLowerCase().includes(filtre)) {
        grille.append(carteBibliotheque(p));
      }
    }
    contenu.append(grille);
    return;
  }

  // Groupés par franchise, alphabétiquement (spec §9).
  const parFranchise = new Map();
  for (const p of packs) {
    if (filtre && !p.nom.toLowerCase().includes(filtre)
        && !p.franchise.toLowerCase().includes(filtre)) {
      continue;
    }
    if (!parFranchise.has(p.franchise)) parFranchise.set(p.franchise, []);
    parFranchise.get(p.franchise).push(p);
  }

  for (const franchise of [...parFranchise.keys()].sort()) {
    const titre = document.createElement('h2');
    titre.textContent = franchise;
    const grille = document.createElement('div');
    grille.className = 'grille';
    for (const p of parFranchise.get(franchise)) {
      grille.append(carteCatalogue(p));
    }
    contenu.append(titre, grille);
  }
}

// ── Démarrage ───────────────────────────────────────────────────────────
ongletCatalogue.addEventListener('click', () => {
  ecran = 'catalogue';
  ongletCatalogue.setAttribute('aria-selected', 'true');
  ongletBiblio.setAttribute('aria-selected', 'false');
  rendre();
});
ongletBiblio.addEventListener('click', () => {
  ecran = 'biblio';
  ongletCatalogue.setAttribute('aria-selected', 'false');
  ongletBiblio.setAttribute('aria-selected', 'true');
  rendre();
});
recherche.addEventListener('input', rendre);

// L'index est un fichier statique servi avec le reste du front : un `fetch`
// local suffit. Le faire transiter par l'IPC serait 130 Ko de données
// statiques passées par un pont fait pour des messages (spec §9).
(async function demarrer() {
  try {
    const reponse = await fetch('catalogue.json');
    const index = await reponse.json();
    packs = index.packs;
  } catch (e) {
    dire(`catalogue.json illisible : ${e}`);
    return;
  }

  const liste = await window.__TAURI__.core.invoke('bibliotheque');
  installes = new Set(liste.map((p) => p.nom));

  window.__TAURI__.event.listen('installation', (evenement) => {
    const { slug, fait, total } = evenement.payload;
    dire(`${slug} : ${fait}/${total}`);
  });

  dire(`${packs.length} personnages disponibles.`);
  rendre();
})();
```

- [ ] **Étape 3 : créer la fenêtre depuis Rust**

Dans `actions.rs` :

```rust
/// Ouvre la fenêtre du catalogue (spec §9).
///
/// **Tout l'inverse de la fenêtre du personnage** : décorée,
/// redimensionnable, focalisable, dans la barre des tâches. Il ne faut rien
/// réutiliser de l'autre — leurs contraintes sont opposées.
///
/// Créée à la demande et détruite à la fermeture, jamais masquée : une
/// fenêtre WebView2 vivante coûte de la mémoire pour rien, et le catalogue
/// s'ouvre quelques fois dans une vie.
pub fn ouvrir_catalogue(app: &AppHandle) {
    const LABEL: &str = "catalogue";

    // Déjà ouverte : on la met au premier plan plutôt que d'en créer une
    // seconde. `set_focus` ici est légitime — contrairement au personnage,
    // c'est une fenêtre que l'utilisateur vient d'appeler.
    if let Some(win) = app.get_webview_window(LABEL) {
        let _ = win.set_focus();
        return;
    }

    let resultat = tauri::WebviewWindowBuilder::new(
        app,
        LABEL,
        tauri::WebviewUrl::App("catalogue.html".into()),
    )
    .title("Catalogue de personnages")
    .inner_size(1000.0, 700.0)
    .resizable(true)
    .build();

    if let Err(e) = resultat {
        eprintln!("fenêtre du catalogue : {e}");
    }
}
```

- [ ] **Étape 4 : vérifier à la main**

```powershell
cargo build
cargo run
```

Vérifier, dans l'ordre :
1. la fenêtre s'ouvre (via le menu, Tâche 11 — en attendant, appeler
   `ouvrir_catalogue` depuis `setup` temporairement) ;
2. les vignettes se chargent depuis le CDN ;
3. faire défiler : les images se chargent au fur et à mesure, pas toutes d'un
   coup (ouvrir l'inspecteur avec F12 et regarder l'onglet réseau) ;
4. la recherche filtre ;
5. cliquer une carte : la progression s'affiche, la pastille apparaît ;
6. onglet « Ma bibliothèque » : le pack installé y figure ;
7. cliquer dessus : **le personnage change à l'écran sans redémarrage** —
   c'est la vérification de la Tâche 9, reportée ici.

⚠️ Si les vignettes ne se chargent pas, vérifier que `"csp": null` est
toujours dans `tauri.conf.json` : c'est ce qui autorise le CDN (spec §3).

- [ ] **Étape 5 : commit**

```powershell
git add ui/catalogue.html ui/catalogue.js src-tauri/src/actions.rs
git commit -m @'
feat(catalogue): la fenetre, deux ecrans et deux gestes

Deux ecrans parce que deux gestes : le catalogue INSTALLE, la bibliotheque
CHOISIT qui s'affiche. Personne ne peut confondre « je le telecharge » et
« je l'affiche », ce qui etait tout le point.

Les 2000 vignettes sont chargees par loading="lazy" : WebView2 est Chromium,
il ne charge que ce qui approche du viewport. Zero ligne de defilement
virtuel a ecrire — on declare une intention, le moteur fait le travail.

Les vignettes SONT les shime1.png, tirees du CDN directement : la CSP est
nulle, rien ne s'y oppose. Les jetons de design sont mesures dans le DOM de
shimejis.xyz, pas approches.

La fenetre est tout l'inverse de celle du personnage : decoree,
redimensionnable, focalisable, dans la taskbar. Creee a la demande et
detruite a la fermeture, jamais masquee.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
'@
```

---

## Tâche 11 : les entrées de menu

**Fichiers :**
- Modifier : `src-tauri/src/actions.rs` (identifiant + cas)
- Modifier : `src-tauri/src/tray.rs` (entrée)
- Modifier : `src-tauri/src/menu_perso.rs` (entrée)

**Interfaces :**
- Consomme : `actions::ouvrir_catalogue` (T10).
- Produit : `actions::ID_CATALOGUE: &str`

> ⚠️ **Un seul `on_menu_event` dans tout le programme.** Les deux menus
> émettent le **même identifiant** et `actions::executer` gagne **un seul
> cas**. Ne surtout pas ajouter de gestionnaire à `menu_perso.rs` : Tauri
> livre chaque événement à *tous* les gestionnaires, l'action s'exécuterait
> deux fois.

- [ ] **Étape 1 : déclarer l'identifiant**

Dans `actions.rs`, auprès des autres :

```rust
/// Proposée par les DEUX menus, comme `recharger` et `dossier` : elle fait
/// exactement la même chose depuis l'un ou l'autre, donc un seul
/// identifiant.
pub const ID_CATALOGUE: &str = "catalogue";
```

- [ ] **Étape 2 : ajouter le cas**

Dans `executer`, dans la section « entrées communes aux deux menus » :

```rust
        ID_CATALOGUE => {
            ouvrir_catalogue(app);
        }
```

- [ ] **Étape 3 : ajouter l'entrée au tray**

Dans `tray.rs`, importer `ID_CATALOGUE` et créer l'entrée :

```rust
    let catalogue = MenuItem::with_id(
        app,
        ID_CATALOGUE,
        "Catalogue de personnages…",
        true,
        None::<&str>,
    )
    .map_err(|e| format!("entrée « catalogue » : {e}"))?;
```

puis l'insérer dans `Menu::with_items`, après `dossier`.

- [ ] **Étape 4 : ajouter l'entrée au menu du personnage**

Dans `menu_perso.rs`, importer `ID_CATALOGUE` et créer l'entrée à côté de
`cacher`, puis l'ajouter à la liste des entrées communes.

> C'est la règle du projet : **toute nouvelle action se branche au menu du
> clic droit dans la même tâche**. L'oubli ne casse aucun test et ne produit
> aucun message — l'entrée resterait simplement à jamais hors de portée.

- [ ] **Étape 5 : vérifier à la main**

```powershell
cargo build
cargo run
```

1. Clic droit sur l'icône du tray (⚠️ elle est derrière le chevron `^` :
   Windows 11 masque par défaut les icônes qu'il ne connaît pas) →
   « Catalogue de personnages… » → la fenêtre s'ouvre.
2. La fermer, puis clic droit **sur le personnage** → la même entrée → la
   fenêtre s'ouvre.
3. **Le test qui compte** : laisser la fenêtre ouverte, rappeler l'entrée
   depuis l'autre menu. Elle doit revenir au premier plan, **pas** en ouvrir
   une seconde — et surtout, aucune action ne doit s'exécuter deux fois.
4. Retirer temporairement le `setup` de test de la Tâche 10 s'il y est encore.

- [ ] **Étape 6 : commit**

```powershell
cargo test
git add src-tauri/src/actions.rs src-tauri/src/tray.rs src-tauri/src/menu_perso.rs
git commit -m @'
feat(menu): le catalogue s'ouvre depuis le tray et le clic droit

Un seul identifiant, un seul cas dans executer, deux menus qui le proposent.
C'est la regle du projet : toute nouvelle action se branche au menu du clic
droit dans la meme tache — son oubli ne casse aucun test et ne produit aucun
message, l'entree resterait simplement hors de portee.

Et surtout : aucun second on_menu_event. Tauri livre chaque evenement de
menu a TOUS les gestionnaires, quel que soit le menu d'origine ; un second
gestionnaire ouvrirait donc le catalogue deux fois.

La fenetre deja ouverte revient au premier plan au lieu d'etre dupliquee.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
'@
```

---

## Tâche 12 : retirer les cinq packs, et mettre à jour la documentation

**Fichiers :**
- Supprimer : `characters/{luffy,naruto-kakashi,one-piece-zoro-01,pierrot-54acb5,group-finity-blank-guy}/`
- Modifier : `CLAUDE.md`
- Modifier : `docs/specs/2026-09-11-catalogue-de-personnages-design.md` (si la franchise a dû se replier)

> ⚠️ **Cette tâche vient en dernier, jamais avant** : la branche ne doit
> jamais passer par un état où l'application n'a plus rien à afficher.

- [ ] **Étape 1 : vérifier la config locale AVANT de supprimer**

```powershell
Get-Content config.json -ErrorAction SilentlyContinue
Get-Content "$env:APPDATA\shimeji-desktop\config.json" -ErrorAction SilentlyContinue
```

Si la clé `personnages` nomme un des cinq packs retirés, l'application
s'arrêtera sur `personnage « … » introuvable`. Deux sorties, au choix :
installer ce pack (`cargo run -- --installer <slug>`) ou remettre `blob`.

- [ ] **Étape 2 : supprimer les cinq packs**

```powershell
Remove-Item -Recurse -Force characters\luffy, characters\naruto-kakashi, characters\one-piece-zoro-01, characters\pierrot-54acb5, characters\group-finity-blank-guy
Get-ChildItem characters
```
Attendu : `blob` seul.

- [ ] **Étape 3 : vérifier que l'application démarre encore**

```powershell
cargo build
$env:SHIMEJI_QUITTER_APRES=8; cargo run
```
Attendu : `personnage chargé : Shimeji (mascotte par défaut)`, aucune erreur.

- [ ] **Étape 4 : vérifier qu'un pack installé survit à la suppression**

```powershell
cargo run -- --installer one-piece-zoro-01
cargo run
```
Ouvrir le catalogue → « Ma bibliothèque » → `one-piece-zoro-01` doit y figurer
et s'afficher au clic, bien qu'il ne soit plus dans le dépôt. **C'est la
démonstration que le catalogue remplace réellement les packs versionnés.**

- [ ] **Étape 5 : lancer toute la suite**

```powershell
cargo test
```
Attendu : PASS. ⚠️ Si un test référençait un des packs retirés, le corriger
pour qu'il vise `blob`.

- [ ] **Étape 6 : mettre à jour `CLAUDE.md`**

Trois sections à reprendre :

1. **« Les packs installés, et leur statut »** — ne garder que `blob`, et dire
   que les autres s'installent depuis le catalogue.
2. **« Ajouter un pack depuis shimejis.xyz »** — remplacer la procédure
   manuelle par : le catalogue in-app, ou `cargo run -- --installer <slug>`.
3. **« Compiler et lancer »** — ajouter `cargo run -- --installer <slug>` à la
   liste des commandes, et `%APPDATA%\shimeji-desktop\characters\` à la
   description des dossiers.

Ajouter aussi, dans « État actuel », une ligne renvoyant à la spec et à ce
plan.

- [ ] **Étape 7 : commit**

```powershell
git add -A
git commit -m @'
feat(catalogue): retire les cinq packs sous droits, blob reste

3,2 Mo de sprites quittent le depot. Les cinq restent installables en un
clic depuis le catalogue : rien n'est perdu, c'est deplace du depot vers la
bibliotheque sous %APPDATA%.

En dernier et jamais avant : la branche ne doit jamais passer par un etat ou
l'application n'a plus rien a afficher.

Le gain juridique reste PARTIEL et il faut le dire : blob est la mascotte de
Shimeji-ee et son art n'est pas de nous non plus. Un depot totalement net
demanderait de le sortir aussi — mais l'application sans reseau n'aurait
alors aucun personnage, et le premier lancement tomberait sur un ecran vide.
Arbitrage assume.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>
'@
```

---

## Vérification finale

- [ ] `cargo test` — vert, et le nombre de tests a augmenté d'au moins 15.
- [ ] `cargo build --release` — l'exe reste **sous 3,5 Mo**. La crate `png`
      l'alourdit ; si le seuil est franchi, le dire dans le rapport, ne pas
      le masquer.

```powershell
cargo build --release
(Get-Item target\release\shimeji-desktop.exe).Length / 1MB
```

- [ ] `git diff etape-2-il-reagit --stat -- src-tauri/src/attach.rs src-tauri/src/render.rs`
      — **`attach.rs` doit être absent**, et `render.rs` ne montrer qu'une
      poignée de lignes (la chaîne de format et sa signature). C'est la
      contrainte de cadrage, et elle se vérifie d'un coup d'œil.

- [ ] **Pas de mesure CPU** : la boucle 60 Hz n'est pas touchée et la fenêtre
      du catalogue est transitoire (spec §11). En lancer une « pour se
      rassurer » ne prouverait rien, et le `CLAUDE.md` explique pourquoi.
