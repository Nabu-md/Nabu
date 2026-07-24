use leptos::prelude::*;

#[component]
pub fn SandboxedHtml(html: String) -> impl IntoView {
    view! {
        <iframe srcdoc=html sandbox="allow-scripts" class="sandboxed-html" />
    }
}
