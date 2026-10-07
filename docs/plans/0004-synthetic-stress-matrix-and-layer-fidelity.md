# PLAN-0004: In-Memory Synthetic PDF Stress Matrix, Concurrency & Layer Engine

- **Target Version**: v0.2.8
- **Author**: Antigravity Pair Programmer
- **Status**: In Progress
- **Date**: 2026-10-07

---

## 1. Objective & Scope

Empower **Kestrel-PDF** as the highest-performance, most resilient PDF reader and editor by creating a comprehensive in-memory PDF stress matrix and parallel test harness.

### Key Requirements
1. **Zero Disk / Zero Commit Invariant**: All PDF files must be generated dynamically in RAM via byte buffers (`Vec<u8>`). Under no circumstances will any PDF file be written to disk or tracked by git.
2. **Exhaustive Feature Spectrum**: Construct a spectrum of documents from the simplest valid PDF 1.7 to overly complex documents featuring:
   - Universal character sets (ASCII, Latin Extended, Cyrillic, Greek, CJK, RTL Hebrew/Arabic, mathematical symbols, currencies, emojis, escaped delimiters).
   - Advanced vector graphics, clipping paths, affine CTM transformations, multi-width strokes, CMYK/Gray/RGB colorspaces.
   - Embedded raster images across multiple formats (RGB, Grayscale, CMYK, JPEG containers, `/SMask` soft mask alpha channels).
   - Complex interactive AcroForms (text fields, checkboxes, dropdowns, UTF-16BE encoding, varied geometries).
   - Optional Content Groups (OCG / Layers) with catalog `/OCProperties`, layer hierarchies, and toggle states.
   - Multi-page mixed orientations (0°, 90°, 180°, 270°) and extreme dimension ratios (postage stamp to oversized blueprint).
   - Boundary-case and malformed stream resilience.
3. **Parallel Multi-Threaded Stress Testing**:
   - Concurrently parse, inspect, layout, and search all synthetic documents across parallel threads.
   - Rapidly switch between all documents inside `KestrelApp`, simulating full UI frame rendering, tab switching, and tool activation without crashes or data races.

---

## 2. Technical Architecture & Design

### 2.1 PDF Optional Content Groups (OCG / Layers)
In PDF 1.5+, Optional Content Groups allow CAD layers, multi-language overlays, watermarks, and design separations:
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct LayerInfo {
    pub id: Option<lopdf::ObjectId>,
    pub name: String,
    pub visible: bool,
}
```
- In `DocumentSession`: parse `/OCProperties` from Catalog root.
  - Read `/OCGs` array to identify layer objects (`<< /Type /OCG /Name (Layer Name) >>`).
  - Read `/D` dictionary to determine initial visibility (`/ON` / `/OFF`).
  - Store extracted layers in `session.layers`.
- In `KestrelApp`:
  - Add `SidebarTab::Layers` to allow inspecting and toggling layer visibility.
  - Render layer controls in the sidebar when layers exist in the active document.

### 2.2 The 8-Tier In-Memory Synthetic Matrix

| Tier | Identifier | Key Capabilities Exercised |
| :--- | :--- | :--- |
| **Tier 1** | `MinimalBarebones` | Simplest valid PDF 1.7: 1 page, 1 standard font, minimal dictionary tree, 0 annotations. |
| **Tier 2** | `UnicodeMultiScript` | Multi-script typography: Latin Accents, Greek, Cyrillic, CJK, RTL Arabic/Hebrew, Math symbols ($\sum, \int, \infty$), Currency ($€, \$, ¥, ₿$), Emojis (🚀, 🦀), Escaped parenthesis `\(\)`. |
| **Tier 3** | `VectorAffineGeometry` | Line widths (0.25 to 10pt), dashed strokes, clipping rects, CMYK/Gray/RGB vector fills, inverted vertical CTMs, arbitrary rotation matrices. |
| **Tier 4** | `RasterImagesSMask` | Raw RGB, Grayscale, CMYK raster buffers, JPEG magic byte streams, `/SMask` per-pixel alpha transparency, inverted CTM positioning. |
| **Tier 5** | `AcroFormExhaustive` | Multi-line text, UTF-16BE BOM values, compact 8pt height fields, checked/unchecked checkboxes, multi-option dropdown choices, read-only fields. |
| **Tier 6** | `OptionalContentLayers` | PDF 1.5+ `/OCProperties`, `/OCGs`, `/D` config dict, marked content `/OC /MC... BDC ... EMC`, multiple layers ("Floorplan", "Electrical", "Watermark"). |
| **Tier 7** | `MixedDimensionsOrientations` | Mixed 0°, 90°, 180°, 270° in 1 document; postage stamp ($72 \times 72$), A4 ($595 \times 842$), poster ($1600 \times 1200$); nested bookmarks (`/Outlines`). |
| **Tier 8** | `ResilienceBoundaryStream` | Stream length edge cases, trailing garbage, empty contents, missing optional dictionaries. |

### 2.3 Parallel Test Architecture (`smoke_test.rs`)
- Concurrent test workers running in `std::thread::scope`:
  - Worker 1..N generate each matrix PDF simultaneously in memory.
  - Concurrently instantiate `DocumentSession` for each generated buffer.
  - Verify layout extraction, text search, and form mutation simultaneously across threads.
- E2E App Sequential & Switching Stress Test:
  - Open `KestrelApp`, load document 1 through 8 in rapid succession.
  - For each document, execute egui frame simulation with simulated screen dimensions.
  - Verify title bar text, page counts, aspect ratios, zoom fitting, and sidebar tabs (Thumbnails, Outlines, Forms, Layers).

---

## 3. Test-Driven Development (TDD) Strategy

- **Phase 1 (Red)**:
  - Add `LayerInfo` struct to `kestrel-core::document`.
  - Add synthetic generation methods in `kestrel-core::synthetic` for layers, extreme typography, vector geometry, and multi-format images.
  - Write parallel E2E smoke tests in `crates/kestrel-app/tests/smoke_test.rs`.
- **Phase 2 (Green)**:
  - Implement `/OCProperties` layer parsing in `kestrel-core::document::DocumentSession`.
  - Add `SidebarTab::Layers` to `kestrel-app` and render layer listing in the sidebar.
  - Make all parallel and E2E tests pass.
- **Phase 3 (Refactor & Verify)**:
  - Verify zero clippy warnings (`-D warnings`), zero formatting issues (`cargo fmt`).
  - Verify cross-platform checks (Linux, macOS, Windows, WASM).

---

## 4. Acceptance Criteria

- [ ] All 8 synthetic PDF tiers construct purely in RAM (0 files written to disk).
- [ ] Concurrent multi-threaded test passes without data races, panics, or deadlocks.
- [ ] OCG layers are correctly detected, parsed, and exposed in `DocumentSession::layers`.
- [ ] `KestrelApp` smoothly opens and switches between all 8 documents with zero errors.
- [ ] All workspace tests pass (`cargo test --workspace`).
- [ ] Zero Clippy warnings across all targets.
