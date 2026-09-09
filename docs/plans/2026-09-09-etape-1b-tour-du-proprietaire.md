# Étape 1b — « Le tour du propriétaire » : plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rendre l'application vivable au quotidien — un tray avec « Quitter », un `config.json` qui règle son caractère sans recompiler, le démarrage avec Windows, et le rechargement à chaud des personnages.

**Architecture:** Aucun changement de comportement. Ce plan **déplace des constantes vers un fichier** et **ajoute une interface**. Si une tâche demande de toucher à `behavior/` ou à `character/physics.rs` autrement que pour y injecter des valeurs, c'est le signe qu'elle appartenait à 1a.

**Tech Stack:** Tauri 2.11.5 (*feature* `tray-icon`), crate `windows` 0.61 (*feature* `Win32_System_Registry`), `serde`/`serde_json`.

**Spec :** `docs/specs/2026-09-08-design.md` — §9 (tour du propriétaire), §8.1 (personnages externes), §7.2 (table d'envies).

**Ce que 1a a livré :** `docs/plans/2026-09-08-etape-1a-il-vit-sur-le-sol.md`, exécuté. Le personnage marche, court, s'arrête, fait demi-tour, circule sur les deux écrans, s'attrape, se lance et atterrit. 126 tests.

---

## Pourquoi ce plan existe, et dans cet ordre

**Le problème du quotidien, aujourd'hui : l'application ne se ferme pas.** La fenêtre est sans bordure, non focalisable, hors taskbar et hors Alt+Tab — c'est exactement ce qu'on voulait, et ça se retourne contre soi. Il faut `Stop-Process -Name shimeji-desktop`.

C'est pour ça que **le tray vient en premier**, avant la config qui est pourtant plus « structurante ». Une application qu'on n'arrête pas d'un clic n'est pas testable à l'usage, et « avoir l'air vivant sur plusieurs jours » (spec §1) est le critère de réussite du projet.

L'ordre qui suit n'est pas négociable, chaque tâche dépendant de la précédente :

| | Tâche | Pourquoi elle vient là |
|---|---|---|
| 1 | **Le tray**, « Quitter », afficher/cacher | rien d'autre n'est utilisable sans ça |
| 2 | **`config.rs`** : résolution des chemins, défauts | les tâches 3 à 5 ont toutes un réglage à stocker |
| 3 | **Les réglages de caractère** dans la config | c'est le but de la décision n° 5 : régler sans recompiler |
| 4 | **Démarrage automatique** | a besoin d'une case dans le tray (1) et d'un réglage (2) |
| 5 | **Rechargement à chaud** | a besoin d'une entrée de tray (1) |
| 6 | **Console retirée, et la mesure du `release`** | ne peut se faire qu'une fois « Quitter » disponible (1) |

---

## Global Constraints

Reprises de 1a, et toujours valables. Les quatre premières ont chacune coûté un diagnostic.

- **60 Hz, sans repli.** Le repli « 30 Hz + interpolation » de la spec §12 est abandonné.
- **`WS_EX_NOACTIVATE` et `WS_EX_TOOLWINDOW`** posés dès la création de chaque fenêtre.
- **`windows` épinglé en `0.61`** — la version de Tauri 2.11.5, sinon les `HWND` sont deux types distincts.
- **`SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)` en première instruction de `main`.**
- **Ne jamais supposer `x ≥ 0`** sur le bureau virtuel.
- **Compiler depuis PowerShell**, jamais depuis Git Bash (piège du `link.exe` de Git for Windows).
- **Aucun Node, aucun npm.** Front statique.
- **Tout en pixels physiques du bureau virtuel.** L'échelle ne sert qu'au sprite.
- **Le temps, l'aléatoire et la couche win32 restent injectés** (spec §10.2). Une valeur qui vient de la config ne change rien à ça : elle est lue au démarrage et passée en paramètre, jamais consultée depuis le cœur.
- **Code abondamment commenté en français**, expliquant le *pourquoi*.
- **Mesurer le CPU après toute modification du chemin 60 Hz** (`CLAUDE.md`, section « Mesurer le CPU »).

### Une contrainte propre à ce plan

> **L'absence de `config.json` n'est pas une erreur, et un fichier partiel non davantage** (spec §9.3).
>
> Toutes les valeurs ont un défaut dans le code. C'est ce qui permet de livrer un dossier sans configuration, et de laisser l'utilisateur n'écrire que la ligne qu'il veut changer. Chaque champ porte donc un `#[serde(default = "…")]`, et **un test vérifie qu'un fichier vide charge** — pas seulement qu'un fichier complet charge.

---

## Structure de fichiers

```
shimeji-desktop/
├── config.json             ← NOUVEAU, optionnel, à côté de l'exe
└── src-tauri/src/
    ├── main.rs             MODIFIÉ : tray, aiguillage, boucle
    ├── config.rs           ← NOUVEAU : chargement + résolution des chemins
    └── tray.rs             ← NOUVEAU : construction du menu et ses actions
```

`config.json` est **listé dans `.gitignore`** — c'est un fichier d'utilisateur, pas de dépôt. Un `config.exemple.json` commenté l'accompagne.

### Deux écarts par rapport à l'annexe A du plan de l'étape 0

L'annexe prévoyait `config.rs` et plaçait le tray dans `main.rs`. Deux ajustements :

1. **`tray.rs` est un fichier à part.** Le menu, ses identifiants et ses actions font une centaine de lignes qui n'ont rien à voir avec l'amorçage. `main.rs` fait déjà 400 lignes.
2. **`config.rs` absorbe `dossier_personnages`**, aujourd'hui dans `main.rs`. La résolution « à côté de l'exe, puis `%APPDATA%` » est la même pour les personnages et pour la config : une seule fonction, deux appels.

---

## Tâche 1 : Le tray, « Quitter », afficher / cacher

**Files:**
- Modify: `src-tauri/Cargo.toml` (*feature* `tray-icon`)
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/main.rs` (`mod tray;`, installation dans `setup`)

**Interfaces:**
- Consomme : `AppHandle`.
- Produit :
  - `tray::ID_AFFICHER`, `ID_RECHARGER`, `ID_DEMARRAGE`, `ID_DOSSIER`, `ID_QUITTER`
  - `tray::installer(&AppHandle, &Path, bool) -> Result<(), String>`
  - `tray::basculer_visibilite(&AppHandle, bool)`

> **La tâche qui rend l'application utilisable.** Aujourd'hui elle ne se ferme
> que par `Stop-Process` : la fenêtre est sans bordure, non focalisable, hors
> taskbar et hors Alt+Tab. C'est exactement ce qu'on voulait, et c'est
> exactement ce qui empêche de la quitter.

- [ ] **Step 1 : Activer la *feature* `tray-icon`**

Le module `tauri::tray` existe déjà, mais la dépendance `tray-icon` n'est pas
tirée : c'est une *feature* de Tauri (`tauri-2.11.5/Cargo.toml:129`), et notre
`Cargo.toml` déclare `features = []`.

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
```

```powershell
cd src-tauri
cargo build
```

Attendu : la compilation réussit et tire `tray-icon`. **Si `tauri::tray::TrayIconBuilder`
reste introuvable**, c'est que la *feature* n'a pas été prise — vérifier qu'il
n'y a pas deux déclarations de `tauri` dans le fichier.

- [ ] **Step 2 : Écrire `src-tauri/src/tray.rs`**

```rust
//! Le tray et son menu (spec §9.1).
//!
//! Responsabilité unique : construire le menu, et traduire un clic en action.
//! **Aucune logique de personnage ici** — les actions se contentent
//! d'appeler ailleurs.
//!
//! Fichier à part plutôt que dans `main.rs` : le menu, ses identifiants et
//! ses actions font une centaine de lignes qui n'ont rien à voir avec
//! l'amorçage, et `main.rs` en fait déjà 400.
//!
//! Signatures vérifiées dans tauri 2.11.5 :
//!   TrayIconBuilder          src/tray/mod.rs:216
//!   MenuItem::with_id        src/menu/normal.rs:48
//!   CheckMenuItem::with_id   src/menu/check.rs:49
//!   Menu::with_items         src/menu/menu.rs:120
//!   PredefinedMenuItem::separator  src/menu/predefined.rs:15
//!   Manager::webview_windows src/lib.rs:588
//!   AppHandle::exit          src/app.rs:574

use std::path::Path;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager};

// Les identifiants des entrées. Des constantes plutôt que des littéraux :
// l'identifiant est écrit à la construction du menu ET lu dans le
// gestionnaire d'événements, donc une faute de frappe donnerait une entrée
// qui ne fait silencieusement rien.
pub const ID_AFFICHER: &str = "afficher";
pub const ID_RECHARGER: &str = "recharger";
pub const ID_DEMARRAGE: &str = "demarrage";
pub const ID_DOSSIER: &str = "dossier";
pub const ID_QUITTER: &str = "quitter";

/// Montre ou cache toutes les fenêtres de personnages.
///
/// `webview_windows()` rend une table de toutes les fenêtres : on n'a donc
/// pas à tenir une liste de labels en parallèle, ce qui serait une seconde
/// source de vérité à maintenir synchronisée.
///
/// Les erreurs sont ignorées volontairement : une fenêtre déjà fermée n'est
/// pas un problème, et il n'y a rien à faire de plus que continuer avec les
/// autres.
pub fn basculer_visibilite(app: &AppHandle, visible: bool) {
    for (_label, win) in app.webview_windows() {
        let _ = if visible { win.show() } else { win.hide() };
    }
}

/// Installe l'icône du tray et son menu.
///
/// `dossier_personnages` est capturé par la fermeture du menu, pour l'entrée
/// « ouvrir le dossier ». `demarrage_actif` initialise la case à cocher
/// d'après ce que dit vraiment le registre — pas d'après la config, qui
/// pourrait mentir si l'utilisateur a retiré l'entrée à la main.
pub fn installer(
    app: &AppHandle,
    dossier_personnages: &Path,
    demarrage_actif: bool,
) -> Result<(), String> {
    // ── Les entrées ─────────────────────────────────────────────────────
    // `with_id` et non `new` : c'est l'identifiant qui reviendra dans
    // l'événement, et le laisser engendrer automatiquement rendrait le
    // `match` du gestionnaire impossible à écrire.
    //
    // Le dernier paramètre est un accélérateur clavier (`Option<&str>`) : on
    // n'en veut aucun. Un raccourci global sur un pet serait envahissant, et
    // `None` demande une annotation de type parce que rien ne permet
    // d'inférer `A`.
    let afficher = CheckMenuItem::with_id(
        app,
        ID_AFFICHER,
        "Afficher les personnages",
        true,
        true, // coché au démarrage : ils sont visibles
        None::<&str>,
    )
    .map_err(|e| format!("entrée « afficher » : {e}"))?;

    let recharger = MenuItem::with_id(
        app,
        ID_RECHARGER,
        "Recharger les personnages",
        true,
        None::<&str>,
    )
    .map_err(|e| format!("entrée « recharger » : {e}"))?;

    let demarrage = CheckMenuItem::with_id(
        app,
        ID_DEMARRAGE,
        "Démarrer avec Windows",
        true,
        demarrage_actif,
        None::<&str>,
    )
    .map_err(|e| format!("entrée « démarrage » : {e}"))?;

    let dossier = MenuItem::with_id(
        app,
        ID_DOSSIER,
        "Ouvrir le dossier des personnages",
        true,
        None::<&str>,
    )
    .map_err(|e| format!("entrée « dossier » : {e}"))?;

    let separateur =
        PredefinedMenuItem::separator(app).map_err(|e| format!("séparateur : {e}"))?;

    let quitter = MenuItem::with_id(app, ID_QUITTER, "Quitter", true, None::<&str>)
        .map_err(|e| format!("entrée « quitter » : {e}"))?;

    // `&[&dyn IsMenuItem<R>]` : les entrées n'ont pas le même type concret
    // (`MenuItem`, `CheckMenuItem`, `PredefinedMenuItem`), donc on passe par
    // des références de trait. C'est la raison du `&` devant chacune.
    let menu = Menu::with_items(
        app,
        &[
            &afficher,
            &recharger,
            &demarrage,
            &dossier,
            &separateur,
            &quitter,
        ],
    )
    .map_err(|e| format!("menu : {e}"))?;

    // ── Le gestionnaire d'événements ────────────────────────────────────
    // `move` : la fermeture doit posséder ce qu'elle utilise, puisqu'elle
    // survit à cette fonction. D'où le `to_path_buf` — on ne peut pas
    // capturer un `&Path` emprunté.
    let dossier_a_ouvrir = dossier_personnages.to_path_buf();

    // Les deux entrées à cocher sont capturées pour pouvoir lire leur état :
    // `is_checked()` dit si l'utilisateur vient de cocher ou de décocher.
    let afficher_pour_evenement = afficher.clone();
    let demarrage_pour_evenement = demarrage.clone();

    TrayIconBuilder::new()
        .menu(&menu)
        // L'icône du bundle, celle de `icons/icon.ico`. `Option` parce
        // qu'un projet peut n'en avoir aucune — ici elle est obligatoire
        // pour `tauri-build`, donc elle est toujours là.
        .icon(
            app.default_window_icon()
                .ok_or("aucune icône par défaut")?
                .clone(),
        )
        .tooltip("shimeji-desktop")
        .on_menu_event(move |app, evenement| {
            match evenement.id().as_ref() {
                ID_AFFICHER => {
                    // `is_checked` rend l'état APRÈS le clic : c'est
                    // directement la visibilité voulue.
                    let visible = afficher_pour_evenement.is_checked().unwrap_or(true);
                    basculer_visibilite(app, visible);
                }

                ID_RECHARGER => {
                    // Tâche 5. Pour l'instant, on le dit plutôt que de ne
                    // rien faire — une entrée de menu muette se diagnostique
                    // mal.
                    println!("rechargement : Tâche 5 du plan 1b");
                }

                ID_DEMARRAGE => {
                    // Tâche 4.
                    let _voulu = demarrage_pour_evenement.is_checked().unwrap_or(false);
                    println!("démarrage automatique : Tâche 4 du plan 1b");
                }

                ID_DOSSIER => {
                    // `explorer` plutôt qu'un plugin Tauri : c'est une ligne,
                    // ça n'ajoute aucune dépendance, et l'échec (dossier
                    // absent) n'a pas de conséquence.
                    let _ = std::process::Command::new("explorer")
                        .arg(&dossier_a_ouvrir)
                        .spawn();
                }

                ID_QUITTER => {
                    // `exit` termine le processus. Les threads des boucles
                    // 60 Hz meurent avec lui — ils ne détiennent aucune
                    // ressource à libérer proprement, seulement un
                    // `AppHandle`.
                    app.exit(0);
                }

                // Un identifiant inconnu ne peut venir que d'une entrée
                // ajoutée sans son cas ici. On le signale plutôt que de
                // l'ignorer.
                autre => eprintln!("entrée de tray non gérée : {autre}"),
            }
        })
        .build(app)
        .map_err(|e| format!("tray : {e}"))?;

    Ok(())
}
```

- [ ] **Step 3 : Installer le tray depuis `main.rs`**

Dans `lancer_application`, à la fin du `setup`, **avant** de lancer le thread
de la boucle :

```rust
            // ── Le tray ─────────────────────────────────────────────────
            // Installé avant la boucle : si le tray échoue, on veut le
            // savoir tout de suite, pas après avoir démarré un thread.
            //
            // `false` pour le démarrage automatique : la Tâche 4 le lira
            // dans le registre. Le mettre en dur ici serait un mensonge
            // durable — d'où le `⬜` qui suit.
            if let Err(e) = tray::installer(&app.handle().clone(), &dossier, false) {
                eprintln!("tray non installé : {e}");
            }
```

Et `mod tray;` dans la liste des modules.

> ⬜ **À reprendre en Tâche 4** : le `false` codé en dur. Le laisser tel quel
> afficherait une case décochée alors que le démarrage pourrait être actif.

- [ ] **Step 4 : Compiler et vérifier à l'œil**

```powershell
cd src-tauri
cargo run
```

| À faire | Attendu |
|---|---|
| regarder la zone de notification | une icône `shimeji-desktop` est apparue |
| clic droit dessus | le menu s'ouvre, cinq entrées et un séparateur |
| décocher « Afficher les personnages » | **le personnage disparaît** |
| recocher | il revient, là où il en était |
| « Ouvrir le dossier des personnages » | l'explorateur ouvre `characters/` |
| « Recharger » et « Démarrer avec Windows » | un message dans la console (Tâches 4 et 5) |
| **« Quitter »** | **le processus s'arrête** — plus besoin de `Stop-Process` |

**Si l'icône n'apparaît pas** : vérifier le message de `tray non installé` dans
la console. La cause la plus probable est `default_window_icon()` qui rend
`None`, ce qui signifierait que `icons/icon.ico` n'a pas été embarqué par
`tauri-build`.

- [ ] **Step 5 : Mesurer le CPU quand les personnages sont cachés**

C'est une mesure, pas une optimisation : elle dit s'il y a quelque chose à
gagner.

```powershell
# personnages visibles
$p = Get-Process -Name shimeji-desktop
$c = $p.CPU; Start-Sleep -Seconds 10; $p.Refresh()
"visible : $([math]::Round((($p.CPU - $c) / 10) * 100, 1)) %"
# puis décocher « Afficher », et refaire la mesure
```

**Attendu : aucune baisse.** La boucle continue de tourner et d'appeler
`set_position` sur une fenêtre cachée. Consigner le chiffre — la Tâche 6 s'en
servira.

- [ ] **Step 6 : Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/tray.rs src-tauri/src/main.rs
git commit -m "feat(etape-1b): le tray, « Quitter », afficher et cacher

La tâche qui rend l'application utilisable au quotidien : jusqu'ici elle ne
se fermait que par Stop-Process, la fenêtre étant sans bordure, non
focalisable, hors taskbar et hors Alt+Tab. C'est exactement ce qu'on voulait,
et exactement ce qui empêchait de la quitter.

tray-icon est une feature de Tauri (Cargo.toml:129) que notre features = []
n'activait pas : le module tauri::tray existait, sa dépendance non.

basculer_visibilite passe par webview_windows() plutôt que par une liste de
labels tenue en parallèle — une seconde source de vérité à synchroniser.

Les identifiants d'entrées sont des constantes : ils sont écrits à la
construction ET lus dans le gestionnaire, donc une faute de frappe donnerait
une entrée qui ne fait silencieusement rien. Un identifiant inconnu est
signalé plutôt qu'ignoré.

Deux entrées ne font encore qu'imprimer un message, et c'est délibéré :
recharger (Tâche 5) et démarrer avec Windows (Tâche 4). Une entrée de menu
muette se diagnostique mal."
```

---

## Tâche 2 : `config.rs` — la résolution des chemins et les défauts

**Files:**
- Create: `src-tauri/src/config.rs`
- Create: `config.exemple.json`
- Modify: `.gitignore` (`config.json` y est déjà)
- Modify: `src-tauri/src/main.rs` (retirer `dossier_personnages`, utiliser `config`)

**Interfaces:**
- Consomme : `serde`, `serde_json`, `std::fs`.
- Produit :
  - `config::Config { personnages, echelle, vitesse, demarrage_automatique, envies, allures }`
  - `config::Envies { flaner, se_reposer }`
  - `config::Allures { poids, durees, chance_demi_tour }`
  - `config::Reglages { vitesse_marche, vitesse_course, allures }` avec
    `depuis(&Config) -> Reglages`
  - `config::resoudre(&str) -> Option<PathBuf>`
  - `config::dossier_personnages() -> PathBuf`
  - `config::charger() -> Config` — **ne peut pas échouer**

> **La contrainte qui gouverne ce fichier : `charger()` ne rend pas de
> `Result`.** L'absence de `config.json` n'est pas une erreur, et un fichier
> partiel non davantage (spec §9.3). Un pet qui refuse de démarrer parce
> qu'une virgule manque dans un fichier optionnel serait absurde.

- [ ] **Step 1 : Écrire le test d'abord**

Créer `src-tauri/src/config.rs` avec **seulement** ce bloc.

```rust
#[cfg(test)]
mod tests {
    use super::*;

    static COMPTEUR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    /// Écrit un `config.json` dans un dossier temporaire et rend son chemin.
    fn fichier_de_test(contenu: &str) -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!(
            "shimeji-cfg-{}-{}",
            std::process::id(),
            COMPTEUR.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&base).unwrap();
        let f = base.join("config.json");
        std::fs::write(&f, contenu).unwrap();
        f
    }

    #[test]
    fn un_fichier_absent_donne_les_defauts() {
        // **Le test le plus important de cette tâche** (spec §9.3).
        let c = charger_depuis(std::path::Path::new("/aucun/chemin/config.json"));
        assert_eq!(c.echelle, Config::default().echelle);
        assert_eq!(c.vitesse, Config::default().vitesse);
        assert!(!c.personnages.is_empty(), "il doit rester un personnage par défaut");
    }

    #[test]
    fn un_fichier_vide_donne_les_defauts() {
        // Un objet JSON vide est valide, et doit donner exactement les
        // mêmes valeurs qu'un fichier absent.
        let f = fichier_de_test("{}");
        assert_eq!(charger_depuis(&f), Config::default());
    }

    #[test]
    fn un_fichier_partiel_ne_change_que_ce_qu_il_declare() {
        // C'est ce qui permet à l'utilisateur de n'écrire qu'une ligne.
        let f = fichier_de_test(r#"{ "vitesse": 0.5 }"#);
        let c = charger_depuis(&f);

        assert_eq!(c.vitesse, 0.5);
        // Tout le reste est intact.
        assert_eq!(c.echelle, Config::default().echelle);
        assert_eq!(c.envies, Config::default().envies);
        assert_eq!(c.allures, Config::default().allures);
    }

    #[test]
    fn un_json_malforme_donne_les_defauts_sans_paniquer() {
        // Une virgule en trop ne doit pas empêcher le personnage de vivre.
        let f = fichier_de_test("{ ceci n'est pas du JSON");
        assert_eq!(charger_depuis(&f), Config::default());
    }

    #[test]
    fn un_champ_inconnu_est_ignore() {
        // Un `config.json` écrit pour une version future, ou une faute de
        // frappe : on prend ce qu'on comprend, on ignore le reste.
        let f = fichier_de_test(r#"{ "vitesse": 2.0, "chose_inventee": 42 }"#);
        assert_eq!(charger_depuis(&f).vitesse, 2.0);
    }

    #[test]
    fn les_envies_partielles_gardent_les_autres_poids() {
        // Les défauts sont imbriqués : déclarer un seul poids ne doit pas
        // remettre les autres à zéro — ce qui rendrait le personnage
        // catatonique.
        let f = fichier_de_test(r#"{ "envies": { "seReposer": 4.0 } }"#);
        let c = charger_depuis(&f);

        assert_eq!(c.envies.se_reposer, 4.0);
        assert_eq!(c.envies.flaner, Config::default().envies.flaner);
    }

    #[test]
    fn les_reglages_appliquent_le_facteur_de_vitesse() {
        let mut c = Config::default();
        c.vitesse = 2.0;
        let r = Reglages::depuis(&c);

        assert_eq!(r.vitesse_marche, crate::character::physics::VITESSE_MARCHE * 2.0);
        assert_eq!(r.vitesse_course, crate::character::physics::VITESSE_COURSE * 2.0);
    }

    #[test]
    fn un_facteur_de_vitesse_absurde_est_borne() {
        // Un 0 figerait le personnage, un 1000 le rendrait invisible. Les
        // valeurs viennent d'un fichier édité à la main : on borne.
        let mut c = Config::default();

        c.vitesse = 0.0;
        assert!(Reglages::depuis(&c).vitesse_marche > 0.0, "il doit pouvoir bouger");

        c.vitesse = 10_000.0;
        assert!(
            Reglages::depuis(&c).vitesse_marche < 2000.0,
            "il ne doit pas traverser l'écran en une image"
        );
    }
}
```

- [ ] **Step 2 : Lancer les tests et les voir échouer**

```powershell
cd src-tauri
cargo test config
```

Attendu : **échec de compilation** — `Config`, `charger_depuis`, `Reglages`
n'existent pas.

- [ ] **Step 3 : Écrire l'implémentation, au-dessus des tests**

```rust
//! La configuration, et la résolution des chemins (spec §9.3, §8.1).
//!
//! Responsabilité unique : trouver les fichiers de l'utilisateur, et en
//! tirer des valeurs utilisables.
//!
//! **`charger()` ne rend pas de `Result`, et c'est une décision.** L'absence
//! de `config.json` n'est pas une erreur, un fichier partiel non davantage,
//! et même un fichier malformé ne doit pas empêcher le personnage de vivre
//! (spec §9.3). Un pet qui refuse de démarrer pour une virgule manquante
//! dans un fichier *optionnel* serait absurde. On signale et on continue.
//!
//! ⚠️ **Ce fichier ne contient aucune valeur en dur qui existerait déjà
//! ailleurs.** Les défauts de vitesse renvoient à `physics::VITESSE_*`, qui
//! sont eux-mêmes tirés de Shimeji-ee. Recopier 50 et 100 ici créerait deux
//! sources de vérité, et l'une des deux finirait par mentir.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Les poids de base de la table d'envies (spec §7.2).
///
/// `rename_all = "camelCase"` pour que le JSON s'écrive `seReposer`, cohérent
/// avec `frameMs` du manifeste.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Envies {
    pub flaner: f32,
    pub se_reposer: f32,
}

impl Default for Envies {
    fn default() -> Self {
        // Les valeurs de départ de la spec §7.2.
        Envies {
            flaner: 5.0,
            se_reposer: 1.0,
        }
    }
}

/// Les réglages de la flânerie : ce qui donne son tempérament au personnage.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Allures {
    /// Poids du tirage entre s'arrêter, marcher et courir.
    pub poids_arret: f32,
    pub poids_marche: f32,
    pub poids_course: f32,

    /// Durées `[min, max]` en secondes de chaque allure.
    pub duree_arret: [f32; 2],
    pub duree_marche: [f32; 2],
    pub duree_course: [f32; 2],

    /// Probabilité de faire demi-tour à chaque changement d'allure.
    ///
    /// C'est **le réglage le plus intéressant du fichier** : c'est lui qui
    /// décide si le personnage paraît décidé ou indécis. Le monter rend son
    /// parcours imprévisible et son déplacement net nul.
    pub chance_demi_tour: f32,
}

impl Default for Allures {
    fn default() -> Self {
        // Repris de `intention::flaner`, où ces valeurs étaient en dur.
        Allures {
            poids_arret: 3.0,
            poids_marche: 6.0,
            poids_course: 1.0,
            duree_arret: [0.8, 3.0],
            duree_marche: [1.5, 5.0],
            duree_course: [0.6, 1.8],
            chance_demi_tour: 0.25,
        }
    }
}

/// Le contenu de `config.json`.
///
/// `#[serde(default)]` **au niveau de la structure** : chaque champ absent
/// prend sa valeur de `Default`. Une seule annotation couvre tous les champs,
/// présents comme futurs — et c'est ce qui rend un fichier partiel valide
/// sans avoir à y penser champ par champ.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    /// Les dossiers de `characters/` à instancier.
    pub personnages: Vec<String>,

    /// Multiplie la taille d'affichage, en plus du `scale` du manifeste et de
    /// l'échelle du moniteur.
    pub echelle: f32,

    /// Multiplie les vitesses de marche et de course.
    ///
    /// Le réglage demandé pour ajuster le rythme sans toucher au code : à 1
    /// on est exactement aux valeurs de Shimeji-ee.
    pub vitesse: f32,

    pub demarrage_automatique: bool,
    pub envies: Envies,
    pub allures: Allures,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            // `blob` est le personnage de test, et le seul livré.
            personnages: vec!["blob".to_string()],
            echelle: 1.0,
            vitesse: 1.0,
            demarrage_automatique: false,
            envies: Envies::default(),
            allures: Allures::default(),
        }
    }
}

/// Les valeurs que le comportement consulte, déjà converties.
///
/// Séparée de `Config` pour une raison de principe : `Config` est ce que
/// l'utilisateur écrit, `Reglages` est ce que le code utilise. Le passage de
/// l'une à l'autre est le seul endroit où l'on borne les valeurs absurdes,
/// et le comportement n'a donc jamais à se demander si ce qu'il reçoit est
/// sain.
///
/// **Passée en paramètre, jamais consultée globalement** : c'est la même
/// discipline que l'horloge et l'aléatoire (spec §10.2), et c'est ce qui
/// garde les tests capables de fournir leurs propres réglages.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reglages {
    pub vitesse_marche: f32,
    pub vitesse_course: f32,
    pub allures: Allures,
}

/// Bornes du facteur de vitesse.
///
/// Les valeurs viennent d'un fichier édité à la main : un `0` figerait le
/// personnage — ce qui ressemblerait à un bug, pas à un réglage — et un
/// `10000` le ferait traverser l'écran en une image, ce qui casserait la
/// détection d'atterrissage par segment.
const FACTEUR_VITESSE_MIN: f32 = 0.1;
const FACTEUR_VITESSE_MAX: f32 = 10.0;

impl Reglages {
    pub fn depuis(config: &Config) -> Reglages {
        use crate::character::physics::{VITESSE_COURSE, VITESSE_MARCHE};

        let facteur = config
            .vitesse
            .clamp(FACTEUR_VITESSE_MIN, FACTEUR_VITESSE_MAX);

        Reglages {
            vitesse_marche: VITESSE_MARCHE * facteur,
            vitesse_course: VITESSE_COURSE * facteur,
            allures: config.allures,
        }
    }
}

/// Cherche un fichier ou un dossier nommé `nom`, à côté de l'exe puis dans
/// `%APPDATA%` (spec §8.1, §9.3).
///
/// Rend `None` si on ne le trouve nulle part — ce qui n'est une erreur que
/// pour l'appelant, qui sait s'il peut s'en passer.
pub fn resoudre(nom: &str) -> Option<PathBuf> {
    // ── À côté de l'exe ────────────────────────────────────────────────
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidat = dir.join(nom);
            if candidat.exists() {
                return Some(candidat);
            }

            // ── Repli de développement ─────────────────────────────────
            // En `cargo run`, l'exe est dans `src-tauri/target/debug/` : on
            // remonte les parents pour trouver la racine du dépôt. Cinq
            // niveaux suffisent à en sortir, et pas assez pour partir
            // explorer le disque.
            for parent in dir.ancestors().take(5) {
                let candidat = parent.join(nom);
                if candidat.exists() {
                    return Some(candidat);
                }
            }
        }
    }

    // ── %APPDATA%\shimeji-desktop\ ─────────────────────────────────────
    if let Ok(appdata) = std::env::var("APPDATA") {
        let candidat = PathBuf::from(appdata).join("shimeji-desktop").join(nom);
        if candidat.exists() {
            return Some(candidat);
        }
    }

    None
}

/// Le dossier des personnages.
///
/// Contrairement à `resoudre`, rend toujours un chemin : s'il n'existe pas,
/// le chargement du manifeste échouera avec un message qui **nomme le chemin
/// cherché**, ce qui est exactement ce qu'il faut pour diagnostiquer.
pub fn dossier_personnages() -> PathBuf {
    resoudre("characters").unwrap_or_else(|| PathBuf::from("characters"))
}

/// Charge la configuration. **Ne peut pas échouer.**
pub fn charger() -> Config {
    match resoudre("config.json") {
        Some(chemin) => charger_depuis(&chemin),
        None => {
            // Silencieux : l'absence de fichier est le cas NORMAL, pas un
            // avertissement à afficher à chaque démarrage.
            Config::default()
        }
    }
}

/// Charge depuis un chemin donné. Séparée de `charger` pour être testable
/// sans toucher au dossier de l'exe.
pub fn charger_depuis(chemin: &Path) -> Config {
    let texte = match std::fs::read_to_string(chemin) {
        Ok(t) => t,
        Err(e) => {
            // Le fichier a été trouvé par `resoudre` mais est illisible :
            // droits, verrou. Là, ça mérite un mot.
            eprintln!("config illisible ({}) : {e} — valeurs par défaut", chemin.display());
            return Config::default();
        }
    };

    match serde_json::from_str(&texte) {
        Ok(c) => c,
        Err(e) => {
            // **Bruyant, celui-là.** L'utilisateur a écrit un fichier et
            // s'attend à ce qu'il serve ; s'il est malformé il doit le
            // savoir, sinon il croira que ses réglages sont pris en compte.
            eprintln!("config.json invalide : {e}");
            eprintln!("  -> valeurs par défaut utilisées, le fichier est ignoré en entier");
            Config::default()
        }
    }
}
```

- [ ] **Step 4 : Écrire `config.exemple.json`**

À la racine, **à côté de `config.json`** qu'il documente. Toutes les valeurs y
sont celles par défaut, pour qu'on puisse le copier en `config.json` et
n'éditer qu'une ligne.

```json
{
  "personnages": ["blob"],

  "echelle": 1,
  "vitesse": 1,

  "demarrageAutomatique": false,

  "envies": {
    "flaner": 5,
    "seReposer": 1
  },

  "allures": {
    "poidsArret": 3,
    "poidsMarche": 6,
    "poidsCourse": 1,
    "dureeArret": [0.8, 3.0],
    "dureeMarche": [1.5, 5.0],
    "dureeCourse": [0.6, 1.8],
    "chanceDemiTour": 0.25
  }
}
```

> **Toutes les clés sont optionnelles.** Un `config.json` réduit à
> `{ "vitesse": 0.7 }` est parfaitement valide et ne change que la vitesse.
>
> `chanceDemiTour` est le réglage le plus intéressant : c'est lui qui décide
> si le personnage paraît décidé ou indécis.

- [ ] **Step 5 : Remplacer `dossier_personnages` de `main.rs`**

Supprimer la fonction de `main.rs` et utiliser `config::dossier_personnages()`
à ses deux points d'appel. Ajouter `mod config;`.

```powershell
cd src-tauri
cargo test
```

Attendu : **134 passed** — 126 de 1a, 8 de `config`.

- [ ] **Step 6 : Vérifier qu'un fichier partiel est pris**

```powershell
cd C:\Users\alri\Documents\shimeji-desktop
'{ "vitesse": 0.3 }' | Set-Content -Encoding utf8 config.json
cd src-tauri
cargo run
```

Attendu : **le personnage marche visiblement plus lentement.** Puis :

```powershell
cd C:\Users\alri\Documents\shimeji-desktop
'{ ceci est cassé' | Set-Content -Encoding utf8 config.json
cd src-tauri
cargo run
```

Attendu : la console imprime `config.json invalide : …` **et le personnage
marche quand même**, à vitesse normale. C'est la contrainte de la spec §9.3.

Retirer `config.json` avant de commiter — il est dans `.gitignore`, mais
autant ne pas le laisser traîner.

- [ ] **Step 7 : Commit**

```bash
git add src-tauri/src/config.rs src-tauri/src/main.rs config.exemple.json
git commit -m "feat(etape-1b): config.rs, la résolution des chemins et les défauts

charger() ne rend pas de Result, et c'est la décision qui gouverne le
fichier : l'absence de config.json n'est pas une erreur, un fichier partiel
non davantage, et même un fichier malformé ne doit pas empêcher le personnage
de vivre (spec §9.3). Un pet qui refuse de démarrer pour une virgule
manquante dans un fichier OPTIONNEL serait absurde.

#[serde(default)] au niveau de la structure, et non champ par champ : une
seule annotation couvre tous les champs, présents comme futurs. Un test
vérifie qu'un fichier VIDE charge, et un autre qu'un fichier partiel ne
change que ce qu'il déclare — c'est ce qui permet d'écrire une seule ligne.

Un JSON malformé est signalé BRUYAMMENT, contrairement à un fichier absent
qui est le cas normal : l'utilisateur qui a écrit un fichier s'attend à ce
qu'il serve, et doit savoir qu'il est ignoré plutôt que de croire ses
réglages pris en compte.

Reglages est séparée de Config : Config est ce que l'utilisateur écrit,
Reglages ce que le code utilise. Le passage de l'une à l'autre est le seul
endroit où les valeurs absurdes sont bornées, et le comportement n'a donc
jamais à se demander si ce qu'il reçoit est sain. Un facteur de vitesse de 0
figerait le personnage, un 10000 casserait la détection d'atterrissage par
segment.

Aucun défaut de vitesse n'est recopié ici : ils renvoient à physics::VITESSE_*,
eux-mêmes tirés de Shimeji-ee. Deux sources de vérité et l'une finirait par
mentir."
```

---

## Tâche 3 : Brancher la config sur le comportement

**Files:**
- Modify: `src-tauri/src/behavior/desire.rs` (`TableEnvies::depuis_config`)
- Modify: `src-tauri/src/behavior/intention.rs` (les allures viennent des réglages)
- Modify: `src-tauri/src/behavior/mod.rs` (`pas` prend `&Reglages`)
- Modify: `src-tauri/src/main.rs`, `src-tauri/src/sim.rs` (passer les réglages)

**Interfaces:**
- Consomme : `config::{Config, Reglages}`.
- Produit :
  - `TableEnvies::depuis_config(&Config) -> TableEnvies`
  - `behavior::pas(…, reglages: &Reglages, …)` — un paramètre de plus
  - `intention::poursuivre(…, reglages: &Reglages, …)`

> **C'est le paiement de la décision n° 5** : « les poids vivent dans la
> config → on règle son caractère sans recompiler ». Jusqu'ici les poids
> étaient dans `TableEnvies::defaut()` et les durées d'allure en dur dans
> `intention::flaner`.

- [ ] **Step 1 : Écrire le test d'abord, dans `desire.rs`**

```rust
    #[test]
    fn la_table_suit_les_poids_de_la_config() {
        // Le test qui prouve qu'on règle sans recompiler.
        let m = manifeste_avec(&["stand", "walk", "sit"]);

        let mut c = crate::config::Config::default();
        c.envies.flaner = 1.0;
        c.envies.se_reposer = 9.0;

        let table = TableEnvies::depuis_config(&c);
        let mut rng = XorShift32::seeded(42);

        let (mut flaner, mut reposer) = (0, 0);
        for _ in 0..10_000 {
            match table.tirer(&m, &mut rng) {
                Some(Intention::Flaner) => flaner += 1,
                Some(Intention::SeReposer) => reposer += 1,
                None => {}
            }
        }

        // Rapport inversé par rapport au défaut : il se repose maintenant
        // neuf fois sur dix.
        assert!(reposer > flaner * 5, "reposer {reposer}, flâner {flaner}");
    }

    #[test]
    fn la_table_par_defaut_est_celle_de_la_config_par_defaut() {
        // Deux chemins vers les mêmes poids : ils ne doivent pas divergerpar
        // inadvertance. `defaut()` doit être exactement
        // `depuis_config(&Config::default())`.
        let a = TableEnvies::defaut();
        let b = TableEnvies::depuis_config(&crate::config::Config::default());

        assert_eq!(a.entrees.len(), b.entrees.len());
        for (ea, eb) in a.entrees.iter().zip(b.entrees.iter()) {
            assert_eq!(ea.intention, eb.intention);
            assert_eq!(ea.base, eb.base);
        }
    }
```

- [ ] **Step 2 : Implémenter `depuis_config`, et faire de `defaut` un alias**

```rust
impl TableEnvies {
    /// Les valeurs de départ de la spec §7.2.
    ///
    /// **Délègue à `depuis_config`** plutôt que de recopier les poids : deux
    /// listes de nombres finiraient par divergerà la première modification,
    /// et un test le vérifie.
    pub fn defaut() -> TableEnvies {
        Self::depuis_config(&crate::config::Config::default())
    }

    /// La table telle que l'utilisateur l'a réglée.
    ///
    /// **Les `poses_requises` ne viennent PAS de la config**, et c'est
    /// délibéré : ce ne sont pas des préférences mais des faits — flâner
    /// exige de savoir marcher. Les laisser configurer permettrait de
    /// demander une intention injouable, ce que la couverture partielle
    /// (spec §8.6) est justement là pour rendre impossible.
    pub fn depuis_config(config: &crate::config::Config) -> TableEnvies {
        TableEnvies {
            entrees: vec![
                EntreeEnvie {
                    intention: Intention::Flaner,
                    base: config.envies.flaner,
                    poses_requises: &[POSE_WALK],
                },
                EntreeEnvie {
                    intention: Intention::SeReposer,
                    base: config.envies.se_reposer,
                    poses_requises: &[POSE_SIT],
                },
            ],
        }
    }
}
```

- [ ] **Step 3 : Faire descendre `Reglages` jusqu'à `flaner`**

`behavior::pas` prend `reglages: &Reglages` et le passe à
`intention::poursuivre`, qui le passe à `flaner`. Dans `flaner`, remplacer les
littéraux :

```rust
    if maintenant >= jusqu_a {
        // Les poids et les durées viennent maintenant des réglages
        // (décision n° 5). `chance_demi_tour` est le plus intéressant des
        // trois : c'est lui qui décide si le personnage paraît décidé ou
        // indécis.
        let a = &reglages.allures;
        let poids = [a.poids_arret, a.poids_marche, a.poids_course];

        allure = match rng.weighted(&poids) {
            Some(0) => Allure::Arret,
            Some(2) => Allure::Course,
            _ => Allure::Marche,
        };

        let plage = match allure {
            Allure::Arret => a.duree_arret,
            Allure::Marche => a.duree_marche,
            Allure::Course => a.duree_course,
        };
        jusqu_a = maintenant + Duration::from_secs_f32(rng.range(plage[0], plage[1]));

        if rng.unit_f32() < a.chance_demi_tour {
            ch.facing = ch.facing.inverse();
        }
    }
```

Et `Allure::vitesse` devient une fonction des réglages plutôt qu'une méthode
sur l'énumération :

```rust
/// La vitesse d'une allure, d'après les réglages.
///
/// Fonction libre et non méthode de `Allure` : la vitesse n'est plus une
/// propriété intrinsèque de l'allure, elle dépend de la configuration.
/// Laisser la méthode obligerait à donner à `Allure` une référence aux
/// réglages, ce qui n'a pas de sens pour une étiquette.
fn vitesse_de(allure: Allure, reglages: &Reglages) -> f32 {
    match allure {
        Allure::Arret => 0.0,
        Allure::Marche => reglages.vitesse_marche,
        Allure::Course => reglages.vitesse_course,
    }
}
```

- [ ] **Step 4 : Appliquer `echelle` — sans changer de signature**

`config.echelle` multiplie la taille d'affichage. Plutôt que de la faire
descendre jusqu'à `attach`, on la **multiplie dans l'échelle passée** depuis
`main.rs` :

```rust
            // L'échelle d'affichage : celle du moniteur, multipliée par le
            // réglage de l'utilisateur. Les fonctions de `attach` n'ont pas
            // à savoir que le second existe — elles reçoivent un seul
            // facteur, et c'est tout ce dont elles ont besoin.
            let echelle_affichage = ecrans[0].scale * config.echelle;
```

> **Renommer `Entrees::echelle_ecran` en `echelle_affichage`**, et de même les
> paramètres `scale_ecran` de `attach`. Le nom actuel deviendrait un mensonge
> dès qu'on y glisse le facteur de la config, et un nom qui ment coûte plus
> cher qu'un renommage mécanique.

- [ ] **Step 5 : `--sim` accepte aussi la config**

Le mode simulation doit refléter les réglages, sinon il vérifie un
comportement que l'utilisateur n'a pas. `sim::executer` prend un `&Config` de
plus, et `main` lui passe celui chargé.

⚠️ **Garder un `Config::default()` dans les tests de `sim.rs`** : un test qui
lirait le `config.json` de la machine ne serait plus reproductible.

- [ ] **Step 6 : Tests, puis vérification à l'œil**

```powershell
cd src-tauri
cargo test
```

Attendu : **136 passed**.

```powershell
cd C:\Users\alri\Documents\shimeji-desktop
'{ "allures": { "chanceDemiTour": 0.9 } }' | Set-Content -Encoding utf8 config.json
cd src-tauri
cargo run
```

Attendu : **il devient hésitant** — il fait demi-tour presque à chaque
changement d'allure et n'avance quasiment plus. C'est la preuve visible que le
caractère se règle sans recompiler.

Retirer `config.json` ensuite.

- [ ] **Step 7 : Commit**

```bash
git add src-tauri/src
git commit -m "feat(etape-1b): le caractère se règle dans config.json

Le paiement de la décision n° 5 : « les poids vivent dans la config, on
règle son caractère sans recompiler ». Jusqu'ici les poids étaient dans
TableEnvies::defaut() et les durées d'allure en dur dans intention::flaner.

TableEnvies::defaut() DÉLÈGUE maintenant à depuis_config(&Config::default())
au lieu de recopier les poids : deux listes de nombres finiraient par
divergerà la première modification. Un test verrouille leur égalité.

Les poses_requises ne viennent PAS de la config, et c'est délibéré : ce ne
sont pas des préférences mais des faits — flâner exige de savoir marcher.
Les laisser configurer permettrait de demander une intention injouable, ce
que la couverture partielle est justement là pour rendre impossible.

Allure::vitesse devient la fonction libre vitesse_de(allure, reglages) : la
vitesse n'est plus une propriété intrinsèque de l'allure. Garder la méthode
obligerait à donner une référence aux réglages à ce qui n'est qu'une
étiquette.

echelle_ecran est renommé echelle_affichage partout : il transporte désormais
le produit de l'échelle du moniteur et du réglage utilisateur, et l'ancien
nom serait devenu un mensonge."
```

---

## Tâche 4 : Le démarrage automatique

**Files:**
- Modify: `src-tauri/Cargo.toml` (*feature* `Win32_System_Registry`)
- Create: `src-tauri/src/autostart.rs`
- Modify: `src-tauri/src/tray.rs` (brancher la case), `src-tauri/src/main.rs`

**Interfaces:**
- Produit :
  - `autostart::est_actif() -> bool`
  - `autostart::activer() -> Result<(), String>`
  - `autostart::desactiver() -> Result<(), String>`

> **Écart assumé par rapport à l'annexe A**, qui plaçait ceci dans `main.rs` :
> c'est du win32 avec des chaînes larges et des `unsafe`, et le mélanger à
> l'amorçage rendrait les deux moins lisibles. Ce n'est pas non plus une
> *sonde* — `probe/` ne fait que lire, jamais écrire.

- [ ] **Step 1 : Activer la *feature* du registre**

```toml
features = [
    # … les cinq existantes …
    "Win32_System_Registry",            # Reg{Open,Set,Query,Delete}Value*
]
```

- [ ] **Step 2 : Écrire `src-tauri/src/autostart.rs`**

```rust
//! Le démarrage avec Windows (spec §9.2).
//!
//! Responsabilité unique : lire, poser et retirer **une** valeur de registre.
//!
//! La clé est `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, celle de
//! l'utilisateur courant. **Pas `HKLM`** : celle-là est globale à la machine,
//! exige des droits administrateur, et un desktop pet n'a aucune raison d'en
//! demander.
//!
//! Spec §9.2 : « au démarrage automatique, aucune fenêtre, aucune
//! notification, aucun vol de focus ». C'est déjà vrai par construction — la
//! fenêtre est non focalisable et hors taskbar depuis l'étape 1a — donc il
//! n'y a rien de particulier à faire ici.
//!
//! Signatures vérifiées dans windows 0.61.3, `Win32/System/Registry/mod.rs` :
//!   RegOpenKeyExW    :376
//!   RegSetValueExW   :626
//!   RegQueryValueExW :489
//!   RegDeleteValueW  :211

use windows::core::PCWSTR;
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
};

/// Le nom de la valeur dans la clé `Run`. C'est ce qui apparaît dans le
/// gestionnaire des tâches, onglet « Démarrage ».
const NOM_VALEUR: &str = "shimeji-desktop";

const CHEMIN_RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

/// Convertit une chaîne Rust en chaîne large terminée par un zéro.
///
/// Les API `…W` de Windows attendent de l'UTF-16 avec un terminateur nul.
/// Rust stocke ses `str` en UTF-8 sans terminateur, donc **les deux
/// conversions sont nécessaires** — et le `Vec` doit rester vivant aussi
/// longtemps que le pointeur qu'on en tire, d'où le fait qu'on le nomme au
/// lieu de l'utiliser en ligne.
fn large(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Ouvre la clé `Run`. Le `HKEY` rendu doit être fermé par l'appelant.
///
/// Fonction privée : c'est un détail d'implémentation, et exposer un handle
/// brut inviterait à oublier de le fermer.
fn ouvrir_run(droits: windows::Win32::System::Registry::REG_SAM_FLAGS) -> Result<HKEY, String> {
    let chemin = large(CHEMIN_RUN);
    let mut cle = HKEY::default();

    // SÉCURITÉ : `chemin` vit jusqu'à la fin de la fonction, et
    // `RegOpenKeyExW` ne conserve pas le pointeur.
    let code = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(chemin.as_ptr()),
            None,
            droits,
            &mut cle,
        )
    };

    if code != ERROR_SUCCESS {
        return Err(format!("ouverture de HKCU\\{CHEMIN_RUN} : code {}", code.0));
    }
    Ok(cle)
}

/// Le démarrage automatique est-il actif ?
///
/// **Interroge le registre, pas la config.** Les deux peuvent divergerdès que
/// l'utilisateur retire l'entrée à la main ou par le gestionnaire des tâches,
/// et c'est le registre qui dit la vérité. C'est lui qui initialise la case
/// du tray.
pub fn est_actif() -> bool {
    let Ok(cle) = ouvrir_run(KEY_READ) else {
        return false;
    };

    let nom = large(NOM_VALEUR);
    // On ne veut pas la valeur, seulement savoir si elle existe : tous les
    // paramètres de sortie sont donc `None`.
    let code = unsafe {
        RegQueryValueExW(cle, PCWSTR(nom.as_ptr()), None, None, None, None)
    };

    unsafe {
        let _ = RegCloseKey(cle);
    }

    code == ERROR_SUCCESS
}

/// Inscrit l'exécutable courant dans la clé `Run`.
pub fn activer() -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("chemin de l'exécutable introuvable : {e}"))?;

    // Les guillemets sont **obligatoires** : sans eux, un chemin contenant
    // une espace (« C:\Program Files\… ») serait coupé par Windows au
    // premier blanc, et il tenterait de lancer « C:\Program ».
    let commande = format!("\"{}\"", exe.display());

    let cle = ouvrir_run(KEY_WRITE)?;
    let nom = large(NOM_VALEUR);
    let valeur = large(&commande);

    // `RegSetValueExW` attend des OCTETS, pas des u16 : on reinterprète le
    // tableau. La longueur est donc `len() * 2`.
    //
    // `align_to` serait plus idiomatique mais rend un triplet à valider ;
    // ici l'alignement de u16 vers u8 est toujours valide (on relâche
    // l'alignement, on ne le resserre pas).
    let octets: &[u8] = unsafe {
        std::slice::from_raw_parts(valeur.as_ptr() as *const u8, valeur.len() * 2)
    };

    let code = unsafe {
        RegSetValueExW(cle, PCWSTR(nom.as_ptr()), None, REG_SZ, Some(octets))
    };

    unsafe {
        let _ = RegCloseKey(cle);
    }

    if code != ERROR_SUCCESS {
        return Err(format!("écriture de la valeur : code {}", code.0));
    }
    Ok(())
}

/// Retire l'entrée de la clé `Run`.
///
/// Une valeur déjà absente n'est **pas** une erreur : le résultat voulu est
/// « elle n'y est plus », et il est atteint.
pub fn desactiver() -> Result<(), String> {
    let cle = ouvrir_run(KEY_WRITE)?;
    let nom = large(NOM_VALEUR);

    let code = unsafe { RegDeleteValueW(cle, PCWSTR(nom.as_ptr())) };

    unsafe {
        let _ = RegCloseKey(cle);
    }

    // `ERROR_FILE_NOT_FOUND` = elle n'y était pas. C'est le résultat voulu.
    use windows::Win32::Foundation::ERROR_FILE_NOT_FOUND;
    if code != ERROR_SUCCESS && code != ERROR_FILE_NOT_FOUND {
        return Err(format!("suppression de la valeur : code {}", code.0));
    }
    Ok(())
}

// Pas de tests unitaires : ces trois fonctions ne contiennent aucune logique,
// seulement des appels au registre. Un test y affirmerait que Windows
// fonctionne — et surtout, il **modifierait le registre de la machine qui
// exécute la suite**, ce qui est inacceptable pour un `cargo test`.
//
// La vérification est manuelle, une fois, au Step 4.
```

- [ ] **Step 3 : Brancher la case du tray**

Dans `tray.rs`, remplacer le `println!` de `ID_DEMARRAGE` :

```rust
                ID_DEMARRAGE => {
                    let voulu = demarrage_pour_evenement.is_checked().unwrap_or(false);

                    let resultat = if voulu {
                        crate::autostart::activer()
                    } else {
                        crate::autostart::desactiver()
                    };

                    if let Err(e) = resultat {
                        eprintln!("démarrage automatique : {e}");
                        // On remet la case dans l'état RÉEL : laisser une
                        // case cochée alors que l'écriture a échoué serait
                        // un mensonge affiché en permanence.
                        let _ = demarrage_pour_evenement.set_checked(!voulu);
                    }
                }
```

Et dans `main.rs`, l'installation lit désormais le registre :

```rust
            if let Err(e) = tray::installer(
                &app.handle().clone(),
                &dossier,
                autostart::est_actif(),
            ) {
```

- [ ] **Step 4 : Vérifier à l'œil, et nettoyer derrière soi**

```powershell
cd src-tauri
cargo run
```

| À faire | Attendu |
|---|---|
| cocher « Démarrer avec Windows » | aucun message d'erreur en console |
| `Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name shimeji-desktop` | la valeur existe, **chemin entre guillemets** |
| gestionnaire des tâches → Démarrage | `shimeji-desktop` y figure |
| quitter, relancer | **la case est déjà cochée** — c'est le registre qui l'a dit |
| décocher, puis vérifier le registre | la valeur a disparu |

⚠️ **Décocher avant de finir**, sinon l'application démarrera à chaque
ouverture de session pendant tout le développement.

- [ ] **Step 5 : Commit**

```bash
git add src-tauri/Cargo.toml src-tauri/src/autostart.rs src-tauri/src/tray.rs src-tauri/src/main.rs
git commit -m "feat(etape-1b): le démarrage avec Windows

Une valeur dans HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run, la
clé de l'utilisateur courant. Pas HKLM : celle-là est globale à la machine,
exige des droits administrateur, et un desktop pet n'a aucune raison d'en
demander.

est_actif() interroge le REGISTRE et non la config : les deux divergent dès
que l'utilisateur retire l'entrée à la main ou par le gestionnaire des
tâches, et c'est le registre qui dit la vérité. C'est lui qui initialise la
case du tray, ce qui remplace le `false` codé en dur de la Tâche 1.

Le chemin est écrit ENTRE GUILLEMETS. Sans eux, un chemin contenant une
espace serait coupé par Windows au premier blanc, et il tenterait de lancer
« C:\\Program ».

Une désactivation sur une valeur déjà absente n'est pas une erreur : le
résultat voulu est qu'elle n'y soit plus, et il est atteint.

Si l'écriture échoue, la case du tray est remise dans son état réel — laisser
une case cochée après un échec serait un mensonge affiché en permanence.

Aucun test unitaire, et c'est délibéré : ces fonctions n'ont pas de logique,
et un test MODIFIERAIT le registre de la machine qui exécute la suite. La
vérification est manuelle, une fois.

Écart assumé par rapport à l'annexe A qui plaçait ceci dans main.rs : c'est
du win32 avec des chaînes larges et des unsafe, et ce n'est pas une sonde —
probe/ ne fait que lire."
```

---

## Tâche 5 : Le rechargement à chaud

**Files:**
- Create: `src-tauri/src/rechargement.rs`
- Modify: `ui/pet.js` (contournement du cache), `src-tauri/src/render.rs`
- Modify: `src-tauri/src/tray.rs`, `src-tauri/src/main.rs`

**Interfaces:**
- Produit :
  - `rechargement::Demande` = `Arc<Mutex<Option<Rechargement>>>`
  - `rechargement::Rechargement { manifeste, reglages, version }`
  - `rechargement::preparer(&Demande, &Path, &str) -> Result<u64, String>`
  - `render::recharger(&AppHandle, &str, u64) -> Result<(), String>`

> **Cette tâche pèse plus qu'il n'y paraît** (spec §9.1) : c'est elle qui rend
> supportable le réglage des animations. Sans elle, chaque ajustement de
> timing dans `mascot.json` coûte un redémarrage — et le réglage du manifeste
> sera l'essentiel du travail de finition.
>
> Le schéma URI de l'étape 1a la rend presque gratuite : les images étant
> servies depuis le disque à chaque requête, il ne reste qu'à contourner le
> cache du webview.

- [ ] **Step 1 : Écrire `src-tauri/src/rechargement.rs`**

```rust
//! Le rechargement à chaud des personnages (spec §9.1).
//!
//! Responsabilité unique : porter un manifeste fraîchement lu depuis le
//! thread du tray jusqu'au thread de la boucle 60 Hz.
//!
//! # Pourquoi un `Mutex` alors que tout le reste s'en passe
//!
//! Le manifeste est **possédé** par le `Character`, qui vit dans le thread de
//! la boucle. Le clic de tray, lui, arrive sur le thread principal de Tauri.
//! Deux threads, une donnée à transmettre : il faut une synchronisation.
//!
//! `Arc<Mutex<Option<…>>>` est le plus simple qui marche :
//!   · `Arc` — plusieurs propriétaires (le tray et la boucle) ;
//!   · `Mutex` — un seul écrit à la fois ;
//!   · `Option` — la boîte est vide la plupart du temps, et `take()` la vide
//!     en récupérant son contenu, ce qui rend « consommer la demande »
//!     atomique et évite d'avoir à remettre un drapeau à zéro.
//!
//! **Le verrou n'est jamais tenu pendant une entrée-sortie** : c'est le tray
//! qui lit le disque, *puis* pose le résultat. La boucle 60 Hz ne fait que
//! `try_lock` et `take` — jamais de blocage à 60 Hz sur un accès disque.

use crate::character::manifest::Manifest;
use crate::config::Reglages;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Ce qu'un rechargement apporte.
pub struct Rechargement {
    pub manifeste: Manifest,
    pub reglages: Reglages,

    /// Numéro de version, incrémenté à chaque rechargement.
    ///
    /// Sert **uniquement** à contourner le cache du webview : les images sont
    /// servies avec `Cache-Control: max-age=3600`, donc une image modifiée
    /// sur le disque ne serait pas rechargée. On change l'URL plutôt que le
    /// cache — `?v=3` au lieu de `?v=2` — ce qui est le contournement usuel
    /// et ne demande aucun changement côté serveur : le gestionnaire du
    /// schéma URI lit `uri().path()`, qui **ignore la requête**.
    pub version: u64,
}

/// La boîte partagée entre le tray et la boucle.
pub type Demande = Arc<Mutex<Option<Rechargement>>>;

pub fn nouvelle_demande() -> Demande {
    Arc::new(Mutex::new(None))
}

/// Relit le manifeste et la config depuis le disque, et dépose le résultat.
///
/// Appelée depuis le thread du tray. Rend la nouvelle version, ou l'erreur —
/// que l'appelant affiche, parce qu'un rechargement silencieusement raté est
/// le pire des cas : on croit tester son nouveau timing et on regarde
/// l'ancien.
pub fn preparer(demande: &Demande, dossier: &Path, personnage: &str) -> Result<u64, String> {
    // ── Les entrées-sorties D'ABORD, verrou non tenu ────────────────────
    let manifeste = Manifest::load(&dossier.join(personnage))
        .map_err(|e| format!("manifeste illisible, rien n'a changé : {e}"))?;

    let config = crate::config::charger();
    let reglages = Reglages::depuis(&config);

    // ── Puis on pose, brièvement ────────────────────────────────────────
    let mut boite = demande
        .lock()
        // `PoisonError` : un autre thread a paniqué en tenant le verrou.
        // Ça ne peut arriver que si la boucle a paniqué, auquel cas le
        // rechargement est le moindre des soucis.
        .map_err(|_| "verrou de rechargement empoisonné".to_string())?;

    // La version repart de celle en attente s'il y en avait une, pour que
    // deux clics rapprochés ne rendent pas le même numéro.
    let version = boite.as_ref().map(|r| r.version).unwrap_or(0) + 1;

    *boite = Some(Rechargement {
        manifeste,
        reglages,
        version,
    });

    Ok(version)
}
```

- [ ] **Step 2 : Contourner le cache dans `ui/pet.js`**

```js
// Version du contenu, changée à chaque rechargement à chaud.
//
// Les images sont servies avec `Cache-Control: max-age=3600` : sans ce
// paramètre, une image modifiée sur le disque ne serait pas relue. On change
// donc l'URL plutôt que le cache.
let version = 0;

function urlDe(n) {
  const cle = `${version}/${n}`;
  if (!cache.has(cle)) {
    const img = new Image();
    // Le gestionnaire du schéma URI lit `uri().path()`, qui ignore la
    // requête : `?v=3` ne change donc rien côté Rust, seulement la clé de
    // cache du webview.
    img.src = `${BASE}${n}?v=${version}`;
    cache.set(cle, img.src);
  }
  return cache.get(cle);
}

// Appelée par Rust après un rechargement à chaud.
window.recharger = (v) => {
  version = v;
  // On force le prochain `poser` à réécrire le `src`, même si l'image
  // demandée porte le même numéro qu'avant.
  derniere = null;
};
```

- [ ] **Step 3 : Ajouter `render::recharger`**

```rust
/// Prévient le webview qu'il doit oublier ses images.
///
/// Séparée de `pousser` parce qu'elle n'arrive que sur action de
/// l'utilisateur, jamais dans la boucle.
pub fn recharger(app: &AppHandle, label: &str, version: u64) -> Result<(), String> {
    let Some(win) = app.get_webview_window(label) else {
        return Err(format!("fenêtre « {label} » absente"));
    };
    win.eval(format!("window.recharger({version})"))
        .map_err(|e| format!("eval : {e}"))
}
```

- [ ] **Step 4 : Consommer la demande dans la boucle**

Dans `boucle`, **au rythme du recensement (8 Hz) et non à 60 Hz** — un
rechargement n'a aucune raison d'être vu en 16 ms :

```rust
        // ── ~8 Hz : une demande de rechargement en attente ? ────────────
        if maintenant.saturating_sub(dernier_recensement) >= PERIODE_MONDE {
            // … le recensement des écrans, déjà là …

            // `try_lock` et non `lock` : la boucle 60 Hz ne doit JAMAIS
            // attendre. Si le tray tient le verrou à cet instant, on
            // réessaiera dans 125 ms et personne ne s'en apercevra.
            if let Ok(mut boite) = demande.try_lock() {
                // `take()` vide la boîte en récupérant son contenu : la
                // demande est consommée atomiquement, sans drapeau à
                // remettre à zéro.
                if let Some(r) = boite.take() {
                    // La pose courante existe-t-elle encore dans le nouveau
                    // manifeste ? Si l'utilisateur vient de la renommer ou de
                    // la retirer, `set_pose` refuserait tout changement et le
                    // personnage resterait figé sur une clé morte.
                    if !r.manifeste.has_pose(&ch.pose) {
                        ch.pose = crate::character::manifest::POSE_STAND.to_string();
                        ch.pose_depuis = maintenant;
                    }

                    ch.manifest = r.manifeste;
                    reglages = r.reglages;

                    // Le webview doit oublier ses images, et la taille de la
                    // fenêtre peut avoir changé (`frameSize`, `scale`).
                    let _ = render::recharger(&handle, &label, r.version);
                    derniere_taille = None;
                    dernier_rendu = None;

                    println!("personnage rechargé (version {})", r.version);
                }
            }

            dernier_recensement = maintenant;
        }
```

`reglages` devient donc `mut` dans la boucle, et `demande: Demande` un
paramètre de plus.

- [ ] **Step 5 : Brancher l'entrée du tray**

```rust
                ID_RECHARGER => {
                    match crate::rechargement::preparer(
                        &demande_pour_evenement,
                        &dossier_a_ouvrir,
                        &personnage_a_recharger,
                    ) {
                        Ok(v) => println!("rechargement demandé (version {v})"),
                        // Bruyant : un rechargement silencieusement raté est
                        // le pire des cas — on croit tester son nouveau
                        // timing et on regarde l'ancien.
                        Err(e) => eprintln!("rechargement impossible : {e}"),
                    }
                }
```

`installer` prend donc deux paramètres de plus : la `Demande` et le nom du
personnage.

- [ ] **Step 6 : Vérifier à l'œil — c'est là que la tâche se juge**

```powershell
cd src-tauri
cargo run
```

Puis, **sans arrêter l'application** :

| À faire | Attendu |
|---|---|
| éditer `characters/blob/mascot.json`, mettre `walk.frameMs` à 60 | rien encore |
| tray → « Recharger les personnages » | `personnage rechargé (version 1)`, et **l'animation de marche s'accélère aussitôt** |
| remettre 240, recharger | elle ralentit |
| mettre `walk.frames` à `[11]` (la pose assise) | il marche en position assise — la preuve que les images sont relues |
| mettre un numéro de frame inexistant, ex. `[999]` | `manifeste illisible, rien n'a changé` **ou** la pose est retirée et il ne marche plus ; dans les deux cas **l'application ne plante pas** |
| casser le JSON (virgule en trop), recharger | `rechargement impossible : …` et **le personnage continue avec l'ancien manifeste** |

> ⚠️ **Le dernier point est le plus important de la tâche.** Un rechargement
> qui échoue doit laisser le personnage vivant avec ce qu'il avait. C'est
> précisément parce qu'on va éditer ce fichier des dizaines de fois qu'un
> échec doit être sans conséquence.

Remettre `mascot.json` dans son état correct avant de commiter — `git diff`
le dira.

- [ ] **Step 7 : Commit**

```bash
git add src-tauri/src ui/pet.js
git commit -m "feat(etape-1b): le rechargement à chaud des personnages

Cette tâche pèse plus qu'il n'y paraît (spec §9.1) : c'est elle qui rend
supportable le réglage des animations. Sans elle, chaque ajustement de timing
dans mascot.json coûte un redémarrage — et le réglage du manifeste sera
l'essentiel du travail de finition.

Le schéma URI de l'étape 1a la rend presque gratuite : les images étant
servies depuis le disque à chaque requête, il ne restait qu'à contourner le
cache du webview. On change l'URL (?v=3) plutôt que le cache, et le
gestionnaire Rust n'a rien à changer puisqu'il lit uri().path(), qui ignore
la requête.

Un Mutex, le seul du projet, et pour une raison précise : le manifeste est
possédé par le Character qui vit dans le thread de la boucle, alors que le
clic de tray arrive sur le thread principal de Tauri.

Le verrou n'est JAMAIS tenu pendant une entrée-sortie : le tray lit le
disque, puis pose le résultat. Et la boucle fait try_lock et non lock —
elle ne doit jamais attendre, quitte à réessayer dans 125 ms.

Option + take() plutôt qu'un drapeau : la demande est consommée
atomiquement, sans état à remettre à zéro.

Si la pose courante a disparu du nouveau manifeste, on repasse à stand :
set_pose refuse les poses absentes, donc le personnage resterait figé sur une
clé morte.

Un rechargement qui échoue laisse le personnage vivant avec ce qu'il avait,
et le dit bruyamment. C'est le point le plus important : on va éditer ce
fichier des dizaines de fois, et un échec silencieux ferait croire qu'on
teste un nouveau timing alors qu'on regarde l'ancien."
```

---

## Tâche 6 : Retirer la console, et régler le CPU

**Files:**
- Modify: `src-tauri/src/main.rs` (`windows_subsystem`, optimisations)
- Modify: `src-tauri/src/tray.rs` (drapeau de visibilité)
- Modify: `CLAUDE.md` (les mesures)

**Interfaces:**
- Produit : `tray::Visibilite` = `Arc<AtomicBool>`

> **Deux choses dans la même tâche parce qu'elles se vérifient ensemble** :
> retirer la console et optimiser touchent tous deux la boucle, et la mesure
> finale doit porter sur le résultat des deux.

> ### ⚠️ Steps 1 et 2 déjà faits, hors plan, le 2026-09-09
>
> La mesure et l'optimisation n° 1 ont été réalisées avant l'exécution de ce
> plan, à la demande de l'auteur. **Trois hypothèses fausses de suite** en
> sont sorties, et la méthodologie qui les a démenties est consignée dans
> `CLAUDE.md`, section « La méthodologie AVANT les chiffres ».
>
> L'acquis :
>
> | | CPU |
> |---|---|
> | fenêtre seule, aucune boucle | **0 %** |
> | `set_position` à chaque image | **21 %** |
> | `set_position` seulement si la position a changé | **12,3 %** |
>
> Et la leçon : **mesurer sur 10 s ne veut rien dire.** Le taux de
> déplacement varie de 0 % à 87 % des images selon ce que fait le personnage,
> donc deux mesures courtes sur la même version donnent 12 % et 25 %.
> Toujours 40 à 60 secondes.
>
> **Il reste donc à faire, dans cette tâche :** le Step 3 (ne rien dessiner
> quand c'est caché — le plus gros gain restant), le Step 4 (retirer la
> console) et une remesure propre du `release` sur 60 s.

- [ ] **Step 1 : ~~Mesurer le `release` AVANT de toucher à quoi que ce soit~~ — remesurer sur 60 s**

```powershell
cd src-tauri
cargo build --release
Start-Process .\target\release\shimeji-desktop.exe
Start-Sleep -Seconds 5
$p = Get-Process -Name shimeji-desktop
$c = $p.CPU; Start-Sleep -Seconds 10; $p.Refresh()
"release, avant optimisation : $([math]::Round((($p.CPU - $c) / 10) * 100, 1)) %"
```

**Consigner le chiffre.** Sans cette mesure, toute optimisation qui suit est
une croyance. La compilation est longue — LTO et `codegen-units = 1`.

- [x] **Step 2 : N'appeler `set_position` que si la position a changé** — **FAIT**, 21 % → 12,3 %.

C'était la piste n° 1 de `CLAUDE.md`. Le code ci-dessous est en place ; il est
gardé ici pour que le plan reste lisible d'un bout à l'autre.

```rust
    // Position entière effectivement posée à la dernière image.
    let mut dernier_coin: Option<(i32, i32)> = None;
```

```rust
                let coin_entier = (coin.x.round() as i32, coin.y.round() as i32);

                // `set_position` ne prend que des entiers : deux positions
                // qui s'arrondissent au même pixel produisent le même appel.
                // À l'arrêt, ce sont 60 appels par seconde pour rien.
                if dernier_coin != Some(coin_entier) {
                    if render::placer(&handle, &label, coin).is_err() {
                        return;
                    }
                    dernier_coin = Some(coin_entier);
                }
```

- [ ] **Step 3 : Ne rien dessiner quand les personnages sont cachés**

C'est la piste n° 2, et la plus rentable : caché, il n'y a **rien** à
afficher.

Dans `tray.rs` :

```rust
/// Partagé entre le tray et les boucles : les personnages sont-ils visibles ?
///
/// `AtomicBool` et non `Mutex<bool>` : c'est un seul booléen lu 60 fois par
/// seconde et écrit à la main. Un atomique se lit sans verrou et ne peut pas
/// être empoisonné.
pub type Visibilite = std::sync::Arc<std::sync::atomic::AtomicBool>;
```

Le gestionnaire de `ID_AFFICHER` l'écrit, la boucle le lit :

```rust
        // ── Caché : on saute tout le rendu ──────────────────────────────
        //
        // Le comportement continue de tourner — il doit avancer pour qu'on
        // le retrouve ailleurs en le réaffichant, et c'est du calcul pur,
        // donc quasi gratuit. Ce qui coûte, c'est de déplacer une fenêtre en
        // couche : c'est exactement ce qu'on saute.
        //
        // `Ordering::Relaxed` : il n'y a aucune autre donnée à synchroniser
        // avec ce booléen, seulement sa propre valeur. Un ordre plus fort
        // n'apporterait rien qu'un coût.
        let visible = visibilite.load(std::sync::atomic::Ordering::Relaxed);

        if visible {
            // … le placement et le rendu, tels quels …
        } else {
            // On oublie la dernière frame poussée : au retour, il faut
            // repousser même si rien n'a changé, puisque le webview a pu
            // être masqué entre-temps.
            dernier_rendu = None;
            dernier_coin = None;
        }
```

- [ ] **Step 4 : Retirer la console**

```rust
// Pas de console en release, mais on la garde en debug.
//
// `cfg_attr(not(debug_assertions), …)` plutôt que l'attribut nu : le mode
// simulation (`--sim`) imprime sa trace sur la sortie standard, et c'est un
// outil de développement. Le priver de console en debug le rendrait muet.
//
// Possible **seulement maintenant** : sans « Quitter » dans le tray, une
// application sans console ne se fermerait plus du tout.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
```

> ⚠️ Un attribut `#![…]` de niveau *crate* doit être **la première chose du
> fichier**, avant les commentaires de module et les `mod`. Le placer après
> donne `inner attribute is not permitted following an outer attribute`.

- [ ] **Step 5 : Mesurer de nouveau, et consigner**

```powershell
cd src-tauri
cargo build --release
Start-Process .\target\release\shimeji-desktop.exe
# … la même mesure qu'au Step 1, visible puis caché …
```

Remplir le tableau de `CLAUDE.md`, section « Mesurer le CPU » :

```markdown
| Configuration | CPU |
|---|---|
| debug, `set_position` seul à 60 Hz | ~14 % |
| **spike de l'étape 0**, qui ne fait que déplacer une fenêtre | ~18 % |
| release, avant optimisation | <à remplir> |
| release, position posée seulement si elle change | <à remplir> |
| release, personnages cachés | <à remplir> |
```

Et **retirer de `CLAUDE.md` les pistes appliquées**, en gardant la troisième
(suspendre à la session verrouillée) qui appartient à l'étape 2.

> **Si le release ne gagne presque rien sur le debug**, ce ne serait pas
> surprenant : le coût est dans la composition alpha de Windows, pas dans
> notre code. Le profil est réglé pour la **taille** (`opt-level = "z"`), pas
> la vitesse. Consigner le fait plutôt que de partir changer le profil — la
> taille de l'exe est un objectif de la spec §4, pas un détail.

- [ ] **Step 6 : Vérifier que « Quitter » marche toujours en release**

```powershell
Start-Process .\target\release\shimeji-desktop.exe
# tray -> Quitter
Get-Process -Name shimeji-desktop -ErrorAction SilentlyContinue
```

Attendu : **aucun processus.** C'est la vérification qui valide le retrait de
la console — sans elle, on livrerait une application impossible à fermer.

- [ ] **Step 7 : Commit**

```bash
git add src-tauri/src CLAUDE.md
git commit -m "perf(etape-1b): console retirée, et le CPU mesuré puis réduit

Deux choses ensemble parce qu'elles se vérifient ensemble : les deux touchent
la boucle, et la mesure finale doit porter sur leur résultat.

Mesuré AVANT de toucher à quoi que ce soit. Sans cette mesure, toute
optimisation qui suit est une croyance — et on en a déjà fait l'expérience à
l'étape 1a en accusant set_size à tort.

Deux optimisations, dans l'ordre de rentabilité :

- set_position n'est appelée que si la position ARRONDIE a changé. L'API ne
  prend que des entiers, donc deux positions qui tombent sur le même pixel
  produisent le même appel. À l'arrêt, ce sont 60 appels système par seconde
  pour rien.
- Caché, on saute tout le rendu. Le comportement continue de tourner — il
  doit avancer pour qu'on le retrouve ailleurs en le réaffichant, et c'est du
  calcul pur donc quasi gratuit. Ce qui coûte, c'est de déplacer une fenêtre
  en couche, et c'est exactement ce qu'on saute.

AtomicBool et non Mutex<bool> pour la visibilité : un seul booléen lu 60 fois
par seconde, qui se lit sans verrou et ne peut pas être empoisonné. Ordering
Relaxed parce qu'il n'y a aucune autre donnée à synchroniser avec lui.

La console ne disparaît qu'en release (cfg_attr sur debug_assertions) : le
mode --sim imprime sa trace, et le priver de console en debug le rendrait
muet. Et ce n'était possible QU'À PARTIR DE MAINTENANT — sans « Quitter »
dans le tray, une application sans console ne se fermerait plus du tout. Le
Step 6 le vérifie explicitement."
```

---

## Auto-revue

### Couverture de la spec §9

| Exigence | Où |
|---|---|
| §9.1 tray : afficher/cacher | Tâche 1 |
| §9.1 tray : ajouter / retirer un personnage | ⬜ **hors périmètre**, voir ci-dessous |
| §9.1 tray : ouvrir le dossier des personnages | Tâche 1 |
| §9.1 tray : recharger les personnages | Tâche 5 |
| §9.1 tray : démarrer avec Windows | Tâche 4 |
| §9.1 tray : quitter | Tâche 1 |
| §9.2 démarrage automatique, clé `Run` | Tâche 4 |
| §9.2 au démarrage : aucune fenêtre, aucun vol de focus | acquis depuis 1a par construction |
| §9.3 `config.json`, résolution des chemins | Tâche 2 |
| §9.3 contenu : personnages, échelle, vitesse, table d'envies | Tâches 2 et 3 |
| §9.3 modificateurs par application | ⬜ **étape 2** — ils dépendent du signal « appli au premier plan » |
| §9.3 absence de fichier ≠ erreur | Tâche 2, deux tests |

**« Ajouter / retirer un personnage » est écarté de ce plan**, et pas oublié :
ajouter un personnage est une **opération de contenu** (déposer un dossier
dans `characters/`, `CLAUDE.md`). L'entrée de menu ne ferait qu'ouvrir le
dossier — ce que « Ouvrir le dossier des personnages » fait déjà — puis
recharger, ce que « Recharger » fait déjà. Une entrée qui en combine deux
autres n'apporte rien tant qu'un seul personnage est instancié ; elle prendra
son sens à l'**étape 3**, avec plusieurs personnages et une liste à composer.

### Ce qui reste incertain

**Un seul point, et il est isolé** : le `PoisonError` du `Mutex` de la
Tâche 5. Il ne peut survenir que si le thread de la boucle a paniqué en tenant
le verrou — auquel cas le rechargement est le moindre des soucis, et le
message le dit. Aucun autre `unwrap` n'est introduit par ce plan.

### Cohérence des types

- `Reglages` est construit une fois au démarrage et à chaque rechargement,
  jamais consulté globalement — même discipline que l'horloge et l'aléatoire.
- `Entrees::echelle_ecran` devient `echelle_affichage` en Tâche 3, et le
  renommage touche aussi `attach::*`.
- `tray::installer` gagne deux paramètres en Tâche 5 (la `Demande`, le nom du
  personnage) et son quatrième, `demarrage_actif`, cesse d'être `false` en dur
  en Tâche 4.
- `behavior::pas` gagne `&Reglages` en Tâche 3 — c'est le seul changement de
  signature du cœur, et il ne touche ni `reflex` ni `desire`.

---

## Annexe — ce que l'étape 2 prendra

Pour situer, et pour que rien de tout ceci ne fuite dans 1b.

| Contenu | Fichiers |
|---|---|
| Les signaux : inactivité, appli au premier plan, heure, batterie, verrouillage | `signals.rs`, `probe/` étendu |
| Les modificateurs par application dans `config.json` | `config.rs` + `desire.rs` |
| `tirer_avec` branché sur les signaux — **le point d'entrée existe déjà** | `behavior/mod.rs`, une ligne |
| S'endormir, se réveiller, manger | `intention.rs` |
| Suspendre la boucle quand la session est verrouillée | `main.rs` |

> ⚠️ **Une inconnue de contenu attend l'étape 2**, consignée dans
> `docs/specs/2026-09-09-frames-shimeji.md` : **Shimeji-ee n'a aucune
> animation de sommeil.** Les frames 38-41 que la spec croyait être
> « s'asseoir puis dormir » appartiennent à `PullUpShimeji`. Trancher avant
> d'écrire le plan de l'étape 2 — la recommandation est `sprawl` (21),
> déclarée sous le nom `sleep` pour que le code ignore qu'il s'agit d'un
> substitut.
