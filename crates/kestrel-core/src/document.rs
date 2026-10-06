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
    pub color: [u8; 3], // RGB [0..255]
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
            visual_signatures: Vec::new(),
            digital_signature: None,
        })
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
    font_cmaps: &std::collections::HashMap<Vec<u8>, std::collections::HashMap<u16, String>>,
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
        let (wx, wy) = transform_point(&gstate.ctm, text_matrix[4], text_matrix[5]);
        let x = wx - media_x0;
        let y = wy - media_y0;
        let scale_tm = (text_matrix[0].powi(2) + text_matrix[1].powi(2)).sqrt();
        let scale_ctm = (gstate.ctm[0].powi(2) + gstate.ctm[1].powi(2)).sqrt();
        let total_scale = (scale_tm * scale_ctm).abs();
        let effective_size = if total_scale > 0.01 {
            current_font_size * total_scale
        } else {
            current_font_size
        };

        text_runs.push(PositionedText {
            text: text.clone(),
            x,
            y,
            font_size: effective_size,
            color: gstate.fill_color,
        });

        // Advance x position in text matrix
        let advance = (text.chars().count() as f32) * effective_size * 0.52;
        text_matrix[4] += advance;
    }
}

fn decode_text_operands_string(
    operands: &[lopdf::Object],
    current_font: &[u8],
    font_cmaps: &std::collections::HashMap<Vec<u8>, std::collections::HashMap<u16, String>>,
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

#[allow(clippy::chunks_exact_to_as_chunks)]
fn convert_image_bytes_to_rgba(
    raw_bytes: &[u8],
    width: u32,
    height: u32,
    colorspace: &str,
) -> Vec<u8> {
    let pixel_count = (width * height) as usize;
    let mut rgba = Vec::with_capacity(pixel_count * 4);

    if colorspace.contains("RGB") || colorspace.is_empty() {
        for chunk in raw_bytes.chunks_exact(3) {
            rgba.push(chunk[0]);
            rgba.push(chunk[1]);
            rgba.push(chunk[2]);
            rgba.push(255);
        }
    } else if colorspace.contains("Gray") {
        for &g in raw_bytes.iter().take(pixel_count) {
            rgba.push(g);
            rgba.push(g);
            rgba.push(g);
            rgba.push(255);
        }
    } else if raw_bytes.len() >= pixel_count * 4 {
        rgba.extend_from_slice(&raw_bytes[..pixel_count * 4]);
    } else {
        for _ in 0..pixel_count {
            rgba.extend_from_slice(&[180, 180, 180, 255]);
        }
    }

    if rgba.len() < pixel_count * 4 {
        rgba.resize(pixel_count * 4, 255);
    }
    rgba
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
    // 1. Extract font ToUnicode CMaps and encodings for this page
    let fonts = doc.get_page_fonts(page_id);
    let mut font_cmaps: std::collections::HashMap<Vec<u8>, std::collections::HashMap<u16, String>> =
        std::collections::HashMap::new();
    let mut font_encodings: std::collections::HashMap<Vec<u8>, String> =
        std::collections::HashMap::new();

    for (font_name, font_dict) in &fonts {
        if let Ok(enc_name) = font_dict.get(b"Encoding").and_then(Object::as_name_str) {
            font_encodings.insert(font_name.clone(), enc_name.to_string());
        }

        if let Ok(to_unicode_obj) = font_dict.get(b"ToUnicode") {
            let stream = match to_unicode_obj {
                Object::Reference(ref_id) => {
                    doc.get_object(*ref_id).and_then(Object::as_stream).ok()
                }
                Object::Stream(s) => Some(s),
                _ => None,
            };

            if let Some(stream) = stream {
                let stream_bytes =
                    decompress_pdf_stream(stream).unwrap_or_else(|_| stream.content.clone());
                let cmap = parse_to_unicode_cmap(&stream_bytes);
                if !cmap.is_empty() {
                    font_cmaps.insert(font_name.clone(), cmap);
                }
            }
        }
    }

    // 2. Extract XObjects dictionary from page resources
    let page_xobjects: std::collections::HashMap<Vec<u8>, lopdf::ObjectId> = {
        let mut map = std::collections::HashMap::new();
        if let Ok(page_dict) = doc.get_object(page_id).and_then(Object::as_dict) {
            let res_opt = match page_dict.get(b"Resources") {
                Ok(Object::Reference(r)) => doc.get_object(*r).and_then(Object::as_dict).ok(),
                Ok(Object::Dictionary(d)) => Some(d),
                _ => None,
            };
            if let Some(res) = res_opt {
                let xobj_dict_opt = match res.get(b"XObject") {
                    Ok(Object::Reference(r)) => doc.get_object(*r).and_then(Object::as_dict).ok(),
                    Ok(Object::Dictionary(d)) => Some(d),
                    _ => None,
                };
                if let Some(xobjs) = xobj_dict_opt {
                    for (name, obj) in xobjs.iter() {
                        if let Ok(ref_id) = obj.as_reference() {
                            map.insert(name.clone(), ref_id);
                        }
                    }
                }
            }
        }
        map
    };

    // 3. Decompress page content operations
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

        let mut text_matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let mut line_matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let mut current_font = Vec::new();
        let mut current_font_size = 12.0f32;
        let mut current_leading = 12.0f32;

        let mut pending_rects: Vec<(f32, f32, f32, f32)> = Vec::new();

        for operation in &content.operations {
            match operation.operator.as_str() {
                // Graphics state save / restore
                "q" => {
                    gstate_stack.push(gstate.clone());
                }
                "Q" => {
                    if let Some(restored) = gstate_stack.pop() {
                        gstate = restored;
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
                        &mut text_runs,
                        &operation.operands,
                        &current_font,
                        &font_cmaps,
                        &font_encodings,
                        &mut text_matrix,
                        &gstate,
                        current_font_size,
                        media_x0,
                        media_y0,
                    );
                }
                "\"" => {
                    if operation.operands.len() >= 3 {
                        line_matrix = multiply_matrix(
                            &[1.0, 0.0, 0.0, 1.0, 0.0, -current_leading],
                            &line_matrix,
                        );
                        text_matrix = line_matrix;
                        emit_text_run(
                            &mut text_runs,
                            &operation.operands[2..],
                            &current_font,
                            &font_cmaps,
                            &font_encodings,
                            &mut text_matrix,
                            &gstate,
                            current_font_size,
                            media_x0,
                            media_y0,
                        );
                    }
                }
                "Tj" | "TJ" => {
                    emit_text_run(
                        &mut text_runs,
                        &operation.operands,
                        &current_font,
                        &font_cmaps,
                        &font_encodings,
                        &mut text_matrix,
                        &gstate,
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
                            &mut rects, rx, ry, rw, rh, &gstate, media_x0, media_y0, true, false,
                        );
                    }
                    pending_rects.clear();
                }
                "s" | "S" => {
                    for &(rx, ry, rw, rh) in &pending_rects {
                        emit_vector_rect(
                            &mut rects, rx, ry, rw, rh, &gstate, media_x0, media_y0, false, true,
                        );
                    }
                    pending_rects.clear();
                }
                "b" | "B" | "b*" | "B*" => {
                    for &(rx, ry, rw, rh) in &pending_rects {
                        emit_vector_rect(
                            &mut rects, rx, ry, rw, rh, &gstate, media_x0, media_y0, true, true,
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
                            if let Some(&xobj_id) = page_xobjects.get(name_bytes) {
                                if let Ok(xobj_stream) =
                                    doc.get_object(xobj_id).and_then(Object::as_stream)
                                {
                                    let is_img = xobj_stream
                                        .dict
                                        .get(b"Subtype")
                                        .and_then(Object::as_name_str)
                                        .map(|s| s == "Image")
                                        .unwrap_or(false);
                                    if is_img {
                                        let pw = xobj_stream
                                            .dict
                                            .get(b"Width")
                                            .and_then(Object::as_i64)
                                            .unwrap_or(0)
                                            as u32;
                                        let ph = xobj_stream
                                            .dict
                                            .get(b"Height")
                                            .and_then(Object::as_i64)
                                            .unwrap_or(0)
                                            as u32;
                                        if pw > 0 && ph > 0 {
                                            let raw_bytes = decompress_pdf_stream(xobj_stream)
                                                .unwrap_or_else(|_| xobj_stream.content.clone());
                                            let cs = xobj_stream
                                                .dict
                                                .get(b"ColorSpace")
                                                .and_then(Object::as_name_str)
                                                .unwrap_or("DeviceRGB");
                                            let rgba =
                                                convert_image_bytes_to_rgba(&raw_bytes, pw, ph, cs);
                                            if !rgba.is_empty() {
                                                let (wx, wy) =
                                                    transform_point(&gstate.ctm, 0.0, 0.0);
                                                let scale_w = (gstate.ctm[0].powi(2)
                                                    + gstate.ctm[1].powi(2))
                                                .sqrt()
                                                .abs();
                                                let scale_h = (gstate.ctm[2].powi(2)
                                                    + gstate.ctm[3].powi(2))
                                                .sqrt()
                                                .abs();
                                                images.push(VisualImage {
                                                    x: wx - media_x0,
                                                    y: wy - media_y0,
                                                    width: if scale_w > 0.01 {
                                                        scale_w
                                                    } else {
                                                        pw as f32
                                                    },
                                                    height: if scale_h > 0.01 {
                                                        scale_h
                                                    } else {
                                                        ph as f32
                                                    },
                                                    pixel_width: pw,
                                                    pixel_height: ph,
                                                    rgba,
                                                });
                                            }
                                        }
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

pub fn parse_to_unicode_cmap(bytes: &[u8]) -> std::collections::HashMap<u16, String> {
    let mut cmap = std::collections::HashMap::new();
    let text = String::from_utf8_lossy(bytes);

    let mut in_bfchar = false;
    let mut in_bfrange = false;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.contains("beginbfchar") {
            in_bfchar = true;
            in_bfrange = false;
            continue;
        }
        if trimmed.contains("endbfchar") {
            in_bfchar = false;
            continue;
        }
        if trimmed.contains("beginbfrange") {
            in_bfrange = true;
            in_bfchar = false;
            continue;
        }
        if trimmed.contains("endbfrange") {
            in_bfrange = false;
            continue;
        }

        if in_bfchar {
            let tokens = extract_hex_tokens(trimmed);
            for chunk in tokens.chunks(2) {
                if chunk.len() == 2 {
                    let src_hex = chunk[0];
                    let dst_hex = chunk[1];
                    if let Ok(src_code) = u16::from_str_radix(src_hex, 16) {
                        let dst_str = hex_to_utf16_string(dst_hex);
                        if !dst_str.is_empty() {
                            cmap.insert(src_code, dst_str);
                        }
                    }
                }
            }
        }

        if in_bfrange {
            let tokens = extract_hex_tokens(trimmed);
            if tokens.len() == 3 {
                let start_hex = tokens[0];
                let end_hex = tokens[1];
                let dst_start_hex = tokens[2];
                if let (Ok(start), Ok(end), Ok(dst_start)) = (
                    u16::from_str_radix(start_hex, 16),
                    u16::from_str_radix(end_hex, 16),
                    u16::from_str_radix(dst_start_hex, 16),
                ) {
                    for code in start..=end {
                        let offset = code - start;
                        let dst_code = dst_start + offset;
                        if let Some(ch) = char::from_u32(dst_code as u32) {
                            cmap.insert(code, ch.to_string());
                        }
                    }
                }
            }
        }
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
    cmap: Option<&std::collections::HashMap<u16, String>>,
    encoding: Option<&str>,
) {
    if bytes.is_empty() {
        return;
    }

    // 1. If CMap is available, use it (2-byte keys)
    if let Some(cmap) = cmap {
        for chunk in bytes.chunks(2) {
            let code = if chunk.len() == 2 {
                u16::from_be_bytes([chunk[0], chunk[1]])
            } else {
                chunk[0] as u16
            };

            if let Some(mapped) = cmap.get(&code) {
                output.push_str(mapped);
            } else if let Some(ch) = char::from_u32(code as u32) {
                if !ch.is_control() || ch == '\n' || ch == '\t' {
                    output.push(ch);
                }
            }
        }
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
        // Fallback: extract any printable ASCII
        for &b in bytes {
            if (32..=126).contains(&b) || b == b'\n' || b == b'\t' {
                output.push(b as char);
            }
        }
    }
}

pub fn sanitize_extracted_text(text: &str) -> String {
    text.replace("?Identity-H Unimplemented?", "")
        .replace("Identity-H Unimplemented", "")
}
