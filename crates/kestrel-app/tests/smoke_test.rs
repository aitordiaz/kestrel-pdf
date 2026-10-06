use egui::Context;
use kestrel_app::app::{ActiveTool, KestrelApp};
use lopdf::{dictionary, Document, Object, Stream};

/// Helper function to create a minimal, valid in-memory PDF document.
fn create_sample_pdf_bytes(title: &str) -> Vec<u8> {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    let content_stream = format!("BT /F1 14 Tf 50 750 Td ({}) Tj ET", title);
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
    doc.save_to(&mut buffer).expect("Serialize sample PDF");
    buffer
}

#[test]
fn test_e2e_smoke_app_lifecycle_and_ui_frame() {
    let mut app = KestrelApp::default();

    // 1. Verify initial default state
    assert_eq!(app.active_tool, ActiveTool::Pan);
    assert_eq!(app.zoom_level, 1.0);
    assert_eq!(app.current_page, 1);
    assert_eq!(app.total_pages, 0);
    assert!(app.sidebar_open);

    // 2. Simulate tool cycling
    let tools = [
        ActiveTool::SelectText,
        ActiveTool::FormFill,
        ActiveTool::EditText,
        ActiveTool::EditImage,
        ActiveTool::SignContract,
        ActiveTool::RedactData,
        ActiveTool::Pan,
    ];
    for tool in tools {
        app.active_tool = tool;
        assert_eq!(app.active_tool, tool);
    }

    // 3. Simulate zoom operations
    app.zoom_level *= 1.15;
    assert!(app.zoom_level > 1.0);
    app.zoom_level = 0.05; // Below min bound test
    app.zoom_level = app.zoom_level.max(0.1);
    assert_eq!(app.zoom_level, 0.1);

    // 4. Simulate document loading state
    let pdf_bytes = create_sample_pdf_bytes("Kestrel Phase 1 Contract");
    app.load_document_bytes(pdf_bytes, Some("sample.pdf".to_string()));
    assert_eq!(app.total_pages, 1);
    assert_eq!(app.current_page, 1);
    assert!(app.session.is_some());

    // 5. Test search functionality
    app.search_query = "Contract".to_string();
    app.execute_search();
    assert_eq!(app.search_results.len(), 1);

    // 6. Run headless egui context frame simulation with loaded document
    let ctx = Context::default();
    let raw_input = egui::RawInput::default();

    let full_output = ctx.run(raw_input, |ctx| {
        app.render_ui(ctx);
    });

    // 7. Assertions on UI frame completion
    assert!(
        !full_output.shapes.is_empty(),
        "Smoke test failed: egui frame should produce rendered UI shapes for loaded document"
    );
}

#[test]
fn test_e2e_smoke_empty_document_state_frame() {
    let mut app = KestrelApp::default();
    let ctx = Context::default();

    let full_output = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });

    assert!(
        !full_output.shapes.is_empty(),
        "Smoke test failed: empty state frame must produce welcome UI shapes"
    );
}
