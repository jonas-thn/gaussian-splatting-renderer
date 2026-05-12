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

    #[wasm_bindgen(js_name = resize)]
    pub fn resize(&mut self, width: u32, height: u32) {
        self.internal.resize(width, height);
    }

    #[wasm_bindgen(js_name = moveCamera)]
    pub fn move_camera(&mut self, forward: f32, right: f32, up: f32, pitch: f32, yaw: f32) {
        self.internal.move_camera(forward, right, up, pitch, yaw);
    }

    #[wasm_bindgen(js_name = setCamera)]
    pub fn set_camera(&mut self, px: f32, py: f32, pz: f32, qx: f32, qy: f32, qz: f32, qw: f32) {
        self.internal.set_camera_with_threshold(px, py, pz, qx, qy, qz, qw);
    }

    #[wasm_bindgen(js_name = setModelTransform)]
    pub fn set_model_transform(&mut self, px: f32, py: f32, pz: f32, qx: f32, qy: f32, qz: f32, qw: f32, scale: f32) {
        self.internal.set_model_transform(px, py, pz, qx, qy, qz, qw, scale);
    }

    #[wasm_bindgen(js_name = setStereoCameras)]
    pub fn set_stereo_cameras(
        &mut self, 
        left_view: &[f32], left_proj: &[f32],
        right_view: &[f32], right_proj: &[f32]
    )
    {
        if let (Ok(lv), Ok(lp), Ok(rv), Ok(rp)) = (
            left_view.try_into(), left_proj.try_into(),
            right_view.try_into(), right_proj.try_into()
        ) {
            self.internal.set_stereo_cameras(&lv, &lp, &rv, &rp);
        } else {
            web_sys::console::warn_1(&JsValue::from_str("Invalid matrix size passed to setStereoCameras"));
        }
    }

    #[wasm_bindgen(js_name = renderStereo)]
    pub fn render_stereo(&mut self) {
        if let Err(e) = self.internal.render_stereo() {
            web_sys::console::error_1(&JsValue::from_str(&e));
        }
    }
}
