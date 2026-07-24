pub mod tree;
pub mod components;


use leptos::prelude::*;

#[derive(Clone, Copy)]
pub struct ThemeContext {
    pub theme: RwSignal<String>,
}

pub fn provide_theme(theme: String) {
    provide_context(ThemeContext { theme: RwSignal::new(theme) });
}