# La régulation de charge — la mesure

**2026-09-22.** Ce que la régulation change, mesuré sur la configuration qui
échouait. Design : `docs/specs/2026-09-21-regulation-de-charge-design.md`.

---

## 1. Le défaut, et sa vraie cause

Quinze personnages, build **debug** : menu contextuel qui ne se ferme plus,
personnages plus attrapables, sortie noyée sous
`failed to send message to the webview`. Le processus répond, il ne bloque pas :
il **sature** — voir §2 du design pour la chaîne complète jusqu'au `PostMessageW`
de tao.

La mesure décisive a été celle-ci :

| Roster (debug) | CPU | Latence de la file |
|---|---|---|
| 1 `blob` | 30,5 % | **0 ms** |
| 4 `blob` | 38,4 % | **0 ms** |
| 15 `blob` | 46,1 % | 8 404 ms |
| 15 packs réels | 69,5 % | 10 543 ms |
| **15 packs réels, CACHÉS** (`SHIMEJI_CACHE=1`) | **6,1 %** | **0 ms** |

> **Deux conclusions, et la seconde a surpris.**
>
> 1. Ce ne sont **ni les webviews ni notre calcul** : quinze personnages cachés
>    coûtent 6 % et zéro latence. La boucle tourne entière ; seuls les
>    déplacements sont sautés. C'est `SetWindowPos` — ce que le dossier CPU
>    disait déjà du CPU, et qui vaut aussi pour la latence.
> 2. **C'est une falaise, pas une pente.** 0 ms à quatre personnages, 8 400 ms à
>    quinze. Rien entre les deux : c'est le comportement d'une file dont le débit
>    d'arrivée vient de dépasser le débit de service. Quinze personnages qui
>    marchent demandent jusqu'à 900 déplacements par seconde ; cette machine en
>    sert environ 400.

## 2. Ce que la régulation change

Même binaire, **mêmes 15 packs réels**, 60 s, inactivité neutralisée dans les
deux essais (`inactiviteSecondes = 86400`) pour qu'elle n'endorme personne et ne
masque pas l'effet mesuré. Seul le seuil varie — `latenceMsSeuil = 0` **éteint**
le signal.

| | CPU | Latence finale | Déplacements par acteur |
|---|---|---|---|
| Signal éteint | 69,7 % | **14 048 ms** | 39 % |
| **Signal actif** | **56,1 %** | **234 ms** | 24 % |

**La latence passe de 14 secondes à 234 millisecondes**, et 14 points de CPU
tombent avec elle. La cadence est tenue dans les deux cas (56,6 et 57,5 img/s) :
ce n'est pas la boucle qui ralentit, c'est la pression qui baisse.

### La trajectoire, qui est le vrai résultat

```
latence     0 ms → flâner ×1.00  reposer ×1.00
latence   593 ms → flâner ×0.25  reposer ×23.70
latence  3633 ms → flâner ×0.25  reposer ×32.00   (plafond)
latence  6415 ms → flâner ×0.25  reposer ×32.00   (plafond)
latence   761 ms → flâner ×0.25  reposer ×30.43
latence     1 ms → flâner ×1.00  reposer ×1.00
latence     0 ms → flâner ×1.00  reposer ×1.00
…
latence   114 ms → flâner ×0.25  reposer ×4.58
```

Elle monte, elle serre, elle relâche, et elle re-serre doucement quand ça
remonte. Aucun état n'est maintenu, rien n'est remis à zéro : le multiplicateur
est recalculé à chaque battement de 2 Hz depuis la latence du moment.

## 3. La réponse binaire ne suffisait pas — et c'est la mesure qui l'a dit

La première version multipliait `se_reposer` par 4 dès le seuil franchi, sans
notion de gravité. Mesurée le 2026-09-21, à 11 `blob` :

| | Latence | Déplacements par acteur |
|---|---|---|
| Signal éteint | 16 916 ms | 36–50 % |
| Signal binaire `×4` | 8 713 ms | 31–46 % |

Elle divisait la latence par deux — réel, mais très insuffisant quand elle vaut
dix secondes. Le signal était « tout allumé » à 100 ms comme à 10 000.

Deux corrections en ont découlé, toutes deux dans `signals::biais_de` :

1. **La graduation.** `ampleur = latence / seuil`, plafonnée à
   `latence_facteur_max` (8). Au seuil exact, `ampleur` vaut 1 et le
   comportement est celui d'avant — c'est ce qui rend le plafond réglable sans
   changer le sens du seuil.
2. **Tarir la source.** `flaner ×0,25`, comme le fait le signal d'inactivité.
   Flâner est l'intention qui **marche**, donc celle qui poste un déplacement
   par image. Encourager le repos sans décourager la marche mettait une
   vingtaine de secondes à converger.

## 4. Les pièges de mesure rencontrés

> ⚠️ **L'inactivité fausse toute comparaison.** Trois essais enchaînés sans
> personne à la machine : l'inactivité monte, `reposer ×8` s'applique, les
> personnages dorment. Le troisième essai affichait « 0 déplacement » et un CPU
> flatteur de 19,7 % — rejoué en premier, il remontait à 46 %. **Neutraliser
> `inactiviteSecondes` dans tous les essais d'une comparaison**, ou les rejouer
> dans l'ordre inverse. C'est la même leçon que celle du dossier CPU (« la
> mesure en marche ne se compare plus d'une version à l'autre »), sous une forme
> nouvelle.

> ⚠️ **Le seuil flottant n'est pas exact.** `Duration::from_secs_f32(0.1)` vaut
> 100,000001 ms : une latence de 100 ms tout rond restait **sous** un seuil
> réglé à 100. La comparaison se fait donc en millisecondes flottantes, jamais
> en `Duration`. Le test `au_seuil_exact_la_graduation_est_neutre` l'attrape, et
> il a échoué à la première écriture.

> ⚠️ **`latenceMsSeuil` n'est pas borné, délibérément.** Le borner à zéro serait
> nuisible : un seuil nul rendrait le signal permanent et endormirait tout le
> monde à jamais. Une valeur nulle ou négative **éteint** le signal — c'est le
> repli inoffensif, et c'est aussi ce qui donne le bouton A/B utilisé ci-dessus.

## 5. Ce qui reste vrai, et ce qui reste à faire

- **Aucun plafond de personnages n'est imposé**, conformément à la décision de
  l'auteur. Ce qui change, c'est qu'au-delà d'une dizaine, ce n'est plus
  seulement le CPU qui monte : c'est **l'interactivité** qui se perd. La
  régulation y répond, elle ne supprime pas la falaise.
- **Le mur est structurel** : une fenêtre par personnage, déplacée par le thread
  principal. Le réduire vraiment demanderait moins de déplacements par seconde
  (30 Hz de déplacement — la règle n° 1 de l'étape 0 l'interdit) ou des
  déplacements groupés (`BeginDeferWindowPos`, que Tauri n'expose pas). Aucune
  des deux n'est engagée ici.
- **Mesuré en debug**, la configuration où le défaut se manifestait. En release,
  la falaise est plus loin : à 11 personnages, aucun échec n'était observé.
