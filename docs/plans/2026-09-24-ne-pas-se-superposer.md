# Ne pas se superposer — plan d'implémentation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal :** les personnages se traversent toujours mais ne s'arrêtent jamais l'un sur l'autre au sol ; ils s'accrochent au mur un par un en faisant la file ; et « Tout le monde › Rester accroché » existe.

**Architecture :** un module pur `behavior/place.rs` (occupants, place libre, bas du mur libre) ; `behavior::pas_parmi` reçoit la liste des occupants de l'image précédente et fait se décaler un personnage à l'arrêt qui chevauche un plus ancien ; la phase `Rejoindre` de l'escalade gère la file. `pas` et `poursuivre` restent, et appellent leurs versions `_parmi` avec une liste vide : la simulation et les 413 tests existants ne changent pas.

**Tech Stack :** Rust, Tauri 2.11.5 — compiler depuis **PowerShell** (`cargo test --quiet`), jamais Git Bash.

**Spec :** `docs/specs/2026-09-24-ne-pas-se-superposer-design.md`

## Global Constraints

- **La règle : on se traverse toujours, on ne s'arrête jamais l'un sur l'autre** — au sol seulement.
- Sur les murs et au plafond, **aucune** place occupée ; seule l'**entrée** sur le mur se fait un par un (zone de départ = une hauteur de corps depuis le bas).
- Espacement : **côte à côte** — deux places qui se touchent ne se chevauchent pas (comparaison stricte).
- Largeur et hauteur de corps : hitbox de la pose **`stand`** × `echelle_affichage`.
- Le **dernier arrivé** cède (`arrete_depuis` plus grand ; égalité → plus grand numéro d'acteur).
- Pas de place libre → **il reste où il est** (chevauchement accepté, jamais d'errance).
- Attente dans la file : **hors délai d'abandon**, mais abandon après **120 s** d'attente.
- Commentaires **en français**, abondants, qui expliquent le Rust non trivial (CLAUDE.md, « Conventions de code »).
- **Jamais** d'édition par PowerShell `Get-Content`/`Set-Content` : outil d'édition, ou Python en UTF-8 explicite (`newline='\n'`).
- Tout nouvel identifiant de menu : table `TOUS` de `menu_perso.rs`, rien d'autre.
- Ne **pas** arrêter l'instance release de l'auteur ; les vérifications passent par `cargo run` (debug).

## Review Focus

1. **Un acteur figé par son menu ouvert** (`fige` dans `main.rs`) doit continuer d'occuper sa place — sinon on s'assoit sur celui qu'on est en train de cliquer. → Tâche 5 pousse son occupant même quand `pas` ne tourne pas.
2. **Un acteur en départ** (animation de sortie) ne doit PAS occuper de place. → Tâche 5 : le `continue` du départ passe avant l'enregistrement de l'occupant.
3. **Un écran plein** (plus de corps que de largeur) ne doit pas faire osciller les personnages. → Tâche 2, test `ecran_plein_il_reste_ou_il_est`.
4. **Un marcheur vers le mur d'un écran voisin** (écran du milieu) ne fait la file que sur le sol de ce mur, pas en route. → Tâche 3, test `la_file_ne_joue_que_sur_le_sol_du_mur`.
5. **Deux personnages qui s'arrêtent à la même image** doivent être départagés sans hasard et sans que les deux bougent. → Tâche 1, test `egalite_le_plus_grand_numero_cede`.

---

### Task 1 : le module pur `behavior/place.rs`

**Files :**
- Create : `src-tauri/src/behavior/place.rs`
- Create : `src-tauri/src/behavior/place_tests.rs`
- Modify : `src-tauri/src/behavior/mod.rs` (ligne `pub mod tenue;` → ajouter `pub mod place;`)
- Modify : `src-tauri/src/character/mod.rs` (champ `arrete_depuis`)

**Interfaces :**
- Produces :
  - `pub struct Occupant { pub acteur: u32, pub platform: PlatformId, pub face: Face, pub offset: f32, pub demi_largeur: f32, pub arrete_depuis: Option<Duration>, pub attend_le_mur: Option<PlatformId> }` (`Debug, Clone, Copy, PartialEq`)
  - `pub struct Voisinage<'a> { pub moi: u32, pub autres: &'a [Occupant] }` + `pub fn seul() -> Voisinage<'static>`
  - `pub fn corps(ch: &Character, echelle: f32) -> (f32, f32)` → (demi-largeur, hauteur)
  - `pub fn a_l_arret(ch: &Character) -> bool`
  - `pub fn attend_le_mur(ch: &Character) -> Option<PlatformId>`
  - `pub fn occupant_de(acteur: u32, ch: &Character, echelle: f32) -> Option<Occupant>`
  - `pub fn place_libre(v: &Voisinage, platform: PlatformId, offset: f32, demi: f32, longueur: f32) -> Option<f32>`
  - `pub fn doit_ceder(v: &Voisinage, platform: PlatformId, offset: f32, demi: f32, arrete_depuis: Option<Duration>) -> bool`
  - `pub fn bas_du_mur_libre(v: &Voisinage, mur: PlatformId, longueur_mur: f32, hauteur: f32) -> bool`
  - `pub fn premier_de_la_file(v: &Voisinage, mur: PlatformId, pied: f32, mon_ecart: f32) -> bool`
  - `Character::arrete_depuis: Option<Duration>`
  - `PhaseGrimpe::Rejoindre { mur, presse, attend_depuis: Option<Duration> }` — **le champ `attend_depuis` est ajouté dès cette tâche** (`a_l_arret` le lit) ; la Tâche 3 s'en sert.

- [ ] **Step 1 : le champ `arrete_depuis` et le champ `attend_depuis`**

Dans `src-tauri/src/character/mod.rs`, après le champ `a_jouer` :

```rust
    /// Depuis quand il est à l'arrêt AU SOL (assis, endormi, en pause, en
    /// file au pied d'un mur…), `None` s'il marche, tombe ou est porté.
    /// C'est ce qui départage deux personnages qui voudraient la même place :
    /// le dernier arrivé cède (spec « ne pas se superposer » §3). Tenu par
    /// `behavior::pas_parmi`. Pour la session seulement.
    pub arrete_depuis: Option<Duration>,
```

et dans `Character::new`, après `a_jouer: None,` : `arrete_depuis: None,`.

Dans `src-tauri/src/behavior/intention.rs`, la variante devient :

```rust
    /// Marcher jusqu'au pied du mur — ou y attendre son tour.
    ///
    /// `attend_depuis` : `Some(t)` quand il fait la file (le bas du mur est
    /// pris, spec « ne pas se superposer » §4), depuis l'instant `t`. C'est
    /// ce qui le compte comme « à l'arrêt » (il occupe sa place dans la
    /// file), et ce qui borne l'attente à 120 s.
    Rejoindre { mur: PlatformId, presse: bool, attend_depuis: Option<Duration> },
```

et à la construction (ligne ~786, phase `Choisir`) : `phase = PhaseGrimpe::Rejoindre { mur, presse, attend_depuis: None };`. Dans le bras `PhaseGrimpe::Rejoindre { mur, presse } =>` (ligne ~848), écrire `PhaseGrimpe::Rejoindre { mur, presse, .. } =>` pour l'instant.

- [ ] **Step 2 : écrire les tests de `place.rs`**

`src-tauri/src/behavior/place_tests.rs` :

```rust
//! Tests de `place.rs` — des cas écrits à la main, sans monde ni boucle.

use super::*;
use crate::world::{PlatformId, RoleEcran};
use std::time::Duration;

fn sol() -> PlatformId {
    PlatformId::ecran(1, RoleEcran::Sol)
}
fn mur() -> PlatformId {
    PlatformId::ecran(1, RoleEcran::MurGauche)
}

/// Un occupant assis sur le sol, arrêté depuis `depuis` secondes.
fn assis(acteur: u32, offset: f32, depuis: u64) -> Occupant {
    Occupant {
        acteur,
        platform: sol(),
        face: Face::Top,
        offset,
        demi_largeur: 45.0,
        arrete_depuis: Some(Duration::from_secs(depuis)),
        attend_le_mur: None,
    }
}

#[test]
fn une_place_libre_est_rendue_telle_quelle() {
    let autres = [assis(1, 100.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    assert_eq!(place_libre(&v, sol(), 500.0, 45.0, 1920.0), Some(500.0));
}

#[test]
fn sur_une_place_prise_il_va_a_cote_du_cote_le_plus_proche() {
    // Assis à 500 (couvre 455..545). Moi à 510 : la place libre la plus
    // proche est collée à droite, 545 + 45 = 590 (côte à côte).
    let autres = [assis(1, 500.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    assert_eq!(place_libre(&v, sol(), 510.0, 45.0, 1920.0), Some(590.0));
    // Moi à 490 : collé à gauche, 455 − 45 = 410.
    assert_eq!(place_libre(&v, sol(), 490.0, 45.0, 1920.0), Some(410.0));
}

#[test]
fn il_saute_par_dessus_une_rangee_entiere() {
    let autres = [assis(1, 500.0, 1), assis(2, 590.0, 1), assis(3, 680.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    // Au milieu de la rangée (590) : 410 et 770 sont à égale distance
    // (180) ; on garde le premier trouvé — peu importe, pourvu que ce soit
    // hors de la rangée.
    let p = place_libre(&v, sol(), 600.0, 45.0, 1920.0).unwrap();
    assert!(p <= 410.0 || p >= 770.0, "{p}");
}

#[test]
fn un_marcheur_n_occupe_rien() {
    let mut o = assis(1, 500.0, 1);
    o.arrete_depuis = None;
    let autres = [o];
    let v = Voisinage { moi: 9, autres: &autres };
    assert_eq!(place_libre(&v, sol(), 500.0, 45.0, 1920.0), Some(500.0));
}

#[test]
fn il_s_ignore_lui_meme() {
    let autres = [assis(9, 500.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    assert_eq!(place_libre(&v, sol(), 500.0, 45.0, 1920.0), Some(500.0));
}

#[test]
fn le_bord_du_sol_borne_la_place() {
    // Assis à 45 (couvre 0..90). Moi à 20 : à gauche il n'y a plus de sol,
    // la place est à droite, 135.
    let autres = [assis(1, 45.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    assert_eq!(place_libre(&v, sol(), 20.0, 45.0, 1920.0), Some(135.0));
}

#[test]
fn ecran_plein_aucune_place() {
    // Un sol de 200 px, déjà occupé au milieu : aucune place de 90 px.
    let autres = [assis(1, 100.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    assert_eq!(place_libre(&v, sol(), 100.0, 45.0, 200.0), None);
}

#[test]
fn le_dernier_arrive_cede() {
    let autres = [assis(1, 500.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    // Moi arrêté à 3 s, lui à 1 s : c'est moi qui cède.
    assert!(doit_ceder(&v, sol(), 510.0, 45.0, Some(Duration::from_secs(3))));
    // Moi arrêté à 0 s : je suis le plus ancien, je reste.
    assert!(!doit_ceder(&v, sol(), 510.0, 45.0, Some(Duration::from_secs(0))));
}

#[test]
fn egalite_le_plus_grand_numero_cede() {
    let autres = [assis(1, 500.0, 2)];
    let v9 = Voisinage { moi: 9, autres: &autres };
    assert!(doit_ceder(&v9, sol(), 510.0, 45.0, Some(Duration::from_secs(2))));
    let autres = [assis(9, 510.0, 2)];
    let v1 = Voisinage { moi: 1, autres: &autres };
    assert!(!doit_ceder(&v1, sol(), 500.0, 45.0, Some(Duration::from_secs(2))));
}

#[test]
fn cote_a_cote_n_est_pas_un_chevauchement() {
    let autres = [assis(1, 500.0, 1)];
    let v = Voisinage { moi: 9, autres: &autres };
    assert!(!doit_ceder(&v, sol(), 590.0, 45.0, Some(Duration::from_secs(3))));
}

#[test]
fn le_bas_du_mur_est_pris_tant_que_le_grimpeur_n_a_pas_monte_d_un_corps() {
    // Mur de 1000 px, offset compté vers le BAS depuis le haut : le bas est
    // à 1000. Hauteur de corps 100 : la zone de départ est 900..1000.
    let grimpeur = |offset| Occupant {
        acteur: 1,
        platform: mur(),
        face: Face::Right,
        offset,
        demi_largeur: 45.0,
        arrete_depuis: None, // en mouvement : compte quand même sur le mur
        attend_le_mur: None,
    };
    let autres = [grimpeur(950.0)];
    assert!(!bas_du_mur_libre(&Voisinage { moi: 9, autres: &autres }, mur(), 1000.0, 100.0));
    let autres = [grimpeur(880.0)];
    assert!(bas_du_mur_libre(&Voisinage { moi: 9, autres: &autres }, mur(), 1000.0, 100.0));
}

#[test]
fn seul_le_plus_proche_du_mur_part_le_premier() {
    let en_file = |acteur, offset| Occupant {
        attend_le_mur: Some(mur()),
        ..assis(acteur, offset, 1)
    };
    // Pied du mur à 0. Lui à 45, moi à 135 : il passe avant moi.
    let autres = [en_file(1, 45.0)];
    let v = Voisinage { moi: 9, autres: &autres };
    assert!(!premier_de_la_file(&v, mur(), 0.0, 135.0));
    // Seul en file : je suis le premier.
    let v = Voisinage { moi: 9, autres: &[] };
    assert!(premier_de_la_file(&v, mur(), 0.0, 135.0));
}
```

- [ ] **Step 3 : lancer, voir échouer**

Run (PowerShell, `src-tauri`) : `cargo test --quiet place 2>&1 | Select-Object -Last 15`
Expected : échec de compilation — `place` n'existe pas.

- [ ] **Step 4 : écrire `place.rs`**

```rust
//! Qui occupe quelle place, et où en trouver une libre (spec « ne pas se
//! superposer », 2026-09-24).
//!
//! Responsabilité unique : des questions sur les places — fonctions pures,
//! ni Tauri, ni écran, ni boucle. Ce que le personnage en FAIT (se décaler,
//! faire la file) est dans `behavior::pas_parmi` et `intention::grimper`.
//!
//! **La règle : on se traverse toujours, on ne s'arrête jamais l'un sur
//! l'autre.** Au sol seulement : sur les murs et au plafond, personne
//! n'occupe rien — seule l'entrée sur un mur se fait un par un.

use super::intention::{Allure, EtatIntention, Intention, PhaseGrimpe};
use crate::character::attach::Attachment;
use crate::character::manifest::POSE_STAND;
use crate::character::Character;
use crate::geom::Face;
use crate::world::PlatformId;
use std::time::Duration;

/// Un personnage posé quelque part, tel que les autres le voient.
///
/// La boucle en dresse la liste une fois par image (`occupant_de`), pour
/// TOUS les personnages posés, en mouvement compris : chaque question
/// ci-dessous filtre ce qui la concerne.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Occupant {
    pub acteur: u32,
    pub platform: PlatformId,
    pub face: Face,
    /// La distance au bord le long de la face (décision n° 1).
    pub offset: f32,
    pub demi_largeur: f32,
    /// `Some` s'il est à l'arrêt au sol — voir `Character::arrete_depuis`.
    pub arrete_depuis: Option<Duration>,
    /// `Some(mur)` s'il fait la file au pied de ce mur.
    pub attend_le_mur: Option<PlatformId>,
}

/// Ce qu'un personnage sait des autres à cette image : qui il est, et la
/// liste des occupants (dont lui-même, qu'il ignore par son numéro).
///
/// `'a` : la liste est EMPRUNTÉE à la boucle, pour une image — le
/// voisinage ne vit pas plus longtemps qu'elle.
#[derive(Debug, Clone, Copy)]
pub struct Voisinage<'a> {
    pub moi: u32,
    pub autres: &'a [Occupant],
}

impl Voisinage<'static> {
    /// Seul au monde : ce que reçoivent la simulation et les tests
    /// existants, par `behavior::pas` et `intention::poursuivre`.
    pub fn seul() -> Voisinage<'static> {
        Voisinage { moi: u32::MAX, autres: &[] }
    }
}

impl<'a> Voisinage<'a> {
    /// Les autres — lui-même exclu. `impl Iterator` : un itérateur dont on
    /// ne nomme pas le type exact (il est long et sans intérêt).
    fn autres(&self) -> impl Iterator<Item = &'a Occupant> + '_ {
        let moi = self.moi;
        self.autres.iter().filter(move |o| o.acteur != moi)
    }
}

/// La demi-largeur et la hauteur de son corps, à cette échelle.
///
/// La hitbox de `stand` et non de la pose courante : la largeur ne doit pas
/// changer quand il s'assoit ou s'endort, sinon la rangée se réarrangerait
/// à chaque changement de pose.
pub fn corps(ch: &Character, echelle: f32) -> (f32, f32) {
    let h = ch.manifest.hitbox_de(POSE_STAND);
    (h.w * echelle / 2.0, h.h * echelle)
}

/// Est-il à l'arrêt ? Tout ce qui ne le déplace pas : se reposer (toutes
/// phases), jouer, flâner en allure `Arret`, et attendre dans la file d'un
/// mur. Le « au sol » est vérifié par l'appelant.
pub fn a_l_arret(ch: &Character) -> bool {
    let Some(ai) = ch.intention else {
        return false;
    };
    match (ai.kind, ai.etat) {
        (Intention::SeReposer, _) | (Intention::Jouer(_), _) => true,
        (Intention::Flaner, EtatIntention::Flanerie { allure, .. }) => allure == Allure::Arret,
        (Intention::Grimper, _) => attend_le_mur(ch).is_some(),
        _ => false,
    }
}

/// Le mur dont il fait la file, s'il en fait une.
pub fn attend_le_mur(ch: &Character) -> Option<PlatformId> {
    match ch.intention?.etat {
        EtatIntention::Grimpe {
            phase: PhaseGrimpe::Rejoindre { mur, attend_depuis: Some(_), .. },
            ..
        } => Some(mur),
        _ => None,
    }
}

/// L'occupant qu'il est, s'il est posé quelque part.
pub fn occupant_de(acteur: u32, ch: &Character, echelle: f32) -> Option<Occupant> {
    let Attachment::On { platform, face, offset } = ch.attachment else {
        return None;
    };
    let (demi_largeur, _) = corps(ch, echelle);
    Some(Occupant {
        acteur,
        platform,
        face,
        offset,
        demi_largeur,
        arrete_depuis: ch.arrete_depuis,
        attend_le_mur: attend_le_mur(ch),
    })
}

/// Les occupants à l'arrêt sur cette face de sol, lui-même exclu.
fn arretes_sur<'a>(v: &'a Voisinage, platform: PlatformId) -> impl Iterator<Item = &'a Occupant> + 'a {
    v.autres()
        .filter(move |o| o.platform == platform && o.face == Face::Top && o.arrete_depuis.is_some())
}

/// Deux places se chevauchent-elles ? **Strictement** : deux places qui se
/// touchent (côte à côte) ne se chevauchent pas. Le `- 0.01` absorbe les
/// arrondis des flottants.
fn chevauche(a: f32, demi_a: f32, b: f32, demi_b: f32) -> bool {
    (a - b).abs() < demi_a + demi_b - 0.01
}

/// La place libre la plus proche de `offset` sur ce sol, pour un corps de
/// demi-largeur `demi`, ou `None` s'il n'y en a aucune.
///
/// Les candidats sont `offset` lui-même, puis les deux bords de chaque place
/// prise (collés à elle, côte à côte), rabattus dans le sol. Le plus proche
/// qui ne chevauche personne gagne. Aucune recherche plus fine : une place
/// libre est forcément collée à un occupant, ou là où il est déjà.
pub fn place_libre(v: &Voisinage, platform: PlatformId, offset: f32, demi: f32, longueur: f32) -> Option<f32> {
    // Un sol trop court pour lui : aucune place.
    if longueur < 2.0 * demi {
        return None;
    }
    let dans_le_sol = |x: f32| x.clamp(demi, longueur - demi);

    let mut candidats = vec![dans_le_sol(offset)];
    for o in arretes_sur(v, platform) {
        candidats.push(dans_le_sol(o.offset - o.demi_largeur - demi));
        candidats.push(dans_le_sol(o.offset + o.demi_largeur + demi));
    }

    let mut meilleur: Option<f32> = None;
    for c in candidats {
        let libre = arretes_sur(v, platform).all(|o| !chevauche(c, demi, o.offset, o.demi_largeur));
        if !libre {
            continue;
        }
        let mieux = match meilleur {
            None => true,
            Some(m) => (c - offset).abs() < (m - offset).abs(),
        };
        if mieux {
            meilleur = Some(c);
        }
    }
    meilleur
}

/// Doit-il céder sa place ? Oui si un occupant PLUS ANCIEN, à l'arrêt sur
/// le même sol, la chevauche. Plus ancien = arrêté plus tôt ; à égalité, le
/// plus petit numéro (déterministe, sans hasard).
pub fn doit_ceder(
    v: &Voisinage,
    platform: PlatformId,
    offset: f32,
    demi: f32,
    arrete_depuis: Option<Duration>,
) -> bool {
    let Some(moi_depuis) = arrete_depuis else {
        return false;
    };
    arretes_sur(v, platform).any(|o| {
        let lui_depuis = o.arrete_depuis.unwrap_or(Duration::ZERO);
        let plus_ancien = lui_depuis < moi_depuis || (lui_depuis == moi_depuis && o.acteur < v.moi);
        plus_ancien && chevauche(offset, demi, o.offset, o.demi_largeur)
    })
}

/// Le bas de ce mur est-il libre ? Non si quelqu'un s'y tient à moins d'une
/// hauteur de corps du bas — en mouvement compris : c'est la zone de départ,
/// la seule contrainte verticale (spec §4). L'offset d'un mur compte vers le
/// BAS depuis le haut : le bas est à `longueur_mur`.
pub fn bas_du_mur_libre(v: &Voisinage, mur: PlatformId, longueur_mur: f32, hauteur: f32) -> bool {
    !v.autres().any(|o| o.platform == mur && o.offset > longueur_mur - hauteur)
}

/// Est-il le premier de la file de ce mur ? Oui si personne n'y attend plus
/// près du pied (`pied`, offset sur le sol) que lui (`mon_ecart`). Sans
/// cela, quand le mur se libère, toute la file se ruerait vers lui.
pub fn premier_de_la_file(v: &Voisinage, mur: PlatformId, pied: f32, mon_ecart: f32) -> bool {
    !v.autres().any(|o| {
        o.attend_le_mur == Some(mur) && {
            let son_ecart = (o.offset - pied).abs();
            son_ecart < mon_ecart || (son_ecart == mon_ecart && o.acteur < v.moi)
        }
    })
}

#[cfg(test)]
#[path = "place_tests.rs"]
mod tests;
```

Vérifier au passage les noms exacts : `PlatformId::ecran`, `RoleEcran::{Sol, MurGauche}` (`world.rs:50`), `Hitbox { w, h }` (`manifest.rs:166`), `POSE_STAND`. Si un nom diffère, corriger le test, pas l'API.

- [ ] **Step 5 : lancer, voir passer**

Run : `cargo test --quiet 2>&1 | Select-String 'test result|^error' -Context 0,6`
Expected : tous verts (413 + 12).

- [ ] **Step 6 : commit**

```powershell
git add src-tauri/src/behavior/place.rs src-tauri/src/behavior/place_tests.rs src-tauri/src/behavior/mod.rs src-tauri/src/behavior/intention.rs src-tauri/src/character/mod.rs
git commit -m "feat(place): qui occupe quelle place, et ou en trouver une libre"
```

---

### Task 2 : se décaler au sol — `pas_parmi`

**Files :**
- Modify : `src-tauri/src/behavior/mod.rs` (`pas` → `pas_parmi`)
- Modify : `src-tauri/src/behavior/intention.rs` (nouvelle `pub(crate) fn marcher_vers`)
- Test : `src-tauri/src/behavior/tests.rs` (section « à plusieurs »)

**Interfaces :**
- Consumes : tout `place.rs` (Tâche 1).
- Produces :
  - `pub fn pas_parmi(ch: &mut Character, world: &World, e: &Entrees, table: &TableEnvies, reglages: &Reglages, maintenant: Duration, dt: f32, rng: &mut dyn Rng, voisins: &place::Voisinage) -> reflex::Reflexe`
  - `pas(...)` inchangé en signature, devient `pas_parmi(..., &place::Voisinage::seul())`.
  - `pub(crate) fn marcher_vers(ch: &mut Character, cible: f32, reglages: &Reglages, maintenant: Duration, dt: f32) -> bool` (vrai = arrivé)
  - helper de test `jouer_foule(persos: &mut [Character], m: &World, e: &Entrees, rng, debut, n, chaque: impl FnMut(&[Character])) -> Duration`

- [ ] **Step 1 : le harnais à plusieurs et les tests**

À la fin de `src-tauri/src/behavior/tests.rs` :

```rust
// ── À plusieurs : ne pas se superposer (spec 2026-09-24) ───────────────────
//
// Un harnais qui fait avancer N personnages ensemble, COMME la boucle de
// `main.rs` : chacun reçoit les occupants de l'image précédente.

fn jouer_foule(
    persos: &mut [Character],
    m: &World,
    e: &Entrees,
    rng: &mut XorShift32,
    debut: Duration,
    n: u32,
    mut chaque: impl FnMut(&[Character]),
) -> Duration {
    let table = desire::TableEnvies::defaut();
    let reglages = reglages_defaut();
    let mut t = debut;
    for _ in 0..n {
        t += Duration::from_secs_f32(DT);
        let occupants: Vec<place::Occupant> = persos
            .iter()
            .enumerate()
            .filter_map(|(i, ch)| place::occupant_de(i as u32, ch, e.echelle_affichage))
            .collect();
        for (i, ch) in persos.iter_mut().enumerate() {
            let v = place::Voisinage { moi: i as u32, autres: &occupants };
            pas_parmi(ch, m, e, &table, &reglages, t, DT, rng, &v);
        }
        chaque(persos);
    }
    t
}

/// La même commande pour tous, à la même image — « Tout le monde › … ».
fn commander_foule(persos: &mut [Character], m: &World, c: crate::menu_perso::Commande, t: Duration, rng: &mut XorShift32) {
    for ch in persos.iter_mut() {
        commander(ch, m, c, t, rng);
    }
}

/// Deux personnages à l'arrêt, sur le même sol, qui se chevauchent.
fn chevauchement_a_l_arret(persos: &[Character]) -> Option<(usize, usize)> {
    let e = entrees(true, 1.0);
    for i in 0..persos.len() {
        for j in (i + 1)..persos.len() {
            let (a, b) = (&persos[i], &persos[j]);
            let (Attachment::On { platform: pa, face: Face::Top, offset: oa }, Attachment::On { platform: pb, face: Face::Top, offset: ob }) = (a.attachment, b.attachment) else {
                continue;
            };
            if pa != pb || !place::a_l_arret(a) || !place::a_l_arret(b) {
                continue;
            }
            let (da, _) = place::corps(a, e.echelle_affichage);
            let (db, _) = place::corps(b, e.echelle_affichage);
            if (oa - ob).abs() < da + db - 1.0 {
                return Some((i, j));
            }
        }
    }
    None
}

fn foule_au_meme_endroit(m: &World, n: usize, offset: f32) -> Vec<Character> {
    let sol = &m.platforms()[0];
    (0..n)
        .map(|_| {
            let mut ch = perso(m);
            ch.attachment = Attachment::On { platform: sol.id, face: Face::Top, offset };
            ch
        })
        .collect()
}

#[test]
fn tout_le_monde_s_assoit_en_rangee() {
    use crate::menu_perso::Commande;
    let m = monde();
    let mut persos = foule_au_meme_endroit(&m, 5, 800.0);
    let mut rng = XorShift32::seeded(71);
    let t0 = Duration::from_secs(1);
    commander_foule(&mut persos, &m, Commande::Imposer(tenue::Tenue::Asseoir), t0, &mut rng);

    // 20 s pour se ranger : un corps (~90 px) à 40 px/s, pour les plus
    // éloignés deux corps — large.
    let t = jouer_foule(&mut persos, &m, &entrees(true, 1.0), &mut rng, t0, 1200, |_| {});
    // Puis 10 s de contrôle : tous à l'arrêt, assis tenu, aucun
    // chevauchement. (Pas `pose == sit` à chaque image : entre deux repos,
    // la tenue relance `SeReposer`, qui peut passer par une autre pose.)
    jouer_foule(&mut persos, &m, &entrees(true, 1.0), &mut rng, t, 600, |p| {
        assert_eq!(chevauchement_a_l_arret(p), None);
        for ch in p {
            assert!(place::a_l_arret(ch), "{:?}", ch.intention);
            assert_eq!(ch.tenue, Some(tenue::Tenue::Asseoir));
        }
    });
}

#[test]
fn lache_sur_un_assis_il_se_decale() {
    use crate::menu_perso::Commande;
    let m = monde();
    let mut persos = foule_au_meme_endroit(&m, 2, 800.0);
    let mut rng = XorShift32::seeded(73);
    let t0 = Duration::from_secs(1);
    commander(&mut persos[0], &m, Commande::Basculer(tenue::Tenue::Asseoir), t0, &mut rng);
    // Le second tombe pile au-dessus.
    let pos = persos[1].pos_connue;
    persos[1].attachment = Attachment::Falling { pos: Point::new(pos.x, pos.y - 300.0), vel: crate::geom::Vec2::zero() };

    // Il tombe, atterrit, s'étale (à l'arrêt, plus récent) : il doit
    // s'écarter. Jamais plus de 3 s de suite l'un sur l'autre à l'arrêt.
    let mut a_la_suite = 0u32;
    jouer_foule(&mut persos, &m, &entrees(true, 1.0), &mut rng, t0, 1800, |p| {
        if chevauchement_a_l_arret(p).is_some() {
            a_la_suite += 1;
            assert!(a_la_suite < 180, "superposés à l'arrêt depuis 3 s");
        } else {
            a_la_suite = 0;
        }
    });
}

#[test]
fn deux_marcheurs_se_traversent_sans_devier() {
    let m = monde();
    let mut persos = foule_au_meme_endroit(&m, 2, 0.0);
    let t0 = Duration::from_secs(1);
    let marche = |ch: &mut Character, offset: f32, facing| {
        let sol = &m.platforms()[0];
        ch.attachment = Attachment::On { platform: sol.id, face: Face::Top, offset };
        ch.facing = facing;
        ch.intention = Some(intention::ActiveIntention {
            kind: intention::Intention::Flaner,
            depuis: t0,
            etat: intention::EtatIntention::Flanerie { allure: intention::Allure::Marche, jusqu_a: t0 + Duration::from_secs(30) },
        });
    };
    marche(&mut persos[0], 600.0, crate::character::Facing::Right);
    marche(&mut persos[1], 800.0, crate::character::Facing::Left);
    let offset = |ch: &Character| match ch.attachment {
        Attachment::On { offset, .. } => offset,
        _ => panic!("il a quitté le sol"),
    };
    let (mut a, mut b) = (offset(&persos[0]), offset(&persos[1]));
    let mut rng = XorShift32::seeded(79);
    // 8 s : ils se croisent vers 700, et continuent.
    jouer_foule(&mut persos, &m, &entrees(true, 1.0), &mut rng, t0, 480, |p| {
        let (na, nb) = (offset(&p[0]), offset(&p[1]));
        assert!(na > a && nb < b, "l'un a dévié ou s'est arrêté : {a}→{na}, {b}→{nb}");
        a = na;
        b = nb;
    });
    assert!(a > b, "ils ne se sont pas croisés");
}

#[test]
fn ecran_plein_il_reste_ou_il_est() {
    // Un seul sol, plus de corps que de largeur : pas d'oscillation, ils
    // restent où ils sont. On le lit sur `place::place_libre` → `None`, et
    // sur un personnage qui ne bouge pas d'un pixel pendant 5 s.
    use crate::menu_perso::Commande;
    let m = monde();
    let sol = &m.platforms()[0];
    let longueur = sol.rect.face_length(Face::Top);
    let (demi, _) = place::corps(&perso(&m), 1.0);
    let n = (longueur / (2.0 * demi)) as usize + 3;
    let mut persos: Vec<Character> = (0..n)
        .map(|i| {
            let mut ch = perso(&m);
            ch.attachment = Attachment::On { platform: sol.id, face: Face::Top, offset: (i as f32 * 2.0 * demi + demi).min(longueur - demi) };
            ch
        })
        .collect();
    let mut rng = XorShift32::seeded(83);
    let t0 = Duration::from_secs(1);
    commander_foule(&mut persos, &m, Commande::Imposer(tenue::Tenue::Asseoir), t0, &mut rng);
    let t = jouer_foule(&mut persos, &m, &entrees(true, 1.0), &mut rng, t0, 600, |_| {});
    let avant: Vec<_> = persos.iter().map(|c| c.attachment).collect();
    jouer_foule(&mut persos, &m, &entrees(true, 1.0), &mut rng, t, 300, |p| {
        let maintenant: Vec<_> = p.iter().map(|c| c.attachment).collect();
        assert_eq!(maintenant, avant, "quelqu'un bouge encore sur un écran plein");
    });
}
```

- [ ] **Step 2 : lancer, voir échouer**

Run : `cargo test --quiet 2>&1 | Select-String '^error' | Select-Object -First 5`
Expected : `cannot find function pas_parmi`.

- [ ] **Step 3 : `marcher_vers` dans `intention.rs`**

Après `fn avancer` :

```rust
/// Un pas de marche vers `cible`, sur la face où il se tient, sans en
/// sortir. Rend `true` s'il y est.
///
/// Pour se décaler d'une place (`behavior::pas_parmi`) ou avancer dans la
/// file d'un mur (`grimper`, phase `Rejoindre`) : de courtes distances sur
/// le même sol, donc ni bord, ni face voisine à traiter — contrairement à
/// `avancer`, qui sert à parcourir le monde.
pub(crate) fn marcher_vers(
    ch: &mut Character,
    cible: f32,
    reglages: &Reglages,
    maintenant: Duration,
    dt: f32,
) -> bool {
    let Attachment::On { platform, face, offset } = ch.attachment else {
        return false;
    };
    let ecart = cible - offset;
    let pas = reglages.vitesse_marche * dt;
    if ecart.abs() <= pas {
        ch.attachment = Attachment::On { platform, face, offset: cible };
        return true;
    }
    // `signum` : +1 vers la droite, −1 vers la gauche. Il regarde où il va.
    ch.facing = if ecart > 0.0 { Facing::Right } else { Facing::Left };
    ch.set_pose(POSE_WALK, maintenant);
    ch.attachment = Attachment::On { platform, face, offset: offset + pas * ecart.signum() };
    false
}
```

- [ ] **Step 4 : `pas_parmi` dans `behavior/mod.rs`**

Renommer la fonction `pas` en `pas_parmi`, lui ajouter le dernier paramètre `voisins: &place::Voisinage`, et recréer `pas` juste au-dessus :

```rust
/// Un pas de comportement pour un personnage SEUL AU MONDE — ce qu'appellent
/// la simulation et les tests. La boucle de `main.rs`, elle, appelle
/// `pas_parmi` avec les autres (spec « ne pas se superposer »).
pub fn pas(
    ch: &mut crate::character::Character,
    world: &World,
    e: &Entrees,
    table: &desire::TableEnvies,
    reglages: &crate::config::Reglages,
    maintenant: std::time::Duration,
    dt: f32,
    rng: &mut dyn crate::rng::Rng,
) -> reflex::Reflexe {
    pas_parmi(ch, world, e, table, reglages, maintenant, dt, rng, &place::Voisinage::seul())
}
```

(Recopier les types exacts des paramètres de l'actuelle `pas` — ceux ci-dessus sont ceux de `behavior/mod.rs:167`.)

Dans `pas_parmi`, **tout en haut** (avant le bloc `cmd`), tenir `arrete_depuis` — c'est l'état au début de l'image, avant que les réflexes ne rendent la main :

```rust
    // ── Depuis quand est-il à l'arrêt ? (spec « ne pas se superposer » §3)
    //
    // Tout en haut : les réflexes rendent la main plus bas, et un
    // personnage qui tombe doit perdre sa place dès cette image.
    let au_sol = matches!(ch.attachment, Attachment::On { face: Face::Top, .. });
    if au_sol && place::a_l_arret(ch) {
        // `get_or_insert` : garde l'instant d'arrivée s'il y en a déjà un.
        ch.arrete_depuis.get_or_insert(maintenant);
    } else {
        ch.arrete_depuis = None;
    }
```

Puis, **juste avant** `let servait_au_mur = tenue::sert_une_tenue_au_mur(ch);` (couche 2) :

```rust
    // ── Céder sa place (spec « ne pas se superposer » §3) ───────────────
    //
    // À l'arrêt au sol, sur la place d'un plus ancien : un pas vers la
    // place libre la plus proche, et l'intention attend. Celui qui attend
    // dans la file d'un mur est exclu — `grimper` choisit sa place lui-même.
    // Pas de place libre (écran plein) : il reste, chevauchement accepté.
    if let Attachment::On { platform, face: Face::Top, offset } = ch.attachment {
        if ch.arrete_depuis.is_some() && place::attend_le_mur(ch).is_none() {
            let (demi, _) = place::corps(ch, e.echelle_affichage);
            if place::doit_ceder(voisins, platform, offset, demi, ch.arrete_depuis) {
                let longueur = world.get(platform).map(|p| p.rect.face_length(Face::Top)).unwrap_or(0.0);
                if let Some(cible) = place::place_libre(voisins, platform, offset, demi, longueur) {
                    // Il bouge : il redevient le dernier arrivé, et
                    // reprendra une place neuve en s'arrêtant.
                    ch.arrete_depuis = None;
                    intention::marcher_vers(ch, cible, reglages, maintenant, dt);
                    return r;
                }
            }
        }
    }
```

- [ ] **Step 5 : lancer, voir passer**

Run : `cargo test --quiet 2>&1 | Select-String 'test result|panicked|^error' -Context 0,4`
Expected : tous verts. Si `tout_le_monde_s_assoit_en_rangee` échoue, lire l'intention et l'offset du personnage fautif avant de toucher au code : allonger la phase de rangement est acceptable, assouplir l'assertion de chevauchement ne l'est pas.

- [ ] **Step 6 : commit**

```powershell
git add src-tauri/src/behavior
git commit -m "feat(place): a l'arret sur la place d'un plus ancien, il se decale"
```

---

### Task 3 : la file au pied du mur

**Files :**
- Modify : `src-tauri/src/behavior/intention.rs` (`poursuivre` → `poursuivre_parmi`, `grimper`, phase `Rejoindre`)
- Modify : `src-tauri/src/behavior/mod.rs` (l'appel à `poursuivre` dans `pas_parmi`)
- Test : `src-tauri/src/behavior/tests.rs`

**Interfaces :**
- Consumes : `place::{Voisinage, corps, bas_du_mur_libre, premier_de_la_file, place_libre}`, `intention::marcher_vers`, `PhaseGrimpe::Rejoindre { attend_depuis }`.
- Produces : `pub fn poursuivre_parmi(ch, world, e, reglages, maintenant, dt, rng, voisins: &place::Voisinage) -> Issue` ; `poursuivre(...)` inchangé en signature = `poursuivre_parmi(..., &place::Voisinage::seul())`. Constante `ATTENTE_MAX_FILE: Duration = 120 s`.

- [ ] **Step 1 : les tests**

À la fin de `tests.rs` :

```rust
/// Le bas du mur gauche du monde de test : longueur de sa face.
fn longueur_mur_gauche(m: &World) -> f32 {
    let mur = mur_gauche(m);
    mur.rect.face_length(Face::Right)
}

#[test]
fn tout_le_monde_grimpe_un_par_un() {
    use crate::menu_perso::Commande;
    let m = monde();
    let mur = mur_gauche(&m).id;
    let longueur = longueur_mur_gauche(&m);
    let (_, hauteur) = place::corps(&perso(&m), 1.0);
    let mut persos = foule_au_meme_endroit(&m, 5, 300.0);
    let mut rng = XorShift32::seeded(89);
    let t0 = Duration::from_secs(1);
    commander_foule(&mut persos, &m, Commande::Imposer(tenue::Tenue::Grimper), t0, &mut rng);

    let mut monte = [false; 5];
    let mut file_vue = 0usize;
    jouer_foule(&mut persos, &m, &entrees(true, 1.0), &mut rng, t0, 60 * 180, |p| {
        // Jamais deux dans la zone de départ du mur.
        let au_depart = p
            .iter()
            .filter(|c| matches!(c.attachment, Attachment::On { platform, offset, .. } if platform == mur && offset > longueur - hauteur))
            .count();
        assert!(au_depart <= 1, "{au_depart} au départ du mur en même temps");
        file_vue = file_vue.max(p.iter().filter(|c| place::attend_le_mur(c).is_some()).count());
        for (i, c) in p.iter().enumerate() {
            if matches!(c.attachment, Attachment::On { face, .. } if face != Face::Top) {
                monte[i] = true;
            }
        }
    });
    assert!(monte.iter().all(|&m| m), "tous ne sont pas montés : {monte:?}");
    assert!(file_vue >= 2, "aucune file ne s'est formée ({file_vue})");
}

#[test]
fn une_file_bloquee_abandonne_apres_deux_minutes() {
    // Un personnage figé au bas du mur (« Rester accroché ») bloque la
    // zone de départ. Le second, qui veut grimper SANS tenue, attend… puis
    // renonce après 120 s d'attente — pas avant.
    let m = monde();
    let longueur = longueur_mur_gauche(&m);
    let mut bloqueur = perso_au_mur(&m, tenue::Tenue::ResterAccroche);
    if let Attachment::On { platform, face, .. } = bloqueur.attachment {
        bloqueur.attachment = Attachment::On { platform, face, offset: longueur };
    }
    let mut grimpeur = perso(&m);
    let t0 = Duration::from_secs(1);
    grimpeur.intention = Some(intention::ActiveIntention::grimper_sur_ordre(t0));
    let mut persos = vec![bloqueur, grimpeur];
    let mut rng = XorShift32::seeded(97);

    let t = jouer_foule(&mut persos, &m, &entrees(true, 1.0), &mut rng, t0, 60 * 100, |_| {});
    assert!(place::attend_le_mur(&persos[1]).is_some(), "il devrait encore attendre à 100 s");
    // Entre 100 et 140 s, il RENONCE à un moment. On ne teste pas qu'il
    // n'attend plus à 140 s : après avoir renoncé, le tirage peut lui
    // redonner l'envie de grimper, et le remettre dans la file.
    let mut a_renonce = false;
    jouer_foule(&mut persos, &m, &entrees(true, 1.0), &mut rng, t, 60 * 40, |p| {
        a_renonce |= place::attend_le_mur(&p[1]).is_none();
    });
    assert!(a_renonce, "il n'a jamais renoncé en 140 s");
}

#[test]
fn la_file_ne_joue_que_sur_le_sol_du_mur() {
    // Monde à trois écrans : depuis l'écran du milieu, il marche vers le
    // mur d'un voisin. En route, il ne fait la file nulle part, même si ce
    // mur est pris.
    let m = World::from_screens(&crate::probe::fake::FakeProbe::trois_ecrans().screens());
    let mut ch = perso(&m);
    let milieu = m.platforms().iter().filter(|p| p.has_face(Face::Top)).nth(1).unwrap();
    ch.attachment = Attachment::On { platform: milieu.id, face: Face::Top, offset: 900.0 };
    let t0 = Duration::from_secs(1);
    ch.intention = Some(intention::ActiveIntention::grimper_sur_ordre(t0));
    // Tous les murs « pris » : un occupant fictif dans la zone de départ de
    // chacun.
    let autres: Vec<place::Occupant> = m
        .platforms()
        .iter()
        .filter(|p| p.has_face(Face::Left) || p.has_face(Face::Right))
        .enumerate()
        .map(|(i, p)| {
            let face = p.faces[0];
            place::Occupant { acteur: 100 + i as u32, platform: p.id, face, offset: p.rect.face_length(face), demi_largeur: 45.0, arrete_depuis: None, attend_le_mur: None }
        })
        .collect();
    let v = place::Voisinage { moi: 0, autres: &autres };
    let (table, reglages) = (desire::TableEnvies::defaut(), reglages_defaut());
    let mut rng = XorShift32::seeded(101);
    let mut t = t0;
    // 3 s : il est encore sur le sol du milieu, loin de tout mur.
    for _ in 0..180 {
        t += Duration::from_secs_f32(DT);
        pas_parmi(&mut ch, &m, &entrees(true, 1.0), &table, &reglages, t, DT, &mut rng, &v);
        if matches!(ch.attachment, Attachment::On { platform, .. } if platform == milieu.id) {
            assert!(place::attend_le_mur(&ch).is_none(), "il fait la file loin du mur");
        }
    }
}
```

`FakeProbe::trois_ecrans` : vérifier le nom exact dans `probe/fake.rs` (le test `depuis_l_ecran_du_milieu_il_rejoint_un_mur_voisin` de `tests.rs` en utilise un — reprendre le même constructeur).

- [ ] **Step 2 : lancer, voir échouer**

Run : `cargo test --quiet grimpe_un_par_un 2>&1 | Select-String 'panicked|test result' -Context 0,3`
Expected : FAIL — « 2 au départ du mur en même temps » (ou plus).

- [ ] **Step 3 : `poursuivre_parmi`**

Renommer `poursuivre` en `poursuivre_parmi`, ajouter le paramètre final `voisins: &super::place::Voisinage`, et le passer à `grimper` : `grimper(ch, world, e, reglages, &mut ai, maintenant, dt, rng, voisins)`. Recréer :

```rust
/// `poursuivre_parmi` pour un personnage seul au monde (simulation, tests).
pub fn poursuivre(
    ch: &mut Character,
    world: &World,
    e: &super::Entrees,
    reglages: &Reglages,
    maintenant: Duration,
    dt: f32,
    rng: &mut dyn Rng,
) -> Issue {
    poursuivre_parmi(ch, world, e, reglages, maintenant, dt, rng, &super::place::Voisinage::seul())
}
```

`grimper` reçoit deux paramètres de plus : `e: &super::Entrees` (après `world`) et `voisins: &super::place::Voisinage` (en dernier). Dans `behavior::pas_parmi`, l'appel de la couche 2 devient `intention::poursuivre_parmi(ch, world, e, reglages, maintenant, dt, rng, voisins)`.

- [ ] **Step 4 : la file, dans la phase `Rejoindre`**

En tête de fichier : `const ATTENTE_MAX_FILE: Duration = Duration::from_secs(120);` avec son commentaire (spec §4 : l'attente ne compte pas dans le délai d'abandon, mais une file bloquée pour de bon renonce).

Le bras devient `PhaseGrimpe::Rejoindre { mur, presse, attend_depuis } => {`. Juste **après** le calcul de `pos` (`let Some(pos) = position_actuelle(...)`), et **avant** `if let Some(f) = Facing::face_a_la_paroi(face_mur)`, insérer :

```rust
            // ── La file au pied du mur (spec « ne pas se superposer » §4) ─
            //
            // Seulement sur le sol de CE mur : en route depuis un autre
            // écran, il marche sans regarder la file.
            if let (Some((sol, pied)), Attachment::On { platform, face: Face::Top, offset }) =
                (sol_au_pied_du_mur(world, mur), ch.attachment)
            {
                if platform == sol {
                    let (demi, hauteur) = super::place::corps(ch, e.echelle_affichage);
                    let longueur_mur = plat_mur.rect.face_length(face_mur);
                    let ecart = (offset - pied).abs();
                    let a_mon_tour = super::place::bas_du_mur_libre(voisins, mur, longueur_mur, hauteur)
                        && super::place::premier_de_la_file(voisins, mur, pied, ecart);

                    if !a_mon_tour {
                        // Sa place dans la file : la place libre la plus
                        // proche du pied du mur. Visée à chaque image, c'est
                        // ce qui fait AVANCER la file quand le premier part.
                        let longueur_sol = world.get(sol).map(|p| p.rect.face_length(Face::Top)).unwrap_or(0.0);
                        let place = super::place::place_libre(voisins, sol, pied, demi, longueur_sol).unwrap_or(offset);
                        let arrive = marcher_vers(ch, place, reglages, maintenant, dt);
                        let attend_depuis = if arrive {
                            // Arrivé à sa place : il attend, face au mur.
                            if let Some(f) = Facing::face_a_la_paroi(face_mur) {
                                ch.facing = f;
                            }
                            ch.set_pose(POSE_STAND, maintenant);
                            let depuis = attend_depuis.unwrap_or(maintenant);
                            if maintenant.saturating_sub(depuis) > ATTENTE_MAX_FILE {
                                // File bloquée pour de bon : il renonce
                                // (décision n° 4, la navigation peut échouer).
                                ch.intention = None;
                                return Issue::Echouee;
                            }
                            // L'attente ne compte pas dans le délai
                            // d'abandon : on recule son début d'autant.
                            ai.depuis += Duration::from_secs_f32(dt);
                            Some(depuis)
                        } else {
                            None
                        };
                        ai.etat = EtatIntention::Grimpe {
                            phase: PhaseGrimpe::Rejoindre { mur, presse, attend_depuis },
                            jusqu_a,
                        };
                        return Issue::EnCours;
                    }
                }
            }
```

Dans le reste du bras (la marche normale vers le mur), la phase réécrite garde `attend_depuis: None` ; vérifier qu'aucune autre construction de `Rejoindre` ne manque le champ (`cargo build` le dira).

- [ ] **Step 5 : lancer, voir passer**

Run : `cargo test --quiet 2>&1 | Select-String 'test result|panicked|^error' -Context 0,4`
Expected : tous verts. Si `tout_le_monde_grimpe_un_par_un` n'a pas vu tout le monde monter en 180 s, relever le nombre de personnages en file à la fin avant de toucher au code : un grimpeur tenu qui redescend dans la zone de départ bloque légitimement la file (spec §4) — changer de graine est acceptable, assouplir « jamais deux au départ » ne l'est pas.

- [ ] **Step 6 : commit**

```powershell
git add src-tauri/src/behavior
git commit -m "feat(place): au pied du mur, ils font la file et montent un par un"
```

---

### Task 4 : « Tout le monde › Rester accroché »

**Files :**
- Modify : `src-tauri/src/behavior/tenue.rs` (`peut_tenir`, `intention_pour`)
- Modify : `src-tauri/src/behavior/mod.rs` (appels à `intention_pour`, `en_l_air`)
- Modify : `src-tauri/src/menu_perso.rs` (table `TOUS`)
- Modify : `src-tauri/src/behavior/tenue_tests.rs`, `src-tauri/src/behavior/tests.rs` (matrice + scénario)

**Interfaces :**
- Produces : `intention_pour(t, e, reglages, table, manifeste, sur_paroi: bool, maintenant) -> ActiveIntention` (paramètre `sur_paroi` inséré avant `maintenant`) ; identifiant de menu `tous.rester`.

- [ ] **Step 1 : les tests**

Dans `tenue_tests.rs`, remplacer la fin de `au_mur_l_absence_ne_change_rien` et ajouter :

```rust
#[test]
fn au_mur_l_absence_ne_change_rien() {
    // Décision de l'auteur : au mur, rien n'interrompt une action tenue.
    let (r, t, m) = (reglages(), TableEnvies::defaut(), blob());
    let e = entrees(false, 8.0);
    let ai = intention_pour(Tenue::Grimper, &e, &r, &t, &m, true, Duration::from_secs(1));
    assert_eq!(ai.kind, Intention::Grimper);
    let ai = intention_pour(Tenue::ResterAccroche, &e, &r, &t, &m, true, Duration::from_secs(1));
    assert!(matches!(ai.etat, EtatIntention::Grimpe { phase: PhaseGrimpe::Accroche, .. }));
}

#[test]
fn rester_accroche_au_sol_part_d_abord_au_mur() {
    let (r, t, m) = (reglages(), TableEnvies::defaut(), blob());
    let ai = intention_pour(Tenue::ResterAccroche, &entrees(true, 1.0), &r, &t, &m, false, Duration::from_secs(1));
    assert!(matches!(ai.etat, EtatIntention::Grimpe { phase: PhaseGrimpe::Choisir { presse: true }, .. }));
}
```

(et ajouter `false` avant `Duration::from_secs(1)` dans les deux autres appels du fichier.)

Dans `tests.rs`, dans `peut_tenir_suit_l_endroit_et_l_attache`, la ligne « Au sol » `assert!(!peut_tenir(Tenue::ResterAccroche, &ch, &table));` devient `assert!(peut_tenir(Tenue::ResterAccroche, &ch, &table));` avec le commentaire « depuis le 2026-09-24 : au sol, il part au mur puis s'y fige ». Ajouter le scénario :

```rust
#[test]
fn tout_le_monde_reste_accroche_depuis_le_sol() {
    use crate::menu_perso::Commande;
    let m = monde();
    let mut persos = foule_au_meme_endroit(&m, 3, 400.0);
    let mut rng = XorShift32::seeded(103);
    let t0 = Duration::from_secs(1);
    commander_foule(&mut persos, &m, Commande::Imposer(tenue::Tenue::ResterAccroche), t0, &mut rng);
    // 150 s pour monter tous, un par un.
    let t = jouer_foule(&mut persos, &m, &entrees(true, 1.0), &mut rng, t0, 60 * 150, |_| {});
    let figes: Vec<_> = persos.iter().map(|c| c.attachment).collect();
    for c in &persos {
        assert!(matches!(c.attachment, Attachment::On { face, .. } if face != Face::Top), "{:?}", c.attachment);
        assert_eq!(c.tenue, Some(tenue::Tenue::ResterAccroche));
    }
    // Puis 10 s : plus personne ne bouge.
    jouer_foule(&mut persos, &m, &entrees(true, 1.0), &mut rng, t, 600, |p| {
        let maintenant: Vec<_> = p.iter().map(|c| c.attachment).collect();
        assert_eq!(maintenant, figes);
    });
}
```

Dans la matrice (`verifier_effet`), remplacer le bras `Commande::Tenir(Tenue::ResterAccroche) => { … }` des « effets immédiats » par un traitement qui couvre aussi `Imposer` et le départ du sol :

```rust
        Commande::Tenir(Tenue::ResterAccroche) | Commande::Imposer(Tenue::ResterAccroche) => {
            // Au sol, il part d'abord au mur (jusqu'à 90 s) ; puis il ne
            // bouge plus pendant 10 s.
            let accroche = |ch: &Character| {
                matches!(ch.attachment, Attachment::On { face, .. } if face != Face::Top)
                    && matches!(ch.intention, Some(intention::ActiveIntention { etat: intention::EtatIntention::Grimpe { phase: intention::PhaseGrimpe::Accroche, .. }, .. }))
            };
            let mut t = t0;
            let mut n = 0;
            while !accroche(&ch) && n < 5400 {
                t = jouer_images(&mut ch, m, &entrees(true, 1.0), rng, t, 1, |_| {});
                n += 1;
            }
            if !accroche(&ch) {
                return Some(format!("jamais accroché en 90 s : {:?}", ch.attachment));
            }
            let depart = ch.attachment;
            let mut bouge = false;
            jouer_images(&mut ch, m, &entrees(true, 1.0), rng, t, 600, |ch| {
                bouge |= ch.attachment != depart;
            });
            return (bouge || ch.tenue != Some(Tenue::ResterAccroche))
                .then(|| format!("n'est pas resté accroché : {:?}", ch.attachment));
        }
```

- [ ] **Step 2 : lancer, voir échouer**

Run : `cargo test --quiet 2>&1 | Select-String '^error' | Select-Object -First 5`
Expected : erreurs de compilation (`intention_pour` n'a pas encore `sur_paroi`).

- [ ] **Step 3 : `tenue.rs`**

`peut_tenir` : `Tenue::Grimper | Tenue::ResterAccroche => true,` (et retirer le bras `Tenue::ResterAccroche => sur_une_paroi`), commentaire mis à jour : « `ResterAccroche` vaut partout depuis le 2026-09-24 : au sol, il part au mur, puis s'y fige ».

`intention_pour` reçoit `sur_paroi: bool` avant `maintenant`, et :

```rust
        // Déjà sur une paroi : il se fige là. Au sol : il part au mur (file
        // comprise), monte, et s'y figera — la pause `Accroche` d'un
        // `ResterAccroche` ne finit jamais (`intention::grimper`).
        Tenue::ResterAccroche if sur_paroi => ActiveIntention::accroche(maintenant),
        Tenue::ResterAccroche => ActiveIntention::grimper_sur_ordre(maintenant),
```

- [ ] **Step 4 : les appelants, `en_l_air`, la table `TOUS`**

Dans `behavior/mod.rs`, les deux appels `tenue::intention_pour(t, e, reglages, table, &ch.manifest, maintenant)` deviennent `tenue::intention_pour(t, e, reglages, table, &ch.manifest, sur_une_paroi(ch), maintenant)`, avec :

```rust
/// Accroché à un mur ou au plafond ?
fn sur_une_paroi(ch: &crate::character::Character) -> bool {
    matches!(ch.attachment, Attachment::On { face, .. } if face != Face::Top)
}
```

Dans `en_l_air`, retirer la garde `t != tenue::Tenue::ResterAccroche &&` (il atterrira, puis partira au mur). Dans `tests.rs`, `perso_au_mur` passe `true` à `intention_pour`.

Dans `menu_perso.rs`, table `TOUS`, après `tous.grimper` :

```rust
    // « Tout le monde › Rester accroché » (demande de l'auteur, 2026-09-24) :
    // ceux qui sont au sol montent puis se figent, les autres se figent là.
    ("tous.rester", "Rester accroché", Commande::Basculer(Tenue::ResterAccroche)),
```

- [ ] **Step 5 : lancer, voir passer**

Run : `cargo test --quiet 2>&1 | Select-String 'test result|panicked|^error' -Context 0,4`
Expected : tous verts, matrice comprise (elle couvre `tous.rester` dans les quinze états).

- [ ] **Step 6 : commit**

```powershell
git add src-tauri/src
git commit -m "feat(menu): Tout le monde > Rester accroche - au sol, il monte puis se fige"
```

---

### Task 5 : la boucle, la mesure, la doc

**Files :**
- Modify : `src-tauri/src/main.rs` (boucle 60 Hz, `for i in 0..acteurs.len()` ligne ~2074)
- Modify : `CLAUDE.md`

**Interfaces :**
- Consumes : `behavior::pas_parmi`, `place::{Occupant, Voisinage, occupant_de}`.

- [ ] **Step 1 : les occupants de l'image précédente**

Avant la boucle principale (à côté de `let mut ordre_pour_tous`), déclarer :

```rust
    // Les occupants de l'image précédente (spec « ne pas se superposer »).
    // Une image de retard, invisible à 60 Hz, et c'est ce qui évite de les
    // recalculer avant la boucle des acteurs : chacun y ajoute le sien après
    // son pas.
    let mut occupants: Vec<behavior::place::Occupant> = Vec::new();
```

Juste avant `for i in 0..acteurs.len() {` :

```rust
        // `take` : on prend la liste de l'image précédente, et `occupants`
        // repart vide pour être remplie pendant cette image-ci.
        let occupants_precedents = std::mem::take(&mut occupants);
```

L'appel `behavior::pas(…)` du bloc `if !fige { … }` devient :

```rust
                behavior::pas_parmi(
                    &mut acteur.ch,
                    &monde,
                    &entrees,
                    &table,
                    &reglages,
                    maintenant,
                    dt,
                    &mut rng,
                    &behavior::place::Voisinage { moi: acteur.id, autres: &occupants_precedents },
                );
```

et **juste après** le bloc `if !fige { … }` (hors du `if` : un acteur figé par son menu garde sa place — Review Focus n° 1) :

```rust
            // Sa place, pour les autres, à l'image suivante. Après le
            // `continue` des départs : un acteur qui s'en va n'occupe rien
            // (Review Focus n° 2).
            if let Some(o) = behavior::place::occupant_de(acteur.id, &acteur.ch, echelle_affichage) {
                occupants.push(o);
            }
```

- [ ] **Step 2 : compiler, tester, lancer**

Run : `cargo test --quiet 2>&1 | Select-String 'test result|^error' -Context 0,4` puis `cargo build --quiet 2>&1 | Select-Object -Last 20`.
Expected : tous verts, build sans erreur.

- [ ] **Step 3 : mesurer le travail par image**

Sans toucher à l'instance release. Avant/après sur le même roster :

```powershell
cd C:\Users\alri\Documents\shimeji-desktop\src-tauri
$env:SHIMEJI_CADENCE='1'; $env:SHIMEJI_QUITTER_APRES='60'; $env:SHIMEJI_PERSONNAGES='blob,blob,blob,blob,blob,blob,blob,blob,blob,blob,blob,blob,blob,blob,blob'
cargo run 2>&1 | Select-Object -Last 15   # lire la ligne « travail par image » de SHIMEJI_CADENCE
```

Faire la même mesure sur le commit d'avant la Tâche 1 (`git stash` n'est pas nécessaire : `git worktree add ../mesure-avant <commit>`). Expected : le travail par image ne bouge pas de façon mesurable (quelques µs au plus).

- [ ] **Step 4 : CLAUDE.md**

- Dans « Toute nouvelle action se branche au menu du clic droit », le tableau « Le menu propose » : ajouter « Rester accroché ✓ » à la section « Tout le monde » (texte après le tableau).
- Ajouter, après le paragraphe « Toute action du clic droit se fait… », un paragraphe court :

> **Ils se traversent, mais ne s'arrêtent jamais l'un sur l'autre** (2026-09-24,
> `behavior/place.rs`). Au sol, un personnage à l'arrêt sur la place d'un plus
> ancien se décale côte à côte (`pas_parmi`) ; au pied d'un mur, ils font la file
> et s'accrochent un par un (`grimper`, phase `Rejoindre`). Sur les murs et au
> plafond, personne n'occupe rien. `pas` et `poursuivre` restent pour la
> simulation et les tests (« seul au monde ») ; seule la boucle appelle
> `pas_parmi`. → `docs/specs/2026-09-24-ne-pas-se-superposer-design.md`

- Mettre à jour le compte de tests (**N tests.**) avec le nombre réel.
- Ajouter la spec et ce plan au tableau des documents.

- [ ] **Step 5 : commit**

```powershell
git add src-tauri/src/main.rs CLAUDE.md
git commit -m "feat(place): la boucle donne a chacun les places des autres"
```
