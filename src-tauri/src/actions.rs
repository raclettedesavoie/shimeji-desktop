//! Ce qu'une entrée de menu **fait**, quel que soit le menu qui l'a
//! proposée (spec §9.1).
//!
//! Responsabilité unique : traduire un identifiant d'entrée en effet. Ni
//! construction de menu, ni logique de personnage — seulement le geste.
//!
//! # Pourquoi ce fichier existe
//!
//! Il y a maintenant **deux** menus : celui du tray et celui du clic droit
//! sur le personnage. Deux entrées leur sont communes — le catalogue et
//! quitter. Les écrire deux fois, c'est garantir qu'elles divergeront à la
//! première correction.
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
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use tauri::menu::{CheckMenuItem, MenuItem};
use tauri::{AppHandle, Wry};

// ── Les identifiants ────────────────────────────────────────────────────
//
// Des constantes et non des littéraux : l'identifiant est écrit à la
// construction du menu ET lu ici, donc une faute de frappe donnerait une
// entrée qui ne fait silencieusement rien.

/// Entrées du menu du **tray**.
pub const ID_AFFICHER: &str = "afficher";
pub const ID_DEMARRAGE: &str = "demarrage";
pub const ID_QUITTER: &str = "quitter";

/// L'entrée de mise à jour du tray. **Une seule entrée pour deux états** :
/// au repos elle propose de vérifier, et une fois une version trouvée elle
/// propose de l'installer. Deux entrées diraient deux fois la même chose, et
/// l'une des deux serait toujours inutile.
pub const ID_MAJ: &str = "maj";

/// Proposée par les DEUX menus, comme `quitter` : elle fait exactement la
/// même chose depuis l'un ou l'autre, donc un seul identifiant — et donc un
/// seul cas dans `executer`.
pub const ID_CATALOGUE: &str = "catalogue";

/// Entrée propre au menu du **personnage**.
///
/// `quitter` n'y figure pas : elle fait exactement la même chose depuis les
/// deux menus, donc elle réutilise telle quelle `ID_QUITTER`.
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

    /// L'entrée de mise à jour. Gardée pour la MÊME raison que `afficher` :
    /// son libellé change quand une version est trouvée, et le menu du tray
    /// n'est construit qu'une fois. Le reconstruire à chaud pour changer un
    /// texte serait une source de bugs pour un gain nul.
    pub maj: MenuItem<Wry>,
}

/// Tout ce dont les actions ont besoin pour agir.
///
/// Partagé par `Arc` entre le gestionnaire de menu (thread principal) et la
/// boucle 60 Hz, qui y dépose la boîte aux lettres.
pub struct Actions {
    pub visibilite: Visibilite,
    pub demande: Demande,
    pub commande: BoiteCommande,

    /// Le roster **voulu** : la liste des personnages qui doivent vivre,
    /// avec ses doublons (design §4).
    ///
    /// `Mutex` parce qu'il change en cours d'exécution — la fenêtre de la
    /// bibliothèque en ajoute et en retire.
    roster: Mutex<Vec<String>>,

    /// Les personnages **réellement présents** à l'écran, publiés par la
    /// boucle 60 Hz à chaque réconciliation.
    ///
    /// ⚠️ **Distinct de `roster`, et c'est tout son intérêt.** `roster` dit
    /// ce qu'on veut ; celui-ci dit où en est la boucle. C'est la seule
    /// façon pour la suppression d'un pack de savoir que les fenêtres ont
    /// vraiment disparu avant d'effacer les PNG — plutôt que de le supposer
    /// après un délai fixe (design §7).
    presents: Mutex<Vec<String>>,

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
        roster: Vec<String>,
        commande: BoiteCommande,
    ) -> Arc<Actions> {
        Arc::new(Actions {
            visibilite,
            demande,
            commande,
            // Les présents sont vides au départ : la boucle les publiera à
            // sa première image. Rien ne les lit avant.
            presents: Mutex::new(Vec::new()),
            roster: Mutex::new(roster),
            cases: Mutex::new(None),
        })
    }

    /// Remplace la liste des personnages voulus et demande le chargement.
    ///
    /// Rend la version du rechargement, que la boucle 60 Hz comparera à la
    /// sienne pour savoir qu'il y a du nouveau.
    ///
    /// `sans_animation` : les retraits sautent-ils l'animation de départ ?
    /// Vrai pour la seule suppression d'un pack du disque — voir le champ du
    /// même nom sur `Rechargement`.
    pub fn definir_roster(&self, voulus: &[String], sans_animation: bool) -> Result<u64, String> {
        // Les entrées-sorties D'ABORD, verrou non tenu : si un manifeste est
        // illisible, on sort sans avoir rien touché et les personnages
        // continuent avec ce qu'ils avaient. Tenir le verrou pendant une
        // lecture de fichier bloquerait en plus la boucle 60 Hz pour rien.
        let version = crate::rechargement::preparer_roster(&self.demande, voulus, sans_animation)?;

        // ── L'avertissement à 10 (design §2) ────────────────────────────
        //
        // Posé ICI et non dans `definir_compte`, pour qu'il sorte quelle que
        // soit la voie empruntée — le clic dans la bibliothèque comme
        // `SHIMEJI_PERSONNAGES`. C'est la règle du projet : l'équivalent
        // scriptable doit être équivalent, pas presque.
        //
        // ⚠️ Il AVERTIT, il n'interdit pas : aucun plafond, aucun refus.
        // Décision de l'auteur prise en connaissance de la mesure.
        if voulus.len() >= crate::commandes::SEUIL_AVERTISSEMENT {
            println!(
                "⚠️  {} personnages à l'écran. Chacun qui marche consomme du \
                 processeur ; à ce nombre, la consommation peut devenir notable.",
                voulus.len()
            );
        }

        match self.roster.lock() {
            Ok(mut r) => {
                *r = voulus.to_vec();
                Ok(version)
            }
            Err(_) => Err("verrou du roster empoisonné".to_string()),
        }
    }

    /// La liste voulue en ce moment, doublons compris.
    ///
    /// `unwrap_or_default` : un verrou empoisonné rend une liste vide, et
    /// l'appelant affichera une bibliothèque vide plutôt que de paniquer
    /// dans un gestionnaire de menu — ce qui tuerait le thread d'interface.
    pub fn roster(&self) -> Vec<String> {
        self.roster.lock().map(|r| r.clone()).unwrap_or_default()
    }

    /// La boucle publie ici ce qui vit RÉELLEMENT à l'écran.
    ///
    /// Appelé à chaque réconciliation, donc au plus 8 fois par seconde, et
    /// jamais à 60 Hz : le verrou n'est pas sur le chemin chaud.
    pub fn publier_presents(&self, noms: Vec<String>) {
        if let Ok(mut p) = self.presents.lock() {
            *p = noms;
        }
    }

    /// Combien d'exemplaires de ce nom vivent réellement à l'écran.
    ///
    /// C'est ce qu'attend la suppression avant d'effacer les fichiers.
    pub fn acteurs_nommes(&self, nom: &str) -> usize {
        self.presents
            .lock()
            .map(|p| p.iter().filter(|n| n.as_str() == nom).count())
            // Verrou empoisonné : on rend 0 plutôt que de bloquer la
            // suppression à jamais. Le pire cas est d'effacer un peu tôt, ce
            // qui n'arrive que si la boucle a déjà paniqué.
            .unwrap_or(0)
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

    /// Remet la case « Démarrer avec Windows » du tray d'accord avec la
    /// réalité.
    ///
    /// Le jumeau exact de `resynchroniser_affichage`, et pour la même raison :
    /// quand l'assistant de première configuration active le démarrage
    /// automatique, la case du tray doit suivre. Sinon l'utilisateur coche
    /// dans l'assistant, ouvre le tray, et y lit « Démarrer avec Windows »
    /// décoché — un mensonge affiché en permanence.
    pub fn resynchroniser_demarrage(&self, actif: bool) {
        let Ok(cases) = self.cases.lock() else {
            return;
        };
        // `let … else` : sans tray installé, il n'y a rien à resynchroniser.
        let Some(cases) = cases.as_ref() else {
            return;
        };

        let _ = cases.demarrage.set_checked(actif);
    }

    /// Annonce, dans le menu du tray, qu'une version est disponible.
    ///
    /// **Le libellé EST l'état.** On ne stocke la version nulle part
    /// ailleurs : la garder en double dans `Actions` créerait une seconde
    /// vérité à tenir d'accord avec ce que l'utilisateur lit — exactement ce
    /// que l'interrupteur de la bibliothèque s'interdit déjà.
    pub fn signaler_maj(&self, version: &str) {
        let Ok(cases) = self.cases.lock() else {
            return;
        };
        // `let … else` : sans tray installé, il n'y a rien à annoncer.
        let Some(cases) = cases.as_ref() else {
            return;
        };

        let _ = cases.maj.set_text(format!("Mettre à jour vers la v{version}"));
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
        ID_CATALOGUE => {
            ouvrir_catalogue(app);
        }

        ID_MAJ => {
            // `installer` revérifie d'abord : que l'entrée dise « Vérifier »
            // ou « Mettre à jour », le geste est le même, et c'est pour ça
            // qu'il n'y a qu'un identifiant. La seule différence entre les
            // deux états est ce que l'utilisateur en attend.
            crate::maj::installer(app.clone());
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
pub(crate) fn appliquer_visibilite(actions: &Actions, app: &AppHandle, visible: bool) {
    // L'ordre compte : on prévient d'abord la boucle, pour qu'elle arrête de
    // dessiner, puis on cache. L'inverse laisserait une image poussée à une
    // fenêtre déjà masquée — inoffensif, mais gratuit.
    actions.visibilite.store(visible, Ordering::Relaxed);
    crate::tray::basculer_visibilite(app, visible);
    actions.resynchroniser_affichage(visible);
}

/// Écrit (ou retire) la clé de démarrage automatique, et rend l'état
/// **réellement** obtenu — qui diffère du voulu si l'écriture a échoué.
pub(crate) fn appliquer_demarrage(voulu: bool) -> bool {
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

/// Ouvre la fenêtre du catalogue, ou la ramène au premier plan.
///
/// Tout l'inverse de la fenêtre du personnage (spec §9) : décorée,
/// redimensionnable, focalisable, présente dans la barre des tâches. C'est
/// une fenêtre d'application ordinaire, et il faut résister à la tentation
/// de lui réutiliser quoi que ce soit de l'autre.
///
/// ⚠️ Le label `catalogue` n'est pas décoratif : il doit correspondre
/// **exactement** à celui déclaré dans `capabilities/catalogue.json`, sinon
/// la capacité ne s'applique pas et les appels `invoke` sont refusés **en
/// silence** — le piège de `render.rs:195-225`.
pub fn ouvrir_catalogue(app: &AppHandle) {
    const LABEL: &str = "catalogue";

    // Déjà ouverte : on la remonte plutôt que d'en créer une seconde.
    // `get_webview_window` rend une `Option` — `if let Some` est le `match`
    // dont la branche `None` continuerait.
    if let Some(existante) = tauri::Manager::get_webview_window(app, LABEL) {
        let _ = existante.show();
        let _ = existante.set_focus();
        return;
    }

    // Créée à la DEMANDE et détruite à la fermeture, jamais masquée : une
    // fenêtre WebView2 vivante coûte de la mémoire pour rien, et le
    // catalogue s'ouvre quelques fois dans une vie.
    let resultat = tauri::WebviewWindowBuilder::new(
        app,
        LABEL,
        tauri::WebviewUrl::App("catalogue.html".into()),
    )
    .title("Catalogue de personnages")
    .inner_size(1000.0, 700.0)
    .min_inner_size(520.0, 400.0)
    .resizable(true)
    .build();

    if let Err(e) = resultat {
        // On ne panique pas : ne pas pouvoir ouvrir le catalogue n'est pas
        // une raison de tuer le personnage, qui lui tourne très bien.
        eprintln!("catalogue : ouverture impossible — {e}");
    }
}

/// Applique un écran de démarrage (spec « application distribuable » §2).
///
/// **Aucun mécanisme nouveau** : les trois branches appellent du code qui
/// existait déjà. C'est ce qui rend ce réglage presque gratuit.
///
/// Appelée à DEUX endroits, et c'est pour cela qu'elle est une fonction :
/// au démarrage (`main.rs`, d'après la config) et à la fin de l'assistant,
/// pour que le choix se voie tout de suite au lieu d'attendre le prochain
/// lancement — un réglage qui ne fait rien tant qu'on n'a pas redémarré
/// paraît cassé.
pub fn appliquer_ecran(actions: &Actions, app: &AppHandle, ecran: crate::config::EcranDemarrage) {
    use crate::config::EcranDemarrage;

    match ecran {
        EcranDemarrage::Gestionnaire => ouvrir_catalogue(app),

        // Rien à faire : les personnages vivent déjà, aucune fenêtre ne
        // s'ouvre. C'est le comportement de toutes les versions d'avant.
        EcranDemarrage::Personnages => {}

        // Exactement ce que fait `SHIMEJI_CACHE=1`, et exactement ce que fait
        // décocher « Afficher » dans le tray — d'où l'appel au même helper,
        // qui remet aussi la case d'accord.
        EcranDemarrage::Tray => appliquer_visibilite(actions, app, false),
    }
}

/// Ouvre l'assistant de première configuration (spec §3).
///
/// Une fenêtre d'application ordinaire, comme le gestionnaire — mais **non
/// redimensionnable** : ses trois écrans ont une taille fixe, et rien n'y
/// gagne à être étiré.
///
/// ⚠️ Le label `onboarding` doit correspondre EXACTEMENT à celui déclaré
/// dans `capabilities/onboarding.json`, sinon les appels `invoke` sont
/// refusés **en silence** — le même piège que pour le catalogue.
pub fn ouvrir_onboarding(app: &AppHandle) {
    const LABEL: &str = "onboarding";

    // Déjà ouverte : on la remonte plutôt que d'en créer une seconde.
    if let Some(existante) = tauri::Manager::get_webview_window(app, LABEL) {
        let _ = existante.show();
        let _ = existante.set_focus();
        return;
    }

    let resultat = tauri::WebviewWindowBuilder::new(
        app,
        LABEL,
        tauri::WebviewUrl::App("onboarding.html".into()),
    )
    .title("Bienvenue")
    .inner_size(520.0, 460.0)
    .resizable(false)
    .center()
    .build();

    if let Err(e) = resultat {
        // On ne panique pas : ne pas pouvoir accueillir l'utilisateur n'est
        // pas une raison de tuer le personnage, qui lui tourne très bien.
        // L'assistant se représentera au prochain lancement, la clé
        // `premiereConfigurationFaite` n'ayant pas été écrite.
        eprintln!("assistant : ouverture impossible — {e}");
    }
}
