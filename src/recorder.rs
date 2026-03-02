use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::stroke::Stroke;

// ─── Format du fichier d'export ───────────────────────────────────────────────

const CURRENT_VERSION: u32 = 1;

/// Enveloppe JSON pour un tracé exporté.
#[derive(Debug, Serialize, Deserialize)]
pub struct RecordFile {
    /// Version du format (pour compatibilité future).
    pub version: u32,
    /// Tous les traits du tracé.
    pub strokes: Vec<Stroke>,
}

// ─── Export ───────────────────────────────────────────────────────────────────

/// Sérialise `strokes` dans un fichier JSON.
/// Retourne le chemin écrit (identique à `path`) ou un message d'erreur.
pub fn export(strokes: &[Stroke], path: &Path) -> Result<(), String> {
    let record = RecordFile {
        version: CURRENT_VERSION,
        strokes: strokes.to_vec(),
    };
    let json = serde_json::to_string_pretty(&record)
        .map_err(|e| format!("Sérialisation échouée : {e}"))?;
    std::fs::write(path, json)
        .map_err(|e| format!("Écriture échouée : {e}"))?;
    Ok(())
}

// ─── Import ───────────────────────────────────────────────────────────────────

/// Désérialise un fichier JSON en `Vec<Stroke>`.
pub fn import(path: &Path) -> Result<Vec<Stroke>, String> {
    let json = std::fs::read_to_string(path)
        .map_err(|e| format!("Lecture échouée : {e}"))?;
    let record: RecordFile = serde_json::from_str(&json)
        .map_err(|e| format!("Format invalide : {e}"))?;
    if record.version != CURRENT_VERSION {
        return Err(format!(
            "Version non supportée : {} (attendu {})",
            record.version, CURRENT_VERSION
        ));
    }
    Ok(record.strokes)
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stroke::StrokePoint;
    use egui::Pos2;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn make_stroke() -> Stroke {
        let mut s = Stroke::new([10, 20, 30, 255]);
        s.push(StrokePoint::new(Pos2::new(0.0, 0.0), 0.5, 0.0));
        s.push(StrokePoint::new(Pos2::new(100.0, 50.0), 0.9, 0.1));
        s
    }

    #[test]
    fn export_creates_valid_json() {
        let strokes = vec![make_stroke()];
        let tmp = NamedTempFile::new().unwrap();
        export(&strokes, tmp.path()).unwrap();
        let content = std::fs::read_to_string(tmp.path()).unwrap();
        assert!(content.contains("\"version\""));
        assert!(content.contains("\"strokes\""));
    }

    #[test]
    fn roundtrip_preserves_strokes() {
        let strokes = vec![make_stroke(), make_stroke()];
        let tmp = NamedTempFile::new().unwrap();
        export(&strokes, tmp.path()).unwrap();
        let loaded = import(tmp.path()).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].points.len(), 2);
        assert!((loaded[0].points[0].pressure - 0.5).abs() < 1e-5);
        assert_eq!(loaded[0].color, [10, 20, 30, 255]);
    }

    #[test]
    fn import_wrong_version_returns_error() {
        let json = r#"{"version": 99, "strokes": []}"#;
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(json.as_bytes()).unwrap();
        let result = import(tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Version non supportée"));
    }

    #[test]
    fn import_invalid_json_returns_error() {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(b"not json").unwrap();
        assert!(import(tmp.path()).is_err());
    }

    #[test]
    fn export_empty_strokes() {
        let tmp = NamedTempFile::new().unwrap();
        export(&[], tmp.path()).unwrap();
        let loaded = import(tmp.path()).unwrap();
        assert!(loaded.is_empty());
    }
}
