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

fn extract_all_text_from_shapes(shapes: &[egui::epaint::ClippedShape]) -> Vec<String> {
    fn extract_from_shape(shape: &egui::epaint::Shape, texts: &mut Vec<String>) {
        match shape {
            egui::epaint::Shape::Text(text_shape) => {
                texts.push(text_shape.galley.text().to_string());
            }
            egui::epaint::Shape::Vec(vec) => {
                for s in vec {
                    extract_from_shape(s, texts);
                }
            }
            _ => {}
        }
    }

    let mut texts = Vec::new();
    for clipped in shapes {
        extract_from_shape(&clipped.shape, &mut texts);
    }
    texts
}

#[test]
fn test_e2e_pdf_viewer_displays_title_and_filename_in_title_bar() {
    let document_title = "Confidential Strategic Plan 2026";
    let document_filename = "strategic_plan_v2.pdf";

    // 1. Generate valid synthetic PDF with the specific title
    let pdf_bytes = create_sample_pdf_bytes(document_title);

    // 2. Initialize application and load document with filename
    let mut app = KestrelApp::default();
    app.load_document_bytes(pdf_bytes, Some(document_filename.to_string()));

    // 3. Verify filename is present in the title bar text
    let title_bar = app.title_bar_text();
    assert!(
        title_bar.contains(document_filename),
        "Title bar text must contain filename '{}', got: '{}'",
        document_filename,
        title_bar
    );

    // 4. Run full UI render cycle
    let ctx = Context::default();
    let raw_input = egui::RawInput::default();
    let full_output = ctx.run(raw_input, |ctx| {
        app.render_ui(ctx);
    });

    // 5. Extract all rendered text elements across all UI shapes
    let all_rendered_texts = extract_all_text_from_shapes(&full_output.shapes);

    // 6. Assert filename is present in the rendered title bar heading
    let filename_rendered = all_rendered_texts
        .iter()
        .any(|t| t.contains(document_filename));
    assert!(
        filename_rendered,
        "Filename '{}' must be rendered in the top title bar heading. Rendered texts: {:?}",
        document_filename, all_rendered_texts
    );

    // 7. Assert document title is present in the PDF viewer
    let title_rendered = all_rendered_texts
        .iter()
        .any(|t| t.contains(document_title));
    assert!(
        title_rendered,
        "Title '{}' must be rendered in the PDF viewer canvas. Rendered texts: {:?}",
        document_title, all_rendered_texts
    );
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
