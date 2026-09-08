//! Géométrie du bureau virtuel : points, vecteurs, rectangles, faces.
//!
//! Responsabilité unique : les primitives spatiales, sans aucune notion de
//! personnage, de fenêtre ou d'écran. Tout est en PIXELS PHYSIQUES du bureau
//! virtuel (spec §3.4) — le facteur d'échelle d'un moniteur ne sert qu'au
//! dimensionnement du sprite, et n'entre jamais ici.

/// Un côté utilisable d'un rectangle.
///
/// Vit dans `geom` et non dans `world` parce que `Rect::point_on` en a besoin :
/// le placer dans `world` créerait un cycle geom → world → geom. C'est de la
/// géométrie — un côté de rectangle.
///
/// `Copy` : quatre variantes sans données, donc copier coûte moins que de
/// raisonner sur qui la possède. `PartialEq` pour les comparaisons de tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    /// Le dessus — on marche dessus. Le sol d'un écran, la barre de titre d'une fenêtre.
    Top,
    /// Le bord gauche — on s'y agrippe (étape 4).
    Left,
    /// Le bord droit — idem.
    Right,
    /// Le dessous — on s'y suspend (étape 4).
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Point { x, y }
    }

    /// Distance euclidienne. Sert à la proximité entre personnages (étape 3)
    /// et au choix du sol le plus proche quand un personnage tombe hors du
    /// bureau (Tâche 6).
    pub fn distance_to(&self, other: Point) -> f32 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Un déplacement, ou une vitesse. Même forme qu'un `Point`, mais un type
/// distinct : additionner une position à une position n'a pas de sens, alors
/// qu'additionner un vecteur à une position en a. Le compilateur le fait
/// respecter gratuitement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    pub fn zero() -> Self {
        Vec2 { x: 0.0, y: 0.0 }
    }
}

/// Un rectangle, défini par son coin supérieur gauche et sa taille.
///
/// Convention d'axes de Windows : `y` croît **vers le bas**. `top()` est donc
/// la plus PETITE valeur de `y`, et `bottom()` la plus grande. C'est
/// contre-intuitif si l'on vient des mathématiques, et c'est la source d'erreur
/// de signe la plus courante dans ce genre de code — d'où ces accesseurs
/// nommés, plutôt que des comparaisons écrites à la main partout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Rect { x, y, w, h }
    }

    pub fn left(&self) -> f32 {
        self.x
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    /// Le plus petit `y` — voir la note sur la convention d'axes.
    pub fn top(&self) -> f32 {
        self.y
    }

    /// Le plus grand `y`.
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    /// Bornes incluses à gauche/en haut, exclues à droite/en bas. Cette
    /// asymétrie est volontaire : deux rectangles adjacents ne se recouvrent
    /// alors jamais sur leur frontière commune, et un point n'appartient donc
    /// jamais à deux écrans à la fois.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.left() && p.x < self.right() && p.y >= self.top() && p.y < self.bottom()
    }

    /// Longueur parcourable d'une face : horizontale pour `Top`/`Bottom`,
    /// verticale pour `Left`/`Right`.
    ///
    /// C'est l'unité de l'`offset` de la décision n° 1 : un personnage
    /// accroché stocke sa distance le long de cette longueur, et non une
    /// position absolue (spec §6.2).
    pub fn face_length(&self, face: Face) -> f32 {
        match face {
            Face::Top | Face::Bottom => self.w,
            Face::Left | Face::Right => self.h,
        }
    }

    /// Le point situé à `offset` le long de `face`, en partant du coin le plus
    /// « petit » de cette face (gauche pour les horizontales, haut pour les
    /// verticales).
    ///
    /// **C'est la fonction qui rend la décision n° 1 possible.** On repart du
    /// rectangle COURANT à chaque image : si la plateforme a bougé ou changé
    /// de taille, la position suit sans une ligne de code de plus (spec §6.2).
    ///
    /// L'offset n'est volontairement pas borné ici : un offset hors bornes est
    /// une information utile — c'est le signe que la plateforme a rétréci sous
    /// le personnage, et c'est `attach.rs` qui en déduit la chute (Tâche 5).
    pub fn point_on(&self, face: Face, offset: f32) -> Point {
        match face {
            Face::Top => Point::new(self.left() + offset, self.top()),
            Face::Bottom => Point::new(self.left() + offset, self.bottom()),
            Face::Left => Point::new(self.left(), self.top() + offset),
            Face::Right => Point::new(self.right(), self.top() + offset),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_on_top_part_du_bord_gauche() {
        let r = Rect::new(100.0, 50.0, 400.0, 200.0);
        let p = r.point_on(Face::Top, 30.0);
        assert_eq!(p, Point::new(130.0, 50.0));
    }

    #[test]
    fn point_on_right_descend_depuis_le_haut() {
        let r = Rect::new(100.0, 50.0, 400.0, 200.0);
        let p = r.point_on(Face::Right, 30.0);
        assert_eq!(p, Point::new(500.0, 80.0));
    }

    #[test]
    fn point_on_bottom_est_bien_en_bas() {
        // Vérifie la convention d'axes : bottom() > top().
        let r = Rect::new(0.0, 0.0, 100.0, 10.0);
        assert_eq!(r.point_on(Face::Bottom, 0.0), Point::new(0.0, 10.0));
    }

    #[test]
    fn face_length_distingue_horizontal_et_vertical() {
        let r = Rect::new(0.0, 0.0, 400.0, 200.0);
        assert_eq!(r.face_length(Face::Top), 400.0);
        assert_eq!(r.face_length(Face::Bottom), 400.0);
        assert_eq!(r.face_length(Face::Left), 200.0);
        assert_eq!(r.face_length(Face::Right), 200.0);
    }

    #[test]
    fn deux_rects_adjacents_ne_partagent_aucun_point() {
        // La borne droite exclusive garantit qu'un point n'est jamais dans
        // deux écrans à la fois — cas réel : x = 1920 sur cette machine.
        let gauche = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let droite = Rect::new(1920.0, 0.0, 1920.0, 1080.0);
        let frontiere = Point::new(1920.0, 500.0);
        assert!(!gauche.contains(frontiere));
        assert!(droite.contains(frontiere));
    }

    #[test]
    fn contains_accepte_les_coordonnees_negatives() {
        // Contrainte globale : ne jamais supposer x >= 0. Un écran branché à
        // gauche donne un rectangle à x négatif.
        let r = Rect::new(-1920.0, 0.0, 1920.0, 1080.0);
        assert!(r.contains(Point::new(-1000.0, 500.0)));
        assert!(!r.contains(Point::new(10.0, 500.0)));
    }

    #[test]
    fn point_on_accepte_un_offset_hors_bornes() {
        // Volontaire : c'est le signal d'une plateforme qui a rétréci, et
        // attach.rs en déduira la chute. Borner ici masquerait l'information.
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(r.point_on(Face::Top, 250.0), Point::new(250.0, 0.0));
    }
}
