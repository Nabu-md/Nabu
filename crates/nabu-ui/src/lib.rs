//! # Nabu UI — Dioxus frontend crate
//!
//! Entry point for the Nabu knowledge management app's WebAssembly frontend.
//! The WASM bundle is loaded inside a Tauri webview; native integration is
//! handled entirely through the IPC abstraction in [`crate::ipc`].

use dioxus::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

pub mod components;
pub mod history;
pub mod ipc;
pub mod models;

/// Dioxus launch entry point — called by wasm-bindgen on WASM module init.
/// The boot splash in `index.html` paints instantly while the bundle loads;
/// [`remove_boot_splash`] removes it once the app is about to mount.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    remove_boot_splash();
    dioxus::web::launch::launch_cfg(
        components::app::App,
        dioxus::web::Config::default(),
    );
}

/// Removes the static boot splash element from `index.html` once the app is
/// about to mount.  The splash (dark background + spinner) paints instantly on
/// launch so the window never shows a white flash while the wasm loads.
fn remove_boot_splash() {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(splash) = document.get_element_by_id("boot-splash") {
                splash.remove();
            }
        }
    }
}

// ── Theme context ──────────────────────────────────────────────────

/// Shared theme context.  The `data-theme` attribute on the document root
/// element drives the CSS palette (dark / light / system) via the design
/// tokens in `src/styles/app.css`.
#[derive(Clone, Copy)]
pub struct ThemeContext {
    pub theme: Signal<String>,
}

/// Provides the theme context and wires the persisted-theme sync loop.
///
/// Must be called inside a component body (so Dioxus hooks are available).
/// Mirrors the LePtOS `provide_theme` semantics: the theme signal is created,
/// provided as context, and a reactive effect keeps the DOM + backend in sync.
pub fn provide_theme(initial_theme: String) {
    // use_signal ties the signal to the component scope so it survives
    // re-renders.  Signal::new would create a fresh signal on every render,
    // losing state.
    let mut theme = use_signal(|| initial_theme);
    let mut sync_ready = use_signal(|| false);

    provide_context(ThemeContext { theme });

    // Load the persisted theme preference on startup so the app opens in the
    // user's last chosen theme (dark / light / system).  Persisted overrides
    // are read from extra_settings via `settings_get`; when none exists, the
    // `theme` field of the full settings (default "system") is used instead.
    spawn_local({
        let mut theme = theme;
        let mut sync_ready = sync_ready;
        async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "key": "theme" }))
                .unwrap();
            let result = crate::ipc::tauri_invoke("settings_get", args).await;
            let mut resolved: Option<String> = None;
            if let Ok(saved) = serde_wasm_bindgen::from_value::<String>(result) {
                if !saved.trim().is_empty() {
                    resolved = Some(saved);
                }
            }
            if resolved.is_none() {
                // Fall back to the canonical settings (honours the "system"
                // default from AppSettings).
                let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
                let settings = crate::ipc::tauri_invoke("get_settings", empty_args).await;
                if let Ok(parsed) = serde_wasm_bindgen::from_value::<SettingsSnapshot>(settings) {
                    if let Some(t) = parsed.theme {
                        if !t.trim().is_empty() {
                            resolved = Some(t);
                        }
                    }
                }
            }
            if let Some(resolved) = resolved {
                theme.set(resolved);
            }
            sync_ready.set(true);
        }
    });

    // Apply the theme to the document root and mirror it to the backend when
    // it changes.  The design system (src/styles/app.css) reads the
    // `data-theme` attribute to swap dark / light / system palettes.
    use_effect(move || {
        let current_theme = theme.read();
        apply_theme_to_document(&current_theme);
        drop(current_theme);

        if !*sync_ready.read() {
            return;
        }

        let theme_val = theme.read().clone();
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                "key": "theme",
                "value": theme_val,
            }))
            .unwrap();
            let _ = crate::ipc::tauri_invoke("settings_set", args).await;
        });
    });
}

/// Minimal projection of the backend `AppSettings` — only the fields the UI
/// needs for startup theming.  Unknown fields are ignored by serde.
#[derive(serde::Deserialize)]
struct SettingsSnapshot {
    theme: Option<String>,
}

/// Sets `data-theme` on the document root element.  "system" removes the
/// attribute so the CSS `prefers-color-scheme` media query takes over.
fn apply_theme_to_document(theme: &str) {
    if let Some(window) = web_sys::window() {
        if let Some(document) = window.document() {
            if let Some(root) = document.document_element() {
                let _ = match theme {
                    "dark" => root.set_attribute("data-theme", "dark"),
                    "light" => root.set_attribute("data-theme", "light"),
                    _ => root.remove_attribute("data-theme"),
                };
            }
        }
    }
}

/// Retrieves the theme context.  Call inside a [`provide_theme`] subtree.
pub fn use_theme() -> ThemeContext {
    use_context::<ThemeContext>()
}

// ── Re-exports ────────────────────────────────────────────────────

pub use components::contexts::*;

