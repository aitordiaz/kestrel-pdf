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
#[derive(Debug, Clone)]
pub struct FormField {
    pub id: String,
    pub name: String,
    pub page_index: u16,
    pub field_type: FormFieldType,
    pub value: String,
    pub rect: [f32; 4],
}
