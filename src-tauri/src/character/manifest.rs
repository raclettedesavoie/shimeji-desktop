//! Chargement et validation de `mascot.json` (spec §8.5).
//!
//! Responsabilité unique : transformer un dossier de personnage en `Manifest`
//! utilisable, ou en une erreur explicite.
//!
//! **Le principe qui gouverne ce fichier : une pose dont les images manquent
//! est RETIRÉE, pas une erreur** (spec §8.6). C'est ce qui rend utilisable
//! n'importe quel pack Shimeji trouvé sur internet, même incomplet — et c'est
//! ce qui, à la Tâche 8, retirera une intention du tirage sans un seul cas
//! particulier à coder.

use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::time::Duration;

// ── Les noms de poses connus du code ───────────────────────────────────
//
// Le vocabulaire vient des slots Shimeji (spec §8.2). Des constantes plutôt
// que des littéraux dispersés : une faute de frappe devient une erreur de
// compilation au lieu d'une pose silencieusement absente.
//
// Ces neuf-là sont celles dont l'étape 1a a besoin. Les autres poses du
// manifeste (`sprawl`, `creep`, `grabWall`, `climbWall`, `grabCeiling`,
// `climbCeiling`, `split`…) sont chargées mais encore inutilisées : elles
// servent aux étapes 2 et 4.
//
// **La correspondance frames → poses vient de `conf/actions.xml` de
// Shimeji-ee**, pas d'une observation à l'œil. Justification pose par pose,
// et les cinq erreurs que ce relevé a corrigées :
// `docs/specs/2026-09-09-frames-shimeji.md`.

pub const POSE_STAND: &str = "stand";
pub const POSE_WALK: &str = "walk";
pub const POSE_RUN: &str = "run";
pub const POSE_SIT: &str = "sit";
pub const POSE_FALL: &str = "fall";
pub const POSE_LAND: &str = "land";

/// Le réveil : il émerge du sommeil.
///
/// ⚠️ **Shimeji-ee n'a pas plus d'animation de réveil que de sommeil.** Comme
/// pour `sleep`, on déclare un substitut sous un nom propre pour que le code
/// ignore que c'en est un. Un pack tiers avec un vrai réveil le déclarerait
/// au même nom, sans une ligne de Rust à changer (spec §8.6).
///
/// **Ce sont les MÊMES frames que `land`, 18 puis 19, et dans le même
/// ordre.** Ça surprend, donc voici le relevé qui le justifie
/// (`docs/specs/2026-09-09-frames-shimeji.md`) : la descente vers
/// l'affalement est `19 → 18 → 20 → 21` — `Tripping` enchaîne 19, 18, 20,
/// 20, 19 et `Creep` enchaîne 20, 20, 21, 21, 21. Se relever, c'est donc
/// remonter cette suite, et 18 puis 19 en est la fin. `land` (`Bouncing`)
/// est exactement le même mouvement : on encaisse, on se redresse.
///
/// ⚠️ **Ne pas « corriger » en inversant l'ordre.** Ça a été essayé : 19 puis
/// 18, c'est se lever PUIS se rasseoir, et comme le comportement normal
/// relève ensuite le personnage, on voit le réveil **deux fois**.
///
/// **Un pack sans cette pose se réveille quand même** : il reste simplement
/// affalé le temps de la phase, puis repart. C'est la couverture partielle,
/// et elle ne demande aucun cas particulier ici.
pub const POSE_WAKE: &str = "wake";

/// Affalé sur le ventre — notre pose de sommeil.
///
/// ⚠️ **Shimeji-ee n'a AUCUNE animation de sommeil**, et aucune frame du pack
/// n'a les yeux fermés : les yeux du blob sont deux points. La frame 21
/// (`Sprawl`) est le substitut le plus lisible, et elle est déclarée sous le
/// nom `sleep` **pour que le code ignore qu'il s'agit d'un substitut** — un
/// pack tiers avec une vraie pose de sommeil la déclarerait au même nom, et
/// rien ne changerait ici (spec §8.6).
///
/// Détail et sprites vérifiés : `docs/specs/2026-09-09-frames-shimeji.md`.
pub const POSE_SLEEP: &str = "sleep";

/// Assis, il se tourne la tête. Source : `SitAndSpinHeadAction`.
pub const POSE_SPIN_HEAD: &str = "spinHead";

/// Assis à balancer les jambes. Source : `SitAndDangleLegs`.
///
/// ⚠️ Son ancre est `64,112` et non `64,128` : les jambes pendent **sous** la
/// ligne de contact. Sur le sol, elles descendent donc de 16 px dans la barre
/// des tâches — c'est ce que fait Shimeji-ee, qui déclare bien cette action
/// avec `BorderType="Floor"`.
pub const POSE_SIT_DANGLE: &str = "sitDangle";

/// Porté, au repos : il pend droit. Frame 1.
pub const POSE_DRAGGED: &str = "dragged";

/// La pose d'accroche à une paroi verticale — `GrabWall`, frame **13**.
///
/// ⚠️ Ce sont bien les frames **12, 13, 14** qui font le mur. Les frames
/// 23, 24, 25 sont celles du **plafond** (`GrabCeiling` / `ClimbCeiling`) :
/// la table de `CLAUDE.md` les attribuait à tort à la paroi verticale.
/// `docs/specs/2026-09-09-frames-shimeji.md` fait foi.
pub const POSE_GRAB_WALL: &str = "grabWall";

/// L'escalade d'une paroi — `ClimbWall`, frames 14, 12, 13.
pub const POSE_CLIMB_WALL: &str = "climbWall";

/// La pose de suspension au plafond — `GrabCeiling`, frame **23**, ancre
/// `64,48` (`docs/specs/2026-09-09-frames-shimeji.md`).
///
/// **Volontairement absente des `poses_requises` de `Grimper`** (voir
/// `behavior::desire`) : un pack sans images de plafond doit pouvoir
/// grimper un mur quand même, et simplement s'arrêter en haut sans jamais
/// basculer (couverture partielle, spec §8.6).
pub const POSE_GRAB_CEILING: &str = "grabCeiling";

/// Le déplacement au plafond — `ClimbCeiling`, frames 23, 24, 25.
pub const POSE_CLIMB_CEILING: &str = "climbCeiling";

/// Les poses de balancement, **tête à gauche**, du plus léger au plus ample.
///
/// Frames 6, 8, 10. Le pied traîne alors à droite, ce qui arrive quand le
/// curseur va vers la **gauche** : un pendule traîne derrière.
///
/// Sept poses à une frame chacune, et non trois animations en boucle : le
/// niveau de balancement est un **état physique** calculé par
/// `physics::integrer_balancier`, pas une animation qui se déroule. C'est ce
/// qui donne l'amplitude proportionnelle à la vitesse et le retour au repos
/// en passant par les niveaux intermédiaires.
pub const POSES_DRAGGED_LEFT: [&str; 3] = ["draggedLeft1", "draggedLeft2", "draggedLeft3"];

/// Les poses de balancement, **tête à droite**. Frames 5, 7, 9.
pub const POSES_DRAGGED_RIGHT: [&str; 3] = ["draggedRight1", "draggedRight2", "draggedRight3"];

/// Le rectangle réellement occupé par le personnage dans la boîte de 128×128
/// (spec §8.4). Sert au hit-testing (Tâche 11) et à la proximité entre
/// personnages (étape 3).
///
/// `#[serde(from = "[f32; 4]")]` : dans le JSON c'est un tableau
/// `[x, y, l, h]`, mais on veut des champs nommés dans le code — se souvenir
/// que `.2` est la largeur est exactement le genre de détail qui produit des
/// bugs silencieux. Serde désérialise le tableau, puis appelle le `From`
/// ci-dessous.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(from = "[f32; 4]")]
pub struct Hitbox {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl From<[f32; 4]> for Hitbox {
    fn from(v: [f32; 4]) -> Self {
        Hitbox {
            x: v[0],
            y: v[1],
            w: v[2],
            h: v[3],
        }
    }
}

/// L'ancre par défaut : le sol sous les pieds, **tout en bas** de la boîte,
/// centré.
///
/// `[64, 128]` et non `[64, 120]` comme le supposait la spec §8.5 : dans
/// `conf/actions.xml` de Shimeji-ee, toutes les poses au sol portent
/// `ImageAnchor="64,128"`. Les seules exceptions sont explicites et
/// déclarées pose par pose dans le manifeste — `64,112` pour les poses
/// assises jambes ballantes, `64,48` pour l'agrippement au plafond.
///
/// Les 8 px d'écart enfonçaient le personnage sous la ligne du sol.
fn ancre_par_defaut() -> [f32; 2] {
    [64.0, 128.0]
}

/// La durée d'affichage par défaut d'une frame (spec §8.5).
fn frame_ms_par_defaut() -> u32 {
    150
}

fn scale_par_defaut() -> f32 {
    1.0
}

fn frame_size_par_defaut() -> [u32; 2] {
    [128, 128]
}

/// Une pose : une suite d'images, un rythme, une ancre.
///
/// `rename_all = "camelCase"` : le JSON écrit `frameMs`, le Rust `frame_ms`.
/// Une seule annotation évite un `rename` par champ.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pose {
    /// Numéros de `shime<n>.png`, dans l'ordre de lecture.
    pub frames: Vec<u32>,

    #[serde(default = "frame_ms_par_defaut")]
    pub frame_ms: u32,

    /// `loop` est un **mot-clé Rust** : impossible de nommer le champ ainsi.
    /// `rename` fait le pont avec le JSON, et `default` donne `false`.
    #[serde(rename = "loop", default)]
    pub looping: bool,

    /// Frame conservée à l'arrêt quand `looping` est faux. C'est ce qui
    /// permet de *rester* endormi après la séquence d'endormissement
    /// (spec §8.5). Absent → la dernière frame de la séquence.
    #[serde(default)]
    pub hold: Option<u32>,

    /// `[x, y]` dans la boîte : le point du sprite à faire coïncider avec la
    /// position sur la plateforme.
    ///
    /// Gardé en tableau brut plutôt qu'en `geom::Point` : `geom` n'a aucune
    /// dépendance, et lui en ajouter une sur `serde` pour ce seul champ
    /// coûterait plus que la conversion au point d'usage (Tâche 5).
    #[serde(default = "ancre_par_defaut")]
    pub anchor: [f32; 2],

    /// Hitbox propre à cette pose. Absent → celle du manifeste (spec §8.5).
    #[serde(default)]
    pub hitbox: Option<Hitbox>,
}

impl Pose {
    /// Durée d'un cycle complet de l'animation.
    pub fn duree_totale(&self) -> Duration {
        Duration::from_millis(self.frame_ms as u64 * self.frames.len() as u64)
    }

    /// Quelle image afficher après `ecoule` passé dans cette pose.
    ///
    /// C'est une **fonction pure du temps écoulé**, et non un compteur qu'on
    /// incrémente. Deux bénéfices : elle est testable sans faire tourner de
    /// boucle, et un décalage d'image ne peut pas s'accumuler.
    pub fn frame_a(&self, ecoule: Duration) -> u32 {
        // Une pose sans frame ne devrait pas exister — `load` les retire.
        // Mais `frame_a` peut être appelée sur une `Pose` construite à la
        // main dans un test, donc on ne suppose rien.
        if self.frames.is_empty() {
            return 1;
        }

        let index = if self.frame_ms == 0 {
            // frameMs à 0 : on tiendrait la première image indéfiniment.
            // Une division par zéro plus loin serait un panic.
            0
        } else {
            (ecoule.as_millis() / self.frame_ms as u128) as usize
        };

        if index < self.frames.len() {
            return self.frames[index];
        }

        // La séquence est finie.
        if self.looping {
            // `%` sur l'index et non sur le temps : une seule opération, et
            // pas de perte de précision sur les longues durées.
            self.frames[index % self.frames.len()]
        } else {
            // `hold` s'il est donné, la dernière frame sinon.
            //
            // `unwrap_or_else` et non `unwrap_or` : on ne veut pas évaluer
            // l'accès au dernier élément si `hold` est présent.
            self.hold
                .unwrap_or_else(|| *self.frames.last().expect("non vide, testé plus haut"))
        }
    }
}

/// Le manifeste complet d'un personnage.
///
/// `BTreeMap` et non `HashMap` : l'ordre des poses devient déterministe, ce
/// qui rend la trace du mode simulation (Tâche 9) reproductible d'une
/// exécution à l'autre. Le coût est nul à cette taille.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub id: String,
    pub name: String,

    #[serde(default = "frame_size_par_defaut")]
    pub frame_size: [u32; 2],

    /// Multiplie la taille d'affichage. Le facteur d'échelle du moniteur s'y
    /// combine (spec §3.4, §8.5).
    #[serde(default = "scale_par_defaut")]
    pub scale: f32,

    pub poses: BTreeMap<String, Pose>,

    /// La hitbox par défaut, pour les poses qui n'en déclarent pas.
    pub hitbox: Hitbox,
}

/// Ce qui peut mal se passer au chargement.
///
/// Un `enum` et non une chaîne : l'appelant peut distinguer « ce dossier
/// n'est pas un personnage » (on l'ignore) de « ce personnage est cassé »
/// (on le signale). Et les tests peuvent vérifier *laquelle* des erreurs
/// s'est produite.
#[derive(Debug)]
pub enum ManifestError {
    FichierIllisible { chemin: String, cause: String },
    JsonInvalide(String),
    /// Aucune pose ne possède ses images. Ce n'est plus de la couverture
    /// partielle, c'est un dossier vide — et là, il faut le dire.
    AucunePoseJouable,
}

// `Display` plutôt que la crate `thiserror` : trois variantes ne justifient
// pas une dépendance, et écrire le message à la main le rend lisible en
// français, ce que le projet exige.
impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::FichierIllisible { chemin, cause } => {
                write!(f, "impossible de lire « {chemin} » : {cause}")
            }
            ManifestError::JsonInvalide(msg) => {
                write!(f, "mascot.json invalide : {msg}")
            }
            ManifestError::AucunePoseJouable => write!(
                f,
                "aucune pose n'a ses images — le dossier img/ est-il vide \
                 ou les numéros de frames faux ?"
            ),
        }
    }
}

// `std::error::Error` : rend l'erreur utilisable avec `Box<dyn Error>` et
// `?`. L'implémentation par défaut suffit, `Display` et `Debug` étant là.
impl std::error::Error for ManifestError {}

impl Manifest {
    /// Charge `<dir>/mascot.json` et **retire les poses dont les images
    /// manquent** dans `<dir>/img/`.
    ///
    /// Le retrait est le cœur de la spec §8.6 : il n'échoue pas, il réduit.
    /// Un personnage sans images d'escalade ne grimpera jamais, et ça se
    /// règle tout seul au tirage des envies (Tâche 8).
    pub fn load(dir: &Path) -> Result<Manifest, ManifestError> {
        let chemin = dir.join("mascot.json");

        // `config::lire_json` et non `fs::read_to_string` : elle retire le
        // BOM que le Bloc-notes et PowerShell écrivent par défaut sur
        // Windows, et que `serde_json` refuse avec un message trompeur.
        //
        // Le manifeste est encore plus exposé que `config.json` : il est
        // *fait* pour être édité à la main, c'est le bénéfice annoncé du
        // format (spec §8.5).
        let texte = crate::config::lire_json(&chemin).map_err(|e| {
            // `map_err` : on remplace l'erreur d'E/S par la nôtre, en gardant
            // son message. C'est ce qui permet de dire QUEL fichier manque.
            ManifestError::FichierIllisible {
                chemin: chemin.display().to_string(),
                cause: e.to_string(),
            }
        })?;

        let mut manifeste: Manifest =
            serde_json::from_str(&texte).map_err(|e| ManifestError::JsonInvalide(e.to_string()))?;

        // ── Retrait des poses injouables ───────────────────────────────
        let dossier_img = dir.join("img");
        let mut retirees: Vec<String> = Vec::new();

        // `retain` garde les éléments pour lesquels la fermeture rend `true`.
        // On collecte au passage les noms retirés, pour pouvoir les
        // annoncer : une pose qui disparaît en silence rendrait le réglage
        // d'un manifeste très pénible à diagnostiquer.
        manifeste.poses.retain(|nom, pose| {
            // Une pose sans frame ne peut rien afficher : même traitement
            // qu'une pose sans images.
            if pose.frames.is_empty() {
                retirees.push(nom.clone());
                return false;
            }

            let toutes_presentes = pose
                .frames
                .iter()
                .all(|n| dossier_img.join(format!("shime{n}.png")).is_file());

            if !toutes_presentes {
                retirees.push(nom.clone());
            }
            toutes_presentes
        });

        if !retirees.is_empty() {
            // `eprintln!` et non `println!` : c'est un diagnostic, il n'a
            // rien à faire dans la trace du mode simulation.
            eprintln!(
                "personnage « {} » : {} pose(s) retirée(s) faute d'images — {}",
                manifeste.id,
                retirees.len(),
                retirees.join(", ")
            );
        }

        if manifeste.poses.is_empty() {
            return Err(ManifestError::AucunePoseJouable);
        }

        Ok(manifeste)
    }

    pub fn pose(&self, nom: &str) -> Option<&Pose> {
        self.poses.get(nom)
    }

    /// **La fonction qui rend la couverture partielle gratuite.**
    ///
    /// Le tirage des envies (Tâche 8) l'appelle pour retirer les options
    /// injouables. Aucun cas particulier ailleurs.
    pub fn has_pose(&self, nom: &str) -> bool {
        self.poses.contains_key(nom)
    }

    /// La hitbox effective d'une pose : la sienne si elle en déclare une,
    /// celle du manifeste sinon (spec §8.5).
    ///
    /// Une pose inconnue rend la hitbox du manifeste plutôt que `None` :
    /// l'appelant (le hit-testing, Tâche 11) a toujours besoin d'un
    /// rectangle, et celui du manifeste est le repli sensé.
    pub fn hitbox_de(&self, nom: &str) -> Hitbox {
        match self.pose(nom) {
            Some(p) => p.hitbox.unwrap_or(self.hitbox),
            None => self.hitbox,
        }
    }
}

// Les tests de ce module vivent dans `manifest_tests.rs`
// (sortis d ici le 2026-09-14 : ils faisaient 259 des 693 lignes).
#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
