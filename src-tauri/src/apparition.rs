//! Où naît un personnage qui vient d'être activé
//! (design `2026-09-15-plusieurs-personnages` §5).
//!
//! Responsabilité unique, et **fonction pure** : elle rend un `Attachment`,
//! et c'est tout. Rien de l'animation de chute n'est écrit ici — les
//! réflexes à 60 Hz gèrent déjà chute et atterrissage depuis l'étape 4a.
//! **C'est la décision n° 1 qui rend ce module presque vide.**

use crate::character::attach::Attachment;
use crate::geom::{Face, Point, Vec2};
use crate::rng::{Rng, XorShift32};
use crate::world::{Platform, PlatformKind, World};

/// Le point d'où tombe un personnage qui apparaît.
///
/// Rend `None` quand il n'y a aucun écran — cas réel d'une session distante
/// en cours d'établissement, et non une erreur.
///
/// ⚠️ **Le `rng` est emprunté, jamais recréé.** Semer un `XorShift32` par
/// personnage avec son index biaiserait le premier tirage de chacun vers les
/// bits de poids faible : tous apparaîtraient au même endroit, et une mesure
/// qui chercherait à le démentir tomberait dans le même piège (voir « Semer
/// l'aléatoire une seule fois » dans CLAUDE.md).
pub fn point_de_chute(
    monde: &World,
    largeur_sprite: f32,
    rng: &mut XorShift32,
) -> Option<Attachment> {
    // On part du SOL d'un écran plutôt que de l'écran lui-même : c'est le
    // rectangle qui porte déjà la zone de travail, donc celui qui sait où
    // s'arrête la barre des tâches (piège Windows n° 3).
    //
    // `matches!` et non `==` : `PlatformKind` ne dérive pas `PartialEq`.
    let sols: Vec<&Platform> = monde
        .platforms()
        .iter()
        .filter(|p| matches!(p.kind, PlatformKind::Screen) && p.has_face(Face::Top))
        .collect();

    // `let … else` : aucun écran, rien à faire. Équivalent d'un `match` dont
    // la branche vide ferait `return None`.
    let Some(sol) = tirer_un(&sols, rng) else {
        return None;
    };

    // Le haut de la zone de travail se lit sur le PLAFOND du même écran :
    // `World::from_screens` le pose en `z.top() - EPAISSEUR`, donc son
    // `rect.bottom()` vaut exactement `z.top()`.
    //
    // `?` : un écran sans plafond n'existe pas — `from_screens` en pousse
    // toujours un —, mais on préfère ne rien rendre à inventer un `y`.
    let plafond = monde
        .platforms()
        .iter()
        .find(|p| p.id.meme_ecran(sol.id) && p.has_face(Face::Bottom))?;
    let y = plafond.rect.bottom();

    // Une demi-largeur de marge de chaque côté : sans elle, il apparaîtrait
    // à moitié hors champ sur les bords.
    let demi = largeur_sprite / 2.0;
    let gauche = sol.rect.left() + demi;
    let droite = sol.rect.right() - demi;

    // Un écran plus étroit que le sprite : on le pose au milieu plutôt que
    // de tirer dans un intervalle vide. `range` rendrait déjà le minimum sur
    // des bornes inversées, mais l'écrire dit POURQUOI c'est le milieu et
    // non le bord gauche.
    let x = if droite <= gauche {
        (sol.rect.left() + sol.rect.right()) / 2.0
    } else {
        rng.range(gauche, droite)
    };

    Some(Attachment::Falling {
        pos: Point::new(x, y),
        // Vitesse nulle : la gravité des réflexes suffit à l'accélérer, et
        // une vitesse initiale le ferait apparaître en train de filer.
        //
        // ⚠️ Elle vaut aussi comme **garantie** : `contact_plafond` ne
        // s'accroche qu'en MONTANT (`apres.y < avant.y`). Naître sans
        // vitesse, juste sous un plafond qui expose sa face `Bottom`, est
        // donc sûr par construction — et le test
        // `il_ne_s_agrippe_pas_au_plafond_en_apparaissant` le verrouille.
        vel: Vec2::new(0.0, 0.0),
    })
}

/// Un élément au hasard dans une tranche, ou `None` si elle est vide.
///
/// `range` rend un `f32` dans `[min, max]` **bornes comprises** : le `.min()`
/// final rend l'index hors bornes impossible plutôt qu'improbable — et c'est
/// le genre de bug qu'on ne reproduit jamais.
fn tirer_un<'a>(elements: &[&'a Platform], rng: &mut XorShift32) -> Option<&'a Platform> {
    if elements.is_empty() {
        return None;
    }
    let i = (rng.range(0.0, elements.len() as f32) as usize).min(elements.len() - 1);
    Some(elements[i])
}

// Les tests de ce module vivent dans `apparition_tests.rs`.
#[cfg(test)]
#[path = "apparition_tests.rs"]
mod tests;
