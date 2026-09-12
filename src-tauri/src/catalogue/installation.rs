//! Les fonctions pures de l'installation (spec §8, §11).
//!
//! Responsabilité unique : transformer des octets en données de manifeste.
//! Aucun réseau, aucun disque — c'est ce qui les rend testables sur des
//! images fabriquées en mémoire, sans sortir de la machine.

use std::collections::{BTreeMap, BTreeSet};

/// La définition d'une pose du vocabulaire Shimeji.
///
/// `&'static` partout : la table entière est connue à la compilation, donc
/// rien n'est alloué et elle ne peut pas devenir incohérente à l'exécution.
pub struct DefPose {
    pub nom: &'static str,
    pub frames: &'static [u32],
    pub frame_ms: u32,
    pub looping: bool,
    /// Ancre imposée par la pose, quand elle ne se déduit pas du dessin.
    pub ancre: Option<[f32; 2]>,
}

/// Le vocabulaire de poses = **les slots Shimeji**.
///
/// La numérotation `shime1..46` est un standard de fait : tous les packs
/// utilisent les mêmes numéros pour les mêmes poses. En visant ce
/// vocabulaire, tout pack du catalogue fonctionne **sans code** (spec §8).
///
/// ⚠️ Correspondance tirée de `conf/actions.xml` de Shimeji-ee et relevée
/// dans `docs/specs/2026-09-09-frames-shimeji.md` — **pas devinée à l'œil**.
/// C'est la leçon de l'étape 1a : tout ce qui avait été réglé à l'œil s'est
/// révélé faux.
pub const POSES: &[DefPose] = &[
    DefPose { nom: "stand", frames: &[1], frame_ms: 150, looping: false, ancre: None },
    DefPose { nom: "walk", frames: &[1, 2, 1, 3], frame_ms: 240, looping: true, ancre: None },
    DefPose { nom: "run", frames: &[1, 2, 1, 3], frame_ms: 80, looping: true, ancre: None },
    DefPose { nom: "sit", frames: &[11], frame_ms: 150, looping: false, ancre: None },

    // `sleep` est la MÊME frame que `sprawl` : Shimeji-ee n'a aucune
    // animation de sommeil, et aucune frame n'a les yeux fermés. Déclarée
    // sous le nom `sleep` pour que le code ignore qu'il s'agit d'un
    // substitut — un pack tiers avec une vraie pose de sommeil la
    // déclarerait au même nom, sans changement côté Rust.
    DefPose { nom: "sleep", frames: &[21], frame_ms: 150, looping: false, ancre: None },

    DefPose { nom: "fall", frames: &[4], frame_ms: 150, looping: false, ancre: None },
    DefPose { nom: "land", frames: &[18, 19], frame_ms: 160, looping: false, ancre: None },

    // Les poses de glisser portent leur ancre remontée : `Dragged.java`
    // place l'ancre à curseur + (0,120) avec ImageAnchor 64,128 — le haut du
    // sprite est donc 8 px AU-DESSUS du curseur, il le tient par la tête.
    DefPose { nom: "dragged", frames: &[1], frame_ms: 150, looping: false, ancre: Some([64.0, 8.0]) },
    DefPose { nom: "draggedRight1", frames: &[5], frame_ms: 150, looping: false, ancre: Some([64.0, 8.0]) },
    DefPose { nom: "draggedRight2", frames: &[7], frame_ms: 150, looping: false, ancre: Some([64.0, 8.0]) },
    DefPose { nom: "draggedRight3", frames: &[9], frame_ms: 150, looping: false, ancre: Some([64.0, 8.0]) },
    DefPose { nom: "draggedLeft1", frames: &[6], frame_ms: 150, looping: false, ancre: Some([64.0, 8.0]) },
    DefPose { nom: "draggedLeft2", frames: &[8], frame_ms: 150, looping: false, ancre: Some([64.0, 8.0]) },
    DefPose { nom: "draggedLeft3", frames: &[10], frame_ms: 150, looping: false, ancre: Some([64.0, 8.0]) },

    DefPose { nom: "sprawl", frames: &[21], frame_ms: 150, looping: false, ancre: None },
    DefPose { nom: "tripping", frames: &[19, 18, 20, 20, 19], frame_ms: 160, looping: false, ancre: None },
    DefPose { nom: "creep", frames: &[20, 20, 21, 21, 21], frame_ms: 160, looping: true, ancre: None },
    DefPose { nom: "sitDangle", frames: &[31, 32, 31, 33], frame_ms: 400, looping: true, ancre: Some([64.0, 112.0]) },
    DefPose { nom: "sitLegsUp", frames: &[30], frame_ms: 150, looping: false, ancre: Some([64.0, 112.0]) },
    DefPose { nom: "sitLookUp", frames: &[26], frame_ms: 150, looping: false, ancre: None },
    DefPose { nom: "spinHead", frames: &[26, 15, 27, 16, 28, 17, 29, 11], frame_ms: 200, looping: false, ancre: None },
    DefPose { nom: "jump", frames: &[22], frame_ms: 150, looping: false, ancre: None },

    DefPose { nom: "grabWall", frames: &[13], frame_ms: 150, looping: false, ancre: None },
    DefPose { nom: "climbWall", frames: &[14, 12, 13], frame_ms: 160, looping: true, ancre: None },
    DefPose { nom: "grabCeiling", frames: &[23], frame_ms: 150, looping: false, ancre: Some([64.0, 48.0]) },
    DefPose { nom: "climbCeiling", frames: &[23, 24, 25], frame_ms: 160, looping: true, ancre: Some([64.0, 48.0]) },

    DefPose { nom: "split", frames: &[42, 43, 44, 45, 46], frame_ms: 160, looping: false, ancre: None },
];

/// Les poses sans lesquelles un personnage ne peut pas vivre.
///
/// Un pack qui n'a pas de quoi tenir debout et marcher n'est pas
/// installable : mieux vaut le refuser **bruyamment** que livrer un dossier
/// qui fera échouer le chargement — ce qui ressemblerait à un bug du moteur
/// plutôt qu'à un pack incomplet.
pub const POSES_VITALES: &[&str] = &["stand", "walk"];

/// Les ancres déclarées par le pack lui-même, dans son `actions.xml`.
///
/// On lit **au motif** plutôt qu'avec un parseur XML : deux attributs à
/// extraire ne justifient pas une dépendance (spec §4). Le format est stable
/// depuis Shimeji-ee, et un XML inattendu rend simplement une table vide —
/// ce qui déclenche le repli documenté, pas une erreur.
pub fn ancres_de_actions_xml(xml: &str) -> BTreeMap<u32, [f32; 2]> {
    let mut ancres = BTreeMap::new();

    // On découpe sur `Image="` : chaque morceau contient donc le chemin de
    // l'image, puis ses attributs, jusqu'au prochain `Image="`. `skip(1)` —
    // le texte AVANT la première occurrence n'est pas une pose.
    for morceau in xml.split("Image=\"").skip(1) {
        // ── Le numéro de la frame ───────────────────────────────────────
        let Some(fin_chemin) = morceau.find('"') else {
            continue;
        };
        let chemin = &morceau[..fin_chemin];
        let Some(pos) = chemin.rfind("shime") else {
            continue;
        };
        let numero_txt: String = chemin[pos + 5..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        let Ok(n) = numero_txt.parse::<u32>() else {
            continue;
        };

        // ── L'ancre, cherchée dans le MÊME morceau ──────────────────────
        // Donc avant le prochain `Image="`, ce qui garantit qu'on ne prend
        // pas l'ancre de la pose suivante.
        let Some(pos_ancre) = morceau.find("ImageAnchor=\"") else {
            continue;
        };
        let apres = &morceau[pos_ancre + 13..];
        let Some(fin) = apres.find('"') else {
            continue;
        };
        let mut parts = apres[..fin].split(',');
        let (Some(x), Some(y)) = (parts.next(), parts.next()) else {
            continue;
        };
        let (Ok(x), Ok(y)) = (x.trim().parse::<f32>(), y.trim().parse::<f32>()) else {
            continue;
        };

        ancres.insert(n, [x, y]);
    }

    ancres
}

/// Les poses dont **toutes** les frames sont présentes (spec §8.7).
pub fn poses_retenues(presentes: &BTreeSet<u32>) -> Vec<&'static DefPose> {
    POSES
        .iter()
        .filter(|def| def.frames.iter().all(|n| presentes.contains(n)))
        .collect()
}

/// Engendre le `mascot.json` du pack.
///
/// Écrit à la main plutôt que sérialisé depuis une struct : le manifeste
/// porte des champs de documentation (`_source`, `_mesures`) que le moteur
/// ignore mais qu'un humain lira dans six mois, et `Manifest` ne les a pas.
pub fn ecrire_mascot_json(
    slug: &str,
    presentes: &BTreeSet<u32>,
    tailles: &BTreeMap<u32, [u32; 2]>,
    ancres: &BTreeMap<u32, [f32; 2]>,
    hitbox: [u32; 4],
) -> Result<String, String> {
    let retenues = poses_retenues(presentes);

    for vitale in POSES_VITALES {
        if !retenues.iter().any(|p| p.nom == *vitale) {
            return Err(format!(
                "pack inutilisable : la pose « {vitale} » manque ({} frames présentes)",
                presentes.len()
            ));
        }
    }

    // La toile déclarée est la PLUS GRANDE des frames : c'est le repli pour
    // les images absentes de la table `frames`, et tant que la fenêtre
    // adaptative n'est pas branchée, c'est elle que le moteur emploie.
    let mut tl = 0u32;
    let mut th = 0u32;
    for [l, h] in tailles.values() {
        if *l > tl {
            tl = *l;
        }
        if *h > th {
            th = *h;
        }
    }
    if tl == 0 || th == 0 {
        return Err("aucune frame mesurable".to_string());
    }

    let mut s = String::new();
    s.push_str("{\n");
    s.push_str(&format!(
        "  \"_source\": \"Installe par shimeji-desktop depuis le catalogue shimejis.xyz, slug '{slug}' (CDN https://sprites.shimejis.xyz/directory/{slug}/). L'art n'est PAS de nous : voir l'avertissement du CLAUDE.md avant toute publication du depot.\",\n"
    ));
    s.push_str(&format!(
        "  \"_mesures\": \"Tailles lues dans l'en-tete PNG de chaque frame, sans decodage. Ancres lues dans l'actions.xml du pack quand le CDN le sert, sinon supposees au milieu-bas (convention Shimeji-ee). Hitbox MESUREE sur la boite opaque de shime1 et non recopiee sur blob : la numerotation des poses se transpose d'un pack a l'autre, les proportions du dessin non. Frames presentes : {}.\",\n",
        presentes.len()
    ));
    s.push_str(&format!("  \"id\": \"{slug}\",\n"));
    s.push_str(&format!("  \"name\": \"{slug}\",\n"));
    s.push_str(&format!("  \"frameSize\": [{tl}, {th}],\n"));
    s.push_str("  \"scale\": 1,\n");
    s.push_str(&format!(
        "  \"hitbox\": [{}, {}, {}, {}],\n",
        hitbox[0], hitbox[1], hitbox[2], hitbox[3]
    ));

    // ── La table `frames` ───────────────────────────────────────────────
    s.push_str("  \"frames\": {\n");
    let mut premiere = true;
    for (n, [l, h]) in tailles {
        if !premiere {
            s.push_str(",\n");
        }
        premiere = false;
        let ancre = match ancres.get(n) {
            Some([x, y]) => format!(", \"anchor\": [{x}, {y}]"),
            // Pas d'ancre déclarée : on n'en invente pas ici. Le repli sur
            // celle de la pose se fait à la lecture (`ancre_de_frame`).
            None => String::new(),
        };
        s.push_str(&format!("    \"{n}\": {{ \"size\": [{l}, {h}]{ancre} }}"));
    }
    s.push_str("\n  },\n");

    // ── Les poses ───────────────────────────────────────────────────────
    s.push_str("  \"poses\": {\n");
    let mut premiere = true;
    for def in &retenues {
        if !premiere {
            s.push_str(",\n");
        }
        premiere = false;

        let frames: Vec<String> = def.frames.iter().map(|n| n.to_string()).collect();
        let [ax, ay] = match def.ancre {
            Some(a) => a,
            // Sans ancre imposée, on prend celle de la première frame lue
            // dans l'`actions.xml`, et à défaut le milieu-bas de la toile —
            // la convention de Shimeji-ee.
            None => def
                .frames
                .first()
                .and_then(|n| ancres.get(n))
                .copied()
                .unwrap_or([tl as f32 / 2.0, th as f32]),
        };

        s.push_str(&format!(
            "    \"{}\": {{ \"frames\": [{}], \"frameMs\": {}, \"loop\": {}, \"anchor\": [{ax}, {ay}] }}",
            def.nom,
            frames.join(", "),
            def.frame_ms,
            def.looping
        ));
    }
    s.push_str("\n  }\n}\n");

    Ok(s)
}

/// La signature qui ouvre tout fichier PNG.
const SIGNATURE_PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Largeur et hauteur, lues dans l'en-tête **sans décoder l'image**.
///
/// Structure d'un PNG : 8 octets de signature, puis des chunks. Le premier
/// est toujours `IHDR`, dont les deux premiers champs sont la largeur et la
/// hauteur, sur 4 octets chacun, en **gros-boutiste**. Les 24 premiers
/// octets suffisent donc.
///
/// Rend `None` sur tout ce qui n'est pas un PNG. Ce n'est pas de la
/// méfiance gratuite : le CDN peut servir une page d'erreur HTML avec un
/// statut 200, et la prendre pour une image écrirait une taille absurde dans
/// le manifeste — que l'on ne découvrirait qu'à l'affichage.
pub fn taille_png(octets: &[u8]) -> Option<[u32; 2]> {
    if octets.len() < 24 {
        return None;
    }
    if octets[..8] != SIGNATURE_PNG {
        return None;
    }
    if &octets[12..16] != b"IHDR" {
        return None;
    }

    // `try_into` rend un `Result` parce qu'une tranche pourrait ne pas faire
    // 4 octets. Ici la longueur est déjà vérifiée, donc `ok()?` est
    // inatteignable — mais il évite un `unwrap`, qui paniquerait.
    let l = u32::from_be_bytes(octets[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(octets[20..24].try_into().ok()?);

    // Une dimension nulle n'est pas une image.
    if l == 0 || h == 0 {
        return None;
    }

    Some([l, h])
}

/// Décode un PNG en RGBA. **Une seule image par installation.**
///
/// Rend `(pixels, largeur, hauteur)`, ou `None` si l'image est illisible ou
/// dans un format qu'on n'accepte pas.
pub fn decoder_png(octets: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let decodeur = png::Decoder::new(octets);

    // `ok()?` partout : un PNG illisible n'est pas une erreur à remonter,
    // c'est un pack dont on mesurera la hitbox autrement (repli documenté
    // dans `hitbox_depuis`).
    let mut lecteur = decodeur.read_info().ok()?;
    let mut tampon = vec![0u8; lecteur.output_buffer_size()];
    let info = lecteur.next_frame(&mut tampon).ok()?;

    // On n'accepte que le RGBA 8 bits : c'est ce que servent tous les packs
    // Shimeji, et convertir les autres formats coûterait la crate `image`
    // entière pour un cas qui ne se présente pas.
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }

    tampon.truncate(info.buffer_size());
    Some((tampon, info.width, info.height))
}

/// La boîte englobante des pixels non transparents, bornes **inclusives**.
///
/// Rend `None` si l'image est entièrement transparente, ce qui arrive sur
/// certaines frames de packs incomplets.
pub fn boite_opaque(rgba: &[u8], largeur: u32, hauteur: u32) -> Option<[u32; 4]> {
    // Un pixel est jugé opaque au-delà de ce seuil, et non dès `> 0` : les
    // bords anticrénelés portent un alpha de 1 ou 2 qui gonflerait la boîte
    // de plusieurs pixels sans qu'on y voie quoi que ce soit.
    const SEUIL_ALPHA: u8 = 16;

    let mut x0 = u32::MAX;
    let mut y0 = u32::MAX;
    let mut x1 = 0u32;
    let mut y1 = 0u32;
    let mut trouve = false;

    for y in 0..hauteur {
        for x in 0..largeur {
            let i = ((y * largeur + x) * 4 + 3) as usize;
            // Garde de longueur : une image tronquée ne doit pas paniquer.
            if i >= rgba.len() {
                continue;
            }
            if rgba[i] > SEUIL_ALPHA {
                trouve = true;
                if x < x0 {
                    x0 = x;
                }
                if y < y0 {
                    y0 = y;
                }
                if x > x1 {
                    x1 = x;
                }
                if y > y1 {
                    y1 = y;
                }
            }
        }
    }

    if trouve {
        Some([x0, y0, x1, y1])
    } else {
        None
    }
}

/// La hitbox `[x, y, largeur, hauteur]` telle que le manifeste l'écrit.
///
/// Repli sur l'image entière quand rien n'est opaque : mieux vaut une hitbox
/// trop large qu'un personnage impossible à attraper (spec §3.3).
pub fn hitbox_depuis(rgba: &[u8], largeur: u32, hauteur: u32) -> [u32; 4] {
    match boite_opaque(rgba, largeur, hauteur) {
        // Bornes inclusives → `+ 1` pour obtenir une dimension.
        Some([x0, y0, x1, y1]) => [x0, y0, x1 - x0 + 1, y1 - y0 + 1],
        None => [0, 0, largeur, hauteur],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fabrique l'en-tête d'un PNG : signature + chunk IHDR.
    fn en_tete(l: u32, h: u32) -> Vec<u8> {
        let mut o = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        o.extend_from_slice(&13u32.to_be_bytes()); // longueur du chunk
        o.extend_from_slice(b"IHDR");
        o.extend_from_slice(&l.to_be_bytes());
        o.extend_from_slice(&h.to_be_bytes());
        o
    }

    /// La taille se lit dans l'en-tête, **sans décoder l'image**.
    ///
    /// C'est ce qui évite 46 décodages par pack : seule la frame dont on
    /// mesure la hitbox est réellement décodée.
    #[test]
    fn la_taille_se_lit_dans_l_en_tete_png() {
        assert_eq!(taille_png(&en_tete(185, 155)), Some([185, 155]));

        // Ce qui n'est pas un PNG est REFUSÉ, jamais deviné : le CDN peut
        // servir une page d'erreur HTML avec un statut 200, et la prendre
        // pour une image écrirait une taille absurde dans le manifeste.
        assert_eq!(taille_png(b"<!doctype html><html>"), None);
        assert_eq!(taille_png(&[]), None);
        assert_eq!(taille_png(&en_tete(128, 128)[..12]), None, "tronqué → refus");

        // Une dimension nulle n'est pas une image.
        assert_eq!(taille_png(&en_tete(0, 128)), None);
    }

    /// La hitbox est MESURÉE sur les pixels opaques, jamais recopiée.
    ///
    /// C'est la leçon de l'étape 1a, où les quatre réglages faits à l'œil se
    /// sont tous révélés faux — et celle de Luffy, chibi dont le chapeau
    /// touche le bord haut de la boîte, que le `y = 20` de `blob` aurait
    /// amputé.
    #[test]
    fn la_hitbox_est_mesuree_sur_les_pixels_opaques() {
        // Une image 4×4 dont seul le carré (1,1)-(2,2) est opaque.
        let mut rgba = vec![0u8; 4 * 4 * 4];
        for y in 1..=2u32 {
            for x in 1..=2u32 {
                let i = ((y * 4 + x) * 4) as usize;
                rgba[i + 3] = 255; // l'alpha est le 4e octet
            }
        }

        assert_eq!(boite_opaque(&rgba, 4, 4), Some([1, 1, 2, 2]));
        // [x, y, largeur, hauteur] — bornes inclusives, donc 2-1+1 = 2.
        assert_eq!(hitbox_depuis(&rgba, 4, 4), [1, 1, 2, 2]);

        // Entièrement transparente : pas de boîte. La hitbox se replie sur
        // l'image entière — mieux vaut trop large qu'un personnage
        // impossible à attraper.
        let vide = vec![0u8; 4 * 4 * 4];
        assert_eq!(boite_opaque(&vide, 4, 4), None);
        assert_eq!(hitbox_depuis(&vide, 4, 4), [0, 0, 4, 4]);
    }

    /// Les ancres se lisent dans l'`actions.xml` du pack, servi par le CDN.
    ///
    /// C'est LA source de vérité : les proportions d'un dessin ne se
    /// transposent pas d'un pack à l'autre, seule la numérotation le fait.
    #[test]
    fn les_ancres_se_lisent_dans_actions_xml() {
        let xml = r#"<?xml version="1.0"?>
        <Mascot>
          <Pose Image="/shime1.png" ImageAnchor="64,128" Duration="1"/>
          <Pose Image="/shime23.png" ImageAnchor="48,4" Duration="1"/>
          <Pose Image="/pas-un-numero.png" ImageAnchor="1,2" Duration="1"/>
        </Mascot>"#;

        let a = ancres_de_actions_xml(xml);
        assert_eq!(a.get(&1), Some(&[64.0, 128.0]));
        assert_eq!(a.get(&23), Some(&[48.0, 4.0]));
        assert_eq!(a.len(), 2, "les images non numérotées sont ignorées");

        // Un XML absent ou illisible ne casse rien : table vide, et
        // l'appelant se replie sur la convention de Shimeji-ee.
        assert!(ancres_de_actions_xml("").is_empty());
        assert!(ancres_de_actions_xml("<html>erreur 500</html>").is_empty());
    }

    /// On ne déclare QUE les poses dont TOUTES les frames existent.
    ///
    /// Un pack incomplet n'est pas une erreur : l'option est simplement
    /// retirée du tirage d'envies (spec §8.7). Aucun cas particulier à
    /// coder — il suffit de ne pas déclarer la pose.
    #[test]
    fn seules_les_poses_completes_sont_retenues() {
        // Un pack qui n'a que 1, 2, 3 : de quoi tenir debout et marcher,
        // rien de plus.
        let presentes: BTreeSet<u32> = [1, 2, 3].into_iter().collect();
        let retenues = poses_retenues(&presentes);
        let noms: Vec<&str> = retenues.iter().map(|p| p.nom).collect();

        assert!(noms.contains(&"stand"), "stand n'a besoin que de la frame 1");
        assert!(noms.contains(&"walk"), "walk a besoin de 1, 2, 3");
        assert!(!noms.contains(&"climbWall"), "climbWall exige 12, 13, 14");
        assert!(!noms.contains(&"sleep"), "sleep exige la frame 21");
    }

    /// Le manifeste écrit doit être relisible **par le moteur**.
    ///
    /// C'est la seule vérification qui prouve qu'il est utilisable : un JSON
    /// syntaxiquement valide mais dont un champ manque ne servirait à rien.
    #[test]
    fn le_mascot_json_ecrit_est_relisible_par_le_moteur() {
        let presentes: BTreeSet<u32> = (1..=46).collect();
        let mut tailles = BTreeMap::new();
        let mut ancres = BTreeMap::new();
        for n in 1..=46u32 {
            tailles.insert(n, [128u32, 128u32]);
            ancres.insert(n, [64.0f32, 128.0f32]);
        }

        let json =
            ecrire_mascot_json("un-slug", &presentes, &tailles, &ancres, [40, 20, 48, 108])
                .expect("génération");

        assert!(!json.starts_with('\u{feff}'), "jamais de BOM");

        let m: crate::character::manifest::Manifest =
            serde_json::from_str(&json).expect("le moteur doit relire ce qu'on écrit");
        assert_eq!(m.id, "un-slug");
        assert!(m.poses.contains_key("stand"));
        assert!(m.poses.contains_key("walk"));
        assert_eq!(m.taille_de_frame(1), [128, 128]);
        assert_eq!(m.frames.len(), 46, "la table frames est écrite");
    }

    /// Un pack sans pose vitale est refusé **bruyamment**.
    ///
    /// Mieux vaut ça que livrer un dossier qui fera échouer le chargement,
    /// ce qui ressemblerait à un bug du moteur.
    #[test]
    fn un_pack_sans_pose_vitale_est_refuse() {
        // Seule shime1 : de quoi tenir debout, pas de quoi marcher.
        let presentes: BTreeSet<u32> = [1].into_iter().collect();
        let mut tailles = BTreeMap::new();
        tailles.insert(1u32, [128u32, 128u32]);

        let r = ecrire_mascot_json("pauvre", &presentes, &tailles, &BTreeMap::new(), [0, 0, 1, 1]);
        assert!(r.is_err(), "un pack sans walk doit être refusé");
    }

    /// Un VRAI PNG du dépôt.
    ///
    /// Sans lui, les deux tests ci-dessus ne prouveraient que la cohérence de
    /// notre fabrication d'octets avec notre lecture — pas qu'on sait lire un
    /// fichier réel.
    #[test]
    fn la_taille_se_lit_sur_une_vraie_frame() {
        // Chemin relatif au dossier du CRATE (`src-tauri/`), d'où le `..`.
        let chemin = std::path::Path::new("../characters/blob/img/shime1.png");
        let octets = std::fs::read(chemin).expect("blob/shime1.png doit exister");
        assert_eq!(taille_png(&octets), Some([128, 128]));

        let (rgba, l, h) = decoder_png(&octets).expect("PNG RGBA 8 bits");
        assert_eq!((l, h), (128, 128));
        let hb = hitbox_depuis(&rgba, l, h);
        assert!(hb[2] > 0 && hb[3] > 0, "blob n'est pas transparent : {hb:?}");
    }
}
