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
    Layers,
    SearchResults,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FitMode {
    FitWidth,
    FitPage,
}

/// Represents the active selection state for text and embedded images.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SelectionState {
    /// Active selected page index (0-based)
    pub page_index: Option<usize>,
    /// Drag selection start in visual PDF points
    pub drag_start_pt: Option<egui::Pos2>,
    /// Drag selection current in visual PDF points
    pub drag_current_pt: Option<egui::Pos2>,
    /// Indices of selected text runs on `page_index`
    pub selected_text_indices: Vec<usize>,
    /// Extracted text string for the current selection
    pub selected_text: Option<String>,
    /// Index of selected image on `page_index`
    pub selected_image_index: Option<usize>,
}

impl SelectionState {
    /// Resets all selection properties to empty.
    pub fn clear(&mut self) {
        self.page_index = None;
        self.drag_start_pt = None;
        self.drag_current_pt = None;
        self.selected_text_indices.clear();
        self.selected_text = None;
        self.selected_image_index = None;
    }

    /// Returns true if neither text nor image is selected.
    pub fn is_empty(&self) -> bool {
        self.selected_text_indices.is_empty() && self.selected_image_index.is_none()
    }

    /// Returns true if non-empty text is selected.
    pub fn has_text(&self) -> bool {
        self.selected_text
            .as_ref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }

    /// Returns true if an embedded image is selected.
    pub fn has_image(&self) -> bool {
        self.selected_image_index.is_some()
    }
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
    pub image_textures: HashMap<(usize, usize), TextureHandle>,
    pub selection: SelectionState,

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
            image_textures: HashMap::new(),
            selection: SelectionState::default(),

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

/// Truncates a filename in the middle with an ellipsis ('…'), preserving the prefix and extension.
pub fn truncate_filename_middle(name: &str, max_len: usize) -> String {
    let char_count = name.chars().count();
    if char_count <= max_len || max_len < 10 {
        return name.to_string();
    }
    let chars: Vec<char> = name.chars().collect();
    let keep_front = (max_len - 1) / 2;
    let keep_back = max_len - 1 - keep_front;
    let front: String = chars[..keep_front].iter().collect();
    let back: String = chars[chars.len() - keep_back..].iter().collect();
    format!("{}…{}", front, back)
}

/// Returns the platform-appropriate copy shortcut string representation (e.g. "⌘C" or "Ctrl+C / Ctrl+Ins").
#[cfg(target_os = "macos")]
pub fn standard_copy_shortcut_str() -> &'static str {
    "⌘C"
}

/// Returns the platform-appropriate copy shortcut string representation (e.g. "⌘C" or "Ctrl+C / Ctrl+Ins").
#[cfg(not(target_os = "macos"))]
pub fn standard_copy_shortcut_str() -> &'static str {
    "Ctrl+C / Ctrl+Ins"
}

/// Returns the platform-appropriate select-all shortcut string representation (e.g. "⌘A" or "Ctrl+A").
#[cfg(target_os = "macos")]
pub fn standard_select_all_shortcut_str() -> &'static str {
    "⌘A"
}

/// Returns the platform-appropriate select-all shortcut string representation (e.g. "⌘A" or "Ctrl+A").
#[cfg(not(target_os = "macos"))]
pub fn standard_select_all_shortcut_str() -> &'static str {
    "Ctrl+A"
}

/// Determines whether standard copy shortcuts were triggered on the active platform.
/// Supports Cmd+C (macOS), Ctrl+C (Windows/Linux/WASM), Ctrl+Insert (IBM CUA),
/// dedicated hardware Key::Copy, and OS/Browser Event::Copy.
pub fn is_copy_shortcut_pressed(input: &egui::InputState) -> bool {
    // 1. Native OS / browser copy event
    if input.events.iter().any(|e| matches!(e, egui::Event::Copy)) {
        return true;
    }

    // 2. Dedicated hardware multimedia copy key
    if input.key_pressed(egui::Key::Copy) {
        return true;
    }

    // Alt modifier disqualifies standard copy
    if input.modifiers.alt {
        return false;
    }

    // 3. Cmd+C (macOS) or Ctrl+C (Windows / Linux / WASM)
    let is_cmd_or_ctrl = input.modifiers.command || input.modifiers.ctrl || input.modifiers.mac_cmd;
    if is_cmd_or_ctrl && !input.modifiers.shift && input.key_pressed(egui::Key::C) {
        return true;
    }

    // 4. IBM CUA Standard: Ctrl+Insert (Windows / Linux)
    if input.modifiers.ctrl && !input.modifiers.shift && input.key_pressed(egui::Key::Insert) {
        return true;
    }

    false
}

/// Determines whether standard select-all shortcuts were triggered on the active platform (Ctrl+A or Cmd+A).
pub fn is_select_all_shortcut_pressed(input: &egui::InputState) -> bool {
    if input.modifiers.alt || input.modifiers.shift {
        return false;
    }

    let is_cmd_or_ctrl = input.modifiers.command || input.modifiers.ctrl || input.modifiers.mac_cmd;
    is_cmd_or_ctrl && input.key_pressed(egui::Key::A)
}

impl KestrelApp {
    /// Returns the truncated title text for compact UI display.
    pub fn compact_title_text(&self) -> String {
        match &self.current_file_name {
            Some(name) => truncate_filename_middle(name, 48),
            None => "Kestrel-PDF".to_string(),
        }
    }
    /// Loads a PDF from raw byte buffer (desktop or WASM).
    pub fn load_document_bytes(&mut self, bytes: Vec<u8>, name: Option<String>) {
        if let Ok(session) = DocumentSession::open_from_bytes(bytes, None) {
            self.total_pages = session.page_count as usize;
            self.current_page = if self.total_pages > 0 { 1 } else { 0 };
            self.current_file_name = name;
            self.textures.clear();
            self.image_textures.clear();
            self.search_results.clear();
            self.adopted_signature = None;
            self.selection.clear();
            self.status_toast = Some("Document loaded successfully.".to_string());
            self.session = Some(session);
        }
    }

    /// Zooms in by 15% (max 5.0x / 500%).
    pub fn zoom_in(&mut self) {
        self.zoom_level = (self.zoom_level * 1.15).min(5.0);
    }

    /// Zooms out by 15% (min 0.1x / 10%).
    pub fn zoom_out(&mut self) {
        self.zoom_level = (self.zoom_level / 1.15).max(0.1);
    }

    /// Resets zoom to 100% (1.0x).
    pub fn reset_zoom(&mut self) {
        self.zoom_level = 1.0;
    }

    /// Sets explicit zoom level clamped between 0.1 and 5.0.
    pub fn set_zoom(&mut self, level: f32) {
        self.zoom_level = level.clamp(0.1, 5.0);
    }

    /// Rotates the current page 90 degrees clockwise.
    pub fn rotate_current_page_clockwise(&mut self) {
        if let Some(session) = &mut self.session {
            let idx = self.current_page.saturating_sub(1);
            session.rotate_page(idx, true);
            let rot = session
                .pages
                .get(idx)
                .map(|p| p.rotation_degrees)
                .unwrap_or(0);
            self.status_toast = Some(format!("Page {} rotated to {}°", self.current_page, rot));
        }
    }

    /// Toggles the visibility of a document layer by index.
    pub fn toggle_layer(&mut self, index: usize) -> bool {
        if let Some(session) = &mut self.session {
            session.toggle_layer(index)
        } else {
            false
        }
    }

    /// Rotates the current page 90 degrees counter-clockwise.
    pub fn rotate_current_page_counter_clockwise(&mut self) {
        if let Some(session) = &mut self.session {
            let idx = self.current_page.saturating_sub(1);
            session.rotate_page(idx, false);
            let rot = session
                .pages
                .get(idx)
                .map(|p| p.rotation_degrees)
                .unwrap_or(0);
            self.status_toast = Some(format!("Page {} rotated to {}°", self.current_page, rot));
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

    /// Copies active text selection to system clipboard.
    pub fn copy_selected_text(&mut self, ctx: &Context) -> bool {
        if let Some(text) = &self.selection.selected_text {
            let count = text.chars().count();
            ctx.copy_text(text.clone());

            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Ok(mut cb) = arboard::Clipboard::new() {
                    let _ = cb.set_text(text.clone());
                }
            }

            self.status_toast = Some(format!("Copied {} characters to clipboard", count));
            true
        } else {
            false
        }
    }

    /// Copies active image selection to system clipboard.
    pub fn copy_selected_image(&mut self, ctx: &Context) -> bool {
        if let (Some(page_idx), Some(img_idx)) = (
            self.selection.page_index,
            self.selection.selected_image_index,
        ) {
            if let Some(session) = &self.session {
                if let Some(layout) = session.get_page_layout(page_idx) {
                    if let Some(img) = layout.images.get(img_idx) {
                        let w = img.pixel_width;
                        let h = img.pixel_height;

                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            if let Ok(mut cb) = arboard::Clipboard::new() {
                                let _ = cb.set_image(arboard::ImageData {
                                    width: w as usize,
                                    height: h as usize,
                                    bytes: std::borrow::Cow::Borrowed(&img.rgba),
                                });
                            }
                        }

                        if let Ok(png_bytes) = layout.encode_image_png(img_idx) {
                            ctx.copy_text(format!(
                                "[Embedded Image: {}×{} px, PNG {} bytes]",
                                w,
                                h,
                                png_bytes.len()
                            ));
                        } else {
                            ctx.copy_text(format!("[Embedded Image: {}×{} px]", w, h));
                        }

                        self.status_toast =
                            Some(format!("Copied image ({}×{} px) to clipboard", w, h));
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Clears any active text or image selection.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
    }

    /// Selects all text runs on the currently active page.
    pub fn select_all_current_page(&mut self) {
        if self.total_pages == 0 {
            return;
        }
        let page_idx = self
            .selection
            .page_index
            .unwrap_or_else(|| self.current_page.saturating_sub(1));

        if let Some(session) = &self.session {
            if let Some(layout) = session.get_page_layout(page_idx) {
                if layout.text_runs.is_empty() {
                    return;
                }
                self.selection.clear();
                self.selection.page_index = Some(page_idx);
                self.selection.selected_text_indices = (0..layout.text_runs.len()).collect();

                let all_text = if !layout.plain_text.trim().is_empty() {
                    layout.plain_text.clone()
                } else {
                    layout
                        .text_runs
                        .iter()
                        .map(|tr| tr.text.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                };

                if !all_text.is_empty() {
                    self.selection.selected_text = Some(all_text);
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

        // Handle global keyboard shortcuts when text input fields (search bar, text forms) do NOT have focus
        let text_edit_focused = ctx.wants_keyboard_input();
        if !text_edit_focused {
            if ctx.input(is_copy_shortcut_pressed) {
                if self.selection.has_text() {
                    self.copy_selected_text(ctx);
                } else if self.selection.has_image() {
                    self.copy_selected_image(ctx);
                }
            } else if self.active_tool == ActiveTool::SelectText
                && ctx.input(is_select_all_shortcut_pressed)
            {
                self.select_all_current_page();
            } else if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.clear_selection();
            }
        }

        // 1. Tier 1: Application Header Bar (Branding, Sidebar Toggle, Document Title, File Operations)
        egui::TopBottomPanel::top("app_header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Left: Sidebar Toggle & Branding
                let sidebar_label = if self.sidebar_open {
                    "◀ Sidebar"
                } else {
                    "▶ Sidebar"
                };
                if ui
                    .button(sidebar_label)
                    .on_hover_text(
                        "Toggle document sidebar (Thumbnails, Outlines, Forms, Layers, Search)",
                    )
                    .clicked()
                {
                    self.sidebar_open = !self.sidebar_open;
                }

                ui.label(egui::RichText::new("🦅 Kestrel-PDF").strong());
                ui.separator();

                // Right: File Action Buttons
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.session.is_some()
                        && ui
                            .button("💾 Save / Export")
                            .on_hover_text("Save modified PDF to disk")
                            .clicked()
                    {
                        self.save_document();
                    }

                    if ui
                        .button("📂 Open File")
                        .on_hover_text("Open PDF document from disk")
                        .clicked()
                    {
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

                    ui.separator();

                    // Center: Document Title Heading (with middle truncation if long and rich tooltip)
                    if let Some(name) = &self.current_file_name {
                        let display_title = truncate_filename_middle(name, 48);
                        ui.heading(display_title).on_hover_text(format!(
                            "Document: {}\nTotal Pages: {}",
                            name, self.total_pages
                        ));
                    } else {
                        ui.heading("No document open");
                    }
                });
            });
        });

        // 2. Tier 2: Document Action & Navigation Ribbon
        egui::TopBottomPanel::top("action_toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Group 1: Page Navigation & Orientation
                if self.total_pages > 0 {
                    if ui.button("⬅ Prev").on_hover_text("Previous Page").clicked()
                        && self.current_page > 1
                    {
                        self.current_page -= 1;
                    }
                    ui.label(format!("Page {} / {}", self.current_page, self.total_pages))
                        .on_hover_text("Current page / Total pages");
                    if ui.button("Next ➡").on_hover_text("Next Page").clicked()
                        && self.current_page < self.total_pages
                    {
                        self.current_page += 1;
                    }
                } else {
                    ui.label("Page 0 / 0");
                }

                ui.separator();

                if ui
                    .button("⟲")
                    .on_hover_text("Rotate Counter-Clockwise (90°)")
                    .clicked()
                {
                    self.rotate_current_page_counter_clockwise();
                }
                if ui
                    .button("⟳")
                    .on_hover_text("Rotate Clockwise (90°)")
                    .clicked()
                {
                    self.rotate_current_page_clockwise();
                }

                ui.separator();

                // Group 2: Segmented Tool Mode Selector (Pill style)
                ui.selectable_value(&mut self.active_tool, ActiveTool::Pan, "✋ Pan")
                    .on_hover_text("Pan & scroll through document");
                ui.selectable_value(&mut self.active_tool, ActiveTool::SelectText, "📝 Select")
                    .on_hover_text(format!(
                        "Select and copy text ({}) or select all ({})",
                        standard_copy_shortcut_str(),
                        standard_select_all_shortcut_str()
                    ));

                let form_count = self.session.as_ref().map(|s| s.forms.len()).unwrap_or(0);
                let form_label = if form_count > 0 {
                    format!("📋 Forms ({})", form_count)
                } else {
                    "📋 Forms".to_string()
                };
                ui.selectable_value(&mut self.active_tool, ActiveTool::FormFill, form_label)
                    .on_hover_text("Fill interactive form fields and checkboxes");

                ui.selectable_value(&mut self.active_tool, ActiveTool::EditText, "✏️ Edit Text")
                    .on_hover_text("Add or edit text annotations");

                if ui
                    .selectable_value(
                        &mut self.active_tool,
                        ActiveTool::SignContract,
                        "✍️ Sign Contract",
                    )
                    .on_hover_text("Sign contract with drawn or digital signature")
                    .clicked()
                    && self.adopted_signature.is_none()
                {
                    self.signature_modal_open = true;
                }

                if self.active_tool == ActiveTool::SignContract
                    && ui
                        .button("🖊 Create Signature")
                        .on_hover_text("Open Signature Pad")
                        .clicked()
                {
                    self.signature_modal_open = true;
                }

                ui.selectable_value(&mut self.active_tool, ActiveTool::RedactData, "🛡️ Redact")
                    .on_hover_text("Permanently redact sensitive document data");

                // Dynamic Selection Actions
                if !self.selection.is_empty() {
                    ui.separator();
                    if self.selection.has_text() {
                        let count = self
                            .selection
                            .selected_text
                            .as_ref()
                            .map(|s| s.chars().count())
                            .unwrap_or(0);
                        if ui
                            .button(format!("📋 Copy Text ({} chars)", count))
                            .on_hover_text(format!(
                                "Copy selected text to clipboard ({})",
                                standard_copy_shortcut_str()
                            ))
                            .clicked()
                        {
                            self.copy_selected_text(ctx);
                        }
                    }
                    if self.selection.has_image()
                        && ui
                            .button("📋 Copy Image")
                            .on_hover_text(format!(
                                "Copy selected image to clipboard ({})",
                                standard_copy_shortcut_str()
                            ))
                            .clicked()
                    {
                        self.copy_selected_image(ctx);
                    }
                    if ui
                        .button("❌ Clear")
                        .on_hover_text("Clear active selection (Escape)")
                        .clicked()
                    {
                        self.clear_selection();
                    }
                }

                // Group 3 & 4: Zoom Controls & Live Search (Right aligned)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("➕").on_hover_text("Zoom In (+15%)").clicked() {
                        self.zoom_in();
                    }
                    ui.label(format!("{:.0}%", self.zoom_level * 100.0))
                        .on_hover_text("Current zoom percentage");
                    if ui.button("➖").on_hover_text("Zoom Out (-15%)").clicked() {
                        self.zoom_out();
                    }
                    if ui
                        .button("Fit Page")
                        .on_hover_text("Fit entire page to viewport")
                        .clicked()
                    {
                        self.pending_fit = Some(FitMode::FitPage);
                    }
                    if ui
                        .button("Fit Width")
                        .on_hover_text("Fit document width to viewport")
                        .clicked()
                    {
                        self.pending_fit = Some(FitMode::FitWidth);
                    }
                    if ui
                        .button("Reset")
                        .on_hover_text("Reset zoom to 100%")
                        .clicked()
                    {
                        self.reset_zoom();
                    }

                    ui.separator();

                    // Search Bar
                    if ui.button("Find").on_hover_text("Execute search").clicked() {
                        self.execute_search();
                    }
                    let search_resp = ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Search…")
                            .desired_width(110.0),
                    );
                    if search_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.execute_search();
                    }
                    ui.label("🔍");
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
                        ui.selectable_value(&mut self.sidebar_tab, SidebarTab::Layers, "Layers");
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
                        SidebarTab::Layers => {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                if let Some(session) = &mut self.session {
                                    if session.layers.is_empty() {
                                        ui.label("No Optional Content Groups (Layers) detected.");
                                    } else {
                                        ui.heading("Document Layers");
                                        ui.add_space(4.0);
                                        for layer in &mut session.layers {
                                            ui.checkbox(&mut layer.visible, &layer.name);
                                        }
                                    }
                                } else {
                                    ui.label("Open a document to view layers.");
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
                        .map(|p| p.visual_dimensions())
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
                                let (page_w, page_h, page_rot) =
                                    if let Some(session) = &self.session {
                                        if let Some(p) = session.pages.get(page_idx) {
                                            let (vw, vh) = p.visual_dimensions();
                                            (vw, vh, p.rotation_degrees)
                                        } else {
                                            (595.28, 841.89, 0)
                                        }
                                    } else {
                                        (595.28, 841.89, 0)
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
                                    || self.active_tool == ActiveTool::SelectText
                                {
                                    egui::Sense::click_and_drag()
                                } else {
                                    egui::Sense::hover()
                                };
                                let (response, painter) =
                                    ui.allocate_painter(Vec2::new(base_width, base_height), sense);
                                let rect = response.rect;

                                // Handle selection operations in SelectText mode
                                if self.active_tool == ActiveTool::SelectText {
                                    if response.drag_started() {
                                        if let Some(pos) = response.interact_pointer_pos() {
                                            let vx = (pos.x - rect.left()) / self.zoom_level;
                                            let vy = (pos.y - rect.top()) / self.zoom_level;
                                            self.selection.clear();
                                            self.selection.page_index = Some(page_idx);
                                            self.selection.drag_start_pt = Some(egui::pos2(vx, vy));
                                            self.selection.drag_current_pt =
                                                Some(egui::pos2(vx, vy));

                                            if let Some(session) = &self.session {
                                                if let Some(layout) =
                                                    session.get_page_layout(page_idx)
                                                {
                                                    if let Some((img_idx, _)) =
                                                        layout.find_image_at_point(vx, vy, page_rot)
                                                    {
                                                        self.selection.selected_image_index =
                                                            Some(img_idx);
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    if response.dragged()
                                        && self.selection.page_index == Some(page_idx)
                                    {
                                        if let Some(pos) = response.interact_pointer_pos() {
                                            let vx = (pos.x - rect.left()) / self.zoom_level;
                                            let vy = (pos.y - rect.top()) / self.zoom_level;
                                            self.selection.drag_current_pt =
                                                Some(egui::pos2(vx, vy));

                                            if let Some(start) = self.selection.drag_start_pt {
                                                let q_rect = [start.x, start.y, vx, vy];
                                                if let Some(session) = &self.session {
                                                    if let Some(layout) =
                                                        session.get_page_layout(page_idx)
                                                    {
                                                        let matched_runs = layout
                                                            .find_text_runs_in_rect(
                                                                q_rect, page_rot,
                                                            );
                                                        self.selection.selected_text_indices =
                                                            matched_runs
                                                                .iter()
                                                                .map(|(idx, _)| *idx)
                                                                .collect();
                                                        let text = layout
                                                            .get_text_in_rect(q_rect, page_rot);
                                                        self.selection.selected_text =
                                                            if !text.is_empty() {
                                                                Some(text)
                                                            } else {
                                                                None
                                                            };

                                                        if self
                                                            .selection
                                                            .selected_text_indices
                                                            .is_empty()
                                                        {
                                                            let matched_imgs = layout
                                                                .find_images_in_rect(
                                                                    q_rect, page_rot,
                                                                );
                                                            self.selection.selected_image_index =
                                                                matched_imgs
                                                                    .first()
                                                                    .map(|(idx, _)| *idx);
                                                        } else {
                                                            self.selection.selected_image_index =
                                                                None;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    if response.clicked() {
                                        if let Some(pos) = response.interact_pointer_pos() {
                                            let vx = (pos.x - rect.left()) / self.zoom_level;
                                            let vy = (pos.y - rect.top()) / self.zoom_level;
                                            self.selection.clear();
                                            self.selection.page_index = Some(page_idx);

                                            if let Some(session) = &self.session {
                                                if let Some(layout) =
                                                    session.get_page_layout(page_idx)
                                                {
                                                    if let Some((img_idx, _)) =
                                                        layout.find_image_at_point(vx, vy, page_rot)
                                                    {
                                                        self.selection.selected_image_index =
                                                            Some(img_idx);
                                                    } else {
                                                        let text_hits = layout
                                                            .find_text_runs_in_rect(
                                                                [
                                                                    vx - 6.0,
                                                                    vy - 6.0,
                                                                    vx + 6.0,
                                                                    vy + 6.0,
                                                                ],
                                                                page_rot,
                                                            );
                                                        if let Some((tr_idx, tr)) =
                                                            text_hits.first()
                                                        {
                                                            self.selection.selected_text_indices =
                                                                vec![*tr_idx];
                                                            self.selection.selected_text =
                                                                Some(tr.text.clone());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Context menu for rapid clipboard copy
                                    response.context_menu(|ui| {
                                        if self.selection.has_text()
                                            && ui
                                                .button(format!(
                                                    "📋 Copy Text ({})",
                                                    standard_copy_shortcut_str()
                                                ))
                                                .clicked()
                                        {
                                            self.copy_selected_text(ctx);
                                            ui.close_menu();
                                        }
                                        if self.selection.has_image()
                                            && ui
                                                .button(format!(
                                                    "📋 Copy Image ({})",
                                                    standard_copy_shortcut_str()
                                                ))
                                                .clicked()
                                        {
                                            self.copy_selected_image(ctx);
                                            ui.close_menu();
                                        }
                                        if ui.button("❌ Clear Selection (Esc)").clicked() {
                                            self.clear_selection();
                                            ui.close_menu();
                                        }
                                    });
                                }

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

                                // 3. Draw Embedded Raster Images
                                if let Some(session) = &self.session {
                                    if let Some(layout) = session.get_page_layout(page_idx) {
                                        for (img_idx, img) in layout.images.iter().enumerate() {
                                            let t_key = (page_idx, img_idx);
                                            let texture = self
                                                .image_textures
                                                .entry(t_key)
                                                .or_insert_with(|| {
                                                    let color_image =
                                                        ColorImage::from_rgba_unmultiplied(
                                                            [
                                                                img.pixel_width as usize,
                                                                img.pixel_height as usize,
                                                            ],
                                                            &img.rgba,
                                                        );
                                                    ctx.load_texture(
                                                        format!(
                                                            "page_img_{}_{}",
                                                            page_idx, img_idx
                                                        ),
                                                        color_image,
                                                        TextureOptions::LINEAR,
                                                    )
                                                });

                                            let (vx, vy) =
                                                kestrel_core::document::map_pdf_point_to_visual(
                                                    img.x,
                                                    img.y + img.height,
                                                    layout.width_pt,
                                                    layout.height_pt,
                                                    page_rot,
                                                );
                                            let (iw, ih) = if page_rot % 180 == 90 {
                                                (
                                                    img.height * self.zoom_level,
                                                    img.width * self.zoom_level,
                                                )
                                            } else {
                                                (
                                                    img.width * self.zoom_level,
                                                    img.height * self.zoom_level,
                                                )
                                            };

                                            let img_rect = egui::Rect::from_min_size(
                                                egui::pos2(
                                                    rect.left() + vx * self.zoom_level,
                                                    rect.top() + vy * self.zoom_level,
                                                ),
                                                Vec2::new(iw.abs(), ih.abs()),
                                            );

                                            painter.image(
                                                texture.id(),
                                                img_rect,
                                                egui::Rect::from_min_max(
                                                    egui::pos2(0.0, 0.0),
                                                    egui::pos2(1.0, 1.0),
                                                ),
                                                Color32::WHITE,
                                            );
                                        }
                                    }
                                }

                                // 4. Draw Vector Rectangles (borders, table grid cells, headers)
                                if let Some(session) = &self.session {
                                    if let Some(layout) = session.get_page_layout(page_idx) {
                                        for r in &layout.rects {
                                            let (vx, vy) =
                                                kestrel_core::document::map_pdf_point_to_visual(
                                                    r.x,
                                                    r.y + r.height,
                                                    layout.width_pt,
                                                    layout.height_pt,
                                                    page_rot,
                                                );
                                            let (rw, rh) = if page_rot % 180 == 90 {
                                                (
                                                    r.height * self.zoom_level,
                                                    r.width * self.zoom_level,
                                                )
                                            } else {
                                                (
                                                    r.width * self.zoom_level,
                                                    r.height * self.zoom_level,
                                                )
                                            };
                                            let r_rect = egui::Rect::from_min_size(
                                                egui::pos2(
                                                    rect.left() + vx * self.zoom_level,
                                                    rect.top() + vy * self.zoom_level,
                                                ),
                                                Vec2::new(rw.abs(), rh.abs()),
                                            );

                                            if let Some(fill) = r.fill_color {
                                                // Avoid repainting whole page white background
                                                if !(fill == [255, 255, 255]
                                                    && rw.abs() >= base_width * 0.98
                                                    && rh.abs() >= base_height * 0.98)
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

                                // 5. Render PDF Positioned Text Content & Search Highlighting
                                if let Some(session) = &self.session {
                                    if let Some(layout) = session.get_page_layout(page_idx) {
                                        if !layout.text_runs.is_empty() {
                                            for tr in &layout.text_runs {
                                                let font_size = (tr.font_size * self.zoom_level)
                                                    .clamp(6.0, 72.0);
                                                let rad = tr.rotation_deg.to_radians();
                                                let h = tr.font_size * 0.85;
                                                let top_x = tr.x - h * rad.sin();
                                                let top_y = tr.y + h * rad.cos();
                                                let (vx, vy) =
                                                    kestrel_core::document::map_pdf_point_to_visual(
                                                        top_x,
                                                        top_y,
                                                        layout.width_pt,
                                                        layout.height_pt,
                                                        page_rot,
                                                    );
                                                let t_x = rect.left() + vx * self.zoom_level;
                                                let t_y = rect.top() + vy * self.zoom_level;

                                                // Live search visual highlight
                                                if !self.search_query.trim().is_empty()
                                                    && tr
                                                        .text
                                                        .to_lowercase()
                                                        .contains(&self.search_query.to_lowercase())
                                                {
                                                    let vb =
                                                        layout.text_run_visual_bounds(tr, page_rot);
                                                    let hl_rect = egui::Rect::from_min_max(
                                                        egui::pos2(
                                                            rect.left() + vb[0] * self.zoom_level
                                                                - 2.0,
                                                            rect.top() + vb[1] * self.zoom_level
                                                                - 1.0,
                                                        ),
                                                        egui::pos2(
                                                            rect.left()
                                                                + vb[2] * self.zoom_level
                                                                + 2.0,
                                                            rect.top()
                                                                + vb[3] * self.zoom_level
                                                                + 1.0,
                                                        ),
                                                    );
                                                    painter.rect_filled(
                                                        hl_rect,
                                                        2.0,
                                                        Color32::from_rgba_unmultiplied(
                                                            255, 235, 59, 140,
                                                        ),
                                                    );
                                                }

                                                let font_id = egui::FontId::proportional(font_size);
                                                let color = Color32::from_rgb(
                                                    tr.color[0],
                                                    tr.color[1],
                                                    tr.color[2],
                                                );

                                                let eff_rot_deg = (tr.rotation_deg
                                                    - page_rot as f32)
                                                    .rem_euclid(360.0);
                                                if eff_rot_deg.abs() < 1.0
                                                    || (eff_rot_deg - 360.0).abs() < 1.0
                                                {
                                                    painter.text(
                                                        egui::pos2(t_x, t_y),
                                                        egui::Align2::LEFT_TOP,
                                                        &tr.text,
                                                        font_id,
                                                        color,
                                                    );
                                                } else {
                                                    let galley = painter.layout_no_wrap(
                                                        tr.text.clone(),
                                                        font_id,
                                                        color,
                                                    );
                                                    let angle_rad = -eff_rot_deg.to_radians();
                                                    painter.add(
                                                        egui::epaint::TextShape::new(
                                                            egui::pos2(t_x, t_y),
                                                            galley,
                                                            color,
                                                        )
                                                        .with_angle(angle_rad),
                                                    );
                                                }
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

                                // 6. Interactive AcroForm Widgets on Page
                                if let Some(session) = &mut self.session {
                                    let (orig_w, orig_h) = session
                                        .pages
                                        .get(page_idx)
                                        .map(|p| (p.width_pt, p.height_pt))
                                        .unwrap_or((595.28, 841.89));

                                    for field in &mut session.forms {
                                        if field.page_index == page_idx as u16 {
                                            let (vx, vy) =
                                                kestrel_core::document::map_pdf_point_to_visual(
                                                    field.rect[0],
                                                    field.rect[3],
                                                    orig_w,
                                                    orig_h,
                                                    page_rot,
                                                );
                                            let fw = (field.rect[2] - field.rect[0]).abs();
                                            let fh = (field.rect[3] - field.rect[1]).abs();
                                            let (f_w, f_h) = if page_rot % 180 == 90 {
                                                (fh * self.zoom_level, fw * self.zoom_level)
                                            } else {
                                                (fw * self.zoom_level, fh * self.zoom_level)
                                            };
                                            let f_x = rect.left() + vx * self.zoom_level;
                                            let f_y = rect.top() + vy * self.zoom_level;
                                            let field_rect = egui::Rect::from_min_size(
                                                egui::pos2(f_x, f_y),
                                                Vec2::new(f_w.max(1.0), f_h.max(1.0)),
                                            );

                                            // In FormFill mode, draw subtle translucent tint and border to indicate editable field
                                            // without occluding underlying text, guidelines, or form outlines.
                                            if self.active_tool == ActiveTool::FormFill {
                                                painter.rect_filled(
                                                    field_rect,
                                                    1.0,
                                                    Color32::from_rgba_unmultiplied(
                                                        219, 234, 254, 45,
                                                    ),
                                                );
                                                painter.rect_stroke(
                                                    field_rect,
                                                    1.0,
                                                    egui::Stroke::new(
                                                        (0.8 * self.zoom_level).clamp(0.5, 2.0),
                                                        Color32::from_rgba_unmultiplied(
                                                            96, 165, 250, 160,
                                                        ),
                                                    ),
                                                );
                                            }

                                            // Draw field value with font scaled to field height
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

                                            if !display_val.is_empty() {
                                                let font_size = (f_h * 0.82).clamp(7.0, 32.0);
                                                let pad_x = (2.0 * self.zoom_level).clamp(1.0, 4.0);
                                                painter.text(
                                                    egui::pos2(
                                                        field_rect.left() + pad_x,
                                                        field_rect.center().y,
                                                    ),
                                                    egui::Align2::LEFT_CENTER,
                                                    &display_val,
                                                    egui::FontId::proportional(font_size),
                                                    Color32::from_rgb(15, 23, 42),
                                                );
                                            }
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

                                // 6. Selection Highlights & Visual Overlays
                                if self.selection.page_index == Some(page_idx) {
                                    if let Some(session) = &self.session {
                                        if let Some(layout) = session.get_page_layout(page_idx) {
                                            // Text Selection Highlights
                                            for &tr_idx in &self.selection.selected_text_indices {
                                                if let Some(tr) = layout.text_runs.get(tr_idx) {
                                                    let b =
                                                        layout.text_run_visual_bounds(tr, page_rot);
                                                    let tr_rect = egui::Rect::from_min_max(
                                                        rect.left_top()
                                                            + egui::vec2(b[0], b[1])
                                                                * self.zoom_level,
                                                        rect.left_top()
                                                            + egui::vec2(b[2], b[3])
                                                                * self.zoom_level,
                                                    );
                                                    painter.rect_filled(
                                                        tr_rect.expand(1.5),
                                                        2.0,
                                                        Color32::from_rgba_unmultiplied(
                                                            59, 130, 246, 85,
                                                        ),
                                                    );
                                                    painter.rect_stroke(
                                                        tr_rect.expand(1.5),
                                                        2.0,
                                                        egui::Stroke::new(
                                                            1.0_f32,
                                                            Color32::from_rgba_unmultiplied(
                                                                37, 99, 235, 180,
                                                            ),
                                                        ),
                                                    );
                                                }
                                            }

                                            // Image Selection Outline, Corner Handles, and Badge
                                            if let Some(img_idx) =
                                                self.selection.selected_image_index
                                            {
                                                if let Some(img) = layout.images.get(img_idx) {
                                                    let b =
                                                        layout.image_visual_bounds(img, page_rot);
                                                    let img_rect = egui::Rect::from_min_max(
                                                        rect.left_top()
                                                            + egui::vec2(b[0], b[1])
                                                                * self.zoom_level,
                                                        rect.left_top()
                                                            + egui::vec2(b[2], b[3])
                                                                * self.zoom_level,
                                                    );
                                                    painter.rect_stroke(
                                                        img_rect,
                                                        2.0,
                                                        egui::Stroke::new(
                                                            2.5_f32,
                                                            Color32::from_rgb(37, 99, 235),
                                                        ),
                                                    );
                                                    for corner in [
                                                        img_rect.left_top(),
                                                        img_rect.right_top(),
                                                        img_rect.left_bottom(),
                                                        img_rect.right_bottom(),
                                                    ] {
                                                        let handle = egui::Rect::from_center_size(
                                                            corner,
                                                            Vec2::splat(8.0),
                                                        );
                                                        painter.rect_filled(
                                                            handle,
                                                            1.0,
                                                            Color32::WHITE,
                                                        );
                                                        painter.rect_stroke(
                                                            handle,
                                                            1.0,
                                                            egui::Stroke::new(
                                                                1.5_f32,
                                                                Color32::from_rgb(37, 99, 235),
                                                            ),
                                                        );
                                                    }
                                                    let badge_rect = egui::Rect::from_min_size(
                                                        egui::pos2(
                                                            img_rect.left(),
                                                            (img_rect.top() - 22.0).max(rect.top()),
                                                        ),
                                                        Vec2::new(135.0, 18.0),
                                                    );
                                                    painter.rect_filled(
                                                        badge_rect,
                                                        3.0,
                                                        Color32::from_rgb(37, 99, 235),
                                                    );
                                                    painter.text(
                                                        badge_rect.center(),
                                                        egui::Align2::CENTER_CENTER,
                                                        format!(
                                                            "🖼️ Image ({}×{} px)",
                                                            img.pixel_width, img.pixel_height
                                                        ),
                                                        egui::FontId::proportional(11.0),
                                                        Color32::WHITE,
                                                    );
                                                }
                                            }
                                        }
                                    }

                                    // Drag Marquee Box
                                    if response.dragged() {
                                        if let (Some(start), Some(curr)) = (
                                            self.selection.drag_start_pt,
                                            self.selection.drag_current_pt,
                                        ) {
                                            let p1 = rect.left_top()
                                                + egui::vec2(start.x, start.y) * self.zoom_level;
                                            let p2 = rect.left_top()
                                                + egui::vec2(curr.x, curr.y) * self.zoom_level;
                                            let marquee_rect = egui::Rect::from_two_pos(p1, p2);
                                            if marquee_rect.width() > 3.0
                                                || marquee_rect.height() > 3.0
                                            {
                                                painter.rect_filled(
                                                    marquee_rect,
                                                    2.0,
                                                    Color32::from_rgba_unmultiplied(
                                                        59, 130, 246, 35,
                                                    ),
                                                );
                                                painter.rect_stroke(
                                                    marquee_rect,
                                                    2.0,
                                                    egui::Stroke::new(
                                                        1.2_f32,
                                                        Color32::from_rgba_unmultiplied(
                                                            37, 99, 235, 160,
                                                        ),
                                                    ),
                                                );
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
