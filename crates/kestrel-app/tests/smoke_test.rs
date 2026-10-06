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

#[test]
fn test_e2e_form_fill_interaction_and_saving() {
    use kestrel_app::app::SidebarTab;
    use kestrel_core::forms::FormField;

    let mut app = KestrelApp::default();
    let pdf_bytes = create_sample_pdf_bytes("Non-Disclosure Agreement 2026");
    app.load_document_bytes(pdf_bytes, Some("nda.pdf".to_string()));

    // Inject AcroForm field into document
    if let Some(session) = &mut app.session {
        session.add_form_field(FormField::new_text(
            "signer_organization",
            "Signer Organization",
            0,
            "Acme Global Inc.",
            [50.0, 600.0, 300.0, 625.0],
            false,
        ));
    }

    // Switch tool and sidebar tab
    app.active_tool = ActiveTool::FormFill;
    app.sidebar_tab = SidebarTab::Forms;

    // Render UI frame
    let ctx = Context::default();
    let full_output = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });

    let rendered_texts = extract_all_text_from_shapes(&full_output.shapes);
    assert!(
        rendered_texts
            .iter()
            .any(|t| t.contains("Signer Organization")),
        "Must render form field name in UI"
    );
    assert!(
        rendered_texts
            .iter()
            .any(|t| t.contains("Acme Global Inc.")),
        "Must render current form field value in UI"
    );

    // Update field value and verify save
    assert!(app
        .session
        .as_mut()
        .unwrap()
        .update_form_field("Signer Organization", "Globex Corp"));
    let saved = app
        .session
        .as_mut()
        .unwrap()
        .save_to_bytes()
        .expect("Save NDA PDF");
    assert!(!saved.is_empty());
}

#[test]
fn test_e2e_contract_signature_creation_and_placement() {
    use kestrel_core::sign::StrokePoint;

    let mut app = KestrelApp::default();
    let pdf_bytes = create_sample_pdf_bytes("Employment Contract for Senior Architect");
    app.load_document_bytes(pdf_bytes, Some("contract.pdf".to_string()));

    // 1. Open signature pad modal
    app.signature_modal_open = true;
    app.signer_name_input = "Dr. Jane Smith".to_string();
    app.embed_digital_signature = true;

    // 2. Simulate stylus stroke capture
    app.signature_pad_current_stroke = vec![
        StrokePoint::new(10.0, 20.0, 0.5),
        StrokePoint::new(50.0, 80.0, 0.9),
        StrokePoint::new(100.0, 40.0, 0.8),
        StrokePoint::new(180.0, 90.0, 0.4),
    ];

    // 3. Adopt signature
    app.adopt_signature_from_pad();
    assert!(
        app.adopted_signature.is_some(),
        "Adopted signature must be populated"
    );
    assert!(
        !app.signature_modal_open,
        "Modal must be closed after adoption"
    );
    assert_eq!(app.active_tool, ActiveTool::SignContract);

    // 4. Place signature on page 0
    app.place_adopted_signature(0, 180.0, 250.0);

    let session = app.session.as_ref().unwrap();
    assert_eq!(
        session.visual_signatures.len(),
        1,
        "Must contain 1 visual signature"
    );
    assert!(
        session.digital_signature.is_some(),
        "Must contain digital signature metadata"
    );
    assert_eq!(
        session.digital_signature.as_ref().unwrap().signer_name,
        "Dr. Jane Smith"
    );

    // 5. Render UI frame and verify digital badge rendering
    let ctx = Context::default();
    let full_output = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });

    let rendered_texts = extract_all_text_from_shapes(&full_output.shapes);
    assert!(
        rendered_texts.iter().any(|t| t.contains("PAdES")),
        "Must render cryptographic verification badge on signed document canvas"
    );
}

#[test]
fn test_e2e_fit_width_and_fit_page_and_visual_layout() {
    use kestrel_app::app::FitMode;

    let mut app = KestrelApp::default();
    let pdf_bytes = create_sample_pdf_bytes("High Fidelity Layout Fit Test");
    app.load_document_bytes(pdf_bytes, Some("layout_test.pdf".to_string()));

    // 1. Initial zoom level is default 1.0
    assert_eq!(app.zoom_level, 1.0);

    // 2. Request Fit Width
    app.pending_fit = Some(FitMode::FitWidth);

    let ctx = Context::default();
    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(1200.0, 900.0),
        )),
        ..Default::default()
    };

    let full_output = ctx.run(raw_input.clone(), |ctx| {
        app.render_ui(ctx);
    });

    // Zoom level must have been recalculated to fit available width
    assert!(
        app.zoom_level > 1.0,
        "Zoom level should adapt to fit 1200px width: got {}",
        app.zoom_level
    );

    // 3. Request Fit Page
    app.pending_fit = Some(FitMode::FitPage);
    let _ = ctx.run(raw_input, |ctx| {
        app.render_ui(ctx);
    });

    assert!(
        app.zoom_level > 0.5,
        "Zoom level should be valid positive value after Fit Page: got {}",
        app.zoom_level
    );

    // 4. Verify text content is in rendered output
    let texts = extract_all_text_from_shapes(&full_output.shapes);
    assert!(
        texts
            .iter()
            .any(|t| t.contains("High Fidelity Layout Fit Test")),
        "Rendered canvas must contain text: {:?}",
        texts
    );
}
