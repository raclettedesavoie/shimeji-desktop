//! La physique : chute, atterrissage, et les vitesses de déplacement.
//!
//! Responsabilité unique : des **fonctions pures** sur des points et des
//! vitesses. Aucune notion de personnage, aucun état. C'est ce qui permet de
//! tester une chute « hauteur et durée connues » (spec §10.1) sans instancier
//! quoi que ce soit.
//!
//! Toutes les vitesses sont en **pixels physiques par seconde**, toutes les
//! accélérations en pixels par seconde carrée, et `y` croît **vers le bas**
//! (convention Windows, voir `geom::Rect`).

use crate::geom::{Face, Point, Vec2};
use crate::world::{PlatformId, World};

// ── La chute, reprise de Shimeji-ee ────────────────────────────────────
//
// `src/com/group_finity/mascot/action/Fall.java` :
//
//     DEFAULT_GRAVITY     = 2      (px par tick, ajoutés à la vitesse)
//     DEFAULT_RESISTANCEY = 0.1    (fraction de la vitesse retirée par tick)
//     DEFAULT_RESISTANCEX = 0.05
//
//     vy = vy - vy * RESISTANCEY + GRAVITY
//     vx = vx - vx * RESISTANCEX
//
// **Il y a donc un frottement de l'air**, que la première version du projet
// n'avait pas — d'où une chute nettement trop rapide. La vitesse limite de
// Shimeji-ee est `GRAVITY / RESISTANCEY = 20` px/tick, soit **500 px/s**,
// contre 1 600 px/s de plafond dur ici. C'est un facteur 3.
//
// Conversion des constantes par tick (`TICK_INTERVAL = 40 ms`, donc
// 25 ticks/s) en constantes par seconde. En notant `V` la vitesse en px/s :
//
//     dV/dt = 25·GRAVITY/T − (RESISTANCEY/T)·V
//           = 1250 − 2,5·V
//
// La vitesse limite se retrouve bien : 1250 / 2,5 = 500 px/s.

/// Accélération de la pesanteur, en px/s².
///
/// `25 × GRAVITY / TICK_INTERVAL` = `25 × 2 / 0,04`.
pub const GRAVITE: f32 = 1250.0;

/// Frottement de l'air vertical, en s⁻¹. `RESISTANCEY / TICK_INTERVAL`.
///
/// C'est lui qui donne la vitesse limite, et lui qui manquait : sans
/// frottement, une chute de 1 000 px prenait 1,2 s au lieu de 2,3 s.
pub const FROTTEMENT_CHUTE_Y: f32 = 2.5;

/// Frottement de l'air **en montée**, en s⁻¹ — notre propre constante.
///
/// Shimeji-ee n'en a pas : `Fall.java` écrit `vy - vy·RESISTANCEY` quel que
/// soit le signe de `vy`. En montée, `vy` étant négatif, ce terme devient
/// POSITIF et **s'ajoute** à la pesanteur au lieu de s'y opposer — lancé vers
/// le haut à notre plafond de 1 200 px/s, le personnage subissait
/// `1250 + 2,5×1200 = 4 250 px/s²` et ne montait que ~235 px. À l'écran, le
/// jet vertical paraissait mou là où le jet horizontal était juste
/// (rapporté le 2026-09-12).
///
/// Freiner **six fois moins** en montée qu'en descente porte l'apex à
/// ~450 px au lancer maximum. Les deux autres valeurs ont été mesurées puis
/// écartées, et c'est pour ça qu'elles sont écrites ici :
///
/// | frottement en montée | apex au lancer max |
/// |---|---|
/// | `FROTTEMENT_CHUTE_Y` (Shimeji-ee) | ~235 px — trop mou |
/// | **`0,40`** | **~450 px** |
/// | `0,0` (aucun frottement) | ~576 px — trop haut, et il s'accrochait au plafond à tout bout de champ |
///
/// Le raccord à l'apex reste continu : de part et d'autre de `vy = 0`,
/// l'accélération vaut `GRAVITE` à un terme près qui tend vers zéro.
pub const FROTTEMENT_MONTEE_Y: f32 = 0.40;

/// Frottement horizontal, en s⁻¹. `RESISTANCEX / TICK_INTERVAL`.
///
/// L'élan horizontal d'un personnage lâché en marchant **décroît** donc.
/// La première version le conservait indéfiniment, ce qui donnait des
/// trajectoires trop plates.
pub const FROTTEMENT_CHUTE_X: f32 = 1.25;

/// Plafond dur de la vitesse de chute, en px/s.
///
/// **Garde-fou, jamais atteint en pratique** : le frottement plafonne
/// naturellement à ~500 px/s. Il ne sert que si un lâcher venait avec une
/// vitesse initiale énorme, et sa raison est technique — la détection
/// d'atterrissage teste le **segment** parcouru dans une image, et un
/// segment de plusieurs milliers de pixels enjamberait plusieurs
/// plateformes, rendant le choix arbitraire.
///
/// À 60 Hz, 900 px/s vaut 15 px par image : bien moins que la moindre
/// plateforme.
pub const VITESSE_CHUTE_MAX: f32 = 900.0;

/// Vitesse de marche.
///
/// **Reprise de Shimeji-ee, pas réglée à l'œil** : l'action `Walk` de
/// `conf/actions.xml` porte `Velocity="-2,0"`, soit 2 px par tick, et
/// `Manager.TICK_INTERVAL = 40 ms` — donc 50 px/s.
pub const VITESSE_MARCHE: f32 = 50.0;

/// Vitesse de course. `Run` porte `Velocity="-4,0"` → 100 px/s.
///
/// Le rapport à la marche est donc exactement **2×**, et c'est ce qui rend
/// le passage de l'une à l'autre lisible. Shimeji-ee a aussi un `Dash` à
/// `-8,0` (200 px/s) qu'on n'utilise pas encore.
pub const VITESSE_COURSE: f32 = 100.0;

/// Vitesse d'escalade, en px/s.
///
/// **Relevée dans `conf/actions.xml`, pas réglée à l'œil.** L'action
/// `ClimbWall` enchaîne huit poses de durées 16, 4, 4, 4, 16, 4, 4, 4 ticks,
/// de vitesses 0, −1, −1, −1, 0, −2, −2, −2 px/tick. Déplacement :
/// `3×4×1 + 3×4×2 = 36 px`. Durée : `56 × 40 ms = 2,24 s`. Soit **16,1 px/s**,
/// c'est-à-dire **trois fois plus lent que la marche** (50 px/s).
///
/// C'est cette lenteur qui donne le « il se hisse » plutôt que « il glisse ».
/// Ne pas l'accélérer pour rendre l'escalade « plus fluide » : on perdrait
/// exactement ce qui la rend jolie. Un mur de 1032 px prend donc 64 s, ce qui
/// est la raison du délai d'abandon à 120 s (design §4.4).
pub const VITESSE_ESCALADE: f32 = 16.1;

/// Bornes `[min, max]` de la durée d'accroche à une paroi, en secondes.
///
/// `HoldOntoWall` de `conf/actions.xml` : `Duration="${500+Math.random()*1000}"`,
/// en millisecondes — donc de 0,5 s à 1,5 s.
pub const DUREE_ACCROCHE: [f32; 2] = [0.5, 1.5];

// ── Le balancier du personnage porté ──────────────────────────────────
//
// **Ce n'est pas une animation, c'est un ressort amorti.** Découvert dans
// `action/Dragged.java` :
//
//     footDx = (footDx + (curseurX − footX) × 0,1) × 0,8
//     footX += footDx
//
// Un point « pied » poursuit le curseur avec un ressort (raideur 0,1) et un
// amortissement (facteur 0,8 par tick). Son **retard** sur le curseur choisit
// ensuite la frame, par les conditions de l'action `Pinched`.
//
// Trois propriétés en découlent, gratuitement, et ce sont exactement les
// trois qui manquaient :
//   · l'amplitude croît avec la vitesse du curseur ;
//   · le retour au repos passe par les frames intermédiaires ;
//   · le système est sous-amorti, donc il oscille un peu avant de se poser.
//
// Conversion des constantes par tick en constantes par seconde, en notant
// `x` la position du pied et `V` sa vitesse en px/s :
//
//     dV/dt = RAIDEUR·(curseur − x) − AMORTISSEMENT·V

/// Raideur du ressort du balancier, en s⁻². `0,08 / TICK_INTERVAL²`.
pub const BALANCIER_RAIDEUR: f32 = 50.0;

/// Amortissement du balancier, en s⁻¹. `0,2 / TICK_INTERVAL`.
///
/// Le rapport d'amortissement vaut `5 / (2·√50) ≈ 0,35` : **sous-amorti**.
/// C'est voulu — un système critique reviendrait au repos sans osciller, et
/// on perdrait le balancement qui fait tout l'intérêt.
pub const BALANCIER_AMORTISSEMENT: f32 = 5.0;

/// Les trois seuils de retard, en pixels, qui choisissent la frame.
///
/// Repris tels quels des conditions de l'action `Pinched` : `±10`, `±30`,
/// `±50`. Ils sont en pixels et non en vitesse, et c'est cohérent — à vitesse
/// constante `s`, le régime permanent du ressort donne un retard de `s/10`,
/// donc 300 px/s de glisser produisent 30 px de retard.
pub const BALANCIER_SEUILS: [f32; 3] = [10.0, 30.0, 50.0];

// ── Le lancer ─────────────────────────────────────────────────────────
//
// Shimeji-ee lance : `conf/actions.xml`, action `Thrown`, enchaîne sur
//
//     <ActionReference Name="Falling"
//         InitialVX="${mascot.environment.cursor.dx}"
//         InitialVY="${mascot.environment.cursor.dy}"/>
//
// et `UserBehavior.mouseReleased` déclenche ce comportement dès qu'on lâche
// un personnage qu'on portait. Il ne tombe donc **jamais** à la verticale
// après un glisser — il part avec la vitesse de la main.
//
// `cursor.dx` n'est pas un delta brut : `environment/Location.java` en fait
// une moyenne exponentielle, `dx = (dx + Δx) / 2` à chaque tick. Le delta est
// donc divisé par deux à chaque tick en l'absence de mouvement, ce qui
// correspond à une constante de lissage de `ln 2 / TICK_INTERVAL ≈ 17,3 s⁻¹`.

/// Constante de lissage de la vitesse du curseur, en s⁻¹.
///
/// `ln 2 / TICK_INTERVAL` : c'est le taux qui divise l'écart par deux à
/// chaque tick de 40 ms, comme `Location.set`.
pub const LISSAGE_CURSEUR: f32 = 17.33;

/// Pente minimale — `montée / |déplacement horizontal|` — pour que le
/// plafond attrape un personnage en vol.
///
/// Sans unité : c'est un rapport de deux longueurs. 2 vaut ~63° au-dessus de
/// l'horizontale. La mesure qui l'a choisi est dans `contact_plafond`, seul
/// endroit qui s'en sert.
pub const PENTE_MIN_PLAFOND: f32 = 2.0;

/// Plafond de la vitesse de lancer, en px/s.
///
/// Un coup de poignet violent peut produire plusieurs milliers de px/s, ce
/// qui traverserait un écran en trois images. Deux raisons de borner :
/// l'atterrissage teste un **segment** et de trop grands pas rendent son
/// choix arbitraire, et un personnage qui disparaît instantanément hors du
/// bureau n'est pas amusant, juste perdu — même si le garde-fou le rattrape.
///
/// 1 200 px/s traverse un écran de 1 920 px en 1,6 s : c'est déjà un beau
/// jet.
pub const VITESSE_LANCER_MAX: f32 = 1200.0;

/// Met à jour la vitesse lissée du curseur.
///
/// `precedent` et `actuel` sont deux positions successives du curseur,
/// séparées de `dt`. Rend la nouvelle vitesse lissée, en px/s.
///
/// `1 − e^{−k·dt}` plutôt qu'un poids fixe : le lissage garde alors le même
/// comportement dans le temps quelle que soit la cadence. Un poids fixe de
/// 0,5 par image lisserait 2,4× plus vite à 60 Hz qu'aux 25 Hz de
/// Shimeji-ee.
pub fn lisser_vitesse_curseur(lissee: Vec2, precedent: Point, actuel: Point, dt: f32) -> Vec2 {
    if dt <= 0.0 {
        return lissee;
    }

    let instantanee = Vec2::new(
        (actuel.x - precedent.x) / dt,
        (actuel.y - precedent.y) / dt,
    );

    let poids = 1.0 - (-LISSAGE_CURSEUR * dt).exp();

    Vec2::new(
        lissee.x + (instantanee.x - lissee.x) * poids,
        lissee.y + (instantanee.y - lissee.y) * poids,
    )
}

/// Borne la vitesse de lancer, en gardant sa direction.
///
/// On borne la **norme** et non chaque axe séparément : borner les axes
/// déformerait la direction du jet, et un lancer en diagonale ne partirait
/// pas là où la main l'a envoyé.
pub fn borner_lancer(v: Vec2) -> Vec2 {
    let norme = (v.x * v.x + v.y * v.y).sqrt();
    if norme <= VITESSE_LANCER_MAX || norme == 0.0 {
        return v;
    }
    let facteur = VITESSE_LANCER_MAX / norme;
    Vec2::new(v.x * facteur, v.y * facteur)
}

/// Un pas d'intégration de la chute libre.
///
/// **Euler semi-implicite** : on met à jour la vitesse *avant* la position.
/// C'est une ligne de différence avec Euler explicite, et c'est bien plus
/// stable — la trajectoire ne dérive pas quand le pas de temps varie un peu,
/// ce qui arrive dès que la machine est chargée.
///
/// Fonction pure : elle rend le nouvel état au lieu de modifier l'ancien.
/// C'est ce qui la rend testable en boucle dans un test, comme ci-dessous.
pub fn integrer_chute(pos: Point, vel: Vec2, dt: f32) -> (Point, Vec2) {
    // Pesanteur **moins frottement**, comme `Fall.java` — mais le frottement
    // n'est pas le même selon qu'il monte ou qu'il descend, et c'est notre
    // seule divergence ici : voir `FROTTEMENT_MONTEE_Y`, qui porte la mesure
    // et les deux valeurs écartées.
    //
    // En descente, c'est le frottement qui plafonne naturellement la chute à
    // ~500 px/s, sans plafond dur.
    let frottement = if vel.y > 0.0 {
        FROTTEMENT_CHUTE_Y * vel.y
    } else {
        FROTTEMENT_MONTEE_Y * vel.y
    };

    // `min` et non `clamp` : seule la chute est plafonnée. Une vitesse
    // ascendante (personnage lâché vers le haut) n'a pas de raison de l'être.
    let vy = (vel.y + (GRAVITE - frottement) * dt).min(VITESSE_CHUTE_MAX);

    // L'élan horizontal décroît aussi (`RESISTANCEX = 0,05`). Sans ça, un
    // personnage lâché en courant garderait sa vitesse jusqu'au sol et la
    // trajectoire paraîtrait plate.
    let vx = vel.x - FROTTEMENT_CHUTE_X * vel.x * dt;

    let nouvelle_vel = Vec2::new(vx, vy);
    let nouvelle_pos = Point::new(pos.x + nouvelle_vel.x * dt, pos.y + nouvelle_vel.y * dt);

    (nouvelle_pos, nouvelle_vel)
}

/// Un pas du ressort amorti du balancier de portage.
///
/// Rend la nouvelle position et la nouvelle vitesse du point « pied », qui
/// poursuit `curseur_x`. Voir le bandeau de constantes ci-dessus pour le
/// modèle et sa provenance.
///
/// Fonction pure, comme `integrer_chute` : c'est ce qui permet de tester le
/// balancier sans souris, sans fenêtre et sans personnage.
pub fn integrer_balancier(pied_x: f32, pied_vx: f32, curseur_x: f32, dt: f32) -> (f32, f32) {
    // Euler semi-implicite ici aussi : la vitesse avant la position. Sur un
    // ressort, l'explicite peut carrément diverger si le pas est grand.
    let acceleration = BALANCIER_RAIDEUR * (curseur_x - pied_x) - BALANCIER_AMORTISSEMENT * pied_vx;
    let nouvelle_vx = pied_vx + acceleration * dt;
    let nouvelle_x = pied_x + nouvelle_vx * dt;
    (nouvelle_x, nouvelle_vx)
}

/// Quel **niveau** de balancement afficher, d'après le retard du pied sur le
/// curseur.
///
/// Rend `(cote, niveau)` où `cote` est le signe du retard et `niveau` va de 0
/// (au repos) à 3 (balancement maximal).
///
/// Les seuils `±10`, `±30`, `±50` viennent des conditions de l'action
/// `Pinched`.
///
/// > ⚠️ **Un écart volontaire, et c'est une correction de bug.** Dans le XML
/// > de Shimeji-ee, les conditions sont évaluées dans l'ordre et
/// > `FootX < cursor.x` (soit `d < 0`) est testée **avant** la bande neutre
/// > `-10 < d < +10`. Cette bande est donc **inatteignable pour un retard
/// > négatif** : un pied immobilisé un dixième de pixel à gauche du curseur
/// > garde la pose 5 indéfiniment, au lieu de pendre droit.
/// >
/// > Ce n'est pas théorique : c'est exactement là que le ressort se pose
/// > après avoir oscillé, donc le personnage terminait **tout portage** dans
/// > une pose légèrement penchée. On teste donc la bande neutre **en
/// > premier**, et symétriquement — ce qui est manifestement l'intention de
/// > la condition `±10` d'origine.
pub fn niveau_balancier(retard: f32) -> (Cote, u8) {
    // La bande neutre d'abord, et symétrique. Voir l'avertissement ci-dessus.
    if retard.abs() < BALANCIER_SEUILS[0] {
        return (Cote::Aucun, 0);
    }

    if retard < 0.0 {
        if retard <= -BALANCIER_SEUILS[2] {
            (Cote::PiedAGauche, 3)
        } else if retard <= -BALANCIER_SEUILS[1] {
            (Cote::PiedAGauche, 2)
        } else {
            (Cote::PiedAGauche, 1)
        }
    } else if retard >= BALANCIER_SEUILS[2] {
        (Cote::PiedADroite, 3)
    } else if retard >= BALANCIER_SEUILS[1] {
        (Cote::PiedADroite, 2)
    } else {
        (Cote::PiedADroite, 1)
    }
}

/// De quel côté le pied traîne.
///
/// **Attention au sens, c'est le point qui a été inversé une fois :** le pied
/// traîne à gauche quand le curseur va à **droite**. Et comme le pied part à
/// gauche, la **tête penche à droite**. C'est un pendule, il traîne derrière.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cote {
    /// Retard négatif : `footX < curseurX`. Curseur vers la droite, tête à
    /// droite. Frames 5, 7, 9 chez Shimeji-ee.
    PiedAGauche,
    /// Retard positif. Curseur vers la gauche, tête à gauche. Frames 6, 8, 10.
    PiedADroite,
    /// Zone neutre : il pend droit. Frame 1.
    Aucun,
}

/// Le personnage a-t-il touché une plateforme en passant de `avant` à
/// `apres` ?
///
/// Rend la plateforme et l'offset où se poser, ou `None` s'il continue de
/// tomber.
///
/// **Trois règles, et pas une de plus** :
///   1. on ne s'accroche qu'en **descendant** — sinon on s'agripperait au
///      sol qu'on traverse par-dessous ;
///   2. on retient la face **la plus haute** traversée — c'est la première
///      rencontrée en descendant ;
///   3. il faut être **au-dessus** de la face horizontalement.
///
/// La règle 2 ne sert à rien à l'étape 1 (les sols des écrans ne se
/// chevauchent pas) mais elle sera exactement ce qu'il faut à l'étape 4, où
/// une barre de titre flotte au-dessus du sol. L'écrire maintenant coûte deux
/// lignes ; l'oublier coûterait un diagnostic.
pub fn atterrissage(world: &World, avant: Point, apres: Point) -> Option<(PlatformId, f32)> {
    // Règle 1 : on descend ? `<` et non `<=` pour accepter le cas où le
    // personnage était déjà pile sur la ligne, sans mouvement vertical.
    if apres.y < avant.y {
        return None;
    }

    let mut meilleur: Option<(PlatformId, f32, f32)> = None; // (id, offset, y de la face)

    for plat in world.platforms() {
        if !plat.has_face(Face::Top) {
            continue;
        }

        let y_face = plat.rect.top();

        // A-t-on franchi cette ligne pendant le pas ?
        let franchie = avant.y <= y_face && apres.y >= y_face;
        if !franchie {
            continue;
        }

        // Règle 3 : est-on au-dessus de la face ?
        //
        // On teste avec `apres.x`, et non avec la position exacte du
        // croisement. C'est volontairement approximatif : à 27 px par image
        // au maximum, l'écart est invisible, et calculer l'intersection
        // exacte ajouterait de la trigonométrie pour rien. La navigation a
        // le droit d'être imparfaite (décision n° 4).
        if apres.x < plat.rect.left() || apres.x > plat.rect.right() {
            continue;
        }

        // Règle 2 : garder la face la plus haute, donc le plus petit `y`.
        let remplace = match meilleur {
            None => true,
            Some((_, _, y)) => y_face < y,
        };
        if remplace {
            meilleur = Some((plat.id, apres.x - plat.rect.left(), y_face));
        }
    }

    meilleur.map(|(id, offset, _)| (id, offset))
}

/// Ce que le personnage a heurté pendant ce pas de chute — sol, mur **ou**
/// plafond.
///
/// Généralise `atterrissage` aux faces verticales et à la face `Bottom`
/// (design §3.2). Rend la `Face` en plus de la plateforme, parce que
/// l'appelant en a besoin pour choisir la pose et l'orientation : on ne se
/// pose pas sur un mur comme on se pose sur un sol, ni sur un plafond comme
/// sur un mur.
///
/// **Les deux premières règles viennent de `Fall.java`, pas d'une
/// intuition ; la troisième s'en écarte délibérément :**
///   1. le sol d'abord — sa boucle fait `break OUTER` sur le sol avant de
///      tester le mur ;
///   2. puis les murs — `hasNext()` teste `floor.isOn(pos) || wall.isOn(pos)`,
///      donc un mur arrête une chute exactement comme un sol, et **sans
///      aucun seuil de vitesse** ;
///   3. **le plafond attrape aussi, désormais.**
///
/// > ⚠️ **Divergence assumée de `Fall.java`, décidée après un essai à
/// > l'écran (2026-09-12).** La version d'origine de ce document disait
/// > « le plafond n'attrape rien », posée par lecture stricte de
/// > `Fall.java::hasNext()` — qui ne teste effectivement que le sol et le
/// > mur, le plafond n'y figurant dans aucune condition. Ce choix a été
/// > **essayé en jeu**, et l'auteur a préféré l'inverse : un personnage
/// > lancé vers le haut doit s'accrocher au plafond, comme il s'accroche à
/// > un mur, plutôt que de passer devant et retomber. On assume donc de
/// > diverger de Shimeji-ee sur ce point précis — le raisonnement
/// > d'origine reste vrai pour la source, il ne s'applique simplement plus
/// > ici.
pub fn contact(world: &World, avant: Point, apres: Point) -> Option<(PlatformId, Face, f32)> {
    // Règle 1. `if let Some(…)` et non un `?` : si le sol n'attrape rien, on
    // veut continuer vers les murs, pas sortir.
    if let Some((id, offset)) = atterrissage(world, avant, apres) {
        return Some((id, Face::Top, offset));
    }

    // Règle 2, puis règle 3 : `if let … return` plutôt que `?`, pour la même
    // raison — si les murs n'attrapent rien, on continue vers le plafond au
    // lieu de sortir de la fonction.
    if let Some(m) = contact_mur(world, avant, apres) {
        return Some(m);
    }

    contact_plafond(world, avant, apres)
}

/// Règle 2 : a-t-on traversé la ligne verticale d'un mur, dans le bon sens ?
///
/// Séparée de `contact` pour que la priorité au sol se lise en une ligne
/// plutôt que d'être enfouie dans une boucle.
fn contact_mur(world: &World, avant: Point, apres: Point) -> Option<(PlatformId, Face, f32)> {
    // (id, face, offset, x de la face) — le `x` ne sert qu'à départager.
    let mut meilleur: Option<(PlatformId, Face, f32, f32)> = None;

    for plat in world.platforms() {
        // Les deux faces verticales, dans un tableau : écrire deux fois le
        // même corps de boucle finirait par diverger.
        for face in [Face::Left, Face::Right] {
            if !plat.has_face(face) {
                continue;
            }

            // `point_on(face, 0.0).x` plutôt que `rect.left()` / `rect.right()`
            // écrits à la main : c'est la MÊME fonction qui placera le
            // personnage, donc les deux ne peuvent pas se désaccorder.
            let x_face = plat.rect.point_on(face, 0.0).x;

            // Le bon sens, et c'est le cœur du test. Une face `Left` regarde
            // vers la gauche : on la heurte en allant vers la DROITE. Une
            // face `Right` regarde vers la droite : on la heurte en allant
            // vers la gauche.
            //
            // ⚠️ **Correction de bug, et l'asymétrie strict/large n'est PAS
            // une coquetterie.** Le côté DÉPART (`avant`) est maintenant
            // testé en **strict** (`<` / `>`), alors que le côté ARRIVÉE
            // (`apres`) reste large (`>=` / `<=`). « Franchir » veut dire
            // qu'on était strictement d'un côté avant, et qu'on est passé de
            // l'autre — quelqu'un déjà pile SUR le plan du mur ne franchit
            // rien, il y est déjà.
            //
            // Avec l'ancienne version, large des deux côtés, un personnage
            // qui vient de LÂCHER un mur repart avec `pos.x == x_face`
            // exactement (voir `intention::grimper`, phase `Accroche` :
            // `Falling { pos: plat.rect.point_on(face, offset), .. }`) et une
            // vitesse horizontale nulle. Son `x` ne bouge donc plus d'une
            // image à l'autre pendant que la gravité le fait descendre :
            // `avant.x == apres.x == x_face` satisfaisait quand même
            // `avant.x >= x_face && apres.x <= x_face`, donc il se
            // raccrochait IMMÉDIATEMENT, indéfiniment — impossible de se
            // décoller d'un mur, à la fin d'une escalade (Tâche 4) comme
            // après un lancer (Tâche 5). Rendre le côté départ strict
            // élimine exactement ce cas : `avant.x` pile sur `x_face` ne
            // vérifie plus `avant.x > x_face` (ni `<`), donc `franchie` est
            // `false` et il continue de tomber.
            let franchie = match face {
                Face::Left => avant.x < x_face && apres.x >= x_face,
                Face::Right => avant.x > x_face && apres.x <= x_face,
                // Les faces horizontales ne passent jamais par ici : le
                // tableau ci-dessus n'en contient pas. `false` est le repli
                // muet correct.
                Face::Top | Face::Bottom => false,
            };
            if !franchie {
                continue;
            }

            // Est-on à la hauteur du mur ? Même approximation volontaire que
            // dans `atterrissage` : on teste avec le point d'ARRIVÉE plutôt
            // que le croisement exact. À 15 px par image au maximum, l'écart
            // est invisible, et la navigation a le droit d'être imparfaite
            // (décision n° 4).
            if apres.y < plat.rect.top() || apres.y > plat.rect.bottom() {
                continue;
            }

            // L'offset d'une face verticale compte vers le BAS depuis le haut
            // du rectangle — c'est la convention de `Rect::point_on`.
            let offset = apres.y - plat.rect.top();

            // Départage : garder le mur rencontré le PLUS TÔT, c'est-à-dire
            // le plus proche du point de départ. Le cas ne se présente
            // qu'avec des écrans qui se recouvrent, mais laisser le choix au
            // hasard de l'ordre du `Vec` serait un bug dormant.
            let remplace = match meilleur {
                None => true,
                Some((_, _, _, x)) => (x_face - avant.x).abs() < (x - avant.x).abs(),
            };
            if remplace {
                meilleur = Some((plat.id, face, offset, x_face));
            }
        }
    }

    meilleur.map(|(id, face, offset, _)| (id, face, offset))
}

/// Règle 3 : a-t-on traversé la ligne horizontale d'un plafond, **en
/// montant** ?
///
/// Symétrique de `atterrissage` (le sol), mais dans l'autre sens vertical, et
/// symétrique de `contact_mur` pour l'asymétrie strict/large ci-dessous.
/// Séparée de `contact` pour la même raison que `contact_mur` : la priorité
/// sol → murs → plafond se lit en trois lignes plutôt que d'être enfouie
/// dans une seule boucle géante.
fn contact_plafond(world: &World, avant: Point, apres: Point) -> Option<(PlatformId, Face, f32)> {
    // On ne s'accroche qu'en MONTANT — symétrique de la règle 1 de
    // `atterrissage`, qui n'accepte que la descente. `<` et non `<=` : un
    // `y` inchangé (aucun mouvement vertical) ne doit pas franchir quoi que
    // ce soit.
    if apres.y >= avant.y {
        return None;
    }

    // **Et il faut arriver bien plus verticalement qu'horizontalement.**
    //
    // Ajoutée le 2026-09-14 après une mesure, déclenchée par une observation à
    // l'écran : depuis que le personnage monte plus haut (voir
    // `FROTTEMENT_MONTEE_Y`), une diagonale franche atteignait le plafond et
    // s'y collait net, si bien qu'elle portait **deux fois moins loin**
    // qu'avant — 204 px au lieu de 453 px pour un jet à 60° lâché en haut
    // d'écran. Un personnage qui file de côté ne « s'agrippe » pas au
    // plafond : il le frôle. Seul celui qui monte vraiment vers lui s'accroche.
    //
    // Le critère se lit dans le PAS lui-même — inutile de faire circuler la
    // vitesse jusqu'ici : `apres - avant` vaut déjà `vitesse × dt`, et le `dt`
    // se simplifie des deux côtés de la comparaison. La fonction reste donc
    // purement géométrique, comme `atterrissage` et `contact_mur`.
    //
    // **Le seuil de 2 est mesuré, pas choisi à l'œil.** Pente relevée à
    // l'instant où le personnage franchit la ligne du plafond, pour un lancer
    // à la vitesse maximale :
    //
    // | lâché à | angle du jet | pente au plafond | voulu |
    // |---|---|---|---|
    // | y = 150 | 45° | 0,78 | il passe |
    // | y = 300 | 60° | 1,03 | il passe |
    // | y = 150 | 60° | 1,56 | il passe |
    // | y = 300 | 75° | 2,99 | **il s'accroche** |
    // | y = 150 | 75° | 3,54 | **il s'accroche** |
    //
    // 2 tombe dans le trou entre 1,56 et 2,99, à bonne distance des deux —
    // un seuil à 1 aurait gardé les deux diagonales à 60°.
    //
    // Écrit en produit (`monte < 2 × |dx|`) et non en quotient : un jet
    // parfaitement vertical a `dx = 0`, et la division rendrait `inf`.
    let monte = avant.y - apres.y;
    if monte < PENTE_MIN_PLAFOND * (apres.x - avant.x).abs() {
        return None;
    }

    // ⚠️ Cette règle nous RAPPROCHE de `Fall.java`, qui ne teste jamais le
    // plafond (ni dans `hasNext()`, ni dans la boucle de `tick()`) : elle
    // restreint notre divergence du 2026-09-12 au seul cas qui l'avait
    // motivée — le lancer vers le haut.

    // (id, offset, y de la face) — pas besoin de départager plusieurs
    // plafonds ici : contrairement aux murs, deux plafonds ne peuvent pas se
    // recouvrir verticalement à la même position (ce sont des lignes
    // horizontales à des hauteurs différentes, et `face_voisine` les
    // fusionne déjà en un seul du point de vue du déplacement). On garde
    // quand même la structure `Option` par cohérence avec `contact_mur`.
    let mut meilleur: Option<(PlatformId, f32, f32)> = None;

    for plat in world.platforms() {
        if !plat.has_face(Face::Bottom) {
            continue;
        }

        // `point_on(Face::Bottom, 0.0).y` plutôt que d'écrire `rect.bottom()`
        // à la main : même raison que dans `contact_mur`, c'est la fonction
        // qui placera aussi le personnage.
        let y_face = plat.rect.point_on(Face::Bottom, 0.0).y;

        // ⚠️ **Même asymétrie strict/large que `contact_mur`, et pour
        // exactement la même raison.** Le côté DÉPART (`avant`) est testé en
        // STRICT (`>`), le côté ARRIVÉE (`apres`) reste large (`<=`). Sans
        // ça, un personnage qui vient tout juste de se LÂCHER du plafond
        // repart avec `pos.y == y_face` exactement (voir la phase `Accroche`
        // de `intention::grimper`, qui utilise `plat.rect.point_on(face,
        // offset)`) et une vitesse verticale nulle à l'instant du lâcher :
        // son `y` ne bougerait donc pas d'une image à l'autre pendant que
        // l'élan horizontal se dissipe, et `avant.y >= y_face && apres.y <=
        // y_face` serait satisfait indéfiniment — il se raccrocherait
        // IMMÉDIATEMENT, à chaque image, sans jamais pouvoir quitter le
        // plafond. Rendre le côté départ strict élimine ce cas : `avant.y`
        // pile sur `y_face` ne vérifie plus `avant.y > y_face`, donc
        // `franchie` est `false` et il continue de tomber.
        let franchie = avant.y > y_face && apres.y <= y_face;
        if !franchie {
            continue;
        }

        // Est-on sous le plafond, horizontalement ? Même approximation
        // volontaire que dans `atterrissage` et `contact_mur` : on teste
        // avec le point d'ARRIVÉE plutôt que le croisement exact (décision
        // n° 4, navigation imparfaite autorisée).
        if apres.x < plat.rect.left() || apres.x > plat.rect.right() {
            continue;
        }

        // L'offset d'une face `Bottom` compte vers la DROITE depuis le bord
        // gauche du rectangle, exactement comme au sol — c'est la
        // convention de `Rect::point_on`.
        let offset = apres.x - plat.rect.left();

        // Départage, par cohérence avec `contact_mur` — garder le plafond
        // rencontré le plus tôt. Le cas ne se présente qu'avec des écrans
        // dont les plafonds se chevaucheraient, ce qui n'arrive pas
        // aujourd'hui, mais laisser le hasard de l'ordre du `Vec` trancher
        // serait un bug dormant.
        let remplace = match meilleur {
            None => true,
            Some((_, _, y)) => (y_face - avant.y).abs() < (y - avant.y).abs(),
        };
        if remplace {
            meilleur = Some((plat.id, offset, y_face));
        }
    }

    meilleur.map(|(id, offset, _)| (id, Face::Bottom, offset))
}

/// Le personnage est-il tombé sous le bas du bureau virtuel ?
///
/// C'est le déclencheur du **garde-fou** de la spec §6.3 : passé cette
/// limite, il est replacé sur le sol le plus proche plutôt que de tomber
/// indéfiniment. Sans ce garde-fou, lâcher un personnage à côté de l'écran
/// le perdrait pour de bon.
///
/// Rend `false` si le monde est vide : on ne peut pas être « sous » un bas
/// qui n'existe pas, et surtout il ne faut pas paniquer.
pub fn sous_le_bureau(world: &World, pos: Point) -> bool {
    // `let … else` : monde vide → pas de limite, donc pas de sortie.
    let Some(b) = world.bounds() else {
        return false;
    };

    // Une marge, pour que le garde-fou ne se déclenche pas pendant un
    // atterrissage normal juste au niveau du sol.
    const MARGE: f32 = 200.0;
    pos.y > b.bottom() + MARGE
}

// Les tests de ce module vivent dans `physics_tests.rs`
// (sortis d ici le 2026-09-14 : ils faisaient 488 des 1138 lignes).
#[cfg(test)]
#[path = "physics_tests.rs"]
mod tests;
