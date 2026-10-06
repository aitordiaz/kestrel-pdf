//! # Kestrel Core
//!
//! Ultra-high-performance PDF processing engine, asynchronous tile rendering,
//! and structural AST manipulation for Kestrel-PDF.

pub mod document;
pub mod forms;
pub mod redact;
pub mod render;
pub mod sign;
pub mod synthetic;

pub use document::{
    decompress_pdf_stream, extract_page_layout, extract_page_text_robust,
    get_page_content_decompressed, map_pdf_point_to_visual, DocumentSession, OutlineItem, PageInfo,
    PageVisualLayout, PositionedText, SearchResult, VectorRect, VisualImage,
};
pub use render::{PageTileKey, RenderPipeline, RenderTileRequest, TileBuffer, TileCache};
