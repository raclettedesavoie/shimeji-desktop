//! La couche d'affichage de l'overlay : répartir les sprites par écran, et
//! fabriquer le JavaScript qui les porte.
//!
//! Responsabilité unique, et **aucune dépendance à Tauri** : ce module ne
//! connaît ni fenêtre, ni `eval`, ni thread. Il transforme de la donnée en
//! donnée — c'est précisément ce qui le rend testable sans écran, alors que
//! tout le reste de cette migration ne se vérifie qu'à l'œil.
//!
//! Conception : `docs/specs/2026-09-23-fenetre-par-ecran-design.md` §5.

use crate::probe::ScreenInfo;

/// Un sprite tel que la boucle le calcule : en pixels du **bureau virtuel**,
/// coin haut-gauche.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteRendu {
    /// Identité stable dans le temps, pour que le webview retrouve le même
    /// élément `<img>` d'une image à l'autre.
    ///
    /// ⚠️ C'est le **compteur monotone** des acteurs, jamais leur index dans
    /// le `Vec` : un index se réutilise quand un acteur part, et le suivant
    /// hériterait alors de la position interpolée du précédent — il
    /// traverserait l'écran en glissant, sans qu'aucune erreur ne le signale.
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub image: u32,
    pub flip: bool,
}

/// Le même sprite, mais en pixels **relatifs à la fenêtre d'un écran**.
///
/// Un type distinct de `SpriteRendu` bien que les champs soient identiques :
/// les deux repères ne sont pas interchangeables, et le compilateur refuse
/// désormais de les confondre. C'est la seule protection qu'on ait contre une
/// erreur qui, sinon, ne se verrait qu'à l'écran et seulement sur le second
/// moniteur.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteRelatif {
    pub id: u32,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub image: u32,
    pub flip: bool,
}

/// Ce qu'un écran doit afficher.
#[derive(Debug, Clone, PartialEq)]
pub struct ChargeEcran {
    /// L'identité de l'écran (`ScreenInfo::id`), dérivée du `HMONITOR` et
    /// donc stable dans le temps (spec §5.2).
    pub ecran: u64,
    pub sprites: Vec<SpriteRelatif>,
}

/// Répartit les sprites sur les écrans qu'ils touchent.
///
/// **Un sprite à cheval est émis dans les DEUX écrans** (conception §5.2),
/// avec des coordonnées relatives à chacun — donc négatives dans celui où il
/// entre par la gauche ou par le haut. Chaque fenêtre le coupe à son bord, et
/// les deux moitiés se rejoignent à l'écran.
///
/// **Un écran sans sprite ne produit aucune charge** (§5.1) : c'est ce qui
/// permet à l'appelant de ne pas créer sa fenêtre, et donc de ne pas payer le
/// péage de ~34 % mesuré par le spike.
pub fn repartir(sprites: &[SpriteRendu], ecrans: &[ScreenInfo]) -> Vec<ChargeEcran> {
    let mut charges: Vec<ChargeEcran> = Vec::new();

    for e in ecrans {
        let z = e.work_area;
        let mut dedans: Vec<SpriteRelatif> = Vec::new();

        for s in sprites {
            // Intersection de deux rectangles, en flottants parce que
            // `work_area` l'est. Les sprites, eux, sont déjà arrondis par la
            // boucle : c'est l'entier qu'elle compare pour savoir si quelque
            // chose a bougé.
            let sx = s.x as f32;
            let sy = s.y as f32;
            let sw = s.w as f32;
            let sh = s.h as f32;

            // Comparaisons STRICTES : un sprite dont le bord droit touche
            // exactement le bord gauche de l'écran ne montre aucun pixel
            // dessus. L'inclure ferait exister une fenêtre pour rien.
            let touche =
                sx < z.right() && sx + sw > z.left() && sy < z.bottom() && sy + sh > z.top();

            if !touche {
                continue;
            }

            dedans.push(SpriteRelatif {
                id: s.id,
                // Le passage en coordonnées de fenêtre. C'est la SEULE
                // conversion de repère du module, et la seule occasion de se
                // tromper — d'où le test dédié.
                x: s.x - z.x as i32,
                y: s.y - z.y as i32,
                w: s.w,
                h: s.h,
                image: s.image,
                flip: s.flip,
            });
        }

        // `is_empty` plutôt que pousser une charge vide : une charge vide
        // ferait croire à l'appelant que l'écran est occupé, et il ouvrirait
        // une fenêtre que personne n'habite.
        if !dedans.is_empty() {
            charges.push(ChargeEcran {
                ecran: e.id,
                sprites: dedans,
            });
        }
    }

    charges
}

/// Fabrique l'appel JavaScript qui porte toute la charge d'un écran.
///
/// # Pourquoi un tableau de tableaux, et pas des objets
///
/// `[4,10,20,128,128,7,0]` fait 21 caractères ; l'objet équivalent en fait
/// plus du double. Cette chaîne est construite **jusqu'à 45 fois par
/// seconde** et traverse la frontière de processus à chaque fois : c'est Rust
/// qui paie sa construction, et le webview son analyse.
///
/// L'ordre des champs est `[id, x, y, w, h, image, flip]` — le **même** que
/// celui que lit `ui/overlay.js`. Les deux se corrompent en silence s'ils
/// divergent : un `w` lu comme un `y` ne produit aucune erreur, juste un
/// sprite au mauvais endroit. **Les modifier ensemble, toujours.**
///
/// # Pourquoi rien n'est échappé
///
/// Il n'entre dans cette chaîne que des entiers et un booléen, tous produits
/// par nous. Aucun nom de fichier, aucune chaîne venue d'un pack tiers — le
/// personnage à afficher passe par `window.declarer`, pas par ici. C'est ce
/// qui rend l'absence d'échappement sûre, et c'est pourquoi il ne faut
/// **jamais** ajouter de texte à cette charge utile sans revoir ce point.
pub fn js_de(charge: &ChargeEcran) -> String {
    // 32 caractères par sprite : une estimation large, pour que le `String`
    // ne se réalloue pas en cours de route.
    let mut js = String::with_capacity(24 + charge.sprites.len() * 32);
    js.push_str("window.poserTous([");

    for (i, s) in charge.sprites.iter().enumerate() {
        if i > 0 {
            js.push(',');
        }
        js.push_str(&format!(
            "[{},{},{},{},{},{},{}]",
            s.id,
            s.x,
            s.y,
            s.w,
            s.h,
            s.image,
            // `u8` plutôt que `bool` : `0`/`1` au lieu de `false`/`true`,
            // soit quatre caractères de moins par sprite sur une chaîne
            // qu'on refabrique 45 fois par seconde.
            u8::from(s.flip)
        ));
    }

    js.push_str("])");
    js
}

// Les tests de ce module vivent dans `overlay_tests.rs`, selon la convention
// du projet (voir `world.rs`).
#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
