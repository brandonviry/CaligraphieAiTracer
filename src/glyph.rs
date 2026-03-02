use egui::Pos2;
use ttf_parser::{Face, OutlineBuilder};

/// Résolution d'échantillonnage des courbes de Bézier (points par courbe).
const BEZIER_STEPS: usize = 16;

/// Contour d'un glyphe : une liste de "traits" (sous-chemins).
/// Chaque trait est une séquence ordonnée de positions 2D.
#[derive(Debug, Clone)]
pub struct GlyphOutline {
    /// Traits du glyphe (un trait = un sous-chemin du contour vectoriel).
    pub strokes: Vec<Vec<Pos2>>,
    /// Largeur d'avance (pour positionner la lettre suivante).
    pub advance_width: f32,
}

/// Extrait le contour d'un seul caractère depuis une face TTF.
/// `scale` : hauteur cible en pixels (ex. 200.0).
pub fn extract_glyph(face: &Face, ch: char, scale: f32) -> Option<GlyphOutline> {
    let glyph_id = face.glyph_index(ch)?;

    let units_per_em = face.units_per_em() as f32;
    let factor = scale / units_per_em;

    let advance_width = face
        .glyph_hor_advance(glyph_id)
        .map(|a| a as f32 * factor)
        .unwrap_or(scale * 0.6);

    let mut builder = ContourBuilder::new(factor);
    face.outline_glyph(glyph_id, &mut builder)?;
    builder.flush_current();

    Some(GlyphOutline {
        strokes: builder.strokes,
        advance_width,
    })
}

/// Convertit une chaîne de caractères en liste de contours positionnés.
/// Les glyphes sont placés côte à côte selon leur avance horizontale.
/// Retourne `(outlines, total_width)`.
pub fn text_to_outlines(
    face: &Face,
    text: &str,
    scale: f32,
) -> (Vec<GlyphOutline>, f32) {
    let mut result = Vec::new();
    let mut cursor_x = 0.0f32;

    for ch in text.chars() {
        if ch == ' ' {
            cursor_x += scale * 0.3;
            continue;
        }

        if let Some(mut outline) = extract_glyph(face, ch, scale) {
            // Décaler horizontalement selon la position du curseur
            for stroke in &mut outline.strokes {
                for pt in stroke.iter_mut() {
                    pt.x += cursor_x;
                }
            }
            cursor_x += outline.advance_width;
            result.push(outline);
        }
    }

    (result, cursor_x)
}

// ─── Builder interne ──────────────────────────────────────────────────────────

struct ContourBuilder {
    factor: f32,
    strokes: Vec<Vec<Pos2>>,
    current: Vec<Pos2>,
    last: Pos2,
}

impl ContourBuilder {
    fn new(factor: f32) -> Self {
        Self {
            factor,
            strokes: Vec::new(),
            current: Vec::new(),
            last: Pos2::ZERO,
        }
    }

    /// Sauvegarde le trait courant s'il a au moins 2 points.
    fn flush_current(&mut self) {
        if self.current.len() >= 2 {
            self.strokes.push(std::mem::take(&mut self.current));
        } else {
            self.current.clear();
        }
    }

    fn scale(&self, x: f32, y: f32) -> Pos2 {
        // TTF : y croît vers le haut → on inverse pour l'écran
        Pos2::new(x * self.factor, -y * self.factor)
    }

    /// Échantillonne une courbe de Bézier quadratique.
    fn sample_quad(&mut self, p0: Pos2, p1: Pos2, p2: Pos2) {
        for i in 1..=BEZIER_STEPS {
            let t = i as f32 / BEZIER_STEPS as f32;
            let mt = 1.0 - t;
            let x = mt * mt * p0.x + 2.0 * mt * t * p1.x + t * t * p2.x;
            let y = mt * mt * p0.y + 2.0 * mt * t * p1.y + t * t * p2.y;
            self.current.push(Pos2::new(x, y));
        }
    }

    /// Échantillonne une courbe de Bézier cubique.
    fn sample_cubic(&mut self, p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2) {
        for i in 1..=BEZIER_STEPS {
            let t = i as f32 / BEZIER_STEPS as f32;
            let mt = 1.0 - t;
            let x = mt * mt * mt * p0.x
                + 3.0 * mt * mt * t * p1.x
                + 3.0 * mt * t * t * p2.x
                + t * t * t * p3.x;
            let y = mt * mt * mt * p0.y
                + 3.0 * mt * mt * t * p1.y
                + 3.0 * mt * t * t * p2.y
                + t * t * t * p3.y;
            self.current.push(Pos2::new(x, y));
        }
    }
}

impl OutlineBuilder for ContourBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.flush_current();
        let pt = self.scale(x, y);
        self.last = pt;
        self.current.push(pt);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let pt = self.scale(x, y);
        self.current.push(pt);
        self.last = pt;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let p0 = self.last;
        let p1 = self.scale(x1, y1);
        let p2 = self.scale(x, y);
        self.sample_quad(p0, p1, p2);
        self.last = p2;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let p0 = self.last;
        let p1 = self.scale(x1, y1);
        let p2 = self.scale(x2, y2);
        let p3 = self.scale(x, y);
        self.sample_cubic(p0, p1, p2, p3);
        self.last = p3;
    }

    fn close(&mut self) {
        // Fermer le sous-chemin : rejoindre le premier point si nécessaire
        if let Some(&first) = self.current.first() {
            if self.last != first {
                self.current.push(first);
            }
        }
        self.flush_current();
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Charge une police de test embarquée (Noto Sans, embarquée en bytes).
    /// On utilise une police système Windows comme fallback pour les tests.
    fn load_test_face() -> Vec<u8> {
        // Cherche une police système courante pour les tests
        for path in &[
            "C:/Windows/Fonts/arial.ttf",
            "C:/Windows/Fonts/calibri.ttf",
            "C:/Windows/Fonts/times.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/System/Library/Fonts/Helvetica.ttc",
        ] {
            if let Ok(bytes) = std::fs::read(path) {
                return bytes;
            }
        }
        panic!("Aucune police de test trouvée sur ce système");
    }

    #[test]
    fn extract_letter_a_has_strokes() {
        let bytes = load_test_face();
        let face = Face::parse(&bytes, 0).expect("Police invalide");
        let outline = extract_glyph(&face, 'A', 200.0);
        assert!(outline.is_some(), "Le glyphe 'A' doit exister");
        let outline = outline.unwrap();
        assert!(!outline.strokes.is_empty(), "'A' doit avoir au moins un trait");
        assert!(
            outline.strokes.iter().all(|s| s.len() >= 2),
            "Chaque trait doit avoir au moins 2 points"
        );
    }

    #[test]
    fn advance_width_positive() {
        let bytes = load_test_face();
        let face = Face::parse(&bytes, 0).expect("Police invalide");
        let outline = extract_glyph(&face, 'A', 200.0).unwrap();
        assert!(outline.advance_width > 0.0, "L'avance doit être positive");
    }

    #[test]
    fn text_to_outlines_two_chars() {
        let bytes = load_test_face();
        let face = Face::parse(&bytes, 0).expect("Police invalide");
        let (outlines, total_width) = text_to_outlines(&face, "AB", 200.0);
        assert_eq!(outlines.len(), 2, "Deux caractères → deux glyphes");
        assert!(total_width > 0.0, "Largeur totale positive");
    }

    #[test]
    fn positions_in_reasonable_range() {
        let bytes = load_test_face();
        let face = Face::parse(&bytes, 0).expect("Police invalide");
        let outline = extract_glyph(&face, 'O', 200.0).unwrap();
        for stroke in &outline.strokes {
            for pt in stroke {
                assert!(
                    pt.x.abs() < 2000.0 && pt.y.abs() < 2000.0,
                    "Point hors limites : {pt:?}"
                );
            }
        }
    }

    #[test]
    fn space_char_skipped() {
        let bytes = load_test_face();
        let face = Face::parse(&bytes, 0).expect("Police invalide");
        let (outlines, _) = text_to_outlines(&face, "A B", 200.0);
        // L'espace ne génère pas de glyphe
        assert_eq!(outlines.len(), 2);
    }
}
