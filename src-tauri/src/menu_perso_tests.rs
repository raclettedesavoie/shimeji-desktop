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
    assert_eq!(commande_de("recharger"), None);
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
    let table = TableEnvies::defaut();
    for entree in &table.entrees {
        assert!(
            ENVIES
                .iter()
                .any(|(_, _, _, c)| *c == Commande::Intention(entree.intention)),
            "{:?} est tirable mais absente du menu contextuel",
            entree.intention
        );
    }
}

/// Un pack complet les propose toutes ; un pack qui ne sait que marcher
/// n'en propose qu'une.
///
/// C'est la couverture partielle (spec §8.6) vue depuis le menu, et la
/// raison pour laquelle `ouvrir` filtre au lieu de tout afficher grisé.
///
/// Ne teste que les `Commande::Intention` : les trois autres n'ont pas de
/// pose requise propre (voir le commentaire de `ouvrir` sur ce point),
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

/// Les identifiants qu'`ouvrir` proposerait pour cet endroit, en ne
/// rejouant que la logique de filtrage (pas la construction réelle des
/// `MenuItem`, qui demande un `AppHandle` Tauri hors de portée des
/// tests unitaires).
fn ids_proposes(table: &TableEnvies, manifeste: &Manifest, ou: Ou) -> Vec<&'static str> {
    ENVIES
        .iter()
        .filter(|(_, _, contextes, _)| contextes.contains(&ou))
        .filter(|(_, _, _, commande)| match commande {
            Commande::Intention(i) => table.jouable(manifeste, *i),
            _ => true,
        })
        .map(|(id, _, _, _)| *id)
        .collect()
}

#[test]
fn le_menu_au_sol_ne_propose_aucune_action_d_accroche() {
    let (table, blob) = table_et_blob();
    let ids = ids_proposes(&table, &blob, Ou::Sol);

    for interdit in ["perso.monter", "perso.rester", "perso.redescendre", "perso.lacher"] {
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

    for interdit in ["perso.flaner", "perso.asseoir", "perso.tete", "perso.jambes", "perso.grimper"] {
        assert!(
            !ids.contains(&interdit),
            "« {interdit} » ne devrait pas apparaître sur un mur : {ids:?}"
        );
    }
    // « Monter plus haut », « Rester accroché », « Redescendre », « Se
    // lâcher ».
    assert_eq!(ids.len(), 4, "{ids:?}");
    assert!(ids.contains(&"perso.monter"));
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
    for interdit in ["perso.flaner", "perso.asseoir", "perso.tete", "perso.jambes", "perso.grimper", "perso.monter"] {
        assert!(!ids.contains(&interdit), "« {interdit} » : {ids:?}");
    }
    assert_eq!(ids.len(), 2, "{ids:?}");
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
