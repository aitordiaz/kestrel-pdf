use egui::{Color32, ColorImage, Context, TextureHandle, TextureOptions, Vec2};
use kestrel_core::document::{DocumentSession, SearchResult};
use kestrel_core::render::{PageTileKey, RenderPipeline};
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
    SearchResults,
}

pub struct KestrelApp {
    pub session: Option<DocumentSession>,
    pub current_file_name: Option<String>,
    pub current_page: usize,
    pub total_pages: usize,
    pub zoom_level: f32,
    pub active_tool: ActiveTool,
    pub sidebar_open: bool,
    pub sidebar_tab: SidebarTab,
    pub search_query: String,
    pub search_results: Vec<SearchResult>,
    pub pipeline: Arc<RenderPipeline>,
    pub textures: HashMap<PageTileKey, TextureHandle>,
}

impl Default for KestrelApp {
    fn default() -> Self {
        Self {
            session: None,
            current_file_name: None,
            current_page: 1,
            total_pages: 0,
            zoom_level: 1.0,
            active_tool: ActiveTool::Pan,
            sidebar_open: true,
            sidebar_tab: SidebarTab::Thumbnails,
            search_query: String::new(),
            search_results: Vec::new(),
            pipeline: Arc::new(RenderPipeline::new(128)),
            textures: HashMap::new(),
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

    /// Renders the entire application UI layout given an egui Context.
    pub fn render_ui(&mut self, ctx: &Context) {
        // Update OS window title bar with active document name
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.title_bar_text()));

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

                ui.separator();

                // Tool Selector
                ui.selectable_value(&mut self.active_tool, ActiveTool::Pan, "✋ Pan");
                ui.selectable_value(&mut self.active_tool, ActiveTool::SelectText, "📝 Select");
                ui.selectable_value(&mut self.active_tool, ActiveTool::FormFill, "📋 Forms");
                ui.selectable_value(&mut self.active_tool, ActiveTool::EditText, "✏️ Edit Text");
                ui.selectable_value(&mut self.active_tool, ActiveTool::SignContract, "✍️ Sign");
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
                    if ui.button("Fit Width").clicked() {
                        self.zoom_level = 1.0;
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

        // 2. Left Sidebar: Thumbnails, Outlines, Search Results
        if self.sidebar_open {
            egui::SidePanel::left("left_sidebar")
                .resizable(true)
                .default_width(240.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Thumbnails, "Pages");
                        ui.selectable_value(
                            &mut self.sidebar_tab,
                            SidebarTab::Outlines,
                            "Outlines",
                        );
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

        // 3. Central Viewport: High-Performance Continuous Page Viewer
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
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            let base_width = 595.0 * self.zoom_level;
                            let base_height = 842.0 * self.zoom_level;

                            for page_idx in 0..self.total_pages {
                                let page_num = page_idx + 1;
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
                                let (response, painter) = ui.allocate_painter(
                                    Vec2::new(base_width, base_height),
                                    egui::Sense::hover(),
                                );
                                let rect = response.rect;

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

                                // 3. Render actual PDF page text content (Title, Headings, Paragraphs)
                                if let Some(session) = &self.session {
                                    if let Some(text) = session.get_page_text(page_idx) {
                                        let mut y_offset = rect.top() + 36.0 * self.zoom_level;
                                        let x_margin = rect.left() + 40.0 * self.zoom_level;

                                        for (line_idx, line) in text.lines().enumerate() {
                                            let trimmed = line.trim();
                                            if trimmed.is_empty() {
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
                                                Color32::from_rgb(15, 23, 42) // Deep slate title
                                            } else {
                                                Color32::from_rgb(51, 65, 85) // Slate body text
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

                                ui.label(format!("Page {}", page_num));
                                ui.add_space(16.0);
                            }
                        });
                    });
            }
        });
    }
}

impl eframe::App for KestrelApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.render_ui(ctx);
    }
}
