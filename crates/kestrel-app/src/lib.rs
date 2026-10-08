pub mod app;
pub mod icons;
pub mod theme;

pub use app::KestrelApp;
pub use theme::Theme;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let document = web_sys::window()
        .ok_or_else(|| JsValue::from_str("No global window object found"))?
        .document()
        .ok_or_else(|| JsValue::from_str("No document object found"))?;

    let canvas = document
        .get_element_by_id("kestrel_canvas")
        .ok_or_else(|| JsValue::from_str("Canvas element 'kestrel_canvas' not found in DOM"))?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    let web_options = eframe::WebOptions::default();
    wasm_bindgen_futures::spawn_local(async move {
        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|_cc| Ok(Box::new(KestrelApp::default()))),
            )
            .await
            .expect("Failed to start KestrelApp in WebAssembly");
    });

    Ok(())
}
