//! Couche 2 du comportement : l'intention en cours (spec §7.1, §7.3).
//!
//! Responsabilité unique : traduire une intention en déplacement et en pose,
//! et dire quand elle est finie, échouée ou expirée. **Une seule intention à
//! la fois.**
//!
//! **Décision n° 4 — la navigation est autorisée à échouer.** Aucun calcul de
//! chemin : la carte des plateformes change 8 fois par seconde, un chemin est
//! périmé avant d'être parcouru. Décision **locale** à chaque image, et une
//! seule règle de sécurité qui remplace toute l'énumération des cas de
//! blocage :
//!
//! > **Toute intention a un délai d'abandon (~20 s)**, après quoi elle échoue
//! > et il repart flâner.
//!
//! Il se coince parfois, il prend des routes idiotes — **et c'est
//! souhaitable** : un pet qui prend un chemin bête est attachant, un qui
//! calcule l'itinéraire optimal a l'air d'un robot (spec §7.3).

use crate::character::attach::Attachment;
use crate::character::manifest::{
    POSE_RUN, POSE_SIT, POSE_SIT_DANGLE, POSE_SPIN_HEAD, POSE_STAND, POSE_WALK,
};
use crate::config::Reglages;
use crate::character::Character;
use crate::geom::Face;
use crate::rng::Rng;
use crate::world::{PlatformId, World};
use std::time::Duration;

/// Le délai au bout duquel **toute** intention échoue (spec §7.3).
///
/// Uniforme, sans exception — y compris pour `Flaner`, qui pourrait durer
/// indéfiniment. L'exception serait une deuxième règle à retenir, et la
/// flânerie qui expire produit simplement un nouveau tirage : de la variété
/// gratuite.
pub const DELAI_ABANDON: Duration = Duration::from_secs(20);

/// À quoi il joue.
///
/// Un `enum` et non un nom de pose libre : la table d'envies a besoin d'une
/// clé `Copy + Eq`, et une variante par animation permet à la couverture
/// partielle de les retirer **séparément** (spec §8.6).
///
/// C'est la lecture littérale de `Jouer(action)` de la spec §7.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Jeu {
    /// Assis, la tête qui tourne — 8 frames, 200 ms chacune.
    TeteQuiTourne,
    /// Assis à balancer les jambes — 4 frames, 400 ms, en boucle.
    JambesQuiBalancent,
}

impl Jeu {
    /// La pose que ce jeu demande. **Une seule** : c'est ce qui rend le
    /// retrait par couverture partielle exact — une animation absente ne
    /// retire que son propre jeu.
    pub fn pose(&self) -> &'static str {
        match self {
            Jeu::TeteQuiTourne => POSE_SPIN_HEAD,
            Jeu::JambesQuiBalancent => POSE_SIT_DANGLE,
        }
    }

    /// Les poses requises, sous la forme attendue par la table d'envies.
    ///
    /// `&'static [&'static str]` : la table stocke des tranches statiques
    /// pour n'allouer jamais. Une constante par jeu, donc, plutôt qu'un
    /// `Vec` construit à la volée.
    pub fn poses_requises(&self) -> &'static [&'static str] {
        match self {
            Jeu::TeteQuiTourne => &[POSE_SPIN_HEAD],
            Jeu::JambesQuiBalancent => &[POSE_SIT_DANGLE],
        }
    }
}

/// Ce que le personnage est en train d'essayer de faire.
///
/// **Une seule à la fois** (spec §7.1). `AllerA(surface)` arrive à l'étape 5
/// — ce sera une variante de plus, et une ligne de plus dans la table
/// d'envies.
///
/// `PartialOrd, Ord` : uniquement pour que les tests puissent ranger des
/// intentions dans un `BTreeSet` (« quelles intentions ai-je vues ? »). Le
/// comportement lui-même ne compare jamais deux intentions par ordre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Intention {
    Flaner,
    SeReposer,
    Jouer(Jeu),
}

/// À quelle vitesse il se déplace pendant une flânerie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Allure {
    Arret,
    Marche,
    Course,
}

/// La vitesse d'une allure, d'après les réglages de l'utilisateur.
///
/// Fonction libre et non méthode de `Allure` : la vitesse n'est plus une
/// propriété intrinsèque de l'allure, elle dépend de la configuration.
/// Garder la méthode obligerait à donner à `Allure` une référence aux
/// réglages, ce qui n'a pas de sens pour une étiquette.
fn vitesse_de(allure: Allure, reglages: &Reglages) -> f32 {
    match allure {
        Allure::Arret => 0.0,
        Allure::Marche => reglages.vitesse_marche,
        Allure::Course => reglages.vitesse_course,
    }
}

impl Allure {
    fn pose(&self) -> &'static str {
        match self {
            Allure::Arret => POSE_STAND,
            Allure::Marche => POSE_WALK,
            Allure::Course => POSE_RUN,
        }
    }
}

/// L'état interne d'une intention en cours.
///
/// Séparé de `Intention` : celle-ci est une **étiquette** (`Copy`, `Eq`),
/// utilisable comme clé dans la table d'envies, alors que celui-ci porte des
/// données qui changent à chaque image. Fondre les deux obligerait la table
/// d'envies à connaître des durées, ce qui n'a rien à y faire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EtatIntention {
    Flanerie { allure: Allure, jusqu_a: Duration },
    Repos { jusqu_a: Duration },
    Jeu { jusqu_a: Duration },
}

/// Une intention en cours, avec le moment où elle a commencé — c'est de là
/// que se déduit le délai d'abandon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActiveIntention {
    pub kind: Intention,
    pub depuis: Duration,
    pub etat: EtatIntention,
}

impl ActiveIntention {
    /// Crée une intention fraîche.
    ///
    /// L'état initial est délibérément **déjà périmé** (`jusqu_a` à zéro) :
    /// la première image de `poursuivre` tirera donc une allure ou une durée
    /// de repos. Cela évite d'exiger un `Rng` ici — ce qui permet à
    /// `reflex.rs` et aux tests d'en construire une sans générateur.
    pub fn nouvelle(kind: Intention, maintenant: Duration) -> Self {
        let etat = match kind {
            Intention::Flaner => EtatIntention::Flanerie {
                allure: Allure::Arret,
                jusqu_a: Duration::ZERO,
            },
            Intention::SeReposer => EtatIntention::Repos {
                jusqu_a: Duration::ZERO,
            },
            Intention::Jouer(_) => EtatIntention::Jeu {
                jusqu_a: Duration::ZERO,
            },
        };
        ActiveIntention {
            kind,
            depuis: maintenant,
            etat,
        }
    }
}

/// Où en est l'intention à la fin de cette image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    EnCours,
    /// Menée à son terme normalement.
    Finie,
    /// Impossible, ou expirée au délai d'abandon. Dans les deux cas, la
    /// couche 3 en tirera une autre — c'est pourquoi les deux ne sont pas
    /// distinguées plus finement : rien n'en dépend.
    Echouee,
}

/// Fait avancer l'intention en cours d'un pas de temps.
///
/// Rend `Finie` s'il n'y a aucune intention : l'appelant (`behavior::pas`) en
/// tire alors une nouvelle. C'est plus simple qu'un `Option<Issue>`, et ça
/// évite un cas particulier au point d'appel.
pub fn poursuivre(
    ch: &mut Character,
    world: &World,
    reglages: &Reglages,
    maintenant: Duration,
    dt: f32,
    rng: &mut dyn Rng,
) -> Issue {
    // `let Some(...) else` : pas d'intention, rien à poursuivre.
    let Some(mut ai) = ch.intention else {
        return Issue::Finie;
    };

    // ── Le délai d'abandon, avant tout le reste ─────────────────────────
    // Décision n° 4. `saturating_sub` : si l'horloge de test recule (elle
    // le peut, `FakeClock::set` existe), on ne veut pas de débordement.
    if maintenant.saturating_sub(ai.depuis) > DELAI_ABANDON {
        ch.intention = None;
        return Issue::Echouee;
    }

    match ai.kind {
        Intention::Flaner => {
            let issue = flaner(ch, world, reglages, &mut ai, maintenant, dt, rng);
            // On réécrit l'intention : `ai` est une COPIE (le type est
            // `Copy`), donc modifier `ai.etat` ne touche pas `ch.intention`
            // tant qu'on ne le réaffecte pas. Oublier cette ligne donnerait
            // un personnage qui retire une allure à chaque image.
            //
            // `if` : `flaner` a pu annuler l'intention (elle est alors
            // `None`) — la réécrire l'aurait ressuscitée.
            if ch.intention.is_some() {
                ch.intention = Some(ai);
            }
            issue
        }

        Intention::SeReposer => {
            let issue = se_reposer(ch, &mut ai, maintenant, rng);
            if ch.intention.is_some() {
                ch.intention = Some(ai);
            }
            issue
        }

        Intention::Jouer(jeu) => {
            let issue = jouer(ch, jeu, &mut ai, maintenant, rng);
            if ch.intention.is_some() {
                ch.intention = Some(ai);
            }
            issue
        }
    }
}

/// Flâner : avancer, s'arrêter, courir, faire demi-tour, changer d'écran.
fn flaner(
    ch: &mut Character,
    world: &World,
    reglages: &Reglages,
    ai: &mut ActiveIntention,
    maintenant: Duration,
    dt: f32,
    rng: &mut dyn Rng,
) -> Issue {
    // On n'extrait l'état que sous la bonne variante. Une intention `Flaner`
    // avec un état `Repos` serait un bug de construction ; `else` le traite
    // comme un échec plutôt que par un `panic!`.
    let EtatIntention::Flanerie {
        mut allure,
        mut jusqu_a,
    } = ai.etat
    else {
        ch.intention = None;
        return Issue::Echouee;
    };

    // ── Renouveler l'allure quand la précédente a expiré ────────────────
    if maintenant >= jusqu_a {
        // Poids, durées et chance de demi-tour viennent maintenant des
        // réglages (décision n° 5 : « les poids vivent dans la config, on
        // règle son caractère sans recompiler »).
        //
        // Le dosage par défaut — il s'arrête souvent, marche beaucoup, court
        // rarement — est ce qui donne l'impression de flânerie plutôt que
        // d'agitation.
        let a = &reglages.allures;
        let poids = [a.poids_arret, a.poids_marche, a.poids_course];

        allure = match rng.weighted(&poids) {
            Some(0) => Allure::Arret,
            Some(2) => Allure::Course,
            // `Some(1)` tombe sur la marche, et le cas `None` aussi — il
            // n'arrive que si l'utilisateur a mis les trois poids à zéro,
            // auquel cas marcher est le repli le moins surprenant.
            _ => Allure::Marche,
        };

        // Une durée aléatoire, plus courte pour la course par défaut : un pet
        // qui court dix secondes d'affilée a l'air pressé, pas vivant.
        let plage = match allure {
            Allure::Arret => a.duree_arret,
            Allure::Marche => a.duree_marche,
            Allure::Course => a.duree_course,
        };
        jusqu_a = maintenant + Duration::from_secs_f32(rng.range(plage[0], plage[1]));

        // Un demi-tour de temps en temps, sans raison : c'est ce qui empêche
        // de deviner la suite. C'est LE réglage qui décide si le personnage
        // paraît décidé ou indécis.
        if rng.unit_f32() < a.chance_demi_tour {
            ch.facing = ch.facing.inverse();
        }
    }

    // ── La pose suit l'allure ───────────────────────────────────────────
    // `set_pose` ignore une pose absente du manifeste (couverture partielle,
    // spec §8.6) : un personnage sans `run` gardera donc sa pose de marche
    // en courant, plutôt que de n'afficher rien. On demande explicitement
    // `stand` en repli pour que l'arrêt reste visible.
    let pose_voulue = allure.pose();
    if ch.manifest.has_pose(pose_voulue) {
        ch.set_pose(pose_voulue, maintenant);
    } else {
        ch.set_pose(POSE_STAND, maintenant);
    }

    // ── Le déplacement, et ce qui arrive au bord ────────────────────────
    if allure != Allure::Arret {
        avancer(ch, world, vitesse_de(allure, reglages) * ch.facing.signe() * dt);
    }

    ai.etat = EtatIntention::Flanerie { allure, jusqu_a };
    Issue::EnCours
}

/// Avance de `pas` pixels le long de la face courante, et traite le bord.
///
/// **Décision locale, pas de plan** (décision n° 4) : au bord, on regarde
/// s'il existe une face voisine dans la direction du mouvement, et sinon on
/// fait demi-tour. Aucun itinéraire n'est calculé.
fn avancer(ch: &mut Character, world: &World, pas: f32) {
    let Attachment::On {
        platform,
        face,
        offset,
    } = ch.attachment
    else {
        // En chute ou porté : ce n'est pas à l'intention de décider, les
        // réflexes s'en occupent.
        return;
    };

    let Some(plat) = world.get(platform) else {
        // La plateforme a disparu entre les réflexes et ici. Improbable dans
        // une même image, mais on ne suppose rien : les réflexes le verront
        // à l'image suivante.
        return;
    };

    let longueur = plat.rect.face_length(face);
    let nouveau = offset + pas;

    // Toujours dans la face : rien de spécial.
    if nouveau >= 0.0 && nouveau <= longueur {
        ch.attachment = Attachment::On {
            platform,
            face,
            offset: nouveau,
        };
        return;
    }

    // ── Le bord est atteint ─────────────────────────────────────────────
    let vers_la_droite = pas > 0.0;

    // Y a-t-il un sol voisin qui prolonge celui-ci de ce côté ? C'est ce qui
    // fait qu'« il circule sur tous les écrans » (étape 1) — et à l'étape 4,
    // ce sera aussi ce qui le fait passer d'une barre de titre à la suivante.
    if let Some((voisine, offset_entree)) =
        face_voisine(world, platform, plat.rect.top(), vers_la_droite)
    {
        ch.attachment = Attachment::On {
            platform: voisine,
            face: Face::Top,
            offset: offset_entree,
        };
        return;
    }

    // Pas de voisin : demi-tour, et on se recale exactement sur le bord.
    //
    // La spec §6.3 autorise aussi de se laisser tomber ou de s'accrocher à
    // une face voisine. À l'étape 1 il n'y a ni murs ni plafonds exposés, et
    // se laisser tomber du bord de l'écran ne mènerait qu'au garde-fou :
    // le demi-tour est la seule issue qui ait du sens ici. Le tirage entre
    // les trois arrive à l'étape 4.
    ch.facing = ch.facing.inverse();
    ch.attachment = Attachment::On {
        platform,
        face,
        offset: nouveau.clamp(0.0, longueur),
    };
}

/// Cherche un sol adjacent à celui de `depuis`, du côté demandé et à peu près
/// à la même hauteur.
///
/// Vit ici et non dans `world.rs` parce que c'est une **décision de
/// navigation**, pas une propriété du monde : « ce sol en prolonge-t-il un
/// autre ? » n'a de sens que pour quelqu'un qui marche dessus.
///
/// Rend la plateforme voisine et l'offset auquel y entrer.
fn face_voisine(
    world: &World,
    depuis: PlatformId,
    hauteur: f32,
    vers_la_droite: bool,
) -> Option<(PlatformId, f32)> {
    /// Tolérance sur la jonction. Deux écrans côte à côte se touchent
    /// exactement, mais des résolutions ou des échelles différentes peuvent
    /// laisser quelques pixels : on ne veut pas d'un demi-tour inexpliqué
    /// pour 2 px.
    const TOLERANCE: f32 = 8.0;

    let source = world.get(depuis)?;

    for plat in world.platforms() {
        if plat.id == depuis || !plat.has_face(Face::Top) {
            continue;
        }

        // À peu près la même hauteur : on ne veut pas qu'il enjambe le vide
        // vers un sol 400 px plus bas.
        if (plat.rect.top() - hauteur).abs() > TOLERANCE {
            continue;
        }

        if vers_la_droite {
            // Le voisin commence là où celui-ci finit.
            if (plat.rect.left() - source.rect.right()).abs() <= TOLERANCE {
                return Some((plat.id, 0.0));
            }
        } else if (source.rect.left() - plat.rect.right()).abs() <= TOLERANCE {
            // On y entre par son bord droit.
            return Some((plat.id, plat.rect.face_length(Face::Top)));
        }
    }

    None
}

/// Jouer : poser une animation assise et la laisser tourner.
///
/// Plus simple que `se_reposer`, dont il ne partage pas la logique de phases :
/// un jeu n'a pas d'étape. Les deux fonctions restent séparées pour cette
/// raison — les fondre demanderait un paramètre « as-tu des phases ? », qui
/// est le signe d'une mauvaise abstraction.
fn jouer(
    ch: &mut Character,
    jeu: Jeu,
    ai: &mut ActiveIntention,
    maintenant: Duration,
    rng: &mut dyn Rng,
) -> Issue {
    let EtatIntention::Jeu { mut jusqu_a } = ai.etat else {
        ch.intention = None;
        return Issue::Echouee;
    };

    // Défense en profondeur : `desire.rs` filtre déjà sur la pose, mais une
    // config bricolée pourrait proposer ce jeu à un personnage qui n'a pas
    // l'animation — et un personnage posé sur une pose inexistante serait
    // invisible. Mieux vaut échouer et re-tirer.
    if !ch.manifest.has_pose(jeu.pose()) {
        ch.intention = None;
        return Issue::Echouee;
    }

    if jusqu_a == Duration::ZERO {
        // Première image : on tire la durée du jeu.
        //
        // 4 à 15 s, la même plage que le repos : bornée sous
        // `DELAI_ABANDON` pour que l'issue soit `Finie` et non `Echouee`.
        jusqu_a = maintenant + Duration::from_secs_f32(rng.range(4.0, 15.0));
        ai.etat = EtatIntention::Jeu { jusqu_a };
    } else if maintenant >= jusqu_a {
        ch.intention = None;
        return Issue::Finie;
    }

    ch.set_pose(jeu.pose(), maintenant);
    Issue::EnCours
}

/// Se reposer : s'asseoir, et ne rien faire pendant un moment.
///
/// À l'étape 2, l'inactivité prolongée enchaînera sur `sleep` — ce sera une
/// pose de plus et une transition, pas un nouveau chemin de code.
fn se_reposer(
    ch: &mut Character,
    ai: &mut ActiveIntention,
    maintenant: Duration,
    rng: &mut dyn Rng,
) -> Issue {
    let EtatIntention::Repos { mut jusqu_a } = ai.etat else {
        ch.intention = None;
        return Issue::Echouee;
    };

    // Défense en profondeur : le tirage ne devrait jamais proposer cette
    // intention à un personnage sans `sit` (desire.rs le filtre). Mais une
    // config bricolée pourrait y parvenir, et un personnage assis sur une
    // pose inexistante serait invisible — mieux vaut échouer.
    if !ch.manifest.has_pose(POSE_SIT) {
        ch.intention = None;
        return Issue::Echouee;
    }

    if jusqu_a == Duration::ZERO {
        // Première image de l'intention : on tire sa durée.
        //
        // Bornée sous `DELAI_ABANDON` : au-delà, le délai d'abandon
        // couperait le repos avant son terme et l'`Issue` serait `Echouee`
        // au lieu de `Finie`. Rien n'en dépend fonctionnellement, mais la
        // trace du mode simulation serait trompeuse.
        jusqu_a = maintenant + Duration::from_secs_f32(rng.range(4.0, 15.0));
        ai.etat = EtatIntention::Repos { jusqu_a };
    } else if maintenant >= jusqu_a {
        // Le repos est arrivé à son terme.
        ch.intention = None;
        return Issue::Finie;
    }

    ch.set_pose(POSE_SIT, maintenant);
    Issue::EnCours
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::manifest::Manifest;
    use crate::geom::Point;
    use crate::probe::fake::FakeProbe;
    use crate::probe::SystemProbe;
    use crate::rng::XorShift32;

    const DT: f32 = 1.0 / 60.0;

    /// Les réglages par défaut. Construits ici et non lus depuis le disque :
    /// un test qui lirait le `config.json` de la machine ne serait plus
    /// reproductible.
    fn reglages() -> Reglages {
        Reglages::depuis(&crate::config::Config::default())
    }

    // Les deux animations de jeu ajoutées ci-dessous ont des numéros de
    // frame ARBITRAIRES (9, 10, 11) : ce manifeste est un DOUBLE, il ne sert
    // qu'à dire quelles poses existent pour les tests. Les vraies frames de
    // `blob` sont déclarées dans `characters/blob/mascot.json`.
    //
    // ⚠️ Le JSON ne supporte aucun commentaire : contrairement à du Rust
    // normal, `//` à l'intérieur du `r#"..."#` ci-dessous ferait échouer
    // `serde_json` avec un message peu clair (« key must be a string »).
    // D'où ce commentaire ici, en dehors de la chaîne, plutôt qu'au milieu
    // des clés `spinHead` / `sitDangle`.
    fn manifeste() -> Manifest {
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40, 20, 48, 100],
            "poses": {
                "stand": { "frames": [1] },
                "walk":  { "frames": [2, 3], "frameMs": 120, "loop": true },
                "run":   { "frames": [4, 5], "frameMs": 80,  "loop": true },
                "sit":   { "frames": [6] },
                "fall":  { "frames": [7], "anchor": [64, 64] },
                "land":  { "frames": [8], "frameMs": 150 },
                "spinHead":  { "frames": [9, 10], "frameMs": 200 },
                "sitDangle": { "frames": [11], "anchor": [64, 112] }
            }
        }"#;
        serde_json::from_str(json).unwrap()
    }

    fn monde() -> World {
        World::from_screens(&FakeProbe::deux_ecrans().screens())
    }

    fn perso(monde: &World, offset: f32) -> Character {
        Character::new(
            manifeste(),
            Attachment::On {
                platform: monde.platforms()[0].id,
                face: Face::Top,
                offset,
            },
            Point::new(offset, 1032.0),
        )
    }

    fn offset_de(ch: &Character) -> f32 {
        match ch.attachment {
            Attachment::On { offset, .. } => offset,
            autre => panic!("attendu On, obtenu {autre:?}"),
        }
    }

    fn plateforme_de(ch: &Character) -> PlatformId {
        match ch.attachment {
            Attachment::On { platform, .. } => platform,
            autre => panic!("attendu On, obtenu {autre:?}"),
        }
    }

    #[test]
    fn flaner_finit_par_faire_avancer_le_personnage() {
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(3);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Flaner, Duration::ZERO));

        let depart = offset_de(&ch);
        let mut t = Duration::ZERO;
        // 3 secondes : assez pour qu'au moins une allure de marche soit
        // tirée, quelle que soit la graine.
        for _ in 0..180 {
            poursuivre(&mut ch, &m, &reglages(), t, DT, &mut rng);
            t += Duration::from_micros(16_667);
        }

        assert_ne!(offset_de(&ch), depart, "il n'a pas bougé en 3 s");
    }

    #[test]
    fn flaner_alterne_les_allures_et_les_poses() {
        // « jamais figé, jamais prévisible » : sur 30 s, les trois allures
        // doivent avoir été vues.
        let m = monde();
        let mut ch = perso(&m, 900.0);
        let mut rng = XorShift32::seeded(11);
        ch.intention = Some(ActiveIntention::nouvelle(Intention::Flaner, Duration::ZERO));

        let mut vues = std::collections::BTreeSet::new();
        let mut t = Duration::ZERO;
        for _ in 0..1_800 {
            poursuivre(&mut ch, &m, &reglages(), t, DT, &mut rng);
            vues.insert(ch.pose.clone());
            t += Duration::from_micros(16_667);
        }

        assert!(vues.contains(POSE_STAND), "jamais arrêté : {vues:?}");
        assert!(vues.contains(POSE_WALK), "jamais marché : {vues:?}");
        assert!(vues.contains(POSE_RUN), "jamais couru : {vues:?}");
    }

    #[test]
    fn arrive_au_bord_il_fait_demi_tour_plutot_que_de_tomber() {
        // Le sol du premier écran va de 0 à 1920. On le place à 3 px du bord
        // droit, tourné à droite, en marche forcée.
        //
        // NOTE : le second écran du monde de test commence exactement à
        // x = 1920, donc `face_voisine` le trouverait. On prend donc un
        // monde à UN SEUL écran pour éprouver le demi-tour.
        let m = World::from_screens(&FakeProbe::un_ecran().screens());
        let mut ch = Character::new(
            manifeste(),
            Attachment::On {
                platform: m.platforms()[0].id,
                face: Face::Top,
                offset: 1917.0,
            },
            Point::new(1917.0, 1032.0),
        );
        ch.facing = crate::character::Facing::Right;
        let mut rng = XorShift32::seeded(5);
        ch.intention = Some(ActiveIntention {
            kind: Intention::Flaner,
            depuis: Duration::ZERO,
            etat: EtatIntention::Flanerie {
                allure: Allure::Marche,
                jusqu_a: Duration::from_secs(60),
            },
        });

        let mut t = Duration::ZERO;
        for _ in 0..30 {
            poursuivre(&mut ch, &m, &reglages(), t, DT, &mut rng);
            t += Duration::from_micros(16_667);
        }

        // Il est toujours accroché — il n'est pas tombé du bord du monde.
        assert!(matches!(ch.attachment, Attachment::On { .. }));
        // Et il repart vers la gauche.
        assert_eq!(ch.facing, crate::character::Facing::Left);
        assert!(offset_de(&ch) <= 1920.0);
    }

    #[test]
    fn il_passe_sur_l_ecran_voisin_quand_il_y_en_a_un() {
        // « il circule sur tous les écrans » (CLAUDE.md, étape 1). Le sol du
        // premier écran finit à x = 1920, celui du second y commence : les
        // deux faces sont adjointes, il doit enjamber la frontière.
        //
        // On force la marche vers la droite depuis tout près du bord.
        let m = monde();
        let mut ch = perso(&m, 1919.0);
        ch.facing = crate::character::Facing::Right;
        let mut rng = XorShift32::seeded(5);
        ch.intention = Some(ActiveIntention {
            kind: Intention::Flaner,
            depuis: Duration::ZERO,
            etat: EtatIntention::Flanerie {
                allure: Allure::Marche,
                jusqu_a: Duration::from_secs(60),
            },
        });

        let premier = m.platforms()[0].id;
        let mut t = Duration::ZERO;
        let mut passe = false;
        for _ in 0..60 {
            poursuivre(&mut ch, &m, &reglages(), t, DT, &mut rng);
            if plateforme_de(&ch) != premier {
                passe = true;
                break;
            }
            t += Duration::from_micros(16_667);
        }

        assert!(passe, "il n'a pas franchi la frontière entre les écrans");
        assert_eq!(m.get(plateforme_de(&ch)).unwrap().rect.left(), 1920.0);
        // Il entre par le bord gauche du sol voisin, donc à un offset petit.
        assert!(offset_de(&ch) < 50.0);
        // Et il continue dans le même sens : pas de demi-tour parasite.
        assert_eq!(ch.facing, crate::character::Facing::Right);
    }

    #[test]
    fn se_reposer_s_assoit_puis_se_termine() {
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::SeReposer,
            Duration::ZERO,
        ));

        // Première image : il s'assoit.
        let issue = poursuivre(&mut ch, &m, &reglages(), Duration::ZERO, DT, &mut rng);
        assert_eq!(issue, Issue::EnCours);
        assert_eq!(ch.pose, POSE_SIT);

        // Il ne bouge pas pendant le repos.
        let ou = offset_de(&ch);
        poursuivre(&mut ch, &m, &reglages(), Duration::from_secs(2), DT, &mut rng);
        assert_eq!(offset_de(&ch), ou);

        // Le repos dure au plus 15 s ; à 16 s il est fini.
        let issue = poursuivre(&mut ch, &m, &reglages(), Duration::from_secs(16), DT, &mut rng);
        assert_eq!(issue, Issue::Finie);
        assert!(ch.intention.is_none());
    }

    #[test]
    fn toute_intention_expire_au_delai_d_abandon() {
        // **LE test de la décision n° 4.** Une seule règle remplace toute
        // l'énumération des cas de blocage : passé 20 s, l'intention échoue.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);

        for kind in [Intention::Flaner, Intention::SeReposer] {
            ch.intention = Some(ActiveIntention::nouvelle(kind, Duration::ZERO));

            // Juste avant le délai : l'intention n'a pas expiré. Elle peut
            // s'être terminée normalement (un repos dure au plus 15 s), donc
            // on vérifie seulement qu'elle n'a pas ÉCHOUÉ.
            let avant = poursuivre(
                &mut ch,
                &m,
                &reglages(),
                DELAI_ABANDON - Duration::from_millis(100),
                DT,
                &mut rng,
            );
            assert_ne!(avant, Issue::Echouee, "{kind:?} a expiré trop tôt");

            // Juste après : expirée. On réarme l'intention, la ligne
            // précédente ayant pu la consommer.
            ch.intention = Some(ActiveIntention::nouvelle(kind, Duration::ZERO));
            let apres = poursuivre(
                &mut ch,
                &m,
                &reglages(),
                DELAI_ABANDON + Duration::from_millis(100),
                DT,
                &mut rng,
            );
            assert_eq!(apres, Issue::Echouee, "{kind:?} n'a pas expiré");
        }
    }

    #[test]
    fn le_delai_court_depuis_le_debut_de_l_intention_pas_depuis_zero() {
        // Une intention commencée à t = 100 s doit expirer à 120 s, pas
        // immédiatement.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::Flaner,
            Duration::from_secs(100),
        ));

        let issue = poursuivre(&mut ch, &m, &reglages(), Duration::from_secs(110), DT, &mut rng);
        assert_eq!(issue, Issue::EnCours);

        let issue = poursuivre(&mut ch, &m, &reglages(), Duration::from_secs(121), DT, &mut rng);
        assert_eq!(issue, Issue::Echouee);
    }

    #[test]
    fn sans_intention_poursuivre_ne_fait_rien_et_le_dit() {
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        assert_eq!(
            poursuivre(&mut ch, &m, &reglages(), Duration::ZERO, DT, &mut rng),
            Issue::Finie
        );
    }

    #[test]
    fn jouer_pose_l_animation_du_jeu_tire() {
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);

        for (jeu, pose) in [
            (Jeu::TeteQuiTourne, POSE_SPIN_HEAD),
            (Jeu::JambesQuiBalancent, POSE_SIT_DANGLE),
        ] {
            ch.intention = Some(ActiveIntention::nouvelle(
                Intention::Jouer(jeu),
                Duration::ZERO,
            ));
            let issue = poursuivre(&mut ch, &m, &reglages(), Duration::ZERO, DT, &mut rng);
            assert_eq!(issue, Issue::EnCours);
            assert_eq!(ch.pose, pose, "jeu {jeu:?}");
        }
    }

    #[test]
    fn jouer_ne_deplace_pas_le_personnage() {
        // Les deux jeux sont des animations assises : `Velocity="0,0"` dans
        // `actions.xml`. Si le personnage dérivait, c'est qu'une vitesse
        // traîne quelque part.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::Jouer(Jeu::TeteQuiTourne),
            Duration::ZERO,
        ));

        let ou = offset_de(&ch);
        for i in 0..120 {
            poursuivre(
                &mut ch,
                &m,
                &reglages(),
                Duration::from_secs_f32(i as f32 * DT),
                DT,
                &mut rng,
            );
        }
        assert_eq!(offset_de(&ch), ou);
    }

    #[test]
    fn jouer_se_termine_avant_le_delai_d_abandon() {
        // Comme le repos : la durée est bornée sous `DELAI_ABANDON`, sinon
        // l'issue serait `Echouee` au lieu de `Finie` et la trace du mode
        // simulation mentirait.
        let m = monde();
        let mut ch = perso(&m, 500.0);
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::Jouer(Jeu::TeteQuiTourne),
            Duration::ZERO,
        ));

        let mut issue = Issue::EnCours;
        for i in 0..(20 * 60) {
            issue = poursuivre(
                &mut ch,
                &m,
                &reglages(),
                Duration::from_secs_f32(i as f32 * DT),
                DT,
                &mut rng,
            );
            if issue != Issue::EnCours {
                break;
            }
        }
        assert_eq!(issue, Issue::Finie);
    }

    #[test]
    fn jouer_sans_la_pose_echoue_au_lieu_de_figer() {
        // Défense en profondeur, comme `se_reposer_sans_pose_sit_echoue` :
        // le tirage filtre déjà, mais une config bricolée ne doit pas
        // produire un personnage invisible.
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "stand": { "frames": [1] }, "walk": { "frames": [2] } }
        }"#;
        let m = monde();
        let mut ch = Character::new(
            serde_json::from_str(json).unwrap(),
            Attachment::On {
                platform: m.platforms()[0].id,
                face: Face::Top,
                offset: 500.0,
            },
            Point::new(500.0, 1032.0),
        );
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::Jouer(Jeu::TeteQuiTourne),
            Duration::ZERO,
        ));

        assert_eq!(
            poursuivre(&mut ch, &m, &reglages(), Duration::ZERO, DT, &mut rng),
            Issue::Echouee
        );
    }

    #[test]
    fn se_reposer_sans_pose_sit_echoue_au_lieu_de_figer() {
        // Défense en profondeur : le tirage ne devrait jamais proposer
        // `SeReposer` à un personnage sans `sit` (desire.rs). Mais si une
        // config bricolée y parvenait, l'intention doit ÉCHOUER — pas
        // asseoir un personnage sur une pose inexistante.
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "stand": { "frames": [1] }, "walk": { "frames": [2] } }
        }"#;
        let m = monde();
        let mut ch = Character::new(
            serde_json::from_str(json).unwrap(),
            Attachment::On {
                platform: m.platforms()[0].id,
                face: Face::Top,
                offset: 500.0,
            },
            Point::new(500.0, 1032.0),
        );
        let mut rng = XorShift32::seeded(1);
        ch.intention = Some(ActiveIntention::nouvelle(
            Intention::SeReposer,
            Duration::ZERO,
        ));

        assert_eq!(
            poursuivre(&mut ch, &m, &reglages(), Duration::ZERO, DT, &mut rng),
            Issue::Echouee
        );
    }
}
