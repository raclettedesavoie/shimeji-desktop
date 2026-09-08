# Spike de l'étape 0 — **gelé**

Ce dossier a rempli son office le **2026-09-08**. Il est archivé ici, hors de la racine,
pour deux raisons précises — et pour aucune autre.

> ⚠️ **Ne pas faire évoluer ce code vers l'application.** L'étape 1 repart d'une structure
> propre (`src-tauri/`, voir l'annexe A du plan de l'étape 0). Ce code est jetable par
> définition : son produit était une réponse écrite, pas un logiciel.
>
> Il est aussi **volontairement incomplet** : il n'a ni `WS_EX_NOACTIVATE` ni
> `WS_EX_TOOLWINDOW`, précisément les deux styles que son analyse a révélés manquants.
> Le recopier tel quel réintroduirait les deux problèmes qu'il a servi à découvrir.

## Ce qu'il a prouvé

Les sept propriétés de l'overlay transparent sur cette machine — d'où la décision de
poursuivre en Tauri plutôt que de basculer sur Electron.

**Le résultat complet est dans [`../specs/2026-09-08-spike-0-resultat.md`](../specs/2026-09-08-spike-0-resultat.md).**
C'est ce document qui compte ; celui-ci n'explique que comment rejouer l'instrument.

## Pourquoi on le garde

1. **C'est le plus petit reproducteur d'un problème d'affichage.** Si la transparence ou
   le premier plan se dérèglent un jour dans l'application, comparer avec ce spike sépare
   en une minute « Tauri/WebView2 a changé » de « j'ai cassé quelque chose ».
2. **Il permet de fermer la seule inconnue laissée ouverte par l'étape 0** : le
   comportement multi-DPI. Les deux écrans de cette machine sont à l'échelle 1, le spike
   n'a donc pas pu l'éprouver. Sur une machine à écrans de DPI différents, il répond en
   deux commandes.

## Comment le rejouer

Depuis **PowerShell**, jamais depuis Git Bash — dans un shell Git Bash, rustc peut pêcher
le `link.exe` de Git for Windows au lieu du linker MSVC et rendre une erreur
`extra operand` totalement opaque.

```powershell
cd docs\spike-etape-0
cargo run
```

La console imprime la topologie des écrans, puis une silhouette blanche de 128×128
traverse le bureau virtuel en ondulant.

> **Pour l'arrêter : `Ctrl+C` dans le terminal**, ou `Stop-Process -Name spike-overlay`.
> La fenêtre est sans bordure, non focalisable, hors taskbar et hors Alt+Tab : elle **ne
> peut pas** se fermer normalement. C'est le comportement voulu, et il se retourne contre
> soi au moment de quitter. L'application a une entrée « Quitter » dans le tray.

## La sonde de styles

`probe-styles.ps1` interroge Windows sur la fenêtre du spike **pendant qu'il tourne** :
quels styles étendus sont réellement posés, et quelle fenêtre recevrait un clic au centre
du personnage.

```powershell
# dans un second terminal, le spike étant lancé
.\probe-styles.ps1
```

C'est ainsi qu'a été vérifiée la propriété n° 4 (clics traversants) — sans cliquer. Un
attribut de fenêtre se demande à Windows ; le constater à la souris n'aurait testé qu'un
point et qu'une application, là où `WindowFromPoint` répond pour toute la fenêtre.

Et c'est aussi elle qui a livré les deux vraies découvertes de l'étape 0, en signalant
deux styles **absents** — ce qu'aucune observation à l'œil ne pouvait donner, un style
absent ne se voyant que plus tard, quand une autre pièce arrive.
