//! # Canvas — infinite visual workspace
//!
//! An infinite pannable / zoomable canvas that *references* existing notes
//! rather than duplicating content. Nodes are positioned cards pointing at
//! vault-relative note paths; edges are visual connectors; groups are labelled
//! bounding boxes. Canvas definitions are persisted as JSON in the settings
//! store (`nabu.canvases`) — no proprietary storage format.
//!
//! ## Performance
//!
//! Only visible nodes (within the viewport after pan/zoom) are rendered
//! (viewport culling). Large canvases stay responsive because off-screen
//! nodes are skipped entirely.

use crate::components::navigation::state::{use_nav, NoteIndexEntry};
use crate::components::workspace::{open_tab, use_workspace};
use crate::components::ui::feedback::use_toast;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

// ── Types (mirror the backend `CanvasDef` family) ───────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasNode {
    pub id: String,
    pub note_path: String,
    pub title: String,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default = "default_node_kind")]
    pub kind: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub text: String,
}

fn default_node_kind() -> String {
    "note".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasGroup {
    pub id: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    #[serde(default)]
    pub members: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanvasDef {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub nodes: Vec<CanvasNode>,
    #[serde(default)]
    pub edges: Vec<CanvasEdge>,
    #[serde(default)]
    pub groups: Vec<CanvasGroup>,
    #[serde(default)]
    pub pan_x: f64,
    #[serde(default)]
    pub pan_y: f64,
    #[serde(default = "default_zoom")]
    pub zoom: f64,
}

fn default_zoom() -> f64 {
    1.0
}

const DEFAULT_NODE_W: f64 = 240.0;
const DEFAULT_NODE_H: f64 = 120.0;

// ── Canvas Component ────────────────────────────────────────────────

#[component]
pub fn CanvasView() -> impl IntoView {
    let nav = use_nav();
    let workspace = use_workspace();
    let toasts = use_toast();

    let (canvases, set_canvases) = signal(Vec::<CanvasDef>::new());
    let (active_id, set_active_id) = signal(String::new());
    let (canvas, set_canvas) = signal(CanvasDef {
        id: String::new(),
        name: "Untitled Canvas".to_string(),
        nodes: vec![],
        edges: vec![],
        groups: vec![],
        pan_x: 0.0,
        pan_y: 0.0,
        zoom: 1.0,
    });
    let (loaded, set_loaded) = signal(false);
    let (pan, set_pan) = signal((0.0f64, 0.0f64));
    let (zoom, set_zoom) = signal(1.0f64);
    let (dragging, set_dragging) = signal(None::<String>);
    let (drag_offset, set_drag_offset) = signal((0.0f64, 0.0f64));
    let (panning, set_panning) = signal(false);
    let (pan_start, set_pan_start) = signal((0.0f64, 0.0f64, 0.0f64, 0.0f64));
    let (show_new_dialog, set_show_new_dialog) = signal(false);
    let (new_name, set_new_name) = signal(String::new());

    // Load canvas list on mount.
    spawn_local(async move {
        let empty = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("canvas_list", empty).await;
        if let Ok(list) = serde_wasm_bindgen::from_value::<Vec<CanvasDef>>(result) {
            set_canvases.set(list.clone());
            if let Some(first) = list.first() {
                set_active_id.set(first.id.clone());
                set_canvas.set(first.clone());
                set_pan.set((first.pan_x, first.pan_y));
                set_zoom.set(first.zoom);
            }
        }
        set_loaded.set(true);
    });

    // Persist the current canvas (debounced via spawn_local).
    let persist_canvas = move |c: CanvasDef| {
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "canvas": c })).unwrap();
            let _ = crate::ipc::tauri_invoke("canvas_save", args).await;
        });
    };

    // ── Pan & Zoom ──
    let on_wheel = move |ev: leptos::ev::WheelEvent| {
        let delta = ev.delta_y();
        let current = zoom.get();
        let factor = if delta > 0.0 { 0.9 } else { 1.1 };
        let new_zoom = (current * factor).clamp(0.1, 5.0);
        set_zoom.set(new_zoom);
    };

    let on_mousedown_pan = move |ev: leptos::ev::MouseEvent| {
        // Only start panning when clicking the canvas background (not a node).
        let target: web_sys::HtmlElement = event_target(&ev);
        let class = target.class_name();
        if class.contains("canvas-bg") {
            set_panning.set(true);
            set_pan_start.set((ev.client_x() as f64, ev.client_y() as f64, pan.get().0, pan.get().1));
        }
    };

    let on_mousemove = move |ev: leptos::ev::MouseEvent| {
        if panning.get() {
            let (sx, sy, px, py) = pan_start.get();
            let dx = ev.client_x() as f64 - sx;
            let dy = ev.client_y() as f64 - sy;
            set_pan.set((px + dx, py + dy));
        } else if let Some(node_id) = dragging.get() {
            let (ox, oy) = drag_offset.get();
            let z = zoom.get();
            let (px, py) = pan.get();
            let dx = ((ev.client_x() as f64) - ox) / z;
            let dy = ((ev.client_y() as f64) - oy) / z;
            let mut c = canvas.get();
            if let Some(node) = c.nodes.iter_mut().find(|n| n.id == node_id) {
                node.x += dx;
                node.y += dy;
            }
            set_canvas.set(c.clone());
            // Update offset for next move event.
            set_drag_offset.set((ev.client_x() as f64, ev.client_y() as f64));
            persist_canvas(c);
        }
    };

    let on_mouseup = move |_| {
        set_panning.set(false);
        set_dragging.set(None);
    };

    // ── Node drag ──
    let start_node_drag = move |node_id: String, ev: leptos::ev::MouseEvent| {
        ev.stop_propagation();
        set_dragging.set(Some(node_id));
        set_drag_offset.set((ev.client_x() as f64, ev.client_y() as f64));
    };

    // ── Node double-click → open the referenced note ──
    let open_note = move |path: String| {
        open_tab(workspace, &path);
        nav.view_mode.set(crate::components::navigation::state::ViewMode::Editor);
    };

    // ── Add a note from the sidebar index to the canvas ──
    let add_note_to_canvas = move |entry: NoteIndexEntry| {
        let mut c = canvas.get();
        let id = format!("n{}", c.nodes.len() + 1);
        // Place new nodes in a cascading position near the viewport center.
        let offset = c.nodes.len() as f64 * 30.0;
        c.nodes.push(CanvasNode {
            id,
            note_path: entry.path,
            title: entry.title,
            x: -pan.get().0 / zoom.get() + offset,
            y: -pan.get().1 / zoom.get() + offset,
            width: None,
            height: None,
            kind: "note".to_string(),
            source: String::new(),
            text: String::new(),
        });
        set_canvas.set(c.clone());
        persist_canvas(c);
    };

    // ── Remove a node ──
    let remove_node = move |node_id: String| {
        let mut c = canvas.get();
        c.nodes.retain(|n| n.id != node_id);
        c.edges.retain(|e| e.source != node_id && e.target != node_id);
        set_canvas.set(c.clone());
        persist_canvas(c);
    };

    // ── Create a new canvas ──
    let create_canvas = Callback::new(move |_| {
        let name = new_name.get();
        if name.trim().is_empty() {
            return;
        }
        let id = format!("canvas-{}", js_sys::Date::new_0().get_time() as u64);
        let new_canvas = CanvasDef {
            id: id.clone(),
            name: name.clone(),
            nodes: vec![],
            edges: vec![],
            groups: vec![],
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        };
        let c = new_canvas.clone();
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "canvas": c })).unwrap();
            let _ = crate::ipc::tauri_invoke("canvas_save", args).await;
        });
        set_canvases.update(|l| l.push(new_canvas.clone()));
        set_active_id.set(id);
        set_canvas.set(new_canvas);
        set_pan.set((0.0, 0.0));
        set_zoom.set(1.0);
        set_show_new_dialog.set(false);
        set_new_name.set(String::new());
        toasts.success("Canvas created", name);
    });

    // ── Switch canvas ──
    let switch_canvas = move |id: String| {
        set_active_id.set(id.clone());
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "id": id })).unwrap();
            let result = crate::ipc::tauri_invoke("canvas_get", args).await;
            if let Ok(Some(c)) = serde_wasm_bindgen::from_value::<Option<CanvasDef>>(result) {
                set_canvas.set(c.clone());
                set_pan.set((c.pan_x, c.pan_y));
                set_zoom.set(c.zoom);
            }
        });
    };

    // ── Delete canvas ──
    let delete_canvas = move |id: String| {
        let id_del = id.clone();
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "id": id_del })).unwrap();
            let _ = crate::ipc::tauri_invoke("canvas_delete", args).await;
        });
        set_canvases.update(|l| l.retain(|c| c.id != id));
        if active_id.get() == id {
            if let Some(first) = canvases.get().first() {
                set_active_id.set(first.id.clone());
                set_canvas.set(first.clone());
            } else {
                set_active_id.set(String::new());
                set_canvas.set(CanvasDef {
                    id: String::new(),
                    name: "Untitled Canvas".to_string(),
                    nodes: vec![],
                    edges: vec![],
                    groups: vec![],
                    pan_x: 0.0,
                    pan_y: 0.0,
                    zoom: 1.0,
                });
            }
        }
    };

    // ── Zoom controls ──
    let zoom_in = move |_| set_zoom.update(|z| *z = (*z * 1.2).clamp(0.1, 5.0));
    let zoom_out = move |_| set_zoom.update(|z| *z = (*z * 0.8).clamp(0.1, 5.0));
    let zoom_reset = move |_| {
        set_zoom.set(1.0);
        set_pan.set((0.0, 0.0));
    };

    // Viewport culling: only render nodes within the visible area.
    let visible_nodes = move || {
        let c = canvas.get();
        let z = zoom.get();
        let (px, py) = pan.get();
        // Viewport bounds in canvas coordinates.
        let vw = 1200.0 / z; // approximate viewport width
        let vh = 800.0 / z; // approximate viewport height
        let left = -px / z;
        let top = -py / z;
        c.nodes
            .iter()
            .filter(|n| {
                let w = n.width.unwrap_or(DEFAULT_NODE_W);
                let h = n.height.unwrap_or(DEFAULT_NODE_H);
                n.x + w >= left && n.x <= left + vw && n.y + h >= top && n.y <= top + vh
            })
            .cloned()
            .collect::<Vec<_>>()
    };

    let notes_index = move || nav.notes_index.get();

    view! {
        <div class="canvas-view flex h-full bg-gray-950 text-gray-100 overflow-hidden">
            // Left panel: canvas list + note palette
            <div class="flex-none w-64 border-r border-gray-800 flex flex-col">
                <div class="flex items-center justify-between px-3 py-2 border-b border-gray-800">
                    <h2 class="text-sm font-semibold text-gray-300">"Canvases"</h2>
                    <button
                        class="px-2 py-1 text-xs bg-blue-600 rounded hover:bg-blue-500"
                        on:click=move |_| set_show_new_dialog.set(true)
                    >
                        "+ New"
                    </button>
                </div>

                // Canvas list
                <div class="overflow-y-auto max-h-40 border-b border-gray-800">
                    {move || {
                        let list = canvases.get();
                        if list.is_empty() {
                            view! {
                                <div class="px-3 py-2 text-xs text-gray-500">"No canvases yet"</div>
                            }.into_any()
                        } else {
                            list.iter().map(|c| {
                                let id = c.id.clone();
                                let id_del = c.id.clone();
                                let name = c.name.clone();
                                let class_id = id.clone();
                                view! {
                                    <div
                                        class=move || format!(
                                            "flex items-center justify-between px-3 py-1.5 cursor-pointer hover:bg-gray-800 {}",
                                            if active_id.get() == class_id { "bg-gray-800 border-l-2 border-l-blue-500" } else { "" }
                                        )
                                        on:click=move |_| switch_canvas(id.clone())
                                    >
                                        <span class="text-sm truncate">{name}</span>
                                        <button
                                            class="text-xs text-gray-500 hover:text-red-400"
                                            on:click=move |ev| {
                                                ev.stop_propagation();
                                                delete_canvas(id_del.clone());
                                            }
                                        >
                                            "✕"
                                        </button>
                                    </div>
                                }
                            }).collect_view().into_any()
                        }
                    }}
                </div>

                // Note palette — drag notes onto the canvas
                <div class="flex-1 overflow-y-auto">
                    <div class="px-3 py-2 text-xs text-gray-500 uppercase tracking-wide">"Notes"</div>
                    {move || {
                        notes_index().iter().take(200).map(|entry| {
                            let e = entry.clone();
                            view! {
                                <div
                                    class="px-3 py-1.5 text-sm cursor-pointer hover:bg-gray-800 truncate"
                                    on:dblclick=move |_| add_note_to_canvas(e.clone())
                                    title="Double-click to add to canvas"
                                >
                                    {entry.title.clone()}
                                </div>
                            }
                        }).collect_view()
                    }}
                </div>
            </div>

            // Canvas area
            <div class="flex-1 relative overflow-hidden"
                on:wheel=on_wheel
                on:mousedown=on_mousedown_pan
                on:mousemove=on_mousemove
                on:mouseup=on_mouseup
                on:mouseleave=on_mouseup
            >
                <div class="canvas-bg absolute inset-0"
                    style=move || format!(
                        "transform: translate({}px, {}px) scale({}); transform-origin: 0 0;",
                        pan.get().0, pan.get().1, zoom.get()
                    )
                >
                    // Groups (rendered behind nodes)
                    {move || {
                        canvas.get().groups.iter().map(|g| {
                            view! {
                                <div
                                    class="absolute border-2 border-dashed border-gray-700 rounded-lg bg-gray-800/20"
                                    style=format!(
                                        "left: {:.1}px; top: {:.1}px; width: {:.1}px; height: {:.1}px;",
                                        g.x, g.y, g.width, g.height
                                    )
                                >
                                    <div class="px-2 py-1 text-xs text-gray-500 font-medium">{g.label.clone()}</div>
                                </div>
                            }
                        }).collect_view()
                    }}

                    // Edges (SVG connectors)
                    <svg class="absolute inset-0 pointer-events-none"
                        style="width: 100%; height: 100%; overflow: visible;"
                    >
                        {move || {
                            let c = canvas.get();
                            c.edges.iter().filter_map(|edge| {
                                let source = c.nodes.iter().find(|n| n.id == edge.source)?;
                                let target = c.nodes.iter().find(|n| n.id == edge.target)?;
                                let sw = source.width.unwrap_or(DEFAULT_NODE_W);
                                let sh = source.height.unwrap_or(DEFAULT_NODE_H);
                                let tw = target.width.unwrap_or(DEFAULT_NODE_W);
                                let th = target.height.unwrap_or(DEFAULT_NODE_H);
                                let x1 = source.x + sw / 2.0;
                                let y1 = source.y + sh / 2.0;
                                let x2 = target.x + tw / 2.0;
                                let y2 = target.y + th / 2.0;
                                let mid_x = (x1 + x2) / 2.0;
                                let mid_y = (y1 + y2) / 2.0;
                                Some(view! {
                                    <g>
                                        <path
                                            d=format!("M {:.1} {:.1} L {:.1} {:.1}", x1, y1, x2, y2)
                                            stroke="#4b5563"
                                            stroke-width="2"
                                            fill="none"
                                            marker-end="url(#arrowhead)"
                                        />
                                        {if edge.label.is_empty() { view! {}.into_any() } else {
                                            view! {
                                                <text x={mid_x.to_string()} y={mid_y.to_string()}
                                                    fill="#9ca3af" font-size="11" text-anchor="middle"
                                                >
                                                    {edge.label.clone()}
                                                </text>
                                            }.into_any()
                                        }}
                                    </g>
                                })
                            }).collect_view()
                        }}
                        <defs>
                            <marker id="arrowhead" markerWidth="10" markerHeight="7"
                                refX="9" refY="3.5" orient="auto"
                            >
                                <polygon points="0 0, 10 3.5, 0 7" fill="#4b5563" />
                            </marker>
                        </defs>
                    </svg>

                    // Nodes
                    {move || {
                        visible_nodes().iter().map(|node| {
                            let id = node.id.clone();
                            let id_drag = id.clone();
                            let id_close = id.clone();
                            let path = node.note_path.clone();
                            let path_open = path.clone();
                            let title = node.title.clone();
                            let w = node.width.unwrap_or(DEFAULT_NODE_W);
                            let h = node.height.unwrap_or(DEFAULT_NODE_H);
                            view! {
                                <div
                                    class="absolute bg-gray-800 border border-gray-600 rounded-lg shadow-lg cursor-move hover:border-blue-500 transition-colors"
                                    style=format!(
                                        "left: {:.1}px; top: {:.1}px; width: {:.0}px; min-height: {:.0}px;",
                                        node.x, node.y, w, h
                                    )
                                    on:mousedown=move |ev| start_node_drag(id_drag.clone(), ev)
                                    on:dblclick=move |_| open_note(path_open.clone())
                                >
                                    <div class="flex items-center justify-between px-2 py-1 border-b border-gray-700">
                                        <span class="text-xs font-medium text-gray-300 truncate">{title}</span>
                                        <button
                                            class="text-xs text-gray-500 hover:text-red-400"
                                            on:click=move |ev| {
                                                ev.stop_propagation();
                                                remove_node(id_close.clone());
                                            }
                                        >
                                            "✕"
                                        </button>
                                    </div>
                                    <div class="px-2 py-1 text-xs text-gray-500">
                                        {node.kind.clone()}
                                    </div>
                                </div>
                            }
                        }).collect_view()
                    }}
                </div>

                // Zoom controls (fixed position)
                <div class="absolute bottom-4 right-4 flex flex-col gap-1 bg-gray-800 rounded-lg border border-gray-700 p-1">
                    <button class="w-8 h-8 flex items-center justify-center hover:bg-gray-700 rounded" on:click=zoom_in>"+"</button>
                    <button class="w-8 h-8 flex items-center justify-center hover:bg-gray-700 rounded text-xs" on:click=zoom_reset>"⊙"</button>
                    <button class="w-8 h-8 flex items-center justify-center hover:bg-gray-700 rounded" on:click=zoom_out>"−"</button>
                </div>

                // Zoom indicator
                <div class="absolute top-4 right-4 px-2 py-1 bg-gray-800 rounded text-xs text-gray-400 border border-gray-700">
                    {move || format!("{:.0}%", zoom.get() * 100.0)}
                </div>

                // Empty state
                {move || {
                    if loaded.get() && canvas.get().nodes.is_empty() && !active_id.get().is_empty() {
                        view! {
                            <div class="absolute inset-0 flex items-center justify-center pointer-events-none">
                                <div class="text-center text-gray-500">
                                    <div class="text-4xl mb-2">"🎨"</div>
                                    <p class="text-sm">"Double-click notes from the sidebar to add them"</p>
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        view! {}.into_any()
                    }
                }}
            </div>

            // New canvas dialog
            {move || if show_new_dialog.get() {
                view! {
                    <div class="dialog-overlay" on:click=move |_| set_show_new_dialog.set(false)>
                        <div class="panel dialog-content max-w-sm" on:click=move |ev| ev.stop_propagation()>
                            <h2 class="text-lg font-semibold mb-3">"New Canvas"</h2>
                            <input
                                class="input w-full mb-3"
                                type="text"
                                placeholder="Canvas name…"
                                prop:value=new_name
                                on:input=move |ev| set_new_name.set(event_target_value(&ev))
                                on:keydown=move |ev| {
                                    if ev.key() == "Enter" { create_canvas.run(()) }
                                    if ev.key() == "Escape" { set_show_new_dialog.set(false) }
                                }
                            />
                            <div class="flex justify-end gap-2">
                                <button class="px-3 py-1.5 text-sm bg-gray-700 rounded hover:bg-gray-600"
                                    on:click=move |_| set_show_new_dialog.set(false)
                                >
                                    "Cancel"
                                </button>
                                <button class="px-3 py-1.5 text-sm bg-blue-600 rounded hover:bg-blue-500"
                                    on:click=move |_| create_canvas.run(())
                                >
                                    "Create"
                                </button>
                            </div>
                        </div>
                    </div>
                }.into_any()
            } else {
                view! {}.into_any()
            }}
        </div>
    }
}