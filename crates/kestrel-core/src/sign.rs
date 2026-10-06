/// 2D point for signature strokes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokePoint {
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
}

/// Visual signature consisting of smoothed ink strokes.
#[derive(Debug, Clone)]
pub struct VisualSignature {
    pub strokes: Vec<Vec<StrokePoint>>,
    pub target_page: u16,
    pub bounding_box: [f32; 4],
}

/// Cryptographic PAdES digital signature metadata.
#[derive(Debug, Clone)]
pub struct DigitalSignatureMeta {
    pub signer_name: String,
    pub contact_info: Option<String>,
    pub location: Option<String>,
    pub reason: Option<String>,
}
