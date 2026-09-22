# shimeji-desktop

Un *desktop pet* pour Windows. Des personnages en pixel-art vivent sur votre bureau :
ils marchent, s'assoient, dorment, grimpent aux murs et se suspendent au plafond —
tout seuls, et sans jamais vous gêner.

Ils réagissent à la machine : ils s'endorment quand vous partez, se réveillent quand
vous revenez, mangent à midi, et traînent le soir.

Compatible avec les packs de personnages **Shimeji** existants : un catalogue de plus
de 2000 packs s'installe en un clic depuis l'application.

---

## Télécharger

**[⬇️ Télécharger shimeji-desktop pour Windows](https://github.com/raclettedesavoie/shimeji-desktop/releases/latest/download/shimeji-desktop-setup.exe)**

Windows 10 ou 11, 64 bits. Environ 1,3 Mo. Rien d'autre à installer : le runtime
Visual C++ est lié statiquement dans l'exécutable, et WebView2 est déjà présent sur
Windows 11 (l'installateur le télécharge au besoin sur Windows 10).

### ⚠️ Windows va vous avertir — c'est attendu

L'application **n'est pas signée** par un certificat de signature de code (ils coûtent
plusieurs centaines d'euros par an, ce qui n'a pas de sens pour un projet personnel).
Windows SmartScreen affiche donc un bandeau bleu :

> **Windows a protégé votre ordinateur**
> Microsoft Defender SmartScreen a empêché le démarrage d'une application non reconnue.

Le bouton pour continuer est **caché** : cliquez sur **« Informations
complémentaires »**, puis sur **« Exécuter quand même »**.

Si vous préférez ne pas faire confiance à un binaire téléchargé — ce qui est une
position parfaitement raisonnable — [compilez-le vous-même](#compiler-depuis-les-sources) :
le code de ce dépôt est exactement celui qui produit l'installateur.

---

## Premier lancement

Un personnage tombe sur votre bureau et se met à vivre. Par-dessus, un petit assistant
vous pose deux questions :

- **Lancer avec Windows ?** — inscrit ou retire une entrée dans
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`. Aucun droit administrateur,
  aucun service.
- **Quel écran au démarrage ?**

| Choix | Ce que vous voyez au lancement |
|---|---|
| **Gestionnaire** | les personnages, et la fenêtre de gestion ouverte |
| **Personnages** | les personnages seuls, aucune fenêtre |
| **Tray** | rien — l'application vit dans la zone de notification, personnages cachés |

L'assistant ne revient plus. Vos réponses sont dans `%APPDATA%\shimeji-desktop\config.json` ;
remettre `premiereConfigurationFaite` à `false` le rejoue.

### Mises à jour

L'application vérifie au démarrage s'il existe une version plus récente. Quand c'est le
cas, une notification vous le dit **une fois**, et l'entrée « Mettre à jour vers la
vX.Y.Z » apparaît dans le menu de la zone de notification.

**Rien ne s'installe sans votre clic.** Appliquer une mise à jour ferme et relance
l'application : ce n'est pas quelque chose à vous imposer au milieu d'une session.

Chaque mise à jour est **signée**, et l'application refuse d'installer un fichier dont
la signature ne correspond pas. Cette signature garantit l'origine de la *mise à jour* ;
elle ne change rien à l'avertissement SmartScreen du premier téléchargement, qui, lui,
demanderait un certificat de signature de code.

> Si vous utilisez la **v0.1.0**, elle ne contient pas encore ce mécanisme : installez la
> version suivante à la main, et les suivantes se proposeront toutes seules.

---

## Au quotidien

L'application vit dans la **zone de notification** (en bas à droite, près de l'horloge).
Son icône donne accès à tout :

- **Afficher les personnages** — les cacher sans quitter
- **Démarrer avec Windows**
- **Ouvrir le gestionnaire…** — le catalogue et votre bibliothèque
- **Vérifier les mises à jour…** — ou « Mettre à jour vers la vX.Y.Z » quand une
  version est disponible
- **Quitter** — la seule chose qui ferme vraiment l'application

> **Fermer la fenêtre du gestionnaire ne quitte pas l'application.** Les personnages
> et l'icône continuent. C'est voulu.

**Clic droit sur un personnage** ouvre son propre menu : lui demander de flâner, de
s'asseoir, de grimper au mur, de se lâcher… Vous pouvez aussi **l'attraper à la souris**
et le lâcher n'importe où : il tombe, et il se rattrape.

Il ne vous gênera jamais : il ne vole pas le focus, n'apparaît ni dans la barre des
tâches ni dans Alt+Tab, et vos clics le traversent partout sauf sur son dessin.

---

## Ajouter des personnages

**Clic droit sur le tray → « Ouvrir le gestionnaire… »**, onglet **Catalogue** : plus de
2000 packs Shimeji, classés par franchise, installables en un clic. L'onglet
**Ma bibliothèque** les active, en met plusieurs exemplaires à l'écran, ou les supprime
du disque.

Tout est immédiat : aucun redémarrage.

Les packs installés vont dans `%APPDATA%\shimeji-desktop\characters\`. Vous pouvez aussi
y déposer un dossier de pack à la main — c'est une opération de contenu, jamais de code.

---

## Désinstaller

Paramètres → Applications → *shimeji-desktop* → Désinstaller.

La désinstallation ne touche pas à `%APPDATA%\shimeji-desktop\` : vos réglages et les
packs que vous avez installés survivent à une réinstallation. Supprimez ce dossier à la
main si vous voulez tout effacer.

---

## Compiler depuis les sources

Windows, avec [Rust](https://rustup.rs/) et la charge de travail **Desktop C++** de
Visual Studio (pour le linker MSVC).

```powershell
git clone https://github.com/raclettedesavoie/shimeji-desktop.git
cd shimeji-desktop\src-tauri

cargo test          # la suite complète, sans écran
cargo run           # lancer sans installer

cargo install tauri-cli --locked --version "^2"
cargo tauri build   # l'installateur, dans target\release\bundle\nsis\
```

> ⚠️ `cargo build` **ne produit pas** d'installateur et l'ignore en silence : le bloc
> `bundle` de `tauri.conf.json` n'est lu que par la CLI Tauri.

Pas de Node, pas de bundler, pas de framework front : l'interface est du HTML statique.

---

## Comment c'est fait

**Tauri v2** et **Rust**. Toute la logique — physique, comportement, fenêtres, signaux
système — est en Rust ; le webview ne fait que poser un `background-position` sur un
sprite. Chaque personnage est sa propre fenêtre de 128×128, transparente et sans bordure,
déplacée à 60 Hz.

Le monde est une liste de rectangles avec une face utilisable : un bord d'écran et une
barre de titre sont la même chose pour la physique. Un personnage accroché ne mémorise
jamais sa position absolue, mais `(plateforme, face, distance au bord)` — c'est ce qui
fait qu'il voyage gratuitement avec une fenêtre qu'on déplace, et qu'il tombe quand elle
se ferme.

Le détail des décisions, et surtout **ce qu'elles ont écarté**, est dans [`docs/`](docs/).

---

## Crédits

- **`blob`**, le personnage livré avec l'application, est la mascotte par défaut de
  **Shimeji-ee**.
- Le concept du *Shimeji* est de **Yuki Yamada** (Group Finity).
- Le catalogue de packs s'appuie sur les ressources de **shimejis.xyz**.

Les packs installés depuis le catalogue restent la propriété de leurs auteurs
respectifs et ne sont pas redistribués par ce dépôt.

---

Projet personnel, Windows uniquement, fait pour le plaisir.
