//! # Session persistence — frontend helpers (Dioxus migration)
//!
//! Bridges the frontend to the backend session + crash-recovery commands
//! (`session_save` / `session_load` / `session_clear` / `recovery_check` /
//! `recovery_discard`).
//!
//! Changes from LePtOS: `leptos::prelude::*` → `dioxus::prelude::*`,
//! `Callback::run` → `Callback::call`, `RwSignal` → `Signal`.

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

/// Mirrors the backend `SessionState` (only the fields the UI uses).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionState {
    pub version: u32,
    #[serde(default)]
    pub saved_at: Option<String>,
    #[serde(default)]
    pub view_mode: Option<String>,
    #[serde(default)]
    pub active_note: Option<String>,
    #[serde(default)]
    pub open_tabs: Vec<String>,
    #[serde(default)]
    pub split_panes: Vec<String>,
    #[serde(default)]
    pub cursor_pos: Option<u32>,
    #[serde(default)]
    pub scroll_top: Option<u32>,
    #[serde(default)]
    pub left_sidebar: Option<bool>,
    #[serde(default)]
    pub right_inspector: Option<bool>,
    #[serde(default)]
    pub window_layout: Option<String>,
}

/// Mirrors the backend `RecoveryStatus`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryStatus {
    pub crashed: bool,
    pub has_session: bool,
    pub session: Option<SessionState>,
}

/// Persists the current session to the backend.
pub fn session_save(state: &SessionState) {
    let args = serde_wasm_bindgen::to_value(state).unwrap();
    spawn_local(async move {
        let _ = crate::ipc::tauri_invoke("session_save", args).await;
    });
}

/// Loads the persisted session (callback receives the result).
pub fn session_load(on_result: Callback<Option<SessionState>>) {
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("session_load", empty_args).await;
        let parsed = serde_wasm_bindgen::from_value::<Option<SessionState>>(result)
            .ok()
            .flatten();
        on_result.call(parsed);
    });
}

/// Clears the persisted session (e.g. after a fresh launch restored it).
pub fn session_clear() {
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let _ = crate::ipc::tauri_invoke("session_clear", empty_args).await;
    });
}

/// Checks whether the previous run crashed and whether a session is available.
pub fn recovery_check(on_result: Callback<RecoveryStatus>) {
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("recovery_check", empty_args).await;
        let status =
            serde_wasm_bindgen::from_value::<RecoveryStatus>(result).unwrap_or(RecoveryStatus {
                crashed: false,
                has_session: false,
                session: None,
            });
        on_result.call(status);
    });
}

/// Clears the recovery-pending marker after the user restored / discarded.
pub fn recovery_discard() {
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let _ = crate::ipc::tauri_invoke("recovery_discard", empty_args).await;
    });
}
