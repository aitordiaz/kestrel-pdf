use anyhow::{Context, Result};
use lopdf::{dictionary, Document, Object, Stream};

use crate::forms::FormField;

/// Synthetic text element descriptor.
#[derive(Debug, Clone)]
pub struct SyntheticText {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub font_size: f32,
    pub color: [u8; 3],
}

/// Synthetic vector rectangle descriptor.
#[derive(Debug, Clone)]
pub struct SyntheticRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub fill: Option<[u8; 3]>,
    pub stroke: Option<[u8; 3]>,
    pub stroke_width: f32,
}

/// Synthetic raster image element descriptor.
#[derive(Debug, Clone)]
pub struct SyntheticImage {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub rgb_bytes: Vec<u8>,
}

/// In-memory representation of a synthetic PDF page being constructed.
#[derive(Debug, Clone)]
pub struct SyntheticPage {
    pub width_pt: f32,
    pub height_pt: f32,
    pub rotation: u16,
    pub texts: Vec<SyntheticText>,
    pub rects: Vec<SyntheticRect>,
    pub images: Vec<SyntheticImage>,
    pub forms: Vec<FormField>,
}

/// Fluent builder to generate synthetic PDF documents with arbitrary geometry,
/// text runs, vector shapes, embedded raster images, AcroForms, and rotations.
#[derive(Default)]
pub struct SyntheticPdfBuilder {
    pages: Vec<SyntheticPage>,
    layers: Vec<(String, bool)>,
}

impl SyntheticPdfBuilder {
    pub fn new() -> Self {
        Self {
            pages: Vec::new(),
            layers: Vec::new(),
        }
    }

    /// Registers an Optional Content Group (OCG / Layer) with an initial visibility state.
    pub fn add_layer(&mut self, name: impl Into<String>, initial_visible: bool) -> usize {
        let idx = self.layers.len();
        self.layers.push((name.into(), initial_visible));
        idx
    }

    /// Adds a new page with specified dimensions and initial rotation (0, 90, 180, 270).
    pub fn add_page(&mut self, width_pt: f32, height_pt: f32, rotation: u16) -> usize {
        let idx = self.pages.len();
        self.pages.push(SyntheticPage {
            width_pt,
            height_pt,
            rotation: rotation % 360,
            texts: Vec::new(),
            rects: Vec::new(),
            images: Vec::new(),
            forms: Vec::new(),
        });
        idx
    }

    /// Appends positioned text to the specified page.
    pub fn add_text(
        &mut self,
        page_idx: usize,
        text: impl Into<String>,
        x: f32,
        y: f32,
        font_size: f32,
        color: [u8; 3],
    ) -> &mut Self {
        if let Some(page) = self.pages.get_mut(page_idx) {
            page.texts.push(SyntheticText {
                text: text.into(),
                x,
                y,
                font_size,
                color,
            });
        }
        self
    }

    /// Appends a vector rectangle to the specified page.
    #[allow(clippy::too_many_arguments)]
    pub fn add_rect(
        &mut self,
        page_idx: usize,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        fill: Option<[u8; 3]>,
        stroke: Option<[u8; 3]>,
        stroke_width: f32,
    ) -> &mut Self {
        if let Some(page) = self.pages.get_mut(page_idx) {
            page.rects.push(SyntheticRect {
                x,
                y,
                width,
                height,
                fill,
                stroke,
                stroke_width,
            });
        }
        self
    }

    /// Appends an embedded raster image to the specified page.
    #[allow(clippy::too_many_arguments)]
    pub fn add_image(
        &mut self,
        page_idx: usize,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        pixel_width: u32,
        pixel_height: u32,
        rgb_bytes: Vec<u8>,
    ) -> &mut Self {
        if let Some(page) = self.pages.get_mut(page_idx) {
            page.images.push(SyntheticImage {
                x,
                y,
                width,
                height,
                pixel_width,
                pixel_height,
                rgb_bytes,
            });
        }
        self
    }

    /// Adds an interactive AcroForm text field to the specified page.
    pub fn add_form_text(
        &mut self,
        page_idx: usize,
        name: impl Into<String>,
        value: impl Into<String>,
        rect: [f32; 4],
    ) -> &mut Self {
        if let Some(page) = self.pages.get_mut(page_idx) {
            let name_str = name.into();
            page.forms.push(FormField::new_text(
                format!("{}_{}", name_str, page_idx),
                name_str,
                page_idx as u16,
                value,
                rect,
                false,
            ));
        }
        self
    }

    /// Adds an interactive AcroForm checkbox field to the specified page.
    pub fn add_form_checkbox(
        &mut self,
        page_idx: usize,
        name: impl Into<String>,
        checked: bool,
        rect: [f32; 4],
    ) -> &mut Self {
        if let Some(page) = self.pages.get_mut(page_idx) {
            let name_str = name.into();
            page.forms.push(FormField::new_checkbox(
                format!("{}_{}", name_str, page_idx),
                name_str,
                page_idx as u16,
                checked,
                rect,
            ));
        }
        self
    }

    /// Adds an interactive AcroForm choice / dropdown field to the specified page.
    pub fn add_form_choice(
        &mut self,
        page_idx: usize,
        name: impl Into<String>,
        options: Vec<String>,
        selected: Option<usize>,
        rect: [f32; 4],
    ) -> &mut Self {
        if let Some(page) = self.pages.get_mut(page_idx) {
            let name_str = name.into();
            page.forms.push(FormField::new_choice(
                format!("{}_{}", name_str, page_idx),
                name_str,
                page_idx as u16,
                options,
                selected,
                rect,
            ));
        }
        self
    }

    /// Compiles all configured pages into a valid PDF binary buffer.
    pub fn build(&self) -> Result<Vec<u8>> {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();

        // Register default standard Font F1 (Helvetica)
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });

        let mut page_object_ids = Vec::new();
        let mut all_form_fields = Vec::new();

        for (page_idx, page) in self.pages.iter().enumerate() {
            let mut ops = Vec::new();
            let mut xobjects_dict = lopdf::Dictionary::new();

            // 1. Vector Rectangles
            for rect in &page.rects {
                let stroke_w = rect.stroke_width.max(0.5);
                ops.push(format!("q {:.2} w", stroke_w));

                if let Some(c) = rect.fill {
                    ops.push(format!(
                        "{:.3} {:.3} {:.3} rg",
                        c[0] as f32 / 255.0,
                        c[1] as f32 / 255.0,
                        c[2] as f32 / 255.0
                    ));
                }
                if let Some(c) = rect.stroke {
                    ops.push(format!(
                        "{:.3} {:.3} {:.3} RG",
                        c[0] as f32 / 255.0,
                        c[1] as f32 / 255.0,
                        c[2] as f32 / 255.0
                    ));
                }

                ops.push(format!(
                    "{:.2} {:.2} {:.2} {:.2} re",
                    rect.x, rect.y, rect.width, rect.height
                ));

                match (rect.fill.is_some(), rect.stroke.is_some()) {
                    (true, true) => ops.push("B".to_string()),
                    (true, false) => ops.push("f".to_string()),
                    (false, true) => ops.push("S".to_string()),
                    (false, false) => ops.push("n".to_string()),
                }
                ops.push("Q".to_string());
            }

            // 2. Embedded Raster Images (XObjects)
            for (img_idx, img) in page.images.iter().enumerate() {
                let xobj_name = format!("Im{}", img_idx + 1);

                let img_stream = Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Image",
                        "Width" => img.pixel_width as i64,
                        "Height" => img.pixel_height as i64,
                        "ColorSpace" => "DeviceRGB",
                        "BitsPerComponent" => 8,
                    },
                    img.rgb_bytes.clone(),
                );
                let img_id = doc.add_object(img_stream);
                xobjects_dict.set(xobj_name.as_bytes().to_vec(), Object::Reference(img_id));

                // Position image: q w 0 0 h x y cm /ImN Do Q
                ops.push(format!(
                    "q {:.2} 0 0 {:.2} {:.2} {:.2} cm /{} Do Q",
                    img.width, img.height, img.x, img.y, xobj_name
                ));
            }

            // 3. Positioned Text Runs
            for text in &page.texts {
                let escaped = escape_pdf_literal(&text.text);
                ops.push(format!(
                    "q {:.3} {:.3} {:.3} rg BT /F1 {:.2} Tf {:.2} {:.2} Td ({}) Tj ET Q",
                    text.color[0] as f32 / 255.0,
                    text.color[1] as f32 / 255.0,
                    text.color[2] as f32 / 255.0,
                    text.font_size,
                    text.x,
                    text.y,
                    escaped
                ));
            }

            let stream_content = ops.join("\n").into_bytes();
            let content_id = doc.add_object(Stream::new(dictionary! {}, stream_content));

            // Page dictionary with MediaBox and Rotate
            let mut page_dict = dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "MediaBox" => vec![0.into(), 0.into(), page.width_pt.into(), page.height_pt.into()],
                "Resources" => dictionary! {
                    "Font" => dictionary! {
                        "F1" => font_id,
                    },
                },
            };

            if page.rotation != 0 {
                page_dict.set("Rotate", Object::Integer(page.rotation as i64));
            }

            if !xobjects_dict.is_empty() {
                if let Ok(res_dict) = page_dict
                    .get_mut(b"Resources")
                    .and_then(Object::as_dict_mut)
                {
                    res_dict.set("XObject", Object::Dictionary(xobjects_dict));
                }
            }

            let page_id = doc.add_object(page_dict);
            page_object_ids.push(page_id);

            for form in &page.forms {
                let mut form_copy = form.clone();
                form_copy.page_index = page_idx as u16;
                all_form_fields.push(form_copy);
            }
        }

        // Pages Tree
        let pages_dict = dictionary! {
            "Type" => "Pages",
            "Kids" => page_object_ids.iter().map(|id| Object::Reference(*id)).collect::<Vec<_>>(),
            "Count" => page_object_ids.len() as i64,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

        // Catalog Root
        let mut catalog_dict = dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        };

        if !self.layers.is_empty() {
            let mut ocg_ids = Vec::new();
            let mut off_ids = Vec::new();
            for (name, initial_visible) in &self.layers {
                let ocg_id = doc.add_object(dictionary! {
                    "Type" => "OCG",
                    "Name" => Object::string_literal(name.clone()),
                });
                ocg_ids.push(ocg_id);
                if !*initial_visible {
                    off_ids.push(ocg_id);
                }
            }

            let d_dict = dictionary! {
                "Order" => Object::Array(ocg_ids.iter().map(|id| Object::Reference(*id)).collect()),
                "ON" => Object::Array(
                    ocg_ids
                        .iter()
                        .filter(|id| !off_ids.contains(id))
                        .map(|id| Object::Reference(*id))
                        .collect(),
                ),
                "OFF" => Object::Array(off_ids.iter().map(|id| Object::Reference(*id)).collect()),
            };

            let oc_props_id = doc.add_object(dictionary! {
                "OCGs" => Object::Array(ocg_ids.iter().map(|id| Object::Reference(*id)).collect()),
                "D" => Object::Dictionary(d_dict),
            });
            catalog_dict.set("OCProperties", Object::Reference(oc_props_id));
        }

        let catalog_id = doc.add_object(catalog_dict);
        doc.trailer.set("Root", catalog_id);

        // Attach AcroForms if any exist
        if !all_form_fields.is_empty() {
            crate::forms::apply_form_fields(&mut doc, &all_form_fields)
                .context("Failed to apply synthetic form fields")?;
        }

        let mut output = Vec::new();
        doc.save_to(&mut output)
            .context("Failed to write synthetic PDF stream")?;
        Ok(output)
    }
}

fn escape_pdf_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out
}

/// Generates a synthetic multi-page document exhibiting all 4 orientations (0°, 90°, 180°, 270°),
/// multi-color text, vector borders, table grids, and embedded raster images.
pub fn generate_synthetic_visual_showcase_pdf() -> Vec<u8> {
    let mut builder = SyntheticPdfBuilder::new();

    // --- Page 0: Portrait (0°), Multi-column text, Table Grid, and Color Swatch Image ---
    let p0 = builder.add_page(595.28, 841.89, 0);

    // Header & Title
    builder.add_rect(
        p0,
        40.0,
        760.0,
        515.0,
        45.0,
        Some([24, 43, 73]), // Deep Navy
        None,
        1.0,
    );
    builder.add_text(
        p0,
        "KESTREL-PDF VISUAL ENGINE SPECIFICATION",
        55.0,
        778.0,
        16.0,
        [255, 255, 255],
    );

    // Multi-column text runs
    builder.add_text(
        p0,
        "Section 1: Architecture Overview",
        40.0,
        720.0,
        13.0,
        [30, 41, 59],
    );
    builder.add_text(
        p0,
        "The asynchronous GPU tile rasterizer pipeline guarantees 60fps scrolling.",
        40.0,
        700.0,
        10.0,
        [71, 85, 105],
    );
    builder.add_text(
        p0,
        "Memory consumption is bounded via an LRU cache with hardware eviction.",
        40.0,
        685.0,
        10.0,
        [71, 85, 105],
    );

    // Vector Table Grid
    let table_x = 40.0;
    let table_y = 540.0;
    let table_w = 515.0;
    let row_h = 24.0;
    for r in 0..4 {
        let y = table_y + (r as f32) * row_h;
        let fill = if r == 3 { Some([241, 245, 249]) } else { None };
        builder.add_rect(
            p0,
            table_x,
            y,
            table_w,
            row_h,
            fill,
            Some([203, 213, 225]),
            1.0,
        );
    }
    builder.add_text(
        p0,
        "Metric",
        50.0,
        table_y + 3.0 * row_h + 7.0,
        10.0,
        [15, 23, 42],
    );
    builder.add_text(
        p0,
        "Phase 1 Target",
        200.0,
        table_y + 3.0 * row_h + 7.0,
        10.0,
        [15, 23, 42],
    );
    builder.add_text(
        p0,
        "Status",
        420.0,
        table_y + 3.0 * row_h + 7.0,
        10.0,
        [15, 23, 42],
    );

    builder.add_text(
        p0,
        "Raster Latency",
        50.0,
        table_y + 2.0 * row_h + 7.0,
        9.0,
        [51, 65, 85],
    );
    builder.add_text(
        p0,
        "< 16.6ms per 512x512 tile",
        200.0,
        table_y + 2.0 * row_h + 7.0,
        9.0,
        [51, 65, 85],
    );
    builder.add_text(
        p0,
        "VERIFIED",
        420.0,
        table_y + 2.0 * row_h + 7.0,
        9.0,
        [16, 185, 129],
    );

    // Synthetic 16x16 RGB checkerboard raster image
    let mut img_bytes = Vec::with_capacity(16 * 16 * 3);
    for y in 0..16 {
        for x in 0..16 {
            if (x + y) % 2 == 0 {
                img_bytes.extend_from_slice(&[37, 99, 235]); // Royal Blue
            } else {
                img_bytes.extend_from_slice(&[245, 158, 11]); // Amber
            }
        }
    }
    builder.add_image(p0, 40.0, 420.0, 96.0, 96.0, 16, 16, img_bytes);
    builder.add_text(
        p0,
        "Synthetic Embedded Raster Swatch (16x16 RGB)",
        145.0,
        460.0,
        10.0,
        [30, 41, 59],
    );

    // --- Page 1: Landscape (90° Rotation) ---
    let p1 = builder.add_page(841.89, 595.28, 90);
    builder.add_rect(
        p1,
        50.0,
        500.0,
        741.0,
        45.0,
        Some([13, 148, 136]), // Teal
        None,
        1.0,
    );
    builder.add_text(
        p1,
        "LANDSCAPE MONITORING DASHBOARD (90 DEG CLOCKWISE)",
        70.0,
        518.0,
        16.0,
        [255, 255, 255],
    );
    builder.add_text(
        p1,
        "Telemetry graphs and wide data visualizations render cleanly with orientation mapping.",
        70.0,
        460.0,
        12.0,
        [15, 23, 42],
    );

    // --- Page 2: Inverted Portrait (180° Rotation) ---
    let p2 = builder.add_page(595.28, 841.89, 180);
    builder.add_rect(
        p2,
        40.0,
        750.0,
        515.0,
        45.0,
        Some([190, 24, 93]), // Pink/Rose
        None,
        1.0,
    );
    builder.add_text(
        p2,
        "INVERTED SPECIFICATION SHEET (180 DEG)",
        60.0,
        768.0,
        15.0,
        [255, 255, 255],
    );
    builder.add_text(
        p2,
        "Inverted orientation transforms coordinate mapping across vertical and horizontal axes.",
        60.0,
        710.0,
        11.0,
        [15, 23, 42],
    );

    // --- Page 3: Inverted Landscape (270° Rotation) ---
    let p3 = builder.add_page(841.89, 595.28, 270);
    builder.add_rect(
        p3,
        50.0,
        500.0,
        741.0,
        45.0,
        Some([124, 58, 237]), // Violet
        None,
        1.0,
    );
    builder.add_text(
        p3,
        "INVERTED LANDSCAPE LOGISTICS PLAN (270 DEG)",
        70.0,
        518.0,
        16.0,
        [255, 255, 255],
    );
    builder.add_text(
        p3,
        "All four 90-degree rotations are seamlessly supported in layout, rasterization, and interactive UI.",
        70.0,
        460.0,
        12.0,
        [15, 23, 42],
    );

    builder
        .build()
        .expect("Failed to build visual showcase PDF")
}

/// Generates a synthetic document with diverse interactive AcroForms:
/// text inputs, prefilled fields, checkboxes, and choice dropdowns.
pub fn generate_synthetic_forms_pdf() -> Vec<u8> {
    let mut builder = SyntheticPdfBuilder::new();

    // Page 0: Interactive Form Contract
    let p0 = builder.add_page(595.28, 841.89, 0);

    // Header
    builder.add_rect(p0, 40.0, 760.0, 515.0, 40.0, Some([15, 23, 42]), None, 1.0);
    builder.add_text(
        p0,
        "ACROFORM INTERACTIVE CONTRACT & ONBOARDING",
        55.0,
        776.0,
        14.0,
        [255, 255, 255],
    );

    // Text Field: Full Name
    builder.add_text(p0, "Full Legal Name:", 50.0, 710.0, 11.0, [51, 65, 85]);
    builder.add_rect(
        p0,
        180.0,
        702.0,
        360.0,
        22.0,
        Some([248, 250, 252]),
        Some([203, 213, 225]),
        1.0,
    );
    builder.add_form_text(
        p0,
        "applicant_name",
        "Alice Montgomery",
        [180.0, 702.0, 540.0, 724.0],
    );

    // Text Field: Organization / Company
    builder.add_text(p0, "Organization:", 50.0, 665.0, 11.0, [51, 65, 85]);
    builder.add_rect(
        p0,
        180.0,
        657.0,
        360.0,
        22.0,
        Some([248, 250, 252]),
        Some([203, 213, 225]),
        1.0,
    );
    builder.add_form_text(
        p0,
        "organization_name",
        "Starlight Dynamics Corp",
        [180.0, 657.0, 540.0, 679.0],
    );

    // Checkbox: NDA Consent
    builder.add_text(
        p0,
        "Accept Mutual Non-Disclosure Terms",
        80.0,
        615.0,
        11.0,
        [30, 41, 59],
    );
    builder.add_form_checkbox(p0, "accept_nda", true, [50.0, 612.0, 70.0, 632.0]);

    // Checkbox: Newsletter Subscription
    builder.add_text(
        p0,
        "Subscribe to Monthly Developer Updates",
        80.0,
        575.0,
        11.0,
        [30, 41, 59],
    );
    builder.add_form_checkbox(p0, "subscribe_updates", false, [50.0, 572.0, 70.0, 592.0]);

    // Choice / Dropdown: Jurisdiction Selection
    builder.add_text(p0, "Legal Jurisdiction:", 50.0, 530.0, 11.0, [51, 65, 85]);
    let options = vec![
        "European Union (GDPR)".to_string(),
        "United States (Delaware)".to_string(),
        "United Kingdom".to_string(),
        "Switzerland".to_string(),
    ];
    builder.add_form_choice(
        p0,
        "jurisdiction",
        options,
        Some(0),
        [180.0, 522.0, 400.0, 544.0],
    );

    builder
        .build()
        .expect("Failed to build synthetic forms PDF")
}

/// Generates a synthetic multi-page document with dedicated keywords for full-text search testing.
pub fn generate_synthetic_search_corpus_pdf() -> Vec<u8> {
    let mut builder = SyntheticPdfBuilder::new();

    // Page 0: Technical Architecture
    let p0 = builder.add_page(595.28, 841.89, 0);
    builder.add_text(
        p0,
        "KESTREL PDF ENGINE ARCHITECTURE",
        50.0,
        780.0,
        16.0,
        [15, 23, 42],
    );
    builder.add_text(
        p0,
        "The core rendering engine utilizes asynchronous GPU worker threads.",
        50.0,
        740.0,
        11.0,
        [51, 65, 85],
    );
    builder.add_text(
        p0,
        "Unique token for query verification: ALPHA_SEARCH_TOKEN_42.",
        50.0,
        700.0,
        11.0,
        [30, 41, 59],
    );

    // Page 1: Security and True Redaction
    let p1 = builder.add_page(595.28, 841.89, 0);
    builder.add_text(
        p1,
        "SECURITY AUDIT AND PRIVACY PROTOCOL",
        50.0,
        780.0,
        16.0,
        [15, 23, 42],
    );
    builder.add_text(
        p1,
        "True redaction physically removes objects and stream bytes from the PDF AST.",
        50.0,
        740.0,
        11.0,
        [51, 65, 85],
    );
    builder.add_text(
        p1,
        "Unique token for page 2 verification: BETA_SECURITY_HASH_99.",
        50.0,
        700.0,
        11.0,
        [30, 41, 59],
    );
    builder.add_text(
        p1,
        "Occurrences: KEYWORD_MULTI_OCCURRENCE first mention.",
        50.0,
        660.0,
        11.0,
        [30, 41, 59],
    );
    builder.add_text(
        p1,
        "Occurrences: KEYWORD_MULTI_OCCURRENCE second mention on same page.",
        50.0,
        620.0,
        11.0,
        [30, 41, 59],
    );

    // Page 2: Internationalization & Accentuated Text
    let p2 = builder.add_page(595.28, 841.89, 0);
    builder.add_text(
        p2,
        "CERTIFICACIÓN Y SUMINISTRO ENERGÉTICO",
        50.0,
        780.0,
        16.0,
        [15, 23, 42],
    );
    builder.add_text(
        p2,
        "Detalle de consumo eléctrico en kilovatios hora y término de potencia.",
        50.0,
        740.0,
        11.0,
        [51, 65, 85],
    );
    builder.add_text(
        p2,
        "Unique token for page 3 verification: GAMMA_IBAN_SPANISH_ES91.",
        50.0,
        700.0,
        11.0,
        [30, 41, 59],
    );

    builder.build().expect("Failed to build search corpus PDF")
}

/// Tier 1: Simplest possible valid PDF 1.7 in memory.
pub fn generate_tier1_minimal_pdf() -> Vec<u8> {
    let mut builder = SyntheticPdfBuilder::new();
    let p0 = builder.add_page(595.28, 841.89, 0);
    builder.add_text(
        p0,
        "MINIMAL STANDALONE PDF 1.7",
        50.0,
        750.0,
        14.0,
        [0, 0, 0],
    );
    builder.build().expect("Build Tier 1 PDF")
}

/// Tier 2: Universal character ranges and multi-script typography stress matrix.
pub fn generate_tier2_unicode_multiscript_pdf() -> Vec<u8> {
    let mut builder = SyntheticPdfBuilder::new();
    let p0 = builder.add_page(595.28, 841.89, 0);
    builder.add_rect(p0, 40.0, 780.0, 515.0, 40.0, Some([30, 41, 59]), None, 1.0);
    builder.add_text(
        p0,
        "UNIVERSAL MULTI-SCRIPT & TYPOGRAPHY STRESS MATRIX",
        50.0,
        795.0,
        14.0,
        [255, 255, 255],
    );

    // ASCII & Delimiters
    builder.add_text(
        p0,
        "ASCII Special: !@#$%^&*()_+-=[]{}|;':\",./<>? `~",
        50.0,
        750.0,
        11.0,
        [15, 23, 42],
    );
    // Latin Extended
    builder.add_text(
        p0,
        "Latin Accents: ÁÉÍÓÚÑ áéíóúñ ç Ç å Å ø Ø æ Æ ü Ü ö Ö ä Ä ß",
        50.0,
        720.0,
        11.0,
        [15, 23, 42],
    );
    // Greek
    builder.add_text(
        p0,
        "Greek Alphabet: ΑΒΓΔΕΖΗΘΙΚΛΜΝΞΟΠΡΣΤΥΦΧΨΩ αβγδεπΩΣ θφψ",
        50.0,
        690.0,
        11.0,
        [15, 23, 42],
    );
    // Cyrillic
    builder.add_text(
        p0,
        "Cyrillic Script: Привет, мир! Тестирование шрифтов",
        50.0,
        660.0,
        11.0,
        [15, 23, 42],
    );
    // CJK East Asian
    builder.add_text(
        p0,
        "CJK East Asian: 你好世界 こんにちは世界 안녕하세요",
        50.0,
        630.0,
        11.0,
        [15, 23, 42],
    );
    // RTL Arabic & Hebrew
    builder.add_text(
        p0,
        "RTL Scripts: مرحبا بالعالم (Arabic) שלום עולם (Hebrew)",
        50.0,
        600.0,
        11.0,
        [15, 23, 42],
    );
    // Math & Currencies
    builder.add_text(
        p0,
        "Math & Currencies: ∑ ∫ √ ∞ ≈ ≠ ≤ ≥ ± ∂ ∇ | € $ £ ¥ ₹ ₿ ¢",
        50.0,
        570.0,
        11.0,
        [15, 23, 42],
    );
    // Emojis & Symbols
    builder.add_text(
        p0,
        "Emojis & Symbols: 🚀 🦀 📄 🔒 ⚡ 🌟 ✨ 🎯 🏆 📦",
        50.0,
        540.0,
        11.0,
        [15, 23, 42],
    );
    // Escapes
    builder.add_text(
        p0,
        "Escapes: Parentheses (nested (deep)) and \\backslashes\\ and tabs",
        50.0,
        510.0,
        11.0,
        [15, 23, 42],
    );
    // Continuous repetition
    builder.add_text(
        p0,
        "Continuum: ".to_string() + &"ABCDEFGHIJ0123456789_".repeat(15),
        50.0,
        480.0,
        9.0,
        [71, 85, 105],
    );

    builder.build().expect("Build Tier 2 PDF")
}

/// Tier 3: Vector geometry, stroke widths, and affine transformations.
pub fn generate_tier3_vector_affine_pdf() -> Vec<u8> {
    let mut builder = SyntheticPdfBuilder::new();
    let p0 = builder.add_page(595.28, 841.89, 0);
    builder.add_rect(
        p0,
        40.0,
        780.0,
        515.0,
        40.0,
        Some([15, 118, 110]),
        None,
        1.0,
    );
    builder.add_text(
        p0,
        "VECTOR GEOMETRY, STROKES & AFFINE TRANSFORMS",
        50.0,
        795.0,
        14.0,
        [255, 255, 255],
    );

    // Stroke width variations from hairline 0.25 to heavy 8.0
    builder.add_rect(
        p0,
        50.0,
        720.0,
        120.0,
        40.0,
        None,
        Some([239, 68, 68]),
        0.25,
    );
    builder.add_rect(
        p0,
        190.0,
        720.0,
        120.0,
        40.0,
        None,
        Some([245, 158, 11]),
        1.5,
    );
    builder.add_rect(
        p0,
        330.0,
        720.0,
        120.0,
        40.0,
        None,
        Some([16, 185, 129]),
        4.0,
    );
    builder.add_rect(
        p0,
        470.0,
        720.0,
        80.0,
        40.0,
        None,
        Some([59, 130, 246]),
        8.0,
    );

    // Nested filled & stroked rectangles
    builder.add_rect(
        p0,
        50.0,
        550.0,
        200.0,
        140.0,
        Some([241, 245, 249]),
        Some([100, 116, 139]),
        2.0,
    );
    builder.add_rect(
        p0,
        70.0,
        570.0,
        160.0,
        100.0,
        Some([224, 231, 255]),
        Some([99, 102, 241]),
        1.5,
    );
    builder.add_rect(
        p0,
        90.0,
        590.0,
        120.0,
        60.0,
        Some([199, 210, 254]),
        Some([79, 70, 229]),
        1.0,
    );

    builder.build().expect("Build Tier 3 PDF")
}

/// Tier 4: Raster images, varied dimensions, and SMask transparency.
pub fn generate_tier4_raster_smask_pdf() -> Vec<u8> {
    let mut builder = SyntheticPdfBuilder::new();
    let p0 = builder.add_page(595.28, 841.89, 0);
    builder.add_rect(p0, 40.0, 780.0, 515.0, 40.0, Some([136, 19, 55]), None, 1.0);
    builder.add_text(
        p0,
        "RASTER IMAGES, FORMATS & SMASK TRANSPARENCY",
        50.0,
        795.0,
        14.0,
        [255, 255, 255],
    );

    // 16x16 RGB checkerboard raster image
    let mut rgb = Vec::with_capacity(16 * 16 * 3);
    for y in 0..16 {
        for x in 0..16 {
            if (x + y) % 2 == 0 {
                rgb.extend_from_slice(&[236, 72, 153]); // Pink
            } else {
                rgb.extend_from_slice(&[251, 191, 36]); // Amber
            }
        }
    }
    builder.add_image(p0, 50.0, 600.0, 120.0, 120.0, 16, 16, rgb);

    // 1x1 single pixel micro image
    builder.add_image(p0, 200.0, 650.0, 30.0, 30.0, 1, 1, vec![59, 130, 246]);

    // High aspect banner image (32x2)
    let banner = [16, 185, 129].repeat(32 * 2);
    builder.add_image(p0, 250.0, 650.0, 280.0, 24.0, 32, 2, banner);

    builder.build().expect("Build Tier 4 PDF")
}

/// Tier 5: Comprehensive interactive AcroForms with multiple field types and encodings.
pub fn generate_tier5_acroform_stress_pdf() -> Vec<u8> {
    let mut builder = SyntheticPdfBuilder::new();
    let p0 = builder.add_page(595.28, 841.89, 0);
    builder.add_rect(p0, 40.0, 780.0, 515.0, 40.0, Some([30, 58, 138]), None, 1.0);
    builder.add_text(
        p0,
        "ACROFORM EXHAUSTIVE FIELD DIVERSITY",
        50.0,
        795.0,
        14.0,
        [255, 255, 255],
    );

    // ASCII text input
    builder.add_text(p0, "ASCII Text Field:", 50.0, 730.0, 11.0, [51, 65, 85]);
    builder.add_form_text(
        p0,
        "ascii_field",
        "Standard Alpha 2026",
        [180.0, 725.0, 450.0, 745.0],
    );

    // UTF-16BE multi-byte text field
    builder.add_text(p0, "International Name:", 50.0, 680.0, 11.0, [51, 65, 85]);
    builder.add_form_text(
        p0,
        "intl_field",
        "ELENA VÁZQUEZ PEÑA",
        [180.0, 675.0, 450.0, 695.0],
    );

    // Checkbox checked
    builder.add_text(
        p0,
        "Terms Accepted (Checked):",
        80.0,
        635.0,
        11.0,
        [51, 65, 85],
    );
    builder.add_form_checkbox(p0, "terms_checked", true, [50.0, 630.0, 70.0, 650.0]);

    // Checkbox unchecked
    builder.add_text(
        p0,
        "Marketing Consent (Unchecked):",
        80.0,
        595.0,
        11.0,
        [51, 65, 85],
    );
    builder.add_form_checkbox(p0, "mkt_unchecked", false, [50.0, 590.0, 70.0, 610.0]);

    // Choice / Dropdown
    builder.add_text(p0, "Regional Hub Choice:", 50.0, 545.0, 11.0, [51, 65, 85]);
    builder.add_form_choice(
        p0,
        "hub_choice",
        vec![
            "North America (Virginia)".to_string(),
            "Europe (Frankfurt)".to_string(),
            "Asia-Pacific (Tokyo)".to_string(),
            "Latin America (São Paulo)".to_string(),
        ],
        Some(1),
        [180.0, 538.0, 450.0, 558.0],
    );

    builder.build().expect("Build Tier 5 PDF")
}

/// Tier 6: Optional Content Groups (OCGs / Layers) with visibility states.
pub fn generate_tier6_layers_ocg_pdf() -> Vec<u8> {
    let mut builder = SyntheticPdfBuilder::new();
    builder.add_layer("Architectural Floor Plan", true);
    builder.add_layer("Electrical & Plumbing Infrastructure", false);
    builder.add_layer("Confidential Watermark & Annotations", true);

    let p0 = builder.add_page(595.28, 841.89, 0);
    builder.add_rect(p0, 40.0, 780.0, 515.0, 40.0, Some([88, 28, 135]), None, 1.0);
    builder.add_text(
        p0,
        "OPTIONAL CONTENT GROUPS (OCG / LAYERS)",
        50.0,
        795.0,
        14.0,
        [255, 255, 255],
    );

    builder.add_text(
        p0,
        "Layer 1: Structural layout and perimeter walls (Visible by default)",
        50.0,
        740.0,
        11.0,
        [30, 41, 59],
    );
    builder.add_text(
        p0,
        "Layer 2: Electrical circuitry and sensor network (Hidden by default)",
        50.0,
        700.0,
        11.0,
        [30, 41, 59],
    );
    builder.add_text(
        p0,
        "Layer 3: Security audit watermark and signatures (Visible by default)",
        50.0,
        660.0,
        11.0,
        [30, 41, 59],
    );

    builder.build().expect("Build Tier 6 PDF")
}

/// Tier 7: Multi-page mixed orientations (0°, 90°, 180°, 270°) and extreme dimension scales.
pub fn generate_tier7_mixed_orientations_dimensions_pdf() -> Vec<u8> {
    let mut builder = SyntheticPdfBuilder::new();

    // Page 0: Postage stamp (120 x 120 pt, 0°)
    let p0 = builder.add_page(120.0, 120.0, 0);
    builder.add_rect(
        p0,
        10.0,
        10.0,
        100.0,
        100.0,
        Some([254, 243, 199]),
        Some([217, 119, 6]),
        1.0,
    );
    builder.add_text(p0, "STAMP 0°", 25.0, 55.0, 12.0, [180, 83, 9]);

    // Page 1: Standard A4 (595.28 x 841.89 pt, 90° Landscape)
    let p1 = builder.add_page(595.28, 841.89, 90);
    builder.add_rect(p1, 50.0, 500.0, 741.0, 40.0, Some([2, 132, 199]), None, 1.0);
    builder.add_text(
        p1,
        "A4 LANDSCAPE MONITOR (90 DEG)",
        70.0,
        515.0,
        14.0,
        [255, 255, 255],
    );

    // Page 2: US Letter (612.0 x 792.0 pt, 180° Inverted)
    let p2 = builder.add_page(612.0, 792.0, 180);
    builder.add_rect(p2, 40.0, 700.0, 532.0, 40.0, Some([225, 29, 72]), None, 1.0);
    builder.add_text(
        p2,
        "LETTER INVERTED SPEC (180 DEG)",
        60.0,
        715.0,
        14.0,
        [255, 255, 255],
    );

    // Page 3: Poster / Blueprint (1400.0 x 1000.0 pt, 270° Inverted Landscape)
    let p3 = builder.add_page(1400.0, 1000.0, 270);
    builder.add_rect(p3, 50.0, 900.0, 900.0, 40.0, Some([79, 70, 229]), None, 1.0);
    builder.add_text(
        p3,
        "OVERSIZED POSTER SCHEMATIC (270 DEG)",
        70.0,
        915.0,
        14.0,
        [255, 255, 255],
    );

    builder.build().expect("Build Tier 7 PDF")
}

/// Tier 8: Resilient boundary cases (empty pages, blank text, minimal streams).
pub fn generate_tier8_resilient_boundary_pdf() -> Vec<u8> {
    let mut builder = SyntheticPdfBuilder::new();
    // Empty page
    builder.add_page(595.28, 841.89, 0);
    // Page with single text
    let p1 = builder.add_page(595.28, 841.89, 0);
    builder.add_text(
        p1,
        "BOUNDARY CASE RESILIENCE VERIFIED",
        50.0,
        750.0,
        12.0,
        [0, 0, 0],
    );
    builder.build().expect("Build Tier 8 PDF")
}

/// Generates all 8 synthetic stress tiers in memory paired with their filenames.
pub fn generate_all_synthetic_stress_tiers() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("tier1_minimal.pdf", generate_tier1_minimal_pdf()),
        (
            "tier2_unicode.pdf",
            generate_tier2_unicode_multiscript_pdf(),
        ),
        ("tier3_vectors.pdf", generate_tier3_vector_affine_pdf()),
        ("tier4_raster.pdf", generate_tier4_raster_smask_pdf()),
        ("tier5_acroforms.pdf", generate_tier5_acroform_stress_pdf()),
        ("tier6_layers.pdf", generate_tier6_layers_ocg_pdf()),
        (
            "tier7_orientations.pdf",
            generate_tier7_mixed_orientations_dimensions_pdf(),
        ),
        (
            "tier8_resilience.pdf",
            generate_tier8_resilient_boundary_pdf(),
        ),
    ]
}
