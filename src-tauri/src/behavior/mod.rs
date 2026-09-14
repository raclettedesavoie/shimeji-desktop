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

// Les tests de ce module vivent dans `tests.rs`
// (sortis d ici le 2026-09-14 : ils faisaient 1179 des 1625 lignes).
#[cfg(test)]
mod tests;
