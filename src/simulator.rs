use egui::Pos2;
use crate::brush::{Brush, Canvas};
use crate::stroke::{Stroke, StrokePoint};

/// Configuration de la simulation calligraphique.
pub struct SimConfig {
    /// Épaisseur de base en pixels (avant modulation par la pression).
    pub base_thickness: f32,
    /// Couleur de l'encre [R, G, B].
    pub ink_color: [u8; 3],
    /// Opacité globale [0.0, 1.0].
    pub opacity: f32,
}

impl Default for SimConfig {
    fn default() -> Self {
        Self {
            base_thickness: 24.0,
            ink_color: [10, 10, 30],
            opacity: 1.0,
        }
    }
}

/// Profil de pression gaussien pour un paramètre t ∈ [0.0, 1.0].
/// 0 au départ → 1 au milieu → 0 à la fin.
pub fn gaussian_pressure(t: f32) -> f32 {
    let centered = t - 0.5;
    // Coefficient 16 → valeur en t=0 ≈ 0.018 (assez faible)
    (-16.0 * centered * centered).exp()
}

/// Longueur totale d'une polyligne (somme des distances entre points consécutifs).
fn polyline_length(points: &[Pos2]) -> f32 {
    points.windows(2).map(|w| (w[1] - w[0]).length()).sum()
}

/// Convertit un trait brut (Vec<Pos2>) en Stroke avec pression simulée.
/// La pression suit un profil gaussien le long de la longueur du trait.
pub fn simulate_stroke(points: &[Pos2], config: &SimConfig) -> Stroke {
    if points.is_empty() {
        return Stroke::new([
            config.ink_color[0],
            config.ink_color[1],
            config.ink_color[2],
            (config.opacity * 255.0) as u8,
        ]);
    }

    let total_len = polyline_length(points);
    let mut stroke = Stroke::new([
        config.ink_color[0],
        config.ink_color[1],
        config.ink_color[2],
        (config.opacity * 255.0) as u8,
    ]);

    let mut accumulated = 0.0f32;

    for (i, &pos) in points.iter().enumerate() {
        // Paramètre t basé sur la longueur accumulée (et non l'index)
        let t = if total_len > 0.0 {
            (accumulated / total_len).clamp(0.0, 1.0)
        } else {
            i as f32 / (points.len().max(2) - 1) as f32
        };

        let pressure = gaussian_pressure(t);

        // Inclinaison simulée : direction du segment courant
        let tilt = if i + 1 < points.len() {
            let dir = points[i + 1] - pos;
            dir.y.atan2(dir.x).abs()
        } else if i > 0 {
            let dir = pos - points[i - 1];
            dir.y.atan2(dir.x).abs()
        } else {
            0.0
        };

        stroke.push(StrokePoint::new(pos, pressure, tilt));

        if i + 1 < points.len() {
            accumulated += (points[i + 1] - pos).length();
        }
    }

    stroke
}

/// Simule l'ensemble des traits d'un glyphe → liste de Strokes.
pub fn simulate_glyph(outline_strokes: &[Vec<Pos2>], config: &SimConfig) -> Vec<Stroke> {
    outline_strokes
        .iter()
        .filter(|s| s.len() >= 2)
        .map(|s| simulate_stroke(s, config))
        .collect()
}

/// Tamponné un Stroke sur le canvas avec la brosse donnée.
/// Appelé point par point pour l'animation progressive.
pub fn paint_stroke_on_canvas(
    stroke: &Stroke,
    brush: &Brush,
    canvas: &mut Canvas,
    config: &SimConfig,
    up_to_index: usize, // combien de points peindre (pour l'animation)
) {
    let pts = &stroke.points;
    if pts.len() < 2 { return; }

    let limit = up_to_index.min(pts.len());
    if limit < 1 { return; }

    let mut dist_since_last_stamp = 0.0f32;

    for i in 0..limit.saturating_sub(1) {
        let a = pts[i].pos2();
        let b = pts[i + 1].pos2();
        let segment_len = (b - a).length();
        if segment_len < 0.001 { continue; }

        // Espacement entre deux tampons en pixels
        let pressure_avg = (pts[i].pressure + pts[i + 1].pressure) * 0.5;
        let stamp_size = config.base_thickness * (0.2 + 0.8 * pressure_avg) * config.opacity;
        let step = (stamp_size * brush.spacing).max(1.0);

        let dir = (b - a) / segment_len;
        let mut t = (step - dist_since_last_stamp).max(0.0);

        while t <= segment_len {
            let pos = a + dir * t;
            canvas.stamp(
                brush,
                pos.x,
                pos.y,
                stamp_size,
                pressure_avg * config.opacity,
                config.ink_color,
            );
            t += step;
        }

        dist_since_last_stamp = (segment_len - (t - step)).rem_euclid(step);
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gaussian_pressure_peak_at_center() {
        let center = gaussian_pressure(0.5);
        let start = gaussian_pressure(0.0);
        let end = gaussian_pressure(1.0);
        assert!(center > 0.99, "pic au centre : {center}");
        assert!(start < 0.05, "départ faible : {start}");
        assert!(end < 0.05, "fin faible : {end}");
    }

    #[test]
    fn gaussian_pressure_symmetric() {
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            let p1 = gaussian_pressure(t);
            let p2 = gaussian_pressure(1.0 - t);
            assert!((p1 - p2).abs() < 1e-5, "non symétrique en t={t}");
        }
    }

    #[test]
    fn simulate_stroke_pressure_in_range() {
        let pts: Vec<Pos2> = (0..20)
            .map(|i| Pos2::new(i as f32 * 10.0, 0.0))
            .collect();
        let stroke = simulate_stroke(&pts, &SimConfig::default());
        for sp in &stroke.points {
            assert!(sp.pressure >= 0.0 && sp.pressure <= 1.0,
                "pression hors [0,1] : {}", sp.pressure);
        }
    }

    #[test]
    fn simulate_stroke_same_count() {
        let pts: Vec<Pos2> = (0..15)
            .map(|i| Pos2::new(i as f32 * 5.0, 0.0))
            .collect();
        let stroke = simulate_stroke(&pts, &SimConfig::default());
        assert_eq!(stroke.points.len(), pts.len());
    }

    #[test]
    fn simulate_glyph_filters_short_strokes() {
        let outlines = vec![
            vec![Pos2::new(0.0, 0.0)],        // trop court → ignoré
            vec![Pos2::new(0.0, 0.0), Pos2::new(10.0, 0.0)], // OK
        ];
        let strokes = simulate_glyph(&outlines, &SimConfig::default());
        assert_eq!(strokes.len(), 1);
    }

    #[test]
    fn simulate_glyph_count_matches() {
        let outlines: Vec<Vec<Pos2>> = (0..4)
            .map(|i| vec![
                Pos2::new(i as f32 * 50.0, 0.0),
                Pos2::new(i as f32 * 50.0 + 20.0, 30.0),
            ])
            .collect();
        let strokes = simulate_glyph(&outlines, &SimConfig::default());
        assert_eq!(strokes.len(), 4);
    }

    #[test]
    fn paint_stroke_modifies_canvas() {
        use crate::brush::{Brush, BrushKind};
        let pts: Vec<Pos2> = (0..10)
            .map(|i| Pos2::new(50.0 + i as f32 * 5.0, 100.0))
            .collect();
        let stroke = simulate_stroke(&pts, &SimConfig::default());
        let brush = Brush::new(BrushKind::RoundSmooth);
        let mut canvas = Canvas::new(300, 300);
        paint_stroke_on_canvas(&stroke, &brush, &mut canvas, &SimConfig::default(), stroke.points.len());
        // Au moins un pixel doit avoir changé
        let changed = canvas.pixels.chunks(4).any(|p| p[0] < 255);
        assert!(changed, "le canvas n'a pas été modifié");
    }
}
