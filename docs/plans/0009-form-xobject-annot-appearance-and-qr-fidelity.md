# PLAN-0009: Form XObject Hierarchy, Annotation Appearance Streams & 1-Bit PNG Predictor Fidelity

- **Target Version**: v0.2.13
- **Author**: Antigravity (Advanced Agentic Coding)
- **Status**: Completed
- **Date**: 2026-10-07

---

## 1. Objective & Scope

### 1.1 Problem Statement
Certain complex PDF documents (notably electronically signed administrative notifications, reports, and certified documents with verification margin strips and QR codes) exhibit visual omissions:
1. **Missing Electronic Signature on Left Margin**:
   - Signature stamps and verification seals are defined as PDF Annotations (`/Annots`) with subtype `/Widget` or `/Stamp` and `/FT /Sig`.
   - Their visual representation resides in a normal appearance stream (`/AP << /N ... >>`).
   - The appearance stream is a Form XObject (`/Subtype /Form`) containing nested Form XObjects (`/FRM`, `/n2`), signature graphics (`/img0 Do`), and signer text runs (`Tj` / `TJ`).
   - Previously, the visual extraction engine (`extract_page_layout`) only read the page's `/Contents` stream and completely ignored `/Annots`, leaving electronic signatures invisible.
2. **Missing Margin Validation Information**:
   - The vertical text strip along the right margin containing the Secure Verification Code (CSV / CVD), validation URLs, and page numbering is contained inside Form XObjects (`/Subtype /Form`) invoked via `Do` (e.g. `/Xi3 Do` or `/Xi1 Do`).
   - Previously, `extract_page_layout` checked `Subtype == "Image"` upon encountering `Do` and ignored any Form XObjects, dropping all margin text.
3. **Missing or Corrupted QR Code**:
   - The verification QR code is an embedded 1-bit monochrome image (`/ColorSpace /DeviceGray`, `/BitsPerComponent 1`, `/Filter /FlateDecode`, `/DecodeParms <</Columns 35 /Colors 1 /Predictor 15 /BitsPerComponent 1>>`).
   - The flate-decompressed byte stream retains PNG filter bytes per row (e.g. 6 bytes per row: 1 filter byte + 5 bit-packed bytes for 35 pixels).
   - The image decoder did not unfilter PNG predictors (`Predictor 10..=15`) and assumed 8 bits per pixel for `DeviceGray`, causing the QR code to decode as white padding or distorted noise.
4. **Rotated Text and Coordinate Fidelity**:
   - Margin text is rotated 90 degrees via `Tm` or parent CTM (`cm`). Text runs must preserve rotation and transform coordinates accurately into visual space.

### 1.2 Target Capabilities
1. **Universal Form XObject (`/Subtype /Form`) Processor**:
   - Process Form XObjects invoked by `Do` in content streams and appearance streams.
   - Apply Form `/Matrix` and `/BBox` coordinate transformations recursively (with safe recursion depth limit to prevent circular references).
   - Support hierarchical resource inheritance (`/Resources` containing fonts, ToUnicode CMaps, and sub-XObjects).
2. **Annotation Appearance Stream (`/Annots` -> `/AP /N`) Renderer**:
   - Inspect page `/Annots` array and extract `/AP /N` normal appearance streams for widgets, signatures, stamps, and annotations.
   - Transform appearance stream coordinates from `/BBox` into annotation `/Rect`.
   - Extract text runs, vector shapes, and raster images from appearance streams into `PageVisualLayout`.
3. **1-Bit Monochrome & PNG Predictor Image Engine**:
   - Implement `unfilter_png_predictor` for PNG filter algorithms (None, Sub, Up, Average, Paeth) with `/Predictor 10..=15`.
   - Implement 1-bit monochrome bit-unpacking (`BitsPerComponent 1`) for `/DeviceGray`.
   - Correctly orient and bound images with arbitrary CTM rotation matrices.
4. **Rotated Text Visualization**:
   - Record text rotation angle in `PositionedText`.
   - Render rotated text cleanly in `kestrel-app` using `egui` text shape rotation.
5. **Strict Data Privacy & Synthetic Testing**:
   - Strictly 0 binary PDF files committed to disk or git.
   - Zero hardcoded local filesystem paths or user/institution PII.
   - 100% reproducible synthetic in-memory PDF generation.

---

## 2. Technical Specification

### 2.1 PNG Predictor & 1-Bit Image Unpacking (`crates/kestrel-core/src/document.rs`)

```rust
/// Unfilters scanlines encoded with PNG predictor (RFC 2083 / ISO 32000-1 §7.4.4.4).
fn unfilter_png_predictor(
    raw: &[u8],
    columns: usize,
    colors: usize,
    bits_per_component: usize,
) -> Vec<u8>;

/// Unpacks 1-bit per component monochrome pixels into 8-bit grayscale values.
fn unpack_1bit_pixels(unfiltered_bytes: &[u8], width: usize, height: usize) -> Vec<u8>;
```

### 2.2 Form XObject & Resource Context Architecture

```rust
#[derive(Clone, Default)]
pub struct ResourceContext {
    pub font_cmaps: HashMap<Vec<u8>, ToUnicodeCMap>,
    pub font_encodings: HashMap<Vec<u8>, String>,
    pub xobjects: HashMap<Vec<u8>, ObjectId>,
}

fn process_content_operations(
    doc: &lopdf::Document,
    operations: &[lopdf::content::Operation],
    resources: &ResourceContext,
    gstate: &mut GraphicsGState,
    gstate_stack: &mut Vec<GraphicsGState>,
    text_runs: &mut Vec<PositionedText>,
    rects: &mut Vec<VectorRect>,
    images: &mut Vec<VisualImage>,
    media_x0: f32,
    media_y0: f32,
    depth: usize,
);
```

### 2.3 Annotation Appearance Stream Processing

```rust
fn process_page_annotations(
    doc: &lopdf::Document,
    page_id: ObjectId,
    page_resources: &ResourceContext,
    text_runs: &mut Vec<PositionedText>,
    rects: &mut Vec<VectorRect>,
    images: &mut Vec<VisualImage>,
    media_x0: f32,
    media_y0: f32,
);
```

---

## 3. Test-Driven Development (TDD) Strategy

### 3.1 Integration Tests (`crates/kestrel-core/tests/integration_tests.rs`)
1. `test_integration_form_xobject_text_and_rect_extraction`:
   - Builds synthetic PDF with a Form XObject invoked via `Do`.
   - Asserts text inside the Form XObject is extracted at the transformed position.
2. `test_integration_annot_signature_appearance_stream_extraction`:
   - Builds synthetic PDF with `/Annots` containing a signature widget with `/AP << /N ... >>`.
   - Asserts signer details and visual stamp are extracted into `PageVisualLayout`.
3. `test_integration_1bit_monochrome_image_png_predictor_decoding`:
   - Constructs a synthetic 1-bit monochrome image (like a QR code) with `Predictor 15`.
   - Asserts all pixels decode cleanly to RGBA without white truncation or distortion.
4. `test_integration_nested_form_xobject_rotation`:
   - Builds nested Form XObjects with 90-degree rotation matrices.
   - Asserts nested text and graphics are properly positioned and oriented.

---

## 4. Phased Execution Steps

1. [x] **Phase 1**: Implement Red failing tests in `crates/kestrel-core/tests/integration_tests.rs`.
2. [x] **Phase 2**: Implement `unfilter_png_predictor` and 1-bit monochrome unpacking in `crates/kestrel-core/src/document.rs`.
3. [x] **Phase 3**: Implement `ResourceContext` and recursive Form XObject processing in `process_content_operations`.
4. [x] **Phase 4**: Implement `/Annots` appearance stream extraction in `extract_page_layout`.
5. [x] **Phase 5**: Update `PositionedText` with rotation support and render rotated text in `crates/kestrel-app/src/app.rs`.
6. [x] **Phase 6**: Verify all tests turn Green and run full verification battery (`cargo fmt`, `cargo clippy`, `cargo test`, `cargo check wasm32`).
7. [x] **Phase 7**: Open PR, verify 8 CI checks, merge, and publish release `v0.2.13`.

---

## 5. Acceptance Criteria

- [x] Form XObjects (`/Subtype /Form`) invoked via `Do` extract text, shapes, and images.
- [x] Nested Form XObjects are processed recursively with matrix transformations.
- [x] Annotation appearance streams (`/AP /N`) are rendered on the page canvas at their `/Rect`.
- [x] 1-bit monochrome images with PNG predictors (10..=15) decode accurately into RGBA bitmaps.
- [x] Zero warnings in `cargo clippy`, 100% pass rate in CI matrix across Linux, macOS, Windows, and WASM.
- [x] Strictly zero binary PDF files or PII committed.
