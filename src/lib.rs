mod renderer;

use crate::renderer::GsRenderer;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct WasmRenderer {
    internal: GsRenderer,
}

#[wasm_bindgen]
impl WasmRenderer {
    pub async fn create(canvas: HtmlCanvasElement) -> Result<WasmRenderer, JsValue> {
        let renderer = GsRenderer::new(canvas)
            .await
            .map_err(|e| JsValue::from_str(&e))?;

        Ok(WasmRenderer { internal: renderer })
    }
}
