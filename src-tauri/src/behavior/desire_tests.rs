//! Les tests de `desire` — sortis du fichier source le 2026-09-14.
//!
//! **Pourquoi ce fichier existe** : `behavior/desire.rs` faisait 467 lignes dont 270 de tests,
//! soit 58 %. Toute lecture du module en payait le double, pour rien la plupart
//! du temps. Les tests n'ont pas changé d'une ligne : ils ont seulement déménagé, et
//! se désindentent d'un cran puisqu'ils ne sont plus enfermés dans un `mod tests { }`.

use super::*;
use crate::character::manifest::Manifest;
use crate::rng::XorShift32;

/// Un manifeste où l'on choisit les poses présentes, pour éprouver la
/// couverture partielle sans toucher au disque.
fn manifeste_avec(poses: &[&str]) -> Manifest {
    let corps: Vec<String> = poses
        .iter()
        .map(|p| format!(r#""{p}": {{ "frames": [1] }}"#))
        .collect();
    let json = format!(
        r#"{{ "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
              "hitbox": [40,20,48,100], "poses": {{ {} }} }}"#,
        corps.join(",")
    );
    serde_json::from_str(&json).expect("manifeste de test valide")
}

fn compter(table: &TableEnvies, m: &Manifest, graine: u32, n: usize) -> (usize, usize, usize) {
    let mut rng = XorShift32::seeded(graine);
    let (mut flaner, mut reposer, mut rien) = (0, 0, 0);
    for _ in 0..n {
        match table.tirer(m, &mut rng) {
            Some(Intention::Flaner) => flaner += 1,
            Some(Intention::SeReposer) => reposer += 1,
            // `compter` ne sert qu'aux tests d'étape 1a, dont les
            // manifestes (via `manifeste_avec`) ne déclarent jamais
            // `spinHead` ni `sitDangle` : le poids des deux lignes
            // `Jouer` est donc TOUJOURS nul, et cette branche ne peut
            // pas être atteinte. `unreachable!` plutôt qu'un compteur
            // muet : si un jour un manifeste de test gagnait ces poses
            // par erreur, on veut un panic bruyant, pas un total qui ne
            // correspond plus à `n`.
            Some(Intention::Jouer(_)) => unreachable!("aucune pose de jeu dans ce manifeste"),
            // Même raison, pour la ligne `Grimper` de l'étape 4a : ces
            // manifestes n'ont ni `grabWall` ni `climbWall`.
            Some(Intention::Grimper) => unreachable!("aucune pose d'escalade dans ce manifeste"),
            None => rien += 1,
        }
    }
    (flaner, reposer, rien)
}

#[test]
fn la_table_par_defaut_privilegie_la_flanerie() {
    // Poids de la spec §7.2 : Flâner 5, Se reposer 1. Donc ~83 / 17 %.
    let m = manifeste_avec(&["stand", "walk", "sit"]);
    let (flaner, reposer, rien) = compter(&TableEnvies::defaut(), &m, 42, 10_000);

    assert_eq!(rien, 0);
    let pct = |n: usize| n as f32 / 100.0;
    assert!((pct(flaner) - 83.3).abs() < 2.0, "flâner {flaner}");
    assert!((pct(reposer) - 16.7).abs() < 2.0, "reposer {reposer}");
}

#[test]
fn une_pose_manquante_retire_l_intention_du_tirage() {
    // **LE test de la couverture partielle appliquée au comportement**
    // (spec §8.6). Pas de `sit` → il ne se repose JAMAIS, et il n'y a
    // aucun cas particulier dans le code pour ça.
    let m = manifeste_avec(&["stand", "walk"]);
    let (flaner, reposer, rien) = compter(&TableEnvies::defaut(), &m, 42, 1_000);

    assert_eq!(reposer, 0, "il ne devrait jamais se reposer sans pose sit");
    assert_eq!(flaner, 1_000);
    assert_eq!(rien, 0);
}

#[test]
fn sans_aucune_pose_jouable_le_tirage_rend_none() {
    // Un personnage qui n'a que `stand` ne peut ni flâner (pas de walk)
    // ni se reposer (pas de sit). `None` = « rien à faire », et
    // l'appelant le traite comme tel — ce n'est pas une erreur.
    let m = manifeste_avec(&["stand"]);
    let (_, _, rien) = compter(&TableEnvies::defaut(), &m, 42, 100);
    assert_eq!(rien, 100);
}

#[test]
fn un_multiplicateur_biaise_sans_commander() {
    // **LE test de la décision n° 3.** On multiplie par 8 l'envie de se
    // reposer, comme le fera « inactif depuis 2 min » à l'étape 2.
    //
    // Poids : Flâner 5, Se reposer 1 × 8 = 8. Donc ~38 / 62 %.
    //
    // Le point n'est pas qu'il se repose : c'est qu'il flâne ENCORE
    // parfois. « Cette marge est le produit. » Un déclenchement
    // donnerait 0 / 100 et un personnage prévisible.
    let m = manifeste_avec(&["stand", "walk", "sit"]);
    let table = TableEnvies::defaut();
    let mut rng = XorShift32::seeded(7);

    let (mut flaner, mut reposer) = (0, 0);
    for _ in 0..10_000 {
        let mult = |i: Intention| match i {
            Intention::SeReposer => 8.0,
            _ => 1.0,
        };
        match table.tirer_avec(&m, &mut rng, mult) {
            Some(Intention::Flaner) => flaner += 1,
            Some(Intention::SeReposer) => reposer += 1,
            // Même raison que dans `compter` : ce manifeste n'a ni
            // `spinHead` ni `sitDangle`, le poids des lignes `Jouer` est
            // nul, la branche est inatteignable.
            Some(Intention::Jouer(_)) => unreachable!("aucune pose de jeu dans ce manifeste"),
            // Même raison, pour la ligne `Grimper` de l'étape 4a : ces
            // manifestes n'ont ni `grabWall` ni `climbWall`.
            Some(Intention::Grimper) => unreachable!("aucune pose d'escalade dans ce manifeste"),
            None => {}
        }
    }

    let pct = |n: usize| n as f32 / 100.0;
    assert!((pct(reposer) - 61.5).abs() < 2.0, "reposer {reposer}");
    // La marge : il flâne encore dans plus d'un tiers des cas.
    assert!(flaner > 3_000, "la marge a disparu : flâner {flaner}");
}

#[test]
fn un_multiplicateur_nul_retire_l_option_completement() {
    // C'est ainsi qu'un signal pourra interdire une intention sans qu'on
    // ajoute un chemin de code (étape 2 : pas de sieste juste après
    // s'être réveillé, par exemple).
    let m = manifeste_avec(&["stand", "walk", "sit"]);
    let table = TableEnvies::defaut();
    let mut rng = XorShift32::seeded(7);

    for _ in 0..500 {
        let mult = |i: Intention| match i {
            Intention::SeReposer => 0.0,
            _ => 1.0,
        };
        assert_eq!(
            table.tirer_avec(&m, &mut rng, mult),
            Some(Intention::Flaner)
        );
    }
}

#[test]
fn la_table_suit_les_poids_de_la_config() {
    // Le test qui prouve qu'on règle le caractère sans recompiler.
    let m = manifeste_avec(&["stand", "walk", "sit"]);

    let c = crate::config::Config {
        envies: crate::config::Envies {
            flaner: 1.0,
            se_reposer: 9.0,
            // `..Default::default()` : la Tâche 3 a ajouté `jouer` à
            // cette structure, et un littéral exhaustif ferait alors
            // échouer la compilation de CE test — pour rien (même
            // raison que le commentaire sur `ModifsAppli` dans
            // `signals.rs`).
            ..crate::config::Envies::default()
        },
        ..crate::config::Config::default()
    };

    let table = TableEnvies::depuis_config(&c);
    let mut rng = XorShift32::seeded(42);

    let (mut flaner, mut reposer) = (0, 0);
    for _ in 0..10_000 {
        match table.tirer(&m, &mut rng) {
            Some(Intention::Flaner) => flaner += 1,
            Some(Intention::SeReposer) => reposer += 1,
            // Même raison que dans `compter` : ce manifeste n'a ni
            // `spinHead` ni `sitDangle`, le poids des lignes `Jouer` est
            // nul, la branche est inatteignable.
            Some(Intention::Jouer(_)) => unreachable!("aucune pose de jeu dans ce manifeste"),
            // Même raison, pour la ligne `Grimper` de l'étape 4a : ces
            // manifestes n'ont ni `grabWall` ni `climbWall`.
            Some(Intention::Grimper) => unreachable!("aucune pose d'escalade dans ce manifeste"),
            None => {}
        }
    }

    // Rapport inversé par rapport au défaut : il se repose maintenant
    // neuf fois sur dix.
    assert!(reposer > flaner * 5, "reposer {reposer}, flâner {flaner}");
}

#[test]
fn la_table_par_defaut_est_celle_de_la_config_par_defaut() {
    // Deux chemins vers les mêmes poids : ils ne doivent pas diverger par
    // inadvertance. `defaut()` doit être exactement
    // `depuis_config(&Config::default())`.
    let a = TableEnvies::defaut();
    let b = TableEnvies::depuis_config(&crate::config::Config::default());

    assert_eq!(a.entrees.len(), b.entrees.len());
    for (ea, eb) in a.entrees.iter().zip(b.entrees.iter()) {
        assert_eq!(ea.intention, eb.intention);
        assert_eq!(ea.base, eb.base);
    }
}

#[test]
fn le_tirage_est_reproductible_a_graine_fixe() {
    // Ce qui rend la trace du mode simulation (Tâche 9) comparable d'une
    // exécution à l'autre.
    let m = manifeste_avec(&["stand", "walk", "sit"]);
    let t = TableEnvies::defaut();
    assert_eq!(compter(&t, &m, 999, 500), compter(&t, &m, 999, 500));
}

#[test]
fn chaque_jeu_est_retire_separement_du_tirage() {
    // **LE test des deux lignes.** Un pack qui n'a que `spinHead` doit
    // jouer quand même — avec cette animation seulement. Une intention
    // `Jouer` unique exigeant les deux poses ne jouerait pas du tout.
    use crate::behavior::intention::Jeu;

    let m = manifeste_avec(&["stand", "walk", "sit", "spinHead"]);
    let table = TableEnvies::defaut();
    let mut rng = XorShift32::seeded(3);

    let mut vus = std::collections::BTreeSet::new();
    for _ in 0..2_000 {
        if let Some(i) = table.tirer(&m, &mut rng) {
            vus.insert(i);
        }
    }

    assert!(
        vus.contains(&Intention::Jouer(Jeu::TeteQuiTourne)),
        "il a spinHead : il doit pouvoir se tourner la tête"
    );
    assert!(
        !vus.contains(&Intention::Jouer(Jeu::JambesQuiBalancent)),
        "il n'a pas sitDangle : cette animation doit être retirée"
    );
}

#[test]
fn le_biais_de_jeu_porte_sur_les_deux_animations() {
    // Le modificateur par application dit « jouer ×3 » sans distinguer
    // les animations : les deux lignes doivent donc en profiter.
    //
    // Pas d'import de `Jeu` ici : contrairement à
    // `chaque_jeu_est_retire_separement_du_tirage`, ce test ne nomme
    // jamais une variante précise — il ne regarde que
    // `Intention::Jouer(_)` — et un import inutilisé laissé en place est
    // un avertissement permanent, qui est exactement ce qui masque le
    // prochain avertissement réel.
    let m = manifeste_avec(&["stand", "walk", "sit", "spinHead", "sitDangle"]);
    let table = TableEnvies::defaut();
    let mut rng = XorShift32::seeded(11);

    let (mut jeux, mut autres) = (0, 0);
    for _ in 0..10_000 {
        let mult = |i: Intention| match i {
            Intention::Jouer(_) => 3.0,
            _ => 1.0,
        };
        match table.tirer_avec(&m, &mut rng, mult) {
            Some(Intention::Jouer(_)) => jeux += 1,
            Some(_) => autres += 1,
            None => {}
        }
    }

    // Poids : flâner 5, reposer 1, deux jeux à 1 × 3 = 6. Donc 6 / 12.
    let part = jeux as f32 / (jeux + autres) as f32;
    assert!((part - 0.5).abs() < 0.03, "part des jeux : {part}");
}
