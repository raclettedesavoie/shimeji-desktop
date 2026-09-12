//! Ce qu'une entrée de menu **fait**, quel que soit le menu qui l'a
//! proposée (spec §9.1).
//!
//! Responsabilité unique : traduire un identifiant d'entrée en effet. Ni
//! construction de menu, ni logique de personnage — seulement le geste.
//!
//! # Pourquoi ce fichier existe
//!
//! Il y a maintenant **deux** menus : celui du tray et celui du clic droit
//! sur le personnage. Quatre entrées leur sont communes — cacher, recharger,
//! ouvrir le dossier, quitter. Les écrire deux fois, c'est garantir qu'elles
//! divergeront à la première correction.
//!
//! # ⚠️ Et surtout : il ne peut y avoir QU'UN SEUL gestionnaire
//!
//! Tauri documente que tout gestionnaire de menu reçoit **tous** les
//! événements de menu, « whether it is coming from this window, another
//! window or from the tray icon menu » (`tauri-2.11.5`,
//! `src/tray/mod.rs:326`) — la répartition se fait sur l'identifiant, jamais
//! sur la fenêtre d'origine.
//!
//! Donner son propre gestionnaire au menu contextuel ferait donc exécuter
//! **chaque action deux fois**. Pour une bascule — afficher/cacher, démarrage
//! avec Windows — deux exécutions s'annulent : le clic paraîtrait sans effet,
//! et le diagnostic serait long. Pour « Quitter », la seconde exécution
//! arriverait sur un processus déjà en train de mourir. D'où la règle, qui
//! n'est pas une préférence de style :
//!
//! > **Un seul `on_menu_event` dans tout le programme**, celui que `tray.rs`
//! > installe, et il délègue ici.

use crate::rechargement::Demande;
use crate::tray::Visibilite;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tauri::menu::CheckMenuItem;
use tauri::{AppHandle, Wry};

// ── Les identifiants ────────────────────────────────────────────────────
//
// Des constantes et non des littéraux : l'identifiant est écrit à la
// construction du menu ET lu ici, donc une faute de frappe donnerait une
// entrée qui ne fait silencieusement rien.

/// Entrées du menu du **tray**.
pub const ID_AFFICHER: &str = "afficher";
pub const ID_RECHARGER: &str = "recharger";
pub const ID_DEMARRAGE: &str = "demarrage";
pub const ID_DOSSIER: &str = "dossier";
pub const ID_QUITTER: &str = "quitter";

/// Entrée propre au menu du **personnage**.
///
/// `recharger`, `dossier` et `quitter` n'y figurent pas : elles font
/// exactement la même chose depuis les deux menus, donc elles réutilisent
/// telles quelles `ID_RECHARGER`, `ID_DOSSIER` et `ID_QUITTER`.
///
/// Seule la visibilité a besoin d'un identifiant distinct, parce qu'elle ne
/// se lit pas de la même façon : côté tray c'est une **case à cocher** dont
/// on lit l'état, côté personnage c'est un « Cacher » sec — voir son
/// commentaire dans `executer`.
pub const ID_P_CACHER: &str = "perso.cacher";

/// La boîte aux lettres qui porte une commande choisie au menu jusqu'à la
/// boucle 60 Hz.
///
/// **Même motif que `rechargement::Demande`, et pour la même raison** : le
/// personnage est possédé par le thread de la boucle, le clic de menu arrive
/// sur le thread principal de Tauri. `Option` plutôt qu'un drapeau séparé —
/// `take()` consomme la commande atomiquement, il n'y a rien à remettre à
/// zéro.
///
/// `menu_perso::Commande` est `Copy` et minuscule (une `Intention` ou une
/// étiquette sans donnée) : le verrou n'est jamais tenu plus que le temps
/// d'une affectation, donc aucun risque de faire attendre les 60 Hz.
///
/// Nommée `BoiteCommande` et non `Commande` : ce dernier nom désigne
/// maintenant le CONTENU de la boîte (`menu_perso::Commande`), qui couvre
/// une intention ordinaire aussi bien que les trois actions propres au menu
/// de l'escalade (« Rester accroché », « Redescendre », « Se lâcher »). Les
/// deux vivraient mal sous le même nom dans deux modules différents utilisés
/// côte à côte.
pub type BoiteCommande = Arc<Mutex<Option<crate::menu_perso::Commande>>>;

pub fn nouvelle_commande() -> BoiteCommande {
    Arc::new(Mutex::new(None))
}

/// Les deux cases à cocher du menu du tray.
///
/// `afficher` est gardée pour une raison précise : quand le menu **du
/// personnage** cache les personnages, la case du tray doit suivre. Sinon
/// l'utilisateur cache par le clic droit, ouvre le tray, et y lit « Afficher
/// les personnages » toujours coché — un mensonge affiché en permanence,
/// exactement ce que le gestionnaire de `demarrage` s'interdit déjà en cas
/// d'échec d'écriture.
///
/// `demarrage` ne sert qu'au tray lui-même (lire son propre état coché) : le
/// menu du personnage ne propose pas ce réglage, c'est un réglage du système
/// et non une humeur du personnage.
#[derive(Clone)]
pub struct CasesTray {
    pub afficher: CheckMenuItem<Wry>,
    pub demarrage: CheckMenuItem<Wry>,
}

/// Tout ce dont les actions ont besoin pour agir.
///
/// Partagé par `Arc` entre le gestionnaire de menu (thread principal) et la
/// boucle 60 Hz, qui y dépose la boîte aux lettres.
pub struct Actions {
    pub visibilite: Visibilite,
    pub demande: Demande,
    pub dossier: PathBuf,
    pub personnage: String,
    pub commande: BoiteCommande,

    /// Renseignées par `tray::installer` **après** la construction du menu :
    /// les cases n'existent pas avant. `Mutex<Option<…>>` et non un champ
    /// obligatoire — si le tray échoue à s'installer, les actions doivent
    /// continuer de marcher depuis le menu du personnage.
    cases: Mutex<Option<CasesTray>>,
}

impl Actions {
    pub fn nouvelles(
        visibilite: Visibilite,
        demande: Demande,
        dossier: PathBuf,
        personnage: String,
        commande: BoiteCommande,
    ) -> Arc<Actions> {
        Arc::new(Actions {
            visibilite,
            demande,
            dossier,
            personnage,
            commande,
            cases: Mutex::new(None),
        })
    }

    pub fn enregistrer_cases(&self, cases: CasesTray) {
        // `if let Ok` : un verrou empoisonné ne doit pas faire paniquer
        // l'installation du tray. On perdrait seulement la synchronisation
        // des cases, ce qui est cosmétique.
        if let Ok(mut c) = self.cases.lock() {
            *c = Some(cases);
        }
    }

    /// Remet la case « Afficher » du tray d'accord avec la réalité.
    fn resynchroniser_affichage(&self, visible: bool) {
        let Ok(cases) = self.cases.lock() else {
            return;
        };
        // `let … else` : sans tray installé, il n'y a rien à resynchroniser.
        let Some(cases) = cases.as_ref() else {
            return;
        };

        let _ = cases.afficher.set_checked(visible);
    }
}

/// Exécute l'entrée `id`. **Le seul endroit du programme qui le fait.**
///
/// `cases_du_tray` est la paire de cases du menu du tray, passée par
/// `tray.rs` pour ses deux entrées à cocher, qui ont besoin de lire leur
/// propre état. Les entrées du menu du personnage ne s'en servent pas : il
/// n'en propose aucune à cocher, précisément pour n'avoir aucun état à
/// synchroniser.
///
/// Un identifiant inconnu est **signalé** et non ignoré : il ne peut venir
/// que d'une entrée ajoutée sans son cas ici.
pub fn executer(actions: &Actions, app: &AppHandle, id: &str, cases_du_tray: &CasesTray) {
    match id {
        // ── Les entrées du tray ─────────────────────────────────────────
        ID_AFFICHER => {
            // `is_checked` rend l'état APRÈS le clic : c'est directement la
            // visibilité voulue.
            let visible = cases_du_tray.afficher.is_checked().unwrap_or(true);
            appliquer_visibilite(actions, app, visible);
        }

        ID_DEMARRAGE => {
            let voulu = cases_du_tray.demarrage.is_checked().unwrap_or(false);
            let reel = appliquer_demarrage(voulu);

            // On remet la case dans son état RÉEL : laisser une case cochée
            // alors que l'écriture a échoué serait un mensonge affiché en
            // permanence.
            if reel != voulu {
                let _ = cases_du_tray.demarrage.set_checked(reel);
            }
        }

        // ── Les entrées du menu du personnage ───────────────────────────
        ID_P_CACHER => {
            // Toujours « cacher », jamais « afficher » : on ne peut pas
            // faire un clic droit sur un personnage invisible. L'entrée n'a
            // donc pas d'état à lire, et c'est ce qui lui évite d'être une
            // case à cocher.
            appliquer_visibilite(actions, app, false);
        }

        // ── Les entrées communes aux deux menus ─────────────────────────
        ID_RECHARGER => {
            match crate::rechargement::preparer(
                &actions.demande,
                &actions.dossier,
                &actions.personnage,
            ) {
                Ok(v) => println!("rechargement demandé (version {v})"),
                // **Bruyant.** Un rechargement silencieusement raté est le
                // pire des cas : on croit tester son nouveau timing et on
                // regarde l'ancien.
                Err(e) => eprintln!("rechargement impossible : {e}"),
            }
        }

        ID_DOSSIER => {
            // `explorer` plutôt qu'un plugin Tauri : c'est une ligne, ça
            // n'ajoute aucune dépendance, et l'échec (dossier absent) n'a pas
            // de conséquence.
            let _ = std::process::Command::new("explorer")
                .arg(&actions.dossier)
                .spawn();
        }

        ID_QUITTER => {
            // Proposée par les DEUX menus. `exit` termine le processus ; les
            // threads des boucles 60 Hz meurent avec lui — ils ne détiennent
            // aucune ressource à libérer proprement, seulement un `AppHandle`.
            app.exit(0);
        }

        // ── Une envie ou une action demandée par le menu du personnage ───
        autre => match crate::menu_perso::commande_de(autre) {
            Some(commande) => deposer_commande(actions, commande),
            None => eprintln!("entrée de menu non gérée : {autre}"),
        },
    }
}

/// Montre ou cache les personnages, et remet la case du tray d'accord.
fn appliquer_visibilite(actions: &Actions, app: &AppHandle, visible: bool) {
    // L'ordre compte : on prévient d'abord la boucle, pour qu'elle arrête de
    // dessiner, puis on cache. L'inverse laisserait une image poussée à une
    // fenêtre déjà masquée — inoffensif, mais gratuit.
    actions.visibilite.store(visible, Ordering::Relaxed);
    crate::tray::basculer_visibilite(app, visible);
    actions.resynchroniser_affichage(visible);
}

/// Écrit (ou retire) la clé de démarrage automatique, et rend l'état
/// **réellement** obtenu — qui diffère du voulu si l'écriture a échoué.
fn appliquer_demarrage(voulu: bool) -> bool {
    let resultat = if voulu {
        crate::autostart::activer()
    } else {
        crate::autostart::desactiver()
    };

    match resultat {
        Ok(()) => {
            println!(
                "démarrage avec Windows : {}",
                if voulu { "activé" } else { "désactivé" }
            );
            voulu
        }
        Err(e) => {
            eprintln!("démarrage automatique : {e}");
            !voulu
        }
    }
}

/// Dépose la commande voulue dans la boîte aux lettres de la boucle 60 Hz.
///
/// On écrase une éventuelle commande non encore consommée : deux clics
/// rapprochés doivent donner le **dernier** voulu, pas une file d'attente
/// qui les jouerait tous les deux.
fn deposer_commande(actions: &Actions, commande: crate::menu_perso::Commande) {
    match actions.commande.lock() {
        Ok(mut boite) => {
            *boite = Some(commande);
            println!("commande du menu : {commande:?}");
        }
        // `PoisonError` : la boucle a paniqué en tenant le verrou. Une
        // commande perdue est alors le moindre des soucis.
        Err(_) => eprintln!("verrou de commande empoisonné"),
    }
}
