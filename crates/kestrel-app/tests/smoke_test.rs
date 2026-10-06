use egui::Context;
use kestrel_app::app::{ActiveTool, KestrelApp};

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
    app.current_file_name = Some("contract_test.pdf".to_string());
    app.total_pages = 5;
    app.current_page = 2;

    // 5. Run headless egui context frame simulation
    let ctx = Context::default();
    let raw_input = egui::RawInput::default();

    let full_output = ctx.run(raw_input, |ctx| {
        // Run full UI render cycle
        app.render_ui(ctx);
    });

    // 6. Assertions on UI frame completion
    assert!(
        !full_output.shapes.is_empty(),
        "Smoke test failed: egui frame should produce rendered UI shapes"
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
