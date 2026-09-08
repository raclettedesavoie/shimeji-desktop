# Journal des décisions — 2026-09-08

Digest de la session de conception. Organisé **par décision**, pas chronologiquement.
La discussion intégrale est dans `2026-09-08-discussion.md`.

À lire quand une décision paraît arbitraire, ou avant de vouloir en défaire une.

---

## Les décisions, et ce qu'elles ont écarté

| # | Décision | Écarté, et pourquoi |
|---|---|---|
| 1 | **Réactif au PC** — il réagit à l'activité de la machine | *Ambiant pur* : trop décoratif. *Tamagotchi* : ajoute une obligation, hors sujet |
| 2 | **Style Shimeji** — il grimpe sur les bords des fenêtres | *Sol seul* (recommandé au départ, 3–4× moins de travail) et *sol puis fenêtres*. Choix assumé du plus long |
| 3 | **Tauri v2 / Rust** | *Electron* (recommandé au départ par pragmatisme), *C# WPF*, *Godot*. Voir « le moment charnière » |
| 4 | **Une fenêtre par personnage**, 128×128 | *Overlay transparent plein écran* : ne couvre pas proprement deux moniteurs de DPI différents. Défaut structurel |
| 5 | **Toute la logique en Rust**, webview = afficheur bête | *Physique en JS* : 60 allers-retours IPC par seconde et par personnage |
| 6 | **JSON maison sur les slots Shimeji** | *Parser les XML Shimeji* : projet à part entière, et enferme le design dans la sémantique Java. *JSON libre* : perd le dépôt direct des packs |
| 7 | **Toujours au premier plan**, bords recouverts filtrés | *Le laisser passer derrière les fenêtres* : on le perd de vue |
| 8 | **Aucune capture de frappe** — seulement actif/inactif | *Hook clavier global* : signature de keylogger, faux positifs antivirus. *FFI `GetLastInputInfo`* : proposé comme moyen terme, écarté au profit du plus simple |
| 9 | **Pas de charge CPU comme signal** | Écarté explicitement après l'avoir intégré |
| 10 | **Pas de bulles de dialogue** | Le prototype VSCode en avait ; non retenues |
| 11 | **Plusieurs persos simultanés, et ils se remarquent** | — |
| 12 | **Il se déplace physiquement vers la fenêtre active** | *Changer seulement d'attitude* : moins vivant |
| 13 | **Démarre avec Windows** | — |
| 14 | **Nouveau dépôt, sprites Luffy/Zoro abandonnés** | Simplifie le manifeste : une seule sorte de frame au lieu de deux |
| 15 | **Code abondamment commenté en français** | Exigence explicite, pas un choix de style. Voir les conventions dans `CLAUDE.md` |

---

## Le moment charnière : la question du greenfield

La recommandation initiale était **de garder Electron** : le prototype VSCode avait déjà
une `BrowserWindow` transparente qui fonctionnait, et le JS était le langage connu.

Alexis a demandé : *« Si j'avais encore rien fait, qu'est-ce que tu recommanderais ? »*

La réponse honnête était **Tauri**, pour une raison précise : ce projet a deux moitiés qui
tirent en sens inverse.

| Moitié | Meilleur outil |
|---|---|
| animer un personnage (sprites, poses, machine à états) | le web — CSS, `image-rendering: pixelated` |
| introspecter les fenêtres Windows (`EnumWindows`, `DwmGetWindowAttribute`) | du natif — bindings typés |

Electron n'atteint la seconde que par FFI, en décrivant à la main des structs win32 en JS.
Un mauvais calibre de struct ne donne pas une erreur mais un **segfault** — et ce serait
sur l'API la plus centrale du projet. Rust les expose typées, et le binaire passe de
~150 Mo à ~10 Mo.

Alexis a pris Rust, en connaissance du surcoût d'apprentissage.

> **Leçon retenue en mémoire** : donner la recommandation sur le fond *avant* le repli
> pragmatique. Présenter l'option confortable d'abord a coûté un aller-retour.

---

## La découverte qui a sauvé le projet

Le design était complet, avec un **risque de contenu non résolu** : les planches Luffy
n'avaient ni animation de sommeil, ni de repas, ni d'escalade. Deux des comportements
demandés en premier dépendaient donc de dessins inexistants.

Alexis a déposé un dossier `Shimeji/` : les **46 frames du mascotte blanc par défaut de
Shimeji-ee**, en 128×128.

Ce jeu contient exactement ce qui manquait :

| Frames | Pose |
|---|---|
| 23, 24, 25 | agripper une paroi verticale (**escalade**) |
| 34, 35, 36 | se hisser par-dessus un bord |
| 22 | suspension au plafond |
| 39, 40, 41 | s'asseoir puis dormir |
| 10 / 4 | chute / atterrissage |
| 18, 20, 21 | ramper |
| 44, 45, 46 | dédoublement (signature Shimeji) |

Trois conséquences, dont une non anticipée :

1. **Le risque de contenu tombe** — on développe contre ce personnage, qui a toutes les poses.
2. **Le 128×128 valide la taille de fenêtre** décidée avant de l'avoir mesurée.
3. **La numérotation `shime1..46` est un standard de fait** : tous les packs Shimeji
   utilisent les mêmes numéros pour les mêmes poses. En visant ce vocabulaire, **n'importe
   quel pack du net devient utilisable en le déposant dans `characters/`.** Ce gain-là
   n'avait pas été anticipé, et il vaut plus que n'importe quel format inventé sur mesure.

---

## Les cinq décisions de design à ne pas défaire

Reprises de `CLAUDE.md` et détaillées dans la spec. Chacune supprime une classe de bugs ;
les annuler ramène les bugs.

1. **Position dérivée, jamais stockée** — un perso accroché stocke `(plateforme, face, offset)`. Rend gratuits : fenêtre déplacée, redimensionnée, fermée.
2. **Premier plan + bords recouverts exclus** — il devient *incapable* de se tenir sur du vide, au lieu qu'on détecte puis corrige.
3. **Les signaux biaisent, ne commandent pas** — « inactif 2 min » multiplie par 8 l'envie de dormir plutôt que de déclencher le sommeil. **Cette marge est le produit.**
4. **La navigation a le droit d'échouer** — délai d'abandon de 20 s. On rend le blocage temporaire par construction au lieu d'énumérer les cas de blocage.
5. **Le comportement est de la donnée** — ajouter un signal = ajouter une ligne de table.

---

## Ce que la session a produit

| Livrable | Où |
|---|---|
| Besoin, stack, architecture, conventions | `CLAUDE.md` |
| Design complet, le *pourquoi* de tout | `docs/specs/2026-09-08-design.md` |
| Plan de l'étape 0 | `docs/plans/2026-09-08-etape-0-spike-overlay.md` |
| Chaîne d'outils, pièges, API vérifiées | `docs/specs/2026-09-08-spike-0-resultat.md` |
| Le spike, qui compile — **archivé après avoir répondu** | `docs/spike-etape-0/` |
| Personnage de test, 46 frames | `characters/blob/img/` |

**Fait depuis** (2026-09-08, même journée) : les 7 vérifications du spike sont passées,
la stack Tauri est confirmée, et l'étape 0 est soldée. Le résultat et les deux découvertes
qu'il a livrées sont dans `docs/specs/2026-09-08-spike-0-resultat.md`.
