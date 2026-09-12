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

use crate::character::attach::Attachment;
use crate::character::Character;
use crate::geom::{Face, Point, Vec2};
use crate::world::World;

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

    /// Ce que l'utilisateur vient de **demander** par le menu contextuel.
    ///
    /// `Some(c)` agit cette image-ci, sans passer par le tirage — que `c`
    /// soit une intention ordinaire (`menu_perso::Commande::Intention`) ou
    /// l'une des trois actions propres au menu de l'escalade (« Rester
    /// accroché », « Redescendre », « Se lâcher »). Rempli par la boucle
    /// 60 Hz depuis la boîte aux lettres du menu, et remis à `None` l'image
    /// suivante : c'est une impulsion, pas un état.
    ///
    /// # Pourquoi ce n'est PAS une entorse à la décision n° 3
    ///
    /// « Les signaux biaisent, ils ne commandent pas » parle des signaux
    /// **système** — inactivité, heure, batterie. La raison en est qu'un
    /// signal qui déclencherait ferait du personnage un afficheur d'état
    /// déguisé, prévisible et mort en trois jours.
    ///
    /// Un clic de l'utilisateur n'a rien de commun avec ça : c'est une
    /// **interaction**, et le besoin la veut directe — « cliquer dessus pour
    /// le faire réagir ». Un menu dont l'entrée « S'asseoir » ne ferait
    /// qu'augmenter une probabilité serait un menu cassé, pas un menu subtil.
    ///
    /// La marge est préservée autrement : le délai d'abandon de 20 s
    /// s'applique à l'intention forcée comme à toute autre, donc il obéit
    /// puis **reprend sa vie tout seul**. On commande un instant, jamais
    /// durablement.
    pub commande: Option<crate::menu_perso::Commande>,
}

/// **La règle « accroché à une face non-`Top` sans raison d'y être, il
/// lâche »**, factorisée en une seule fonction (Tâche 7, étape 4a).
///
/// Rend `true` si elle a agi (le personnage tombe désormais), `false`
/// sinon — pour que l'appelant sache s'il doit rendre la main tout de suite
/// (une chute qui démarre est un réflexe, pas une décision, et les couches
/// 2/3 ne doivent pas tourner par-dessus).
///
/// ⚠️ **Pourquoi une fonction et non des copies du même `if` — la leçon qui
/// a coûté un bug, puis un second.** Cette règle existait déjà, mais à un
/// SEUL des endroits où elle doit s'appliquer : la garde de chaque image,
/// ci-dessous dans `pas`. Elle manquait à `intention::poursuivre` — et une
/// première correction (Tâche 7) ne l'y avait ajoutée qu'à UN des deux
/// points de sortie possibles : le délai d'abandon. Un personnage qui
/// expirait au plafond restait donc accroché, intention `None` — et la
/// couche 3, juste après, lui reposait aussitôt un `Grimper` neuf, en phase
/// `Choisir`, sans que la garde de `pas` ne le voie jamais : elle ne
/// s'exécute qu'AVANT que l'intention ne soit effacée, donc à l'image de
/// l'abandon comme à la suivante elle voit toujours une intention `Grimper`
/// valide.
///
/// **Et ce correctif partiel a repoussé exactement le même bug d'un cran**
/// (relecture finale de l'étape 4a) : `grimper()` a sept points de sortie,
/// et deux d'entre eux — la garde de face de la phase `Choisir`, et
/// l'absence de sol au pied du mur — laissaient eux aussi le personnage
/// accroché sans que le délai d'abandon n'y soit pour rien. `poursuivre`
/// appelle maintenant cette fonction à un unique point d'étranglement, APRÈS
/// avoir calculé l'issue de l'intention, quelle qu'en soit la source — voir
/// son commentaire. Deux copies (ou deux appels à des endroits différents
/// pour des raisons différentes) de cette même règle finissent toujours par
/// diverger : c'est exactement ce qu'illustrent ces deux bugs successifs, où
/// la règle vivait au bon endroit pour l'usage courant mais pas pour celui-ci.
/// Il n'y en a maintenant qu'une, appelée aux deux endroits qui comptent : à
/// chaque image dans `pas`, et une fois par appel dans `poursuivre`.
///
/// Ne se prononce que sur `Attachment::On` : `Falling` et `Dragged` ne sont
/// pas concernés — on ne lâche pas ce qu'on ne tient pas.
pub(crate) fn lacher_si_accroche(ch: &mut Character, world: &World) -> bool {
    // `if let … = ch.attachment` : `Attachment` est `Copy` (voir son
    // en-tête dans `attach.rs`), donc cette lecture en prend une COPIE — on
    // peut réassigner `ch.attachment` dans le corps sans conflit d'emprunt.
    if let Attachment::On { platform, face, offset } = ch.attachment {
        if face != Face::Top {
            // On repart du rectangle COURANT pour savoir d'où il tombe
            // (décision n° 1). `if let Some(…)` : si la plateforme a
            // disparu dans le même souffle, le Réflexe 1 s'en occupera à
            // l'image suivante — il n'y a rien à faire ici.
            if let Some(plat) = world.get(platform) {
                ch.attachment = Attachment::Falling {
                    pos: plat.rect.point_on(face, offset),
                    vel: Vec2::zero(),
                };
                return true;
            }
        }
    }
    false
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

    // ── La commande de l'utilisateur : elle, elle CHOISIT ───────────────
    //
    // Avant l'interruption et avant les couches 2 et 3 : un clic sur
    // « S'asseoir » doit l'asseoir, pas augmenter ses chances de s'asseoir.
    // Le long commentaire du champ `Entrees::commande` dit pourquoi ce n'est
    // pas une entorse à la décision n° 3 — en deux mots, un signal système
    // biaise, une interaction commande.
    //
    // **Après les réflexes, en revanche.** Un personnage qu'on tient à la
    // souris ou qui tombe ne doit pas se mettre à jouer en plein vol : les
    // réflexes sont « non négociables » (décision n° 5), et ils le restent
    // pour le menu comme pour tout le reste. En pratique le cas ne se
    // présente guère — ouvrir le menu demande un clic droit, pas un
    // glisser — mais l'ordre des blocs suffit à le rendre impossible.
    if let Some(cmd) = e.commande {
        match cmd {
            crate::menu_perso::Commande::Intention(voulue) => {
                // ── Le garde-fou de l'endroit (bug rapporté à l'écran) ──
                //
                // Une commande de SOL (`Flaner`, `SeReposer`, `Jouer`)
                // arrivée alors qu'il est encore accroché à un mur ou au
                // plafond doit être IGNORÉE, et surtout pas posée : la poser
                // ferait basculer `sans_escalade` à `true` juste plus bas
                // (son intention ne serait plus `Grimper`), et la règle de
                // sécurité du monde vertical le ferait tomber — exactement
                // le symptôme constaté à l'écran. Le menu ne propose plus ces
                // entrées hors du sol (voir `menu_perso::ouvrir`), mais le
                // clic et cette image ne sont pas le même instant : un
                // rechargement, ou simplement le temps qu'a mis l'utilisateur
                // à choisir dans le menu, peuvent l'avoir fait changer
                // d'endroit entre-temps.
                //
                // `Grimper` est EXEMPTÉ de cette garde : c'est justement la
                // commande qui REPREND une escalade en cours (voir le point
                // 1 du bug — la phase `Choisir` ne fait plus échouer
                // l'intention sur une face non-`Top`, elle y reprend
                // l'escalade), et c'est un cas où être sur une paroi est
                // attendu, pas une incohérence.
                let exige_le_sol = voulue != intention::Intention::Grimper;
                let sur_une_paroi = matches!(
                    ch.attachment,
                    crate::character::attach::Attachment::On { face, .. } if face != Face::Top
                );

                if exige_le_sol && sur_une_paroi {
                    // Ignorée : ni pose d'intention, ni retour anticipé — on
                    // continue plus bas comme si la commande n'était jamais
                    // arrivée.
                } else if table.jouable(&ch.manifest, voulue) {
                    // `jouable` : entre le clic et cette image, un
                    // rechargement à chaud a pu remplacer le manifeste par un
                    // pack plus pauvre. Forcer une intention dont la pose
                    // manque figerait le personnage sur une image absente —
                    // la couverture partielle (spec §8.6) vaut ici aussi.
                    ch.intention = Some(intention::ActiveIntention::nouvelle(voulue, maintenant));

                    // On rend la main tout de suite : l'intention neuve sera
                    // poursuivie à l'image suivante. La poursuivre ici aussi
                    // ne casserait rien, mais ferait avancer de deux images
                    // en une.
                    return r;
                }
            }

            crate::menu_perso::Commande::ResterAccroche => {
                // Même garde-fou que ci-dessus, dans l'autre sens : cette
                // commande n'a de sens que sur une paroi (mur ou plafond).
                // Le menu ne la propose qu'à cet endroit, mais un
                // rechargement ou un décrochage entre-temps peut l'avoir
                // fait changer — l'ignorer plutôt que de poser une intention
                // d'accroche pendant qu'il est au sol.
                if matches!(
                    ch.attachment,
                    crate::character::attach::Attachment::On { face, .. } if face != Face::Top
                ) {
                    // `ActiveIntention::accroche` : l'intention que pose
                    // déjà un lancer contre une paroi. Elle ne connaît pas la
                    // face — elle relit `ch.attachment` à l'exécution — donc
                    // elle marche pareillement sur un mur ou au plafond.
                    ch.intention = Some(intention::ActiveIntention::accroche(maintenant));
                    return r;
                }
            }

            crate::menu_perso::Commande::Redescendre => {
                // Ne vaut que sur un MUR (`Left`/`Right`) : au plafond,
                // « redescendre » n'a pas de sens (voir `menu_perso::Commande`
                // et le commentaire de `ouvrir`).
                if matches!(
                    ch.attachment,
                    crate::character::attach::Attachment::On {
                        face: Face::Left | Face::Right,
                        ..
                    }
                ) {
                    ch.intention = Some(intention::ActiveIntention::redescendre(maintenant));
                    return r;
                }
            }

            crate::menu_perso::Commande::SeLacher => {
                // **Le point élégant de cette commande** : on efface
                // simplement l'intention, sans y ajouter la moindre ligne de
                // physique. On ne `return` PAS ici — contrairement aux trois
                // cas ci-dessus — précisément pour que le flot continue vers
                // la « règle de sécurité du monde vertical » juste plus bas :
                // elle voit alors une intention `None` sur une face non-`Top`
                // et fait tomber le personnage TOUTE SEULE, dans cette même
                // image. C'est très exactement `FallFromWall` /
                // `FallFromCeiling` de Shimeji-ee, obtenu en réutilisant une
                // règle qui existe déjà pour un tout autre usage plutôt qu'en
                // écrivant une seconde chute.
                //
                // Au sol, cette commande ne fait rien de dangereux non plus :
                // la même règle de sécurité, plus bas, ne se déclenche que
                // sur une face non-`Top` (elle rend `false` sinon), donc
                // l'effacement se contente ici de laisser la couche 3
                // re-tirer une intention neuve — inoffensif.
                ch.intention = None;
            }
        }
    }

    // ── La règle de sécurité du monde vertical ──────────────────────────
    //
    // > **Un personnage accroché à une face autre que `Top`, et qui n'a
    // > PAS d'intention `Grimper`, se lâche.** (design §4.5, corrigé après
    // > analyse — voir ci-dessous.)
    //
    // ⚠️ **Correction au plan de la Tâche 3.** Le plan initial plaçait
    // cette règle entre la couche 2 et la couche 3, déclenchée seulement
    // quand l'intention en cours SE TERMINE. C'est insuffisant : le bloc
    // `e.commande` ci-dessus pose une intention et sort par un `return`
    // AVANT d'atteindre ce point. Un clic droit sur « Flâner » pendant que
    // le personnage est sur un mur poserait donc l'intention `Flaner` sans
    // jamais passer par cette règle — et `avancer` déplace l'offset LE
    // LONG DE LA FACE COURANTE : le personnage « marcherait »
    // verticalement le long du mur, en pose de marche, avec des
    // demi-tours. C'est exactement le bug que cette tâche doit rendre
    // impossible.
    //
    // La règle est donc placée ICI — après le `return` de la commande, et
    // avant les couches 2 et 3 — et elle est TOTALE : elle se redéclenche
    // à CHAQUE image tant que le personnage est accroché à une face
    // verticale sans intention `Grimper`, pas seulement au moment où une
    // intention se termine. C'est ce qui couvre le chemin de la commande
    // en plus du chemin normal des couches 2/3.
    //
    // La condition est « l'intention courante n'est pas `Grimper` »,
    // **`None` compris** (Tâche 4). Les deux sens comptent, et chacun
    // couvre un bug réel :
    //
    //   · une intention autre que `Grimper` — `Flaner` posée par un clic
    //     droit, par exemple — doit le faire lâcher, sans quoi il
    //     marcherait verticalement le long du mur ;
    //   · `Grimper` ne doit PAS le faire lâcher, sans quoi l'escalade se
    //     ferait tomber elle-même dès sa première image sur la paroi.
    //
    // Conséquence de PLACEMENT, à ne pas confondre avec ce qui précède :
    // cette règle s'exécute AVANT la couche 2, donc `intention::poursuivre`
    // n'est même pas appelée quand elle tire. Le scénario « une intention
    // se termine PENDANT la couche 2, puis la couche 3 en tire une
    // nouvelle, dans la même image » n'est rattrapé à l'image SUIVANTE que
    // si la couche 3 tire autre chose que `Grimper` — **ce n'était PAS
    // garanti**, et c'est précisément ce qui a produit un vrai bug (Tâche 7,
    // trouvé par l'invariant du monde vertical de `sim.rs`) : quand
    // `Grimper` expirait à son délai d'abandon (120 s) pendant que le
    // personnage était encore au plafond, la couche 3 lui repostait
    // aussitôt un `Grimper` tout neuf — et cette règle-ci, qui n'exempte
    // que le TYPE `Grimper` sans savoir s'il s'agit de la même escalade ou
    // d'une autre, ne voyait donc jamais passer l'image où il aurait dû
    // lâcher. D'où `lacher_si_accroche`, appelée maintenant aussi dans
    // `intention::poursuivre`, à SON point d'étranglement unique — voir son
    // commentaire pour le détail, et pour la seconde moitié de ce bug
    // (les sorties de `grimper()` autres que le délai d'abandon), corrigée
    // dans la même vague.
    //
    // Conséquence à retenir : **le sol est le seul endroit où l'on peut ne
    // rien faire.** C'est aussi ce qui rend le délai d'abandon lisible à
    // l'œil — au bout de deux minutes il en a marre, il lâche, il tombe.
    // C'est `FallFromWall` de Shimeji-ee.
    // `!matches!(…)` : vrai quand l'intention n'est PAS `Grimper`, `None`
    // inclus — `matches!` sur un `Option` ne filtre que le cas
    // `Some(Grimper)`, et tout le reste (y compris `None`) tombe donc dans
    // la négation. C'est exactement la règle voulue, en une expression
    // plutôt qu'en deux tests.
    //
    // Pas besoin de tester `ch.attachment` ici : `lacher_si_accroche` le
    // fait déjà, et rend `false` sans rien changer s'il n'y a rien à
    // lâcher (au sol, en chute, ou porté).
    let sans_escalade = !matches!(ch.intention, Some(ai) if ai.kind == intention::Intention::Grimper);

    if sans_escalade && lacher_si_accroche(ch, world) {
        // On rend la main : la chute est un réflexe, et c'est lui
        // qui posera la pose `fall` à l'image suivante. Tirer une
        // envie maintenant la ferait s'appliquer à un personnage
        // en l'air.
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
                // Neutre : aucun de ces tests ne parle d'escalade.
                grimper: 1.0,
            },
            utilisateur_actif: actif,
            commande: None,
        }
    }

    // ── La règle de sécurité du monde vertical (Tâche 3, étape 4a) ──────
    //
    // Ces tests verrouillent la règle « accroché à une face autre que
    // `Top`, sans intention `Grimper`, il se lâche ». Ils couvrent : le
    // cas nominal (aucune intention), le contre-exemple (le sol ne doit
    // jamais déclencher la règle), la non-régression d'une chute déjà en
    // cours, et — depuis la Tâche 4 — les DEUX sens de la condition sur
    // l'intention : `Flaner` sur un mur fait lâcher, `Grimper` non.

    /// Le mur gauche du monde de test (`FakeProbe::un_ecran`), face
    /// `Right` — la face regarde vers l'intérieur de l'écran (spec du
    /// design §2.1).
    fn mur_gauche(m: &World) -> &crate::world::Platform {
        m.platforms()
            .iter()
            .find(|p| p.has_face(Face::Right))
            .expect("un écran isolé a un mur gauche")
    }

    #[test]
    fn une_intention_finie_sur_un_mur_le_fait_lacher() {
        let m = monde();
        let mut ch = perso(&m);
        let mur = mur_gauche(&m);

        // Accroché à mi-hauteur, sans intention. La règle est placée AVANT
        // la couche 2 (correction 1 de la Tâche 3) : ici `intention` est
        // déjà `None`, donc elle tire et fait `return` dès cette première
        // image, sans même que `intention::poursuivre` soit appelé.
        ch.attachment = Attachment::On {
            platform: mur.id,
            face: Face::Right,
            offset: 400.0,
        };
        ch.intention = None;

        let mut rng = XorShift32::seeded(1);
        pas(
            &mut ch,
            &m,
            &entrees(true, 1.0),
            &desire::TableEnvies::defaut(),
            &crate::config::Reglages::depuis(&crate::config::Config::default()),
            Duration::from_secs(1),
            DT,
            &mut rng,
        );

        // Il tombe, et il tombe DE LÀ OÙ IL ÉTAIT — pas d'un point
        // recalculé ailleurs (décision n° 1) : on repart du rectangle
        // courant de la plateforme, avec sa face et son offset.
        match ch.attachment {
            Attachment::Falling { pos, vel } => {
                assert_eq!(pos, Point::new(0.0, 400.0));
                assert_eq!(vel, crate::geom::Vec2::zero());
            }
            autre => panic!("il devrait tomber, il est {autre:?}"),
        }
    }

    #[test]
    fn lacher_un_mur_le_fait_vraiment_tomber_sur_plusieurs_images() {
        // ── Le test qui manquait depuis les Tâches 3, 4 et 5 ────────────
        //
        // Bug corrigé (relecture de la Tâche 5) : `intention::grimper`
        // lâche un mur avec `Falling { pos: plat.rect.point_on(face,
        // offset), vel: Vec2::zero() }` — `x` reste donc EXACTEMENT sur le
        // plan du mur. Comme la vitesse horizontale est nulle, `x` ne bouge
        // plus d'une image à l'autre pendant que la gravité fait descendre
        // `y` seul, et l'ancien test de franchissement de `contact_mur`
        // (large des deux côtés) considérait ça comme un nouveau
        // franchissement : il se raccrochait IMMÉDIATEMENT, indéfiniment.
        //
        // Ce bug ne se voit QUE sur plusieurs images de la boucle complète
        // — `une_intention_finie_sur_un_mur_le_fait_lacher` ci-dessus ne
        // vérifie qu'un seul appel à `pas`, et l'état qu'il obtient
        // (`Falling` avec `vel = zero`) est EXACTEMENT celui du bug : la
        // toute première image après le lâcher est indiscernable, qu'on
        // retombe vraiment ou qu'on se rattrape à l'image suivante. Il faut
        // faire tourner la boucle pour le voir.
        let m = monde();
        let mut ch = perso(&m);
        let mur = mur_gauche(&m);

        ch.attachment = Attachment::On {
            platform: mur.id,
            face: Face::Right,
            offset: 400.0,
        };
        ch.intention = None;

        let mut rng = XorShift32::seeded(1);
        let table = desire::TableEnvies::defaut();
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());

        // Une demi-seconde simulée : assez pour distinguer « il tombe » de
        // « il se rattrape à chaque image », mais pas assez pour qu'il ait
        // atteint le sol depuis y = 400 (632 px plus bas) — sinon le test
        // ne pourrait plus distinguer « il tombe encore » de « il a fini de
        // tomber », ce qui serait vrai aussi.
        for i in 0..30 {
            pas(
                &mut ch,
                &m,
                &entrees(true, 1.0),
                &table,
                &reglages,
                Duration::from_secs(1) + Duration::from_secs_f32(i as f32 * DT),
                DT,
                &mut rng,
            );
        }

        match ch.attachment {
            Attachment::Falling { vel, .. } => {
                // La vitesse de chute doit avoir GRANDI : c'est la
                // signature d'une chute réelle sous la gravité, pas d'un
                // lâcher suivi d'un raccrochage immédiat (qui laisserait
                // `vel` à zéro, remise à zéro à chaque image).
                assert!(
                    vel.y > 50.0,
                    "il devrait être tombé sous l'effet de la gravité, vy = {}",
                    vel.y
                );
            }
            autre => panic!(
                "il devrait être tombé du mur et le rester, il est {autre:?}"
            ),
        }
    }

    #[test]
    fn une_intention_finie_sur_le_sol_ne_le_fait_pas_lacher() {
        // Le contre-exemple, indispensable : sans lui, un bug qui
        // détacherait TOUT LE MONDE (pas seulement les faces verticales)
        // passerait inaperçu. Le sol reste le seul endroit où l'on peut
        // ne rien faire.
        let m = monde();
        let mut ch = perso(&m);
        ch.intention = None;

        let mut rng = XorShift32::seeded(1);
        pas(
            &mut ch,
            &m,
            &entrees(true, 1.0),
            &desire::TableEnvies::defaut(),
            &crate::config::Reglages::depuis(&crate::config::Config::default()),
            Duration::from_secs(1),
            DT,
            &mut rng,
        );

        assert!(
            matches!(ch.attachment, Attachment::On { face: Face::Top, .. }),
            "il ne doit pas quitter le sol"
        );
        assert!(ch.intention.is_some(), "la couche 3 doit lui en tirer une");
    }

    #[test]
    fn lacher_un_mur_n_ecrase_pas_une_chute_deja_en_cours() {
        // Le cas du personnage qui s'est lâché lui-même à l'image
        // précédente : il est déjà `Falling` avec une vitesse. La règle
        // ne doit pas la remettre à zéro — elle ne concerne QUE
        // `Attachment::On` (voir la note d'emprunt dans `pas`).
        let m = monde();
        let mut ch = perso(&m);
        ch.attachment = Attachment::Falling {
            pos: Point::new(100.0, 200.0),
            vel: crate::geom::Vec2::new(0.0, 300.0),
        };
        ch.intention = None;

        let mut rng = XorShift32::seeded(1);
        pas(
            &mut ch,
            &m,
            &entrees(true, 1.0),
            &desire::TableEnvies::defaut(),
            &crate::config::Reglages::depuis(&crate::config::Config::default()),
            Duration::from_secs(1),
            DT,
            &mut rng,
        );

        match ch.attachment {
            // Le Réflexe 3 (la chute) l'a fait avancer : la vitesse a
            // grandi, elle n'a pas été effacée par notre règle.
            Attachment::Falling { vel, .. } => assert!(vel.y > 300.0),
            autre => panic!("il devrait toujours tomber, il est {autre:?}"),
        }
    }

    #[test]
    fn une_intention_flaner_sur_un_mur_le_fait_lacher() {
        // Le sens qui motive la règle : `Flaner` appelle `avancer`, qui
        // déplace l'offset *le long de la face courante*. Sans cette
        // règle, un personnage accroché à qui l'on tire (ou impose, via un
        // clic droit) une `Flaner` se mettrait à « marcher » verticalement
        // le long du mur au lieu de lâcher prise.
        let m = monde();
        let mut ch = perso(&m);
        let mur = mur_gauche(&m);

        ch.attachment = Attachment::On {
            platform: mur.id,
            face: Face::Right,
            offset: 400.0,
        };
        ch.intention = Some(intention::ActiveIntention::nouvelle(
            intention::Intention::Flaner,
            Duration::ZERO,
        ));

        let mut rng = XorShift32::seeded(1);
        pas(
            &mut ch,
            &m,
            &entrees(true, 1.0),
            &desire::TableEnvies::defaut(),
            &crate::config::Reglages::depuis(&crate::config::Config::default()),
            Duration::from_secs(1),
            DT,
            &mut rng,
        );

        assert!(
            matches!(ch.attachment, Attachment::Falling { .. }),
            "une Flaner sur un mur doit le faire lâcher, il est {:?}",
            ch.attachment
        );
    }

    #[test]
    fn une_intention_grimper_sur_un_mur_ne_le_fait_pas_lacher() {
        // Le sens contraire, tout aussi indispensable : sans lui, l'escalade
        // se ferait tomber elle-même dès sa première image sur la paroi —
        // `Grimper` accroche le personnage à une face autre que `Top`
        // exactement comme le fait cette règle de sécurité, donc si la
        // condition ne l'exemptait pas, il ne resterait jamais assez
        // longtemps sur le mur pour monter.
        //
        // ⚠️ **État de départ corrigé (relecture finale, après correction 1).**
        // Une version antérieure démarrait ce test avec une intention
        // `Grimper` TOUTE FRAÎCHE (`ActiveIntention::nouvelle`, phase
        // `Choisir`) tout en étant déjà accroché à un mur — un état qui
        // n'arrive plus jamais en jeu (`Choisir` ne s'atteint qu'au sol, et
        // `accroche`/`Rejoindre` posent directement `Accroche` ou
        // `Paroi`). Ce n'était pas un test de « une escalade en cours tient
        // le mur » : c'était, sans le savoir, un test du bug que la
        // correction 1 corrige précisément — la garde de face de `Choisir`
        // échouant et laissant le personnage pendu. Une fois la règle
        // corrigée, ce même état a fait tomber le personnage un temps (voir
        // `une_grimper_fraiche_sur_un_mur_reprend_l_escalade_au_lieu_de_tomber`
        // plus bas, qui documente pourquoi ce n'est plus le cas depuis la
        // tâche du menu contextuel de l'escalade : `Choisir` reprend
        // maintenant l'escalade au lieu d'échouer sur une face verticale).
        //
        // La propriété que CE test-ci doit garder est différente et reste
        // vraie : une escalade **légitimement en cours**, en phase `Paroi`
        // (donc déjà en train de monter ou descendre la paroi), ne doit
        // jamais se lâcher elle-même. D'où un départ en `Paroi { cible }` à
        // mi-mur plutôt qu'en `Choisir` — la phase est ce qui sépare les deux
        // tests : une intention `Grimper` VALIDE tient le mur, une intention
        // `Grimper` qui échoue ou se termine le lâche, quelle que soit sa
        // phase.
        let m = monde();
        let mut ch = perso(&m);
        let mur = mur_gauche(&m);

        ch.attachment = Attachment::On {
            platform: mur.id,
            face: Face::Right,
            offset: 400.0,
        };
        ch.intention = Some(intention::ActiveIntention {
            kind: intention::Intention::Grimper,
            depuis: Duration::ZERO,
            etat: intention::EtatIntention::Grimpe {
                // Une cible loin de l'offset de départ (400) : sur la durée
                // du test, il n'a pas le temps de l'atteindre, donc la phase
                // reste `Paroi` — en train de grimper, pas en train de
                // choisir ou de sortir.
                phase: intention::PhaseGrimpe::Paroi { cible: 0.0 },
                jusqu_a: Duration::ZERO,
            },
        });

        let mut rng = XorShift32::seeded(1);
        let table = desire::TableEnvies::defaut();
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());

        // Plusieurs images, pas une seule : une escalade en cours doit tenir
        // sur la durée, pas seulement à la première image.
        for i in 0..60 {
            pas(
                &mut ch,
                &m,
                &entrees(true, 1.0),
                &table,
                &reglages,
                Duration::from_secs(1) + Duration::from_secs_f32(i as f32 * DT),
                DT,
                &mut rng,
            );
        }

        assert!(
            matches!(ch.attachment, Attachment::On { face: Face::Right, .. }),
            "une Grimper en cours (phase Paroi) ne doit pas le faire lâcher, il est {:?}",
            ch.attachment
        );
    }

    #[test]
    fn une_grimper_fraiche_sur_un_mur_reprend_l_escalade_au_lieu_de_tomber() {
        // ⚠️ **Ce test verrouillait l'ANCIEN comportement, avant la tâche du
        // menu contextuel de l'escalade.** Il s'appelait alors
        // `une_grimper_qui_echoue_sur_un_mur_le_fait_tomber_sur_plusieurs_images`,
        // et documentait que la garde de face de la phase `Choisir`
        // («`face != Face::Top` ») faisait échouer une intention `Grimper`
        // toute fraîche posée pendant qu'on est déjà accroché à un mur — et
        // que le point d'étranglement unique (relecture finale de l'étape
        // 4a) faisait alors tomber le personnage proprement, plutôt que le
        // laisser pendu.
        //
        // **C'était exactement le bug rapporté à l'écran** (menu du
        // personnage) : choisir « Grimper au mur » alors qu'on est déjà sur
        // un mur le faisait tomber. La garde de `Choisir` a donc changé —
        // voir son commentaire dans `grimper` — pour REPRENDRE l'escalade
        // sur une face verticale ou le plafond, au lieu d'échouer. Ce même
        // scénario de départ doit donc maintenant produire l'ISSUE
        // OPPOSÉE : il continue de monter, il ne tombe plus.
        //
        // Le point d'étranglement unique (`lacher_si_accroche` appelé une
        // seule fois, après le calcul de l'issue) reste, lui, inchangé et
        // toujours nécessaire — c'est lui qui garantit qu'aucun des AUTRES
        // points de sortie de `grimper()` ne laisse le personnage pendu sans
        // qu'on le remarque. Seule la réponse de `Choisir` a changé.
        let m = monde();
        let mut ch = perso(&m);
        let mur = mur_gauche(&m);

        // Accroché à un mur (face `Right`), avec une intention `Grimper`
        // TOUTE FRAÎCHE : `ActiveIntention::nouvelle` la construit en phase
        // `Choisir` — précisément la phase qui, avant cette tâche, échouait
        // ici puisque la face n'est pas `Top`, et qui reprend maintenant
        // l'escalade.
        ch.attachment = Attachment::On {
            platform: mur.id,
            face: Face::Right,
            offset: 400.0,
        };
        ch.intention = Some(intention::ActiveIntention::nouvelle(
            intention::Intention::Grimper,
            Duration::ZERO,
        ));

        let mut rng = XorShift32::seeded(1);
        let table = desire::TableEnvies::defaut();
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());

        // Plusieurs images : la première ne fait que transiter `Choisir` →
        // `Paroi`, c'est la SUITE qui prouve qu'il monte réellement plutôt
        // que de tomber.
        for i in 0..30 {
            pas(
                &mut ch,
                &m,
                &entrees(true, 1.0),
                &table,
                &reglages,
                Duration::from_secs(1) + Duration::from_secs_f32(i as f32 * DT),
                DT,
                &mut rng,
            );
        }

        match ch.attachment {
            Attachment::On { face, offset, .. } => {
                assert_eq!(face, Face::Right, "il devrait toujours être sur le mur");
                assert!(
                    offset != 400.0,
                    "il devrait être en train de monter, offset={offset}"
                );
            }
            autre => panic!(
                "une intention Grimper fraîche sur un mur doit reprendre l'escalade, pas tomber, il est {autre:?}"
            ),
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

    #[test]
    fn le_reveil_du_deverrouillage_survit_a_un_utilisateur_actif() {
        // **Le pendant du test ci-dessus, et il est indispensable.**
        //
        // L'interruption termine le sommeil dès que l'utilisateur redevient
        // actif — c'est ce qu'on veut, sauf au déverrouillage : l'utilisateur
        // vient de taper son mot de passe, il est donc actif PAR
        // CONSTRUCTION. Si le réveil était une phase `Endormi`, il serait
        // coupé à la première image et l'on ne verrait rien du tout.
        //
        // C'est exactement pourquoi `Selevant` est une phase distincte, et ce
        // test est ce qui empêche de la refondre dans `Endormi` plus tard.
        let m = monde();
        let mut ch = perso(&m);
        let mut rng = XorShift32::seeded(1);
        let table = desire::TableEnvies::defaut();
        let r = crate::config::Reglages::depuis(&crate::config::Config::default());

        ch.set_pose(POSE_SLEEP, Duration::ZERO);
        ch.intention = Some(intention::ActiveIntention::reveil(Duration::ZERO));

        // L'utilisateur est ACTIF : c'est tout le sel du test.
        let e = entrees(true, 1.0);

        // Une seconde plus tard, il doit encore être en train d'émerger —
        // donc ni debout, ni parti flâner.
        for i in 0..60 {
            let t = Duration::from_secs_f32(i as f32 * DT);
            pas(&mut ch, &m, &e, &table, &r, t, DT, &mut rng);
        }

        let emerge = matches!(
            ch.intention,
            Some(intention::ActiveIntention {
                etat: intention::EtatIntention::Repos {
                    phase: intention::PhaseRepos::Selevant,
                    ..
                },
                ..
            })
        );
        assert!(
            emerge,
            "le réveil a été interrompu : il est en {:?}",
            ch.intention
        );
        assert_eq!(ch.pose, POSE_SLEEP, "il dort encore au bout d'une seconde");
    }

    // ── Le menu contextuel : une commande CHOISIT ───────────────────────
    //
    // Les quatre tests ci-dessous verrouillent la frontière entre « un signal
    // biaise » (décision n° 3) et « une interaction commande ». Elle est
    // subtile, et le commentaire du champ `Entrees::commande` l'explique ;
    // ces tests la rendent impossible à défaire par accident.

    /// Le test central : cliquer « S'asseoir » l'assoit, **tout de suite**.
    ///
    /// Pas « augmente ses chances de s'asseoir » : le tirage pondéré n'a
    /// aucune part ici, et c'est vérifié en donnant au repos un biais NUL.
    /// Si la commande passait par la couche 3, ce poids de 0 l'empêcherait
    /// d'être tirée et le test échouerait.
    #[test]
    fn une_commande_impose_l_intention_sans_passer_par_le_tirage() {
        let m = monde();
        let mut ch = perso(&m);
        let table = desire::TableEnvies::defaut();
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
        let mut rng = XorShift32::seeded(7);

        let mut e = entrees(true, 0.0);
        e.commande = Some(crate::menu_perso::Commande::Intention(intention::Intention::SeReposer));

        pas(
            &mut ch,
            &m,
            &e,
            &table,
            &reglages,
            Duration::from_secs(1),
            DT,
            &mut rng,
        );

        assert_eq!(
            ch.intention.map(|i| i.kind),
            Some(intention::Intention::SeReposer),
            "un clic sur « S'asseoir » doit asseoir, pas pondérer"
        );
    }

    /// Une commande remplace ce qu'il était en train de faire.
    ///
    /// Sans ça, cliquer « Flâner » pendant une sieste ne ferait rien avant
    /// l'expiration du délai d'abandon — jusqu'à 20 s d'attente, que
    /// l'utilisateur lirait comme un menu cassé.
    #[test]
    fn une_commande_interrompt_l_intention_en_cours() {
        let m = monde();
        let mut ch = perso(&m);
        let table = desire::TableEnvies::defaut();
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
        let mut rng = XorShift32::seeded(11);

        ch.intention = Some(intention::ActiveIntention::nouvelle(
            intention::Intention::SeReposer,
            Duration::ZERO,
        ));

        let mut e = entrees(true, 1.0);
        e.commande = Some(crate::menu_perso::Commande::Intention(intention::Intention::Flaner));

        pas(
            &mut ch,
            &m,
            &e,
            &table,
            &reglages,
            Duration::from_secs(2),
            DT,
            &mut rng,
        );

        assert_eq!(
            ch.intention.map(|i| i.kind),
            Some(intention::Intention::Flaner)
        );
    }

    /// **La marge survit** : il obéit, puis il reprend sa vie tout seul.
    ///
    /// C'est ce qui empêche le menu de transformer le personnage en
    /// marionnette. Le délai d'abandon (spec §7.3) s'applique à une intention
    /// forcée comme à toute autre — si on l'exemptait, un clic sur
    /// « S'asseoir » l'assiérait pour toujours.
    #[test]
    fn une_intention_commandee_expire_comme_les_autres() {
        let m = monde();
        let mut ch = perso(&m);
        let table = desire::TableEnvies::defaut();
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
        let mut rng = XorShift32::seeded(13);

        let mut e = entrees(true, 1.0);
        e.commande = Some(crate::menu_perso::Commande::Intention(intention::Intention::SeReposer));
        pas(
            &mut ch,
            &m,
            &e,
            &table,
            &reglages,
            Duration::ZERO,
            DT,
            &mut rng,
        );

        let posee = ch.intention.expect("l'intention vient d'être posée");
        let debut = posee.depuis;

        // La moitié qui fait mordre ce test : sans elle, il passerait aussi
        // avec un `Entrees::commande` totalement ignoré — la couche 3 tirant
        // de toute façon quelque chose, `depuis` avancerait pareil et
        // l'assertion finale serait vraie pour de mauvaises raisons.
        assert_eq!(
            posee.kind,
            intention::Intention::SeReposer,
            "la commande doit d'abord avoir pris effet"
        );

        // Une image bien après le délai d'abandon, SANS commande cette
        // fois : c'est l'impulsion qui est passée, pas un état permanent.
        let e = entrees(true, 1.0);
        let tard = debut + intention::DELAI_ABANDON + Duration::from_secs(1);
        pas(
            &mut ch,
            &m,
            &e,
            &table,
            &reglages,
            tard,
            DT,
            &mut rng,
        );

        let apres = ch.intention.expect("la couche 3 doit avoir re-tiré");
        assert!(
            apres.depuis > debut,
            "l'intention commandée aurait dû expirer et laisser place à un              nouveau tirage ; sans ça le menu ferait une marionnette"
        );
    }

    /// Une commande dont le personnage n'a pas les poses est refusée.
    ///
    /// Le menu filtre déjà (`TableEnvies::jouable`), mais un rechargement à
    /// chaud vers un pack plus pauvre peut survenir entre le clic et l'image
    /// suivante. Sans ce garde-fou, le personnage se figerait sur une pose
    /// absente — et il n'est pas question de compter sur le fait que la
    /// fenêtre soit étroite.
    #[test]
    fn une_commande_injouable_est_ignoree() {
        let m = monde();
        let mut ch = perso(&m);

        // Un manifeste qui ne sait que marcher : `sit` lui manque, donc
        // `SeReposer` est injouable.
        // `serde_json::from_str` directement : `Manifest::load` lit un
        // dossier, et on veut ici un personnage volontairement incomplet qui
        // n'existe sur aucun disque.
        ch.manifest = serde_json::from_str(
            r#"{ "id": "t", "name": "T", "frameSize": [128,128], "scale": 1,
                 "hitbox": [40,20,48,100],
                 "poses": { "walk": { "frames": [1] } } }"#,
        )
        .expect("manifeste de test valide");

        let table = desire::TableEnvies::defaut();
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
        let mut rng = XorShift32::seeded(17);

        let mut e = entrees(true, 1.0);
        e.commande = Some(crate::menu_perso::Commande::Intention(intention::Intention::SeReposer));

        pas(
            &mut ch,
            &m,
            &e,
            &table,
            &reglages,
            Duration::from_secs(1),
            DT,
            &mut rng,
        );

        assert_ne!(
            ch.intention.map(|i| i.kind),
            Some(intention::Intention::SeReposer),
            "il ne sait pas s'asseoir : la commande doit être refusée"
        );

        // ── Le contraste, sans lequel ce test ne prouverait rien ─────────
        //
        // Une assertion « il ne s'est PAS assis » est vraie aussi d'un
        // programme qui ignorerait purement et simplement les commandes. On
        // rejoue donc exactement le même scénario sur un personnage complet :
        // là, il doit s'asseoir. C'est la différence entre les deux qui
        // démontre que c'est bien la POSE MANQUANTE qui a fait refuser.
        let mut complet = perso(&m);
        pas(
            &mut complet,
            &m,
            &e,
            &table,
            &reglages,
            Duration::from_secs(1),
            DT,
            &mut rng,
        );
        assert_eq!(
            complet.intention.map(|i| i.kind),
            Some(intention::Intention::SeReposer),
            "le même clic sur un pack complet doit, lui, asseoir"
        );
    }

    // ── Le menu contextuel pendant l'escalade (le bug rapporté à l'écran) ──
    //
    // Un personnage accroché à un mur, sur qui l'on choisit une entrée du
    // menu, tombait quelle que soit l'entrée : la phase `Choisir` de
    // `Grimper` exigeait le sol et échouait sinon, et la règle de sécurité
    // du monde vertical faisait le reste. Les quatre tests ci-dessous
    // verrouillent la correction.

    /// **Le test qui verrouille le bug rapporté à l'écran.** « Grimper au
    /// mur », choisi pendant qu'il est déjà accroché à un mur, doit le faire
    /// MONTER — pas tomber.
    #[test]
    fn grimper_commande_depuis_un_mur_fait_monter_et_ne_le_fait_pas_tomber() {
        let m = monde();
        let mut ch = perso(&m);
        let mur = mur_gauche(&m);
        let table = desire::TableEnvies::defaut();
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
        let mut rng = XorShift32::seeded(23);

        ch.attachment = Attachment::On {
            platform: mur.id,
            face: Face::Right,
            offset: 400.0,
        };
        ch.intention = None;

        let mut e = entrees(true, 1.0);
        e.commande = Some(crate::menu_perso::Commande::Intention(
            intention::Intention::Grimper,
        ));

        // Plusieurs images : la première ne fait que poser l'intention
        // (`Choisir`), c'est la SUITE qui prouve qu'il monte réellement
        // plutôt que de tomber. Avec l'ANCIEN code, `Choisir` échouait dès
        // la deuxième image (face `Right` != `Top`), et la règle de
        // sécurité le faisait tomber : ce test échouait avant la correction.
        for i in 0..60 {
            pas(
                &mut ch,
                &m,
                &e,
                &table,
                &reglages,
                Duration::from_secs(1) + Duration::from_secs_f32(i as f32 * DT),
                DT,
                &mut rng,
            );
            // Impulsion : ne se pose qu'à la première image.
            e.commande = None;
        }

        match ch.attachment {
            Attachment::On { face, offset, .. } => {
                assert_eq!(face, Face::Right, "il doit rester sur le mur");
                assert!(
                    offset != 400.0,
                    "il devrait être en train de grimper (l'offset devrait avoir bougé), offset={offset}"
                );
            }
            autre => panic!(
                "« Grimper au mur » depuis un mur ne doit pas le faire tomber, il est {autre:?}"
            ),
        }
    }

    /// « Se lâcher » : l'intention est effacée, et c'est la règle de
    /// sécurité du monde vertical — pas une ligne de physique écrite ici —
    /// qui le fait tomber, dans la même image.
    ///
    /// Sur plusieurs images, comme `lacher_un_mur_le_fait_vraiment_tomber_sur_plusieurs_images` :
    /// la toute première image après le lâcher a une vitesse nulle,
    /// indiscernable d'un raccrochage immédiat.
    #[test]
    fn se_lacher_le_fait_vraiment_tomber_sur_plusieurs_images() {
        let m = monde();
        let mut ch = perso(&m);
        let mur = mur_gauche(&m);
        let table = desire::TableEnvies::defaut();
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
        let mut rng = XorShift32::seeded(29);

        ch.attachment = Attachment::On {
            platform: mur.id,
            face: Face::Right,
            offset: 400.0,
        };
        // Accroché, en train de tenir légitimement — pour prouver que
        // c'est bien la COMMANDE qui le fait lâcher, pas une intention déjà
        // expirée ou absente.
        ch.intention = Some(intention::ActiveIntention {
            kind: intention::Intention::Grimper,
            depuis: Duration::ZERO,
            etat: intention::EtatIntention::Grimpe {
                phase: intention::PhaseGrimpe::Accroche,
                jusqu_a: Duration::from_secs(60),
            },
        });

        let mut e = entrees(true, 1.0);
        e.commande = Some(crate::menu_perso::Commande::SeLacher);

        for i in 0..30 {
            pas(
                &mut ch,
                &m,
                &e,
                &table,
                &reglages,
                Duration::from_secs(1) + Duration::from_secs_f32(i as f32 * DT),
                DT,
                &mut rng,
            );
            e.commande = None;
        }

        match ch.attachment {
            Attachment::Falling { vel, .. } => {
                assert!(
                    vel.y > 50.0,
                    "« Se lâcher » devrait l'avoir fait tomber sous la gravité, vy = {}",
                    vel.y
                );
            }
            autre => panic!("« Se lâcher » doit le faire tomber, il est {autre:?}"),
        }
    }

    /// **Doublé avec `expire_au_plafond_il_tombe_au_lieu_de_marcher_dessus`
    /// (`intention.rs`), et c'est délibéré — pas un copier-coller inutile.**
    ///
    /// Le point d'étranglement unique de `poursuivre` — « toute intention
    /// qui se termine sur une face non-`Top` fait lâcher » — est un SEUL
    /// code, partagé entre mur et plafond (`lacher_si_accroche` ne distingue
    /// pas les deux faces). Mais ce partage a déjà menti deux fois sur cette
    /// branche : à la Tâche 7 puis à la relecture finale de l'étape 4a, un
    /// chemin qu'on croyait couvert par « c'est la même règle » ne l'était
    /// pas. Après la réécriture des deux tests du menu contextuel de
    /// l'escalade (qui ont changé la réponse de la phase `Choisir` sur une
    /// face verticale), il ne restait plus AUCUN test qui fasse expirer une
    /// escalade par le DÉLAI D'ABANDON sur un MUR — le seul test qui
    /// touchait encore un mur passait par `Choisir`, précisément le chemin
    /// que la réécriture a retiré. Ce test comble ce trou plutôt que de
    /// faire confiance au partage de code une troisième fois.
    #[test]
    fn expire_sur_un_mur_il_tombe_vraiment_sur_plusieurs_images() {
        let m = monde();
        let mut ch = perso(&m);
        let mur = mur_gauche(&m);
        let table = desire::TableEnvies::defaut();
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
        let mut rng = XorShift32::seeded(41);

        ch.attachment = Attachment::On {
            platform: mur.id,
            face: Face::Right,
            offset: 400.0,
        };
        // Phase `Paroi`, PAS `Choisir` : c'est le chemin qui reste après la
        // réécriture des deux tests de la relecture — l'escalade est
        // légitimement EN COURS (en train de monter, cible loin de
        // l'offset), et c'est le délai d'abandon SEUL qui doit la faire
        // échouer, pas une garde de face.
        //
        // `depuis: ZERO`, horloge démarrée après le délai d'abandon de
        // l'escalade (120 s à vitesse ×1, voir `DELAI_ABANDON_GRIMPE`) :
        // elle est donc déjà périmée à la première image, sans dérouler
        // 120 s de montée pour y arriver — même recette que le test du
        // plafond.
        ch.intention = Some(intention::ActiveIntention {
            kind: intention::Intention::Grimper,
            depuis: Duration::ZERO,
            etat: intention::EtatIntention::Grimpe {
                phase: intention::PhaseGrimpe::Paroi { cible: 0.0 },
                jusqu_a: Duration::ZERO,
            },
        });

        // Plusieurs images, comme les autres tests de chute de ce module :
        // la toute première après l'expiration a une vitesse nulle,
        // indiscernable d'un raccrochage immédiat. Il faut voir la vitesse
        // croître sous la gravité pour être sûr qu'il tombe vraiment.
        for i in 0..30 {
            pas(
                &mut ch,
                &m,
                &entrees(true, 1.0),
                &table,
                &reglages,
                Duration::from_secs(121) + Duration::from_secs_f32(i as f32 * DT),
                DT,
                &mut rng,
            );
        }

        match ch.attachment {
            Attachment::Falling { vel, .. } => {
                assert!(
                    vel.y > 50.0,
                    "il devrait être tombé du mur à l'expiration du délai d'abandon, vy = {}",
                    vel.y
                );
            }
            autre => panic!(
                "une escalade expirée sur un mur doit le faire tomber, il est {autre:?}"
            ),
        }
    }

    /// « Redescendre » le ramène jusqu'au sol, en reprenant la phase `Paroi`
    /// existante — pas une seconde implémentation de la descente.
    #[test]
    fn redescendre_le_ramene_au_sol() {
        let m = monde();
        let mut ch = perso(&m);
        let mur = mur_gauche(&m);
        let longueur = mur.rect.face_length(Face::Right);
        let table = desire::TableEnvies::defaut();
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
        let mut rng = XorShift32::seeded(31);

        // Presque en bas déjà : quelques images suffisent pour vérifier
        // qu'il atteint le sol, sans faire durer le test pour rien.
        ch.attachment = Attachment::On {
            platform: mur.id,
            face: Face::Right,
            offset: longueur - 0.5,
        };
        ch.intention = None;

        let mut e = entrees(true, 1.0);
        e.commande = Some(crate::menu_perso::Commande::Redescendre);

        for i in 0..10 {
            pas(
                &mut ch,
                &m,
                &e,
                &table,
                &reglages,
                Duration::from_secs(1) + Duration::from_secs_f32(i as f32 * DT),
                DT,
                &mut rng,
            );
            e.commande = None;
        }

        assert!(
            matches!(ch.attachment, Attachment::On { face: Face::Top, .. }),
            "« Redescendre » devrait l'avoir ramené au sol, il est {:?}",
            ch.attachment
        );
    }

    /// Point 5 de la tâche : une commande de SOL arrivée alors qu'il est sur
    /// un mur doit être IGNORÉE — surtout pas le faire tomber. Le menu ne la
    /// propose plus hors du sol, mais le clic et cette image ne sont pas le
    /// même instant (rechargement, désynchronisation).
    #[test]
    fn une_commande_de_sol_recue_pendant_l_escalade_est_ignoree() {
        let m = monde();
        let mut ch = perso(&m);
        let mur = mur_gauche(&m);
        let table = desire::TableEnvies::defaut();
        let reglages = crate::config::Reglages::depuis(&crate::config::Config::default());
        let mut rng = XorShift32::seeded(37);

        ch.attachment = Attachment::On {
            platform: mur.id,
            face: Face::Right,
            offset: 400.0,
        };
        // Escalade légitimement EN COURS : sans intention `Grimper`, la
        // règle de sécurité le ferait tomber de toute façon, ce qui ne
        // prouverait rien sur la commande elle-même.
        ch.intention = Some(intention::ActiveIntention {
            kind: intention::Intention::Grimper,
            depuis: Duration::ZERO,
            etat: intention::EtatIntention::Grimpe {
                phase: intention::PhaseGrimpe::Paroi { cible: 0.0 },
                jusqu_a: Duration::ZERO,
            },
        });

        let mut e = entrees(true, 1.0);
        e.commande = Some(crate::menu_perso::Commande::Intention(
            intention::Intention::SeReposer,
        ));

        pas(
            &mut ch,
            &m,
            &e,
            &table,
            &reglages,
            Duration::from_secs(1),
            DT,
            &mut rng,
        );

        assert!(
            matches!(ch.attachment, Attachment::On { face: Face::Right, .. }),
            "une commande de sol reçue sur un mur doit être ignorée, pas le faire tomber : {:?}",
            ch.attachment
        );
        assert_eq!(
            ch.intention.map(|i| i.kind),
            Some(intention::Intention::Grimper),
            "l'escalade en cours ne doit pas être interrompue par une commande refusée"
        );
    }
}
