use crate::components::app::App;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

pub mod components;
pub mod ipc;
pub mod models;
pub mod tree;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    remove_boot_splash();
    mount_to_body(|| view! { <App /> });
}

/// Removes the static boot splash element from `index.html` once the app is
/// about to mount. The splash (dark background + spinner) paints instantly on
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

#[derive(Clone, Copy)]
pub struct ThemeContext {
    pub theme: leptos::prelude::RwSignal<String>,
}

pub fn provide_theme(initial_theme: String) {
    let theme = RwSignal::new(initial_theme);
    provide_context(ThemeContext { theme });

    // Reactively update backend when theme changes
    Effect::new(move |_| {
        let current_theme = theme.get();
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(
                &serde_json::json!({"key": "theme", "value": current_theme}),
            )
            .unwrap();
            let _ = crate::ipc::tauri_invoke("settings_set", args).await;
        });
    });
}

pub fn use_theme() -> ThemeContext {
    expect_context::<ThemeContext>()
}
