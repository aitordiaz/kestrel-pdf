//! # Kestrel Core
//!
//! Ultra-high-performance PDF processing engine, asynchronous tile rendering,
//! and structural AST manipulation for Kestrel-PDF.

pub mod document;
pub mod forms;
pub mod redact;
pub mod render;
pub mod sign;

pub use document::{DocumentSession, PageInfo};
pub use render::{PageTileKey, RenderTileRequest, TileBuffer, TileCache};
