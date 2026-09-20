# Rendre l'application distribuable — design

*2026-09-20*

L'objectif : qu'une personne sans Rust, sans Cargo et sans environnement de
développement puisse **installer** shimeji-desktop, le **lancer**, être
**accueillie** une fois, et retrouver ensuite ses préférences à chaque
démarrage.

```
Installateur NSIS
        ↓
Installation (Program Files + raccourci menu Démarrer)
        ↓
Premier lancement
        ↓
Les personnages vivent déjà · l'assistant s'ouvre par-dessus
  ├── démarrer avec Windows ?
  ├── quel écran au démarrage ?
  └── terminer → toast « je continue en arrière-plan »
        ↓
Lancements suivants : pas d'assistant, l'écran choisi
```

---

## 0. Les deux constats qui ont précédé le design

Ils contredisent l'énoncé initial de la demande, et les consigner ici évite
qu'on les redécouvre.

### `config.demarrage_automatique` est du code mort

Déclaré (`config.rs:261`), initialisé à `false` (`config.rs:290`), et
**jamais lu ni écrit** — un `grep` sur le nom ne rend que ces deux lignes.

La seule vérité du démarrage automatique est **le registre**. La case du tray
se coche depuis `autostart::est_actif()`, et `ID_DEMARRAGE`
(`actions.rs:277`) écrit dans le registre sans toucher à la configuration.

Brancher l'assistant sur ce booléen créerait donc **une seconde vérité**, à
tenir d'accord avec le registre pour toujours. C'est précisément l'erreur que
CLAUDE.md interdit pour l'interrupteur de la bibliothèque :

> L'interrupteur n'est que le reflet de `compte > 0`. […] il n'y a aucun
> compte « en sommeil » stocké à côté, donc aucune seconde vérité à tenir
> d'accord avec `config.personnages`. **Ne pas en ajouter une.**

→ Le champ est **supprimé** (§2). L'assistant écrit dans le registre, par le
module qui existe déjà.

### `cargo-tauri` est installé, et il est indispensable

CLAUDE.md porte « `cargo-tauri` ⬜ non installé, et **inutile** ». Les deux
moitiés sont fausses aujourd'hui :

- `~/.cargo/bin/cargo-tauri.exe` existe — `tauri-cli 2.11.4`, 28 Mo ;
- `cargo build` **ignore entièrement** le bloc `bundle` de `tauri.conf.json`.
  C'est la CLI qui le lit et qui appelle NSIS. Sans elle, il n'y a pas
  d'installateur du tout.

La ligne du tableau d'outillage de CLAUDE.md est à corriger.

---

## 1. La chaîne de build

### Ce qui est déjà en place, et qu'il ne faut pas « configurer »

| Exigence | Où c'est déjà fait |
|---|---|
| exe sans console | `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`, `main.rs:14` |
| pas de redistribuable VC++ à installer | `rustflags = ["-C", "target-feature=+crt-static"]`, `.cargo/config.toml` |
| exe petit | `[profile.release]` : `opt-level = "z"`, `lto`, `codegen-units = 1`, `strip`, `panic = "abort"` |
| pas de bundler, pas de Node | `frontendDist: "../ui"` — le front est statique, aucun `beforeBuildCommand` |

### La commande

```powershell
cd C:\Users\alri\Documents\shimeji-desktop\src-tauri
cargo tauri build
```

Sortie : `target/release/bundle/nsis/shimeji-desktop_0.1.0_x64-setup.exe`.

`cargo build --release` reste valable pour obtenir **l'exe seul**, et c'est ce
qu'on continue d'utiliser pour les mesures CPU — mais il ne produit aucun
installateur.

Deux dépendances réseau, une fois chacune :

1. la CLI **télécharge NSIS** au premier bundle ;
2. l'installateur embarque par défaut le *bootstrapper* WebView2, qui
   l'installe sur la machine cible s'il manque. Inutile sur Windows 11, où
   WebView2 est présent — mais c'est ce qui rend l'installateur correct sur
   Windows 10.

### La clé manquante — `resources`

C'est **le** défaut que l'installateur non testé aurait produit. Aujourd'hui
`tauri.conf.json` n'a aucune clé `resources` : NSIS livrerait l'exe nu,
`config::resoudre("characters")` ne trouverait rien à côté de l'exe, et
l'application installée démarrerait avec **zéro personnage et aucun moyen
d'en obtenir un** tant que le catalogue n'a rien téléchargé.

```json
"bundle": {
  "active": true,
  "targets": ["nsis"],
  "icon": ["icons/icon.ico"],
  "resources": { "../characters": "characters" }
}
```

La forme **map** (source → destination) est nécessaire : `tauri-utils` accepte
`BundleResources::List` ou `::Map` (`config.rs:1508`), et seule la seconde
permet de poser `../characters` sous le nom `characters` à côté de l'exe.

204 Ko. Le dossier atterrit dans `Program Files`, donc **en lecture seule** —
et c'est le bon état : `%APPDATA%\shimeji-desktop\characters\` reste la seule
zone inscriptible, ce qui rend la règle de suppressibilité
(`est_dans_la_bibliotheque`) vraie *par construction* au lieu de par chance.

### Ce qu'on ne change pas, et pourquoi

| Clé | Valeur gardée | Raison |
|---|---|---|
| `identifier` | `dev.local.shimeji-desktop` | il devient l'AppUserModelID du toast **et** l'identité de désinstallation/mise à jour. Le changer après une première installation chez quelqu'un laisserait deux entrées dans « Applications installées ». |
| `productName` | `shimeji-desktop` | il devient le nom de l'exe et du dossier d'installation. Le passer à « Shimeji Desktop » casserait `Get-Process -Name shimeji-desktop` dans **tous** les outils de mesure CPU (`docs/outils/mesurer-roster.ps1`, le dossier CPU), pour un gain purement cosmétique. |

`version` doit rester d'accord entre `Cargo.toml` et `tauri.conf.json` : c'est
la seconde qui nomme le fichier du setup.

---

## 2. L'état persisté

Deux clés ajoutées à `Config`, une retirée.

```rust
/// L'assistant de première configuration a-t-il été mené à son terme ?
///
/// **Un seul booléen pour deux effets** : il commande l'ouverture de
/// l'assistant ET l'émission du toast. Le remettre à `false` à la main rejoue
/// les deux — c'est la « contrôlabilité par la configuration » demandée, sans
/// seconde clé à tenir d'accord avec la première.
pub premiere_configuration_faite: bool,   // défaut : false

/// Ce qui s'ouvre au démarrage.
pub ecran_au_demarrage: EcranDemarrage,   // défaut : Personnages
```

```rust
#[derive(Deserialize, Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum EcranDemarrage {
    /// Les personnages vivent, ET la fenêtre du gestionnaire s'ouvre.
    Gestionnaire,
    /// Les personnages vivent, aucune fenêtre. C'est le comportement actuel.
    Personnages,
    /// Les personnages démarrent CACHÉS : exactement l'état que
    /// `SHIMEJI_CACHE=1` et la case « Afficher » du tray produisent déjà.
    Tray,
}
```

### Le mapping sur des mécanismes qui existent déjà

Aucun des trois écrans n'introduit de mécanisme. Ils choisissent entre trois
chemins déjà écrits :

| Valeur | Ce que `setup` fait |
|---|---|
| `gestionnaire` | `actions::ouvrir_catalogue(&handle)` — ce que `SHIMEJI_CATALOGUE=1` fait déjà |
| `personnages` | rien |
| `tray` | la visibilité de départ est « cachée » — ce que `SHIMEJI_CACHE=1` fait déjà |

### Le repli sur valeur inconnue

`charger_depuis` jette **toute** la configuration sur une erreur de parsing
(`config.rs:637`). Une valeur inconnue dans `ecran_au_demarrage` ne doit donc
pas invalider le fichier entier.

**`#[serde(other)]` ne convient pas ici** : serde ne l'accepte que sur les
énumérations *taguées* (`#[serde(tag = …)]`), pas sur une énumération
sérialisée en simple chaîne. Le repli passe donc par un désérialiseur nommé :

```rust
#[serde(deserialize_with = "ecran_tolerant")]
pub ecran_au_demarrage: EcranDemarrage,
```

```rust
/// Lit `ecran_au_demarrage` sans jamais échouer.
///
/// On désérialise d'abord une `String` — ça, ça ne peut échouer que si la
/// valeur n'est pas une chaîne du tout — puis on traduit nous-mêmes. Une
/// valeur inconnue devient `Personnages` avec un avertissement, au lieu de
/// faire rejeter le fichier ENTIER par `charger_depuis`.
///
/// `D: Deserializer<'de>` et la durée de vie `'de` sont imposés par serde :
/// `'de` est celle des données d'entrée, que le désérialiseur peut emprunter.
fn ecran_tolerant<'de, D>(d: D) -> Result<EcranDemarrage, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let brut = String::deserialize(d)?;
    match brut.as_str() {
        "gestionnaire" => Ok(EcranDemarrage::Gestionnaire),
        "personnages" => Ok(EcranDemarrage::Personnages),
        "tray" => Ok(EcranDemarrage::Tray),
        autre => {
            eprintln!("ecran_au_demarrage : « {autre} » inconnu, « personnages » utilisé");
            Ok(EcranDemarrage::Personnages)
        }
    }
}
```

Aucun `#[serde(default)]` par champ n'est nécessaire : `Config` en porte un
**au niveau de la structure** (`config.rs:246`), donc un `config.json`
antérieur, qui ne connaît ni l'une ni l'autre des nouvelles clés, se charge
déjà sans erreur.

### ⚠️ Les clés JSON sont en camelCase

`Config` est annotée `#[serde(default, rename_all = "camelCase")]`. Les noms
écrits dans `config.json` ne sont donc **pas** ceux des champs Rust :

| Champ Rust | Clé dans `config.json` |
|---|---|
| `premiere_configuration_faite` | `premiereConfigurationFaite` |
| `ecran_au_demarrage` | `ecranAuDemarrage` |
| `demarrage_automatique` (supprimé) | `demarrageAutomatique` |

C'est un piège pour `ecrire_cles`, qui manipule des chaînes littérales et non
la structure : y écrire `"premiere_configuration_faite"` produirait une clé
que `Config` ne relirait jamais, donc un assistant qui **revient à chaque
lancement**. `ecrire_personnages` y échappe par accident — `personnages`
s'écrit pareil dans les deux conventions. Les tests de §6 couvrent ce point
explicitement, en **relisant par `charger_depuis`** plutôt qu'en comparant le
JSON.

### `demarrage_automatique` est supprimé

Code mort (§0). Le garder inviterait le prochain lecteur à le brancher, et
donc à créer la seconde vérité.

La suppression est **sans risque de rupture** : serde ignore les clés
inconnues par défaut, donc un `config.json` existant qui porte encore
`"demarrageAutomatique": true` continue de se charger sans bruit, la clé étant
simplement ignorée.

### L'écriture — généraliser la technique chirurgicale

`ecrire_personnages` (`config.rs:532`) sait déjà faire la seule chose
correcte : relire en `serde_json::Value`, ne toucher qu'à une clé, réécrire.
Sérialiser depuis `Config` perdrait les clés inconnues et remettrait les
valeurs par défaut partout — l'utilisateur verrait son fichier réglé à la main
écrasé par un clic.

On extrait donc le corps :

```rust
/// Remplace les clés nommées dans `chemin`, en laissant tout le reste.
pub fn ecrire_cles(chemin: &Path, cles: &[(&str, serde_json::Value)]) -> Result<(), String>
```

`ecrire_personnages` devient un appel à `ecrire_cles(chemin, &[("personnages",
json!(noms))])`. Aucun de ses appelants ne change, et ses tests existants
deviennent des tests d'`ecrire_cles` par la bande.

Un pendant de `definir_personnages` — qui résout `chemin_charge()` ou crée un
`config.json` dans `%APPDATA%` — écrit les clés de l'assistant. C'est ce qui
rend l'application installée capable d'écrire ses préférences alors que son
dossier d'installation est en lecture seule.

---

## 3. L'assistant

### La fenêtre

Label `onboarding`, `ui/onboarding.html` + `ui/onboarding.js`, même facture
que le catalogue : décorée, focalisable, présente dans la barre des tâches,
**non redimensionnable**, ~520×460, centrée.

`capabilities/onboarding.json`, portée à cette seule fenêtre :

```json
{
  "identifier": "onboarding",
  "description": "Permet à l'assistant de première configuration d'appeler Rust.",
  "windows": ["onboarding"],
  "permissions": ["core:default"]
}
```

> ⚠️ Le label doit correspondre **exactement** à celui de la capability, sinon
> les appels `invoke` sont refusés **en silence** — le piège déjà payé sur le
> catalogue (`actions.rs:376`).

Elle s'ouvre depuis `setup`, après la création des personnages, quand
`!config.premiere_configuration_faite`. Les personnages **vivent déjà** : la
boucle 60 Hz démarre normalement, `blob` tombe du haut de l'écran, et
l'assistant s'ouvre par-dessus. C'est l'argument du produit — la personne voit
immédiatement ce qu'elle a installé, au lieu d'un bureau vide en se demandant
si ça marche.

### Les trois écrans

Pas de navigation en arrière : deux questions ne méritent pas un assistant à
état.

| Écran | Contenu |
|---|---|
| **Bienvenue** | qui il est, une phrase, « il est déjà sur ton bureau » |
| **Réglages** | ☐ Démarrer avec Windows · ○ Gestionnaire ○ Personnages ○ Tray seul |
| **Fin** | « Terminer », et le rappel que l'icône de la zone de notification pilote tout |

### Les deux commandes

Ajoutées au `generate_handler!` existant (`main.rs:362`).

> ⚠️ Une commande oubliée dans cette liste est introuvable côté JS **sans
> erreur de compilation** — c'est écrit noir sur blanc à côté de la macro.

```rust
/// L'état à afficher à l'ouverture de l'assistant.
#[tauri::command]
fn onboarding_etat() -> EtatOnboarding
```

Elle pré-coche la case depuis **`autostart::est_actif()`** — le registre, pas
la configuration (§0). Le cas existe réellement : quelqu'un qui réinstalle
par-dessus une version où il avait activé le démarrage automatique doit
retrouver la case cochée.

```rust
/// Applique les choix, marque la configuration comme faite, tente le toast.
#[tauri::command]
fn onboarding_terminer(
    app: tauri::AppHandle,
    demarrage_auto: bool,
    ecran: String,
) -> Result<(), String>
```

Dans l'ordre :

1. `autostart::activer()` ou `desactiver()` selon `demarrage_auto` ;
2. `ecrire_cles` des deux clés de §2 ;
3. l'écran choisi est appliqué **immédiatement** (`gestionnaire` ouvre le
   catalogue, `tray` cache les personnages) — sinon le réglage ne se verrait
   qu'au lancement suivant, et paraîtrait sans effet ;
4. le toast, en *best-effort* (§4) ;
5. la fenêtre `onboarding` se ferme.

**Le traitement de l'échec** suit le précédent de `ID_DEMARRAGE`
(`actions.rs:277`) : si `activer()` échoue, la commande rend l'`Err` et le
webview **décoche la case** plutôt que d'afficher un état qui n'est pas celui
du registre. Laisser une case cochée après une écriture ratée serait un
mensonge affiché en permanence.

L'échec de l'écriture de la configuration, lui, est **fatal au sens de
l'assistant** : si on ne peut pas écrire `premiere_configuration_faite`,
l'assistant reviendra au prochain lancement, et il vaut mieux le dire que le
laisser boucler en silence.

---

## 4. Le toast

`tauri-plugin-notification` (version 2), **absent du `Cargo.lock`** : une
récupération réseau unique à prévoir.

Le message :

```
🐱 Shimeji Desktop est actif
L'application continue de fonctionner en arrière-plan.
Vous pouvez la contrôler depuis l'icône dans la zone de notification.
```

### Le piège de l'AppUserModelID

Sur Windows 10/11, un toast exige un **AppUserModelID enregistré**, ce que
fournit le raccourci du menu Démarrer posé par NSIS. Une application lancée
par `cargo run` n'en a pas : **le toast ne s'affichera pas en
développement**, quelle que soit l'implémentation. Ce n'est pas un défaut à
corriger, c'est le fonctionnement de Windows.

Conséquence de méthode : l'échec est imprimé, jamais fatal, et
`SHIMEJI_TOAST=1` force la tentative en imprimant le `Result` — pour que ce ne
soit pas un neuvième échec muet. Cela suit la règle récurrente du projet :
*tout ce qui demanderait un clic reçoit un équivalent scriptable.*

### Une seule fois

Il est dans le chemin de `onboarding_terminer`, qui ne s'exécute qu'une fois.
Aucune clé dédiée : remettre `premiere_configuration_faite` à `false` rejoue
l'assistant **et** le toast.

---

## 5. Le cycle de vie

### Le défaut actuel

Il n'y a **aucun** `on_window_event` ni `RunEvent::ExitRequested` dans le
projet. Tauri quitte quand la dernière fenêtre se ferme : avec un roster vide
— un état normal, décocher le dernier personnage est permis — fermer le
gestionnaire **tue l'application**, tray compris.

### Le correctif

`main.rs` passe de `.run(context)` à `.build(context)` suivi d'un `.run()`
avec gestionnaire d'événements :

```rust
.build(tauri::generate_context!())
.expect("échec au lancement de l'application Tauri")
.run(|_app, evenement| {
    // Tauri termine le processus quand la dernière fenêtre se ferme. Nous
    // vivons dans le tray : fermer le gestionnaire ne doit rien tuer.
    //
    // `code: None` est ESSENTIEL. `app.exit(0)` du menu « Quitter » émet le
    // même événement, mais avec un code. Sans ce filtre on rendrait
    // l'application impossible à quitter — le défaut serait bien pire que
    // celui qu'on corrige.
    if let tauri::RunEvent::ExitRequested { api, code: None, .. } = evenement {
        api.prevent_exit();
    }
});
```

### Le tray

Rien à construire : l'entrée « Catalogue de personnages… » (`ID_CATALOGUE`)
rouvre déjà la fenêtre, et « Quitter » (`ID_QUITTER`, `app.exit(0)`) quitte
déjà — et traverse le filtre ci-dessus.

Seul changement : le libellé devient « **Ouvrir le gestionnaire…** », qui
décrit mieux ce que fait l'entrée maintenant que cette fenêtre est *la*
fenêtre principale. L'identifiant `ID_CATALOGUE` ne change pas.

> ⚠️ Rappel non négociable : **un seul `on_menu_event` dans tout le
> programme**. Rien ici n'en ajoute.

---

## 6. Vérification

### Ce qui se teste sans écran

Le projet a 299 tests ; ceux-ci s'y ajoutent, dans `config_tests.rs` :

1. l'aller-retour des deux nouvelles clés dans `config.json` ;
2. `ecrire_cles` **préserve les clés voisines** — le cœur de la technique
   chirurgicale ;
3. `ecrire_cles` sur un fichier absent le crée ; sur un fichier illisible,
   n'écrit rien et rend une `Err` ;
4. une valeur inconnue d'`ecran_au_demarrage` retombe sur `Personnages` **sans
   invalider le reste du fichier** ;
5. un `config.json` portant encore `demarrageAutomatique` se charge sans
   erreur ;
6. les clés écrites par `ecrire_cles` sont **relues** par `charger_depuis` —
   le seul test qui attrape une faute de camelCase ;
7. `premiere_configuration_faite` vaut `false` par défaut (donc l'assistant
   s'ouvre bien au premier lancement, y compris sans aucun `config.json`).

### Ce qui exige un humain

C'est le seul endroit du projet où un clic est inévitable — comme le
« Quitter » du tray l'a exigé une fois, le 2026-09-10.

1. `cargo tauri build` produit le setup ;
2. installer le setup ;
3. lancer **depuis le menu Démarrer** (pas depuis `target/`) et vérifier :
   - `blob` apparaît → la ressource `characters` est bien posée (§1) ;
   - l'assistant s'ouvre par-dessus ;
   - le toast s'affiche à la validation → l'AUMID est bien enregistré (§4) ;
   - fermer le gestionnaire ne tue rien → §5 ;
4. relancer : **pas d'assistant**, et l'écran choisi est celui qui s'ouvre ;
5. si « démarrer avec Windows » a été coché : la valeur est dans
   `HKCU\…\Run` et pointe l'exe **installé**, pas celui de `target/`.

Le point 5 mérite l'attention : `autostart::activer()` enregistre
`std::env::current_exe()`. Lancé depuis `target/debug/`, il enregistre le
binaire de développement — correct, mais c'est une raison de plus de valider
ce chemin **sur l'application installée**.

---

## 7. Hors périmètre

Décidé, pas oublié :

- ❌ **mise à jour automatique** — pas de canal de distribution, pas de serveur
- ❌ **signature de code** — l'installateur déclenchera SmartScreen au
  téléchargement. C'est normal pour un binaire non signé, et le certificat est
  un sujet administratif, pas technique
- ❌ **tableau de bord de réglages** — le gestionnaire reste le catalogue à
  deux onglets ; en faire un panneau de configuration complet est un chantier
  d'interface à part entière
- ❌ **choix du personnage dans l'assistant** — `blob` est livré, le catalogue
  est à un clic
- ❌ **`onboarding_version`** — YAGNI : il n'y a ni canal de mise à jour ni
  seconde version de l'assistant
- ❌ **cible MSI** — `nsis` seul, comme aujourd'hui

---

## 8. Ce que ce design change dans CLAUDE.md

À corriger une fois l'implémentation faite :

- la ligne « `cargo-tauri` ⬜ non installé, et **inutile** » du tableau
  d'outillage — il est installé (`tauri-cli 2.11.4`) et **indispensable** ;
- la section « Compiler et lancer » gagne `cargo tauri build` ;
- la liste des variables de diagnostic gagne `SHIMEJI_TOAST` ;
- `config.exemple.json` perd `demarrage_automatique` et gagne les deux
  nouvelles clés.
