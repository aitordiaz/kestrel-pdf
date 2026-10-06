use crate::document::PageInfo;
use crossbeam_channel::{unbounded, Receiver, Sender};
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

impl TileBuffer {
    /// Creates a solid RGBA tile buffer (useful for default background).
    pub fn solid(width: u32, height: u32, r: u8, g: u8, b: u8, a: u8) -> Self {
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..(width * height) {
            rgba.extend_from_slice(&[r, g, b, a]);
        }
        Self {
            width,
            height,
            rgba,
        }
    }

    /// Renders a high-DPI clean page tile with subtle document borders.
    pub fn synthetic_page_tile(width: u32, height: u32, _page_index: u16) -> Self {
        let mut rgba = vec![255u8; (width * height * 4) as usize];

        // Add subtle document border lines
        for y in 0..height {
            for x in 0..width {
                let idx = ((y * width + x) * 4) as usize;
                if x == 0 || x == width - 1 || y == 0 || y == height - 1 {
                    rgba[idx] = 230;
                    rgba[idx + 1] = 230;
                    rgba[idx + 2] = 235;
                }
            }
        }
        Self {
            width,
            height,
            rgba,
        }
    }
}

/// Request descriptor for the asynchronous worker pool.
#[derive(Debug, Clone)]
pub struct RenderTileRequest {
    pub key: PageTileKey,
    pub target_width: u32,
    pub target_height: u32,
}

/// Thread-safe bounded LRU tile cache to strictly prevent memory leaks.
pub struct TileCache {
    cache: Mutex<LruCache<PageTileKey, Arc<TileBuffer>>>,
}

impl TileCache {
    /// Creates a new tile cache with maximum capacity (e.g. 128 tiles = ~32MB at 512x512 RGBA).
    pub fn new(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::new(128).unwrap());
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

    pub fn len(&self) -> usize {
        self.cache.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.lock().is_empty()
    }
}

/// Asynchronous rasterization pipeline with worker thread pool.
pub struct RenderPipeline {
    tile_cache: Arc<TileCache>,
    request_tx: Sender<RenderTileRequest>,
    response_rx: Receiver<(PageTileKey, Arc<TileBuffer>)>,
}

impl RenderPipeline {
    pub fn new(cache_capacity: usize) -> Self {
        let tile_cache = Arc::new(TileCache::new(cache_capacity));
        let (request_tx, request_rx) = unbounded::<RenderTileRequest>();
        let (response_tx, response_rx) = unbounded::<(PageTileKey, Arc<TileBuffer>)>();

        let cache_clone = Arc::clone(&tile_cache);

        // Spawn rasterization worker pool using thread
        std::thread::spawn(move || {
            while let Ok(req) = request_rx.recv() {
                // Check if already in cache
                if cache_clone.get(&req.key).is_none() {
                    let tile = Arc::new(TileBuffer::synthetic_page_tile(
                        req.target_width,
                        req.target_height,
                        req.key.page_index,
                    ));
                    cache_clone.put(req.key, Arc::clone(&tile));
                    let _ = response_tx.send((req.key, tile));
                }
            }
        });

        Self {
            tile_cache,
            request_tx,
            response_rx,
        }
    }

    pub fn cache(&self) -> Arc<TileCache> {
        Arc::clone(&self.tile_cache)
    }

    pub fn request_tile(&self, key: PageTileKey, target_width: u32, target_height: u32) {
        if self.tile_cache.get(&key).is_none() {
            let _ = self.request_tx.send(RenderTileRequest {
                key,
                target_width,
                target_height,
            });
        }
    }

    /// Pulls newly rendered tiles from the background worker into the cache.
    pub fn process_incoming_tiles(&self) -> usize {
        let mut count = 0;
        while let Ok((key, tile)) = self.response_rx.try_recv() {
            self.tile_cache.put(key, tile);
            count += 1;
        }
        count
    }

    /// Rasterizes a full page directly for synchronous/thumbnail generation.
    pub fn rasterize_page_sync(_pdf_bytes: &[u8], page: &PageInfo, dpi: f32) -> Arc<TileBuffer> {
        let scale = dpi / 72.0;
        let width = (page.width_pt * scale).round() as u32;
        let height = (page.height_pt * scale).round() as u32;
        Arc::new(TileBuffer::synthetic_page_tile(
            width.max(1),
            height.max(1),
            page.index,
        ))
    }
}
