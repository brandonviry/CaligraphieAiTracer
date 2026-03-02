use egui::Pos2;
use serde::{Deserialize, Serialize};

/// Un point d'un tracé calligraphique avec ses données de stylet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrokePoint {
    pub pos: [f32; 2],      // on utilise [f32;2] au lieu de Pos2 pour serde
    pub pressure: f32,       // 0.0 (aucune) → 1.0 (maximale)
    pub tilt: f32,           // inclinaison en radians (0 = vertical)
}

impl StrokePoint {
    pub fn new(pos: Pos2, pressure: f32, tilt: f32) -> Self {
        Self {
            pos: [pos.x, pos.y],
            pressure: pressure.clamp(0.0, 1.0),
            tilt: tilt.clamp(0.0, std::f32::consts::FRAC_PI_2),
        }
    }

    pub fn pos2(&self) -> Pos2 {
        Pos2::new(self.pos[0], self.pos[1])
    }
}

/// Un tracé = séquence ordonnée de points.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stroke {
    pub points: Vec<StrokePoint>,
    pub color: [u8; 4], // RGBA
}

impl Stroke {
    pub fn new(color: [u8; 4]) -> Self {
        Self { points: Vec::new(), color }
    }

    pub fn push(&mut self, point: StrokePoint) {
        self.points.push(point);
    }

    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Interpolation Catmull-Rom : retourne une liste lissée de positions.
    /// `steps` = nombre de sous-points entre chaque paire de points.
    pub fn catmull_rom_positions(&self, steps: usize) -> Vec<(Pos2, f32)> {
        let pts = &self.points;
        if pts.len() < 2 {
            return pts.iter().map(|p| (p.pos2(), p.pressure)).collect();
        }

        let mut result = Vec::new();
        let n = pts.len();

        for i in 0..n.saturating_sub(1) {
            let p0 = pts[i.saturating_sub(1).min(n - 1)].pos2();
            let p1 = pts[i].pos2();
            let p2 = pts[(i + 1).min(n - 1)].pos2();
            let p3 = pts[(i + 2).min(n - 1)].pos2();

            let pr1 = pts[i].pressure;
            let pr2 = pts[(i + 1).min(n - 1)].pressure;

            for s in 0..=steps {
                let t = s as f32 / steps as f32;
                let pos = catmull_rom(p0, p1, p2, p3, t);
                let pressure = pr1 + (pr2 - pr1) * t;
                result.push((pos, pressure));
            }
        }

        result
    }
}

/// Évalue la courbe de Catmull-Rom en t ∈ [0,1] pour les 4 points de contrôle.
fn catmull_rom(p0: Pos2, p1: Pos2, p2: Pos2, p3: Pos2, t: f32) -> Pos2 {
    let t2 = t * t;
    let t3 = t2 * t;

    let x = 0.5
        * ((2.0 * p1.x)
            + (-p0.x + p2.x) * t
            + (2.0 * p0.x - 5.0 * p1.x + 4.0 * p2.x - p3.x) * t2
            + (-p0.x + 3.0 * p1.x - 3.0 * p2.x + p3.x) * t3);

    let y = 0.5
        * ((2.0 * p1.y)
            + (-p0.y + p2.y) * t
            + (2.0 * p0.y - 5.0 * p1.y + 4.0 * p2.y - p3.y) * t2
            + (-p0.y + 3.0 * p1.y - 3.0 * p2.y + p3.y) * t3);

    Pos2::new(x, y)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(x: f32, y: f32, p: f32) -> StrokePoint {
        StrokePoint::new(Pos2::new(x, y), p, 0.0)
    }

    #[test]
    fn pressure_clamped() {
        let sp = StrokePoint::new(Pos2::ZERO, 1.5, 0.0);
        assert_eq!(sp.pressure, 1.0);
        let sp2 = StrokePoint::new(Pos2::ZERO, -0.3, 0.0);
        assert_eq!(sp2.pressure, 0.0);
    }

    #[test]
    fn stroke_push_and_len() {
        let mut s = Stroke::new([0, 0, 0, 255]);
        assert!(s.is_empty());
        s.push(pt(0.0, 0.0, 0.5));
        s.push(pt(10.0, 0.0, 0.8));
        assert!(!s.is_empty());
        assert_eq!(s.points.len(), 2);
    }

    #[test]
    fn catmull_rom_single_segment() {
        let mut s = Stroke::new([0, 0, 0, 255]);
        s.push(pt(0.0, 0.0, 1.0));
        s.push(pt(100.0, 0.0, 1.0));
        let smooth = s.catmull_rom_positions(10);
        // on doit avoir au moins autant de points qu'avec les points bruts
        assert!(smooth.len() >= 2);
        // le premier point doit être proche de (0,0)
        let (first, _) = smooth[0];
        assert!((first.x - 0.0).abs() < 1.0);
    }

    #[test]
    fn catmull_rom_interpolates_pressure() {
        let mut s = Stroke::new([0, 0, 0, 255]);
        s.push(pt(0.0, 0.0, 0.0));
        s.push(pt(100.0, 0.0, 1.0));
        let smooth = s.catmull_rom_positions(10);
        // la pression doit augmenter de gauche à droite
        let pressures: Vec<f32> = smooth.iter().map(|(_, p)| *p).collect();
        for w in pressures.windows(2) {
            assert!(w[1] >= w[0] - 1e-5, "pression doit croître : {} >= {}", w[1], w[0]);
        }
    }

    #[test]
    fn serde_roundtrip() {
        let mut s = Stroke::new([255, 0, 0, 255]);
        s.push(pt(1.0, 2.0, 0.5));
        s.push(pt(3.0, 4.0, 0.9));
        let json = serde_json::to_string(&s).unwrap();
        let s2: Stroke = serde_json::from_str(&json).unwrap();
        assert_eq!(s2.points.len(), 2);
        assert!((s2.points[0].pressure - 0.5).abs() < 1e-5);
    }
}
