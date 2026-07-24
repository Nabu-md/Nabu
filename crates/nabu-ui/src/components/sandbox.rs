use leptos::prelude::*;
use web_sys::{HtmlIFrameElement, MessageEvent};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[component]
pub fn AppBlockSandbox(html_content: String) -> impl IntoView {
    let iframe_ref = NodeRef::<leptos::html::Iframe>::new();

    let on_message = Closure::<dyn FnMut(MessageEvent)>::new(|event: MessageEvent| {
        if let Ok(data) = event.data().as_string() {
            leptos::logging::log!("Message from sandbox: {}", data);
        }
    });

    Effect::new(move |_| {
        if let Some(iframe) = iframe_ref.get() {
            let window = iframe.content_window().unwrap();
            window.add_event_listener_with_callback("message", on_message.as_ref().unchecked_ref()).unwrap();
        }
    });

    view! {
        <iframe 
            node_ref=iframe_ref
            srcdoc=html_content
            class="sandbox-frame"
            sandbox="allow-scripts allow-forms"
            title="App Block Sandbox"
        />
    }
}
