//! Raw Tauri event-listener binding.
//!
//! This is the **only** module that calls into the Tauri frontend listen API
//! (`window.__TAURI__.event.listen`). All other code in the UI subscribes
//! through [`crate::events::service::EventService`] instead.
//!
//! The bridge lives in `src-tauri/src/event_bridge.rs` and broadcasts every
//! platform event on a single channel — [`FRONTEND_EVENT_CHANNEL`] (`"nabu-event"`) —
//! as a `FrontendEvent` envelope (`{ event_type, timestamp, payload }`). This
//! module installs a listener on that channel and turns each raw JS event into
//! a typed [`crate::events::types::FrontendEvent`].

use js_sys::{Reflect, JSON};
use wasm_bindgen::prelude::*;

use crate::events::types::{parse_raw, EventError, FrontendEvent, RawFrontendEvent};

/// Raw binding to Tauri's global event API: `window.__TAURI__.event.listen`.
///
/// `listen(event, cb)` returns a `Promise` that resolves to an "unlisten"
/// function used to detach the listener. Mirrors the `__TAURI__.core.invoke`
/// pattern used in [`crate::ipc`].
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = "listen")]
    pub fn tauri_event_listen(event: &str, cb: &js_sys::Function) -> js_sys::Promise;
}

/// Returns `true` when the Tauri global event API is available on this window.
///
/// This is `false` when the UI is served outside of Tauri (e.g. `cargo dioxus
/// serve` on the dev server, or unit tests). The listener is only installed
/// when Tauri is present, so the rest of the app keeps working in a plain
/// browser without panicking.
pub fn tauri_available() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let key = JsValue::from_str("__TAURI__");
    match Reflect::get(&window, &key) {
        Ok(tauri) => !tauri.is_undefined() && !tauri.is_null(),
        Err(_) => false,
    }
}

/// Read the `payload` field from a Tauri listener event object.
///
/// Tauri dispatches `{ event, payload, id }` to `listen` callbacks. The
/// `payload` carries the envelope emitted by the backend bridge. Returns
/// `None` when the field is absent (malformed event).
pub(crate) fn extract_payload<'a>(event: &'a JsValue) -> Option<JsValue> {
    let key = JsValue::from_str("payload");
    Reflect::get(event, &key).ok()
}

/// Normalize a JS event object's envelope into a typed [`FrontendEvent`].
///
/// Handles both representations Tauri may deliver:
///
/// * `payload` as a JS **object** (the common case — `emit_str` embeds the
///   envelope JSON as an object literal), and
/// * `payload` as a raw JSON **string** (some Tauri versions / emission paths
///   hand the string through verbatim).
///
/// Either way the envelope is reduced to a JSON string and parsed with
/// `serde_json` — robust, allocation-light, and independent of the
/// `serde-wasm-bindgen` value quirks.
pub(crate) fn parse_event(event: &JsValue) -> Result<FrontendEvent, EventError> {
    let payload = extract_payload(event)
        .ok_or_else(|| EventError::PayloadExtraction("event has no `payload` field".into()))?;

    // Reduce the JS value to a JSON string once, then deserialize from it.
    let json = if let Some(s) = payload.as_string() {
        s
    } else if payload.is_object() {
        let s = JSON::stringify(&payload)
            .map_err(|e| EventError::MalformedPayload(format!("JSON.stringify payload: {e:?}")))?;
        s.as_string()
            .ok_or_else(|| EventError::MalformedPayload("stringified payload lost".into()))?
    } else {
        // Anything else (null, undefined, number, …) is not a valid envelope.
        return Err(EventError::MalformedPayload(format!(
            "payload is not a JSON object or string (got {})",
            if payload.is_null() { "null" } else { "non-object/non-string" }
        )));
    };

    let raw: RawFrontendEvent = serde_json::from_str(&json)
        .map_err(|e| EventError::MalformedPayload(format!("envelope: {e}")))?;

    parse_raw(raw)
}

// NOTE: `parse_event` operates on raw `JsValue`s, which can only be constructed
// meaningfully inside a JavaScript runtime (i.e. a `wasm-bindgen-test`). The
// pure-Rust deserialization path (`parse_raw`) is fully unit-tested in
// [`crate::events::types`], which exercises the exact same envelope shape the
// live `listen` callback delivers.
