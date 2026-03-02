mod app;
mod brush;
mod config;
mod fonts;
mod glyph;
mod recorder;
mod server;
mod simulator;
mod stroke;

use app::CalliApp;
use config::AppConfig;
use server::JobQueue;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

fn main() -> eframe::Result<()> {
    // 1. Charger la config (crée config.toml si absent)
    let cfg = AppConfig::load();

    // 2. Créer les dossiers nécessaires
    let _ = std::fs::create_dir_all(&cfg.fonts.cache_dir);
    let _ = std::fs::create_dir_all("assets/brushes");

    // 3. Créer la file d'attente partagée entre le thread TCP et le thread UI
    let queue: JobQueue = Arc::new(Mutex::new(VecDeque::new()));

    // 4. Lancer le serveur TCP si activé dans la config
    if cfg.server.enabled {
        server::start(queue.clone(), cfg.server.port);
    }

    // 5. Lancer l'UI egui
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CaligraphieAiTracer")
            .with_inner_size([1280.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "CaligraphieAiTracer",
        options,
        Box::new(move |_cc| Ok(Box::new(CalliApp::new(cfg, queue)))),
    )
}
