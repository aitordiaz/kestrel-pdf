use egui::{Color32, Context};
use kestrel_app::app::{
    clamp_rgba_image_to_max_side, is_copy_shortcut_pressed, is_select_all_shortcut_pressed,
    standard_copy_shortcut_str, standard_select_all_shortcut_str, truncate_filename_middle,
    ActiveTool, FitMode, KestrelApp, SidebarTab,
};
use kestrel_app::theme::Theme;
use kestrel_core::synthetic::{
    generate_all_synthetic_stress_tiers, generate_synthetic_forms_pdf,
    generate_synthetic_search_corpus_pdf, generate_synthetic_visual_showcase_pdf,
};
use lopdf::{dictionary, Document, Object, Stream};

fn has_rect_with_fill(shapes: &[egui::epaint::ClippedShape], target_fill: egui::Color32) -> bool {
    fn check_shape(shape: &egui::epaint::Shape, target: egui::Color32) -> bool {
        match shape {
            egui::epaint::Shape::Rect(rect_shape) => rect_shape.fill == target,
            egui::epaint::Shape::Vec(vec) => vec.iter().any(|s| check_shape(s, target)),
            _ => false,
        }
    }
    shapes.iter().any(|c| check_shape(&c.shape, target_fill))
}

fn find_rects_with_fill(
    shapes: &[egui::epaint::ClippedShape],
    target_fill: egui::Color32,
) -> Vec<egui::Rect> {
    fn collect_rects(
        shape: &egui::epaint::Shape,
        target: egui::Color32,
        acc: &mut Vec<egui::Rect>,
    ) {
        match shape {
            egui::epaint::Shape::Rect(rect_shape) => {
                if rect_shape.fill == target {
                    acc.push(rect_shape.rect);
                }
            }
            egui::epaint::Shape::Vec(vec) => {
                for s in vec {
                    collect_rects(s, target, acc);
                }
            }
            _ => {}
        }
    }
    let mut acc = Vec::new();
    for c in shapes {
        collect_rects(&c.shape, target_fill, &mut acc);
    }
    acc
}

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
    assert_eq!(app.page_input_text, "1");
    assert!(
        !app.sidebar_open,
        "Sidebar must default to closed for focused reading"
    );

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
    app.sidebar_open = true;
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

#[test]
fn test_e2e_orientation_and_page_rotation_workflow() {
    let pdf_bytes = generate_synthetic_visual_showcase_pdf();
    let mut app = KestrelApp::default();
    app.load_document_bytes(pdf_bytes, Some("orientations.pdf".to_string()));

    assert_eq!(app.total_pages, 4);
    assert_eq!(app.current_page, 1);

    // Initial page rotation is 0°
    assert_eq!(app.session.as_ref().unwrap().pages[0].rotation_degrees, 0);

    // 1. Rotate Page 1 Clockwise
    app.rotate_current_page_clockwise();
    assert_eq!(app.session.as_ref().unwrap().pages[0].rotation_degrees, 90);
    assert_eq!(
        app.session.as_ref().unwrap().pages[0].visual_dimensions(),
        (841.89, 595.28)
    );
    assert!(app
        .status_toast
        .as_ref()
        .unwrap()
        .contains("Page 1 rotated to 90°"));

    // Render frame and verify toast text
    let ctx = Context::default();
    let output1 = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let texts1 = extract_all_text_from_shapes(&output1.shapes);
    assert!(texts1.iter().any(|t| t.contains("Page 1 rotated to 90°")));

    // 2. Rotate Counter-Clockwise back to 0°
    app.rotate_current_page_counter_clockwise();
    assert_eq!(app.session.as_ref().unwrap().pages[0].rotation_degrees, 0);
    assert_eq!(
        app.session.as_ref().unwrap().pages[0].visual_dimensions(),
        (595.28, 841.89)
    );

    // 3. Navigate to Page 2 (90° Landscape page)
    app.current_page = 2;
    assert_eq!(app.session.as_ref().unwrap().pages[1].rotation_degrees, 90);

    // 4. Test Fit Width on rotated page
    app.pending_fit = Some(FitMode::FitWidth);
    let output2 = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(1000.0, 800.0),
            )),
            ..Default::default()
        },
        |ctx| {
            app.render_ui(ctx);
        },
    );

    assert!(app.zoom_level > 0.5 && app.zoom_level < 3.0);
    let texts2 = extract_all_text_from_shapes(&output2.shapes);
    assert!(texts2
        .iter()
        .any(|t| t.contains("LANDSCAPE MONITORING DASHBOARD")));
}

#[test]
fn test_e2e_zoom_controls_and_responsive_fitting() {
    let pdf_bytes = generate_synthetic_visual_showcase_pdf();
    let mut app = KestrelApp::default();
    app.load_document_bytes(pdf_bytes, Some("zoom_test.pdf".to_string()));

    assert_eq!(app.zoom_level, 1.0);

    // Zoom In
    app.zoom_in();
    assert!(app.zoom_level > 1.14 && app.zoom_level < 1.16);

    // Zoom Out
    app.zoom_out();
    assert!((app.zoom_level - 1.0).abs() < 0.05);

    // Reset Zoom
    app.zoom_level = 2.5;
    app.reset_zoom();
    assert_eq!(app.zoom_level, 1.0);

    // Clamping min / max
    app.set_zoom(0.001);
    assert_eq!(app.zoom_level, 0.1);
    app.set_zoom(99.0);
    assert_eq!(app.zoom_level, 5.0);

    // Responsive Fit Width & Fit Page across screen resolutions
    let ctx = Context::default();
    let test_screens = [
        (3840.0, 2160.0), // 4K
        (1920.0, 1080.0), // 1080p
        (1280.0, 800.0),  // Laptop
        (768.0, 1024.0),  // Tablet
        (390.0, 844.0),   // Mobile
    ];

    for (w, h) in test_screens {
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(w, h),
            )),
            ..Default::default()
        };

        // Fit Width
        app.pending_fit = Some(FitMode::FitWidth);
        let _ = ctx.run(raw_input.clone(), |ctx| {
            app.render_ui(ctx);
        });
        assert!(
            app.zoom_level >= 0.1 && app.zoom_level <= 5.0,
            "FitWidth zoom level must be clamped for resolution {}x{}: got {}",
            w,
            h,
            app.zoom_level
        );

        // Fit Page
        app.pending_fit = Some(FitMode::FitPage);
        let _ = ctx.run(raw_input, |ctx| {
            app.render_ui(ctx);
        });
        assert!(
            app.zoom_level >= 0.1 && app.zoom_level <= 5.0,
            "FitPage zoom level must be clamped for resolution {}x{}: got {}",
            w,
            h,
            app.zoom_level
        );
    }
}

#[test]
fn test_e2e_multi_page_search_navigation_and_visual_highlighting() {
    let pdf_bytes = generate_synthetic_search_corpus_pdf();
    let mut app = KestrelApp::default();
    app.load_document_bytes(pdf_bytes, Some("search_test.pdf".to_string()));

    // 1. Execute search query
    app.search_query = "ALPHA_SEARCH_TOKEN_42".to_string();
    app.execute_search();

    assert_eq!(app.search_results.len(), 1);
    assert_eq!(app.sidebar_tab, SidebarTab::SearchResults);

    // 2. Render UI and assert highlight is drawn
    let ctx = Context::default();
    let output1 = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });

    let texts1 = extract_all_text_from_shapes(&output1.shapes);
    assert!(texts1.iter().any(|t| t.contains("Found 1 matching pages:")));
    assert!(texts1.iter().any(|t| t.contains("ALPHA_SEARCH_TOKEN_42")));

    // Assert visual highlight rectangle is rendered with search highlight color
    let has_search_highlight = has_rect_with_fill(&output1.shapes, Theme::SEARCH_HIGHLIGHT_ACTIVE)
        || has_rect_with_fill(&output1.shapes, Theme::SEARCH_HIGHLIGHT_REGULAR);
    assert!(
        has_search_highlight,
        "Search highlight rect must be drawn behind matching search keyword on canvas"
    );

    // 3. Multi-occurrence search & navigation
    app.search_query = "GAMMA_IBAN_SPANISH_ES91".to_string();
    app.execute_search();
    assert_eq!(app.search_results.len(), 1);
    assert_eq!(app.search_results[0].page_index, 2);

    // Click search result to navigate to page 3
    app.navigate_to_search_result(0);
    assert_eq!(app.current_page, 3);
    assert_eq!(app.page_input_text, "3");

    let output2 = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let texts2 = extract_all_text_from_shapes(&output2.shapes);
    assert!(texts2.iter().any(|t| t.contains("CERTIFICACIÓN")));
    assert!(texts2.iter().any(|t| t.contains("GAMMA_IBAN_SPANISH_ES91")));
}

#[test]
fn test_e2e_form_fill_interactive_lifecycle_and_roundtrip_saving() {
    let pdf_bytes = generate_synthetic_forms_pdf();
    let mut app = KestrelApp::default();
    app.load_document_bytes(pdf_bytes, Some("onboarding.pdf".to_string()));

    assert_eq!(app.total_pages, 1);
    app.active_tool = ActiveTool::FormFill;
    app.sidebar_tab = SidebarTab::Forms;

    // Render initial UI
    let ctx = Context::default();
    let output1 = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let texts1 = extract_all_text_from_shapes(&output1.shapes);
    assert!(texts1.iter().any(|t| t.contains("Alice Montgomery")));
    assert!(texts1.iter().any(|t| t.contains("Starlight Dynamics Corp")));
    assert!(texts1.iter().any(|t| t.contains("European Union (GDPR)")));

    // Interactively update forms
    assert!(app
        .session
        .as_mut()
        .unwrap()
        .update_form_field("applicant_name", "Samantha Croft"));
    assert!(app
        .session
        .as_mut()
        .unwrap()
        .update_form_field("accept_nda", "Off"));
    assert!(app
        .session
        .as_mut()
        .unwrap()
        .update_form_field("subscribe_updates", "Yes"));
    assert!(app
        .session
        .as_mut()
        .unwrap()
        .update_form_field("jurisdiction", "United Kingdom"));

    // Save filled document to bytes
    let saved_bytes = app
        .session
        .as_mut()
        .unwrap()
        .save_to_bytes()
        .expect("Save filled PDF bytes");
    assert!(!saved_bytes.is_empty());

    // Load saved bytes into a second app instance
    let mut app2 = KestrelApp::default();
    app2.load_document_bytes(saved_bytes, Some("saved_onboarding.pdf".to_string()));
    app2.active_tool = ActiveTool::FormFill;
    app2.sidebar_tab = SidebarTab::Forms;

    // Verify session data
    let session2 = app2.session.as_ref().unwrap();
    let app_field = session2
        .forms
        .iter()
        .find(|f| f.name == "applicant_name")
        .unwrap();
    assert_eq!(app_field.value, "Samantha Croft");
    let nda_field = session2
        .forms
        .iter()
        .find(|f| f.name == "accept_nda")
        .unwrap();
    assert_eq!(nda_field.value, "Off");
    let sub_field = session2
        .forms
        .iter()
        .find(|f| f.name == "subscribe_updates")
        .unwrap();
    assert_eq!(sub_field.value, "Yes");
    let jur_field = session2
        .forms
        .iter()
        .find(|f| f.name == "jurisdiction")
        .unwrap();
    assert_eq!(jur_field.value, "United Kingdom");

    // Render UI of app2 and assert modified values appear in the UI
    let output2 = ctx.run(egui::RawInput::default(), |ctx| {
        app2.render_ui(ctx);
    });
    let texts2 = extract_all_text_from_shapes(&output2.shapes);
    assert!(texts2.iter().any(|t| t.contains("Samantha Croft")));
    assert!(texts2.iter().any(|t| t.contains("United Kingdom")));
}

#[test]
fn test_e2e_embedded_images_and_vector_graphics_rendering() {
    let pdf_bytes = generate_synthetic_visual_showcase_pdf();
    let mut app = KestrelApp::default();
    app.load_document_bytes(pdf_bytes, Some("showcase.pdf".to_string()));

    let ctx = Context::default();
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });

    // 1. Assert embedded image was loaded into GPU texture map
    assert_eq!(
        app.image_textures.len(),
        1,
        "Embedded 16x16 raster image should be bound as GPU texture"
    );

    // 2. Assert vector header rectangle was painted with Deep Navy fill
    let has_navy_header = has_rect_with_fill(&output.shapes, Color32::from_rgb(24, 43, 73));
    assert!(
        has_navy_header,
        "Header vector rectangle must be painted with Color32(24, 43, 73)"
    );

    // 3. Assert title text is rendered
    let texts = extract_all_text_from_shapes(&output.shapes);
    assert!(texts
        .iter()
        .any(|t| t.contains("KESTREL-PDF VISUAL ENGINE SPECIFICATION")));
}

#[test]
fn test_e2e_full_lifecycle_stress_session() {
    use kestrel_core::sign::StrokePoint;

    // 1. Initial empty state
    let mut app = KestrelApp::default();
    let ctx = Context::default();
    let out_empty = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let texts_empty = extract_all_text_from_shapes(&out_empty.shapes);
    assert!(texts_empty.iter().any(|t| t.contains("Kestrel-PDF")));
    assert!(texts_empty.iter().any(|t| t.contains("Abrir fichero")));

    // 2. Load synthetic showcase PDF
    let pdf_bytes = generate_synthetic_visual_showcase_pdf();
    app.load_document_bytes(pdf_bytes, Some("full_stress.pdf".to_string()));
    assert_eq!(app.total_pages, 4);

    // 3. Page rotation & zoom
    app.rotate_current_page_clockwise();
    assert_eq!(app.session.as_ref().unwrap().pages[0].rotation_degrees, 90);
    app.pending_fit = Some(FitMode::FitPage);

    // 4. Full-text search
    app.search_query = "Specification".to_string();
    app.execute_search();
    assert!(!app.search_results.is_empty());

    // 5. Digital & visual signature flow
    app.signature_modal_open = true;
    app.signer_name_input = "Chief Architect John Doe".to_string();
    app.signature_pad_current_stroke = vec![
        StrokePoint::new(10.0, 10.0, 0.5),
        StrokePoint::new(40.0, 50.0, 0.8),
        StrokePoint::new(100.0, 20.0, 0.6),
    ];
    app.adopt_signature_from_pad();
    app.place_adopted_signature(0, 200.0, 300.0);

    // 6. Render UI frame
    let out_full = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let texts_full = extract_all_text_from_shapes(&out_full.shapes);
    assert!(texts_full.iter().any(|t| t.contains("PAdES")));

    // 7. Save and verify serialized output
    let saved = app
        .session
        .as_mut()
        .unwrap()
        .save_to_bytes()
        .expect("Save stress PDF");
    assert!(!saved.is_empty());
}

#[test]
fn test_e2e_form_field_compact_geometry_and_non_occlusion() {
    use kestrel_core::forms::FormField;
    use kestrel_core::synthetic::SyntheticPdfBuilder;

    let mut builder = SyntheticPdfBuilder::new();
    let p0 = builder.add_page(595.0, 842.0, 0);
    // Background text run that sits right under/near the form field
    builder.add_text(
        p0,
        "Underlying Document Underscore Line ______",
        100.0,
        650.0,
        10.0,
        [0, 0, 0],
    );
    let bytes = builder.build().expect("Build PDF");

    let mut app = KestrelApp::default();
    app.load_document_bytes(bytes, Some("compact_form.pdf".to_string()));

    // Inject compact form field (height = 11.5 pt)
    if let Some(session) = &mut app.session {
        session.add_form_field(FormField::new_text(
            "compact_input",
            "Compact Input",
            0,
            "Filled Value",
            [100.0, 646.0, 300.0, 657.5],
            false,
        ));
    }

    let ctx = Context::default();

    // 1. In Pan mode: background text must be present, and NO opaque 200-alpha background shape
    app.active_tool = ActiveTool::Pan;
    let out_pan = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let texts_pan = extract_all_text_from_shapes(&out_pan.shapes);
    assert!(
        texts_pan
            .iter()
            .any(|t| t.contains("Underlying Document Underscore Line")),
        "Background text must be visible in Pan mode"
    );
    assert!(
        texts_pan.iter().any(|t| t.contains("Filled Value")),
        "Form value must be visible"
    );

    // Verify no shape has opaque 200-alpha fill
    let has_opaque_form_fill = out_pan.shapes.iter().any(|cs| match &cs.shape {
        egui::epaint::Shape::Rect(rect_shape) => {
            rect_shape.fill == Color32::from_rgba_unmultiplied(239, 246, 255, 200)
        }
        _ => false,
    });
    assert!(
        !has_opaque_form_fill,
        "Form field must not render opaque 200-alpha background"
    );
}

#[test]
fn test_e2e_parallel_open_and_exhaustive_memory_pdf_stress_matrix() {
    let tiers = std::sync::Arc::new(generate_all_synthetic_stress_tiers());
    assert_eq!(tiers.len(), 8);

    // Concurrently open and simulate full UI execution for all 8 tiers in parallel threads
    std::thread::scope(|s| {
        for i in 0..8 {
            let tiers = std::sync::Arc::clone(&tiers);
            s.spawn(move || {
                let (filename, bytes) = &tiers[i];
                let mut app = KestrelApp::default();
                app.load_document_bytes(bytes.clone(), Some(filename.to_string()));

                assert!(app.session.is_some());
                assert!(app.title_bar_text().contains(filename));
                assert!(app.total_pages >= 1);

                let ctx = Context::default();
                let out = ctx.run(egui::RawInput::default(), |ctx| {
                    app.render_ui(ctx);
                });
                assert!(!out.shapes.is_empty());

                // Additional tier-specific checks
                if *filename == "tier6_layers.pdf" {
                    app.sidebar_tab = SidebarTab::Layers;
                    let out_layers = ctx.run(egui::RawInput::default(), |ctx| {
                        app.render_ui(ctx);
                    });
                    assert!(!out_layers.shapes.is_empty());
                    assert!(app.toggle_layer(0));
                } else if *filename == "tier5_acroforms.pdf" {
                    app.active_tool = ActiveTool::FormFill;
                    app.sidebar_tab = SidebarTab::Forms;
                    let out_forms = ctx.run(egui::RawInput::default(), |ctx| {
                        app.render_ui(ctx);
                    });
                    assert!(!out_forms.shapes.is_empty());
                } else if *filename == "tier7_orientations.pdf" {
                    app.rotate_current_page_clockwise();
                    let out_rot = ctx.run(egui::RawInput::default(), |ctx| {
                        app.render_ui(ctx);
                    });
                    assert!(!out_rot.shapes.is_empty());
                }
            });
        }
    });
}

#[test]
fn test_e2e_sequential_rapid_document_switching_across_all_tiers() {
    let tiers = generate_all_synthetic_stress_tiers();
    let mut app = KestrelApp::default();
    let ctx = Context::default();

    for (filename, bytes) in &tiers {
        // 1. Rapidly switch active document in memory
        app.load_document_bytes(bytes.clone(), Some(filename.to_string()));

        // 2. Assert document loaded and title updated
        assert!(app.session.is_some());
        assert!(app.title_bar_text().contains(filename));
        assert!(app.total_pages >= 1);
        assert_eq!(app.current_page, 1);

        // 3. Render base frame
        let out1 = ctx.run(egui::RawInput::default(), |ctx| {
            app.render_ui(ctx);
        });
        assert!(!out1.shapes.is_empty());

        // 4. Test zoom manipulations
        app.zoom_in();
        app.zoom_out();
        app.pending_fit = Some(FitMode::FitWidth);
        app.pending_fit = Some(FitMode::FitPage);

        // 5. Test sidebar tab cycling
        app.sidebar_tab = SidebarTab::Thumbnails;
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.render_ui(ctx));

        app.sidebar_tab = SidebarTab::Outlines;
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.render_ui(ctx));

        app.sidebar_tab = SidebarTab::Forms;
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.render_ui(ctx));

        app.sidebar_tab = SidebarTab::Layers;
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.render_ui(ctx));

        app.sidebar_tab = SidebarTab::SearchResults;
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.render_ui(ctx));

        // 6. Test tool cycling
        app.active_tool = ActiveTool::SelectText;
        app.active_tool = ActiveTool::FormFill;
        app.active_tool = ActiveTool::Pan;
    }
}

#[test]
fn test_e2e_truncate_filename_middle_helper() {
    // 1. Short filename remains intact
    assert_eq!(truncate_filename_middle("contract.pdf", 48), "contract.pdf");
    assert_eq!(truncate_filename_middle("doc.pdf", 20), "doc.pdf");

    // 2. Long filename gets truncated in the middle preserving extension
    let long_name = "Super_Long_Enterprise_Contract_Specification_Document_With_Metadata_2026.pdf";
    let truncated = truncate_filename_middle(long_name, 48);
    assert_eq!(truncated.chars().count(), 48);
    assert!(truncated.contains('…'));
    assert!(truncated.starts_with("Super_Long_"));
    assert!(truncated.ends_with(".pdf"));
}

#[test]
fn test_e2e_toolbar_responsive_layout_with_ultra_long_filename() {
    let ultra_long_name = "Super_Extremely_Long_Enterprise_Contract_Specification_Document_With_Lots_Of_Metadata_And_Long_Subsections_Version_2026_Final_Draft_Signed.pdf";
    let pdf_bytes = kestrel_core::synthetic::generate_tier1_minimal_pdf();

    let mut app = KestrelApp::default();
    app.load_document_bytes(pdf_bytes, Some(ultra_long_name.to_string()));

    // Verify window title has full name
    assert!(app.title_bar_text().contains(ultra_long_name));

    // Run frame simulation
    let ctx = Context::default();
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });

    let rendered_texts = extract_all_text_from_shapes(&out.shapes);

    // 1. Header renders truncated title with .pdf extension preserved
    let truncated_title = truncate_filename_middle(ultra_long_name, 48);
    assert!(
        rendered_texts.iter().any(|t| t.contains(&truncated_title)),
        "Header must render truncated title '{}'. Rendered: {:?}",
        truncated_title,
        rendered_texts
    );
    assert!(
        rendered_texts.iter().any(|t| t.contains(".pdf")),
        "Truncated title must retain .pdf extension"
    );

    // 2. File action buttons remain visible and uncrowded in Tier 1
    assert!(rendered_texts.iter().any(|t| t.contains("Abrir fichero")));
    assert!(rendered_texts.iter().any(|t| t.contains("Save / Export")));

    // 3. Navigation controls remain visible and functional in Tier 2
    assert!(rendered_texts.iter().any(|t| t.contains("/ 1")));
    assert!(rendered_texts.iter().any(|t| t.contains("Prev")));
    assert!(rendered_texts.iter().any(|t| t.contains("Next")));

    // 4. Interactive tool selector buttons are present in Tier 2
    assert!(rendered_texts.iter().any(|t| t.contains("Pan")));
    assert!(rendered_texts.iter().any(|t| t.contains("Select")));
    assert!(rendered_texts.iter().any(|t| t.contains("Forms")));
    assert!(rendered_texts.iter().any(|t| t.contains("Edit Text")));
    assert!(rendered_texts.iter().any(|t| t.contains("Sign Contract")));
    assert!(rendered_texts.iter().any(|t| t.contains("Redact")));

    // 5. Zoom & Search controls are present in Tier 2
    assert!(rendered_texts.iter().any(|t| t.contains("100%")));
    assert!(rendered_texts.iter().any(|t| t.contains("Fit Width")));
    assert!(rendered_texts.iter().any(|t| t.contains("Fit Page")));
    assert!(rendered_texts.iter().any(|t| t.contains("Find")));

    // 6. Test tool activation with ultra-long title loaded
    app.active_tool = ActiveTool::FormFill;
    assert_eq!(app.active_tool, ActiveTool::FormFill);
    app.active_tool = ActiveTool::SelectText;
    assert_eq!(app.active_tool, ActiveTool::SelectText);
    app.active_tool = ActiveTool::Pan;
    assert_eq!(app.active_tool, ActiveTool::Pan);
}

#[test]
fn test_e2e_select_and_copy_text_lifecycle() {
    let mut builder = kestrel_core::synthetic::SyntheticPdfBuilder::new();
    let p_idx = builder.add_page(595.28, 841.89, 0);
    builder.add_text(
        p_idx,
        "Universal Contract Statement 2026",
        60.0,
        740.0,
        14.0,
        [0, 0, 0],
    );

    let bytes = builder.build().expect("Build synthetic PDF");
    let mut app = KestrelApp::default();
    app.load_document_bytes(bytes, Some("contract.pdf".to_string()));

    app.active_tool = ActiveTool::SelectText;
    assert!(app.selection.is_empty());

    // Select text on page 0
    let layout = app.session.as_ref().unwrap().get_page_layout(0).unwrap();
    let text = layout.get_text_in_rect([50.0, 50.0, 400.0, 150.0], 0);
    assert!(text.contains("Universal Contract Statement 2026"));

    app.selection.page_index = Some(0);
    app.selection.selected_text_indices = vec![0];
    app.selection.selected_text = Some(text);

    assert!(app.selection.has_text());
    assert!(!app.selection.is_empty());

    let ctx = Context::default();
    let copied = app.copy_selected_text(&ctx);
    assert!(copied);
    assert!(app.status_toast.is_some());
    assert!(app.status_toast.as_ref().unwrap().contains("Copied"));

    // Render frame and verify Copy Text button appears in toolbar
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let rendered = extract_all_text_from_shapes(&out.shapes);
    assert!(
        rendered.iter().any(|t| t.contains("Copy Text")),
        "Toolbar must display Copy Text button when text is selected"
    );

    // Clear selection
    app.clear_selection();
    assert!(app.selection.is_empty());
}

#[test]
fn test_e2e_select_and_copy_image_lifecycle() {
    let mut builder = kestrel_core::synthetic::SyntheticPdfBuilder::new();
    let p_idx = builder.add_page(595.28, 841.89, 0);
    let mut img_rgb = Vec::with_capacity(32 * 32 * 3);
    for _ in 0..(32 * 32) {
        img_rgb.extend_from_slice(&[0, 128, 255]);
    }
    builder.add_image(p_idx, 80.0, 450.0, 200.0, 120.0, 32, 32, img_rgb);

    let bytes = builder.build().expect("Build synthetic PDF with image");
    let mut app = KestrelApp::default();
    app.load_document_bytes(bytes, Some("diagram.pdf".to_string()));

    app.active_tool = ActiveTool::SelectText;
    app.selection.page_index = Some(0);
    app.selection.selected_image_index = Some(0);

    assert!(app.selection.has_image());
    assert!(!app.selection.is_empty());

    let ctx = Context::default();
    let copied = app.copy_selected_image(&ctx);
    assert!(copied);
    assert!(app.status_toast.is_some());
    assert!(app.status_toast.as_ref().unwrap().contains("32×32 px"));

    // Render frame and verify Copy Image button appears in toolbar
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let rendered = extract_all_text_from_shapes(&out.shapes);
    assert!(
        rendered.iter().any(|t| t.contains("Copy Image")),
        "Toolbar must display Copy Image button when image is selected"
    );
}

#[test]
fn test_e2e_platform_copy_shortcuts_helpers() {
    let ctx = Context::default();

    // 1. macOS Cmd+C
    let raw = egui::RawInput {
        modifiers: egui::Modifiers {
            mac_cmd: true,
            ..Default::default()
        },
        events: vec![egui::Event::Key {
            key: egui::Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                mac_cmd: true,
                ..Default::default()
            },
        }],
        ..Default::default()
    };
    let _ = ctx.run(raw, |_| {});
    let triggered = ctx.input(is_copy_shortcut_pressed);
    assert!(triggered, "Cmd+C on macOS must trigger copy");

    // 2. Windows / Linux Ctrl+C
    let raw = egui::RawInput {
        modifiers: egui::Modifiers {
            ctrl: true,
            ..Default::default()
        },
        events: vec![egui::Event::Key {
            key: egui::Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
        }],
        ..Default::default()
    };
    let _ = ctx.run(raw, |_| {});
    let triggered = ctx.input(is_copy_shortcut_pressed);
    assert!(triggered, "Ctrl+C on Windows/Linux must trigger copy");

    // 3. IBM CUA Standard Ctrl+Insert
    let raw = egui::RawInput {
        modifiers: egui::Modifiers {
            ctrl: true,
            ..Default::default()
        },
        events: vec![egui::Event::Key {
            key: egui::Key::Insert,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
        }],
        ..Default::default()
    };
    let _ = ctx.run(raw, |_| {});
    let triggered = ctx.input(is_copy_shortcut_pressed);
    assert!(triggered, "Ctrl+Insert must trigger copy");

    // 4. Hardware Key::Copy
    let raw = egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::Copy,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
        ..Default::default()
    };
    let _ = ctx.run(raw, |_| {});
    let triggered = ctx.input(is_copy_shortcut_pressed);
    assert!(triggered, "Dedicated hardware Key::Copy must trigger copy");

    // 5. Native OS / Browser Event::Copy
    let raw = egui::RawInput {
        events: vec![egui::Event::Copy],
        ..Default::default()
    };
    let _ = ctx.run(raw, |_| {});
    let triggered = ctx.input(is_copy_shortcut_pressed);
    assert!(triggered, "Event::Copy must trigger copy");

    // 6. Regular 'C' key without modifier must NOT trigger copy
    let raw = egui::RawInput {
        events: vec![egui::Event::Key {
            key: egui::Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        }],
        ..Default::default()
    };
    let _ = ctx.run(raw, |_| {});
    let triggered = ctx.input(is_copy_shortcut_pressed);
    assert!(!triggered, "Regular 'C' without modifier must not copy");

    // 7. Alt+Ctrl+C should NOT trigger standard copy
    let raw = egui::RawInput {
        modifiers: egui::Modifiers {
            ctrl: true,
            alt: true,
            ..Default::default()
        },
        events: vec![egui::Event::Key {
            key: egui::Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                alt: true,
                ..Default::default()
            },
        }],
        ..Default::default()
    };
    let _ = ctx.run(raw, |_| {});
    let triggered = ctx.input(is_copy_shortcut_pressed);
    assert!(!triggered, "Alt+Ctrl+C must not trigger standard copy");
}

#[test]
fn test_e2e_select_all_shortcut() {
    let ctx = Context::default();

    // 1. Check helper with Ctrl+A
    let raw = egui::RawInput {
        modifiers: egui::Modifiers {
            ctrl: true,
            ..Default::default()
        },
        events: vec![egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
        }],
        ..Default::default()
    };
    let _ = ctx.run(raw, |_| {});
    let triggered = ctx.input(is_select_all_shortcut_pressed);
    assert!(triggered, "Ctrl+A must trigger select-all");

    // 2. Check helper with Cmd+A
    let raw = egui::RawInput {
        modifiers: egui::Modifiers {
            mac_cmd: true,
            ..Default::default()
        },
        events: vec![egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                mac_cmd: true,
                ..Default::default()
            },
        }],
        ..Default::default()
    };
    let _ = ctx.run(raw, |_| {});
    let triggered = ctx.input(is_select_all_shortcut_pressed);
    assert!(triggered, "Cmd+A must trigger select-all");

    // 3. Test select_all_current_page on a multi-line document
    let mut builder = kestrel_core::synthetic::SyntheticPdfBuilder::new();
    let p_idx = builder.add_page(595.28, 841.89, 0);
    builder.add_text(p_idx, "Header Section 2026", 50.0, 780.0, 14.0, [0, 0, 0]);
    builder.add_text(
        p_idx,
        "Body paragraph content",
        50.0,
        750.0,
        11.0,
        [0, 0, 0],
    );
    builder.add_text(p_idx, "Footer note page 1", 50.0, 50.0, 9.0, [0, 0, 0]);

    let bytes = builder.build().expect("Build synthetic PDF");
    let mut app = KestrelApp::default();
    app.load_document_bytes(bytes, Some("multiline.pdf".to_string()));
    app.active_tool = ActiveTool::SelectText;

    assert!(app.selection.is_empty());
    app.select_all_current_page();

    assert!(app.selection.has_text());
    assert_eq!(app.selection.page_index, Some(0));
    assert_eq!(app.selection.selected_text_indices.len(), 3);
    let selected = app.selection.selected_text.as_ref().unwrap();
    assert!(selected.contains("Header Section 2026"));
    assert!(selected.contains("Body paragraph content"));
    assert!(selected.contains("Footer note page 1"));
}

#[test]
fn test_e2e_platform_tooltip_strings() {
    let copy_str = standard_copy_shortcut_str();
    let select_all_str = standard_select_all_shortcut_str();

    #[cfg(target_os = "macos")]
    {
        assert_eq!(copy_str, "⌘C");
        assert_eq!(select_all_str, "⌘A");
    }

    #[cfg(not(target_os = "macos"))]
    {
        assert!(copy_str.contains("Ctrl+C"));
        assert!(copy_str.contains("Ctrl+Ins"));
        assert_eq!(select_all_str, "Ctrl+A");
    }
}

#[test]
fn test_e2e_keyboard_focus_guard() {
    let mut builder = kestrel_core::synthetic::SyntheticPdfBuilder::new();
    let p_idx = builder.add_page(595.28, 841.89, 0);
    builder.add_text(p_idx, "Protected Text Sample", 50.0, 750.0, 12.0, [0, 0, 0]);

    let bytes = builder.build().expect("Build synthetic PDF");
    let mut app = KestrelApp::default();
    app.load_document_bytes(bytes, Some("focus_test.pdf".to_string()));
    app.active_tool = ActiveTool::SelectText;

    let ctx = Context::default();

    // Set selection
    app.selection.page_index = Some(0);
    app.selection.selected_text_indices = vec![0];
    app.selection.selected_text = Some("Protected Text Sample".to_string());

    // When text input is not focused and Ctrl+C is pressed, copy triggers
    let raw = egui::RawInput {
        modifiers: egui::Modifiers {
            ctrl: true,
            ..Default::default()
        },
        events: vec![egui::Event::Key {
            key: egui::Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
        }],
        ..Default::default()
    };
    let _ = ctx.run(raw, |ctx| {
        app.render_ui(ctx);
    });

    assert!(app.status_toast.is_some());
    assert!(app.status_toast.as_ref().unwrap().contains("Copied"));

    // Now simulate keyboard focus on a text edit widget
    app.status_toast = None;
    let text_edit_id = egui::Id::new("mock_focused_input");
    ctx.memory_mut(|mem| mem.request_focus(text_edit_id));
    assert!(
        ctx.wants_keyboard_input(),
        "Context must report keyboard input wanted when focus is requested"
    );

    let raw_while_focused = egui::RawInput {
        modifiers: egui::Modifiers {
            ctrl: true,
            ..Default::default()
        },
        events: vec![egui::Event::Key {
            key: egui::Key::C,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers {
                ctrl: true,
                ..Default::default()
            },
        }],
        ..Default::default()
    };
    let _ = ctx.run(raw_while_focused, |ctx| {
        app.render_ui(ctx);
    });

    assert!(
        app.status_toast.is_none(),
        "Copying document text must NOT trigger when keyboard input is focused by an active input widget"
    );
}

#[test]
fn test_e2e_form_xobject_and_rotated_signature_rendering_smoke() {
    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Helvetica",
    });

    // 1. Right-margin Form XObject with vertical verification code
    let form_content = b"BT /F1 9 Tf 0 1 -1 0 15 50 Tm (CSV-VERIFICATION-CODE-98765) Tj ET";
    let form_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 30.into(), 300.into()],
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => font_id,
                },
            },
        },
        form_content.to_vec(),
    ));

    // 2. Electronic signature appearance stream with nested Form XObject and text
    let n2_content = b"BT /F1 8 Tf 10 200 Td (Verified Digital Signer Identity) Tj ET";
    let n2_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 230.into(), 230.into()],
            "Resources" => dictionary! {
                "Font" => dictionary! {
                    "F1" => font_id,
                },
            },
        },
        n2_content.to_vec(),
    ));

    let frm_content = b"0 1 -1 0 230 0 cm /n2 Do";
    let frm_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 230.into(), 230.into()],
            "Resources" => dictionary! {
                "XObject" => dictionary! {
                    "n2" => n2_id,
                },
            },
        },
        frm_content.to_vec(),
    ));

    let ap_sig_id = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject",
            "Subtype" => "Form",
            "BBox" => vec![0.into(), 0.into(), 30.into(), 230.into()],
            "Resources" => dictionary! {
                "XObject" => dictionary! {
                    "FRM" => frm_id,
                },
            },
        },
        b"/FRM Do".to_vec(),
    ));

    let sig_annot_id = doc.add_object(dictionary! {
        "Type" => "Annot",
        "Subtype" => "Widget",
        "FT" => "Sig",
        "Rect" => vec![15.into(), 400.into(), 45.into(), 630.into()],
        "AP" => dictionary! {
            "N" => ap_sig_id,
        },
    });

    let page_content = b"BT /F1 12 Tf 50 700 Td (Document Content Main Body) Tj ET /FormRight Do";
    let content_id = doc.add_object(Stream::new(dictionary! {}, page_content.to_vec()));

    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Contents" => content_id,
        "Annots" => vec![sig_annot_id.into()],
        "Resources" => dictionary! {
            "Font" => dictionary! {
                "F1" => font_id,
            },
            "XObject" => dictionary! {
                "FormRight" => form_id,
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

    let mut buf = Vec::new();
    doc.save_to(&mut buf).expect("Save synthetic document");

    let mut app = KestrelApp::default();
    app.load_document_bytes(buf, Some("official_notice_smoke.pdf".to_string()));
    assert!(app.session.is_some(), "Session must be initialized");

    let ctx = Context::default();
    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(1024.0, 768.0),
        )),
        ..Default::default()
    };
    let out = ctx.run(raw_input, |ctx| {
        app.render_ui(ctx);
    });

    let rendered_texts = extract_all_text_from_shapes(&out.shapes);
    assert!(
        rendered_texts
            .iter()
            .any(|t| t.contains("Document Content Main Body")),
        "Main body text must be rendered in UI"
    );
    assert!(
        rendered_texts
            .iter()
            .any(|t| t.contains("CSV-VERIFICATION-CODE-98765")),
        "Right-margin Form XObject verification code must be rendered in UI"
    );
    assert!(
        rendered_texts
            .iter()
            .any(|t| t.contains("Verified Digital Signer Identity")),
        "Left-margin digital signature appearance text must be rendered in UI"
    );
}

#[test]
fn test_e2e_page_navigator_text_input_jump_and_steppers() {
    // 1. Empty state verification: prominent CTA and closed sidebar
    let mut app = KestrelApp::default();
    assert!(
        !app.sidebar_open,
        "Sidebar must be closed by default for focused reading"
    );
    assert_eq!(app.page_input_text, "1");

    let ctx = Context::default();
    let out_empty = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let texts_empty = extract_all_text_from_shapes(&out_empty.shapes);
    assert!(
        texts_empty.iter().any(|t| t.contains("Abrir fichero")),
        "Empty state must display prominent Abrir fichero CTA button"
    );
    assert!(
        texts_empty.iter().any(|t| t.contains("Kestrel-PDF")),
        "Empty state must display Kestrel-PDF branding"
    );

    // 2. Load 4-page synthetic document
    let pdf_bytes = generate_synthetic_visual_showcase_pdf();
    app.load_document_bytes(pdf_bytes, Some("test_nav_showcase.pdf".to_string()));
    assert_eq!(app.total_pages, 4);
    assert_eq!(app.current_page, 1);
    assert_eq!(app.page_input_text, "1");
    assert!(
        !app.sidebar_open,
        "Sidebar remains closed when opening document"
    );

    // 3. Render loaded frame: Page Navigator displays input [ 1 ] and denominator / 4
    let out_loaded = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let texts_loaded = extract_all_text_from_shapes(&out_loaded.shapes);
    assert!(
        texts_loaded.iter().any(|t| t.contains("/ 4")),
        "Navigator must display total pages denominator '/ 4'"
    );
    assert!(
        texts_loaded.iter().any(|t| t.contains("Prev")),
        "Navigator must display Prev stepper"
    );
    assert!(
        texts_loaded.iter().any(|t| t.contains("Next")),
        "Navigator must display Next stepper"
    );

    // 4. Test direct jump via text input (typing "3")
    app.set_current_page(3);
    assert_eq!(app.current_page, 3);
    assert_eq!(app.page_input_text, "3");

    // 5. Test Stepper Next to page 4
    app.set_current_page(app.current_page + 1);
    assert_eq!(app.current_page, 4);
    assert_eq!(app.page_input_text, "4");

    // 6. Test clamping on overflow (typing page 999 -> clamped to 4)
    app.set_current_page(999);
    assert_eq!(app.current_page, 4, "Page must clamp to total_pages (4)");
    assert_eq!(app.page_input_text, "4");

    // 7. Test clamping on underflow (typing page 0 -> clamped to 1)
    app.set_current_page(0);
    assert_eq!(app.current_page, 1, "Page must clamp to minimum (1)");
    assert_eq!(app.page_input_text, "1");

    // 8. Re-render UI frame on page 1
    let out_p1 = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let texts_p1 = extract_all_text_from_shapes(&out_p1.shapes);
    assert!(
        texts_p1.iter().any(|t| t.contains("Abrir fichero")),
        "Header bar must retain prominent Abrir fichero button"
    );
    assert!(
        texts_p1.iter().any(|t| t.contains("/ 4")),
        "Navigator denominator must remain '/ 4'"
    );
}

#[test]
fn test_e2e_modern_design_system_tokens_and_visual_consistency() {
    use kestrel_app::Theme;

    let mut app = KestrelApp::default();
    let ctx = Context::default();

    // 1. Empty state frame: Check warm salmon CTA and slate dark panels
    let empty_out = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    assert!(
        has_rect_with_fill(&empty_out.shapes, Theme::ACCENT_SALMON),
        "Empty state must render large Warm Salmon (#D97757) Open File CTA"
    );
    assert!(
        has_rect_with_fill(&empty_out.shapes, Theme::PANEL_DARK),
        "Header must render Slate 900 (#0F172A) panel fill"
    );

    // 2. Loaded document state: Check Ribbon, Actions, and Canvas
    let pdf_bytes = generate_synthetic_visual_showcase_pdf();
    app.load_document_bytes(pdf_bytes, Some("modern_theme_doc.pdf".to_string()));
    let loaded_out = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    assert!(
        has_rect_with_fill(&loaded_out.shapes, Theme::ACCENT_SALMON),
        "Loaded toolbar must contain Warm Salmon accent for active tool / primary button"
    );
    assert!(
        has_rect_with_fill(&loaded_out.shapes, Theme::ACCENT_OCHRE),
        "Header must contain Warm Ochre accent for Save / Export button"
    );
    assert!(
        has_rect_with_fill(&loaded_out.shapes, Theme::CANVAS_BACKDROP),
        "Central panel must render Slate 700 canvas backdrop surround"
    );

    // 3. Status Toast Notification: Check status bar presence
    app.status_toast = Some("Document loaded successfully with 4 pages.".to_string());
    let toast_out = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let toast_texts = extract_all_text_from_shapes(&toast_out.shapes);
    assert!(
        toast_texts
            .iter()
            .any(|t| t.contains("Document loaded successfully")),
        "Toast message must be rendered"
    );
    assert!(
        toast_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::INFO)),
        "Toast must render information badge icon"
    );

    // 4. Left Sidebar: Open and cycle tabs, checking themed cards
    app.sidebar_open = true;
    app.sidebar_tab = SidebarTab::Thumbnails;
    let sidebar_out = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let sidebar_texts = extract_all_text_from_shapes(&sidebar_out.shapes);
    assert!(
        sidebar_texts.iter().any(|t| t.contains("Page 1")),
        "Thumbnails sidebar tab must render page items"
    );

    // 5. Signature Modal: Open and verify themed action buttons
    app.signature_modal_open = true;
    let modal_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280.0, 800.0),
        )),
        ..Default::default()
    };
    let _ = ctx.run(modal_input.clone(), |ctx| {
        app.render_ui(ctx);
    });
    let modal_out = ctx.run(modal_input, |ctx| {
        app.render_ui(ctx);
    });
    let modal_texts = extract_all_text_from_shapes(&modal_out.shapes);
    assert!(
        modal_texts.iter().any(|t| t.contains("Sign Contract")),
        "Signature modal window title must be rendered"
    );
    assert!(
        modal_texts
            .iter()
            .any(|t| t.contains("Adopt & Place Signature")),
        "Adopt button must be present in signature modal"
    );
}

#[test]
fn test_e2e_phosphor_icon_font_glyphs_and_typography() {
    let mut app = KestrelApp::default();
    let ctx = egui::Context::default();

    // 1. Initial empty state: Header brand icon and open file icon rendered
    let out = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let texts = extract_all_text_from_shapes(&out.shapes);
    assert!(
        texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::APP_LOGO)),
        "Header or welcome card must render APP_LOGO vector icon"
    );
    assert!(
        texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::OPEN_FILE)),
        "Header or welcome card must render OPEN_FILE vector icon"
    );

    // 2. Load synthetic document: Action ribbon icons rendered
    let doc_bytes = generate_synthetic_visual_showcase_pdf();
    app.load_document_bytes(doc_bytes, Some("icons_test.pdf".to_string()));

    let out_doc = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });
    let doc_texts = extract_all_text_from_shapes(&out_doc.shapes);

    // Navigation and rotation icons
    assert!(
        doc_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::PREV_PAGE)),
        "Navigator must render PREV_PAGE icon"
    );
    assert!(
        doc_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::NEXT_PAGE)),
        "Navigator must render NEXT_PAGE icon"
    );
    assert!(
        doc_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::ROTATE_CCW)),
        "Ribbon must render ROTATE_CCW icon"
    );
    assert!(
        doc_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::ROTATE_CW)),
        "Ribbon must render ROTATE_CW icon"
    );

    // Interactive tool icons
    assert!(
        doc_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::TOOL_PAN)),
        "Ribbon must render TOOL_PAN icon"
    );
    assert!(
        doc_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::TOOL_SELECT)),
        "Ribbon must render TOOL_SELECT icon"
    );
    assert!(
        doc_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::TOOL_FORMS)),
        "Ribbon must render TOOL_FORMS icon"
    );
    assert!(
        doc_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::TOOL_EDIT_TEXT)),
        "Ribbon must render TOOL_EDIT_TEXT icon"
    );
    assert!(
        doc_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::TOOL_SIGN)),
        "Ribbon must render TOOL_SIGN icon"
    );
    assert!(
        doc_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::TOOL_REDACT)),
        "Ribbon must render TOOL_REDACT icon"
    );

    // Zoom steppers
    assert!(
        doc_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::ZOOM_IN)),
        "Zoom controls must render ZOOM_IN icon"
    );
    assert!(
        doc_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::ZOOM_OUT)),
        "Zoom controls must render ZOOM_OUT icon"
    );

    // 3. Signature modal: Modal action icons
    app.signature_modal_open = true;
    let modal_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1280.0, 800.0),
        )),
        ..Default::default()
    };
    let _ = ctx.run(modal_input.clone(), |ctx| {
        app.render_ui(ctx);
    });
    let modal_out = ctx.run(modal_input, |ctx| {
        app.render_ui(ctx);
    });
    let modal_texts = extract_all_text_from_shapes(&modal_out.shapes);
    assert!(
        modal_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::TRASH)),
        "Signature modal must render TRASH icon"
    );
    assert!(
        modal_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::UNDO)),
        "Signature modal must render UNDO icon"
    );

    // 4. Verify absence of legacy broken emoji codepoints
    let legacy_emojis = [
        "🦅", "📂", "💾", "✍️", "✋", "📝", "📋", "✏️", "🛡️", "➕", "➖", "🔍", "🔒", "🗑",
    ];
    for emoji in legacy_emojis {
        assert!(
            !doc_texts.iter().any(|t| t.contains(emoji)),
            "Legacy emoji '{}' should not be present in document UI text shapes",
            emoji
        );
        assert!(
            !modal_texts.iter().any(|t| t.contains(emoji)),
            "Legacy emoji '{}' should not be present in modal UI text shapes",
            emoji
        );
    }
}

#[test]
fn test_e2e_responsive_toolbar_modes_at_different_viewport_widths() {
    let mut app = KestrelApp::default();
    let doc_bytes = generate_synthetic_visual_showcase_pdf();
    let test_filename = "enterprise_quarterly_financial_report_audit_signed.pdf";
    app.load_document_bytes(doc_bytes, Some(test_filename.to_string()));

    // 1. Wide Viewport (1400px width): Full labels rendered
    let ctx_wide = egui::Context::default();
    let wide_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1400.0, 900.0),
        )),
        ..Default::default()
    };
    let wide_out = ctx_wide.run(wide_input, |ctx| {
        app.render_ui(ctx);
    });
    let wide_texts = extract_all_text_from_shapes(&wide_out.shapes);
    assert!(
        wide_texts.iter().any(|t| t.contains("Prev")),
        "Wide viewport must render 'Prev' in page navigator"
    );
    assert!(
        wide_texts.iter().any(|t| t.contains("Next")),
        "Wide viewport must render 'Next' in page navigator"
    );
    assert!(
        wide_texts.iter().any(|t| t.contains("Sign Contract")),
        "Wide viewport must render 'Sign Contract' in tool ribbon"
    );
    assert!(
        wide_texts.iter().any(|t| t.contains("Fit Page")),
        "Wide viewport must render 'Fit Page' in zoom cluster"
    );
    assert!(
        wide_texts.iter().any(|t| t.contains("Fit Width")),
        "Wide viewport must render 'Fit Width' in zoom cluster"
    );
    assert!(
        wide_texts.iter().any(|t| t.contains("Abrir fichero")),
        "Wide viewport must render 'Abrir fichero' in header CTA"
    );
    assert!(
        wide_texts.iter().any(|t| t.contains("Save / Export")),
        "Wide viewport must render 'Save / Export' in header CTA"
    );

    // 2. Medium Viewport (900px width): Compact labels rendered
    let ctx_med = egui::Context::default();
    let med_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(900.0, 700.0),
        )),
        ..Default::default()
    };
    let med_out = ctx_med.run(med_input, |ctx| {
        app.render_ui(ctx);
    });
    let med_texts = extract_all_text_from_shapes(&med_out.shapes);
    assert!(
        med_texts.iter().any(|t| t.contains("Sign")),
        "Medium viewport must render 'Sign' in tool ribbon"
    );
    assert!(
        med_texts.iter().any(|t| t.contains("Fit")),
        "Medium viewport must render 'Fit' in zoom cluster"
    );

    // 3. Compact / Half-Screen Viewport (680px width): Iconic mode rendered
    let ctx_compact = egui::Context::default();
    let compact_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(680.0, 600.0),
        )),
        ..Default::default()
    };
    let compact_out = ctx_compact.run(compact_input, |ctx| {
        app.render_ui(ctx);
    });
    let compact_texts = extract_all_text_from_shapes(&compact_out.shapes);

    // All tool icons must be present even in compact mode
    assert!(
        compact_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::TOOL_PAN)),
        "Compact viewport must render TOOL_PAN icon"
    );
    assert!(
        compact_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::TOOL_SELECT)),
        "Compact viewport must render TOOL_SELECT icon"
    );
    assert!(
        compact_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::TOOL_SIGN)),
        "Compact viewport must render TOOL_SIGN icon"
    );
    assert!(
        compact_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::TOOL_FORMS)),
        "Compact viewport must render TOOL_FORMS icon"
    );
    assert!(
        compact_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::TOOL_EDIT_TEXT)),
        "Compact viewport must render TOOL_EDIT_TEXT icon"
    );
    assert!(
        compact_texts
            .iter()
            .any(|t| t.contains(kestrel_app::icons::TOOL_REDACT)),
        "Compact viewport must render TOOL_REDACT icon"
    );
    assert!(
        compact_texts.iter().any(|t| t.contains("Abrir")),
        "Compact viewport must render compact 'Abrir' CTA"
    );
    assert!(
        compact_texts.iter().any(|t| t.contains("Save")),
        "Compact viewport must render compact 'Save' CTA"
    );
}

#[test]
fn test_clamp_rgba_image_to_max_side_and_texture_safety() {
    // 1. Image already within bounds -> returns borrowed Cow with original dimensions
    let small_rgba = vec![255u8; 100 * 50 * 4];
    let (w, h, cow) = clamp_rgba_image_to_max_side(100, 50, &small_rgba, 2048);
    assert_eq!(w, 100);
    assert_eq!(h, 50);
    assert!(matches!(cow, std::borrow::Cow::Borrowed(_)));

    // 2. High-resolution tile (e.g. 2976 x 4209 at 500% zoom) exceeding 2048 max texture side
    let large_w = 2976;
    let large_h = 4209;
    let large_rgba = vec![128u8; large_w * large_h * 4];
    let (clamped_w, clamped_h, clamped_cow) =
        clamp_rgba_image_to_max_side(large_w, large_h, &large_rgba, 2048);

    assert!(clamped_w <= 2048, "Width {} must be <= 2048", clamped_w);
    assert!(clamped_h <= 2048, "Height {} must be <= 2048", clamped_h);
    assert_eq!(clamped_h, 2048);
    assert_eq!(clamped_w, 1448);
    assert_eq!(clamped_cow.len(), clamped_w * clamped_h * 4);
    assert!(matches!(clamped_cow, std::borrow::Cow::Owned(_)));

    // 3. Confirm loading into egui Context with 2048 limit does not panic
    let ctx = egui::Context::default();
    let img = egui::ColorImage::from_rgba_unmultiplied([clamped_w, clamped_h], &clamped_cow);
    let handle = ctx.load_texture("safe_clamped_tile", img, egui::TextureOptions::LINEAR);
    assert_eq!(handle.size(), [clamped_w, clamped_h]);
}

#[test]
fn test_e2e_search_navigation_scroll_and_page_synchronization() {
    let pdf_bytes = generate_synthetic_search_corpus_pdf();
    let mut app = KestrelApp::default();
    app.load_document_bytes(pdf_bytes, Some("search_corpus.pdf".to_string()));
    assert_eq!(app.total_pages, 3);
    assert_eq!(app.current_page, 1);

    // 1. Execute search query for token on Page 3
    app.search_query = "GAMMA_IBAN_SPANISH_ES91".to_string();
    app.execute_search();

    assert_eq!(app.search_results.len(), 1);
    assert_eq!(app.sidebar_tab, SidebarTab::SearchResults);
    assert!(app.sidebar_open);

    // Initial execute_search should have selected result 0 and scheduled navigation
    assert_eq!(app.selected_search_result, Some(0));
    assert_eq!(app.current_page, 3);
    assert_eq!(app.page_input_text, "3");
    assert_eq!(app.scroll_to_page, Some(2));
    assert!(app.scroll_to_search_match);

    // 2. Render frame with egui Context
    let ctx = Context::default();
    let _out = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });

    // After rendering, scroll target must have been consumed
    assert_eq!(app.scroll_to_page, None);
    assert!(!app.scroll_to_search_match);

    // 3. Test next/previous search result navigation cycling
    app.search_query = "ALPHA".to_string();
    app.execute_search();
    assert!(!app.search_results.is_empty());
    assert_eq!(app.selected_search_result, Some(0));

    // Navigate to next
    app.next_search_result();
    assert!(app.selected_search_result.is_some());

    // Navigate to previous
    app.prev_search_result();
    assert_eq!(app.selected_search_result, Some(0));
}

#[test]
fn test_e2e_exact_word_search_highlight_bounds_not_whole_paragraph() {
    let pdf_bytes = generate_synthetic_search_corpus_pdf();
    let mut app = KestrelApp::default();
    app.load_document_bytes(pdf_bytes, Some("search_corpus.pdf".to_string()));

    // Target a specific keyword in a long sentence on Page 1:
    // Sentence: "Unique token for query verification: ALPHA_SEARCH_TOKEN_42." (58 chars, font_size 11)
    // Query: "ALPHA_SEARCH_TOKEN_42" (21 chars)
    app.search_query = "ALPHA_SEARCH_TOKEN_42".to_string();
    app.execute_search();

    let ctx = Context::default();
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        app.render_ui(ctx);
    });

    // Extract all rectangles with the search highlight color
    let highlight_rects = find_rects_with_fill(&output.shapes, Theme::SEARCH_HIGHLIGHT_ACTIVE);
    assert!(
        !highlight_rects.is_empty(),
        "Must render active search highlight rectangle"
    );

    let hl_rect = highlight_rects[0];
    let hl_width = hl_rect.width();

    // The whole line is ~58 chars long (> 320px wide).
    // The exact word "ALPHA_SEARCH_TOKEN_42" is 21 chars long (~120-180px wide).
    // With whole paragraph bloat, hl_width was > 330px.
    assert!(
        hl_width < 250.0,
        "Highlight width ({}) must be restricted to the exact word and strictly smaller than whole paragraph (> 330px)",
        hl_width
    );
    assert!(
        hl_width > 50.0,
        "Highlight width ({}) must cover the searched keyword",
        hl_width
    );
}
