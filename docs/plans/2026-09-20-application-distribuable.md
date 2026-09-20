# Application distribuable — plan d'implémentation

> **Pour les exécutants agentiques :** SOUS-COMPÉTENCE REQUISE — utiliser
> `superpowers:subagent-driven-development` (recommandé) ou
> `superpowers:executing-plans` pour exécuter ce plan tâche par tâche. Les
> étapes sont des cases à cocher (`- [ ]`).

**But :** qu'une personne sans Rust, sans Cargo et sans environnement de
développement puisse installer shimeji-desktop, être accueillie une fois, et
retrouver ses préférences à chaque démarrage.

**Architecture :** on n'invente aucun mécanisme. L'installateur est celui de
Tauri (NSIS), à qui il ne manque qu'une clé `resources`. L'assistant est une
fenêtre comme celle du catalogue, avec sa capability, qui appelle deux
commandes. Le démarrage automatique passe par `autostart.rs` **et le
registre**, jamais par la configuration. Les trois écrans de démarrage
choisissent entre trois chemins déjà écrits (`ouvrir_catalogue`, rien,
`visibilite = false`).

**Stack :** Rust · Tauri 2.11.5 · `tauri-cli 2.11.4` (déjà installé) · NSIS ·
`tauri-plugin-notification` 2 (à ajouter) · HTML/CSS/JS statique, sans bundler.

**Spec :** `docs/specs/2026-09-20-application-distribuable-design.md` — à lire
**avant** ce plan. Le plan applique la spec ; il ne la remplace pas.

## Contraintes globales

Copiées de la spec et de CLAUDE.md. Elles s'appliquent à **toutes** les tâches.

- **Compiler depuis PowerShell**, jamais depuis Git Bash : rustc y pêche le
  `link.exe` de Git for Windows au lieu du linker MSVC, et l'erreur
  (`extra operand`) est opaque.
- **Ne jamais éditer un fichier source par `Get-Content`/`Set-Content`.** Les
  sources sont en UTF-8 accentué ; PowerShell 5.1 les relit en cp1252 et les
  double-encode en ajoutant un BOM. Le code compile et les tests passent —
  c'est ce qui rend le défaut invisible. Passer par un outil d'édition.
- **Borner les sorties de build** : `cargo test --quiet`, et
  `cargo build 2>&1 | Select-Object -Last 40`.
- **Commenter abondamment, en français**, le *pourquoi* et non le *quoi*.
  Citer la section de la spec que le bloc applique. Expliquer les
  constructions Rust non élémentaires (`let … else`, `Arc`/`Mutex`, durées de
  vie, combinateurs).
- **`cargo test` ne reconstruit pas l'exe.** Après une correction,
  `cargo build` avant de relancer l'application.
- **Un seul `on_menu_event` dans tout le programme.** Aucune tâche n'en ajoute.
- **Les clés de `config.json` sont en camelCase** (`#[serde(default,
  rename_all = "camelCase")]`, `config.rs:246`) :
  `premiereConfigurationFaite`, `ecranAuDemarrage`. Écrire le nom Rust
  produirait une clé que `Config` ne relirait jamais — donc un assistant qui
  revient à chaque lancement, sans erreur nulle part.
- **La vérité du démarrage automatique est le registre**, lu par
  `autostart::est_actif()`. Jamais la configuration.
- **Épingler `windows = "0.61"`** — aucune tâche ne touche à cette dépendance.
- `identifier` (`dev.local.shimeji-desktop`) et `productName`
  (`shimeji-desktop`) **ne changent pas** : ils fixent l'AppUserModelID, le nom
  de l'exe et l'identité de désinstallation.

## Structure des fichiers

| Fichier | Responsabilité | Tâche |
|---|---|---|
| `src-tauri/tauri.conf.json` | **modifié** — la clé `resources` | 1 |
| `src-tauri/src/config.rs` | **modifié** — `EcranDemarrage`, 2 clés, `ecrire_cles`, `chemin_d_ecriture`, `definir_onboarding` | 2 |
| `src-tauri/src/config_tests.rs` | **modifié** — 7 tests | 2 |
| `src-tauri/src/main.rs` | **modifié** — `.build()/.run()`, ouverture de l'assistant, application de l'écran, plugin | 3, 5, 6 |
| `src-tauri/src/tray.rs` | **modifié** — libellé de l'entrée | 3 |
| `src-tauri/src/actions.rs` | **modifié** — `pub(crate)` sur deux helpers, `appliquer_ecran`, `ouvrir_onboarding`, `resynchroniser_demarrage` | 4, 5 |
| `src-tauri/src/commandes.rs` | **modifié** — `onboarding_etat`, `onboarding_terminer` | 4 |
| `src-tauri/capabilities/onboarding.json` | **créé** — la capability de la fenêtre | 4 |
| `ui/onboarding.html` | **créé** — les trois écrans | 5 |
| `ui/onboarding.js` | **créé** — la navigation et les deux appels IPC | 5 |
| `src-tauri/src/toast.rs` | **créé** — une fonction, l'émission du toast | 6 |
| `src-tauri/Cargo.toml` | **modifié** — `tauri-plugin-notification` | 6 |
| `CLAUDE.md`, `config.exemple.json` | **modifiés** — §8 de la spec | 7 |

---

## Tâche 1 : l'installateur livre les personnages

C'est le défaut que l'installateur non testé aurait produit : sans `resources`,
NSIS livre l'exe nu, `config::resoudre("characters")` ne trouve rien à côté de
l'exe, et l'application installée démarre avec **zéro personnage et aucun
moyen d'en obtenir un**.

**Fichiers :**
- Modifier : `src-tauri/tauri.conf.json`

**Interfaces :**
- Consomme : rien
- Produit : un setup NSIS qui contient `characters\blob\`. Aucune API Rust.

- [ ] **Étape 1 : constater le défaut — le script NSIS ne mentionne pas `characters`**

Depuis PowerShell :

```powershell
cd C:\Users\alri\Documents\shimeji-desktop\src-tauri
cargo tauri build 2>&1 | Select-Object -Last 20
```

Ce premier build télécharge NSIS (une fois) et prend plusieurs minutes.
Ensuite :

```powershell
Select-String -Path .\target\release\nsis\x64\installer.nsi -Pattern "characters" -SimpleMatch
```

Attendu : **aucune ligne**. C'est la preuve scriptable que le pack n'est pas
livré — on n'a pas besoin d'installer quoi que ce soit pour le voir.

- [ ] **Étape 2 : ajouter la clé `resources`**

`src-tauri/tauri.conf.json`, bloc `bundle` :

```json
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "icon": ["icons/icon.ico"],
    "resources": { "../characters": "characters" }
  }
```

La forme **map** (source → destination) est nécessaire : `tauri-utils` accepte
`BundleResources::List` ou `::Map` (`tauri-utils-2.9.3/src/config.rs:1508`), et
seule la seconde permet de poser `../characters` sous le nom `characters` à
côté de l'exe. Une liste `["../characters/**/*"]` recréerait l'arborescence
`../` dans le bundle.

204 Ko. Le dossier atterrit dans `Program Files`, donc **en lecture seule** —
et c'est l'état voulu : `%APPDATA%\shimeji-desktop\characters\` reste la seule
zone inscriptible, ce qui rend `config::est_dans_la_bibliotheque` vraie par
construction plutôt que par chance.

- [ ] **Étape 3 : rebuild et vérifier que le script NSIS livre bien le pack**

```powershell
cargo tauri build 2>&1 | Select-Object -Last 20
Select-String -Path .\target\release\nsis\x64\installer.nsi -Pattern "characters" -SimpleMatch
```

Attendu : **plusieurs lignes**, dont des `File` pour `mascot.json` et les PNG,
et des `Delete` correspondants dans la section de désinstallation.

```powershell
Get-ChildItem .\target\release\bundle\nsis\
```

Attendu : `shimeji-desktop_0.1.0_x64-setup.exe`.

- [ ] **Étape 4 : commit**

```bash
git add src-tauri/tauri.conf.json
git commit -m "fix(bundle): l'installateur livre characters/, sans quoi l'app installee n'a aucun personnage"
```

---

## Tâche 2 : l'état persisté

Deux clés ajoutées, une retirée, et la technique d'écriture chirurgicale
généralisée. **Tâche purement Rust, entièrement testable sans écran.**

**Fichiers :**
- Modifier : `src-tauri/src/config.rs`
- Test : `src-tauri/src/config_tests.rs`

**Interfaces :**
- Consomme : `config::lire_json`, `config::chemin_charge`,
  `config::ecrire_personnages` (existants)
- Produit :
  - `pub enum EcranDemarrage { Gestionnaire, Personnages, Tray }`
    (`Debug, Clone, Copy, PartialEq`)
  - `pub fn EcranDemarrage::en_json(&self) -> &'static str`
  - `pub fn EcranDemarrage::depuis_json(s: &str) -> EcranDemarrage`
  - `Config.premiere_configuration_faite: bool`
  - `Config.ecran_au_demarrage: EcranDemarrage`
  - `pub fn config::ecrire_cles(chemin: &Path, cles: &[(&str, serde_json::Value)]) -> Result<(), String>`
  - `pub fn config::definir_onboarding(fait: bool, ecran: EcranDemarrage) -> Result<(), String>`
  - **Supprimé :** `Config.demarrage_automatique`

- [ ] **Étape 1 : écrire les tests qui échouent**

À ajouter à la fin de `src-tauri/src/config_tests.rs` (le helper
`fichier_de_test` existe déjà en haut du fichier) :

```rust
// ── L'état de la première configuration (plan 2026-09-20, tâche 2) ──────

#[test]
fn par_defaut_la_premiere_configuration_n_est_pas_faite() {
    // C'est CE défaut qui fait s'ouvrir l'assistant au premier lancement,
    // y compris quand il n'existe aucun config.json (spec §2).
    let c = Config::default();
    assert!(!c.premiere_configuration_faite);
    assert_eq!(c.ecran_au_demarrage, EcranDemarrage::Personnages);
}

#[test]
fn les_deux_cles_se_relisent() {
    let f = fichier_de_test(
        r#"{ "premiereConfigurationFaite": true, "ecranAuDemarrage": "gestionnaire" }"#,
    );
    let c = charger_depuis(&f);
    assert!(c.premiere_configuration_faite);
    assert_eq!(c.ecran_au_demarrage, EcranDemarrage::Gestionnaire);
}

#[test]
fn un_ecran_inconnu_retombe_sur_personnages_sans_jeter_le_fichier() {
    // Le point délicat : `charger_depuis` jette TOUTE la configuration sur
    // une erreur de parsing (config.rs). Une valeur inconnue ne doit donc
    // pas être une erreur de parsing — d'où `ecran_tolerant` (spec §2).
    let f = fichier_de_test(
        r#"{ "echelle": 2.5, "ecranAuDemarrage": "sur-la-lune" }"#,
    );
    let c = charger_depuis(&f);
    assert_eq!(c.ecran_au_demarrage, EcranDemarrage::Personnages);
    assert_eq!(c.echelle, 2.5, "le reste du fichier doit survivre");
}

#[test]
fn une_config_portant_encore_demarrage_automatique_se_charge() {
    // Le champ a été supprimé (spec §0) : serde ignore les clés inconnues,
    // donc un fichier d'avant continue de marcher.
    let f = fichier_de_test(r#"{ "demarrageAutomatique": true, "echelle": 3 }"#);
    let c = charger_depuis(&f);
    assert_eq!(c.echelle, 3.0);
}

#[test]
fn ecrire_cles_preserve_les_voisines() {
    // Le cœur de la technique chirurgicale : on ne re-sérialise JAMAIS
    // `Config` par-dessus un fichier réglé à la main.
    let f = fichier_de_test(r#"{ "echelle": 2, "_note": "gardez-moi" }"#);
    ecrire_cles(
        &f,
        &[("premiereConfigurationFaite", serde_json::json!(true))],
    )
    .unwrap();

    let texte = lire_json(&f).unwrap();
    let v: serde_json::Value = serde_json::from_str(&texte).unwrap();
    assert_eq!(v["echelle"], 2);
    assert_eq!(v["_note"], "gardez-moi");
    assert_eq!(v["premiereConfigurationFaite"], true);
}

#[test]
fn ecrire_cles_cree_un_fichier_absent_et_refuse_un_illisible() {
    // Absent : cas NORMAL au premier lancement, on crée.
    let dossier = std::env::temp_dir().join("shimeji-test-cles-absent");
    std::fs::create_dir_all(&dossier).unwrap();
    let neuf = dossier.join("config.json");
    let _ = std::fs::remove_file(&neuf);
    ecrire_cles(&neuf, &[("ecranAuDemarrage", serde_json::json!("tray"))]).unwrap();
    assert_eq!(charger_depuis(&neuf).ecran_au_demarrage, EcranDemarrage::Tray);

    // Illisible : on n'écrit RIEN. L'écraser perdrait des réglages que
    // l'utilisateur croit avoir.
    let casse = fichier_de_test("{ ceci n'est pas du json");
    let avant = std::fs::read_to_string(&casse).unwrap();
    assert!(ecrire_cles(&casse, &[("echelle", serde_json::json!(9))]).is_err());
    assert_eq!(std::fs::read_to_string(&casse).unwrap(), avant);
}

#[test]
fn ce_qu_ecrit_ecrire_cles_est_relu_par_charger_depuis() {
    // LE test qui attrape une faute de camelCase. Comparer le JSON ne
    // l'attraperait pas : il faut faire l'aller-RETOUR complet.
    let f = fichier_de_test("{}");
    ecrire_cles(
        &f,
        &[
            ("premiereConfigurationFaite", serde_json::json!(true)),
            (
                "ecranAuDemarrage",
                serde_json::json!(EcranDemarrage::Tray.en_json()),
            ),
        ],
    )
    .unwrap();

    let c = charger_depuis(&f);
    assert!(
        c.premiere_configuration_faite,
        "clé mal nommée : l'assistant reviendrait à chaque lancement"
    );
    assert_eq!(c.ecran_au_demarrage, EcranDemarrage::Tray);
}
```

- [ ] **Étape 2 : lancer les tests pour les voir échouer**

```powershell
cd C:\Users\alri\Documents\shimeji-desktop\src-tauri
cargo test --quiet config 2>&1 | Select-Object -Last 30
```

Attendu : **échec de compilation** — `cannot find type EcranDemarrage`,
`no field premiere_configuration_faite`, `cannot find function ecrire_cles`.
C'est le bon échec : il nomme exactement ce qui manque.

- [ ] **Étape 3 : ajouter `EcranDemarrage` et son désérialiseur tolérant**

Dans `src-tauri/src/config.rs`, **avant** la déclaration de `Config` :

```rust
/// Ce qui s'ouvre au démarrage (spec §2).
///
/// Aucune de ces trois valeurs n'introduit de mécanisme : elles choisissent
/// entre trois chemins qui existent déjà — `actions::ouvrir_catalogue`, rien,
/// et la visibilité à `false` que `SHIMEJI_CACHE=1` pose déjà.
///
/// Pas de `#[derive(Deserialize)]` : la désérialisation passe par
/// `ecran_tolerant` ci-dessous, qui ne doit jamais échouer. Pas de
/// `Serialize` non plus — l'écriture passe par `en_json`, ce qui garde sous
/// les yeux le fait que les clés du fichier sont en camelCase.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EcranDemarrage {
    /// Les personnages vivent, ET la fenêtre du gestionnaire s'ouvre.
    Gestionnaire,
    /// Les personnages vivent, aucune fenêtre. C'est le comportement d'avant.
    Personnages,
    /// Les personnages démarrent CACHÉS — l'état exact que produisent
    /// `SHIMEJI_CACHE=1` et la case « Afficher » décochée.
    Tray,
}

impl EcranDemarrage {
    /// Le texte écrit dans `config.json`.
    ///
    /// `&'static str` : les trois chaînes sont des littéraux du programme,
    /// elles vivent aussi longtemps que lui. Rien à allouer.
    pub fn en_json(&self) -> &'static str {
        match self {
            EcranDemarrage::Gestionnaire => "gestionnaire",
            EcranDemarrage::Personnages => "personnages",
            EcranDemarrage::Tray => "tray",
        }
    }

    /// L'inverse, **total** : tout ce qui n'est pas reconnu vaut
    /// `Personnages`. C'est ce qui permet à `ecran_tolerant` de ne jamais
    /// échouer, et à `onboarding_terminer` d'accepter une chaîne venue du
    /// webview sans la valider deux fois.
    pub fn depuis_json(s: &str) -> EcranDemarrage {
        match s {
            "gestionnaire" => EcranDemarrage::Gestionnaire,
            "tray" => EcranDemarrage::Tray,
            _ => EcranDemarrage::Personnages,
        }
    }
}

/// Lit `ecranAuDemarrage` sans jamais échouer.
///
/// **Pourquoi pas `#[serde(other)]`** : serde ne l'accepte que sur les
/// énumérations *taguées* (`#[serde(tag = …)]`), pas sur une énumération
/// sérialisée en simple chaîne. Il faut donc un désérialiseur nommé.
///
/// On désérialise d'abord une `String` — ça n'échoue que si la valeur n'est
/// pas une chaîne du tout — puis on traduit nous-mêmes. Une valeur inconnue
/// devient `Personnages` avec un avertissement, au lieu de faire rejeter le
/// fichier ENTIER par `charger_depuis` (spec §2).
///
/// `D: Deserializer<'de>` et la durée de vie `'de` sont imposés par serde :
/// `'de` est celle des données d'entrée, que le désérialiseur peut emprunter.
fn ecran_tolerant<'de, D>(d: D) -> Result<EcranDemarrage, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let brut = String::deserialize(d)?;
    let choisi = EcranDemarrage::depuis_json(&brut);

    // Bruyant seulement quand la valeur était vraiment inconnue : dire
    // « personnages est inconnu » quand l'utilisateur a écrit
    // « personnages » serait un faux avertissement.
    if choisi == EcranDemarrage::Personnages && brut != "personnages" {
        eprintln!("ecranAuDemarrage : « {brut} » inconnu, « personnages » utilisé");
    }
    Ok(choisi)
}
```

`String::deserialize` exige que le trait soit dans la portée. `config.rs`
importe déjà `serde::Deserialize` (le `derive`) ; si le compilateur s'en
plaint, l'import à ajouter en tête est `use serde::Deserialize;`.

- [ ] **Étape 4 : modifier `Config` — deux champs ajoutés, un supprimé**

Dans la `struct Config`, remplacer la ligne :

```rust
    pub demarrage_automatique: bool,
```

par :

```rust
    /// L'assistant de première configuration a-t-il été mené à son terme ?
    ///
    /// **Un seul booléen pour deux effets** (spec §2) : il commande
    /// l'ouverture de l'assistant ET l'émission du toast. Le remettre à
    /// `false` à la main rejoue les deux — c'est la contrôlabilité par la
    /// configuration, sans seconde clé à tenir d'accord avec la première.
    ///
    /// ⚠️ La clé dans le fichier est `premiereConfigurationFaite` :
    /// `Config` est en `rename_all = "camelCase"`.
    pub premiere_configuration_faite: bool,

    /// Ce qui s'ouvre au démarrage. Clé : `ecranAuDemarrage`.
    #[serde(deserialize_with = "ecran_tolerant")]
    pub ecran_au_demarrage: EcranDemarrage,
```

> **`demarrage_automatique` disparaît**, et ce n'est pas un oubli. C'était du
> code mort : déclaré, initialisé, et **jamais lu ni écrit** (spec §0). La
> seule vérité du démarrage automatique est le registre, lu par
> `autostart::est_actif()`. Le garder inviterait le prochain lecteur à le
> brancher, et donc à créer une seconde vérité à tenir d'accord pour
> toujours — exactement ce que CLAUDE.md interdit pour l'interrupteur de la
> bibliothèque.
>
> Aucune rupture : serde ignore les clés inconnues, donc un `config.json`
> existant qui porte `"demarrageAutomatique": true` se charge sans bruit.

Dans `impl Default for Config`, remplacer :

```rust
            demarrage_automatique: false,
```

par :

```rust
            premiere_configuration_faite: false,
            ecran_au_demarrage: EcranDemarrage::Personnages,
```

- [ ] **Étape 5 : généraliser l'écriture chirurgicale**

Remplacer le **corps** de `ecrire_personnages` (`config.rs:532`, garder son
commentaire de documentation en l'adaptant) et ajouter `ecrire_cles` :

```rust
/// Remplace les clés nommées dans `chemin`, en laissant **tout** le reste.
///
/// Édition chirurgicale : on relit en `serde_json::Value`, on ne touche
/// qu'aux clés visées, on réécrit. Sérialiser depuis `Config` perdrait toutes
/// les clés inconnues et remettrait les valeurs par défaut partout (spec
/// §10) — l'utilisateur verrait son fichier réglé à la main écrasé par un
/// clic.
///
/// ⚠️ Les noms passés ici sont ceux du **fichier**, donc en camelCase
/// (`premiereConfigurationFaite`), et non ceux des champs Rust. Une faute
/// produit une clé que `Config` ne relira jamais, sans la moindre erreur.
///
/// Un fichier absent n'est pas une erreur : c'est le cas normal au premier
/// lancement, et on le crée. Un fichier présent mais ILLISIBLE, si —
/// l'écraser perdrait des réglages que l'utilisateur croit avoir.
pub fn ecrire_cles(chemin: &Path, cles: &[(&str, serde_json::Value)]) -> Result<(), String> {
    let mut valeur: serde_json::Value = match lire_json(chemin) {
        Ok(texte) => serde_json::from_str(&texte)
            .map_err(|e| format!("config.json illisible, rien n'est écrit : {e}"))?,
        Err(_) => serde_json::json!({}),
    };

    // `let … else` : `as_object_mut` rend `None` si la racine est un tableau
    // ou un scalaire. On n'aurait alors rien où insérer.
    let Some(objet) = valeur.as_object_mut() else {
        return Err("config.json n'est pas un objet JSON".to_string());
    };

    for (nom, contenu) in cles {
        // `clone` : `contenu` est emprunté à l'appelant, et `insert` veut la
        // propriété. Les valeurs sont minuscules (un booléen, une chaîne).
        objet.insert(nom.to_string(), contenu.clone());
    }

    // `to_string_pretty` : le fichier est édité à la main par l'auteur, une
    // seule ligne le rendrait pénible.
    let texte =
        serde_json::to_string_pretty(&valeur).map_err(|e| format!("sérialisation : {e}"))?;

    // `write` écrit en UTF-8 SANS BOM — c'est ce qu'il faut : `serde_json`
    // refuse le BOM avec le message trompeur « expected value at line 1
    // column 1 », et c'est nous qui relirions ce fichier.
    std::fs::write(chemin, texte).map_err(|e| format!("écriture de config.json : {e}"))
}

/// Remplace la clé `personnages` de `chemin`, en laissant tout le reste.
///
/// `personnages` est un **multi-ensemble** (design « plusieurs personnages »
/// §4) : `["blob", "blob"]` veut dire deux blob à l'écran, et les doublons
/// doivent survivre à l'aller-retour. La **liste vide est acceptée** : zéro
/// personnage est un état normal, l'application vit alors dans le tray.
pub fn ecrire_personnages(chemin: &Path, noms: &[String]) -> Result<(), String> {
    // `personnages` s'écrit pareil en snake_case et en camelCase — c'est un
    // hasard, pas une règle. Voir l'avertissement d'`ecrire_cles`.
    ecrire_cles(chemin, &[("personnages", serde_json::json!(noms))])
}
```

- [ ] **Étape 6 : factoriser le chemin d'écriture et ajouter `definir_onboarding`**

Remplacer `definir_personnages` (`config.rs:588`) par ces trois fonctions :

```rust
/// Où écrire la configuration : le fichier réellement chargé, ou un fichier
/// neuf dans `%APPDATA%`.
///
/// Jamais dans le dépôt : celui-ci peut être en lecture seule — et il l'est
/// forcément une fois l'application **installée**, son dossier étant sous
/// `Program Files`. C'est cette fonction qui rend une application installée
/// capable d'enregistrer ses préférences.
fn chemin_d_ecriture() -> Result<PathBuf, String> {
    if let Some(c) = chemin_charge() {
        return Ok(c);
    }

    // `let … else` : sans `%APPDATA%` il n'y a nulle part où écrire, et
    // c'est une vraie erreur — pas un cas à ignorer.
    let Ok(appdata) = std::env::var("APPDATA") else {
        return Err("%APPDATA% introuvable".to_string());
    };
    let dossier = PathBuf::from(appdata).join("shimeji-desktop");
    std::fs::create_dir_all(&dossier)
        .map_err(|e| format!("création de {} : {e}", dossier.display()))?;
    Ok(dossier.join("config.json"))
}

/// Enregistre la liste des personnages dans le `config.json` réellement
/// chargé, ou en crée un dans `%APPDATA%` s'il n'y en avait aucun.
pub fn definir_personnages(noms: &[String]) -> Result<(), String> {
    ecrire_personnages(&chemin_d_ecriture()?, noms)
}

/// Enregistre le résultat de l'assistant de première configuration.
///
/// Les deux clés d'un seul coup : elles sont écrites au même instant, et un
/// seul appel veut dire une seule relecture-réécriture du fichier.
pub fn definir_onboarding(fait: bool, ecran: EcranDemarrage) -> Result<(), String> {
    ecrire_cles(
        &chemin_d_ecriture()?,
        &[
            ("premiereConfigurationFaite", serde_json::json!(fait)),
            ("ecranAuDemarrage", serde_json::json!(ecran.en_json())),
        ],
    )
}
```

- [ ] **Étape 7 : lancer les tests**

```powershell
cargo test --quiet 2>&1 | Select-Object -Last 30
```

Attendu : **tout passe**, les 299 tests existants plus les 7 nouveaux.

Si un test existant échoue sur `demarrage_automatique`, c'est qu'il
référençait le champ supprimé : le retirer du test, pas le remettre dans
`Config`.

- [ ] **Étape 8 : commit**

```bash
git add src-tauri/src/config.rs src-tauri/src/config_tests.rs
git commit -m "feat(config): l'etat de la premiere configuration, et ecrire_cles

demarrage_automatique est supprime : code mort, jamais lu ni ecrit.
La verite du demarrage auto est le registre (autostart::est_actif)."
```

---

## Tâche 3 : fermer le gestionnaire ne quitte plus

**Fichiers :**
- Modifier : `src-tauri/src/main.rs` (la fin de `main`, `main.rs:688`)
- Modifier : `src-tauri/src/tray.rs:130-137`

**Interfaces :**
- Consomme : rien
- Produit : rien de nouveau côté API. `ID_CATALOGUE` ne change pas.

- [ ] **Étape 1 : empêcher la sortie sur fermeture de fenêtre**

Dans `main.rs`, remplacer la fin de la chaîne :

```rust
        .run(tauri::generate_context!())
        .expect("échec au lancement de l'application Tauri");
```

par :

```rust
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
            if let tauri::RunEvent::ExitRequested { api, code: None, .. } = evenement {
                api.prevent_exit();
            }
        });
```

- [ ] **Étape 2 : renommer l'entrée du tray**

`tray.rs:133`, remplacer `"Catalogue de personnages…"` par
`"Ouvrir le gestionnaire…"`.

L'identifiant `ID_CATALOGUE` **ne change pas** : c'est lui qui relie les deux
menus à `actions::executer`, et il est aussi porté par le menu du personnage.
Seul le libellé bouge, parce que cette fenêtre est désormais *la* fenêtre
principale et pas seulement un catalogue.

- [ ] **Étape 3 : compiler et vérifier que rien n'a cassé**

```powershell
cargo test --quiet 2>&1 | Select-Object -Last 20
cargo build 2>&1 | Select-Object -Last 40
```

Attendu : compilation propre, tests au vert.

- [ ] **Étape 4 : vérifier le comportement, sans humain pour la moitié**

```powershell
$env:SHIMEJI_CATALOGUE=1; $env:SHIMEJI_QUITTER_APRES=20; cargo run
```

`SHIMEJI_QUITTER_APRES` appelle `exit(0)` — **la ligne exacte** de l'entrée
« Quitter ». Attendu : le processus disparaît au bout de 20 s. C'est la preuve
que `code: Some(_)` traverse bien le filtre.

Puis, et c'est le seul geste humain de cette tâche : relancer avec
`$env:SHIMEJI_CATALOGUE=1; cargo run`, **fermer la fenêtre du gestionnaire à
la croix**, et vérifier que le personnage continue de vivre et que l'icône du
tray est toujours là.

```powershell
Get-Process -Name shimeji-desktop
```

Attendu : le processus existe toujours.

- [ ] **Étape 5 : commit**

```bash
git add src-tauri/src/main.rs src-tauri/src/tray.rs
git commit -m "feat(cycle de vie): fermer le gestionnaire ne quitte plus l'application

Le filtre code: None laisse passer app.exit(0) du menu Quitter."
```

---

## Tâche 4 : les deux commandes de l'assistant

Le Rust de l'assistant, **avant** son interface. Cette tâche se termine sur un
programme qui compile et dont les commandes sont appelables — mais aucune
fenêtre ne les appelle encore. C'est voulu : la tâche 5 n'aura plus qu'à
poser du HTML.

**Fichiers :**
- Modifier : `src-tauri/src/actions.rs`
- Modifier : `src-tauri/src/commandes.rs`
- Modifier : `src-tauri/src/main.rs` (le `generate_handler!`, `main.rs:362`)
- Créer : `src-tauri/capabilities/onboarding.json`

**Interfaces :**
- Consomme : `config::EcranDemarrage`, `config::definir_onboarding` (tâche 2) ;
  `autostart::est_actif/activer/desactiver` ; `actions::Actions`,
  `actions::CasesTray`, `actions::ouvrir_catalogue` (existants)
- Produit :
  - `pub(crate) fn actions::appliquer_demarrage(voulu: bool) -> bool`
    (passe de privé à `pub(crate)`)
  - `pub(crate) fn actions::appliquer_visibilite(actions: &Actions, app: &AppHandle, visible: bool)`
    (idem)
  - `pub fn Actions::resynchroniser_demarrage(&self, actif: bool)`
  - `pub fn actions::appliquer_ecran(actions: &Actions, app: &AppHandle, ecran: EcranDemarrage)`
  - `pub struct commandes::EtatOnboarding { demarrage_auto: bool }` (`Serialize`,
    donc `demarrageAuto` côté JS)
  - `#[tauri::command] pub fn commandes::onboarding_etat() -> EtatOnboarding`
  - `#[tauri::command] pub fn commandes::onboarding_terminer(app, actions, demarrage_auto: bool, ecran: String) -> Result<(), String>`

- [ ] **Étape 1 : ouvrir les deux helpers d'`actions.rs`**

`actions.rs:319` et `actions.rs:330` : remplacer `fn appliquer_visibilite` par
`pub(crate) fn appliquer_visibilite` et `fn appliquer_demarrage` par
`pub(crate) fn appliquer_demarrage`.

`pub(crate)` et non `pub` : ces deux fonctions sont le geste **interne** que
partagent le menu et l'assistant ; rien hors du binaire n'a à les voir.

- [ ] **Étape 2 : remettre la case « démarrage » du tray d'accord**

Dans `impl Actions` (`actions.rs`, à côté de `resynchroniser_affichage`) :

```rust
    /// Remet la case « Démarrer avec Windows » du tray d'accord avec la
    /// réalité.
    ///
    /// Le jumeau exact de `resynchroniser_affichage`, et pour la même raison :
    /// quand l'assistant active le démarrage automatique, la case du tray doit
    /// suivre. Sinon l'utilisateur coche dans l'assistant, ouvre le tray, et y
    /// lit « Démarrer avec Windows » décoché — un mensonge affiché en
    /// permanence.
    pub fn resynchroniser_demarrage(&self, actif: bool) {
        let Ok(cases) = self.cases.lock() else {
            return;
        };
        // `let … else` : sans tray installé, il n'y a rien à resynchroniser.
        let Some(cases) = cases.as_ref() else {
            return;
        };

        let _ = cases.demarrage.set_checked(actif);
    }
```

- [ ] **Étape 3 : appliquer un écran de démarrage**

Toujours dans `actions.rs`, fonction libre, à côté d'`ouvrir_catalogue` :

```rust
/// Applique un écran de démarrage (spec §2).
///
/// **Aucun mécanisme nouveau** : les trois branches appellent du code qui
/// existait déjà. C'est ce qui rend ce réglage presque gratuit.
///
/// Appelée à DEUX endroits, et c'est pour cela qu'elle est une fonction :
/// au démarrage (`main.rs`, d'après la config) et à la fin de l'assistant,
/// pour que le choix se voie tout de suite au lieu d'attendre le prochain
/// lancement — un réglage qui ne fait rien tant qu'on n'a pas redémarré
/// paraît cassé.
pub fn appliquer_ecran(actions: &Actions, app: &AppHandle, ecran: crate::config::EcranDemarrage) {
    use crate::config::EcranDemarrage;

    match ecran {
        EcranDemarrage::Gestionnaire => ouvrir_catalogue(app),

        // Rien à faire : les personnages vivent déjà, aucune fenêtre ne
        // s'ouvre. C'est le comportement de toutes les versions d'avant.
        EcranDemarrage::Personnages => {}

        // Exactement ce que fait `SHIMEJI_CACHE=1`, et exactement ce que fait
        // décocher « Afficher » dans le tray — d'où l'appel au même helper,
        // qui remet aussi la case d'accord.
        EcranDemarrage::Tray => appliquer_visibilite(actions, app, false),
    }
}
```

- [ ] **Étape 4 : les deux commandes**

À la fin de `src-tauri/src/commandes.rs` :

```rust
// ── L'assistant de première configuration (spec §3) ─────────────────────

/// Ce que l'assistant affiche à son ouverture.
///
/// `Serialize` seulement : la donnée ne circule que de Rust vers le webview.
/// Tauri sérialise les champs en camelCase côté JS — `demarrage_auto` se lit
/// donc `demarrageAuto` dans `onboarding.js`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EtatOnboarding {
    pub demarrage_auto: bool,
}

/// L'état à afficher à l'ouverture de l'assistant.
///
/// ⚠️ Le REGISTRE et non la configuration (spec §0). Le cas existe vraiment :
/// quelqu'un qui réinstalle par-dessus une version où il avait activé le
/// démarrage automatique doit retrouver la case cochée.
#[tauri::command]
pub fn onboarding_etat() -> EtatOnboarding {
    EtatOnboarding {
        demarrage_auto: crate::autostart::est_actif(),
    }
}

/// Applique les choix de l'assistant et le marque comme fait.
///
/// L'ordre des cinq gestes compte, et il est décrit dans la spec §3 :
/// registre, configuration, écran, toast, fermeture.
///
/// `ecran` arrive en `String` depuis le webview plutôt qu'en énumération :
/// `EcranDemarrage::depuis_json` est **totale** (tout inconnu vaut
/// `Personnages`), donc il n'y a rien à valider deux fois, et une
/// désérialisation qui échoue côté Tauri donnerait un message illisible.
#[tauri::command]
pub fn onboarding_terminer(
    app: AppHandle,
    actions: tauri::State<'_, std::sync::Arc<crate::actions::Actions>>,
    demarrage_auto: bool,
    ecran: String,
) -> Result<(), String> {
    // ── 1. Le registre ──────────────────────────────────────────────────
    // `appliquer_demarrage` rend l'état RÉELLEMENT obtenu, qui diffère du
    // voulu si l'écriture a échoué.
    let reel = crate::actions::appliquer_demarrage(demarrage_auto);
    actions.resynchroniser_demarrage(reel);

    // ── 2. La configuration ─────────────────────────────────────────────
    // Écrite AVANT d'appliquer l'écran : si elle échoue, on veut le dire
    // sans avoir déjà changé l'affichage.
    //
    // C'est le seul échec FATAL au sens de l'assistant : ne pas pouvoir
    // écrire la clé, c'est un assistant qui revient au prochain lancement,
    // et mieux vaut le dire que le laisser boucler en silence.
    let choisi = crate::config::EcranDemarrage::depuis_json(&ecran);
    crate::config::definir_onboarding(true, choisi)?;

    // ── 3. L'écran choisi, tout de suite ────────────────────────────────
    // `&**actions` : `actions` est une `State<Arc<Actions>>`. Un premier
    // déréférencement donne l'`Arc<Actions>`, un second l'`Actions` — et on
    // en reprend une référence. Écrit explicitement plutôt que de compter
    // sur deux coercitions enchaînées, qui compilent mal selon le contexte.
    crate::actions::appliquer_ecran(&**actions, &app, choisi);

    // ── 4. Le toast — branché à la tâche 6 ──────────────────────────────

    // ── 5. La fenêtre se ferme ──────────────────────────────────────────
    // `if let Some` : si elle a déjà été fermée à la croix pendant l'appel,
    // il n'y a rien à fermer et ce n'est pas une erreur.
    if let Some(fenetre) = tauri::Manager::get_webview_window(&app, "onboarding") {
        let _ = fenetre.close();
    }

    // ⚠️ Si le registre a refusé, on le signale MAINTENANT — après avoir
    // tout le reste enregistré. L'assistant ne doit pas se rejouer pour ça,
    // mais le webview doit décocher la case plutôt que d'afficher un état
    // qui n'est pas celui du registre (précédent d'`ID_DEMARRAGE`).
    if reel != demarrage_auto {
        return Err("le démarrage avec Windows n'a pas pu être enregistré".to_string());
    }
    Ok(())
}
```

> ⚠️ **L'erreur est rendue APRÈS la fermeture de la fenêtre.** C'est
> délibéré : le geste principal (enregistrer la configuration) a réussi, seul
> l'accessoire a échoué. Renvoyer l'erreur avant laisserait l'assistant ouvert
> avec une configuration déjà écrite — donc un assistant qui ne se rouvrirait
> plus jamais s'il était fermé à la croix.

- [ ] **Étape 5 : enregistrer les commandes**

`main.rs:362`, dans `generate_handler!` :

```rust
        .invoke_handler(tauri::generate_handler![
            commandes::installer,
            commandes::bibliotheque,
            commandes::definir_compte,
            commandes::supprimer,
            commandes::onboarding_etat,
            commandes::onboarding_terminer
        ])
```

> ⚠️ Une commande oubliée ici est introuvable côté JS **sans erreur de
> compilation** — l'avertissement est déjà écrit à côté de la macro.

- [ ] **Étape 6 : la capability**

Créer `src-tauri/capabilities/onboarding.json` :

```json
{
  "identifier": "onboarding",
  "description": "Permet a la fenetre de l'assistant de premiere configuration d'appeler Rust. Portee a CETTE fenetre, comme celle du catalogue.",
  "windows": ["onboarding"],
  "permissions": ["core:default"]
}
```

> ⚠️ Le champ `windows` doit contenir **exactement** le label utilisé à la
> création de la fenêtre (tâche 5). Une divergence fait refuser les `invoke`
> **en silence** — le piège déjà payé sur le catalogue
> (`actions.rs:376`).

Pas de `core:event:allow-listen` ici, contrairement au catalogue : l'assistant
n'écoute aucun événement, il ne fait qu'appeler.

- [ ] **Étape 7 : compiler**

```powershell
cargo test --quiet 2>&1 | Select-Object -Last 20
cargo build 2>&1 | Select-Object -Last 40
```

Attendu : compilation propre. Les commandes ne sont encore appelées par
personne — c'est normal.

- [ ] **Étape 8 : commit**

```bash
git add src-tauri/src/actions.rs src-tauri/src/commandes.rs src-tauri/src/main.rs src-tauri/capabilities/onboarding.json
git commit -m "feat(onboarding): les deux commandes et la capability"
```

---

## Tâche 5 : la fenêtre de l'assistant

**Fichiers :**
- Créer : `ui/onboarding.html`
- Créer : `ui/onboarding.js`
- Modifier : `src-tauri/src/actions.rs` (`ouvrir_onboarding`)
- Modifier : `src-tauri/src/main.rs` (ouverture depuis `setup`)

**Interfaces :**
- Consomme : `commandes::onboarding_etat`, `commandes::onboarding_terminer`
  (tâche 4) ; `config::EcranDemarrage`, `Config.premiere_configuration_faite`
  (tâche 2) ; `actions::appliquer_ecran` (tâche 4)
- Produit : `pub fn actions::ouvrir_onboarding(app: &AppHandle)` ; la variable
  `SHIMEJI_ONBOARDING=1`

- [ ] **Étape 1 : l'ouverture de la fenêtre**

Dans `actions.rs`, juste après `ouvrir_catalogue` :

```rust
/// Ouvre l'assistant de première configuration (spec §3).
///
/// Une fenêtre d'application ordinaire, comme le catalogue — mais **non
/// redimensionnable** : ses trois écrans ont une taille fixe, et rien n'y
/// gagne à être étiré.
///
/// ⚠️ Le label `onboarding` doit correspondre EXACTEMENT à celui déclaré
/// dans `capabilities/onboarding.json`, sinon les appels `invoke` sont
/// refusés **en silence**.
pub fn ouvrir_onboarding(app: &AppHandle) {
    const LABEL: &str = "onboarding";

    // Déjà ouverte : on la remonte plutôt que d'en créer une seconde.
    if let Some(existante) = tauri::Manager::get_webview_window(app, LABEL) {
        let _ = existante.show();
        let _ = existante.set_focus();
        return;
    }

    let resultat = tauri::WebviewWindowBuilder::new(
        app,
        LABEL,
        tauri::WebviewUrl::App("onboarding.html".into()),
    )
    .title("Bienvenue")
    .inner_size(520.0, 460.0)
    .resizable(false)
    .center()
    .build();

    if let Err(e) = resultat {
        // On ne panique pas : ne pas pouvoir accueillir l'utilisateur n'est
        // pas une raison de tuer le personnage, qui lui tourne très bien.
        // L'assistant se représentera au prochain lancement, la clé n'ayant
        // pas été écrite.
        eprintln!("assistant : ouverture impossible — {e}");
    }
}
```

- [ ] **Étape 2 : l'ouvrir au premier lancement**

Dans `main.rs`, dans `setup`, **remplacer** le bloc `SHIMEJI_CATALOGUE`
(`main.rs:684`) par :

```rust
            // ── L'écran de démarrage, et l'assistant (spec §2, §3) ───────
            //
            // L'ordre est celui de la spec : au PREMIER lancement l'assistant
            // prime, et l'écran enregistré n'est appliqué qu'ensuite, par
            // `onboarding_terminer`. Appliquer les deux ouvrirait deux
            // fenêtres au tout premier démarrage.
            //
            // Les personnages, eux, vivent DÉJÀ : la boucle 60 Hz est lancée
            // plus haut. C'est délibéré (spec §3) — l'utilisateur voit
            // immédiatement ce qu'il a installé, au lieu d'un bureau vide en
            // se demandant si ça marche.
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
            // interface dessus.
            if std::env::var("SHIMEJI_CATALOGUE").is_ok() {
                actions::ouvrir_catalogue(&poignee);
            }
```

**Aucune variable à ajouter** : `configuration` est toujours vivante à cet
endroit de `setup`. La boucle 60 Hz en prend un **clone**
(`let configuration_boucle = configuration.clone();`, `main.rs:654`), pas la
propriété — le commentaire qui l'accompagne le dit déjà : « `setup` continue
de s'en servir plus haut ». Le bloc ci-dessus étant placé **après** la ligne
654, il peut lire `configuration.premiere_configuration_faite` directement.

- [ ] **Étape 3 : `ui/onboarding.html`**

Même facture que `catalogue.html` : jetons de couleur en tête, police système,
aucun framework, aucun bundler.

```html
<!doctype html>
<html lang="fr">
<head>
<meta charset="utf-8">
<title>Bienvenue</title>
<style>
  /* Les mêmes jetons que catalogue.html : les deux fenêtres appartiennent
     visiblement à la même application. */
  :root {
    --fond: rgb(36, 36, 36);
    --texte: rgb(201, 201, 201);
    --accent: rgb(120, 170, 255);
  }

  body {
    margin: 0;
    padding: 28px 32px;
    background: var(--fond);
    color: var(--texte);
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,
                 "Helvetica Neue", Arial, sans-serif;
    display: flex;
    flex-direction: column;
    height: 100vh;
    box-sizing: border-box;
  }

  h1 { font-size: 20px; margin: 0 0 4px; }
  p  { line-height: 1.5; margin: 0 0 16px; }
  .discret { opacity: 0.65; font-size: 13px; }

  /* Un seul écran visible à la fois. `hidden` seul ne suffit pas face à
     `display: flex`, d'où la règle explicite. */
  section { flex: 1; }
  section[hidden] { display: none; }

  fieldset {
    border: 1px solid rgba(255, 255, 255, 0.15);
    margin: 0 0 18px;
    padding: 12px 14px;
  }
  legend { padding: 0 6px; font-size: 13px; opacity: 0.8; }
  label { display: block; padding: 5px 0; cursor: pointer; }

  footer { display: flex; justify-content: flex-end; gap: 10px; }

  button {
    background: rgba(255, 255, 255, 0.12);
    color: var(--texte);
    border: 1px solid rgba(255, 255, 255, 0.2);
    padding: 8px 18px;
    cursor: pointer;
    font: inherit;
  }
  button:hover { background: rgba(255, 255, 255, 0.2); }
  button:disabled { opacity: 0.5; cursor: default; }

  #erreur { color: rgb(255, 140, 140); font-size: 13px; min-height: 18px; }
</style>
</head>
<body>

<section id="ecran-1">
  <h1>🐱 Bienvenue</h1>
  <p>Shimeji Desktop fait vivre de petits personnages sur votre bureau. Ils se
     déplacent, se reposent, grimpent aux murs — tout seuls.</p>
  <p class="discret">Un personnage est déjà en train de tomber sur votre
     écran. Deux questions, et c'est fini.</p>
</section>

<section id="ecran-2" hidden>
  <h1>Deux réglages</h1>

  <fieldset>
    <legend>Au démarrage de Windows</legend>
    <label>
      <input type="checkbox" id="demarrage-auto">
      Lancer Shimeji Desktop avec Windows
    </label>
  </fieldset>

  <fieldset>
    <legend>À l'ouverture de l'application</legend>
    <label><input type="radio" name="ecran" value="gestionnaire">
      Ouvrir le gestionnaire de personnages</label>
    <label><input type="radio" name="ecran" value="personnages" checked>
      Afficher seulement les personnages</label>
    <label><input type="radio" name="ecran" value="tray">
      Rester dans la zone de notification, personnages cachés</label>
  </fieldset>

  <p id="erreur"></p>
</section>

<section id="ecran-3" hidden>
  <h1>C'est prêt</h1>
  <p>Shimeji Desktop continue de fonctionner <strong>en arrière-plan</strong>,
     même quand aucune fenêtre n'est ouverte.</p>
  <p>Retrouvez-le par son icône dans la <strong>zone de notification</strong>,
     en bas à droite : elle permet d'ouvrir le gestionnaire, de cacher les
     personnages, et de quitter.</p>
  <p class="discret">Un clic droit sur un personnage ouvre son propre menu.</p>
</section>

<footer>
  <button id="precedent" hidden>Précédent</button>
  <button id="suivant">Suivant</button>
</footer>

<script src="onboarding.js"></script>
</body>
</html>
```

- [ ] **Étape 4 : `ui/onboarding.js`**

```js
// L'assistant de première configuration (spec §3).
//
// Délibérément bête, comme `pet.js` : il navigue entre trois sections et
// fait deux appels. Toute la logique — registre, configuration, écran,
// toast — est en Rust, dans `commandes::onboarding_terminer`.
//
// `window.__TAURI__` est disponible parce que `withGlobalTauri` est à `true`
// dans `tauri.conf.json` : aucun import, aucun bundler.

const invoke = window.__TAURI__.core.invoke;

const ecrans = [
  document.getElementById("ecran-1"),
  document.getElementById("ecran-2"),
  document.getElementById("ecran-3"),
];
const precedent = document.getElementById("precedent");
const suivant = document.getElementById("suivant");
const erreur = document.getElementById("erreur");
const caseDemarrage = document.getElementById("demarrage-auto");

let courant = 0;

function afficher() {
  ecrans.forEach((s, i) => { s.hidden = i !== courant; });
  precedent.hidden = courant === 0;
  suivant.textContent = courant === ecrans.length - 1 ? "Terminer" : "Suivant";
}

// L'état initial de la case vient du REGISTRE, pas de la configuration
// (spec §0) : quelqu'un qui réinstalle par-dessus une version où il avait
// activé le démarrage automatique doit retrouver sa case cochée.
invoke("onboarding_etat")
  .then((etat) => { caseDemarrage.checked = etat.demarrageAuto; })
  .catch((e) => { console.error("onboarding_etat :", e); });

precedent.addEventListener("click", () => {
  if (courant > 0) { courant -= 1; afficher(); }
});

suivant.addEventListener("click", () => {
  if (courant < ecrans.length - 1) { courant += 1; afficher(); return; }

  // Dernier écran : on valide. Le bouton est désactivé pendant l'appel —
  // un double clic lancerait deux fois l'écriture et deux toasts.
  suivant.disabled = true;
  erreur.textContent = "";

  const ecranChoisi = document.querySelector('input[name="ecran"]:checked').value;

  invoke("onboarding_terminer", {
    demarrageAuto: caseDemarrage.checked,
    ecran: ecranChoisi,
  }).catch((e) => {
    // La fenêtre est normalement déjà fermée quand une erreur arrive : le
    // seul cas où elle ne l'est pas est un échec d'écriture de la
    // configuration, qui laisse tout ouvert.
    //
    // On DÉCOCHE la case si c'est le registre qui a refusé — afficher une
    // case cochée alors que rien n'a été écrit serait un mensonge, et c'est
    // exactement ce que l'entrée du tray s'interdit déjà.
    caseDemarrage.checked = false;
    erreur.textContent = String(e);
    suivant.disabled = false;
    courant = 1;
    afficher();
  });
});

afficher();
```

> Note sur les noms : Tauri convertit les paramètres de commande en camelCase
> côté JS. `demarrage_auto` en Rust s'envoie donc `demarrageAuto` ici, et
> `EtatOnboarding.demarrage_auto` se lit `etat.demarrageAuto`. Une faute sur
> ce point donne un `undefined` silencieux, pas une erreur.

- [ ] **Étape 5 : vérifier l'assistant, sans toucher au `config.json`**

```powershell
cargo build 2>&1 | Select-Object -Last 40
$env:SHIMEJI_ONBOARDING=1; cargo run
```

Attendu, à l'œil : un personnage tombe **et** l'assistant s'ouvre par-dessus.
Trois écrans, « Précédent » caché sur le premier, « Terminer » sur le
dernier. Le choix « Rester dans la zone de notification » fait disparaître les
personnages **immédiatement** à la validation.

Vérification scriptable de ce qui a été écrit :

```powershell
Get-Content "$env:APPDATA\shimeji-desktop\config.json" | Select-String "premiereConfiguration|ecranAuDemarrage"
```

Attendu : les deux clés, en camelCase, avec les valeurs choisies. **Si elles
manquent, c'est le piège du camelCase** — vérifier `definir_onboarding`.

Puis, sans la variable :

```powershell
cargo run
```

Attendu : **pas d'assistant**, et l'écran choisi.

- [ ] **Étape 6 : commit**

```bash
git add ui/onboarding.html ui/onboarding.js src-tauri/src/actions.rs src-tauri/src/main.rs
git commit -m "feat(onboarding): la fenetre de l'assistant, et SHIMEJI_ONBOARDING"
```

---

## Tâche 6 : le toast

**Fichiers :**
- Modifier : `src-tauri/Cargo.toml`
- Créer : `src-tauri/src/toast.rs`
- Modifier : `src-tauri/src/main.rs` (déclaration du module, `.plugin(…)`)
- Modifier : `src-tauri/src/commandes.rs` (le geste n° 4 laissé en attente)

**Interfaces :**
- Consomme : `tauri::AppHandle`
- Produit : `pub fn toast::arriere_plan(app: &AppHandle)` ; la variable
  `SHIMEJI_TOAST=1`

- [ ] **Étape 1 : la dépendance**

Depuis PowerShell, dans `src-tauri` :

```powershell
cargo add tauri-plugin-notification@2 2>&1 | Select-Object -Last 15
```

Récupération réseau unique — la crate est absente du `Cargo.lock`. Vérifier
ensuite que la ligne ajoutée est bien commentée à la main, comme toutes les
autres de ce `Cargo.toml` :

```toml
# Le toast de fin d'assistant (spec §4). Une seule notification dans tout le
# programme — mais l'écrire à la main demanderait du WinRT, là où la crate
# `windows` que nous avons n'expose que du Win32.
tauri-plugin-notification = "2"
```

- [ ] **Étape 2 : `src-tauri/src/toast.rs`**

```rust
//! La notification de fin de première configuration (spec §4).
//!
//! Responsabilité unique : émettre **une** notification Windows, sans jamais
//! faire échouer ce qui l'appelle.

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

/// Annonce que l'application continue en arrière-plan.
///
/// # ⚠️ Pourquoi ça ne marchera pas sous `cargo run`
///
/// Sur Windows 10/11, un toast exige un **AppUserModelID enregistré**, que
/// fournit le raccourci du menu Démarrer posé par l'installateur NSIS. Une
/// application lancée depuis `target\debug\` n'en a pas : le toast ne
/// s'affichera **pas** en développement, quelle que soit l'implémentation.
/// Ce n'est pas un défaut à corriger, c'est le fonctionnement de Windows.
///
/// D'où le traitement : *best-effort*, jamais fatal, et `SHIMEJI_TOAST=1`
/// imprime le `Result` — pour que ce ne soit pas un échec muet de plus
/// (règle du projet : tout ce qui demanderait un clic reçoit un équivalent
/// scriptable).
pub fn arriere_plan(app: &AppHandle) {
    let resultat = app
        .notification()
        .builder()
        .title("🐱 Shimeji Desktop est actif")
        .body(
            "L'application continue de fonctionner en arrière-plan.\n\
             Vous pouvez la contrôler depuis l'icône dans la zone de notification.",
        )
        .show();

    let trace = std::env::var("SHIMEJI_TOAST").is_ok();

    match resultat {
        Ok(()) => {
            if trace {
                println!("[toast] affiché");
            }
        }
        Err(e) => {
            // Toujours imprimé, même sans la variable : un toast qui
            // n'arrive pas est une information, pas un incident.
            eprintln!("[toast] non affiché : {e}");
            if trace {
                eprintln!(
                    "[toast] cause la plus probable : aucun AppUserModelID — \
                     normal hors d'une installation NSIS"
                );
            }
        }
    }
}
```

- [ ] **Étape 3 : déclarer le module et le plugin**

Dans `main.rs`, à côté des autres `mod` :

```rust
mod toast;
```

Et dans la chaîne du `Builder`, avant `.invoke_handler` :

```rust
        // Le plugin de notification (spec §4). Enregistré même si le toast
        // n'est émis qu'une fois dans la vie de l'application : sans lui,
        // `app.notification()` panique au lieu de rendre une erreur.
        .plugin(tauri_plugin_notification::init())
```

Aucune permission à déclarer : le système de capabilities gouverne l'API
**JavaScript** du plugin. Ici, l'émission vient de Rust, où rien ne la filtre.

- [ ] **Étape 4 : brancher le geste n° 4 de `onboarding_terminer`**

Dans `commandes.rs`, remplacer le commentaire laissé en attente à la tâche 4 :

```rust
    // ── 4. Le toast — branché à la tâche 6 ──────────────────────────────
```

par :

```rust
    // ── 4. Le toast (spec §4) ───────────────────────────────────────────
    // Best-effort : il ne peut pas s'afficher hors d'une installation NSIS,
    // et ce n'est pas une raison de faire échouer l'assistant.
    //
    // Il ne se joue qu'une fois parce qu'il est ICI : `onboarding_terminer`
    // n'est appelée qu'une fois. Aucune clé de configuration dédiée —
    // remettre `premiereConfigurationFaite` à `false` rejoue les deux.
    crate::toast::arriere_plan(&app);
```

- [ ] **Étape 5 : vérifier que l'échec est propre en développement**

```powershell
cargo build 2>&1 | Select-Object -Last 40
$env:SHIMEJI_ONBOARDING=1; $env:SHIMEJI_TOAST=1; cargo run
```

Puis dérouler l'assistant jusqu'à « Terminer ».

Attendu **en développement** : une ligne `[toast] non affiché : …` suivie de
la cause probable, **et l'assistant se termine quand même normalement** — la
configuration est écrite, la fenêtre se ferme, l'écran choisi s'applique.
C'est exactement le comportement voulu ; la vérification du toast qui
s'affiche vraiment est à la tâche 7, sur l'application installée.

- [ ] **Étape 6 : commit**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/toast.rs src-tauri/src/main.rs src-tauri/src/commandes.rs
git commit -m "feat(toast): la notification de fin d'assistant, et SHIMEJI_TOAST"
```

---

## Tâche 7 : la vérification de bout en bout, et la documentation

La seule tâche du projet qui demande un humain — comme le « Quitter » du tray
l'a exigé une fois, le 2026-09-10.

**Fichiers :**
- Modifier : `CLAUDE.md`
- Modifier : `config.exemple.json`

**Interfaces :**
- Consomme : tout ce qui précède
- Produit : un installateur vérifié et une documentation à jour

- [ ] **Étape 1 : produire l'installateur**

```powershell
cd C:\Users\alri\Documents\shimeji-desktop\src-tauri
cargo tauri build 2>&1 | Select-Object -Last 20
Get-ChildItem .\target\release\bundle\nsis\
```

Attendu : `shimeji-desktop_0.1.0_x64-setup.exe`.

- [ ] **Étape 2 : vérifier l'exe release avant même d'installer**

```powershell
Get-Item .\target\release\shimeji-desktop.exe | Select-Object Length
```

Attendu : quelques Mo. Et surtout, lancé directement, **aucune console
n'apparaît** — `windows_subsystem = "windows"` (`main.rs:14`).

- [ ] **Étape 3 : installer, et dérouler le parcours complet**

Installer le setup, puis **lancer depuis le menu Démarrer** — pas depuis
`target\`, c'est tout l'intérêt : le raccourci porte l'AppUserModelID.

À vérifier dans l'ordre :

1. **un personnage apparaît** → la ressource `characters` est bien posée
   (tâche 1). S'il n'y en a aucun, c'est là qu'est le défaut ;
2. **l'assistant s'ouvre** par-dessus le personnage ;
3. cocher « Lancer avec Windows », choisir un écran, **Terminer** ;
4. **le toast s'affiche** → l'AppUserModelID est bien enregistré (tâche 6) ;
5. **fermer le gestionnaire à la croix ne tue rien** → tâche 3 : le
   personnage continue, l'icône du tray est toujours là ;
6. **« Quitter » dans le tray termine bien le processus** → le filtre
   `code: None` laisse passer `app.exit(0)` ;
7. relancer : **pas d'assistant**, et l'écran choisi s'ouvre.

- [ ] **Étape 4 : vérifier ce que l'installation a écrit**

```powershell
Get-Content "$env:APPDATA\shimeji-desktop\config.json" |
  Select-String "premiereConfigurationFaite|ecranAuDemarrage"

Get-ItemProperty "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name shimeji-desktop
```

Attendu : les deux clés en camelCase avec les valeurs choisies, et une valeur
de registre qui pointe l'exe **installé** (`…\Program Files\…`), pas celui de
`target\`.

> Ce dernier point mérite l'attention : `autostart::activer()` enregistre
> `std::env::current_exe()`. Lancé depuis `target\debug\`, il enregistre le
> binaire de développement — correct, mais c'est une raison de plus de
> valider ce chemin sur l'application **installée**.

- [ ] **Étape 5 : mettre `config.exemple.json` à jour**

Remplacer la ligne :

```json
  "demarrageAutomatique": false,
```

par :

```json
  "premiereConfigurationFaite": false,
  "ecranAuDemarrage": "personnages",
```

- [ ] **Étape 6 : corriger CLAUDE.md**

Quatre points, listés en §8 de la spec :

1. **le tableau d'outillage** : la ligne
   « `cargo-tauri` ⬜ non installé, et **inutile** » devient
   « `cargo-tauri` ✅ `tauri-cli 2.11.4` — **indispensable** : `cargo build`
   ignore entièrement le bloc `bundle`, c'est la CLI qui fabrique le NSIS » ;
2. **« Compiler et lancer »** gagne :

   ```powershell
   cargo tauri build   # l'installateur NSIS, dans target/release/bundle/nsis/
   ```

3. **la table des variables de diagnostic** gagne deux lignes :

   | Variable | Ce qu'elle fait |
   |---|---|
   | `SHIMEJI_ONBOARDING=1` | force l'assistant de première configuration, sans toucher au `config.json` |
   | `SHIMEJI_TOAST=1` | trace le résultat de la notification — qui ne peut PAS s'afficher hors d'une installation NSIS, faute d'AppUserModelID |

4. **« État actuel »** gagne un paragraphe : l'application est distribuable,
   l'installateur NSIS livre `characters/`, l'assistant s'ouvre une fois,
   fermer le gestionnaire ne quitte plus.

- [ ] **Étape 7 : commit**

```bash
git add CLAUDE.md config.exemple.json
git commit -m "docs: l'application est distribuable — installateur, assistant, toast

cargo-tauri n'est pas « inutile » : cargo build ignore le bloc bundle."
```

---

## Ce que ce plan ne fait pas

Décidé en §7 de la spec, pas oublié : pas de mise à jour automatique, pas de
signature de code (l'installateur déclenchera SmartScreen — normal pour un
binaire non signé), pas de nouveau tableau de bord de réglages, pas de choix
de personnage dans l'assistant, pas d'`onboarding_version`, pas de cible MSI.
