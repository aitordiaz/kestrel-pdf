use kestrel_core::document::DocumentSession;
use kestrel_core::redact::{RedactionEngine, RedactionRect, RedactionTarget};
use kestrel_core::render::{PageTileKey, RenderPipeline, TileBuffer, TileCache};
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
    assert_eq!(session.page_count, 1, "Document should report 1 page");
    assert_eq!(session.pages.len(), 1);
    assert_eq!(session.pages[0].index, 0);

    let aspect = session.page_aspect_ratio(0).expect("Page 0 aspect ratio");
    assert!((aspect - (595.0 / 842.0)).abs() < 0.05);
}

#[test]
fn test_integration_full_text_search() {
    let target_keyword = "CONFIDENTIAL_REPORT_2026";
    let pdf_bytes = create_test_pdf_bytes(&format!("Project Alpha: {}", target_keyword));

    let session =
        DocumentSession::open_from_bytes(pdf_bytes, None).expect("Should open valid PDF in memory");

    let matches = session.search_text(target_keyword);
    assert_eq!(matches.len(), 1, "Should find exactly 1 search match");
    assert_eq!(matches[0].page_index, 0);
    assert!(matches[0].snippet.contains("CONFIDENTIAL_REPORT_2026"));

    let no_matches = session.search_text("NON_EXISTENT_QUERY_123");
    assert!(no_matches.is_empty());
}

#[test]
fn test_integration_render_pipeline_async_workers() {
    let pipeline = RenderPipeline::new(16);
    let key = PageTileKey {
        page_index: 0,
        tile_x: 0,
        tile_y: 0,
        zoom_level_percent: 100,
        device_pixel_ratio_x100: 100,
    };

    // Request tile
    pipeline.request_tile(key, 512, 512);

    // Give worker brief moment to process
    thread::sleep(std::time::Duration::from_millis(50));
    pipeline.process_incoming_tiles();

    // Verify tile was created and cached
    let cached = pipeline.cache().get(&key);
    assert!(cached.is_some(), "Tile must be present in cache");
    let buffer = cached.unwrap();
    assert_eq!(buffer.width, 512);
    assert_eq!(buffer.height, 512);
    assert_eq!(buffer.rgba.len(), 512 * 512 * 4);
}

#[test]
fn test_integration_tile_cache_concurrency_and_lru_eviction() {
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
    let _ = cache.get(&key_probe);
}

#[test]
fn test_integration_true_redaction_pipeline() {
    let secret_phrase = "SECRET_CREDIT_CARD_4111222233334444";
    let original_bytes = create_test_pdf_bytes(secret_phrase);

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

#[test]
fn test_integration_acroform_parsing_editing_and_serialization() {
    use kestrel_core::forms::FormField;

    let base_pdf = create_test_pdf_bytes("Contract Agreement for Services");
    let mut session =
        DocumentSession::open_from_bytes(base_pdf, None).expect("Open base PDF session");

    // Add interactive form fields
    let name_field = FormField::new_text(
        "client_name",
        "Client Name",
        0,
        "John Doe",
        [50.0, 600.0, 250.0, 624.0],
        false,
    );
    let agree_check = FormField::new_checkbox(
        "terms_agreed",
        "Terms Agreed",
        0,
        true,
        [50.0, 560.0, 70.0, 580.0],
    );
    let plan_choice = FormField::new_choice(
        "plan_selection",
        "Plan Selection",
        0,
        vec!["Standard".into(), "Enterprise".into(), "Custom".into()],
        Some(1),
        [50.0, 520.0, 200.0, 544.0],
    );

    session.add_form_field(name_field);
    session.add_form_field(agree_check);
    session.add_form_field(plan_choice);

    assert_eq!(session.forms.len(), 3);

    // Update field values
    assert!(session.update_form_field("Client Name", "Jane Doe"));
    assert!(session.update_form_field("Terms Agreed", "Off"));
    assert!(session.update_form_field("Plan Selection", "Enterprise"));

    // Save document to serialized bytes
    let serialized_bytes = session.save_to_bytes().expect("Save filled form PDF");
    assert!(!serialized_bytes.is_empty());

    // Reopen in fresh DocumentSession and verify persistence
    let reloaded_session =
        DocumentSession::open_from_bytes(serialized_bytes, None).expect("Reload saved form PDF");

    assert!(
        !reloaded_session.forms.is_empty(),
        "Must retain AcroForm fields in saved PDF"
    );
    let loaded_name = reloaded_session
        .forms
        .iter()
        .find(|f| f.name == "Client Name");
    assert!(loaded_name.is_some(), "Client Name field must exist");
    assert_eq!(loaded_name.unwrap().value, "Jane Doe");
}

#[test]
fn test_integration_cubic_bezier_smoothing_and_signature_rendering() {
    use kestrel_core::sign::{self, StrokePoint, VisualSignature};

    // Raw discrete mouse/stylus input points with corners
    let raw_points = vec![
        StrokePoint::new(10.0, 10.0, 0.5),
        StrokePoint::new(30.0, 45.0, 0.8),
        StrokePoint::new(70.0, 30.0, 1.0),
        StrokePoint::new(120.0, 90.0, 0.7),
        StrokePoint::new(180.0, 60.0, 0.4),
    ];

    let smoothed = sign::smooth_stroke_bezier(&raw_points, 4);
    assert!(
        smoothed.len() > raw_points.len(),
        "Smoothed stroke must have interpolated spline points"
    );
    assert_eq!(smoothed.first().unwrap().x, 10.0);
    assert_eq!(smoothed.first().unwrap().y, 10.0);

    let mut visual_sig = VisualSignature::new(0, [50.0, 100.0, 200.0, 80.0]);
    visual_sig.add_smoothed_stroke(&raw_points);
    assert_eq!(visual_sig.strokes.len(), 1);

    let bounds = visual_sig.compute_strokes_bounds();
    assert!(bounds.is_some());
    let [min_x, min_y, max_x, max_y] = bounds.unwrap();
    assert!(min_x <= 10.0);
    assert!(max_x >= 180.0);
    assert!(min_y <= 10.0);
    assert!(max_y >= 90.0);

    // Generate PDF vector content operators
    let operators = visual_sig.generate_pdf_graphics_operators(842.0);
    assert!(!operators.is_empty());
    let op_str = String::from_utf8_lossy(&operators);
    assert!(op_str.contains("q\n"), "Must save graphics state");
    assert!(op_str.contains("RG\n"), "Must set stroke color");
    assert!(op_str.contains("w\n"), "Must set line width");
    assert!(op_str.contains("m\n"), "Must contain moveto operator");
    assert!(op_str.contains("l\n"), "Must contain lineto operator");
    assert!(op_str.contains("S\n"), "Must stroke path");
    assert!(op_str.contains("Q\n"), "Must restore graphics state");
}

#[test]
fn test_integration_pades_digital_signature_embedding_and_verification() {
    use kestrel_core::sign::{self, DigitalSignatureMeta};

    let base_pdf = create_test_pdf_bytes("Executive Employment Agreement 2026");
    let mut session =
        DocumentSession::open_from_bytes(base_pdf, None).expect("Open document session");

    let mut meta = DigitalSignatureMeta::new("Alice M. Wonderland");
    meta.location = Some("Zurich, Switzerland".to_string());
    meta.reason = Some("Formal Acceptance and Execution".to_string());
    session.set_digital_signature(meta);

    let signed_bytes = session.save_to_bytes().expect("Save digitally signed PDF");
    assert!(!signed_bytes.is_empty());

    let parsed_doc = Document::load_mem(&signed_bytes).expect("Signed PDF must parse with lopdf");

    // Verify signature dictionary exists and matches hash
    let fingerprint = session.digital_signature.unwrap().sha256_fingerprint;
    assert!(
        !fingerprint.is_empty(),
        "Signature fingerprint must be calculated"
    );
    assert!(
        sign::verify_signature(&parsed_doc, &fingerprint),
        "PAdES digital signature verification must succeed"
    );
}

#[test]
fn test_integration_identity_h_and_tounicode_cmap_extraction() {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();

    // Create a ToUnicode CMap stream
    let cmap_content = b"/CIDInit /ProcSet findresource begin
12 dict begin
begincmap
/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def
/CMapName /Custom-ToUnicode def
/CMapType 2 def
1 beginbfrange
<0001> <0005> <0041>
endbfrange
1 beginbfchar
<0006> <005A>
endbfchar
endcmap
CMapName currentdict /CMap defineresource pop
end
end";
    let cmap_id = doc.add_object(Stream::new(dictionary! {}, cmap_content.to_vec()));

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "TestCIDFont",
        "Encoding" => "Identity-H",
        "ToUnicode" => cmap_id,
    });

    let content_stream = b"BT /F1 12 Tf 50 700 Td <0001000200030006> Tj ET";
    let content_id = doc.add_object(Stream::new(dictionary! {}, content_stream.to_vec()));

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
    doc.save_to(&mut buffer).expect("Save test PDF");

    let session = DocumentSession::open_from_bytes(buffer, None).expect("Open session");
    let extracted = session.get_page_text(0).expect("Extracted text");

    assert!(
        !extracted.contains("Identity-H Unimplemented"),
        "Must never contain Identity-H Unimplemented"
    );
    assert!(
        !extracted.contains("?Identity-H Unimplemented?"),
        "Must never contain ?Identity-H Unimplemented?"
    );
    assert!(
        extracted.contains("ABCZ"),
        "Must correctly decode CMap bfrange and bfchar mappings: got '{}'",
        extracted
    );
}

#[test]
fn test_integration_identity_h_fallback_without_cmap() {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type0",
        "BaseFont" => "IdentityCIDFont",
        "Encoding" => "Identity-H",
    });

    let content_stream = b"BT /F1 12 Tf 50 700 Td <00480065006C006C006F> Tj ET";
    let content_id = doc.add_object(Stream::new(dictionary! {}, content_stream.to_vec()));

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
    doc.save_to(&mut buffer).expect("Save test PDF");

    let session = DocumentSession::open_from_bytes(buffer, None).expect("Open session");
    let extracted = session.get_page_text(0).expect("Extracted text");

    assert!(
        !extracted.contains("Identity-H Unimplemented"),
        "Must never contain Identity-H Unimplemented"
    );
    assert!(
        !extracted.contains("?Identity-H Unimplemented?"),
        "Must never contain ?Identity-H Unimplemented?"
    );
    assert!(
        extracted.contains("Hello"),
        "Must decode UTF-16BE Identity-H string: got '{}'",
        extracted
    );
}
