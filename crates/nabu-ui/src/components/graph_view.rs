use leptos::prelude::*;
use web_sys::CanvasRenderingContext2d;
use wasm_bindgen::JsCast;

#[derive(Clone, Copy, PartialEq)]
pub enum GraphMode { Default, TagView, BlocksView }

#[component]
pub fn GraphView(mode: GraphMode) -> impl IntoView {

    let graph_data = Resource::new(|| (), |_| async move {
        // In a real implementation, this would call Tauri IPC
        // invoke("get_graph_data", ...).await
        serde_json::json!({"nodes": [], "edges": []})
    });

    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();

    Effect::new(move |_| {
        if let Some(canvas) = canvas_ref.get() {
            let _data = graph_data.get();
            let context = canvas
                .get_context("2d")
                .unwrap()
                .unwrap()
                .dyn_into::<CanvasRenderingContext2d>()
                .unwrap();
            
            context.clear_rect(0.0, 0.0, 400.0, 400.0);
        }
    });

    view! {
        <canvas node_ref=canvas_ref width=400 height=400 class="graph-canvas" />
    }
}
