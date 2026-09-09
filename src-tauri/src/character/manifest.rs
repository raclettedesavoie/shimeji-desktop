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

/// Porté, au repos : il pend droit. Frame 1.
pub const POSE_DRAGGED: &str = "dragged";

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    static COMPTEUR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    /// Écrit un mascot.json et les PNG demandés dans un dossier temporaire,
    /// et rend son chemin.
    ///
    /// Les PNG sont des fichiers **vides** : `load` ne vérifie que leur
    /// EXISTENCE, pas leur contenu. Décoder l'image serait le travail du
    /// webview, et l'exiger ici rendrait le test lent pour rien.
    fn dossier_de_test(json: &str, frames_presentes: &[u32]) -> std::path::PathBuf {
        // `std::env::temp_dir()` plus un nom unique : pas de dépendance à une
        // crate de fichiers temporaires pour douze tests.
        let base = std::env::temp_dir().join(format!(
            "shimeji-test-{}-{}",
            std::process::id(),
            // Un compteur croissant : deux appels dans le même test ne
            // doivent pas se marcher dessus.
            COMPTEUR.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(base.join("img")).unwrap();
        std::fs::write(base.join("mascot.json"), json).unwrap();
        for n in frames_presentes {
            std::fs::write(base.join("img").join(format!("shime{n}.png")), b"").unwrap();
        }
        base
    }

    const MINIMAL: &str = r#"{
        "id": "t",
        "name": "Test",
        "frameSize": [128, 128],
        "scale": 1,
        "hitbox": [40, 20, 48, 100],
        "poses": {
            "stand": { "frames": [1] },
            "walk":  { "frames": [2, 3], "frameMs": 120, "loop": true }
        }
    }"#;

    #[test]
    fn charge_un_manifeste_correct() {
        let d = dossier_de_test(MINIMAL, &[1, 2, 3]);
        let m = Manifest::load(&d).expect("manifeste valide");

        assert_eq!(m.id, "t");
        assert_eq!(m.frame_size, [128, 128]);
        assert!(m.has_pose(POSE_STAND));
        assert!(m.has_pose(POSE_WALK));
    }

    #[test]
    fn les_defauts_s_appliquent_aux_champs_absents() {
        let d = dossier_de_test(MINIMAL, &[1, 2, 3]);
        let m = Manifest::load(&d).unwrap();

        let stand = m.pose(POSE_STAND).unwrap();
        // `frameMs` absent → 150 (spec §8.5)
        assert_eq!(stand.frame_ms, 150);
        // `loop` absent → false
        assert!(!stand.looping);
        // `anchor` absent → [64, 128], le sol tout en bas de la boîte
        assert_eq!(stand.anchor, [64.0, 128.0]);
        // `hitbox` de pose absente → celle du manifeste
        assert_eq!(m.hitbox_de(POSE_STAND), m.hitbox);
    }

    #[test]
    fn une_pose_dont_les_images_manquent_est_retiree_sans_erreur() {
        // LE test de la couverture partielle (spec §8.6). `walk` demande les
        // frames 2 et 3 ; on ne fournit que la 1. Le chargement doit
        // RÉUSSIR, et `walk` doit avoir disparu.
        let d = dossier_de_test(MINIMAL, &[1]);
        let m = Manifest::load(&d).expect("le chargement doit réussir malgré tout");

        assert!(m.has_pose(POSE_STAND));
        assert!(!m.has_pose(POSE_WALK), "walk devait être retirée");
    }

    #[test]
    fn une_pose_partiellement_couverte_est_retiree_entierement() {
        // La frame 2 est là, la 3 non. On ne joue pas une animation trouée :
        // c'est tout ou rien par pose.
        let d = dossier_de_test(MINIMAL, &[1, 2]);
        let m = Manifest::load(&d).unwrap();
        assert!(!m.has_pose(POSE_WALK));
    }

    #[test]
    fn un_manifeste_sans_aucune_pose_jouable_est_une_erreur() {
        // La limite de la tolérance : un personnage dont AUCUNE pose n'a
        // d'image n'est pas un personnage à couverture partielle, c'est un
        // dossier vide. Là, il faut le dire.
        let d = dossier_de_test(MINIMAL, &[]);
        match Manifest::load(&d) {
            Err(ManifestError::AucunePoseJouable) => {}
            autre => panic!("attendu AucunePoseJouable, obtenu {autre:?}"),
        }
    }

    #[test]
    fn un_manifeste_avec_bom_se_charge() {
        // Même piège que pour `config.json`, et plus probable encore : le
        // manifeste est fait pour être édité à la main.
        let avec_bom = format!("{}{}", '\u{feff}', MINIMAL);
        let d = dossier_de_test(&avec_bom, &[1, 2, 3]);
        let m = Manifest::load(&d).expect("un BOM ne doit pas empêcher le chargement");
        assert_eq!(m.id, "t");
    }

    #[test]
    fn un_json_malforme_donne_une_erreur_explicite_sans_paniquer() {
        // Spec §10.1 : « Manifeste malformé → erreur explicite, aucune
        // panique. »
        let d = dossier_de_test("{ ceci n'est pas du JSON", &[1]);
        match Manifest::load(&d) {
            Err(ManifestError::JsonInvalide(msg)) => {
                assert!(!msg.is_empty(), "le message doit dire où ça casse");
            }
            autre => panic!("attendu JsonInvalide, obtenu {autre:?}"),
        }
    }

    #[test]
    fn un_mascot_json_absent_donne_une_erreur_explicite() {
        let d = std::env::temp_dir().join("shimeji-test-dossier-inexistant-xyz");
        match Manifest::load(&d) {
            Err(ManifestError::FichierIllisible { .. }) => {}
            autre => panic!("attendu FichierIllisible, obtenu {autre:?}"),
        }
    }

    #[test]
    fn une_pose_sans_frame_est_rejetee_a_la_validation() {
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "stand": { "frames": [1] }, "vide": { "frames": [] } }
        }"#;
        let d = dossier_de_test(json, &[1]);
        let m = Manifest::load(&d).unwrap();
        // Une pose à zéro frame ne peut rien afficher : retirée comme une
        // pose sans images, pour la même raison.
        assert!(!m.has_pose("vide"));
        assert!(m.has_pose(POSE_STAND));
    }

    #[test]
    fn frame_a_avance_dans_une_animation_en_boucle() {
        let d = dossier_de_test(MINIMAL, &[1, 2, 3]);
        let m = Manifest::load(&d).unwrap();
        let walk = m.pose(POSE_WALK).unwrap();

        // frames [2, 3], 120 ms chacune, en boucle
        assert_eq!(walk.frame_a(Duration::from_millis(0)), 2);
        assert_eq!(walk.frame_a(Duration::from_millis(119)), 2);
        assert_eq!(walk.frame_a(Duration::from_millis(120)), 3);
        // 240 ms : retour au début
        assert_eq!(walk.frame_a(Duration::from_millis(240)), 2);
        assert_eq!(walk.frame_a(Duration::from_millis(361)), 3);
    }

    #[test]
    fn frame_a_tient_la_derniere_image_quand_loop_est_faux() {
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "seq": { "frames": [1, 2, 3], "frameMs": 100 } }
        }"#;
        let d = dossier_de_test(json, &[1, 2, 3]);
        let m = Manifest::load(&d).unwrap();
        let seq = m.pose("seq").unwrap();

        assert_eq!(seq.frame_a(Duration::from_millis(0)), 1);
        assert_eq!(seq.frame_a(Duration::from_millis(250)), 3);
        // Au-delà de la séquence : on TIENT la dernière, on ne reboucle pas.
        assert_eq!(seq.frame_a(Duration::from_secs(10)), 3);
    }

    #[test]
    fn hold_choisit_l_image_tenue_a_la_fin() {
        // C'est ce qui permet de RESTER endormi après la séquence
        // d'endormissement (spec §8.5). Ici, la séquence finit sur 3 mais
        // c'est 2 qui doit être tenue.
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "seq": { "frames": [1, 2, 3], "frameMs": 100, "hold": 2 } }
        }"#;
        let d = dossier_de_test(json, &[1, 2, 3]);
        let m = Manifest::load(&d).unwrap();
        let seq = m.pose("seq").unwrap();

        assert_eq!(seq.frame_a(Duration::from_millis(250)), 3);
        assert_eq!(seq.frame_a(Duration::from_secs(10)), 2);
    }

    #[test]
    fn le_manifeste_reel_de_blob_se_charge() {
        // Le seul test qui touche au dépôt : il vérifie que le mascot.json
        // est cohérent avec les 46 PNG réellement présents.
        // Un chemin relatif depuis src-tauri/, où cargo exécute les tests.
        let d = std::path::Path::new("../characters/blob");
        let m = Manifest::load(d).expect("blob doit se charger");

        assert_eq!(m.id, "blob");
        // blob possède TOUTES les poses (spec §8.7) : aucune ne doit avoir
        // été retirée. Si ce test échoue, c'est qu'un numéro de frame du
        // mascot.json ne correspond à aucun fichier.
        for pose in [
            POSE_STAND, POSE_WALK, POSE_RUN, POSE_SIT, POSE_FALL, POSE_LAND, POSE_DRAGGED,
        ] {
            assert!(m.has_pose(pose), "pose manquante : {pose}");
        }
    }
}
