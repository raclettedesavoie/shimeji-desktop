# Spike « une fenêtre par écran » — la mesure

**2026-09-22.** Branche `spike-fenetre-par-ecran`. **Mesure, pas promotion** —
comme `spike-deplacements-groupes` et `spike-deplacement-30hz`.

Machine : 3 écrans (2× 1920×1080 à 100 %, portable 1920×1080 à 125 %), build
**release**, Teams et Visual Studio ouverts en fond. 60 s par essai.

---

## 1. Ce qu'on cherchait

L'application déplace N fenêtres en couche à 60 Hz. À 15 personnages, la file
du thread principal prend **8 à 14 secondes** de retard, et l'utilisateur ne
peut plus ouvrir le gestionnaire pour réduire son roster — le défaut qui a
déclenché ce spike.

`spike-deplacements-groupes` avait déjà montré que grouper les messages n'y
change rien : *« le coût est la recomposition DWM par fenêtre, pas le nombre de
messages »*. L'idée testée ici supprime donc les déplacements : **une fenêtre
plein écran par écran, qui ne bouge jamais**, et des personnages déplacés en
CSS à l'intérieur.

Deux inconnues à lever avant d'écrire la moindre ligne d'affichage réel :

1. Une fenêtre plein écran transparente, au premier plan, **immobile**, coûte
   combien ?
2. Les `eval` qui portent les positions coûtent combien **sur la file** ?
   `charge.rs` documente que `eval` emprunte **exactement le même canal** que
   `set_position` (`run_on_main_thread` → `Message::Task` → `PostMessageW`).
   Rien ne garantissait a priori qu'il soit moins cher.

## 2. Les deux premières réponses

| Phase | shimeji | webviews | total | latence médiane / max |
|---|---|---|---|---|
| 0 — témoin, aucune fenêtre | 0,7 % | — | **0,7 %** | 0 / 2 ms |
| 1 — 3 fenêtres plein écran **immobiles** | 0,6 % | 2,3 % | **2,9 %** | 0 / 5 ms |
| 2 — + 174 `eval`/s, 15 figurants | 17,2 % | 139,6 % | **156,8 %** | **0 / 51 ms** |

> **La file n'est plus le problème.** 174 `eval`/s tiennent **0 ms médian**, là
> où 900 `SetWindowPos`/s donnent 8 à 14 **secondes**. La question posée par
> l'auteur — « `eval` n'emprunte-t-il pas le même canal ? » — a sa réponse :
> oui, et ce canal encaisse très bien 174 messages/s.
>
> **Mais le coût a changé de place** : les renderers passent de 2,3 % à 139,6 %.

## 3. Ce qui coûte n'est ni la surface, ni les sprites

Trois essais ont éliminé les hypothèses une par une. C'est la partie utile du
spike, parce que **les deux hypothèses évidentes étaient fausses** — encore.

| Variante (phase 2) | evals/s | webviews | total |
|---|---|---|---|
| 15 figurants, plein écran | 174 | 139,6 % | 156,8 % |
| **1 figurant**, plein écran | 57 | 42,3 % | 53,0 % |
| 15 figurants, fenêtres **480×480** (9× moins de surface) | 174 | 121,1 % | 136,7 % |
| **3 figurants**, plein écran | 171 | 116,7 % | 134,1 % |

- **Pas la surface** : diviser l'aire par 9 fait passer 139,6 % à 121,1 %.
- **Pas les sprites** : 3 figurants coûtent autant que 15 (116,7 % contre
  139,6 %), à débit d'`eval` identique.
- **Pas la charge utile** : la chaîne de 3 entrées coûte comme celle de 15.

> **C'est le NOMBRE d'appels `eval`, et c'est un coût fixe par appel :
> ~6,8 ms de CPU chacun** (0,423 cœur·s / 57 appels). Pour un `ExecuteScript`
> qui exécute une ligne de JavaScript, c'est énorme — et c'est la vraie
> découverte de ce spike.

C'est aussi pourquoi l'application actuelle ne le paie pas : `render::pousser`
n'appelle `eval` **qu'au changement de pose**, quelques fois par seconde.

## 4. La courbe qui décide

15 figurants, 3 fenêtres plein écran. Seul le **débit d'`eval`** varie ; la
boucle reste à 60 Hz.

| `eval` | evals/s | shimeji | webviews | **total** | latence max |
|---|---|---|---|---|---|
| 60 Hz | 174 | 17,2 % | 139,6 % | **156,8 %** | 51 ms |
| 30 Hz | 87 | 8,7 % | 70,9 % | **79,6 %** | 80 ms |
| **15 Hz** | 44 | 3,1 % | 27,1 % | **30,2 %** | **3 ms** |
| 10 Hz | 29 | 3,5 % | 24,3 % | **27,8 %** | 29 ms |

La cadence de la boucle est tenue partout (57,6 à 58,2 img/s). En dessous de
15 Hz la courbe s'aplatit vers un plancher d'environ 25 % : il ne reste plus
que le coût d'existence des trois webviews.

### La comparaison qui compte

| 15 personnages | CPU total | Latence de la file |
|---|---|---|
| **application actuelle** (une fenêtre par personnage) | 91 % | **8 000 à 14 000 ms** |
| **overlay, `eval` à 15 Hz** | **30 %** | **3 ms** |

Trois fois moins de CPU, et le blocage de l'interface disparaît.

## 5. Ce que ça ne règle pas, et qui reste à décider

**Le sprite ne bouge que 15 fois par seconde.** À 60 Hz de boucle, envoyer les
positions à 15 Hz donne un mouvement visiblement saccadé. Pour retrouver 60 Hz
à l'écran il faudrait que le webview **interpole** entre deux positions reçues
(`requestAnimationFrame` entre la dernière position connue et la suivante).

Ce n'est **pas** « faire calculer la physique en JS » — la physique resterait
entièrement en Rust, le JS ne ferait que lisser entre deux vérités qu'on lui
donne. Mais c'est une nuance, pas une évidence, et elle mérite d'être tranchée
explicitement :

- l'interpolation introduit **~66 ms de retard visuel** (on lisse vers une
  position déjà passée), ou impose d'**extrapoler** (on dessine une position
  que Rust n'a pas encore validée, donc parfois fausse) ;
- **rien de tout ça n'a été mesuré ici.** La courbe du §4 mesure un sprite qui
  saute, pas un sprite lissé. Le lissage ajoutera du travail dans le renderer,
  d'un montant inconnu.

## 6. Et ce que le spike n'a PAS testé

- **`SHIMEJI_SPIKE_OPAQUE` et `SHIMEJI_SPIKE_PLAT`** ont été ajoutés puis
  rendus inutiles par le §3 : dès lors que le coût est par appel `eval` et non
  par pixel, ni la transparence ni `will-change` ne pouvaient l'expliquer. Les
  leviers restent dans le code pour qui voudrait les rejouer.
- **Les événements Tauri (`emit_to`) comme transport alternatif.** `render.rs`
  documente pourquoi le projet les a écartés (liste de contrôle d'accès qui
  refuse en silence, diffusion à tous les webviews). Ils pourraient être moins
  chers par appel — non mesuré.
- **La hitbox**, le découpage d'un personnage à cheval sur deux écrans, et le
  comportement réel. Le spike n'affiche que des figurants qui rebondissent.
- **L'interpolation**, cf. §5.

## 7. Les pièges de mesure rencontrés

> ⚠️ **Deux `cargo build` simultanés se bloquent sur le verrou de `target/`.**
> Le second paraît durer 11 minutes alors qu'il en a passé 9 à attendre le
> premier. Ne pas en conclure que la compilation incrémentale est lente.

> ⚠️ **Mesurer le CPU du seul processus `shimeji-desktop` ne dit presque
> rien ici.** Tout le coût est dans les renderers WebView2, qui sont des
> processus séparés : 17,2 % pour le processus, 139,6 % pour son arbre. Le
> dossier CPU du projet n'avait jamais isolé cette part. `mesurer-spike-overlay.ps1`
> relève les deux.

> ⚠️ **Le `frontendDist` est embarqué dans le binaire en release.** Modifier
> `ui/spike-overlay.html` exige une reconstruction ; l'oublier ferait mesurer
> l'ancienne page en croyant mesurer la nouvelle.
