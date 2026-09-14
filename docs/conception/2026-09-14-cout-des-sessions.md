# Ce que coûte une session d'assistance — la mesure du 2026-09-14

> Même méthode que le dossier CPU : une hypothèse évidente, une mesure, et l'hypothèse
> qui tombe. C'est la cinquième fois sur ce projet.

## L'hypothèse de départ, et pourquoi elle était fausse

La question posée était « les sessions consomment beaucoup, comment réduire ? ». La
première réponse — **« c'est `CLAUDE.md`, il fait 54,7 Ko »** — était plausible :
le fichier est chargé à chaque session, et deux de ses sections (`## Stack` à 16,3 Ko et
`## État actuel` à 15,3 Ko) faisaient à elles seules 58 % du total.

Elle était **fausse au sens où elle visait 8 % du problème**.

L'auteur a suggéré une autre piste : « c'est peut-être le nombre d'agents sur ma dernière
session ». Une première vérification a semblé la démentir — mais elle cherchait le mauvais
nom d'outil (`Task` au lieu d'`Agent`) et a rendu zéro partout. **L'intuition était la
bonne.**

## Le relevé

Les transcriptions vivent dans
`~/.claude/projects/c--Users-alri-Documents-shimeji-desktop/`, un fichier `.jsonl` par
session, et un sous-dossier `<session>/subagents/` par session ayant lancé des agents.
Chaque ligne porte un champ `message.usage` avec `cache_read_input_tokens`,
`cache_creation_input_tokens` et `output_tokens`.

**`cache_read_input_tokens` est le bon indicateur** : c'est le contexte relu à chaque
requête. Il est facturé une fraction du prix d'un token neuf, mais il est payé
*intégralement, à chaque tour*, ce qui en fait le poste dominant dès qu'une session dure.

### Session `f8053c5f` — 25 agents

| | requêtes | cache_read | cache_create | output |
|---|---|---|---|---|
| fil principal | 297 | **83,9 M** | 1,36 M | 520 088 |
| les 25 sous-agents | **1 426** | **211,0 M** | 8,5 M | 591 429 |
| **total** | 1 723 | **294,9 M** | | |

**Les sous-agents pèsent 71,5 % de la session.** Ils ont produit **4,8 fois plus de
requêtes** que le fil principal.

### Les six dernières sessions

| session | requêtes | agents | contexte moyen | cache_read |
|---|---|---|---|---|
| `f8053c5f` | 297 | **25** | 282 k | 83,9 M *(+ 211 M d'agents)* |
| `817c157f` | 425 | 0 | 187 k | 79,6 M |
| `ede62b85` | 166 | 0 | 145 k | 24,0 M |
| `da00e320` | 86 | 0 | 108 k | 9,3 M |
| `26752b7e` | 33 | 0 | 86 k | 2,8 M |
| `7cb6eebb` | 22 | 0 | 72 k | 1,6 M |

Cumul du projet, toutes sessions : **1 147 M** de `cache_read`.

## Ce que ces chiffres disent, et ce qu'ils ne disent pas

**Le coût ≈ nombre de requêtes × taille du contexte.** Les deux facteurs comptent, et le
tableau les sépare bien : `817c157f` coûte cher par sa **longueur** (425 requêtes sans un
seul agent), `f8053c5f` par sa **largeur** (un contexte moyen de 282 k, gonflé par les
retours d'agents, plus 211 M dépensés hors du fil principal).

**Ce que ça ne dit pas : que les agents sont mauvais.** Un agent qui balaie quarante
fichiers et ne ramène qu'une conclusion *économise* du contexte au fil principal. Le
chiffre condamne les agents **lancés par réflexe** — et 25 dans une session, dont 8 relancés
par `SendMessage`, ressemble à un réflexe plutôt qu'à un choix.

⚠️ **Le piège de mesure, identique à celui du CPU** : ces sessions n'ont pas fait le même
travail. Comparer `f8053c5f` (25 agents) à `817c157f` (0 agent) ne démontre donc rien à
lui seul sur les agents — c'est le partage **interne** à `f8053c5f`, 83,9 M contre 211 M,
qui tranche, parce que là les deux moitiés sont dans la même session.

## Ce qui a été fait le 2026-09-14

1. **Les trois serveurs MCP morts coupés au niveau projet** (`.claude/settings.json`) :
   `chrome-devtools` et deux `azure-devops` échouaient en `CONNECT_TIMEOUT` après 30 s
   chacun, sans aucun usage ici. 90 s d'attente au démarrage, et leurs schémas d'outils
   dans le contexte de chaque session **et de chaque sous-agent**.
2. **`CLAUDE.md` ramené de 54,7 à ~38 Ko** : le récit du dossier CPU est parti dans
   `docs/specs/2026-09-09-mesure-cpu.md`, l'historique des étapes dans
   `docs/conception/2026-09-14-journal-des-etapes.md`. **Rien n'a été réécrit ni perdu** —
   seules les *règles encore actives* restent dans `CLAUDE.md`.
3. **Les tests sortis des onze plus gros modules**, dans des fichiers voisins : 11 942 →
   5 805 lignes de source à lire (**−51 %**). `intention.rs` passe de 3 394 à 1 622 lignes.
   Les 241 tests passent, et le compte par fichier est identique à `HEAD` fichier par
   fichier.

## Comment refaire la mesure

```bash
cd ~/.claude/projects/c--Users-alri-Documents-shimeji-desktop
python -c "
import json,glob,os
for f in sorted(glob.glob('*.jsonl'),key=os.path.getmtime,reverse=True)[:6]:
    n=cr=0; ag=0
    for line in open(f,encoding='utf-8'):
        try: d=json.loads(line)
        except: continue
        m=d.get('message') or {}
        u=m.get('usage')
        if u: n+=1; cr+=u.get('cache_read_input_tokens',0)
        c=m.get('content')
        if isinstance(c,list):
            for b in c:
                if isinstance(b,dict) and b.get('type')=='tool_use' and b.get('name')=='Agent': ag+=1
    # Les sous-agents ont leurs propres transcriptions, a cote :
    sub=sum(u.get('cache_read_input_tokens',0)
            for g in glob.glob(f[:-6]+'/subagents/**/*.jsonl',recursive=True)
            for u in [ (json.loads(l).get('message') or {}).get('usage') or {} for l in open(g,encoding='utf-8') ])
    print('%s req=%4d agents=%2d ctx_moyen=%7d principal=%6.1f M sous-agents=%6.1f M'
          %(f[:8],n,ag,cr//max(n,1),cr/1e6,sub/1e6))
"
```

> **L'erreur à ne pas refaire** : l'outil s'appelle **`Agent`**, pas `Task`. Chercher
> `"name":"Task"` rend zéro sur toutes les sessions et fait conclure, à tort, qu'aucun
> agent n'a jamais tourné. Et **ne pas oublier `subagents/`** : sans ce dossier, on rate
> les 211 M — c'est-à-dire 72 % de ce qu'on cherche.
