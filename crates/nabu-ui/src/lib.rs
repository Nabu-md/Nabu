pub mod tree;
pub mod ipc;
pub mod components;

use leptos::prelude::*;

#[derive(Clone, Copy)]
pub struct ThemeContext {
    pub theme: leptos::prelude::RwSignal<String>,
}

pub fn provide_theme(initial_theme: String) {
    let theme = RwSignal::new(initial_theme);
    provide_context(ThemeContext { theme });

    // Reactively update backend when theme changes
    create_effect(move |_| {
        let current_theme = theme.get();
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({"key": "theme", "value": current_theme})).unwrap();
            let _ = crate::ipc::tauri_invoke("settings_set", args).await;
        });
    });
}

pub fn use_theme() -> ThemeContext {
    expect_context::<ThemeContext>()
}