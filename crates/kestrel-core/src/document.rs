use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Geometry and basic information about a single PDF page.
#[derive(Debug, Clone)]
pub struct PageInfo {
    pub index: u16,
    pub width_pt: f32,
    pub height_pt: f32,
    pub rotation_degrees: u16,
}

/// Represents an active document session.
pub struct DocumentSession {
    pub file_path: Option<PathBuf>,
    pub page_count: u16,
    pub pages: Vec<PageInfo>,
}

impl DocumentSession {
    /// Opens a PDF document from a filesystem path.
    pub fn open_from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        let bytes = std::fs::read(&path_buf)
            .with_context(|| format!("Failed to read PDF file at {:?}", path_buf))?;
        Self::open_from_bytes(bytes, Some(path_buf))
    }

    /// Opens a PDF document from in-memory byte buffer (essential for WASM).
    pub fn open_from_bytes(_bytes: Vec<u8>, file_path: Option<PathBuf>) -> Result<Self> {
        // In Phase 1 implementation, binds to PDFium instance to query page tree
        Ok(Self {
            file_path,
            page_count: 0,
            pages: Vec::new(),
        })
    }

    /// Returns the aspect ratio of the specified page.
    pub fn page_aspect_ratio(&self, page_index: usize) -> Option<f32> {
        self.pages.get(page_index).map(|p| p.width_pt / p.height_pt)
    }
}
