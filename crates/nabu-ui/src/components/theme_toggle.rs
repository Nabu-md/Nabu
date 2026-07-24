use leptos::prelude::*;
use crate::ThemeContext;

#[component]
pub fn ThemeToggle() -> impl IntoView {
    let context = expect_context::<ThemeContext>();
    
    let toggle = move |_| {
        context.theme.update(|t| {
            *t = if t == "dark" { "light".to_string() } else { "dark".to_string() };
        });
    };

    view! {
        <button on:click=toggle>
            "Toggle Theme: " {move || context.theme.get()}
        </button>
    }
}
