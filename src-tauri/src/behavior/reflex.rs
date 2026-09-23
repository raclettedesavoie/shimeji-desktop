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
    POSES_DRAGGED_LEFT, POSES_DRAGGED_RIGHT, POSE_DRAGGED, POSE_FALL, POSE_GRAB_CEILING,
    POSE_GRAB_WALL, POSE_LAND, POSE_SPRAWL, POSE_STAND,
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
            //
            // Les ancres étant désormais lues PAR IMAGE, la conversion a
            // besoin des deux numéros de frames : celle affichée à l'instant
            // du lâcher, et la première de la chute.
            let pos = match (
                ch.manifest.has_pose(&ch.pose),
                ch.manifest.premiere_frame(POSE_FALL),
            ) {
                (true, Some(frame_apres)) => position_conservant_le_sprite(
                    e.souris,
                    ch.frame_courante(maintenant),
                    &ch.pose,
                    frame_apres,
                    POSE_FALL,
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
            // `match` explicite plutôt qu'un `if face == Face::Top` : les
            // quatre cas se lisent d'un coup, et le compilateur exigera d'en
            // traiter un cinquième si `Face` en gagnait un.
            //
            // ⚠️ **`ch.attachment` n'est PLUS affecté avant ce `match`**
            // (correction de la relecture finale). Une version antérieure
            // l'écrivait ici, avant même de savoir quelle face avait été
            // touchée — un bug qui aurait mordu dès que `Face::Bottom`
            // aurait attaché sans poser la bonne pose. Chaque bras attache
            // donc lui-même, explicitement — y compris `Face::Bottom`
            // depuis que le plafond accroche (design §3.2, révisé le
            // 2026-09-12) : il n'y a plus de bras muet dans ce `match`.
            return match face {
                Face::Top => {
                    ch.attachment = Attachment::On { platform, face, offset };
                    ch.set_pose(POSE_LAND, maintenant);
                    ch.intention = None;
                    Reflexe::Atterrissage
                }

                // Une face `Right` est celle d'un mur GAUCHE d'écran : le
                // personnage se tient à sa droite, donc il regarde à gauche
                // pour faire face à la paroi. Et symétriquement pour `Left`,
                // qui est celle d'un mur DROIT.
                Face::Right | Face::Left => {
                    ch.attachment = Attachment::On { platform, face, offset };
                    ch.facing = if face == Face::Right {
                        Facing::Left
                    } else {
                        Facing::Right
                    };
                    ch.set_pose(POSE_GRAB_WALL, maintenant);

                    // ⚠️ Voir le commentaire d'`ActiveIntention::accroche` :
                    // sans cette intention, la règle de sécurité du monde
                    // vertical (Tâche 3, `behavior/mod.rs`) le ferait tomber
                    // dès l'image suivante — jeté contre un mur, il ne
                    // tiendrait qu'une image.
                    ch.intention =
                        Some(crate::behavior::intention::ActiveIntention::accroche(maintenant));
                    Reflexe::Accroche
                }

                // Un lancer vers le haut s'accroche désormais au plafond —
                // `contact` peut rendre `Bottom` depuis la révision du
                // 2026-09-12 de la design §3.2 (divergence assumée de
                // `Fall.java`, voir le commentaire de `contact`).
                //
                // L'orientation suit la vitesse HORIZONTALE, comme au sol :
                // « regarder la surface » n'a pas de sens au plafond, on
                // réutilise donc `orienter_selon_la_chute` plutôt que
                // d'inventer une seconde règle. On lui passe `nouvelle_vel.x`
                // — la vitesse la plus fraîche à l'instant du contact — et
                // non le `vel.x` d'avant ce pas, qui date d'une image.
                Face::Bottom => {
                    ch.attachment = Attachment::On { platform, face, offset };
                    orienter_selon_la_chute(ch, nouvelle_vel.x);
                    ch.set_pose(POSE_GRAB_CEILING, maintenant);

                    // Même raison que pour un mur : sans cette intention, la
                    // règle de sécurité du monde vertical le ferait tomber
                    // dès l'image suivante — accroché au plafond, il ne
                    // tiendrait qu'une image.
                    ch.intention =
                        Some(crate::behavior::intention::ActiveIntention::accroche(maintenant));
                    Reflexe::Accroche
                }
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
            // ── Puis il reste étalé un moment (2026-09-23) ──────────────
            //
            // Après TOUTE chute — lancer, lâcher d'un mur, apparition — et
            // non seulement après un lancer : c'est le choix de l'auteur.
            //
            // Le réflexe ne fait que POSER l'intention : la durée est tirée
            // par la couche 2, qui seule a l'aléatoire et les réglages (voir
            // `PhaseRepos::Etale`). C'est le même partage que pour
            // `accroche`, plus haut dans ce fichier.
            //
            // Sans `sprawl`, il se relève tout de suite, comme avant :
            // couverture partielle (spec §8.6), aucun cas particulier.
            if ch.manifest.has_pose(POSE_SPRAWL) {
                ch.set_pose(POSE_SPRAWL, maintenant);
                ch.intention =
                    Some(crate::behavior::intention::ActiveIntention::etale(maintenant));
            } else {
                ch.set_pose(POSE_STAND, maintenant);
            }
        } else {
            // Toujours en train d'atterrir : les couches 2 et 3 attendent.
            return Reflexe::Atterrissage;
        }
    }

    Reflexe::Aucun
}

// Les tests de ce module vivent dans `reflex_tests.rs`
// (sortis d ici le 2026-09-14 : ils faisaient 1038 des 1418 lignes).
#[cfg(test)]
#[path = "reflex_tests.rs"]
mod tests;
