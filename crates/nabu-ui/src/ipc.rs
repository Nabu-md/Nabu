use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    /// Raw Tauri `invoke` — declared as a synchronous function returning a
    /// `js_sys::Promise` so that callers can decide whether to unwrap or
    /// gracefully handle rejection via [`JsFuture`].
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    fn invoke(cmd: &str, args: JsValue) -> js_sys::Promise;
}

/// Raw Tauri IPC invoke. Returns the `JsValue` result on success.
/// Panics on rejection (caught by `console_error_panic_hook` in release builds).
pub async fn tauri_invoke(cmd: &str, args: JsValue) -> JsValue {
    JsFuture::from(invoke(cmd, args))
        .await
        .unwrap()
}

/// Like [`tauri_invoke`] but catches promise rejections and returns `None`
/// instead of panicking.  Useful for soft-fail probes where a missing or
/// errored command should not crash the renderer.
pub async fn tauri_invoke_safe(cmd: &str, args: JsValue) -> Option<JsValue> {
    match JsFuture::from(invoke(cmd, args)).await {
        Ok(val) => Some(val),
        Err(_) => None,
    }
}
