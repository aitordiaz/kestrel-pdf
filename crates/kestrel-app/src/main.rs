#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Kestrel-PDF")
            .with_inner_size([1280.0, 840.0])
            .with_min_inner_size([640.0, 480.0])
            .with_active(true),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };

    eframe::run_native(
        "Kestrel-PDF",
        native_options,
        Box::new(|_cc| Ok(Box::new(kestrel_app::KestrelApp::default()))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {}
