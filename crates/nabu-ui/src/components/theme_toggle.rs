use crate::components::ui::button::{Button, ButtonVariant};
use crate::ThemeContext;
use leptos::prelude::*;

#[component]
pub fn ThemeToggle() -> impl IntoView {
    let context = expect_context::<ThemeContext>();

    let toggle = Callback::new(move |_| {
        context.theme.update(|t| {
            *t = if t == "dark" {
                "light".to_string()
            } else {
                "dark".to_string()
            };
        });
    });

    view! {
        <Button variant=ButtonVariant::Ghost on_click=toggle>
            "Toggle Theme: " {move || context.theme.get()}
        </Button>
    }
}
