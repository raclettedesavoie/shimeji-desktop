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
    /// tirage**, celui-ci **interrompt un sommeil** (Tâche 5). Deux usages,
    /// deux champs — les fondre obligerait à deviner l'un depuis l'autre.
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
