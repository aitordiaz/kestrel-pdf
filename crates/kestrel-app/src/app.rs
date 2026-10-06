use kestrel_core::render::TileCache;
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

pub struct KestrelApp {
    pub current_file_name: Option<String>,
    pub current_page: usize,
    pub total_pages: usize,
    pub zoom_level: f32,
    pub active_tool: ActiveTool,
    pub sidebar_open: bool,
    pub tile_cache: Arc<TileCache>,
}

impl Default for KestrelApp {
    fn default() -> Self {
        Self {
            current_file_name: None,
            current_page: 1,
            total_pages: 0,
            zoom_level: 1.0,
            active_tool: ActiveTool::Pan,
            sidebar_open: true,
            tile_cache: Arc::new(TileCache::new(128)),
        }
    }
}

impl eframe::App for KestrelApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top Toolbar
        egui::TopBottomPanel::top("top_toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🦅 Kestrel-PDF");
                ui.separator();

                if ui.button("📂 Open File").clicked() {
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PDF Documents", &["pdf"])
                        .pick_file()
                    {
                        self.current_file_name = Some(path.file_name().unwrap().to_string_lossy().to_string());
                        self.total_pages = 1;
                        self.current_page = 1;
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

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Zoom controls
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
                });
            });
        });

        // Left Sidebar: Thumbnails and Outlines
        if self.sidebar_open {
            egui::SidePanel::left("left_sidebar")
                .resizable(true)
                .default_width(220.0)
                .show(ctx, |ui| {
                    ui.heading("Navigation");
                    ui.separator();
                    if self.total_pages == 0 {
                        ui.label("No document loaded.");
                    } else {
                        ui.label(format!("Page {} of {}", self.current_page, self.total_pages));
                    }
                });
        }

        // Central Viewport: Rendered Page Canvas
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.total_pages == 0 {
                ui.centered_and_justified(|ui| {
                    ui.label("Drag & drop a PDF file here or click 'Open File' to begin.");
                });
            } else {
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        // Render viewport area
                        ui.label(format!("Viewing: {:?} at {:.0}% zoom", self.current_file_name, self.zoom_level * 100.0));
                    });
            }
        });
    }
}
