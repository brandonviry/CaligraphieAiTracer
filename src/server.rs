use serde::Deserialize;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::thread;

// ─── Type partagé entre les threads ──────────────────────────────────────────

pub type JobQueue = Arc<Mutex<VecDeque<DrawJob>>>;

// ─── Structure d'un job de dessin ─────────────────────────────────────────────

/// Message JSON envoyé par le script externe.
/// Tous les champs sauf `text` sont optionnels.
///
/// Exemple minimal :
/// ```json
/// {"text": "Bonjour"}
/// ```
///
/// Exemple complet :
/// ```json
/// {
///   "text": "Bonjour",
///   "font": "Dancing Script",
///   "brush": "brush_pen",
///   "color": [10, 10, 30],
///   "thickness": 18.0,
///   "speed": 8.0,
///   "clear_before": true
/// }
/// ```
#[derive(Debug, Clone, Deserialize, serde::Serialize)]
pub struct DrawJob {
    /// Texte à tracer (obligatoire).
    pub text: String,
    /// Nom de la police Google Fonts (ex. "Dancing Script").
    pub font: Option<String>,
    /// Identifiant de la brosse : "round_smooth", "flat_calligraphy",
    /// "brush_pen", "dry_ink", ou nom d'une brosse custom.
    pub brush: Option<String>,
    /// Couleur de l'encre [R, G, B] entre 0 et 255.
    pub color: Option<[u8; 3]>,
    /// Épaisseur de base en pixels.
    pub thickness: Option<f32>,
    /// Vitesse d'animation (points par frame).
    pub speed: Option<f32>,
    /// Si true, efface le canvas avant de tracer.
    pub clear_before: Option<bool>,
}

// ─── Démarrage du serveur TCP ─────────────────────────────────────────────────

/// Lance le serveur TCP dans un thread dédié.
/// Chaque message JSON reçu est parsé en DrawJob et poussé dans la queue.
/// Le thread tourne indéfiniment jusqu'à la fermeture de l'app.
pub fn start(queue: JobQueue, port: u16) {
    let addr = format!("127.0.0.1:{port}");

    thread::spawn(move || {
        let listener = match TcpListener::bind(&addr) {
            Ok(l) => {
                println!("[server] Écoute sur {addr}");
                l
            }
            Err(e) => {
                eprintln!("[server] Impossible de démarrer le serveur TCP sur {addr} : {e}");
                return;
            }
        };

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let queue = queue.clone();
                    thread::spawn(move || handle_connection(stream, queue));
                }
                Err(e) => {
                    eprintln!("[server] Erreur de connexion : {e}");
                }
            }
        }
    });
}

/// Lit les lignes JSON d'une connexion et les pousse dans la queue.
/// Une connexion peut envoyer plusieurs jobs (une ligne JSON par job).
fn handle_connection(stream: std::net::TcpStream, queue: JobQueue) {
    let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_default();
    let reader = BufReader::new(stream);

    for line in reader.lines() {
        match line {
            Ok(json) => {
                let json = json.trim().to_string();
                if json.is_empty() {
                    continue;
                }
                match serde_json::from_str::<DrawJob>(&json) {
                    Ok(job) => {
                        println!("[server] Job reçu de {peer} : «{}»", job.text);
                        if let Ok(mut q) = queue.lock() {
                            q.push_back(job);
                        }
                    }
                    Err(e) => {
                        eprintln!("[server] JSON invalide de {peer} : {e}\n  → {json}");
                    }
                }
            }
            Err(e) => {
                eprintln!("[server] Erreur de lecture depuis {peer} : {e}");
                break;
            }
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Result<DrawJob, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn minimal_job_only_text() {
        let job = parse(r#"{"text": "Bonjour"}"#).unwrap();
        assert_eq!(job.text, "Bonjour");
        assert!(job.font.is_none());
        assert!(job.color.is_none());
        assert!(job.clear_before.is_none());
    }

    #[test]
    fn full_job_all_fields() {
        let json = r#"{
            "text": "Hello",
            "font": "Pacifico",
            "brush": "brush_pen",
            "color": [255, 0, 0],
            "thickness": 24.0,
            "speed": 10.0,
            "clear_before": true
        }"#;
        let job = parse(json).unwrap();
        assert_eq!(job.text, "Hello");
        assert_eq!(job.font.as_deref(), Some("Pacifico"));
        assert_eq!(job.color, Some([255, 0, 0]));
        assert_eq!(job.thickness, Some(24.0));
        assert_eq!(job.speed, Some(10.0));
        assert_eq!(job.clear_before, Some(true));
    }

    #[test]
    fn invalid_json_returns_error() {
        assert!(parse("not json").is_err());
        assert!(parse("{}").is_err()); // text manquant
    }

    #[test]
    fn queue_push_pop() {
        let queue: JobQueue = Arc::new(Mutex::new(VecDeque::new()));
        {
            let mut q = queue.lock().unwrap();
            q.push_back(DrawJob {
                text: "Test".into(),
                font: None,
                brush: None,
                color: None,
                thickness: None,
                speed: None,
                clear_before: None,
            });
        }
        let mut q = queue.lock().unwrap();
        let job = q.pop_front().unwrap();
        assert_eq!(job.text, "Test");
        assert!(q.is_empty());
    }
}
