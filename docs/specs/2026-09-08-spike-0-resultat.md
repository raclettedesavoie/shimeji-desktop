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

### Bloquant : la charge de travail C++ est absente

Confirmé de trois façons indépendantes :

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

## Résultat des vérifications visuelles

⬜ **Non effectuées** — nécessite la charge de travail C++, puis `cargo tauri dev`.

Topologie des écrans relevée (sortie console attendue au lancement) :

```
(coller ici les lignes « écran <i> : … » et « bureau virtuel : … »)
```

### Les sept propriétés

| # | Propriété | Résultat | Observations |
|---|---|---|---|
| 1 | Transparence réelle | ⬜ | |
| 2 | Premier plan | ⬜ | |
| 3 | Hors taskbar et Alt+Tab | ⬜ | |
| 4 | Clics traversants | ⬜ | |
| 5 | Pas de vol de focus | ⬜ | |
| 6 | Fluidité à 60 Hz | ⬜ | |
| 7 | Multi-écran | ⬜ | |

Les remèdes établis pour chaque échec sont dans le plan, Tâche 3 Step 3.

### Décision

⬜ En attente des vérifications visuelles.

### Conséquences pour l'étape 1

- ✅ Le motif `AppHandle` + recherche par label est retenu pour `render.rs`.
- ⬜ Cadence de déplacement à confirmer (60 Hz visé ; repli 30 Hz + interpolation).
- ⬜ Comportement multi-DPI à observer.
