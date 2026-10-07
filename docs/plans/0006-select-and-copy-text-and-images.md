# PLAN-0006: High-Fidelity Text and Image Selection & Clipboard Copy Engine

- **Target Version**: v0.2.10
- **Author**: Antigravity (Advanced Agentic Coding)
- **Status**: Draft
- **Date**: 2026-10-07

---

## 1. Objective & Scope

### 1.1 Problem Statement
While Kestrel-PDF renders text and embedded raster images with high visual fidelity, users cannot currently select text passages or embedded images on the page, nor can they copy selected text or image data to their system clipboard. Modern PDF readers (e.g. Adobe Acrobat, Apple Preview, PDF Expert) provide intuitive text and image selection with immediate visual feedback, keyboard shortcuts (`Ctrl+C` / `Cmd+C`), and multi-format clipboard copying.

### 1.2 Target Capabilities
1. **Interactive Selection Mode**:
   - In `ActiveTool::SelectText` ("📝 Select"), enable full point-and-drag interaction over rendered pages.
   - **Text Selection**: Click-and-drag marquee box or click on individual text runs to select text. Highlight all selected text runs with semi-transparent accent blue rectangles.
   - **Image Selection**: Single-click on any embedded raster image to select it. Highlight selected images with a crisp accent border, corner selection handles, and an informational badge ("🖼️ Image W×H px").
2. **System Clipboard Copying**:
   - **Copy Text**: Copy selected text runs concatenated in logical reading order to the system clipboard via `egui::Context::copy_text` and native platform clipboard.
   - **Copy Image**: Copy selected image as an RGBA bitmap to the system clipboard on desktop platforms (`arboard`), and encode to PNG bytes with fallback data URI and image info.
   - **Keyboard Shortcuts**: Standard `Ctrl+C` (Windows/Linux) and `Cmd+C` (macOS) copy whichever item (text or image) is currently selected. `Escape` clears active selection.
3. **Contextual Action Toolbar & Status Feedback**:
   - Display dynamic action buttons in Tier 2 toolbar when items are selected (`📋 Copy Text`, `📋 Copy Image`, `❌ Clear Selection`).
   - Floating contextual action bubble near the active selection.
   - Status toast notification confirming copy action (e.g. "Copied 42 characters to clipboard", "Copied image 256×256 px to clipboard").

### 1.3 Out of Scope for Phase 1
- Optical Character Recognition (OCR) for raster images without underlying text streams (deferred to subsequent phase).
- Arbitrary non-rectangular polygon lasso selection.

---

## 2. Technical Specification

### 2.1 Core Layout Spatial Queries (`crates/kestrel-core/src/document.rs`)

We extend `PageVisualLayout` with geometric query methods that map visual canvas points (origin top-left, rotated appropriately) to underlying PDF elements:

```rust
impl PageVisualLayout {
    /// Finds all text runs whose visual bounding box intersects the given visual rectangle [min_x, min_y, max_x, max_y].
    pub fn find_text_runs_in_rect(
        &self,
        rect: [f32; 4],
        rotation: u16,
    ) -> Vec<(usize, &PositionedText)>;

    /// Concatenates text from all text runs intersecting the rectangle in logical reading order.
    pub fn get_text_in_rect(&self, rect: [f32; 4], rotation: u16) -> String;

    /// Finds the top-most embedded image containing the visual point (visual_x, visual_y).
    pub fn find_image_at_point(
        &self,
        visual_x: f32,
        visual_y: f32,
        rotation: u16,
    ) -> Option<(usize, &VisualImage)>;

    /// Finds embedded images intersecting the visual rectangle.
    pub fn find_images_in_rect(
        &self,
        rect: [f32; 4],
        rotation: u16,
    ) -> Vec<(usize, &VisualImage)>;

    /// Encodes a specific embedded image into PNG bytes.
    pub fn encode_image_png(&self, image_index: usize) -> anyhow::Result<Vec<u8>>;
}

/// Encodes an arbitrary RGBA byte buffer into PNG format.
pub fn encode_rgba_to_png(rgba: &[u8], width: u32, height: u32) -> anyhow::Result<Vec<u8>>;
```

### 2.2 App Selection State (`crates/kestrel-app/src/app.rs`)

We introduce `SelectionState` to track active selections without interfering with scroll state or document rendering:

```rust
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SelectionState {
    /// Page index (0-based) where selection is currently active
    pub page_index: Option<usize>,
    /// Drag start coordinates in visual PDF points (unscaled by zoom)
    pub drag_start_pt: Option<egui::Pos2>,
    /// Drag current coordinates in visual PDF points (unscaled by zoom)
    pub drag_current_pt: Option<egui::Pos2>,
    /// Indices of selected text runs on `page_index`
    pub selected_text_indices: Vec<usize>,
    /// Compiled string of selected text
    pub selected_text: Option<String>,
    /// Index of selected image on `page_index`
    pub selected_image_index: Option<usize>,
}

impl SelectionState {
    pub fn clear(&mut self);
    pub fn is_empty(&self) -> bool;
    pub fn has_text(&self) -> bool;
    pub fn has_image(&self) -> bool;
}
```

### 2.3 Cross-Platform Clipboard Architecture

- **Desktop (Windows, macOS, Linux)**:
  - Text: `egui::Context::copy_text(&text)` + `arboard::Clipboard::set_text(&text)`.
  - Image: `arboard::Clipboard::set_image(arboard::ImageData { width, height, bytes: Cow::Borrowed(&rgba) })`.
- **WebAssembly (wasm32)**:
  - Text: `egui::Context::copy_text(&text)`.
  - Image: `egui::Context::copy_text(&data_uri)` (Base64 PNG fallback) + informative status toast.

---

## 3. Test-Driven Development (TDD) Strategy

### 3.1 Core Spatial Query Tests (`crates/kestrel-core/tests/integration_tests.rs`)
- `test_text_selection_spatial_query`:
  Construct synthetic PDF in RAM with known text positions ("Alpha", "Beta", "Gamma"). Query bounding boxes covering "Alpha" and "Beta", assert returned text is "Alpha Beta".
- `test_image_selection_and_png_encoding`:
  Construct synthetic PDF in RAM with a 16×16 colored image. Query point inside image bounds, assert returned image index and dimensions match. Test `encode_rgba_to_png` produces valid PNG magic header (`\x89PNG\r\n\x1a\n`).

### 3.2 App Interaction E2E Smoke Tests (`crates/kestrel-app/tests/smoke_test.rs`)
- `test_e2e_select_and_copy_text`:
  Load in-memory synthetic PDF, set `ActiveTool::SelectText`, simulate selection rectangle over text, verify `selected_text` is populated, call `copy_selected_text()`, verify status toast feedback.
- `test_e2e_select_and_copy_image`:
  Load in-memory synthetic PDF with image, simulate selecting image, verify `selected_image_index`, call `copy_selected_image()`, verify status toast feedback.
- `test_e2e_clear_selection`:
  Verify `clear()` resets text and image selection state.

---

## 4. Phased Execution Steps

1. [ ] **Phase 1**: Write Red failing tests in `crates/kestrel-core/tests/integration_tests.rs`.
2. [ ] **Phase 2**: Implement layout query methods (`find_text_runs_in_rect`, `find_image_at_point`, `encode_rgba_to_png`) in `crates/kestrel-core/src/document.rs`. Make core tests pass (Green).
3. [ ] **Phase 3**: Add `SelectionState` and UI rendering in `crates/kestrel-app/src/app.rs` (canvas drag handler, highlights, corner handles, Tier 2 action buttons, shortcut keys).
4. [ ] **Phase 4**: Add E2E tests in `crates/kestrel-app/tests/smoke_test.rs`.
5. [ ] **Phase 5**: Run full pre-flight verification (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo check wasm32`).
6. [ ] **Phase 6**: Open PR, verify CI matrix across 8 jobs, merge to `main`, and tag release `v0.2.10`.

---

## 5. Acceptance Criteria

- [ ] Users can drag a selection box to select text runs on any page in `Select` mode.
- [ ] Users can click on an embedded image to select it in `Select` mode.
- [ ] `Ctrl+C` / `Cmd+C` copies selected text or image immediately.
- [ ] Tier 2 toolbar provides explicit `📋 Copy Text`, `📋 Copy Image`, and `❌ Clear` controls when selections exist.
- [ ] Native image clipboard support on Windows, macOS, and Linux.
- [ ] 0 local PDF files committed; 100% synthetic in-memory fixtures.
- [ ] All 8 CI matrix checks pass with 0 warnings.
