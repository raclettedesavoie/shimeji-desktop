//! La configuration, et la résolution des chemins (spec §9.3, §8.1).
//!
//! Responsabilité unique : trouver les fichiers de l'utilisateur, et en
//! tirer des valeurs utilisables.
//!
//! **`charger()` ne rend pas de `Result`, et c'est une décision.** L'absence
//! de `config.json` n'est pas une erreur, un fichier partiel non davantage,
//! et même un fichier malformé ne doit pas empêcher le personnage de vivre
//! (spec §9.3). Un pet qui refuse de démarrer pour une virgule manquante
//! dans un fichier *optionnel* serait absurde. On signale et on continue.
//!
//! ⚠️ **Ce fichier ne contient aucune valeur en dur qui existerait déjà
//! ailleurs.** Les défauts de vitesse renvoient à `physics::VITESSE_*`, qui
//! sont eux-mêmes tirés de Shimeji-ee. Recopier 50 et 100 ici créerait deux
//! sources de vérité, et l'une des deux finirait par mentir.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Les poids de base de la table d'envies (spec §7.2).
///
/// `rename_all = "camelCase"` pour que le JSON s'écrive `seReposer`, cohérent
/// avec le `frameMs` du manifeste.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Envies {
    pub flaner: f32,
    pub se_reposer: f32,

    /// Le poids des DEUX animations de jeu (Tâche 3) : `desire.rs` en fait
    /// deux lignes de table qui partagent cette même valeur. Un réglage par
    /// jeu serait un réglage de plus sans effet observable, puisque rien ne
    /// les distingue pour l'utilisateur.
    pub jouer: f32,
}

impl Default for Envies {
    fn default() -> Self {
        // Les valeurs de départ de la spec §7.2.
        Envies {
            flaner: 5.0,
            se_reposer: 1.0,
            jouer: 1.0,
        }
    }
}

/// Les réglages de la flânerie : ce qui donne son tempérament au personnage.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Allures {
    /// Poids du tirage entre s'arrêter, marcher et courir.
    pub poids_arret: f32,
    pub poids_marche: f32,
    pub poids_course: f32,

    /// Durées `[min, max]` en secondes de chaque allure.
    pub duree_arret: [f32; 2],
    pub duree_marche: [f32; 2],
    pub duree_course: [f32; 2],

    /// Probabilité de faire demi-tour à chaque changement d'allure.
    ///
    /// C'est **le réglage le plus intéressant du fichier** : c'est lui qui
    /// décide si le personnage paraît décidé ou indécis. Le monter rend son
    /// parcours imprévisible et son déplacement net nul.
    pub chance_demi_tour: f32,
}

impl Default for Allures {
    fn default() -> Self {
        // Repris de `intention::flaner`, où ces valeurs étaient en dur.
        Allures {
            poids_arret: 3.0,
            poids_marche: 6.0,
            poids_course: 1.0,
            duree_arret: [0.8, 3.0],
            duree_marche: [1.5, 5.0],
            duree_course: [0.6, 1.8],
            chance_demi_tour: 0.25,
        }
    }
}

/// Les seuils et multiplicateurs des signaux (design de l'étape 2, §4).
///
/// Tout est ici plutôt qu'en dur dans `signals.rs` : c'est la décision n° 5,
/// et c'est aussi ce qui permet de régler « à partir de combien de temps
/// d'absence il s'endort » sans recompiler — le réglage qu'on voudra
/// certainement toucher après une journée d'usage.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SignauxReglages {
    /// Au-delà de cette durée sans aucune entrée, l'utilisateur est
    /// considéré comme parti.
    pub inactivite_secondes: f32,
    pub inactif_flaner: f32,
    pub inactif_se_reposer: f32,

    /// Le créneau « tard le soir », en heures locales. **Il passe par
    /// minuit** quand `debut > fin`, ce qui est le cas par défaut.
    pub soir_debut: u8,
    pub soir_fin: u8,
    pub soir_se_reposer: f32,

    /// En dessous de ce pourcentage **et** sur batterie, il fatigue.
    pub batterie_seuil: u8,
    pub batterie_se_reposer: f32,

    /// À partir de quel biais de repos il s'affale au lieu de rester assis
    /// (Tâche 4). 2,0 = « il faut qu'un signal ait au moins doublé l'envie
    /// de repos ».
    pub seuil_sommeil: f32,
}

impl Default for SignauxReglages {
    fn default() -> Self {
        // Les valeurs de départ de la spec §7.2.
        SignauxReglages {
            inactivite_secondes: 120.0,
            inactif_flaner: 0.2,
            inactif_se_reposer: 8.0,
            soir_debut: 22,
            soir_fin: 6,
            soir_se_reposer: 3.0,
            batterie_seuil: 20,
            batterie_se_reposer: 2.0,
            seuil_sommeil: 2.0,
        }
    }
}

/// Ce qu'une application au premier plan change au caractère du personnage.
///
/// Des `Option<f32>` et non des `f32` nus : une ligne qui ne parle que de
/// `flaner` ne doit pas remettre les autres poids à zéro. Avec des `f32`, un
/// champ absent vaudrait `0.0` — ce qui **interdirait** l'intention, le
/// contraire d'un défaut inoffensif.
#[derive(Debug, Clone, Copy, PartialEq, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ModifsAppli {
    pub flaner: Option<f32>,
    pub se_reposer: Option<f32>,

    /// Le biais de jeu ne distingue pas les deux animations (voir
    /// `Envies::jouer`) : un seul champ suffit ici aussi.
    pub jouer: Option<f32>,
}

/// Le contenu de `config.json`.
///
/// `#[serde(default)]` **au niveau de la structure** : chaque champ absent
/// prend sa valeur de `Default`. Une seule annotation couvre tous les champs,
/// présents comme futurs — et c'est ce qui rend un fichier partiel valide
/// sans avoir à y penser champ par champ.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    /// Les dossiers de `characters/` à instancier.
    pub personnages: Vec<String>,

    /// Multiplie la taille d'affichage, en plus du `scale` du manifeste et de
    /// l'échelle du moniteur.
    pub echelle: f32,

    /// Multiplie les vitesses de marche et de course.
    ///
    /// À 1, on est exactement aux valeurs de Shimeji-ee.
    pub vitesse: f32,

    pub demarrage_automatique: bool,
    pub envies: Envies,
    pub allures: Allures,

    pub signaux: SignauxReglages,

    /// Les modificateurs par application, `"Code.exe"` → ses poids.
    ///
    /// Une table associative, donc **ajouter une application ne demande
    /// aucun code** : c'est la forme la plus littérale de « ajouter un
    /// signal = ajouter une ligne » (décision n° 5).
    ///
    /// `BTreeMap` et non `HashMap` : l'ordre d'itération est stable, donc un
    /// message de diagnostic qui les liste ne change pas d'ordre d'une
    /// exécution à l'autre. Le coût de recherche est sans importance — la
    /// table a trois entrées et n'est consultée que 2 fois par seconde.
    pub applications: std::collections::BTreeMap<String, ModifsAppli>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            // `blob` est le personnage de test, et le seul livré.
            personnages: vec!["blob".to_string()],
            echelle: 1.0,
            vitesse: 1.0,
            demarrage_automatique: false,
            envies: Envies::default(),
            allures: Allures::default(),
            signaux: SignauxReglages::default(),
            // Vide par défaut : aucun modificateur d'application n'est
            // imposé. Le fichier d'exemple en montre deux, commentés par
            // leur seule présence.
            applications: std::collections::BTreeMap::new(),
        }
    }
}

/// Les valeurs que le comportement consulte, déjà converties.
///
/// Séparée de `Config` pour une raison de principe : `Config` est ce que
/// l'utilisateur écrit, `Reglages` est ce que le code utilise. Le passage de
/// l'une à l'autre est le seul endroit où l'on borne les valeurs absurdes, et
/// le comportement n'a donc jamais à se demander si ce qu'il reçoit est sain.
///
/// **Passée en paramètre, jamais consultée globalement** : c'est la même
/// discipline que l'horloge et l'aléatoire (spec §10.2), et c'est ce qui
/// garde les tests capables de fournir leurs propres réglages.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reglages {
    pub vitesse_marche: f32,
    pub vitesse_course: f32,
    pub allures: Allures,
}

/// Bornes du facteur de vitesse.
///
/// Les valeurs viennent d'un fichier édité à la main : un `0` figerait le
/// personnage — ce qui ressemblerait à un bug, pas à un réglage — et un
/// `10000` le ferait traverser l'écran en une image, ce qui casserait la
/// détection d'atterrissage par segment.
const FACTEUR_VITESSE_MIN: f32 = 0.1;
const FACTEUR_VITESSE_MAX: f32 = 10.0;

impl Reglages {
    pub fn depuis(config: &Config) -> Reglages {
        use crate::character::physics::{VITESSE_COURSE, VITESSE_MARCHE};

        let facteur = config
            .vitesse
            .clamp(FACTEUR_VITESSE_MIN, FACTEUR_VITESSE_MAX);

        Reglages {
            vitesse_marche: VITESSE_MARCHE * facteur,
            vitesse_course: VITESSE_COURSE * facteur,
            allures: config.allures,
        }
    }
}

/// Lit un fichier JSON de l'utilisateur, **BOM retiré**.
///
/// # Pourquoi cette fonction existe
///
/// `serde_json` refuse une marque d'ordre des octets (BOM) : il donne
/// `expected value at line 1 column 1`, un message qui ne dit pas du tout ce
/// qui se passe et laisse chercher une virgule manquante dans un fichier
/// parfaitement valide.
///
/// **Et sur Windows, le BOM est le cas par défaut** : le Bloc-notes en écrit
/// un, `Set-Content -Encoding utf8` de PowerShell 5.1 aussi. Un utilisateur
/// qui édite `config.json` ou `mascot.json` à la main tombe donc dessus
/// presque à coup sûr.
///
/// Découvert en **lançant** l'application avec un `config.json` écrit par
/// PowerShell : il était valide et pourtant rejeté. Aucune relecture ne
/// l'aurait montré.
///
/// `strip_prefix` sur une chaîne rend une `Option` — `None` s'il n'y avait
/// pas de BOM, ce qui est le cas le plus courant. D'où le `unwrap_or`.
pub fn lire_json(chemin: &Path) -> std::io::Result<String> {
    let texte = std::fs::read_to_string(chemin)?;

    // U+FEFF, le BOM, tel qu'il apparaît une fois l'UTF-8 décodé.
    const BOM: char = '\u{feff}';
    Ok(texte.strip_prefix(BOM).unwrap_or(&texte).to_string())
}

/// Cherche un fichier ou un dossier nommé `nom`, à côté de l'exe puis dans
/// `%APPDATA%` (spec §8.1, §9.3).
///
/// Rend `None` si on ne le trouve nulle part — ce qui n'est une erreur que
/// pour l'appelant, qui sait s'il peut s'en passer.
pub fn resoudre(nom: &str) -> Option<PathBuf> {
    // ── À côté de l'exe ────────────────────────────────────────────────
    // `if let Ok(...)` : `current_exe` peut échouer sur des systèmes
    // exotiques. Ce n'est pas une raison de ne pas démarrer.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidat = dir.join(nom);
            if candidat.exists() {
                return Some(candidat);
            }

            // ── Repli de développement ─────────────────────────────────
            // En `cargo run`, l'exe est dans `src-tauri/target/debug/` : on
            // remonte les parents pour trouver la racine du dépôt. Cinq
            // niveaux suffisent à en sortir, et pas assez pour partir
            // explorer le disque.
            for parent in dir.ancestors().take(5) {
                let candidat = parent.join(nom);
                if candidat.exists() {
                    return Some(candidat);
                }
            }
        }
    }

    // ── %APPDATA%\shimeji-desktop\ ─────────────────────────────────────
    if let Ok(appdata) = std::env::var("APPDATA") {
        let candidat = PathBuf::from(appdata).join("shimeji-desktop").join(nom);
        if candidat.exists() {
            return Some(candidat);
        }
    }

    None
}

/// Le dossier des personnages.
///
/// Contrairement à `resoudre`, rend toujours un chemin : s'il n'existe pas,
/// le chargement du manifeste échouera avec un message qui **nomme le chemin
/// cherché**, ce qui est exactement ce qu'il faut pour diagnostiquer.
pub fn dossier_personnages() -> PathBuf {
    resoudre("characters").unwrap_or_else(|| PathBuf::from("characters"))
}

/// Charge la configuration. **Ne peut pas échouer.**
pub fn charger() -> Config {
    match resoudre("config.json") {
        Some(chemin) => charger_depuis(&chemin),
        None => {
            // Silencieux : l'absence de fichier est le cas NORMAL, pas un
            // avertissement à afficher à chaque démarrage.
            Config::default()
        }
    }
}

/// Charge depuis un chemin donné. Séparée de `charger` pour être testable
/// sans toucher au dossier de l'exe.
pub fn charger_depuis(chemin: &Path) -> Config {
    let texte = match lire_json(chemin) {
        Ok(t) => t,
        Err(_) => {
            // Fichier absent ou illisible. On ne dit rien : `charger` ne
            // parvient ici que si `resoudre` l'a trouvé, et les tests
            // appellent volontairement des chemins inexistants.
            return Config::default();
        }
    };

    match serde_json::from_str(&texte) {
        Ok(c) => c,
        Err(e) => {
            // **Bruyant, celui-là.** L'utilisateur a écrit un fichier et
            // s'attend à ce qu'il serve ; s'il est malformé il doit le
            // savoir, sinon il croira que ses réglages sont pris en compte.
            eprintln!("config.json invalide : {e}");
            eprintln!("  -> valeurs par défaut utilisées, le fichier est ignoré en entier");
            Config::default()
        }
    }
}

#[cfg(test)]
mod tests {
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
}
