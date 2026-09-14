//! L'index du catalogue : la liste des packs disponibles (spec §7).
//!
//! Responsabilité unique : lire `catalogue.json`. Ne télécharge rien et ne
//! sait rien des URL — le slug suffit, l'URL s'en déduit dans `mod.rs`.

use serde::Deserialize;

/// Un pack du catalogue : trois champs, et pas un de plus.
///
/// Ni nombre de frames ni URL (spec §7) : le premier demanderait de sonder
/// le CDN pack par pack pour une information que l'installation découvre de
/// toute façon, la seconde se déduit du slug par une règle écrite une seule
/// fois. Un champ `url` par pack serait 2063 occasions de diverger.
#[derive(Debug, Clone, Deserialize)]
pub struct Pack {
    pub slug: String,
    pub nom: String,
    pub franchise: String,
}

/// L'index entier. Les champs `genere_le` et `source` du fichier ne sont pas
/// déclarés ici : `serde` ignore par défaut ce qu'il ne connaît pas, et ils
/// sont de la documentation pour l'humain, pas de la donnée pour le code.
#[derive(Debug, Clone, Deserialize)]
pub struct Index {
    pub packs: Vec<Pack>,
}

/// Lit l'index, BOM compris.
///
/// Le BOM a coûté un diagnostic sur ce projet : `serde_json` le refuse avec
/// le message trompeur « expected value at line 1 column 1 ». On le retire
/// ici comme `config::lire_json` le fait déjà pour les fichiers.
pub fn lire(texte: &str) -> Result<Index, String> {
    const BOM: char = '\u{feff}';
    // `strip_prefix` rend une `Option` : `unwrap_or(texte)` veut dire « s'il
    // n'y avait pas de BOM, garde le texte tel quel ».
    let propre = texte.strip_prefix(BOM).unwrap_or(texte);
    serde_json::from_str(propre).map_err(|e| format!("catalogue.json illisible : {e}"))
}

/// Un nom présentable, déduit du slug.
///
/// 1636 des 2063 slugs du catalogue sont des dépôts communautaires portant un
/// suffixe hexadécimal de 6 caractères (`zoro-8bd774`) : on le retire. Les
/// autres gardent leur slug, simplement mis en forme.
pub fn nom_lisible(slug: &str) -> String {
    let sans_suffixe = match slug.rsplit_once('-') {
        // `len() == 6` ET tout hexadécimal : les deux conditions, sinon on
        // amputerait « luffy-01 » de son numéro.
        Some((debut, fin)) if fin.len() == 6 && fin.chars().all(|c| c.is_ascii_hexdigit()) => debut,
        _ => slug,
    };

    let avec_espaces = sans_suffixe.replace('-', " ");

    // Première lettre en capitale. `chars().next()` plutôt qu'un index : un
    // caractère UTF-8 peut faire plusieurs octets, et `&s[0..1]` paniquerait
    // au milieu de l'un d'eux.
    let mut c = avec_espaces.chars();
    match c.next() {
        Some(premiere) => premiere.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// L'index se lit, BOM compris.
    ///
    /// Le BOM a déjà coûté un diagnostic sur ce projet : `serde_json` le
    /// refuse avec le message trompeur « expected value at line 1 column 1 ».
    #[test]
    fn l_index_se_lit_avec_ou_sans_bom() {
        let json = r#"{
            "genere_le": "2026-09-11",
            "packs": [
                { "slug": "one-piece-luffy-01", "nom": "Luffy", "franchise": "One Piece" }
            ]
        }"#;

        let i = lire(json).expect("index valide");
        assert_eq!(i.packs.len(), 1);
        assert_eq!(i.packs[0].slug, "one-piece-luffy-01");

        let avec_bom = format!("\u{feff}{json}");
        assert!(lire(&avec_bom).is_ok(), "le BOM ne doit pas faire échouer");

        assert!(lire("pas du json").is_err());
    }

    /// Le VRAI `ui/catalogue.json` du dépôt se lit.
    ///
    /// Le test décisif de la tâche : les deux autres jouent sur du JSON
    /// écrit à la main, donc sur ce que nous croyons que le script produit.
    /// Celui-ci lit ce qu'il produit **réellement** — c'est le seul qui
    /// attraperait un BOM, une profondeur de sérialisation trop courte
    /// (`ConvertTo-Json` sans `-Depth` écrit « System.Object[] »), ou un
    /// champ renommé.
    ///
    /// Le chemin est relatif à `src-tauri/`, d'où `cargo test` s'exécute —
    /// même convention que le test qui lit un vrai PNG de `blob`.
    #[test]
    fn le_vrai_catalogue_du_depot_se_lit() {
        let chemin = std::path::Path::new("../ui/catalogue.json");
        let texte = std::fs::read_to_string(chemin).expect("ui/catalogue.json doit exister");

        let i = lire(&texte).expect("le catalogue engendré doit être lisible");

        assert!(
            i.packs.len() > 1000,
            "le catalogue en compte plus de 2000 ; {} sent l'extraction cassée",
            i.packs.len()
        );

        // Aucun champ vide : c'est ce qui détecterait une profondeur de
        // sérialisation trop courte, qui rendrait des chaînes littérales
        // « System.Object[] » plutôt que des valeurs.
        for p in &i.packs {
            assert!(!p.slug.is_empty(), "slug vide");
            assert!(!p.nom.is_empty(), "nom vide pour {}", p.slug);
            assert!(!p.franchise.is_empty(), "franchise vide pour {}", p.slug);
            assert!(
                !p.franchise.contains("System.Object"),
                "franchise non sérialisée pour {} : {}",
                p.slug,
                p.franchise
            );
        }

        // Le slug construit une URL et un chemin : il est validé à
        // l'installation, mais un catalogue qui en contient un refusé
        // offrirait une carte impossible à installer.
        for p in &i.packs {
            assert!(
                crate::catalogue::slug_valide(&p.slug),
                "slug refusé par la validation : {}",
                p.slug
            );
        }
    }

    /// Le nom lisible se déduit du slug pour les dépôts communautaires.
    #[test]
    fn le_nom_lisible_retire_le_suffixe_hexadecimal() {
        // 1636 des 2063 slugs sont de cette forme.
        assert_eq!(nom_lisible("zoro-8bd774"), "Zoro");
        assert_eq!(nom_lisible("pierrot-54acb5"), "Pierrot");
        // Un suffixe qui n'est pas hexadécimal de 6 caractères reste.
        assert_eq!(nom_lisible("one-piece-luffy-01"), "One piece luffy 01");
        assert_eq!(nom_lisible(""), "");
    }
}
