//! # Universal Undo/Redo — Frontend Integration (Dioxus)
//!
//! Bridges the shared component library toast system to the backend
//! [`HistoryManager`] over IPC. Provides:
//!
//! - a [`HistoryContext`] exposing `can_undo` / `can_redo` signals
//! - [`Undo`] / [`Redo`] helpers that run the backend action and show toast
//!   feedback ("Undid X", "Nothing to undo", "Nothing to redo")
//! - global keyboard shortcuts: Cmd/Ctrl+Z (undo), Cmd/Ctrl+Shift+Z and
//!   Ctrl+Y (redo)
//!
//! While focus is inside an editable element (`textarea`, `input`,
//! `contenteditable`), the shortcuts are intentionally **not** intercepted so
//! the native editing history (CodeMirror / textarea) is preserved.
//!
//! ## Reactivity note
//!
//! Both [`HistoryContext`] and `ToastContext` are `Copy` and are captured
//! **at render time** in [`HistoryProvider`](crate::components::contexts::HistoryProvider).
//! They are then threaded into async tasks and the window-level keydown listener
//! as plain values — never via `expect_context` inside a `spawn_local` future
//! or a raw DOM callback, which have no reactive owner.

use crate::components::ui::feedback::{use_toast, ToastContext};
use dioxus::prelude::*;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

/// Backend status projection used to drive button disabled states.
#[derive(serde::Deserialize, Clone, Debug)]
pub struct HistoryStatus {
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_label: Option<String>,
    pub redo_label: Option<String>,
    pub undo_len: usize,
    pub redo_len: usize,
    pub max_depth: usize,
}

/// Shared history context provided at the app root.
#[derive(Clone, Copy)]
pub struct HistoryContext {
    /// Whether an undo is currently possible.
    pub can_undo: Signal<bool>,
    /// Whether a redo is currently possible.
    pub can_redo: Signal<bool>,
}

impl HistoryContext {
    pub fn can_undo(&self) -> bool {
        *self.can_undo.read()
    }

    pub fn can_redo(&self) -> bool {
        *self.can_redo.read()
    }
}

/// Retrieves the history context (call inside a [`HistoryProvider`] subtree).
pub fn use_history() -> HistoryContext {
    use_context::<HistoryContext>()
}

/// Fetches `can_undo` / `can_redo` from the backend and updates the signals.
///
/// `history` is passed by value so this is safe to call from async tasks.
pub fn refresh_history_state(mut history: HistoryContext) {
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("history_status", empty_args).await;
        if let Ok(status) = serde_wasm_bindgen::from_value::<HistoryStatus>(result) {
            history.can_undo.set(status.can_undo);
            history.can_redo.set(status.can_redo);
        }
    });
}

/// Runs the backend undo action, shows feedback, and refreshes state.
pub fn undo(history: HistoryContext, toasts: ToastContext) {
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("history_undo", empty_args).await;
        match serde_wasm_bindgen::from_value::<Option<String>>(result) {
            Ok(Some(label)) => {
                toasts.info("Undo", format!("Undid: {label}"));
                notify_history_changed();
            }
            Ok(None) => toasts.warning("Undo", "Nothing to undo"),
            Err(_) => toasts.error("Undo", "Could not undo that action"),
        }
        refresh_history_state(history);
    });
}

/// Runs the backend redo action, shows feedback, and refreshes state.
pub fn redo(history: HistoryContext, toasts: ToastContext) {
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("history_redo", empty_args).await;
        match serde_wasm_bindgen::from_value::<Option<String>>(result) {
            Ok(Some(label)) => {
                toasts.info("Redo", format!("Redid: {label}"));
                notify_history_changed();
            }
            Ok(None) => toasts.warning("Redo", "Nothing to redo"),
            Err(_) => toasts.error("Redo", "Could not redo that action"),
        }
        refresh_history_state(history);
    });
}

/// Dispatches a window-level custom event after the vault changed via undo /
/// redo. Screens that display derived filesystem state (e.g. the Trash screen)
/// listen for `nabu:history-changed` and refresh, so an undo of a restore or
/// delete is reflected immediately instead of only after a remount.
fn notify_history_changed() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Ok(event) = web_sys::CustomEvent::new("nabu:history-changed") else {
        return;
    };
    let _ = window.dispatch_event(&event);
}

/// Returns `true` when keyboard focus is inside an editable element so native
/// editor history (CodeMirror / textarea undo) must be left untouched.
fn focus_is_in_editor() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    let Some(document) = window.document() else {
        return false;
    };
    let Some(active) = document.active_element() else {
        return false;
    };
    let tag = active.tag_name().to_ascii_lowercase();
    if tag == "textarea" || tag == "input" || tag == "select" {
        return true;
    }
    // contenteditable elements (e.g. CodeMirror's hidden editable area or a
    // contenteditable rich-text editor). `contenteditable="false"` is not an
    // editor, so only treat the attribute as editable when it isn't "false".
    active
        .get_attribute("contenteditable")
        .map_or(false, |v| !v.eq_ignore_ascii_case("false"))
}

/// Installed at most once for the lifetime of the app. If `App` ever re-mounts
/// (state reset, dev HMR) the leaked listener must not be registered twice,
/// otherwise a single Cmd+Z would fire undo N times.
static SHORTCUTS_INSTALLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Installs a global keydown listener for Cmd/Ctrl+Z, Cmd/Ctrl+Shift+Z and
/// Ctrl+Y. Shortcuts are ignored while focus is inside an editor so native
/// editing history keeps working.
///
/// The handler leaks intentionally (app-lifetime listener); both contexts are
/// captured by value at install time.
pub fn install_undo_shortcuts(history: HistoryContext, toasts: ToastContext) {
    if SHORTCUTS_INSTALLED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };

    let handler = Closure::<dyn Fn(web_sys::KeyboardEvent)>::wrap(Box::new(
        move |ev: web_sys::KeyboardEvent| {
            // Only react to the shortcut when an undo/redo modifier is pressed.
            let is_undo = (ev.meta_key() || ev.ctrl_key())
                && !ev.shift_key()
                && ev.key().eq_ignore_ascii_case("z");
            let is_redo = (ev.meta_key() || ev.ctrl_key())
                && ev.shift_key()
                && ev.key().eq_ignore_ascii_case("z");
            let is_redo_y = ev.ctrl_key() && ev.key().eq_ignore_ascii_case("y");

            if !(is_undo || is_redo || is_redo_y) {
                return;
            }
            // Preserve native editing history inside editors/inputs.
            if focus_is_in_editor() {
                return;
            }
            ev.prevent_default();
            ev.stop_propagation();
            if is_undo {
                undo(history, toasts);
            } else {
                redo(history, toasts);
            }
        },
    ));

    let _ = window.add_event_listener_with_callback(
        "keydown",
        handler.as_ref().unchecked_ref(),
    );
    // Leak the handler so it lives for the lifetime of the app window.
    // (Standard pattern for app-lifetime DOM listeners in wasm.)
    std::mem::forget(handler);
}
