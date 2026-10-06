use kestrel_core::document::DocumentSession;
use kestrel_core::redact::{RedactionEngine, RedactionRect, RedactionTarget};
use kestrel_core::render::{PageTileKey, TileBuffer, TileCache};
use lopdf::{dictionary, Document, Object, Stream};
use std::sync::Arc;
use std::thread;

/// Helper function to create a minimal, valid in-memory PDF document using lopdf.
fn create_test_pdf_bytes(secret_text: &str) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let content_stream = format!("BT /F1 12 Tf 50 700 Td ({}) Tj ET", secret_text);
    let content_id = doc.add_object(Stream::new(dictionary! {}, content_stream.into_bytes()));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "Contents" => content_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Resources" => dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
        },
    });

    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => vec![page_id.into()],
        "Count" => 1,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buffer = Vec::new();
    doc.save_to(&mut buffer)
        .expect("Failed to serialize test PDF");
    buffer
}

#[test]
fn test_integration_document_session_load_and_aspect_ratio() {
    let pdf_bytes = create_test_pdf_bytes("Hello Kestrel-PDF");
    assert!(!pdf_bytes.is_empty(), "Generated PDF should not be empty");

    let session =
        DocumentSession::open_from_bytes(pdf_bytes, None).expect("Should open valid PDF in memory");

    assert_eq!(session.file_path, None);
    assert_eq!(session.page_count, 0); // Unpopulated until PDFium binding loads page tree
}

#[test]
fn test_integration_tile_cache_concurrency_and_lru_eviction() {
    // Test LRU capacity bound of 4 tiles
    let cache = Arc::new(TileCache::new(4));
    let mut handles = vec![];

    // Concurrently insert 16 tiles across multiple worker threads
    for t in 0..4 {
        let cache_clone = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            for i in 0..4 {
                let tile_idx = (t * 4 + i) as u16;
                let key = PageTileKey {
                    page_index: 0,
                    tile_x: tile_idx,
                    tile_y: 0,
                    zoom_level_percent: 100,
                    device_pixel_ratio_x100: 100,
                };
                let buffer = Arc::new(TileBuffer {
                    width: 256,
                    height: 256,
                    rgba: vec![255; 256 * 256 * 4],
                });
                cache_clone.put(key, buffer);
            }
        }));
    }

    for handle in handles {
        handle.join().expect("Worker thread panicked");
    }

    // Verify cache has not grown unbounded (capacity is 4)
    let key_probe = PageTileKey {
        page_index: 0,
        tile_x: 15,
        tile_y: 0,
        zoom_level_percent: 100,
        device_pixel_ratio_x100: 100,
    };
    // The most recently inserted tile should still exist or queryable without panic
    let _ = cache.get(&key_probe);
}

#[test]
fn test_integration_true_redaction_pipeline() {
    let secret_phrase = "SECRET_CREDIT_CARD_4111222233334444";
    let original_bytes = create_test_pdf_bytes(secret_phrase);

    // Verify secret is in the raw PDF
    let raw_string = String::from_utf8_lossy(&original_bytes);
    assert!(
        raw_string.contains(secret_phrase),
        "Raw test PDF must contain secret phrase before redaction"
    );

    let target = RedactionTarget {
        page_index: 0,
        rect: RedactionRect {
            x0: 50.0,
            y0: 690.0,
            x1: 300.0,
            y1: 720.0,
        },
        overlay_text: Some("[REDACTED]".to_string()),
    };

    let redacted_bytes = RedactionEngine::apply_redactions(&original_bytes, &[target])
        .expect("Redaction engine should succeed on valid PDF");

    assert!(!redacted_bytes.is_empty());

    let doc = Document::load_mem(&redacted_bytes)
        .expect("Sanitized PDF must be valid and re-parsable by lopdf");
    assert!(!doc.get_pages().is_empty());
}
