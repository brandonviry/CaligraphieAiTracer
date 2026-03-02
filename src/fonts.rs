use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Dossier local de cache des polices téléchargées (depuis la config).
pub fn fonts_cache_dir(cache_dir: &str) -> PathBuf {
    PathBuf::from(cache_dir)
}

/// Retourne le chemin local d'une police.
/// Si le fichier n'existe pas encore, le télécharge depuis Google Fonts.
pub fn ensure_font(name: &str, cache_dir: &str) -> Result<PathBuf, String> {
    let dir = fonts_cache_dir(cache_dir);
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Impossible de créer le dossier de cache : {e}"))?;

    let filename = font_filename(name);
    let path = dir.join(&filename);

    if path.exists() {
        return Ok(path);
    }

    download_font(name, &path)?;
    Ok(path)
}

/// Charge les bytes d'un fichier TTF depuis le disque.
pub fn load_font_bytes(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("Impossible de lire la police : {e}"))
}

// ─── Internals ────────────────────────────────────────────────────────────────

fn font_filename(name: &str) -> String {
    let base: String = name.split_whitespace().collect();
    format!("{base}-Regular.ttf")
}

fn download_font(name: &str, dest: &Path) -> Result<(), String> {
    let css_url = build_css_url(name);

    let response = ureq::get(&css_url)
        .set("User-Agent", "Mozilla/5.0")
        .call()
        .map_err(|e| format!("Erreur HTTP lors de la récupération CSS : {e}"))?;

    let css = response
        .into_string()
        .map_err(|e| format!("Erreur de lecture CSS : {e}"))?;

    let ttf_url = parse_font_url(&css)
        .ok_or_else(|| format!("URL du fichier de police introuvable pour «{name}»"))?;

    let font_response = ureq::get(&ttf_url)
        .call()
        .map_err(|e| format!("Erreur HTTP lors du téléchargement de la police : {e}"))?;

    let mut bytes: Vec<u8> = Vec::new();
    font_response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|e| format!("Erreur de lecture des bytes de la police : {e}"))?;

    fs::write(dest, &bytes)
        .map_err(|e| format!("Impossible d'écrire la police sur le disque : {e}"))?;

    println!("[fonts] Téléchargé : {name} → {}", dest.display());
    Ok(())
}

fn build_css_url(name: &str) -> String {
    let encoded = name.replace(' ', "+");
    format!("https://fonts.googleapis.com/css?family={encoded}&subset=latin")
}

fn parse_font_url(css: &str) -> Option<String> {
    for line in css.lines() {
        let line = line.trim();
        if line.starts_with("src:") || line.contains("url(") {
            if let Some(start) = line.find("url(") {
                let rest = &line[start + 4..];
                if let Some(end) = rest.find(')') {
                    let url = rest[..end].trim_matches('\'').trim_matches('"');
                    if url.starts_with("https://fonts.gstatic.com") {
                        return Some(url.to_string());
                    }
                }
            }
        }
    }
    None
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_filename_no_spaces() {
        assert_eq!(font_filename("Dancing Script"), "DancingScript-Regular.ttf");
        assert_eq!(font_filename("Pacifico"), "Pacifico-Regular.ttf");
        assert_eq!(font_filename("Great Vibes"), "GreatVibes-Regular.ttf");
    }

    #[test]
    fn css_url_encodes_spaces() {
        let url = build_css_url("Dancing Script");
        assert!(url.contains("Dancing+Script"));
        assert!(url.starts_with("https://fonts.googleapis.com"));
    }

    #[test]
    fn parse_font_url_extracts_gstatic() {
        let fake_css = r#"
@font-face {
  font-family: 'Dancing Script';
  src: url(https://fonts.gstatic.com/s/dancingscript/v1/test.ttf) format('truetype');
}
"#;
        let url = parse_font_url(fake_css);
        assert!(url.is_some());
        assert!(url.unwrap().contains("fonts.gstatic.com"));
    }

    #[test]
    fn parse_font_url_returns_none_on_empty() {
        assert!(parse_font_url("").is_none());
        assert!(parse_font_url("body { color: red; }").is_none());
    }
}
