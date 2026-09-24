//! Qui occupe quelle place, et où en trouver une libre (spec « ne pas se
//! superposer », 2026-09-24).
//!
//! Responsabilité unique : des questions sur les places — fonctions pures,
//! ni Tauri, ni écran, ni boucle. Ce que le personnage en FAIT (se décaler,
//! faire la file) est dans `behavior::pas_parmi` et `intention::grimper`.
//!
//! **La règle : on se traverse toujours, on ne s'arrête jamais l'un sur
//! l'autre.** Au sol seulement : sur les murs et au plafond, personne
//! n'occupe rien — seule l'entrée sur un mur se fait un par un.

use super::intention::{Allure, EtatIntention, Intention, PhaseGrimpe};
use crate::character::attach::Attachment;
use crate::character::manifest::POSE_STAND;
use crate::character::Character;
use crate::geom::Face;
use crate::world::PlatformId;
use std::time::Duration;

/// Un personnage posé quelque part, tel que les autres le voient.
///
/// La boucle en dresse la liste une fois par image (`occupant_de`), pour
/// TOUS les personnages posés, en mouvement compris : chaque question
/// ci-dessous filtre ce qui la concerne.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Occupant {
    pub acteur: u32,
    pub platform: PlatformId,
    pub face: Face,
    /// La distance au bord le long de la face (décision n° 1).
    pub offset: f32,
    pub demi_largeur: f32,
    /// `Some` s'il est à l'arrêt au sol — voir `Character::arrete_depuis`.
    pub arrete_depuis: Option<Duration>,
    /// `Some(mur)` s'il fait la file au pied de ce mur.
    pub attend_le_mur: Option<PlatformId>,
    /// `Some(mur)` s'il se dirige vers ce mur pour y grimper — en marche OU
    /// en file. C'est sur lui que se règle la priorité : sans cela, partis
    /// ensemble d'un même point, tous arrivaient au mur libre à la même
    /// image et s'y accrochaient d'un coup.
    pub vise_le_mur: Option<PlatformId>,
}

/// Ce qu'un personnage sait des autres à cette image : qui il est, et la
/// liste des occupants (dont lui-même, qu'il ignore par son numéro).
///
/// `'a` : la liste est EMPRUNTÉE à la boucle, pour une image — le
/// voisinage ne vit pas plus longtemps qu'elle.
#[derive(Debug, Clone, Copy)]
pub struct Voisinage<'a> {
    pub moi: u32,
    pub autres: &'a [Occupant],
}

impl Voisinage<'static> {
    /// Seul au monde : ce que reçoivent la simulation et les tests
    /// existants, par `behavior::pas` et `intention::poursuivre`.
    pub fn seul() -> Voisinage<'static> {
        Voisinage { moi: u32::MAX, autres: &[] }
    }
}

impl<'a> Voisinage<'a> {
    /// Les autres — lui-même exclu. `impl Iterator` : un itérateur dont on
    /// ne nomme pas le type exact (il est long et sans intérêt).
    fn autres(&self) -> impl Iterator<Item = &'a Occupant> + '_ {
        let moi = self.moi;
        self.autres.iter().filter(move |o| o.acteur != moi)
    }
}

/// La demi-largeur et la hauteur de son corps, à cette échelle.
///
/// La hitbox de `stand` et non de la pose courante : la largeur ne doit pas
/// changer quand il s'assoit ou s'endort, sinon la rangée se réarrangerait
/// à chaque changement de pose.
pub fn corps(ch: &Character, echelle: f32) -> (f32, f32) {
    let h = ch.manifest.hitbox_de(POSE_STAND);
    (h.w * echelle / 2.0, h.h * echelle)
}

/// Est-il à l'arrêt ? Tout ce qui ne le déplace pas : se reposer (toutes
/// phases), jouer, flâner en allure `Arret`, et attendre dans la file d'un
/// mur. Le « au sol » est vérifié par l'appelant.
pub fn a_l_arret(ch: &Character) -> bool {
    let Some(ai) = ch.intention else {
        return false;
    };
    match (ai.kind, ai.etat) {
        (Intention::SeReposer, _) | (Intention::Jouer(_), _) => true,
        (Intention::Flaner, EtatIntention::Flanerie { allure, .. }) => allure == Allure::Arret,
        (Intention::Grimper, _) => attend_le_mur(ch).is_some(),
        _ => false,
    }
}

/// Le mur dont il fait la file, s'il en fait une.
pub fn attend_le_mur(ch: &Character) -> Option<PlatformId> {
    match ch.intention?.etat {
        EtatIntention::Grimpe {
            phase: PhaseGrimpe::Rejoindre { mur, attend_depuis: Some(_), .. },
            ..
        } => Some(mur),
        _ => None,
    }
}

/// Le mur vers lequel il marche pour y grimper (phase `Rejoindre`), qu'il
/// soit en route ou en file.
pub fn vise_le_mur(ch: &Character) -> Option<PlatformId> {
    match ch.intention?.etat {
        EtatIntention::Grimpe { phase: PhaseGrimpe::Rejoindre { mur, .. }, .. } => Some(mur),
        _ => None,
    }
}

/// L'occupant qu'il est, s'il est posé quelque part.
pub fn occupant_de(acteur: u32, ch: &Character, echelle: f32) -> Option<Occupant> {
    let Attachment::On { platform, face, offset } = ch.attachment else {
        return None;
    };
    let (demi_largeur, _) = corps(ch, echelle);
    Some(Occupant {
        acteur,
        platform,
        face,
        offset,
        demi_largeur,
        arrete_depuis: ch.arrete_depuis,
        attend_le_mur: attend_le_mur(ch),
        vise_le_mur: vise_le_mur(ch),
    })
}

/// Les occupants à l'arrêt sur cette face de sol, lui-même exclu.
fn arretes_sur<'a>(v: &'a Voisinage, platform: PlatformId) -> impl Iterator<Item = &'a Occupant> + 'a {
    v.autres()
        .filter(move |o| o.platform == platform && o.face == Face::Top && o.arrete_depuis.is_some())
}

/// Deux places se chevauchent-elles ? **Strictement** : deux places qui se
/// touchent (côte à côte) ne se chevauchent pas. Le `- 0.01` absorbe les
/// arrondis des flottants.
fn chevauche(a: f32, demi_a: f32, b: f32, demi_b: f32) -> bool {
    (a - b).abs() < demi_a + demi_b - 0.01
}

/// La place libre la plus proche de `offset` sur ce sol, pour un corps de
/// demi-largeur `demi`, ou `None` s'il n'y en a aucune.
///
/// Les candidats sont `offset` lui-même, puis les deux bords de chaque place
/// prise (collés à elle, côte à côte), rabattus dans le sol. Le plus proche
/// qui ne chevauche personne gagne. Aucune recherche plus fine : une place
/// libre est forcément collée à un occupant, ou là où il est déjà.
pub fn place_libre(v: &Voisinage, platform: PlatformId, offset: f32, demi: f32, longueur: f32) -> Option<f32> {
    place_libre_parmi(v, platform, offset, demi, longueur, |_| true)
}

/// Sa place dans la file de `mur` : la place libre la plus proche du pied
/// (`pied`, sur le sol `sol`), en ne comptant, parmi ceux qui visent ce mur,
/// que ceux qui sont DEVANT lui — plus près du pied que `mon_ecart`, ou à
/// égalité avec un plus petit numéro (le même ordre que
/// `premier_de_la_file`). Les autres occupants à l'arrêt (un assis au pied
/// du mur) comptent toujours.
///
/// Sans ce départage (relecture finale), deux personnages arrivés à la même
/// image voyaient chacun l'autre arrêté, visaient la place d'à côté, se
/// voyaient en marche l'image suivante, revenaient — et tremblotaient sans
/// fin au même endroit.
pub fn place_dans_la_file(
    v: &Voisinage,
    mur: PlatformId,
    sol: PlatformId,
    pied: f32,
    demi: f32,
    longueur_sol: f32,
    mon_ecart: f32,
) -> Option<f32> {
    let moi = v.moi;
    place_libre_parmi(v, sol, pied, demi, longueur_sol, |o| {
        if o.vise_le_mur != Some(mur) {
            return true;
        }
        let son_ecart = (o.offset - pied).abs();
        son_ecart < mon_ecart || (son_ecart == mon_ecart && o.acteur < moi)
    })
}

/// `place_libre`, en ne comptant que les occupants que `compte` retient.
///
/// `impl Fn(&Occupant) -> bool` : n'importe quelle fonction ou fermeture qui
/// dit oui ou non pour un occupant — le filtre que chaque appelant fournit.
fn place_libre_parmi(
    v: &Voisinage,
    platform: PlatformId,
    offset: f32,
    demi: f32,
    longueur: f32,
    compte: impl Fn(&Occupant) -> bool,
) -> Option<f32> {
    // Un sol trop court pour lui : aucune place.
    if longueur < 2.0 * demi {
        return None;
    }
    let dans_le_sol = |x: f32| x.clamp(demi, longueur - demi);
    // `collect` une fois : la liste filtrée sert deux fois ci-dessous.
    let pris: Vec<&Occupant> = arretes_sur(v, platform).filter(|o| compte(o)).collect();

    let mut candidats = vec![dans_le_sol(offset)];
    for o in &pris {
        candidats.push(dans_le_sol(o.offset - o.demi_largeur - demi));
        candidats.push(dans_le_sol(o.offset + o.demi_largeur + demi));
    }

    let mut meilleur: Option<f32> = None;
    for c in candidats {
        let libre = pris.iter().all(|o| !chevauche(c, demi, o.offset, o.demi_largeur));
        if !libre {
            continue;
        }
        let mieux = match meilleur {
            None => true,
            Some(m) => (c - offset).abs() < (m - offset).abs(),
        };
        if mieux {
            meilleur = Some(c);
        }
    }
    meilleur
}

/// Doit-il céder sa place ? Oui si un occupant PLUS ANCIEN, à l'arrêt sur
/// le même sol, la chevauche. Plus ancien = arrêté plus tôt ; à égalité, le
/// plus petit numéro (déterministe, sans hasard).
pub fn doit_ceder(
    v: &Voisinage,
    platform: PlatformId,
    offset: f32,
    demi: f32,
    arrete_depuis: Option<Duration>,
) -> bool {
    let Some(moi_depuis) = arrete_depuis else {
        return false;
    };
    arretes_sur(v, platform).any(|o| {
        let lui_depuis = o.arrete_depuis.unwrap_or(Duration::ZERO);
        let plus_ancien = lui_depuis < moi_depuis || (lui_depuis == moi_depuis && o.acteur < v.moi);
        plus_ancien && chevauche(offset, demi, o.offset, o.demi_largeur)
    })
}

/// Le bas de ce mur est-il libre ? Non si quelqu'un s'y tient à moins d'une
/// hauteur de corps du bas — en mouvement compris : c'est la zone de départ,
/// la seule contrainte verticale (spec §4). L'offset d'un mur compte vers le
/// BAS depuis le haut : le bas est à `longueur_mur`.
pub fn bas_du_mur_libre(v: &Voisinage, mur: PlatformId, longueur_mur: f32, hauteur: f32) -> bool {
    !v.autres().any(|o| o.platform == mur && o.offset > longueur_mur - hauteur)
}

/// Est-il le premier de la file de ce mur ? Oui si personne ne VISE ce mur
/// plus près du pied (`pied`, offset sur le sol) que lui (`mon_ecart`) —
/// en file ou encore en marche. Sans cela, quand le mur se libère, toute la
/// file se ruerait vers lui ; et des personnages partis ensemble s'y
/// accrocheraient à la même image.
pub fn premier_de_la_file(v: &Voisinage, mur: PlatformId, pied: f32, mon_ecart: f32) -> bool {
    !v.autres().any(|o| {
        o.vise_le_mur == Some(mur) && {
            let son_ecart = (o.offset - pied).abs();
            son_ecart < mon_ecart || (son_ecart == mon_ecart && o.acteur < v.moi)
        }
    })
}

#[cfg(test)]
#[path = "place_tests.rs"]
mod tests;
