use leptos::prelude::*;

#[component]
pub fn AppBlockSandbox(html_content: String) -> impl IntoView {
    let iframe_ref = NodeRef::<leptos::html::Iframe>::new();

    Effect::new(move |_| {
        if let Some(iframe) = iframe_ref.get() {
            // Secure message handling logic
        }
    });

    view! {
        <iframe 
            node_ref=iframe_ref
            srcdoc=html_content
            class="sandbox-frame"
            sandbox="allow-scripts"
            title="App Block Sandbox"
        />
    }
}
