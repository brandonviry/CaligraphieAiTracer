/// Système de brosses bitmap.
///
/// Chaque brosse est un "stamp" : une image en niveaux de gris [0.0, 1.0]
/// qui est tamponnée le long du tracé à intervalles réguliers.
/// La taille et l'opacité du tampon varient avec la pression.

// ─── Type de brosse ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum BrushKind {
    /// Cercle anti-aliasé (généré programmatiquement)
    RoundSmooth,
    /// Rectangle incliné pour la calligraphie plate (généré programmatiquement)
    FlatCalligraphy { angle_deg: f32 },
    /// Ellipse avec grain pour brush pen (généré programmatiquement)
    BrushPen,
    /// Texture arrachée pour encre sèche (PNG embarqué)
    DryInk,
    /// PNG externe chargé depuis le disque
    Custom { pixels: Vec<f32>, width: usize, height: usize },
}

/// Une brosse prête à l'emploi avec son stamp en mémoire.
#[derive(Debug, Clone)]
pub struct Brush {
    pub kind: BrushKind,
    /// Pixels du stamp en niveaux de gris [0.0, 1.0], row-major.
    pub stamp: Vec<f32>,
    pub stamp_w: usize,
    pub stamp_h: usize,
    /// Espacement entre deux tampons (en fraction de la taille du stamp).
    /// 0.1 = très dense, 0.5 = espacé.
    pub spacing: f32,
}

impl Brush {
    /// Crée une brosse à partir de son type.
    pub fn new(kind: BrushKind) -> Self {
        let size = 64usize;
        let (stamp, stamp_w, stamp_h, spacing) = match &kind {
            BrushKind::RoundSmooth => {
                (gen_round_smooth(size), size, size, 0.15)
            }
            BrushKind::FlatCalligraphy { angle_deg } => {
                (gen_flat_calligraphy(size, *angle_deg), size, size, 0.10)
            }
            BrushKind::BrushPen => {
                (gen_brush_pen(size), size, size, 0.12)
            }
            BrushKind::DryInk => {
                (gen_dry_ink(size), size, size, 0.20)
            }
            BrushKind::Custom { pixels, width, height } => {
                (pixels.clone(), *width, *height, 0.15)
            }
        };
        Self { kind, stamp, stamp_w, stamp_h, spacing }
    }

    /// Charge une brosse depuis un PNG en niveaux de gris (bytes bruts).
    /// Les pixels RGB sont convertis en luminance.
    pub fn from_png_bytes(bytes: &[u8]) -> Result<Self, String> {
        let img = image::load_from_memory(bytes)
            .map_err(|e| format!("Erreur décodage PNG brosse : {e}"))?
            .to_luma8();
        let w = img.width() as usize;
        let h = img.height() as usize;
        let pixels: Vec<f32> = img.pixels().map(|p| p.0[0] as f32 / 255.0).collect();
        Ok(Self::new(BrushKind::Custom { pixels, width: w, height: h }))
    }

    /// Retourne le nom d'affichage de la brosse.
    pub fn label(&self) -> &'static str {
        match self.kind {
            BrushKind::RoundSmooth => "Ronde lisse",
            BrushKind::FlatCalligraphy { .. } => "Plate calligraphique",
            BrushKind::BrushPen => "Brush pen",
            BrushKind::DryInk => "Encre sèche",
            BrushKind::Custom { .. } => "Personnalisée",
        }
    }
}

/// Liste des brosses disponibles par défaut.
pub fn default_brushes() -> Vec<Brush> {
    vec![
        Brush::new(BrushKind::RoundSmooth),
        Brush::new(BrushKind::FlatCalligraphy { angle_deg: 45.0 }),
        Brush::new(BrushKind::BrushPen),
        Brush::new(BrushKind::DryInk),
    ]
}

/// Charge les brosses PNG custom listées dans la config.
/// Les brosses valides sont ajoutées après les brosses par défaut.
/// Les erreurs sont loggées mais n'interrompent pas le démarrage.
pub fn load_custom_brushes(paths: &[String]) -> Vec<Brush> {
    let mut result = Vec::new();
    for path in paths {
        match std::fs::read(path) {
            Ok(bytes) => match Brush::from_png_bytes(&bytes) {
                Ok(brush) => {
                    println!("[brushes] Chargée : {path}");
                    result.push(brush);
                }
                Err(e) => eprintln!("[brushes] Erreur PNG {path} : {e}"),
            },
            Err(e) => eprintln!("[brushes] Fichier introuvable {path} : {e}"),
        }
    }
    result
}

/// Construit la liste complète : brosses par défaut + brosses custom.
pub fn all_brushes(custom_paths: &[String]) -> Vec<Brush> {
    let mut brushes = default_brushes();
    brushes.extend(load_custom_brushes(custom_paths));
    brushes
}

// ─── Canvas pixel ─────────────────────────────────────────────────────────────

/// Canvas RGBA en mémoire sur lequel on tamponné les brosses.
pub struct Canvas {
    pub pixels: Vec<u8>, // RGBA, row-major
    pub width: usize,
    pub height: usize,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            pixels: vec![255u8; width * height * 4], // fond blanc opaque
            width,
            height,
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(255);
    }

    /// Tamponné le stamp de la brosse à la position (cx, cy).
    /// `size`     : taille cible du stamp en pixels (modifiée par la pression)
    /// `pressure` : [0.0, 1.0] — contrôle l'opacité
    /// `color`    : [R, G, B] encre
    pub fn stamp(
        &mut self,
        brush: &Brush,
        cx: f32,
        cy: f32,
        size: f32,
        pressure: f32,
        color: [u8; 3],
    ) {
        if size < 1.0 || pressure <= 0.0 {
            return;
        }

        let sw = brush.stamp_w as f32;
        let sh = brush.stamp_h as f32;
        let scale_x = size / sw;
        let scale_y = size / sh;

        let x0 = (cx - size * 0.5).floor() as i32;
        let y0 = (cy - size * 0.5).floor() as i32;
        let x1 = (cx + size * 0.5).ceil() as i32;
        let y1 = (cy + size * 0.5).ceil() as i32;

        for py in y0..=y1 {
            if py < 0 || py >= self.height as i32 { continue; }
            for px in x0..=x1 {
                if px < 0 || px >= self.width as i32 { continue; }

                // Coordonnée dans le stamp (bi-linéaire)
                let sx = (px as f32 - cx + size * 0.5) / scale_x;
                let sy = (py as f32 - cy + size * 0.5) / scale_y;

                let alpha = sample_bilinear(&brush.stamp, brush.stamp_w, brush.stamp_h, sx, sy);
                let alpha = (alpha * pressure).clamp(0.0, 1.0);

                if alpha < 0.001 { continue; }

                let idx = (py as usize * self.width + px as usize) * 4;
                // Alpha compositing sur fond blanc
                self.pixels[idx]     = blend(self.pixels[idx],     color[0], alpha);
                self.pixels[idx + 1] = blend(self.pixels[idx + 1], color[1], alpha);
                self.pixels[idx + 2] = blend(self.pixels[idx + 2], color[2], alpha);
                // Alpha canal toujours 255 (fond opaque)
            }
        }
    }
}

/// Alpha compositing linéaire : `dst * (1 - a) + src * a`
#[inline]
fn blend(dst: u8, src: u8, alpha: f32) -> u8 {
    (dst as f32 * (1.0 - alpha) + src as f32 * alpha).round() as u8
}

/// Échantillonnage bi-linéaire dans un stamp.
fn sample_bilinear(pixels: &[f32], w: usize, h: usize, x: f32, y: f32) -> f32 {
    if x < 0.0 || y < 0.0 || x >= w as f32 || y >= h as f32 {
        return 0.0;
    }
    let x0 = x.floor() as usize;
    let y0 = y.floor() as usize;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let tx = x - x0 as f32;
    let ty = y - y0 as f32;

    let p00 = pixels[y0 * w + x0];
    let p10 = pixels[y0 * w + x1];
    let p01 = pixels[y1 * w + x0];
    let p11 = pixels[y1 * w + x1];

    p00 * (1.0 - tx) * (1.0 - ty)
        + p10 * tx * (1.0 - ty)
        + p01 * (1.0 - tx) * ty
        + p11 * tx * ty
}

// ─── Générateurs de stamps ────────────────────────────────────────────────────

/// Cercle gaussien anti-aliasé.
fn gen_round_smooth(size: usize) -> Vec<f32> {
    let mut pixels = vec![0.0f32; size * size];
    let cx = (size as f32 - 1.0) * 0.5;
    let cy = cx;
    let r = cx;
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            // Profil gaussien : 1.0 au centre, 0.0 au bord
            let v = (-2.5 * (d / r).powi(2)).exp();
            pixels[y * size + x] = v.clamp(0.0, 1.0);
        }
    }
    pixels
}

/// Rectangle incliné pour brosse plate calligraphique.
fn gen_flat_calligraphy(size: usize, angle_deg: f32) -> Vec<f32> {
    let mut pixels = vec![0.0f32; size * size];
    let cx = (size as f32 - 1.0) * 0.5;
    let cy = cx;
    let angle = angle_deg.to_radians();
    let cos_a = angle.cos();
    let sin_a = angle.sin();
    // Demi-axes du rectangle orienté
    let half_len = size as f32 * 0.45; // axe long
    let half_wid = size as f32 * 0.12; // axe court

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            // Rotation inverse
            let u = dx * cos_a + dy * sin_a;
            let v = -dx * sin_a + dy * cos_a;
            // Distances normalisées
            let du = (u.abs() / half_len).clamp(0.0, 1.0);
            let dv = (v.abs() / half_wid).clamp(0.0, 1.0);
            // Bords anti-aliasés
            let alpha = ((1.0 - du) * (1.0 - dv)).powf(0.5);
            pixels[y * size + x] = alpha.clamp(0.0, 1.0);
        }
    }
    pixels
}

/// Ellipse avec grain pour brush pen.
fn gen_brush_pen(size: usize) -> Vec<f32> {
    let mut pixels = vec![0.0f32; size * size];
    let cx = (size as f32 - 1.0) * 0.5;
    let cy = cx;
    let rx = size as f32 * 0.20; // axe court
    let ry = size as f32 * 0.45; // axe long
    // Graine fixe pour la reproductibilité du grain
    let mut seed = 42u64;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let d = (dx * dx / (rx * rx) + dy * dy / (ry * ry)).sqrt();
            let base = (-2.0 * d * d).exp();
            // Bruit pseudo-aléatoire déterministe (LCG simple)
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let noise = (seed >> 33) as f32 / u32::MAX as f32;
            let grain = if d < 1.0 { noise * 0.3 } else { 0.0 };
            pixels[y * size + x] = (base + grain).clamp(0.0, 1.0);
        }
    }
    pixels
}

/// Texture arrachée pour encre sèche.
fn gen_dry_ink(size: usize) -> Vec<f32> {
    let mut pixels = vec![0.0f32; size * size];
    let cx = (size as f32 - 1.0) * 0.5;
    let cy = cx;
    let r = cx * 0.9;
    let mut seed = 137u64;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            // Bruit pour déformer le bord
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let n1 = (seed >> 33) as f32 / u32::MAX as f32;
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let n2 = (seed >> 33) as f32 / u32::MAX as f32;
            let ragged = r + (n1 - 0.5) * r * 0.4;
            if d < ragged {
                // Intérieur : opacité aléatoire (effet encre sèche)
                let fill = 0.5 + n2 * 0.5;
                let falloff = (1.0 - (d / ragged).powi(2)).sqrt();
                pixels[y * size + x] = (fill * falloff).clamp(0.0, 1.0);
            }
        }
    }
    pixels
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_stamp_center_is_max() {
        let brush = Brush::new(BrushKind::RoundSmooth);
        let cx = brush.stamp_w / 2;
        let cy = brush.stamp_h / 2;
        let center = brush.stamp[cy * brush.stamp_w + cx];
        // Le centre doit être proche de 1.0
        assert!(center > 0.9, "centre = {center}");
    }

    #[test]
    fn round_stamp_edge_is_low() {
        let brush = Brush::new(BrushKind::RoundSmooth);
        let edge = brush.stamp[0]; // coin supérieur gauche
        assert!(edge < 0.1, "bord = {edge}");
    }

    #[test]
    fn flat_stamp_has_pixels() {
        let brush = Brush::new(BrushKind::FlatCalligraphy { angle_deg: 45.0 });
        let sum: f32 = brush.stamp.iter().sum();
        assert!(sum > 0.0);
    }

    #[test]
    fn all_stamps_correct_size() {
        for brush in default_brushes() {
            assert_eq!(brush.stamp.len(), brush.stamp_w * brush.stamp_h);
            assert!(brush.stamp.iter().all(|&v| v >= 0.0 && v <= 1.0),
                "Pixels hors [0,1] pour {}", brush.label());
        }
    }

    #[test]
    fn canvas_stamp_changes_pixels() {
        let mut canvas = Canvas::new(200, 200);
        let brush = Brush::new(BrushKind::RoundSmooth);
        // Canvas blanc au départ
        assert_eq!(canvas.pixels[0], 255);
        // On tamponné au centre
        canvas.stamp(&brush, 100.0, 100.0, 40.0, 1.0, [0, 0, 0]);
        // Le centre doit être plus sombre
        let idx = (100 * 200 + 100) * 4;
        assert!(canvas.pixels[idx] < 200, "pixel centre = {}", canvas.pixels[idx]);
    }

    #[test]
    fn canvas_clear_resets_to_white() {
        let mut canvas = Canvas::new(100, 100);
        let brush = Brush::new(BrushKind::RoundSmooth);
        canvas.stamp(&brush, 50.0, 50.0, 30.0, 1.0, [0, 0, 0]);
        canvas.clear();
        assert!(canvas.pixels.iter().all(|&p| p == 255));
    }
}
