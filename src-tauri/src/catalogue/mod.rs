//! Le catalogue de personnages : parcourir, installer (spec §4).
//!
//! Responsabilité unique : transformer un slug du catalogue shimejis.xyz en
//! un dossier de personnage utilisable par le moteur. Ne sait rien de
//! l'affichage — la fenêtre du catalogue, elle, est en HTML.

pub mod index;
pub mod installation;
pub mod reseau;

use crate::catalogue::installation::*;
use crate::catalogue::reseau::{Reseau, ReseauWinHttp};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Le CDN qui sert les frames, déjà en 128×128 et déjà numérotées.
///
/// L'extension Chrome « Shimeji Browser Extension » ne contient aucun
/// sprite : elle les tire d'ici. Installer un pack est donc un
/// **téléchargement**, pas une extraction.
const CDN: &str = "https://sprites.shimejis.xyz/directory";

/// Au-delà de ce numéro, on arrête de chercher.
///
/// Borne de sûreté et non limite du format : sans elle, un CDN qui
/// répondrait 200 à tout ferait boucler l'installation jusqu'à remplir le
/// disque. 46 est le vocabulaire standard, on laisse de la marge.
const FRAME_MAX: u32 = 120;

/// Le slug construit un **chemin** et une **URL** : il est validé avant les
/// deux.
///
/// Même modèle que la validation du nom dans le schéma `shime://` : des
/// caractères anodins seulement, ce qui suffit à interdire tout `..` ou
/// séparateur de chemin.
pub fn slug_valide(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= 100
        && slug
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Installe le pack `slug` sous `racine`, et rend son dossier.
///
/// `progres` est appelé après chaque frame écrite — c'est ce qui alimentera
/// la barre de progression de la fenêtre du catalogue.
///
/// `&mut dyn FnMut` plutôt qu'un second paramètre générique : la fonction
/// l'est déjà sur le réseau, et un `impl FnMut` de plus alourdirait chaque
/// appel sans rien apporter.
pub fn installer_avec<R: Reseau>(
    reseau: &R,
    racine: &Path,
    slug: &str,
    progres: &mut dyn FnMut(u32, u32),
) -> Result<PathBuf, String> {
    if !slug_valide(slug) {
        return Err(format!("slug refusé : « {slug} »"));
    }

    let final_ = racine.join(slug);

    // Le dossier de travail porte un nom **pointé** : s'il survit à une
    // coupure, il est ignoré au chargement (aucun personnage ne commence par
    // un point) et écrasé au prochain essai.
    let partiel = racine.join(format!(".{slug}.partiel"));

    let _ = std::fs::remove_dir_all(&partiel);
    std::fs::create_dir_all(partiel.join("img"))
        .map_err(|e| format!("création de {} : {e}", partiel.display()))?;

    // ── Les frames ──────────────────────────────────────────────────────
    let mut presentes: BTreeSet<u32> = BTreeSet::new();
    let mut tailles: BTreeMap<u32, [u32; 2]> = BTreeMap::new();
    let mut premiere_image: Option<Vec<u8>> = None;

    // On continue au-delà de 46 tant que les fichiers existent, et on
    // s'arrête après deux absences consécutives : certains packs ont des
    // trous, mais aucun n'a deux trous de suite suivis de contenu.
    let mut absences = 0;
    let mut n = 1u32;

    while n <= FRAME_MAX {
        let url = format!("{CDN}/{slug}/img/shime{n}.png");

        let octets = match reseau.get(&url) {
            Ok(Some(o)) => o,
            Ok(None) => {
                // 404 : absence NORMALE, on avance.
                absences += 1;
                if n > 46 && absences >= 2 {
                    break;
                }
                n += 1;
                continue;
            }
            Err(e) => {
                // Une PANNE, elle, interrompt. Ménage avant de rendre
                // l'erreur : on ne laisse pas le dossier partiel encombrer
                // la bibliothèque.
                let _ = std::fs::remove_dir_all(&partiel);
                return Err(format!("téléchargement interrompu : {e}"));
            }
        };
        absences = 0;

        let Some(taille) = taille_png(&octets) else {
            // Ce n'est pas un PNG : on l'ignore plutôt que d'échouer, le CDN
            // servant parfois une page d'erreur avec un statut 200.
            n += 1;
            continue;
        };

        std::fs::write(partiel.join("img").join(format!("shime{n}.png")), &octets)
            .map_err(|e| format!("écriture de shime{n}.png : {e}"))?;

        if n == 1 {
            premiere_image = Some(octets);
        }
        presentes.insert(n);
        tailles.insert(n, taille);
        progres(presentes.len() as u32, 46);
        n += 1;
    }

    // ── Les ancres ──────────────────────────────────────────────────────
    // Une panne ici n'est PAS fatale : l'absence d'`actions.xml` a un repli
    // documenté (la convention Shimeji-ee), contrairement à l'absence de
    // frames. D'où le `_` qui avale aussi bien le 404 que l'erreur.
    //
    // ⚠️ `<slug>/actions.xml`, et NON `<slug>/conf/actions.xml` comme
    // l'annonçaient la spec et le plan : le CDN rend 404 sur le second,
    // vérifié sur quatre slugs. Le repli étant silencieux, l'erreur ne se
    // voyait pas — tous les packs s'installaient avec l'ancre de convention.
    let ancres = match reseau.get(&format!("{CDN}/{slug}/actions.xml")) {
        Ok(Some(o)) => {
            let texte = String::from_utf8_lossy(&o);
            ancres_de_actions_xml(&texte)
        }
        _ => BTreeMap::new(),
    };

    // ── La hitbox : UNE image décodée ───────────────────────────────────
    let hitbox = match premiere_image.as_ref().and_then(|o| decoder_png(o)) {
        Some((rgba, l, h)) => hitbox_depuis(&rgba, l, h),
        // shime1 illisible : la toile entière. Mieux vaut une hitbox trop
        // large qu'un personnage impossible à attraper (spec §3.3).
        None => {
            let [l, h] = tailles.get(&1).copied().unwrap_or([128, 128]);
            [0, 0, l, h]
        }
    };

    // ── Le manifeste ────────────────────────────────────────────────────
    let json = match ecrire_mascot_json(slug, &presentes, &tailles, &ancres, hitbox) {
        Ok(j) => j,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&partiel);
            return Err(e);
        }
    };
    std::fs::write(partiel.join("mascot.json"), json)
        .map_err(|e| format!("écriture du manifeste : {e}"))?;

    // ── Le renommage : c'est LUI qui rend l'installation atomique ───────
    let _ = std::fs::remove_dir_all(&final_);
    std::fs::rename(&partiel, &final_).map_err(|e| format!("renommage final : {e}"))?;

    Ok(final_)
}

/// L'installation réelle : vrai réseau, vraie bibliothèque.
pub fn installer(slug: &str, progres: &mut dyn FnMut(u32, u32)) -> Result<PathBuf, String> {
    let Some(racine) = crate::config::dossier_bibliotheque() else {
        return Err("%APPDATA% introuvable : pas de bibliothèque".to_string());
    };
    installer_avec(&ReseauWinHttp::new(), &racine, slug, progres)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalogue::reseau::ReseauFake;

    /// Fabrique un PNG dont seul l'EN-TÊTE est valide.
    ///
    /// Suffisant pour `taille_png` ; `decoder_png` échouera dessus, ce qui
    /// exerce précisément le repli de la hitbox sur la toile entière.
    fn png_factice(l: u32, h: u32) -> Vec<u8> {
        let mut o = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        o.extend_from_slice(&13u32.to_be_bytes());
        o.extend_from_slice(b"IHDR");
        o.extend_from_slice(&l.to_be_bytes());
        o.extend_from_slice(&h.to_be_bytes());
        o
    }

    fn reseau_d_un_pack_complet(slug: &str) -> ReseauFake {
        let mut r = Vec::new();
        for n in 1..=46u32 {
            r.push((
                format!("{CDN}/{slug}/img/shime{n}.png"),
                Some(png_factice(128, 128)),
            ));
        }
        r.push((
            format!("{CDN}/{slug}/actions.xml"),
            Some(br#"<Mascot><Pose Image="/shime1.png" ImageAnchor="64,128"/></Mascot>"#.to_vec()),
        ));
        ReseauFake::new(r)
    }

    /// Le slug construit un CHEMIN et une URL : il est validé avant les deux.
    #[test]
    fn le_slug_est_valide_avant_tout() {
        assert!(slug_valide("one-piece-luffy-01"));
        assert!(slug_valide("zoro-8bd774"));
        assert!(!slug_valide(""));
        assert!(!slug_valide(".."), "traversée de chemin");
        assert!(!slug_valide("a/b"), "séparateur de chemin");
        assert!(!slug_valide("a\\b"));
        assert!(!slug_valide("a b"), "espace");
    }

    /// Une installation complète, de bout en bout, sans réseau réel.
    #[test]
    fn une_installation_complete_produit_un_dossier_lisible() {
        let racine = std::env::temp_dir().join("shimeji-test-install-ok");
        let _ = std::fs::remove_dir_all(&racine);
        let reseau = reseau_d_un_pack_complet("pack-test");

        let mut vus = Vec::new();
        let dossier = installer_avec(&reseau, &racine, "pack-test", &mut |fait, total| {
            vus.push((fait, total));
        })
        .expect("l'installation doit réussir");

        assert!(dossier.join("mascot.json").is_file());
        assert!(dossier.join("img").join("shime1.png").is_file());
        assert!(!vus.is_empty(), "la progression doit être rapportée");

        // Le manifeste écrit doit être relisible par le moteur.
        let texte = crate::config::lire_json(&dossier.join("mascot.json")).expect("lecture");
        let m: crate::character::manifest::Manifest =
            serde_json::from_str(&texte).expect("le moteur doit relire");
        assert!(m.poses.contains_key("walk"));
    }

    /// **LE cas qui compte** : une coupure à mi-parcours ne laisse JAMAIS un
    /// personnage à moitié installé.
    ///
    /// C'est le genre de panne qui se diagnostique très mal, parce qu'elle
    /// ressemble à un bug du moteur plutôt qu'à un téléchargement raté.
    #[test]
    fn une_coupure_ne_laisse_pas_de_pack_a_moitie() {
        let racine = std::env::temp_dir().join("shimeji-test-install-coupe");
        let _ = std::fs::remove_dir_all(&racine);
        let reseau = reseau_d_un_pack_complet("pack-coupe");
        reseau.echouer_apres(20);

        let r = installer_avec(&reseau, &racine, "pack-coupe", &mut |_, _| {});
        assert!(r.is_err(), "la coupure doit faire échouer l'installation");
        assert!(
            !racine.join("pack-coupe").exists(),
            "le dossier final ne doit pas exister"
        );
    }

    /// Un pack trop pauvre est refusé, et ne laisse rien derrière lui.
    #[test]
    fn un_pack_sans_pose_vitale_ne_s_installe_pas() {
        let racine = std::env::temp_dir().join("shimeji-test-install-pauvre");
        let _ = std::fs::remove_dir_all(&racine);
        // Seule shime1 existe : de quoi tenir debout, pas de quoi marcher.
        let reseau = ReseauFake::new(vec![(
            format!("{CDN}/pauvre/img/shime1.png"),
            Some(png_factice(128, 128)),
        )]);

        let r = installer_avec(&reseau, &racine, "pauvre", &mut |_, _| {});
        assert!(r.is_err());
        assert!(!racine.join("pauvre").exists());
    }
}
