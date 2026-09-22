# shimeji-desktop

Un *desktop pet* pour Windows. Des personnages en pixel-art vivent sur le bureau :
ils se déplacent, se reposent, **grimpent sur les bords des fenêtres ouvertes**, et
**réagissent à l'activité de la machine**. Compatible avec les packs de personnages
**Shimeji** existants — on en dépose un dans un dossier, il fonctionne.

Projet plaisir, personnel, Windows uniquement.

---

## Le besoin

### Ce que le personnage fait

Il n'attend aucun ordre. Il **enchaîne des activités tout seul** avec une part
d'aléatoire, pour qu'on ne devine jamais la suite : se déplacer, s'arrêter, s'asseoir,
dormir, manger, sauter, s'amuser. L'objectif n'est pas la liste des animations, c'est
**qu'il ait l'air vivant** — jamais figé, jamais prévisible.

### Où il vit

Il n'est pas cantonné au bas de l'écran. Il **explore** :

- il marche sur le bureau, au-dessus de la barre des tâches
- il **grimpe le long des bords des fenêtres** et s'assoit sur leur barre de titre
- il escalade les bords de l'écran, se suspend au plafond
- si la fenêtre sous ses pieds disparaît, **il tombe** — et il atterrit
- il circule sur **tous les écrans**

### Ce à quoi il réagit

| Quand… | …il |
|---|---|
| l'utilisateur ne touche plus à rien | s'assoit, puis s'endort |
| l'utilisateur revient | se réveille |
| il est midi | mange |
| il est tard le soir | traîne, dort davantage |
| l'application au premier plan change | change d'attitude **et se déplace vers cette fenêtre** |
| la batterie est faible | fatigue |
| la session est verrouillée | disparaît proprement |

### Interaction

On peut **l'attraper à la souris et le lâcher** n'importe où (il tombe et se rattrape),
**cliquer dessus** pour le faire réagir, et le **cacher ou le rappeler** à tout moment.
Il ne gêne jamais : pas de vol de focus, absent de la barre des tâches, les clics le
traversent.

### Plusieurs personnages

Plusieurs personnages coexistent à l'écran et **se remarquent** : ils s'approchent et
réagissent l'un à l'autre. Ajouter un personnage est une **opération de contenu**
(déposer un dossier), jamais une modification de code.

### Hors sujet — décisions déjà prises

- ❌ **Pas un Tamagotchi** — aucune stat à surveiller, aucune obligation, il ne meurt pas
- ❌ **Aucune capture de frappe** — on sait seulement si l'utilisateur est actif, jamais quelle touche
- ❌ **Pas un assistant** — il ne notifie rien, ne rappelle rien, n'a aucune utilité productive
- ❌ **Pas un jeu** — pas de score, pas de progression
- ❌ **Pas de charge CPU comme signal** — écarté explicitement
- ❌ **Pas de bulles de dialogue**

C'est du plaisir visuel, rien d'autre. **Toute idée qui ajoute une obligation est hors sujet.**

---

## Stack

| Couche | Choix |
|---|---|
| Application | **Tauri v2** — binaire natif ~10 Mo |
| Logique | **Rust** — toute la logique, sans exception |
| Affichage | **Webview** — HTML/CSS minimal, ~60 lignes de JS |
| APIs Windows | crate **`windows`** (officielle Microsoft), bindings typés |
| Contenu | fichiers **externes**, à côté de l'exe |

Volontairement **pas** de TypeScript, pas de bundler, pas de framework front, pas de
canvas. Le webview est un afficheur de sprite délibérément bête.

### Pourquoi Tauri plutôt qu'Electron

Ce projet a deux moitiés : l'animation d'un personnage (le web est excellent) et
l'introspection des fenêtres Windows (`EnumWindows`, `DwmGetWindowAttribute`,
`GetForegroundWindow`). Electron n'atteint la seconde que par FFI, en décrivant à la
main des structs win32 en JS — un mauvais calibre de struct donne un segfault, pas une
erreur, et c'est sur l'API la plus centrale du projet. Rust les expose **typées**.
Bonus : ~10 Mo à distribuer au lieu de ~150.

### Distribution

Rust est une dépendance de **compilation**, pas d'exécution. On livre un exe ; la
machine cible n'a besoin que de **WebView2** (déjà présent sur Windows 11) et du runtime
Visual C++ — à **lier statiquement** pour que l'exe soit totalement autonome.

### Chaîne d'outils — opérationnelle (vérifié le 2026-09-08)

**Tout est installé, ne rien réinstaller.** Le spike de l'étape 0 compile.

| Outil | État |
|---|---|
| rustc / cargo | ✅ 1.98.1 |
| cible `x86_64-pc-windows-msvc` | ✅ installée |
| Charge C++ / MSVC 14.51.36231 / SDK 10.0.26100.0 | ✅ sur **Visual Studio 18 Insiders** |
| WebView2 | ✅ 152.0.4191.66 |
| `cargo-tauri` | ✅ `tauri-cli 2.11.4` — **indispensable** pour l'installateur (voir plus bas) |
| Node | ⚠️ v14.17.0 — **délibérément inutilisé** |

**Trois pièges de cette machine, chacun ayant déjà coûté un diagnostic :**

> **1. La charge C++ vit sur VS 18 Insiders, pas sur 2022 Community.** `vswhere` ne la
> voit **qu'avec `-prerelease`** ; sans ce drapeau on conclut à tort qu'elle est absente.
> rustc, lui, la détecte seul — aucune variable d'environnement, aucun *Developer
> PowerShell* nécessaire.

> **2. Ne pas installer Node.** Tauri s'installe par npm *ou* par cargo. La voie npm
> exigerait Node 18+, que cette machine n'a pas — mais le front est statique, donc **Node
> ne sert nulle part**. Il n'y a rien à corriger.

> **3. Compiler depuis PowerShell, pas depuis Git Bash.** Dans un shell Git Bash, rustc
> peut pêcher le `link.exe` de Git for Windows (un utilitaire coreutils de liens durs) au
> lieu du linker MSVC, et rendre une erreur `extra operand` totalement opaque.

### Compiler et lancer

Depuis **PowerShell** (voir piège 3).

```powershell
cd C:\Users\alri\Documents\shimeji-desktop\src-tauri
cargo test               # la suite complète, sans écran
cargo run                # l'application : un personnage sur le sol
cargo run -- --sim 30    # 30 min de comportement sans écran (spec §10.3)
cargo run -- --sim 1440  # 24 h : la preuve d'ensemble de l'étape 2, voir plus bas
cargo run -- --demarrage etat|on|off   # le démarrage avec Windows, scriptable
cargo run -- --installer <slug>        # installe un pack du catalogue dans %APPDATA%
cargo tauri build                      # l'INSTALLATEUR NSIS, voir l'avertissement ci-dessous
```

> ⚠️ **`cargo build` ne produit PAS d'installateur, et l'ignore en silence.** Le bloc
> `bundle` de `tauri.conf.json` n'est lu que par la **CLI** ; `cargo` ne le voit jamais.
> L'installateur ne sort que de `cargo tauri build`, dans
> `target/release/bundle/nsis/`. Le tableau d'outillage ci-dessus a longtemps porté
> « `cargo-tauri` non installé, et **inutile** » — c'était faux sur les deux points.
>
> ⚠️ **Et l'installateur ne livre `characters/` que par la clé `resources`** de
> `tauri.conf.json`. Sans elle, NSIS pose l'exe nu : `resoudre("characters")` ne trouve
> rien à côté de l'exe, et l'application **installée** démarre avec zéro personnage et
> aucun moyen d'en obtenir un. Ça se vérifie **sans installer** :
> `Select-String target/release/nsis/x64/installer.nsi -Pattern characters` — zéro
> occurrence veut dire pack absent.

> Les personnages se cherchent dans **deux** dossiers : la bibliothèque
> `%APPDATA%\shimeji-desktop\characters\`, puis le `characters/` du dépôt —
> qui ne contient plus que `blob`. Voir « Les packs livrés » plus bas.

**Treize variables d'environnement de diagnostic.** Les trois premières ont chacune
servi à démentir une hypothèse fausse — voir « Mesurer le CPU » plus bas ; les autres
remplacent un clic dans le tray ou dans une fenêtre, ou rendent observable un calcul qui,
sinon, ne se verrait qu'à l'œil et sur plusieurs minutes :

| Variable | Ce qu'elle fait |
|---|---|
| `SHIMEJI_CADENCE=1` | images/s réelles, travail par image, **et le taux de déplacement** |
| `SHIMEJI_SANS_BOUCLE=1` | crée la fenêtre et n'anime rien |
| `SHIMEJI_TRACE=1` | trace chaque image servie par le schéma URI |
| `SHIMEJI_CACHE=1` | démarre caché, comme si « Afficher » était décoché |
| `SHIMEJI_QUITTER_APRES=<s>` | appelle `exit(0)` — la ligne de « Quitter » — après *s* secondes |
| `SHIMEJI_SIGNAUX=1` | imprime, deux fois par seconde, les **six** signaux et le biais qu'ils produisent — le sixième est la **latence** du thread principal (régulation de charge) |
| `SHIMEJI_ESCALADE=1` | force l'intention `Grimper` dès la première image, et trace (phase, face, offset, pose) à chaque changement — étape 4a, voir plus bas « mesurer l'ancre » |
| `SHIMEJI_MENU=1` | signale quand Windows **refuse le premier plan** à l'ouverture du menu contextuel — la cause du menu qui reste collé à l'écran, voir `render::prendre_le_premier_plan` |
| `SHIMEJI_CATALOGUE=1` | ouvre la **fenêtre du catalogue** au démarrage — l'équivalent scriptable de l'entrée de menu, et ce qui a prouvé que l'IPC de Tauri répondait |
| `SHIMEJI_PERSONNAGES=<a>,<b>,…` | le **roster de départ**, doublons compris (`blob,blob` = deux blob) — l'équivalent scriptable des clics dans « Ma bibliothèque » |
| `SHIMEJI_ROSTER=<s>:<a>,<b>` | un **changement de roster** après *s* secondes. C'est le seul moyen d'observer un **départ** sans qu'un humain clique |
| `SHIMEJI_ONBOARDING=1` | force **l'assistant de première configuration**, sans toucher au `config.json` — évite d'avoir à le supprimer entre deux essais |
| `SHIMEJI_TOAST=1` | trace le résultat du **toast** de fin d'assistant, succès comme échec |

**Et un fichier témoin** : créer `characters/recharger.txt` déclenche un rechargement à
chaud, puis le fichier est supprimé.

> **Le principe, récurrent sur ce projet : tout ce qui demanderait un clic reçoit un
> équivalent scriptable.** Sonde de styles Win32, `--demarrage etat`, fichier témoin,
> `SHIMEJI_CACHE`, `SHIMEJI_QUITTER_APRES`, énumération des fenêtres, lecture de
> l'en-tête PE — **tout se vérifie sans humain**.
>
> ✅ **La seule exception a été levée le 2026-09-10** : que le menu du tray *dépêche*
> ses clics ne se script pas, et un humain a donc cliqué « Quitter » une fois, sur le
> build release. Processus disparu, aucun résidu. Un seul clic suffisait : les cinq
> entrées partagent le même gestionnaire d'événements.

> ⚠️ **`cargo test` ne reconstruit pas l'exe.** Il compile le harnais de test. Après une
> correction, `cargo build` avant de relancer l'application — sinon on vérifie un binaire
> plus ancien que la source, et l'on conclut à tort que le correctif ne marche pas. C'est
> arrivé une fois, sur le correctif du BOM.

Le **spike de l'étape 0** est archivé dans `docs/spike-etape-0/` (`cargo run` y fonctionne
aussi). Voir son `README.md` pour ce qu'il prouve et la sonde de styles Win32 qui
l'accompagne. **Ne pas le faire évoluer vers l'application** : il lui manque délibérément
les deux styles étendus que son analyse a révélés nécessaires.

### ⚠️ Mesurer le CPU — vérification systématique

**Un desktop pet qui consomme se fait désinstaller.** Toute modification du chemin à
60 Hz — la boucle de `main.rs`, `render.rs`, la sonde — doit être suivie d'une **mesure**,
pas d'une intuition.

> ⚠️⚠️ **Mesurer sur 10 secondes ne veut rien dire.** La consommation dépend de ce que le
> personnage *fait* : en marche il déplace sa fenêtre à chaque image, à l'arrêt jamais. Le
> taux de déplacement va de **0 % à 87 %** selon la tranche. **Toujours 40 à 60 s.**
>
> Et depuis l'étape 2, **la mesure « en marche » ne se compare plus d'une version à
> l'autre** : les signaux changent le comportement, donc la charge. Pour comparer deux
> versions, utiliser une configuration où la charge ne dépend **pas** du comportement —
> `SHIMEJI_CACHE=1` (la boucle tourne entière, rien n'est déplacé) ou le travail par image
> de `SHIMEJI_CADENCE=1`.

```powershell
$p = Get-Process -Name shimeji-desktop
$c = $p.CPU; Start-Sleep -Seconds 60; $p.Refresh()
"$([math]::Round((($p.CPU - $c) / 60) * 100, 1)) % d'un coeur"
```

**Le diagnostic, acquis et non supposé** (le détail des mesures : `docs/specs/2026-09-09-mesure-cpu.md`) :

| Configuration, build `release` | CPU |
|---|---|
| fenêtre seule, aucune boucle (`SHIMEJI_SANS_BOUCLE=1`) | **0 %** |
| **caché** (`SHIMEJI_CACHE=1`) = tout notre calcul, zéro déplacement | **0,9 %** |
| en marche | **12 %** |

Donc : **0,9 % = tout ce que nous calculons**, les ~11 points restants = `SetWindowPos` sur
une fenêtre en couche. Le coût est proportionnel au **nombre de déplacements**, et c'est le
seul levier. **Optimiser notre code ne rapporterait rien.**

**Trois choses à ne PAS faire**, chacune démentie par la mesure :

1. **Ne pas descendre la cadence sous 60 Hz** — le travail par image ne fait que 1 à 5 % du
   budget de 16,7 ms. Ce serait payer en fluidité ce qui ne coûte rien.
2. **Ne pas repasser le profil en `opt-level = 3`** — le `release` ne gagne rien sur le
   debug (le coût est chez Windows), et on paierait la taille de l'exe.
3. **Ne pas conclure sans `SHIMEJI_CACHE=1`** — quatre hypothèses « évidentes » ont été
   formulées puis démenties par la mesure. Elles sont consignées dans le dossier CPU
   précisément parce qu'elles paraissaient toutes justes.

### ⚠️ Semer l'aléatoire une seule fois — le piège des graines séquentielles

**Re-semer `XorShift32::seeded(n)` avec de petits entiers séquentiels biaise le PREMIER
tirage.** L'état initial d'un petit entier laisse `next_u32` dans les bits de poids
faible, donc `weighted` retombe systématiquement sur l'**index de poids faible** de la
table. Une boucle `for graine in 1..200` qui n'observe qu'un tirage par graine mesure
donc toujours la même chose.

Constaté le 2026-09-10, en mesurant si un réveil pouvait replonger dans le sommeil :

| Méthode | Ce qu'elle a rendu |
|---|---|
| re-semer par petits entiers, un tirage chacun | **0 sur 199** — un faux négatif complet |
| semer **une fois**, laisser l'état avancer | **225 sur 2000**, soit le 1/8 attendu |

La première méthode aurait classé un défaut réel comme inexistant. **Semer une fois et
laisser l'état avancer** est la seule méthode fiable pour mesurer une distribution ; la
graine explicite reste là pour la **reproductibilité**, pas pour l'échantillonnage.

### ⚠️ Le coût d'une session d'assistance — mesuré le 2026-09-14

Les sessions sur ce projet coûtaient cher. La mesure, faite sur les transcriptions de
`~/.claude/projects/`, a démenti l'hypothèse évidente (« c'est CLAUDE.md qui est trop
gros ») — encore une, après les quatre du dossier CPU :

| Session `f8053c5f` | requêtes | contexte relu |
|---|---|---|
| le fil principal | 297 | 83,9 M |
| **ses 25 sous-agents** | **1 426** | **211,0 M** |

**Les sous-agents faisaient 72 % du coût.** Chacun redémarre avec un contexte complet —
CLAUDE.md compris — et produit ses propres tours. Une session sans aucun agent
(`817c157f`) a coûté 79,6 M pour 425 requêtes : le fil principal seul plafonne vers 80 M.

Trois règles en découlent, la première valant les deux autres réunies :

1. **Un sous-agent doit rapporter une conclusion, pas explorer à l'aveugle.** Il est
   rentable pour balayer beaucoup de fichiers et n'en ramener que la réponse ; il est
   ruineux lancé par réflexe, ou en escadrille sur une tâche séquentielle.
2. **Borner les sorties de build.** Une erreur `cargo` non tronquée pèse plusieurs
   milliers de tokens : `cargo test --quiet`, et `cargo build 2>&1 | Select-Object -Last 40`.
3. **Ce fichier reste court.** Il est relu à chaque session *et par chaque sous-agent*.
   Le récit va dans `docs/` ; ici ne restent que les règles encore actives. Il est passé
   de 54,7 à 36,8 Ko le 2026-09-14 par ce principe.

→ Le détail de la mesure : `docs/conception/2026-09-14-cout-des-sessions.md`.

### L'auteur apprend Rust sur ce projet

Le choix de Tauri est en partie motivé par l'envie d'apprendre Rust. Conséquence pour
toute session d'assistance : **expliquer le Rust plutôt que le supposer acquis** — la
propriété, l'emprunt, les traits, `Result`. Préférer du code simple et lisible à du code
idiomatique et dense : un `match` explicite vaut mieux qu'une chaîne de combinateurs
astucieuse.

---

## Conventions de code

**Le code de ce projet doit être abondamment commenté**, en français, pour être relu
facilement des semaines plus tard par quelqu'un qui apprend encore Rust. C'est une
exigence explicite de l'auteur, pas une préférence de style — ne pas « nettoyer » les
commentaires au nom de la concision.

### Commenter le *pourquoi*, pas le *quoi*

```rust
// ✗ inutile — le code le dit déjà
// incrémente le compteur
count += 1;

// ✓ utile — explique une décision non évidente
// On repart du rectangle courant de la plateforme au lieu de mémoriser la
// position : c'est ce qui rend gratuit le déplacement de la fenêtre (décision n° 1).
let pos = platform.rect.point_on(face, offset);
```

### Renvoyer à la décision que le code applique

Quand un bloc met en œuvre une décision du design, **citer la section de la spec**. C'est
ce qui permet, en relisant, de retrouver le raisonnement sans le reconstituer :

```rust
// Les clics traversent en permanence ; on ne les réactive que dans la
// hitbox de la pose courante (spec §3.3).
win.set_ignore_cursor_events(true)?;
```

### Expliquer les constructions Rust non évidentes

Tout ce qui n'est pas du Rust élémentaire mérite une ligne : `let Some(x) = … else`,
les durées de vie annotées, `Arc`/`Mutex`, les `impl Trait`, les combinateurs sur
`Option`/`Result`, et toute raison liée à l'emprunt.

```rust
// `let … else` : si la fenêtre a été fermée, on sort du thread. Équivalent
// d'un `match` dont la branche None ferait `return`.
let Some(win) = handle.get_webview_window(LABEL) else {
    return;
};
```

### Découper les fonctions longues par bandeaux

Les boucles et les `setup` deviennent vite illisibles. Les scander :

```rust
// ── Topologie des écrans ────────────────────────────────
// ── La fenêtre du personnage ────────────────────────────
// ── Déplacement à 60 Hz ─────────────────────────────────
```

### En-tête de fichier

Chaque module s'ouvre sur deux ou trois lignes disant **sa responsabilité unique** et la
section de la spec dont il relève. Si cet en-tête devient difficile à écrire, c'est que le
fichier fait trop de choses.

### ⚠️ Ne jamais éditer un fichier source par PowerShell `Get-Content`/`Set-Content`

Les sources de ce projet sont en **UTF-8 accentué**, et PowerShell 5.1 relit un
fichier en **cp1252** puis le réécrit en UTF-8 : tout le fichier est
double-encodé (« même » devient « mÃªme »), et un **BOM** s'ajoute en tête.

**Le code compile et les tests passent** — seuls les commentaires sont touchés,
ce qui est exactement ce qui rend le défaut facile à ne pas voir. Arrivé le
2026-09-15 sur `main.rs` : 660 lignes corrompues pour corriger *un* commentaire.

Les éditions passent donc par un outil d'édition, ou par Python en UTF-8
explicite. La réparation, si le mal est fait, demande de reconstruire la table
inverse de cp1252 à la main — .NET laisse passer les cinq octets que cp1252 ne
définit pas (0x81, 0x8D, 0x8F, 0x90, 0x9D), là où Python refuse de les encoder.

### Ce qui ne s'explique pas en commentaire

Un nom mal choisi ne se rattrape pas par un commentaire. Nommer d'abord, commenter
ensuite.

### ⚠️ Toute nouvelle action se branche au menu du clic droit

**Dès qu'une intention jouable est ajoutée, elle est ajoutée au menu contextuel du
personnage — dans la même tâche, pas « plus tard ».** C'est une ligne dans la table
`ENVIES` de `src-tauri/src/menu_perso.rs`, et rien d'autre : l'identifiant est décodé
par `commande_de`, la disponibilité est déduite du manifeste (couverture partielle,
spec §8.6), et `actions::executer` n'a aucun cas à ajouter.

L'oubli ne casse **aucun test** et ne produit **aucun message** : l'intention existe
pour le tirage aléatoire, mais reste à jamais hors de portée de l'utilisateur. C'est
précisément pourquoi la règle est écrite ici plutôt que laissée au bon sens.

#### Le menu dépend de l'endroit où il est (étape 4a, tâche du menu de l'escalade)

Un personnage accroché à un mur ou au plafond n'a plus le même menu qu'au sol : lui
proposer « Flâner » ou « S'asseoir » le ferait tomber (règle de sécurité du monde
vertical), et un personnage encore au sol n'a que faire de « Se lâcher ». `ENVIES` porte
donc, pour chaque entrée, la liste des contextes (`menu_perso::Ou : Sol | Mur | Plafond`)
où elle a du sens, et `menu_perso::ou_de(&ch.attachment)` calcule celui du personnage
courant, appelé juste avant `ouvrir` dans `main.rs`.

| Où il est | Le menu propose |
|---|---|
| au sol (face `Top`), en chute, ou porté | Flâner · S'asseoir · Faire tourner la tête · Balancer les jambes · Grimper au mur — inchangé |
| sur un mur (face `Left`/`Right`) | Monter plus haut · Rester accroché · Redescendre · Se lâcher |
| au plafond (face `Bottom`) | Rester accroché · Se lâcher |

Pas de « Redescendre » au plafond, et c'est délibéré : il faudrait traverser jusqu'au
bord, basculer sur un mur, puis descendre — de la navigation calculée, que la décision
n° 4 exclut (YAGNI). Shimeji ne le propose pas non plus.

« Grimper au mur » (au sol) et « Monter plus haut » (sur un mur) partagent la même
commande (`Grimper`) sous deux identifiants et deux libellés : c'est la même action,
seul son nom change selon qu'on la déclenche ou qu'on la reprend.

Trois des quatre nouvelles entrées ne sont **pas** des intentions tirables — la table
`ENVIES` porte donc un `menu_perso::Commande` (`Intention(…)`, `ResterAccroche`,
`Redescendre`, `SeLacher`) plutôt qu'une `Intention` nue, et `Entrees::commande` /
`behavior::pas` ont été mis à jour en conséquence :

- **Monter plus haut** est `Commande::Intention(Grimper)`, sans code spécifique : la
  phase `Choisir` de `grimper()` (`intention.rs`) a été corrigée pour **reprendre**
  l'escalade en cours (phase `Paroi` ou `Plafond`, cible tirée au sort) au lieu
  d'échouer sur une face non-`Top` — c'était précisément le bug rapporté à l'écran
  (choisir une entrée de menu pendant qu'on est accroché le faisait tomber).
- **Rester accroché** pose `ActiveIntention::accroche` — l'intention qu'un lancer
  contre une paroi installe déjà (`HoldOntoWall`/`HoldOntoCeiling` de Shimeji-ee).
- **Redescendre** pose `ActiveIntention::redescendre`, qui vise le bas de la face via
  une nouvelle phase `PhaseGrimpe::ChoisirDescente` — décidée à la première image,
  comme `Choisir`, parce que la longueur de la face demande `World`. La descente
  elle-même reste celle de la phase `Paroi` existante : rien n'est réécrit.
- **Se lâcher** efface simplement l'intention (`ch.intention = None`) sans y ajouter la
  moindre ligne de physique : la règle de sécurité du monde vertical, déjà là pour un
  tout autre usage, fait tomber le personnage dans la **même image** — c'est
  `FallFromWall`/`FallFromCeiling` de Shimeji-ee obtenu par pure réutilisation.

`behavior::pas` refuse en plus toute commande devenue impossible entre le clic et
l'image suivante (rechargement à chaud, ou simplement le temps qu'a mis l'utilisateur à
choisir) — une commande de sol reçue pendant qu'il est accroché est **ignorée**, jamais
appliquée : l'appliquer le ferait tomber par le même mécanisme que ci-dessus, pour de
mauvaises raisons cette fois.

> **Et une seconde règle, non négociable : un seul `on_menu_event` dans tout le
> programme.** Tauri livre *tout* événement de menu à *tous* les gestionnaires, quel que
> soit le menu d'origine (`tauri-2.11.5`, `src/tray/mod.rs:326`). Un second gestionnaire
> exécuterait donc chaque action **deux fois** — et deux bascules s'annulent, si bien que
> le clic paraîtrait sans effet. Le gestionnaire unique est installé par `tray.rs` et
> délègue à `actions.rs` ; `menu_perso.rs` ne fait que **proposer**, il ne déclenche
> rien.

---

## Architecture

### Une fenêtre par personnage

Chaque personnage est **sa propre petite fenêtre de 128×128**, transparente, sans
bordure, hors taskbar, toujours au premier plan, déplacée par Rust. C'est ce que fait le
vrai Shimeji.

**Ne pas revenir à un overlay transparent plein écran** : une seule fenêtre ne peut pas
couvrir proprement deux moniteurs de DPI différents, et c'est un défaut structurel. Avec
une fenêtre par personnage, changer d'écran, c'est écrire une position.

Le hit-testing : les clics traversent en permanence, et Rust ne les réactive que quand le
curseur entre dans la **hitbox serrée** de l'animation courante (`GetCursorPos` sondé à
~30 Hz, quasi gratuit).

### Où vit la logique

La position étant appliquée par Rust, faire calculer la physique en JS coûterait un
aller-retour IPC **60 fois par seconde par personnage**. Donc :

| Couche | Où |
|---|---|
| Fenêtres, signaux, tray, config, **physique, comportement** | **Rust** |
| Dessin du sprite | Webview (reçoit `{frame, flip}`, pose un `background-position`) |

Corollaire : Rust détenant **tous** les personnages, le comportement social est une
simple vérification côté coordinateur — aucun canal entre fenêtres à inventer.

### Le monde = une liste de plateformes

Tout ce sur quoi un personnage peut se tenir, s'accrocher ou se suspendre est un
rectangle avec une face utilisable. Deux sources seulement :

- **chaque écran** → sol, 2 murs, plafond
- **chaque fenêtre** → dessus (barre de titre), 2 murs, dessous (suspension)

La physique et les comportements **ne savent jamais** si une plateforme est le bas de
l'écran ou la barre de titre de VSCode. C'est ce qui permet de livrer le sol d'abord et
de brancher les fenêtres ensuite sans rien réécrire.

### Trois horloges

| Rythme | Quoi | Pourquoi |
|---|---|---|
| **60 Hz** | physique, dessin, **et la seule plateforme occupée** | interroger *une* fenêtre est quasi gratuit → il colle à la fenêtre qu'on déplace |
| **~8 Hz** | recensement complet des fenêtres, filtres, occlusion | une fenêtre qui apparaît est vue en ~125 ms : imperceptible |
| **~2 Hz** | inactivité, appli active, heure, batterie | aucun de ces signaux ne bouge vite |

---

## Décisions de design à ne pas défaire

Chacune remplace une classe entière de bugs. Les annuler ramène les bugs.

### 1. Un personnage accroché ne stocke jamais sa position absolue

Il stocke `(plateforme, face, distance au bord)`. Sa position écran est **recalculée à
chaque image** depuis le rectangle courant de la plateforme.

Conséquences **gratuites** : la fenêtre bouge → il voyage avec ; elle est redimensionnée
→ il garde sa distance au bord ; elle rétrécit sous lui ou se ferme → un seul test, il
tombe. Un personnage assis sur une barre de titre qu'on balade est *le* moment qui fait
sourire avec un Shimeji — ici c'est une conséquence du modèle, pas une feature.

### 2. Toujours au premier plan, mais jamais sur un bord recouvert

Il reste visible au-dessus de tout. En échange, **un bord recouvert n'est pas exposé
comme plateforme** : un bord est un segment, on lui retire les fenêtres de z-order
supérieur (soustraction d'intervalles 1D). Il ne peut alors physiquement pas s'asseoir
sur du vide.

### 3. Les signaux biaisent, ils ne commandent pas

Si « inactif 2 min » **déclenchait** le sommeil, on aurait un afficheur d'état système
déguisé en personnage : prévisible, mort en trois jours. « Inactif 2 min » **multiplie
par 8 l'envie de dormir** — il s'endort presque toujours, mais parfois il s'assoit,
parfois il traîne encore. **Cette marge est le produit.**

### 4. La navigation est autorisée à échouer

Pas de calcul de chemin : la carte change 8 fois par seconde, un chemin est périmé avant
d'être parcouru. Décision **locale** à chaque image (cible au-dessus → grimper ; en
dessous → se laisser tomber ; même surface → avancer).

Il se coince parfois, il prend des routes idiotes — **et c'est souhaitable** : un pet qui
prend un chemin bête est attachant, un qui calcule l'itinéraire optimal a l'air d'un
robot.

Une seule règle de sécurité remplace toute l'énumération des cas de blocage :

> **Toute intention a un délai d'abandon (~20 s)**, après quoi elle échoue et il repart
> flâner.

### 5. Le comportement est de la donnée, pas du code

Trois couches, qui ne se parlent que vers le bas :

1. **Réflexes** (60 Hz, non négociables) — plateforme disparue → chute ; attrapé → porté ; contact → atterrissage ; session verrouillée → planqué
2. **Intention** (une seule à la fois) — `Flâner` · `AllerÀ(surface)` · `SeReposer` · `Jouer(action)`
3. **Envie** — tirage pondéré, où les **signaux modifient les poids** et les **slots manquants retirent les options**

Ajouter un signal = ajouter une ligne de table. Aucun nouveau chemin de code. Les poids
vivent dans la config → on règle son caractère **sans recompiler**.

---

## Personnages

Les personnages sont des **fichiers externes**, jamais embarqués dans le binaire : on en
ajoute un sans recompiler, on itère sur les animations sans build, et n'importe qui peut
déposer le sien.

```
shimeji-desktop.exe
characters/
└── blob/
    ├── mascot.json      ← manifeste (poses, timings, ancres, hitbox)
    └── img/
        └── shime1.png … shime46.png   (128×128 chacun)
```

### Le vocabulaire de poses = les slots Shimeji

La numérotation `shime1..46` est un **standard de fait** : tous les packs Shimeji
utilisent les mêmes numéros pour les mêmes poses. En visant ce vocabulaire, **tout pack
trouvé sur internet fonctionne** avec nos comportements, sans code.

On lit les **images** des packs par leur numérotation. On **n'implémente pas** les
`actions.xml` / `behaviors.xml` de Shimeji : ces XML sont baroques (séquences imbriquées,
sélecteurs conditionnels, expressions Java évaluées à l'exécution), c'est un projet en
soi, et ça enfermerait le design dans la sémantique du Shimeji Java — où nos signaux
système n'existent pas.

### Une frame = un fichier + une ancre

Toutes les frames sont en **128×128**, le personnage placé dans la boîte. Chaque pose
déclare une **ancre** (le sol sous ses pieds, la main qui agrippe) : positionner devient
« place l'ancre ici ».

> Ne **pas** recalculer une compensation de décalage quand la taille du sprite change —
> l'ancre existe exactement pour rendre ça inutile.

### Couverture partielle

Un personnage sans images d'escalade **ne grimpera jamais** : l'option est retirée du
tirage d'envies. Aucun cas particulier à coder. C'est ce qui rend les packs tiers
utilisables même incomplets.

### Le personnage de test — `blob`

Le mascotte blanc par défaut de Shimeji-ee, 46 frames. Il possède **toutes** les poses,
y compris celles qui débloquent les comportements difficiles :

| Frames | Pose |
|---|---|
| 12, 13, 14 | agripper et escalader une **paroi verticale** |
| 23, 24, 25 | se suspendre et se déplacer au **plafond** |
| 34, 35, 36 | se hisser par-dessus un bord |
| 39, 40, 41 | s'asseoir puis dormir |
| 10 / 4 | chute / atterrissage |
| 18, 20, 21 | ramper |
| 44, 45, 46 | dédoublement (signature Shimeji) |

Développer contre `blob` **découple « le moteur marche » de « j'ai les bons dessins »**.

### Les packs livrés — `blob`, et lui seul

Le dépôt ne versionne plus que `blob`, le mascotte de référence du moteur. Les
cinq packs qui y vivaient (`luffy`, `naruto-kakashi`, `one-piece-zoro-01`,
`pierrot-54acb5`, `group-finity-blank-guy`) en sont sortis le 2026-09-14 :
3,2 Mo de sprites sous droits, tous réinstallables en un clic. Rien n'est
perdu — c'est **déplacé du dépôt vers la bibliothèque**.

| Dossier | Rôle | Qui y écrit |
|---|---|---|
| `characters/` du dépôt | `blob` seul, le personnage de référence | nous, à la main |
| `%APPDATA%\shimeji-desktop\characters\` | **la bibliothèque** — tout ce que le catalogue installe | le code, jamais l'humain |

La **bibliothèque gagne** sur le dossier livré en cas d'homonyme : un pack que
l'utilisateur a installé doit l'emporter, sinon on obtient un « je l'ai
installé et il ne se passe rien » indébogable.

### Ajouter un pack — le catalogue, plus jamais à la main

**Depuis l'application** : clic droit sur le personnage ou sur l'icône du
tray → « Catalogue de personnages… ». Deux écrans : « Catalogue » **installe**,
« Ma bibliothèque » **active et supprime**. Le changement est immédiat, sans
redémarrage.

Sur une carte de la bibliothèque, **quatre gestes, une seule commande**
(`definir_compte`) : le fond de la carte ajoute un exemplaire, `−` en retire
un, l'interrupteur met à 0 ou à 1, la poubelle supprime le pack du disque.

> **L'interrupteur n'est que le reflet de `compte > 0`.** Éteindre trois blob
> puis rallumer en ramène **un** : il n'y a aucun compte « en sommeil » stocké
> à côté, donc aucune seconde vérité à tenir d'accord avec
> `config.personnages`. Ne pas en ajouter une.

> ⚠️ **`config.personnages` est un MULTI-ENSEMBLE** : `["blob", "blob"]` veut
> dire deux blob à l'écran. Le compteur d'une carte, c'est le nombre
> d'occurrences du nom — et rien d'autre ne le stocke.

> ⚠️ **La suppression est définitive, et ne porte que sur la bibliothèque.**
> Un pack est supprimable **si et seulement si** son dossier résout dans
> `%APPDATA%` — règle générale, et surtout pas un cas particulier nommé
> « blob » : on ne supprime jamais un fichier versionné, et le dossier livré
> n'est peut-être même pas inscriptible.

**En ligne de commande**, l'équivalent scriptable :

```powershell
cargo run -- --installer one-piece-luffy-01
```

L'installation télécharge les frames du CDN, lit les vraies ancres dans
l'`actions.xml` du pack, mesure la hitbox sur les pixels opaques et écrit le
`mascot.json`. Le dossier ne prend son nom définitif qu'une fois tout réussi :
une installation interrompue ne laisse jamais un pack à moitié installé.

> ⚠️ **Deux pièges du CDN, payés une fois chacun** (2026-09-14) :
> l'`actions.xml` est à `<slug>/actions.xml` et **non** `<slug>/conf/…` (404),
> et **deux schémas coexistent** — `one-piece-luffy-01` est en balises
> japonaises (`画像`, `基準座標`), `pierrot-54acb5` en anglais (`Image`,
> `ImageAnchor`). Les deux se lisent. Aucun test ne les voyait : ils servent un
> faux réseau, et le repli sur l'ancre de convention est silencieux.

L'index des 2353 packs est pré-engendré dans `ui/catalogue.json`. Le
rafraîchir est un geste de **maintenance**, joué à la main :

```powershell
.	ools
ecuperer-packs.ps1 -Index
```

> ✅ **La hitbox ne se règle plus à l'œil — elle se mesure** (2026-09-12).
> `docs/outils/mesurer-hitbox.ps1 -Pack <nom> -Ecrire` relève, pour chaque
> pose, la boîte englobante des pixels opaques de ses frames et l'écrit dans le
> `mascot.json`. C'est ce qui a corrigé une hitbox de `blob` deux fois trop
> étroite (48 px déclarés pour 91 px dessinés, et 19 px amputés en haut) : le
> personnage n'était cliquable que sur une colonne centrale.

> ⚠️ **Mesurer l'ancre et la hitbox — ne pas recopier celles de `blob`.** La
> numérotation des poses est un standard de fait et se transpose telle quelle ; les
> **proportions du dessin, non**. Luffy est un chibi dont le chapeau touche le bord haut
> de la boîte : le `y = 20` de la hitbox de `blob` l'aurait amputé. La mesure qui a
> tranché avait été consignée dans le `mascot.json` de `luffy` — pack désormais
> installé et non plus versionné. Le catalogue applique cette mesure **tout
> seul**, pour chaque pack qu'il installe : c'est précisément ce qui rend la
> règle inutile à retenir.
>
> C'est la même leçon qu'à l'étape 1a, où les quatre réglages faits à l'œil étaient faux.

---

## Ordre de construction

Chaque étape est agréable en elle-même, et aucune ne dépend d'un dessin manquant.

| | Étape | Résultat |
|---|---|---|
| ✅ 0 | Validation technique | fenêtre transparente, sans bordure, au premier plan, hors taskbar, clics traversants, PNG déplacé à 60 Hz sur 2 écrans |
| ✅ 1 | **Il vit sur le sol** | marche, court, s'arrête, demi-tour, tous les écrans ; attrapable et il tombe ; tray, démarrage auto |
| ✅ 2 | **Il réagit** | s'endort quand on part, se réveille au retour, mange à midi |
| ✅ 3a | **Plusieurs personnages** | ils **coexistent** : compteur et interrupteur par pack dans la bibliothèque, apparition en tombant, départ animé, suppression du disque |
| ⏸️ 3b | **Et ils se remarquent** | **toujours de côté** — le comportement social (s'approcher, réagir l'un à l'autre) n'est PAS fait |
| 4 | **Il grimpe** | ✅ **4a** : bords et plafond de l'**écran** — reste **les fenêtres** (barres de titre, chute quand la fenêtre se ferme) ← *la prochaine* |
| 5 | **Il suit** | se déplace vers l'application au premier plan |

**L'étape 0 est un spike jetable et non négociable.** Le seul point faible de Tauri face
à Electron est justement l'overlay transparent : il faut le prouver sur cette machine
avant d'écrire une ligne de physique, pas après.

---

## Pièges Windows déjà identifiés

1. **Le rectangle que Windows donne n'est pas celui qu'on voit.** Une fenêtre Win10/11
   déclare ~7 px de bordure de redimensionnement invisible de chaque côté. Utilisé tel
   quel, le personnage est assis 7 px au-dessus de la barre de titre, dans le vide —
   subtilement faux, et très visible sur du pixel-art. Demander les bornes **visuelles**
   au compositeur (`DwmGetWindowAttribute` / `DWMWA_EXTENDED_FRAME_BOUNDS`).
2. **Windows 11 est plein de fenêtres fantômes.** Des applis modernes déclarent des
   fenêtres invisibles qui se présentent comme visibles. Écarter les fenêtres masquées
   (`DWMWA_CLOAKED`), les minimisées, les fenêtres outils, les minuscules — **et les
   fenêtres des personnages eux-mêmes**, sinon ils s'assoient les uns sur les autres.
3. **Le sol est la zone de travail, pas l'écran** — sinon il marche sous la barre des
   tâches.
4. **Coordonnées** : tout en pixels physiques du bureau virtuel ; n'appliquer le facteur
   d'échelle qu'au dimensionnement du sprite.

---

## État actuel

**L'application est distribuable.** `cargo tauri build` produit un installateur
NSIS qui livre `blob` avec l'exe ; au **premier lancement** un assistant de trois
écrans demande le démarrage avec Windows et l'écran de départ
(`gestionnaire` · `personnages` · `tray`), puis un toast annonce que
l'application continue en arrière-plan. Les deux réponses vivent dans
`config.json` sous `premiereConfigurationFaite` et `ecranAuDemarrage`, et
l'assistant ne revient plus. **Fermer le gestionnaire ne quitte plus
l'application** — seul « Quitter » le fait.

> ⚠️ **Le démarrage avec Windows n'a qu'une seule vérité : le registre.**
> `Config::demarrage_automatique` a été **supprimé** le 2026-09-20 : il était
> déclaré, initialisé, et jamais lu ni écrit. Ne pas le réintroduire — ce serait
> une seconde vérité à tenir d'accord avec `autostart::est_actif()`, la même
> erreur que l'interrupteur de la bibliothèque s'interdit déjà.

> ⚠️ **Les clés de `config.json` sont en camelCase** (`rename_all`). Écrire un nom
> de champ Rust dans `config::ecrire_cles` produit une clé que `Config` ne relira
> **jamais**, sans la moindre erreur — donc, pour l'assistant, un assistant qui
> revient à chaque lancement. Le test `ce_qu_ecrit_ecrire_cles_est_relu_par_charger_depuis`
> est le seul qui l'attrape, parce qu'il fait l'aller-**retour** complet.

**Plusieurs personnages vivent à l'écran en même temps.** On les active depuis
« Ma bibliothèque » (compteur et interrupteur par pack, doublons compris) ;
chacun **apparaît en tombant** du haut d'un écran tiré au sort, et repart en se
ramassant, sautant, puis tombant hors de l'écran. Une poubelle supprime un pack
du disque. Le reste est inchangé — marche, escalade, attrape-souris, tray,
`config.json`.

**316 tests.** Et le CPU, mesuré sur le programme réel (release, 60 s) :

| Roster | Caché | En marche |
|---|---|---|
| 1 | 0,7 % | — |
| 4 | 0,9 % | — |
| **10** | **0,7 %** | **67,9 %**, cadence tenue à 58,8 img/s |

> ⚠️ **Le mode caché est PLAT** — 0,7 % à un personnage comme à dix. Notre
> calcul ne grandit pas avec le roster : les signaux à 2 Hz, le recensement du
> monde à 8 Hz et la sonde du curseur sont payés **une seule fois** par la
> boucle unique. Ce qui coûte reste `SetWindowPos`, donc le **nombre de
> personnages qui MARCHENT** — un personnage assis ou endormi est gratuit.
>
> **Aucun plafond n'est imposé**, par décision de l'auteur prise en
> connaissance de la mesure : la bibliothèque **avertit** à partir de 10, et
> n'interdit rien.
>
> ⚠️ **Et ce qui se perd au-delà n'est pas que du CPU — c'est l'interactivité.**
> À 15 personnages en debug, la file de messages du thread principal prend
> **14 s** de retard : le menu ne se ferme plus, les clics traversent, les
> sprites se figent. C'est une **falaise** (0 ms à 4 personnages, 8 400 ms à 15),
> pas une pente, et c'est `SetWindowPos` — cachés, 15 personnages coûtent 6 % et
> 0 ms. La **régulation de charge** y répond : un sixième signal mesure cette
> latence et biaise vers le repos, ce qui la ramène à 234 ms. Elle ne supprime
> pas la falaise, elle empêche d'y tomber.
> → `docs/specs/2026-09-22-mesure-regulation.md`

> ⚠️ **Ce qui n'est PAS fait : le comportement social.** Ils coexistent, ils ne
> se remarquent pas. Ne pas conclure de « plusieurs personnages » que l'étape 3
> est soldée.

**Le catalogue de 2353 packs** se parcourt par franchise et s'installe en un
clic dans `%APPDATA%`. Le dépôt ne versionne plus que `blob`.

> ⚠️ **La branche `catalogue-de-personnages` part de `etape-2-il-reagit`.**
> L'étape 4a y a été fusionnée le 2026-09-14 ; **4b a été fusionnée puis
> revertée**, à la demande de l'auteur. Le commit de merge reste dans
> l'historique : une refusion de 4b demandera donc de reverter le revert, git
> la considérant déjà intégrée.

| Où | Contenu |
|---|---|
| `docs/specs/2026-09-08-design.md` | le design complet — le *pourquoi* de chaque décision |
| `docs/plans/2026-09-08-etape-0-spike-overlay.md` | le plan de l'étape 0, **soldé** ; **annexe A = structure de fichiers verrouillée pour l'étape 1** |
| `docs/specs/2026-09-08-spike-0-resultat.md` | **le résultat de l'étape 0** : grille remplie, décision de stack, API vérifiées, et les 2 découvertes à appliquer |
| `docs/plans/2026-09-08-etape-1a-il-vit-sur-le-sol.md` | le plan de l'étape 1a, **exécuté** — 11 tâches |
| `docs/plans/2026-09-09-etape-1b-tour-du-proprietaire.md` | le plan de l'étape 1b, **soldé** — tray, config, démarrage auto, rechargement à chaud, CPU |
| `docs/specs/2026-09-09-frames-shimeji.md` | **la correspondance frames → poses**, tirée des sources de Shimeji-ee — à lire avant de toucher au `mascot.json` |
| `docs/specs/2026-09-09-etape-2-design.md` | le design de l'étape 2 : les cinq signaux, le biais, le sommeil, l'interruption |
| `docs/plans/2026-09-09-etape-2-il-reagit.md` | le plan de l'étape 2, **exécuté** — 7 tâches, 71 étapes |
| `docs/specs/2026-09-11-etape-4a-il-grimpe-design.md` | le design de l'étape 4a : les plateformes verticales, `contact()`, l'intention `Grimper`, le monde vertical |
| `docs/plans/2026-09-11-etape-4a-il-grimpe.md` | le plan de l'étape 4a, **soldé** — 7 tâches |
| `docs/specs/2026-09-15-plusieurs-personnages-design.md` | **le design de « plusieurs personnages »** : la boucle unique, le multi-ensemble, l'apparition, le départ, la suppression — et le CPU mesuré AVANT d'être conçu |
| `docs/plans/2026-09-15-plusieurs-personnages.md` | son plan, **soldé** — 13 tâches |
| `docs/specs/2026-09-20-application-distribuable-design.md` | **le design de la distribution** : l'installateur NSIS, l'assistant de première configuration, le toast, et le code mort qu'il a fallu retirer |
| `docs/plans/2026-09-20-application-distribuable.md` | son plan, **soldé** — 7 tâches |
| `docs/specs/2026-09-21-regulation-de-charge-design.md` | **le design de la régulation de charge** : le sixième signal, ce que la mesure a REFUSÉ (`SetWindowPos` direct), et la tension avec « pas de charge CPU comme signal » |
| `docs/plans/2026-09-21-regulation-de-charge.md` | son plan, **soldé** — 4 tâches |
| `docs/specs/2026-09-22-mesure-regulation.md` | **la mesure** : la falaise entre 4 et 15 personnages, les 14 s ramenées à 234 ms, et les trois pièges de mesure |
| `docs/conception/2026-09-14-cout-des-sessions.md` | **ce que coûte une session d'assistance** : le relevé, et l'hypothèse évidente qui était fausse |
| `docs/conception/2026-09-14-journal-des-etapes.md` | **le récit de chaque étape** (0, 1a, 1b, 2, 4a) et les réglages « à l'œil » qui se sont révélés faux — extrait de ce fichier le 2026-09-14 |
| `docs/specs/2026-09-09-mesure-cpu.md` | **le dossier CPU complet** : les quatre hypothèses démenties par la mesure — à lire avant de toucher au chemin 60 Hz |
| `docs/conception/2026-09-08-journal-decisions.md` | **pourquoi** chaque décision, et ce qu'elle a écarté — à lire avant d'en défaire une |
| `docs/conception/2026-09-08-discussion.md` | la discussion de conception intégrale, verbatim |
| `docs/specs/2026-09-11-catalogue-de-personnages-design.md` | **le design du catalogue** : les deux gestes, la bibliothèque, l'installation sans un pixel recadré |
| `docs/plans/2026-09-11-catalogue-de-personnages.md` | le plan du catalogue, **soldé** — 12 tâches, et les deux pièges du CDN corrigés en cours de route |
| `docs/spike-etape-0/` | le spike **archivé et gelé** + la sonde Win32 rejouable — ne pas le faire évoluer vers l'application |
| `characters/blob/img/` | les 46 frames du personnage de test |

### Les règles héritées de l'étape 0 — à ne pas défaire

Le spike a validé les sept propriétés (transparence, premier plan, clics traversants…) et
laissé cinq contraintes, toutes **appliquées** aujourd'hui. Les défaire ramène les bugs.

1. **60 Hz est la cadence retenue** — le repli « 30 Hz + interpolation » de la spec §12 est
   abandonné, ne pas le réintroduire.
2. **`WS_EX_NOACTIVATE` dès la création de la fenêtre** — Tauri ne le pose pas. Sans lui,
   attraper le personnage **volerait le focus de l'éditeur**.
3. **`WS_EX_TOOLWINDOW` au même endroit** — `skip_taskbar` ne couvre que la barre des
   tâches ; l'exclusion d'Alt+Tab ne tiendrait qu'à une heuristique de Windows 11.
4. **Épingler `windows = "0.61"`** — la version dont dépend Tauri 2.11.5. Deux versions
   majeures de cette crate donnent deux types `HWND` distincts, et l'erreur de compilation
   parle alors de deux types de même nom.
5. **`SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)` en PREMIÈRE instruction de
   `main`** — pas laissée à Tauri, qui ne la fixe qu'à la création de sa boucle
   d'événements. Sans elle, Windows **ment** : il rend des pixels logiques (1536×816) en
   les présentant comme des pixels d'écran (réels : 1920×1020, échelle 1,25). La sonde et
   la boucle ne verraient pas le même bureau — un bug reproductible à moitié. C'est fait
   dans `probe::win32::activer_conscience_dpi`.

**Le récit de chaque étape** — 0, 1a, 1b, 2, 4a, la journée simulée, et surtout les
réglages « faits à l'œil » qui se sont tous révélés faux — est dans
`docs/conception/2026-09-14-journal-des-etapes.md`.

> **La leçon transverse, qui vaut pour les étapes 4 et 5 :** avant d'inventer une constante
> d'animation ou de physique, **la chercher dans les sources de Shimeji-ee**. Tout ce qui
> avait été réglé à l'œil à l'étape 1a était faux, et la spec §8.5 l'annonçait elle-même.

### La prochaine action

**L'étape 4 complète** — les plateformes de **fenêtres** (barres de titre, chute
quand la fenêtre se ferme, soustraction d'intervalles 1D pour les bords
recouverts, spec §2.2 et décision n° 2) — puis l'**étape 5** (il suit
l'application au premier plan).

L'étape 4a ayant posé les plateformes d'**écran** et toute la physique
verticale, il ne reste que le recensement des fenêtres et leur filtrage :
`geom.rs`, `Attachment` et le comportement d'escalade ne devraient pas changer.

> ⚠️ **Et c'est là que le CPU redeviendra une question.** Le recensement à 8 Hz
> est aujourd'hui payé une seule fois pour tout le roster, ce qui est exactement
> ce qui rend dix personnages gratuits en mode caché. `EnumWindows` y est
> beaucoup plus cher que la liste des écrans : **re-mesurer avec
> `docs/outils/mesurer-roster.ps1 -Cache`** après l'avoir branché, et comparer
> aux 0,7 % d'aujourd'hui. Si le chiffre monte avec le roster, c'est que du
> travail partagé est passé par erreur dans la boucle par personnage.

**L'étape 3b (qu'ils se remarquent) reste de côté**, à la demande de l'auteur —
mais elle est devenue facile : tous les personnages vivent dans un seul `Vec`,
donc une rencontre est une vérification côté coordinateur.

> ⚠️ **Reste ouvert depuis l'étape 4a : mesurer l'ancre de `grabWall`/`climbWall`.**
> C'est la seule vérification du projet qui ne se scripte pas — il faut **regarder** le
> personnage accroché à un mur. Marche à suivre : `cargo build` puis `cargo run` avec
> `SHIMEJI_ESCALADE=1`. Si le rendu ne convient pas, l'ancre se corrige dans
> `characters/blob/mascot.json`, **jamais** dans `attach.rs` (décision n° 1).
