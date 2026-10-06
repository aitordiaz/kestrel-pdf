use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions, Vec2};
use kestrel_core::document::{DocumentSession, SearchResult};
use kestrel_core::forms::{FormField, FormFieldType};
use kestrel_core::render::{PageTileKey, RenderPipeline};
use kestrel_core::sign::{self, DigitalSignatureMeta, StrokePoint, VisualSignature};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTool {
    Pan,
    SelectText,
    FormFill,
    EditText,
    EditImage,
    SignContract,
    RedactData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarTab {
    Thumbnails,
    Outlines,
    Forms,
    SearchResults,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitMode {
    FitWidth,
    FitPage,
}

pub struct KestrelApp {
    pub session: Option<DocumentSession>,
    pub current_file_name: Option<String>,
    pub current_page: usize,
    pub total_pages: usize,
    pub zoom_level: f32,
    pub pending_fit: Option<FitMode>,
    pub active_tool: ActiveTool,
    pub sidebar_open: bool,
    pub sidebar_tab: SidebarTab,
    pub search_query: String,
    pub search_results: Vec<SearchResult>,
    pub pipeline: Arc<RenderPipeline>,
    pub textures: HashMap<PageTileKey, TextureHandle>,

    // Phase 2: AcroForms & Contract Signing
    pub signature_modal_open: bool,
    pub signature_pad_raw_strokes: Vec<Vec<StrokePoint>>,
    pub signature_pad_current_stroke: Vec<StrokePoint>,
    pub adopted_signature: Option<VisualSignature>,
    pub signature_blue_ink: bool,
    pub signer_name_input: String,
    pub signature_reason_input: String,
    pub embed_digital_signature: bool,
    pub status_toast: Option<String>,
    pub last_window_title: String,
}

impl Default for KestrelApp {
    fn default() -> Self {
        Self {
            session: None,
            current_file_name: None,
            current_page: 1,
            total_pages: 0,
            zoom_level: 1.0,
            pending_fit: None,
            active_tool: ActiveTool::Pan,
            sidebar_open: true,
            sidebar_tab: SidebarTab::Thumbnails,
            search_query: String::new(),
            search_results: Vec::new(),
            pipeline: Arc::new(RenderPipeline::new(128)),
            textures: HashMap::new(),

            // Signature & Form defaults
            signature_modal_open: false,
            signature_pad_raw_strokes: Vec::new(),
            signature_pad_current_stroke: Vec::new(),
            adopted_signature: None,
            signature_blue_ink: true,
            signer_name_input: "John Doe".to_string(),
            signature_reason_input: "Approved and Signed via Kestrel-PDF".to_string(),
            embed_digital_signature: true,
            status_toast: None,
            last_window_title: String::new(),
        }
    }
}

impl KestrelApp {
    /// Loads a PDF from raw byte buffer (desktop or WASM).
    pub fn load_document_bytes(&mut self, bytes: Vec<u8>, name: Option<String>) {
        if let Ok(session) = DocumentSession::open_from_bytes(bytes, None) {
            self.total_pages = session.page_count as usize;
            self.current_page = if self.total_pages > 0 { 1 } else { 0 };
            self.current_file_name = name;
            self.textures.clear();
            self.search_results.clear();
            self.adopted_signature = None;
            self.status_toast = Some("Document loaded successfully.".to_string());
            self.session = Some(session);
        }
    }

    /// Triggers search query on active document session.
    pub fn execute_search(&mut self) {
        if let Some(session) = &self.session {
            self.search_results = session.search_text(&self.search_query);
            if !self.search_results.is_empty() {
                self.sidebar_tab = SidebarTab::SearchResults;
            }
        }
    }

    /// Returns the current title bar text including the active filename.
    pub fn title_bar_text(&self) -> String {
        match &self.current_file_name {
            Some(name) => format!("🦅 Kestrel-PDF — {}", name),
            None => "🦅 Kestrel-PDF".to_string(),
        }
    }

    /// Adopts the smoothed ink drawn in the signature pad.
    pub fn adopt_signature_from_pad(&mut self) {
        if self.signature_pad_raw_strokes.is_empty() && self.signature_pad_current_stroke.is_empty()
        {
            return;
        }

        let mut sig = VisualSignature {
            ink_color_rgb: if self.signature_blue_ink {
                [0, 51, 160] // Deep Royal Blue
            } else {
                [15, 23, 42] // Deep Slate / Black
            },
            ..Default::default()
        };

        for stroke in &self.signature_pad_raw_strokes {
            sig.add_smoothed_stroke(stroke);
        }
        if !self.signature_pad_current_stroke.is_empty() {
            sig.add_smoothed_stroke(&self.signature_pad_current_stroke);
        }

        self.adopted_signature = Some(sig);
        self.signature_modal_open = false;
        self.active_tool = ActiveTool::SignContract;
        self.status_toast =
            Some("✍️ Signature adopted! Click on the document page to place it.".to_string());
    }

    /// Places the adopted signature at specified page coordinates.
    pub fn place_adopted_signature(&mut self, page_index: u16, center_x: f32, center_y: f32) {
        if let Some(mut sig) = self.adopted_signature.clone() {
            sig.target_page = page_index;
            sig.bounding_box = [center_x - 75.0, center_y - 25.0, 150.0, 50.0];

            if let Some(session) = &mut self.session {
                session.add_visual_signature(sig);

                if self.embed_digital_signature {
                    let mut meta = DigitalSignatureMeta::new(&self.signer_name_input);
                    meta.reason = Some(self.signature_reason_input.clone());
                    session.set_digital_signature(meta);
                }

                self.status_toast = Some(format!("Signature stamped on Page {}", page_index + 1));
            }
        }
    }

    /// Serializes and saves the active document to disk (or browser).
    pub fn save_document(&mut self) {
        if let Some(session) = &mut self.session {
            match session.save_to_bytes() {
                Ok(bytes) => {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let default_name = self
                            .current_file_name
                            .as_deref()
                            .map(|n| {
                                if let Some(stripped) = n.strip_suffix(".pdf") {
                                    format!("{}_signed.pdf", stripped)
                                } else {
                                    format!("{}_signed.pdf", n)
                                }
                            })
                            .unwrap_or_else(|| "signed_document.pdf".to_string());

                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("PDF Document", &["pdf"])
                            .set_file_name(&default_name)
                            .save_file()
                        {
                            if let Ok(()) = std::fs::write(&path, &bytes) {
                                self.status_toast = Some(format!(
                                    "Saved signed document to {:?}",
                                    path.file_name().unwrap_or_default()
                                ));
                            }
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        self.status_toast =
                            Some(format!("Document prepared ({} bytes)", bytes.len()));
                    }
                }
                Err(err) => {
                    self.status_toast = Some(format!("Error saving document: {}", err));
                }
            }
        }
    }

    /// Renders the entire application UI layout given an egui Context.
    pub fn render_ui(&mut self, ctx: &Context) {
        // Update OS window title bar only when changed to avoid infinite repaint loops
        let current_title = self.title_bar_text();
        if self.last_window_title != current_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(current_title.clone()));
            self.last_window_title = current_title;
        }

        // Poll for newly rasterized background tiles
        self.pipeline.process_incoming_tiles();

        // 1. Top Toolbar
        egui::TopBottomPanel::top("top_toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(self.title_bar_text());
                ui.separator();

                if ui.button("📂 Open File").clicked() {
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PDF Documents", &["pdf"])
                        .pick_file()
                    {
                        if let Ok(bytes) = std::fs::read(&path) {
                            let name = path.file_name().map(|n| n.to_string_lossy().to_string());
                            self.load_document_bytes(bytes, name);
                        }
                    }
                }

                if self.session.is_some() && ui.button("💾 Save / Export").clicked() {
                    self.save_document();
                }

                ui.separator();

                // Tool Selector
                ui.selectable_value(&mut self.active_tool, ActiveTool::Pan, "✋ Pan");
                ui.selectable_value(&mut self.active_tool, ActiveTool::SelectText, "📝 Select");

                let form_count = self.session.as_ref().map(|s| s.forms.len()).unwrap_or(0);
                let form_label = if form_count > 0 {
                    format!("📋 Forms ({})", form_count)
                } else {
                    "📋 Forms".to_string()
                };
                ui.selectable_value(&mut self.active_tool, ActiveTool::FormFill, form_label);

                ui.selectable_value(&mut self.active_tool, ActiveTool::EditText, "✏️ Edit Text");

                if ui
                    .selectable_value(
                        &mut self.active_tool,
                        ActiveTool::SignContract,
                        "✍️ Sign Contract",
                    )
                    .clicked()
                    && self.adopted_signature.is_none()
                {
                    self.signature_modal_open = true;
                }

                if self.active_tool == ActiveTool::SignContract
                    && ui.button("🖊 Create Signature").clicked()
                {
                    self.signature_modal_open = true;
                }

                ui.selectable_value(&mut self.active_tool, ActiveTool::RedactData, "🛡️ Redact");

                ui.separator();

                // Live search bar
                ui.label("🔍");
                let search_resp = ui.text_edit_singleline(&mut self.search_query);
                if search_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.execute_search();
                }
                if ui.button("Find").clicked() {
                    self.execute_search();
                }

                // Right aligned Zoom & Page info
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("➕").clicked() {
                        self.zoom_level = (self.zoom_level * 1.15).min(10.0);
                    }
                    ui.label(format!("{:.0}%", self.zoom_level * 100.0));
                    if ui.button("➖").clicked() {
                        self.zoom_level = (self.zoom_level / 1.15).max(0.1);
                    }
                    if ui.button("Fit Page").clicked() {
                        self.pending_fit = Some(FitMode::FitPage);
                    }
                    if ui.button("Fit Width").clicked() {
                        self.pending_fit = Some(FitMode::FitWidth);
                    }
                    if ui.button("Reset").clicked() {
                        self.zoom_level = 1.0;
                    }

                    ui.separator();

                    if self.total_pages > 0 {
                        if ui.button("Next ➡").clicked() && self.current_page < self.total_pages {
                            self.current_page += 1;
                        }
                        ui.label(format!("Page {} / {}", self.current_page, self.total_pages));
                        if ui.button("⬅ Prev").clicked() && self.current_page > 1 {
                            self.current_page -= 1;
                        }
                    }
                });
            });
        });

        // 2. Status Banner / Toast
        if let Some(toast) = &self.status_toast.clone() {
            egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!("ℹ {}", toast));
                    if ui.button("✖").clicked() {
                        self.status_toast = None;
                    }
                });
            });
        }

        // 3. Left Sidebar: Thumbnails, Outlines, Forms, Search Results
        if self.sidebar_open {
            egui::SidePanel::left("left_sidebar")
                .resizable(true)
                .default_width(260.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Thumbnails, "Pages");
                        ui.selectable_value(
                            &mut self.sidebar_tab,
                            SidebarTab::Outlines,
                            "Outlines",
                        );
                        ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Forms, "Forms");
                        ui.selectable_value(
                            &mut self.sidebar_tab,
                            SidebarTab::SearchResults,
                            "Search",
                        );
                    });
                    ui.separator();

                    match self.sidebar_tab {
                        SidebarTab::Thumbnails => {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                for i in 1..=self.total_pages {
                                    let is_selected = self.current_page == i;
                                    let label = format!("Page {}", i);
                                    if ui.selectable_label(is_selected, label).clicked() {
                                        self.current_page = i;
                                    }
                                }
                                if self.total_pages == 0 {
                                    ui.label("No pages loaded.");
                                }
                            });
                        }
                        SidebarTab::Outlines => {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                if let Some(session) = &self.session {
                                    if session.outlines.is_empty() {
                                        ui.label("No document outlines / bookmarks.");
                                    } else {
                                        for outline in &session.outlines {
                                            if ui.link(&outline.title).clicked() {
                                                self.current_page =
                                                    (outline.target_page as usize) + 1;
                                            }
                                        }
                                    }
                                } else {
                                    ui.label("Open a document to view outlines.");
                                }
                            });
                        }
                        SidebarTab::Forms => {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                if let Some(session) = &mut self.session {
                                    if session.forms.is_empty() {
                                        ui.label("No AcroForm fields detected.");
                                        ui.add_space(8.0);
                                        if ui.button("➕ Add Form Field").clicked() {
                                            let new_field = FormField::new_text(
                                                "custom_name",
                                                "Full Name",
                                                0,
                                                "",
                                                [72.0, 700.0, 300.0, 725.0],
                                                false,
                                            );
                                            session.add_form_field(new_field);
                                        }
                                    } else {
                                        ui.heading(format!("Fields ({})", session.forms.len()));
                                        ui.separator();

                                        for (idx, field) in session.forms.iter_mut().enumerate() {
                                            ui.group(|ui| {
                                                ui.label(format!("{}. {}", idx + 1, field.name));
                                                ui.label(format!("Page {}", field.page_index + 1));

                                                match &mut field.field_type {
                                                    FormFieldType::Text { multiline, .. } => {
                                                        if *multiline {
                                                            ui.text_edit_multiline(
                                                                &mut field.value,
                                                            );
                                                        } else {
                                                            ui.text_edit_singleline(
                                                                &mut field.value,
                                                            );
                                                        }
                                                    }
                                                    FormFieldType::CheckBox { checked } => {
                                                        if ui.checkbox(checked, "Checked").changed()
                                                        {
                                                            field.value = if *checked {
                                                                "Yes".into()
                                                            } else {
                                                                "Off".into()
                                                            };
                                                        }
                                                    }
                                                    FormFieldType::Choice { options, selected } => {
                                                        let current_text = selected
                                                            .and_then(|i| options.get(i))
                                                            .cloned()
                                                            .unwrap_or_default();
                                                        egui::ComboBox::from_id_salt(format!(
                                                            "combo_{}",
                                                            idx
                                                        ))
                                                        .selected_text(current_text)
                                                        .show_ui(ui, |ui| {
                                                            for (opt_i, opt) in
                                                                options.iter().enumerate()
                                                            {
                                                                if ui
                                                                    .selectable_value(
                                                                        selected,
                                                                        Some(opt_i),
                                                                        opt,
                                                                    )
                                                                    .clicked()
                                                                {
                                                                    field.value = opt.clone();
                                                                }
                                                            }
                                                        });
                                                    }
                                                    _ => {
                                                        ui.text_edit_singleline(&mut field.value);
                                                    }
                                                }
                                            });
                                            ui.add_space(4.0);
                                        }
                                    }
                                } else {
                                    ui.label("Open a document to edit form fields.");
                                }
                            });
                        }
                        SidebarTab::SearchResults => {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                if self.search_results.is_empty() {
                                    ui.label("No matches found.");
                                } else {
                                    ui.label(format!(
                                        "Found {} matching pages:",
                                        self.search_results.len()
                                    ));
                                    for res in &self.search_results {
                                        let btn_label =
                                            format!("Page {}: {}", res.page_index + 1, res.snippet);
                                        if ui.button(btn_label).clicked() {
                                            self.current_page = (res.page_index as usize) + 1;
                                        }
                                    }
                                }
                            });
                        }
                    }
                });
        }

        // 4. Central Viewport: High-Performance Continuous Page Viewer
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.total_pages == 0 {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("Welcome to Kestrel-PDF");
                        ui.add_space(8.0);
                        ui.label(
                            "Instantaneous, high-performance universal PDF reader and editor.",
                        );
                        ui.add_space(16.0);
                        if ui.button("📂 Choose a PDF to open").clicked() {
                            #[cfg(not(target_arch = "wasm32"))]
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("PDF Documents", &["pdf"])
                                .pick_file()
                            {
                                if let Ok(bytes) = std::fs::read(&path) {
                                    let name =
                                        path.file_name().map(|n| n.to_string_lossy().to_string());
                                    self.load_document_bytes(bytes, name);
                                }
                            }
                        }
                    });
                });
            } else {
                // Apply pending Fit Width / Fit Page if requested
                if let Some(fit_mode) = self.pending_fit.take() {
                    let (page_w, page_h) = self
                        .session
                        .as_ref()
                        .and_then(|s| s.pages.get(self.current_page.saturating_sub(1)))
                        .map(|p| (p.width_pt, p.height_pt))
                        .unwrap_or((595.28, 841.89));

                    let avail = ui.available_size();
                    let margin_x = 48.0;
                    let margin_y = 48.0;
                    match fit_mode {
                        FitMode::FitWidth => {
                            let target_zoom = (avail.x - margin_x) / page_w;
                            self.zoom_level = target_zoom.clamp(0.1, 5.0);
                        }
                        FitMode::FitPage => {
                            let zoom_w = (avail.x - margin_x) / page_w;
                            let zoom_h = (avail.y - margin_y) / page_h;
                            self.zoom_level = zoom_w.min(zoom_h).clamp(0.1, 5.0);
                        }
                    }
                }

                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            for page_idx in 0..self.total_pages {
                                let (page_w, page_h) = if let Some(session) = &self.session {
                                    if let Some(p) = session.pages.get(page_idx) {
                                        (p.width_pt, p.height_pt)
                                    } else {
                                        (595.28, 841.89)
                                    }
                                } else {
                                    (595.28, 841.89)
                                };
                                let base_width = page_w * self.zoom_level;
                                let base_height = page_h * self.zoom_level;

                                let key = PageTileKey {
                                    page_index: page_idx as u16,
                                    tile_x: 0,
                                    tile_y: 0,
                                    zoom_level_percent: (self.zoom_level * 100.0) as u16,
                                    device_pixel_ratio_x100: 100,
                                };

                                // Request tile from background worker
                                let target_w = (base_width as u32).max(64);
                                let target_h = (base_height as u32).max(64);
                                self.pipeline.request_tile(key, target_w, target_h);

                                // Check if rasterized tile is in cache, bind to GPU texture
                                if !self.textures.contains_key(&key) {
                                    if let Some(tile_buf) = self.pipeline.cache().get(&key) {
                                        let img = ColorImage::from_rgba_unmultiplied(
                                            [tile_buf.width as usize, tile_buf.height as usize],
                                            &tile_buf.rgba,
                                        );
                                        let handle = ctx.load_texture(
                                            format!(
                                                "tile_{}_{}",
                                                key.page_index, key.zoom_level_percent
                                            ),
                                            img,
                                            TextureOptions::LINEAR,
                                        );
                                        self.textures.insert(key, handle);
                                    }
                                }

                                // Render Page Card with drop-shadow border
                                ui.add_space(16.0);
                                let sense = if self.active_tool == ActiveTool::SignContract
                                    || self.active_tool == ActiveTool::FormFill
                                {
                                    egui::Sense::click_and_drag()
                                } else {
                                    egui::Sense::hover()
                                };
                                let (response, painter) =
                                    ui.allocate_painter(Vec2::new(base_width, base_height), sense);
                                let rect = response.rect;

                                // Handle placing signature when clicking in SignContract mode
                                if self.active_tool == ActiveTool::SignContract
                                    && response.clicked()
                                {
                                    if let Some(hover_pos) = response.hover_pos() {
                                        let pdf_scale = self.zoom_level;
                                        let pt_x = (hover_pos.x - rect.left()) / pdf_scale;
                                        let pt_y = (hover_pos.y - rect.top()) / pdf_scale;
                                        self.place_adopted_signature(page_idx as u16, pt_x, pt_y);
                                    }
                                }

                                // 1. Draw page paper background with subtle document border
                                painter.rect_filled(rect, 4.0, Color32::WHITE);
                                painter.rect_stroke(
                                    rect,
                                    4.0,
                                    egui::Stroke::new(1.0_f32, Color32::from_rgb(200, 205, 215)),
                                );

                                // 2. If tile texture exists, draw behind text
                                if let Some(texture) = self.textures.get(&key) {
                                    painter.image(
                                        texture.id(),
                                        rect,
                                        egui::Rect::from_min_max(
                                            egui::pos2(0.0, 0.0),
                                            egui::pos2(1.0, 1.0),
                                        ),
                                        Color32::WHITE,
                                    );
                                }

                                // 3. Draw Vector Rectangles (borders, table grid cells, headers)
                                if let Some(session) = &self.session {
                                    if let Some(layout) = session.get_page_layout(page_idx) {
                                        for r in &layout.rects {
                                            let c_x = rect.left() + r.x * self.zoom_level;
                                            let c_y = rect.top()
                                                + (page_h - (r.y + r.height)) * self.zoom_level;
                                            let c_w = (r.width * self.zoom_level).abs();
                                            let c_h = (r.height * self.zoom_level).abs();
                                            let r_rect = egui::Rect::from_min_size(
                                                egui::pos2(c_x, c_y),
                                                Vec2::new(c_w, c_h),
                                            );

                                            if let Some(fill) = r.fill_color {
                                                // Avoid repainting whole page white background
                                                if !(fill == [255, 255, 255]
                                                    && c_w >= base_width * 0.98
                                                    && c_h >= base_height * 0.98)
                                                {
                                                    painter.rect_filled(
                                                        r_rect,
                                                        0.0,
                                                        Color32::from_rgb(
                                                            fill[0], fill[1], fill[2],
                                                        ),
                                                    );
                                                }
                                            }
                                            if let Some(stroke) = r.stroke_color {
                                                let sw = (r.stroke_width * self.zoom_level)
                                                    .clamp(0.5, 5.0);
                                                painter.rect_stroke(
                                                    r_rect,
                                                    0.0,
                                                    egui::Stroke::new(
                                                        sw,
                                                        Color32::from_rgb(
                                                            stroke[0], stroke[1], stroke[2],
                                                        ),
                                                    ),
                                                );
                                            }
                                        }
                                    }
                                }

                                // 4. Render PDF Positioned Text Content
                                if let Some(session) = &self.session {
                                    if let Some(layout) = session.get_page_layout(page_idx) {
                                        if !layout.text_runs.is_empty() {
                                            for tr in &layout.text_runs {
                                                let t_x = rect.left() + tr.x * self.zoom_level;
                                                let font_size = (tr.font_size * self.zoom_level)
                                                    .clamp(6.0, 72.0);
                                                let t_y = rect.top()
                                                    + (page_h - tr.y - tr.font_size * 0.85)
                                                        * self.zoom_level;
                                                let font_id = egui::FontId::proportional(font_size);
                                                let color = Color32::from_rgb(
                                                    tr.color[0],
                                                    tr.color[1],
                                                    tr.color[2],
                                                );

                                                painter.text(
                                                    egui::pos2(t_x, t_y),
                                                    egui::Align2::LEFT_TOP,
                                                    &tr.text,
                                                    font_id,
                                                    color,
                                                );
                                            }
                                        } else if let Some(text) = session.get_page_text(page_idx) {
                                            // Fallback linear text lines
                                            let mut y_offset = rect.top() + 36.0 * self.zoom_level;
                                            let x_margin = rect.left() + 40.0 * self.zoom_level;

                                            for (line_idx, line) in text.lines().enumerate() {
                                                let trimmed = line.trim();
                                                if trimmed.is_empty()
                                                    || trimmed.contains("Identity-H Unimplemented")
                                                {
                                                    y_offset += 14.0 * self.zoom_level;
                                                    continue;
                                                }
                                                let font_size = if line_idx == 0 {
                                                    (22.0 * self.zoom_level).clamp(12.0, 52.0)
                                                } else {
                                                    (14.0 * self.zoom_level).clamp(8.0, 36.0)
                                                };
                                                let font_id = egui::FontId::proportional(font_size);
                                                let color = if line_idx == 0 {
                                                    Color32::from_rgb(15, 23, 42)
                                                } else {
                                                    Color32::from_rgb(51, 65, 85)
                                                };

                                                painter.text(
                                                    egui::pos2(x_margin, y_offset),
                                                    egui::Align2::LEFT_TOP,
                                                    trimmed,
                                                    font_id,
                                                    color,
                                                );
                                                y_offset += font_size * 1.45;
                                                if y_offset > rect.bottom() - 24.0 {
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }

                                // 5. Interactive AcroForm Widgets on Page
                                if let Some(session) = &mut self.session {
                                    for field in &mut session.forms {
                                        if field.page_index == page_idx as u16 {
                                            // Map PDF coordinates [llx, lly, urx, ury] to egui canvas
                                            let f_x = rect.left() + field.rect[0] * self.zoom_level;
                                            let f_y = rect.top()
                                                + (page_h - field.rect[3]) * self.zoom_level;
                                            let f_w = ((field.rect[2] - field.rect[0])
                                                * self.zoom_level)
                                                .max(24.0);
                                            let f_h = ((field.rect[3] - field.rect[1])
                                                * self.zoom_level)
                                                .max(20.0);
                                            let field_rect = egui::Rect::from_min_size(
                                                egui::pos2(f_x, f_y),
                                                Vec2::new(f_w, f_h),
                                            );

                                            // Draw subtle background and border for interactive widget
                                            painter.rect_filled(
                                                field_rect,
                                                2.0,
                                                Color32::from_rgba_unmultiplied(239, 246, 255, 200),
                                            );
                                            painter.rect_stroke(
                                                field_rect,
                                                2.0,
                                                egui::Stroke::new(
                                                    1.0_f32,
                                                    Color32::from_rgb(96, 165, 250),
                                                ),
                                            );

                                            // Draw field value
                                            let display_val = match &field.field_type {
                                                FormFieldType::CheckBox { checked } => {
                                                    if *checked {
                                                        "✔".to_string()
                                                    } else {
                                                        "".to_string()
                                                    }
                                                }
                                                _ => field.value.clone(),
                                            };

                                            painter.text(
                                                egui::pos2(
                                                    field_rect.left() + 4.0,
                                                    field_rect.center().y,
                                                ),
                                                egui::Align2::LEFT_CENTER,
                                                &display_val,
                                                egui::FontId::proportional(
                                                    (13.0 * self.zoom_level).clamp(9.0, 24.0),
                                                ),
                                                Color32::from_rgb(30, 41, 59),
                                            );
                                        }
                                    }
                                }

                                // 5. Visual Signatures placed on this page
                                if let Some(session) = &self.session {
                                    for sig in &session.visual_signatures {
                                        if sig.target_page == page_idx as u16 {
                                            if let Some(bounds) = sig.compute_strokes_bounds() {
                                                let orig_w = (bounds[2] - bounds[0]).max(1.0);
                                                let orig_h = (bounds[3] - bounds[1]).max(1.0);

                                                let target_x = rect.left()
                                                    + sig.bounding_box[0] * self.zoom_level;
                                                let target_y = rect.top()
                                                    + sig.bounding_box[1] * self.zoom_level;
                                                let target_w =
                                                    sig.bounding_box[2] * self.zoom_level;
                                                let target_h =
                                                    sig.bounding_box[3] * self.zoom_level;

                                                let sx = target_w / orig_w;
                                                let sy = target_h / orig_h;

                                                let color = Color32::from_rgb(
                                                    sig.ink_color_rgb[0],
                                                    sig.ink_color_rgb[1],
                                                    sig.ink_color_rgb[2],
                                                );

                                                for stroke in &sig.strokes {
                                                    for i in 0..stroke.len().saturating_sub(1) {
                                                        let p1 = stroke[i];
                                                        let p2 = stroke[i + 1];

                                                        let x1 = target_x + (p1.x - bounds[0]) * sx;
                                                        let y1 = target_y + (p1.y - bounds[1]) * sy;
                                                        let x2 = target_x + (p2.x - bounds[0]) * sx;
                                                        let y2 = target_y + (p2.y - bounds[1]) * sy;

                                                        let stroke_width = (sig.stroke_width
                                                            * p1.pressure
                                                            * self.zoom_level)
                                                            .clamp(1.0, 8.0);
                                                        painter.line_segment(
                                                            [
                                                                egui::pos2(x1, y1),
                                                                egui::pos2(x2, y2),
                                                            ],
                                                            egui::Stroke::new(stroke_width, color),
                                                        );
                                                    }
                                                }

                                                // Draw digital certificate badge if embedded
                                                if session.digital_signature.is_some() {
                                                    let badge_rect = egui::Rect::from_min_size(
                                                        egui::pos2(
                                                            target_x,
                                                            target_y + target_h + 2.0,
                                                        ),
                                                        Vec2::new(target_w, 14.0 * self.zoom_level),
                                                    );
                                                    painter.rect_filled(
                                                        badge_rect,
                                                        2.0,
                                                        Color32::from_rgba_unmultiplied(
                                                            220, 252, 231, 220,
                                                        ),
                                                    );
                                                    painter.text(
                                                        badge_rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        "🔒 Digitally Verified PAdES / SHA-256",
                                                        egui::FontId::proportional(
                                                            (9.0 * self.zoom_level)
                                                                .clamp(7.0, 14.0),
                                                        ),
                                                        Color32::from_rgb(22, 101, 52),
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }

                                ui.label(format!("Page {}", page_idx + 1));
                                ui.add_space(16.0);
                            }
                        });
                    });
            }
        });

        // 5. Stylus / Mouse Signature Pad Window
        if self.signature_modal_open {
            egui::Window::new("✍️ Sign Contract — Digital & Visual Signature")
                .collapsible(false)
                .resizable(false)
                .default_size(Vec2::new(440.0, 480.0))
                .show(ctx, |ui| {
                    ui.label("Draw your signature below using mouse or pen stylus with Bézier smoothing:");

                    // Drawing Canvas Pad (400x160)
                    let pad_size = Vec2::new(400.0, 160.0);
                    let (pad_resp, pad_painter) = ui.allocate_painter(pad_size, egui::Sense::drag());
                    let pad_rect = pad_resp.rect;

                    pad_painter.rect_filled(pad_rect, 4.0, Color32::from_rgb(250, 250, 252));
                    pad_painter.rect_stroke(
                        pad_rect,
                        4.0,
                        egui::Stroke::new(1.5_f32, Color32::from_rgb(203, 213, 225)),
                    );

                    // Track drag points into current stroke
                    if pad_resp.drag_started() {
                        self.signature_pad_current_stroke.clear();
                    }
                    if pad_resp.dragged() {
                        if let Some(pos) = pad_resp.interact_pointer_pos() {
                            if pad_rect.contains(pos) {
                                let pressure = 1.0;
                                self.signature_pad_current_stroke.push(StrokePoint::new(
                                    pos.x - pad_rect.left(),
                                    pos.y - pad_rect.top(),
                                    pressure,
                                ));
                            }
                        }
                    }
                    if pad_resp.drag_stopped() && !self.signature_pad_current_stroke.is_empty() {
                        let stroke = std::mem::take(&mut self.signature_pad_current_stroke);
                        let smoothed = sign::smooth_stroke_bezier(&stroke, 4);
                        self.signature_pad_raw_strokes.push(smoothed);
                    }

                    // Render existing pad strokes
                    let ink_color = if self.signature_blue_ink {
                        Color32::from_rgb(0, 51, 160)
                    } else {
                        Color32::from_rgb(15, 23, 42)
                    };

                    for stroke in &self.signature_pad_raw_strokes {
                        for i in 0..stroke.len().saturating_sub(1) {
                            let p1 = stroke[i];
                            let p2 = stroke[i + 1];
                            pad_painter.line_segment(
                                [
                                    egui::pos2(pad_rect.left() + p1.x, pad_rect.top() + p1.y),
                                    egui::pos2(pad_rect.left() + p2.x, pad_rect.top() + p2.y),
                                ],
                                egui::Stroke::new(2.2_f32, ink_color),
                            );
                        }
                    }
                    for i in 0..self.signature_pad_current_stroke.len().saturating_sub(1) {
                        let p1 = self.signature_pad_current_stroke[i];
                        let p2 = self.signature_pad_current_stroke[i + 1];
                        pad_painter.line_segment(
                            [
                                egui::pos2(pad_rect.left() + p1.x, pad_rect.top() + p1.y),
                                egui::pos2(pad_rect.left() + p2.x, pad_rect.top() + p2.y),
                            ],
                            egui::Stroke::new(2.2_f32, ink_color),
                        );
                    }

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Ink Color:");
                        ui.radio_value(&mut self.signature_blue_ink, true, "Royal Blue");
                        ui.radio_value(&mut self.signature_blue_ink, false, "Deep Slate");

                        ui.separator();
                        if ui.button("🗑 Clear Pad").clicked() {
                            self.signature_pad_raw_strokes.clear();
                            self.signature_pad_current_stroke.clear();
                        }
                        if ui.button("↩ Undo").clicked() {
                            self.signature_pad_raw_strokes.pop();
                        }
                    });

                    ui.separator();
                    ui.heading("🔒 Cryptographic PAdES Metadata");
                    ui.checkbox(&mut self.embed_digital_signature, "Embed PAdES Digital Signature (SHA-256 Digest)");

                    if self.embed_digital_signature {
                        ui.horizontal(|ui| {
                            ui.label("Signer Name:");
                            ui.text_edit_singleline(&mut self.signer_name_input);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Reason:");
                            ui.text_edit_singleline(&mut self.signature_reason_input);
                        });
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("✅ Adopt & Place Signature").clicked() {
                            self.adopt_signature_from_pad();
                        }
                        if ui.button("Cancel").clicked() {
                            self.signature_modal_open = false;
                        }
                    });
                });
        }
    }
}

impl eframe::App for KestrelApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.render_ui(ctx);
    }
}
