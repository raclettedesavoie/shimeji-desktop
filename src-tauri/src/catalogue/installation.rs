//! Les fonctions pures de l'installation (spec §8, §11).
//!
//! Responsabilité unique : transformer des octets en données de manifeste.
//! Aucun réseau, aucun disque — c'est ce qui les rend testables sur des
//! images fabriquées en mémoire, sans sortir de la machine.

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
