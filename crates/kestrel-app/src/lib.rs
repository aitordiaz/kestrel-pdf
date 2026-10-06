pub mod app;

pub use app::KestrelApp;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();

    let web_options = eframe::WebOptions::default();
    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "kestrel_canvas",
                web_options,
                Box::new(|_cc| Ok(Box::new(KestrelApp::default()))),
            )
            .await
            .expect("Failed to start KestrelApp in WebAssembly");
    });

    Ok(())
}
