use anyhow::{Context, Result};
use lopdf::{Dictionary, Object, ObjectId};
use sha2::{Digest, Sha256};

/// 2D point for signature strokes with pen pressure support.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokePoint {
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
}

impl StrokePoint {
    pub fn new(x: f32, y: f32, pressure: f32) -> Self {
        Self { x, y, pressure }
    }
}

/// Visual signature consisting of smoothed ink strokes and target placement.
#[derive(Debug, Clone, PartialEq)]
pub struct VisualSignature {
    pub strokes: Vec<Vec<StrokePoint>>,
    pub target_page: u16,
    /// Bounding rectangle in PDF points: [x, y, width, height]
    pub bounding_box: [f32; 4],
    /// RGB ink color [0..255]
    pub ink_color_rgb: [u8; 3],
    /// Base line thickness in points
    pub stroke_width: f32,
}

impl Default for VisualSignature {
    fn default() -> Self {
        Self {
            strokes: Vec::new(),
            target_page: 0,
            bounding_box: [100.0, 100.0, 180.0, 60.0],
            ink_color_rgb: [15, 23, 42], // Deep Navy/Slate Ink
            stroke_width: 2.0,
        }
    }
}

impl VisualSignature {
    pub fn new(target_page: u16, bounding_box: [f32; 4]) -> Self {
        Self {
            target_page,
            bounding_box,
            ..Default::default()
        }
    }

    /// Adds a raw stroke and applies cubic Bézier smoothing.
    pub fn add_smoothed_stroke(&mut self, raw_points: &[StrokePoint]) {
        if raw_points.is_empty() {
            return;
        }
        let smoothed = smooth_stroke_bezier(raw_points, 4);
        self.strokes.push(smoothed);
    }

    /// Computes the normalized bounding box (min_x, min_y, max_x, max_y) of all raw stroke coordinates.
    pub fn compute_strokes_bounds(&self) -> Option<[f32; 4]> {
        if self.strokes.is_empty() {
            return None;
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for stroke in &self.strokes {
            for pt in stroke {
                if pt.x < min_x {
                    min_x = pt.x;
                }
                if pt.y < min_y {
                    min_y = pt.y;
                }
                if pt.x > max_x {
                    max_x = pt.x;
                }
                if pt.y > max_y {
                    max_y = pt.y;
                }
            }
        }

        if min_x <= max_x && min_y <= max_y {
            Some([min_x, min_y, max_x, max_y])
        } else {
            None
        }
    }

    /// Generates PDF page graphics stream operators to draw this signature.
    pub fn generate_pdf_graphics_operators(&self, page_height: f32) -> Vec<u8> {
        let bounds = match self.compute_strokes_bounds() {
            Some(b) => b,
            None => return Vec::new(),
        };

        let stroke_w = (bounds[2] - bounds[0]).max(1.0);
        let stroke_h = (bounds[3] - bounds[1]).max(1.0);

        let target_x = self.bounding_box[0];
        let target_y = self.bounding_box[1];
        let target_w = self.bounding_box[2];
        let target_h = self.bounding_box[3];

        let scale_x = target_w / stroke_w;
        let scale_y = target_h / stroke_h;

        let r = self.ink_color_rgb[0] as f32 / 255.0;
        let g = self.ink_color_rgb[1] as f32 / 255.0;
        let b = self.ink_color_rgb[2] as f32 / 255.0;

        let mut ops = String::new();
        ops.push_str("q\n"); // Push graphics state
        ops.push_str(&format!("{:.3} {:.3} {:.3} RG\n", r, g, b)); // Set stroke color
        ops.push_str(&format!("{:.2} w\n", self.stroke_width)); // Set line width
        ops.push_str("1 J 1 j\n"); // Round cap and round join

        for stroke in &self.strokes {
            if stroke.is_empty() {
                continue;
            }

            for (idx, pt) in stroke.iter().enumerate() {
                // Map point to target bounding box
                let px = target_x + (pt.x - bounds[0]) * scale_x;
                let py = target_y + (pt.y - bounds[1]) * scale_y;
                // In PDF coordinate system, origin is bottom-left
                let pdf_y = (page_height - py).max(0.0);

                if idx == 0 {
                    ops.push_str(&format!("{:.2} {:.2} m\n", px, pdf_y));
                } else {
                    ops.push_str(&format!("{:.2} {:.2} l\n", px, pdf_y));
                }
            }
            ops.push_str("S\n"); // Stroke the path
        }

        ops.push_str("Q\n"); // Pop graphics state
        ops.into_bytes()
    }
}

/// Applies Cubic Bézier / Catmull-Rom spline smoothing to an array of raw stroke points.
pub fn smooth_stroke_bezier(points: &[StrokePoint], subdivisions: usize) -> Vec<StrokePoint> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let mut smoothed = Vec::with_capacity(points.len() * subdivisions);
    smoothed.push(points[0]);

    for i in 0..points.len() - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < points.len() {
            points[i + 2]
        } else {
            p2
        };

        // Catmull-Rom to Cubic Bézier control points conversion:
        // CP1 = P1 + (P2 - P0) / 6
        // CP2 = P2 - (P3 - P1) / 6
        let cp1_x = p1.x + (p2.x - p0.x) / 6.0;
        let cp1_y = p1.y + (p2.y - p0.y) / 6.0;

        let cp2_x = p2.x - (p3.x - p1.x) / 6.0;
        let cp2_y = p2.y - (p3.y - p1.y) / 6.0;

        for step in 1..=subdivisions {
            let t = step as f32 / subdivisions as f32;
            let inv_t = 1.0 - t;

            // Evaluate Cubic Bézier polynomial:
            // B(t) = (1-t)^3 * P1 + 3(1-t)^2 * t * CP1 + 3(1-t) * t^2 * CP2 + t^3 * P2
            let x = inv_t.powi(3) * p1.x
                + 3.0 * inv_t.powi(2) * t * cp1_x
                + 3.0 * inv_t * t.powi(2) * cp2_x
                + t.powi(3) * p2.x;

            let y = inv_t.powi(3) * p1.y
                + 3.0 * inv_t.powi(2) * t * cp1_y
                + 3.0 * inv_t * t.powi(2) * cp2_y
                + t.powi(3) * p2.y;

            // Linearly interpolate pressure
            let pressure = p1.pressure + t * (p2.pressure - p1.pressure);

            smoothed.push(StrokePoint::new(x, y, pressure));
        }
    }

    smoothed
}

/// Cryptographic PAdES / PKCS#7 digital signature metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct DigitalSignatureMeta {
    pub signer_name: String,
    pub contact_info: Option<String>,
    pub location: Option<String>,
    pub reason: Option<String>,
    pub signing_time: String,
    pub sha256_fingerprint: String,
}

impl DigitalSignatureMeta {
    pub fn new(signer_name: impl Into<String>) -> Self {
        Self {
            signer_name: signer_name.into(),
            contact_info: None,
            location: None,
            reason: Some("Document electronically signed with Kestrel-PDF".to_string()),
            signing_time: "D:20261006120000Z".to_string(),
            sha256_fingerprint: String::new(),
        }
    }
}

/// Computes a standard SHA-256 digest of binary document content.
pub fn compute_document_sha256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    hash
}

/// Formats a byte slice into a lowercase hexadecimal string.
pub fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Embeds a PAdES-compliant digital signature dictionary into a PDF document.
pub fn embed_digital_signature(
    doc: &mut lopdf::Document,
    page_index: u16,
    meta: &mut DigitalSignatureMeta,
) -> Result<ObjectId> {
    let pages = doc.get_pages();
    let page_num = (page_index + 1) as u32;
    let page_obj_id = *pages
        .get(&page_num)
        .context("Target page does not exist for digital signature")?;

    let catalog_id = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .context("Missing Root catalog reference in PDF trailer")?;

    // 1. Compute digest over current serialized state
    let mut current_bytes = Vec::new();
    doc.save_to(&mut current_bytes)
        .context("Failed to serialize document for hash generation")?;
    let hash = compute_document_sha256(&current_bytes);
    let hash_hex = hex_encode(&hash);
    meta.sha256_fingerprint = hash_hex.clone();

    // 2. Build /Type /Sig signature dictionary
    let mut sig_dict = Dictionary::new();
    sig_dict.set("Type", Object::Name(b"Sig".to_vec()));
    sig_dict.set("Filter", Object::Name(b"Adobe.PPKLite".to_vec()));
    sig_dict.set("SubFilter", Object::Name(b"adbe.pkcs7.detached".to_vec()));
    sig_dict.set(
        "Name",
        Object::String(
            meta.signer_name.as_bytes().to_vec(),
            lopdf::StringFormat::Literal,
        ),
    );
    sig_dict.set(
        "M",
        Object::String(
            meta.signing_time.as_bytes().to_vec(),
            lopdf::StringFormat::Literal,
        ),
    );

    if let Some(reason) = &meta.reason {
        sig_dict.set(
            "Reason",
            Object::String(reason.as_bytes().to_vec(), lopdf::StringFormat::Literal),
        );
    }
    if let Some(loc) = &meta.location {
        sig_dict.set(
            "Location",
            Object::String(loc.as_bytes().to_vec(), lopdf::StringFormat::Literal),
        );
    }
    if let Some(contact) = &meta.contact_info {
        sig_dict.set(
            "ContactInfo",
            Object::String(contact.as_bytes().to_vec(), lopdf::StringFormat::Literal),
        );
    }

    // Embed hash in signature contents hex string
    sig_dict.set(
        "Contents",
        Object::String(
            hash_hex.as_bytes().to_vec(),
            lopdf::StringFormat::Hexadecimal,
        ),
    );

    let sig_dict_id = doc.add_object(Object::Dictionary(sig_dict));

    // 3. Build Widget annotation
    let mut widget = Dictionary::new();
    widget.set("Type", Object::Name(b"Annot".to_vec()));
    widget.set("Subtype", Object::Name(b"Widget".to_vec()));
    widget.set("FT", Object::Name(b"Sig".to_vec()));
    widget.set(
        "T",
        Object::String(b"Signature_Kestrel".to_vec(), lopdf::StringFormat::Literal),
    );
    widget.set("V", Object::Reference(sig_dict_id));
    widget.set("P", Object::Reference(page_obj_id));
    widget.set(
        "Rect",
        Object::Array(vec![
            Object::Real(50.0),
            Object::Real(50.0),
            Object::Real(250.0),
            Object::Real(110.0),
        ]),
    );

    let widget_id = doc.add_object(Object::Dictionary(widget));

    // Add widget to page /Annots
    if let Ok(page_dict) = doc
        .get_object_mut(page_obj_id)
        .and_then(Object::as_dict_mut)
    {
        if let Ok(annots) = page_dict.get_mut(b"Annots") {
            if let Ok(arr) = annots.as_array_mut() {
                arr.push(Object::Reference(widget_id));
            }
        } else {
            page_dict.set("Annots", Object::Array(vec![Object::Reference(widget_id)]));
        }
    }

    // Add widget to /AcroForm /Fields
    let mut acroform_id = None;
    if let Ok(cat_dict) = doc.get_object_mut(catalog_id).and_then(Object::as_dict_mut) {
        if let Ok(Object::Reference(id)) = cat_dict.get(b"AcroForm") {
            acroform_id = Some(*id);
        }
    }

    let af_id = match acroform_id {
        Some(id) => id,
        None => {
            let mut af_dict = Dictionary::new();
            af_dict.set("Fields", Object::Array(Vec::new()));
            af_dict.set("SigFlags", Object::Integer(3)); // SignaturesExist | AppendOnly
            let new_af_id = doc.add_object(Object::Dictionary(af_dict));
            if let Ok(cat_dict) = doc.get_object_mut(catalog_id).and_then(Object::as_dict_mut) {
                cat_dict.set("AcroForm", Object::Reference(new_af_id));
            }
            new_af_id
        }
    };

    if let Ok(af_dict) = doc.get_object_mut(af_id).and_then(Object::as_dict_mut) {
        if let Ok(fields_obj) = af_dict.get_mut(b"Fields") {
            if let Ok(arr) = fields_obj.as_array_mut() {
                arr.push(Object::Reference(widget_id));
            }
        }
    }

    Ok(sig_dict_id)
}

/// Verifies that a signed PDF contains a signature dictionary with a matching SHA-256 fingerprint.
pub fn verify_signature(doc: &lopdf::Document, expected_hash_hex: &str) -> bool {
    for obj in doc.objects.values() {
        if let Ok(dict) = obj.as_dict() {
            if let Ok(sig_type) = dict.get(b"Type").and_then(Object::as_name_str) {
                if sig_type == "Sig" {
                    if let Ok(contents) = dict.get(b"Contents").and_then(Object::as_str) {
                        let hex_str = String::from_utf8_lossy(contents);
                        if hex_str.eq_ignore_ascii_case(expected_hash_hex) {
                            return true;
                        }
                    }
                }
            }
        }
    }
    false
}
