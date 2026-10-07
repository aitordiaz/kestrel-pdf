# PLAN-0003: Image Placement, Alpha SMask & AcroForm Layout Fidelity

- **Target Version**: v0.2.6
- **Author**: Antigravity Pair Programmer
- **Status**: Completed
- **Date**: 2026-10-07

## 1. Objective & Scope

Resolve real-world PDF layout defects reported when inspecting invoices (`Factura.pdf`) and administrative forms (`REPSOL_CAT_Cambio de titular_ACT_CAT_03.pdf`):
1. **Embedded Image Positioning**: When PDF content streams use negative vertical scaling in the Current Transformation Matrix (`cm [w, 0, 0, -h, x, y]`), images are vertically offset by $2 \times h$, detaching category icons and illustrations from their text counterparts.
2. **Image Decompression & Transparency**: JPEG DCT streams and transparent Flate images with Soft Masks (`/SMask`) must decode properly rather than treating compressed bytes as raw RGB or ignoring transparency channels.
3. **AcroForm Layout & Sizing Overlap**: Form widgets currently enforce an arbitrary minimum size (`.max(20.0)` height and `.max(24.0)` width) and an opaque 78% alpha blue background, causing form fields to expand over adjacent text lines and obscure labels, underlines, and form prompts.
4. **UTF-16BE Form String Decoding**: Form field values encoded in 2-byte UTF-16BE (with Byte Order Mark `0xFE, 0xFF`) must decode cleanly instead of generating corrupted characters and null bytes.

---

## 2. Technical Specification

### 2.1 Full-Affine Bounding Box for Embedded Images (`kestrel-core`)
In PDF content streams, the unit square $[0, 1] \times [0, 1]$ is transformed by `gstate.ctm`:
```rust
let p0 = transform_point(&gstate.ctm, 0.0, 0.0);
let p1 = transform_point(&gstate.ctm, 1.0, 0.0);
let p2 = transform_point(&gstate.ctm, 1.0, 1.0);
let p3 = transform_point(&gstate.ctm, 0.0, 1.0);

let min_x = p0.0.min(p1.0).min(p2.0).min(p3.0) - media_x0;
let max_x = p0.0.max(p1.0).max(p2.0).max(p3.0) - media_x0;
let min_y = p0.1.min(p1.1).min(p2.1).min(p3.1) - media_y0;
let max_y = p0.1.max(p1.1).max(p2.1).max(p3.1) - media_y0;
```
- Store `x: min_x`, `y: min_y`, `width: max_x - min_x`, `height: max_y - min_y`.
- In `kestrel-app`, `map_pdf_point_to_visual(img.x, img.y + img.height, ...)` maps the top edge `(min_x, max_y)` to visual top-left coordinates `(vx, vy)` consistently for both positive and inverted CTM transforms.

### 2.2 JPEG Container & SMask Alpha Decoding (`kestrel-core`)
- In `convert_image_bytes_to_rgba`, if `raw_bytes` begins with JPEG magic bytes `[0xFF, 0xD8, 0xFF]`, decode via `image::load_from_memory` to obtain clean RGBA pixels.
- If the XObject stream dictionary references an `/SMask` soft mask, extract and decompress the mask stream, assigning the mask bytes to the alpha channel (`rgba[i * 4 + 3] = smask[i]`).

### 2.3 AcroForm Widget Dimension & Transparency (`kestrel-app`)
- Remove `.max(20.0)` height and `.max(24.0)` width constraints on widget geometry:
  `f_w = fw * self.zoom_level`
  `f_h = fh * self.zoom_level`
- Never render an opaque 78% alpha blue background over the page.
  - In normal viewing mode (`active_tool != ActiveTool::FormFill`), draw transparent background (`Color32::TRANSPARENT`), allowing all text, lines, and underlines beneath to show with full fidelity.
  - In `ActiveTool::FormFill` mode, render a delicate 18% translucent tint (`rgba(219, 234, 254, 45)`) and a subtle 1px stroke, preserving legibility of underlying content.
- Dynamically scale field value text to fit field height: `(f_h * 0.82).clamp(8.0, 32.0)` with padding proportional to zoom.

### 2.4 UTF-16BE Form String Decoding (`kestrel-core`)
- In `parse_widget_dict`, detect UTF-16BE BOM (`0xFE, 0xFF`) and decode via `String::from_utf16_lossy` or `lopdf::Document::decode_text(None, bytes)`.

---

## 3. Test-Driven Development (TDD) Strategy

- **Test 1**: Verify inverted CTM image placement (`cm [w, 0, 0, -h, x, y]`) extracts exact expected bounding box with `y + height` matching the top coordinate.
- **Test 2**: Verify image SMask alpha extraction produces transparent alpha channels.
- **Test 3**: Verify UTF-16BE form field string decodes to accented Spanish characters (`AITOR DÍAZ MEDINA`).
- **Test 4**: E2E UI test ensuring form fields with compact height ($\le 12\text{ pt}$) do not expand to 20 pt and retain transparent/translucent backgrounds preserving underlying text visibility.

---

## 4. Phased Execution Steps

1. [x] Step 1: Write SDD Specification in `docs/plans/0003-image-placement-and-form-layout-fidelity.md`.
2. [x] Step 2: Add failing unit/integration tests in `crates/kestrel-core/tests/` and `crates/kestrel-app/tests/`.
3. [x] Step 3: Implement CTM bounding box calculation, SMask alpha, and UTF-16BE form decoding in `kestrel-core`.
4. [x] Step 4: Implement exact widget sizing and translucent/transparent form rendering in `kestrel-app`.
5. [x] Step 5: Verify all tests pass, run pre-flight checks (`fmt`, `clippy`, `wasm check`).
6. [x] Step 6: Open Pull Request and merge.

---

## 5. Acceptance Criteria

- [x] Images placed with negative vertical scale (`cm [w, 0, 0, -h, x, y]`) align accurately with their textual baselines.
- [x] JPEG images and Flate images with `/SMask` decode cleanly without artifacting or black backgrounds.
- [x] Form fields respect their exact PDF bounding boxes without overlapping neighboring lines.
- [x] Document text beneath form inputs remains clearly visible.
- [x] All 32 workspace tests pass cleanly with zero Clippy warnings.
