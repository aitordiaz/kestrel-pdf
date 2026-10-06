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

/// Represents an active document session with parsed geometry, forms, and signatures.
pub struct DocumentSession {
    pub file_path: Option<PathBuf>,
    pub raw_bytes: Vec<u8>,
    pub page_count: u16,
    pub pages: Vec<PageInfo>,
    pub page_texts: Vec<String>,
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
        let page_dict_map = doc.get_pages();

        for (page_num, object_id) in &page_dict_map {
            let mut width_pt = 595.28; // Standard A4 default
            let mut height_pt = 841.89;
            let mut rotation_degrees = 0;

            if let Ok(page_obj) = doc.get_object(*object_id) {
                if let Ok(dict) = page_obj.as_dict() {
                    // Check MediaBox [llx, lly, urx, ury]
                    if let Ok(mediabox) = dict.get(b"MediaBox").and_then(Object::as_array) {
                        if mediabox.len() >= 4 {
                            let x0 = mediabox[0].as_float().unwrap_or(0.0);
                            let y0 = mediabox[1].as_float().unwrap_or(0.0);
                            let x1 = mediabox[2].as_float().unwrap_or(595.28);
                            let y1 = mediabox[3].as_float().unwrap_or(841.89);
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

            // Extract page text robustly
            let text = extract_page_text_robust(&doc, *page_num);
            page_texts.push(text);
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

/// Extracts page text robustly, decoding Identity-H and ToUnicode CMaps while sanitizing any unimplemented tags.
pub fn extract_page_text_robust(doc: &lopdf::Document, page_num: u32) -> String {
    let pages = doc.get_pages();
    let page_id = match pages.get(&page_num) {
        Some(&id) => id,
        None => return String::new(),
    };

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
                let stream_bytes = stream
                    .decompressed_content()
                    .unwrap_or_else(|_| stream.content.clone());
                let cmap = parse_to_unicode_cmap(&stream_bytes);
                if !cmap.is_empty() {
                    font_cmaps.insert(font_name.clone(), cmap);
                }
            }
        }
    }

    // 2. Decode page content operations
    if let Ok(content_bytes) = doc.get_page_content(page_id) {
        if let Ok(content) = lopdf::content::Content::decode(&content_bytes) {
            let mut extracted = String::new();
            let mut current_font = Vec::new();

            for operation in &content.operations {
                match operation.operator.as_str() {
                    "Tf" => {
                        if let Some(font_obj) = operation.operands.first() {
                            if let Ok(f_name) = font_obj.as_name() {
                                current_font = f_name.to_vec();
                            }
                        }
                    }
                    "Tj" | "TJ" => {
                        decode_text_operands(
                            &mut extracted,
                            &operation.operands,
                            &current_font,
                            &font_cmaps,
                            &font_encodings,
                        );
                    }
                    "ET" | "T*" => {
                        if !extracted.ends_with('\n') {
                            extracted.push('\n');
                        }
                    }
                    "'" | "\"" => {
                        extracted.push('\n');
                        decode_text_operands(
                            &mut extracted,
                            &operation.operands,
                            &current_font,
                            &font_cmaps,
                            &font_encodings,
                        );
                    }
                    _ => {}
                }
            }

            let cleaned = sanitize_extracted_text(&extracted);
            if !cleaned.trim().is_empty() {
                return cleaned;
            }
        }
    }

    // 3. Fallback: lopdf built-in extraction, strictly stripped of any Identity-H Unimplemented tags
    let fallback = doc.extract_text(&[page_num]).unwrap_or_default();
    sanitize_extracted_text(&fallback)
}

fn parse_to_unicode_cmap(bytes: &[u8]) -> std::collections::HashMap<u16, String> {
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
            let tokens: Vec<&str> = trimmed
                .split_whitespace()
                .filter(|t| t.starts_with('<') && t.ends_with('>'))
                .collect();
            for chunk in tokens.chunks(2) {
                if chunk.len() == 2 {
                    let src_hex = chunk[0].trim_matches(|c| c == '<' || c == '>');
                    let dst_hex = chunk[1].trim_matches(|c| c == '<' || c == '>');
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
            let tokens: Vec<&str> = trimmed
                .split_whitespace()
                .filter(|t| t.starts_with('<') && t.ends_with('>'))
                .collect();
            if tokens.len() == 3 {
                let start_hex = tokens[0].trim_matches(|c| c == '<' || c == '>');
                let end_hex = tokens[1].trim_matches(|c| c == '<' || c == '>');
                let dst_start_hex = tokens[2].trim_matches(|c| c == '<' || c == '>');
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

fn decode_text_operands(
    output: &mut String,
    operands: &[lopdf::Object],
    current_font: &[u8],
    font_cmaps: &std::collections::HashMap<Vec<u8>, std::collections::HashMap<u16, String>>,
    font_encodings: &std::collections::HashMap<Vec<u8>, String>,
) {
    let cmap_opt = font_cmaps.get(current_font);
    let encoding_opt = font_encodings.get(current_font).map(|s| s.as_str());

    for op in operands {
        match op {
            lopdf::Object::String(bytes, _) => {
                decode_single_string(output, bytes, cmap_opt, encoding_opt);
            }
            lopdf::Object::Array(arr) => {
                for item in arr {
                    match item {
                        lopdf::Object::String(bytes, _) => {
                            decode_single_string(output, bytes, cmap_opt, encoding_opt);
                        }
                        lopdf::Object::Integer(i) if *i < -100 => {
                            output.push(' ');
                        }
                        lopdf::Object::Real(r) if *r < -100.0 => {
                            output.push(' ');
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
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

    // 3. Standard encoding / ASCII / WinAnsi
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
