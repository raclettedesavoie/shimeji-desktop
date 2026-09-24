//! Les tests de `menu_perso` — sortis du fichier source le 2026-09-14.
//!
//! **Pourquoi ce fichier existe** : `menu_perso.rs` faisait 623 lignes dont 229 de tests,
//! soit 37 %. Toute lecture du module en payait le double, pour rien la plupart
//! du temps. Les tests n'ont pas changé d'une ligne : ils ont seulement déménagé, et
//! se désindentent d'un cran puisqu'ils ne sont plus enfermés dans un `mod tests { }`.

use super::*;

/// Le décodage doit être l'exact inverse de la construction.
///
/// Le test qui compte vraiment de ce fichier : si une ligne d'`ENVIES`
/// était ajoutée avec un identifiant en double, `commande_de` rendrait
/// systématiquement la première — l'entrée du menu serait présente et
/// déclencherait une **autre** action. Silencieux, et très pénible à
/// diagnostiquer à l'œil.
#[test]
fn chaque_envie_se_decode_en_elle_meme() {
    for (id, _, _, commande) in ENVIES {
        assert_eq!(
            commande_de(id),
            Some(*commande),
            "l'identifiant « {id} » ne rend pas sa commande"
        );
    }
}

#[test]
fn un_identifiant_inconnu_ne_decode_rien() {
    // C'est ce qui permet à `actions::executer` d'utiliser `commande_de`
    // comme cas par défaut sans avaler les entrées du tray.
    assert_eq!(commande_de("afficher"), None);
    assert_eq!(commande_de(""), None);
}

/// Toutes les envies du menu doivent exister dans la table d'envies.
///
/// Sinon `TableEnvies::jouable` rendrait `false` et l'entrée
/// n'apparaîtrait **jamais**, sur aucun pack — un menu amputé sans le
/// moindre message. C'est le mode d'échec exact d'un oubli dans la liste
/// que le commentaire d'`ENVIES` demande de tenir à jour.
///
/// Ne concerne que les lignes `Commande::Intention` : les trois autres
/// commandes n'ont pas de ligne dans `TableEnvies` — ce ne sont pas des
/// intentions tirables, voir le commentaire de `Commande`.
#[test]
fn toutes_les_envies_du_menu_sont_dans_la_table() {
    let table = TableEnvies::defaut();
    for (id, _, _, commande) in ENVIES {
        if let Commande::Intention(i) = commande {
            assert!(
                table.entrees.iter().any(|e| e.intention == *i),
                "« {id} » n'est pas dans la table d'envies : l'entrée serait toujours cachée"
            );
        }
    }
}

#[test]
fn toute_intention_de_la_table_d_envies_est_proposee_par_le_menu() {
    // ⚠️ Ce test est le rattrapage de l'oubli que `CLAUDE.md` décrit :
    // une intention qui existe pour le tirage mais qu'aucune entrée de
    // menu ne propose est un manque SILENCIEUX. Il ne l'est plus.
    //
    // C'est la RÉCIPROQUE de `toutes_les_envies_du_menu_sont_dans_la_table`
    // ci-dessus : celui-là interdit une entrée de menu sans ligne de
    // table, celui-ci interdit une ligne de table sans entrée de menu.
    // Il faut les deux pour que la correspondance soit exacte.
    //
    // Depuis les actions tenues (spec « menu sur mesure » §2), une intention
    // est proposée soit telle quelle (ponctuelle), soit par la tenue qui la
    // sert (`Basculer`) : les deux comptent.
    let table = TableEnvies::defaut();
    for entree in &table.entrees {
        assert!(
            ENVIES.iter().any(|(_, _, _, c)| match c {
                Commande::Intention(i) => *i == entree.intention,
                Commande::Basculer(t) => t.intention() == entree.intention,
                _ => false,
            }),
            "{:?} est tirable mais absente du menu contextuel",
            entree.intention
        );
    }
}

/// Un pack complet les propose toutes ; un pack qui ne sait que marcher
/// n'en propose qu'une.
///
/// C'est la couverture partielle (spec §8.6) vue depuis le menu, et la
/// raison pour laquelle `lignes` filtre au lieu de tout afficher grisé.
///
/// Ne teste que les `Commande::Intention` : les trois autres n'ont pas de
/// pose requise propre (voir le commentaire de `lignes` sur ce point),
/// donc rien à vérifier de leur côté de la couverture partielle.
#[test]
fn la_couverture_partielle_retire_les_envies_injouables() {
    let table = TableEnvies::defaut();

    let blob = Manifest::load(std::path::Path::new("../characters/blob"))
        .expect("le personnage de test doit être lisible");
    let intentions: Vec<_> = ENVIES
        .iter()
        .filter_map(|(_, _, _, c)| match c {
            Commande::Intention(i) => Some(*i),
            _ => None,
        })
        .collect();
    let proposees = intentions.iter().filter(|i| table.jouable(&blob, **i)).count();
    assert_eq!(proposees, intentions.len(), "blob a toutes les poses");
}

// ── Le filtrage par endroit (le bug rapporté à l'écran) ─────────────

/// Construit un manifeste et une table complets, pour ne tester ici que
/// le filtrage par `Ou` — pas la couverture partielle, déjà couverte
/// ci-dessus.
fn table_et_blob() -> (TableEnvies, Manifest) {
    (
        TableEnvies::defaut(),
        Manifest::load(std::path::Path::new("../characters/blob"))
            .expect("le personnage de test doit être lisible"),
    )
}

/// Les identifiants d'ENVIES que `lignes` propose pour cet endroit — les
/// entrées communes (cacher, catalogue, quitter) écartées, puisque ces
/// tests-ci ne portent que sur le filtrage des envies.
fn ids_proposes(table: &TableEnvies, manifeste: &Manifest, ou: Ou) -> Vec<&'static str> {
    lignes(manifeste, table, ou, None, &[])
        .into_iter()
        .filter_map(|l| match l {
            Ligne::Entree { id, .. } => Some(id),
            Ligne::Titre { .. } | Ligne::Separateur => None,
        })
        .filter(|id| ENVIES.iter().any(|(i, _, _, _)| i == id))
        .collect()
}

#[test]
fn le_menu_au_sol_ne_propose_aucune_action_d_accroche() {
    let (table, blob) = table_et_blob();
    let ids = ids_proposes(&table, &blob, Ou::Sol);

    for interdit in ["perso.rester", "perso.redescendre", "perso.lacher"] {
        assert!(
            !ids.contains(&interdit),
            "« {interdit} » ne devrait pas apparaître au sol : {ids:?}"
        );
    }
    // Et les cinq envies habituelles restent là — inchangé.
    assert_eq!(ids.len(), 5, "{ids:?}");
}

#[test]
fn le_menu_sur_un_mur_ne_propose_aucune_envie_de_sol() {
    let (table, blob) = table_et_blob();
    let ids = ids_proposes(&table, &blob, Ou::Mur);

    for interdit in ["perso.flaner", "perso.asseoir", "perso.tete", "perso.jambes"] {
        assert!(
            !ids.contains(&interdit),
            "« {interdit} » ne devrait pas apparaître sur un mur : {ids:?}"
        );
    }
    // Grimper au mur, Rester accroché, Redescendre, Se lâcher.
    assert_eq!(ids.len(), 4, "{ids:?}");
    assert!(ids.contains(&"perso.grimper"));
    assert!(ids.contains(&"perso.rester"));
    assert!(ids.contains(&"perso.redescendre"));
    assert!(ids.contains(&"perso.lacher"));
}

#[test]
fn le_menu_au_plafond_ne_propose_ni_envie_de_sol_ni_redescendre() {
    let (table, blob) = table_et_blob();
    let ids = ids_proposes(&table, &blob, Ou::Plafond);

    // Pas de « Redescendre » au plafond : ça demanderait de traverser
    // jusqu'au bout, basculer sur un mur, puis descendre — de la
    // navigation calculée, que la décision n° 4 exclut (YAGNI). Shimeji
    // ne le propose pas non plus.
    assert!(!ids.contains(&"perso.redescendre"), "{ids:?}");
    for interdit in ["perso.flaner", "perso.asseoir", "perso.tete", "perso.jambes"] {
        assert!(!ids.contains(&interdit), "« {interdit} » : {ids:?}");
    }
    // Grimper au mur (tenu, il traverse le plafond), Rester accroché, Se
    // lâcher.
    assert_eq!(ids.len(), 3, "{ids:?}");
    assert!(ids.contains(&"perso.grimper"));
    assert!(ids.contains(&"perso.rester"));
    assert!(ids.contains(&"perso.lacher"));
}

#[test]
fn ou_de_lit_correctement_les_quatre_etats() {
    let plateforme = crate::world::PlatformId(0);

    assert_eq!(
        ou_de(&Attachment::On {
            platform: plateforme,
            face: Face::Top,
            offset: 0.0
        }),
        Ou::Sol
    );
    assert_eq!(
        ou_de(&Attachment::On {
            platform: plateforme,
            face: Face::Left,
            offset: 0.0
        }),
        Ou::Mur
    );
    assert_eq!(
        ou_de(&Attachment::On {
            platform: plateforme,
            face: Face::Right,
            offset: 0.0
        }),
        Ou::Mur
    );
    assert_eq!(
        ou_de(&Attachment::On {
            platform: plateforme,
            face: Face::Bottom,
            offset: 0.0
        }),
        Ou::Plafond
    );
    assert_eq!(
        ou_de(&Attachment::Falling {
            pos: crate::geom::Point::new(0.0, 0.0),
            vel: crate::geom::Vec2::zero(),
        }),
        Ou::Sol
    );
    assert_eq!(ou_de(&Attachment::Dragged), Ou::Sol);
}


// ── Le destinataire de la commande (correction du 2026-09-16) ───────────
//
// Ces trois tests décrivent le défaut rapporté à l'écran : « quand je fais un
// clic droit sur un personnage, il ne fait plus ses actions quand je les
// lance ». Voir l'en-tête de `commande_pour`.

/// Le cas nominal, et le seul qui comptait vraiment : la commande revient à
/// celui qui a ouvert le menu, **même si le curseur est ailleurs** — et il
/// l'est toujours, puisqu'il est sur l'entrée de menu qu'on vient de cliquer.
#[test]
fn la_commande_revient_a_celui_qui_a_ouvert_le_menu() {
    let mut demandeur = Some("pet-3".to_string());
    let mut boite = Some(Commande::Basculer(crate::behavior::tenue::Tenue::ResterAccroche));

    assert_eq!(
        commande_pour(&mut demandeur, &mut boite, "pet-3"),
        Some(Commande::Basculer(crate::behavior::tenue::Tenue::ResterAccroche))
    );
    // Servie une fois et une seule : ni la boîte ni le demandeur ne
    // resserviraient l'image suivante.
    assert_eq!(boite, None);
    assert_eq!(demandeur, None);
}

/// Et pas à un autre : sans quoi N personnages joueraient l'entrée choisie
/// pour un seul.
#[test]
fn un_autre_acteur_ne_recoit_rien() {
    let mut demandeur = Some("pet-3".to_string());
    let mut boite = Some(Commande::SeLacher);

    assert_eq!(commande_pour(&mut demandeur, &mut boite, "pet-7"), None);
    // ⚠️ La boîte n'est PAS vidée : la commande attend son destinataire, qui
    // vient peut-être plus loin dans le même parcours du `Vec`.
    assert_eq!(boite, Some(Commande::SeLacher));
    assert_eq!(demandeur, Some("pet-3".to_string()));
}

/// Le menu est ouvert, rien n'a encore été choisi : on garde le demandeur
/// pour les images suivantes, parce que le clic met quelques images à
/// revenir par la boucle d'événements de Tauri.
#[test]
fn sans_commande_le_demandeur_est_garde() {
    let mut demandeur = Some("pet-3".to_string());
    let mut boite: Option<Commande> = None;

    assert_eq!(commande_pour(&mut demandeur, &mut boite, "pet-3"), None);
    assert_eq!(demandeur, Some("pet-3".to_string()));
}

// ── « Tout le monde grimpe au mur » (2026-09-23) ───────────────────────────────

#[test]
fn l_ordre_a_tous_est_servi_a_qui_n_a_rien_demande() {
    let grimper = Commande::Intention(Intention::Grimper);
    assert_eq!(commande_de_l_acteur(None, Some(grimper)), Some(grimper));
    assert_eq!(commande_de_l_acteur(None, None), None);
}

#[test]
fn la_commande_personnelle_l_emporte_sur_l_ordre_a_tous() {
    // Choisir « S'asseoir » pour CE personnage ne doit pas l'envoyer au mur
    // parce qu'un ordre collectif est tombé dans la même image.
    let asseoir = Commande::Intention(Intention::SeReposer);
    let grimper = Commande::Intention(Intention::Grimper);
    assert_eq!(commande_de_l_acteur(Some(asseoir), Some(grimper)), Some(asseoir));
}

// ── Les deux « Cacher » (2026-09-23) ─────────────────────────────────────

/// Le libellé d'une entrée, ou `None` si elle n'est pas dans le menu.
fn libelle_de(lignes: &[Ligne], cherche: &str) -> Option<&'static str> {
    lignes.iter().find_map(|l| match l {
        Ligne::Entree { id, libelle, .. } if *id == cherche => Some(*libelle),
        _ => None,
    })
}

/// « Cacher ce personnage » est proposé partout — au sol, au mur, au
/// plafond — et « Cacher tous les personnages » dit enfin qu'il cache tout
/// le monde : les deux côte à côte, un libellé ambigu ferait hésiter.
#[test]
fn les_deux_cacher_sont_proposes_partout_et_se_distinguent() {
    let (table, blob) = table_et_blob();
    for ou in [Ou::Sol, Ou::Mur, Ou::Plafond] {
        let l = lignes(&blob, &table, ou, None, &[]);
        assert_eq!(
            libelle_de(&l, crate::actions::ID_P_CACHER_CE),
            Some("Cacher ce personnage"),
            "{ou:?}"
        );
        assert_eq!(
            libelle_de(&l, crate::actions::ID_P_CACHER),
            Some("Cacher tous les personnages"),
            "{ou:?}"
        );
    }
}

/// « Cacher ce personnage » n'est PAS une envie : il n'a pas à passer par
/// `commande_de`, qui le rendrait à `behavior::pas` — lequel n'en ferait
/// rien. C'est la boucle qui le traite, par `Actions::cacher_le_demandeur`.
#[test]
fn cacher_ce_personnage_n_est_pas_une_envie() {
    assert_eq!(commande_de(crate::actions::ID_P_CACHER_CE), None);
}

/// Un menu ne commence ni ne finit par un séparateur, et n'en aligne jamais
/// deux : dans les trois cas il aurait l'air cassé.
#[test]
fn le_menu_n_a_aucun_separateur_mal_place() {
    let (table, blob) = table_et_blob();
    for ou in [Ou::Sol, Ou::Mur, Ou::Plafond] {
        let l = lignes(&blob, &table, ou, None, &[]);
        assert_ne!(l.first(), Some(&Ligne::Separateur), "{ou:?}");
        assert_ne!(l.last(), Some(&Ligne::Separateur), "{ou:?}");
        assert!(
            !l.windows(2).any(|p| p[0] == Ligne::Separateur && p[1] == Ligne::Separateur),
            "{ou:?}"
        );
    }
}

// ── Les coches et la section « Tout le monde » (spec §3) ────────────────

use crate::behavior::tenue::Tenue;

fn coche_de(lignes: &[Ligne], cherche: &str) -> Option<bool> {
    lignes.iter().find_map(|l| match l {
        Ligne::Entree { id, coche, .. } if *id == cherche => Some(*coche),
        _ => None,
    })
}

#[test]
fn la_coche_du_personnage_suit_sa_tenue() {
    let (table, blob) = table_et_blob();
    let l = lignes(&blob, &table, Ou::Sol, Some(Tenue::Asseoir), &[Some(Tenue::Asseoir)]);
    assert_eq!(coche_de(&l, "perso.asseoir"), Some(true));
    assert_eq!(coche_de(&l, "perso.flaner"), Some(false));
    // Une action ponctuelle n'a jamais de coche.
    assert_eq!(coche_de(&l, "perso.tete"), Some(false));
}

#[test]
fn tout_le_monde_n_est_coche_que_si_tous_la_tiennent() {
    let (table, blob) = table_et_blob();
    let un_seul = lignes(&blob, &table, Ou::Sol, None, &[Some(Tenue::Asseoir), None]);
    assert_eq!(coche_de(&un_seul, "tous.asseoir"), Some(false));

    let tous = lignes(&blob, &table, Ou::Sol, None, &[Some(Tenue::Asseoir), Some(Tenue::Asseoir)]);
    assert_eq!(coche_de(&tous, "tous.asseoir"), Some(true));
}

#[test]
fn la_section_tout_le_monde_suit_son_titre_avec_les_actions_du_sol() {
    let (table, blob) = table_et_blob();
    // Même au mur : la section « Tout le monde » ne propose que le sol.
    let l = lignes(&blob, &table, Ou::Mur, None, &[None]);
    let titre = l
        .iter()
        .position(|x| *x == Ligne::Titre { texte: "Tout le monde" })
        .expect("le titre de section");
    let apres: Vec<&str> = l[titre + 1..]
        .iter()
        .map_while(|x| match x {
            Ligne::Entree { id, .. } if id.starts_with("tous.") => Some(*id),
            _ => None,
        })
        .collect();
    assert_eq!(apres, ["tous.flaner", "tous.asseoir", "tous.tete", "tous.jambes", "tous.grimper"]);
}

#[test]
fn monter_plus_haut_a_disparu() {
    let (table, blob) = table_et_blob();
    for ou in [Ou::Sol, Ou::Mur, Ou::Plafond] {
        assert_eq!(coche_de(&lignes(&blob, &table, ou, None, &[]), "perso.monter"), None);
    }
}

#[test]
fn les_identifiants_tous_ne_vont_pas_au_demandeur() {
    assert_eq!(commande_de("tous.grimper"), None);
    assert_eq!(
        commande_de_tous("tous.grimper"),
        Some(Commande::Basculer(Tenue::Grimper))
    );
    assert_eq!(commande_de_tous("perso.grimper"), None);
}

#[test]
fn resoudre_pour_tous_tient_si_un_seul_ne_la_tient_pas() {
    let c = resoudre_pour_tous(
        Commande::Basculer(Tenue::Asseoir),
        &[Some(Tenue::Asseoir), None],
    );
    assert_eq!(c, Commande::Tenir(Tenue::Asseoir));
}

#[test]
fn resoudre_pour_tous_relache_si_tous_la_tiennent() {
    let c = resoudre_pour_tous(
        Commande::Basculer(Tenue::Asseoir),
        &[Some(Tenue::Asseoir), Some(Tenue::Asseoir)],
    );
    assert_eq!(c, Commande::Relacher(Tenue::Asseoir));
}

#[test]
fn resoudre_pour_tous_laisse_passer_le_reste() {
    let c = Commande::Intention(Intention::Jouer(Jeu::TeteQuiTourne));
    assert_eq!(resoudre_pour_tous(c, &[None]), c);
}

#[test]
fn chaque_entree_affichee_est_un_identifiant_connu() {
    // `choisir_entree_menu` n'accepte que ce que `id_connu` reconnaît : une
    // entrée affichée mais inconnue serait un clic sans effet.
    let (table, blob) = table_et_blob();
    for ou in [Ou::Sol, Ou::Mur, Ou::Plafond] {
        for l in lignes(&blob, &table, ou, None, &[None]) {
            if let Ligne::Entree { id, .. } = l {
                assert_eq!(id_connu(id), Some(id), "{id}");
            }
        }
    }
    assert_eq!(id_connu("n'importe.quoi"), None);
}
