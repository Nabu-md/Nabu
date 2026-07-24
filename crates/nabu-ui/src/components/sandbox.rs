use leptos::prelude::*;

#[component]
pub fn AppBlockSandbox(html_content: String) -> impl IntoView {
    view! {
        <iframe 
            srcdoc=html_content
            class="sandbox-frame"
            sandbox="allow-scripts"
            title="App Block Sandbox"
        />
    }
}
