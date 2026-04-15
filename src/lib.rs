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

    #[wasm_bindgen(js_name = loadModel)]
    pub fn load_model(&mut self, ply_data: &[u8]) -> Result<(), JsValue> {
        self.internal
            .load_model(ply_data)
            .map_err(|e| JsValue::from_str(&e))
    }

    pub fn render(&mut self) {
        if let Err(e) = self.internal.render() {
            web_sys::console::error_1(&JsValue::from_str(&e));
        }
    }
}
