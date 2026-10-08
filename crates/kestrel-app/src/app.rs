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

    /// Selects an embedded image on the specified page.
    pub fn select_image(&mut self, page_index: usize, image_index: usize) {
        self.clear();
        self.page_index = Some(page_index);
        self.selected_image_index = Some(image_index);
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
    pub page_input_text: String,
    pub page_input_has_focus: bool,
    pub selected_search_result: Option<usize>,
    pub scroll_to_page: Option<usize>,
    pub scroll_to_search_match: bool,

    // Phase 3: In-Place Text & Image Editing State
    pub editing_text_modal_open: bool,
    pub editing_text_page: usize,
    pub editing_text_run_index: Option<usize>,
    pub editing_text_buffer: String,
    pub editing_text_pos: egui::Pos2,
    pub editing_text_size: f32,
    pub editing_text_color: [u8; 3],
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
            sidebar_open: false,
            sidebar_tab: SidebarTab::Outlines,
            search_query: String::new(),
            search_results: Vec::new(),
            selected_search_result: None,
            scroll_to_page: None,
            scroll_to_search_match: false,
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
            page_input_text: "1".to_string(),
            page_input_has_focus: false,

            // Phase 3 editing defaults
            editing_text_modal_open: false,
            editing_text_page: 0,
            editing_text_run_index: None,
            editing_text_buffer: String::new(),
            editing_text_pos: egui::pos2(100.0, 500.0),
            editing_text_size: 14.0,
            editing_text_color: [15, 23, 42],
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

/// Downsamples an RGBA image buffer if either dimension exceeds `max_side` to prevent GPU texture dimension overflow panics.
/// If already within bounds, returns a borrowed slice with zero allocations.
pub fn clamp_rgba_image_to_max_side<'a>(
    width: usize,
    height: usize,
    rgba: &'a [u8],
    max_side: usize,
) -> (usize, usize, std::borrow::Cow<'a, [u8]>) {
    if (width <= max_side && height <= max_side) || width == 0 || height == 0 || max_side == 0 {
        return (width, height, std::borrow::Cow::Borrowed(rgba));
    }

    let max_dim = width.max(height);
    let scale = (max_side as f32 / max_dim as f32).min(1.0);
    let new_w = ((width as f32 * scale).round() as usize).clamp(1, max_side);
    let new_h = ((height as f32 * scale).round() as usize).clamp(1, max_side);

    let mut scaled = Vec::with_capacity(new_w * new_h * 4);
    for y in 0..new_h {
        let src_y = ((y as f32 / scale).floor() as usize).min(height.saturating_sub(1));
        for x in 0..new_w {
            let src_x = ((x as f32 / scale).floor() as usize).min(width.saturating_sub(1));
            let idx = (src_y * width + src_x) * 4;
            if idx + 4 <= rgba.len() {
                scaled.extend_from_slice(&rgba[idx..idx + 4]);
            } else {
                scaled.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }
    (new_w, new_h, std::borrow::Cow::Owned(scaled))
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

/// Robust parser for user-entered page numbers.
/// Handles formats like "3", "3/4", "3 / 4", "3 of 4", "3 de 4", "p3", "p.3", "page 3", etc.
pub fn parse_page_number(input: &str) -> Option<usize> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }

    // Split by '/', '\\', " of ", or " de " to isolate the numerator/target page
    let page_str = if let Some((first, _)) = s.split_once('/') {
        first.trim()
    } else if let Some((first, _)) = s.split_once('\\') {
        first.trim()
    } else if let Some((first, _)) = s.split_once(" of ") {
        first.trim()
    } else if let Some((first, _)) = s.split_once(" de ") {
        first.trim()
    } else {
        s
    };

    // Strip non-digit leading characters like "page", "pag", "p.", "p", "#"
    let page_str = page_str
        .trim_start_matches(|c: char| {
            c.is_alphabetic() || c == '.' || c == ' ' || c == '#' || c == ':'
        })
        .trim();

    if let Ok(val) = page_str.parse::<usize>() {
        return Some(val);
    }

    // Fallback: extract the first contiguous sequence of ASCII digits
    let digits: String = page_str
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();

    if !digits.is_empty() {
        if let Ok(val) = digits.parse::<usize>() {
            return Some(val);
        }
    }

    None
}

impl KestrelApp {
    /// Returns the truncated title text for compact UI display.
    pub fn compact_title_text(&self) -> String {
        match &self.current_file_name {
            Some(name) => truncate_filename_middle(name, 48),
            None => "Kestrel-PDF".to_string(),
        }
    }
    /// Safely updates the current page index, synchronizes the input buffer, and queues programmatic viewport scroll.
    pub fn set_current_page(&mut self, page: usize) {
        if self.total_pages > 0 {
            self.current_page = page.clamp(1, self.total_pages);
        } else {
            self.current_page = 0;
        }
        self.page_input_text = if self.current_page > 0 {
            self.current_page.to_string()
        } else {
            "0".to_string()
        };
        if self.current_page > 0 {
            self.scroll_to_page = Some(self.current_page - 1);
        }
    }

    /// Loads a PDF from raw byte buffer (desktop or WASM).
    pub fn load_document_bytes(&mut self, bytes: Vec<u8>, name: Option<String>) {
        if let Ok(session) = DocumentSession::open_from_bytes(bytes, None) {
            self.total_pages = session.page_count as usize;
            self.set_current_page(if self.total_pages > 0 { 1 } else { 0 });
            self.current_file_name = name;
            self.textures.clear();
            self.image_textures.clear();
            self.search_results.clear();
            self.selected_search_result = None;
            self.scroll_to_page = None;
            self.scroll_to_search_match = false;
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
                self.sidebar_open = true;
                self.navigate_to_search_result(0);
            } else {
                self.selected_search_result = None;
                self.status_toast = Some("No search matches found".to_string());
            }
        }
    }

    /// Navigates to a specific search result by index, synchronizing page indicator and scheduling viewport scroll.
    pub fn navigate_to_search_result(&mut self, idx: usize) {
        if let Some(res) = self.search_results.get(idx).cloned() {
            self.selected_search_result = Some(idx);
            self.set_current_page(res.page_index as usize + 1);
            self.scroll_to_page = Some(res.page_index as usize);
            self.scroll_to_search_match = true;
            self.status_toast = Some(format!(
                "Navigated to search match on Page {}",
                res.page_index + 1
            ));
        }
    }

    /// Navigates to the next search result in circular order.
    pub fn next_search_result(&mut self) {
        if self.search_results.is_empty() {
            return;
        }
        let next_idx = match self.selected_search_result {
            Some(idx) => (idx + 1) % self.search_results.len(),
            None => 0,
        };
        self.navigate_to_search_result(next_idx);
    }

    /// Navigates to the previous search result in circular order.
    pub fn prev_search_result(&mut self) {
        if self.search_results.is_empty() {
            return;
        }
        let prev_idx = match self.selected_search_result {
            Some(idx) => {
                if idx == 0 {
                    self.search_results.len().saturating_sub(1)
                } else {
                    idx - 1
                }
            }
            None => 0,
        };
        self.navigate_to_search_result(prev_idx);
    }

    /// Returns the current title bar text including the active filename.
    pub fn title_bar_text(&self) -> String {
        match &self.current_file_name {
            Some(name) => format!("Kestrel-PDF — {}", name),
            None => "Kestrel-PDF".to_string(),
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
        self.status_toast = Some(format!(
            "{} Signature adopted! Click on the document page to place it.",
            crate::icons::CHECK_CIRCLE
        ));
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

    /// Replaces the currently selected image with new RGBA pixel bytes.
    pub fn replace_selected_image_with_rgba(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> bool {
        if let (Some(page_idx), Some(img_idx)) = (
            self.selection.page_index,
            self.selection.selected_image_index,
        ) {
            if let Some(session) = &mut self.session {
                if let Err(e) = session.replace_image(page_idx, img_idx, rgba, width, height) {
                    self.status_toast = Some(format!("Error replacing image: {}", e));
                    return false;
                } else {
                    self.textures.clear();
                    self.image_textures.clear();
                    self.status_toast =
                        Some(format!("Image replaced with {}×{} px", width, height));
                    return true;
                }
            }
        }
        false
    }

    /// Deletes the currently selected image from the document.
    pub fn delete_selected_image(&mut self) -> bool {
        if let (Some(page_idx), Some(img_idx)) = (
            self.selection.page_index,
            self.selection.selected_image_index,
        ) {
            if let Some(session) = &mut self.session {
                if let Err(e) = session.delete_image(page_idx, img_idx) {
                    self.status_toast = Some(format!("Error deleting image: {}", e));
                    return false;
                } else {
                    self.selection.clear();
                    self.textures.clear();
                    self.image_textures.clear();
                    self.status_toast = Some("Image deleted from document".to_string());
                    return true;
                }
            }
        }
        false
    }

    /// Prompts a native file dialog to replace the selected image with a local PNG/JPG file.
    pub fn replace_selected_image_dialog(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        {
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(img) = image::load_from_memory(&bytes) {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    self.replace_selected_image_with_rgba(&rgba.into_raw(), w, h);
                }
            }
        }
    }

    /// Prompts a native file dialog to insert a new image onto the active page.
    pub fn insert_image_dialog(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        {
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(img) = image::load_from_memory(&bytes) {
                    let rgba = img.to_rgba8();
                    let (w, h) = rgba.dimensions();
                    let page_idx = self.current_page.saturating_sub(1);
                    if let Some(session) = &mut self.session {
                        let width_pt = 180.0_f32;
                        let height_pt = (180.0 * h as f32 / w.max(1) as f32).clamp(40.0, 500.0);
                        if let Err(e) = session.insert_image(
                            page_idx,
                            &rgba.into_raw(),
                            w,
                            h,
                            100.0,
                            400.0,
                            width_pt,
                            height_pt,
                        ) {
                            self.status_toast = Some(format!("Error inserting image: {}", e));
                        } else {
                            self.textures.clear();
                            self.image_textures.clear();
                            self.status_toast = Some("Image inserted into page".to_string());
                        }
                    }
                }
            }
        }
    }

    /// Inserts a blank page into the document after the current page.
    pub fn insert_blank_page_action(&mut self) -> bool {
        if let Some(session) = &mut self.session {
            let at = self.current_page;
            if let Err(e) = session.insert_blank_page(at, 595.28, 841.89) {
                self.status_toast = Some(format!("Error inserting blank page: {}", e));
                return false;
            } else {
                self.total_pages = session.page_count as usize;
                self.textures.clear();
                self.image_textures.clear();
                self.set_current_page(at + 1);
                self.status_toast = Some(format!("Inserted blank page {}", at + 1));
                return true;
            }
        }
        false
    }

    /// Deletes the current page from the document.
    pub fn delete_current_page_action(&mut self) -> bool {
        if self.total_pages <= 1 {
            self.status_toast = Some("Cannot delete the only page in document".to_string());
            return false;
        }
        let page_to_del = self.current_page.saturating_sub(1);
        if let Some(session) = &mut self.session {
            if let Err(e) = session.delete_page(page_to_del) {
                self.status_toast = Some(format!("Error deleting page: {}", e));
                return false;
            } else {
                self.total_pages = session.page_count as usize;
                self.textures.clear();
                self.image_textures.clear();
                let next_p = self.current_page.min(self.total_pages).max(1);
                self.set_current_page(next_p);
                self.status_toast = Some(format!("Deleted page {}", page_to_del + 1));
                return true;
            }
        }
        false
    }

    /// Duplicates the current page.
    pub fn duplicate_current_page_action(&mut self) -> bool {
        let page_idx = self.current_page.saturating_sub(1);
        if let Some(session) = &mut self.session {
            if let Err(e) = session.duplicate_page(page_idx) {
                self.status_toast = Some(format!("Error duplicating page: {}", e));
                return false;
            } else {
                self.total_pages = session.page_count as usize;
                self.textures.clear();
                self.image_textures.clear();
                self.set_current_page(self.current_page + 1);
                self.status_toast = Some(format!("Duplicated page {}", page_idx + 1));
                return true;
            }
        }
        false
    }

    /// Reorders the current page up or down.
    pub fn move_current_page_action(&mut self, up: bool) -> bool {
        let curr = self.current_page.saturating_sub(1);
        let target = if up {
            if curr == 0 {
                return false;
            }
            curr - 1
        } else {
            if curr + 1 >= self.total_pages {
                return false;
            }
            curr + 1
        };

        if let Some(session) = &mut self.session {
            if let Err(e) = session.reorder_page(curr, target) {
                self.status_toast = Some(format!("Error reordering page: {}", e));
                return false;
            } else {
                self.textures.clear();
                self.image_textures.clear();
                self.set_current_page(target + 1);
                self.status_toast = Some(format!("Moved page to position {}", target + 1));
                return true;
            }
        }
        false
    }

    /// Applies changes from the text editing modal (either modifying an existing text run or inserting a new text box).
    pub fn apply_text_edit(&mut self) -> bool {
        if let Some(session) = &mut self.session {
            let res = if let Some(run_idx) = self.editing_text_run_index {
                session.modify_text_run(self.editing_text_page, run_idx, &self.editing_text_buffer)
            } else {
                session.insert_text_box(
                    self.editing_text_page,
                    &self.editing_text_buffer,
                    self.editing_text_pos.x,
                    self.editing_text_pos.y,
                    self.editing_text_size,
                    self.editing_text_color,
                )
            };

            match res {
                Ok(()) => {
                    self.editing_text_modal_open = false;
                    self.textures.clear();
                    self.image_textures.clear();
                    self.status_toast = Some(if self.editing_text_run_index.is_some() {
                        "Text run updated successfully".to_string()
                    } else {
                        "New text box inserted successfully".to_string()
                    });
                    true
                }
                Err(e) => {
                    self.status_toast = Some(format!("Error applying text change: {}", e));
                    false
                }
            }
        } else {
            false
        }
    }

    /// Deletes the text run currently being edited in the modal.
    pub fn delete_editing_text_run(&mut self) -> bool {
        if let (Some(session), Some(run_idx)) = (&mut self.session, self.editing_text_run_index) {
            match session.delete_text_run(self.editing_text_page, run_idx) {
                Ok(()) => {
                    self.editing_text_modal_open = false;
                    self.editing_text_run_index = None;
                    self.textures.clear();
                    self.image_textures.clear();
                    self.status_toast = Some("Text run deleted successfully".to_string());
                    true
                }
                Err(e) => {
                    self.status_toast = Some(format!("Error deleting text run: {}", e));
                    false
                }
            }
        } else {
            false
        }
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
        // Apply global design system visuals, dark slate palette, and component metrics
        crate::theme::Theme::apply(ctx);

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
            let is_cmd_or_ctrl =
                ctx.input(|i| i.modifiers.command || i.modifiers.ctrl || i.modifiers.mac_cmd);
            if is_cmd_or_ctrl
                && !ctx.input(|i| i.modifiers.shift)
                && ctx.input(|i| i.key_pressed(egui::Key::O))
            {
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
            } else if ctx.input(is_copy_shortcut_pressed) {
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

        // 1. Tier 1: Application Header Bar (Branding, Primary Open CTA, Title, Save, Sidebar Toggle)
        egui::TopBottomPanel::top("app_header")
            .frame(crate::theme::Theme::header_frame())
            .show(ctx, |ui| {
                let header_w = ui.available_width();
                let is_header_compact = header_w < 820.0;
                let is_header_tiny = header_w < 640.0;

                ui.horizontal(|ui| {
                    // Branding
                    ui.label(
                        egui::RichText::new(format!("{} Kestrel-PDF", crate::icons::APP_LOGO))
                            .strong()
                            .size(15.0)
                            .color(crate::theme::Theme::TEXT_PRIMARY),
                    );
                    crate::theme::Theme::vertical_divider(ui);

                    // Prominent Primary Action: "Abrir fichero" Button (Responsive & comfortable)
                    let open_text = if is_header_compact {
                        format!("{} Abrir", crate::icons::OPEN_FILE)
                    } else {
                        format!("{} Abrir fichero", crate::icons::OPEN_FILE)
                    };
                    let open_min_w = if is_header_compact { 0.0 } else { 120.0 };
                    let open_btn = crate::theme::Theme::primary_button(open_text)
                        .min_size(Vec2::new(open_min_w, 28.0));

                    if ui
                        .add(open_btn)
                        .on_hover_text("Abrir documento PDF desde el disco (Ctrl+O / Cmd+O)")
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

                    if self.session.is_some() {
                        let save_text = if is_header_compact {
                            format!("{} Save", crate::icons::SAVE_FILE)
                        } else {
                            format!("{} Save / Export", crate::icons::SAVE_FILE)
                        };
                        let save_min_w = if is_header_compact { 0.0 } else { 110.0 };
                        let save_btn = crate::theme::Theme::accent_button(save_text)
                            .min_size(Vec2::new(save_min_w, 28.0));
                        if ui
                            .add(save_btn)
                            .on_hover_text("Guardar PDF modificado en el disco")
                            .clicked()
                        {
                            self.save_document();
                        }
                    }

                    crate::theme::Theme::vertical_divider(ui);

                    // Center: Document Title Heading (with dynamic middle truncation according to width)
                    if let Some(name) = &self.current_file_name {
                        let max_len = if is_header_tiny {
                            20
                        } else if is_header_compact {
                            32
                        } else {
                            48
                        };
                        let display_title = truncate_filename_middle(name, max_len);
                        ui.label(
                            egui::RichText::new(display_title)
                                .color(crate::theme::Theme::TEXT_PRIMARY)
                                .strong()
                                .size(13.0),
                        )
                        .on_hover_text(format!(
                            "Document: {}\nTotal Pages: {}",
                            name, self.total_pages
                        ));
                    } else {
                        ui.label(
                            egui::RichText::new("Ningún documento abierto")
                                .color(crate::theme::Theme::TEXT_MUTED)
                                .size(13.0),
                        );
                    }

                    // Right: Sidebar Toggle
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let sidebar_label = if is_header_compact {
                            crate::icons::SIDEBAR.to_string()
                        } else if self.sidebar_open {
                            format!("{} Cerrar panel", crate::icons::SIDEBAR)
                        } else {
                            format!("{} Panel lateral", crate::icons::SIDEBAR)
                        };
                        let toggle_btn = if self.sidebar_open {
                            crate::theme::Theme::accent_button(sidebar_label)
                        } else {
                            crate::theme::Theme::secondary_button(sidebar_label)
                        };
                        if ui
                            .add(toggle_btn)
                            .on_hover_text(
                                "Alternar panel lateral (Índice, Formularios, Capas, Búsqueda)",
                            )
                            .clicked()
                        {
                            self.sidebar_open = !self.sidebar_open;
                        }
                    });
                });
            });

        // 2. Tier 2: Document Action & Navigation Ribbon
        egui::TopBottomPanel::top("action_toolbar")
            .frame(crate::theme::Theme::ribbon_frame())
            .show(ctx, |ui| {
                let avail_w = ui.available_width();
                let is_wide = avail_w >= 1050.0;
                let is_medium = (780.0..1050.0).contains(&avail_w);
                let is_compact = avail_w < 780.0;

                ui.horizontal(|ui| {
                    // Group 1: In-Flow Page Navigator: [ < Prev ] [ 1 ] / 4 [ Next > ]
                    if self.total_pages > 0 {
                        crate::theme::Theme::pill_frame().show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);

                            let prev_enabled = self.current_page > 1;
                            let prev_label = if is_wide {
                                format!("{} Prev", crate::icons::PREV_PAGE)
                            } else {
                                crate::icons::PREV_PAGE.to_string()
                            };
                            let prev_btn = crate::theme::Theme::secondary_button(prev_label)
                                .min_size(Vec2::new(if is_wide { 32.0 } else { 22.0 }, 24.0));
                            if ui
                                .add_enabled(prev_enabled, prev_btn)
                                .on_hover_text("Página anterior (Left / Up)")
                                .clicked()
                            {
                                self.set_current_page(self.current_page.saturating_sub(1));
                            }

                            let page_box_w = if is_compact { 42.0 } else { 54.0 };
                            let page_edit = egui::TextEdit::singleline(&mut self.page_input_text)
                                .id_source("page_nav_input")
                                .desired_width(page_box_w)
                                .font(egui::TextStyle::Monospace)
                                .horizontal_align(egui::Align::Center);
                            let resp = ui.add(page_edit).on_hover_text(
                                "Escribe el número de página (p. ej. 3 o 3/4) y pulsa Enter para saltar directamente",
                            );

                            self.page_input_has_focus = resp.has_focus();

                            let enter_pressed =
                                ui.input(|i| i.key_pressed(egui::Key::Enter));

                            if (resp.has_focus() && enter_pressed)
                                || resp.lost_focus()
                                || (enter_pressed
                                    && parse_page_number(&self.page_input_text)
                                        != Some(self.current_page))
                            {
                                if resp.has_focus() {
                                    resp.surrender_focus();
                                }
                                if let Some(target) = parse_page_number(&self.page_input_text) {
                                    self.set_current_page(target);
                                } else {
                                    self.page_input_text = self.current_page.to_string();
                                }
                                ui.ctx().request_repaint();
                            }

                            ui.label(
                                egui::RichText::new(format!("/ {}", self.total_pages))
                                    .color(crate::theme::Theme::TEXT_MUTED)
                                    .size(13.0)
                                    .strong(),
                            )
                            .on_hover_text(format!("Total de páginas: {}", self.total_pages));

                            let next_enabled = self.current_page < self.total_pages;
                            let next_label = if is_wide {
                                format!("Next {}", crate::icons::NEXT_PAGE)
                            } else {
                                crate::icons::NEXT_PAGE.to_string()
                            };
                            let next_btn = crate::theme::Theme::secondary_button(next_label)
                                .min_size(Vec2::new(if is_wide { 32.0 } else { 22.0 }, 24.0));
                            if ui
                                .add_enabled(next_enabled, next_btn)
                                .on_hover_text("Página siguiente (Right / Down)")
                                .clicked()
                            {
                                self.set_current_page(self.current_page + 1);
                            }
                        });
                    } else {
                        crate::theme::Theme::pill_frame().show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("0 / 0")
                                    .color(crate::theme::Theme::TEXT_MUTED)
                                    .size(12.0),
                            );
                        });
                    }

                    crate::theme::Theme::vertical_divider(ui);

                    // Page Rotations
                    crate::theme::Theme::pill_frame().show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                        if ui
                            .add(
                                crate::theme::Theme::secondary_button(crate::icons::ROTATE_CCW)
                                    .min_size(Vec2::new(24.0, 24.0)),
                            )
                            .on_hover_text("Rotate Counter-Clockwise (90°)")
                            .clicked()
                        {
                            self.rotate_current_page_counter_clockwise();
                        }
                        if ui
                            .add(
                                crate::theme::Theme::secondary_button(crate::icons::ROTATE_CW)
                                    .min_size(Vec2::new(24.0, 24.0)),
                            )
                            .on_hover_text("Rotate Clockwise (90°)")
                            .clicked()
                        {
                            self.rotate_current_page_clockwise();
                        }
                    });

                    crate::theme::Theme::vertical_divider(ui);

                    // Group 2: Segmented Tool Mode Selector (Pill style)
                    crate::theme::Theme::pill_frame().show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);

                        let pan_label = if is_compact {
                            crate::icons::TOOL_PAN.to_string()
                        } else {
                            format!("{} Pan", crate::icons::TOOL_PAN)
                        };
                        if crate::theme::Theme::segmented_tool_button(
                            ui,
                            self.active_tool == ActiveTool::Pan,
                            pan_label,
                        )
                        .on_hover_text("Pan & scroll through document (Space + Drag)")
                        .clicked()
                        {
                            self.active_tool = ActiveTool::Pan;
                        }

                        let select_label = if is_compact {
                            crate::icons::TOOL_SELECT.to_string()
                        } else {
                            format!("{} Select", crate::icons::TOOL_SELECT)
                        };
                        if crate::theme::Theme::segmented_tool_button(
                            ui,
                            self.active_tool == ActiveTool::SelectText,
                            select_label,
                        )
                        .on_hover_text(format!(
                            "Select and copy text ({}) or select all ({})",
                            standard_copy_shortcut_str(),
                            standard_select_all_shortcut_str()
                        ))
                        .clicked()
                        {
                            self.active_tool = ActiveTool::SelectText;
                        }

                        let form_count = self.session.as_ref().map(|s| s.forms.len()).unwrap_or(0);
                        let form_label = if is_compact {
                            crate::icons::TOOL_FORMS.to_string()
                        } else if is_wide && form_count > 0 {
                            format!("{} Forms ({})", crate::icons::TOOL_FORMS, form_count)
                        } else {
                            format!("{} Forms", crate::icons::TOOL_FORMS)
                        };
                        if crate::theme::Theme::segmented_tool_button(
                            ui,
                            self.active_tool == ActiveTool::FormFill,
                            form_label,
                        )
                        .on_hover_text("Fill interactive form fields and checkboxes")
                        .clicked()
                        {
                            self.active_tool = ActiveTool::FormFill;
                        }

                        let edit_label = if is_compact {
                            crate::icons::TOOL_EDIT_TEXT.to_string()
                        } else if is_medium {
                            format!("{} Edit", crate::icons::TOOL_EDIT_TEXT)
                        } else {
                            format!("{} Edit Text", crate::icons::TOOL_EDIT_TEXT)
                        };
                        if crate::theme::Theme::segmented_tool_button(
                            ui,
                            self.active_tool == ActiveTool::EditText,
                            edit_label,
                        )
                        .on_hover_text("Add or edit text annotations")
                        .clicked()
                        {
                            self.active_tool = ActiveTool::EditText;
                        }

                        let img_tool_label = if is_compact {
                            crate::icons::TOOL_EDIT_IMAGE.to_string()
                        } else if is_medium {
                            format!("{} Image", crate::icons::TOOL_EDIT_IMAGE)
                        } else {
                            format!("{} Edit Image", crate::icons::TOOL_EDIT_IMAGE)
                        };
                        if crate::theme::Theme::segmented_tool_button(
                            ui,
                            self.active_tool == ActiveTool::EditImage,
                            img_tool_label,
                        )
                        .on_hover_text("Select, replace, insert, or manipulate images")
                        .clicked()
                        {
                            self.active_tool = ActiveTool::EditImage;
                        }

                        if self.active_tool == ActiveTool::EditText {
                            let add_txt_label = if is_compact {
                                crate::icons::ADD.to_string()
                            } else {
                                format!("{} Add Text", crate::icons::ADD)
                            };
                            if ui
                                .add(
                                    crate::theme::Theme::accent_button(add_txt_label)
                                        .min_size(Vec2::new(0.0, 24.0)),
                                )
                                .on_hover_text("Insert a new text annotation onto current page")
                                .clicked()
                            {
                                self.editing_text_modal_open = true;
                                self.editing_text_page = self.current_page.saturating_sub(1);
                                self.editing_text_run_index = None;
                                self.editing_text_buffer = String::new();
                                self.editing_text_pos = egui::pos2(100.0, 500.0);
                                self.editing_text_size = 14.0;
                                self.editing_text_color = [15, 23, 42];
                            }
                        }

                        if self.active_tool == ActiveTool::EditImage {
                            let insert_img_label = if is_compact {
                                crate::icons::ADD.to_string()
                            } else {
                                format!("{} Insert Image", crate::icons::ADD)
                            };
                            if ui
                                .add(
                                    crate::theme::Theme::accent_button(insert_img_label)
                                        .min_size(Vec2::new(0.0, 24.0)),
                                )
                                .on_hover_text("Insert an image from file onto current page")
                                .clicked()
                            {
                                self.insert_image_dialog();
                            }

                            if self.selection.has_image() {
                                if ui
                                    .add(
                                        crate::theme::Theme::secondary_button(format!(
                                            "{} Replace",
                                            crate::icons::COPY_IMAGE
                                        ))
                                        .min_size(Vec2::new(0.0, 24.0)),
                                    )
                                    .on_hover_text("Replace selected image")
                                    .clicked()
                                    {
                                        self.replace_selected_image_dialog();
                                    }

                                if ui
                                    .add(
                                        crate::theme::Theme::secondary_button(format!(
                                            "{} Delete",
                                            crate::icons::TRASH
                                        ))
                                        .min_size(Vec2::new(0.0, 24.0)),
                                    )
                                    .on_hover_text("Delete selected image")
                                    .clicked()
                                    {
                                        self.delete_selected_image();
                                    }
                            }
                        }

                        let sign_active = self.active_tool == ActiveTool::SignContract;
                        let sign_label = if is_compact {
                            crate::icons::TOOL_SIGN.to_string()
                        } else if is_medium {
                            format!("{} Sign", crate::icons::TOOL_SIGN)
                        } else {
                            format!("{} Sign Contract", crate::icons::TOOL_SIGN)
                        };
                        if crate::theme::Theme::segmented_tool_button(
                            ui,
                            sign_active,
                            sign_label,
                        )
                        .on_hover_text("Sign contract with drawn or digital signature")
                        .clicked()
                        {
                            self.active_tool = ActiveTool::SignContract;
                            if self.adopted_signature.is_none() {
                                self.signature_modal_open = true;
                            }
                        }

                        if sign_active {
                            let create_sig_label = if is_compact {
                                crate::icons::TOOL_SIGN.to_string()
                            } else {
                                format!("{} Create Signature", crate::icons::TOOL_SIGN)
                            };
                            if ui
                                .add(
                                    crate::theme::Theme::accent_button(create_sig_label)
                                        .min_size(Vec2::new(0.0, 24.0)),
                                )
                                .on_hover_text("Open Signature Pad")
                                .clicked()
                            {
                                self.signature_modal_open = true;
                            }
                        }

                        let redact_label = if is_compact {
                            crate::icons::TOOL_REDACT.to_string()
                        } else {
                            format!("{} Redact", crate::icons::TOOL_REDACT)
                        };
                        if crate::theme::Theme::segmented_tool_button(
                            ui,
                            self.active_tool == ActiveTool::RedactData,
                            redact_label,
                        )
                        .on_hover_text("Permanently redact sensitive document data")
                        .clicked()
                        {
                            self.active_tool = ActiveTool::RedactData;
                        }
                    });

                    // Dynamic Selection Actions
                    if !self.selection.is_empty() {
                        crate::theme::Theme::vertical_divider(ui);
                        crate::theme::Theme::pill_frame().show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
                            if self.selection.has_text() {
                                let count = self
                                    .selection
                                    .selected_text
                                    .as_ref()
                                    .map(|s| s.chars().count())
                                    .unwrap_or(0);
                                let copy_text_label = if is_compact {
                                    format!("{} Copy", crate::icons::COPY_TEXT)
                                } else {
                                    format!("{} Copy Text ({} chars)", crate::icons::COPY_TEXT, count)
                                };
                                if ui
                                    .add(
                                        crate::theme::Theme::accent_button(copy_text_label)
                                            .min_size(Vec2::new(0.0, 24.0)),
                                    )
                                    .on_hover_text(format!(
                                        "Copy selected text to clipboard ({})",
                                        standard_copy_shortcut_str()
                                    ))
                                    .clicked()
                                {
                                    self.copy_selected_text(ctx);
                                }
                            }
                            if self.selection.has_image() {
                                let copy_img_label = if is_compact {
                                    format!("{} Img", crate::icons::COPY_IMAGE)
                                } else {
                                    format!("{} Copy Image", crate::icons::COPY_IMAGE)
                                };
                                if ui
                                    .add(
                                        crate::theme::Theme::accent_button(copy_img_label)
                                            .min_size(Vec2::new(0.0, 24.0)),
                                    )
                                    .on_hover_text(format!(
                                        "Copy selected image to clipboard ({})",
                                        standard_copy_shortcut_str()
                                    ))
                                    .clicked()
                                {
                                    self.copy_selected_image(ctx);
                                }
                            }
                            if ui
                                .add(
                                    crate::theme::Theme::secondary_button(format!(
                                        "{} Clear",
                                        crate::icons::CLOSE
                                    ))
                                    .min_size(Vec2::new(0.0, 24.0)),
                                )
                                .on_hover_text("Clear active selection (Escape)")
                                .clicked()
                            {
                                self.clear_selection();
                            }
                        });
                    }

                    // Group 3 & 4: Zoom Controls & Live Search (Right aligned)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        crate::theme::Theme::pill_frame().show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                            if ui
                                .add(
                                    crate::theme::Theme::secondary_button(crate::icons::ZOOM_IN)
                                        .min_size(Vec2::new(24.0, 24.0)),
                                )
                                .on_hover_text("Zoom In (+15%)")
                                .clicked()
                            {
                                self.zoom_in();
                            }
                            ui.label(
                                egui::RichText::new(format!("{:.0}%", self.zoom_level * 100.0))
                                    .color(crate::theme::Theme::TEXT_PRIMARY)
                                    .size(12.0)
                                    .strong(),
                            )
                            .on_hover_text("Current zoom percentage");
                            if ui
                                .add(
                                    crate::theme::Theme::secondary_button(crate::icons::ZOOM_OUT)
                                        .min_size(Vec2::new(24.0, 24.0)),
                                )
                                .on_hover_text("Zoom Out (-15%)")
                                .clicked()
                            {
                                self.zoom_out();
                            }

                            if is_wide {
                                if ui
                                    .add(
                                        crate::theme::Theme::secondary_button("Fit Page")
                                            .min_size(Vec2::new(0.0, 24.0)),
                                    )
                                    .on_hover_text("Fit entire page to viewport")
                                    .clicked()
                                {
                                    self.pending_fit = Some(FitMode::FitPage);
                                }
                                if ui
                                    .add(
                                        crate::theme::Theme::secondary_button("Fit Width")
                                            .min_size(Vec2::new(0.0, 24.0)),
                                    )
                                    .on_hover_text("Fit document width to viewport")
                                    .clicked()
                                {
                                    self.pending_fit = Some(FitMode::FitWidth);
                                }
                                if ui
                                    .add(
                                        crate::theme::Theme::secondary_button("Reset")
                                            .min_size(Vec2::new(0.0, 24.0)),
                                    )
                                    .on_hover_text("Reset zoom to 100%")
                                    .clicked()
                                {
                                    self.reset_zoom();
                                }
                            } else if is_medium {
                                if ui
                                    .add(
                                        crate::theme::Theme::secondary_button("Fit")
                                            .min_size(Vec2::new(0.0, 24.0)),
                                    )
                                    .on_hover_text("Fit page width to viewport")
                                    .clicked()
                                {
                                    self.pending_fit = Some(FitMode::FitWidth);
                                }
                                if ui
                                    .add(
                                        crate::theme::Theme::secondary_button("Reset")
                                            .min_size(Vec2::new(0.0, 24.0)),
                                    )
                                    .on_hover_text("Reset zoom to 100%")
                                    .clicked()
                                {
                                    self.reset_zoom();
                                }
                            } else {
                                if ui
                                    .add(
                                        crate::theme::Theme::secondary_button("Fit")
                                            .min_size(Vec2::new(0.0, 24.0)),
                                    )
                                    .on_hover_text("Fit page width to viewport")
                                    .clicked()
                                {
                                    self.pending_fit = Some(FitMode::FitWidth);
                                }
                            }
                        });

                        crate::theme::Theme::vertical_divider(ui);

                        // Search Bar
                        crate::theme::Theme::pill_frame().show(ui, |ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(3.0, 0.0);
                            let find_label = if is_compact {
                                crate::icons::SEARCH.to_string()
                            } else {
                                format!("{} Find", crate::icons::SEARCH)
                            };
                            if ui
                                .add(
                                    crate::theme::Theme::secondary_button(find_label)
                                        .min_size(Vec2::new(0.0, 24.0)),
                                )
                                .on_hover_text("Buscar término en el documento")
                                .clicked()
                            {
                                self.execute_search();
                            }
                            let search_w = if is_compact {
                                65.0
                            } else if is_medium {
                                90.0
                            } else {
                                110.0
                            };
                            let search_resp = ui.add(
                                egui::TextEdit::singleline(&mut self.search_query)
                                    .desired_width(search_w)
                                    .hint_text("Search…"),
                            );
                            let enter_pressed = search_resp.has_focus()
                                && ui.input(|i| i.key_pressed(egui::Key::Enter));
                            if enter_pressed {
                                if !self.search_results.is_empty() {
                                    self.next_search_result();
                                } else {
                                    self.execute_search();
                                }
                            }

                            if !self.search_results.is_empty() && !is_compact {
                                if ui
                                    .add(
                                        crate::theme::Theme::secondary_button(
                                            crate::icons::CARET_LEFT,
                                        )
                                        .min_size(Vec2::new(18.0, 24.0)),
                                    )
                                    .on_hover_text("Resultado anterior")
                                    .clicked()
                                {
                                    self.prev_search_result();
                                }
                                if ui
                                    .add(
                                        crate::theme::Theme::secondary_button(
                                            crate::icons::CARET_RIGHT,
                                        )
                                        .min_size(Vec2::new(18.0, 24.0)),
                                    )
                                    .on_hover_text("Siguiente resultado")
                                    .clicked()
                                {
                                    self.next_search_result();
                                }
                            }
                        });
                    });
                });
            });

        // 2. Status Banner / Toast
        if let Some(toast) = &self.status_toast.clone() {
            egui::TopBottomPanel::bottom("status_bar")
                .frame(crate::theme::Theme::status_frame())
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(crate::icons::INFO)
                                .color(crate::theme::Theme::ACCENT_OCHRE)
                                .strong()
                                .size(14.0),
                        );
                        ui.label(
                            egui::RichText::new(toast)
                                .color(crate::theme::Theme::TEXT_PRIMARY)
                                .size(12.0),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    crate::theme::Theme::secondary_button(crate::icons::CLOSE)
                                        .min_size(Vec2::new(24.0, 20.0)),
                                )
                                .on_hover_text("Cerrar mensaje")
                                .clicked()
                            {
                                self.status_toast = None;
                            }
                        });
                    });
                });
        }

        // 3. Left Sidebar: Thumbnails, Outlines, Forms, Search Results
        if self.sidebar_open {
            egui::SidePanel::left("left_sidebar")
                .frame(crate::theme::Theme::header_frame())
                .resizable(true)
                .default_width(280.0)
                .show(ctx, |ui| {
                    crate::theme::Theme::pill_frame().show(ui, |ui| {
                        ui.spacing_mut().item_spacing = Vec2::new(2.0, 0.0);
                        if crate::theme::Theme::segmented_tool_button(
                            ui,
                            self.sidebar_tab == SidebarTab::Thumbnails,
                            "Pages",
                        )
                        .clicked()
                        {
                            self.sidebar_tab = SidebarTab::Thumbnails;
                        }
                        if crate::theme::Theme::segmented_tool_button(
                            ui,
                            self.sidebar_tab == SidebarTab::Outlines,
                            "Outlines",
                        )
                        .clicked()
                        {
                            self.sidebar_tab = SidebarTab::Outlines;
                        }
                        if crate::theme::Theme::segmented_tool_button(
                            ui,
                            self.sidebar_tab == SidebarTab::Forms,
                            "Forms",
                        )
                        .clicked()
                        {
                            self.sidebar_tab = SidebarTab::Forms;
                        }
                        if crate::theme::Theme::segmented_tool_button(
                            ui,
                            self.sidebar_tab == SidebarTab::Layers,
                            "Layers",
                        )
                        .clicked()
                        {
                            self.sidebar_tab = SidebarTab::Layers;
                        }
                        if crate::theme::Theme::segmented_tool_button(
                            ui,
                            self.sidebar_tab == SidebarTab::SearchResults,
                            "Search",
                        )
                        .clicked()
                        {
                            self.sidebar_tab = SidebarTab::SearchResults;
                        }
                    });
                    ui.add_space(4.0);
                    ui.separator();

                    match self.sidebar_tab {
                        SidebarTab::Thumbnails => {
                            if self.total_pages > 0 {
                                ui.horizontal(|ui| {
                                    if ui
                                        .add(
                                            crate::theme::Theme::accent_button(format!(
                                                "{} Page",
                                                crate::icons::ADD
                                            ))
                                            .min_size(Vec2::new(0.0, 22.0)),
                                        )
                                        .on_hover_text("Insert blank page after current page")
                                        .clicked()
                                    {
                                        self.insert_blank_page_action();
                                    }

                                    if ui
                                        .add(
                                            crate::theme::Theme::secondary_button(format!(
                                                "{} Copy",
                                                crate::icons::DUPLICATE
                                            ))
                                            .min_size(Vec2::new(0.0, 22.0)),
                                        )
                                        .on_hover_text("Duplicate current page")
                                        .clicked()
                                    {
                                        self.duplicate_current_page_action();
                                    }

                                    if self.total_pages > 1
                                        && ui
                                            .add(
                                                crate::theme::Theme::secondary_button(format!(
                                                    "{} Del",
                                                    crate::icons::TRASH
                                                ))
                                                .min_size(Vec2::new(0.0, 22.0)),
                                            )
                                            .on_hover_text("Delete current page")
                                            .clicked()
                                    {
                                        self.delete_current_page_action();
                                    }
                                });
                                ui.add_space(4.0);
                                ui.separator();
                                ui.add_space(4.0);
                            }

                            egui::ScrollArea::vertical().show(ui, |ui| {
                                for i in 1..=self.total_pages {
                                    let is_selected = self.current_page == i;
                                    let label = format!("Page {}", i);

                                    ui.horizontal(|ui| {
                                        let btn = if is_selected {
                                            crate::theme::Theme::primary_button(label)
                                        } else {
                                            crate::theme::Theme::secondary_button(label)
                                        };
                                        if ui
                                            .add_sized([ui.available_width() - 56.0, 26.0], btn)
                                            .clicked()
                                        {
                                            self.set_current_page(i);
                                        }

                                        if i > 1
                                            && ui
                                                .add(
                                                    crate::theme::Theme::secondary_button(
                                                        crate::icons::CARET_UP,
                                                    )
                                                    .min_size(Vec2::new(24.0, 24.0)),
                                                )
                                                .on_hover_text("Move page up")
                                                .clicked()
                                        {
                                            self.set_current_page(i);
                                            self.move_current_page_action(true);
                                        }
                                        if i < self.total_pages
                                            && ui
                                                .add(
                                                    crate::theme::Theme::secondary_button(
                                                        crate::icons::CARET_DOWN,
                                                    )
                                                    .min_size(Vec2::new(24.0, 24.0)),
                                                )
                                                .on_hover_text("Move page down")
                                                .clicked()
                                        {
                                            self.set_current_page(i);
                                            self.move_current_page_action(false);
                                        }
                                    });
                                    ui.add_space(2.0);
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
                                        let mut target_page = None;
                                        for outline in &session.outlines {
                                            crate::theme::Theme::card_frame().show(ui, |ui| {
                                                if ui.link(&outline.title).clicked() {
                                                    target_page =
                                                        Some((outline.target_page as usize) + 1);
                                                }
                                            });
                                            ui.add_space(3.0);
                                        }
                                        if let Some(page) = target_page {
                                            self.set_current_page(page);
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
                                        if ui
                                            .add(crate::theme::Theme::accent_button(format!(
                                                "{} Add Form Field",
                                                crate::icons::ADD
                                            )))
                                            .clicked()
                                        {
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
                                        ui.horizontal(|ui| {
                                            ui.heading(format!("Fields ({})", session.forms.len()));
                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::Center),
                                                |ui| {
                                                    if ui
                                                        .add(crate::theme::Theme::accent_button(
                                                            format!("{} Add", crate::icons::ADD),
                                                        ))
                                                        .clicked()
                                                    {
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
                                                },
                                            );
                                        });
                                        ui.separator();

                                        for (idx, field) in session.forms.iter_mut().enumerate() {
                                            crate::theme::Theme::card_frame().show(ui, |ui| {
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "{}. {}",
                                                        idx + 1,
                                                        field.name
                                                    ))
                                                    .color(Color32::WHITE)
                                                    .strong(),
                                                );
                                                ui.label(
                                                    egui::RichText::new(format!(
                                                        "Page {}",
                                                        field.page_index + 1
                                                    ))
                                                    .color(crate::theme::Theme::TEXT_MUTED)
                                                    .size(11.0),
                                                );

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
                                            crate::theme::Theme::card_frame().show(ui, |ui| {
                                                ui.checkbox(&mut layer.visible, &layer.name);
                                            });
                                            ui.add_space(2.0);
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
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "Found {} matching pages:",
                                            self.search_results.len()
                                        ))
                                        .color(crate::theme::Theme::TEXT_SECONDARY),
                                    );
                                    ui.add_space(4.0);
                                    let mut nav_idx = None;
                                    for (res_idx, res) in self.search_results.iter().enumerate() {
                                        let is_selected =
                                            self.selected_search_result == Some(res_idx);
                                        crate::theme::Theme::card_frame().show(ui, |ui| {
                                            let btn_label = format!(
                                                "Page {}: {}",
                                                res.page_index + 1,
                                                res.snippet
                                            );
                                            let btn = if is_selected {
                                                crate::theme::Theme::accent_button(btn_label)
                                            } else {
                                                crate::theme::Theme::secondary_button(btn_label)
                                            };
                                            if ui
                                                .add(btn)
                                                .on_hover_text(
                                                    "Saltar a este resultado en el documento",
                                                )
                                                .clicked()
                                            {
                                                nav_idx = Some(res_idx);
                                            }
                                        });
                                        ui.add_space(3.0);
                                    }
                                    if let Some(idx) = nav_idx {
                                        self.navigate_to_search_result(idx);
                                    }
                                }
                            });
                        }
                    }
                });
        }

        // 4. Central Viewport: High-Performance Continuous Page Viewer
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(crate::theme::Theme::CANVAS_BACKDROP))
            .show(ctx, |ui| {
            if self.total_pages == 0 {
                ui.centered_and_justified(|ui| {
                    egui::Frame::group(ui.style())
                        .fill(crate::theme::Theme::PANEL_SURFACE)
                        .stroke(egui::Stroke::new(1.0_f32, crate::theme::Theme::BORDER_DARK))
                        .rounding(12.0)
                        .inner_margin(egui::Margin::same(36.0))
                        .show(ui, |ui| {
                            ui.set_max_width(440.0);
                            ui.vertical_centered(|ui| {
                                ui.label(
                                    egui::RichText::new(crate::icons::APP_LOGO)
                                        .color(crate::theme::Theme::ACCENT_SALMON)
                                        .size(54.0),
                                );
                                ui.add_space(8.0);
                                ui.heading(
                                    egui::RichText::new("Kestrel-PDF")
                                        .color(Color32::WHITE)
                                        .strong()
                                        .size(22.0),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(
                                        "Lector y editor de PDF universal de ultra-alto rendimiento",
                                    )
                                    .color(crate::theme::Theme::TEXT_MUTED)
                                    .size(13.0),
                                );
                                ui.add_space(28.0);

                                let big_open_btn = egui::Button::new(
                                    egui::RichText::new(format!(
                                        "{}  Abrir fichero PDF",
                                        crate::icons::OPEN_FILE
                                    ))
                                    .color(Color32::WHITE)
                                    .strong()
                                    .size(15.0),
                                )
                                .fill(crate::theme::Theme::ACCENT_SALMON)
                                .rounding(8.0)
                                .min_size(Vec2::new(220.0, 42.0));

                                if ui
                                    .add(big_open_btn)
                                    .on_hover_text(
                                        "Selecciona un documento PDF para abrir (Ctrl+O / Cmd+O)",
                                    )
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

                                ui.add_space(12.0);
                                ui.label(
                                    egui::RichText::new("o arrastra y suelta tu archivo PDF aquí")
                                        .color(Color32::from_rgb(100, 116, 139))
                                        .size(12.0),
                                );
                            });
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

                                let max_side = ctx.input(|i| i.max_texture_side).clamp(512, 16384);

                                // Request tile from background worker (clamped to max texture dimension)
                                let scale_ratio = if base_width > max_side as f32 || base_height > max_side as f32 {
                                    (max_side as f32 / base_width.max(base_height)).min(1.0)
                                } else {
                                    1.0
                                };
                                let target_w = ((base_width * scale_ratio) as u32).clamp(64, max_side as u32);
                                let target_h = ((base_height * scale_ratio) as u32).clamp(64, max_side as u32);
                                self.pipeline.request_tile(key, target_w, target_h);

                                // Check if rasterized tile is in cache, bind to GPU texture
                                if !self.textures.contains_key(&key) {
                                    if let Some(tile_buf) = self.pipeline.cache().get(&key) {
                                        let (w, h, buf) = clamp_rgba_image_to_max_side(
                                            tile_buf.width as usize,
                                            tile_buf.height as usize,
                                            &tile_buf.rgba,
                                            max_side,
                                        );
                                        let img = ColorImage::from_rgba_unmultiplied(
                                            [w, h],
                                            &buf,
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
                                    || self.active_tool == ActiveTool::EditText
                                    || self.active_tool == ActiveTool::EditImage
                                {
                                    egui::Sense::click_and_drag()
                                } else {
                                    egui::Sense::hover()
                                };
                                let (response, painter) =
                                    ui.allocate_painter(Vec2::new(base_width, base_height), sense);
                                let rect = response.rect;

                                if self.scroll_to_page == Some(page_idx)
                                    && !self.scroll_to_search_match
                                {
                                    ui.scroll_to_rect(rect, Some(egui::Align::TOP));
                                    response.scroll_to_me(Some(egui::Align::TOP));
                                    self.scroll_to_page = None;
                                }

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
                                                    "{} Copy Text ({})",
                                                    crate::icons::COPY_TEXT,
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
                                                    "{} Copy Image ({})",
                                                    crate::icons::COPY_IMAGE,
                                                    standard_copy_shortcut_str()
                                                ))
                                                .clicked()
                                        {
                                            self.copy_selected_image(ctx);
                                            ui.close_menu();
                                        }
                                        if ui
                                            .button(format!(
                                                "{} Clear Selection (Esc)",
                                                crate::icons::CLOSE
                                            ))
                                            .clicked()
                                        {
                                            self.clear_selection();
                                            ui.close_menu();
                                        }
                                            if self.active_tool == ActiveTool::EditImage
                                                && self.selection.has_image()
                                            {
                                                if ui
                                                    .button(format!(
                                                        "{} Replace Image...",
                                                        crate::icons::COPY_IMAGE
                                                    ))
                                                    .clicked()
                                                {
                                                    self.replace_selected_image_dialog();
                                                    ui.close_menu();
                                                }
                                                if ui
                                                    .button(format!(
                                                        "{} Delete Image",
                                                        crate::icons::TRASH
                                                    ))
                                                    .clicked()
                                                {
                                                    self.delete_selected_image();
                                                    ui.close_menu();
                                                }
                                            }
                                        });
                                }

                                // Handle clicking in EditText mode
                                if self.active_tool == ActiveTool::EditText
                                    && response.clicked()
                                {
                                    if let Some(hover_pos) = response.hover_pos() {
                                        let pdf_scale = self.zoom_level;
                                        let visual_x = (hover_pos.x - rect.left()) / pdf_scale;
                                        let visual_y = (hover_pos.y - rect.top()) / pdf_scale;

                                        let mut clicked_run = None;
                                        if let Some(session) = &self.session {
                                            if let Some(layout) = session.get_page_layout(page_idx) {
                                                for (tr_idx, tr) in layout.text_runs.iter().enumerate() {
                                                    let b = layout.text_run_visual_bounds(tr, page_rot);
                                                    if visual_x >= b[0] - 2.0
                                                        && visual_x <= b[2] + 2.0
                                                        && visual_y >= b[1] - 2.0
                                                        && visual_y <= b[3] + 2.0
                                                    {
                                                        clicked_run = Some((
                                                            tr_idx,
                                                            tr.text.clone(),
                                                            tr.font_size,
                                                            tr.color,
                                                        ));
                                                        break;
                                                    }
                                                }
                                            }
                                        }

                                        if let Some((idx, text, font_size, color)) = clicked_run {
                                            self.editing_text_modal_open = true;
                                            self.editing_text_page = page_idx;
                                            self.editing_text_run_index = Some(idx);
                                            self.editing_text_buffer = text;
                                            self.editing_text_size = font_size;
                                            self.editing_text_color = color;
                                        } else {
                                            let pdf_y = (base_height / pdf_scale) - visual_y;
                                            self.editing_text_modal_open = true;
                                            self.editing_text_page = page_idx;
                                            self.editing_text_run_index = None;
                                            self.editing_text_buffer = String::new();
                                            self.editing_text_pos = egui::pos2(visual_x, pdf_y);
                                            self.editing_text_size = 14.0;
                                            self.editing_text_color = [15, 23, 42];
                                        }
                                    }
                                }

                                // Handle clicking in EditImage mode
                                if self.active_tool == ActiveTool::EditImage
                                    && response.clicked()
                                {
                                    if let Some(hover_pos) = response.hover_pos() {
                                        let pdf_scale = self.zoom_level;
                                        let visual_x = (hover_pos.x - rect.left()) / pdf_scale;
                                        let visual_y = (hover_pos.y - rect.top()) / pdf_scale;

                                        if let Some(session) = &self.session {
                                            if let Some(layout) = session.get_page_layout(page_idx) {
                                                if let Some((img_idx, _img)) =
                                                    layout.find_image_at_point(visual_x, visual_y, page_rot)
                                                {
                                                    self.selection.clear();
                                                    self.selection.page_index = Some(page_idx);
                                                    self.selection.selected_image_index = Some(img_idx);
                                                } else {
                                                    self.selection.clear();
                                                }
                                            }
                                        }
                                    }
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
                                                    let max_side = ctx.input(|i| i.max_texture_side).clamp(512, 16384);
                                                    let (w, h, buf) = clamp_rgba_image_to_max_side(
                                                        img.pixel_width as usize,
                                                        img.pixel_height as usize,
                                                        &img.rgba,
                                                        max_side,
                                                    );
                                                    let color_image =
                                                        ColorImage::from_rgba_unmultiplied(
                                                            [w, h],
                                                            &buf,
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

                                                let font_id = egui::FontId::proportional(font_size);
                                                let eff_rot_deg = (tr.rotation_deg - page_rot as f32).rem_euclid(360.0);

                                                // Live search visual highlight (exact substring bounds, not whole paragraph)
                                                let query_trimmed = self.search_query.trim();
                                                if !query_trimmed.is_empty() {
                                                    let q_lower: Vec<char> = query_trimmed.to_lowercase().chars().collect();
                                                    let t_chars: Vec<char> = tr.text.chars().collect();
                                                    let t_lower: Vec<char> = tr.text.to_lowercase().chars().collect();

                                                    if !q_lower.is_empty() && t_lower.len() >= q_lower.len() {
                                                        let q_len = q_lower.len();
                                                        for i in 0..=(t_lower.len() - q_len) {
                                                            if t_lower[i..i + q_len] == q_lower[..] {
                                                                let prefix: String = t_chars[..i].iter().collect();
                                                                let matched_text: String = t_chars[i..i + q_len].iter().collect();

                                                                let prefix_w = if prefix.is_empty() {
                                                                    0.0
                                                                } else {
                                                                    painter
                                                                        .layout_no_wrap(prefix, font_id.clone(), Color32::TRANSPARENT)
                                                                        .size()
                                                                        .x
                                                                };
                                                                let match_w = painter
                                                                    .layout_no_wrap(matched_text, font_id.clone(), Color32::TRANSPARENT)
                                                                    .size()
                                                                    .x;

                                                                let is_active_result = self.selected_search_result.is_some_and(|sel_idx| {
                                                                    self.search_results
                                                                        .get(sel_idx)
                                                                        .is_some_and(|r| r.page_index as usize == page_idx)
                                                                });

                                                                let hl_color = if is_active_result {
                                                                    crate::theme::Theme::SEARCH_HIGHLIGHT_ACTIVE
                                                                } else {
                                                                    crate::theme::Theme::SEARCH_HIGHLIGHT_REGULAR
                                                                };

                                                                if eff_rot_deg.abs() < 1.0 || (eff_rot_deg - 360.0).abs() < 1.0 {
                                                                    let hl_rect = egui::Rect::from_min_max(
                                                                        egui::pos2(t_x + prefix_w - 1.5, t_y - 1.0),
                                                                        egui::pos2(
                                                                            t_x + prefix_w + match_w + 1.5,
                                                                            t_y + font_size * 1.15 + 1.0,
                                                                        ),
                                                                    );
                                                                    painter.rect_filled(hl_rect, 2.0, hl_color);

                                                                    if self.scroll_to_search_match && self.scroll_to_page == Some(page_idx) {
                                                                        ui.scroll_to_rect(hl_rect, Some(egui::Align::Center));
                                                                        self.scroll_to_search_match = false;
                                                                        self.scroll_to_page = None;
                                                                    }
                                                                } else {
                                                                    let angle_rad = -eff_rot_deg.to_radians();
                                                                    let dir = egui::vec2(angle_rad.cos(), angle_rad.sin());
                                                                    let perp = egui::vec2(-angle_rad.sin(), angle_rad.cos());

                                                                    let p0 = egui::pos2(t_x, t_y) + dir * (prefix_w - 1.5) + perp * (-1.0);
                                                                    let p1 = egui::pos2(t_x, t_y)
                                                                        + dir * (prefix_w + match_w + 1.5)
                                                                        + perp * (-1.0);
                                                                    let p2 = egui::pos2(t_x, t_y)
                                                                        + dir * (prefix_w + match_w + 1.5)
                                                                        + perp * (font_size * 1.15 + 1.0);
                                                                    let p3 = egui::pos2(t_x, t_y)
                                                                        + dir * (prefix_w - 1.5)
                                                                        + perp * (font_size * 1.15 + 1.0);

                                                                    painter.add(egui::Shape::convex_polygon(
                                                                        vec![p0, p1, p2, p3],
                                                                        hl_color,
                                                                        egui::Stroke::NONE,
                                                                    ));

                                                                    if self.scroll_to_search_match && self.scroll_to_page == Some(page_idx) {
                                                                        let bbox = egui::Rect::from_two_pos(p0, p2);
                                                                        ui.scroll_to_rect(bbox, Some(egui::Align::Center));
                                                                        self.scroll_to_search_match = false;
                                                                        self.scroll_to_page = None;
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                let color = Color32::from_rgb(
                                                    tr.color[0],
                                                    tr.color[1],
                                                    tr.color[2],
                                                );

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
                                                        crate::icons::CHECK.to_string()
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
                                                        format!(
                                                            "{} Digitally Verified PAdES / SHA-256",
                                                            crate::icons::LOCK
                                                        ),
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
                                                            "{} Image ({}×{} px)",
                                                            crate::icons::COPY_IMAGE,
                                                            img.pixel_width,
                                                            img.pixel_height
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

                                 if self.scroll_to_page == Some(page_idx) {
                                    ui.scroll_to_rect(rect, Some(egui::Align::TOP));
                                    response.scroll_to_me(Some(egui::Align::TOP));
                                    self.scroll_to_page = None;
                                    self.scroll_to_search_match = false;
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
            egui::Window::new(format!(
                "{} Sign Contract — Digital & Visual Signature",
                crate::icons::TOOL_SIGN
            ))
            .collapsible(false)
            .resizable(false)
            .default_size(Vec2::new(460.0, 500.0))
            .show(ctx, |ui| {
                ui.label(
                    egui::RichText::new(
                        "Draw your signature below using mouse or pen stylus with Bézier smoothing:",
                    )
                    .color(crate::theme::Theme::TEXT_SECONDARY)
                    .size(12.0),
                );
                ui.add_space(6.0);

                // Drawing Canvas Pad (420x160)
                let pad_size = Vec2::new(420.0, 160.0);
                let (pad_resp, pad_painter) = ui.allocate_painter(pad_size, egui::Sense::drag());
                let pad_rect = pad_resp.rect;

                pad_painter.rect_filled(pad_rect, 6.0, Color32::from_rgb(250, 250, 252));
                pad_painter.rect_stroke(
                    pad_rect,
                    6.0,
                    egui::Stroke::new(1.5_f32, crate::theme::Theme::BORDER_DARK),
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
                            egui::Stroke::new(2.4_f32, ink_color),
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
                        egui::Stroke::new(2.4_f32, ink_color),
                    );
                }

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Ink Color:")
                            .color(crate::theme::Theme::TEXT_SECONDARY),
                    );
                    ui.radio_value(&mut self.signature_blue_ink, true, "Royal Blue");
                    ui.radio_value(&mut self.signature_blue_ink, false, "Deep Slate");

                    ui.separator();
                    if ui
                        .add(crate::theme::Theme::secondary_button(format!(
                            "{} Clear Pad",
                            crate::icons::TRASH
                        )))
                        .clicked()
                    {
                        self.signature_pad_raw_strokes.clear();
                        self.signature_pad_current_stroke.clear();
                    }
                    if ui
                        .add(crate::theme::Theme::secondary_button(format!(
                            "{} Undo",
                            crate::icons::UNDO
                        )))
                        .clicked()
                    {
                        self.signature_pad_raw_strokes.pop();
                    }
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                crate::theme::Theme::card_frame().show(ui, |ui| {
                    ui.heading(
                        egui::RichText::new(format!(
                            "{} Cryptographic PAdES Metadata",
                            crate::icons::LOCK
                        ))
                        .color(Color32::WHITE)
                        .size(14.0),
                    );
                    ui.add_space(4.0);
                    ui.checkbox(
                        &mut self.embed_digital_signature,
                        "Embed PAdES Digital Signature (SHA-256 Digest)",
                    );

                    if self.embed_digital_signature {
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Signer Name:")
                                    .color(crate::theme::Theme::TEXT_SECONDARY),
                            );
                            ui.text_edit_singleline(&mut self.signer_name_input);
                        });
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Reason:")
                                    .color(crate::theme::Theme::TEXT_SECONDARY),
                            );
                            ui.text_edit_singleline(&mut self.signature_reason_input);
                        });
                    }
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(crate::theme::Theme::primary_button(format!(
                            "{} Adopt & Place Signature",
                            crate::icons::CHECK
                        )))
                        .clicked()
                    {
                        self.adopt_signature_from_pad();
                    }
                    if ui
                        .add(crate::theme::Theme::secondary_button("Cancel"))
                        .clicked()
                    {
                        self.signature_modal_open = false;
                    }
                });
            });
        }

        // 6. Text Editing Modal Window
        if self.editing_text_modal_open {
            egui::Window::new(if self.editing_text_run_index.is_some() {
                format!("{} Edit Text Run", crate::icons::TOOL_EDIT_TEXT)
            } else {
                format!("{} Insert New Text Box", crate::icons::TOOL_EDIT_TEXT)
            })
            .collapsible(false)
            .resizable(true)
            .default_size(Vec2::new(420.0, 320.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "Page {} — {}",
                            self.editing_text_page + 1,
                            if self.editing_text_run_index.is_some() {
                                "Modify In-Place"
                            } else {
                                "New Text Block"
                            }
                        ))
                        .color(crate::theme::Theme::TEXT_MUTED)
                        .size(12.0),
                    );
                });
                ui.add_space(8.0);

                ui.label(
                    egui::RichText::new("Text Content:")
                        .strong()
                        .color(crate::theme::Theme::TEXT_PRIMARY),
                );
                ui.add_space(4.0);
                ui.add(
                    egui::TextEdit::multiline(&mut self.editing_text_buffer)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("Font Size:");
                    ui.add(
                        egui::DragValue::new(&mut self.editing_text_size)
                            .speed(0.5)
                            .range(6.0..=72.0)
                            .suffix(" pt"),
                    );

                    ui.add_space(16.0);
                    ui.label("Color:");
                    let mut egui_color = Color32::from_rgb(
                        self.editing_text_color[0],
                        self.editing_text_color[1],
                        self.editing_text_color[2],
                    );
                    if ui.color_edit_button_srgba(&mut egui_color).changed() {
                        self.editing_text_color = [egui_color.r(), egui_color.g(), egui_color.b()];
                    }
                });

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    if ui
                        .add(crate::theme::Theme::primary_button("Apply Changes"))
                        .clicked()
                    {
                        self.apply_text_edit();
                    }

                    if self.editing_text_run_index.is_some()
                        && ui
                            .add(crate::theme::Theme::secondary_button(format!(
                                "{} Delete Run",
                                crate::icons::TRASH
                            )))
                            .clicked()
                    {
                        self.delete_editing_text_run();
                    }

                    if ui
                        .add(crate::theme::Theme::secondary_button("Cancel"))
                        .clicked()
                    {
                        self.editing_text_modal_open = false;
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
