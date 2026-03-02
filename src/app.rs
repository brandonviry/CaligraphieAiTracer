use egui::{Color32, ColorImage, RichText, TextureHandle, TextureOptions, Vec2};
use std::path::PathBuf;
use ttf_parser::Face;

use crate::brush::{all_brushes, Brush, Canvas};
use crate::config::AppConfig;
use crate::fonts::{ensure_font, load_font_bytes};
use crate::glyph::text_to_outlines;
use crate::recorder;
use crate::server::{DrawJob, JobQueue};
use crate::simulator::{paint_stroke_on_canvas, simulate_glyph, SimConfig};
use crate::stroke::Stroke;

// ─── État de l'application ────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
enum AppState {
    Idle,
    Animating,
    Done,
}

/// Snapshot des paramètres UI avant d'entrer en mode édition (pour annuler).
struct UiSnapshot {
    text: String,
    font_idx: usize,
    brush_idx: usize,
    ink_color: [f32; 3],
    base_thickness: f32,
    animation_speed: f32,
}

// ─── Application principale ───────────────────────────────────────────────────

pub struct CalliApp {
    config: AppConfig,

    // Paramètres de l'éditeur (panneau gauche)
    text: String,
    font_idx: usize,
    brush_idx: usize,
    ink_color: [f32; 3],
    base_thickness: f32,
    animation_speed: f32,

    // Pipeline
    brushes: Vec<Brush>,
    font_list: Vec<String>,
    face_bytes: Option<Vec<u8>>,
    strokes: Vec<Stroke>,
    canvas: Canvas,

    // Animation
    state: AppState,
    anim_stroke_idx: usize,
    anim_point_idx: usize,
    canvas_dirty: bool,
    texture: Option<TextureHandle>,

    // File d'attente partagée (TCP + UI)
    queue: JobQueue,
    // Job actuellement en cours d'animation (affiché séparément)
    current_job: Option<DrawJob>,

    // Mode édition d'un job de la file
    // Quand Some(idx), le panneau affiche un bandeau et les boutons Valider/Annuler
    editing_queue_idx: Option<usize>,
    edit_snapshot: Option<UiSnapshot>,

    // Export / Import
    export_path: String,

    status: String,
}

impl CalliApp {
    pub fn new(config: AppConfig, queue: JobQueue) -> Self {
        let brushes = all_brushes(&config.brushes.custom);
        let font_list = config.fonts.list.clone();

        let brush_idx = brushes
            .iter()
            .position(|b| b.label().to_lowercase().replace(' ', "_") == config.defaults.brush)
            .unwrap_or(0);

        let font_idx = font_list
            .iter()
            .position(|f| f == &config.defaults.font)
            .unwrap_or(0);

        let ink = config.defaults.color;
        let ink_color = [
            ink[0] as f32 / 255.0,
            ink[1] as f32 / 255.0,
            ink[2] as f32 / 255.0,
        ];

        let canvas_w = config.canvas.width;
        let canvas_h = config.canvas.height;

        Self {
            text: String::from("Bonjour"),
            font_idx,
            brush_idx,
            ink_color,
            base_thickness: config.defaults.thickness,
            animation_speed: config.defaults.speed,
            brushes,
            font_list,
            face_bytes: None,
            strokes: Vec::new(),
            canvas: Canvas::new(canvas_w, canvas_h),
            state: AppState::Idle,
            anim_stroke_idx: 0,
            anim_point_idx: 0,
            canvas_dirty: false,
            texture: None,
            queue,
            current_job: None,
            editing_queue_idx: None,
            edit_snapshot: None,
            export_path: String::from("tracé.json"),
            status: String::from("Prêt."),
            config,
        }
    }

    // ── Création d'un DrawJob depuis les paramètres UI courants ───────────────

    fn current_ui_job(&self) -> DrawJob {
        DrawJob {
            text: self.text.clone(),
            font: Some(self.font_list[self.font_idx].clone()),
            brush: Some(self.brushes[self.brush_idx].label().to_lowercase().replace(' ', "_")),
            color: Some(self.ink_color_u8()),
            thickness: Some(self.base_thickness),
            speed: Some(self.animation_speed),
            clear_before: Some(true),
        }
    }

    // ── Gestion de la file d'attente ──────────────────────────────────────────

    /// Ajoute le job UI courant à la fin de la file.
    fn enqueue_current(&mut self) {
        if self.text.trim().is_empty() { return; }
        let job = self.current_ui_job();
        if let Ok(mut q) = self.queue.lock() {
            q.push_back(job);
        }
        self.status = format!("Job «{}» ajouté à la file.", self.text);
    }

    /// Vide la file et lance le job courant immédiatement.
    fn trace_now(&mut self) {
        if self.text.trim().is_empty() { return; }
        let job = self.current_ui_job();
        if let Ok(mut q) = self.queue.lock() {
            q.clear();
            q.push_front(job);
        }
        if self.state != AppState::Animating {
            self.poll_queue();
        }
    }

    /// Dépile un job et le démarre. Appelé quand l'app est libre.
    fn poll_queue(&mut self) {
        let job = {
            if let Ok(mut q) = self.queue.try_lock() {
                q.pop_front()
            } else {
                return;
            }
        };

        if let Some(job) = job {
            self.apply_job(&job);
            self.current_job = Some(job);
        }
    }

    /// Applique les paramètres d'un job et lance le tracé.
    fn apply_job(&mut self, job: &DrawJob) {
        self.text = job.text.clone();

        if let Some(ref font) = job.font {
            if let Some(idx) = self.font_list.iter().position(|f| f == font) {
                self.font_idx = idx;
            }
        }
        if let Some(ref brush) = job.brush {
            if let Some(idx) = self.brushes.iter().position(|b| {
                b.label().to_lowercase().replace(' ', "_") == *brush
            }) {
                self.brush_idx = idx;
            }
        }
        if let Some(color) = job.color {
            self.ink_color = [
                color[0] as f32 / 255.0,
                color[1] as f32 / 255.0,
                color[2] as f32 / 255.0,
            ];
        }
        if let Some(t) = job.thickness { self.base_thickness = t; }
        if let Some(s) = job.speed     { self.animation_speed = s; }

        if job.clear_before.unwrap_or(true) {
            self.canvas.clear();
            self.canvas_dirty = true;
        }

        self.start_tracing();
    }

    /// Charge un job de la file dans les contrôles UI (mode édition).
    fn load_job_for_editing(&mut self, idx: usize) {
        let job = {
            if let Ok(q) = self.queue.try_lock() {
                q.get(idx).cloned()
            } else {
                return;
            }
        };

        if let Some(job) = job {
            // Sauvegarder l'état UI actuel pour pouvoir annuler
            self.edit_snapshot = Some(UiSnapshot {
                text: self.text.clone(),
                font_idx: self.font_idx,
                brush_idx: self.brush_idx,
                ink_color: self.ink_color,
                base_thickness: self.base_thickness,
                animation_speed: self.animation_speed,
            });

            // Charger les paramètres du job dans les contrôles
            self.text = job.text.clone();
            if let Some(ref font) = job.font {
                if let Some(i) = self.font_list.iter().position(|f| f == font) {
                    self.font_idx = i;
                }
            }
            if let Some(ref brush) = job.brush {
                if let Some(i) = self.brushes.iter().position(|b| {
                    b.label().to_lowercase().replace(' ', "_") == *brush
                }) {
                    self.brush_idx = i;
                }
            }
            if let Some(color) = job.color {
                self.ink_color = [
                    color[0] as f32 / 255.0,
                    color[1] as f32 / 255.0,
                    color[2] as f32 / 255.0,
                ];
            }
            if let Some(t) = job.thickness { self.base_thickness = t; }
            if let Some(s) = job.speed     { self.animation_speed = s; }

            self.editing_queue_idx = Some(idx);
            self.status = format!("Modification du job {} — ajustez les paramètres puis validez.", idx + 1);
        }
    }

    /// Valide les modifications et les écrit dans la file.
    fn confirm_edit(&mut self) {
        if let Some(idx) = self.editing_queue_idx {
            let new_job = self.current_ui_job();
            if let Ok(mut q) = self.queue.lock() {
                if let Some(slot) = q.get_mut(idx) {
                    *slot = new_job;
                }
            }
            self.status = format!("Job {} mis à jour.", idx + 1);
        }
        self.editing_queue_idx = None;
        self.edit_snapshot = None;
    }

    /// Annule les modifications et restaure l'état UI d'avant.
    fn cancel_edit(&mut self) {
        if let Some(snap) = self.edit_snapshot.take() {
            self.text            = snap.text;
            self.font_idx        = snap.font_idx;
            self.brush_idx       = snap.brush_idx;
            self.ink_color       = snap.ink_color;
            self.base_thickness  = snap.base_thickness;
            self.animation_speed = snap.animation_speed;
        }
        self.editing_queue_idx = None;
        self.status = "Modification annulée.".into();
    }

    // ── Pipeline de tracé ─────────────────────────────────────────────────────

    fn start_tracing(&mut self) {
        if self.text.trim().is_empty() {
            self.status = "Texte vide.".into();
            return;
        }

        let font_name  = self.font_list[self.font_idx].clone();
        let cache_dir  = self.config.fonts.cache_dir.clone();
        let glyph_scale = self.config.canvas.glyph_scale;
        let canvas_w   = self.config.canvas.width;
        let canvas_h   = self.config.canvas.height;

        self.status = format!("Chargement de «{font_name}»…");

        let path = match ensure_font(&font_name, &cache_dir) {
            Ok(p) => p,
            Err(e) => { self.status = format!("Erreur police : {e}"); return; }
        };

        let bytes = match load_font_bytes(&path) {
            Ok(b) => b,
            Err(e) => { self.status = format!("Erreur lecture : {e}"); return; }
        };

        self.face_bytes = Some(bytes);
        let face = match Face::parse(self.face_bytes.as_ref().unwrap(), 0) {
            Ok(f) => f,
            Err(e) => { self.status = format!("Police invalide : {e:?}"); return; }
        };

        let (outlines, total_w) = text_to_outlines(&face, &self.text, glyph_scale);
        let sim_cfg = SimConfig {
            base_thickness: self.base_thickness,
            ink_color: self.ink_color_u8(),
            opacity: 1.0,
        };

        let mut all_strokes = Vec::new();
        for outline in &outlines {
            all_strokes.extend(simulate_glyph(&outline.strokes, &sim_cfg));
        }

        if all_strokes.is_empty() {
            self.status = "Aucun trait généré.".into();
            return;
        }

        let offset_x = ((canvas_w as f32 - total_w) * 0.5).max(20.0);
        let offset_y = (canvas_h as f32 - glyph_scale) * 0.5;

        for stroke in &mut all_strokes {
            for pt in &mut stroke.points {
                pt.pos[0] += offset_x;
                pt.pos[1] += offset_y;
            }
        }

        self.strokes = all_strokes;
        self.anim_stroke_idx = 0;
        self.anim_point_idx  = 0;
        self.state = AppState::Animating;
        self.status = format!("Tracé de «{}» en cours…", self.text);
    }

    fn step_animation(&mut self) {
        let speed = self.animation_speed as usize;
        let cfg = SimConfig {
            base_thickness: self.base_thickness,
            ink_color: self.ink_color_u8(),
            opacity: 1.0,
        };

        for _ in 0..speed {
            if self.anim_stroke_idx >= self.strokes.len() {
                self.state = AppState::Done;
                let name = self.current_job.as_ref().map(|j| j.text.as_str()).unwrap_or("?");
                self.status = format!("«{}» terminé.", name);
                self.current_job = None;
                return;
            }

            let brush = &self.brushes[self.brush_idx];
            let stroke = &self.strokes[self.anim_stroke_idx];
            self.anim_point_idx += 1;

            if self.anim_point_idx >= stroke.points.len() {
                paint_stroke_on_canvas(stroke, brush, &mut self.canvas, &cfg, stroke.points.len());
                self.anim_stroke_idx += 1;
                self.anim_point_idx  = 0;
            } else {
                paint_stroke_on_canvas(stroke, brush, &mut self.canvas, &cfg, self.anim_point_idx);
            }
            self.canvas_dirty = true;
        }
    }

    fn ink_color_u8(&self) -> [u8; 3] {
        [
            (self.ink_color[0] * 255.0) as u8,
            (self.ink_color[1] * 255.0) as u8,
            (self.ink_color[2] * 255.0) as u8,
        ]
    }

    fn sync_texture(&mut self, ctx: &egui::Context) {
        if !self.canvas_dirty { return; }
        self.canvas_dirty = false;
        let w = self.config.canvas.width;
        let h = self.config.canvas.height;
        let img = ColorImage::from_rgba_unmultiplied([w, h], &self.canvas.pixels);
        match &mut self.texture {
            Some(tex) => tex.set(img, TextureOptions::LINEAR),
            None => { self.texture = Some(ctx.load_texture("canvas", img, TextureOptions::LINEAR)); }
        }
    }

    // ── Rendu de la section file d'attente ────────────────────────────────────

    fn show_queue_panel(&mut self, ui: &mut egui::Ui) {
        let queue_len = self.queue.try_lock().map(|q| q.len()).unwrap_or(0);

        ui.separator();
        ui.add_space(6.0);

        // Titre avec compteur
        ui.horizontal(|ui| {
            ui.label(RichText::new("File d'attente").strong());
            if queue_len > 0 {
                ui.label(
                    RichText::new(format!("({queue_len})"))
                        .color(Color32::from_rgb(100, 160, 255))
                        .small(),
                );
            } else {
                ui.label(RichText::new("(vide)").color(Color32::GRAY).small());
            }
        });
        ui.add_space(4.0);

        // Job EN COURS
        if let Some(ref job) = self.current_job.clone() {
            egui::Frame::new()
                .fill(Color32::from_rgb(30, 80, 40))
                .corner_radius(4.0)
                .inner_margin(egui::Margin::same(6))
                .show(ui, |ui| {
                    ui.label(RichText::new("▶ EN COURS").color(Color32::from_rgb(100, 220, 100)).small().strong());
                    ui.label(RichText::new(format!("«{}»", job.text)).strong());
                    ui.label(RichText::new(self.job_summary(job)).color(Color32::LIGHT_GRAY).small());
                });
            ui.add_space(4.0);
        }

        // Jobs en attente
        let mut to_remove:  Option<usize> = None;
        let mut move_up:    Option<usize> = None;
        let mut move_down:  Option<usize> = None;
        let mut start_edit: Option<usize> = None;
        let mut clear_all = false;

        if queue_len > 0 {
            egui::ScrollArea::vertical()
                .max_height(240.0)
                .id_salt("queue_scroll")
                .show(ui, |ui| {
                    if let Ok(q) = self.queue.try_lock() {
                        for (i, job) in q.iter().enumerate() {
                            let is_being_edited = self.editing_queue_idx == Some(i);

                            ui.horizontal(|ui| {
                                // Flèches réordonnancement
                                ui.vertical(|ui| {
                                    if ui.add_enabled(i > 0, egui::Button::new("▲").small())
                                        .on_hover_text("Monter ce job").clicked()
                                    {
                                        move_up = Some(i);
                                    }
                                    if ui.add_enabled(i + 1 < queue_len, egui::Button::new("▼").small())
                                        .on_hover_text("Descendre ce job").clicked()
                                    {
                                        move_down = Some(i);
                                    }
                                });

                                // Numéro + texte + résumé
                                ui.vertical(|ui| {
                                    let label = if is_being_edited {
                                        RichText::new(format!("{}. «{}» ✏", i + 1, job.text))
                                            .small().strong().color(Color32::from_rgb(255, 200, 80))
                                    } else {
                                        RichText::new(format!("{}. «{}»", i + 1, job.text))
                                            .small().strong()
                                    };
                                    ui.label(label);
                                    ui.label(
                                        RichText::new(self.job_summary(job))
                                            .color(Color32::GRAY)
                                            .small(),
                                    );
                                });

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.small_button("✕").on_hover_text("Supprimer ce job").clicked() {
                                        to_remove = Some(i);
                                    }
                                    // Bouton ✏ : désactivé si on est déjà en train d'éditer un autre job
                                    let edit_available = self.editing_queue_idx.is_none() || is_being_edited;
                                    let edit_btn = ui.add_enabled(
                                        edit_available,
                                        egui::Button::new("✏").small(),
                                    ).on_hover_text("Charger dans le panneau pour modification");
                                    if edit_btn.clicked() && !is_being_edited {
                                        start_edit = Some(i);
                                    }
                                });
                            });

                            if i < queue_len - 1 {
                                ui.separator();
                            }
                        }
                    }
                });

            ui.add_space(4.0);
            if ui.small_button("✕  Vider la file").on_hover_text("Supprimer tous les jobs en attente").clicked() {
                clear_all = true;
            }
        }

        // ── Appliquer les mutations après avoir relâché le lock ──

        if let Some(idx) = start_edit {
            self.load_job_for_editing(idx);
        }

        if let Some(idx) = to_remove {
            if let Ok(mut q) = self.queue.lock() {
                q.remove(idx);
            }
            // Si on supprime le job en cours d'édition → annuler l'édition
            if self.editing_queue_idx == Some(idx) {
                self.cancel_edit();
            }
        }

        if let Some(idx) = move_up {
            if idx > 0 {
                if let Ok(mut q) = self.queue.lock() {
                    q.swap(idx, idx - 1);
                }
                // L'index d'édition suit le job déplacé
                if self.editing_queue_idx == Some(idx) {
                    self.editing_queue_idx = Some(idx - 1);
                } else if self.editing_queue_idx == Some(idx - 1) {
                    self.editing_queue_idx = Some(idx);
                }
            }
        }

        if let Some(idx) = move_down {
            if let Ok(mut q) = self.queue.lock() {
                if idx + 1 < q.len() {
                    q.swap(idx, idx + 1);
                }
            }
            if self.editing_queue_idx == Some(idx) {
                self.editing_queue_idx = Some(idx + 1);
            } else if self.editing_queue_idx == Some(idx + 1) {
                self.editing_queue_idx = Some(idx);
            }
        }

        if clear_all {
            if let Ok(mut q) = self.queue.lock() {
                q.clear();
            }
            if self.editing_queue_idx.is_some() {
                self.cancel_edit();
            }
            self.status = "File vidée.".into();
        }
    }

    /// Résumé court d'un job pour l'affichage dans la file.
    fn job_summary(&self, job: &DrawJob) -> String {
        let font  = job.font.as_deref().unwrap_or("—");
        let brush = job.brush.as_deref().unwrap_or("—");
        let thick = job.thickness.map(|t| format!("{t:.0}px")).unwrap_or_default();
        let speed = job.speed.map(|s| format!("v:{s:.0}")).unwrap_or_default();
        format!("{font} · {brush} · {thick} · {speed}")
    }

    // ── Export / Import ───────────────────────────────────────────────────────

    /// Exporte les traits courants dans `self.export_path`.
    fn do_export(&mut self) {
        if self.strokes.is_empty() {
            self.status = "Rien à exporter (canvas vide).".into();
            return;
        }
        let path = PathBuf::from(&self.export_path);
        match recorder::export(&self.strokes, &path) {
            Ok(()) => self.status = format!("Exporté → {}", self.export_path),
            Err(e) => self.status = format!("Erreur export : {e}"),
        }
    }

    /// Charge un fichier JSON et rejoue l'animation depuis le début.
    fn do_replay(&mut self) {
        let path = PathBuf::from(&self.export_path);
        match recorder::import(&path) {
            Ok(strokes) => {
                self.strokes = strokes;
                self.canvas.clear();
                self.canvas_dirty = true;
                self.anim_stroke_idx = 0;
                self.anim_point_idx  = 0;
                self.state = AppState::Animating;
                self.current_job = None;
                self.status = format!("Replay depuis «{}» ({} traits).", self.export_path, self.strokes.len());
            }
            Err(e) => self.status = format!("Erreur import : {e}"),
        }
    }

    /// Panneau Export / Import affiché sous la file d'attente.
    fn show_export_panel(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.add_space(6.0);
        ui.label(RichText::new("Export / Rejouer").strong());
        ui.add_space(4.0);

        ui.label(RichText::new("Fichier :").small());
        ui.text_edit_singleline(&mut self.export_path);
        ui.add_space(4.0);

        let has_strokes   = !self.strokes.is_empty();
        let not_animating = self.state != AppState::Animating;

        ui.horizontal(|ui| {
            if ui.add_enabled(has_strokes, egui::Button::new("💾 Exporter"))
                .on_hover_text("Sauvegarde le tracé courant en JSON")
                .clicked()
            {
                self.do_export();
            }
            if ui.add_enabled(not_animating, egui::Button::new("▶ Rejouer"))
                .on_hover_text("Charge et rejoue un tracé JSON")
                .clicked()
            {
                self.do_replay();
            }
        });
    }
}

// ─── eframe::App ──────────────────────────────────────────────────────────────

impl eframe::App for CalliApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Dépiler un job si l'app est libre (pas en mode édition pour éviter de perturber les contrôles)
        if self.editing_queue_idx.is_none()
            && (self.state == AppState::Idle || self.state == AppState::Done)
        {
            self.poll_queue();
        }

        if self.state == AppState::Animating {
            self.step_animation();
            ctx.request_repaint();
        }

        self.sync_texture(ctx);

        let canvas_w = self.config.canvas.width as f32;
        let canvas_h = self.config.canvas.height as f32;

        // ── Panneau gauche ──
        egui::SidePanel::left("controls")
            .exact_width(290.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(12.0);
                    ui.heading("CaligraphieAI");
                    ui.separator();
                    ui.add_space(8.0);

                    // ── Bandeau mode édition ──
                    let editing = self.editing_queue_idx;
                    if let Some(idx) = editing {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(60, 45, 10))
                            .corner_radius(4.0)
                            .inner_margin(egui::Margin::same(6))
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(format!("✏ Modification du job {}", idx + 1))
                                        .color(Color32::from_rgb(255, 200, 80))
                                        .small()
                                        .strong(),
                                );
                                ui.add_space(4.0);
                                ui.horizontal(|ui| {
                                    if ui.button("✓ Valider").on_hover_text("Sauvegarder les modifications dans la file").clicked() {
                                        self.confirm_edit();
                                    }
                                    if ui.button("✗ Annuler").on_hover_text("Restaurer les paramètres d'avant").clicked() {
                                        self.cancel_edit();
                                    }
                                });
                            });
                        ui.add_space(6.0);
                    }

                    // ── Éditeur ──
                    ui.label("Texte :");
                    ui.text_edit_multiline(&mut self.text);
                    ui.add_space(6.0);

                    ui.label("Police :");
                    let font_name = self.font_list[self.font_idx].clone();
                    egui::ComboBox::from_id_salt("font_combo")
                        .selected_text(&font_name)
                        .show_ui(ui, |ui| {
                            for (i, name) in self.font_list.iter().enumerate() {
                                ui.selectable_value(&mut self.font_idx, i, name.as_str());
                            }
                        });
                    ui.add_space(6.0);

                    ui.label("Brosse :");
                    let brush_label = self.brushes[self.brush_idx].label();
                    egui::ComboBox::from_id_salt("brush_combo")
                        .selected_text(brush_label)
                        .show_ui(ui, |ui| {
                            for i in 0..self.brushes.len() {
                                let label = self.brushes[i].label();
                                ui.selectable_value(&mut self.brush_idx, i, label);
                            }
                        });
                    ui.add_space(6.0);

                    ui.label("Couleur de l'encre :");
                    ui.color_edit_button_rgb(&mut self.ink_color);
                    ui.add_space(6.0);

                    ui.label(format!("Épaisseur : {:.0} px", self.base_thickness));
                    ui.add(egui::Slider::new(&mut self.base_thickness, 4.0..=60.0).show_value(false));
                    ui.add_space(6.0);

                    ui.label(format!("Vitesse : {:.0} pts/frame", self.animation_speed));
                    ui.add(egui::Slider::new(&mut self.animation_speed, 1.0..=30.0).show_value(false));
                    ui.add_space(10.0);

                    // ── Boutons d'action (désactivés en mode édition) ──
                    let animating = self.state == AppState::Animating;
                    let in_edit   = self.editing_queue_idx.is_some();

                    ui.horizontal(|ui| {
                        if ui.add_enabled(!animating && !in_edit, egui::Button::new("▶  Tracer maintenant"))
                            .on_hover_text("Vide la file et trace immédiatement")
                            .clicked()
                        {
                            self.trace_now();
                        }
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!in_edit, egui::Button::new("+ Ajouter à la file"))
                            .on_hover_text("Ajoute ce job à la fin de la file")
                            .clicked()
                        {
                            self.enqueue_current();
                        }
                        if ui.button("✕ Effacer canvas")
                            .on_hover_text("Efface le canvas")
                            .clicked()
                        {
                            self.canvas.clear();
                            self.canvas_dirty = true;
                            self.strokes.clear();
                            self.state = AppState::Idle;
                            self.status = "Canvas effacé.".into();
                        }
                    });

                    // ── File d'attente ──
                    self.show_queue_panel(ui);

                    // ── Export / Import ──
                    self.show_export_panel(ui);

                    // ── Status ──
                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.label(RichText::new(&self.status).small().italics());
                });
            });

        // ── Canvas central ──
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(tex) = &self.texture {
                let available = ui.available_size();
                let tex_size  = Vec2::new(canvas_w, canvas_h);
                let scale     = (available.x / tex_size.x).min(available.y / tex_size.y).min(1.0);
                ui.add(egui::Image::new(tex).fit_to_exact_size(tex_size * scale));
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("Saisissez un texte et cliquez sur ▶ Tracer maintenant")
                            .size(18.0)
                            .color(Color32::from_gray(160)),
                    );
                });
            }
        });
    }
}
