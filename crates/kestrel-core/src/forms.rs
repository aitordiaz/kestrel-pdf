use anyhow::{Context, Result};
use lopdf::{Dictionary, Object, ObjectId};
use std::collections::HashMap;

/// Interactive AcroForm field types.
#[derive(Debug, Clone, PartialEq)]
pub enum FormFieldType {
    Text {
        multiline: bool,
        password: bool,
    },
    CheckBox {
        checked: bool,
    },
    RadioButton {
        selected: bool,
        group_name: String,
    },
    Choice {
        options: Vec<String>,
        selected: Option<usize>,
    },
    Signature,
}

/// Represents an interactive AcroForm widget within a document.
#[derive(Debug, Clone, PartialEq)]
pub struct FormField {
    pub id: String,
    pub name: String,
    pub page_index: u16,
    pub field_type: FormFieldType,
    pub value: String,
    /// Bounding rectangle in PDF points: [llx, lly, urx, ury]
    pub rect: [f32; 4],
    pub required: bool,
    pub read_only: bool,
}

impl FormField {
    /// Creates a new text input field.
    pub fn new_text(
        id: impl Into<String>,
        name: impl Into<String>,
        page_index: u16,
        value: impl Into<String>,
        rect: [f32; 4],
        multiline: bool,
    ) -> Self {
        let val_str = value.into();
        Self {
            id: id.into(),
            name: name.into(),
            page_index,
            field_type: FormFieldType::Text {
                multiline,
                password: false,
            },
            value: val_str,
            rect,
            required: false,
            read_only: false,
        }
    }

    /// Creates a new checkbox field.
    pub fn new_checkbox(
        id: impl Into<String>,
        name: impl Into<String>,
        page_index: u16,
        checked: bool,
        rect: [f32; 4],
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            page_index,
            field_type: FormFieldType::CheckBox { checked },
            value: if checked {
                "Yes".to_string()
            } else {
                "Off".to_string()
            },
            rect,
            required: false,
            read_only: false,
        }
    }

    /// Creates a new choice / dropdown selection field.
    pub fn new_choice(
        id: impl Into<String>,
        name: impl Into<String>,
        page_index: u16,
        options: Vec<String>,
        selected: Option<usize>,
        rect: [f32; 4],
    ) -> Self {
        let val = selected
            .and_then(|idx| options.get(idx))
            .cloned()
            .unwrap_or_default();

        Self {
            id: id.into(),
            name: name.into(),
            page_index,
            field_type: FormFieldType::Choice { options, selected },
            value: val,
            rect,
            required: false,
            read_only: false,
        }
    }

    /// Creates a new signature field widget.
    pub fn new_signature(
        id: impl Into<String>,
        name: impl Into<String>,
        page_index: u16,
        rect: [f32; 4],
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            page_index,
            field_type: FormFieldType::Signature,
            value: String::new(),
            rect,
            required: false,
            read_only: false,
        }
    }

    /// Sets the value of the field and updates internal type flags accordingly.
    pub fn set_value(&mut self, new_val: &str) {
        self.value = new_val.to_string();
        match &mut self.field_type {
            FormFieldType::CheckBox { checked } => {
                *checked = new_val.eq_ignore_ascii_case("yes")
                    || new_val.eq_ignore_ascii_case("true")
                    || new_val == "1"
                    || new_val == "On";
            }
            FormFieldType::Choice { options, selected } => {
                *selected = options.iter().position(|opt| opt == new_val);
            }
            FormFieldType::RadioButton { selected, .. } => {
                *selected = !new_val.is_empty() && new_val != "Off";
            }
            FormFieldType::Text { .. } | FormFieldType::Signature => {}
        }
    }
}

/// Helper function to parse a PDF rect array [llx, lly, urx, ury].
fn parse_rect(rect_obj: &Object) -> Option<[f32; 4]> {
    if let Ok(arr) = rect_obj.as_array() {
        if arr.len() >= 4 {
            let x0 = arr[0].as_float().ok()?;
            let y0 = arr[1].as_float().ok()?;
            let x1 = arr[2].as_float().ok()?;
            let y1 = arr[3].as_float().ok()?;
            return Some([x0, y0, x1, y1]);
        }
    }
    None
}

/// Extracts all interactive AcroForm fields present in a PDF document.
pub fn extract_form_fields(doc: &lopdf::Document) -> Vec<FormField> {
    let mut fields = Vec::new();

    // Map page object id to page index (0-based)
    let page_map: HashMap<ObjectId, u16> = doc
        .get_pages()
        .iter()
        .map(|(num, id)| (*id, (num.saturating_sub(1)) as u16))
        .collect();

    // 1. Locate AcroForm in Catalog
    let acroform_dict = match doc.catalog().and_then(|cat| cat.get(b"AcroForm")) {
        Ok(Object::Reference(ref_id)) => doc.get_object(*ref_id).and_then(Object::as_dict).ok(),
        Ok(Object::Dictionary(dict)) => Some(dict),
        _ => None,
    };

    if let Some(acroform) = acroform_dict {
        if let Ok(fields_arr) = acroform.get(b"Fields").and_then(Object::as_array) {
            for (idx, item) in fields_arr.iter().enumerate() {
                if let Some(field) = parse_field_object(doc, item, &page_map, idx) {
                    fields.push(field);
                }
            }
        }
    }

    // 2. Also inspect page annotations for any widget annotations not in AcroForm Fields array
    for (page_id, &page_index) in &page_map {
        if let Ok(page_obj) = doc.get_object(*page_id).and_then(Object::as_dict) {
            if let Ok(annots) = page_obj.get(b"Annots").and_then(Object::as_array) {
                for (a_idx, annot_ref) in annots.iter().enumerate() {
                    let annot_dict = match annot_ref {
                        Object::Reference(id) => doc.get_object(*id).and_then(Object::as_dict).ok(),
                        Object::Dictionary(d) => Some(d),
                        _ => None,
                    };

                    if let Some(dict) = annot_dict {
                        if let Ok(subtype) = dict.get(b"Subtype").and_then(Object::as_name_str) {
                            if subtype == "Widget" {
                                let name = dict
                                    .get(b"T")
                                    .and_then(Object::as_str)
                                    .map(|s| String::from_utf8_lossy(s).to_string())
                                    .unwrap_or_else(|_| format!("field_{}_{}", page_index, a_idx));

                                // Only add if not already captured
                                if !fields.iter().any(|f| f.name == name) {
                                    if let Some(field) = parse_widget_dict(dict, &name, page_index)
                                    {
                                        fields.push(field);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fields
}

fn parse_field_object(
    doc: &lopdf::Document,
    obj_ref: &Object,
    page_map: &HashMap<ObjectId, u16>,
    index: usize,
) -> Option<FormField> {
    let dict = match obj_ref {
        Object::Reference(id) => doc.get_object(*id).and_then(Object::as_dict).ok()?,
        Object::Dictionary(d) => d,
        _ => return None,
    };

    let name = dict
        .get(b"T")
        .and_then(Object::as_str)
        .map(|s| String::from_utf8_lossy(s).to_string())
        .unwrap_or_else(|_| format!("field_{}", index));

    let page_index = dict
        .get(b"P")
        .ok()
        .and_then(|p| p.as_reference().ok())
        .and_then(|p_id| page_map.get(&p_id).copied())
        .unwrap_or(0);

    parse_widget_dict(dict, &name, page_index)
}

#[allow(clippy::chunks_exact_to_as_chunks)]
fn decode_pdf_form_string(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xfe, 0xff]) {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u16s)
    } else if let Ok(s) = std::str::from_utf8(bytes) {
        s.to_string()
    } else {
        lopdf::Document::decode_text(None, bytes)
    }
}

fn parse_widget_dict(dict: &Dictionary, name: &str, page_index: u16) -> Option<FormField> {
    let ft = dict
        .get(b"FT")
        .and_then(Object::as_name_str)
        .unwrap_or("Tx");

    let rect = dict
        .get(b"Rect")
        .ok()
        .and_then(parse_rect)
        .unwrap_or([50.0, 50.0, 200.0, 74.0]);

    let val_str = dict
        .get(b"V")
        .map(|v| match v {
            Object::String(bytes, _) => decode_pdf_form_string(bytes),
            Object::Name(name_bytes) => String::from_utf8_lossy(name_bytes).to_string(),
            Object::Integer(i) => i.to_string(),
            Object::Real(r) => r.to_string(),
            _ => String::new(),
        })
        .unwrap_or_default();

    let flags = dict.get(b"Ff").and_then(Object::as_i64).unwrap_or(0);
    let read_only = (flags & 1) != 0;
    let required = (flags & 2) != 0;

    let field_type = match ft {
        "Tx" => {
            let multiline = (flags & (1 << 12)) != 0;
            let password = (flags & (1 << 13)) != 0;
            FormFieldType::Text {
                multiline,
                password,
            }
        }
        "Btn" => {
            let is_radio = (flags & (1 << 15)) != 0;
            if is_radio {
                FormFieldType::RadioButton {
                    selected: val_str != "Off" && !val_str.is_empty(),
                    group_name: name.to_string(),
                }
            } else {
                let checked = val_str == "Yes" || val_str == "On" || val_str == "true";
                FormFieldType::CheckBox { checked }
            }
        }
        "Ch" => {
            let mut options = Vec::new();
            if let Ok(opt_arr) = dict.get(b"Opt").and_then(Object::as_array) {
                for item in opt_arr {
                    if let Ok(opt_s) = item.as_str() {
                        options.push(String::from_utf8_lossy(opt_s).to_string());
                    } else if let Ok(inner_arr) = item.as_array() {
                        if let Some(last) = inner_arr.last().and_then(|o| o.as_str().ok()) {
                            options.push(String::from_utf8_lossy(last).to_string());
                        }
                    }
                }
            }
            let selected = options.iter().position(|o| o == &val_str);
            FormFieldType::Choice { options, selected }
        }
        "Sig" => FormFieldType::Signature,
        _ => FormFieldType::Text {
            multiline: false,
            password: false,
        },
    };

    Some(FormField {
        id: format!("{}_{}", name, page_index),
        name: name.to_string(),
        page_index,
        field_type,
        value: val_str,
        rect,
        required,
        read_only,
    })
}

/// Applies modified form field values to an existing `lopdf::Document`.
pub fn apply_form_fields(doc: &mut lopdf::Document, fields: &[FormField]) -> Result<()> {
    if fields.is_empty() {
        return Ok(());
    }

    // Locate AcroForm or create it if missing
    let catalog_id = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .context("Missing Root catalog reference in PDF trailer")?;

    let field_value_map: HashMap<&str, &FormField> =
        fields.iter().map(|f| (f.name.as_str(), f)).collect();

    let mut matched_names = std::collections::HashSet::new();

    // Iterate through document objects to update existing fields matching names
    let object_ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    for id in object_ids {
        if let Ok(dict) = doc.get_object_mut(id).and_then(Object::as_dict_mut) {
            if let Ok(t_bytes) = dict.get(b"T").and_then(Object::as_str) {
                let name = String::from_utf8_lossy(t_bytes).to_string();
                if let Some(field) = field_value_map.get(name.as_str()) {
                    matched_names.insert(name);
                    match &field.field_type {
                        FormFieldType::CheckBox { checked } => {
                            let state = if *checked { "Yes" } else { "Off" };
                            dict.set("V", Object::Name(state.as_bytes().to_vec()));
                            dict.set("AS", Object::Name(state.as_bytes().to_vec()));
                        }
                        FormFieldType::RadioButton { selected, .. } => {
                            let state = if *selected { "Yes" } else { "Off" };
                            dict.set("V", Object::Name(state.as_bytes().to_vec()));
                            dict.set("AS", Object::Name(state.as_bytes().to_vec()));
                        }
                        FormFieldType::Text { .. } | FormFieldType::Choice { .. } => {
                            dict.set(
                                "V",
                                Object::String(
                                    field.value.as_bytes().to_vec(),
                                    lopdf::StringFormat::Literal,
                                ),
                            );
                        }
                        FormFieldType::Signature => {}
                    }
                }
            }
        }
    }

    // For any field that was not already present in the document objects, add it
    for field in fields {
        if !matched_names.contains(&field.name) {
            add_form_field(doc, field.clone())?;
        }
    }

    // Ensure NeedAppearances is set so standard PDF viewers regenerate the field appearance
    let acroform_ref = {
        let cat_dict = doc.get_object_mut(catalog_id)?.as_dict_mut()?;
        if let Ok(acro) = cat_dict.get(b"AcroForm") {
            match acro {
                Object::Reference(id) => Some(*id),
                _ => None,
            }
        } else {
            None
        }
    };

    if let Some(af_id) = acroform_ref {
        if let Ok(af_dict) = doc.get_object_mut(af_id).and_then(Object::as_dict_mut) {
            af_dict.set("NeedAppearances", Object::Boolean(true));
        }
    }

    Ok(())
}

/// Adds a new form field directly to the document's AcroForm and target page annotations.
pub fn add_form_field(doc: &mut lopdf::Document, field: FormField) -> Result<()> {
    let pages = doc.get_pages();
    let page_num = (field.page_index + 1) as u32;
    let page_obj_id = *pages
        .get(&page_num)
        .context("Target page does not exist in document")?;

    let catalog_id = doc
        .trailer
        .get(b"Root")
        .and_then(Object::as_reference)
        .context("Missing Root catalog reference in PDF trailer")?;

    // Create the Field / Widget dictionary
    let mut widget = Dictionary::new();
    widget.set("Type", Object::Name(b"Annot".to_vec()));
    widget.set("Subtype", Object::Name(b"Widget".to_vec()));
    widget.set(
        "T",
        Object::String(field.name.as_bytes().to_vec(), lopdf::StringFormat::Literal),
    );
    widget.set(
        "Rect",
        Object::Array(vec![
            Object::Real(field.rect[0]),
            Object::Real(field.rect[1]),
            Object::Real(field.rect[2]),
            Object::Real(field.rect[3]),
        ]),
    );
    widget.set("P", Object::Reference(page_obj_id));

    let (ft, v_obj) = match &field.field_type {
        FormFieldType::Text { .. } => (
            "Tx",
            Object::String(
                field.value.as_bytes().to_vec(),
                lopdf::StringFormat::Literal,
            ),
        ),
        FormFieldType::CheckBox { checked } => (
            "Btn",
            Object::Name(if *checked {
                b"Yes".to_vec()
            } else {
                b"Off".to_vec()
            }),
        ),
        FormFieldType::RadioButton { selected, .. } => (
            "Btn",
            Object::Name(if *selected {
                b"Yes".to_vec()
            } else {
                b"Off".to_vec()
            }),
        ),
        FormFieldType::Choice { options, .. } => {
            let opt_arr = options
                .iter()
                .map(|opt| Object::String(opt.as_bytes().to_vec(), lopdf::StringFormat::Literal))
                .collect();
            widget.set("Opt", Object::Array(opt_arr));
            (
                "Ch",
                Object::String(
                    field.value.as_bytes().to_vec(),
                    lopdf::StringFormat::Literal,
                ),
            )
        }
        FormFieldType::Signature => ("Sig", Object::Null),
    };

    widget.set("FT", Object::Name(ft.as_bytes().to_vec()));
    widget.set("V", v_obj);

    // Add widget object to document
    let widget_id = doc.add_object(Object::Dictionary(widget));

    // Register widget in page's /Annots
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

    // Register widget in AcroForm /Fields
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
            af_dict.set("NeedAppearances", Object::Boolean(true));
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

    Ok(())
}
