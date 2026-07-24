pub mod tree;
pub mod components;

use leptos::prelude::*;

#[derive(Clone, Copy)]
pub struct ThemeContext {
    pub theme: leptos::prelude::RwSignal<String>,
}

pub fn provide_theme(initial_theme: String) {
    provide_context(ThemeContext { theme: RwSignal::new(initial_theme) });
}

pub fn use_theme() -> ThemeContext {
    expect_context::<ThemeContext>()
}