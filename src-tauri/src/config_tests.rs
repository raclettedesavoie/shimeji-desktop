//! Les tests de `config` — sortis du fichier source le 2026-09-14.
//!
//! **Pourquoi ce fichier existe** : `config.rs` faisait 652 lignes dont 171 de tests,
//! soit 26 %. Toute lecture du module en payait le double, pour rien la plupart
//! du temps. Les tests n'ont pas changé d'une ligne : ils ont seulement déménagé, et
//! se désindentent d'un cran puisqu'ils ne sont plus enfermés dans un `mod tests { }`.

use super::*;

static COMPTEUR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Écrit un `config.json` dans un dossier temporaire et rend son chemin.
fn fichier_de_test(contenu: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!(
        "shimeji-cfg-{}-{}",
        std::process::id(),
        COMPTEUR.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&base).unwrap();
    let f = base.join("config.json");
    std::fs::write(&f, contenu).unwrap();
    f
}

#[test]
fn un_fichier_absent_donne_les_defauts() {
    // **Le test le plus important de cette tâche** (spec §9.3).
    let c = charger_depuis(Path::new("Z:/aucun/chemin/config.json"));
    assert_eq!(c, Config::default());
    assert!(
        !c.personnages.is_empty(),
        "il doit rester un personnage par défaut"
    );
}

#[test]
fn un_fichier_vide_donne_les_defauts() {
    // Un objet JSON vide est valide, et doit donner exactement les mêmes
    // valeurs qu'un fichier absent.
    let f = fichier_de_test("{}");
    assert_eq!(charger_depuis(&f), Config::default());
}

#[test]
fn un_fichier_partiel_ne_change_que_ce_qu_il_declare() {
    // C'est ce qui permet à l'utilisateur de n'écrire qu'une ligne.
    let f = fichier_de_test(r#"{ "vitesse": 0.5 }"#);
    let c = charger_depuis(&f);

    assert_eq!(c.vitesse, 0.5);
    // Tout le reste est intact.
    assert_eq!(c.echelle, Config::default().echelle);
    assert_eq!(c.envies, Config::default().envies);
    assert_eq!(c.allures, Config::default().allures);
}

#[test]
fn un_json_malforme_donne_les_defauts_sans_paniquer() {
    // Une virgule en trop ne doit pas empêcher le personnage de vivre.
    let f = fichier_de_test("{ ceci n'est pas du JSON");
    assert_eq!(charger_depuis(&f), Config::default());
}

#[test]
fn un_fichier_avec_bom_est_lu_normalement() {
    // **Le cas par défaut sur Windows.** Le Bloc-notes et
    // `Set-Content -Encoding utf8` de PowerShell 5.1 écrivent tous deux
    // un BOM, et `serde_json` le refuse avec un message qui ne dit pas
    // ce qui se passe.
    //
    // Trouvé en LANÇANT l'application, pas par relecture : le fichier
    // était valide et pourtant rejeté.
    let avec_bom = format!("{}{}", '\u{feff}', r#"{ "vitesse": 0.3 }"#);
    let f = fichier_de_test(&avec_bom);
    assert_eq!(charger_depuis(&f).vitesse, 0.3);
}

#[test]
fn une_inactivite_negative_ne_fait_pas_paniquer() {
    // **Le test de la vague de correction finale, point 3.**
    // `{"inactiviteSecondes": -1}` est du JSON parfaitement valide :
    // `serde_json` le désérialise sans erreur, mais
    // `Duration::from_secs_f32(-1.0)` PANIQUE. Sans le bornage, ce
    // fichier passerait `un_json_malforme_donne_les_defauts_sans_paniquer`
    // haut la main (il n'est pas malformé) et ferait quand même figer
    // le personnage au premier battement de la boucle à 2 Hz.
    let f = fichier_de_test(r#"{ "signaux": { "inactiviteSecondes": -1 } }"#);
    let c = charger_depuis(&f);

    assert!(
        c.signaux.inactivite_secondes >= 0.0,
        "la valeur négative n'a pas été bornée : {}",
        c.signaux.inactivite_secondes
    );
    // La preuve directe : l'appel qui panique dans `signals.rs` ne
    // panique plus sur la valeur bornée.
    let _ = std::time::Duration::from_secs_f32(c.signaux.inactivite_secondes);
}

#[test]
fn un_champ_inconnu_est_ignore() {
    // Un `config.json` écrit pour une version future, ou une faute de
    // frappe : on prend ce qu'on comprend, on ignore le reste.
    let f = fichier_de_test(r#"{ "vitesse": 2.0, "choseInventee": 42 }"#);
    assert_eq!(charger_depuis(&f).vitesse, 2.0);
}

#[test]
fn les_envies_partielles_gardent_les_autres_poids() {
    // Les défauts sont imbriqués : déclarer un seul poids ne doit pas
    // remettre les autres à zéro — ce qui rendrait le personnage
    // catatonique.
    let f = fichier_de_test(r#"{ "envies": { "seReposer": 4.0 } }"#);
    let c = charger_depuis(&f);

    assert_eq!(c.envies.se_reposer, 4.0);
    assert_eq!(c.envies.flaner, Config::default().envies.flaner);
}

#[test]
fn les_allures_partielles_gardent_les_autres_reglages() {
    let f = fichier_de_test(r#"{ "allures": { "chanceDemiTour": 0.9 } }"#);
    let c = charger_depuis(&f);

    assert_eq!(c.allures.chance_demi_tour, 0.9);
    assert_eq!(c.allures.poids_marche, Allures::default().poids_marche);
    assert_eq!(c.allures.duree_course, Allures::default().duree_course);
}

#[test]
fn les_reglages_appliquent_le_facteur_de_vitesse() {
    let c = Config {
        vitesse: 2.0,
        ..Config::default()
    };
    let r = Reglages::depuis(&c);

    assert_eq!(r.vitesse_marche, crate::character::physics::VITESSE_MARCHE * 2.0);
    assert_eq!(r.vitesse_course, crate::character::physics::VITESSE_COURSE * 2.0);
}

#[test]
fn un_facteur_de_vitesse_absurde_est_borne() {
    // Un 0 figerait le personnage, un 10000 le rendrait invisible. Les
    // valeurs viennent d'un fichier édité à la main : on borne.
    let fige = Config {
        vitesse: 0.0,
        ..Config::default()
    };
    assert!(
        Reglages::depuis(&fige).vitesse_marche > 0.0,
        "il doit pouvoir bouger"
    );

    let fou = Config {
        vitesse: 10_000.0,
        ..Config::default()
    };
    assert!(
        Reglages::depuis(&fou).vitesse_marche < 2000.0,
        "il ne doit pas traverser l'écran en une image"
    );
}

#[test]
fn resoudre_trouve_le_dossier_des_personnages_du_depot() {
    // Le repli de développement : l'exe des tests est dans
    // `target/debug/deps/`, donc on doit remonter jusqu'à la racine.
    let d = dossier_personnages();
    assert!(
        d.join("blob").join("mascot.json").is_file(),
        "characters/blob/mascot.json introuvable depuis {}",
        d.display()
    );
}
