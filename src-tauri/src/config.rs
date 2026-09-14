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

    /// Le poids de l'envie de grimper (étape 4a).
    ///
    /// Le même que `se_reposer` : l'escalade est longue (jusqu'à 64 s pour un
    /// mur entier, contre quelques secondes pour un repos), donc un poids
    /// égal se traduit déjà, à l'œil, par beaucoup de temps passé sur les
    /// murs. Le monter le ferait vivre en hauteur.
    pub grimper: f32,
}

impl Default for Envies {
    fn default() -> Self {
        // Les valeurs de départ de la spec §7.2.
        Envies {
            flaner: 5.0,
            se_reposer: 1.0,
            jouer: 1.0,
            grimper: 1.0,
        }
    }
}

/// Les réglages de la flânerie : ce qui donne son tempérament au personnage.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Allures {
    /// Poids du tirage entre s'arrêter, marcher et courir.
    ///
    /// `poids_course` vaut **0 par défaut** : la course n'appartient plus à
    /// la flânerie aléatoire. Elle est réservée à des actions précises qui
    /// la demanderont explicitement (se dépêcher vers une fenêtre, fuir un
    /// autre personnage). Le poids reste réglable : le mettre à 1 dans
    /// `config.json` rend exactement l'ancien comportement.
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
            // 0 : il ne se met plus à courir sans raison (voir le champ).
            poids_course: 0.0,
            duree_arret: [0.8, 3.0],
            duree_marche: [1.5, 5.0],
            duree_course: [0.6, 1.8],
            chance_demi_tour: 0.25,
        }
    }
}

/// Ce qui décide s'il est casse-cou ou prudent sur un mur (décision n° 5).
///
/// Séparée d'`Allures` parce qu'elle ne décrit pas la même chose : `Allures`
/// règle la flânerie au sol, celle-ci règle la sortie d'une accroche. Les
/// fondre donnerait une structure dont la moitié des champs ne s'applique
/// jamais au cas courant.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Escalade {
    /// Poids du tirage de sortie, en fin d'accroche : se lâcher et tomber.
    pub poids_lacher: f32,

    /// Poids du tirage de sortie : redescendre tranquillement.
    ///
    /// Deux fois le poids de `poids_lacher` par défaut : un personnage qui se
    /// lâcherait une fois sur deux passerait son temps en l'air, et la chute
    /// perdrait sa valeur de surprise.
    pub poids_redescendre: f32,

    /// Bornes `[min, max]` de la durée d'une accroche, en secondes.
    pub duree_accroche: [f32; 2],
}

impl Default for Escalade {
    fn default() -> Self {
        Escalade {
            poids_lacher: 1.0,
            poids_redescendre: 2.0,
            // Relevée dans `conf/actions.xml`, comme toutes les durées
            // d'animation du projet : la valeur vit dans `physics.rs`, et la
            // config ne fait que la reprendre comme valeur PAR DÉFAUT.
            duree_accroche: crate::character::physics::DUREE_ACCROCHE,
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

/// Bornes de `inactivite_secondes` (vague de correction finale).
///
/// La valeur vient d'un `config.json` édité à la main, et
/// `Duration::from_secs_f32` **panique** sur un flottant négatif —
/// `{"signaux": {"inactiviteSecondes": -1}}` est du JSON parfaitement valide,
/// que `serde_json` désérialise sans erreur. Le panique arrive ensuite, dans
/// `signals::biais_de` et `signals::utilisateur_actif`, au premier battement
/// de la boucle à 2 Hz — et en `release`, il n'y a pas de console : le
/// symptôme est « le personnage s'est figé, sans raison », rien de plus.
///
/// Une borne haute existe aussi, généreuse (24 h) : elle n'empêche rien
/// d'utile, elle écarte seulement les valeurs qui trahissent une faute de
/// frappe (un zéro de trop).
const INACTIVITE_SECONDES_MIN: f32 = 0.0;
const INACTIVITE_SECONDES_MAX: f32 = 24.0 * 3600.0;

impl SignauxReglages {
    /// Ramène les champs qui peuvent faire paniquer un appelant dans une
    /// plage sûre.
    ///
    /// **Appelée une seule fois, à la sortie de `charger_depuis`** — c'est
    /// l'endroit choisi pour couvrir tous les sites d'appel à la fois :
    /// `signals::biais_de` et `signals::utilisateur_actif` lisent tous deux
    /// `Config::signaux` directement, sans passer par `Reglages::depuis` (qui
    /// ne borne que ce qu'il recopie lui-même, comme le facteur de vitesse).
    /// Borner ici, une fois, évite d'avoir à se souvenir de le refaire à
    /// chaque nouveau consommateur de ce champ.
    ///
    /// `seuilSommeil` négatif n'est volontairement PAS bordé ici : une
    /// comparaison (`>=`) ne panique jamais sur une valeur négative, borner
    /// n'y gagnerait rien.
    fn borner(&mut self) {
        self.inactivite_secondes = self
            .inactivite_secondes
            .clamp(INACTIVITE_SECONDES_MIN, INACTIVITE_SECONDES_MAX);
    }
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

    /// Les réglages de l'escalade (étape 4a).
    pub escalade: Escalade,

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
            escalade: Escalade::default(),
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

    /// Vitesse d'escalade, déjà multipliée par le facteur de vitesse de
    /// l'utilisateur — exactement comme la marche et la course. Sans ce
    /// facteur, régler `vitesse` accélérerait la marche et laisserait
    /// l'escalade à son rythme, ce qui serait incohérent à l'œil.
    pub vitesse_escalade: f32,

    pub allures: Allures,
    pub escalade: Escalade,

    /// À partir de quel biais de repos il s'affale au lieu de rester assis
    /// (Tâche 4, `behavior::intention::se_reposer`).
    pub seuil_sommeil: f32,
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
        use crate::character::physics::{VITESSE_COURSE, VITESSE_ESCALADE, VITESSE_MARCHE};

        let facteur = config
            .vitesse
            .clamp(FACTEUR_VITESSE_MIN, FACTEUR_VITESSE_MAX);

        Reglages {
            vitesse_marche: VITESSE_MARCHE * facteur,
            vitesse_course: VITESSE_COURSE * facteur,
            vitesse_escalade: VITESSE_ESCALADE * facteur,
            allures: config.allures,
            escalade: config.escalade,
            seuil_sommeil: config.signaux.seuil_sommeil,
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

/// La bibliothèque : là où le catalogue **installe**.
///
/// Contrairement à `resoudre`, elle ne CHERCHE pas : c'est une destination
/// d'écriture, toujours au même endroit. Rendre un chemin qui n'existe pas
/// encore est donc normal — c'est à l'installation de le créer.
///
/// `Option` parce que `%APPDATA%` peut manquer sur un système exotique, et
/// que l'absence de bibliothèque n'est pas une erreur : on retombe alors sur
/// le seul dossier livré.
pub fn dossier_bibliotheque() -> Option<PathBuf> {
    match std::env::var("APPDATA") {
        Ok(appdata) => Some(
            PathBuf::from(appdata)
                .join("shimeji-desktop")
                .join("characters"),
        ),
        Err(_) => None,
    }
}

/// Où trouver le personnage nommé `nom` : la bibliothèque D'ABORD, le
/// dossier livré ENSUITE.
///
/// ⚠️ **`resoudre` n'est volontairement PAS modifiée.** Ses appelants gardent
/// exactement le comportement qu'ils ont, ce qui rend ce changement sans
/// risque de régression. On ajoute à côté, on ne détourne pas l'existant.
///
/// Ça referme aussi un piège connu : en `cargo run`, `resoudre("characters")`
/// trouvait toujours le dossier du dépôt et masquait `%APPDATA%`. Ici les
/// deux racines sont consultées, dans un ordre fixe et écrit.
pub fn dossier_du_personnage(nom: &str) -> Option<PathBuf> {
    // `as_deref` : `Option<PathBuf>` → `Option<&Path>`. On prête le chemin
    // sans le copier ni céder la propriété de l'`Option` locale, qui doit
    // vivre jusqu'à la fin de l'expression.
    let biblio = dossier_bibliotheque();
    personnage_dans(biblio.as_deref(), &dossier_personnages(), nom)
}

/// Le cœur de `dossier_du_personnage`, **paramétré par ses deux racines**.
///
/// Séparée pour une raison de test et non d'esthétique : la version publique
/// lit `%APPDATA%` par `env::var`, variable GLOBALE au processus. La modifier
/// dans un test la modifierait pour tous les tests tournant en parallèle.
/// Aucune variable d'environnement n'entre donc dans aucun test.
pub fn personnage_dans(bibliotheque: Option<&Path>, livre: &Path, nom: &str) -> Option<PathBuf> {
    // `if let Some(b)` : la bibliothèque peut ne pas exister, ce qui n'est
    // pas une erreur — on passe simplement au dossier livré.
    if let Some(b) = bibliotheque {
        let candidat = b.join(nom);
        if candidat.is_dir() {
            return Some(candidat);
        }
    }

    let candidat = livre.join(nom);
    if candidat.is_dir() {
        return Some(candidat);
    }

    None
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

    match serde_json::from_str::<Config>(&texte) {
        Ok(mut c) => {
            // Un JSON valide peut quand même contenir une valeur absurde
            // (voir `SignauxReglages::borner`) : un fichier malformé n'est
            // pas la seule façon de casser la promesse de la spec §9.3.
            c.signaux.borner();
            c
        }
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

// Les tests de ce module vivent dans `config_tests.rs`
// (sortis d ici le 2026-09-14 : ils faisaient 171 des 652 lignes).
#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
