use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ─── Structs de configuration ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasConfig {
    pub width: usize,
    pub height: usize,
    pub glyph_scale: f32,
}

impl Default for CanvasConfig {
    fn default() -> Self {
        Self { width: 900, height: 600, glyph_scale: 220.0 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub enabled: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { port: 7777, enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsConfig {
    pub font: String,
    pub brush: String,
    pub color: [u8; 3],
    pub thickness: f32,
    pub speed: f32,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            font: "Dancing Script".into(),
            brush: "brush_pen".into(),
            color: [10, 10, 30],
            thickness: 18.0,
            speed: 4.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontsConfig {
    /// Polices Google Fonts disponibles dans le sélecteur.
    /// Ajouter n'importe quel nom de police Google Fonts : elle sera
    /// téléchargée automatiquement au premier usage.
    pub list: Vec<String>,
    /// Dossier de cache des fichiers TTF (relatif au .exe).
    pub cache_dir: String,
}

impl Default for FontsConfig {
    fn default() -> Self {
        Self {
            list: vec![
                "Dancing Script".into(),
                "Great Vibes".into(),
                "Pacifico".into(),
                "Sacramento".into(),
                "Caveat".into(),
                "Pinyon Script".into(),
                "Alex Brush".into(),
                "Allura".into(),
            ],
            cache_dir: "assets/fonts".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrushesConfig {
    /// Chemins vers des PNGs de brosses personnalisées (relatifs au .exe).
    /// Format : PNG 64×64, niveaux de gris, blanc = encre pleine.
    pub custom: Vec<String>,
}

impl Default for BrushesConfig {
    fn default() -> Self {
        Self { custom: Vec::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub canvas: CanvasConfig,
    pub server: ServerConfig,
    pub defaults: DefaultsConfig,
    pub fonts: FontsConfig,
    pub brushes: BrushesConfig,
}

// ─── Chargement / Sauvegarde ──────────────────────────────────────────────────

impl AppConfig {
    /// Chemin du fichier config à côté du .exe.
    pub fn config_path() -> PathBuf {
        // En développement (cargo run) → dossier courant
        // En production (.exe) → même dossier que l'exécutable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let path = dir.join("config.toml");
                // Ne pas utiliser le dossier target/ en dev
                if !dir.to_string_lossy().contains("target") {
                    return path;
                }
            }
        }
        PathBuf::from("config.toml")
    }

    /// Charge la config depuis `config.toml`.
    /// Si le fichier est absent, crée un config.toml par défaut et retourne la config par défaut.
    pub fn load() -> Self {
        let path = Self::config_path();

        if !path.exists() {
            let default = AppConfig::default();
            if let Err(e) = default.save() {
                eprintln!("[config] Impossible d'écrire config.toml : {e}");
            } else {
                println!("[config] Fichier config.toml créé : {}", path.display());
            }
            return default;
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => match toml::from_str(&content) {
                Ok(cfg) => {
                    println!("[config] Chargé depuis {}", path.display());
                    cfg
                }
                Err(e) => {
                    eprintln!("[config] Erreur de parsing config.toml : {e} — valeurs par défaut utilisées");
                    AppConfig::default()
                }
            },
            Err(e) => {
                eprintln!("[config] Impossible de lire config.toml : {e}");
                AppConfig::default()
            }
        }
    }

    /// Sauvegarde la config dans `config.toml`.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        let content = toml::to_string_pretty(self)
            .map_err(|e| format!("Erreur sérialisation config : {e}"))?;
        std::fs::write(&path, content)
            .map_err(|e| format!("Impossible d'écrire {} : {e}", path.display()))
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_fonts() {
        let cfg = AppConfig::default();
        assert!(!cfg.fonts.list.is_empty());
        assert!(cfg.fonts.list.contains(&"Dancing Script".to_string()));
    }

    #[test]
    fn default_canvas_size() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.canvas.width, 900);
        assert_eq!(cfg.canvas.height, 600);
        assert!(cfg.canvas.glyph_scale > 0.0);
    }

    #[test]
    fn serialize_deserialize_roundtrip() {
        let original = AppConfig::default();
        let toml_str = toml::to_string_pretty(&original).unwrap();
        let parsed: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.fonts.list.len(), original.fonts.list.len());
        assert_eq!(parsed.canvas.width, original.canvas.width);
        assert_eq!(parsed.server.port, original.server.port);
    }

    #[test]
    fn custom_font_list_survives_roundtrip() {
        let mut cfg = AppConfig::default();
        cfg.fonts.list.push("Lobster".into());
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let parsed: AppConfig = toml::from_str(&toml_str).unwrap();
        assert!(parsed.fonts.list.contains(&"Lobster".to_string()));
    }

    #[test]
    fn custom_brush_paths_survives_roundtrip() {
        let mut cfg = AppConfig::default();
        cfg.brushes.custom.push("assets/brushes/ma_brosse.png".into());
        let toml_str = toml::to_string_pretty(&cfg).unwrap();
        let parsed: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.brushes.custom.len(), 1);
    }
}
