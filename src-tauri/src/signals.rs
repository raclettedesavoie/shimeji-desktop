//! Les signaux du système, traduits en biais d'envie (design §7.2, décision
//! n° 3 ; design de l'étape 2 §4).
//!
//! Responsabilité unique : `Signaux` + `Config` → `Biais`. **Rien d'autre.**
//! Aucun appel système, aucun état, aucune horloge — une fonction pure, donc
//! testable en entier sans écran ni attente.
//!
//! > **Le seul verbe autorisé ici est MULTIPLIER.** Si l'on se surprend à
//! > écrire « si inactif alors dormir », la décision n° 3 est perdue et l'on
//! > a livré un afficheur d'état système déguisé en personnage.
//!
//! # Pourquoi un `Biais` plutôt que les `Signaux` eux-mêmes
//!
//! Trois raisons, par ordre d'importance :
//!
//! 1. **`Entrees` reste `Copy`.** L'étape 1a a conçu `Entrees` pour grossir
//!    précisément ici. Y mettre `Signaux`, qui contient une `String`,
//!    casserait son `Copy` et se paierait dans les signatures des trois
//!    couches.
//! 2. **La comparaison de chaînes sort du chemin 60 Hz.** Chercher
//!    `"Code.exe"` dans la table des applications se fait **2 fois par
//!    seconde**, pas 60. Sur un projet qui a mesuré son CPU trois fois de
//!    suite, ce n'est pas un détail de style.
//! 3. **Le point d'entrée existait déjà** : `desire::tirer_avec`, écrite à
//!    l'étape 1a avec son test, prend exactement un multiplicateur par
//!    intention.

use crate::behavior::intention::Intention;
use crate::config::Config;
use crate::probe::Signaux;
use std::time::Duration;

/// Un multiplicateur par intention.
///
/// Un champ par intention plutôt qu'une table associative : elles se comptent
/// sur les doigts, et un `match` exhaustif fait échouer la compilation quand
/// on en ajoute une — ce qui est exactement le rappel qu'on veut.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Biais {
    pub flaner: f32,
    pub se_reposer: f32,
    pub jouer: f32,

    /// Le biais de l'escalade (étape 4a).
    ///
    /// **Aucun signal ne le touche encore**, et il vaut donc toujours 1.0.
    /// Il existe quand même, parce que `pour` est un `match` exhaustif : le
    /// champ manquant, ajouter `Intention::Grimper` ne compilerait pas. C'est
    /// exactement le rappel qu'on attend de cette forme — la table reste
    /// complète, et le jour où « la fenêtre active est un jeu » devra faire
    /// grimper davantage, il n'y aura qu'une multiplication à écrire.
    pub grimper: f32,
}

impl Biais {
    /// Le biais qui ne biaise rien. C'est celui de toute l'étape 1, et celui
    /// que la simulation et la boucle passent tant que la Tâche 6 n'a pas
    /// branché la sonde.
    pub fn neutre() -> Biais {
        Biais {
            flaner: 1.0,
            se_reposer: 1.0,
            jouer: 1.0,
            grimper: 1.0,
        }
    }

    pub fn pour(&self, i: Intention) -> f32 {
        match i {
            Intention::Flaner => self.flaner,
            Intention::SeReposer => self.se_reposer,
            // `Jouer(_)` : le biais ne distingue pas les animations. « jouer
            // ×3 » vaut pour les deux, et les distinguer serait un réglage de
            // plus sans effet observable.
            Intention::Jouer(_) => self.jouer,
            Intention::Grimper => self.grimper,
        }
    }
}

/// Vrai si `heure` tombe dans le créneau `[debut, fin[`.
///
/// ⚠️ **Le créneau par défaut passe par minuit** (22 h → 6 h). Écrit
/// naïvement `heure >= debut && heure < fin`, il serait **toujours faux** :
/// aucune heure n'est à la fois ≥ 22 et < 6. C'est le genre de bug qui ne se
/// voit pas — le signal ne mord jamais, et l'on cherche ailleurs.
fn dans_le_creneau(heure: u8, debut: u8, fin: u8) -> bool {
    if debut <= fin {
        // Créneau ordinaire, par exemple 12 h → 14 h.
        heure >= debut && heure < fin
    } else {
        // Créneau à cheval sur minuit : 22 h → 6 h.
        heure >= debut || heure < fin
    }
}

/// Traduit l'état du système en multiplicateurs d'envie.
///
/// Les modificateurs **se multiplient** entre eux : inactif, le soir, sur
/// batterie faible donne 8 × 3 × 2 = ×48 sur le repos. Additionner donnerait
/// 13 et écraserait la différence entre « un signal » et « tous les
/// signaux ».
pub fn biais_de(s: &Signaux, c: &Config) -> Biais {
    let r = &c.signaux;
    let mut b = Biais::neutre();

    // ── L'utilisateur est parti ─────────────────────────────────────────
    if s.inactivite >= Duration::from_secs_f32(r.inactivite_secondes) {
        b.flaner *= r.inactif_flaner;
        b.se_reposer *= r.inactif_se_reposer;
    }

    // ── Tard le soir, ou la nuit ────────────────────────────────────────
    if dans_le_creneau(s.heure, r.soir_debut, r.soir_fin) {
        b.se_reposer *= r.soir_se_reposer;
    }

    // ── Batterie faible ─────────────────────────────────────────────────
    //
    // `if let Some(p)` : `None` veut dire « pas de batterie », PAS « batterie
    // vide » (voir `Batterie::pourcent`). Et la condition `!sur_secteur` est
    // indispensable : un portable branché à 10 % se recharge, il ne
    // s'épuise pas.
    if let Some(pourcent) = s.batterie.pourcent {
        if !s.batterie.sur_secteur && pourcent < r.batterie_seuil {
            b.se_reposer *= r.batterie_se_reposer;
        }
    }

    // ── Notre propre file de rendu sature ───────────────────────────────
    //
    // Le sixième signal, et le seul qui parle de NOUS et non du système
    // (spec « régulation de charge » §5). Quand le thread principal prend
    // du retard, les déplacements, les menus et le hit-testing en prennent
    // aussi : à onze personnages en debug, l'application devenait
    // inutilisable.
    //
    // ⚠️ **Il multiplie, il n'ordonne pas** (décision n° 3). Des personnages
    // s'assoient l'un après l'autre — chacun au moment où LUI tire sa
    // prochaine envie — et un personnage assis ne poste plus rien : la
    // latence retombe, ce bloc cesse de s'appliquer. Rien à remettre à zéro.
    // ⚠️ **La comparaison se fait en millisecondes flottantes, pas en
    // `Duration`.** `Duration::from_secs_f32(0.1)` vaut 100,000001 ms —
    // `0.1f32` n'est pas exactement un dixième — si bien qu'une latence de
    // 100 ms tout rond restait SOUS un seuil réglé à 100. Le test
    // `le_seuil_de_latence_est_inclusif` est là pour ça, et il a échoué.
    //
    // Et ce détour évite en prime `from_secs_f32`, qui **panique** sur un
    // flottant négatif venu d'un `config.json` écrit à la main.
    //
    // `> 0.0` : un seuil nul ou négatif **éteint** le signal au lieu de
    // l'allumer en permanence. C'est le repli inoffensif — un seuil absurde
    // rend la régulation muette, il n'endort pas tout le monde à jamais.
    let latence_ms = s.latence_file.as_secs_f32() * 1000.0;
    if r.latence_ms_seuil > 0.0 && latence_ms >= r.latence_ms_seuil {
        // ⚠️ **La réponse est GRADUÉE, et la mesure l'a exigé.** Une réponse
        // binaire (×4 dès le seuil) a été mesurée le 2026-09-21 : elle
        // divise la latence par deux, ce qui ne suffit pas quand elle vaut
        // dix secondes. Le signal était « tout allumé » sans aucune notion
        // de gravité, à 100 ms comme à 10 000.
        //
        // `ampleur` vaut 1 au seuil exact — donc le comportement au seuil
        // est inchangé — et croît avec le dépassement, jusqu'au plafond.
        let ampleur = (latence_ms / r.latence_ms_seuil).min(r.latence_facteur_max);
        b.se_reposer *= r.latence_se_reposer * ampleur;

        // Et l'on tarit la source : flâner est l'intention qui MARCHE, donc
        // celle qui poste un déplacement par image. L'encourager au repos
        // sans décourager la marche mettrait vingt secondes à converger.
        b.flaner *= r.latence_flaner;
    }

    // ── L'application au premier plan ───────────────────────────────────
    //
    // `as_ref()` sur l'`Option<String>` pour emprunter la chaîne sans la
    // déplacer ; `get` sur la table accepte un `&str` grâce à `Borrow`.
    if let Some(nom) = s.appli_active.as_ref() {
        if let Some(m) = c.applications.get(nom.as_str()) {
            // `if let Some(x)` sur chaque champ : un modificateur absent
            // laisse le poids intact, il ne le remet pas à zéro.
            if let Some(x) = m.flaner {
                b.flaner *= x;
            }
            if let Some(x) = m.se_reposer {
                b.se_reposer *= x;
            }
            if let Some(x) = m.jouer {
                b.jouer *= x;
            }
        }
    }

    b
}

/// L'utilisateur vient-il de toucher à quelque chose ?
///
/// Vit ici, à côté de `biais_de`, et pas dans les deux boucles (`main.rs` et
/// `sim.rs`) : c'est la **définition** de « actif », et la seule chose que le
/// comportement sait de l'inactivité. Recopiée dans les deux boucles, elle
/// finirait par diverger — et la divergence se lirait « la simulation prouve
/// un comportement que l'application n'a pas », soit précisément ce que le
/// mode simulation existe pour empêcher.
///
/// Même seuil que `biais_de` (`inactiviteSecondes`) : c'est ce qui rend
/// impossible d'être « actif » et « inactif » à la même image.
pub fn utilisateur_actif(s: &Signaux, c: &Config) -> bool {
    s.inactivite < Duration::from_secs_f32(c.signaux.inactivite_secondes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::behavior::intention::Intention;
    use crate::config::{Config, ModifsAppli};
    use crate::probe::{Batterie, Signaux};
    use std::time::Duration;

    /// Le point de départ : personne n'est parti, il est 15 h, la machine est
    /// sur secteur, la session est ouverte. **Aucun signal ne mord.**
    fn rien_de_special() -> Signaux {
        Signaux {
            inactivite: Duration::ZERO,
            appli_active: None,
            heure: 15,
            batterie: Batterie {
                pourcent: None,
                sur_secteur: true,
            },
            session_verrouillee: false,
            latence_file: Duration::ZERO,
        }
    }

    #[test]
    fn sans_signal_le_biais_est_neutre() {
        let b = biais_de(&rien_de_special(), &Config::default());
        assert_eq!(b, Biais::neutre());
        assert_eq!(b.pour(Intention::Flaner), 1.0);
        assert_eq!(b.pour(Intention::SeReposer), 1.0);
    }

    #[test]
    fn inactif_multiplie_le_sommeil_et_divise_la_flanerie() {
        // Les valeurs de la spec §7.2 : flâner ×0,2 et se reposer ×8.
        let mut s = rien_de_special();
        s.inactivite = Duration::from_secs(180); // > 2 min

        let b = biais_de(&s, &Config::default());
        assert_eq!(b.pour(Intention::SeReposer), 8.0);
        assert_eq!(b.pour(Intention::Flaner), 0.2);
    }

    #[test]
    fn le_seuil_d_inactivite_est_franc() {
        // Juste avant les 120 s : rien. Juste après : tout.
        let mut s = rien_de_special();
        s.inactivite = Duration::from_secs(119);
        assert_eq!(biais_de(&s, &Config::default()), Biais::neutre());

        s.inactivite = Duration::from_secs(121);
        assert_ne!(biais_de(&s, &Config::default()), Biais::neutre());
    }

    #[test]
    fn le_creneau_du_soir_passe_par_minuit() {
        // **LE test qui attrape le bug évident.** Le créneau par défaut est
        // 22 h → 6 h : écrit naïvement `heure >= 22 && heure < 6`, il est
        // TOUJOURS faux. Il faut le comparer en OU quand il passe minuit.
        let c = Config::default();
        let soir = |h: u8| {
            let mut s = rien_de_special();
            s.heure = h;
            biais_de(&s, &c).pour(Intention::SeReposer)
        };

        for h in [22, 23, 0, 3, 5] {
            assert_eq!(soir(h), 3.0, "il est {h} h : c'est le soir ou la nuit");
        }
        for h in [6, 12, 18, 21] {
            assert_eq!(soir(h), 1.0, "il est {h} h : ce n'est pas le soir");
        }
    }

    #[test]
    fn pas_de_batterie_n_est_pas_une_batterie_vide() {
        // **Le piège de `GetSystemPowerStatus`** : sur une machine sans
        // batterie, `BatteryLifePercent` vaut 255. Confondre « pas de
        // batterie » et « batterie à plat » ferait traîner un pet sur une
        // tour de bureau, en permanence, sans raison visible.
        let mut s = rien_de_special();
        s.batterie = Batterie {
            pourcent: None,
            sur_secteur: false,
        };
        assert_eq!(biais_de(&s, &Config::default()), Biais::neutre());
    }

    #[test]
    fn la_batterie_faible_ne_fatigue_que_sur_batterie() {
        let c = Config::default();
        let mut s = rien_de_special();

        // 10 % mais branché : il ne se décharge pas, donc rien.
        s.batterie = Batterie {
            pourcent: Some(10),
            sur_secteur: true,
        };
        assert_eq!(biais_de(&s, &c), Biais::neutre());

        // 10 % et débranché : ×2 sur le repos.
        s.batterie = Batterie {
            pourcent: Some(10),
            sur_secteur: false,
        };
        assert_eq!(biais_de(&s, &c).pour(Intention::SeReposer), 2.0);
    }

    #[test]
    fn les_modificateurs_se_multiplient() {
        // Inactif, le soir, sur batterie faible : 8 × 3 × 2 = 48.
        //
        // C'est VOULU (spec §4) : il s'endort quasi certainement — et
        // « quasi » reste le produit. Additionner donnerait 13, ce qui
        // écraserait la différence entre « un signal » et « tous ».
        let mut s = rien_de_special();
        s.inactivite = Duration::from_secs(300);
        s.heure = 23;
        s.batterie = Batterie {
            pourcent: Some(5),
            sur_secteur: false,
        };

        let b = biais_de(&s, &Config::default());
        assert_eq!(b.pour(Intention::SeReposer), 48.0);
        assert_eq!(b.pour(Intention::Flaner), 0.2);
    }

    #[test]
    fn l_appli_active_applique_ses_modificateurs() {
        let mut c = Config::default();
        c.applications.insert(
            "chrome.exe".to_string(),
            // `..Default::default()` et non tous les champs : la Tâche 3
            // ajoutera `jouer` à cette structure, et un littéral exhaustif
            // ferait alors échouer la compilation de CE test — pour rien.
            ModifsAppli {
                flaner: Some(1.5),
                se_reposer: Some(2.0),
                ..Default::default()
            },
        );

        let mut s = rien_de_special();
        s.appli_active = Some("chrome.exe".to_string());

        let b = biais_de(&s, &c);
        assert_eq!(b.pour(Intention::Flaner), 1.5);
        assert_eq!(b.pour(Intention::SeReposer), 2.0);
    }

    #[test]
    fn une_appli_sans_ligne_ne_change_rien() {
        // La table est vide par défaut : n'importe quelle application doit
        // donner un biais neutre, sinon le défaut du fichier de config
        // changerait le caractère du personnage sans que rien ne le dise.
        let mut s = rien_de_special();
        s.appli_active = Some("notepad.exe".to_string());
        assert_eq!(biais_de(&s, &Config::default()), Biais::neutre());
    }

    #[test]
    fn utilisateur_actif_partage_le_seuil_du_biais() {
        // **Le seuil est franc** : juste avant, actif ; juste après, plus.
        // Le même test existe pour `biais_de`
        // (`le_seuil_d_inactivite_est_franc`) — c'est volontaire, ce sont
        // deux fonctions distinctes qui doivent rester d'accord sur le même
        // seuil, sans quoi « actif » et « inactif » pourraient être vrais à
        // la même image.
        let c = Config::default();
        let mut s = rien_de_special();

        s.inactivite = Duration::from_secs(119);
        assert!(utilisateur_actif(&s, &c), "119 s : encore actif");

        s.inactivite = Duration::from_secs(121);
        assert!(!utilisateur_actif(&s, &c), "121 s : plus actif");
    }

    #[test]
    fn un_modificateur_absent_laisse_le_poids_intact() {
        // `ModifsAppli` a des champs `Option` : une ligne qui ne parle que de
        // `flaner` ne doit pas remettre `seReposer` à zéro. Avec des `f32`
        // nus, un champ absent vaudrait 0.0 — donc INTERDIRAIT l'intention,
        // ce qui est le contraire d'un défaut inoffensif.
        let mut c = Config::default();
        c.applications.insert(
            "Code.exe".to_string(),
            ModifsAppli {
                flaner: Some(0.5),
                se_reposer: None,
                ..Default::default()
            },
        );

        let mut s = rien_de_special();
        s.appli_active = Some("Code.exe".to_string());

        let b = biais_de(&s, &c);
        assert_eq!(b.pour(Intention::Flaner), 0.5);
        assert_eq!(b.pour(Intention::SeReposer), 1.0);
    }

    // ── Le sixième signal : notre propre file de rendu ──────────────────

    /// En dessous du seuil, le signal n'existe pas — c'est le cas de toute
    /// machine qui se porte bien, et il doit rester strictement neutre.
    #[test]
    fn une_file_fluide_ne_biaise_rien() {
        let mut s = rien_de_special();
        s.latence_file = Duration::from_millis(5);
        assert_eq!(biais_de(&s, &Config::default()), Biais::neutre());
    }

    /// Au-delà du seuil, le repos est encouragé ET la flânerie découragée —
    /// c'est elle qui marche, donc elle qui déplace une fenêtre à chaque image.
    ///
    /// La réponse est **graduée** : à 250 ms pour un seuil de 100, le
    /// dépassement vaut 2,5, donc le repos est multiplié par 4 × 2,5 = 10.
    #[test]
    fn une_file_saturee_pousse_au_repos() {
        let mut s = rien_de_special();
        s.latence_file = Duration::from_millis(250);

        let b = biais_de(&s, &Config::default());
        assert_eq!(b.se_reposer, 10.0);
        assert_eq!(b.flaner, 0.25);
        // `jouer` n'est pas touché : une animation sur place ne déplace rien.
        assert_eq!(b.jouer, 1.0);
    }

    /// La graduation est plafonnée. Mesurée à quinze personnages, la latence
    /// atteint 10 000 ms — sans plafond, le repos serait multiplié par 400 et
    /// le tirage ne serait plus un tirage mais un ordre (décision n° 3).
    #[test]
    fn la_graduation_est_plafonnee() {
        let mut s = rien_de_special();
        s.latence_file = Duration::from_secs(10);

        let b = biais_de(&s, &Config::default());
        // 4 × 8 (le plafond), et non 4 × 100.
        assert_eq!(b.se_reposer, 32.0);
    }

    /// Au seuil EXACT, la graduation vaut 1 : le comportement y est celui de la
    /// réponse binaire d'origine. C'est ce qui rend le plafond et la pente
    /// réglables sans changer le sens du seuil.
    #[test]
    fn au_seuil_exact_la_graduation_est_neutre() {
        let mut s = rien_de_special();
        s.latence_file = Duration::from_millis(100);
        assert_eq!(biais_de(&s, &Config::default()).se_reposer, 4.0);
    }

    /// Le seuil est une borne INCLUSIVE, comme celui de l'inactivité : deux
    /// signaux voisins qui se compareraient différemment seraient un piège à
    /// la relecture.
    #[test]
    fn le_seuil_de_latence_est_inclusif() {
        let mut s = rien_de_special();
        s.latence_file = Duration::from_millis(100);
        assert_eq!(biais_de(&s, &Config::default()).se_reposer, 4.0);
    }

    /// Un seuil nul ou négatif dans un `config.json` écrit à la main **éteint**
    /// le signal.
    ///
    /// Deux défauts évités d'un coup : `Duration::from_secs_f32` panique sur un
    /// flottant négatif (le défaut déjà corrigé pour `inactiviteSecondes`), et
    /// surtout un seuil de zéro ferait s'endormir TOUS les personnages en
    /// permanence — une panne bien pire qu'un signal muet.
    #[test]
    fn un_seuil_de_latence_absurde_eteint_le_signal() {
        let mut s = rien_de_special();
        s.latence_file = Duration::from_millis(500);

        for seuil in [-5.0, 0.0] {
            let mut c = Config::default();
            c.signaux.latence_ms_seuil = seuil;
            assert_eq!(
                biais_de(&s, &c),
                Biais::neutre(),
                "un seuil de {seuil} doit éteindre le signal, pas l'allumer"
            );
        }
    }
}
