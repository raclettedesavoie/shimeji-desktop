//! Couche 1 du comportement : les réflexes (spec §7.1).
//!
//! **Ce ne sont pas des décisions, ce sont des conséquences physiques.** Ils
//! ne consultent ni l'envie ni l'intention — ils peuvent en revanche
//! l'annuler. Quand l'un d'eux s'impose, les couches 2 et 3 ne tournent pas
//! du tout dans cette image.
//!
//! Les quatre réflexes de la spec :
//!   · plateforme disparue   → je tombe          ✅ ici
//!   · attrapé à la souris   → je suis porté     ✅ ici
//!   · contact avec le sol   → j'atterris        ✅ ici
//!   · session verrouillée   → je disparais      ⬜ étape 2 (signal)
//!
//! Le garde-fou « tombé sous le bureau » s'y ajoute (spec §6.3). Il n'est pas
//! dans la liste de la spec parce que ce n'est pas un réflexe du personnage,
//! c'est un filet de sécurité du programme — mais sa place est ici, au même
//! rang de priorité.

use super::Entrees;
use crate::character::attach::{
    hors_bornes, position_conservant_le_sprite, world_position, Attachment,
};
use crate::character::manifest::{
    POSES_DRAGGED_LEFT, POSES_DRAGGED_RIGHT, POSE_DRAGGED, POSE_FALL, POSE_GRAB_WALL, POSE_LAND,
    POSE_STAND,
};
use crate::character::physics::{
    borner_lancer, contact, integrer_balancier, integrer_chute, lisser_vitesse_curseur,
    niveau_balancier, sous_le_bureau, Cote,
};
use crate::character::{Character, Facing};
use crate::geom::{Face, Vec2};
use crate::world::World;
use std::time::Duration;

/// Ce qui s'est imposé à cette image. `Aucun` laisse la main aux couches 2
/// et 3.
///
/// Rendu à l'appelant plutôt que gardé pour soi : le mode simulation
/// (Tâche 9) en fait sa trace, et c'est ce qui permet de vérifier « il n'a
/// jamais été bloqué » sans regarder l'écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reflexe {
    Chute,
    Porte,
    Atterrissage,
    /// Il vient de s'accrocher à une paroi verticale — le lancer contre un
    /// mur (design §3.2). Distinct d'`Atterrissage` pour que la trace du mode
    /// simulation puisse les compter séparément.
    Accroche,
    /// Rattrapé par le garde-fou après être tombé sous le bureau.
    Rattrape,
    Aucun,
}

/// Avance le balancier d'un personnage porté, et rend la pose à afficher.
///
/// Reprise fidèle de `Dragged.java` + des conditions de l'action `Pinched` :
/// un point « pied » poursuit le curseur par un ressort amorti, et son
/// **retard** choisit la pose.
///
/// Les trois propriétés qu'on cherchait tombent gratuitement du modèle :
///   · plus la main va vite, plus le retard est grand, plus il balance ;
///   · le retour au repos repasse par les niveaux intermédiaires ;
///   · le ressort étant sous-amorti, il oscille un peu avant de se poser.
///
/// > **Le sens est celui d'un pendule : il traîne derrière.** Curseur vers la
/// > gauche → le pied reste en arrière, donc à droite → la **tête penche à
/// > gauche**. C'est le point qui avait été inversé, et le source le tranche :
/// > la condition `FootX < cursor.x` (pied à gauche, curseur parti à droite)
/// > sélectionne les frames 5, 7, 9.
fn avancer_balancier(ch: &mut Character, curseur: crate::geom::Point, dt: f32) -> &'static str {
    // Le ressort du balancier.
    let (x, vx) = integrer_balancier(ch.portage.pied_x, ch.portage.pied_vx, curseur.x, dt);
    ch.portage.pied_x = x;
    ch.portage.pied_vx = vx;

    // La vitesse lissée du curseur, qui servira à LANCER au relâchement.
    // Tenue à jour ici parce que c'est le seul endroit qui voit passer les
    // positions successives du curseur pendant un portage.
    ch.portage.curseur_v = lisser_vitesse_curseur(
        ch.portage.curseur_v,
        ch.portage.curseur_precedent,
        curseur,
        dt,
    );
    ch.portage.curseur_precedent = curseur;

    let (cote, niveau) = niveau_balancier(ch.portage.pied_x - curseur.x);

    match cote {
        Cote::Aucun => POSE_DRAGGED,
        // `niveau` vaut 1 à 3 hors du cas neutre, d'où le `- 1`.
        Cote::PiedAGauche => POSES_DRAGGED_RIGHT[(niveau - 1) as usize],
        Cote::PiedADroite => POSES_DRAGGED_LEFT[(niveau - 1) as usize],
    }
}

/// Oriente le personnage d'après sa vitesse horizontale de chute.
///
/// Repris de `Fall.java`, qui commence chaque tick par :
///
/// ```java
/// if( this.getVelocityX() != 0 )
///     getMascot().setLookRight( this.getVelocityX() > 0 );
/// ```
///
/// Sans cela, un personnage **lancé vers la droite tombe la tête à gauche** :
/// le sprite de chute est asymétrique, et il partait toujours dans
/// l'orientation qu'il avait avant d'être saisi (forcée à `Left` pendant le
/// portage).
///
/// La **zone morte** est un ajout : `!= 0` sur un flottant est presque
/// toujours vrai, et la vitesse horizontale d'un lâcher « immobile » n'est
/// jamais exactement nulle — elle vient d'une moyenne lissée du curseur. Sans
/// zone morte, un résidu d'un pixel par seconde suffirait à retourner le
/// sprite.
fn orienter_selon_la_chute(ch: &mut Character, vx: f32) {
    /// En px/s. Un lâcher franchement latéral dépasse allègrement ce seuil ;
    /// un résidu de lissage, non.
    const ZONE_MORTE: f32 = 5.0;

    if vx > ZONE_MORTE {
        ch.facing = Facing::Right;
    } else if vx < -ZONE_MORTE {
        ch.facing = Facing::Left;
    }
    // Dans la zone morte : on ne touche à rien. Il garde son orientation, ce
    // qui est le comportement voulu pour une chute verticale.
}

/// Applique les réflexes, dans l'ordre de priorité.
///
/// L'ordre compte, et il n'est pas arbitraire : « porté » passe avant
/// « plateforme disparue » parce qu'un personnage tenu à la main n'a pas de
/// plateforme à perdre. L'inverse le ferait tomber de la main de
/// l'utilisateur si la fenêtre sous lui se fermait.
pub fn appliquer(
    ch: &mut Character,
    world: &World,
    e: &Entrees,
    maintenant: Duration,
    dt: f32,
) -> Reflexe {
    // ── Tenir `pos_connue` à jour ───────────────────────────────────────
    // Avant tout le reste, et à chaque image : c'est de là que partira une
    // chute si la plateforme disparaît. Voir le commentaire du champ — c'est
    // un cache, pas une source de vérité.
    if let Some(p) = world_position(&ch.attachment, world, e.souris) {
        ch.pos_connue = p;
    }

    // ── Réflexe 2 : porté ───────────────────────────────────────────────
    // Traité en premier, voir la note sur l'ordre.
    match ch.attachment {
        Attachment::Dragged => {
            if e.bouton_gauche {
                // Toujours tenu. On ne teste PAS `curseur_sur_le_personnage`
                // ici : pendant un déplacement rapide, le sprite traîne
                // derrière le curseur et sortirait de sa propre hitbox — il
                // se décrocherait tout seul.
                let pose = avancer_balancier(ch, e.souris, dt);
                ch.set_pose(pose, maintenant);
                return Reflexe::Porte;
            }

            // ── Relâché : ON LE LANCE ───────────────────────────────
            //
            // Il ne tombe PAS à la verticale : il part avec la vitesse de la
            // main, exactement comme l'action `Thrown` de Shimeji-ee, qui
            // enchaîne sur `Falling` avec `InitialVX/VY = cursor.dx/dy`.
            //
            // On utilise la vitesse LISSÉE et non `pied_vx` : le ressort
            // oscille autour de la vitesse du curseur, donc `pied_vx` peut
            // être momentanément de signe opposé et produirait un jet à
            // l'envers.
            let elan = borner_lancer(ch.portage.curseur_v);

            // ── Et on convertit la position ─────────────────────────
            //
            // Les poses de portage ont l'ancre sur la tête, `fall` l'a sous
            // les pieds : garder `pos = curseur` ferait **repartir le
            // personnage 120 px plus haut** que là où on le tenait. C'est un
            // saut bien visible, et c'était le cas avant cette conversion.
            let pos = match (ch.manifest.pose(&ch.pose), ch.manifest.pose(POSE_FALL)) {
                (Some(avant), Some(apres)) => position_conservant_le_sprite(
                    e.souris,
                    avant,
                    apres,
                    &ch.manifest,
                    e.echelle_affichage,
                    ch.facing,
                ),
                // L'une des deux poses manque (couverture partielle) : on ne
                // peut rien convertir, on part du curseur. Un saut vaut mieux
                // qu'un personnage qui ne tombe pas.
                _ => e.souris,
            };

            ch.attachment = Attachment::Falling { pos, vel: elan };

            // Dès cette image, et pas à la suivante : sinon la première image
            // de chute s'affiche dans l'orientation du portage (toujours
            // `Left`), ce qui donne un bref clignotement sur un lancer vers la
            // droite.
            orienter_selon_la_chute(ch, elan.x);

            ch.set_pose(POSE_FALL, maintenant);
            ch.intention = None;
            return Reflexe::Chute;
        }

        _ => {
            // Pas encore tenu : le devient-il ?
            if e.bouton_gauche && e.curseur_sur_le_personnage {
                ch.attachment = Attachment::Dragged;

                // Tout l'état de portage repart de zéro, sur le curseur —
                // comme `Dragged.init()` qui fait `setFootX(cursor.x)`. Sans
                // ça, il hériterait du retard ET de la vitesse d'un portage
                // précédent : il balancerait violemment à l'instant de la
                // saisie, et serait lancé au relâchement suivant sans même
                // avoir bougé.
                ch.portage = crate::character::Portage::neuf(e.souris);

                // Shimeji-ee force `setLookRight(false)` pendant tout le
                // portage : le sprite n'est jamais miroité. C'est nécessaire,
                // pas cosmétique — les frames de balancement sont
                // asymétriques, et les miroiter inverserait le sens du
                // balancier par-dessus notre choix.
                ch.facing = Facing::Left;

                ch.set_pose(POSE_DRAGGED, maintenant);
                // Être soulevé annule ce qu'il était en train de faire :
                // reprendre une promenade après avoir été déplacé de deux
                // écrans n'aurait aucun sens.
                ch.intention = None;
                return Reflexe::Porte;
            }
        }
    }

    // ── Réflexe 1 : plateforme disparue ou trop courte ──────────────────
    if hors_bornes(&ch.attachment, world) {
        ch.attachment = Attachment::Falling {
            // Le seul usage de `pos_connue` : `world_position` rendrait
            // `None`, il n'y a plus rien dont dériver la position.
            pos: ch.pos_connue,
            vel: Vec2::zero(),
        };
        ch.set_pose(POSE_FALL, maintenant);
        ch.intention = None;
        return Reflexe::Chute;
    }

    // ── Réflexe 3 : la chute, et son issue ──────────────────────────────
    if let Attachment::Falling { pos, vel } = ch.attachment {
        // L'orientation suit la vitesse horizontale, à chaque image, comme
        // `Fall.java`. Utile au-delà du lancer : un personnage dont la
        // plateforme se dérobe alors qu'il marchait garde le sens de sa
        // marche.
        orienter_selon_la_chute(ch, vel.x);

        // Le garde-fou d'abord : inutile de chercher un atterrissage à
        // 50 000 px sous le bureau.
        if sous_le_bureau(world, pos) {
            // `if let Some(...)` et non `unwrap` : un monde vide ne doit pas
            // faire paniquer. Dans ce cas on le laisse tomber — il n'y a
            // nulle part où le poser, et le monde reviendra.
            if let Some((platform, offset)) = world.nearest_floor(pos) {
                ch.attachment = Attachment::On {
                    platform,
                    face: Face::Top,
                    offset,
                };
                ch.set_pose(POSE_LAND, maintenant);
                ch.intention = None;
                return Reflexe::Rattrape;
            }
        }

        let (nouvelle_pos, nouvelle_vel) = integrer_chute(pos, vel, dt);

        // A-t-on heurté quelque chose pendant ce pas ? `contact` rend la
        // FACE, parce qu'on ne se pose pas sur un mur comme sur un sol
        // (design §3.2, étape 4a).
        if let Some((platform, face, offset)) = contact(world, pos, nouvelle_pos) {
            ch.attachment = Attachment::On { platform, face, offset };

            // `match` explicite plutôt qu'un `if face == Face::Top` : les
            // quatre cas se lisent d'un coup, et le compilateur exigera d'en
            // traiter un cinquième si `Face` en gagnait un.
            return match face {
                Face::Top => {
                    ch.set_pose(POSE_LAND, maintenant);
                    ch.intention = None;
                    Reflexe::Atterrissage
                }

                // Une face `Right` est celle d'un mur GAUCHE d'écran : le
                // personnage se tient à sa droite, donc il regarde à gauche
                // pour faire face à la paroi. Et symétriquement pour `Left`,
                // qui est celle d'un mur DROIT.
                Face::Right | Face::Left => {
                    ch.facing = if face == Face::Right {
                        Facing::Left
                    } else {
                        Facing::Right
                    };
                    ch.set_pose(POSE_GRAB_WALL, maintenant);

                    // ⚠️ Voir le commentaire d'`accroche_au_mur` : sans cette
                    // intention, la règle de sécurité du monde vertical
                    // (Tâche 3, `behavior/mod.rs`) le ferait tomber dès
                    // l'image suivante — jeté contre un mur, il ne tiendrait
                    // qu'une image.
                    ch.intention = Some(
                        crate::behavior::intention::ActiveIntention::accroche_au_mur(maintenant),
                    );
                    Reflexe::Accroche
                }

                // `contact` ne rend jamais `Bottom` : le plafond n'attrape
                // rien (design §3.2, d'après `Fall.java`). On ne panique pas
                // pour autant — on traite comme une chute qui continue.
                Face::Bottom => Reflexe::Chute,
            };
        }

        ch.attachment = Attachment::Falling {
            pos: nouvelle_pos,
            vel: nouvelle_vel,
        };
        ch.set_pose(POSE_FALL, maintenant);
        return Reflexe::Chute;
    }

    // ── Fin de la pose d'atterrissage ───────────────────────────────────
    // `land` n'est pas une intention : c'est la queue d'un réflexe. On la
    // laisse se jouer, puis on rend la main.
    if ch.pose == POSE_LAND {
        if ch.pose_terminee(maintenant) {
            ch.set_pose(POSE_STAND, maintenant);
        } else {
            // Toujours en train d'atterrir : les couches 2 et 3 attendent.
            return Reflexe::Atterrissage;
        }
    }

    Reflexe::Aucun
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::manifest::{Manifest, POSE_FALL, POSE_GRAB_WALL, POSE_LAND, POSE_STAND};
    use crate::geom::Point;
    use crate::probe::fake::FakeProbe;
    use crate::probe::SystemProbe;
    use crate::world::PlatformId;

    const DT: f32 = 1.0 / 60.0;

    fn manifeste() -> Manifest {
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40, 20, 48, 100],
            "poses": {
                "stand":   { "frames": [1] },
                "walk":    { "frames": [2, 3], "frameMs": 120, "loop": true },
                "run":     { "frames": [4, 5], "frameMs": 80,  "loop": true },
                "sit":     { "frames": [6] },
                "fall":    { "frames": [7], "anchor": [64, 64] },
                "land":    { "frames": [8], "frameMs": 150 },
                "dragged":       { "frames": [1],  "anchor": [64, 8] },
                "draggedRight1": { "frames": [5],  "anchor": [64, 8] },
                "draggedRight2": { "frames": [7],  "anchor": [64, 8] },
                "draggedRight3": { "frames": [9],  "anchor": [64, 8] },
                "draggedLeft1":  { "frames": [6],  "anchor": [64, 8] },
                "draggedLeft2":  { "frames": [8],  "anchor": [64, 8] },
                "draggedLeft3":  { "frames": [10], "anchor": [64, 8] }
            }
        }"#;
        serde_json::from_str(json).unwrap()
    }

    fn monde() -> World {
        World::from_screens(&FakeProbe::deux_ecrans().screens())
    }

    fn sol(monde: &World) -> PlatformId {
        monde.platforms()[0].id
    }

    fn perso_pose_sur_le_sol(monde: &World) -> Character {
        Character::new(
            manifeste(),
            Attachment::On {
                platform: sol(monde),
                face: Face::Top,
                offset: 300.0,
            },
            Point::new(300.0, 1032.0),
        )
    }

    fn entrees_neutres() -> Entrees {
        Entrees {
            souris: Point::new(0.0, 0.0),
            echelle_affichage: 1.0,
            bouton_gauche: false,
            curseur_sur_le_personnage: false,
            biais: crate::signals::Biais::neutre(),
            utilisateur_actif: true,
            commande: None,
        }
    }

    #[test]
    fn pose_sur_le_sol_aucun_reflexe_ne_s_impose() {
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);
        assert_eq!(r, Reflexe::Aucun);
    }

    #[test]
    fn plateforme_disparue_il_tombe() {
        // Le réflexe n° 1. On lui donne une plateforme qui n'existe pas —
        // c'est l'équivalent exact d'une fenêtre qu'on ferme (étape 4).
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::On {
            platform: PlatformId(999_999),
            face: Face::Top,
            offset: 300.0,
        };

        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Chute);
        assert!(matches!(ch.attachment, Attachment::Falling { .. }));
        assert_eq!(ch.pose, POSE_FALL);
    }

    #[test]
    fn la_chute_amorcee_part_de_la_derniere_position_connue() {
        // C'est le seul usage légitime de `pos_connue` : sans lui, on ne
        // saurait pas d'où commencer à tomber.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.pos_connue = Point::new(742.0, 1032.0);
        ch.attachment = Attachment::On {
            platform: PlatformId(999_999),
            face: Face::Top,
            offset: 300.0,
        };

        appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        match ch.attachment {
            Attachment::Falling { pos, vel } => {
                assert_eq!(pos.x, 742.0);
                // Elle commence sans vitesse verticale : il ne saute pas, le
                // sol se dérobe.
                assert_eq!(vel.y, 0.0);
            }
            autre => panic!("attendu Falling, obtenu {autre:?}"),
        }
    }

    #[test]
    fn offset_hors_bornes_il_tombe() {
        // La plateforme a rétréci sous lui. Même réflexe, même code.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::On {
            platform: sol(&m),
            face: Face::Top,
            offset: 99_999.0,
        };

        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);
        assert_eq!(r, Reflexe::Chute);
    }

    #[test]
    fn curseur_dans_la_hitbox_et_bouton_enfonce_il_est_attrape() {
        // Le réflexe n° 2.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);

        let e = Entrees {
            souris: Point::new(300.0, 1000.0),
            echelle_affichage: 1.0,
            bouton_gauche: true,
            curseur_sur_le_personnage: true,
            biais: crate::signals::Biais::neutre(),
            utilisateur_actif: true,
            commande: None,
        };

        let r = appliquer(&mut ch, &m, &e, Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Porte);
        assert_eq!(ch.attachment, Attachment::Dragged);
        assert_eq!(ch.pose, crate::character::manifest::POSE_DRAGGED);
    }

    #[test]
    fn porte_immobile_il_pend_droit() {
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;
        // Le pied déjà sur le curseur : c'est l'état « au repos ». Sans cette
        // ligne il partirait de 0, à 800 px du curseur, donc en balancement
        // maximal — ce qui teste autre chose.
        ch.portage.pied_x = 800.0;

        let e = Entrees {
            souris: Point::new(800.0, 300.0),
            echelle_affichage: 1.0,
            bouton_gauche: true,
            curseur_sur_le_personnage: true,
            biais: crate::signals::Biais::neutre(),
            utilisateur_actif: true,
            commande: None,
        };
        appliquer(&mut ch, &m, &e, Duration::ZERO, DT);
        assert_eq!(ch.pose, POSE_DRAGGED);
    }

    /// Porte le personnage et déplace le curseur à `vitesse` px/s pendant
    /// `secondes`, en pas de 1/60 s. Rend la pose atteinte à la fin.
    ///
    /// C'est le seul moyen honnête d'éprouver un ressort : lui donner une
    /// entrée réaliste et regarder son régime, plutôt que de vérifier un
    /// seuil sur une seule image.
    fn glisser(ch: &mut Character, m: &World, vitesse: f32, secondes: f32) -> String {
        let mut x = 800.0f32;
        let mut t = Duration::ZERO;
        let pas = (secondes / DT) as usize;

        for _ in 0..pas {
            x += vitesse * DT;
            let e = Entrees {
                souris: Point::new(x, 300.0),
                echelle_affichage: 1.0,
                bouton_gauche: true,
                curseur_sur_le_personnage: true,
                biais: crate::signals::Biais::neutre(),
                utilisateur_actif: true,
                commande: None,
            };
            appliquer(ch, m, &e, t, DT);
            t += Duration::from_micros(16_667);
        }
        ch.pose.clone()
    }

    #[test]
    fn le_balancier_traine_derriere_le_curseur() {
        // **LE test du sens**, celui qui avait été inversé. Un pendule traîne
        // derrière : curseur vers la GAUCHE → tête à GAUCHE.
        let m = monde();

        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;
        ch.portage.pied_x = 800.0;
        let pose = glisser(&mut ch, &m, -400.0, 0.5);
        assert!(
            POSES_DRAGGED_LEFT.contains(&pose.as_str()),
            "curseur à gauche : attendu une pose tête à gauche, obtenu {pose}"
        );

        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;
        ch.portage.pied_x = 800.0;
        let pose = glisser(&mut ch, &m, 400.0, 0.5);
        assert!(
            POSES_DRAGGED_RIGHT.contains(&pose.as_str()),
            "curseur à droite : attendu une pose tête à droite, obtenu {pose}"
        );
    }

    #[test]
    fn plus_la_main_va_vite_plus_il_balance() {
        // L'amplitude doit suivre la vitesse. En régime permanent le retard
        // vaut `vitesse / 10`, et les seuils sont à 10, 30 et 50 px — donc
        // 150 px/s, 400 px/s et 700 px/s tombent dans trois niveaux distincts.
        let m = monde();

        let niveau = |vitesse: f32| -> usize {
            let mut ch = perso_pose_sur_le_sol(&m);
            ch.attachment = Attachment::Dragged;
            ch.portage.pied_x = 800.0;
            let pose = glisser(&mut ch, &m, vitesse, 1.0);
            POSES_DRAGGED_LEFT
                .iter()
                .position(|p| *p == pose.as_str())
                .map(|i| i + 1)
                .unwrap_or(0)
        };

        let lent = niveau(-150.0);
        let moyen = niveau(-400.0);
        let vif = niveau(-700.0);

        assert!(lent < moyen, "lent {lent} devrait balancer moins que {moyen}");
        assert!(moyen < vif, "moyen {moyen} devrait balancer moins que {vif}");
    }

    #[test]
    fn le_retour_au_repos_passe_par_les_niveaux_intermediaires() {
        // Il ne doit pas SAUTER au repos quand la main s'arrête : le ressort
        // le ramène en repassant par les poses de moindre amplitude.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;
        ch.portage.pied_x = 800.0;

        // On le lance fort, puis on immobilise le curseur.
        let ample = glisser(&mut ch, &m, -700.0, 1.0);
        assert!(POSES_DRAGGED_LEFT.contains(&ample.as_str()));

        // Curseur figé : on collecte les poses traversées.
        let mut vues = std::collections::BTreeSet::new();
        let x_final = 800.0 - 700.0 * 1.0;
        let mut t = Duration::from_secs(1);
        for _ in 0..120 {
            let e = Entrees {
                souris: Point::new(x_final, 300.0),
                echelle_affichage: 1.0,
                bouton_gauche: true,
                curseur_sur_le_personnage: true,
                biais: crate::signals::Biais::neutre(),
                utilisateur_actif: true,
                commande: None,
            };
            appliquer(&mut ch, &m, &e, t, DT);
            vues.insert(ch.pose.clone());
            t += Duration::from_micros(16_667);
        }

        // Il finit au repos…
        assert_eq!(ch.pose, POSE_DRAGGED, "il devrait avoir fini de balancer");
        // …et il est passé par au moins un niveau intermédiaire en chemin,
        // au lieu de sauter directement de l'amplitude maximale au repos.
        assert!(
            vues.len() >= 3,
            "retour trop brusque : seulement {:?}",
            vues
        );
    }

    #[test]
    fn attraper_repart_du_repos_sur_le_curseur() {
        // Sans réinitialisation, il hériterait du retard d'un portage
        // précédent et balancerait violemment à l'instant de la saisie.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.portage.pied_x = -99_999.0;
        ch.portage.pied_vx = 12_345.0;

        let e = Entrees {
            souris: Point::new(640.0, 480.0),
            echelle_affichage: 1.0,
            bouton_gauche: true,
            curseur_sur_le_personnage: true,
            biais: crate::signals::Biais::neutre(),
            utilisateur_actif: true,
            commande: None,
        };
        appliquer(&mut ch, &m, &e, Duration::ZERO, DT);

        assert_eq!(ch.portage.pied_x, 640.0);
        assert_eq!(ch.portage.pied_vx, 0.0);
        assert_eq!(ch.pose, POSE_DRAGGED, "il pend droit au moment de la saisie");
        // Et le sprite n'est pas miroité pendant le portage.
        assert_eq!(ch.facing, crate::character::Facing::Left);
    }

    #[test]
    fn le_bouton_seul_ne_suffit_pas_a_l_attraper() {
        // Sinon tout clic n'importe où sur le bureau l'arracherait.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        let e = Entrees {
            souris: Point::new(50.0, 50.0),
            echelle_affichage: 1.0,
            bouton_gauche: true,
            curseur_sur_le_personnage: false,
            biais: crate::signals::Biais::neutre(),
            utilisateur_actif: true,
            commande: None,
        };
        assert_eq!(
            appliquer(&mut ch, &m, &e, Duration::ZERO, DT),
            Reflexe::Aucun
        );
    }

    #[test]
    fn etre_attrape_annule_l_intention_en_cours() {
        // Les réflexes ne consultent pas l'intention, mais ils l'INVALIDENT :
        // reprendre une promenade après avoir été soulevé n'aurait aucun sens
        // (spec §7.1).
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.intention = Some(crate::behavior::intention::ActiveIntention::nouvelle(
            crate::behavior::intention::Intention::Flaner,
            Duration::ZERO,
        ));

        let e = Entrees {
            souris: Point::new(300.0, 1000.0),
            echelle_affichage: 1.0,
            bouton_gauche: true,
            curseur_sur_le_personnage: true,
            biais: crate::signals::Biais::neutre(),
            utilisateur_actif: true,
            commande: None,
        };
        appliquer(&mut ch, &m, &e, Duration::ZERO, DT);

        assert!(ch.intention.is_none());
    }

    #[test]
    fn relacher_le_bouton_ne_fait_pas_sauter_le_sprite() {
        // **L'invariante, et non une coordonnée.** Les poses de portage ont
        // l'ancre sur la tête et `fall` sous les pieds : garder `pos =
        // curseur` faisait repartir le personnage 120 px plus haut que là où
        // on le tenait, ce qui se voyait très bien.
        //
        // On vérifie donc que le COIN DE LA FENÊTRE est inchangé — c'est la
        // seule vraie position d'écran, et l'assertion reste juste si l'on
        // change une ancre plus tard.
        use crate::character::attach::window_top_left;

        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;
        ch.portage = crate::character::Portage::neuf(Point::new(1200.0, 400.0));
        ch.set_pose(POSE_DRAGGED, Duration::ZERO);

        let coin_avant = window_top_left(
            Point::new(1200.0, 400.0),
            ch.manifest.pose(POSE_DRAGGED).unwrap(),
            &ch.manifest,
            1.0,
            ch.facing,
        );

        let e = Entrees {
            souris: Point::new(1200.0, 400.0),
            echelle_affichage: 1.0,
            bouton_gauche: false,
            curseur_sur_le_personnage: true,
            biais: crate::signals::Biais::neutre(),
            utilisateur_actif: true,
            commande: None,
        };
        let r = appliquer(&mut ch, &m, &e, Duration::ZERO, DT);
        assert_eq!(r, Reflexe::Chute);

        let pos = match ch.attachment {
            Attachment::Falling { pos, .. } => pos,
            autre => panic!("attendu Falling, obtenu {autre:?}"),
        };
        let coin_apres = window_top_left(
            pos,
            ch.manifest.pose(POSE_FALL).unwrap(),
            &ch.manifest,
            1.0,
            ch.facing,
        );

        assert!(
            (coin_apres.x - coin_avant.x).abs() < 0.01
                && (coin_apres.y - coin_avant.y).abs() < 0.01,
            "le sprite a sauté : coin {coin_avant:?} -> {coin_apres:?}"
        );
    }

    #[test]
    fn relacher_apres_un_glisser_le_lance() {
        // **Le lancer.** Il ne doit pas tomber à la verticale après un
        // glisser : il part avec la vitesse de la main, comme l'action
        // `Thrown` de Shimeji-ee.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;
        ch.portage = crate::character::Portage::neuf(Point::new(800.0, 300.0));

        // On le glisse vers la droite pendant une demi-seconde…
        glisser(&mut ch, &m, 600.0, 0.5);

        // …puis on lâche, curseur à la dernière position atteinte.
        let x_final = 800.0 + 600.0 * 0.5;
        let e = Entrees {
            souris: Point::new(x_final, 300.0),
            echelle_affichage: 1.0,
            bouton_gauche: false,
            curseur_sur_le_personnage: true,
            biais: crate::signals::Biais::neutre(),
            utilisateur_actif: true,
            commande: None,
        };
        appliquer(&mut ch, &m, &e, Duration::from_millis(500), DT);

        match ch.attachment {
            Attachment::Falling { vel, .. } => {
                assert!(
                    vel.x > 300.0,
                    "il devrait partir vers la droite avec de l'élan, vx = {}",
                    vel.x
                );
            }
            autre => panic!("attendu Falling, obtenu {autre:?}"),
        }
    }

    #[test]
    fn lance_a_droite_il_tombe_la_tete_a_droite() {
        // Repris de `Fall.java` : l'orientation suit la vitesse horizontale.
        // Sans ça, il partait toujours dans l'orientation du portage (forcée
        // à `Left`), donc un jet vers la droite montrait un sprite de chute
        // tête à gauche.
        let m = monde();

        let lancer = |vitesse: f32| -> crate::character::Facing {
            let mut ch = perso_pose_sur_le_sol(&m);
            ch.attachment = Attachment::Dragged;
            ch.portage = crate::character::Portage::neuf(Point::new(800.0, 300.0));
            glisser(&mut ch, &m, vitesse, 0.5);

            let x_final = 800.0 + vitesse * 0.5;
            let e = Entrees {
                souris: Point::new(x_final, 300.0),
                echelle_affichage: 1.0,
                bouton_gauche: false,
                curseur_sur_le_personnage: true,
                biais: crate::signals::Biais::neutre(),
                utilisateur_actif: true,
                commande: None,
            };
            appliquer(&mut ch, &m, &e, Duration::from_millis(500), DT);
            ch.facing
        };

        assert_eq!(lancer(600.0), crate::character::Facing::Right, "jet à droite");
        assert_eq!(lancer(-600.0), crate::character::Facing::Left, "jet à gauche");
    }

    #[test]
    fn une_chute_verticale_ne_retourne_pas_le_sprite() {
        // La zone morte : un résidu de lissage ne doit pas retourner le
        // sprite. On le fait tomber d'une plateforme disparue en regardant à
        // droite — il doit continuer à regarder à droite.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.facing = crate::character::Facing::Right;
        ch.attachment = Attachment::Falling {
            pos: Point::new(300.0, 500.0),
            vel: Vec2::new(1.0, 0.0), // un pixel par seconde : du bruit
        };

        appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);
        assert_eq!(ch.facing, crate::character::Facing::Right);
    }

    #[test]
    fn relacher_sans_avoir_bouge_le_laisse_tomber_droit() {
        // Le pendant du test précédent : un simple clic-relâche ne doit pas
        // le catapulter.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;
        ch.portage = crate::character::Portage::neuf(Point::new(800.0, 300.0));

        // Curseur immobile pendant une demi-seconde.
        glisser(&mut ch, &m, 0.0, 0.5);

        let e = Entrees {
            souris: Point::new(800.0, 300.0),
            echelle_affichage: 1.0,
            bouton_gauche: false,
            curseur_sur_le_personnage: true,
            biais: crate::signals::Biais::neutre(),
            utilisateur_actif: true,
            commande: None,
        };
        appliquer(&mut ch, &m, &e, Duration::from_millis(500), DT);

        match ch.attachment {
            Attachment::Falling { vel, .. } => {
                assert!(vel.x.abs() < 10.0, "élan parasite vx = {}", vel.x);
                assert!(vel.y.abs() < 10.0, "élan parasite vy = {}", vel.y);
            }
            autre => panic!("attendu Falling, obtenu {autre:?}"),
        }
    }

    #[test]
    fn porte_il_reste_porte_tant_que_le_bouton_est_enfonce() {
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Dragged;

        let e = Entrees {
            souris: Point::new(800.0, 300.0),
            echelle_affichage: 1.0,
            bouton_gauche: true,
            curseur_sur_le_personnage: false, // il a glissé sous le curseur
            biais: crate::signals::Biais::neutre(),
            utilisateur_actif: true,
            commande: None,
        };
        let r = appliquer(&mut ch, &m, &e, Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Porte);
        assert_eq!(ch.attachment, Attachment::Dragged);
    }

    #[test]
    fn en_chute_il_avance_puis_atterrit() {
        // Le réflexe n° 3. On le lâche juste au-dessus du sol et on itère
        // jusqu'à l'atterrissage — au plus 2 secondes de temps simulé.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Falling {
            pos: Point::new(300.0, 900.0),
            vel: Vec2::zero(),
        };

        let mut t = Duration::ZERO;
        let mut atterri = false;
        for _ in 0..120 {
            let r = appliquer(&mut ch, &m, &entrees_neutres(), t, DT);
            if r == Reflexe::Atterrissage {
                atterri = true;
                break;
            }
            assert_eq!(r, Reflexe::Chute, "il devait encore tomber");
            t += Duration::from_micros(16_667);
        }

        assert!(atterri, "il n'a jamais atterri");
        assert_eq!(ch.pose, POSE_LAND);
        match ch.attachment {
            Attachment::On { platform, face, .. } => {
                assert_eq!(platform, sol(&m));
                assert_eq!(face, Face::Top);
            }
            autre => panic!("attendu On, obtenu {autre:?}"),
        }
    }

    #[test]
    fn tomber_sous_le_bureau_le_replace_sur_le_sol_le_plus_proche() {
        // Le garde-fou de la spec §6.3. Lâché très à droite, dans le vide
        // au-delà du second écran.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.attachment = Attachment::Falling {
            pos: Point::new(9_000.0, 50_000.0),
            vel: Vec2::new(0.0, 100.0),
        };

        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Rattrape);
        match ch.attachment {
            Attachment::On {
                platform, offset, ..
            } => {
                // Le sol du second écran, et un offset tenable.
                assert_eq!(m.get(platform).unwrap().rect.left(), 1920.0);
                assert!(offset >= 0.0 && offset <= 1920.0);
            }
            autre => panic!("attendu On, obtenu {autre:?}"),
        }
    }

    #[test]
    fn la_pose_land_finie_il_repasse_a_stand() {
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        // `land` dure 150 ms dans le manifeste de test.
        ch.set_pose(POSE_LAND, Duration::ZERO);

        // Juste avant la fin : il tient sa pose.
        appliquer(&mut ch, &m, &entrees_neutres(), Duration::from_millis(100), DT);
        assert_eq!(ch.pose, POSE_LAND);

        appliquer(&mut ch, &m, &entrees_neutres(), Duration::from_millis(200), DT);
        assert_eq!(ch.pose, POSE_STAND);
    }

    #[test]
    fn pos_connue_est_tenue_a_jour_a_chaque_image() {
        // Sans quoi une chute amorcée partirait d'une position périmée.
        let m = monde();
        let mut ch = perso_pose_sur_le_sol(&m);
        ch.pos_connue = Point::new(0.0, 0.0);

        appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        assert_eq!(ch.pos_connue, Point::new(300.0, 1032.0));
    }

    #[test]
    fn un_personnage_sans_pose_fall_tombe_quand_meme() {
        // Couverture partielle (spec §8.6) : l'absence d'une pose ne doit
        // jamais empêcher un RÉFLEXE. Les réflexes sont non négociables ;
        // c'est le tirage des ENVIES qui se restreint (Tâche 8).
        let json = r#"{
            "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
            "hitbox": [40,20,48,100],
            "poses": { "stand": { "frames": [1] } }
        }"#;
        let m = monde();
        let mut ch = Character::new(
            serde_json::from_str(json).unwrap(),
            Attachment::On {
                platform: PlatformId(999_999),
                face: Face::Top,
                offset: 10.0,
            },
            Point::new(300.0, 500.0),
        );

        let r = appliquer(&mut ch, &m, &entrees_neutres(), Duration::ZERO, DT);

        assert_eq!(r, Reflexe::Chute);
        assert!(matches!(ch.attachment, Attachment::Falling { .. }));
        // La pose demandée n'existe pas : il garde celle qu'il avait plutôt
        // que d'afficher du vide.
        assert_eq!(ch.pose, POSE_STAND);
    }

    // ── Tâche 5, étape 4a : le lancer qui s'accroche ────────────────────
    //
    // Ces trois tests couvrent : le mur qui attrape un lancer, le POINT NON
    // ÉVIDENT de la tâche (sans l'intention posée en phase `Accroche`, la
    // règle de sécurité de la Tâche 3 le ferait tomber dès l'image
    // suivante), et le contre-exemple du coin, où le sol doit gagner.

    /// Un `blob` complet, posé sur le sol — chargé depuis le vrai manifeste
    /// plutôt que le manifeste minimal `manifeste()` d'en haut : celui-ci
    /// n'a pas de pose `grabWall`, or ces tests-ci doivent la vérifier.
    fn perso(m: &World) -> Character {
        // `&…[0]` : `Platform` n'implémente pas `Copy` (voir `world.rs`), on
        // emprunte donc plutôt que de tenter de le sortir du slice — même
        // motif que le `perso` de `behavior/mod.rs`.
        let sol = &m.platforms()[0];
        Character::new(
            Manifest::load(std::path::Path::new("../characters/blob"))
                .expect("le personnage de test doit être lisible"),
            Attachment::On {
                platform: sol.id,
                face: Face::Top,
                offset: 500.0,
            },
            sol.rect.point_on(Face::Top, 500.0),
        )
    }

    #[test]
    fn jete_contre_un_mur_il_s_y_accroche() {
        let m = World::from_screens(&FakeProbe::un_ecran().screens());
        let mut ch = perso(&m);

        // Lancé vers la gauche, à mi-hauteur.
        ch.attachment = Attachment::Falling {
            pos: Point::new(30.0, 400.0),
            vel: crate::geom::Vec2::new(-600.0, 0.0),
        };

        // Quelques images suffisent pour parcourir les 30 px.
        let mut accroche = false;
        for i in 0..20 {
            let r = appliquer(
                &mut ch,
                &m,
                &entrees_neutres(),
                Duration::from_secs_f32(i as f32 * DT),
                DT,
            );
            if r == Reflexe::Accroche {
                accroche = true;
                break;
            }
        }

        assert!(accroche, "il devrait s'accrocher au mur gauche");
        assert!(matches!(
            ch.attachment,
            Attachment::On { face: Face::Right, .. }
        ));
        assert_eq!(ch.pose, POSE_GRAB_WALL);
        assert_eq!(ch.facing, Facing::Left, "il regarde le mur");
    }

    #[test]
    fn accroche_par_un_lancer_il_ne_lache_pas_a_l_image_suivante() {
        // LE test de la tâche. Sans l'intention posée en phase `Accroche`,
        // la couche 2 rend `Finie` et la règle de sécurité le fait tomber :
        // jeté contre un mur, il ne tiendrait qu'une image.
        let m = World::from_screens(&FakeProbe::un_ecran().screens());
        let mut ch = perso(&m);
        ch.attachment = Attachment::Falling {
            pos: Point::new(30.0, 400.0),
            vel: crate::geom::Vec2::new(-600.0, 0.0),
        };

        for i in 0..20 {
            let r = appliquer(
                &mut ch,
                &m,
                &entrees_neutres(),
                Duration::from_secs_f32(i as f32 * DT),
                DT,
            );
            if r == Reflexe::Accroche {
                break;
            }
        }

        assert!(
            ch.intention.is_some(),
            "une intention doit avoir été posée, sinon la règle de sécurité le lâche"
        );

        // Et on le vérifie réellement, en faisant tourner le comportement
        // complet plusieurs images.
        let mut rng = crate::rng::XorShift32::seeded(4);
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
        for i in 0..10 {
            crate::behavior::pas(
                &mut ch,
                &m,
                &entrees_neutres(),
                &crate::behavior::desire::TableEnvies::defaut(),
                &reglages,
                Duration::from_secs_f32(1.0 + i as f32 * DT),
                DT,
                &mut rng,
            );
        }

        assert!(
            matches!(ch.attachment, Attachment::On { face: Face::Right, .. }),
            "il doit tenir le mur, il est {:?}",
            ch.attachment
        );

        // ── La bonne raison, et pas seulement le bon résultat ───────────
        //
        // Bug corrigé (relecture de la Tâche 5) : `PhaseGrimpe::Accroche`
        // n'avait aucune initialisation paresseuse de `jusqu_a`, donc la
        // toute première image sautait directement au tirage
        // lâcher/redescendre — il ne tenait jamais la seconde promise. Ce
        // test passait quand même, mais pour la MAUVAISE raison : un bug
        // séparé dans `contact_mur` (le franchissement large des deux
        // côtés) faisait se raccrocher IMMÉDIATEMENT tout personnage lâché
        // pile sur le plan du mur, ce qui reproduisait accidentellement
        // `Attachment::On`. On vérifie donc maintenant l'ÉTAT INTERNE :
        // l'intention doit être `Grimper`, en phase `Accroche`, avec un
        // `jusqu_a` déjà tiré (donc non nul) — la preuve qu'il tient
        // vraiment le mur pendant sa durée d'accroche, et non qu'il
        // s'est lâché puis instantanément rattrapé.
        let tient_vraiment = matches!(
            ch.intention,
            Some(crate::behavior::intention::ActiveIntention {
                kind: crate::behavior::intention::Intention::Grimper,
                etat: crate::behavior::intention::EtatIntention::Grimpe {
                    phase: crate::behavior::intention::PhaseGrimpe::Accroche,
                    jusqu_a,
                },
                ..
            }) if jusqu_a != Duration::ZERO
        );
        assert!(
            tient_vraiment,
            "il devrait être en train de tenir le mur (Grimper/Accroche, \
             jusqu_a tiré), il est {:?}",
            ch.intention
        );
    }

    #[test]
    fn jete_dans_un_coin_il_atterrit_au_lieu_de_s_accrocher() {
        let m = World::from_screens(&FakeProbe::un_ecran().screens());
        let mut ch = perso(&m);
        ch.attachment = Attachment::Falling {
            pos: Point::new(30.0, 1020.0),
            vel: crate::geom::Vec2::new(-600.0, 400.0),
        };

        for i in 0..20 {
            let r = appliquer(
                &mut ch,
                &m,
                &entrees_neutres(),
                Duration::from_secs_f32(i as f32 * DT),
                DT,
            );
            if r == Reflexe::Atterrissage || r == Reflexe::Accroche {
                break;
            }
        }

        assert!(
            matches!(ch.attachment, Attachment::On { face: Face::Top, .. }),
            "le sol gagne sur le mur"
        );
    }
}
