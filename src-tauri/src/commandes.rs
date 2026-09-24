//! Les commandes appelées par la fenêtre du catalogue (spec §9).
//!
//! Responsabilité unique : exposer trois gestes au webview. Toute la logique
//! est ailleurs — ici on ne fait que traduire un appel JS en appel Rust.
//!
//! ⚠️ Ce module est le SEUL du projet à utiliser l'IPC de Tauri. Le
//! personnage, lui, passe par `eval` (voir `render.rs`), qui ne va que de
//! Rust vers JS et n'exige aucune permission. Le catalogue, devant appeler
//! Rust, a besoin de l'IPC — donc d'une capacité, déclarée dans
//! `capabilities/catalogue.json` et **portée à cette seule fenêtre**.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// Un pack présent sur le disque, tel que la fenêtre a besoin de le connaître.
///
/// `Serialize` et non `Deserialize` : la donnée ne circule que dans un sens,
/// de Rust vers le webview.
#[derive(Serialize)]
pub struct PackInstalle {
    pub nom: String,

    /// Combien d'exemplaires vivent à l'écran. **0 = éteint.**
    ///
    /// Le champ `actif: bool` d'avant a disparu : `compte > 0` le dit, et
    /// deux champs qui disent la même chose finissent par se contredire.
    pub compte: usize,

    /// La poubelle est-elle active ? Voir `config::est_dans_la_bibliotheque`.
    pub supprimable: bool,
}

/// Installe un pack depuis le catalogue.
///
/// `async` parce que 46 téléchargements prennent environ 3 s : une commande
/// bloquante figerait la fenêtre du catalogue pendant tout ce temps
/// (spec §9).
///
/// L'événement `installation` porte la progression. Un seul événement, une
/// seule forme — `{slug, fait, total}`.
#[tauri::command]
pub async fn installer(app: AppHandle, slug: String) -> Result<(), String> {
    // `spawn_blocking` : le téléchargement est du travail BLOQUANT (WinHttp
    // n'est pas asynchrone). Le laisser sur l'exécuteur async figerait les
    // autres commandes.
    //
    // Les deux `clone` sont indispensables : la closure est `move`, donc
    // elle PREND ce qu'elle utilise. Sans eux, `slug` partirait dans la
    // tâche et le message d'erreur final ne pourrait plus le nommer.
    let slug_pour_tache = slug.clone();
    let app_pour_tache = app.clone();

    tauri::async_runtime::spawn_blocking(move || {
        crate::catalogue::installer(&slug_pour_tache, &mut |fait, total| {
            // L'échec d'émission est ignoré : la fenêtre a pu être fermée
            // pendant le téléchargement, ce qui n'est pas une raison
            // d'interrompre celui-ci.
            let _ = app_pour_tache.emit(
                "installation",
                serde_json::json!({ "slug": slug_pour_tache, "fait": fait, "total": total }),
            );
        })
    })
    .await
    // Deux erreurs différentes se succèdent ici : la tâche a pu être
    // annulée (le premier `?`), puis l'installation a pu échouer (le
    // `map` porte sur son Result à elle).
    .map_err(|e| format!("tâche d'installation interrompue : {e}"))?
    .map(|_chemin| ())
}

/// Ce que contient la bibliothèque, plus le dossier livré.
///
/// Les deux racines sont listées, et la bibliothèque gagne en cas d'homonyme
/// — le même ordre qu'à la résolution (spec §5), sans quoi la liste
/// mentirait sur ce qui s'afficherait réellement.
#[tauri::command]
pub fn bibliotheque(
    actions: tauri::State<'_, std::sync::Arc<crate::actions::Actions>>,
) -> Vec<PackInstalle> {
    // Le roster VOULU et non la config relue : c'est lui qui fait foi entre
    // deux écritures du fichier, et c'est lui que les boutons viennent de
    // modifier. Relire le disque ferait clignoter le compteur.
    let roster = actions.roster();

    // Un `BTreeSet` : il dédoublonne les homonymes des deux racines ET
    // range par ordre alphabétique, ce qui donne une liste stable d'un
    // affichage à l'autre. Un `HashSet` rendrait un ordre différent à
    // chaque ouverture de la fenêtre.
    let mut noms: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    // `into_iter().flatten()` : on parcourt les racines existantes et on
    // ignore silencieusement celles qui manquent — une bibliothèque vide
    // n'est pas une erreur. `flatten` sur un itérateur d'`Option` laisse
    // tomber les `None`.
    let racines = [
        crate::config::dossier_bibliotheque(),
        Some(crate::config::dossier_personnages()),
    ];
    for racine in racines.into_iter().flatten() {
        let Ok(entrees) = std::fs::read_dir(&racine) else {
            continue;
        };
        for entree in entrees.flatten() {
            // Un personnage est un dossier contenant un mascot.json. Ce test
            // écarte d'un coup les dossiers `.partiel` d'une installation
            // interrompue et tout fichier égaré.
            if entree.path().join("mascot.json").is_file() {
                if let Some(nom) = entree.file_name().to_str() {
                    noms.insert(nom.to_string());
                }
            }
        }
    }

    noms.into_iter()
        .map(|nom| PackInstalle {
            // `compte_de` est la SEULE définition du compteur : le nombre
            // d'occurrences dans le multi-ensemble, et rien d'autre n'est
            // stocké à côté (design §4).
            compte: crate::roster::compte_de(&roster, &nom),
            supprimable: crate::config::est_dans_la_bibliotheque(&nom),
            nom,
        })
        .collect()
}

/// Combien d'exemplaires de ce personnage doivent vivre à l'écran.
///
/// **Une seule commande pour les quatre gestes** de la bibliothèque
/// (design §4) : le clic sur la carte appelle `n + 1`, le bouton moins
/// appelle `n - 1`, l'interrupteur appelle `0` ou `1`. L'interrupteur n'est
/// donc que le reflet de `compte > 0` — aucun état caché, donc rien qui
/// puisse se désynchroniser, et `config.personnages` dit toute la vérité.
///
/// Le changement est **immédiat**, sans redémarrage : le roster déposé est
/// ramassé par la boucle 60 Hz, qui réconcilie à l'image suivante.
///
/// `State<...>` : Tauri injecte ici ce que `main.rs` a confié à `manage`. Le
/// type demandé doit correspondre **exactement** à celui qui a été confié
/// (`Arc<Actions>`), sinon la commande échoue à l'exécution et non à la
/// compilation.
#[tauri::command]
pub fn definir_compte(
    actions: tauri::State<'_, std::sync::Arc<crate::actions::Actions>>,
    nom: String,
    combien: usize,
) -> Result<(), String> {
    // On refuse un personnage introuvable AVANT tout le reste : sinon
    // l'application ne redémarrerait plus, le chargement échouant sur un nom
    // qui ne résout pas.
    if combien > 0 && crate::config::dossier_du_personnage(&nom).is_none() {
        return Err(format!("personnage « {nom} » introuvable"));
    }

    let voulus = roster_avec(&actions.roster(), &nom, combien);

    // L'ordre compte : on CHARGE d'abord, on ENREGISTRE ensuite. Si un
    // manifeste est illisible, rien n'a changé — ni à l'écran, ni dans la
    // config, qui aurait sinon nommé un personnage qui ne charge pas.
    actions.definir_roster(&voulus, false)?;
    crate::config::definir_personnages(&voulus)?;

    println!("roster : {voulus:?}");
    Ok(())
}

/// Retire UN exemplaire de `nom` du roster voulu, et l'enregistre — le
/// « Cacher ce personnage » du clic droit (2026-09-23).
///
/// C'est le « − » de la bibliothèque, par le même chemin que
/// `definir_compte` : charger d'abord, enregistrer ensuite. **Quel**
/// exemplaire part n'est pas décidé ici : la boucle l'a déjà mis en départ
/// avant d'appeler, et un acteur en départ ne compte plus comme présent — la
/// réconciliation qui suit ne trouve donc rien de plus à retirer.
///
/// ⚠️ **Fait des entrées-sorties** (manifestes, `config.json`) : à appeler
/// depuis un thread à part, jamais depuis la boucle 60 Hz.
pub fn retirer_un_exemplaire(
    actions: &crate::actions::Actions,
    nom: &str,
) -> Result<(), String> {
    let actuel = actions.roster();
    // `saturating_sub` : 0 − 1 reste 0 au lieu de déborder. Un nom absent
    // du roster (déjà retiré par la bibliothèque entre-temps) ne change rien.
    let reste = crate::roster::compte_de(&actuel, nom).saturating_sub(1);
    let voulus = roster_avec(&actuel, nom, reste);

    actions.definir_roster(&voulus, false)?;
    crate::config::definir_personnages(&voulus)?;

    println!("roster : {voulus:?}");
    Ok(())
}

/// À partir de combien de personnages la bibliothèque avertit.
///
/// La loi mesurée est `CPU ~= 0,9 + 0,3 x placements/s` : à 10 personnages,
/// ~72 % d'un cœur au taux observé. Voir le design §2 — et la réserve sur la
/// file de déplacements, qui n'est pas de la consommation mais du retard.
pub const SEUIL_AVERTISSEMENT: usize = 10;

/// Le roster où `nom` apparaît exactement `combien` fois.
///
/// Fonction **pure**, extraite pour être testable sans Tauri : les commandes
/// ont besoin d'un `State`, que `cargo test` ne peut pas fabriquer.
///
/// L'ordre des autres noms est préservé — c'est un multi-ensemble, mais un
/// `config.json` relu par un humain gagne à ne pas voir ses lignes danser à
/// chaque clic.
pub fn roster_avec(actuel: &[String], nom: &str, combien: usize) -> Vec<String> {
    let mut voulus: Vec<String> = actuel
        .iter()
        .filter(|n| n.as_str() != nom)
        .cloned()
        .collect();
    for _ in 0..combien {
        voulus.push(nom.to_string());
    }
    voulus
}

/// Supprime un pack du disque. **Définitivement, et sans corbeille.**
///
/// L'ordre n'est pas un détail (design §7) :
///   1. mettre le compte à 0 — retrait **immédiat, sans l'animation de
///      départ** ;
///   2. attendre que ses acteurs aient réellement disparu ;
///   3. effacer le dossier.
///
/// Effacer les PNG pendant qu'une fenêtre les réclame encore par le schéma
/// `shime://` donnerait un personnage à moitié dessiné en pleine chute. Et un
/// adieu animé sur un geste irréversible serait de toute façon déplacé : la
/// suppression est brutale parce qu'elle est définitive.
#[tauri::command]
pub async fn supprimer(
    actions: tauri::State<'_, std::sync::Arc<crate::actions::Actions>>,
    nom: String,
) -> Result<(), String> {
    // On refuse AVANT d'avoir rien retiré de l'écran : sinon un pack du dépôt
    // disparaîtrait de la vue sans être supprimé, et l'utilisateur croirait à
    // une suppression réussie.
    if !crate::config::est_dans_la_bibliotheque(&nom) {
        return Err(format!(
            "« {nom} » n'est pas dans votre bibliothèque : il est livré avec l'application"
        ));
    }

    // 1. Le retirer de l'écran, SANS animation de départ.
    let voulus = roster_avec(&actions.roster(), &nom, 0);
    actions.definir_roster(&voulus, true)?;
    crate::config::definir_personnages(&voulus)?;

    // Le `State` ne traverse pas un `await` : on en sort ce dont la tâche a
    // besoin. C'est aussi ce qui rend la commande `Send`, exigence de
    // `spawn_blocking`.
    let actions_tache = std::sync::Arc::clone(&actions);
    let nom_tache = nom.clone();

    // 2 et 3. `spawn_blocking` : cette attente est du travail BLOQUANT. La
    // laisser sur l'exécuteur async figerait les autres commandes — la même
    // raison qui a fait écrire `installer` ainsi.
    tauri::async_runtime::spawn_blocking(move || -> Result<(), String> {
        // La boucle promet de retirer un acteur sans animation dès l'image
        // suivante ; on laisse largement de quoi, puis on renonce plutôt que
        // d'attendre indéfiniment.
        let limite = std::time::Instant::now() + std::time::Duration::from_secs(6);
        while actions_tache.acteurs_nommes(&nom_tache) > 0 {
            if std::time::Instant::now() > limite {
                return Err(format!(
                    "« {nom_tache} » est encore à l'écran : rien n'a été supprimé"
                ));
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }

        let Some(dossier) = crate::config::dossier_bibliotheque().map(|b| b.join(&nom_tache))
        else {
            return Err("%APPDATA% introuvable".to_string());
        };
        std::fs::remove_dir_all(&dossier)
            .map_err(|e| format!("suppression de {} : {e}", dossier.display()))
    })
    .await
    // Deux erreurs différentes se succèdent : la tâche a pu être annulée (le
    // premier `?`), puis la suppression a pu échouer (le second).
    .map_err(|e| format!("tâche de suppression interrompue : {e}"))??;

    println!("« {nom} » supprimé du disque");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noms(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn ajouter_un_exemplaire_en_ajoute_un_seul() {
        let r = roster_avec(&noms(&["blob"]), "blob", 2);
        assert_eq!(r, noms(&["blob", "blob"]));
    }

    #[test]
    fn mettre_a_zero_les_retire_tous() {
        // L'interrupteur éteint : il n'y a pas de compte « en sommeil » à
        // garder quelque part.
        let r = roster_avec(&noms(&["blob", "blob", "luffy"]), "blob", 0);
        assert_eq!(r, noms(&["luffy"]));
    }

    #[test]
    fn rallumer_ne_ramene_qu_un_exemplaire() {
        // Éteindre trois blob puis rallumer en ramène UN. C'est la décision
        // de l'auteur, et elle supprime une classe de bugs : il n'y a pas
        // deux vérités à tenir d'accord.
        let eteint = roster_avec(&noms(&["blob", "blob", "blob"]), "blob", 0);
        let rallume = roster_avec(&eteint, "blob", 1);
        assert_eq!(rallume, noms(&["blob"]));
    }

    #[test]
    fn l_ordre_des_autres_est_preserve() {
        // Un `config.json` relu par un humain ne doit pas voir ses lignes
        // danser à chaque clic.
        let r = roster_avec(&noms(&["zoro", "blob", "luffy"]), "blob", 1);
        assert_eq!(r, noms(&["zoro", "luffy", "blob"]));
    }

    #[test]
    fn un_nom_absent_s_ajoute() {
        let r = roster_avec(&noms(&["blob"]), "luffy", 1);
        assert_eq!(r, noms(&["blob", "luffy"]));
    }

    #[test]
    fn tout_eteindre_donne_une_liste_vide() {
        // Zéro personnage est un état NORMAL (design §4).
        let r = roster_avec(&noms(&["blob"]), "blob", 0);
        assert!(r.is_empty());
    }
}

// ── L'assistant de première configuration (spec §3) ─────────────────────

/// Ce que l'assistant affiche à son ouverture.
///
/// `Serialize` seulement : la donnée ne circule que de Rust vers le webview.
/// `rename_all = "camelCase"` parce que c'est la convention de l'autre côté —
/// `demarrage_auto` se lit donc `etat.demarrageAuto` dans `onboarding.js`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EtatOnboarding {
    pub demarrage_auto: bool,
}

/// L'état à afficher à l'ouverture de l'assistant.
///
/// ⚠️ Le REGISTRE et non la configuration. Le cas existe vraiment : quelqu'un
/// qui réinstalle par-dessus une version où il avait activé le démarrage
/// automatique doit retrouver la case cochée. C'est aussi pourquoi
/// `Config::demarrage_automatique` a été supprimé — il aurait été une
/// seconde vérité à tenir d'accord avec celle-ci.
#[tauri::command]
pub fn onboarding_etat() -> EtatOnboarding {
    EtatOnboarding {
        demarrage_auto: crate::autostart::est_actif(),
    }
}

/// Applique les choix de l'assistant et le marque comme fait.
///
/// L'ordre des cinq gestes compte, et il est décrit dans la spec §3 :
/// registre, configuration, écran, toast, fermeture.
///
/// `ecran` arrive en `String` depuis le webview plutôt qu'en énumération :
/// `EcranDemarrage::depuis_json` est **totale** (tout inconnu vaut
/// `Personnages`), donc il n'y a rien à valider deux fois, et une
/// désérialisation qui échouerait côté Tauri donnerait un message illisible.
#[tauri::command]
pub fn onboarding_terminer(
    app: AppHandle,
    actions: tauri::State<'_, std::sync::Arc<crate::actions::Actions>>,
    demarrage_auto: bool,
    ecran: String,
) -> Result<(), String> {
    // ── 1. Le registre ──────────────────────────────────────────────────
    // `appliquer_demarrage` rend l'état RÉELLEMENT obtenu, qui diffère du
    // voulu si l'écriture a échoué.
    let reel = crate::actions::appliquer_demarrage(demarrage_auto);
    actions.resynchroniser_demarrage(reel);

    // ── 2. La configuration ─────────────────────────────────────────────
    // Écrite AVANT d'appliquer l'écran : si elle échoue, on veut le dire
    // sans avoir déjà changé l'affichage.
    //
    // C'est le seul échec FATAL au sens de l'assistant : ne pas pouvoir
    // écrire la clé, c'est un assistant qui revient au prochain lancement,
    // et mieux vaut le dire que le laisser boucler en silence.
    let choisi = crate::config::EcranDemarrage::depuis_json(&ecran);
    crate::config::definir_onboarding(true, choisi)?;

    // ── 3. L'écran choisi, tout de suite ────────────────────────────────
    // `&**actions` : `actions` est une `State<Arc<Actions>>`. Un premier
    // déréférencement donne l'`Arc<Actions>`, un second l'`Actions` — et on
    // en reprend une référence. Écrit explicitement plutôt que de compter
    // sur deux coercitions enchaînées, qui compilent mal selon le contexte.
    crate::actions::appliquer_ecran(&**actions, &app, choisi);

    // ── 4. Le toast (spec §4) ───────────────────────────────────────────
    // Best-effort : il ne peut pas s'afficher hors d'une installation NSIS,
    // faute d'AppUserModelID, et ce n'est pas une raison de faire échouer
    // l'assistant.
    //
    // Il ne se joue qu'une fois parce qu'il est ICI : `onboarding_terminer`
    // n'est appelée qu'une fois. Aucune clé de configuration dédiée —
    // remettre `premiereConfigurationFaite` à `false` rejoue les deux.
    crate::toast::arriere_plan(&app);

    // ── 5. La fenêtre se ferme ──────────────────────────────────────────
    // `if let Some` : si elle a déjà été fermée à la croix pendant l'appel,
    // il n'y a rien à fermer et ce n'est pas une erreur.
    if let Some(fenetre) = tauri::Manager::get_webview_window(&app, "onboarding") {
        let _ = fenetre.close();
    }

    // ⚠️ Si le REGISTRE a refusé, on le signale MAINTENANT — après avoir
    // tout le reste enregistré. Le geste principal (écrire la
    // configuration) a réussi ; rendre l'erreur plus tôt laisserait
    // l'assistant ouvert avec une configuration déjà écrite, donc un
    // assistant qui ne se rouvrirait plus jamais s'il était fermé à la
    // croix. Le webview décoche la case plutôt que d'afficher un état qui
    // n'est pas celui du registre — le précédent d'`ID_DEMARRAGE`.
    if reel != demarrage_auto {
        return Err("le démarrage avec Windows n'a pas pu être enregistré".to_string());
    }
    Ok(())
}

// ── Le menu du clic droit (spec « menu sur mesure » §4) ─────────────────

/// `menu.js` a dessiné et mesuré le menu : le placer et le montrer.
#[tauri::command]
pub fn placer_menu(app: tauri::AppHandle, largeur: f32, hauteur: f32) {
    crate::menu_fenetre::placer(&app, largeur, hauteur);
}

/// Une entrée a été choisie. Le menu se ferme D'ABORD — son personnage
/// repart, et le focus revient à l'application d'avant — puis le choix
/// passe par le même `executer` que le tray.
///
/// `id_connu` : on n'exécute que nos identifiants, jamais une chaîne
/// arbitraire venue d'un webview.
#[tauri::command]
pub fn choisir_entree_menu(
    app: tauri::AppHandle,
    actions: tauri::State<'_, std::sync::Arc<crate::actions::Actions>>,
    id: String,
) {
    crate::menu_fenetre::fermer(&app);
    match crate::menu_perso::id_connu(&id) {
        // `inner().clone()` : un `Arc` de plus sur les mêmes actions — la
        // méthode le consomme pour l'emporter sur le thread principal.
        Some(id) => actions.inner().clone().executer_choix_du_menu(app, id),
        None => eprintln!("menu : identifiant inconnu « {id} », ignoré"),
    }
}

/// Clic ailleurs ou Échap.
#[tauri::command]
pub fn fermer_menu(app: tauri::AppHandle) {
    crate::menu_fenetre::fermer(&app);
}
