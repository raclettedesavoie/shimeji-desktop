# La régulation de charge — design

**2026-09-21.** Comment l'application évite de se paralyser elle-même quand le
roster grandit, sans plafond en dur et sans toucher à la cadence.

---

## 1. Le défaut observé

Onze personnages, build **debug**. L'application devient inutilisable :

- le menu contextuel s'ouvre mais **ne se ferme plus**, et cliquer une entrée ne
  fait rien — le menu du tray compris ;
- on ne peut plus **attraper** un personnage ni faire un clic droit dessus ;
- la sortie standard se remplit de `rendu impossible pour pet-N : eval :
  runtime error: failed to send message to the webview`, plusieurs centaines de
  lignes par seconde.

Le processus, lui, répond (`Responding = True`) et consomme 69 % d'un cœur. Ce
n'est donc pas un blocage, c'est une **saturation**.

## 2. Ce que l'erreur veut dire — la chaîne, vérifiée

Elle n'a rien à voir avec le webview, et son texte est trompeur :

1. `win.eval(…)` et `win.set_position(…)`, appelés depuis le thread de la boucle
   60 Hz, passent tous deux par `send_user_message`
   (`tauri-runtime-wry-2.11.4/src/lib.rs:235`).
2. Hors du thread principal, celle-ci fait `proxy.send_event(message)`, dont
   l'échec devient `Error::FailedToSendMessage`
   (`tauri-runtime-2.11.3/src/lib.rs:132`) — le texte affiché.
3. `send_event`, sur Windows, est **un `PostMessageW` vers la fenêtre du thread
   principal** (`tao-0.35.3/src/platform_impl/windows/event_loop.rs:570`).

Or `PostMessageW` échoue quand **la file de messages du thread destinataire est
pleine** — Windows la plafonne à 10 000 messages postés. L'événement est alors
perdu sans recours : dans le code de tao, `event_send.send(event)` n'est même
pas atteint.

> **Le message ne signale pas un webview cassé. Il signale que le thread
> principal ne dépile plus assez vite et que sa file a débordé.** C'est un
> compteur de saturation déguisé en erreur.

Et c'est ce qui explique que les trois symptômes arrivent ensemble : une file
pleine, ce sont aussi les messages du menu qui ne sont plus dépilés et les
`set_ignore_cursor_events` qui ne sont plus appliqués — d'où les clics qui
traversent tout.

Le débit en cause : chaque personnage qui marche poste **un message par image et
par pixel changé**, soit jusqu'à 660/s à onze. La déduplication existante
(`dernier_coin`, comparée en **entiers**) fait déjà son travail ; il n'y a pas de
gras à retirer de ce côté.

## 3. Ce qui a été essayé et REFUSÉ par la mesure

L'idée évidente — « sortir le déplacement du thread principal en appelant
`SetWindowPos` nous-mêmes » — a été mesurée avant d'être crue. Build **release**,
11 `blob`, taux de déplacement comparable (11–23 % par acteur) :

| Voie | Cadence tenue | Coût d'**un** déplacement |
|---|---|---|
| **Tauri actuel** (`set_position` → `PostMessageW`) | **58,2 img/s** | **~500 µs** |
| `SetWindowPos` + `SWP_ASYNCWINDOWPOS` | 21 → 57 img/s | 2 900 – 8 100 µs |
| `SetWindowPos` direct | **9,5 → 47 img/s** | **~9 600 µs** |

`SetWindowPos` inter-thread **bloque l'appelant** jusqu'à ce que le thread
propriétaire traite la demande : ~10 ms par appel, la boucle 60 Hz tombe à
9 img/s. Le drapeau asynchrone atténue sans supprimer.

> ⚠️ **Ne pas réessayer.** Poster un message et rendre la main immédiatement est
> la meilleure des trois voies : le transport de Tauri n'est pas le problème, il
> est déjà la solution. Le code du spike a été retiré après mesure.

**Un piège de mesure, payé une fois.** Les trois runs enchaînés sans personne à
la machine, l'inactivité montait (`reposer ×8`) et les personnages dormaient : le
mode asynchrone affichait « 0 déplacement » et un CPU flatteur de 19,7 %, contre
46 % une fois rejoué en premier. **Comparer deux voies exige un taux de
déplacement comparable** — c'est la même leçon que celle déjà écrite dans le
dossier CPU, sous une forme nouvelle.

**Et en release, à N=11, il n'y a eu aucun échec.** Le mur existe, il est
simplement plus loin qu'en debug, où le thread principal est bien plus lent.

## 4. Pourquoi pas un plafond en dur

Parce qu'il coderait en dur une valeur qui dépend de la machine. Un cœur plus
rapide vide la file plus vite et repousse le mur ; plus de cœurs n'y changent
rien, puisque tout passe par la file d'**un seul** thread.

L'auteur a refusé le plafond pour cette raison, et il a raison. Mais ne rien
faire code en dur l'hypothèse inverse — « ça tiendra toujours » — qui est fausse
sur une machine plus lente ou un roster plus grand.

## 5. Le design retenu : un sixième signal

**Aucune nouvelle envie.** « Se reposer » existe déjà. On ajoute un **signal**, à
côté des cinq actuels, et il **multiplie le poids de l'envie de se reposer** —
exactement comme le fait l'inactivité avec son `×8`. Décision n° 3 : les signaux
biaisent, ils ne commandent pas.

### 5.1 Mesurer la santé du thread principal

Le signal ne se déduit pas de `N` : il se **mesure**, dans l'unité qui compte.

À ~2 Hz, la boucle poste un jeton horodaté par `AppHandle::run_on_main_thread`
(`tauri-2.11.5/src/app.rs:495`), qui emprunte **le même canal** que `eval` et
`set_position`. Le temps que met ce jeton à être exécuté **est** la latence de
la file. Aucune estimation, aucune constante devinée.

Propriétés qui en découlent, et qui sont l'intérêt du choix :

- sur une machine rapide, la latence reste basse, le multiplicateur reste à 1, et
  **le signal n'existe pour ainsi dire pas** ;
- sur une machine lente, ou à gros roster, il monte progressivement — c'est un
  signal **précoce**, là où `FailedToSendMessage` n'arrive qu'une fois les
  10 000 messages atteints, c'est-à-dire une fois l'application déjà perdue ;
- il n'y a **rien à régler par machine**.

### 5.2 La réponse : des personnages s'assoient, un par un

Le biais est calculé **une fois pour tout le roster** et lu par chaque
personnage au moment où *lui* choisit sa prochaine envie. Comme ils ne
choisissent pas tous au même instant, ils s'assoient **l'un après l'autre**, et
ça s'arrête de soi-même dès que la latence retombe.

Un personnage assis ou endormi ne poste plus rien : la pression retombe, la
latence redescend, le multiplicateur revient à 1. **C'est une boucle de
rétroaction complète, sans état à maintenir** — donc rien à remettre à zéro, et
aucun risque de rester coincé en mode dégradé.

Et comme c'est un biais, un personnage peut décider de marcher quand même. Cette
marge est délibérée (décision n° 3). À l'écran, ça se lit comme du comportement,
pas comme une dégradation technique.

### 5.3 Les seuils vivent dans la config

`config.signaux` reçoit deux réglages de plus, comme tous les autres signaux :
le seuil de latence à partir duquel le signal agit, et le multiplicateur
appliqué à `se_reposer`. On règle donc le caractère de la régulation **sans
recompiler** (décision n° 5).

### 5.4 Le compteur de saturation

Le `eprintln!` par échec disparaît : à plusieurs centaines de lignes par
seconde, il noie la sortie — c'est lui qui a rendu la session de diagnostic
illisible — et il formate une chaîne sur le chemin 60 Hz. À la place, un
**compteur agrégé**, affiché avec la trace de cadence.

Il reste utile comme signal **tardif** : s'il n'est jamais nul, c'est que la
régulation n'a pas suffi, et c'est ce chiffre-là qu'il faudra regarder.

## 6. La tension avec une décision déjà prise

Le besoin exclut explicitement : « ❌ **Pas de charge CPU comme signal** —
écarté explicitement ».

**Ce signal-ci n'est pas celui-là**, et la distinction doit rester nette :

| Écarté | Retenu ici |
|---|---|
| « la machine est chargée » → le personnage réagit | « **notre propre** file de rendu sature » → on réduit notre propre pression |
| un signal sur le monde extérieur, qui fait du pet un afficheur d'état système | une régulation interne, dont l'effet visible est un effet de bord |
| enrichit le comportement | protège l'usabilité |

La ressemblance est réelle : dans les deux cas le personnage s'assoit quand ça
chauffe. Si l'auteur juge que la distinction est trop mince, le repli est de
garder **seulement** le compteur de saturation (§5.4) et la trace, et de ne pas
brancher le biais — la mesure resterait utile, et le défaut resterait visible
au lieu d'être silencieux. C'est sa décision, pas celle de cette spec.

## 7. Ce que ce design ne fait PAS

- **Aucun plafond**, aucun refus d'ajouter un personnage. L'avertissement à 10 de
  la bibliothèque reste tel quel, mais son texte doit dire ce qui est désormais
  mesuré : au-delà, ce n'est pas seulement le CPU qui monte, c'est
  l'interactivité qui se perd.
- **Aucune baisse de cadence.** La règle n° 1 héritée de l'étape 0 tient : 60 Hz,
  pas de repli à 30 Hz.
- **Aucun changement de transport.** Voir §3.
- **Aucun état « bridé »** par personnage. Un multiplicateur, et rien d'autre.

## 8. Comment ça se vérifie

- `biais_de` reste une **fonction pure** : la latence entre dans `Signaux`, le
  multiplicateur sort dans `Biais`. Des tests de table, sans écran ni attente,
  comme les cinq signaux existants.
- La **mesure** de latence est isolée dans son propre module, pour être testable
  sans Tauri : ce qu'elle publie est une durée, le reste ne sait pas d'où elle
  vient.
- `SHIMEJI_SIGNAUX=1` affiche la latence et le multiplicateur qu'elle produit,
  comme les cinq autres — l'équivalent scriptable habituel.
- La preuve d'ensemble est une mesure : 11 personnages en **debug**, la
  configuration qui échouait. Attendu — zéro `FailedToSendMessage`, menu et
  attrapage qui répondent, et quelques personnages assis.
