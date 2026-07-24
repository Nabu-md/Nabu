use leptos::*;
use web_sys::{HtmlCanvasElement, CanvasRenderingContext2d};
use wasm_bindgen::JsCast;

#[component]
pub fn GraphView() -> impl IntoView {
    let canvas_ref = create_node_ref::<html::Canvas>();

    create_effect(move |_| {
        if let Some(canvas) = canvas_ref.get() {
            let context = canvas
                .get_context("2d")
                .unwrap()
                .unwrap()
                .dyn_into::<CanvasRenderingContext2d>()
                .unwrap();
            
            // Placeholder: Basic rendering
            context.begin_path();
            context.arc(50.0, 50.0, 20.0, 0.0, 2.0 * std::f64::consts::PI).unwrap();
            context.stroke();
        }
    });

    view! {
        <canvas node_ref=canvas_ref width=400 height=400 />
    }
}
