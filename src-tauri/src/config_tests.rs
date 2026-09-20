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

/// La bibliothèque gagne sur le dossier livré.
///
/// C'est le seul ordre défendable : un pack installé par l'utilisateur
/// qui porte le nom d'un pack livré doit gagner, sinon on obtient un
/// « je l'ai installé et il ne se passe rien » indébogable.
///
/// ⚠️ On teste la fonction **paramétrée** et jamais la publique : celle-ci
/// lit `%APPDATA%` par `env::var`, qui est global au PROCESSUS. Le
/// modifier ici le modifierait pour tous les tests tournant en
/// parallèle — le genre d'échec qui n'arrive qu'une fois sur dix et
/// coûte une soirée.
#[test]
fn la_bibliotheque_gagne_sur_le_dossier_livre() {
    let base = std::env::temp_dir().join("shimeji-test-resolution");
    let biblio = base.join("biblio");
    let livre = base.join("livre");

    // `let _ =` : l'erreur « existe déjà » est sans intérêt, un test
    // relancé retrouvant les dossiers du précédent.
    let _ = std::fs::create_dir_all(biblio.join("blob"));
    let _ = std::fs::create_dir_all(livre.join("blob"));
    let _ = std::fs::create_dir_all(livre.join("seulement-livre"));

    assert_eq!(
        personnage_dans(Some(&biblio), &livre, "blob"),
        Some(biblio.join("blob")),
        "la bibliothèque doit gagner"
    );
    assert_eq!(
        personnage_dans(Some(&biblio), &livre, "seulement-livre"),
        Some(livre.join("seulement-livre")),
        "à défaut, le dossier livré"
    );
    assert_eq!(
        personnage_dans(Some(&biblio), &livre, "inexistant"),
        None,
        "introuvable partout → None"
    );
    assert_eq!(
        personnage_dans(None, &livre, "blob"),
        Some(livre.join("blob")),
        "sans bibliothèque, le dossier livré suffit"
    );
}

/// On modifie la SEULE clé `personnages`, en laissant tout le reste intact.
///
/// Une sérialisation depuis `Config` perdrait les clés inconnues et
/// remettrait les valeurs par défaut partout : l'utilisateur verrait son
/// fichier réglé à la main écrasé par un clic dans une autre fenêtre
/// (spec §10).
#[test]
fn ecrire_le_personnage_preserve_le_reste_du_fichier() {
    let chemin = std::env::temp_dir().join("shimeji-test-config-chirurgie.json");
    std::fs::write(
        &chemin,
        r#"{
  "echelle": 1.5,
  "vitesse": 2,
  "personnages": ["blob"],
  "une_cle_que_le_code_ne_connait_pas": { "a": 1 }
}"#,
    )
    .expect("écriture du fichier de test");

    ecrire_personnages(&chemin, &["luffy".to_string()]).expect("l'écriture doit réussir");

    let texte = lire_json(&chemin).expect("relecture");
    let v: serde_json::Value = serde_json::from_str(&texte).expect("JSON valide");

    assert_eq!(v["personnages"][0], "luffy", "la clé visée est changée");
    assert_eq!(v["echelle"], 1.5, "les autres réglages survivent");
    assert_eq!(v["vitesse"], 2);
    assert_eq!(
        v["une_cle_que_le_code_ne_connait_pas"]["a"], 1,
        "les clés inconnues survivent"
    );
    assert!(!texte.starts_with('\u{feff}'), "jamais de BOM");
}

/// Un fichier absent est CRÉÉ, avec la seule clé qu'on sait devoir y mettre.
#[test]
fn ecrire_le_personnage_cree_le_fichier_absent() {
    let chemin = std::env::temp_dir().join("shimeji-test-config-neuve.json");
    let _ = std::fs::remove_file(&chemin);

    ecrire_personnages(&chemin, &["blob".to_string()]).expect("création");

    let texte = lire_json(&chemin).expect("relecture");
    let v: serde_json::Value = serde_json::from_str(&texte).expect("JSON valide");
    assert_eq!(v["personnages"][0], "blob");
}

// ── Le multi-ensemble (étape « plusieurs personnages ») ─────────────────

#[test]
fn ecrire_personnages_ecrit_un_multi_ensemble() {
    let dossier = std::env::temp_dir().join("shimeji-test-multi-ensemble");
    let _ = std::fs::create_dir_all(&dossier);
    let chemin = dossier.join("config.json");
    let _ = std::fs::remove_file(&chemin);

    let noms = vec!["blob".to_string(), "blob".to_string(), "luffy".to_string()];
    ecrire_personnages(&chemin, &noms).expect("écriture");

    let relu = charger_depuis(&chemin);
    // Les doublons SURVIVENT : c'est tout l'objet du multi-ensemble.
    assert_eq!(relu.personnages, noms);
}

#[test]
fn ecrire_personnages_accepte_la_liste_vide() {
    // Zéro personnage est un état NORMAL (design §4) : décocher le dernier
    // est permis, et l'application vit alors dans le tray.
    let dossier = std::env::temp_dir().join("shimeji-test-liste-vide");
    let _ = std::fs::create_dir_all(&dossier);
    let chemin = dossier.join("config.json");
    let _ = std::fs::remove_file(&chemin);

    ecrire_personnages(&chemin, &[]).expect("écriture");
    let relu = charger_depuis(&chemin);
    assert!(relu.personnages.is_empty());
}

#[test]
fn ecrire_personnages_preserve_les_cles_inconnues() {
    // L'édition est CHIRURGICALE : sérialiser depuis `Config` remettrait
    // les valeurs par défaut partout, et l'utilisateur verrait son fichier
    // réglé à la main écrasé par un clic dans une autre fenêtre.
    let dossier = std::env::temp_dir().join("shimeji-test-cles-inconnues");
    let _ = std::fs::create_dir_all(&dossier);
    let chemin = dossier.join("config.json");

    std::fs::write(
        &chemin,
        r#"{ "personnages": ["blob"], "mon_reglage_a_moi": 42, "echelle": 2.0 }"#,
    )
    .expect("préparation");

    ecrire_personnages(&chemin, &["luffy".to_string()]).expect("écriture");

    let texte = std::fs::read_to_string(&chemin).expect("relecture");
    let v: serde_json::Value = serde_json::from_str(&texte).expect("JSON");
    assert_eq!(v["mon_reglage_a_moi"], 42);
    assert_eq!(v["echelle"], 2.0);
    assert_eq!(v["personnages"], serde_json::json!(["luffy"]));
}

#[test]
fn ecrire_personnages_n_ecrit_jamais_de_bom() {
    // `serde_json` refuse le BOM avec le message trompeur « expected value
    // at line 1 column 1 », et c'est NOUS qui relisons ce fichier.
    let dossier = std::env::temp_dir().join("shimeji-test-bom");
    let _ = std::fs::create_dir_all(&dossier);
    let chemin = dossier.join("config.json");
    let _ = std::fs::remove_file(&chemin);

    ecrire_personnages(&chemin, &["blob".to_string()]).expect("écriture");

    let octets = std::fs::read(&chemin).expect("relecture");
    assert_ne!(&octets[0..3], &[0xEF, 0xBB, 0xBF]);
}

#[test]
fn une_config_sans_cle_personnages_garde_le_defaut_blob() {
    // Inchangé : c'est le premier démarrage, pas une liste vidée à la main.
    let dossier = std::env::temp_dir().join("shimeji-test-sans-cle");
    let _ = std::fs::create_dir_all(&dossier);
    let chemin = dossier.join("config.json");
    std::fs::write(&chemin, r#"{ "echelle": 1.0 }"#).expect("préparation");

    let relu = charger_depuis(&chemin);
    assert_eq!(relu.personnages, vec!["blob".to_string()]);
}

// ── L'état de la première configuration (plan 2026-09-20, tâche 2) ──────

#[test]
fn par_defaut_la_premiere_configuration_n_est_pas_faite() {
    // C'est CE défaut qui fait s'ouvrir l'assistant au premier lancement,
    // y compris quand il n'existe aucun config.json (spec §2).
    let c = Config::default();
    assert!(!c.premiere_configuration_faite);
    assert_eq!(c.ecran_au_demarrage, EcranDemarrage::Personnages);
}

#[test]
fn les_deux_cles_se_relisent() {
    let f = fichier_de_test(
        r#"{ "premiereConfigurationFaite": true, "ecranAuDemarrage": "gestionnaire" }"#,
    );
    let c = charger_depuis(&f);
    assert!(c.premiere_configuration_faite);
    assert_eq!(c.ecran_au_demarrage, EcranDemarrage::Gestionnaire);
}

#[test]
fn un_ecran_inconnu_retombe_sur_personnages_sans_jeter_le_fichier() {
    // Le point délicat : `charger_depuis` jette TOUTE la configuration sur
    // une erreur de parsing. Une valeur inconnue ne doit donc pas être une
    // erreur de parsing — d'où `ecran_tolerant` (spec §2).
    let f = fichier_de_test(r#"{ "echelle": 2.5, "ecranAuDemarrage": "sur-la-lune" }"#);
    let c = charger_depuis(&f);
    assert_eq!(c.ecran_au_demarrage, EcranDemarrage::Personnages);
    assert_eq!(c.echelle, 2.5, "le reste du fichier doit survivre");
}

#[test]
fn une_config_portant_encore_demarrage_automatique_se_charge() {
    // Le champ a été supprimé (spec §0) : serde ignore les clés inconnues,
    // donc un fichier d'avant continue de marcher.
    let f = fichier_de_test(r#"{ "demarrageAutomatique": true, "echelle": 3 }"#);
    let c = charger_depuis(&f);
    assert_eq!(c.echelle, 3.0);
}

#[test]
fn ecrire_cles_preserve_les_voisines() {
    // Le cœur de la technique chirurgicale : on ne re-sérialise JAMAIS
    // `Config` par-dessus un fichier réglé à la main.
    let f = fichier_de_test(r#"{ "echelle": 2, "_note": "gardez-moi" }"#);
    ecrire_cles(
        &f,
        &[("premiereConfigurationFaite", serde_json::json!(true))],
    )
    .unwrap();

    let texte = lire_json(&f).unwrap();
    let v: serde_json::Value = serde_json::from_str(&texte).unwrap();
    assert_eq!(v["echelle"], 2);
    assert_eq!(v["_note"], "gardez-moi");
    assert_eq!(v["premiereConfigurationFaite"], true);
}

#[test]
fn ecrire_cles_cree_un_fichier_absent_et_refuse_un_illisible() {
    // Absent : cas NORMAL au premier lancement, on crée.
    let dossier = std::env::temp_dir().join("shimeji-test-cles-absent");
    std::fs::create_dir_all(&dossier).unwrap();
    let neuf = dossier.join("config.json");
    let _ = std::fs::remove_file(&neuf);
    ecrire_cles(&neuf, &[("ecranAuDemarrage", serde_json::json!("tray"))]).unwrap();
    assert_eq!(
        charger_depuis(&neuf).ecran_au_demarrage,
        EcranDemarrage::Tray
    );

    // Illisible : on n'écrit RIEN. L'écraser perdrait des réglages que
    // l'utilisateur croit avoir.
    let casse = fichier_de_test("{ ceci n'est pas du json");
    let avant = std::fs::read_to_string(&casse).unwrap();
    assert!(ecrire_cles(&casse, &[("echelle", serde_json::json!(9))]).is_err());
    assert_eq!(std::fs::read_to_string(&casse).unwrap(), avant);
}

#[test]
fn ce_qu_ecrit_ecrire_cles_est_relu_par_charger_depuis() {
    // LE test qui attrape une faute de camelCase. Comparer le JSON ne
    // l'attraperait pas : il faut faire l'aller-RETOUR complet.
    let f = fichier_de_test("{}");
    ecrire_cles(
        &f,
        &[
            ("premiereConfigurationFaite", serde_json::json!(true)),
            (
                "ecranAuDemarrage",
                serde_json::json!(EcranDemarrage::Tray.en_json()),
            ),
        ],
    )
    .unwrap();

    let c = charger_depuis(&f);
    assert!(
        c.premiere_configuration_faite,
        "clé mal nommée : l'assistant reviendrait à chaque lancement"
    );
    assert_eq!(c.ecran_au_demarrage, EcranDemarrage::Tray);
}
