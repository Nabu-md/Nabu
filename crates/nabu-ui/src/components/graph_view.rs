use leptos::prelude::*;
use web_sys::CanvasRenderingContext2d;
use wasm_bindgen::JsCast;

#[derive(Clone, Copy, PartialEq)]
pub enum GraphMode { Default, TagView, BlocksView }

#[component]
pub fn GraphView(mode: GraphMode) -> impl IntoView {
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();

    let graph_data = LocalResource::new(move || async move {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let res = crate::ipc::tauri_invoke("get_graph_data", args).await;
        serde_wasm_bindgen::from_value::<serde_json::Value>(res).unwrap_or_default()
    });

    Effect::new(move |_| {
        if let Some(canvas) = canvas_ref.get() {
            if let Some(data) = graph_data.get() {
                let context = canvas.get_context("2d").unwrap().unwrap()
                    .dyn_into::<CanvasRenderingContext2d>().unwrap();
                
                context.clear_rect(0.0, 0.0, canvas.width() as f64, canvas.height() as f64);
                
                if let Some(nodes) = data["nodes"].as_array() {
                    for _node in nodes {
                        context.begin_path();
                        context.arc(100.0, 100.0, 10.0, 0.0, std::f64::consts::PI * 2.0).unwrap();
                        context.fill();
                    }
                }
            }
        }
    });

    view! {
        <canvas node_ref=canvas_ref width=800 height=600 class="graph-canvas" />
    }
}
