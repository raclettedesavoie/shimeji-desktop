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
| Visual Studio 2022 Community | installé, **sans la charge de travail C++** ❌ |
| `cargo-tauri` | non installé ⬜ |
| tauri (résolu par cargo) | **2.11.5** |

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

**À retenir** : une fois la charge C++ installée, rustc localise le linker via
l'installation Visual Studio et passe un chemin absolu ; le problème disparaît de
lui-même. Mais **si l'erreur `extra operand` persiste après installation**, la cause est
le `PATH` — lancer alors la compilation depuis un *Developer PowerShell for VS 2022*, ou
depuis PowerShell plutôt que depuis un shell Git Bash.

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
