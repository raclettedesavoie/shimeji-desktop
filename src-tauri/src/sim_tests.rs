//! Les tests de `sim` — sortis du fichier source le 2026-09-14.
//!
//! **Pourquoi ce fichier existe** : `sim.rs` faisait 760 lignes dont 229 de tests,
//! soit 30 %. Toute lecture du module en payait le double, pour rien la plupart
//! du temps. Les tests n'ont pas changé d'une ligne : ils ont seulement déménagé, et
//! se désindentent d'un cran puisqu'ils ne sont plus enfermés dans un `mod tests { }`.

use super::*;

/// Le dossier du personnage de test, relatif à `src-tauri/` où cargo
/// exécute les tests.
fn blob() -> &'static Path {
    Path::new("../characters/blob")
}

/// La config par défaut, construite et non lue depuis le disque : un test
/// qui lirait le `config.json` de la machine ne serait plus
/// reproductible.
fn defauts() -> crate::config::Config {
    crate::config::Config::default()
}

#[test]
fn une_simulation_courte_produit_un_resume_coherent() {
    let r = executer(2, 42, blob(), &defauts()).expect("la simulation doit aboutir");

    // 2 minutes à 60 Hz.
    assert_eq!(r.images, 2 * 60 * 60);
    assert!(r.intentions_tirees > 0, "aucune intention tirée");
}

#[test]
fn il_flane_et_il_se_repose_sur_une_longue_duree() {
    // Ce que la spec §10.3 demande de pouvoir vérifier sans écran.
    // 30 minutes : assez pour que les deux intentions sortent, même avec
    // un rapport de poids de 5 contre 1.
    let r = executer(30, 42, blob(), &defauts()).expect("la simulation doit aboutir");

    assert!(r.poses_vues.contains("walk"), "il n'a jamais marché");
    // Et il ne court **jamais** : la course a quitté le tirage de la
    // flânerie (`poids_course` = 0). Ce test la verrouille dehors — si
    // quelqu'un remettait un poids par défaut, il le dirait.
    assert!(
        !r.poses_vues.contains("run"),
        "il a couru alors que la course a quitté la flânerie"
    );
    assert!(r.poses_vues.contains("stand"), "il ne s'est jamais arrêté");
    assert!(r.poses_vues.contains("sit"), "il ne s'est jamais reposé");
}

#[test]
fn il_n_est_jamais_bloque_plus_que_le_delai_d_abandon() {
    // **La régression de comportement que rien d'autre ne détecte.**
    // « Bloqué » = ni la pose, ni la position, ni l'intention n'ont
    // changé (voir le commentaire de l'empreinte).
    let r = executer(30, 42, blob(), &defauts()).expect("la simulation doit aboutir");

    let limite = crate::behavior::intention::DELAI_ABANDON + Duration::from_secs(1);
    assert!(
        r.blocage_max <= limite,
        "bloqué {:?}, limite {:?}",
        r.blocage_max,
        limite
    );
}

#[test]
fn la_simulation_est_reproductible_a_graine_fixe() {
    let a = executer(5, 999, blob(), &defauts()).unwrap();
    let b = executer(5, 999, blob(), &defauts()).unwrap();
    assert_eq!(a, b);
}

#[test]
fn deux_graines_donnent_deux_histoires() {
    // Sinon l'aléatoire ne sert à rien, et « jamais prévisible » est
    // faux. On compare la SIGNATURE et non les compteurs : deux
    // histoires différentes peuvent tirer autant d'intentions.
    let a = executer(5, 1, blob(), &defauts()).unwrap();
    let b = executer(5, 2, blob(), &defauts()).unwrap();
    assert_ne!(a.signature, b.signature);
}

#[test]
fn un_dossier_de_personnage_invalide_donne_une_erreur_lisible() {
    let e =
        executer(1, 1, Path::new("../characters/inexistant"), &defauts()).expect_err("doit échouer");
    assert!(e.contains("mascot.json"), "message peu clair : {e}");
}

#[test]
fn la_chronologie_couvre_une_journee_entiere() {
    // L'heure doit avancer, faire le tour, et rester dans 0..24.
    let h = |min| signaux_de_la_journee(min).heure;
    assert_eq!(h(0), 9, "la journée commence à 9 h");
    assert_eq!(h(60), 10);
    assert_eq!(h(15 * 60), 0, "9 h + 15 h = minuit");
    for min in 0..(24 * 60) {
        assert!(signaux_de_la_journee(min).heure < 24);
    }
}

#[test]
fn la_chronologie_alterne_presence_et_absence() {
    // Aux heures de travail il est là ; la nuit il est parti. Sans cette
    // alternance, la simulation ne prouverait rien : un biais constant ne
    // se distingue pas d'un biais absent.
    let inactif = |min| signaux_de_la_journee(min).inactivite;
    assert_eq!(inactif(30), std::time::Duration::ZERO, "10 h : il travaille");
    assert!(
        inactif(15 * 60) > std::time::Duration::from_secs(120),
        "minuit : il est parti depuis longtemps"
    );
}

#[test]
fn une_journee_entiere_dort_au_bon_moment() {
    // **LE test de l'étape**, et il vérifie cinq choses d'un coup — la
    // cinquième (il a grimpé) a rejoint les quatre premières à l'étape
    // 4a, Tâche 7, précisément pour ne pas payer une deuxième
    // simulation de 24 h.
    //
    // Une seule simulation pour les cinq : dérouler 24 h fait
    // 5,2 millions d'images, soit une à trois secondes en debug. La
    // lancer quatre fois multiplierait par quatre le temps de la suite
    // entière, qui tient aujourd'hui en 0,43 s.
    //
    // > Si ce test dépasse ~10 s sur la machine, le marquer `#[ignore]`
    // > et s'appuyer sur `--sim 1440` (Step 8), qui est de toute façon
    // > l'artefact qu'on lit.
    let r = executer(24 * 60, 42, blob(), &defauts()).expect("la simulation doit aboutir");

    // ── 1. Il dort ──────────────────────────────────────────────────
    // 2 h, 3 h, 4 h : personne devant la machine.
    let nuit: u32 = (2..=4).map(|h| r.endormi_par_heure[h]).sum();
    assert!(nuit > 0, "il n'a pas dormi de la nuit");

    // ── 2. Il dort AU BON MOMENT ────────────────────────────────────
    // C'est la différence entre un signal branché et un signal qui
    // marche : un total de sommeil ne dirait rien, seule la ventilation
    // par heure le dit. 9 h-11 h, il est au clavier.
    let matin: u32 = (9..=11).map(|h| r.endormi_par_heure[h]).sum();
    assert!(
        nuit > matin * 5,
        "il dort autant le matin que la nuit : le signal ne mord pas              (matin {matin} s, nuit {nuit} s)"
    );

    // ── 2 bis. Présent le soir, il ne dort PAS profondément ──────────
    //
    // Aux heures 22-23, la chronologie le dit PRÉSENT (voir le
    // commentaire de `signaux_de_la_journee`) alors que le signal du soir
    // (22 h→6 h, ×3 sur le repos) est déjà actif. Sans l'invariant
    // « phase Endormi ⇒ utilisateur absent » (`intention.rs`), le sommeil
    // profond resterait atteignable : le ×3 du soir suffit à franchir
    // `seuilSommeil = 2.0` à lui seul, présence ou pas.
    //
    // Ce n'est PAS une redite de l'assertion précédente : `matin` ne
    // couvre que 9 h-11 h, où aucun signal de sommeil ne mord — elle ne
    // pouvait donc rien dire de la contradiction entre le signal du soir
    // et la présence.
    let soir_present: u32 = (22..=23).map(|h| r.endormi_par_heure[h]).sum();
    assert_eq!(
        soir_present, 0,
        "il dort profondément à 22-23 h alors que l'utilisateur est présent : \
         l'entrée en sommeil ne doit pas être possible utilisateur actif"
    );

    // ── 3. Il sort du sommeil, par un moyen ou un autre ─────────────
    //
    // La chronologie compte DEUX retours de l'utilisateur : à 14 h
    // (après la pause déjeuner) et à 20 h (après la soirée). 9 h est le
    // DÉBUT de la journée simulée, pas un retour — il n'y a personne
    // avant. Vérifié en rejouant `signaux_de_la_journee` sur les 1440
    // minutes et en comptant les transitions absent → présent (vague de
    // correction finale, point 4 : l'ancien commentaire disait « cinq »,
    // ce qui était faux).
    //
    // ⚠️ **Cette assertion ne teste PAS le mécanisme d'interruption.**
    // `resume.reveils` compte TOUTE transition de la pose `sleep` vers
    // autre chose — y compris la fin naturelle d'un sommeil (20-60 s,
    // `PhaseRepos::Endormi`) ou l'expiration au délai d'abandon (20 s).
    // L'assertion resterait verte même si le bloc d'interruption de
    // `behavior::mod::pas` disparaissait entièrement : un sommeil finit
    // toujours par se terminer tout seul. Le vrai mécanisme du réveil
    // (« redevenir actif termine le sommeil, sans le choisir ») est
    // couvert ailleurs, par les tests unitaires de `behavior/mod.rs`
    // (`redevenir_actif_reveille_le_personnage_endormi`,
    // `le_reveil_ne_choisit_pas_la_suite`,
    // `etre_actif_n_empeche_pas_de_s_asseoir`). Ici, on vérifie
    // seulement qu'il ne reste pas coincé en sommeil pour toujours — et
    // la vraie propriété de l'étape, « il ne dort pas profondément
    // pendant que l'utilisateur travaille », est déjà couverte par
    // l'assertion sur les heures 22-23 ci-dessus.
    assert!(r.reveils > 0, "il ne s'est jamais réveillé");

    // ── 4. LA MARGE SURVIT — le test de la décision n° 3 ────────────
    //
    // Même avec ×8 sur le repos pendant toute la nuit, il ne doit PAS
    // avoir dormi 100 % du temps. « Cette marge est le produit. » Si
    // elle disparaissait, le signal COMMANDERAIT au lieu de biaiser, et
    // aucun autre test ne s'en apercevrait.
    let nuit_complete: u32 = (0..6).map(|h| r.endormi_par_heure[h]).sum();
    let six_heures: u32 = 6 * 3600;
    assert!(
        nuit_complete < (six_heures as f32 * 0.95) as u32,
        "il a dormi {nuit_complete} s sur {six_heures} : la marge a disparu"
    );

    // ── Et la décision n° 4 tient toujours ──────────────────────────
    // Le sommeil est la plus longue immobilité du programme : c'est ici
    // qu'un blocage se verrait.
    assert!(
        r.blocage_max < crate::behavior::intention::DELAI_ABANDON,
        "blocage de {:?}, au-delà du délai d'abandon",
        r.blocage_max
    );

    // ── 5. Il a grimpé (étape 4a) ────────────────────────────────────
    //
    // Réutilise la MÊME simulation de 24 h plutôt que d'en ajouter une
    // seconde : ce test coûte déjà ~7 s à lui seul (5,2 millions
    // d'images), et le lancer deux fois doublerait ce coût pour rien.
    // L'invariant du monde vertical est vérifié À CHAQUE IMAGE dans la
    // boucle ci-dessus (`Attachment::On { face != Top, .. }` ⇒ pose
    // d'escalade) — ici on vérifie seulement qu'il a eu l'OCCASION de
    // s'exercer : un poids `envies.grimper` remis à zéro par erreur
    // laisserait l'invariant vrai par vacuité, sans qu'aucune assertion
    // précédente ne le remarque.
    assert!(
        r.temps_accroche > Duration::ZERO,
        "en 24 h il n'a jamais grimpé une seule fois"
    );
}
