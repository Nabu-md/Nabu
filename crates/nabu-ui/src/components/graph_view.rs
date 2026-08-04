//! # Knowledge Graph View
//!
//! Phase 13.1: a full interactive graph canvas for exploring connected
//! knowledge. Backed by the backend `graph_data` command (wikilinks extracted
//! from note markdown on demand) and the `note_links` command for the
//! relationship panel.
//!
//! Features:
//! - deterministic force-directed layout (stable across reloads)
//! - pan (drag background), zoom (wheel, buttons, keyboard), node drag
//! - hover previews, click to select + relationship panel, double-click opens
//! - search + folder/tag filtering with dimming
//! - focus mode (highlight a node and its neighbours)
//! - minimap + viewport indicator
//! - keyboard navigation (arrows, Enter, Esc, +/−)

use crate::components::ui::feedback::{use_toast, LoadingBlock, SpinnerSize};
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::info::EmptyState;
use crate::components::workspace::{open_tab, use_workspace};
use crate::models::graph::{BacklinkEntry, GraphData, GraphEdgeData, GraphNodeData, NoteLinks};
use leptos::prelude::*;
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{CanvasRenderingContext2d, WheelEvent};

#[derive(Clone, Copy, PartialEq)]
pub enum GraphMode {
    Default,
    TagView,
    BlocksView,
}

/// A stable hash of a path, used for deterministic initial node positions.
fn hash_path(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Deterministic Fruchterman–Reingold-style layout. Pure function of the
/// graph so the same vault always produces the same layout.
fn compute_layout(nodes: &[GraphNodeData], edges: &[GraphEdgeData]) -> Vec<(f64, f64)> {
    let n = nodes.len();
    if n == 0 {
        return Vec::new();
    }
    let mut pos: Vec<(f64, f64)> = nodes
        .iter()
        .map(|nd| {
            let h = hash_path(&nd.path);
            let a = ((h % 360) as f64) * std::f64::consts::PI / 180.0;
            let r = 0.4 + ((h >> 8) % 100) as f64 / 200.0;
            (r * a.cos(), r * a.sin())
        })
        .collect();

    let mut idx: HashMap<&str, usize> = HashMap::new();
    for (i, nd) in nodes.iter().enumerate() {
        idx.insert(nd.path.as_str(), i);
    }
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in edges {
        if e.broken {
            continue;
        }
        if let (Some(&a), Some(&b)) = (idx.get(e.source.as_str()), idx.get(e.target.as_str())) {
            if a != b && !adj[a].contains(&b) {
                adj[a].push(b);
                adj[b].push(a);
            }
        }
    }

    let k = 1.4 / (n as f64).sqrt().max(1.0);
    let iterations = 110usize.min(50 + n / 3);
    for iter in 0..iterations {
        // Repulsion between all pairs.
        let mut disp = vec![(0.0f64, 0.0f64); n];
        for i in 0..n {
            for j in (i + 1)..n {
                let dx = pos[i].0 - pos[j].0;
                let dy = pos[i].1 - pos[j].1;
                let d2 = dx * dx + dy * dy + 0.001;
                let d = d2.sqrt();
                let f = k * k / d2;
                let fx = f * dx / d;
                let fy = f * dy / d;
                disp[i].0 += fx;
                disp[i].1 += fy;
                disp[j].0 -= fx;
                disp[j].1 -= fy;
            }
        }
        // Attraction along edges.
        for a in 0..n {
            for &b in &adj[a] {
                if a >= b {
                    continue;
                }
                let dx = pos[a].0 - pos[b].0;
                let dy = pos[a].1 - pos[b].1;
                let d = (dx * dx + dy * dy).sqrt().max(0.001);
                let f = d * d / k;
                let fx = f * dx / d;
                let fy = f * dy / d;
                disp[a].0 -= fx;
                disp[a].1 -= fy;
                disp[b].0 += fx;
                disp[b].1 += fy;
            }
        }
        // Apply with cooling temperature + mild gravity toward the center.
        let t = 0.25 * (1.0 - iter as f64 / iterations as f64).max(0.02);
        for i in 0..n {
            let len = (disp[i].0 * disp[i].0 + disp[i].1 * disp[i].1).sqrt().max(0.001);
            let step = len.min(t);
            pos[i].0 += disp[i].0 / len * step + -0.015 * pos[i].0;
            pos[i].1 += disp[i].1 / len * step + -0.015 * pos[i].1;
        }
    }

    // Normalize into a bounded world space, centered on the origin.
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in &pos {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    let span_x = (max_x - min_x).max(1.0);
    let span_y = (max_y - min_y).max(1.0);
    let scale = 900.0 / span_x.max(span_y);
    let cx = (max_x + min_x) / 2.0;
    let cy = (max_y + min_y) / 2.0;
    pos.iter()
        .map(|&(x, y)| ((x - cx) * scale, (y - cy) * scale))
        .collect()
}

/// World→screen transform state (zoom + pan).
#[derive(Clone, Copy)]
struct ViewTransform {
    zoom: f64,
    offset_x: f64,
    offset_y: f64,
}

impl ViewTransform {
    fn world_to_screen(&self, wx: f64, wy: f64) -> (f64, f64) {
        (wx * self.zoom + self.offset_x, wy * self.zoom + self.offset_y)
    }
    fn screen_to_world(&self, sx: f64, sy: f64) -> (f64, f64) {
        ((sx - self.offset_x) / self.zoom, (sy - self.offset_y) / self.zoom)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum DragState {
    Panning { last_x: f64, last_y: f64 },
    MovingNode { idx: usize, last_x: f64, last_y: f64 },
}

/// Returns `Some(index)` of the node under the given screen point, else None.
fn hit_test(
    nodes: &[GraphNodeData],
    positions: &[(f64, f64)],
    view: &ViewTransform,
    sx: f64,
    sy: f64,
) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, node) in nodes.iter().enumerate() {
        let (x, y) = view.world_to_screen(positions[i].0, positions[i].1);
        let r = node_radius(node) * view.zoom;
        let d = ((sx - x).powi(2) + (sy - y).powi(2)).sqrt();
        let hit_radius = (r + 6.0).max(10.0);
        if d <= hit_radius && best.map_or(true, |(_, bd)| d < bd) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// Visual node radius (world units), sized by degree.
fn node_radius(node: &GraphNodeData) -> f64 {
    7.0 + (node.degree as f64).min(14.0) * 0.9
}

/// A stable hue (0..360) derived from the note's folder, for node colour.
fn folder_hue(folder: &str) -> f64 {
    (hash_path(folder) % 360) as f64
}

/// Loads the relationship panel data for a node.
async fn fetch_links(path: String) -> Option<NoteLinks> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path })).ok()?;
    let res = crate::ipc::tauri_invoke("note_links", args).await;
    serde_wasm_bindgen::from_value::<NoteLinks>(res).ok()
}

#[component]
pub fn GraphView(_mode: GraphMode) -> impl IntoView {
    let canvas_ref = NodeRef::<leptos::html::Canvas>::new();
    let ws = use_workspace();
    let toasts = use_toast();

    // ── State ────────────────────────────────────────────────────────
    let (graph, set_graph) = signal(None::<GraphData>);
    let (positions, set_positions) = signal(Vec::<(f64, f64)>::new());
    let (zoom, set_zoom) = signal(1.0f64);
    let (offset, set_offset) = signal((0.0f64, 0.0f64));
    let (selected, set_selected) = signal(None::<usize>);
    let (hovered, set_hovered) = signal(None::<usize>);
    let (dragging, set_dragging) = signal(None::<DragState>);
    let (search, set_search) = signal(String::new());
    let (folder_filter, set_folder_filter) = signal(String::new());
    let (tag_filter, set_tag_filter) = signal(String::new());
    let (focus_mode, set_focus_mode) = signal(false);
    let (links, set_links) = signal(None::<NoteLinks>);
    let (links_version, set_links_version) = signal(0u32);
    let (canvas_w, set_canvas_w) = signal(800.0f64);
    let (canvas_h, set_canvas_h) = signal(600.0f64);
    // Set when `graph_data` fails so the user gets a retry affordance instead
    // of an eternal spinner (Phase 12.3 error-handling standard).
    let (load_error, set_load_error) = signal(None::<String>);

    // ── Load graph data ──────────────────────────────────────────────
    let retry = Callback::new(move |_| {
        set_load_error.set(None);
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let res = crate::ipc::tauri_invoke("graph_data", args).await;
            match serde_wasm_bindgen::from_value::<GraphData>(res) {
                Ok(g) => {
                    let pos = compute_layout(&g.nodes, &g.edges);
                    set_positions.set(pos);
                    set_graph.set(Some(g));
                }
                Err(e) => set_load_error.set(Some(format!("Could not build the graph: {e}"))),
            }
        });
    });
    retry.run(());

    // Fit the view to the layout once the graph arrives.
    let fit = Callback::new(move |_| {
        if let (Some(g), Some(pos)) = (graph.get(), positions.get().as_slice().first().map(|_| positions.get())) {
            if g.nodes.is_empty() || pos.is_empty() {
                return;
            }
            let w = canvas_w.get();
            let h = canvas_h.get();
            let z = (w / 1400.0).min(h / 1000.0).clamp(0.25, 2.5);
            set_zoom.set(z);
            set_offset.set((w / 2.0, h / 2.0));
        }
    });
    Effect::new(move |_| {
        let _ = graph.get();
        fit.run(());
    });

    // Keep the canvas backing store in sync with its CSS size.
    Effect::new(move |_| {
        if let Some(canvas) = canvas_ref.get() {
            if let Some(parent) = canvas.parent_element() {
                let el = parent.unchecked_ref::<web_sys::HtmlElement>();
                let w = el.client_width() as f64;
                let h = el.client_height() as f64;
                if w > 0.0 && h > 0.0 {
                    canvas.set_width(w.max(200.0) as u32);
                    canvas.set_height(h.max(200.0) as u32);
                    set_canvas_w.set(w.max(200.0));
                    set_canvas_h.set(h.max(200.0));
                }
            }
        }
    });

    // Resize listener so the canvas tracks the window.
    let resize_handle = window_event_listener_untyped("resize", move |_ev| {
        if let Some(canvas) = canvas_ref.get() {
            if let Some(parent) = canvas.parent_element() {
                let el = parent.unchecked_ref::<web_sys::HtmlElement>();
                let w = el.client_width() as f64;
                let h = el.client_height() as f64;
                if w > 0.0 && h > 0.0 {
                    canvas.set_width(w.max(200.0) as u32);
                    canvas.set_height(h.max(200.0) as u32);
                    set_canvas_w.set(w.max(200.0));
                    set_canvas_h.set(h.max(200.0));
                }
            }
        }
    });
    on_cleanup(move || resize_handle.remove());

    // ── Relationship panel: load `note_links` when selection changes ──
    Effect::new(move |_| {
        let _ = selected.get();
        let _ = links_version.get();
        let sel = selected.get();
        let Some(g) = graph.get() else { return };
        if let Some(i) = sel {
            if let Some(node) = g.nodes.get(i) {
                let path = node.path.clone();
                spawn_local(async move {
                    if let Some(links) = fetch_links(path).await {
                        set_links.set(Some(links));
                    }
                });
            }
        } else {
            set_links.set(None);
        }
    });

    // ── Derived: visible / dimmed sets ───────────────────────────────
    let is_visible = move |node: &GraphNodeData| -> bool {
        if !search.get().is_empty() {
            let q = search.get().to_lowercase();
            let hay = format!(
                "{} {} {}",
                node.title.to_lowercase(),
                node.folder.to_lowercase(),
                node.tags.join(" ").to_lowercase()
            );
            if !hay.contains(&q) {
                return false;
            }
        }
        if !folder_filter.get().is_empty() && node.folder != folder_filter.get() {
            return false;
        }
        if !tag_filter.get().is_empty() && !node.tags.contains(&tag_filter.get()) {
            return false;
        }
        true
    };

    // Focus mode: the selected node + its neighbours stay bright.
    let focus_set = move || -> Option<std::collections::HashSet<usize>> {
        if !focus_mode.get() {
            return None;
        }
        let g = graph.get()?;
        let i = selected.get()?;
        let mut set = std::collections::HashSet::new();
        set.insert(i);
        let mut idx: HashMap<&str, usize> = HashMap::new();
        for (k, nd) in g.nodes.iter().enumerate() {
            idx.insert(nd.path.as_str(), k);
        }
        for e in &g.edges {
            if e.broken {
                continue;
            }
            let (a, b) = (idx.get(e.source.as_str()), idx.get(e.target.as_str()));
            if let (Some(&a), Some(&b)) = (a, b) {
                if a == i {
                    set.insert(b);
                }
                if b == i {
                    set.insert(a);
                }
            }
        }
        Some(set)
    };

    // ── Rendering ────────────────────────────────────────────────────
    Effect::new(move |_| {
        let _ = graph.get();
        let _ = positions.get();
        let _ = zoom.get();
        let _ = offset.get();
        let _ = selected.get();
        let _ = hovered.get();
        let _ = dragging.get();
        let _ = search.get();
        let _ = folder_filter.get();
        let _ = tag_filter.get();
        let _ = focus_mode.get();

        let Some(canvas) = canvas_ref.get() else { return };
        let Some(g) = graph.get() else { return };
        let pos = positions.get();
        if pos.len() != g.nodes.len() {
            return;
        }
        let Ok(Some(ctx)) = canvas.get_context("2d") else { return };
        let ctx: CanvasRenderingContext2d = ctx.unchecked_into();
        let w = canvas_w.get();
        let h = canvas_h.get();

        ctx.clear_rect(0.0, 0.0, w, h);
        // Subtle background.
        ctx.set_fill_style_str("#0b1220");
        ctx.fill_rect(0.0, 0.0, w, h);

        let view = ViewTransform {
            zoom: zoom.get(),
            offset_x: offset.get().0,
            offset_y: offset.get().1,
        };
        let focus = focus_set();
        let sel = selected.get();
        let hov = hovered.get();

        // ── Edges ──
        let mut node_idx: HashMap<&str, usize> = HashMap::new();
        for (k, nd) in g.nodes.iter().enumerate() {
            node_idx.insert(nd.path.as_str(), k);
        }
        for e in &g.edges {
            let (Some(&a), Some(&b)) = (node_idx.get(e.source.as_str()), node_idx.get(e.target.as_str()))
            else {
                continue;
            };
            let (x1, y1) = view.world_to_screen(pos[a].0, pos[a].1);
            let (x2, y2) = view.world_to_screen(pos[b].0, pos[b].1);
            let highlighted = sel.map_or(false, |s| a == s || b == s)
                || hov.map_or(false, |s| a == s || b == s)
                || focus.as_ref().map_or(false, |f| f.contains(&a) && f.contains(&b));
            ctx.begin_path();
            ctx.move_to(x1, y1);
            ctx.line_to(x2, y2);
            if e.broken {
                ctx.set_stroke_style_str("rgba(248,113,113,0.35)");
                ctx.set_line_width(1.0 * view.zoom);
                ctx.set_line_dash(&wasm_bindgen::JsValue::from_str("4 4"))
                    .ok();
            } else if highlighted {
                ctx.set_stroke_style_str("rgba(96,165,250,0.8)");
                ctx.set_line_width(1.8 * view.zoom);
            } else {
                ctx.set_stroke_style_str("rgba(100,116,139,0.35)");
                ctx.set_line_width(1.0 * view.zoom);
            }
            ctx.stroke();
            ctx.set_line_dash(&wasm_bindgen::JsValue::from_str("")).ok();
        }

        // ── Nodes ──
        for (i, node) in g.nodes.iter().enumerate() {
            let (x, y) = view.world_to_screen(pos[i].0, pos[i].1);
            let r = node_radius(node) * view.zoom;

            let mut dimmed = !is_visible(node);
            if let Some(f) = &focus {
                if !f.contains(&i) {
                    dimmed = true;
                }
            }
            let is_sel = sel == Some(i);
            let is_hov = hov == Some(i);

            let alpha = if dimmed { 0.12 } else { 1.0 };
            let hue = folder_hue(&node.folder);
            ctx.set_global_alpha(alpha);
            ctx.begin_path();
            ctx.arc(x, y, r, 0.0, std::f64::consts::PI * 2.0).ok();
            if is_sel || is_hov {
                ctx.set_fill_style_str(&format!("hsl({hue}, 85%, 65%)"));
            } else {
                ctx.set_fill_style_str(&format!("hsl({hue}, 60%, 45%)"));
            }
            ctx.fill();
            // Stroke: brighten selection.
            if is_sel {
                ctx.set_stroke_style_str("rgba(255,255,255,0.9)");
                ctx.set_line_width(2.0);
                ctx.stroke();
            }
            ctx.set_global_alpha(1.0);

            // Label for selected / hovered nodes (and large ones when zoomed).
            if is_sel || is_hov || (view.zoom > 0.9 && node.degree >= 4) {
                ctx.set_font(&format!("{}px Inter, sans-serif", (12.0 * view.zoom).max(10.0)));
                ctx.set_fill_style_str(if dimmed { "rgba(148,163,184,0.6)" } else { "#e2e8f0" });
                ctx.fill_text(&node.title, x + r + 4.0, y + 4.0).ok();
            }
        }

        // ── Tooltip (hover preview) ──
        if let Some(i) = hov {
            if let Some(node) = g.nodes.get(i) {
                let (x, y) = view.world_to_screen(pos[i].0, pos[i].1);
                let tip = format!(
                    "{}  ·  {} in/{} out  ·  {}",
                    node.title,
                    node.backlink_count,
                    node.outgoing_count,
                    if node.folder.is_empty() { "vault root" } else { &node.folder }
                );
                ctx.set_font("12px Inter, sans-serif");
                let tw = ctx.measure_text(&tip).ok().map(|m| m.width()).unwrap_or(200.0) + 16.0;
                let tx = (x + 10.0).clamp(0.0, (w - tw).max(0.0));
                let ty = (y - 8.0).clamp(12.0, h - 12.0);
                ctx.set_fill_style_str("rgba(15,23,42,0.92)");
                ctx.fill_rect(tx, ty - 18.0, tw, 22.0);
                ctx.set_stroke_style_str("rgba(100,116,139,0.5)");
                ctx.set_line_width(1.0);
                ctx.stroke_rect(tx, ty - 18.0, tw, 22.0);
                ctx.set_fill_style_str("#e2e8f0");
                ctx.fill_text(&tip, tx + 8.0, ty).ok();
            }
        }

        // ── Minimap ──
        if !pos.is_empty() {
            let mm_w = 140.0;
            let mm_h = 90.0;
            let mm_x = w - mm_w - 12.0;
            let mm_y = 12.0;
            ctx.set_fill_style_str("rgba(15,23,42,0.85)");
            ctx.fill_rect(mm_x, mm_y, mm_w, mm_h);
            ctx.set_stroke_style_str("rgba(100,116,139,0.4)");
            ctx.set_line_width(1.0);
            ctx.stroke_rect(mm_x, mm_y, mm_w, mm_h);
            let (mut min_x, mut min_y, mut max_x, mut max_y) =
                (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for &(px, py) in &pos {
                min_x = min_x.min(px);
                min_y = min_y.min(py);
                max_x = max_x.max(px);
                max_y = max_y.max(py);
            }
            let span = (max_x - min_x).max(1.0).max(max_y - min_y).max(1.0);
            let s = (mm_w - 8.0) / span;
            for (i, &(px, py)) in pos.iter().enumerate() {
                let mx = mm_x + 4.0 + (px - min_x) * s;
                let my = mm_y + 4.0 + (py - min_y) * s;
                ctx.begin_path();
                ctx.arc(mx, my, if sel == Some(i) { 2.6 } else { 1.6 }, 0.0, std::f64::consts::PI * 2.0).ok();
                ctx.set_fill_style_str(if sel == Some(i) { "#60a5fa" } else { "rgba(148,163,184,0.8)" });
                ctx.fill();
            }
            // Viewport indicator.
            let (vx, vy) = view.screen_to_world(0.0, 0.0);
            let (vx2, vy2) = view.screen_to_world(w, h);
            ctx.set_stroke_style_str("rgba(96,165,250,0.8)");
            ctx.set_line_width(1.0);
            ctx.stroke_rect(
                mm_x + 4.0 + (vx - min_x) * s,
                mm_y + 4.0 + (vy - min_y) * s,
                ((vx2 - vx) * s).abs(),
                ((vy2 - vy) * s).abs(),
            );
        }
    });

    // ── Interactions ─────────────────────────────────────────────────
    let on_mousedown = move |ev: web_sys::MouseEvent| {
        if ev.button() != 0 {
            return;
        }
        let sx = ev.offset_x() as f64;
        let sy = ev.offset_y() as f64;
        let view = ViewTransform {
            zoom: zoom.get(),
            offset_x: offset.get().0,
            offset_y: offset.get().1,
        };
        if let (Some(g), Some(pos)) = (graph.get(), positions.get().as_slice().first().map(|_| positions.get())) {
            if let Some(i) = hit_test(&g.nodes, &pos, &view, sx, sy) {
                set_selected.set(Some(i));
                set_hovered.set(Some(i));
                set_dragging.set(Some(DragState::MovingNode {
                    idx: i,
                    last_x: sx,
                    last_y: sy,
                }));
                return;
            }
        }
        set_selected.set(None);
        set_dragging.set(Some(DragState::Panning {
            last_x: sx,
            last_y: sy,
        }));
    };

    let on_mousemove = move |ev: web_sys::MouseEvent| {
        let sx = ev.offset_x() as f64;
        let sy = ev.offset_y() as f64;
        let view = ViewTransform {
            zoom: zoom.get(),
            offset_x: offset.get().0,
            offset_y: offset.get().1,
        };
        if let Some(drag) = dragging.get() {
            match drag {
                DragState::Panning { last_x, last_y } => {
                    let (ox, oy) = offset.get();
                    set_offset.set((ox + (sx - last_x), oy + (sy - last_y)));
                    set_dragging.set(Some(DragState::Panning {
                        last_x: sx,
                        last_y: sy,
                    }));
                }
                DragState::MovingNode { idx, last_x, last_y } => {
                    let mut pos = positions.get();
                    if let Some(p) = pos.get_mut(idx) {
                        let (wx, wy) = view.screen_to_world(sx, sy);
                        // Shift by the delta in world units.
                        let (plx, ply) = view.screen_to_world(last_x, last_y);
                        p.0 += wx - plx;
                        p.1 += wy - ply;
                    }
                    set_positions.set(pos);
                    set_dragging.set(Some(DragState::MovingNode {
                        idx,
                        last_x: sx,
                        last_y: sy,
                    }));
                }
            }
            return;
        }
        // Hover detection.
        if let (Some(g), Some(pos)) = (graph.get(), positions.get().as_slice().first().map(|_| positions.get())) {
            let hit = hit_test(&g.nodes, &pos, &view, sx, sy);
            set_hovered.set(hit);
        }
    };

    let on_mouseup = move |_ev: web_sys::MouseEvent| {
        set_dragging.set(None);
    };

    let on_leave = move |_ev: web_sys::MouseEvent| {
        set_dragging.set(None);
        set_hovered.set(None);
    };

    let on_dblclick = move |ev: web_sys::MouseEvent| {
        let sx = ev.offset_x() as f64;
        let sy = ev.offset_y() as f64;
        let view = ViewTransform {
            zoom: zoom.get(),
            offset_x: offset.get().0,
            offset_y: offset.get().1,
        };
        if let (Some(g), Some(pos)) = (graph.get(), positions.get().as_slice().first().map(|_| positions.get())) {
            if let Some(i) = hit_test(&g.nodes, &pos, &view, sx, sy) {
                if let Some(node) = g.nodes.get(i) {
                    open_tab(ws, &node.path);
                }
            }
        }
    };

    let on_wheel = move |ev: WheelEvent| {
        ev.prevent_default();
        let delta = ev.delta_y();
        let factor = if delta < 0.0 { 1.12 } else { 1.0 / 1.12 };
        let new_zoom = (zoom.get() * factor).clamp(0.2, 4.0);
        let z = zoom.get();
        if (new_zoom - z).abs() < 1e-6 {
            return;
        }
        let sx = ev.offset_x() as f64;
        let sy = ev.offset_y() as f64;
        let (ox, oy) = offset.get();
        // Zoom around the cursor.
        let (wx, wy) = ((sx - ox) / z, (sy - oy) / z);
        set_zoom.set(new_zoom);
        set_offset.set((sx - wx * new_zoom, sy - wy * new_zoom));
    };

    // ── Keyboard navigation ──────────────────────────────────────────
    let key_handle = window_event_listener_untyped("keydown", move |ev| {
        let ev = ev.unchecked_ref::<web_sys::KeyboardEvent>();
        if ev.meta_key() || ev.ctrl_key() || ev.alt_key() {
            return;
        }
        // Never hijack typing: ignore events that originated in form fields
        // (search box, selects) or contenteditable elements.
        if let Some(target) = ev.target() {
            if let Ok(el) = target.dyn_into::<web_sys::HtmlElement>() {
                let tag = el.tag_name().to_ascii_lowercase();
                if matches!(tag.as_str(), "input" | "textarea" | "select")
                    || el.is_content_editable()
                {
                    return;
                }
            }
        }
        match ev.key().as_str() {
            "ArrowRight" | "ArrowLeft" | "ArrowUp" | "ArrowDown" => {
                let Some(g) = graph.get() else { return };
                let pos = positions.get();
                let view = ViewTransform {
                    zoom: zoom.get(),
                    offset_x: offset.get().0,
                    offset_y: offset.get().1,
                };
                let cur = selected.get();
                // Neighbour set for the current selection, else all nodes.
                let candidates: Vec<usize> = if let Some(i) = cur {
                    let mut idx: HashMap<&str, usize> = HashMap::new();
                    for (k, nd) in g.nodes.iter().enumerate() {
                        idx.insert(nd.path.as_str(), k);
                    }
                    let mut set = std::collections::HashSet::new();
                    for e in &g.edges {
                        if e.broken {
                            continue;
                        }
                        let (a, b) = (idx.get(e.source.as_str()), idx.get(e.target.as_str()));
                        if let (Some(&a), Some(&b)) = (a, b) {
                            if a == i {
                                set.insert(b);
                            }
                            if b == i {
                                set.insert(a);
                            }
                        }
                    }
                    set.into_iter().collect()
                } else {
                    (0..g.nodes.len()).collect()
                };
                let (cx, cy) = if let Some(i) = cur {
                    pos.get(i).copied().unwrap_or((0.0, 0.0))
                } else {
                    (0.0, 0.0)
                };
                let dir = match ev.key().as_str() {
                    "ArrowRight" => (1.0, 0.0),
                    "ArrowLeft" => (-1.0, 0.0),
                    "ArrowUp" => (0.0, -1.0),
                    _ => (0.0, 1.0),
                };
                let mut best: Option<(usize, f64)> = None;
                for &j in &candidates {
                    if Some(j) == cur {
                        continue;
                    }
                    let (jx, jy) = pos[j];
                    let dx = jx - cx;
                    let dy = jy - cy;
                    let dot = dx * dir.0 + dy * dir.1;
                    if dot <= 0.0 {
                        continue;
                    }
                    let dist = (dx * dx + dy * dy).sqrt();
                    // Bias towards the direction axis.
                    let score = dist / dot;
                    if best.map_or(true, |(_, bd)| score < bd) {
                        best = Some((j, score));
                    }
                }
                if let Some((j, _)) = best {
                    set_selected.set(Some(j));
                    // Centre the selection.
                    let (jx, jy) = view.world_to_screen(pos[j].0, pos[j].1);
                    let (ox, oy) = offset.get();
                    set_offset.set((ox + (canvas_w.get() / 2.0 - jx), oy + (canvas_h.get() / 2.0 - jy)));
                }
            }
            "Enter" => {
                if let Some(i) = selected.get() {
                    if let Some(g) = graph.get() {
                        if let Some(node) = g.nodes.get(i) {
                            open_tab(ws, &node.path);
                        }
                    }
                }
            }
            "Escape" => {
                set_selected.set(None);
                set_focus_mode.set(false);
            }
            "f" => {
                set_focus_mode.update(|v| *v = !*v);
            }
            "+" | "=" => {
                set_zoom.update(|z| *z = (*z * 1.2).clamp(0.2, 4.0));
            }
            "-" | "_" => {
                set_zoom.update(|z| *z = (*z / 1.2).clamp(0.2, 4.0));
            }
            "0" => {
                fit.run(());
            }
            _ => {}
        }
    });
    on_cleanup(move || key_handle.remove());

    // ── Relationship panel actions ───────────────────────────────────
    let open_note = move |path: String| open_tab(ws, &path);

    let link_mention_action = move |path: String, title: String| {
        let toasts = toasts;
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                "path": path,
                "title": title,
            }))
            .unwrap();
            let res = crate::ipc::tauri_invoke("link_mention", args).await;
            if serde_wasm_bindgen::from_value::<String>(res).is_ok() {
                toasts.success("Linked", format!("[[{title}]] added to the note"));
                // Tell the editor this file changed on disk so it reloads
                // instead of autosaving the stale buffer over the new link.
                crate::components::workspace::bump_content_version(ws, &path);
                set_links_version.update(|v| *v += 1);
            } else {
                toasts.error("Link mention", "Could not convert that mention into a link");
            }
        });
    };

    let ignore_mention_action = move |title: String| {
        let toasts = toasts;
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "title": title })).unwrap();
            let res = crate::ipc::tauri_invoke("mention_ignore", args).await;
            if serde_wasm_bindgen::from_value::<()>(res).is_ok() {
                toasts.info("Ignored", format!("{title} will no longer be suggested"));
                set_links_version.update(|v| *v += 1);
            } else {
                toasts.error("Ignore mention", "Could not save the ignore preference");
            }
        });
    };

    let open_external = move |url: String| {
        if let Some(window) = web_sys::window() {
            let _ = window.open_with_url_and_target(&url, "_blank");
        }
    };

    let copy_wikilink = move |title: String| {
        let toasts = toasts;
        spawn_local(async move {
            if let Some(window) = web_sys::window() {
                let cb = window.navigator().clipboard();
                let _ = cb.write_text(&format!("[[{title}]]"));
                toasts.success("Copied", format!("[[{title}]]"));
            }
        });
    };

    // Callback wrappers so the relationship panel's reactive children
    // closures only capture Copy values (keeps the view closure `Fn` — a
    // non-Copy closure moved into a nested `move` child would make it FnOnce
    // and break Leptos' `ToChildren`/`ChildrenFn` bounds).
    let open_note_cb = Callback::new(open_note);
    let link_mention_action_cb =
        Callback::new(move |(path, title): (String, String)| link_mention_action(path, title));
    let ignore_mention_cb = Callback::new(ignore_mention_action);
    let open_external_cb = Callback::new(open_external);
    let copy_wikilink_cb = Callback::new(copy_wikilink);

    // ── Folder / tag filter options ──────────────────────────────────
    let folders = move || {
        let mut set = std::collections::BTreeSet::new();
        if let Some(g) = graph.get() {
            for n in &g.nodes {
                if !n.folder.is_empty() {
                    set.insert(n.folder.clone());
                }
            }
        }
        set.into_iter().collect::<Vec<_>>()
    };
    let tags = move || {
        let mut set = std::collections::BTreeSet::new();
        if let Some(g) = graph.get() {
            for n in &g.nodes {
                for t in &n.tags {
                    set.insert(t.clone());
                }
            }
        }
        set.into_iter().collect::<Vec<_>>()
    };

    // ── Selected node + stats for the side panel ─────────────────────
    let selected_node = move || {
        let g = graph.get()?;
        let i = selected.get()?;
        g.nodes.get(i).cloned()
    };

    view! {
        <div class="graph-view relative w-full h-full overflow-hidden bg-gray-950">
            // Loading state
            {move || if graph.get().is_none() {
                if let Some(err) = load_error.get() {
                    view! {
                        <div class="absolute inset-0 flex items-center justify-center p-8">
                            <div class="w-full max-w-md">
                                <crate::components::ui::feedback::ErrorPanel
                                    title="Couldn't load the graph".to_string()
                                    message=err
                                    details="graph_data IPC failed".to_string()
                                    on_retry=retry
                                ></crate::components::ui::feedback::ErrorPanel>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <div class="absolute inset-0 flex items-center justify-center">
                            <LoadingBlock label="Building graph…" size=SpinnerSize::Lg />
                        </div>
                    }.into_any()
                }
            } else {
                view! {}.into_any()
            }}

            // Empty state
            {move || if let Some(g) = graph.get() {
                if g.nodes.is_empty() {
                    view! {
                        <div class="absolute inset-0 flex items-center justify-center">
                            <EmptyState
                                icon=crate::components::ui::icons::Icon::Network
                                title="No connections yet".to_string()
                                description="Create a few notes with [[links]] and they will appear here as a knowledge graph.".to_string()
                            ></EmptyState>
                        </div>
                    }.into_any()
                } else {
                    view! {}.into_any()
                }
            } else {
                view! {}.into_any()
            }}

            // Toolbar
            <div class="graph-toolbar absolute top-3 left-3 z-10 flex flex-col gap-2 bg-gray-900/90 border border-gray-800 rounded-lg p-2 shadow-lg w-64">
                <div class="flex items-center gap-2">
                    <input
                        type="text"
                        placeholder="Search nodes…"
                        class="input text-xs flex-1"
                        prop:value=search
                        on:input=move |ev| set_search.set(event_target_value(&ev))
                    />
                    <button
                        class=move || format!("btn btn-sm {}", if focus_mode.get() { "btn-primary" } else { "btn-ghost" })
                        title="Focus mode: highlight a node and its neighbours"
                        on:click=move |_| set_focus_mode.update(|v| *v = !*v)
                    >"◎"</button>
                </div>
                <div class="flex gap-2 text-xs">
                    <select
                        class="input text-xs flex-1"
                        prop:value=folder_filter
                        on:change=move |ev| set_folder_filter.set(event_target_value(&ev))
                    >
                        <option value="">"All folders"</option>
                        {move || folders().into_iter().map(|f| {
                            let f2 = f.clone();
                            view! { <option value=f2>{f}</option> }
                        }).collect_view()}
                    </select>
                    <select
                        class="input text-xs flex-1"
                        prop:value=tag_filter
                        on:change=move |ev| set_tag_filter.set(event_target_value(&ev))
                    >
                        <option value="">"All tags"</option>
                        {move || tags().into_iter().map(|t| {
                            let t2 = t.clone();
                            view! { <option value=t2>"#" {t}</option> }
                        }).collect_view()}
                    </select>
                </div>
                <div class="flex items-center gap-2 text-[11px] text-gray-400">
                    <span>Zoom</span>
                    <button class="btn btn-xs btn-ghost" on:click=move |_| set_zoom.update(|z| *z = (*z / 1.25).clamp(0.2, 4.0))>"−"</button>
                    {move || format!("{:.0}%", zoom.get() * 100.0)}
                    <button class="btn btn-xs btn-ghost" on:click=move |_| set_zoom.update(|z| *z = (*z * 1.25).clamp(0.2, 4.0))>"+"</button>
                    <button class="btn btn-xs btn-ghost" on:click=move |_| fit.run(())>"Fit"</button>
                </div>
                <div class="text-[11px] text-gray-500">
                    {move || if let Some(g) = graph.get() {
                        format!("{} notes · {} links · {} orphans · {} clusters", g.nodes.len(), g.edges.len(), g.orphan_count, g.cluster_count)
                    } else { String::new() }}
                </div>
            </div>

            // Canvas
            <canvas
                node_ref=canvas_ref
                class="graph-canvas absolute inset-0 w-full h-full"
                on:mousedown=on_mousedown
                on:mousemove=on_mousemove
                on:mouseup=on_mouseup
                on:mouseleave=on_leave
                on:dblclick=on_dblclick
                on:wheel=on_wheel
            ></canvas>

            // Relationship panel
            {move || {
                let node = selected_node();
                let Some(node) = node else { return view! {}.into_any() };
                // Per-node clones threaded through Copy callbacks so the
                // reactive children closures never move `node`/`links` out of
                // this closure's environment (which would make it FnOnce).
                let node_path_open = node.path.clone();
                let node_path_link = node.path.clone();
                let node_title_copy = node.title.clone();
                let open_node_cb = Callback::new(move |_| open_note_cb.run(node_path_open.clone()));
                let copy_link_cb =
                    Callback::new(move |_| copy_wikilink_cb.run(node_title_copy.clone()));
                let link_mention_cb = Callback::new(move |title: String| {
                    link_mention_action_cb.run((node_path_link.clone(), title))
                });
                view! {
                    <div class="graph-side-panel absolute right-3 top-3 bottom-3 z-10 w-80 bg-gray-900/95 border border-gray-800 rounded-lg shadow-xl overflow-y-auto">
                        <div class="px-3 py-2 border-b border-gray-800 flex items-center justify-between">
                            <div class="min-w-0">
                                <div class="text-sm font-semibold text-gray-100 truncate">{node.title.clone()}</div>
                                <div class="text-[11px] text-gray-500 truncate">
                                    {if node.folder.is_empty() { "vault root".to_string() } else { node.folder.clone() }}
                                </div>
                            </div>
                            <button class="btn btn-xs btn-ghost" on:click=move |_| set_selected.set(None)>"✕"</button>
                        </div>

                        <div class="px-3 py-2 border-b border-gray-800 grid grid-cols-3 gap-2 text-center">
                            <div>
                                <div class="text-sm font-semibold text-blue-300">{node.backlink_count}</div>
                                <div class="text-[10px] text-gray-500 uppercase tracking-wide">"Backlinks"</div>
                            </div>
                            <div>
                                <div class="text-sm font-semibold text-amber-300">{node.outgoing_count}</div>
                                <div class="text-[10px] text-gray-500 uppercase tracking-wide">"Outgoing"</div>
                            </div>
                            <div>
                                <div class="text-sm font-semibold text-purple-300">{node.degree}</div>
                                <div class="text-[10px] text-gray-500 uppercase tracking-wide">"Degree"</div>
                            </div>
                        </div>

                        <div class="px-3 py-2 flex flex-wrap gap-1">
                            <button class="btn btn-sm btn-primary" on:click=move |_| open_node_cb.run(())>"Open note"</button>
                            <button
                                class="btn btn-sm btn-ghost"
                                on:click=move |_| copy_link_cb.run(())
                            >"Copy [[link]]"</button>
                        </div>

                        <GraphPanelSection
                            title="Backlinks".to_string()
                            icon=Icon::Link
                            loaded=links.get().is_some()
                        >
                            {move || {
                                let Some(links) = links.get() else { return view! {}.into_any() };
                                if links.backlinks.is_empty() {
                                    return view! { <div class="px-3 py-2 text-xs text-gray-500">"No notes link to this one yet."</div> }.into_any();
                                }
                                view! {
                                    <div class="divide-y divide-gray-800/60">
                                        {links.backlinks.into_iter().map(|b| {
                                            view! { <BacklinkRow backlink=b open_note=open_note_cb /> }
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            }}
                        </GraphPanelSection>

                        <GraphPanelSection
                            title="Outgoing".to_string()
                            icon="➡️"
                            loaded=links.get().is_some()
                        >
                            {move || {
                                let Some(links) = links.get() else { return view! {}.into_any() };
                                if links.outgoing.is_empty() {
                                    return view! { <div class="px-3 py-2 text-xs text-gray-500">"No links written in this note."</div> }.into_any();
                                }
                                view! {
                                    <div class="divide-y divide-gray-800/60">
                                        {links.outgoing.into_iter().map(|o| {
                                            let kind = o.kind.clone();
                                            let target = o.target.clone();
                                            // Each `move` handler owns its own copy of the
                                            // target so no closure steals it from the others.
                                            let target_external = o.target.clone();
                                            let target_wikilink = o.target.clone();
                                            let count = o.count;
                                            view! {
                                                <div class="px-3 py-2 flex items-center justify-between gap-2">
                                                    <div class="min-w-0">
                                                        <div class="text-xs text-gray-300 truncate">
                                                            {if o.path.is_some() { target } else { target }}
                                                        </div>
                                                        <div class="text-[10px] text-gray-500">
                                                            {match kind.as_str() {
                                                                "internal" => "note",
                                                                "broken" => "broken link",
                                                                _ => "external URL",
                                                            }}
                                                            {if count > 1 { format!(" · ×{count}") } else { String::new() }}
                                                        </div>
                                                    </div>
                                                    <div class="flex gap-1 shrink-0">
                                                        {if let Some(path) = o.path.clone() {
                                                            view! {
                                                                <button class="btn btn-xs btn-ghost" on:click=move |_| open_note_cb.run(path.clone())>"Open"</button>
                                                            }.into_any()
                                                        } else {
                                                            view! {}.into_any()
                                                        }}
                                                        {if kind == "external" {
                                                            view! {
                                                                <button class="btn btn-xs btn-ghost" on:click=move |_| open_external_cb.run(target_external.clone())>"↗"</button>
                                                            }.into_any()
                                                        } else {
                                                            view! {}.into_any()
                                                        }}
                                                        <button class="btn btn-xs btn-ghost" on:click=move |_| copy_wikilink_cb.run(target_wikilink.clone())>"⧉"</button>
                                                    </div>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            }}
                        </GraphPanelSection>

                        <GraphPanelSection
                            title="Unlinked mentions".to_string()
                            icon="💬"
                            loaded=links.get().is_some()
                        >
                            {move || {
                                let Some(links) = links.get() else { return view! {}.into_any() };
                                if links.mentions.is_empty() {
                                    return view! { <div class="px-3 py-2 text-xs text-gray-500">"No plain-text mentions of other notes."</div> }.into_any();
                                }
                                view! {
                                    <div class="divide-y divide-gray-800/60">
                                        {links.mentions.into_iter().map(|m| {
                                            let title = m.title.clone();
                                            let title_link = title.clone();
                                            let title_ignore = title.clone();
                                            let snippet = m.snippet.clone();
                                            let s = m.match_start;
                                            let e = m.match_end;
                                            view! {
                                                <div class="px-3 py-2">
                                                    <div class="flex items-center justify-between gap-2">
                                                        <span class="text-xs font-medium text-gray-300 truncate">{title}</span>
                                                        <div class="flex gap-1 shrink-0">
                                                            <button class="btn btn-xs btn-primary" on:click=move |_| link_mention_cb.run(title_link.clone())>"Link"</button>
                                                            <button class="btn btn-xs btn-ghost" on:click=move |_| ignore_mention_cb.run(title_ignore.clone())>"Ignore"</button>
                                                        </div>
                                                    </div>
                                                    <div class="text-[11px] text-gray-500 mt-1 leading-snug">
                                                        <MentionSnippet snippet=snippet match_start=s match_end=e />
                                                    </div>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                }.into_any()
                            }}
                        </GraphPanelSection>
                    </div>
                }.into_any()
            }}
        </div>
    }
}

/// A collapsible section header for the relationship panel.
#[component]
fn GraphPanelSection(
    title: String,
    icon: Icon,
    loaded: bool,
    children: ChildrenFn,
) -> impl IntoView {
    let (open, set_open) = signal(true);
    view! {
        <div class="border-b border-gray-800/60">
            <button
                class="w-full px-3 py-2 flex items-center justify-between text-xs font-semibold text-gray-400 hover:text-gray-200"
                on:click=move |_| set_open.update(|v| *v = !*v)
            >
                <span>{render_icon_view(icon)} " " {title}</span>
                <span class="text-gray-600">{move || if open.get() { render_icon_view(Icon::ChevronDown) } else { render_icon_view(Icon::ChevronRight) }}</span>
            </button>
            {move || if open.get() {
                if loaded {
                    children().into_any()
                } else {
                    view! { <div class="px-3 py-2 text-xs text-gray-600">"Loading…"</div> }.into_any()
                }
            } else {
                view! {}.into_any()
            }}
        </div>
    }
}

/// A single backlink row with the linking note + context snippet.
#[component]
fn BacklinkRow(backlink: BacklinkEntry, open_note: Callback<String>) -> impl IntoView {
    let path = backlink.path.clone();
    let title = backlink.title.clone();
    let folder = backlink.folder.clone();
    let count = backlink.count;
    let snippet = backlink.snippet.clone();
    let s = backlink.match_start;
    let e = backlink.match_end;
    view! {
        <div class="px-3 py-2">
            <button
                class="w-full text-left"
                on:click=move |_| open_note.run(path.clone())
            >
                <div class="flex items-center justify-between gap-2">
                    <span class="text-xs font-medium text-blue-300 truncate">{title}</span>
                    {if count > 1 { view! { <span class="text-[10px] text-gray-500 shrink-0">{format!("×{count}")}</span> }.into_any() } else { view! {}.into_any() }}
                </div>
                <div class="text-[10px] text-gray-600 truncate">
                    {if folder.is_empty() { "vault root".to_string() } else { folder }}
                </div>
            </button>
            <div class="text-[11px] text-gray-500 mt-1 leading-snug">
                <MentionSnippet snippet=snippet match_start=s match_end=e />
            </div>
        </div>
    }
}

/// Renders a snippet with the matched span highlighted. Shared with the
/// right inspector so backlink / mention context renders identically.
#[component]
pub(crate) fn MentionSnippet(
    snippet: String,
    match_start: usize,
    match_end: usize,
) -> impl IntoView {
    let chars: Vec<char> = snippet.chars().collect();
    let s = match_start.min(chars.len());
    let e = match_end.clamp(s, chars.len());
    let before: String = chars[..s].iter().collect();
    let matched: String = chars[s..e].iter().collect();
    let after: String = chars[e..].iter().collect();
    view! {
        <span>
            {before}
            <mark class="mention-mark">{matched}</mark>
            {after}
        </span>
    }
}
