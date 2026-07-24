use leptos::prelude::*;
use web_sys::CanvasRenderingContext2d;
use wasm_bindgen::JsCast;

#[component]
pub fn GraphView() -> impl IntoView {
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();

    Effect::new(move |_| {
        if let Some(canvas) = canvas_ref.get() {
            let context = canvas
                .get_context("2d")
                .unwrap()
                .unwrap()
                .dyn_into::<CanvasRenderingContext2d>()
                .unwrap();
            
            context.clear_rect(0.0, 0.0, 400.0, 400.0);
            context.begin_path();
            context.arc(200.0, 200.0, 50.0, 0.0, 2.0 * std::f64::consts::PI).unwrap();
            context.stroke();
        }
    });

    view! {
        <canvas node_ref=canvas_ref width=400 height=400 class="graph-canvas" />
    }
}
