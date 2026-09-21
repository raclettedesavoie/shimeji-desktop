# La régulation de charge — plan d'implémentation

> **Pour un exécutant agentique :** utiliser `superpowers:subagent-driven-development`
> ou `superpowers:executing-plans` pour dérouler ce plan tâche par tâche. Les étapes
> sont des cases à cocher (`- [ ]`).

**Objectif :** quand le thread principal sature, quelques personnages s'assoient
d'eux-mêmes — sans plafond en dur, sans baisse de cadence, sans état à maintenir.

**Architecture :** un **sixième signal**. On mesure la latence de la file de
messages du thread principal en y postant un jeton horodaté à 2 Hz ; cette durée
entre dans `Signaux`, et `signals::biais_de` la traduit en un multiplicateur sur
l'envie de se reposer, comme il le fait déjà pour l'inactivité, l'heure et la
batterie. Un personnage assis ne poste plus rien : la latence retombe, le
multiplicateur revient à 1. Boucle de rétroaction complète.

**Stack :** Rust, Tauri 2.11.5, `AppHandle::run_on_main_thread`
(`tauri-2.11.5/src/app.rs:495`), `std::sync::atomic`.

**Spec :** `docs/specs/2026-09-21-regulation-de-charge-design.md` — à lire avant
la première tâche, en particulier §3 (ce qui a été **refusé par la mesure**) et
§6 (la tension avec « pas de charge CPU comme signal »).

## Contraintes globales

- **Commentaires en français et abondants**, expliquant le *pourquoi* ; citer la
  section de la spec qu'un bloc applique (conventions de `CLAUDE.md`).
- **Ne jamais éditer une source par `Get-Content`/`Set-Content`** — UTF-8 accentué,
  PowerShell 5.1 double-encode tout le fichier en silence.
- **Compiler depuis PowerShell**, jamais Git Bash (le `link.exe` de Git for Windows).
- **Borner les sorties** : `cargo test --quiet`, `cargo build 2>&1 | Select-Object -Last 40`.
- **`cargo test` ne reconstruit pas l'exe** : `cargo build` avant tout essai à l'écran.
- **Le seul verbe autorisé dans `signals.rs` est MULTIPLIER** (décision n° 3). Si
  l'on écrit « si latence alors dormir », le design est perdu.
- **Ne pas toucher au transport** (`set_position`, `eval`) : mesuré, c'est déjà la
  meilleure des trois voies (spec §3).
- Point de départ : branche `regulation-de-charge`, **316 tests** au vert.

---

### Tâche 1 : `charge.rs` — mesurer la latence du thread principal

**Fichiers :**
- Créer : `src-tauri/src/charge.rs`
- Modifier : `src-tauri/src/main.rs` (déclaration `mod charge;`, à côté de `mod clock;`)

**Interfaces :**
- Produit : `charge::Moniteur`, avec
  `Moniteur::nouveau() -> Moniteur`,
  `Moniteur::latence(&self) -> Duration`,
  `Moniteur::enregistrer(&self, d: Duration)`,
  `Moniteur::prendre_le_vol(&self) -> bool`,
  et la fonction libre `charge::sonder(app: &tauri::AppHandle, m: &Arc<Moniteur>)`.

**Pourquoi un drapeau « en vol ».** Sous saturation, poster un second jeton
pendant que le premier attend ajouterait à l'embouteillage qu'on mesure. Un seul
jeton à la fois : tant qu'il n'est pas revenu, la latence lue reste la dernière
connue, ce qui est exactement le comportement voulu.

- [ ] **Étape 1 : écrire le test qui échoue**

Créer `src-tauri/src/charge.rs` avec le module de tests seul (le code viendra à
l'étape 3) :

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn une_latence_non_mesuree_vaut_zero() {
        let m = Moniteur::nouveau();
        assert_eq!(m.latence(), Duration::ZERO);
    }

    #[test]
    fn la_derniere_mesure_est_celle_qui_est_lue() {
        let m = Moniteur::nouveau();
        m.enregistrer(Duration::from_millis(42));
        assert_eq!(m.latence(), Duration::from_millis(42));
        m.enregistrer(Duration::from_millis(7));
        assert_eq!(m.latence(), Duration::from_millis(7));
    }

    /// Le cœur du drapeau : un seul jeton en vol à la fois.
    #[test]
    fn un_seul_jeton_en_vol_a_la_fois() {
        let m = Moniteur::nouveau();
        // Le premier décollage est accordé…
        assert!(m.prendre_le_vol());
        // …le second est refusé tant que rien n'est revenu.
        assert!(!m.prendre_le_vol());

        // Le retour du jeton libère la place.
        m.enregistrer(Duration::from_millis(3));
        assert!(m.prendre_le_vol());
    }
}
```

- [ ] **Étape 2 : lancer le test pour le voir échouer**

```powershell
cd C:\Users\alri\Documents\shimeji-desktop\src-tauri
cargo test --quiet charge 2>&1 | Select-Object -Last 20
```

Attendu : échec de compilation, `cannot find type Moniteur in this scope`.

- [ ] **Étape 3 : écrire l'implémentation minimale**

Au-dessus du `mod tests` de `charge.rs` :

```rust
//! La santé de la file de messages du thread principal (spec
//! « régulation de charge » §5.1).
//!
//! Responsabilité unique : publier **une durée** — le temps qu'un jeton met à
//! être traité par le thread principal. Ce module ne décide rien : il ne sait
//! pas ce qu'on fera de cette durée, et surtout pas qu'elle finira en envie de
//! se reposer.
//!
//! # Pourquoi mesurer plutôt que compter les personnages
//!
//! Le nombre de personnages qu'une machine encaisse dépend de la machine — un
//! cœur plus rapide vide la file plus vite. Un plafond en dur coderait donc en
//! dur une valeur qui n'est vraie que sur la machine où on l'a écrite. La
//! latence, elle, se mesure dans l'unité qui compte, partout.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// La dernière latence mesurée, et le drapeau du jeton en vol.
///
/// `Atomic*` et non `Mutex` : les deux accès sont une lecture et une écriture
/// de valeur simple, et le thread principal ne doit jamais attendre un verrou
/// que nous tiendrions — ce serait ajouter à l'embouteillage qu'on mesure.
pub struct Moniteur {
    /// En **microsecondes**. `u64` parce qu'il n'existe pas d'`AtomicDuration`,
    /// et que µs donne largement la finesse utile (on parle de dizaines de ms).
    dernière_latence_us: AtomicU64,

    /// Vrai pendant qu'un jeton attend son tour dans la file.
    en_vol: AtomicBool,
}

impl Moniteur {
    pub fn nouveau() -> Moniteur {
        Moniteur {
            dernière_latence_us: AtomicU64::new(0),
            en_vol: AtomicBool::new(false),
        }
    }

    /// La dernière latence connue. Zéro tant que rien n'a été mesuré.
    pub fn latence(&self) -> Duration {
        Duration::from_micros(self.dernière_latence_us.load(Ordering::Relaxed))
    }

    /// Le jeton est revenu : on publie sa durée et on libère la place.
    pub fn enregistrer(&self, d: Duration) {
        // `min` avec `u64::MAX` est inutile en pratique (il faudrait 584 000
        // ans de latence) mais `as u64` sur un `u128` tronquerait en silence,
        // et on préfère saturer que mentir.
        let us = u64::try_from(d.as_micros()).unwrap_or(u64::MAX);
        self.dernière_latence_us.store(us, Ordering::Relaxed);
        self.en_vol.store(false, Ordering::Relaxed);
    }

    /// Réserve la place du jeton. Rend `false` si un jeton attend déjà.
    ///
    /// `swap` et non `load` puis `store` : les deux en une seule opération
    /// atomique, sinon deux appels simultanés pourraient tous deux voir
    /// `false` et décoller ensemble.
    pub fn prendre_le_vol(&self) -> bool {
        !self.en_vol.swap(true, Ordering::Relaxed)
    }
}

/// Poste un jeton horodaté dans la file du thread principal.
///
/// C'est **le même canal** que `eval` et `set_position` empruntent
/// (`run_on_main_thread` → `Message::Task` → `PostMessageW`) : le temps que met
/// ce jeton EST la latence que subissent les déplacements et les menus. Y
/// mesurer autre chose — un `GetTickCount`, une charge CPU — ne dirait rien de
/// ce qui nous intéresse.
///
/// Ne poste rien si un jeton est déjà en vol : sous saturation, en ajouter
/// aggraverait ce qu'on mesure.
pub fn sonder(app: &tauri::AppHandle, moniteur: &Arc<Moniteur>) {
    if !moniteur.prendre_le_vol() {
        return;
    }

    let depart = Instant::now();
    // `clone` de l'`Arc` : la fermeture doit vivre jusqu'à son exécution sur
    // l'autre thread, donc elle possède sa propre référence comptée.
    let m = moniteur.clone();

    // `move` : la fermeture emporte `depart` et `m`. Elle est `FnOnce + Send`,
    // ce qu'exige `run_on_main_thread`.
    let poste = app.run_on_main_thread(move || {
        m.enregistrer(depart.elapsed());
    });

    // La file a refusé le jeton (elle est pleine, ou l'application se ferme).
    // On libère le drapeau, sinon un unique refus suspendrait la mesure pour
    // toujours — et c'est précisément quand ça sature qu'on veut la voir.
    if poste.is_err() {
        moniteur.enregistrer(Duration::MAX);
    }
}
```

Puis déclarer le module dans `main.rs`, à côté de `mod clock;` :

```rust
mod charge;
```

- [ ] **Étape 4 : lancer les tests**

```powershell
cargo test --quiet charge 2>&1 | Select-Object -Last 12
```

Attendu : 3 tests au vert.

- [ ] **Étape 5 : commit**

```bash
git add src-tauri/src/charge.rs src-tauri/src/main.rs
git commit -m "feat(charge): mesurer la latence de la file du thread principal"
```

---

### Tâche 2 : le sixième signal dans `biais_de`

**Fichiers :**
- Modifier : `src-tauri/src/probe/mod.rs:67-92` (le champ dans `Signaux`)
- Modifier : `src-tauri/src/probe/win32.rs:414`, `src-tauri/src/probe/fake.rs:43` et `:215`, `src-tauri/src/sim.rs:155` (les trois constructeurs)
- Modifier : `src-tauri/src/config.rs:145-221` (deux réglages + le bornage)
- Modifier : `src-tauri/src/signals.rs:103` (le bloc de biais) et son `mod tests`

**Interfaces :**
- Consomme : rien de la Tâche 1 — cette tâche est **pure**, et c'est voulu : elle
  se teste sans Tauri, sans écran et sans attente.
- Produit : `Signaux::latence_file: Duration` ;
  `SignauxReglages::latence_ms_seuil: f32` (défaut `100.0`) et
  `SignauxReglages::latence_se_reposer: f32` (défaut `4.0`).

**Pourquoi le champ est dans `Signaux` alors que la sonde ne le mesure pas.**
Parce que `biais_de` doit rester **une fonction pure d'une seule structure** :
c'est ce qui rend les cinq signaux existants testables en table. La sonde Win32
y met `Duration::ZERO` et la boucle le renseigne juste après — un commentaire le
dit à chacun des trois constructeurs, sans quoi le prochain lecteur croira à un
oubli.

**Défauts choisis, et pourquoi.** `100 ms` : au-delà, un clic de menu se voit
attendre, c'est le seuil où l'utilisateur *sent* le retard. `×4` : plus faible
que l'inactivité (`×8`, « il doit vraiment dormir ») et plus fort que le soir
(`×3`) — une pression nette, pas un ordre.

- [ ] **Étape 1 : écrire les tests qui échouent**

Dans `src-tauri/src/signals.rs`, dans `mod tests`, après le test de la batterie :

```rust
    /// En dessous du seuil, le signal n'existe pas — c'est le cas de toute
    /// machine qui se porte bien, et il doit rester strictement neutre.
    #[test]
    fn une_file_fluide_ne_biaise_rien() {
        let mut s = rien_de_special();
        s.latence_file = Duration::from_millis(5);
        assert_eq!(biais_de(&s, &Config::default()), Biais::neutre());
    }

    /// Au-delà du seuil, l'envie de se reposer est multipliée — et RIEN
    /// d'autre n'est touché : la régulation ne commande pas, elle pousse.
    #[test]
    fn une_file_saturee_pousse_au_repos() {
        let mut s = rien_de_special();
        s.latence_file = Duration::from_millis(250);

        let b = biais_de(&s, &Config::default());
        assert_eq!(b.se_reposer, 4.0);
        assert_eq!(b.flaner, 1.0);
        assert_eq!(b.jouer, 1.0);
    }

    /// Le seuil est une borne INCLUSIVE, comme celui de l'inactivité : deux
    /// signaux voisins qui se compareraient différemment seraient un piège à
    /// la relecture.
    #[test]
    fn le_seuil_de_latence_est_inclusif() {
        let mut s = rien_de_special();
        s.latence_file = Duration::from_millis(100);
        assert_eq!(biais_de(&s, &Config::default()).se_reposer, 4.0);
    }

    /// Un seuil négatif dans un `config.json` écrit à la main ferait paniquer
    /// `Duration::from_secs_f32` — exactement le défaut déjà corrigé pour
    /// `inactiviteSecondes`. Le bornage doit couvrir le nouveau champ aussi.
    #[test]
    fn un_seuil_de_latence_negatif_ne_fait_pas_paniquer() {
        let mut c = Config::default();
        c.signaux.latence_ms_seuil = -5.0;
        c.signaux.borner_pour_les_tests();

        let mut s = rien_de_special();
        s.latence_file = Duration::from_millis(1);
        // Ne panique pas, et le signal s'applique (toute latence dépasse 0).
        let _ = biais_de(&s, &c);
    }
```

Et compléter le constructeur d'essai, dans le même `mod tests` :

```rust
    fn rien_de_special() -> Signaux {
        Signaux {
            // … les champs existants, inchangés …
            latence_file: Duration::ZERO,
        }
    }
```

- [ ] **Étape 2 : lancer les tests pour les voir échouer**

```powershell
cargo test --quiet signals 2>&1 | Select-Object -Last 20
```

Attendu : échec de compilation, `struct Signaux has no field named latence_file`.

- [ ] **Étape 3 : ajouter le champ et les réglages**

Dans `src-tauri/src/probe/mod.rs`, à la fin de `pub struct Signaux` :

```rust
    /// Le temps qu'un jeton met à traverser la file du thread principal
    /// (spec « régulation de charge » §5.1).
    ///
    /// ⚠️ **Ce n'est PAS la sonde qui la mesure** — elle n'en sait rien, et y
    /// met `Duration::ZERO`. C'est la boucle qui la renseigne juste après,
    /// depuis `charge::Moniteur`. Le champ vit ici quand même pour que
    /// `signals::biais_de` reste une fonction pure d'UNE structure, ce qui est
    /// ce qui rend les six signaux testables en table.
    ///
    /// ⚠️ Et ce n'est pas non plus « la machine est chargée », signal écarté
    /// explicitement par le besoin : c'est **notre propre** file de rendu.
    /// Voir §6 de la spec.
    pub latence_file: std::time::Duration,
```

Puis, dans chacun des trois constructeurs, ajouter la ligne avec son
commentaire :

- `src-tauri/src/probe/win32.rs:415`, dans `super::Signaux { … }` :

```rust
            // La sonde ne sait pas la mesurer : c'est la boucle qui la
            // renseigne (voir le commentaire du champ).
            latence_file: std::time::Duration::ZERO,
```

- `src-tauri/src/probe/fake.rs:43` et `:215`, dans les deux `Signaux { … }` :

```rust
                latence_file: std::time::Duration::ZERO,
```

- `src-tauri/src/sim.rs:155`, dans `crate::probe::Signaux { … }` :

```rust
        // La simulation n'a pas de thread principal à saturer : la file y est
        // toujours fluide, et ce signal n'y joue jamais.
        latence_file: std::time::Duration::ZERO,
```

Dans `src-tauri/src/config.rs`, dans `pub struct SignauxReglages`, après
`batterie_se_reposer` :

```rust
    /// Au-delà de cette latence de la file du thread principal, il se repose
    /// davantage — la régulation de charge (spec du 2026-09-21).
    ///
    /// En **millisecondes** et non en secondes, contrairement à
    /// `inactivite_secondes` : on parle de dizaines de millisecondes, et
    /// `0.1` dans un `config.json` serait beaucoup plus facile à mal lire que
    /// `100`.
    pub latence_ms_seuil: f32,
    pub latence_se_reposer: f32,
```

Dans `impl Default for SignauxReglages` :

```rust
            latence_ms_seuil: 100.0,
            latence_se_reposer: 4.0,
```

Dans `fn borner(&mut self)`, après le bornage de `inactivite_secondes` :

```rust
        // Même raison exactement que ci-dessus : `Duration::from_secs_f32`
        // panique sur un flottant négatif, et `{"latenceMsSeuil": -1}` est du
        // JSON parfaitement valide.
        self.latence_ms_seuil = self.latence_ms_seuil.clamp(0.0, LATENCE_MS_MAX);
```

Et, à côté des deux constantes existantes :

```rust
/// Borne haute de `latence_ms_seuil` : 60 s. Elle n'empêche rien d'utile, elle
/// écarte les fautes de frappe — un seuil au-delà rendrait le signal muet.
const LATENCE_MS_MAX: f32 = 60_000.0;
```

Enfin, rendre `borner` atteignable depuis les tests de `signals.rs`. Juste
au-dessus de `fn borner`, ajouter :

```rust
    /// Le même bornage, exposé aux tests d'autres modules.
    ///
    /// `borner` est privée **et doit le rester** : elle n'a qu'un seul site
    /// d'appel légitime, la sortie de `charger_depuis`. Cette porte-ci n'existe
    /// que pour permettre à `signals.rs` de vérifier qu'une config bornée ne
    /// fait pas paniquer `biais_de`.
    #[cfg(test)]
    pub fn borner_pour_les_tests(&mut self) {
        self.borner();
    }
```

- [ ] **Étape 4 : écrire le bloc de biais**

Dans `src-tauri/src/signals.rs`, dans `biais_de`, après le bloc « Batterie
faible » et avant « L'application au premier plan » :

```rust
    // ── Notre propre file de rendu sature ───────────────────────────────
    //
    // Le sixième signal, et le seul qui parle de NOUS et non du système
    // (spec « régulation de charge » §5). Quand le thread principal prend du
    // retard, les déplacements, les menus et le hit-testing en prennent
    // aussi : à onze personnages en debug, l'application devenait
    // inutilisable.
    //
    // ⚠️ **Il multiplie, il n'ordonne pas** (décision n° 3). Des personnages
    // s'assoient l'un après l'autre — chacun au moment où LUI tire sa
    // prochaine envie — et un personnage assis ne poste plus rien : la
    // latence retombe, ce bloc cesse de s'appliquer. Rien à remettre à zéro.
    if s.latence_file >= Duration::from_secs_f32(r.latence_ms_seuil / 1000.0) {
        b.se_reposer *= r.latence_se_reposer;
    }
```

- [ ] **Étape 5 : lancer les tests**

```powershell
cargo test --quiet 2>&1 | Select-Object -Last 10
```

Attendu : **320 tests** au vert (316 + 4).

- [ ] **Étape 6 : commit**

```bash
git add src-tauri/src/probe src-tauri/src/config.rs src-tauri/src/signals.rs src-tauri/src/sim.rs
git commit -m "feat(signaux): la latence de la file devient le sixieme signal"
```

---

### Tâche 3 : brancher la mesure dans la boucle, et les deux traces

**Fichiers :**
- Modifier : `src-tauri/src/main.rs:1290-1400` (le bloc à 2 Hz et la trace des signaux)
- Modifier : `src-tauri/src/main.rs:2008-2013` (le `eprintln!` par échec)
- Modifier : `src-tauri/src/main.rs` (déclarations avant la boucle, et la trace de cadence)

**Interfaces :**
- Consomme : `charge::Moniteur`, `charge::sonder` (Tâche 1) ; `Signaux::latence_file` (Tâche 2).
- Produit : rien pour les tâches suivantes.

**Pourquoi le compteur remplace le `eprintln!`.** À onze personnages, ce message
sortait plusieurs centaines de fois par seconde : il noie la sortie — c'est lui
qui a rendu la session de diagnostic illisible — et il formate une chaîne sur le
chemin 60 Hz. Agrégé, il garde toute sa valeur de signal **tardif** : s'il n'est
jamais nul, la régulation n'a pas suffi.

- [ ] **Étape 1 : déclarer le moniteur et le compteur**

Dans `boucle`, à côté de `let trace_signaux = …` :

```rust
    // Le moniteur de la file du thread principal (spec « régulation de
    // charge » §5.1). `Arc` parce que la fermeture postée sur l'autre thread
    // doit en posséder une référence.
    let moniteur_charge = std::sync::Arc::new(charge::Moniteur::nouveau());

    // Combien de fois `pousser` a échoué depuis la dernière trace. Remplace un
    // `eprintln!` par échec, qui sortait des centaines de fois par seconde.
    let mut echecs_de_rendu: u32 = 0;
```

- [ ] **Étape 2 : sonder et renseigner le signal, à 2 Hz**

Dans le bloc `if maintenant.saturating_sub(dernier_signal) >= PERIODE_SIGNAUX {`,
remplacer `let s = sonde.signaux();` par :

```rust
            let mut s = sonde.signaux();

            // La sonde Win32 ne connaît pas ce signal-là : il parle de notre
            // propre file, pas du système. On le renseigne ici, juste avant
            // `biais_de` — c'est le seul endroit du programme qui sait les
            // deux moitiés.
            s.latence_file = moniteur_charge.latence();

            // Et on relance un jeton pour la prochaine fois. Posté APRÈS la
            // lecture : le jeton en cours de vol n'est pas celui qu'on vient
            // de lire, et on ne veut pas attendre son retour ici — à 2 Hz,
            // lire la valeur précédente est parfaitement suffisant.
            charge::sonder(&handle, &moniteur_charge);
```

- [ ] **Étape 3 : afficher la latence dans `SHIMEJI_SIGNAUX=1`**

Dans le bloc `if trace_signaux {`, remplacer la chaîne de format et ajouter
l'argument correspondant :

```rust
                println!(
                    "signaux : inactif {:.0} s · {} · {} h · batterie {} · verrouillé {} \
                     · latence {:.0} ms → flâner ×{:.2} reposer ×{:.2} jouer ×{:.2}",
                    s.inactivite.as_secs_f32(),
                    s.appli_active.as_deref().unwrap_or("-"),
                    s.heure,
                    match s.batterie.pourcent {
                        Some(p) => format!("{p} %"),
                        None => "-".to_string(),
                    },
                    s.session_verrouillee,
                    s.latence_file.as_secs_f32() * 1000.0,
                    biais.flaner,
                    biais.se_reposer,
                    biais.jouer,
                );
```

- [ ] **Étape 4 : agréger les échecs de rendu**

Remplacer le bloc `if let Err(e) = render::pousser(…)` (`main.rs:2008`) par :

```rust
            if amorcage || acteur.dernier_rendu != Some(rendu) {
                if render::pousser(&handle, &acteur.label, rendu).is_err() {
                    // ⚠️ **Compté, pas imprimé.** Ce message sortait des
                    // centaines de fois par seconde à onze personnages, et il
                    // noyait tout le reste — y compris les lignes qui auraient
                    // servi au diagnostic. Le total part dans la trace de
                    // cadence, une fois toutes les cinq secondes.
                    //
                    // Et l'échec n'est PAS anodin : il veut dire que la file du
                    // thread principal a débordé (voir §2 de la spec). S'il
                    // n'est jamais nul, la régulation n'a pas suffi.
                    echecs_de_rendu += 1;
                } else {
                    acteur.dernier_rendu = Some(rendu);
                }
            }
```

> ⚠️ Vérifier en écrivant ce bloc que `acteur.dernier_rendu = Some(rendu);`
> n'est bien affecté que dans la branche de succès — c'est déjà le cas
> aujourd'hui, et l'inverse ferait sauter à jamais une frame perdue.

- [ ] **Étape 5 : afficher le compteur dans `SHIMEJI_CADENCE=1`**

Dans le bloc de trace de cadence, juste après le `println!` existant, et avant
les remises à zéro :

```rust
                // N'afficher que si ce n'est pas zéro : une ligne « 0 échec »
                // toutes les cinq secondes serait du bruit, et c'est
                // l'apparition du chiffre qui doit sauter aux yeux.
                if echecs_de_rendu > 0 {
                    println!(
                        "  ⚠️  {echecs_de_rendu} images perdues : la file du thread \
                         principal a débordé (latence {:.0} ms)",
                        moniteur_charge.latence().as_secs_f32() * 1000.0
                    );
                }
                echecs_de_rendu = 0;
```

- [ ] **Étape 6 : compiler et lancer toute la suite**

```powershell
cargo test --quiet 2>&1 | Select-Object -Last 8
cargo build 2>&1 | Select-Object -Last 8
```

Attendu : 320 tests au vert, et un binaire à jour (obligatoire avant la Tâche 4 —
`cargo test` ne reconstruit pas l'exe).

- [ ] **Étape 7 : vérifier que le signal reste muet au repos**

```powershell
$env:SHIMEJI_PERSONNAGES = "blob"; $env:SHIMEJI_SIGNAUX = "1"
cargo run 2>&1 | Select-String "latence" | Select-Object -First 5
```

Attendu : `latence 0 ms` ou quelques millisecondes, et **aucun** changement des
multiplicateurs. Un personnage seul ne doit rien déclencher — si la latence est
déjà à 100 ms ici, le seuil est mal choisi et il faut le dire avant d'aller plus
loin.

- [ ] **Étape 8 : commit**

```bash
git add src-tauri/src/main.rs
git commit -m "feat(charge): brancher le signal dans la boucle, et agreger les echecs"
```

---

### Tâche 4 : la preuve, et la documentation

**Fichiers :**
- Créer : `docs/specs/2026-09-21-mesure-regulation.md`
- Modifier : `CLAUDE.md` (l'avertissement à 10, la liste des variables, l'état actuel)

**Interfaces :**
- Consomme : tout ce qui précède.

**La preuve est une mesure, pas une impression.** La configuration à reproduire
est celle qui échouait : **11 personnages en debug**. C'est le seul essai qui
vaut, parce que le release à N=11 ne saturait déjà pas (spec §3).

- [ ] **Étape 1 : mesurer AVANT, sur le commit d'origine du défaut**

```powershell
cd C:\Users\alri\Documents\shimeji-desktop\src-tauri
git stash
git checkout 446de90 -- .
cargo build 2>&1 | Select-Object -Last 3
$env:SHIMEJI_PERSONNAGES = (@("blob") * 11) -join ","
$env:SHIMEJI_CADENCE = "1"
cargo run 2>&1 | Select-String "rendu impossible" | Measure-Object | Select-Object -ExpandProperty Count
```

Laisser tourner ~40 s, arrêter, noter le nombre. Puis revenir :

```powershell
git checkout HEAD -- .
git stash pop
```

- [ ] **Étape 2 : mesurer APRÈS**

```powershell
cargo build 2>&1 | Select-Object -Last 3
$env:SHIMEJI_PERSONNAGES = (@("blob") * 11) -join ","
$env:SHIMEJI_CADENCE = "1"; $env:SHIMEJI_SIGNAUX = "1"
cargo run
```

Pendant ~60 s, relever : le nombre d'« images perdues », la latence affichée, la
cadence tenue, et le nombre de personnages assis à l'œil.

- [ ] **Étape 3 : vérifier ce qui avait cassé — à la main, il n'y a pas d'autre voie**

Sur cette même instance à onze personnages :

1. clic droit sur un personnage → le menu s'ouvre **et se ferme** après un choix ;
2. l'action choisie est bien jouée par **ce** personnage ;
3. maintenir le clic gauche l'attrape et le relâcher le fait tomber ;
4. le menu du tray répond.

> C'est la deuxième exception à « tout se vérifie sans humain » de ce projet, et
> pour la même raison que la première : que Windows *dépêche* les messages d'un
> menu ne se script pas.

- [ ] **Étape 4 : écrire la mesure**

Créer `docs/specs/2026-09-21-mesure-regulation.md` avec le tableau
avant/après (images perdues, latence, cadence, personnages assis), la
configuration exacte, et — si une hypothèse a été démentie en route — la
consigner, comme le fait déjà le dossier CPU.

- [ ] **Étape 5 : mettre `CLAUDE.md` à jour**

Trois retouches, et pas une de plus :

1. Dans « Le mode caché est PLAT », après la phrase sur l'absence de plafond,
   ajouter que ce qui se perd au-delà n'est pas seulement du CPU mais
   **l'interactivité** (menu, attrapage), et que la régulation de charge y
   répond en biaisant vers le repos — avec le renvoi à la nouvelle spec.
2. Dans le tableau des variables de diagnostic, préciser que `SHIMEJI_SIGNAUX=1`
   affiche désormais **six** signaux, latence comprise.
3. Dans le tableau des documents, ajouter les deux nouveaux fichiers
   (`2026-09-21-regulation-de-charge-design.md` et `2026-09-21-mesure-regulation.md`).

> ⚠️ `CLAUDE.md` doit rester court : il est relu à chaque session. Le récit va
> dans `docs/`, pas ici.

- [ ] **Étape 6 : commit**

```bash
git add docs CLAUDE.md
git commit -m "docs: la mesure de la regulation de charge, avant et apres"
```

---

## Ce que ce plan ne fait PAS

- **Aucun plafond**, aucun refus d'ajouter un personnage (décision de l'auteur).
- **Aucune baisse de cadence** — règle n° 1 héritée de l'étape 0.
- **Aucun changement de transport** — mesuré et refusé, spec §3.
- **Aucune entrée de menu** : la régulation n'est pas une action jouable, la règle
  « toute nouvelle action se branche au menu du clic droit » ne s'applique pas ici.
- **Aucun état « bridé »** par personnage : un multiplicateur, et rien d'autre.
