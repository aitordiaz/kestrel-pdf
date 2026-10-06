use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;
use std::sync::Arc;

/// Composite key representing a specific rendered tile of a PDF page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageTileKey {
    pub page_index: u16,
    pub tile_x: u16,
    pub tile_y: u16,
    pub zoom_level_percent: u16,
    pub device_pixel_ratio_x100: u16,
}

/// Raw RGBA8 pixel buffer for a rasterized tile.
#[derive(Clone)]
pub struct TileBuffer {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Request descriptor for the asynchronous worker pool.
#[derive(Debug, Clone)]
pub struct RenderTileRequest {
    pub key: PageTileKey,
    pub target_width: u32,
    pub target_height: u32,
}

/// Thread-safe bounded LRU tile cache to prevent unbounded memory growth.
pub struct TileCache {
    cache: Mutex<LruCache<PageTileKey, Arc<TileBuffer>>>,
}

impl TileCache {
    /// Creates a new tile cache with maximum capacity (e.g. 128 tiles = ~64MB at 512x512 RGBA).
    pub fn new(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(64).unwrap());
        Self {
            cache: Mutex::new(LruCache::new(cap)),
        }
    }

    pub fn get(&self, key: &PageTileKey) -> Option<Arc<TileBuffer>> {
        self.cache.lock().get(key).cloned()
    }

    pub fn put(&self, key: PageTileKey, tile: Arc<TileBuffer>) {
        self.cache.lock().put(key, tile);
    }

    pub fn clear(&self) {
        self.cache.lock().clear();
    }
}
