# Étape 0 — résultat du spike

Plan : `docs/plans/2026-09-08-etape-0-spike-overlay.md`
Design : `docs/specs/2026-09-08-design.md`

---

## Chaîne d'outils

Relevé le 2026-09-08.

| Outil | Version / état |
|---|---|
| rustc | 1.98.1 (48a229cea 2026-09-01) ✅ |
| cargo | 1.98.1 (797e8a9bc 2026-08-05) ✅ |
| cible `x86_64-pc-windows-msvc` | installée ✅ |
| WebView2 | 152.0.4191.66 ✅ |
| Charge de travail C++ | installée sur **Visual Studio 18 Insiders** ✅ |
| Toolset MSVC | 14.51.36231 ✅ |
| SDK Windows | 10.0.26100.0 ✅ |
| `cargo-tauri` | non installé ⬜ (nécessaire seulement pour `tauri dev`) |
| tauri (résolu par cargo) | **2.11.5** |
| **Compilation du spike** | ✅ **réussie** — `cargo build`, 2 min 24 s, exe de 12,3 Mo |

### Où vit réellement la chaîne C++

Deux installations Visual Studio coexistent, et **seule l'Insiders porte la charge C++** :

```
C:\Program Files\Microsoft Visual Studio\2022\Community     ← pas de C++
C:\Program Files\Microsoft Visual Studio\18\Insiders        ← C++ ✅
  └── VC\Tools\MSVC\14.51.36231\bin\Hostx64\x64\link.exe
```

Conséquence pratique : `vswhere` ne la voit **qu'avec `-prerelease`**. Un diagnostic qui
interroge vswhere sans ce drapeau conclura à tort que la charge C++ est absente — c'est
l'erreur commise pendant ce spike.

**rustc 1.98.1 détecte l'installation Insiders sans configuration.** Aucune variable
d'environnement, aucun `Developer PowerShell` n'a été nécessaire : `cargo build` depuis un
PowerShell ordinaire a suffi.

### Historique — le faux diagnostic « la charge C++ est absente » (résolu)

> ⚠️ **Cette section est un récit, pas un état.** La charge C++ est installée et le spike
> compile (voir le tableau en tête de document). On la garde parce que le diagnostic était
> **faux**, et que la façon dont il s'est trompé peut se reproduire : les trois preuves
> ci-dessous paraissaient concluantes et ne l'étaient pas — elles interrogeaient toutes
> Visual Studio **2022 Community**, alors que la charge C++ vit sur **18 Insiders**.

Confirmé — à tort — de trois façons apparemment indépendantes :

```
vswhere -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64  →  (vide)
C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\  →  inexistant
C:\Program Files (x86)\Windows Kits\10\bin\  →  inexistant
```

Visual Studio 2022 Community est bien installé, mais sans MSVC ni SDK Windows. La cible
MSVC de Rust n'a donc ni linker ni bibliothèques système.

**Correction** — modifier l'installation existante plutôt qu'en ajouter une :

```powershell
& "C:\Program Files (x86)\Microsoft Visual Studio\Installer\setup.exe" modify `
  --installPath "C:\Program Files\Microsoft Visual Studio\2022\Community" `
  --add Microsoft.VisualStudio.Workload.NativeDesktop --includeRecommended --passive
```

Ou : Visual Studio Installer → *Modifier* → cocher **« Développement Desktop en C++ »**.

### Piège annexe : le mauvais `link.exe`

`cargo build` échoue avec :

```
error: linking with `link.exe` failed: exit code: 1
  = note: link: extra operand '…build_script_build….rcgu.o'
note: `link.exe` returned an unexpected error
note: in the Visual Studio installer, ensure the "C++ build tools" workload is selected
```

`extra operand` est une formulation d'erreur **GNU coreutils**, pas MSVC : le `link.exe`
invoqué est celui livré par **Git for Windows** (`<Git>/usr/bin/link.exe`, l'utilitaire de
création de liens durs), et non le linker de Visual Studio.

Enchaînement : faute d'installation C++ détectable, rustc n'a pas pu passer un chemin
absolu vers le vrai linker et s'est rabattu sur `link.exe` tel que résolu par le `PATH` —
où le `usr/bin` de Git précédait tout.

**Confirmé résolu** : une fois la charge C++ installée, rustc a localisé le linker via
l'installation Visual Studio et passé un chemin absolu ; l'erreur a disparu d'elle-même.

Mais **si l'erreur `extra operand` réapparaît un jour**, la cause est le `PATH` et non
l'installation — compiler alors depuis un *Developer PowerShell*, ou depuis PowerShell
plutôt que depuis un shell Git Bash.

### Deux corrections nécessaires au script de build

Une fois le linker en place, `tauri-build` a échoué deux fois de suite. Les deux
corrections valent pour **l'application de l'étape 1**, pas seulement pour le spike :

| Erreur | Cause | Correction |
|---|---|---|
| `` `icons/icon.ico` not found; required for generating a Windows Resource file `` | `tauri-build` exige une icône Windows, même pour un projet jetable | créer `icons/icon.ico` (généré ici depuis `shime1.png`, en tailles 16 → 256) |
| échec silencieux du chargement du front | `frontendDist` est résolu **relativement au dossier du `tauri.conf.json`** — le `../ui` conventionnel suppose une config dans `src-tauri/` | avec une config à la racine du projet : `"./ui"` |

Le second est une erreur de conception du plan initial, qui avait recopié le chemin
conventionnel `../ui` d'une arborescence `src-tauri/` vers une arborescence plate.
**L'étape 1 utilisera `src-tauri/`**, donc `../ui` y sera correct — c'est bien le spike
qui était l'exception.

---

## Vérifications de code effectuées sans compilateur

Le linker manquant interdit `cargo build`, et aussi `cargo check` : les scripts de build
de Tauri sont des exécutables, qui doivent donc être liés pour s'exécuter.

Les incertitudes d'API du plan ont malgré tout été levées, en lisant les sources de
**tauri 2.11.5** téléchargées par `cargo fetch`.

| Appel | Vérifié | Emplacement dans les sources |
|---|---|---|
| `available_monitors()` | ✅ dans `impl App<Wry>` — l'appel sur le `app` de `setup` est correct | `src/app.rs:888` |
| `WebviewWindowBuilder::shadow(bool)` | ✅ existe — la réserve du plan est levée | `src/webview/webview_window.rs:612` |
| `set_ignore_cursor_events(bool)` | ✅ sur `WebviewWindow` | `src/webview/webview_window.rs:2133` |
| `set_position<Pos: Into<Position>>` | ✅ et `From<PhysicalPosition<P>> for Position` existe | `webview_window.rs:2255`, `dpi-0.1.2/src/lib.rs:771` |
| `Manager::get_webview_window(&str)` | ✅ | `src/lib.rs:576` |
| `App::handle() -> &AppHandle<R>` | ✅ | `src/app.rs:1259` |
| `WebviewWindow: Send` | ❓ **non vérifiable** — aucune implémentation explicite, déduit des champs | — |

### Conséquence sur le code du spike

La dernière ligne a changé la conception. Plutôt que de déplacer une `WebviewWindow` dans
un thread — ce qui exige `Send`, non vérifiable sans compilateur — le spike passe par
`AppHandle` (le point d'entrée multi-thread documenté de Tauri) et retrouve la fenêtre par
son label à chaque image.

Un accès à une table de hachage par image est négligeable, et le motif gère en prime le
cas de la fenêtre fermée. **À conserver pour l'étape 1.**

---

## Résultat des vérifications

Effectuées le **2026-09-08**, spike lancé par `.\target\debug\spike-overlay.exe`
(`cargo run` équivalent — le CLI Tauri n'a jamais été nécessaire).

### Topologie des écrans relevée

```
écran 0 : position=(0, 0) taille=(1920 × 1080) échelle=1
écran 1 : position=(1920, 0) taille=(1920 × 1080) échelle=1
bureau virtuel : x de 0 à 3840
```

Deux points à retenir, car ils **ferment deux questions et en ouvrent une** :

- **Aucune coordonnée négative** sur cette machine (écran secondaire à droite). Le piège
  n° 7 du plan — « bureau virtuel à coordonnées négatives » — ne peut donc pas se
  manifester ici. Le code ne doit pour autant **jamais** supposer `x ≥ 0` : brancher un
  écran à gauche suffit à le faire mentir.
- **Échelle 1 sur les deux écrans.** Le comportement multi-DPI est donc **non observable
  sur cette machine** : le spike ne peut pas répondre à cette question, et la
  vérification n° 7 ne teste ici que le franchissement de frontière, pas le changement
  d'échelle. C'est la seule inconnue que l'étape 0 laisse derrière elle.

### Les sept propriétés

Les propriétés 1, 2, 3, 5, 6 et 7 ont été **constatées à l'œil**. La propriété 4 a été
vérifiée **par sonde Win32** plutôt qu'en cliquant (voir ci-dessous) — c'est une meilleure
preuve, pas un contournement.

| # | Propriété | Résultat | Observations |
|---|---|---|---|
| 1 | Transparence réelle | ✅ | aucun carré opaque. `WS_EX_LAYERED` présent |
| 2 | Premier plan | ✅ | `WS_EX_TOPMOST` présent ; reste visible sur fenêtre maximisée |
| 3 | Hors taskbar et Alt+Tab | ✅ **avec réserve** | absent des deux à l'œil, **mais `WS_EX_TOOLWINDOW` n'est pas posé** — voir « les deux découvertes » |
| 4 | Clics traversants | ✅ | `WS_EX_TRANSPARENT` posé ; `WindowFromPoint` renvoie la fenêtre du dessous sur 5/5 échantillons |
| 5 | Pas de vol de focus | ✅ **pour la mauvaise raison** | `WS_EX_NOACTIVATE` **absent** : le focus est préservé seulement parce que la fenêtre est inatteignable au clic — voir « les deux découvertes » |
| 6 | Fluidité à 60 Hz | ✅ | déplacement régulier, aucune saccade ni rémanence. **Le repli 30 Hz + interpolation est inutile** |
| 7 | Multi-écran | ✅ | franchit x = 1920 sans disparaître ni sauter. Changement d'échelle non testable ici |

### La sonde Win32 — pourquoi ne pas avoir cliqué

Le clic traversant n'est pas un comportement à constater, c'est un **attribut de fenêtre**.
Plutôt que de cliquer une icône sous la silhouette, on interroge Windows directement :

| Ce qu'on demande | Ce qu'on apprend |
|---|---|
| `GetWindowLong(hwnd, GWL_EXSTYLE)` | quels styles étendus sont réellement posés |
| `WindowFromPoint(centre de la fenêtre)` | quelle fenêtre recevrait un clic à cet endroit |

Sortie relevée (`exstyle = 0x000C0138`) :

```
WS_EX_LAYERED     (composition alpha)    OUI
WS_EX_TOPMOST     (premier plan)         OUI
WS_EX_TRANSPARENT (clics traversants)    OUI
WS_EX_TOOLWINDOW  (hors Alt+Tab)         non
WS_EX_NOACTIVATE  (pas de vol focus)     non

WindowFromPoint, 5 échantillons à 200 ms d'intervalle, la fenêtre se déplaçant :
  (2911, 227) → DirectUIHWND    TRAVERSE
  (2836, 214) → DirectUIHWND    TRAVERSE
  (2771, 219) → DirectUIHWND    TRAVERSE
  (2711, 237) → DirectUIHWND    TRAVERSE
  (2646, 270) → DirectUIHWND    TRAVERSE
```

Cette preuve est **plus forte** qu'un clic réussi : elle vaut pour tout point de la
fenêtre et pour toute application dessous, là qu'un clic n'aurait testé qu'un point et
qu'une application. Elle est aussi rejouable sans les yeux de personne.

Le script est jetable comme le spike ; il n'est pas conservé. Sa substance est ce tableau.

---

## Les deux découvertes — ce que l'œil ne pouvait pas voir

Ce sont les vrais gains de l'étape 0. Les deux portent sur des styles **absents**, et un
style absent ne se voit pas : il se manifeste plus tard, quand une autre pièce arrive.

### ① `WS_EX_NOACTIVATE` manque, et l'étape 1 va en avoir besoin

La propriété 5 est verte, mais **pas pour la raison qu'on croyait**. Le spike n'a pas
`WS_EX_NOACTIVATE` : rien n'empêche sa fenêtre d'être activée. Elle ne l'est jamais
uniquement parce que `WS_EX_TRANSPARENT` la rend **inatteignable au clic**. Le focus est
préservé par accident de configuration, pas par intention.

Or l'étape 1 casse précisément cette protection. La spec §3.3 réactive les clics quand le
curseur entre dans la hitbox de la pose courante — c'est ce qui rend le personnage
attrapable. À cet instant, `WS_EX_TRANSPARENT` est retiré, et **attraper le personnage
activerait sa fenêtre** : le focus quitterait l'éditeur en cours de frappe. Exactement ce
que « il ne gêne jamais » interdit.

`.focused(false)` de Tauri ne couvre pas ce cas : il ne concerne que l'affichage initial,
pas l'activation par clic.

> **Conséquence** : la fenêtre de personnage de l'étape 1 doit porter `WS_EX_NOACTIVATE`
> **dès sa création**, avant que la hitbox n'existe. Sinon le bug apparaîtra en même temps
> que l'attrapabilité, et les deux se diagnostiqueront ensemble — pour rien.

### ② `WS_EX_TOOLWINDOW` manque, et l'exclusion d'Alt+Tab n'est donc pas garantie

À l'œil, `spike-overlay` n'apparaît ni dans la barre des tâches ni dans Alt+Tab. Mais le
style qui *garantit* l'exclusion d'Alt+Tab n'est pas posé : `.skip_taskbar(true)` de Tauri
passe par `ITaskbarList::DeleteTab`, qui retire de la **barre des tâches** — Alt+Tab n'en
découle pas, il l'accompagne par heuristique de Windows 11.

Le plan avait anticipé ce cas (Tâche 3 Step 3, ligne « 3 — visible en Alt+Tab ») en le
classant en réserve mineure. Il l'est ici, puisque le résultat observé est bon. Mais on ne
s'appuie pas sur une heuristique pour une propriété qui a un style dédié.

> **Conséquence** : poser `WS_EX_TOOLWINDOW` à l'étape 1, en même temps que
> `WS_EX_NOACTIVATE` — même appel, même endroit, coût nul.

### Comment les poser — vérifié dans les sources

Tout a été relu dans les sources téléchargées, comme pour les six appels du tableau
précédent. Rien ici n'est écrit de mémoire.

| Élément | Vérifié | Emplacement |
|---|---|---|
| `WebviewWindow::hwnd() -> Result<HWND>` | ✅ sous `#[cfg(windows)]` | `tauri-2.11.5/src/webview/webview_window.rs:1847` |
| `GetWindowLongPtrW(HWND, WINDOW_LONG_PTR_INDEX) -> isize` | ✅ | `windows-0.61.3/…/WindowsAndMessaging/mod.rs:1132` |
| `SetWindowLongPtrW(HWND, WINDOW_LONG_PTR_INDEX, isize) -> isize` | ✅ | idem `:2262` |
| `GWL_EXSTYLE` | ✅ `WINDOW_LONG_PTR_INDEX(-20)` | idem `:3755` |
| `WS_EX_NOACTIVATE` / `WS_EX_TOOLWINDOW` | ✅ `WINDOW_EX_STYLE(134217728)` / `(128)` | idem `:7276`, `:7286` |
| *feature* à activer | ✅ `Win32_UI_WindowsAndMessaging` | `windows-0.61.3/Cargo.toml:728` |

#### Contrainte non évidente : la version de la crate `windows` doit être celle de Tauri

Tauri 2.11.5 dépend de **`windows` 0.61.3**, et son `hwnd()` renvoie le `HWND` de *cette*
version (`use windows::Win32::Foundation::HWND;`, `webview_window.rs:43`).

En Rust, **deux versions majeures d'une même crate produisent deux types distincts**, même
de nom identique. Prendre `windows = "0.62"` dans notre `Cargo.toml` ferait que le `HWND`
rendu par `hwnd()` ne serait pas celui attendu par notre `SetWindowLongPtrW` — et l'erreur
du compilateur parlerait de deux types nommés pareil, ce qui est déroutant la première
fois.

> **Épingler `windows = { version = "0.61", features = ["Win32_UI_WindowsAndMessaging"] }`**
> et le vérifier dans `Cargo.lock` (une seule entrée `name = "windows"`) à chaque montée
> de version de Tauri.

#### Deuxième piège : `WINDOW_EX_STYLE` est un *newtype*, pas un entier

`GetWindowLongPtrW` rend un `isize` nu, mais les constantes sont des
`WINDOW_EX_STYLE(u32)`. On ne peut donc pas les combiner par `|` directement : il faut
sortir la valeur du newtype par `.0`, puis la convertir.

```rust
// `unsafe` : ces deux appels franchissent la frontière FFI vers Win32.
// Rust ne peut pas garantir que `hwnd` est un handle valide — c'est nous
// qui l'affirmons, ce qui est légitime : il sort de `build()` juste avant.
unsafe {
    let actuels = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);

    // `.0` extrait le u32 du newtype `WINDOW_EX_STYLE` ; `as isize` l'aligne
    // sur le type de retour de Get/SetWindowLongPtrW. Sans ces deux
    // conversions, le `|` ne compile pas.
    let ajout = (WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) as isize;

    // On ajoute nos bits sans écraser ceux que Tauri a déjà posés
    // (LAYERED, TOPMOST, TRANSPARENT) : d'où le `|` et non une affectation.
    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, actuels | ajout);
}
```

**Ne pas** écrire `SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ajout)` : cela retirerait
`WS_EX_LAYERED` et `WS_EX_TOPMOST`, donc la transparence et le premier plan — les deux
propriétés que ce spike vient d'établir.

---

## Décision

### ✅ **Poursuivre en Tauri.** Sans réserve bloquante.

Les trois propriétés dont l'absence aurait renvoyé la stack vers Electron — transparence
réelle (1), premier plan (2), clics traversants (4) — sont acquises **et** expliquées par
des styles étendus vérifiés, pas seulement constatées. Aucun remède du plan n'a eu à être
appliqué : le spike a fonctionné du premier coup une fois la chaîne C++ en place.

Le seul point faible attendu de Tauri face à Electron était l'overlay transparent. Il
tient. Le design reste valable à 100 % : ni §3.1 (toute la logique en Rust) ni §4
(distribution) ne bougent.

**L'interdiction d'écrire de la physique est levée.**

## Conséquences pour l'étape 1

| | Ce que l'étape 0 impose ou permet |
|---|---|
| ✅ | Le motif `AppHandle` + recherche par label est retenu pour `render.rs` |
| ✅ | **60 Hz est fluide** : la cadence visée est la cadence retenue. Le repli « 30 Hz + interpolation » de la spec §12 est **abandonné** — ne pas le porter dans le plan de l'étape 1 |
| ✅ | La combinaison `decorations(false)` + `transparent(true)` + `always_on_top(true)` + `skip_taskbar(true)` + `shadow(false)` + `focused(false)` est la bonne base, à recopier telle quelle |
| ⚠️ | **Ajouter `WS_EX_NOACTIVATE`** dès la création de la fenêtre — sans quoi attraper le personnage volera le focus (découverte ①) |
| ⚠️ | **Ajouter `WS_EX_TOOLWINDOW`** au même endroit (découverte ②) |
| ⚠️ | Ne **jamais** supposer `x ≥ 0` sur le bureau virtuel, bien que ce soit vrai sur cette machine |
| ⚠️ | **Épingler `windows = "0.61"`**, la version dont dépend Tauri 2.11.5 — sinon les `HWND` sont deux types distincts |
| ✅ | `hwnd()`, `Get/SetWindowLongPtrW`, `GWL_EXSTYLE` et les constantes de style sont **tous vérifiés dans les sources** : l'étape 1 n'a plus d'incertitude d'API à lever |
| ⬜ | **Le multi-DPI reste non vérifié** — les deux écrans sont à l'échelle 1. À traiter comme un risque ouvert, pas comme un acquis. Il ne se manifestera qu'à l'étape 4 (fenêtres) ou sur une autre machine. **C'est la seule inconnue que l'étape 0 laisse ouverte** |
