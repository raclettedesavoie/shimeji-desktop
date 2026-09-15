# Plusieurs personnages simultanés — plan d'implémentation

> **Pour l'exécutant :** ce plan s'exécute **tâche par tâche**, dans l'ordre.
> Chaque tâche se termine par une suite verte et un commit. Les étapes sont
> des cases à cocher (`- [ ]`) — une étape = une action de 2 à 5 minutes.

**But :** faire vivre plusieurs personnages à l'écran en même temps, les
activer et les désactiver depuis « Ma bibliothèque » avec un compteur par pack,
supprimer un pack du disque, et faire apparaître chaque nouveau personnage en
tombant du haut de l'écran.

**Architecture :** une seule boucle à 60 Hz pilotant un `Vec<Acteur>`. Le
travail partagé (signaux à 2 Hz, recensement du monde à 8 Hz, sonde du curseur)
est payé une fois ; le travail par personnage (physique, comportement, rendu,
déplacement) est payé N fois. `config.personnages` devient un **multi-ensemble**
où une répétition vaut un exemplaire de plus.

**Stack :** Rust, Tauri v2.11.5, crate `windows` 0.61. Pas de TypeScript, pas de
bundler, pas de framework front.

**Spec :** `docs/specs/2026-09-15-plusieurs-personnages-design.md` — à lire avant
de commencer. Le plan argumente *depuis* elle ; les deux voyagent ensemble.

---

## Contraintes globales

Elles valent pour **toutes** les tâches, sans être répétées.

- **Compiler depuis PowerShell, jamais depuis Git Bash.** Dans Git Bash, rustc
  peut pêcher le `link.exe` de Git for Windows au lieu du linker MSVC et rendre
  une erreur `extra operand` opaque.
- **Borner les sorties de build** : `cargo test --quiet`, et
  `cargo build 2>&1 | Select-Object -Last 40`. Une erreur `cargo` non tronquée
  pèse plusieurs milliers de tokens.
- **`cargo test` ne reconstruit pas l'exe.** Après une correction, `cargo build`
  avant de relancer l'application, sinon on vérifie un binaire plus ancien que
  la source.
- **Le code est abondamment commenté, en français**, et commente le *pourquoi*.
  Chaque bloc appliquant une décision du design **cite sa section**.
  C'est une exigence explicite de l'auteur, pas une préférence de style.
- **Toute nouvelle action jouable se branche au menu du clic droit dans la même
  tâche** — une ligne dans `ENVIES` de `menu_perso.rs`. (Aucune tâche de ce plan
  n'ajoute d'intention jouable ; la règle est rappelée parce que son oubli ne
  casse aucun test.)
- **Un seul `on_menu_event` dans tout le programme**, installé par `tray.rs`.
  Tauri livre chaque événement de menu à *tous* les gestionnaires.
- **Un seul RNG, semé une fois.** Ne jamais créer un `XorShift32::seeded(n)` par
  personnage avec de petits entiers séquentiels : le premier tirage serait
  biaisé vers le même index pour tous (piège documenté dans `CLAUDE.md`).
- **Les tests d'un module vivent dans `<module>_tests.rs`**, inclus par
  `#[cfg(test)] #[path = "<module>_tests.rs"] mod tests;` en fin de fichier.
- Le point de départ est la branche **`plusieurs-personnages`**, **266 tests
  verts**.

---

## Structure des fichiers

| Fichier | Responsabilité | État |
|---|---|---|
| `src-tauri/src/roster.rs` | La réconciliation « présents vs voulus ». **Pure** : ne connaît ni Tauri, ni le disque | **créé** (Tâche 2) |
| `src-tauri/src/roster_tests.rs` | Ses tests | **créé** (Tâche 2) |
| `src-tauri/src/apparition.rs` | Où naît un personnage : écran et `x` tirés au sort. **Pure** | **créé** (Tâche 5) |
| `src-tauri/src/apparition_tests.rs` | Ses tests, dont le non-accrochage au plafond | **créé** (Tâche 5) |
| `src-tauri/src/main.rs` | `Acteur`, le `Vec`, la boucle, les fenêtres, l'élection sous le curseur, le départ | modifié (Tâches 1, 4, 5, 6, 7) |
| `src-tauri/src/config.rs` | Le multi-ensemble, la liste vide | modifié (Tâche 3) |
| `src-tauri/src/commandes.rs` | `definir_compte`, `supprimer` ; `PackInstalle` enrichi | modifié (Tâches 8, 9) |
| `src-tauri/src/actions.rs` | Le roster partagé remplace `perso` | modifié (Tâche 6) |
| `src-tauri/src/rechargement.rs` | La demande porte un roster | modifié (Tâche 6) |
| `src-tauri/src/tray.rs` | Visibilité sur tous les acteurs | modifié (Tâche 4) |
| `ui/catalogue.js`, `ui/catalogue.html` | La carte de bibliothèque, le bandeau des 10 | modifié (Tâches 8, 9, 10) |

---

## Tâche 1 — Le spike : créer et détruire une fenêtre depuis le thread de la boucle

**Pourquoi en premier.** Tout le reste en dépend. Aujourd'hui l'unique fenêtre
naît dans `setup`, sur le thread principal. Demain elles naissent et meurent
depuis le thread de la boucle. Le design (§3) raisonne sur les sources de
`tauri-runtime-wry` — qui panique si `WindowMessage::Close` est traité *sur* le
thread principal — mais **ce raisonnement n'est pas vérifié à l'exécution**. Si
ça ne marche pas, l'architecture change, et il vaut mieux le savoir maintenant.

**Fichiers :**
- Modifier : `src-tauri/src/main.rs` (dans `setup`, avant le lancement de la boucle)

**Interfaces :**
- Produit : `fn creer_fenetre_personnage(app: &tauri::AppHandle, label: &str, nom: &str, taille: (u32, u32)) -> Result<tauri::WebviewWindow, String>` — extraite du code de `setup`, réutilisée par toutes les tâches suivantes.

- [ ] **Étape 1 : extraire la création de fenêtre dans une fonction**

Dans `main.rs`, sortir le bloc `WebviewWindowBuilder` de `setup` vers une
fonction libre. **Reprendre exactement la combinaison actuelle**, sans rien
retirer : ce sont les propriétés validées par l'étape 0.

```rust
/// Crée la fenêtre d'UN personnage.
///
/// Extraite de `setup` : les fenêtres naissent désormais en cours
/// d'exécution, depuis le thread de la boucle, et plus seulement au
/// démarrage (design §3).
///
/// ⚠️ Les deux styles étendus sont posés ICI et pas ailleurs. Les oublier
/// sur les fenêtres n° 2 et suivantes donnerait des personnages qui volent
/// le focus et apparaissent dans Alt+Tab — un défaut qui ne se verrait que
/// sur le deuxième personnage, donc jamais pendant le développement d'une
/// tâche à N=1.
fn creer_fenetre_personnage(
    app: &tauri::AppHandle,
    label: &str,
    nom: &str,
    taille: (u32, u32),
) -> Result<tauri::WebviewWindow, String> {
    // Le fragment dit à `pet.js` quel personnage servir. Il doit porter le
    // nom du pack, jamais `blob` en dur : sinon le manifeste chargé serait
    // le bon et les IMAGES celles de blob — un personnage parfaitement
    // animé avec le mauvais dessin, que rien ne signale.
    let win = tauri::WebviewWindowBuilder::new(
        app,
        label,
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

    // Non bloquant : la fenêtre marche sans, elle est seulement moins
    // polie. Mieux vaut un personnage qui vole le focus qu'aucun personnage.
    match render::appliquer_styles_etendus(&win) {
        Ok(()) => println!("styles étendus posés sur {label} (NOACTIVATE, TOOLWINDOW)"),
        Err(e) => eprintln!("styles étendus NON appliqués sur {label} : {e}"),
    }

    Ok(win)
}
```

Remplacer le bloc correspondant de `setup` par un appel à cette fonction.

- [ ] **Étape 2 : vérifier que rien n'a changé**

Run : `cargo test --quiet`
Attendu : **266 passed**, aucun échec.

Run : `cargo build 2>&1 | Select-Object -Last 10` puis `cargo run`
Attendu : le personnage apparaît et marche exactement comme avant.
Arrêter par « Quitter » dans le tray.

- [ ] **Étape 3 : écrire le spike de création/destruction depuis un thread**

Dans `setup`, **temporairement**, juste avant le lancement de la boucle :

```rust
// ── SPIKE (Tâche 1) : à retirer à l'étape 6 de cette tâche ──────────
//
// Il s'agit de PROUVER, et non de supposer, qu'une fenêtre de
// personnage peut naître et mourir depuis un thread autre que le
// principal. Les sources de `tauri-runtime-wry` le laissent penser
// (lib.rs:3492 ne panique que dans l'autre sens), mais toute
// l'architecture de cette étape en dépend.
if std::env::var("SHIMEJI_SPIKE_FENETRES").is_ok() {
    let h = app.handle().clone();
    let t = taille;
    std::thread::spawn(move || {
        for i in 0..20 {
            let label = format!("spike-{i}");
            match creer_fenetre_personnage(&h, &label, "blob", t) {
                Ok(w) => {
                    println!("[spike] {label} créée");
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    match w.destroy() {
                        Ok(()) => println!("[spike] {label} détruite"),
                        Err(e) => println!("[spike] ÉCHEC destruction {label} : {e}"),
                    }
                }
                Err(e) => println!("[spike] ÉCHEC création {label} : {e}"),
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        println!("[spike] terminé : 20 cycles création/destruction");
    });
}
```

- [ ] **Étape 4 : exécuter le spike**

Run :
```powershell
cd C:\Users\alri\Documents\shimeji-desktop\src-tauri
cargo build 2>&1 | Select-Object -Last 10
$env:SHIMEJI_SPIKE_FENETRES="1"; $env:SHIMEJI_QUITTER_APRES="20"; cargo run
```

Attendu : 20 lignes « créée » **et** 20 lignes « détruite », aucun `ÉCHEC`,
aucun panic, et le processus se termine seul.

- [ ] **Étape 5 : vérifier qu'aucune fenêtre ne fuit**

Pendant l'exécution, dans un autre terminal :
```powershell
(Get-Process -Name shimeji-desktop).MainWindowHandle
Get-Process -Name shimeji-desktop | Select-Object Handles, WorkingSet
```
Attendu : le nombre de handles ne croît pas de façon monotone sur les 20 cycles.

> **Si le spike échoue** (panic, ou destruction refusée) : **arrêter le plan et
> le signaler**. Le repli serait de faire créer et détruire les fenêtres par le
> thread principal via `handle.run_on_main_thread(…)`, la boucle se contentant
> de demander. Ce n'est pas une variante à improviser en cours de route : c'est
> une révision du design §3.

- [ ] **Étape 6 : retirer le spike et committer**

Supprimer le bloc `SHIMEJI_SPIKE_FENETRES` en entier. Garder
`creer_fenetre_personnage`.

Run : `cargo test --quiet` → 266 passed.

```bash
git add -A
git commit -m "refactor(fenetres): extrait creer_fenetre_personnage de setup

Prealable a N personnages : les fenetres naissent desormais en cours
d'execution et plus seulement au demarrage. Un spike jetable a PROUVE
qu'une fenetre peut naitre et mourir depuis le thread de la boucle
(20 cycles creation/destruction, aucun handle fuite) plutot que de le
deduire des sources de tauri-runtime-wry.

Les deux styles etendus de l'etape 0 sont poses dans cette fonction, et
non chez l'appelant : les oublier sur la fenetre n 2 serait invisible a
N=1.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Tâche 2 — `roster.rs` : la réconciliation, pure et testée

**Pourquoi maintenant.** C'est le cœur logique de l'étape, et il ne touche ni
Tauri, ni le disque, ni l'écran. Il se teste entièrement en `cargo test`.

**Fichiers :**
- Créer : `src-tauri/src/roster.rs`
- Créer : `src-tauri/src/roster_tests.rs`
- Modifier : `src-tauri/src/main.rs` (ajouter `mod roster;`)

**Interfaces :**
- Produit :
  - `enum ActionRoster { Creer(String), RetirerUn(String) }` (dérive `Debug`, `PartialEq`)
  - `fn reconcilier(presents: &[String], voulus: &[String]) -> Vec<ActionRoster>`
  - `fn compte_de(liste: &[String], nom: &str) -> usize`

- [ ] **Étape 1 : écrire les tests qui échouent**

Créer `src-tauri/src/roster_tests.rs` :

```rust
//! Les tests de `roster` — la réconciliation « présents vs voulus ».
use super::*;

/// Un raccourci : les tests manipulent des listes de noms, et
/// `vec!["blob"]` ne se convertit pas tout seul en `Vec<String>`.
fn noms(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn rien_a_faire_quand_les_deux_listes_concordent() {
    let actions = reconcilier(&noms(&["blob", "luffy"]), &noms(&["blob", "luffy"]));
    assert!(actions.is_empty());
}

#[test]
fn l_ordre_des_listes_n_a_aucune_importance() {
    // Un multi-ensemble n'est pas une liste ordonnée : « blob puis luffy »
    // et « luffy puis blob » décrivent le même écran.
    let actions = reconcilier(&noms(&["blob", "luffy"]), &noms(&["luffy", "blob"]));
    assert!(actions.is_empty());
}

#[test]
fn un_nom_voulu_et_absent_se_cree() {
    let actions = reconcilier(&noms(&[]), &noms(&["blob"]));
    assert_eq!(actions, vec![ActionRoster::Creer("blob".to_string())]);
}

#[test]
fn un_nom_present_et_non_voulu_se_retire() {
    let actions = reconcilier(&noms(&["blob"]), &noms(&[]));
    assert_eq!(actions, vec![ActionRoster::RetirerUn("blob".to_string())]);
}

#[test]
fn les_doublons_sont_comptes_et_non_dedoublonnes() {
    // Le cœur du multi-ensemble : deux blob voulus quand un seul existe
    // donne UNE création, pas zéro.
    let actions = reconcilier(&noms(&["blob"]), &noms(&["blob", "blob"]));
    assert_eq!(actions, vec![ActionRoster::Creer("blob".to_string())]);
}

#[test]
fn trois_voulus_pour_un_present_donnent_deux_creations() {
    let actions = reconcilier(&noms(&["blob"]), &noms(&["blob", "blob", "blob"]));
    assert_eq!(
        actions,
        vec![
            ActionRoster::Creer("blob".to_string()),
            ActionRoster::Creer("blob".to_string()),
        ]
    );
}

#[test]
fn un_de_trois_se_retire_une_seule_fois() {
    let actions = reconcilier(&noms(&["blob", "blob", "blob"]), &noms(&["blob", "blob"]));
    assert_eq!(actions, vec![ActionRoster::RetirerUn("blob".to_string())]);
}

#[test]
fn creations_et_retraits_coexistent_dans_une_meme_reconciliation() {
    let actions = reconcilier(&noms(&["blob", "zoro"]), &noms(&["blob", "luffy"]));
    // Les retraits AVANT les créations : à nombre constant de personnages,
    // on ne veut pas de pic transitoire (design §2, le CPU).
    assert_eq!(
        actions,
        vec![
            ActionRoster::RetirerUn("zoro".to_string()),
            ActionRoster::Creer("luffy".to_string()),
        ]
    );
}

#[test]
fn tout_retirer_est_permis() {
    // La liste vide est un état NORMAL, pas une erreur (design §4).
    let actions = reconcilier(&noms(&["blob", "luffy"]), &noms(&[]));
    assert_eq!(actions.len(), 2);
    assert!(actions.contains(&ActionRoster::RetirerUn("blob".to_string())));
    assert!(actions.contains(&ActionRoster::RetirerUn("luffy".to_string())));
}

#[test]
fn compte_de_compte_les_occurrences() {
    let l = noms(&["blob", "luffy", "blob"]);
    assert_eq!(compte_de(&l, "blob"), 2);
    assert_eq!(compte_de(&l, "luffy"), 1);
    assert_eq!(compte_de(&l, "zoro"), 0);
}
```

- [ ] **Étape 2 : créer le module et l'enregistrer**

Créer `src-tauri/src/roster.rs` avec l'en-tête et le squelette qui ne compile
pas encore (les fonctions ont un corps `todo!()`), et ajouter `mod roster;`
dans la liste des modules de `main.rs` (ordre alphabétique, après
`mod render;`).

- [ ] **Étape 3 : lancer les tests pour les voir échouer**

Run : `cargo test --quiet roster 2>&1 | Select-Object -Last 20`
Attendu : ÉCHEC — `not yet implemented` sur chaque test.

- [ ] **Étape 4 : écrire l'implémentation**

```rust
//! La réconciliation du roster : ce qu'il faut faire pour que les
//! personnages présents à l'écran correspondent à la liste voulue.
//!
//! Responsabilité unique, et **fonction pure** : ce module ne connaît ni
//! Tauri, ni le disque, ni l'écran. C'est ce qui rend l'étape « plusieurs
//! personnages » entièrement testable sans écran (design §4).

use std::collections::BTreeMap;

/// Ce qu'il faut faire d'un nom pour rapprocher le présent du voulu.
///
/// `PartialEq` et `Debug` : les tests comparent des `Vec<ActionRoster>`
/// entiers, ce qui donne des messages d'échec lisibles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionRoster {
    /// Il manque un exemplaire de ce personnage : en créer un.
    Creer(String),
    /// Il y en a un de trop : en retirer un (le plus récent, côté appelant).
    RetirerUn(String),
}

/// Combien d'exemplaires de `nom` dans cette liste.
///
/// C'est **la seule définition du compteur** affiché par la bibliothèque :
/// `config.personnages` est un multi-ensemble, et le compteur en est le
/// nombre d'occurrences (design §4). Aucune autre source de vérité.
pub fn compte_de(liste: &[String], nom: &str) -> usize {
    liste.iter().filter(|n| n.as_str() == nom).count()
}

/// Ce qu'il faut faire pour passer de `presents` à `voulus`.
///
/// **L'ordre des deux listes n'a aucune importance** : ce sont des
/// multi-ensembles, pas des séquences. Deux blob et un luffy décrivent le
/// même écran quel que soit l'ordre d'écriture dans `config.json`.
///
/// Les **retraits sont rendus avant les créations**. À nombre constant de
/// personnages — remplacer zoro par luffy — l'ordre inverse ferait exister
/// brièvement un personnage de plus, donc une fenêtre en couche de plus à
/// déplacer. Le coût est proportionnel au nombre de déplacements (design
/// §2), et ce pic n'a aucune raison d'être payé.
pub fn reconcilier(presents: &[String], voulus: &[String]) -> Vec<ActionRoster> {
    // `BTreeMap` et non `HashMap` : il range par ordre alphabétique, donc
    // deux appels avec les mêmes listes rendent EXACTEMENT le même Vec.
    // Un `HashMap` rendrait un ordre différent à chaque exécution, et les
    // tests seraient intermittents — le pire genre de test.
    let mut compte: BTreeMap<&str, i32> = BTreeMap::new();

    for n in presents {
        *compte.entry(n.as_str()).or_insert(0) -= 1;
    }
    for n in voulus {
        *compte.entry(n.as_str()).or_insert(0) += 1;
    }

    // Deux passes, pour que TOUS les retraits précèdent TOUTES les
    // créations — une seule passe les entrelacerait par ordre alphabétique.
    let mut actions = Vec::new();

    for (nom, delta) in &compte {
        // `delta < 0` : il y en a plus de présents que de voulus.
        for _ in 0..(-*delta).max(0) {
            actions.push(ActionRoster::RetirerUn(nom.to_string()));
        }
    }
    for (nom, delta) in &compte {
        for _ in 0..(*delta).max(0) {
            actions.push(ActionRoster::Creer(nom.to_string()));
        }
    }

    actions
}

// Les tests de ce module vivent dans `roster_tests.rs`.
#[cfg(test)]
#[path = "roster_tests.rs"]
mod tests;
```

- [ ] **Étape 5 : lancer les tests**

Run : `cargo test --quiet roster 2>&1 | Select-Object -Last 20`
Attendu : **10 passed**.

Run : `cargo test --quiet 2>&1 | Select-Object -Last 5`
Attendu : **276 passed** (266 + 10).

- [ ] **Étape 6 : commit**

```bash
git add src-tauri/src/roster.rs src-tauri/src/roster_tests.rs src-tauri/src/main.rs
git commit -m "feat(roster): la reconciliation presents/voulus, pure et testee

config.personnages devient un multi-ensemble : une repetition vaut un
exemplaire de plus. reconcilier() rend les creations et retraits, retraits
d'abord pour ne pas payer un pic de fenetres a nombre constant.

BTreeMap et non HashMap : l'ordre rendu doit etre le meme d'une execution
a l'autre, sinon les tests seraient intermittents.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Tâche 3 — `config` : le multi-ensemble et la liste vide

**Fichiers :**
- Modifier : `src-tauri/src/config.rs` (`ecrire_personnage` → `ecrire_personnages`, `definir_personnage` → `definir_personnages`)
- Modifier : `src-tauri/src/config_tests.rs`
- Modifier : `src-tauri/src/commandes.rs` (l'appel dans `choisir`, provisoirement)

**Interfaces :**
- Consomme : `config::chemin_charge() -> Option<PathBuf>`, `config::lire_json`
- Produit :
  - `fn ecrire_personnages(chemin: &Path, noms: &[String]) -> Result<(), String>`
  - `fn definir_personnages(noms: &[String]) -> Result<(), String>`

- [ ] **Étape 1 : écrire les tests qui échouent**

Ajouter à `src-tauri/src/config_tests.rs` :

```rust
#[test]
fn ecrire_personnages_ecrit_un_multi_ensemble() {
    let dossier = std::env::temp_dir().join("shimeji-test-multi-ensemble");
    let _ = std::fs::create_dir_all(&dossier);
    let chemin = dossier.join("config.json");
    let _ = std::fs::remove_file(&chemin);

    let noms = vec!["blob".to_string(), "blob".to_string(), "luffy".to_string()];
    ecrire_personnages(&chemin, &noms).expect("écriture");

    let relu = charger_depuis(&chemin);
    // Les doublons SURVIVENT : c'est tout l'objet du multi-ensemble.
    assert_eq!(relu.personnages, noms);
}

#[test]
fn ecrire_personnages_accepte_la_liste_vide() {
    // Zéro personnage est un état NORMAL (design §4) : décocher le dernier
    // est permis, et l'application vit alors dans le tray.
    let dossier = std::env::temp_dir().join("shimeji-test-liste-vide");
    let _ = std::fs::create_dir_all(&dossier);
    let chemin = dossier.join("config.json");
    let _ = std::fs::remove_file(&chemin);

    ecrire_personnages(&chemin, &[]).expect("écriture");
    let relu = charger_depuis(&chemin);
    assert!(relu.personnages.is_empty());
}

#[test]
fn ecrire_personnages_preserve_les_cles_inconnues() {
    // L'édition est CHIRURGICALE : sérialiser depuis `Config` remettrait
    // les valeurs par défaut partout, et l'utilisateur verrait son fichier
    // regle a la main ecrase par un clic dans une autre fenetre.
    let dossier = std::env::temp_dir().join("shimeji-test-cles-inconnues");
    let _ = std::fs::create_dir_all(&dossier);
    let chemin = dossier.join("config.json");

    std::fs::write(
        &chemin,
        r#"{ "personnages": ["blob"], "mon_reglage_a_moi": 42, "echelle": 2.0 }"#,
    )
    .expect("préparation");

    ecrire_personnages(&chemin, &["luffy".to_string()]).expect("écriture");

    let texte = std::fs::read_to_string(&chemin).expect("relecture");
    let v: serde_json::Value = serde_json::from_str(&texte).expect("JSON");
    assert_eq!(v["mon_reglage_a_moi"], 42);
    assert_eq!(v["echelle"], 2.0);
    assert_eq!(v["personnages"], serde_json::json!(["luffy"]));
}

#[test]
fn ecrire_personnages_n_ecrit_jamais_de_bom() {
    // `serde_json` refuse le BOM avec le message trompeur « expected value
    // at line 1 column 1 », et c'est NOUS qui relisons ce fichier.
    let dossier = std::env::temp_dir().join("shimeji-test-bom");
    let _ = std::fs::create_dir_all(&dossier);
    let chemin = dossier.join("config.json");
    let _ = std::fs::remove_file(&chemin);

    ecrire_personnages(&chemin, &["blob".to_string()]).expect("écriture");

    let octets = std::fs::read(&chemin).expect("relecture");
    assert_ne!(&octets[0..3], &[0xEF, 0xBB, 0xBF]);
}

#[test]
fn une_config_sans_cle_personnages_garde_le_defaut_blob() {
    // Inchangé : c'est le premier démarrage, pas une liste vidée à la main.
    let dossier = std::env::temp_dir().join("shimeji-test-sans-cle");
    let _ = std::fs::create_dir_all(&dossier);
    let chemin = dossier.join("config.json");
    std::fs::write(&chemin, r#"{ "echelle": 1.0 }"#).expect("préparation");

    let relu = charger_depuis(&chemin);
    assert_eq!(relu.personnages, vec!["blob".to_string()]);
}
```

- [ ] **Étape 2 : lancer les tests pour les voir échouer**

Run : `cargo test --quiet config 2>&1 | Select-Object -Last 20`
Attendu : ÉCHEC à la compilation — `cannot find function ecrire_personnages`.

- [ ] **Étape 3 : implémenter**

Dans `config.rs`, remplacer `ecrire_personnage` par :

```rust
/// Remplace la clé `personnages` de `chemin`, en laissant tout le reste.
///
/// Prend une **liste** et non un nom : `personnages` est un multi-ensemble
/// depuis l'étape « plusieurs personnages » (design §4). `["blob", "blob"]`
/// veut dire deux blob à l'écran, et les doublons doivent survivre à
/// l'aller-retour.
///
/// Édition **chirurgicale** : on relit en `serde_json::Value`, on ne touche
/// qu'à une clé, on réécrit. Sérialiser depuis `Config` perdrait toutes les
/// clés inconnues et remettrait les valeurs par défaut partout (spec §10) —
/// l'utilisateur verrait son fichier réglé à la main écrasé par un clic.
pub fn ecrire_personnages(chemin: &Path, noms: &[String]) -> Result<(), String> {
    // Un fichier absent n'est pas une erreur : c'est le cas normal au
    // premier geste, et on le crée. Un fichier présent mais ILLISIBLE, si —
    // l'écraser perdrait des réglages que l'utilisateur croit avoir.
    let mut valeur: serde_json::Value = match lire_json(chemin) {
        Ok(texte) => serde_json::from_str(&texte)
            .map_err(|e| format!("config.json illisible, rien n'est écrit : {e}"))?,
        Err(_) => serde_json::json!({}),
    };

    let Some(objet) = valeur.as_object_mut() else {
        return Err("config.json n'est pas un objet JSON".to_string());
    };
    objet.insert("personnages".to_string(), serde_json::json!(noms));

    let texte =
        serde_json::to_string_pretty(&valeur).map_err(|e| format!("sérialisation : {e}"))?;

    // `write` écrit en UTF-8 SANS BOM.
    std::fs::write(chemin, texte).map_err(|e| format!("écriture de config.json : {e}"))
}

/// Enregistre la liste des personnages dans le `config.json` réellement
/// chargé, ou en crée un dans `%APPDATA%` s'il n'y en avait aucun.
pub fn definir_personnages(noms: &[String]) -> Result<(), String> {
    match chemin_charge() {
        Some(c) => ecrire_personnages(&c, noms),
        None => {
            // Jamais dans le dépôt : celui-ci peut être en lecture seule, et
            // y écrire salirait un dossier versionné.
            let Ok(appdata) = std::env::var("APPDATA") else {
                return Err("%APPDATA% introuvable".to_string());
            };
            let dossier = PathBuf::from(appdata).join("shimeji-desktop");
            std::fs::create_dir_all(&dossier)
                .map_err(|e| format!("création de {} : {e}", dossier.display()))?;
            ecrire_personnages(&dossier.join("config.json"), noms)
        }
    }
}
```

Dans `commandes.rs`, `choisir` appelle provisoirement
`crate::config::definir_personnages(&[nom.clone()])` — elle disparaîtra à la
Tâche 8.

- [ ] **Étape 4 : lancer les tests**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 5`
Attendu : **281 passed** (276 + 5).

- [ ] **Étape 5 : commit**

```bash
git add src-tauri/src/config.rs src-tauri/src/config_tests.rs src-tauri/src/commandes.rs
git commit -m "feat(config): personnages devient un multi-ensemble

ecrire_personnage(nom) devient ecrire_personnages(&[String]) : les
doublons survivent a l'aller-retour, et la liste vide est acceptee — zero
personnage est un etat normal, pas une erreur de configuration.

L'edition reste chirurgicale et sans BOM : les cles inconnues du
config.json regle a la main ne doivent pas etre ecrasees par un clic.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Tâche 4 — `Acteur` : généraliser la boucle, en restant à N=1

**Le gros du travail, et la tâche la plus risquée.** Elle ne doit produire
**aucun changement visible** : un personnage, le même comportement. C'est
précisément ce qui la rend vérifiable — toute différence à l'écran est une
régression.

Le travail consiste à trier les ~20 variables locales de `boucle` selon le
tableau du design §3, puis à passer d'un `Character` à un `Vec<Acteur>` d'un
seul élément.

**Fichiers :**
- Modifier : `src-tauri/src/main.rs` (`struct Acteur`, signature et corps de `boucle`)
- Modifier : `src-tauri/src/tray.rs` (visibilité sur tous les acteurs)

**Interfaces :**
- Consomme : `creer_fenetre_personnage` (Tâche 1)
- Produit : `struct Acteur { label: String, nom: String, ch: character::Character, dernier_rendu: Option<render::Rendu>, derniere_taille: Option<(u32, u32)>, dernier_coin: Option<(i32, i32)>, clics_traversent: bool, derniere_trace_grimpe: Option<(String, String, String)> }`

- [ ] **Étape 1 : déclarer `Acteur`**

Dans `main.rs`, au-dessus de `boucle` :

```rust
/// Un personnage à l'écran : sa fenêtre, son état, et le peu de mémoire de
/// rendu qu'il faut pour n'appeler Windows que sur changement.
///
/// **Ce qui est ici est ce qui doit exister N fois.** Tout ce qui est
/// partagé — le monde, les signaux, le biais, le RNG, les horloges — reste
/// une variable locale de `boucle` et n'est calculé qu'UNE fois par image
/// (design §3). C'est ce partage qui fait que N personnages ne coûtent pas
/// N fois notre calcul.
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

    // ── La mémoire de rendu ─────────────────────────────────────────
    // Ces quatre champs existent pour une seule raison : n'appeler Windows
    // que quand quelque chose a changé. Le coût étant proportionnel au
    // nombre de déplacements (design §2), s'en priver multiplierait la
    // consommation par ~2.
    dernier_rendu: Option<render::Rendu>,
    derniere_taille: Option<(u32, u32)>,
    dernier_coin: Option<(i32, i32)>,
    clics_traversent: bool,

    /// Diagnostic `SHIMEJI_ESCALADE=1` : la dernière ligne tracée, pour ne
    /// tracer que les changements.
    derniere_trace_grimpe: Option<(String, String, String)>,
}
```

- [ ] **Étape 2 : changer la signature de `boucle`**

Remplacer les paramètres `label: String` et `mut ch: character::Character` par
un seul `mut acteurs: Vec<Acteur>`, et retirer `nom_personnage: String` (il vit
désormais dans chaque `Acteur`). Adapter l'appel dans `setup` : il construit un
`Vec` d'un seul `Acteur`.

- [ ] **Étape 3 : déplacer les locales par personnage dans `Acteur`**

Dans le corps de `boucle`, supprimer les déclarations de `dernier_rendu`,
`derniere_taille`, `dernier_coin`, `clics_traversent` et
`derniere_trace_grimpe`, et lire/écrire les champs de l'acteur à la place.

**Laisser où elles sont** : `monde`, `biais`, `utilisateur_actif`, `verrouille`,
`ecrans_echelle`, `echelle_affichage`, `rng`, `reglages`, `table`,
`config_courante`, les horloges et les compteurs de cadence.

- [ ] **Étape 4 : encadrer le travail par personnage d'une boucle `for`**

Scander le corps par bandeaux, selon la convention du projet :

```rust
// ── 60 Hz partagé : les entrées ─────────────────────────────────────
let m = sonde.mouse();

// ── L'élection : UN SEUL personnage sous le curseur ─────────────────
//
// Il n'y a qu'un curseur, donc au plus un personnage concerné. Sans
// cette règle, deux personnages superposés seraient attrapés ENSEMBLE
// par un même clic et se suivraient jusqu'au relâchement (design §3,
// piège n° 2).
//
// Celui qui est déjà `Dragged` garde la priorité : sinon un glisser
// rapide passant au-dessus d'un voisin transférerait la prise.
let elu: Option<usize> = elire_sous_le_curseur(&acteurs, &monde, m.pos, echelle_affichage);

// ── 60 Hz par personnage ────────────────────────────────────────────
for (i, acteur) in acteurs.iter_mut().enumerate() {
    let sur_le_personnage = elu == Some(i);

    // Viennent ici, DANS CET ORDRE et sans rien changer d'autre que
    // `ch` → `acteur.ch` et `label` → `&acteur.label`, les blocs que la
    // boucle contient déjà :
    //
    //   · « Absorber le clic, mais seulement là où il faut »
    //     (`doit_traverser` / `traverser_les_clics`)
    //   · « Clic droit sur le personnage : le menu contextuel »
    //   · la construction de `Entrees` et l'appel à `behavior::pas`
    //   · le diagnostic `SHIMEJI_ESCALADE=1`
    //   · « Sur changement seulement : la taille de la fenêtre »
    //   · le rendu, et `render::placer`
    //
    // ⚠️ **Le `continue` du menu contextuel ne peut PAS devenir un
    // `continue` de cette boucle `for`.**
    //
    // Aujourd'hui il repart sur une image neuve, et pour une raison qui
    // vaut désormais pour TOUS les acteurs : `menu_perso::ouvrir` bloque
    // plusieurs secondes, donc `maintenant` est périmé au retour, et `dt`
    // vaut toujours 16,7 ms. Poursuivre ferait juger toutes les échéances
    // — délai d'abandon, durée de pose — sur un instant faux.
    //
    // Un `continue` du `for` ne sauterait que CE personnage et laisserait
    // les autres tourner sur l'instant périmé : le bug d'origine, étendu à
    // N−1 personnages au lieu d'un.
    //
    // Poser donc un drapeau `menu_ouvert = true`, sortir du `for` par
    // `break`, et faire `continue` sur la boucle englobante juste après.
}
```

Et écrire la fonction d'élection, juste au-dessus de `boucle` :

```rust
/// Quel personnage le curseur désigne-t-il ? **Au plus un.**
///
/// Rend son index dans le `Vec`. L'ordre de départage est celui du `Vec` :
/// arbitraire, mais **stable** — le même personnage gagne tant que rien ne
/// change. Deux fenêtres toujours au premier plan n'ont de toute façon pas
/// de z-order que nous contrôlions (design §3).
fn elire_sous_le_curseur(
    acteurs: &[Acteur],
    monde: &world::World,
    souris: geom::Point,
    echelle: f32,
) -> Option<usize> {
    // Un personnage déjà porté garde la main, où que soit le curseur : un
    // glisser rapide fait sortir le sprite de sa propre hitbox, et le
    // relâcher tout seul serait le bug que `doit_traverser` évite déjà.
    for (i, a) in acteurs.iter().enumerate() {
        if matches!(a.ch.attachment, character::attach::Attachment::Dragged) {
            return Some(i);
        }
    }

    for (i, a) in acteurs.iter().enumerate() {
        // `match` sur le couple : position indérivable (plateforme
        // disparue) ou pose absente du manifeste, on ne peut pas savoir.
        // On passe au suivant plutôt que de décider à sa place.
        let (Some(pos), Some(pose)) = (
            character::attach::world_position(&a.ch.attachment, monde, souris),
            a.ch.manifest.pose(&a.ch.pose),
        ) else {
            continue;
        };

        if character::attach::hitbox_ecran(
            pos, &a.ch.pose, pose, &a.ch.manifest, echelle, a.ch.facing,
        )
        .contains(souris)
        {
            return Some(i);
        }
    }

    None
}
```

- [ ] **Étape 5 : traiter le menu contextuel et le rechargement dans la boucle `for`**

Le menu s'ouvre pour l'acteur élu, avec **son** manifeste et **son** contexte
d'accroche. La commande revient dans la boîte partagée ; comme
`menu_perso::ouvrir` **bloque**, la boucle sait encore de qui il s'agit :
appliquer la commande au seul acteur courant.

Le rechargement à chaud reste à N=1 dans cette tâche : il applique le nouveau
manifeste à **tous** les acteurs portant le nom rechargé.

- [ ] **Étape 6 : généraliser `tray.rs` et le chemin « caché »**

Le chemin « caché par l'utilisateur OU session verrouillée » doit parcourir le
`Vec`. **Oublier un acteur laisserait un personnage seul à l'écran après un
verrouillage de session — un défaut de confidentialité, pas un défaut
cosmétique** (design §9).

- [ ] **Étape 7 : compiler et lancer les tests**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 5`
Attendu : **281 passed**, inchangé — aucun test ne devait bouger.

- [ ] **Étape 8 : vérifier à l'écran que RIEN n'a changé**

Run : `cargo build 2>&1 | Select-Object -Last 10` puis `cargo run`

Vérifier, dans l'ordre :
- le personnage apparaît, marche, s'arrête, fait demi-tour ;
- on peut l'attraper à la souris et le lâcher, il tombe et atterrit ;
- le clic droit ouvre son menu et une entrée produit son effet ;
- « Cacher » puis « Afficher » dans le tray le font disparaître et revenir ;
- `characters/recharger.txt` déclenche un rechargement.

- [ ] **Étape 9 : mesurer que le tri n'a rien coûté**

Run :
```powershell
cargo build --release 2>&1 | Select-Object -Last 5
$env:SHIMEJI_CACHE="1"; .\target\release\shimeji-desktop.exe
# dans un autre terminal :
$p = Get-Process -Name shimeji-desktop
$c = $p.CPU; Start-Sleep -Seconds 60; $p.Refresh()
"$([math]::Round((($p.CPU - $c) / 60) * 100, 1)) % d'un coeur"
```
Attendu : **~0,8 à 0,9 %**, la référence du design §2. **60 secondes, jamais
10** — et le mode caché, parce que c'est la seule configuration où la charge ne
dépend pas du comportement.

- [ ] **Étape 10 : commit**

```bash
git add -A
git commit -m "refactor(boucle): un Vec<Acteur> la ou il y avait un Character

Le gros du travail de l'etape, et volontairement SANS changement visible :
un personnage, le meme comportement. Les ~20 locales de boucle sont triees
en partage (monde, biais, RNG, horloges — calcule une fois) et par
personnage (Character, memoire de rendu, traversee des clics).

Deux regles qui n'existaient pas a N=1 : un seul personnage elu sous le
curseur (sinon deux superposes seraient attrapes ensemble), et un label
issu d'un compteur monotone jamais reutilise.

Le chemin cache et le verrouillage de session parcourent le Vec : en
oublier un laisserait un personnage a l'ecran session verrouillee.

Mesure en mode cache : 0,8 % d'un coeur, la reference du design 2.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Tâche 5 — L'apparition : il tombe du haut de l'écran

**Encore à N=1**, et déjà visible : le personnage de démarrage tombe désormais
au lieu d'être posé au milieu du sol.

**Fichiers :**
- Créer : `src-tauri/src/apparition.rs`
- Créer : `src-tauri/src/apparition_tests.rs`
- Modifier : `src-tauri/src/main.rs` (`mod apparition;` et le placement initial)

**Interfaces :**
- Consomme : `world::World`, `rng::XorShift32`, `character::manifest::Manifest`
- Produit : `fn point_de_chute(monde: &world::World, largeur_sprite: f32, rng: &mut rng::XorShift32) -> Option<character::attach::Attachment>`

- [ ] **Étape 1 : écrire les tests qui échouent**

Créer `src-tauri/src/apparition_tests.rs` :

```rust
//! Les tests de `apparition` — où naît un personnage qui s'active.
use super::*;
use crate::character::attach::Attachment;
use crate::character::physics;
use crate::probe::fake::FakeProbe;
use crate::probe::SystemProbe;
use crate::rng::Rng;

/// Un monde d'un écran, construit comme le fait `setup`.
///
/// ⚠️ **Adapter le constructeur de `FakeProbe`** à celui que `world_tests.rs`
/// utilise déjà — le relire plutôt que de deviner. Ce test doit échouer
/// parce que `point_de_chute` n'existe pas, pas parce qu'un nom de sonde
/// est faux.
fn monde_d_un_ecran() -> world::World {
    let sonde = FakeProbe::default();
    world::World::from_screens(&sonde.screens())
}

#[test]
fn il_nait_en_chute_libre_et_pas_accroche() {
    // C'est TOUT le point : les réflexes à 60 Hz gèrent déjà la chute et
    // l'atterrissage, donc naître en `Falling` suffit (design §5).
    let monde = monde_d_un_ecran();
    let mut rng = rng::XorShift32::seeded(12345);
    let att = point_de_chute(&monde, 128.0, &mut rng).expect("un écran existe");
    assert!(matches!(att, Attachment::Falling { .. }));
}

#[test]
fn il_nait_sans_vitesse() {
    // Une vitesse initiale non nulle le ferait apparaître en train de
    // filer : la gravité des réflexes suffit à l'accélérer.
    let monde = monde_d_un_ecran();
    let mut rng = rng::XorShift32::seeded(999);
    let Attachment::Falling { vel, .. } =
        point_de_chute(&monde, 128.0, &mut rng).expect("un écran")
    else {
        panic!("attendu Falling");
    };
    assert_eq!(vel.x, 0.0);
    assert_eq!(vel.y, 0.0);
}

#[test]
fn le_x_reste_dans_la_zone_de_travail_marges_comprises() {
    // Sans marge il apparaîtrait à moitié hors champ sur les bords.
    //
    // ⚠️ Le RNG est semé UNE FOIS et on le laisse avancer sur les 500
    // tirages : le re-semer avec 0, 1, 2… biaiserait chaque premier tirage
    // vers les bits de poids faible, et cette boucle mesurerait 500 fois la
    // même chose (piège documenté dans CLAUDE.md).
    let monde = monde_d_un_ecran();
    let mut rng = rng::XorShift32::seeded(7);
    let sol = monde.premier_sol().expect("un sol");
    let demi = 64.0;

    for _ in 0..500 {
        let Attachment::Falling { pos, .. } =
            point_de_chute(&monde, 128.0, &mut rng).expect("un écran")
        else {
            panic!("attendu Falling");
        };
        assert!(pos.x >= sol.rect.left() + demi, "x = {} trop à gauche", pos.x);
        assert!(pos.x <= sol.rect.right() - demi, "x = {} trop à droite", pos.x);
    }
}

#[test]
fn le_x_varie_reellement_d_une_apparition_a_l_autre() {
    // « à un x aléatoire » : si la fonction rendait toujours le même point,
    // les trois tests ci-dessus passeraient quand même.
    let monde = monde_d_un_ecran();
    let mut rng = rng::XorShift32::seeded(4242);
    let mut vus = std::collections::BTreeSet::new();

    for _ in 0..50 {
        let Attachment::Falling { pos, .. } =
            point_de_chute(&monde, 128.0, &mut rng).expect("un écran")
        else {
            panic!("attendu Falling");
        };
        vus.insert(pos.x as i32);
    }
    assert!(vus.len() > 20, "seulement {} x distincts sur 50", vus.len());
}

#[test]
fn il_ne_s_agrippe_pas_au_plafond_en_apparaissant() {
    // ⚠️ LE VRAI RISQUE de cette tâche (design §5).
    //
    // La plateforme « plafond » est posée JUSTE au-dessus de la zone de
    // travail et expose sa face `Bottom`, celle à laquelle on se suspend.
    // Un personnage qui apparaît au sommet naît donc à quelques pixels
    // d'une surface accrochable. S'il s'y agrippait, tous les personnages
    // apparaîtraient collés au plafond au lieu de tomber.
    //
    // ⚠️ `contact` prend **deux positions** — l'avant et l'après d'une
    // image — et non une position et une vitesse : c'est un test de
    // trajectoire balayée, pas de point. On lui pose donc exactement la
    // question de la première image de chute.
    let monde = monde_d_un_ecran();
    let mut rng = rng::XorShift32::seeded(31337);

    // 200 apparitions, parce qu'un seul tirage tomberait peut-être sur un
    // x qu'aucun plafond ne couvre.
    for _ in 0..200 {
        let Attachment::Falling { pos, .. } =
            point_de_chute(&monde, 128.0, &mut rng).expect("un écran")
        else {
            panic!("attendu Falling");
        };

        // La première image de chute : la gravité sur 1/60 s.
        let apres = geom::Point::new(pos.x, pos.y + physics::GRAVITE / 60.0 / 60.0);
        assert!(
            physics::contact(&monde, pos, apres).is_none(),
            "il s'accroche dès l'apparition en {pos:?} au lieu de tomber"
        );
    }
}

#[test]
fn un_monde_sans_ecran_rend_none_sans_paniquer() {
    // Cas réel : session distante en cours d'établissement.
    let monde = world::World::from_screens(&[]);
    let mut rng = rng::XorShift32::seeded(1);
    assert!(point_de_chute(&monde, 128.0, &mut rng).is_none());
}
```

- [ ] **Étape 2 : lancer les tests pour les voir échouer**

Run : `cargo test --quiet apparition 2>&1 | Select-Object -Last 20`
Attendu : ÉCHEC — `cannot find function point_de_chute`.

- [ ] **Étape 3 : implémenter**

Créer `src-tauri/src/apparition.rs` :

```rust
//! Où naît un personnage qui vient d'être activé (design §5).
//!
//! Responsabilité unique, et **fonction pure** : elle rend un `Attachment`,
//! et c'est tout. Rien de l'animation de chute n'est écrit ici — les
//! réflexes à 60 Hz gèrent déjà chute et atterrissage depuis l'étape 4a.
//! **C'est la décision n° 1 qui rend cette tâche presque vide.**

use crate::character::attach::Attachment;
use crate::geom::{Face, Point, Vec2};
use crate::rng::{Rng, XorShift32};
use crate::world::{Platform, PlatformKind, World};

/// Le point d'où tombe un personnage qui apparaît.
///
/// Rend `None` quand il n'y a aucun écran — cas réel d'une session distante
/// en cours d'établissement, et non une erreur.
///
/// ⚠️ **Le `rng` est emprunté, jamais recréé.** Semer un `XorShift32` par
/// personnage avec son index biaiserait le premier tirage de chacun vers
/// les bits de poids faible : tous apparaîtraient au même endroit, et une
/// mesure qui chercherait à le démentir tomberait dans le même piège (voir
/// « Semer l'aléatoire une seule fois » dans CLAUDE.md).
pub fn point_de_chute(
    monde: &World,
    largeur_sprite: f32,
    rng: &mut XorShift32,
) -> Option<Attachment> {
    // On part du SOL d'un écran plutôt que de l'écran lui-même : c'est le
    // rectangle qui porte déjà la zone de travail, donc celui qui sait où
    // s'arrête la barre des tâches (piège Windows n° 3).
    //
    // `matches!` et non `==` : `PlatformKind` ne dérive pas `PartialEq`.
    let sols: Vec<&Platform> = monde
        .platforms()
        .iter()
        .filter(|p| matches!(p.kind, PlatformKind::Screen) && p.has_face(Face::Top))
        .collect();

    // `let … else` : aucun écran, rien à faire. Équivalent d'un `match`
    // dont la branche vide ferait `return None`.
    let Some(sol) = tirer_un(&sols, rng) else {
        return None;
    };

    // Le haut de la zone de travail se lit sur le PLAFOND du même écran :
    // `from_screens` le pose en `z.top() - EPAISSEUR`, donc son
    // `rect.bottom()` vaut exactement `z.top()`.
    //
    // Naître exactement là, et non au-dessus, est ce qui évite qu'il
    // s'agrippe à la face `Bottom` du plafond au lieu de tomber — c'est le
    // point que le test `il_ne_s_agrippe_pas_au_plafond_en_apparaissant`
    // vérifie, et le seul vrai risque de ce module.
    let plafond = monde
        .platforms()
        .iter()
        .find(|p| p.id.meme_ecran(sol.id) && p.has_face(Face::Bottom))?;
    let y = plafond.rect.bottom();

    // Une demi-largeur de marge de chaque côté : sans elle, il
    // apparaîtrait à moitié hors champ sur les bords.
    let demi = largeur_sprite / 2.0;
    let gauche = sol.rect.left() + demi;
    let droite = sol.rect.right() - demi;

    // Un écran plus étroit que le sprite : on le pose au milieu plutôt que
    // de tirer dans un intervalle vide. `range` rend déjà le minimum quand
    // les bornes sont inversées, mais l'écrire ici dit POURQUOI c'est le
    // milieu et non le bord gauche.
    let x = if droite <= gauche {
        (sol.rect.left() + sol.rect.right()) / 2.0
    } else {
        rng.range(gauche, droite)
    };

    Some(Attachment::Falling {
        pos: Point::new(x, y),
        // Vitesse nulle : la gravité des réflexes suffit à l'accélérer.
        // Une vitesse initiale le ferait apparaître en train de filer.
        vel: Vec2::new(0.0, 0.0),
    })
}

/// Un élément au hasard dans une tranche, ou `None` si elle est vide.
///
/// `range` rend un `f32` dans `[min, max]` **bornes comprises** : le `min`
/// avec `len - 1` plutôt qu'avec `len` éviterait un index hors bornes une
/// fois sur des milliards, mais `min` le rend impossible tout court — et
/// c'est le genre de bug qu'on ne reproduit jamais.
fn tirer_un<'a>(elements: &[&'a Platform], rng: &mut XorShift32) -> Option<&'a Platform> {
    if elements.is_empty() {
        return None;
    }
    let i = (rng.range(0.0, elements.len() as f32) as usize).min(elements.len() - 1);
    Some(elements[i])
}

// Les tests de ce module vivent dans `apparition_tests.rs`.
#[cfg(test)]
#[path = "apparition_tests.rs"]
mod tests;
```

> **Pourquoi ce module n'ajoute rien à `world.rs`** : il n'utilise que
> `platforms()`, `has_face`, `meme_ecran` et `premier_sol`, qui existent tous.
> Ajouter un `World::sols()` serait plus joli mais élargirait la surface d'un
> module que cette étape n'a aucune raison de toucher — et le design §11 dit
> explicitement que `world.rs` ne change pas.

- [ ] **Étape 4 : lancer les tests**

Run : `cargo test --quiet apparition 2>&1 | Select-Object -Last 20`
Attendu : **6 passed**.

> **Si `il_ne_s_agrippe_pas_au_plafond_en_apparaissant` échoue**, ne pas
> déplacer le point d'apparition « un peu plus bas » jusqu'à ce que ça passe.
> Lire `character::physics::contact` et comprendre **pourquoi** la face
> `Bottom` du plafond répond. La correction est alors dans la condition de
> contact, pas dans une constante ajustée à l'œil — c'est très exactement
> l'erreur que l'étape 1a a commise quatre fois.

- [ ] **Étape 5 : brancher l'apparition au démarrage**

Dans `setup`, remplacer le placement « au milieu du premier sol » par un appel à
`apparition::point_de_chute`. Le RNG de la boucle n'existe pas encore à cet
instant : en créer un semé par l'horloge, **une seule fois**, et le passer à la
boucle pour qu'elle continue de l'avancer.

- [ ] **Étape 6 : vérifier à l'écran**

Run : `cargo build 2>&1 | Select-Object -Last 10` puis `cargo run`
Attendu : le personnage **tombe du haut de l'écran** à un `x` qui change à
chaque lancement, atterrit, puis se comporte normalement.

Relancer trois fois pour voir trois `x` différents.

- [ ] **Étape 7 : commit**

```bash
git add src-tauri/src/apparition.rs src-tauri/src/apparition_tests.rs src-tauri/src/main.rs
git commit -m "feat(apparition): il tombe du haut de l'ecran, a un x aleatoire

Presque vide, et c'est la decision n 1 qui le permet : Attachment::Falling
existe deja et les reflexes a 60 Hz gerent chute et atterrissage depuis
l'etape 4a. La fonction ne fait que choisir un ecran et un x.

Le test qui comptait : PROUVER qu'il ne s'agrippe pas au plafond en
apparaissant. La plateforme plafond est posee juste au-dessus de la zone
de travail et expose sa face Bottom ; sans ce test, tous les personnages
seraient apparus colles au plafond.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Tâche 6 — Le roster vivant : plusieurs personnages à l'écran

**Le point n° 1 du brief est atteint à la fin de cette tâche.**

**Fichiers :**
- Modifier : `src-tauri/src/rechargement.rs` (la demande porte un roster)
- Modifier : `src-tauri/src/actions.rs` (le roster partagé remplace `perso`)
- Modifier : `src-tauri/src/main.rs` (la réconciliation dans la boucle, `SHIMEJI_PERSONNAGES`)

**Interfaces :**
- Consomme : `roster::reconcilier`, `roster::compte_de` (Tâche 2) ; `apparition::point_de_chute` (Tâche 5) ; `creer_fenetre_personnage` (Tâche 1)
- Produit :
  - `struct PersonnageCharge { nom: String, dossier: PathBuf, manifeste: Manifest }`
  - `struct Rechargement { personnages: Vec<PersonnageCharge>, voulus: Vec<String>, reglages, table, echelle_config, config, version, sans_animation: bool }`
  - `fn rechargement::preparer_roster(demande: &Demande, voulus: &[String]) -> Result<u64, String>`
  - `fn Actions::definir_roster(&self, voulus: &[String]) -> Result<u64, String>`
  - `fn Actions::roster(&self) -> Vec<String>`

- [ ] **Étape 1 : faire porter un roster à la demande de rechargement**

Dans `rechargement.rs`, remplacer le champ `manifeste: Manifest` par
`personnages: Vec<PersonnageCharge>` et ajouter `voulus: Vec<String>`.

`preparer_roster` lit **tous** les manifestes nécessaires (un par nom distinct)
**avant** de prendre le verrou — la discipline actuelle du fichier :

```rust
/// Un pack chargé, prêt à être instancié autant de fois qu'il le faut.
///
/// `Manifest` dérive `Clone` : chaque acteur possède sa copie. Quelques Ko
/// par personnage, ce qui ne justifie pas d'introduire un `Arc` et l'emprunt
/// partagé qui va avec (design §4).
pub struct PersonnageCharge {
    pub nom: String,
    pub dossier: PathBuf,
    pub manifeste: Manifest,
}

/// Lit ce qu'il faut pour afficher exactement `voulus`, et le dépose.
///
/// ⚠️ **Les entrées-sorties D'ABORD, verrou non tenu.** Un manifeste
/// illisible fait sortir ici sans rien avoir touché, et les personnages
/// continuent avec ce qu'ils avaient. Tenir le verrou pendant une lecture
/// de fichier bloquerait en plus la boucle 60 Hz pour rien.
///
/// Un nom **introuvable** est signalé bruyamment et **ignoré** : les autres
/// personnages doivent vivre. Une liste entièrement vide est un succès —
/// zéro personnage est un état normal (design §4).
pub fn preparer_roster(demande: &Demande, voulus: &[String]) -> Result<u64, String> {
    // ── Les entrées-sorties D'ABORD, verrou non tenu ────────────────
    //
    // Un `BTreeSet` : on ne lit le manifeste d'un pack QU'UNE FOIS, même
    // si trois exemplaires en sont voulus — et l'ordre est stable d'un
    // appel à l'autre, contrairement à un `HashSet`.
    let distincts: std::collections::BTreeSet<&String> = voulus.iter().collect();

    let mut personnages = Vec::new();
    for nom in distincts {
        // Un nom introuvable est signalé et IGNORÉ : les autres
        // personnages doivent vivre. Échouer ici sur un seul pack effacé
        // à la main rendrait toute la bibliothèque inutilisable.
        let Some(dossier) = crate::config::dossier_du_personnage(nom) else {
            eprintln!("personnage « {nom} » introuvable : ignoré");
            continue;
        };
        match Manifest::load(&dossier) {
            Ok(manifeste) => personnages.push(PersonnageCharge {
                nom: nom.clone(),
                dossier,
                manifeste,
            }),
            Err(e) => eprintln!("personnage « {nom} » illisible, ignoré : {e}"),
        }
    }

    // Ne garder que les noms réellement chargés : sinon la réconciliation
    // demanderait sans fin la création d'un personnage qu'elle ne peut
    // pas créer, à chaque passage à 8 Hz.
    let voulus: Vec<String> = voulus
        .iter()
        .filter(|n| personnages.iter().any(|p| &p.nom == *n))
        .cloned()
        .collect();

    let config = crate::config::charger();
    let reglages = Reglages::depuis(&config);
    let table = crate::behavior::desire::TableEnvies::depuis_config(&config);

    // ── Puis on pose, brièvement ────────────────────────────────────
    let mut boite = demande
        .lock()
        .map_err(|_| "verrou de rechargement empoisonné".to_string())?;

    // ⚠️ Un compteur MONOTONE, et non la version de la demande en attente
    // — garder EXACTEMENT le mécanisme actuel. La boucle vide la boîte par
    // `take()` : une version déduite du contenu repartirait de 1 à chaque
    // fois, et `pet.js`, qui indexe son cache d'images par `version/frame`,
    // servirait les frames de l'ANCIEN personnage pour toute pose déjà vue.
    let version = PROCHAINE_VERSION.fetch_add(1, Ordering::Relaxed) + 1;

    *boite = Some(Rechargement {
        personnages,
        voulus,
        reglages,
        table,
        echelle_config: config.echelle,
        config,
        version,
        sans_animation: false,
    });

    Ok(version)
}
```

> Le champ `sans_animation` n'est utilisé qu'à la Tâche 9 (la suppression) ;
> le déclarer dès maintenant évite de retoucher la structure et ses deux
> appelants plus tard. Une variante `preparer_roster_immediat` le posera à
> `true` — **le même corps, un booléen de plus**, jamais un second chemin.

- [ ] **Étape 2 : remplacer `Actions::perso` par le roster partagé**

Dans `actions.rs`, `perso: Mutex<(String, PathBuf)>` devient
`roster: Mutex<Vec<String>>`. `changer_personnage` devient :

```rust
/// Remplace la liste des personnages voulus et demande le chargement.
///
/// Rend la version du rechargement, que la boucle 60 Hz comparera à la
/// sienne pour savoir qu'il y a du nouveau.
pub fn definir_roster(&self, voulus: &[String]) -> Result<u64, String> {
    // Les entrées-sorties d'abord, verrou non tenu (voir `preparer_roster`).
    let version = crate::rechargement::preparer_roster(&self.demande, voulus)?;

    match self.roster.lock() {
        Ok(mut r) => {
            *r = voulus.to_vec();
            Ok(version)
        }
        Err(_) => Err("verrou du roster empoisonné".to_string()),
    }
}

/// La liste voulue en ce moment. Rend une liste vide si le verrou est
/// empoisonné : l'appelant affichera une bibliothèque vide plutôt que de
/// paniquer dans un gestionnaire de menu, ce qui tuerait le thread
/// d'interface.
pub fn roster(&self) -> Vec<String> {
    self.roster.lock().map(|r| r.clone()).unwrap_or_default()
}
```

- [ ] **Étape 3 : réconcilier dans la boucle**

Dans la section « Une demande de rechargement en attente ? » (à ~8 Hz), après
avoir appliqué manifestes et réglages, ajouter la réconciliation :

```rust
// ── Réconcilier les acteurs présents avec la liste voulue ───────
//
// Le MÊME chemin sert au changement depuis la bibliothèque et au
// rechargement à chaud par le fichier témoin : relire, recharger,
// réconcilier. Un seul chemin de code, donc pas de second chemin qui
// diverge à la première correction (design §4).
let presents: Vec<String> = acteurs.iter().map(|a| a.nom.clone()).collect();

for action in roster::reconcilier(&presents, &r.voulus) {
    match action {
        roster::ActionRoster::Creer(nom) => {
            // Le manifeste a été lu par le thread de la commande : il
            // n'y a AUCUNE entrée-sortie ici, à 60 Hz.
            let Some(charge) = r.personnages.iter().find(|p| p.nom == nom) else {
                eprintln!("« {nom} » voulu mais non chargé : ignoré");
                continue;
            };

            // ⚠️ Compteur MONOTONE, jamais l'index dans le Vec : un
            // label réutilisé se heurterait à une fenêtre que Windows
            // détruit encore (design §3, piège n° 3).
            let label = format!("pet-{prochain_label}");
            prochain_label += 1;

            let taille = character::attach::window_size(
                &charge.manifeste,
                echelle_affichage,
            );
            // Le point d'apparition AVANT la fenêtre : s'il n'y a aucun
            // écran, on n'a pas créé de fenêtre à détruire.
            let Some(att) = apparition::point_de_chute(
                &monde,
                taille.0 as f32,
                &mut rng,
            ) else {
                eprintln!("aucun écran : « {nom} » n'apparaît pas");
                continue;
            };

            match creer_fenetre_personnage(&handle, &label, &nom, taille) {
                Ok(_) => {
                    // `pos_connue` reçoit le point de chute : c'est le
                    // champ qu'amorce une chute qui commence (voir son
                    // commentaire dans `character/mod.rs`).
                    let depart_pos = match att {
                        character::attach::Attachment::Falling { pos, .. } => pos,
                        // Inatteignable — `point_de_chute` ne rend que
                        // `Falling`. On préfère quand même un repli lisible
                        // à un `unreachable!()` qui tuerait la boucle si le
                        // module changeait un jour.
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
                        // `true` : la fenêtre naît avec les clics
                        // traversants, posés par `creer_fenetre_personnage`.
                        // Mentir ici ferait sauter le premier appel de
                        // `traverser_les_clics`, et le personnage serait
                        // incliquable jusqu'au prochain changement d'état.
                        clics_traversent: true,
                        derniere_trace_grimpe: None,
                        depart: None,
                    });
                    println!("« {nom} » apparaît ({label})");
                }
                // Bruyant : une création silencieusement ratée donnerait
                // un compteur à 3 pour 2 personnages à l'écran.
                Err(e) => eprintln!("« {nom} » n'a pas pu apparaître : {e}"),
            }
        }

        roster::ActionRoster::RetirerUn(nom) => {
            // `rposition` : le plus RÉCEMMENT ajouté part le premier,
            // ce qu'attend quelqu'un qui vient de cliquer « + » puis
            // « − » (design §4).
            let Some(i) = acteurs.iter().rposition(|a| a.nom == nom) else {
                continue;
            };
            let parti = acteurs.remove(i);
            if let Some(w) = handle.get_webview_window(&parti.label) {
                let _ = w.destroy();
            }
            println!("« {nom} » s'en va ({})", parti.label);
        }
    }
}
```

- [ ] **Étape 4 : l'équivalent scriptable**

Remplacer `SHIMEJI_CHANGER` par `SHIMEJI_PERSONNAGES` :

```rust
// `SHIMEJI_PERSONNAGES=blob,blob,luffy` installe un roster de départ —
// l'équivalent scriptable des clics dans « Ma bibliothèque », selon la
// règle du projet : tout ce qui demanderait un clic en reçoit un.
//
// Combinée à `SHIMEJI_TRACE=1`, elle dit quel personnage est réellement
// servi, image par image.
if let Ok(liste) = std::env::var("SHIMEJI_PERSONNAGES") {
    let voulus: Vec<String> = liste
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Les doublons sont CONSERVÉS : `blob,blob` veut dire deux blob, et
    // c'est précisément ce que cette variable sert à rejouer.
    match actions.definir_roster(&voulus) {
        Ok(v) => println!("[diag] roster de départ {voulus:?} -> version {v}"),
        // Bruyant, et non fatal : on veut voir POURQUOI le roster demandé
        // n'a pas pris, pas démarrer sur un écran vide sans explication.
        Err(e) => eprintln!("[diag] roster de départ refusé : {e}"),
    }
}
```

- [ ] **Étape 5 : traiter la liste vide au démarrage**

Dans `lancer_application`, retirer le `std::process::exit(1)` quand le
personnage est introuvable. Nouvelle règle :

- liste **vide** → on démarre, aucun personnage, aucun message d'erreur ;
- nom **introuvable** dans une liste non vide → message bruyant, ce nom est
  ignoré, les autres vivent.

- [ ] **Étape 6 : lancer les tests**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 5`
Attendu : **287 passed** au minimum (281 + 6 de la Tâche 5).

- [ ] **Étape 7 : vérifier à l'écran — LE moment de l'étape**

```powershell
cargo build 2>&1 | Select-Object -Last 10
$env:SHIMEJI_PERSONNAGES="blob,blob,blob"; cargo run
```

Attendu : **trois blob tombent du haut de l'écran**, à trois `x` différents,
atterrissent et vivent chacun leur vie.

Vérifier ensuite, et c'est ce qui prouve l'élection de la Tâche 4 :
- attraper l'un des trois à la souris → **lui seul** est attrapé ;
- clic droit sur l'un → le menu s'ouvre pour **celui-là** ;
- « Cacher » dans le tray → **les trois** disparaissent.

- [ ] **Étape 8 : commit**

```bash
git add -A
git commit -m "feat(roster): plusieurs personnages vivent a l'ecran

Le point n 1 du brief. La demande de rechargement porte desormais un
roster ; la boucle reconcilie a 8 Hz les acteurs presents avec la liste
voulue, cree les manquants — qui apparaissent en tombant — et detruit les
fenetres des partants.

Le meme chemin sert au changement depuis la bibliotheque et au
rechargement a chaud : un seul chemin de code, pas deux qui divergeront.

Aucune entree-sortie a 60 Hz : les manifestes sont lus par le thread de la
commande avant la prise du verrou.

SHIMEJI_PERSONNAGES=blob,blob,luffy remplace SHIMEJI_CHANGER.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Tâche 7 — Le départ : il se ramasse, il saute, il tombe

**Fichiers :**
- Modifier : `src-tauri/src/main.rs` (`struct Depart`, le court-circuit dans la boucle)

**Interfaces :**
- Produit : `enum PhaseDepart { SeRamasse, Saute, Tombe }`, `struct Depart { phase: PhaseDepart, depuis: Duration, vel: Vec2, pos: Point }`, champ `depart: Option<Depart>` sur `Acteur`

- [ ] **Étape 1 : déclarer les types du départ**

```rust
/// Les trois temps d'un départ (design §6).
///
/// ⚠️ **Il n'existe AUCUNE frame « plier les jambes » dans le vocabulaire
/// Shimeji** — vérifié dans `docs/specs/2026-09-09-frames-shimeji.md`, qui
/// est tiré des sources de Shimeji-ee. Le standard n'a que `jump`, la frame
/// 22, une seule image. Ces trois temps composent la lecture cherchée avec
/// ce qui existe réellement.
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
/// Sa présence **court-circuite `behavior::pas`**, et c'est délibéré : le
/// réflexe d'atterrissage est non négociable par définition (spec §7.1,
/// couche 1). Y introduire un « sauf si je pars » le rendrait négociable,
/// et ce serait le premier pas vers un réflexe plein de cas particuliers.
///
/// Un acteur en départ n'est plus un personnage vivant — il quitte la
/// scène, et la scène n'a plus son mot à dire.
struct Depart {
    phase: PhaseDepart,
    /// Le temps de l'horloge injectée au début de la **phase courante**.
    depuis: Duration,
    /// Le début du départ **entier**. Distinct de `depuis`, et c'est
    /// nécessaire : le garde-fou des 3 s porte sur le départ complet, pas
    /// sur sa dernière phase. Les confondre rendrait le garde-fou
    /// inopérant, puisqu'il repartirait de zéro à chaque changement de pose.
    debut: Duration,
    pos: Point,
    vel: Vec2,
}
```

- [ ] **Étape 2 : écrire le court-circuit dans la boucle**

Tout au début du traitement d'un acteur, avant l'élection et avant
`behavior::pas` :

```rust
// ── Un acteur en départ ne vit plus : il tombe, et c'est tout ────
if let Some(d) = &mut acteur.depart {
    let fini = avancer_le_depart(d, &mut acteur.ch, &monde, maintenant, dt);
    if fini {
        a_retirer.push(i);
    }
    continue;   // ni comportement, ni hit-testing, ni menu
}
```

Et la fonction, au-dessus de `boucle` :

```rust
/// Fait avancer un départ d'une image. Rend `true` quand l'acteur doit
/// être retiré.
///
/// **La couverture partielle s'applique toute seule** : un pack sans `sit`
/// ou sans `jump` saute simplement la phase correspondante. Aucun cas
/// particulier à coder — c'est la règle §8.6, qui retire du jeu ce qui
/// n'est pas dessiné.
fn avancer_le_depart(
    d: &mut Depart,
    ch: &mut character::Character,
    monde: &world::World,
    maintenant: Duration,
    dt: f32,
) -> bool {
    // Le garde-fou de durée, testé en PREMIER : un acteur qui ne partirait
    // jamais ferait fuir une fenêtre à chaque désactivation, et c'est le
    // genre de fuite qu'on ne voit qu'après une heure d'usage.
    if maintenant.saturating_sub(d.debut) > DUREE_MAX_DEPART {
        return true;
    }

    match d.phase {
        PhaseDepart::SeRamasse => {
            // `set_pose` ignore une pose absente du manifeste : un pack sans
            // `sit` reste simplement dans sa pose courante pendant 120 ms.
            // C'est la couverture partielle (spec §8.6) obtenue sans écrire
            // un seul cas particulier.
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
                // se lit comme « il glisse ». La valeur est un point de
                // départ à régler à l'œil, comme les deux durées.
                d.vel = Vec2::new(0.0, -IMPULSION_DEPART);
            }
        }

        PhaseDepart::Tombe => {
            ch.set_pose("fall", maintenant);

            // La même gravité que la physique normale, mais **sans son test
            // de contact** : c'est très exactement ce que « il traverse le
            // sol » veut dire, et la raison d'être de ce court-circuit.
            d.vel.y += character::physics::GRAVITE * dt;
            d.pos.y += d.vel.y * dt;

            // Le garde-fou de sortie d'écran. `bounds()` couvre TOUS les
            // écrans : sur deux moniteurs de hauteurs différentes, prendre
            // le seul écran de départ retirerait le personnage trop tôt sur
            // l'un des deux.
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
    // `ch.pos_connue` est ce que la boucle lit pour placer la fenêtre.
    ch.pos_connue = d.pos;
    false
}
```

Et les cinq constantes, juste au-dessus :

```rust
/// Les durées des deux premières phases du départ.
///
/// **Points de départ à régler à l'œil**, comme toutes les constantes
/// d'animation de ce projet — et comme elles, à chercher d'abord dans les
/// sources de Shimeji-ee avant d'en inventer une (leçon transverse de
/// l'étape 1a, où les quatre réglages faits à l'œil étaient faux).
const DUREE_SE_RAMASSE: Duration = Duration::from_millis(120);
const DUREE_SAUTE: Duration = Duration::from_millis(150);

/// La vitesse initiale vers le haut, en px/s. Sans elle, « il saute » se lit
/// comme « il glisse ».
const IMPULSION_DEPART: f32 = 350.0;

/// De combien il faut dépasser le bas du bureau pour être hors de vue. Une
/// hauteur de fenêtre suffit largement.
const MARGE_SORTIE: f32 = 200.0;

/// Au-delà, l'acteur est retiré quoi qu'il arrive.
const DUREE_MAX_DEPART: Duration = Duration::from_secs(3);
```

- [ ] **Étape 3 : les deux garde-fous**

1. **La sortie d'écran** : l'acteur est retiré dès que son `y` dépasse le bas de
   son écran d'une hauteur de sprite.
2. **La durée** : au-delà de **3 s**, il est retiré de toute façon. Un acteur
   qui ne partirait jamais ferait fuir une fenêtre à chaque désactivation.

- [ ] **Étape 4 : brancher le départ sur `RetirerUn`**

Dans la réconciliation (Tâche 6), `RetirerUn` ne détruit plus la fenêtre :
il pose `acteur.depart = Some(Depart { … })`. La destruction a lieu quand
`avancer_le_depart` rend `true`.

⚠️ Un acteur déjà en départ **ne compte plus comme présent** dans
`reconcilier` : sinon réactiver un personnage pendant que le précédent tombe
n'en créerait pas de nouveau.

- [ ] **Étape 5 : l'équivalent scriptable du départ**

```rust
// `SHIMEJI_ROSTER=8:blob,luffy` joue un changement de roster après 8 s —
// le clic dans la bibliothèque, y compris une DÉSACTIVATION, donc le
// départ observable sans humain.
```

- [ ] **Étape 6 : lancer les tests**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 5`
Attendu : inchangé, aucune régression.

- [ ] **Étape 7 : vérifier à l'œil — la seule vérification qui demande un humain**

```powershell
cargo build 2>&1 | Select-Object -Last 10
$env:SHIMEJI_PERSONNAGES="blob,blob"; $env:SHIMEJI_ROSTER="10:blob"; cargo run
```

Attendu : au bout de 10 s, **l'un des deux se ramasse, saute et tombe hors de
l'écran**, puis sa fenêtre disparaît. L'autre continue de vivre.

> **Que `sit → jump → fall` se lise comme « il se ramasse et il saute » est un
> jugement esthétique : aucune assertion ne le rend.** Les durées (120 ms /
> 150 ms) sont des points de départ à régler à l'œil. Si le rendu ne convient
> pas, ce sont ces deux constantes qu'on ajuste — et le rechargement à chaud du
> manifeste est fait pour ça.

- [ ] **Étape 8 : commit**

```bash
git add -A
git commit -m "feat(depart): il se ramasse, il saute, il tombe hors de l'ecran

sit ~120 ms puis jump ~150 ms puis fall. Il n'existe AUCUNE frame « plier
les jambes » dans le vocabulaire Shimeji — verifie dans le releve des 46
poses — donc ces trois temps composent la lecture cherchee avec ce qui
existe.

Le depart court-circuite behavior::pas plutot que d'ajouter une exception
au reflexe d'atterrissage : celui-ci est non negociable par definition, et
y mettre un « sauf si je pars » serait le premier pas vers un reflexe plein
de cas particuliers.

Deux garde-fous : sortie d'ecran, et 3 s maximum — un acteur qui ne
partirait jamais ferait fuir une fenetre a chaque desactivation.

Un pack sans sit ni jump tombe directement : couverture partielle, aucun
cas particulier a coder.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Tâche 8 — `definir_compte` et la carte de bibliothèque

**Le point n° 2 du brief est atteint à la fin de cette tâche.**

**Fichiers :**
- Modifier : `src-tauri/src/commandes.rs` (`definir_compte` remplace `choisir` ; `PackInstalle` enrichi)
- Modifier : `src-tauri/src/main.rs` (`generate_handler!`)
- Modifier : `ui/catalogue.js`, `ui/catalogue.html`

**Interfaces :**
- Consomme : `roster::compte_de` (Tâche 2), `Actions::definir_roster` / `Actions::roster` (Tâche 6), `config::definir_personnages` (Tâche 3)
- Produit :
  - `struct PackInstalle { nom: String, compte: usize, supprimable: bool }`
  - `#[tauri::command] fn definir_compte(actions, nom: String, combien: usize) -> Result<(), String>`

- [ ] **Étape 1 : enrichir `PackInstalle` et `bibliotheque`**

```rust
/// Un pack présent sur le disque, tel que la fenêtre a besoin de le
/// connaître.
#[derive(Serialize)]
pub struct PackInstalle {
    pub nom: String,
    /// Combien d'exemplaires vivent à l'écran. **0 = éteint.**
    ///
    /// Le champ `actif: bool` d'avant a disparu : `compte > 0` le dit, et
    /// deux champs qui disent la même chose finissent par se contredire.
    pub compte: usize,
    /// La poubelle est-elle active ? Voir `supprimer` (Tâche 9).
    pub supprimable: bool,
}
```

`bibliotheque()` lit le compte via `roster::compte_de` sur la liste courante.

- [ ] **Étape 2 : écrire `definir_compte`**

```rust
/// Combien d'exemplaires de ce personnage doivent vivre à l'écran.
///
/// **Une seule commande pour les quatre gestes** de la bibliothèque
/// (design §4) : le clic sur la carte appelle `n + 1`, le bouton `−`
/// appelle `n − 1`, l'interrupteur appelle `0` ou `1`. L'interrupteur
/// n'est que le reflet de `compte > 0` — aucun état caché, donc rien qui
/// puisse se désynchroniser.
///
/// Le changement est **immédiat**, sans redémarrage : le roster déposé est
/// ramassé par la boucle 60 Hz à l'image suivante.
#[tauri::command]
pub fn definir_compte(
    actions: tauri::State<'_, std::sync::Arc<crate::actions::Actions>>,
    nom: String,
    combien: usize,
) -> Result<(), String> {
    // On refuse un personnage introuvable AVANT tout le reste : sinon
    // l'application ne redémarrerait plus, le chargement échouant sur un
    // nom qui ne résout pas.
    if combien > 0 && crate::config::dossier_du_personnage(&nom).is_none() {
        return Err(format!("personnage « {nom} » introuvable"));
    }

    // Reconstruire la liste : on retire toutes les occurrences du nom, puis
    // on en remet `combien`. L'ordre des autres est préservé — c'est un
    // multi-ensemble, mais un `config.json` relu par un humain gagne à ne
    // pas voir ses lignes danser à chaque clic.
    let mut voulus: Vec<String> = actions.roster().into_iter().filter(|n| n != &nom).collect();
    for _ in 0..combien {
        voulus.push(nom.clone());
    }

    // L'ordre compte : on CHARGE d'abord, on ENREGISTRE ensuite. Si un
    // manifeste est illisible, rien n'a changé — ni à l'écran, ni dans la
    // config, qui aurait sinon nommé un personnage qui ne charge pas.
    actions.definir_roster(&voulus)?;
    crate::config::definir_personnages(&voulus)?;

    println!("roster : {voulus:?}");
    Ok(())
}
```

Supprimer `choisir`, et remplacer son entrée dans `generate_handler!` de
`main.rs`.

- [ ] **Étape 3 : la carte de bibliothèque**

Dans `ui/catalogue.js`, remplacer `carteBibliotheque`. La carte porte
vignette, nom, `− n +`, interrupteur, et (Tâche 9) la poubelle.

```js
// ── La bibliothèque : le SECOND geste, devenu un compteur ──────────────
//
// Une seule commande pour quatre gestes (design §4) : le fond de la carte
// ajoute, « − » retire, l'interrupteur met à 0 ou à 1. L'interrupteur
// n'est que le reflet de `compte > 0` : éteindre trois blob puis rallumer
// en ramène UN, et il n'y a aucun compte « en sommeil » à tenir d'accord
// avec la liste.
function carteBibliotheque(pack) { … }
```

Le compteur n'est affiché que si `compte > 0` : une bibliothèque de 40 packs
dont 2 sont actifs ne doit pas être un mur de zéros.

- [ ] **Étape 4 : le CSS**

Dans `ui/catalogue.html`, ajouter les styles de l'interrupteur et du compteur
**dans la palette existante** — fond `rgb(36,36,36)`, texte `rgb(201,201,201)`.
Ces valeurs ont été **mesurées** dans le DOM de shimejis.xyz : ne pas
introduire une seconde palette.

Les boutons `−` et `+` appellent `event.stopPropagation()` : sans quoi le clic
remonterait au fond de la carte et ajouterait **aussi** un exemplaire.

- [ ] **Étape 5 : vérifier à l'écran**

```powershell
cargo build 2>&1 | Select-Object -Last 10
$env:SHIMEJI_CATALOGUE="1"; cargo run
```

Dans « Ma bibliothèque » :
- cliquer trois fois sur la carte de `blob` → **trois blob tombent**, le
  compteur affiche 3 ;
- cliquer `−` → **le plus récent** s'en va, le compteur affiche 2 ;
- éteindre l'interrupteur → **les deux** s'en vont, le compteur disparaît ;
- rallumer → **un seul** revient ;
- fermer et relancer l'application → le roster est **celui qu'on avait laissé**.

- [ ] **Étape 6 : commit**

```bash
git add -A
git commit -m "feat(bibliotheque): un compteur et un interrupteur par personnage

Le point n 2 du brief. « Choisir » ne remplace plus : il ajoute et retire.
Une seule commande, definir_compte(nom, combien), sert les quatre gestes —
clic sur la carte (+1), bouton moins (-1), interrupteur (0 ou 1).

L'interrupteur n'est que le reflet de compte > 0 : eteindre trois blob puis
rallumer en ramene un. Aucun compte « en sommeil » a stocker, donc aucune
seconde verite a tenir d'accord avec config.personnages.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Tâche 9 — La suppression : la poubelle

**Le point n° 3 du brief est atteint à la fin de cette tâche.**

**Fichiers :**
- Modifier : `src-tauri/src/config.rs` (`est_dans_la_bibliotheque`)
- Modifier : `src-tauri/src/config_tests.rs`
- Modifier : `src-tauri/src/commandes.rs` (`supprimer`)
- Modifier : `ui/catalogue.js`, `ui/catalogue.html`

**Interfaces :**
- Consomme : `Actions::definir_roster` / `Actions::roster` (Tâche 6), `Depart` (Tâche 7)
- Produit :
  - `fn config::est_dans_la_bibliotheque(nom: &str) -> bool`
  - `#[tauri::command] async fn supprimer(app, actions, nom: String) -> Result<(), String>`
  - `fn Actions::acteurs_nommes(&self, nom: &str) -> usize` — combien d'acteurs de ce nom vivent **réellement**, publié par la boucle
  - `fn Actions::definir_roster_immediat(&self, voulus: &[String]) -> Result<u64, String>` — comme `definir_roster`, mais les retraits **sautent l'animation de départ**

> **Deux ajouts qui ne sont pas des détails.**
>
> `acteurs_nommes` est distinct du roster *voulu*, et c'est tout son intérêt :
> c'est la seule façon de savoir que la boucle a **fini**, plutôt que de le
> supposer après un délai fixe. La boucle le publie à chaque réconciliation,
> dans un `Mutex<Vec<String>>` des noms réellement présents.
>
> `definir_roster_immediat` existe parce que la Tâche 7 a fait de `RetirerUn`
> une animation de ~1 s, et que la suppression ne doit pas l'attendre (design
> §7). Concrètement : `Rechargement` gagne un champ `sans_animation: bool`, que
> la réconciliation lit pour détruire la fenêtre au lieu de poser un `Depart`.
> **Un booléen, pas un second chemin de réconciliation** — deux chemins
> divergeraient à la première correction.

- [ ] **Étape 1 : écrire les tests de la règle de suppressibilité**

```rust
#[test]
fn un_pack_du_depot_n_est_pas_supprimable() {
    // `blob` est livré dans le dépôt. La règle est GÉNÉRALE — « son dossier
    // résout dans la bibliothèque » — et surtout pas un cas particulier
    // nommé « blob » : nous ne supprimons jamais un fichier versionné, et
    // le dossier livré n'est peut-être même pas inscriptible (une
    // installation ordinaire le pose dans Program Files).
    assert!(!est_dans_la_bibliotheque("blob"));
}

#[test]
fn un_pack_inexistant_n_est_pas_supprimable() {
    assert!(!est_dans_la_bibliotheque("ce-pack-n-existe-pas-du-tout"));
}
```

- [ ] **Étape 2 : lancer les tests pour les voir échouer**

Run : `cargo test --quiet est_dans_la_bibliotheque 2>&1 | Select-Object -Last 15`
Attendu : ÉCHEC — fonction absente.

- [ ] **Étape 3 : implémenter la règle**

```rust
/// Ce pack vit-il dans la bibliothèque `%APPDATA%` ?
///
/// **C'est LA règle de suppressibilité** (design §7) : on ne supprime que
/// ce que le catalogue a installé, jamais un fichier versionné du dépôt.
///
/// Le cas de l'homonyme est assumé : un pack présent dans les DEUX racines
/// est supprimable, on efface la copie de la bibliothèque, et celle du
/// dépôt réapparaît alors dans la liste. C'est la conséquence directe de la
/// règle de résolution « bibliothèque d'abord, dépôt ensuite ». Ça peut
/// surprendre ; ça ne peut pas détruire de données.
pub fn est_dans_la_bibliotheque(nom: &str) -> bool {
    match dossier_bibliotheque() {
        Some(b) => b.join(nom).join("mascot.json").is_file(),
        None => false,
    }
}
```

`bibliotheque()` remplit `supprimable` avec cette fonction.

- [ ] **Étape 4 : écrire `supprimer`**

```rust
/// Supprime un pack du disque. **Définitivement, et sans corbeille.**
///
/// L'ordre n'est pas un détail (design §7) :
///   1. mettre le compte à 0 — retrait IMMÉDIAT, **sans l'animation de
///      départ** ;
///   2. attendre que ses acteurs aient disparu ;
///   3. effacer le dossier.
///
/// Effacer les PNG pendant qu'une fenêtre les réclame encore par le schéma
/// `shime://` donnerait un personnage à moitié dessiné en pleine chute. Et
/// un adieu animé sur un geste irréversible serait de toute façon déplacé :
/// la suppression est brutale parce qu'elle est définitive.
#[tauri::command]
pub async fn supprimer(
    app: AppHandle,
    actions: tauri::State<'_, std::sync::Arc<crate::actions::Actions>>,
    nom: String,
) -> Result<(), String> {
    // On refuse AVANT d'avoir rien retiré de l'écran : sinon un pack du
    // dépôt disparaîtrait de la vue sans être supprimé, et l'utilisateur
    // croirait à une suppression réussie.
    if !crate::config::est_dans_la_bibliotheque(&nom) {
        return Err(format!(
            "« {nom} » n'est pas dans votre bibliothèque : il est livré avec l'application"
        ));
    }

    // 1. Le retirer de l'écran. Les acteurs de ce nom sont marqués partants
    //    à l'image suivante — mais SANS l'animation : voir plus bas.
    let voulus: Vec<String> = actions.roster().into_iter().filter(|n| n != &nom).collect();
    actions.definir_roster_immediat(&voulus)?;
    crate::config::definir_personnages(&voulus)?;

    // 2. Attendre qu'ils aient réellement disparu.
    //
    // `spawn_blocking` : cette attente est du travail BLOQUANT. La laisser
    // sur l'exécuteur async figerait les autres commandes — la même raison
    // qui a fait écrire `installer` ainsi.
    //
    // La boucle 60 Hz a promis de retirer un acteur en 3 s au maximum
    // (garde-fou de `avancer_le_depart`) ; on laisse le double, puis on
    // renonce plutôt que d'attendre indéfiniment.
    let nom_pour_tache = nom.clone();
    let actions_pour_tache = std::sync::Arc::clone(&actions);
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        let limite = std::time::Instant::now() + std::time::Duration::from_secs(6);
        while actions_pour_tache.acteurs_nommes(&nom_pour_tache) > 0 {
            if std::time::Instant::now() > limite {
                return Err(format!(
                    "« {nom_pour_tache} » est encore à l'écran : rien n'a été supprimé"
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        // 3. Effacer. Maintenant seulement : plus aucune fenêtre ne réclame
        //    ces PNG par le schéma `shime://`.
        let Some(dossier) = crate::config::dossier_bibliotheque().map(|b| b.join(&nom_pour_tache))
        else {
            return Err("%APPDATA% introuvable".to_string());
        };
        std::fs::remove_dir_all(&dossier)
            .map_err(|e| format!("suppression de {} : {e}", dossier.display()))
    })
    .await
    .map_err(|e| format!("tâche de suppression interrompue : {e}"))??;

    println!("« {nom} » supprimé du disque");
    let _ = app;
    Ok(())
}
```

> **`acteurs_nommes` est nouveau** : un compteur partagé que la boucle tient à
> jour à chaque réconciliation, exposé par `Actions`. L'ajouter dans
> `actions.rs` comme un `Mutex<Vec<String>>` des noms **réellement présents** —
> distinct du roster *voulu*, et c'est tout l'intérêt : c'est la seule façon de
> savoir que la boucle a fini, plutôt que de le supposer après un délai fixe.

- [ ] **Étape 5 : la poubelle et sa confirmation**

Dans `ui/catalogue.js`, ajouter le bouton 🗑 en haut à droite de la carte,
**désactivé** si `!pack.supprimable`, avec une infobulle disant pourquoi.

```js
// `confirm` : c'est le SEUL geste de toute l'application qui detruise
// quelque chose, et il est sans corbeille.
if (!confirm(`Supprimer definitivement « ${pack.nom} » du disque ?`)) return;
```

L'échec est **bruyant** dans la barre d'état : une suppression silencieusement
ratée laisserait croire que le pack est parti alors qu'il reviendra au prochain
démarrage.

- [ ] **Étape 6 : lancer les tests**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 5`
Attendu : **289 passed** au minimum (287 + 2).

- [ ] **Étape 7 : vérifier à l'écran**

```powershell
cargo run -- --installer pierrot-54acb5
$env:SHIMEJI_CATALOGUE="1"; cargo run
```

- la poubelle de `blob` est **désactivée** ;
- la poubelle de `pierrot-54acb5` demande confirmation ;
- annuler → rien ne se passe ;
- confirmer alors qu'il est actif → il disparaît de l'écran **et** de la
  liste, et `%APPDATA%\shimeji-desktop\characters\pierrot-54acb5` n'existe plus.

- [ ] **Étape 8 : commit**

```bash
git add -A
git commit -m "feat(bibliotheque): la poubelle supprime un pack du disque

Le point n 3 du brief. La regle est GENERALE — supprimable si et seulement
si le dossier resout dans la bibliotheque %APPDATA% — et pas un cas
particulier nomme blob : nous ne supprimons jamais un fichier versionne, et
le dossier livre n'est peut-etre meme pas inscriptible.

L'ordre compte : compte a 0 d'abord, SANS animation de depart, puis
attendre que les acteurs aient disparu, puis effacer. Effacer les PNG
pendant qu'une fenetre les reclame encore par shime:// donnerait un
personnage a moitie dessine en pleine chute.

Confirmation demandee : c'est le seul geste de l'application qui detruise
quelque chose, et il est sans corbeille.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Tâche 10 — L'avertissement à 10 personnages

**Fichiers :**
- Modifier : `ui/catalogue.js`, `ui/catalogue.html`
- Modifier : `src-tauri/src/commandes.rs` (la trace sur la sortie standard)

- [ ] **Étape 1 : le bandeau**

Dans `rendre()`, quand la somme des compteurs atteint **10** :

```js
// ⚠️ Il AVERTIT, il n'interdit pas (design §2). Aucun bouton desactive,
// aucune confirmation, aucun plafond : c'est un panneau, pas une barriere.
// Decision de l'auteur, prise en connaissance de la mesure.
```

Texte exact :

> ⚠️ 10 personnages à l'écran. Chacun qui marche consomme du processeur ; à ce
> nombre, la consommation peut devenir notable.

Il disparaît si le total redescend sous 10.

- [ ] **Étape 2 : l'équivalent scriptable**

Dans `definir_compte`, quand le total atteint 10, écrire la **même ligne** sur
la sortie standard. C'est la règle du projet : tout ce qui demanderait un œil
sur une fenêtre en reçoit un équivalent lisible sans elle.

- [ ] **Étape 3 : vérifier**

```powershell
cargo build 2>&1 | Select-Object -Last 10
$env:SHIMEJI_PERSONNAGES="blob,blob,blob,blob,blob,blob,blob,blob,blob,blob"; cargo run
```
Attendu : l'avertissement sur la sortie standard, **et dix blob à l'écran** —
rien n'est refusé.

- [ ] **Étape 4 : commit**

```bash
git add -A
git commit -m "feat(bibliotheque): avertit a partir de 10 personnages

Il avertit, il n'interdit pas : aucun plafond dur, aucun bouton desactive,
aucune confirmation. Decision de l'auteur prise en connaissance de la
mesure du design 2.

La meme ligne part sur la sortie standard — l'equivalent scriptable de
l'avertissement.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Tâche 11 — La mesure : N=10, et la réserve du design

**Aucun code.** C'est une tâche de mesure, et son résultat est consigné
**quel qu'il soit**.

- [ ] **Étape 1 : mesurer le mode caché à N=4 et N=10**

```powershell
cargo build --release 2>&1 | Select-Object -Last 5
$env:SHIMEJI_PERSONNAGES="blob,blob,blob,blob"; $env:SHIMEJI_CACHE="1"
.\target\release\shimeji-desktop.exe
# puis le protocole de 60 s de CLAUDE.md
```

Attendu : proche de `0,9 % + ε × N`. **Une dérive signifierait que la boucle
unique fait, par personnage, du travail qu'elle ne devrait pas faire** — par
exemple une sonde de signaux passée par erreur dans la boucle `for`.

- [ ] **Étape 2 : mesurer en marche à N=10, sur 60 s**

⚠️ **60 secondes, jamais 10.** Et le chiffre brut ne se compare **pas** d'une
version à l'autre : le noter avec le taux de déplacement de
`SHIMEJI_CADENCE=1`, qui est ce qui le rend interprétable.

- [ ] **Étape 3 : observer la file de déplacements**

C'est la réserve du design §2, et elle n'est **pas** mesurée à ce jour. Le
symptôme cherché n'est pas une chute de cadence — la boucle ne bloque pas et
tiendra ses 60 Hz — mais des **personnages qui traînent visiblement derrière
leur position calculée**, le thread principal de Tauri ayant pris du retard.

Regarder à l'œil pendant que les dix marchent.

- [ ] **Étape 4 : consigner dans le design**

Ajouter les relevés au §2 de
`docs/specs/2026-09-15-plusieurs-personnages-design.md`, **y compris s'ils
contredisent la loi annoncée**. Cinq hypothèses évidentes sont déjà mortes sur
ce projet ; une sixième ne serait pas une surprise.

- [ ] **Étape 5 : commit**

```bash
git add docs/specs/2026-09-15-plusieurs-personnages-design.md
git commit -m "docs(mesure): le CPU a N=10, et la file de deplacements

Consigne dans le design la mesure que la reserve du 2 annoncait, plutot
que de la laisser en supposition.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Tâche 12 — *(optionnelle)* Grouper les déplacements — go/no-go sur mesure

**À ne faire que si la Tâche 11 montre un problème réel.** Sinon, sauter et
noter pourquoi.

L'idée (design §2) : appeler `SetWindowPos` directement depuis le thread de la
boucle, et grouper les N déplacements d'une image dans **une seule** transaction
`BeginDeferWindowPos` / `DeferWindowPos` / `EndDeferWindowPos`.

- [ ] **Étape 1 : mesurer l'état de départ** (N=10, 60 s, avec le taux de déplacement)
- [ ] **Étape 2 : implémenter le groupage** dans `render.rs`
- [ ] **Étape 3 : re-mesurer dans la MÊME configuration**
- [ ] **Étape 4 : trancher**

> **Si le gain ne se voit pas sur 60 s, le code est JETÉ** (`git checkout`) et la
> mesure consignée dans le design. On n'applique pas une optimisation non
> mesurée : c'est la règle du projet, et quatre hypothèses évidentes y sont
> déjà mortes. Une cinquième qui aurait l'air juste n'est pas une raison.

- [ ] **Étape 5 : commit** (le code **ou** la seule mesure, selon le verdict)

---

## Tâche 13 — La documentation

- [ ] **Étape 1 : mettre `CLAUDE.md` à jour**

- « État actuel » : plusieurs personnages, le nombre de tests, les CPU mesurés ;
- « Ordre de construction » : l'étape 3 **n'est toujours pas faite** — ils
  coexistent, ils ne se remarquent pas. Le dire explicitement, sans quoi une
  session future croira le social livré ;
- le tableau des variables d'environnement : `SHIMEJI_PERSONNAGES` et
  `SHIMEJI_ROSTER` remplacent `SHIMEJI_CHANGER` ;
- « Ajouter un pack » : le compteur, l'interrupteur, la poubelle ;
- la section « Mesurer le CPU » : **la loi**
  `CPU ≈ 0,9 + 0,3 × placements/s`, et que la variable est le nombre de
  personnages qui **marchent**, pas le nombre de personnages.

⚠️ **`CLAUDE.md` reste court** : il est relu à chaque session *et par chaque
sous-agent*. Le récit va dans `docs/`, seules les règles encore actives restent.

- [ ] **Étape 2 : ajouter l'étape au journal**

Dans `docs/conception/2026-09-14-journal-des-etapes.md` : ce qui a été
construit, et surtout **le piège du CPU qui s'est refermé une cinquième fois**
(N=2 moins cher que N=1) — c'est la partie qui servira le plus tard.

- [ ] **Étape 3 : marquer le plan soldé**

- [ ] **Étape 4 : lancer la suite complète une dernière fois**

Run : `cargo test --quiet 2>&1 | Select-Object -Last 5`

- [ ] **Étape 5 : commit**

```bash
git add -A
git commit -m "docs: plusieurs personnages, plan solde

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>"
```

---

## Ce que ce plan ne fait pas

- **Aucun comportement social.** Ils coexistent, ils ne se remarquent pas.
  C'était l'étape 3 et elle reste de côté.
- **Aucune plateforme de fenêtre** (étape 4b, revertée délibérément sur cette
  branche — ne pas tenter de la refusionner, git la considère déjà intégrée).
- **Aucun suivi de l'application au premier plan** (étape 5).
- **Aucun changement à** `menu_perso.rs`, `behavior/`, `character/`, `world.rs`,
  `geom.rs`. S'il faut les modifier, c'est le signe que quelque chose a dérivé
  du design — s'arrêter et le signaler plutôt que d'élargir.
