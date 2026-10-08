# PLAN-0016: Phase 3 — In-Place Text & Image Editing and Page Tree Mutations

- **Target Version**: v0.3.0
- **Author**: Antigravity & Core Engineering Team
- **Status**: Completed
- **Date**: 2026-10-08

---

## 1. Objective & Scope

### Problem Statement
In versions up to v0.2.19, Kestrel-PDF provided high-speed rendering, AcroForm filling, and digital/visual contract signing, but users could not edit text runs, insert text annotations, manipulate embedded images, or reorder/delete/insert pages directly inside their PDFs.

### Target Capabilities (Phase 3 of Roadmap)
1. **Object Inspection & Hit-Testing**: Point-and-click selection for text runs, vector rects, and embedded raster images, with visual bounding boxes and corner transform handles.
2. **In-Place Text Editing & Insertion**:
   - In-place modification of existing text runs in PDF content streams.
   - Text box insertion with user-configurable font size, position, and color.
   - Deletion of individual text runs.
3. **Image Manipulation**:
   - Replacement of embedded raster images (XObjects) with arbitrary RGBA images.
   - Insertion of new image XObjects into page content streams.
   - Deletion of existing image XObjects.
4. **Document Structure Organization**:
   - Insertion of blank pages at any index.
   - Deletion of pages (with safeguard against deleting the last page).
   - Reordering of pages (move up, move down, swap).
   - Duplication of existing pages.

---

## 2. Technical Specification & API Contracts

### A. Core Engine (`kestrel-core::document::DocumentSession`)
```rust
impl DocumentSession {
    // Text editing
    pub fn modify_text_run(&mut self, page_index: usize, run_index: usize, new_text: &str) -> Result<()>;
    pub fn insert_text_box(&mut self, page_index: usize, text: &str, x: f32, y: f32, font_size: f32, color: [u8; 3]) -> Result<()>;
    pub fn delete_text_run(&mut self, page_index: usize, run_index: usize) -> Result<()>;

    // Image manipulation
    pub fn replace_image(&mut self, page_index: usize, image_index: usize, new_rgba: &[u8], width: u32, height: u32) -> Result<()>;
    pub fn insert_image(&mut self, page_index: usize, rgba: &[u8], width: u32, height: u32, x: f32, y: f32, width_pt: f32, height_pt: f32) -> Result<()>;
    pub fn delete_image(&mut self, page_index: usize, image_index: usize) -> Result<()>;

    // Page tree mutations
    pub fn insert_blank_page(&mut self, at_index: usize, width_pt: f32, height_pt: f32) -> Result<()>;
    pub fn delete_page(&mut self, page_index: usize) -> Result<()>;
    pub fn reorder_page(&mut self, from_index: usize, to_index: usize) -> Result<()>;
    pub fn duplicate_page(&mut self, page_index: usize) -> Result<()>;
}
```

### B. Application State (`kestrel-app::app::KestrelApp`)
- `ActiveTool::EditText`: Click to select text run or insert text box at click coordinates; inline editing modal/popover.
- `ActiveTool::EditImage`: Click to select image; display image manipulation actions (Replace, Delete, Insert); corner resize handles.
- Thumbnails sidebar & toolbar: Add Page, Delete Page, Move Up, Move Down buttons.

---

## 3. Test-Driven Development (TDD) Strategy

### Failing Tests (Red Phase):
1. **`test_page_tree_insert_delete_reorder_duplicate`**: Tests inserting a blank page, reordering pages, duplicating a page, and deleting a page, verifying that `page_count` and layout geometry update properly.
2. **`test_in_place_text_edit_and_insert_text_box`**: Tests inserting a new text box on page 0 and modifying an existing text run, asserting that the new text is discoverable via `search_text` and visual layout.
3. **`test_image_manipulation_replace_and_delete`**: Tests replacing an image XObject with a new 64x64 RGBA texture and deleting an image, verifying visual layout and XObject counts.
4. **`test_e2e_edit_tools_and_page_organization`**: E2E smoke test verifying tool switching to `ActiveTool::EditText` and `ActiveTool::EditImage`, inserting text, and triggering page tree changes.

---

## 4. Phased Implementation Steps

1. **Phase 3A: Engine Page Tree Mutations** (`insert_blank_page`, `delete_page`, `reorder_page`, `duplicate_page`).
2. **Phase 3B: Engine Text Editing & Insertion** (`modify_text_run`, `insert_text_box`, `delete_text_run`).
3. **Phase 3C: Engine Image Manipulation** (`replace_image`, `insert_image`, `delete_image`).
4. **Phase 3D: UI Integration** (wiring `EditText`, `EditImage`, popover controls, page organizer buttons into `kestrel-app`).
5. **Phase 3E: Verification & Release** (formatting, clippy, all unit/integration/E2E tests, PR, and tag).
