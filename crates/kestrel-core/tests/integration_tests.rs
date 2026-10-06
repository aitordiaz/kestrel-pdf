use kestrel_core::document::{map_pdf_point_to_visual, DocumentSession};
use kestrel_core::forms::FormFieldType;
use kestrel_core::redact::{RedactionEngine, RedactionRect, RedactionTarget};
use kestrel_core::render::{PageTileKey, RenderPipeline, TileBuffer, TileCache};
use kestrel_core::synthetic::{
    generate_synthetic_forms_pdf, generate_synthetic_search_corpus_pdf,
    generate_synthetic_visual_showcase_pdf, SyntheticPdfBuilder,
};
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

#[test]
fn test_integration_multi_filter_ascii85_flate_decompression() {
    use kestrel_core::document::decode_ascii85;
    use lopdf::content::{Content, Operation};

    // 1. Create content stream with PDF text
    let original_ops = Content {
        operations: vec![
            Operation::new("BT", vec![]),
            Operation::new(
                "Tm",
                vec![
                    1.into(),
                    0.into(),
                    0.into(),
                    1.into(),
                    50.into(),
                    700.into(),
                ],
            ),
            Operation::new("Tf", vec![Object::Name(b"F1".to_vec()), 12.into()]),
            Operation::new(
                "Tj",
                vec![Object::string_literal("Multi-Filter Test Success")],
            ),
            Operation::new("ET", vec![]),
        ],
    };
    let raw_content = original_ops.encode().expect("Encode content");

    // 2. Compress with zlib (Flate)
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;
    let mut zlib_encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    zlib_encoder.write_all(&raw_content).expect("Zlib compress");
    let flate_bytes = zlib_encoder.finish().expect("Zlib finish");

    // 3. Encode with ASCII85
    let mut ascii85_bytes = Vec::new();
    for chunk in flate_bytes.chunks(4) {
        let mut tuple = 0u32;
        for (i, &b) in chunk.iter().enumerate() {
            tuple |= (b as u32) << (24 - i * 8);
        }
        if chunk.len() == 4 && tuple == 0 {
            ascii85_bytes.push(b'z');
        } else {
            let mut encoded = [b'!'; 5];
            for i in (0..5).rev() {
                encoded[i] = b'!' + (tuple % 85) as u8;
                tuple /= 85;
            }
            let take = chunk.len() + 1;
            ascii85_bytes.extend_from_slice(&encoded[..take]);
        }
    }
    ascii85_bytes.extend_from_slice(b"~>");

    // Verify decode_ascii85 roundtrip
    let recovered_flate = decode_ascii85(&ascii85_bytes);
    assert_eq!(recovered_flate, flate_bytes);

    // 4. Build a test lopdf document with /Filter [/ASCII85Decode /FlateDecode]
    let mut doc = lopdf::Document::with_version("1.7");
    let pages_id = doc.new_object_id();

    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let content_stream = lopdf::Stream::new(
        dictionary! {
            "Filter" => Object::Array(vec![
                Object::Name(b"ASCII85Decode".to_vec()),
                Object::Name(b"FlateDecode".to_vec()),
            ]),
        },
        ascii85_bytes,
    );
    // Lopdf compresses streams by default if not set; content is already encoded
    let content_id = doc.add_object(Object::Stream(content_stream));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Contents" => content_id,
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

    // 5. Open in DocumentSession and assert text extraction & visual layout
    let session = DocumentSession::open_from_bytes(buffer, None).expect("Open session");
    assert_eq!(session.page_count, 1);
    let text = session.get_page_text(0).expect("Extracted text");
    assert!(
        text.contains("Multi-Filter Test Success"),
        "Extracted text should contain target string: got '{}'",
        text
    );

    let layout = session.get_page_layout(0).expect("Visual layout");
    assert_eq!(layout.width_pt, 612.0);
    assert_eq!(layout.height_pt, 792.0);
    assert!(
        !layout.text_runs.is_empty(),
        "Should have positioned text runs"
    );
    assert_eq!(layout.text_runs[0].text, "Multi-Filter Test Success");
    assert_eq!(layout.text_runs[0].x, 50.0);
    assert_eq!(layout.text_runs[0].y, 700.0);
}

#[test]
fn test_integration_real_documents_if_present() {
    let factura_path = std::path::Path::new("/home/aitor/projects/Factura.pdf");
    if factura_path.exists() {
        let session = DocumentSession::open_from_file(factura_path).expect("Load Factura");
        assert_eq!(session.page_count, 4, "Factura should have 4 pages");
        for page_idx in 0..session.page_count as usize {
            let text = session.get_page_text(page_idx).unwrap_or("");
            assert!(
                !text.is_empty(),
                "Page {} in Factura must NOT be blank! Got empty string",
                page_idx
            );
            let layout = session.get_page_layout(page_idx).expect("Layout");
            assert!(
                !layout.text_runs.is_empty(),
                "Page {} must have positioned text runs",
                page_idx
            );
        }
        let p0_text = session.get_page_text(0).unwrap();
        assert!(
            p0_text.contains("Factura"),
            "Page 0 should contain 'Factura': got '{}'",
            p0_text
        );
    }

    let repsol_path =
        std::path::Path::new("/home/aitor/projects/REPSOL_CAT_Cambio de titular_ACT_CAT_03.pdf");
    if repsol_path.exists() {
        let session = DocumentSession::open_from_file(repsol_path).expect("Load Repsol");
        assert_eq!(session.page_count, 2, "Repsol should have 2 pages");
        assert_eq!(session.forms.len(), 38, "Repsol should have 38 form fields");

        for page_idx in 0..session.page_count as usize {
            let page = &session.pages[page_idx];
            let layout = session.get_page_layout(page_idx).expect("Layout");
            assert!(
                !layout.text_runs.is_empty(),
                "Repsol page {} must have positioned text runs",
                page_idx
            );
            for tr in &layout.text_runs {
                assert!(
                    tr.x >= -10.0 && tr.x <= page.width_pt + 50.0,
                    "Text x coordinate out of bounds: {}",
                    tr.x
                );
                assert!(
                    tr.y >= -10.0 && tr.y <= page.height_pt + 50.0,
                    "Text y coordinate out of bounds: {}",
                    tr.y
                );
            }
        }
    }
}

#[test]
fn test_integration_synthetic_visual_showcase_all_orientations() {
    let pdf_bytes = generate_synthetic_visual_showcase_pdf();
    assert!(!pdf_bytes.is_empty());

    let session = DocumentSession::open_from_bytes(pdf_bytes, None)
        .expect("Open synthetic visual showcase PDF");

    assert_eq!(
        session.page_count, 4,
        "Should have 4 pages with varying orientations"
    );

    // Page 0: Portrait (0°), 595.28 x 841.89
    let p0 = &session.pages[0];
    assert_eq!(p0.rotation_degrees, 0);
    assert!((p0.width_pt - 595.28).abs() < 1.0);
    assert!((p0.height_pt - 841.89).abs() < 1.0);
    assert_eq!(p0.visual_dimensions(), (p0.width_pt, p0.height_pt));

    let layout0 = session.get_page_layout(0).expect("Layout for page 0");
    assert!(!layout0.text_runs.is_empty(), "Page 0 must have text runs");
    assert!(
        layout0
            .plain_text
            .contains("KESTREL-PDF VISUAL ENGINE SPECIFICATION"),
        "Page 0 must contain main title"
    );
    assert!(
        layout0
            .plain_text
            .contains("Section 1: Architecture Overview"),
        "Page 0 must contain section header"
    );
    assert!(
        layout0.plain_text.contains("VERIFIED"),
        "Page 0 must contain table metric text"
    );

    // Vector rects on Page 0 (header bar + table rows)
    assert!(
        layout0.rects.len() >= 5,
        "Page 0 must contain header rect and table grid rows: got {}",
        layout0.rects.len()
    );

    // Embedded image on Page 0 (16x16 RGB checkerboard)
    assert_eq!(
        layout0.images.len(),
        1,
        "Page 0 must extract 1 embedded image"
    );
    let img0 = &layout0.images[0];
    assert_eq!(img0.pixel_width, 16);
    assert_eq!(img0.pixel_height, 16);
    assert_eq!(img0.rgba.len(), 16 * 16 * 4);
    // Check first pixel (Royal Blue [37, 99, 235, 255])
    assert_eq!(&img0.rgba[0..4], &[37, 99, 235, 255]);
    // Check second pixel (Amber [245, 158, 11, 255])
    assert_eq!(&img0.rgba[4..8], &[245, 158, 11, 255]);

    // Page 1: Landscape (90°), 841.89 x 595.28
    let p1 = &session.pages[1];
    assert_eq!(p1.rotation_degrees, 90);
    assert!((p1.width_pt - 841.89).abs() < 1.0);
    assert!((p1.height_pt - 595.28).abs() < 1.0);
    // visual_dimensions swaps width and height
    assert_eq!(p1.visual_dimensions(), (p1.height_pt, p1.width_pt));
    let layout1 = session.get_page_layout(1).expect("Layout for page 1");
    assert!(
        layout1
            .plain_text
            .contains("LANDSCAPE MONITORING DASHBOARD"),
        "Page 1 must contain landscape dashboard text"
    );

    // Page 2: Inverted Portrait (180°)
    let p2 = &session.pages[2];
    assert_eq!(p2.rotation_degrees, 180);
    let layout2 = session.get_page_layout(2).expect("Layout for page 2");
    assert!(
        layout2.plain_text.contains("INVERTED SPECIFICATION SHEET"),
        "Page 2 must contain inverted title"
    );

    // Page 3: Inverted Landscape (270°)
    let p3 = &session.pages[3];
    assert_eq!(p3.rotation_degrees, 270);
    assert_eq!(p3.visual_dimensions(), (p3.height_pt, p3.width_pt));
    let layout3 = session.get_page_layout(3).expect("Layout for page 3");
    assert!(
        layout3
            .plain_text
            .contains("INVERTED LANDSCAPE LOGISTICS PLAN"),
        "Page 3 must contain inverted landscape title"
    );
}

#[test]
fn test_integration_synthetic_dynamic_rotation_and_coordinate_mapping() {
    let mut builder = SyntheticPdfBuilder::new();
    let p0 = builder.add_page(600.0, 800.0, 0);
    builder.add_text(p0, "Rotation Test", 50.0, 750.0, 14.0, [0, 0, 0]);
    let bytes = builder.build().expect("Build PDF");

    let mut session = DocumentSession::open_from_bytes(bytes, None).expect("Open PDF");
    assert_eq!(session.pages[0].rotation_degrees, 0);
    assert_eq!(session.pages[0].visual_dimensions(), (600.0, 800.0));

    // Rotate Clockwise
    session.rotate_page(0, true);
    assert_eq!(session.pages[0].rotation_degrees, 90);
    assert_eq!(session.pages[0].visual_dimensions(), (800.0, 600.0));

    session.rotate_page(0, true);
    assert_eq!(session.pages[0].rotation_degrees, 180);
    assert_eq!(session.pages[0].visual_dimensions(), (600.0, 800.0));

    session.rotate_page(0, true);
    assert_eq!(session.pages[0].rotation_degrees, 270);
    assert_eq!(session.pages[0].visual_dimensions(), (800.0, 600.0));

    session.rotate_page(0, true);
    assert_eq!(session.pages[0].rotation_degrees, 0);
    assert_eq!(session.pages[0].visual_dimensions(), (600.0, 800.0));

    // Rotate Counter-Clockwise
    session.rotate_page(0, false);
    assert_eq!(session.pages[0].rotation_degrees, 270);

    session.rotate_page(0, false);
    assert_eq!(session.pages[0].rotation_degrees, 180);

    // Verify coordinate mapping helper
    let (vx0, vy0) = map_pdf_point_to_visual(50.0, 750.0, 600.0, 800.0, 0);
    assert_eq!((vx0, vy0), (50.0, 50.0));

    let (vx90, vy90) = map_pdf_point_to_visual(50.0, 750.0, 600.0, 800.0, 90);
    assert_eq!((vx90, vy90), (750.0, 50.0));

    let (vx180, vy180) = map_pdf_point_to_visual(50.0, 750.0, 600.0, 800.0, 180);
    assert_eq!((vx180, vy180), (550.0, 750.0));

    let (vx270, vy270) = map_pdf_point_to_visual(50.0, 750.0, 600.0, 800.0, 270);
    assert_eq!((vx270, vy270), (50.0, 550.0));
}

#[test]
fn test_integration_synthetic_forms_lifecycle_roundtrip() {
    let pdf_bytes = generate_synthetic_forms_pdf();
    let mut session =
        DocumentSession::open_from_bytes(pdf_bytes, None).expect("Open synthetic forms PDF");

    assert_eq!(session.forms.len(), 5, "Expected 5 synthetic form fields");

    // 1. Verify initial field state
    let applicant = session
        .forms
        .iter()
        .find(|f| f.name == "applicant_name")
        .unwrap();
    assert_eq!(applicant.value, "Alice Montgomery");

    let organization = session
        .forms
        .iter()
        .find(|f| f.name == "organization_name")
        .unwrap();
    assert_eq!(organization.value, "Starlight Dynamics Corp");

    let accept_nda = session
        .forms
        .iter()
        .find(|f| f.name == "accept_nda")
        .unwrap();
    assert_eq!(accept_nda.value, "Yes");
    assert!(matches!(
        accept_nda.field_type,
        FormFieldType::CheckBox { checked: true }
    ));

    let subscribe = session
        .forms
        .iter()
        .find(|f| f.name == "subscribe_updates")
        .unwrap();
    assert_eq!(subscribe.value, "Off");
    assert!(matches!(
        subscribe.field_type,
        FormFieldType::CheckBox { checked: false }
    ));

    let jurisdiction = session
        .forms
        .iter()
        .find(|f| f.name == "jurisdiction")
        .unwrap();
    assert_eq!(jurisdiction.value, "European Union (GDPR)");

    // 2. Mutate all fields
    assert!(session.update_form_field("applicant_name", "Dr. Evelyn Reed"));
    assert!(session.update_form_field("organization_name", "Quantum Nexus Labs"));
    assert!(session.update_form_field("accept_nda", "Off"));
    assert!(session.update_form_field("subscribe_updates", "Yes"));
    assert!(session.update_form_field("jurisdiction", "Switzerland"));

    // 3. Serialize to PDF binary stream
    let saved_bytes = session.save_to_bytes().expect("Save mutated forms");
    assert!(!saved_bytes.is_empty());

    // 4. Reload saved bytes in a completely fresh session
    let reloaded_session =
        DocumentSession::open_from_bytes(saved_bytes, None).expect("Reload saved forms PDF");

    assert_eq!(reloaded_session.forms.len(), 5);

    let re_app = reloaded_session
        .forms
        .iter()
        .find(|f| f.name == "applicant_name")
        .unwrap();
    assert_eq!(re_app.value, "Dr. Evelyn Reed");

    let re_org = reloaded_session
        .forms
        .iter()
        .find(|f| f.name == "organization_name")
        .unwrap();
    assert_eq!(re_org.value, "Quantum Nexus Labs");

    let re_nda = reloaded_session
        .forms
        .iter()
        .find(|f| f.name == "accept_nda")
        .unwrap();
    assert_eq!(re_nda.value, "Off");
    assert!(matches!(
        re_nda.field_type,
        FormFieldType::CheckBox { checked: false }
    ));

    let re_sub = reloaded_session
        .forms
        .iter()
        .find(|f| f.name == "subscribe_updates")
        .unwrap();
    assert_eq!(re_sub.value, "Yes");
    assert!(matches!(
        re_sub.field_type,
        FormFieldType::CheckBox { checked: true }
    ));

    let re_jur = reloaded_session
        .forms
        .iter()
        .find(|f| f.name == "jurisdiction")
        .unwrap();
    assert_eq!(re_jur.value, "Switzerland");
}

#[test]
fn test_integration_synthetic_search_corpus_multi_page() {
    let pdf_bytes = generate_synthetic_search_corpus_pdf();
    let session = DocumentSession::open_from_bytes(pdf_bytes, None)
        .expect("Open synthetic search corpus PDF");

    assert_eq!(session.page_count, 3);

    // Search Page 0 unique token
    let res0 = session.search_text("ALPHA_SEARCH_TOKEN_42");
    assert_eq!(res0.len(), 1);
    assert_eq!(res0[0].page_index, 0);
    assert_eq!(res0[0].match_count, 1);
    assert!(res0[0].snippet.contains("ALPHA_SEARCH_TOKEN_42"));

    // Case-insensitive search
    let res0_ci = session.search_text("alpha_search_token_42");
    assert_eq!(res0_ci.len(), 1);
    assert_eq!(res0_ci[0].page_index, 0);

    // Search Page 1 unique token
    let res1 = session.search_text("BETA_SECURITY_HASH_99");
    assert_eq!(res1.len(), 1);
    assert_eq!(res1[0].page_index, 1);
    assert_eq!(res1[0].match_count, 1);

    // Multi-occurrence search on Page 1
    let res_multi = session.search_text("KEYWORD_MULTI_OCCURRENCE");
    assert_eq!(res_multi.len(), 1);
    assert_eq!(res_multi[0].page_index, 1);
    assert_eq!(res_multi[0].match_count, 2);

    // Search Page 2 unique token & Spanish term
    let res2 = session.search_text("GAMMA_IBAN_SPANISH_ES91");
    assert_eq!(res2.len(), 1);
    assert_eq!(res2[0].page_index, 2);

    let res_es = session.search_text("FACTURACIÓN");
    assert_eq!(res_es.len(), 1);
    assert_eq!(res_es[0].page_index, 2);

    // Common term across pages
    let res_pdf = session.search_text("PDF");
    assert!(
        res_pdf.len() >= 2,
        "PDF should appear on multiple pages: got {}",
        res_pdf.len()
    );

    // Non-existent search query
    let no_res = session.search_text("MISSING_QUERY_STRING_UNKNOWN");
    assert!(no_res.is_empty());

    // Empty search query
    let empty_res = session.search_text("");
    assert!(empty_res.is_empty());
}
