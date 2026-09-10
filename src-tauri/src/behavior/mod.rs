//! Le comportement, en trois couches qui ne communiquent que vers le bas
//! (décision n° 5, spec §7.1).
//!
//!   1. RÉFLEXES   — non négociables, 60 Hz  → `reflex.rs`
//!   2. INTENTION  — une seule à la fois     → `intention.rs`
//!   3. ENVIE      — tirage pondéré          → `desire.rs`
//!
//! Responsabilité de ce fichier : les types partagés par les trois couches,
//! et l'enchaînement lui-même (`pas`, Tâche 8).

pub mod desire;
pub mod intention;
pub mod reflex;

use crate::geom::Point;

/// Ce que le monde extérieur dit au personnage à cette image.
///
/// Regroupé dans une structure plutôt que passé en trois paramètres : à
/// l'étape 2 s'y ajouteront l'inactivité, l'heure et la batterie, et les
/// signatures des trois couches n'auront pas à changer. C'est le pendant, du
/// côté des entrées, de « ajouter un signal = ajouter une ligne ».
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entrees {
    pub souris: Point,

    pub bouton_gauche: bool,

    /// Le facteur d'échelle d'affichage : celui du moniteur, **multiplié par
    /// le réglage `echelle` de l'utilisateur**.
    ///
    /// Les réflexes en ont besoin pour **une seule chose** : convertir une
    /// position d'une ancre à l'autre quand la pose change au relâchement
    /// d'un portage (`attach::position_conservant_le_sprite`). Les ancres
    /// sont exprimées dans la boîte du sprite, donc la conversion se met à
    /// l'échelle avec lui (spec §3.4).
    ///
    /// C'est le seul endroit où le comportement touche à l'échelle, et c'est
    /// légitime : c'est une donnée de l'environnement, comme la souris.
    pub echelle_affichage: f32,

    /// Le curseur est-il dans la **hitbox de la pose courante** ?
    ///
    /// Calculé par l'appelant (Tâche 11) et non ici : la hitbox dépend de la
    /// pose et de l'échelle de l'écran, que le hit-testing connaît déjà.
    /// Le passer tout cuit garde les réflexes purs et testables sans
    /// manifeste.
    pub curseur_sur_le_personnage: bool,

    /// Les multiplicateurs d'envie du moment (décision n° 3).
    ///
    /// Recalculés à ~2 Hz par `signals::biais_de` et transportés tels quels
    /// jusqu'ici. Le comportement ne voit **jamais** un signal : il ne voit
    /// que des poids déjà multipliés, ce qui rend impossible d'écrire « si
    /// inactif alors dormir ».
    pub biais: crate::signals::Biais,

    /// L'utilisateur vient-il de toucher à quelque chose ?
    ///
    /// Dérivé du même seuil que le biais (`inactiviteSecondes`), mais gardé
    /// à part parce qu'il ne sert pas à la même chose : le biais **pondère un
    /// tirage**, celui-ci **interrompt un sommeil** (Tâche 5) **et conditionne
    /// l'entrée en sommeil** (`intention::se_reposer`, vague de correction
    /// finale — voir l'invariant « phase `Endormi` ⇒ utilisateur absent »).
    /// Deux usages, deux champs — les fondre obligerait à deviner l'un depuis
    /// l'autre.
    pub utilisateur_actif: bool,
}

/// Un pas de comportement : les trois couches, dans l'ordre, une fois.
///
/// C'est la seule fonction que la boucle 60 Hz (Tâche 10) et le mode
/// simulation (Tâche 9) appellent. Les deux partagent donc **exactement** le
/// même comportement — c'est ce qui rend la simulation représentative.
///
/// Rend le réflexe qui s'est éventuellement imposé, pour la trace.
pub fn pas(
    ch: &mut crate::character::Character,
    world: &crate::world::World,
    e: &Entrees,
    table: &desire::TableEnvies,
    reglages: &crate::config::Reglages,
    maintenant: std::time::Duration,
    dt: f32,
    rng: &mut dyn crate::rng::Rng,
) -> reflex::Reflexe {
    // ── Couche 1 : les réflexes ─────────────────────────────────────────
    // S'ils s'imposent, les couches 2 et 3 ne tournent pas du tout dans
    // cette image (spec §7.1).
    let r = reflex::appliquer(ch, world, e, maintenant, dt);
    if r != reflex::Reflexe::Aucun {
        return r;
    }

    // ── L'interruption : un signal ARRÊTE, il ne CHOISIT pas ────────────
    //
    // Ajout à la décision n° 3, documenté dans le design de l'étape 2 §6.
    //
    // Le problème : si le réveil passait par le tirage, il dormirait jusqu'à
    // 20 s après le retour de l'utilisateur — le temps que l'intention
    // expire. Trop lent pour « il se réveille au retour ».
    //
    // La règle : redevenir actif **termine** le sommeil. La couche 3 re-tire
    // juste après, avec des poids redevenus normaux, et il part flâner OU se
    // rasseoir OU jouer. Le signal n'a pas choisi — c'est ce qui distingue
    // une interruption d'un déclenchement.
    //
    // ⚠️ **Depuis la phase `Endormi` SEULEMENT.** Une pause normale se prend
    // pendant que l'utilisateur travaille : appliquer la règle à la position
    // assise empêcherait le personnage de se reposer tant qu'on touche au
    // clavier, c'est-à-dire au seul moment où on le regarde. Le sommeil, lui,
    // n'est atteint que parce qu'un signal a poussé le biais au-dessus du
    // seuil — donc, en pratique, parce que l'utilisateur était parti.
    if e.utilisateur_actif {
        // `matches!` : on ne veut lire que la phase, sans démonter toute
        // l'intention ni la reconstruire.
        let dort = matches!(
            ch.intention,
            Some(intention::ActiveIntention {
                etat: intention::EtatIntention::Repos {
                    phase: intention::PhaseRepos::Endormi,
                    ..
                },
                ..
            })
        );
        if dort {
            // On efface l'intention et on NE POSE AUCUNE POSE : la couche 3,
            // juste en dessous, va tirer la suite et c'est elle qui décidera
            // de la pose. Poser `stand` ici serait précisément « choisir ».
            ch.intention = None;
        }
    }

    // ── Couche 2 : poursuivre l'intention en cours ──────────────────────
    match intention::poursuivre(ch, world, e, reglages, maintenant, dt, rng) {
        intention::Issue::EnCours => return r,
        // Finie ou échouée : on passe à la couche 3.
        intention::Issue::Finie | intention::Issue::Echouee => {}
    }

    // ── Couche 3 : tirer une nouvelle envie ─────────────────────────────
    //
    // **La ligne que l'étape 1a avait écrite pour ce moment.** `tirer_avec`
    // existait déjà, avec son test (`un_multiplicateur_biaise_sans_commander`) :
    // brancher les signaux ne touche donc ni `desire.rs`, ni `intention.rs`,
    // ni `reflex.rs`. C'est la décision n° 5 qui se paie ici.
    if let Some(kind) = table.tirer_avec(&ch.manifest, rng, |i| e.biais.pour(i)) {
        ch.intention = Some(intention::ActiveIntention::nouvelle(kind, maintenant));
    }
    // `None` = aucune intention jouable (personnage très incomplet). On ne
    // fait rien : il reste dans sa pose, et on réessaiera à l'image
    // suivante. Ce n'est pas une erreur.

    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::attach::Attachment;
    use crate::character::manifest::Manifest;
    use crate::character::manifest::{POSE_SIT, POSE_SLEEP};
    use crate::character::Character;
    use crate::geom::{Face, Point};
    use crate::probe::SystemProbe;
    use crate::rng::XorShift32;
    use crate::world::World;
    use std::time::Duration;

    const DT: f32 = 1.0 / 60.0;

    fn monde() -> World {
        World::from_screens(&crate::probe::fake::FakeProbe::un_ecran().screens())
    }

    /// Un `blob` complet, posé sur le sol.
    fn perso(m: &World) -> Character {
        // `&…[0]` : `Platform` n'implémente pas `Copy`, on emprunte donc au
        // lieu d'essayer de sortir la valeur du slice (comme partout
        // ailleurs dans le projet, voir `world.rs` ou `intention.rs`).
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

    fn entrees(actif: bool, biais_repos: f32) -> Entrees {
        Entrees {
            souris: Point::new(0.0, 0.0),
            echelle_affichage: 1.0,
            bouton_gauche: false,
            curseur_sur_le_personnage: false,
            biais: crate::signals::Biais {
                flaner: 1.0,
                se_reposer: biais_repos,
                jouer: 1.0,
            },
            utilisateur_actif: actif,
        }
    }

    #[test]
    fn redevenir_actif_reveille_le_personnage_endormi() {
        let m = monde();
        let mut ch = perso(&m);
        let mut rng = XorShift32::seeded(1);
        let table = desire::TableEnvies::defaut();
        let r = crate::config::Reglages::depuis(&crate::config::Config::default());

        // Il dort.
        ch.set_pose(POSE_SLEEP, Duration::ZERO);
        ch.intention = Some(intention::ActiveIntention {
            kind: intention::Intention::SeReposer,
            depuis: Duration::ZERO,
            etat: intention::EtatIntention::Repos {
                phase: intention::PhaseRepos::Endormi,
                jusqu_a: Duration::from_secs(60),
            },
        });

        // L'utilisateur revient.
        //
        // DEUX images, et c'est structurel : la couche 3 ne fait que TIRER une
        // intention ; c'est `poursuivre`, à l'image suivante, qui applique la
        // pose. Après une seule image, la pose est encore `sleep` même quand
        // le réveil a parfaitement fonctionné.
        //
        // Deux images = 33 ms. La promesse « il se réveille au retour » est
        // intacte : ce qui compte est qu'il ne dorme plus au battement
        // suivant du 2 Hz, pas à la microseconde.
        let e = entrees(true, 1.0);
        pas(&mut ch, &m, &e, &table, &r, Duration::from_secs(10), DT, &mut rng);
        pas(
            &mut ch,
            &m,
            &e,
            &table,
            &r,
            Duration::from_secs(10) + Duration::from_secs_f32(DT),
            DT,
            &mut rng,
        );

        // L'intention de repos ne doit plus être là. Elle a pu être remplacée
        // dans la même image par un nouveau tirage — c'est le fonctionnement
        // voulu — donc on vérifie qu'il ne DORT plus, pas que l'intention est
        // vide.
        assert_ne!(
            ch.pose, POSE_SLEEP,
            "il dort encore alors que l'utilisateur est revenu"
        );
    }

    #[test]
    fn le_reveil_ne_choisit_pas_la_suite() {
        // **LE test de la frontière.** Le signal ARRÊTE le sommeil ; il ne
        // décide pas de ce qui vient après. Sur 200 réveils, on doit voir
        // PLUSIEURS suites différentes — sinon c'est un déclenchement
        // déguisé, et la décision n° 3 est perdue.
        let m = monde();
        let table = desire::TableEnvies::defaut();
        let r = crate::config::Reglages::depuis(&crate::config::Config::default());
        let mut rng = XorShift32::seeded(4);
        let e = entrees(true, 1.0);

        let mut suites = std::collections::BTreeSet::new();
        for i in 0..200 {
            let mut ch = perso(&m);
            ch.set_pose(POSE_SLEEP, Duration::ZERO);
            ch.intention = Some(intention::ActiveIntention {
                kind: intention::Intention::SeReposer,
                depuis: Duration::ZERO,
                etat: intention::EtatIntention::Repos {
                    phase: intention::PhaseRepos::Endormi,
                    jusqu_a: Duration::from_secs(60),
                },
            });

            let t = Duration::from_secs(10 + i);
            pas(&mut ch, &m, &e, &table, &r, t, DT, &mut rng);

            if let Some(ai) = ch.intention {
                suites.insert(ai.kind);
            }
        }

        assert!(
            suites.len() >= 2,
            "le réveil mène toujours à la même chose ({suites:?}) : c'est un \
             déclenchement, pas une interruption"
        );
    }

    #[test]
    fn etre_actif_n_empeche_pas_de_s_asseoir() {
        // **LE piège, et la raison d'être de la phase.** Une pause normale se
        // prend PENDANT qu'on travaille. Si « actif → termine le repos »
        // s'appliquait à la position assise, le personnage ne s'assiérait
        // plus jamais tant qu'on touche au clavier — c'est-à-dire au seul
        // moment où on le regarde.
        let m = monde();
        let mut ch = perso(&m);
        let mut rng = XorShift32::seeded(1);
        let table = desire::TableEnvies::defaut();
        let r = crate::config::Reglages::depuis(&crate::config::Config::default());

        ch.set_pose(POSE_SIT, Duration::ZERO);
        ch.intention = Some(intention::ActiveIntention {
            kind: intention::Intention::SeReposer,
            depuis: Duration::ZERO,
            etat: intention::EtatIntention::Repos {
                phase: intention::PhaseRepos::Assis,
                jusqu_a: Duration::from_secs(10),
            },
        });

        let e = entrees(true, 1.0);
        for i in 0..60 {
            let t = Duration::from_secs_f32(1.0 + i as f32 * DT);
            pas(&mut ch, &m, &e, &table, &r, t, DT, &mut rng);
        }

        assert_eq!(
            ch.pose, POSE_SIT,
            "il a été interrompu alors qu'il était seulement assis"
        );
    }

    #[test]
    fn il_continue_de_dormir_tant_que_personne_ne_revient() {
        let m = monde();
        let mut ch = perso(&m);
        let mut rng = XorShift32::seeded(1);
        let table = desire::TableEnvies::defaut();
        let r = crate::config::Reglages::depuis(&crate::config::Config::default());

        ch.set_pose(POSE_SLEEP, Duration::ZERO);
        ch.intention = Some(intention::ActiveIntention {
            kind: intention::Intention::SeReposer,
            depuis: Duration::ZERO,
            etat: intention::EtatIntention::Repos {
                phase: intention::PhaseRepos::Endormi,
                jusqu_a: Duration::from_secs(60),
            },
        });

        // Toujours parti, et un biais qui pousse au sommeil.
        let e = entrees(false, 8.0);
        for i in 0..(10 * 60) {
            let t = Duration::from_secs_f32(1.0 + i as f32 * DT);
            pas(&mut ch, &m, &e, &table, &r, t, DT, &mut rng);
        }

        assert_eq!(ch.pose, POSE_SLEEP, "il s'est réveillé tout seul");
    }
}
