use anyhow::{Context, Result};
use lopdf::{Dictionary, Object, Stream};
use std::path::{Path, PathBuf};

use crate::forms::{self, FormField};
use crate::sign::{self, DigitalSignatureMeta, VisualSignature};

/// Geometry and basic information about a single PDF page.
#[derive(Debug, Clone, PartialEq)]
pub struct PageInfo {
    pub index: u16,
    pub width_pt: f32,
    pub height_pt: f32,
    pub rotation_degrees: u16,
}

impl PageInfo {
    /// Returns the effective visual dimensions (width, height) accounting for 90° and 270° rotations.
    pub fn visual_dimensions(&self) -> (f32, f32) {
        if self.rotation_degrees % 180 == 90 {
            (self.height_pt, self.width_pt)
        } else {
            (self.width_pt, self.height_pt)
        }
    }
}

/// Outline / Table of Contents entry.
#[derive(Debug, Clone, PartialEq)]
pub struct OutlineItem {
    pub title: String,
    pub target_page: u16,
    pub children: Vec<OutlineItem>,
}

/// Optional Content Group (OCG / Layer) metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerInfo {
    pub name: String,
    pub visible: bool,
}

/// Search match location and context snippet.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchResult {
    pub page_index: u16,
    pub snippet: String,
    pub match_count: usize,
}

/// Individual positioned text fragment extracted from a PDF content stream.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionedText {
    pub text: String,
    pub x: f32, // PDF points (y=0 at bottom-left of MediaBox)
    pub y: f32, // PDF points (y=0 at bottom-left of MediaBox)
    pub font_size: f32,
    pub color: [u8; 3],    // RGB [0..255]
    pub rotation_deg: f32, // Rotation angle in degrees (0.0 = horizontal)
}

/// Vector rectangle (shape, border, cell highlight) extracted from PDF graphics stream.
#[derive(Debug, Clone, PartialEq)]
pub struct VectorRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub fill_color: Option<[u8; 3]>,
    pub stroke_color: Option<[u8; 3]>,
    pub stroke_width: f32,
}

/// Embedded raster image extracted from a PDF page content stream.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualImage {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub rgba: Vec<u8>,
}

/// Visual layout representation of a single PDF page for high-fidelity rendering.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PageVisualLayout {
    pub width_pt: f32,
    pub height_pt: f32,
    pub text_runs: Vec<PositionedText>,
    pub rects: Vec<VectorRect>,
    pub images: Vec<VisualImage>,
    pub plain_text: String,
}

impl PageVisualLayout {
    /// Computes the visual bounding box [min_x, min_y, max_x, max_y] of a text run.
    pub fn text_run_visual_bounds(&self, tr: &PositionedText, rotation: u16) -> [f32; 4] {
        let (vx, vy) = map_pdf_point_to_visual(
            tr.x,
            tr.y + tr.font_size * 0.85,
            self.width_pt,
            self.height_pt,
            rotation,
        );
        let approx_w = (tr.text.chars().count() as f32) * tr.font_size * 0.55 + 4.0;
        let approx_h = tr.font_size;
        let total_rot = ((tr.rotation_deg - rotation as f32).round() as i32).rem_euclid(360);
        if (45..=135).contains(&total_rot) || (225..=315).contains(&total_rot) {
            let min_x = vx.min(vx + approx_h);
            let max_x = vx.max(vx + approx_h);
            let min_y = vy.min(vy + approx_w);
            let max_y = vy.max(vy + approx_w);
            [min_x, min_y, max_x, max_y]
        } else {
            let min_x = vx.min(vx + approx_w);
            let max_x = vx.max(vx + approx_w);
            let min_y = vy.min(vy + approx_h);
            let max_y = vy.max(vy + approx_h);
            [min_x, min_y, max_x, max_y]
        }
    }

    /// Computes the visual bounding box [min_x, min_y, max_x, max_y] of an embedded image.
    pub fn image_visual_bounds(&self, img: &VisualImage, rotation: u16) -> [f32; 4] {
        let (vx, vy) = map_pdf_point_to_visual(
            img.x,
            img.y + img.height,
            self.width_pt,
            self.height_pt,
            rotation,
        );
        let (iw, ih) = if rotation % 180 == 90 {
            (img.height, img.width)
        } else {
            (img.width, img.height)
        };
        let min_x = vx.min(vx + iw);
        let max_x = vx.max(vx + iw);
        let min_y = vy.min(vy + ih);
        let max_y = vy.max(vy + ih);
        [min_x, min_y, max_x, max_y]
    }

    /// Finds all text runs whose visual bounding box intersects the given visual rectangle [min_x, min_y, max_x, max_y].
    /// Returns elements ordered logically in reading order (top-to-bottom, left-to-right).
    pub fn find_text_runs_in_rect(
        &self,
        rect: [f32; 4],
        rotation: u16,
    ) -> Vec<(usize, &PositionedText)> {
        let q_min_x = rect[0].min(rect[2]);
        let q_max_x = rect[0].max(rect[2]);
        let q_min_y = rect[1].min(rect[3]);
        let q_max_y = rect[1].max(rect[3]);

        let mut matched = Vec::new();
        for (idx, tr) in self.text_runs.iter().enumerate() {
            let b = self.text_run_visual_bounds(tr, rotation);
            let b_min_x = b[0].min(b[2]);
            let b_max_x = b[0].max(b[2]);
            let b_min_y = b[1].min(b[3]);
            let b_max_y = b[1].max(b[3]);

            if b_min_x <= q_max_x && b_max_x >= q_min_x && b_min_y <= q_max_y && b_max_y >= q_min_y
            {
                matched.push((idx, tr, b_min_x, b_min_y, tr.font_size));
            }
        }

        // Sort in natural reading order
        matched.sort_by(|a, b| {
            let line_threshold = (a.4.max(b.4) * 0.6).max(4.0);
            if (a.3 - b.3).abs() > line_threshold {
                a.3.partial_cmp(&b.3).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        matched
            .into_iter()
            .map(|(idx, tr, _, _, _)| (idx, tr))
            .collect()
    }

    /// Concatenates text from all text runs intersecting the rectangle in logical reading order.
    pub fn get_text_in_rect(&self, rect: [f32; 4], rotation: u16) -> String {
        let runs = self.find_text_runs_in_rect(rect, rotation);
        if runs.is_empty() {
            return String::new();
        }

        let mut result = String::new();
        let mut last_y = f32::MIN;
        let mut last_size: f32 = 12.0;

        for (_, tr) in runs {
            let b = self.text_run_visual_bounds(tr, rotation);
            let vy = b[1];
            let font_size = tr.font_size;
            let line_threshold = (last_size.max(font_size) * 0.6).max(4.0);

            if last_y > f32::MIN && (vy - last_y).abs() > line_threshold {
                result.push('\n');
            } else if !result.is_empty() && !result.ends_with(' ') && !result.ends_with('\n') {
                result.push(' ');
            }

            result.push_str(&tr.text);
            last_y = vy;
            last_size = font_size;
        }

        result
    }

    /// Finds the top-most embedded image containing the visual point (visual_x, visual_y).
    pub fn find_image_at_point(
        &self,
        visual_x: f32,
        visual_y: f32,
        rotation: u16,
    ) -> Option<(usize, &VisualImage)> {
        for (idx, img) in self.images.iter().enumerate().rev() {
            let b = self.image_visual_bounds(img, rotation);
            if visual_x >= b[0] && visual_x <= b[2] && visual_y >= b[1] && visual_y <= b[3] {
                return Some((idx, img));
            }
        }
        None
    }

    /// Finds embedded images intersecting the visual rectangle.
    pub fn find_images_in_rect(&self, rect: [f32; 4], rotation: u16) -> Vec<(usize, &VisualImage)> {
        let q_min_x = rect[0].min(rect[2]);
        let q_max_x = rect[0].max(rect[2]);
        let q_min_y = rect[1].min(rect[3]);
        let q_max_y = rect[1].max(rect[3]);

        let mut matched = Vec::new();
        for (idx, img) in self.images.iter().enumerate() {
            let b = self.image_visual_bounds(img, rotation);
            if b[0] <= q_max_x && b[2] >= q_min_x && b[1] <= q_max_y && b[3] >= q_min_y {
                matched.push((idx, img));
            }
        }
        matched
    }

    /// Encodes a specific embedded image into PNG bytes.
    pub fn encode_image_png(&self, image_index: usize) -> Result<Vec<u8>> {
        let img = self
            .images
            .get(image_index)
            .context("Image index out of bounds")?;
        encode_rgba_to_png(&img.rgba, img.pixel_width, img.pixel_height)
    }
}

/// Encodes an arbitrary RGBA byte buffer into PNG format.
pub fn encode_rgba_to_png(rgba: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut bytes);
    image::write_buffer_with_format(
        &mut cursor,
        rgba,
        width,
        height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .context("Failed to encode RGBA buffer to PNG")?;
    Ok(bytes)
}

/// Represents an active document session with parsed geometry, forms, and signatures.
pub struct DocumentSession {
    pub file_path: Option<PathBuf>,
    pub raw_bytes: Vec<u8>,
    pub page_count: u16,
    pub pages: Vec<PageInfo>,
    pub page_texts: Vec<String>,
    pub page_layouts: Vec<PageVisualLayout>,
    pub outlines: Vec<OutlineItem>,
    pub forms: Vec<FormField>,
    pub layers: Vec<LayerInfo>,
    pub visual_signatures: Vec<VisualSignature>,
    pub digital_signature: Option<DigitalSignatureMeta>,
}

impl DocumentSession {
    /// Opens a PDF document from a filesystem path.
    pub fn open_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let bytes = std::fs::read(&path_buf)
            .with_context(|| format!("Failed to read PDF file at {:?}", path_buf))?;
        Self::open_from_bytes(bytes, Some(path_buf))
    }

    /// Opens a PDF document from an in-memory byte buffer.
    pub fn open_from_bytes(bytes: Vec<u8>, file_path: Option<PathBuf>) -> Result<Self> {
        let doc =
            lopdf::Document::load_mem(&bytes).context("Failed to parse PDF document structure")?;

        let mut pages = Vec::new();
        let mut page_texts = Vec::new();
        let mut page_layouts = Vec::new();
        let page_dict_map = doc.get_pages();

        for (page_num, object_id) in &page_dict_map {
            let mut width_pt = 595.28; // Standard A4 default
            let mut height_pt = 841.89;
            let mut rotation_degrees = 0;
            let mut media_x0 = 0.0;
            let mut media_y0 = 0.0;

            if let Ok(page_obj) = doc.get_object(*object_id) {
                if let Ok(dict) = page_obj.as_dict() {
                    // Check MediaBox [llx, lly, urx, ury]
                    if let Ok(mediabox) = dict.get(b"MediaBox").and_then(Object::as_array) {
                        if mediabox.len() >= 4 {
                            let x0 = mediabox[0].as_float().unwrap_or(0.0);
                            let y0 = mediabox[1].as_float().unwrap_or(0.0);
                            let x1 = mediabox[2].as_float().unwrap_or(595.28);
                            let y1 = mediabox[3].as_float().unwrap_or(841.89);
                            media_x0 = x0;
                            media_y0 = y0;
                            width_pt = (x1 - x0).abs();
                            height_pt = (y1 - y0).abs();
                        }
                    }

                    // Check Rotate
                    if let Ok(rot) = dict.get(b"Rotate").and_then(Object::as_i64) {
                        rotation_degrees = (rot % 360) as u16;
                    }
                }
            }

            pages.push(PageInfo {
                index: (*page_num as u16).saturating_sub(1),
                width_pt,
                height_pt,
                rotation_degrees,
            });

            // Extract page visual layout and robust text
            let layout =
                extract_page_layout(&doc, *object_id, width_pt, height_pt, media_x0, media_y0);
            page_texts.push(layout.plain_text.clone());
            page_layouts.push(layout);
        }

        let page_count = pages.len() as u16;

        // Parse Outlines / Table of Contents if present
        let mut outlines = Vec::new();
        if let Ok(catalog) = doc.catalog() {
            if let Ok(outlines_dict) = catalog.get(b"Outlines").and_then(Object::as_dict) {
                if let Ok(first_ref) = outlines_dict.get(b"First").and_then(Object::as_reference) {
                    if let Ok(first_obj) = doc.get_object(first_ref).and_then(Object::as_dict) {
                        if let Ok(title_bytes) = first_obj.get(b"Title").and_then(Object::as_str) {
                            outlines.push(OutlineItem {
                                title: String::from_utf8_lossy(title_bytes).to_string(),
                                target_page: 0,
                                children: Vec::new(),
                            });
                        }
                    }
                }
            }
        }

        // Extract Optional Content Groups (OCGs / Layers)
        let mut layers = Vec::new();
        if let Ok(catalog) = doc.catalog() {
            if let Ok(oc_props_obj) = catalog.get(b"OCProperties") {
                let oc_props_dict = match oc_props_obj {
                    Object::Reference(r) => doc.get_object(*r).and_then(Object::as_dict).ok(),
                    Object::Dictionary(d) => Some(d),
                    _ => None,
                };
                if let Some(ocp) = oc_props_dict {
                    let mut off_ids = std::collections::HashSet::new();
                    if let Ok(d_obj) = ocp.get(b"D") {
                        let d_dict = match d_obj {
                            Object::Reference(r) => {
                                doc.get_object(*r).and_then(Object::as_dict).ok()
                            }
                            Object::Dictionary(d) => Some(d),
                            _ => None,
                        };
                        if let Some(d) = d_dict {
                            if let Ok(off_arr) = d.get(b"OFF").and_then(Object::as_array) {
                                for item in off_arr {
                                    if let Ok(ref_id) = item.as_reference() {
                                        off_ids.insert(ref_id);
                                    }
                                }
                            }
                        }
                    }

                    if let Ok(ocgs_arr) = ocp.get(b"OCGs").and_then(Object::as_array) {
                        for item in ocgs_arr {
                            if let Ok(ref_id) = item.as_reference() {
                                if let Ok(ocg_dict) =
                                    doc.get_object(ref_id).and_then(Object::as_dict)
                                {
                                    let name = ocg_dict
                                        .get(b"Name")
                                        .map(|n| {
                                            if let Ok(bytes) = n.as_str() {
                                                String::from_utf8_lossy(bytes).to_string()
                                            } else {
                                                "Layer".to_string()
                                            }
                                        })
                                        .unwrap_or_else(|_| "Layer".to_string());
                                    let visible = !off_ids.contains(&ref_id);
                                    layers.push(LayerInfo { name, visible });
                                }
                            }
                        }
                    }
                }
            }
        }

        // Extract interactive AcroForms
        let forms = forms::extract_form_fields(&doc);

        Ok(Self {
            file_path,
            raw_bytes: bytes,
            page_count,
            pages,
            page_texts,
            page_layouts,
            outlines,
            forms,
            layers,
            visual_signatures: Vec::new(),
            digital_signature: None,
        })
    }

    /// Toggles visibility of an Optional Content Group (Layer) by index.
    pub fn toggle_layer(&mut self, index: usize) -> bool {
        if let Some(layer) = self.layers.get_mut(index) {
            layer.visible = !layer.visible;
            true
        } else {
            false
        }
    }

    /// Returns the text content for a given page index.
    pub fn get_page_text(&self, page_index: usize) -> Option<&str> {
        self.page_texts.get(page_index).map(|s| s.as_str())
    }

    /// Returns the visual layout for a given page index.
    pub fn get_page_layout(&self, page_index: usize) -> Option<&PageVisualLayout> {
        self.page_layouts.get(page_index)
    }

    /// Returns the aspect ratio (width / height) of the specified page.
    pub fn page_aspect_ratio(&self, page_index: usize) -> Option<f32> {
        self.pages.get(page_index).map(|p| p.width_pt / p.height_pt)
    }

    /// Extracts text for a given page.
    pub fn extract_text_for_page(&self, page_index: usize) -> Result<String> {
        let doc = lopdf::Document::load_mem(&self.raw_bytes)?;
        let page_num = (page_index + 1) as u32;
        let text = extract_page_text_robust(&doc, page_num);
        Ok(text)
    }

    /// Performs full-text search across all document pages.
    pub fn search_text(&self, query: &str) -> Vec<SearchResult> {
        if query.trim().is_empty() {
            return Vec::new();
        }

        let mut results = Vec::new();
        let query_lower = query.to_lowercase();

        if let Ok(doc) = lopdf::Document::load_mem(&self.raw_bytes) {
            for page in &self.pages {
                let page_num = (page.index + 1) as u32;
                let text = extract_page_text_robust(&doc, page_num);
                if !text.is_empty() {
                    let text_lower = text.to_lowercase();
                    let matches: Vec<_> = text_lower.match_indices(&query_lower).collect();
                    if !matches.is_empty() {
                        let first_idx = matches[0].0;
                        let start = first_idx.saturating_sub(20);
                        let end = (first_idx + query.len() + 30).min(text.len());
                        let snippet = format!("...{}...", text[start..end].replace('\n', " "));
                        results.push(SearchResult {
                            page_index: page.index,
                            snippet,
                            match_count: matches.len(),
                        });
                    }
                }
            }
        }

        results
    }

    /// Updates the value of a form field identified by name or id.
    pub fn update_form_field(&mut self, name_or_id: &str, value: &str) -> bool {
        for field in &mut self.forms {
            if field.name == name_or_id || field.id == name_or_id {
                field.set_value(value);
                return true;
            }
        }
        false
    }

    /// Rotates the specified page clockwise (true) or counter-clockwise (false) by 90 degrees.
    pub fn rotate_page(&mut self, page_index: usize, clockwise: bool) {
        if let Some(page) = self.pages.get_mut(page_index) {
            if clockwise {
                page.rotation_degrees = (page.rotation_degrees + 90) % 360;
            } else {
                page.rotation_degrees = (page.rotation_degrees + 270) % 360;
            }
        }
    }

    /// Adds a new interactive form field.
    pub fn add_form_field(&mut self, field: FormField) {
        self.forms.push(field);
    }

    /// Adds a visual ink signature to be rendered and embedded in the document.
    pub fn add_visual_signature(&mut self, signature: VisualSignature) {
        self.visual_signatures.push(signature);
    }

    /// Sets the cryptographic digital signature metadata.
    pub fn set_digital_signature(&mut self, meta: DigitalSignatureMeta) {
        self.digital_signature = Some(meta);
    }

    /// Serializes the document including modified form fields, visual signatures, and digital signatures.
    pub fn save_to_bytes(&mut self) -> Result<Vec<u8>> {
        let mut doc = lopdf::Document::load_mem(&self.raw_bytes)
            .context("Failed to load document for serialization")?;

        // 1. Apply updated AcroForm field values
        forms::apply_form_fields(&mut doc, &self.forms)
            .context("Failed to apply form field values")?;

        // 2. Stamp visual signatures into page content streams
        let pages = doc.get_pages();
        for sig in &self.visual_signatures {
            let page_num = (sig.target_page + 1) as u32;
            if let Some(&page_obj_id) = pages.get(&page_num) {
                let page_height = self
                    .pages
                    .get(sig.target_page as usize)
                    .map(|p| p.height_pt)
                    .unwrap_or(842.0);

                let ops_bytes = sig.generate_pdf_graphics_operators(page_height);
                if !ops_bytes.is_empty() {
                    let sig_stream = Stream::new(Dictionary::new(), ops_bytes);
                    let sig_stream_id = doc.add_object(Object::Stream(sig_stream));

                    if let Ok(page_dict) = doc
                        .get_object_mut(page_obj_id)
                        .and_then(Object::as_dict_mut)
                    {
                        if let Ok(contents) = page_dict.get_mut(b"Contents") {
                            match contents {
                                Object::Reference(existing_id) => {
                                    *contents = Object::Array(vec![
                                        Object::Reference(*existing_id),
                                        Object::Reference(sig_stream_id),
                                    ]);
                                }
                                Object::Array(arr) => {
                                    arr.push(Object::Reference(sig_stream_id));
                                }
                                _ => {
                                    *contents = Object::Reference(sig_stream_id);
                                }
                            }
                        } else {
                            page_dict.set("Contents", Object::Reference(sig_stream_id));
                        }
                    }
                }
            }
        }

        // 3. Embed cryptographic PAdES digital signature if present
        if let Some(mut meta) = self.digital_signature.clone() {
            sign::embed_digital_signature(&mut doc, 0, &mut meta)
                .context("Failed to embed digital signature")?;
            self.digital_signature = Some(meta);
        }

        // 4. Save and return bytes
        let mut output = Vec::new();
        doc.save_to(&mut output)
            .context("Failed to write PDF binary stream")?;
        Ok(output)
    }
}

/// Decodes an ASCII85 (Adobe variant) encoded byte slice.
pub fn decode_ascii85(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut count = 0;
    let mut tuple = 0u32;
    for &b in input {
        if b == b'~' {
            break;
        }
        if b.is_ascii_whitespace() {
            continue;
        }
        if b == b'z' && count == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        if (b'!'..=b'u').contains(&b) {
            tuple = tuple * 85 + (b - b'!') as u32;
            count += 1;
            if count == 5 {
                out.extend_from_slice(&tuple.to_be_bytes());
                tuple = 0;
                count = 0;
            }
        }
    }
    if count > 0 {
        let padding = 5 - count;
        for _ in 0..padding {
            tuple = tuple * 85 + 84;
        }
        let bytes = tuple.to_be_bytes();
        out.extend_from_slice(&bytes[..count - 1]);
    }
    out
}

/// Decodes an ASCII Hex encoded byte slice.
pub fn decode_ascii_hex(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut first_nibble = None;
    for &b in input {
        if b == b'>' {
            break;
        }
        if b.is_ascii_whitespace() {
            continue;
        }
        let nibble = match b {
            b'0'..=b'9' => b - b'0',
            b'a'..=b'f' => b - b'a' + 10,
            b'A'..=b'F' => b - b'A' + 10,
            _ => continue,
        };
        match first_nibble {
            None => first_nibble = Some(nibble),
            Some(high) => {
                out.push((high << 4) | nibble);
                first_nibble = None;
            }
        }
    }
    if let Some(high) = first_nibble {
        out.push(high << 4);
    }
    out
}

/// Decodes a Flate (zlib or raw deflate) compressed byte slice.
pub fn decode_flate(input: &[u8]) -> Result<Vec<u8>> {
    use flate2::read::{DeflateDecoder, ZlibDecoder};
    use std::io::Read;

    let mut zlib = ZlibDecoder::new(input);
    let mut out = Vec::new();
    if zlib.read_to_end(&mut out).is_ok() && !out.is_empty() {
        return Ok(out);
    }

    let mut deflate = DeflateDecoder::new(input);
    let mut out = Vec::new();
    deflate
        .read_to_end(&mut out)
        .context("Flate decompression failed")?;
    Ok(out)
}

/// Decompresses a PDF stream applying all filters in sequential decoding order.
pub fn decompress_pdf_stream(stream: &lopdf::Stream) -> Result<Vec<u8>> {
    let filter_obj = match stream.dict.get(b"Filter") {
        Ok(obj) => obj,
        Err(_) => return Ok(stream.content.clone()),
    };

    let mut filters = Vec::new();
    match filter_obj {
        Object::Name(name) => {
            filters.push(String::from_utf8_lossy(name).to_string());
        }
        Object::Array(arr) => {
            for item in arr {
                if let Ok(name) = item.as_name_str() {
                    filters.push(name.to_string());
                }
            }
        }
        _ => return Ok(stream.content.clone()),
    }

    if filters.is_empty() {
        return Ok(stream.content.clone());
    }

    let mut current_data = stream.content.clone();
    for filter in &filters {
        match filter.as_str() {
            "ASCII85Decode" | "A85" => {
                current_data = decode_ascii85(&current_data);
            }
            "ASCIIHexDecode" | "AHx" => {
                current_data = decode_ascii_hex(&current_data);
            }
            "FlateDecode" | "Fl" => match decode_flate(&current_data) {
                Ok(decompressed) => current_data = decompressed,
                Err(_) => {
                    if let Ok(native) = stream.decompressed_content() {
                        return Ok(native);
                    }
                }
            },
            "LZWDecode" | "LZW" => {
                if let Ok(native) = stream.decompressed_content() {
                    return Ok(native);
                }
            }
            _ => {}
        }
    }

    Ok(current_data)
}

/// Extracts and decompresses the complete content stream for a given page.
pub fn get_page_content_decompressed(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
) -> Result<Vec<u8>> {
    let page_obj = doc.get_object(page_id).context("Page object not found")?;
    let page_dict = page_obj.as_dict().context("Page object is not a dict")?;
    let contents = match page_dict.get(b"Contents") {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()),
    };

    let mut stream_ids = Vec::new();
    match contents {
        Object::Reference(id) => stream_ids.push(*id),
        Object::Array(arr) => {
            for item in arr {
                if let Object::Reference(id) = item {
                    stream_ids.push(*id);
                }
            }
        }
        Object::Stream(s) => return decompress_pdf_stream(s),
        _ => return Ok(Vec::new()),
    }

    let mut result = Vec::new();
    for (i, sid) in stream_ids.iter().enumerate() {
        if let Ok(obj) = doc.get_object(*sid) {
            if let Ok(stream) = obj.as_stream() {
                if let Ok(decompressed) = decompress_pdf_stream(stream) {
                    if i > 0 && !result.is_empty() {
                        result.push(b'\n');
                    }
                    result.extend_from_slice(&decompressed);
                }
            }
        }
    }

    Ok(result)
}

#[derive(Clone)]
struct GraphicsGState {
    ctm: [f32; 6],
    fill_color: [u8; 3],
    stroke_color: [u8; 3],
    line_width: f32,
}

impl Default for GraphicsGState {
    fn default() -> Self {
        Self {
            ctm: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            fill_color: [0, 0, 0],
            stroke_color: [0, 0, 0],
            line_width: 1.0,
        }
    }
}

fn get_op_float(obj: &Object) -> f32 {
    obj.as_float().unwrap_or(0.0)
}

fn float_to_u8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn cmyk_to_rgb(c: f32, m: f32, y: f32, k: f32) -> [u8; 3] {
    let r = ((1.0 - c) * (1.0 - k)).clamp(0.0, 1.0) * 255.0;
    let g = ((1.0 - m) * (1.0 - k)).clamp(0.0, 1.0) * 255.0;
    let b = ((1.0 - y) * (1.0 - k)).clamp(0.0, 1.0) * 255.0;
    [r.round() as u8, g.round() as u8, b.round() as u8]
}

fn multiply_matrix(m1: &[f32; 6], m2: &[f32; 6]) -> [f32; 6] {
    [
        m1[0] * m2[0] + m1[1] * m2[2],
        m1[0] * m2[1] + m1[1] * m2[3],
        m1[2] * m2[0] + m1[3] * m2[2],
        m1[2] * m2[1] + m1[3] * m2[3],
        m1[4] * m2[0] + m1[5] * m2[2] + m2[4],
        m1[4] * m2[1] + m1[5] * m2[3] + m2[5],
    ]
}

fn transform_point(m: &[f32; 6], x: f32, y: f32) -> (f32, f32) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

#[allow(clippy::too_many_arguments)]
fn emit_vector_rect(
    rects: &mut Vec<VectorRect>,
    rx: f32,
    ry: f32,
    rw: f32,
    rh: f32,
    gstate: &GraphicsGState,
    media_x0: f32,
    media_y0: f32,
    fill: bool,
    stroke: bool,
) {
    let (p0_x, p0_y) = transform_point(&gstate.ctm, rx, ry);
    let (p1_x, p1_y) = transform_point(&gstate.ctm, rx + rw, ry + rh);
    let min_x = p0_x.min(p1_x) - media_x0;
    let min_y = p0_y.min(p1_y) - media_y0;
    let width = (p1_x - p0_x).abs();
    let height = (p1_y - p0_y).abs();

    if width > 0.5 && height > 0.5 {
        rects.push(VectorRect {
            x: min_x,
            y: min_y,
            width,
            height,
            fill_color: if fill { Some(gstate.fill_color) } else { None },
            stroke_color: if stroke {
                Some(gstate.stroke_color)
            } else {
                None
            },
            stroke_width: if stroke { gstate.line_width } else { 0.0 },
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_text_run(
    text_runs: &mut Vec<PositionedText>,
    operands: &[lopdf::Object],
    current_font: &[u8],
    font_cmaps: &std::collections::HashMap<Vec<u8>, ToUnicodeCMap>,
    font_encodings: &std::collections::HashMap<Vec<u8>, String>,
    text_matrix: &mut [f32; 6],
    gstate: &GraphicsGState,
    current_font_size: f32,
    media_x0: f32,
    media_y0: f32,
) {
    let raw_text = decode_text_operands_string(operands, current_font, font_cmaps, font_encodings);
    let text = sanitize_extracted_text(&raw_text);
    if !text.trim().is_empty() {
        let trm = multiply_matrix(text_matrix, &gstate.ctm);
        let wx = trm[4];
        let wy = trm[5];
        let x = wx - media_x0;
        let y = wy - media_y0;
        let scale = (trm[0].powi(2) + trm[1].powi(2)).sqrt().abs();
        let effective_size = if scale > 0.01 {
            current_font_size * scale
        } else {
            current_font_size
        };
        let angle_rad = trm[1].atan2(trm[0]);
        let rotation_deg = angle_rad.to_degrees();

        text_runs.push(PositionedText {
            text: text.clone(),
            x,
            y,
            font_size: effective_size,
            color: gstate.fill_color,
            rotation_deg,
        });

        // Advance x position in text matrix
        let advance = (text.chars().count() as f32) * effective_size * 0.52;
        text_matrix[4] += advance;
    }
}

fn decode_text_operands_string(
    operands: &[lopdf::Object],
    current_font: &[u8],
    font_cmaps: &std::collections::HashMap<Vec<u8>, ToUnicodeCMap>,
    font_encodings: &std::collections::HashMap<Vec<u8>, String>,
) -> String {
    let mut output = String::new();
    let cmap_opt = font_cmaps.get(current_font);
    let encoding_opt = font_encodings.get(current_font).map(|s| s.as_str());

    for op in operands {
        match op {
            lopdf::Object::String(bytes, _) => {
                decode_single_string(&mut output, bytes, cmap_opt, encoding_opt);
            }
            lopdf::Object::Array(arr) => {
                for item in arr {
                    match item {
                        lopdf::Object::String(bytes, _) => {
                            decode_single_string(&mut output, bytes, cmap_opt, encoding_opt);
                        }
                        lopdf::Object::Integer(i) if *i < -120 => {
                            output.push(' ');
                        }
                        lopdf::Object::Real(r) if *r < -120.0 => {
                            output.push(' ');
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    output
}

/// Maps a PDF coordinate point (origin bottom-left) to visual top-left coordinates taking page rotation into account.
pub fn map_pdf_point_to_visual(
    x: f32,
    y: f32,
    page_w: f32,
    page_h: f32,
    rotation: u16,
) -> (f32, f32) {
    match rotation % 360 {
        90 => (y, x),
        180 => (page_w - x, y),
        270 => (page_h - y, page_w - x),
        _ => (x, page_h - y),
    }
}

fn unfilter_png_predictor(
    raw: &[u8],
    columns: usize,
    colors: usize,
    bits_per_component: usize,
    rows: usize,
) -> Vec<u8> {
    let bits_per_row = columns * colors * bits_per_component;
    let bytes_per_row = bits_per_row.div_ceil(8);
    let stride = 1 + bytes_per_row;
    let bpp = (colors * bits_per_component).div_ceil(8).max(1);

    let mut result = Vec::with_capacity(rows * bytes_per_row);
    let mut prior_row = vec![0u8; bytes_per_row];

    for r in 0..rows {
        let row_start = r * stride;
        if row_start >= raw.len() {
            break;
        }
        let filter = raw[row_start];
        let row_slice = if row_start + 1 + bytes_per_row <= raw.len() {
            &raw[row_start + 1..row_start + 1 + bytes_per_row]
        } else if row_start + 1 < raw.len() {
            &raw[row_start + 1..]
        } else {
            &[]
        };

        let mut out_row = vec![0u8; bytes_per_row];
        for c in 0..bytes_per_row {
            let x = if c < row_slice.len() { row_slice[c] } else { 0 };
            let a = if c >= bpp { out_row[c - bpp] } else { 0 };
            let b = prior_row[c];
            let c_prev = if c >= bpp { prior_row[c - bpp] } else { 0 };

            let val = match filter {
                0 => x,
                1 => x.wrapping_add(a),
                2 => x.wrapping_add(b),
                3 => x.wrapping_add(((a as u16 + b as u16) / 2) as u8),
                4 => x.wrapping_add(paeth_predictor(a, b, c_prev)),
                _ => x,
            };
            out_row[c] = val;
        }

        result.extend_from_slice(&out_row);
        prior_row = out_row;
    }

    result
}

fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let a_i = a as i16;
    let b_i = b as i16;
    let c_i = c as i16;
    let p = a_i + b_i - c_i;
    let pa = (p - a_i).abs();
    let pb = (p - b_i).abs();
    let pc = (p - c_i).abs();
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn rotate_rgba_if_needed(
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    ctm: &[f32; 6],
) -> (Vec<u8>, u32, u32) {
    let a = ctm[0];
    let b = ctm[1];
    let c = ctm[2];
    let d = ctm[3];

    let angle_rad = b.atan2(a);
    let deg = angle_rad.to_degrees().round() as i32;
    let norm_deg = deg.rem_euclid(360);

    if (80..=100).contains(&norm_deg)
        || (a.abs() < 0.01 && b.abs() > 0.01 && c.abs() > 0.01 && d.abs() < 0.01 && b > 0.0)
    {
        // Rotate 90 degrees clockwise
        let mut rotated = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let src_idx = ((y * width + x) * 4) as usize;
                let dst_x = height - 1 - y;
                let dst_y = x;
                let dst_idx = ((dst_y * height + dst_x) * 4) as usize;
                if src_idx + 3 < rgba.len() && dst_idx + 3 < rotated.len() {
                    rotated[dst_idx..dst_idx + 4].copy_from_slice(&rgba[src_idx..src_idx + 4]);
                }
            }
        }
        (rotated, height, width)
    } else if (170..=190).contains(&norm_deg) {
        // Rotate 180 degrees
        let mut rotated = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let src_idx = ((y * width + x) * 4) as usize;
                let dst_x = width - 1 - x;
                let dst_y = height - 1 - y;
                let dst_idx = ((dst_y * width + dst_x) * 4) as usize;
                if src_idx + 3 < rgba.len() && dst_idx + 3 < rotated.len() {
                    rotated[dst_idx..dst_idx + 4].copy_from_slice(&rgba[src_idx..src_idx + 4]);
                }
            }
        }
        (rotated, width, height)
    } else if (260..=280).contains(&norm_deg)
        || (a.abs() < 0.01 && b.abs() > 0.01 && c.abs() > 0.01 && d.abs() < 0.01 && b < 0.0)
    {
        // Rotate 270 degrees
        let mut rotated = vec![0u8; (width * height * 4) as usize];
        for y in 0..height {
            for x in 0..width {
                let src_idx = ((y * width + x) * 4) as usize;
                let dst_x = y;
                let dst_y = width - 1 - x;
                let dst_idx = ((dst_y * height + dst_x) * 4) as usize;
                if src_idx + 3 < rgba.len() && dst_idx + 3 < rotated.len() {
                    rotated[dst_idx..dst_idx + 4].copy_from_slice(&rgba[src_idx..src_idx + 4]);
                }
            }
        }
        (rotated, height, width)
    } else {
        (rgba, width, height)
    }
}

#[allow(clippy::chunks_exact_to_as_chunks)]
fn convert_image_bytes_to_rgba(
    raw_bytes: &[u8],
    width: u32,
    height: u32,
    colorspace: &str,
    bits_per_component: usize,
    decode_parms: Option<&lopdf::Dictionary>,
    smask_bytes: Option<&[u8]>,
) -> Vec<u8> {
    let pixel_count = (width * height) as usize;

    // 1. If bytes represent a standard image container (e.g., JPEG DCTDecode), decode via image crate
    if raw_bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        if let Ok(dynamic_img) = image::load_from_memory(raw_bytes) {
            let mut decoded_rgba = dynamic_img.to_rgba8().into_raw();
            if let Some(smask) = smask_bytes {
                if smask.len() >= pixel_count && decoded_rgba.len() >= pixel_count * 4 {
                    for (i, &a) in smask.iter().take(pixel_count).enumerate() {
                        decoded_rgba[i * 4 + 3] = a;
                    }
                }
            }
            return decoded_rgba;
        }
    }

    // 2. Unfilter PNG predictor if specified in DecodeParms
    let predictor = decode_parms
        .and_then(|d| d.get(b"Predictor").ok())
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(1) as usize;

    let columns = decode_parms
        .and_then(|d| d.get(b"Columns").ok())
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(width as i64) as usize;

    let colors = decode_parms
        .and_then(|d| d.get(b"Colors").ok())
        .and_then(|o| o.as_i64().ok())
        .unwrap_or_else(|| {
            if colorspace.contains("RGB") {
                3
            } else if colorspace.contains("CMYK") {
                4
            } else {
                1
            }
        }) as usize;

    let bpc = decode_parms
        .and_then(|d| d.get(b"BitsPerComponent").ok())
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(bits_per_component as i64) as usize;

    let processed_bytes = if (10..=15).contains(&predictor) {
        unfilter_png_predictor(raw_bytes, columns, colors, bpc, height as usize)
    } else {
        raw_bytes.to_vec()
    };

    let mut rgba = Vec::with_capacity(pixel_count * 4);

    // 3. Unpack 1-bit monochrome images
    if bpc == 1 && (colorspace.contains("Gray") || colors == 1) {
        let bytes_per_row = (width as usize).div_ceil(8);
        for r in 0..height as usize {
            let row_start = r * bytes_per_row;
            if row_start >= processed_bytes.len() {
                break;
            }
            let row_end = (row_start + bytes_per_row).min(processed_bytes.len());
            let row_slice = &processed_bytes[row_start..row_end];
            for c in 0..width as usize {
                let byte_idx = c / 8;
                let bit_idx = 7 - (c % 8);
                let bit = if byte_idx < row_slice.len() {
                    (row_slice[byte_idx] >> bit_idx) & 1
                } else {
                    0
                };
                let val = if bit == 1 { 255 } else { 0 };
                rgba.push(val);
                rgba.push(val);
                rgba.push(val);
                rgba.push(255);
            }
        }
    } else if colorspace.contains("RGB") || colorspace.is_empty() {
        for chunk in processed_bytes.chunks_exact(3) {
            rgba.push(chunk[0]);
            rgba.push(chunk[1]);
            rgba.push(chunk[2]);
            rgba.push(255);
        }
    } else if colorspace.contains("Gray") {
        for &g in processed_bytes.iter().take(pixel_count) {
            rgba.push(g);
            rgba.push(g);
            rgba.push(g);
            rgba.push(255);
        }
    } else if colorspace.contains("CMYK") {
        for chunk in processed_bytes.chunks_exact(4) {
            let c = chunk[0] as f32 / 255.0;
            let m = chunk[1] as f32 / 255.0;
            let y = chunk[2] as f32 / 255.0;
            let k = chunk[3] as f32 / 255.0;
            let rgb = cmyk_to_rgb(c, m, y, k);
            rgba.push(rgb[0]);
            rgba.push(rgb[1]);
            rgba.push(rgb[2]);
            rgba.push(255);
        }
    } else if processed_bytes.len() >= pixel_count * 4 {
        rgba.extend_from_slice(&processed_bytes[..pixel_count * 4]);
    } else {
        for _ in 0..pixel_count {
            rgba.extend_from_slice(&[180, 180, 180, 255]);
        }
    }

    if rgba.len() < pixel_count * 4 {
        rgba.resize(pixel_count * 4, 255);
    }

    // Apply Soft Mask (alpha transparency channel) if present
    if let Some(smask) = smask_bytes {
        if smask.len() >= pixel_count && rgba.len() >= pixel_count * 4 {
            for (i, &a) in smask.iter().take(pixel_count).enumerate() {
                rgba[i * 4 + 3] = a;
            }
        }
    }

    rgba
}

/// Extracts high-fidelity visual layout (positioned text runs and vector rects)
/// from a PDF page's decompressed content stream.
#[derive(Clone, Default)]
pub struct ResourceContext {
    pub font_cmaps: std::collections::HashMap<Vec<u8>, ToUnicodeCMap>,
    pub font_encodings: std::collections::HashMap<Vec<u8>, String>,
    pub xobjects: std::collections::HashMap<Vec<u8>, lopdf::ObjectId>,
}

impl ResourceContext {
    pub fn merge(&self, child: &Self) -> Self {
        let mut merged = self.clone();
        for (k, v) in &child.font_cmaps {
            merged.font_cmaps.insert(k.clone(), v.clone());
        }
        for (k, v) in &child.font_encodings {
            merged.font_encodings.insert(k.clone(), v.clone());
        }
        for (k, v) in &child.xobjects {
            merged.xobjects.insert(k.clone(), *v);
        }
        merged
    }
}

fn parse_rect(rect_obj: &lopdf::Object) -> Option<[f32; 4]> {
    let arr = rect_obj.as_array().ok()?;
    if arr.len() < 4 {
        return None;
    }
    let x1 = get_op_float(&arr[0]);
    let y1 = get_op_float(&arr[1]);
    let x2 = get_op_float(&arr[2]);
    let y2 = get_op_float(&arr[3]);
    Some([x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2)])
}

fn extract_resources(doc: &lopdf::Document, res_dict: &lopdf::Dictionary) -> ResourceContext {
    let mut ctx = ResourceContext::default();

    // 1. Fonts
    if let Ok(font_obj) = res_dict.get(b"Font") {
        let f_dict_opt = match font_obj {
            lopdf::Object::Reference(r) => doc.get_object(*r).and_then(lopdf::Object::as_dict).ok(),
            lopdf::Object::Dictionary(d) => Some(d),
            _ => None,
        };
        if let Some(f_dict) = f_dict_opt {
            for (font_name, obj) in f_dict.iter() {
                let font_dict_opt = match obj {
                    lopdf::Object::Reference(r) => {
                        doc.get_object(*r).and_then(lopdf::Object::as_dict).ok()
                    }
                    lopdf::Object::Dictionary(d) => Some(d),
                    _ => None,
                };
                if let Some(font_dict) = font_dict_opt {
                    // Resolve /Encoding (could be an indirect reference or dictionary)
                    let enc_obj = match font_dict.get(b"Encoding") {
                        Ok(lopdf::Object::Reference(r)) => doc.get_object(*r).ok(),
                        Ok(o) => Some(o),
                        _ => None,
                    };
                    if let Some(o) = enc_obj {
                        if let Ok(enc_name) = o.as_name_str() {
                            ctx.font_encodings
                                .insert(font_name.clone(), enc_name.to_string());
                        } else if let Ok(ed) = o.as_dict() {
                            if let Ok(base_enc) =
                                ed.get(b"BaseEncoding").and_then(lopdf::Object::as_name_str)
                            {
                                ctx.font_encodings
                                    .insert(font_name.clone(), base_enc.to_string());
                            }
                        }
                    }

                    // Resolve /ToUnicode (could be on font_dict or on DescendantFonts)
                    let to_unicode_obj = font_dict.get(b"ToUnicode").ok().or_else(|| {
                        if let Ok(descendants) = font_dict
                            .get(b"DescendantFonts")
                            .and_then(lopdf::Object::as_array)
                        {
                            if let Some(first_desc) = descendants.first() {
                                let desc_dict = match first_desc {
                                    lopdf::Object::Reference(r) => {
                                        doc.get_object(*r).and_then(lopdf::Object::as_dict).ok()
                                    }
                                    lopdf::Object::Dictionary(d) => Some(d),
                                    _ => None,
                                };
                                if let Some(d) = desc_dict {
                                    return d.get(b"ToUnicode").ok();
                                }
                            }
                        }
                        None
                    });

                    if let Some(to_unicode_obj) = to_unicode_obj {
                        let stream = match to_unicode_obj {
                            lopdf::Object::Reference(ref_id) => doc
                                .get_object(*ref_id)
                                .and_then(lopdf::Object::as_stream)
                                .ok(),
                            lopdf::Object::Stream(s) => Some(s),
                            _ => None,
                        };

                        if let Some(stream) = stream {
                            let stream_bytes = decompress_pdf_stream(stream)
                                .unwrap_or_else(|_| stream.content.clone());
                            let cmap = parse_to_unicode_cmap(&stream_bytes);
                            if !cmap.is_empty() {
                                ctx.font_cmaps.insert(font_name.clone(), cmap);
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. XObjects
    if let Ok(xobj_obj) = res_dict.get(b"XObject") {
        let x_dict_opt = match xobj_obj {
            lopdf::Object::Reference(r) => doc.get_object(*r).and_then(lopdf::Object::as_dict).ok(),
            lopdf::Object::Dictionary(d) => Some(d),
            _ => None,
        };
        if let Some(xobjs) = x_dict_opt {
            for (name, obj) in xobjs.iter() {
                if let Ok(ref_id) = obj.as_reference() {
                    ctx.xobjects.insert(name.clone(), ref_id);
                }
            }
        }
    }

    ctx
}

#[allow(clippy::too_many_arguments)]
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
) {
    let mut text_matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut line_matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let mut current_font = Vec::new();
    let mut current_font_size = 12.0f32;
    let mut current_leading = 12.0f32;

    let mut pending_rects: Vec<(f32, f32, f32, f32)> = Vec::new();

    for operation in operations {
        match operation.operator.as_str() {
            // Graphics state save / restore
            "q" => {
                gstate_stack.push(gstate.clone());
            }
            "Q" => {
                if let Some(restored) = gstate_stack.pop() {
                    *gstate = restored;
                }
            }
            // Concatenate matrix to CTM
            "cm" => {
                if operation.operands.len() >= 6 {
                    let m = [
                        get_op_float(&operation.operands[0]),
                        get_op_float(&operation.operands[1]),
                        get_op_float(&operation.operands[2]),
                        get_op_float(&operation.operands[3]),
                        get_op_float(&operation.operands[4]),
                        get_op_float(&operation.operands[5]),
                    ];
                    gstate.ctm = multiply_matrix(&m, &gstate.ctm);
                }
            }
            // Colors
            "rg" => {
                if operation.operands.len() >= 3 {
                    let r = get_op_float(&operation.operands[0]);
                    let g = get_op_float(&operation.operands[1]);
                    let b = get_op_float(&operation.operands[2]);
                    gstate.fill_color = [float_to_u8(r), float_to_u8(g), float_to_u8(b)];
                }
            }
            "RG" => {
                if operation.operands.len() >= 3 {
                    let r = get_op_float(&operation.operands[0]);
                    let g = get_op_float(&operation.operands[1]);
                    let b = get_op_float(&operation.operands[2]);
                    gstate.stroke_color = [float_to_u8(r), float_to_u8(g), float_to_u8(b)];
                }
            }
            "g" => {
                if let Some(op0) = operation.operands.first() {
                    let val = float_to_u8(get_op_float(op0));
                    gstate.fill_color = [val, val, val];
                }
            }
            "G" => {
                if let Some(op0) = operation.operands.first() {
                    let val = float_to_u8(get_op_float(op0));
                    gstate.stroke_color = [val, val, val];
                }
            }
            "k" => {
                if operation.operands.len() >= 4 {
                    let c = get_op_float(&operation.operands[0]);
                    let m = get_op_float(&operation.operands[1]);
                    let y = get_op_float(&operation.operands[2]);
                    let k = get_op_float(&operation.operands[3]);
                    gstate.fill_color = cmyk_to_rgb(c, m, y, k);
                }
            }
            "K" => {
                if operation.operands.len() >= 4 {
                    let c = get_op_float(&operation.operands[0]);
                    let m = get_op_float(&operation.operands[1]);
                    let y = get_op_float(&operation.operands[2]);
                    let k = get_op_float(&operation.operands[3]);
                    gstate.stroke_color = cmyk_to_rgb(c, m, y, k);
                }
            }
            "sc" | "scn" => match operation.operands.len() {
                1 => {
                    let val = float_to_u8(get_op_float(&operation.operands[0]));
                    gstate.fill_color = [val, val, val];
                }
                3 => {
                    let r = get_op_float(&operation.operands[0]);
                    let g = get_op_float(&operation.operands[1]);
                    let b = get_op_float(&operation.operands[2]);
                    gstate.fill_color = [float_to_u8(r), float_to_u8(g), float_to_u8(b)];
                }
                4 => {
                    let c = get_op_float(&operation.operands[0]);
                    let m = get_op_float(&operation.operands[1]);
                    let y = get_op_float(&operation.operands[2]);
                    let k = get_op_float(&operation.operands[3]);
                    gstate.fill_color = cmyk_to_rgb(c, m, y, k);
                }
                _ => {}
            },
            "SC" | "SCN" => match operation.operands.len() {
                1 => {
                    let val = float_to_u8(get_op_float(&operation.operands[0]));
                    gstate.stroke_color = [val, val, val];
                }
                3 => {
                    let r = get_op_float(&operation.operands[0]);
                    let g = get_op_float(&operation.operands[1]);
                    let b = get_op_float(&operation.operands[2]);
                    gstate.stroke_color = [float_to_u8(r), float_to_u8(g), float_to_u8(b)];
                }
                4 => {
                    let c = get_op_float(&operation.operands[0]);
                    let m = get_op_float(&operation.operands[1]);
                    let y = get_op_float(&operation.operands[2]);
                    let k = get_op_float(&operation.operands[3]);
                    gstate.stroke_color = cmyk_to_rgb(c, m, y, k);
                }
                _ => {}
            },
            "w" => {
                if let Some(op0) = operation.operands.first() {
                    gstate.line_width = get_op_float(op0).max(0.2);
                }
            }
            // Text object operators
            "BT" => {
                text_matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
                line_matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
            }
            "ET" => {}
            "Tf" => {
                if let Some(font_obj) = operation.operands.first() {
                    if let Ok(f_name) = font_obj.as_name() {
                        current_font = f_name.to_vec();
                    }
                }
                if operation.operands.len() >= 2 {
                    current_font_size = get_op_float(&operation.operands[1]).max(1.0);
                }
            }
            "TL" => {
                if let Some(op0) = operation.operands.first() {
                    current_leading = get_op_float(op0);
                }
            }
            "Tm" => {
                if operation.operands.len() >= 6 {
                    text_matrix = [
                        get_op_float(&operation.operands[0]),
                        get_op_float(&operation.operands[1]),
                        get_op_float(&operation.operands[2]),
                        get_op_float(&operation.operands[3]),
                        get_op_float(&operation.operands[4]),
                        get_op_float(&operation.operands[5]),
                    ];
                    line_matrix = text_matrix;
                }
            }
            "Td" => {
                if operation.operands.len() >= 2 {
                    let tx = get_op_float(&operation.operands[0]);
                    let ty = get_op_float(&operation.operands[1]);
                    line_matrix = multiply_matrix(&[1.0, 0.0, 0.0, 1.0, tx, ty], &line_matrix);
                    text_matrix = line_matrix;
                }
            }
            "TD" => {
                if operation.operands.len() >= 2 {
                    let tx = get_op_float(&operation.operands[0]);
                    let ty = get_op_float(&operation.operands[1]);
                    current_leading = -ty;
                    line_matrix = multiply_matrix(&[1.0, 0.0, 0.0, 1.0, tx, ty], &line_matrix);
                    text_matrix = line_matrix;
                }
            }
            "T*" => {
                line_matrix =
                    multiply_matrix(&[1.0, 0.0, 0.0, 1.0, 0.0, -current_leading], &line_matrix);
                text_matrix = line_matrix;
            }
            "'" => {
                line_matrix =
                    multiply_matrix(&[1.0, 0.0, 0.0, 1.0, 0.0, -current_leading], &line_matrix);
                text_matrix = line_matrix;
                emit_text_run(
                    text_runs,
                    &operation.operands,
                    &current_font,
                    &resources.font_cmaps,
                    &resources.font_encodings,
                    &mut text_matrix,
                    gstate,
                    current_font_size,
                    media_x0,
                    media_y0,
                );
            }
            "\"" => {
                if operation.operands.len() >= 3 {
                    line_matrix =
                        multiply_matrix(&[1.0, 0.0, 0.0, 1.0, 0.0, -current_leading], &line_matrix);
                    text_matrix = line_matrix;
                    emit_text_run(
                        text_runs,
                        &operation.operands[2..],
                        &current_font,
                        &resources.font_cmaps,
                        &resources.font_encodings,
                        &mut text_matrix,
                        gstate,
                        current_font_size,
                        media_x0,
                        media_y0,
                    );
                }
            }
            "Tj" | "TJ" => {
                emit_text_run(
                    text_runs,
                    &operation.operands,
                    &current_font,
                    &resources.font_cmaps,
                    &resources.font_encodings,
                    &mut text_matrix,
                    gstate,
                    current_font_size,
                    media_x0,
                    media_y0,
                );
            }
            // Path construction & painting
            "re" => {
                if operation.operands.len() >= 4 {
                    let rx = get_op_float(&operation.operands[0]);
                    let ry = get_op_float(&operation.operands[1]);
                    let rw = get_op_float(&operation.operands[2]);
                    let rh = get_op_float(&operation.operands[3]);
                    pending_rects.push((rx, ry, rw, rh));
                }
            }
            "f" | "f*" | "F" => {
                for &(rx, ry, rw, rh) in &pending_rects {
                    emit_vector_rect(
                        rects, rx, ry, rw, rh, gstate, media_x0, media_y0, true, false,
                    );
                }
                pending_rects.clear();
            }
            "s" | "S" => {
                for &(rx, ry, rw, rh) in &pending_rects {
                    emit_vector_rect(
                        rects, rx, ry, rw, rh, gstate, media_x0, media_y0, false, true,
                    );
                }
                pending_rects.clear();
            }
            "b" | "B" | "b*" | "B*" => {
                for &(rx, ry, rw, rh) in &pending_rects {
                    emit_vector_rect(
                        rects, rx, ry, rw, rh, gstate, media_x0, media_y0, true, true,
                    );
                }
                pending_rects.clear();
            }
            "n" => {
                pending_rects.clear();
            }
            "Do" => {
                if let Some(op0) = operation.operands.first() {
                    if let Ok(name_bytes) = op0.as_name() {
                        if let Some(&xobj_id) = resources.xobjects.get(name_bytes) {
                            if let Ok(xobj_stream) =
                                doc.get_object(xobj_id).and_then(lopdf::Object::as_stream)
                            {
                                let subtype = xobj_stream
                                    .dict
                                    .get(b"Subtype")
                                    .and_then(lopdf::Object::as_name_str)
                                    .unwrap_or("");
                                if subtype == "Image" {
                                    let pw = xobj_stream
                                        .dict
                                        .get(b"Width")
                                        .and_then(lopdf::Object::as_i64)
                                        .unwrap_or(0)
                                        as u32;
                                    let ph = xobj_stream
                                        .dict
                                        .get(b"Height")
                                        .and_then(lopdf::Object::as_i64)
                                        .unwrap_or(0)
                                        as u32;
                                    if pw > 0 && ph > 0 {
                                        let raw_bytes = decompress_pdf_stream(xobj_stream)
                                            .unwrap_or_else(|_| xobj_stream.content.clone());
                                        let cs = xobj_stream
                                            .dict
                                            .get(b"ColorSpace")
                                            .and_then(lopdf::Object::as_name_str)
                                            .unwrap_or("DeviceRGB");
                                        let bpc = xobj_stream
                                            .dict
                                            .get(b"BitsPerComponent")
                                            .ok()
                                            .and_then(|o| o.as_i64().ok())
                                            .unwrap_or(8)
                                            as usize;
                                        let decode_parms =
                                            xobj_stream.dict.get(b"DecodeParms").ok().and_then(
                                                |o| match o {
                                                    lopdf::Object::Reference(r) => doc
                                                        .get_object(*r)
                                                        .and_then(lopdf::Object::as_dict)
                                                        .ok(),
                                                    lopdf::Object::Dictionary(d) => Some(d),
                                                    _ => None,
                                                },
                                            );

                                        let smask_bytes: Option<Vec<u8>> = if let Ok(smask_obj) =
                                            xobj_stream.dict.get(b"SMask")
                                        {
                                            let smask_stream = match smask_obj {
                                                lopdf::Object::Reference(r) => doc
                                                    .get_object(*r)
                                                    .and_then(lopdf::Object::as_stream)
                                                    .ok(),
                                                lopdf::Object::Stream(s) => Some(s),
                                                _ => None,
                                            };
                                            smask_stream.and_then(|s| decompress_pdf_stream(s).ok())
                                        } else {
                                            None
                                        };

                                        let rgba = convert_image_bytes_to_rgba(
                                            &raw_bytes,
                                            pw,
                                            ph,
                                            cs,
                                            bpc,
                                            decode_parms,
                                            smask_bytes.as_deref(),
                                        );
                                        if !rgba.is_empty() {
                                            let (oriented_rgba, final_pw, final_ph) =
                                                rotate_rgba_if_needed(rgba, pw, ph, &gstate.ctm);

                                            let p0 = transform_point(&gstate.ctm, 0.0, 0.0);
                                            let p1 = transform_point(&gstate.ctm, 1.0, 0.0);
                                            let p2 = transform_point(&gstate.ctm, 1.0, 1.0);
                                            let p3 = transform_point(&gstate.ctm, 0.0, 1.0);

                                            let min_x =
                                                p0.0.min(p1.0).min(p2.0).min(p3.0) - media_x0;
                                            let max_x =
                                                p0.0.max(p1.0).max(p2.0).max(p3.0) - media_x0;
                                            let min_y =
                                                p0.1.min(p1.1).min(p2.1).min(p3.1) - media_y0;
                                            let max_y =
                                                p0.1.max(p1.1).max(p2.1).max(p3.1) - media_y0;
                                            let w = max_x - min_x;
                                            let h = max_y - min_y;

                                            images.push(VisualImage {
                                                x: min_x,
                                                y: min_y,
                                                width: if w > 0.01 { w } else { final_pw as f32 },
                                                height: if h > 0.01 { h } else { final_ph as f32 },
                                                pixel_width: final_pw,
                                                pixel_height: final_ph,
                                                rgba: oriented_rgba,
                                            });
                                        }
                                    }
                                } else if subtype == "Form" && depth < 8 {
                                    // Recursive Form XObject execution
                                    let saved_gstate = gstate.clone();
                                    if let Ok(mat_arr) = xobj_stream
                                        .dict
                                        .get(b"Matrix")
                                        .and_then(lopdf::Object::as_array)
                                    {
                                        if mat_arr.len() >= 6 {
                                            let m = [
                                                get_op_float(&mat_arr[0]),
                                                get_op_float(&mat_arr[1]),
                                                get_op_float(&mat_arr[2]),
                                                get_op_float(&mat_arr[3]),
                                                get_op_float(&mat_arr[4]),
                                                get_op_float(&mat_arr[5]),
                                            ];
                                            gstate.ctm = multiply_matrix(&m, &gstate.ctm);
                                        }
                                    }

                                    let child_res =
                                        if let Ok(res_obj) = xobj_stream.dict.get(b"Resources") {
                                            let rd_opt = match res_obj {
                                                lopdf::Object::Reference(r) => doc
                                                    .get_object(*r)
                                                    .and_then(lopdf::Object::as_dict)
                                                    .ok(),
                                                lopdf::Object::Dictionary(d) => Some(d),
                                                _ => None,
                                            };
                                            if let Some(rd) = rd_opt {
                                                resources.merge(&extract_resources(doc, rd))
                                            } else {
                                                resources.clone()
                                            }
                                        } else {
                                            resources.clone()
                                        };

                                    let form_bytes = decompress_pdf_stream(xobj_stream)
                                        .unwrap_or_else(|_| xobj_stream.content.clone());
                                    if let Ok(form_content) =
                                        lopdf::content::Content::decode(&form_bytes)
                                    {
                                        process_content_operations(
                                            doc,
                                            &form_content.operations,
                                            &child_res,
                                            gstate,
                                            gstate_stack,
                                            text_runs,
                                            rects,
                                            images,
                                            media_x0,
                                            media_y0,
                                            depth + 1,
                                        );
                                    }
                                    *gstate = saved_gstate;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Extracts high-fidelity visual layout (positioned text runs and vector rects)
/// from a PDF page's decompressed content stream.
pub fn extract_page_layout(
    doc: &lopdf::Document,
    page_id: lopdf::ObjectId,
    page_width: f32,
    page_height: f32,
    media_x0: f32,
    media_y0: f32,
) -> PageVisualLayout {
    // 1. Extract base page resources (fonts, ToUnicode CMaps, encodings, and XObjects)
    let mut page_resources = ResourceContext::default();
    if let Ok(page_dict) = doc.get_object(page_id).and_then(lopdf::Object::as_dict) {
        let res_opt = match page_dict.get(b"Resources") {
            Ok(lopdf::Object::Reference(r)) => {
                doc.get_object(*r).and_then(lopdf::Object::as_dict).ok()
            }
            Ok(lopdf::Object::Dictionary(d)) => Some(d),
            _ => None,
        };
        if let Some(res) = res_opt {
            page_resources = extract_resources(doc, res);
        }
    }

    // Also merge any fonts discoverable via lopdf helper
    let page_fonts = doc.get_page_fonts(page_id);
    for (font_name, font_dict) in &page_fonts {
        if !page_resources.font_encodings.contains_key(font_name) {
            let enc_obj = match font_dict.get(b"Encoding") {
                Ok(lopdf::Object::Reference(r)) => doc.get_object(*r).ok(),
                Ok(obj) => Some(obj),
                _ => None,
            };
            if let Some(obj) = enc_obj {
                if let Ok(enc_name) = obj.as_name_str() {
                    page_resources
                        .font_encodings
                        .insert(font_name.clone(), enc_name.to_string());
                } else if let Ok(enc_dict) = obj.as_dict() {
                    if let Ok(base_enc) = enc_dict
                        .get(b"BaseEncoding")
                        .and_then(lopdf::Object::as_name_str)
                    {
                        page_resources
                            .font_encodings
                            .insert(font_name.clone(), base_enc.to_string());
                    }
                }
            }
        }
    }

    // 2. Decompress page content operations
    let content_bytes = match get_page_content_decompressed(doc, page_id) {
        Ok(bytes) => bytes,
        Err(_) => doc.get_page_content(page_id).unwrap_or_default(),
    };

    let mut text_runs = Vec::new();
    let mut rects = Vec::new();
    let mut images = Vec::new();

    if let Ok(content) = lopdf::content::Content::decode(&content_bytes) {
        let mut gstate = GraphicsGState::default();
        let mut gstate_stack = Vec::new();
        process_content_operations(
            doc,
            &content.operations,
            &page_resources,
            &mut gstate,
            &mut gstate_stack,
            &mut text_runs,
            &mut rects,
            &mut images,
            media_x0,
            media_y0,
            0,
        );
    }

    // 3. Process page annotations (digital signature appearance streams, stamps, badges)
    if let Ok(page_dict) = doc.get_object(page_id).and_then(lopdf::Object::as_dict) {
        if let Ok(annots_obj) = page_dict.get(b"Annots") {
            let annot_arr_opt = match annots_obj {
                lopdf::Object::Array(arr) => Some(arr.clone()),
                lopdf::Object::Reference(r) => doc
                    .get_object(*r)
                    .and_then(lopdf::Object::as_array)
                    .ok()
                    .cloned(),
                _ => None,
            };
            if let Some(annot_arr) = annot_arr_opt {
                for annot_ref in &annot_arr {
                    let annot_dict_opt = match annot_ref {
                        lopdf::Object::Reference(r) => {
                            doc.get_object(*r).and_then(lopdf::Object::as_dict).ok()
                        }
                        lopdf::Object::Dictionary(d) => Some(d),
                        _ => None,
                    };
                    if let Some(ad) = annot_dict_opt {
                        if let Ok(ap_obj) = ad.get(b"AP") {
                            let ap_dict_opt = match ap_obj {
                                lopdf::Object::Reference(r) => {
                                    doc.get_object(*r).and_then(lopdf::Object::as_dict).ok()
                                }
                                lopdf::Object::Dictionary(d) => Some(d),
                                _ => None,
                            };
                            if let Some(ap_dict) = ap_dict_opt {
                                if let Ok(n_obj) = ap_dict.get(b"N") {
                                    let stream_opt = match n_obj {
                                        lopdf::Object::Reference(r) => doc
                                            .get_object(*r)
                                            .and_then(lopdf::Object::as_stream)
                                            .ok(),
                                        lopdf::Object::Stream(s) => Some(s),
                                        _ => None,
                                    };
                                    if let Some(ap_stream) = stream_opt {
                                        let rect = ad
                                            .get(b"Rect")
                                            .ok()
                                            .and_then(parse_rect)
                                            .unwrap_or([0.0, 0.0, 100.0, 100.0]);
                                        let bbox = ap_stream
                                            .dict
                                            .get(b"BBox")
                                            .ok()
                                            .and_then(parse_rect)
                                            .unwrap_or([
                                                0.0,
                                                0.0,
                                                rect[2] - rect[0],
                                                rect[3] - rect[1],
                                            ]);

                                        let rect_w = rect[2] - rect[0];
                                        let rect_h = rect[3] - rect[1];
                                        let bbox_w = bbox[2] - bbox[0];
                                        let bbox_h = bbox[3] - bbox[1];

                                        let (sx, sy, tx, ty) = if bbox_w > 0.001 && bbox_h > 0.001 {
                                            let sx = rect_w / bbox_w;
                                            let sy = rect_h / bbox_h;
                                            let tx = rect[0] - bbox[0] * sx;
                                            let ty = rect[1] - bbox[1] * sy;
                                            (sx, sy, tx, ty)
                                        } else {
                                            (1.0, 1.0, rect[0], rect[1])
                                        };

                                        let mut init_matrix = [sx, 0.0, 0.0, sy, tx, ty];
                                        if let Ok(mat_arr) = ap_stream
                                            .dict
                                            .get(b"Matrix")
                                            .and_then(lopdf::Object::as_array)
                                        {
                                            if mat_arr.len() >= 6 {
                                                let m = [
                                                    get_op_float(&mat_arr[0]),
                                                    get_op_float(&mat_arr[1]),
                                                    get_op_float(&mat_arr[2]),
                                                    get_op_float(&mat_arr[3]),
                                                    get_op_float(&mat_arr[4]),
                                                    get_op_float(&mat_arr[5]),
                                                ];
                                                init_matrix = multiply_matrix(&m, &init_matrix);
                                            }
                                        }

                                        let ap_res =
                                            if let Ok(res_obj) = ap_stream.dict.get(b"Resources") {
                                                let rd_opt = match res_obj {
                                                    lopdf::Object::Reference(r) => doc
                                                        .get_object(*r)
                                                        .and_then(lopdf::Object::as_dict)
                                                        .ok(),
                                                    lopdf::Object::Dictionary(d) => Some(d),
                                                    _ => None,
                                                };
                                                if let Some(rd) = rd_opt {
                                                    extract_resources(doc, rd)
                                                } else {
                                                    ResourceContext::default()
                                                }
                                            } else {
                                                ResourceContext::default()
                                            };
                                        let annot_res = page_resources.merge(&ap_res);

                                        let mut gstate = GraphicsGState {
                                            ctm: init_matrix,
                                            ..GraphicsGState::default()
                                        };
                                        let mut gstate_stack = Vec::new();

                                        let ap_bytes = decompress_pdf_stream(ap_stream)
                                            .unwrap_or_else(|_| ap_stream.content.clone());
                                        if let Ok(ap_content) =
                                            lopdf::content::Content::decode(&ap_bytes)
                                        {
                                            process_content_operations(
                                                doc,
                                                &ap_content.operations,
                                                &annot_res,
                                                &mut gstate,
                                                &mut gstate_stack,
                                                &mut text_runs,
                                                &mut rects,
                                                &mut images,
                                                media_x0,
                                                media_y0,
                                                0,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Generate plain_text from text runs
    let mut plain_text = String::new();
    if !text_runs.is_empty() {
        let mut sorted = text_runs.clone();
        sorted.sort_by(|a, b| {
            let band_a = -(a.y / 4.0).round() as i32;
            let band_b = -(b.y / 4.0).round() as i32;
            band_a
                .cmp(&band_b)
                .then_with(|| (a.x as i32).cmp(&(b.x as i32)))
        });

        let mut last_band = None;
        for tr in &sorted {
            let band = -(tr.y / 4.0).round() as i32;
            if let Some(lb) = last_band {
                if band != lb {
                    plain_text.push('\n');
                } else {
                    plain_text.push(' ');
                }
            }
            plain_text.push_str(&tr.text);
            last_band = Some(band);
        }
    } else {
        // Fallback: use lopdf native extract_text if available
        if let Ok(fallback) = doc.extract_text(&[page_id.0]) {
            plain_text = sanitize_extracted_text(&fallback);
        }
    }

    PageVisualLayout {
        width_pt: page_width,
        height_pt: page_height,
        text_runs,
        rects,
        images,
        plain_text,
    }
}

/// Extracts page text robustly, decoding Identity-H and ToUnicode CMaps while sanitizing any unimplemented tags.
pub fn extract_page_text_robust(doc: &lopdf::Document, page_num: u32) -> String {
    let pages = doc.get_pages();
    let page_id = match pages.get(&page_num) {
        Some(&id) => id,
        None => return String::new(),
    };

    let layout = extract_page_layout(doc, page_id, 595.28, 841.89, 0.0, 0.0);
    if !layout.plain_text.trim().is_empty() {
        return layout.plain_text;
    }

    // Fallback: lopdf built-in extraction, strictly stripped of any Identity-H Unimplemented tags
    let fallback = doc.extract_text(&[page_num]).unwrap_or_default();
    sanitize_extracted_text(&fallback)
}

fn extract_hex_tokens(line: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('<') {
        if let Some(end) = rest[start..].find('>') {
            let hex = &rest[start + 1..start + end];
            tokens.push(hex.trim());
            rest = &rest[start + end + 1..];
        } else {
            break;
        }
    }
    tokens
}

/// Represents an extracted ToUnicode CMap supporting 1-byte, 2-byte, or mixed codespaces.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToUnicodeCMap {
    /// Mapping from character code to decoded Unicode string.
    pub map: std::collections::HashMap<u16, String>,
    /// Default byte length per character code (1 or 2).
    pub code_bytes: usize,
    /// Explicit codespace ranges (start, end, byte_len) parsed from begincodespacerange.
    pub codespace_ranges: Vec<(u32, u32, usize)>,
}

impl std::ops::Deref for ToUnicodeCMap {
    type Target = std::collections::HashMap<u16, String>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl ToUnicodeCMap {
    /// Returns true if the CMap contains no character mappings.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Decodes a raw byte slice from a content stream string into the output buffer.
    pub fn decode_bytes(&self, bytes: &[u8], output: &mut String) {
        if bytes.is_empty() {
            return;
        }

        // 1. If explicit codespace ranges are present, match each position
        if !self.codespace_ranges.is_empty() {
            let mut idx = 0;
            while idx < bytes.len() {
                let remaining = bytes.len() - idx;
                let mut matched = false;

                // Check 1-byte ranges first if remaining >= 1
                let b0 = bytes[idx] as u32;
                for &(start, end, len) in &self.codespace_ranges {
                    if len == 1 && b0 >= start && b0 <= end {
                        let code = b0 as u16;
                        if let Some(s) = self.map.get(&code) {
                            output.push_str(s);
                        } else if (32..=126).contains(&b0) || b0 == 10 || b0 == 9 {
                            output.push((b0 as u8) as char);
                        }
                        idx += 1;
                        matched = true;
                        break;
                    }
                }
                if matched {
                    continue;
                }

                // Check 2-byte ranges if remaining >= 2
                if remaining >= 2 {
                    let code2 = u16::from_be_bytes([bytes[idx], bytes[idx + 1]]) as u32;
                    for &(start, end, len) in &self.codespace_ranges {
                        if len == 2 && code2 >= start && code2 <= end {
                            let code = code2 as u16;
                            if let Some(s) = self.map.get(&code) {
                                output.push_str(s);
                            } else if code <= 127
                                && ((32..=126).contains(&(code as u8)) || code == 10 || code == 9)
                            {
                                output.push((code as u8) as char);
                            }
                            idx += 2;
                            matched = true;
                            break;
                        }
                    }
                    if matched {
                        continue;
                    }
                }

                // Fallback to self.code_bytes if no range matched
                if self.code_bytes == 1 || remaining < 2 {
                    let code = bytes[idx] as u16;
                    if let Some(s) = self.map.get(&code) {
                        output.push_str(s);
                    } else if (32..=126).contains(&bytes[idx])
                        || bytes[idx] == 10
                        || bytes[idx] == 9
                    {
                        output.push(bytes[idx] as char);
                    }
                    idx += 1;
                } else {
                    let code = u16::from_be_bytes([bytes[idx], bytes[idx + 1]]);
                    if let Some(s) = self.map.get(&code) {
                        output.push_str(s);
                    } else if code <= 127
                        && ((32..=126).contains(&(code as u8)) || code == 10 || code == 9)
                    {
                        output.push((code as u8) as char);
                    }
                    idx += 2;
                }
            }
            return;
        }

        // 2. If no codespace ranges defined, use self.code_bytes
        if self.code_bytes == 1 {
            for &b in bytes {
                let code = b as u16;
                if let Some(s) = self.map.get(&code) {
                    output.push_str(s);
                } else if (32..=126).contains(&b) || b == 10 || b == 9 {
                    output.push(b as char);
                }
            }
        } else {
            for chunk in bytes.chunks(2) {
                let code = if chunk.len() == 2 {
                    u16::from_be_bytes([chunk[0], chunk[1]])
                } else {
                    chunk[0] as u16
                };

                if let Some(s) = self.map.get(&code) {
                    output.push_str(s);
                } else if code <= 127
                    && ((32..=126).contains(&(code as u8)) || code == 10 || code == 9)
                {
                    output.push((code as u8) as char);
                }
            }
        }
    }
}

pub fn parse_to_unicode_cmap(bytes: &[u8]) -> ToUnicodeCMap {
    let mut cmap = ToUnicodeCMap {
        map: std::collections::HashMap::new(),
        code_bytes: 2,
        codespace_ranges: Vec::new(),
    };
    let text = String::from_utf8_lossy(bytes);

    let mut in_codespace = false;
    let mut in_bfchar = false;
    let mut in_bfrange = false;
    let mut max_src_hex_len = 0;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.contains("begincodespacerange") {
            in_codespace = true;
            in_bfchar = false;
            in_bfrange = false;
            continue;
        }
        if trimmed.contains("endcodespacerange") {
            in_codespace = false;
            continue;
        }
        if trimmed.contains("beginbfchar") {
            in_bfchar = true;
            in_codespace = false;
            in_bfrange = false;
            continue;
        }
        if trimmed.contains("endbfchar") {
            in_bfchar = false;
            continue;
        }
        if trimmed.contains("beginbfrange") {
            in_bfrange = true;
            in_codespace = false;
            in_bfchar = false;
            continue;
        }
        if trimmed.contains("endbfrange") {
            in_bfrange = false;
            continue;
        }

        if in_codespace {
            let tokens = extract_hex_tokens(trimmed);
            for chunk in tokens.chunks(2) {
                if chunk.len() == 2 {
                    let start_hex = chunk[0];
                    let end_hex = chunk[1];
                    let byte_len = if start_hex.len() <= 2 { 1 } else { 2 };
                    if let (Ok(start), Ok(end)) = (
                        u32::from_str_radix(start_hex, 16),
                        u32::from_str_radix(end_hex, 16),
                    ) {
                        cmap.codespace_ranges.push((start, end, byte_len));
                    }
                }
            }
        }

        if in_bfchar {
            let tokens = extract_hex_tokens(trimmed);
            for chunk in tokens.chunks(2) {
                if chunk.len() == 2 {
                    let src_hex = chunk[0];
                    let dst_hex = chunk[1];
                    max_src_hex_len = max_src_hex_len.max(src_hex.len());
                    if let Ok(src_code) = u16::from_str_radix(src_hex, 16) {
                        let dst_str = hex_to_utf16_string(dst_hex);
                        if !dst_str.is_empty() {
                            cmap.map.insert(src_code, dst_str);
                        }
                    }
                }
            }
        }

        if in_bfrange {
            let tokens = extract_hex_tokens(trimmed);
            if tokens.len() >= 3 {
                let start_hex = tokens[0];
                let end_hex = tokens[1];
                max_src_hex_len = max_src_hex_len.max(start_hex.len());
                if let (Ok(start), Ok(end)) = (
                    u16::from_str_radix(start_hex, 16),
                    u16::from_str_radix(end_hex, 16),
                ) {
                    if tokens.len() == 3 {
                        let dst_start_hex = tokens[2];
                        if let Ok(dst_start) = u32::from_str_radix(dst_start_hex, 16) {
                            for code in start..=end {
                                let offset = (code - start) as u32;
                                let dst_code = dst_start + offset;
                                if let Some(ch) = char::from_u32(dst_code) {
                                    cmap.map.insert(code, ch.to_string());
                                }
                            }
                        }
                    } else {
                        // Array form: <start> <end> [ <dst0> <dst1> ... ]
                        for (idx, code) in (start..=end).enumerate() {
                            if let Some(&dst_hex) = tokens.get(2 + idx) {
                                let dst_str = hex_to_utf16_string(dst_hex);
                                if !dst_str.is_empty() {
                                    cmap.map.insert(code, dst_str);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Determine default code_bytes
    if !cmap.codespace_ranges.is_empty() {
        if cmap.codespace_ranges.iter().all(|(_, _, len)| *len == 1) {
            cmap.code_bytes = 1;
        } else {
            cmap.code_bytes = 2;
        }
    } else if max_src_hex_len > 0 && max_src_hex_len <= 2 {
        cmap.code_bytes = 1;
    } else {
        cmap.code_bytes = 2;
    }

    cmap
}

fn hex_to_utf16_string(hex: &str) -> String {
    let mut u16_units = Vec::new();
    let mut chars = hex.chars();
    while let (Some(c1), Some(c2), Some(c3), Some(c4)) =
        (chars.next(), chars.next(), chars.next(), chars.next())
    {
        let chunk: String = [c1, c2, c3, c4].iter().collect();
        if let Ok(val) = u16::from_str_radix(&chunk, 16) {
            u16_units.push(val);
        }
    }
    if !u16_units.is_empty() {
        String::from_utf16_lossy(&u16_units)
    } else if let Ok(byte_val) = u8::from_str_radix(hex, 16) {
        (byte_val as char).to_string()
    } else {
        String::new()
    }
}

fn decode_single_string(
    output: &mut String,
    bytes: &[u8],
    cmap: Option<&ToUnicodeCMap>,
    encoding: Option<&str>,
) {
    if bytes.is_empty() {
        return;
    }

    // 1. If CMap is available, use it (handles 1-byte, 2-byte, or mixed codespaces dynamically)
    if let Some(cmap) = cmap {
        cmap.decode_bytes(bytes, output);
        return;
    }

    // 2. If encoding is Identity-H without CMap: decode as 2-byte UTF-16BE
    if encoding == Some("Identity-H") {
        for chunk in bytes.chunks(2) {
            let code = if chunk.len() == 2 {
                u16::from_be_bytes([chunk[0], chunk[1]])
            } else {
                chunk[0] as u16
            };

            if let Some(ch) = char::from_u32(code as u32) {
                if !ch.is_control() || ch == '\n' || ch == '\t' {
                    output.push(ch);
                }
            }
        }
        return;
    }

    // 3. UTF-16BE with BOM
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let decoded = lopdf::Document::decode_text(encoding, bytes);
        let sanitized = sanitize_extracted_text(&decoded);
        output.push_str(&sanitized);
        return;
    }

    // 4. Check for valid UTF-8
    if let Ok(utf8_str) = std::str::from_utf8(bytes) {
        let sanitized = sanitize_extracted_text(utf8_str);
        if !sanitized.is_empty() {
            output.push_str(&sanitized);
            return;
        }
    }

    // 5. Standard encoding / ASCII / WinAnsi fallback
    let decoded = lopdf::Document::decode_text(encoding, bytes);
    let sanitized = sanitize_extracted_text(&decoded);
    if !sanitized.is_empty() {
        output.push_str(&sanitized);
    } else {
        // Fallback: decode Windows-1252 / ISO-8859-1 for accented characters (e.g. Spanish, French, German)
        for &b in bytes {
            if (32..=126).contains(&b) || b == b'\n' || b == b'\t' {
                output.push(b as char);
            } else if b >= 160 {
                // ISO-8859-1 codepoints 160..=255 map directly to Unicode U+00A0..=U+00FF
                if let Some(ch) = char::from_u32(b as u32) {
                    output.push(ch);
                }
            } else {
                // Windows-1252 specific symbols (128..=159)
                match b {
                    128 => output.push('€'),
                    130 => output.push('‚'),
                    131 => output.push('ƒ'),
                    132 => output.push('„'),
                    133 => output.push('…'),
                    134 => output.push('†'),
                    135 => output.push('‡'),
                    136 => output.push('ˆ'),
                    137 => output.push('‰'),
                    138 => output.push('Š'),
                    139 => output.push('‹'),
                    140 => output.push('Œ'),
                    142 => output.push('Ž'),
                    145 => output.push('‘'),
                    146 => output.push('’'),
                    147 => output.push('“'),
                    148 => output.push('”'),
                    149 => output.push('•'),
                    150 => output.push('–'),
                    151 => output.push('—'),
                    152 => output.push('˜'),
                    153 => output.push('™'),
                    154 => output.push('š'),
                    155 => output.push('›'),
                    156 => output.push('œ'),
                    158 => output.push('ž'),
                    159 => output.push('Ÿ'),
                    _ => {}
                }
            }
        }
    }
}

pub fn sanitize_extracted_text(text: &str) -> String {
    text.replace("?Identity-H Unimplemented?", "")
        .replace("Identity-H Unimplemented", "")
}
