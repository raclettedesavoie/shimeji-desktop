# Mesurer le CPU — le dossier complet

> Extrait de `CLAUDE.md` le 2026-09-14 pour alleger le contexte charge a chaque session.
> **Rien n'a ete reecrit** : le texte ci-dessous est celui qui vivait dans `CLAUDE.md`,
> section « Stack ». CLAUDE.md n'en garde que le protocole et la regle ; le *raisonnement*
> — quatre hypotheses formulees puis dementies par la mesure — est ici.
>
> A lire avant toute modification du chemin a 60 Hz (`main.rs`, `render.rs`, la sonde),
> et avant de proposer la moindre optimisation : les quatre hypotheses ci-dessous
> paraissaient toutes evidentes, et toutes etaient fausses.

### ⚠️ Mesurer le CPU — vérification systématique

**Un desktop pet qui consomme se fait désinstaller.** Toute modification du chemin à
60 Hz — la boucle de `main.rs`, `render.rs`, la sonde — doit être suivie d'une **mesure**,
pas d'une intuition.

### ⚠️⚠️ La méthodologie AVANT les chiffres — trois hypothèses fausses de suite

**Mesurer sur 10 secondes ne veut rien dire, et c'est le piège central ici.** La
consommation dépend entièrement de **ce que le personnage est en train de faire** :
en marche il déplace sa fenêtre à chaque image, à l'arrêt il ne la déplace pas du tout.
Relevé sur des tranches de 5 s consécutives, le taux de déplacement va de **0 % à 87 %**
des images. Deux mesures de 10 s sur la même version peuvent donc donner 12 % et 25 %.

> **Trois hypothèses ont été formulées et démenties par la mesure, dans cet ordre.**
> Les garder ici parce que chacune paraissait évidente :
>
> 1. « C'est `set_size` appelé à chaque image. » → **Faux.** Le retirer n'a rien changé
>    (14,8 % → 14,1 %). Il a été séparé quand même : appeler une API du système 60 fois
>    par seconde pour une valeur constante est une faute par principe.
> 2. « Le coût est inhérent à la fenêtre en couche, notre boucle n'y est pour rien. » →
>    **Faux.** Avec `SHIMEJI_SANS_BOUCLE=1` — la fenêtre créée, aucune animation — la
>    consommation est de **0 %**. Tout vient de la boucle.
> 3. « Notre logique 60 Hz est trop lourde. » → **Faux aussi.** `SHIMEJI_CADENCE=1`
>    montre **58 img/s** et un travail de **100 à 900 µs par image**, soit 1 à 5 % du
>    budget de 16,7 ms. Le calcul n'est pas le problème.
>
> **Ce qui coûte réellement** : chaque `set_position` est dispatché au thread principal
> de Tauri, qui appelle `SetWindowPos` sur une fenêtre **en couche** — Windows y refait
> une composition alpha. Le coût est donc proportionnel au **nombre de déplacements**,
> et c'est le seul levier.

### Le protocole, et les outils

**Toujours 40 à 60 secondes**, pour que marche et arrêt s'équilibrent :

```powershell
$p = Get-Process -Name shimeji-desktop
$c = $p.CPU; Start-Sleep -Seconds 60; $p.Refresh()
"$([math]::Round((($p.CPU - $c) / 60) * 100, 1)) % d'un coeur"
```

Les variables de diagnostic, **conservées** parce que ce sont elles qui ont démenti les
hypothèses 2 et 3 :

| Variable | Ce qu'elle donne |
|---|---|
| `SHIMEJI_CADENCE=1` | images/s réelles, travail moyen par image, **et le nombre de déplacements sur le nombre d'images** — c'est ce dernier chiffre qui explique tout |
| `SHIMEJI_SANS_BOUCLE=1` | crée la fenêtre et n'anime rien : sépare le coût de la fenêtre de celui de la boucle |
| `SHIMEJI_CACHE=1` | la boucle tourne entièrement, mais ne déplace ni ne dessine rien : sépare **notre calcul** du coût des déplacements |
| `SHIMEJI_TRACE=1` | trace chaque image servie par le schéma URI (a diagnostiqué le sprite invisible) |

**Chiffres de référence, build *debug*, mesures de 40 à 60 s, le 2026-09-09 :**

| Configuration | CPU |
|---|---|
| fenêtre seule, **aucune boucle** (`SHIMEJI_SANS_BOUCLE=1`) | **0 %** |
| `set_position` à chaque image, quoi qu'il arrive | **21 %** |
| **`set_position` seulement si la position a changé au pixel** | **12,3 %** |
| **la même chose, build `release`** (exe de 2,7 Mo) | **12 %** |
| **caché** (`SHIMEJI_CACHE=1`), build `release` | **0,9 %** |
| **étape 2, release, en marche** (signaux à 2 Hz branchés) | 6,4 % puis 3,9 % — ⚠️ **non comparable**, voir la note |
| **étape 2, release, caché** (`SHIMEJI_CACHE=1`) | **0,9 %** — identique à l'étape 1b : les signaux à 2 Hz ne coûtent rien |

> **Le relevé « caché » chiffre enfin le partage.** En mode caché la boucle tourne
> *entièrement* — sonde du curseur, hit-testing, physique, comportement, 60 fois par
> seconde ; **seuls le déplacement et la poussée du sprite sont sautés.** Donc :
> **0,9 % = tout ce que nous calculons**, et les **~11 points restants = `SetWindowPos`
> sur une fenêtre en couche.** C'est la confirmation directe du diagnostic, et la raison
> pour laquelle optimiser notre code ne rapporterait rien.

> **Le `release` ne gagne rien sur le debug, et c'est cohérent.** Notre travail par image
> ne représente que 1 à 5 % du budget de 16,7 ms — les optimisations du compilateur n'ont
> presque rien sur quoi mordre. Le coût est dans `SetWindowPos` de Windows, que le profil
> release ne change pas. Le profil visant d'ailleurs la **taille** (`opt-level = "z"`,
> LTO), il n'y avait pas de raison d'attendre mieux.
>
> Corollaire : **inutile de repasser le profil en `opt-level = 3`.** On paierait la taille
> de l'exe — un objectif de la spec §4, tenu ici à 2,7 Mo contre ~10 Mo visés — pour un
> gain nul.

> ### ⚠️⚠️ La quatrième hypothèse, et pourquoi « en marche » ne se compare plus
>
> **Depuis l'étape 2, la mesure « en marche » ne veut plus rien dire toute seule**, et
> c'est une conséquence directe de la décision n° 3.
>
> Les deux relevés de 60 s ont donné **6,4 % puis 3,9 %**, sous la référence de 12 %.
> L'explication est plausible : la machine qui mesure est celle où l'auteur travaille,
> l'inactivité réelle monte, le biais multiplie par 8 l'envie de se reposer, et un
> personnage assis ne déplace pas sa fenêtre. Le coût étant proportionnel au **nombre de
> déplacements**, il baisse.
>
> **Mais cette explication ne DÉMONTRE rien.** La charge de travail a changé en même
> temps que le code : un surcoût de quelques points aurait pu être masqué par la chute du
> taux de déplacement. C'est précisément le piège que la section « la méthodologie AVANT
> les chiffres » décrit — appliqué ici à nous-mêmes, une quatrième fois.
>
> **L'expérience qui isole, et qu'il faut faire à sa place :** `SHIMEJI_CACHE=1` sur le
> build release. La boucle tourne **entièrement** — sonde du curseur, hit-testing,
> physique, comportement, **et les cinq appels système à 2 Hz** — mais ne déplace jamais
> la fenêtre. La charge est donc **identique par construction**, quoi que fasse le
> personnage, et le chiffre redevient comparable :
>
> | | CPU en mode caché |
> |---|---|
> | étape 1b, sans la sonde de signaux | 0,9 % |
> | **étape 2, sonde à 2 Hz branchée** | **0,9 %** |
>
> Verdict : **les cinq appels système ne coûtent rien de mesurable**, et cette fois c'est
> établi et non supposé. `appli_active` — le seul des cinq à ouvrir un handle de processus
> — n'y paraît pas davantage.
>
> **La leçon, à garder pour les étapes 3 à 5 :** dès qu'un signal modifie le comportement,
> le CPU « en marche » mesure le **comportement**, pas le code. Pour comparer deux
> versions, il faut une configuration où la charge ne dépend pas du comportement —
> `SHIMEJI_CACHE=1` en est une, et le travail par image de `SHIMEJI_CADENCE=1` en est une
> autre.

**Pistes restantes**, par rentabilité décroissante :

1. ~~Suspendre la boucle quand la session est verrouillée~~ — **appliqué** (étape 2,
   Tâche 6) : le verrouillage emprunte le même chemin que « caché », donc le même
   **0,9 %** mesuré plus haut. Non re-mesuré séparément ici : verrouiller la session
   demande le mot de passe de l'auteur au déverrouillage, et cette vérification lui est
   laissée (voir le rapport de la Tâche 6).
2. **Descendre à 8 Hz les images où la position ne change pas** — le personnage à l'arrêt
   n'a besoin ni de 60 déplacements ni de 60 décisions par seconde. Gain modeste, le
   travail de calcul étant déjà négligeable.
3. ~~Ne pas appeler `set_position` quand la position n'a pas changé~~ — **appliqué**,
   21 % → 12,3 %.
4. ~~Ne rien dessiner quand les personnages sont cachés~~ — **appliqué** (Tâche 1 de 1b,
   tirée en avant) **et mesuré** : 12 % → **0,9 %**. Caché, il ne reste plus que notre
   propre calcul.

> **Ce qu'il ne faut PAS faire :** descendre la cadence sous 60 Hz. L'étape 0 a établi
> que 60 Hz est fluide sur cette machine, et le travail par image ne représente que 1 à
> 5 % du budget. Ce serait payer en fluidité ce qui ne coûte rien.

**`cargo run` suffit — pas besoin de `cargo tauri dev`.** Le front étant statique, les
assets sont embarqués dans le binaire à la compilation. Le CLI Tauri ne devient nécessaire
que pour produire un installateur.

> **Pour arrêter l'application : « Quitter » dans le menu du tray.** Depuis la Tâche 1
> du plan 1b, c'est la voie normale. Ce qui suit ne vaut que si le tray n'a pas pu
> s'installer — le message `tray non installé` le dirait.
>
> ⚠️ **En secours : `Stop-Process -Name shimeji-desktop`** (ou `Ctrl+C` dans le terminal,
> en debug seulement — le build `release` n'a pas de console). La fenêtre est volontairement
> non focalisable, sans bordure, hors taskbar et hors Alt+Tab : elle ne peut donc **pas** se
> fermer normalement. C'est le comportement voulu, et c'est exactement pourquoi « Quitter »
> était la Tâche 1 du plan 1b, avant le retrait de la console.

**Deux exigences de `tauri-build` découvertes à l'étape 0**, valables aussi pour
l'application :

- `icons/icon.ico` est **obligatoire**, même pour un projet jetable — son absence fait
  échouer le script de build.
- `frontendDist` est résolu **relativement au dossier du `tauri.conf.json`**. Le spike a
  une arborescence plate, donc `"./ui"` ; l'étape 1 utilisera `src-tauri/`, où le `"../ui"`
  conventionnel sera correct.

