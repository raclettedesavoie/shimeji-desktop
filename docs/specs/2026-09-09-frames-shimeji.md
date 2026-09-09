# La correspondance frames → poses, tirée de Shimeji-ee

Relevé le **2026-09-09** depuis les sources de Shimeji-ee
(`conf/actions.xml`, `src/com/group_finity/mascot/Manager.java`).

**C'est désormais la source de vérité du `mascot.json` de `blob`.** Ce qui
figurait avant dans la spec §8.5 avait été établi **à l'œil**, et se trompait sur
la majorité des poses — le personnage marchait à reculons avec l'image de chute
aux pieds.

---

## Pourquoi ce document existe

La spec §8.5 disait, à raison :

> **La correspondance frames → poses ci-dessus est un point de départ**, établi à
> l'œil sur `blob`. La raffiner consiste à éditer du JSON : c'est précisément le
> bénéfice du format.

Elle a été raffinée. Mais éditer le JSON sans écrire **d'où viennent** les
numéros aurait produit un fichier de 25 lignes de chiffres magiques, impossible
à vérifier ou à corriger six semaines plus tard. D'où ce tableau.

---

## Ce que Shimeji-ee dit, action par action

`Duration` est en **ticks**, et `Manager.TICK_INTERVAL = 40 ms`. Les `frameMs`
du manifeste sont donc `Duration × 40`.

`Velocity` est en **pixels par tick** ; multipliée par 25 elle donne des px/s.

| Action Shimeji-ee | Frames | Durée | Vitesse | Notre pose |
|---|---|---|---|---|
| `Stand` | 1 | — *(Stay)* | 0 | `stand` |
| `Walk` | **1, 2, 1, 3** | 6 t → 240 ms | −2 → **50 px/s** | `walk` |
| `Run` | **1, 2, 1, 3** | 2 t → 80 ms | −4 → **100 px/s** | `run` |
| `Dash` | 1, 2, 1, 3 | 2 t | −8 → 200 px/s | *(inutilisé)* |
| `Sit` | **11** | — *(Stay)* | 0 | `sit` |
| `Falling` | **4** | — *(Embedded)* | — | `fall` |
| `Bouncing` | **18, 19** | 4 t → 160 ms | 0 | `land` |
| `Pinched` | **9, 7, 5, 1, 6, 8, 10** | 5 t → 200 ms | 0 | `dragged*` (découpé, voir plus bas) |
| `Resisting` | 5, 6, 1 *(alternance)* | 5 t | 0 | *(inutilisé)* |
| `Tripping` | 19, 18, 20, 20, 19 | 4–8 t | −8 → 0 *(décélère)* | `tripping` |
| `Sprawl` | **21** | — *(Stay)* | 0 | `sprawl` |
| `Creep` | 20, 20, 21, 21, 21 | 4–28 t | −2 → 0 | `creep` |
| `Jumping` | **22** | — | — | `jump` |
| `GrabWall` | **13** | — | — | `grabWall` |
| `ClimbWall` | 14, 12, 13 | 4–16 t | 0,±1/±2 | `climbWall` |
| `GrabCeiling` | **23** | — | — | `grabCeiling` — ancre **64,48** |
| `ClimbCeiling` | 23, 24, 25 | — | — | `climbCeiling` |
| `SitAndDangleLegs` | 31, 32, 31, 33 | 5–15 t | 0 | `sitDangle` — ancre **64,112** |
| `SitWithLegsUp` | 30 | — | — | `sitLegsUp` |
| `SitAndLookUp` | 26 | — | — | `sitLookUp` |
| `Divide1` | 42, 43, 44, 45, 46 | 2–20 t | 0 | `split` |

Toutes les poses au sol portent `ImageAnchor="64,128"`. **C'est l'ancre par
défaut** — et non `64,120` comme le supposait la spec, ce qui enfonçait le
personnage de 8 px sous la ligne du sol.

---

## Les cinq erreurs que ce relevé a corrigées

| # | Ce qui était écrit | La vérité | Ce qu'on voyait |
|---|---|---|---|
| 1 | miroir : sprites tournés à **droite** | tournés à **gauche** | **il marchait à reculons** — pattes dans un sens, déplacement dans l'autre |
| 2 | `walk` = 2, 3, 1, 4 | **1, 2, 1, 3** | la frame 4 (chute) apparaissait dans le cycle de marche |
| 3 | `fall` = 10 | **4** | 10 est une pose de balancement de `Pinched` |
| 4 | `land` = 4 | **18, 19** | 4 est la chute : il « atterrissait » en tombant |
| 5 | `sit` = 39, `run` = 30-33 | **11**, et **1,2,1,3** | 39 appartient à `PullUpShimeji`, 30-33 aux poses assises |

### Le miroir : la preuve, et pas une supposition

`Walk` porte `Velocity="-2,0"` — **négative**. Avec le sprite tel quel, le
personnage va vers la **gauche**. Idem `Run` (`-4,0`), `Dash` (`-8,0`) et
`Creep` (`-2,0`). Aucune action de déplacement n'a de vitesse positive.

Donc : **non miroité = tourné vers la gauche**, et `Facing::flipped()` rend
`true` pour `Right`. Un test le verrouille désormais
(`facing_droite_demande_un_miroir`).

---

## Le seul endroit où l'on s'écarte volontairement de Shimeji-ee

### Le balancier du personnage porté

Shimeji-ee joue `Pinched` = **un cycle unique** `9,7,5,1,6,8,10`, qui bat
indépendamment de ce que fait la souris. On le **découpe en trois poses** :

| Pose | Frames | Quand |
|---|---|---|
| `dragged` | 1 | souris immobile ou lente |
| `draggedLeft` | 5, 7, 9 | souris vers la gauche, au-delà de 120 px/s |
| `draggedRight` | 6, 8, 10 | souris vers la droite |

**Pourquoi s'en écarter :** le cycle fixe ressemble à une animation qu'on
regarde ; le balancier qui suit la main ressemble à une peluche qu'on tient.
C'est la différence entre un personnage et un GIF, et c'est exactement ce que
vise le critère de réussite de la spec §1.

Le seuil de 120 px/s est réglé à l'œil — Shimeji-ee n'a pas d'équivalent dont
s'inspirer. Il est franchement dépassé par un glisser volontaire et jamais
atteint par un tremblement de main.

> Le sens retenu est le **direct** : on tire à gauche → il penche à gauche. Un
> pendule réel traînerait *derrière* et pencherait à droite. Les deux se
> défendent ; pour inverser, échanger deux constantes dans
> `behavior/reflex.rs::pose_portee`.

---

## ✅ Tranché le 2026-09-09 — comment il dort

**Shimeji-ee n'a AUCUNE animation de sommeil.** Aucune action ne s'appelle
`Sleep`, `Doze` ni `Nap`, et les frames 38-41 que la spec croyait être
« s'asseoir puis dormir » appartiennent en réalité à `PullUpShimeji` — soulever
un autre shimeji.

L'étape 2 promet « il s'endort quand on part ». **Le dessin n'existe pas dans
ce pack.** Trois issues, à trancher à l'étape 2 :

1. **`sprawl` (21)** — allongé sur le ventre. C'est le plus proche visuellement
   d'un personnage endormi, et c'est du contenu existant : coût nul.
2. **`sitLookUp` (26) ou `sitDangle`** — assis immobile. Moins lisible comme
   sommeil, mais plus digne.
3. **Dessiner une pose de sommeil** — sortirait du pack d'origine, et perdrait
   la compatibilité « n'importe quel pack du net fonctionne ».

**Décision de l'auteur : la 1**, et sous forme de *séquence* plutôt que de pose
unique :

| | Pose | Frame |
|---|---|---|
| il s'assoit | `sit` | **11** |
| puis il s'affale | `sleep` | **21** |

C'est exactement la promesse de la spec (« il s'assoit, **puis** il s'endort »),
et l'enchaînement fait la moitié du travail de lisibilité : ce n'est pas la
frame 21 qui dit « il dort », c'est le passage de 11 à 21 après un moment
d'immobilité.

**La pose s'appelle `sleep` dans le manifeste, pas `sprawl`.** Le code ne doit
pas savoir qu'il s'agit d'un substitut : un pack tiers qui aurait une vraie
pose de sommeil la déclarerait au même nom, et rien ne changerait côté Rust —
c'est précisément le bénéfice du format (spec §8.6). Le manifeste de `blob`
déclare donc **deux poses sur la frame 21** : `sprawl` (l'action Shimeji-ee) et
`sleep` (notre usage).

> **Vérifié sur les sprites, et pas seulement dans le XML :** aucune frame du
> pack n'a les yeux fermés — les yeux du blob sont deux points, il n'existe
> aucune version « endormie » à trouver. 21 est le plus proche : à plat sur le
> ventre, corps horizontal. Les candidats assis (26, 31-33) ont été écartés
> parce qu'ils rendent « il dort » et « il est à l'arrêt » visuellement
> identiques.

---

## Comment rejouer ce relevé

Les sources sont dans `C:\Users\alri\Downloads\shimejieesrc (2)`. Le script
qui a produit le tableau ci-dessus :

```python
import re, io, sys, xml.etree.ElementTree as ET
sys.stdout.reconfigure(encoding='utf-8')
s = io.open('conf/actions.xml', encoding='utf-8-sig').read()
# Retirer les namespaces : ElementTree refuse le préfixe xsi autrement.
s = re.sub(r'\s(xmlns|xsi):\w+="[^"]*"', '', s)
s = re.sub(r'\sxmlns="[^"]*"', '', s)
root = ET.fromstring(s)

for a in root.iter('Action'):
    poses = list(a.iter('Pose'))
    if not poses:
        continue
    print("-- %s type=%s border=%s" % (a.get('Name'), a.get('Type'), a.get('BorderType') or '-'))
    for p in poses:
        n = re.search(r'shime(\d+)', p.get('Image') or '')
        print("     shime%-3s anchor=%-8s vel=%-8s dur=%s" % (
            n.group(1) if n else '?', p.get('ImageAnchor'),
            p.get('Velocity') or '-', p.get('Duration')))
```

Pour l'index inverse (« quelles actions utilisent la frame N ? »), qui est ce
qui a permis de retrouver que 18-21 appartiennent à `Bouncing`/`Tripping`/
`Creep`/`Sprawl` et non à une séquence d'atterrissage unique, remplacer la
boucle finale par une accumulation dans un dictionnaire `frame → actions`.

### Vérifier quelles frames l'application demande vraiment

C'est ce qui a diagnostiqué le sprite invisible, et c'est resté outillé :

```powershell
$env:SHIMEJI_TRACE="1"
.\target\debug\shimeji-desktop.exe
```

Chaque image servie par le schéma URI est alors tracée. Après correction, un
personnage qui marche ne doit demander que **1, 2, 3** — `stand`, `walk` et
`run` partagent ces trois images.

---

## Ce qu'on n'a délibérément pas repris

Conformément à la décision n° 6 (`docs/conception/2026-09-08-journal-decisions.md`),
on **lit** `actions.xml` comme documentation, on ne l'**implémente** pas :

- les **séquences composites** (`Type="Sequence"` avec des `ActionReference`) —
  c'est le rôle de nos intentions, et les nôtres réagissent à des signaux
  système que Shimeji-ee ignore ;
- les **expressions Java** évaluées à l'exécution
  (`${mascot.environment.workArea.left + Math.random()*...}`) ;
- les **conditions** de `behaviors.xml`.

Ce document ne concerne que la couche la plus basse et la plus stable : quelle
image montrer pour quelle pose.
