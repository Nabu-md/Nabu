//! Knowledge Inbox UI component (Dioxus migration).
//!
//! Split-pane interface for reviewing, organising, approving, and processing
//! captured knowledge before final storage. State is driven by EventBus events
//! via Tauri IPC — no polling.

use crate::components::ui::button::{Button, ButtonVariant};
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::info::EmptyState;
use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

// ── Types ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InboxStatus {
    Pending,
    Processing,
    Ready,
    Approved,
    Rejected,
    Failed,
}

impl Default for InboxStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct InboxItem {
    pub id: String,
    pub title: String,
    pub object_type: String,
    pub source: String,
    pub status: InboxStatus,
    pub mime_type: Option<String>,
    pub source_file: Option<String>,
    pub thumbnail: Option<String>,
    pub confidence: Option<f64>,
    pub suggested_folder: Option<String>,
    pub metadata: InboxMetadata,
    pub duplicate_info: Option<DuplicateInfo>,
    pub timeline_info: Option<TimelineInfo>,
    pub ocr_info: Option<OcrInfo>,
    pub processing_history: Vec<ProcessingHistoryEntry>,
    pub warnings: Vec<String>,
    pub selected: bool,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct InboxMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub source_url: Option<String>,
    pub tags: Vec<String>,
    pub custom: HashMap<String, serde_json::Value>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct DuplicateInfo {
    pub confidence: String,
    pub candidate_ids: Vec<String>,
    pub reason: Option<String>,
    pub duplicate_source: Option<String>,
    pub content_hash: Option<String>,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct TimelineInfo {
    pub document_date: Option<String>,
    pub created_date: Option<String>,
    pub modified_date: Option<String>,
    pub detected_event_date: Option<String>,
    pub extraction_confidence: Option<String>,
}

#[derive(Clone, PartialEq, Debug, Default, Serialize, Deserialize)]
pub struct OcrInfo {
    pub extracted_text: Option<String>,
    pub confidence: Option<f64>,
    pub recognition_language: Option<String>,
    pub page_count: Option<u32>,
    pub processing_duration_ms: Option<u64>,
    pub is_scanned: Option<bool>,
    pub warning: Option<String>,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct ProcessingHistoryEntry {
    pub processor_name: String,
    pub timestamp: String,
    pub duration_ms: u64,
    pub success: bool,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}

// ── State ────────────────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct InboxState {
    pub items: Vec<InboxItem>,
    pub selected_ids: Vec<String>,
    pub preview_id: Option<String>,
    pub filter: String,
    pub sort_by: SortField,
    pub sort_ascending: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum SortField {
    #[default]
    Timestamp,
    Title,
    Source,
    Status,
    ObjectType,
}

// ── Inbox Actions ────────────────────────────────────────────────────────────

fn approve_item(id: String, toasts: crate::components::ui::feedback::ToastContext) {
    spawn_local(async move {
        let result = crate::ipc::tauri_invoke(
            "inbox_approve",
            serde_wasm_bindgen::to_value(&serde_json::json!({"id": id})).unwrap(),
        )
        .await;
        if serde_wasm_bindgen::from_value::<()>(result).is_err() {
            toasts.error("Approve", "Could not approve that capture");
        }
    });
}

fn reject_item(id: String, toasts: crate::components::ui::feedback::ToastContext) {
    spawn_local(async move {
        let result = crate::ipc::tauri_invoke(
            "inbox_reject",
            serde_wasm_bindgen::to_value(&serde_json::json!({"id": id, "reason": "User rejected"}))
                .unwrap(),
        )
        .await;
        if serde_wasm_bindgen::from_value::<()>(result).is_err() {
            toasts.error("Reject", "Could not reject that capture");
        }
    });
}

fn retry_item(id: String, toasts: crate::components::ui::feedback::ToastContext) {
    spawn_local(async move {
        let result = crate::ipc::tauri_invoke(
            "inbox_retry",
            serde_wasm_bindgen::to_value(&serde_json::json!({"id": id})).unwrap(),
        )
        .await;
        if serde_wasm_bindgen::from_value::<()>(result).is_err() {
            toasts.error("Retry", "Could not retry that capture");
        }
    });
}

fn delete_item(id: String, toasts: crate::components::ui::feedback::ToastContext) {
    spawn_local(async move {
        let result = crate::ipc::tauri_invoke(
            "inbox_delete",
            serde_wasm_bindgen::to_value(&serde_json::json!({"id": id})).unwrap(),
        )
        .await;
        if serde_wasm_bindgen::from_value::<()>(result).is_err() {
            toasts.error("Delete", "Could not delete that capture");
        }
    });
}

/// Maps a thumbnail icon name (from the backend) to the frontend [`Icon`] enum.
fn thumbnail_to_icon(thumbnail: &str) -> Icon {
    match thumbnail {
        "image" => Icon::Image,
        "music-3" => Icon::Music,
        "play" => Icon::Play,
        "code-block" => Icon::CodeBlock,
        "code" => Icon::CodeBlock,
        "mail" => Icon::Mail,
        "bookmark" => Icon::Bookmark,
        "book-text" => Icon::BookText,
        "file-text" => Icon::FileText,
        "pen-line" => Icon::PenLine,
        "user" => Icon::User,
        "calendar" => Icon::Calendar,
        "list-checks" => Icon::ListChecks,
        "folder" => Icon::Folder,
        "file-pen" => Icon::FilePen,
        "sticky-note" => Icon::StickyNote,
        "folder-tree" => Icon::FolderTree,
        "layout-dashboard" => Icon::Dashboard,
        "dashboard" => Icon::Dashboard,
        "file" => Icon::File,
        _ => Icon::File,
    }
}

/// Returns a Tailwind colour class for a confidence score (0.0–1.0).
fn confidence_color(score: f64) -> &'static str {
    match score {
        s if s >= 0.8 => "text-green-400",
        s if s >= 0.5 => "text-amber-400",
        s if s >= 0.3 => "text-orange-400",
        _ => "text-red-400",
    }
}

/// Renders the confidence bar — a small horizontal bar with a width
/// proportional to the score, coloured by confidence level.
fn confidence_bar(score: f64) -> Element {
    let pct = ((score * 100.0).round() as i64).clamp(0, 100);
    let bar_class = match score {
        s if s >= 0.8 => "bg-green-400",
        s if s >= 0.5 => "bg-amber-400",
        s if s >= 0.3 => "bg-orange-400",
        _ => "bg-red-400",
    };
    rsx! {
        div { class: "w-full bg-gray-700 rounded h-1.5 mt-1 overflow-hidden" }
        div {
            class: "{bar_class} h-1.5 rounded transition-all",
            style: "width: {pct}%",
        }
    }
}

/// Captures a file dropped onto the inbox, routing it through the canonical
/// CaptureEngine via the `capture_file_drop` Tauri command.
fn capture_file_drop(file: web_sys::File, toasts: crate::components::ui::feedback::ToastContext) {
    let filename = file.name();
    let mime_type = file.type_();
    spawn_local(async move {
        let array_buffer = match file.array_buffer().await {
            Ok(buf) => buf,
            Err(_) => {
                toasts.error("File Drop", "Could not read the dropped file");
                return;
            }
        };
        let data = js_sys::Uint8Array::new(&array_buffer).to_vec();
        #[derive(serde::Serialize)]
        struct FileDropArgs {
            filename: String,
            mime_type: String,
            data: Vec<u8>,
        }
        let mime = if mime_type.is_empty() {
            "application/octet-stream".to_string()
        } else {
            mime_type
        };
        let args = serde_wasm_bindgen::to_value(&FileDropArgs {
            filename: filename.clone(),
            mime_type: mime,
            data,
        })
        .unwrap();
        let result = crate::ipc::tauri_invoke("capture_file_drop", args).await;
        match serde_wasm_bindgen::from_value::<String>(result) {
            Ok(id) => toasts.success("File Drop", format!("Captured '{}' to inbox", id)),
            Err(_) => toasts.error("File Drop", "Could not capture the dropped file"),
        }
    });
}

// ── Inbox Component ──────────────────────────────────────────────────────────

#[component]
pub fn Inbox() -> Element {
    let mut state = use_signal(|| InboxState::default());
    let toasts = crate::components::ui::feedback::use_toast();
    let mut drag_over = use_signal(|| false);

    // Subscribe to EventBus events via Tauri IPC (no polling)
    spawn_local(async move {
        let _ = crate::ipc::tauri_invoke(
            "inbox_subscribe",
            serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap(),
        )
        .await;
    });

    let mut toggle_select_item = move |id: String| {
        state.with_mut(|s| {
            if let Some(item) = s.items.iter_mut().find(|i| i.id == id) {
                item.selected = !item.selected;
            }
        });
    };

    let mut set_preview = move |id: String| {
        state.with_mut(|s| s.preview_id = Some(id));
    };

    let mut on_sort_change = move |field: SortField| {
        state.with_mut(|s| {
            if s.sort_by == field {
                s.sort_ascending = !s.sort_ascending;
            } else {
                s.sort_by = field;
                s.sort_ascending = true;
            }
        });
    };

    // Batch action handlers — use raw closures (Signal and ToastContext are Copy)
    let batch_approve = move |_: MouseEvent| {
        let ids: Vec<String> = state
            .read()
            .items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.id.clone())
            .collect();
        if !ids.is_empty() {
            let toasts_b = toasts;
            spawn_local(async move {
                let result = crate::ipc::tauri_invoke(
                    "inbox_batch_approve",
                    serde_wasm_bindgen::to_value(&serde_json::json!({"ids": ids})).unwrap(),
                )
                .await;
                if serde_wasm_bindgen::from_value::<()>(result).is_err() {
                    toasts_b.error("Approve", "Could not approve the selected captures");
                }
            });
        }
    };

    let batch_reject = move |_: MouseEvent| {
        let ids: Vec<String> = state
            .read()
            .items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.id.clone())
            .collect();
        if !ids.is_empty() {
            let toasts_b = toasts;
            spawn_local(async move {
                let result = crate::ipc::tauri_invoke(
                    "inbox_batch_reject",
                    serde_wasm_bindgen::to_value(&serde_json::json!({
                        "ids": ids,
                        "reason": "User rejected"
                    }))
                    .unwrap(),
                )
                .await;
                if serde_wasm_bindgen::from_value::<()>(result).is_err() {
                    toasts_b.error("Reject", "Could not reject the selected captures");
                }
            });
        }
    };

    let batch_delete = move |_: MouseEvent| {
        let ids: Vec<String> = state
            .read()
            .items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.id.clone())
            .collect();
        if !ids.is_empty() {
            let toasts_b = toasts;
            spawn_local(async move {
                let result = crate::ipc::tauri_invoke(
                    "inbox_batch_delete",
                    serde_wasm_bindgen::to_value(&serde_json::json!({"ids": ids})).unwrap(),
                )
                .await;
                if serde_wasm_bindgen::from_value::<()>(result).is_err() {
                    toasts_b.error("Delete", "Could not delete the selected captures");
                }
            });
        }
    };

    let batch_retry = move |_: MouseEvent| {
        let ids: Vec<String> = state
            .read()
            .items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.id.clone())
            .collect();
        if !ids.is_empty() {
            let toasts_b = toasts;
            spawn_local(async move {
                let result = crate::ipc::tauri_invoke(
                    "inbox_batch_retry",
                    serde_wasm_bindgen::to_value(&serde_json::json!({"ids": ids})).unwrap(),
                )
                .await;
                if serde_wasm_bindgen::from_value::<()>(result).is_err() {
                    toasts_b.error("Retry", "Could not retry the selected captures");
                }
            });
        }
    };

    // ── Keyboard shortcuts (inbox-scoped) ────────────────────────────────
    let on_keydown = move |ev: KeyboardEvent| {
        let web = ev.as_web_event();
        // Skip if typing in an input — the search box handles its own keys.
        let tag = web.target().and_then(|target| {
            let el: web_sys::Element = target.dyn_into().ok()?;
            Some(el.tag_name().to_ascii_lowercase())
        });
        if tag.as_deref() == Some("input") || tag.as_deref() == Some("textarea") {
            return;
        }
        let meta = web.meta_key() || web.ctrl_key();
        let shift = web.shift_key();
        let key = web.key();

        if meta && key.eq_ignore_ascii_case("a") {
            ev.prevent_default();
            state.with_mut(|s| {
                let all_selected = s.items.iter().all(|i| i.selected);
                for item in &mut s.items {
                    item.selected = !all_selected;
                }
            });
        } else if !meta && key.eq_ignore_ascii_case("a") {
            ev.prevent_default();
            let ids: Vec<String> = state
                .read()
                .items
                .iter()
                .filter(|i| i.selected)
                .map(|i| i.id.clone())
                .collect();
            if !ids.is_empty() {
                let toasts_k = toasts;
                spawn_local(async move {
                    let result = crate::ipc::tauri_invoke(
                        "inbox_batch_approve",
                        serde_wasm_bindgen::to_value(&serde_json::json!({"ids": ids}))
                            .unwrap(),
                    )
                    .await;
                    if serde_wasm_bindgen::from_value::<()>(result).is_err() {
                        toasts_k.error("Approve", "Could not approve the selected captures");
                    }
                });
            }
        } else if !meta && key.eq_ignore_ascii_case("r") {
            ev.prevent_default();
            let ids: Vec<String> = state
                .read()
                .items
                .iter()
                .filter(|i| i.selected)
                .map(|i| i.id.clone())
                .collect();
            if !ids.is_empty() {
                let toasts_k = toasts;
                spawn_local(async move {
                    let result = crate::ipc::tauri_invoke(
                        "inbox_batch_reject",
                        serde_wasm_bindgen::to_value(&serde_json::json!({
                            "ids": ids,
                            "reason": "User rejected"
                        }))
                        .unwrap(),
                    )
                    .await;
                    if serde_wasm_bindgen::from_value::<()>(result).is_err() {
                        toasts_k.error("Reject", "Could not reject the selected captures");
                    }
                });
            }
        } else if !meta && key.eq_ignore_ascii_case("d") {
            ev.prevent_default();
            let ids: Vec<String> = state
                .read()
                .items
                .iter()
                .filter(|i| i.selected)
                .map(|i| i.id.clone())
                .collect();
            if !ids.is_empty() {
                let toasts_k = toasts;
                spawn_local(async move {
                    let result = crate::ipc::tauri_invoke(
                        "inbox_batch_delete",
                        serde_wasm_bindgen::to_value(&serde_json::json!({"ids": ids}))
                            .unwrap(),
                    )
                    .await;
                    if serde_wasm_bindgen::from_value::<()>(result).is_err() {
                        toasts_k.error("Delete", "Could not delete the selected captures");
                    }
                });
            }
        } else if !meta && (key == " " || key == "Spacebar") {
            ev.prevent_default();
            let id = state.read().preview_id.clone();
            if let Some(id) = id {
                toggle_select_item(id);
            }
        } else if !meta && (key == "Enter" || key == "ArrowRight") {
            ev.prevent_default();
            let preview_id = state.read().preview_id.clone();
            if let Some(id) = preview_id {
                approve_item(id, toasts);
            }
        } else if meta && shift && (key == "ArrowLeft" || key == "ArrowRight") {
            ev.prevent_default();
            state.with_mut(|s| {
                s.sort_ascending = !s.sort_ascending;
            });
        }
    };

    // ── Drag & drop over the inbox container ──────────────────────────
    let on_dragover = move |ev: DragEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        drag_over.set(true);
    };
    let on_dragleave = move |ev: DragEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        drag_over.set(false);
    };
    let on_drop = move |ev: DragEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        drag_over.set(false);
        let web = ev.as_web_event();
        if let Some(dt) = web.data_transfer() {
            if let Some(file_list) = dt.files() {
                let len = file_list.length();
                for i in 0..len {
                    if let Some(file) = file_list.item(i) {
                        capture_file_drop(file, toasts);
                    }
                }
            }
        }
    };

    // Compute filtered + sorted items during render.
    let filtered_items: Vec<InboxItem> = {
        let s = state.read();
        let mut items = s.items.clone();
        if !s.filter.is_empty() {
            let f = s.filter.to_lowercase();
            items.retain(|i| {
                i.title.to_lowercase().contains(&f)
                    || i.source.to_lowercase().contains(&f)
                    || i.object_type.to_lowercase().contains(&f)
            });
        }
        items.sort_by(|a, b| {
            let ord = match s.sort_by {
                SortField::Timestamp => a.id.cmp(&b.id),
                SortField::Title => a.title.cmp(&b.title),
                SortField::Source => a.source.cmp(&b.source),
                SortField::Status => a.status.cmp(&b.status),
                SortField::ObjectType => a.object_type.cmp(&b.object_type),
            };
            if s.sort_ascending { ord } else { ord.reverse() }
        });
        items
    };

    // Read state values needed for rendering.
    let selected_count = state.read().items.iter().filter(|i| i.selected).count();
    let total_count = state.read().items.len();
    let current_filter = state.read().filter.clone();
    let preview_id = state.read().preview_id.clone();
    let drag_over_val = *drag_over.read();
    let drag_class = if drag_over_val { "drag-over" } else { "" };

    rsx! {
        div {
            class: "inbox flex h-full bg-gray-950 text-gray-100 overflow-hidden relative {drag_class}",
            tabindex: "0",
            onkeydown: on_keydown,
            ondragover: on_dragover,
            ondragleave: on_dragleave,
            ondrop: on_drop,

            // ── Left panel: Queue ──
            div { class: "flex-none w-96 border-r border-gray-800 flex flex-col" }

            div { class: "flex items-center gap-2 p-3 border-b border-gray-800" }
            input {
                r#type: "text",
                placeholder: "Search inbox...",
                class: "flex-1 bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none",
                value: "{current_filter}",
                oninput: move |ev: FormEvent| {
                    state.with_mut(|s| s.filter = ev.value());
                },
            }

            // Batch actions bar
            {if selected_count > 0 {
                rsx! {
                    div { class: "flex items-center gap-2 px-3 py-2 bg-blue-900/30 border-b border-blue-800/30" }
                    span { class: "text-xs text-blue-400", "{selected_count} selected" }
                    button {
                        class: "px-2 py-1 text-xs bg-green-700 rounded hover:bg-green-600",
                        onclick: batch_approve,
                        "Approve"
                    }
                    button {
                        class: "px-2 py-1 text-xs bg-red-700 rounded hover:bg-red-600",
                        onclick: batch_reject,
                        "Reject"
                    }
                    button {
                        class: "px-2 py-1 text-xs bg-yellow-700 rounded hover:bg-yellow-600",
                        onclick: batch_retry,
                        "Retry"
                    }
                    button {
                        class: "px-2 py-1 text-xs bg-gray-700 rounded hover:bg-gray-600",
                        onclick: batch_delete,
                        "Delete"
                    }
                }
            } else { rsx! {} }}

            // Queue list
            div { class: "flex-1 overflow-y-auto" }
            {if filtered_items.is_empty() {
                rsx! {
                    div { class: "h-full flex items-center justify-center p-6" }
                    EmptyState {
                        icon: Some(Icon::Inbox),
                        title: "Inbox is empty".to_string(),
                        description: Some("Captured knowledge appears here, ready to review and file into your vault.".to_string()),
                    }
                }
            } else {
                rsx! {
                    div { class: "divide-y divide-gray-800" }
                    for item in &filtered_items {
                        {
                            let id = item.id.clone();
                            let dbl_id = id.clone();
                            let title = item.title.clone();
                            let source = item.source.clone();
                            let object_type = item.object_type.clone();
                            let selected = item.selected;
                            let warnings = item.warnings.clone();
                            let duplicate_info = item.duplicate_info.clone();
                            let ocr_info = item.ocr_info.clone();
                            let thumbnail = item.thumbnail.clone();
                            let confidence = item.confidence;
                            let suggested_folder = item.suggested_folder.clone();
                            let is_selected = selected;
                            let has_warnings = !warnings.is_empty();
                            let has_dup = duplicate_info.is_some();
                            let has_ocr_warning = ocr_info.as_ref().map_or(false, |o| o.warning.is_some());
                            let icon = thumbnail.as_deref().map(thumbnail_to_icon).unwrap_or(Icon::File);
                            let border_class = if !is_selected {
                                if has_warnings && has_dup { "border-l-yellow-500" }
                                else if has_warnings && has_ocr_warning { "border-l-orange-500" }
                                else { "border-l-transparent" }
                            } else { "border-l-transparent" };
                            let item_class = if is_selected { "bg-gray-800 border-l-blue-500" } else { border_class };
                            rsx! {
                                div {
                                    class: "inbox-item px-3 py-2 cursor-pointer hover:bg-gray-800 border-l-2 transition-colors {item_class}",
                                    onclick: move |_: MouseEvent| {
                                        toggle_select_item(id.clone());
                                    },
                                    ondoubleclick: move |_: MouseEvent| {
                                        set_preview(dbl_id.clone());
                                    },
                                    div { class: "flex items-center gap-2" }
                                    span {
                                        class: "flex-shrink-0 w-5 h-5 text-gray-400",
                                        "aria-hidden": "true",
                                        {render_icon_view(icon)}
                                    }
                                    span { class: "text-sm font-medium truncate max-w-36", "{title}" }
                                }
                                div { class: "flex items-center gap-2 mt-1 ml-7" }
                                span { class: "text-xs text-gray-500", "{source}" }
                                span { class: "text-xs text-gray-600", "\u{2022}" }
                                span { class: "text-xs text-gray-500", "{object_type}" }
                                {suggested_folder.as_ref().map(|folder| rsx! {
                                    span { class: "text-xs text-blue-400 mt-1 block", {render_icon_view(Icon::MapPin)} " {folder}" }
                                })}
                                {if !warnings.is_empty() {
                                    rsx! {
                                        span {
                                            class: "text-xs text-yellow-400",
                                            {render_icon_view(Icon::Warning)}
                                            " {warnings.len()} warning(s)"
                                        }
                                    }
                                } else { rsx! {} }}
                                {confidence.map(|score| {
                                    let colour = confidence_color(score);
                                    let pct = ((score * 100.0).round() as i64).clamp(0, 100);
                                    rsx! {
                                        div { class: "ml-7 mt-1 w-full max-w-[120px]" }
                                        {confidence_bar(score)}
                                        span { class: "{colour} text-xs", "{pct}% confidence" }
                                    }
                                })}
                            }
                        }
                    }
                }
            }}

            // Footer
            div { class: "flex items-center justify-between px-3 py-2 border-t border-gray-800 text-xs text-gray-500" }
            span { "{total_count} items" }
            div { class: "flex gap-2" }
            button { class: "hover:text-gray-300", onclick: move |_: MouseEvent| on_sort_change(SortField::Timestamp), "Sort by Date" }
            button { class: "hover:text-gray-300", onclick: move |_: MouseEvent| on_sort_change(SortField::Title), "Sort by Title" }
            button { class: "hover:text-gray-300", onclick: move |_: MouseEvent| on_sort_change(SortField::Status), "Sort by Status" }
        }

        // ── Right panel: Preview ──
        div { class: "flex-1 flex flex-col overflow-hidden" }
        {if let Some(id) = &preview_id {
            {
                let item_opt = state.read().items.iter().find(|i| i.id == *id).cloned();
                match item_opt {
                    Some(item) => rsx! {
                        div { class: "flex h-full" }
                        div { class: "flex-1 overflow-y-auto p-4" }
                        InboxPreview { item: item.clone() }
                        div { class: "flex-none w-72 border-l border-gray-800 overflow-y-auto p-4" }
                        InboxMetadataSidebar { item: item }
                    },
                    None => rsx! {
                        div { class: "flex items-center justify-center h-full text-gray-500", "Select an item to preview" }
                    },
                }
            }
        } else {
            rsx! {
                div { class: "flex items-center justify-center h-full text-gray-500", "Select an item to preview" }
            }
        }}
    }
}

// ── Inbox Preview ────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum InboxPreviewTab {
    Details,
    Duplicate,
    Timeline,
    Ocr,
    History,
}

#[component]
fn InboxPreview(item: InboxItem) -> Element {
    let toasts = crate::components::ui::feedback::use_toast();
    let mut active_tab = use_signal(|| InboxPreviewTab::Details);
    let approve_id = item.id.clone();
    let reject_id = item.id.clone();
    let retry_id = item.id.clone();
    let delete_id = item.id.clone();

    let tab_labels: [(&'static str, InboxPreviewTab); 5] = [
        ("Details", InboxPreviewTab::Details),
        ("Duplicate", InboxPreviewTab::Duplicate),
        ("Timeline", InboxPreviewTab::Timeline),
        ("OCR", InboxPreviewTab::Ocr),
        ("History", InboxPreviewTab::History),
    ];

    rsx! {
        div { class: "inbox-preview" }
        div { class: "flex items-center gap-1 mb-4 border-b border-gray-800 pb-2" }
        for (label, tab_val) in tab_labels {
            {
                let is_active = *active_tab.read() == tab_val;
                let tab_class = if is_active { "bg-blue-600 text-white" } else { "text-gray-400 hover:text-gray-200" };
                rsx! {
                    button {
                        class: "px-3 py-1 text-sm rounded {tab_class}",
                        onclick: move |_: MouseEvent| { active_tab.set(tab_val); },
                        "{label}"
                    }
                }
            }
        }

        // Tab content
        div { class: "flex-1 overflow-y-auto" }
        {match *active_tab.read() {
            InboxPreviewTab::Details => rsx! { InboxDetails { item: item.clone() } },
            InboxPreviewTab::Duplicate => rsx! { InboxDuplicateReview { item: item.clone() } },
            InboxPreviewTab::Timeline => rsx! { InboxTimelineReview { item: item.clone() } },
            InboxPreviewTab::Ocr => rsx! { InboxOcrReview { item: item.clone() } },
            InboxPreviewTab::History => rsx! { InboxHistory { item: item.clone() } },
        }}

        // Action buttons
        div { class: "flex items-center gap-2 mt-4 pt-4 border-t border-gray-800" }
        button {
            class: "px-3 py-1.5 text-sm bg-green-700 rounded hover:bg-green-600",
            onclick: move |_: MouseEvent| { approve_item(approve_id.clone(), toasts); },
            {render_icon_view(Icon::CircleCheck)} " Approve"
        }
        button {
            class: "px-3 py-1.5 text-sm bg-red-700 rounded hover:bg-red-600",
            onclick: move |_: MouseEvent| { reject_item(reject_id.clone(), toasts); },
            {render_icon_view(Icon::CircleX)} " Reject"
        }
        button {
            class: "px-3 py-1.5 text-sm bg-yellow-700 rounded hover:bg-yellow-600",
            onclick: move |_: MouseEvent| { retry_item(retry_id.clone(), toasts); },
            {render_icon_view(Icon::RefreshCw)} " Retry"
        }
        button {
            class: "px-3 py-1.5 text-sm bg-gray-700 rounded hover:bg-gray-600",
            onclick: move |_: MouseEvent| { delete_item(delete_id.clone(), toasts); },
            {render_icon_view(Icon::Trash2)} " Delete"
        }
    }
}

#[component]
fn InboxDetails(item: InboxItem) -> Element {
    let classification_text = item
        .metadata
        .custom
        .get("classification")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();

    rsx! {
        div { class: "space-y-3" }
        div {}
        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Title" }
        p { class: "text-lg font-medium", "{item.title.clone()}" }
        div { class: "grid grid-cols-2 gap-3" }
        div {}
        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Type" }
        p { class: "text-sm", "{item.object_type.clone()}" }
        div {}
        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Source" }
        p { class: "text-sm", "{item.source.clone()}" }
        div {}
        label { class: "text-xs text-gray-500 uppercase tracking-wide", "MIME Type" }
        p { class: "text-sm", "{item.mime_type.clone().unwrap_or_default()}" }
        div {}
        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Source File" }
        p { class: "text-sm text-gray-400 truncate", "{item.source_file.clone().unwrap_or_default()}" }

        {if item.metadata.custom.get("classification").is_some() {
            rsx! {
                div { class: "mt-3" }
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Classification" }
                div { class: "flex items-center gap-2 mt-1" }
                span { class: "text-sm font-medium text-blue-300", "{classification_text}" }
                {item.confidence.map(|score| {
                    let colour = confidence_color(score);
                    let pct = ((score * 100.0).round() as i64).clamp(0, 100);
                    rsx! {
                        span { class: "{colour} text-xs", "{pct}% confidence" }
                        {confidence_bar(score)}
                    }
                })}
            }
        } else { rsx! {} }}

        {if let Some(folder) = item.suggested_folder.clone() {
            rsx! {
                div { class: "mt-3" }
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Suggested Destination" }
                div { class: "flex items-center gap-2 mt-1 p-2 bg-blue-900/20 border border-blue-700/30 rounded-lg" }
                {render_icon_view(Icon::MapPin)}
                span { class: "text-sm text-blue-300", "{folder}" }
                span { class: "text-xs text-gray-500", "(suggested)" }
            }
        } else { rsx! {} }}

        {if !item.warnings.is_empty() {
            rsx! {
                div { class: "mt-3" }
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Warnings" }
                ul { class: "mt-1 space-y-1" }
                for w in &item.warnings {
                    li { class: "text-sm text-yellow-400", "{w}" }
                }
            }
        } else { rsx! {} }}
    }
}

#[component]
fn InboxDuplicateReview(item: InboxItem) -> Element {
    rsx! {
        div { class: "space-y-3" }
        {if let Some(dup) = &item.duplicate_info {
            rsx! {
                div { class: "space-y-3" }
                div { class: "p-3 bg-yellow-900/20 border border-yellow-700/30 rounded-lg" }
                div { class: "flex items-center gap-2" }
                span { class: "text-yellow-400", {render_icon_view(Icon::Warning)} }
                span { class: "text-sm font-medium text-yellow-300", "Potential Duplicate Detected" }
                p { class: "text-xs text-yellow-400/70 mt-1", "{dup.reason.clone().unwrap_or_default()}" }

                div { class: "grid grid-cols-2 gap-3 text-sm" }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Confidence" }
                p { class: "text-gray-300", "{dup.confidence}" }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Content Hash" }
                p { class: "text-gray-300 font-mono text-xs truncate", "{dup.content_hash.clone().unwrap_or_default()}" }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Duplicate Source" }
                p { class: "text-gray-300 text-xs truncate", "{dup.duplicate_source.clone().unwrap_or_default()}" }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Candidates" }
                p { class: "text-gray-300", "{dup.candidate_ids.len()} found" }

                div { class: "flex gap-2" }
                button { class: "px-3 py-1.5 text-sm bg-green-700 rounded hover:bg-green-600", "Keep Both" }
                button { class: "px-3 py-1.5 text-sm bg-blue-700 rounded hover:bg-blue-600", "Replace" }
                button { class: "px-3 py-1.5 text-sm bg-gray-700 rounded hover:bg-gray-600", "Ignore" }
            }
        } else {
            rsx! {
                div { class: "text-gray-500 text-sm", "No duplicate detected for this item." }
            }
        }}
    }
}

#[component]
fn InboxTimelineReview(item: InboxItem) -> Element {
    rsx! {
        div { class: "space-y-3" }
        {if let Some(tl) = &item.timeline_info {
            rsx! {
                div { class: "space-y-3" }
                div { class: "grid grid-cols-2 gap-3 text-sm" }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Document Date" }
                p { class: "text-gray-300", "{tl.document_date.clone().unwrap_or_default()}" }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Created Date" }
                p { class: "text-gray-300", "{tl.created_date.clone().unwrap_or_default()}" }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Modified Date" }
                p { class: "text-gray-300", "{tl.modified_date.clone().unwrap_or_default()}" }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Detected Event Date" }
                p { class: "text-gray-300", "{tl.detected_event_date.clone().unwrap_or_default()}" }

                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Extraction Confidence" }
                p { class: "text-gray-300", "{tl.extraction_confidence.clone().unwrap_or_default()}" }
            }
        } else {
            rsx! {
                div { class: "text-gray-500 text-sm", "No timeline information extracted for this item." }
            }
        }}
    }
}

#[component]
fn InboxOcrReview(item: InboxItem) -> Element {
    rsx! {
        div { class: "space-y-3" }
        {if let Some(ocr) = &item.ocr_info {
            let conf_text = ocr
                .confidence
                .map_or_else(|| "N/A".to_string(), |c| format!("{:.0}%", c * 100.0));
            let pages_text = ocr
                .page_count
                .map_or_else(|| "N/A".to_string(), |p| p.to_string());
            let dur_text = ocr
                .processing_duration_ms
                .map_or_else(|| "N/A".to_string(), |d| format!("{}ms", d));
            let scanned_text = ocr
                .is_scanned
                .map_or_else(|| "N/A".to_string(), |s| s.to_string());
            rsx! {
                div { class: "space-y-3" }
                div { class: "grid grid-cols-2 gap-3 text-sm" }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Confidence" }
                p { class: "text-gray-300", "{conf_text}" }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Language" }
                p { class: "text-gray-300", "{ocr.recognition_language.clone().unwrap_or_default()}" }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Pages Processed" }
                p { class: "text-gray-300", "{pages_text}" }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Duration" }
                p { class: "text-gray-300", "{dur_text}" }
                div {}
                label { class: "text-xs text-gray-500 uppercase tracking-wide", "Scanned Document" }
                p { class: "text-gray-300", "{scanned_text}" }

                {if let Some(text) = &ocr.extracted_text {
                    rsx! {
                        div {}
                        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Extracted Text" }
                        pre {
                            class: "mt-1 text-xs text-gray-300 bg-gray-900 p-3 rounded-lg overflow-auto max-h-48 whitespace-pre-wrap",
                            "{text}"
                        }
                    }
                } else { rsx! {} }}

                {if let Some(warn) = &ocr.warning {
                    rsx! {
                        div { class: "text-sm text-orange-400 flex items-center gap-1" }
                        {render_icon_view(Icon::Warning)}
                        "{warn}"
                    }
                } else { rsx! {} }}
            }
        } else {
            rsx! {
                div { class: "text-gray-500 text-sm", "No OCR information available for this item." }
            }
        }}
    }
}

#[component]
fn InboxHistory(item: InboxItem) -> Element {
    let history = item.processing_history.clone();
    rsx! {
        div { class: "space-y-2" }
        {if history.is_empty() {
            rsx! {
                div { class: "text-gray-500 text-sm", "No processing history available." }
            }
        } else {
            rsx! {
                div { class: "space-y-2" }
                for entry in &history {
                    {
                        let success = entry.success;
                        let warnings = entry.warnings.clone();
                        let error = entry.error.clone();
                        let processor_name = entry.processor_name.clone();
                        let duration_ms = entry.duration_ms;
                        let timestamp = entry.timestamp.clone();
                        let status_class = if success { "text-green-400" } else { "text-red-400" };
                        rsx! {
                            div {
                                class: "flex items-start gap-3 p-2 rounded-lg bg-gray-900/50 text-sm",
                            }
                            span {
                                class: "mt-0.5 {status_class}",
                                {if success { render_icon_view(Icon::CircleCheck) } else { render_icon_view(Icon::CircleX) }}
                            }
                            div { class: "flex-1 min-w-0" }
                            div { class: "flex items-center justify-between" }
                            span { class: "font-medium text-gray-200", "{processor_name}" }
                            span { class: "text-xs text-gray-500", "{duration_ms}ms" }
                            div { class: "text-xs text-gray-500 mt-0.5", "{timestamp}" }
                            {if !warnings.is_empty() {
                                rsx! {
                                    ul { class: "mt-1 space-y-0.5" }
                                    for w in &warnings {
                                        li { class: "text-xs text-yellow-400", "{w}" }
                                    }
                                }
                            } else { rsx! {} }}
                            {error.as_ref().map(|err| rsx! {
                                div { class: "text-xs text-red-400 mt-1", "{err}" }
                            })}
                        }
                    }
                }
            }
        }}
    }
}

// ── Metadata Sidebar ────────────────────────────────────────────────────────

#[component]
fn InboxMetadataSidebar(item: InboxItem) -> Element {
    let mut title = use_signal(|| item.metadata.title.clone().unwrap_or_default());
    let mut author = use_signal(|| item.metadata.author.clone().unwrap_or_default());
    let mut language = use_signal(|| item.metadata.language.clone().unwrap_or_default());
    let mut tags = use_signal(|| item.metadata.tags.join(", "));
    let mut destination = use_signal(|| item.suggested_folder.clone().unwrap_or_default());
    let toasts = crate::components::ui::feedback::use_toast();
    let item_for_click = item.clone();
    let placeholder_text = item.suggested_folder.clone().unwrap_or_default();
    let reset_title = item.metadata.title.clone().unwrap_or_default();
    let reset_author = item.metadata.author.clone().unwrap_or_default();
    let reset_language = item.metadata.language.clone().unwrap_or_default();
    let reset_tags = item.metadata.tags.join(", ");
    let reset_dest = item.suggested_folder.clone().unwrap_or_default();

    rsx! {
        div { class: "space-y-3" }
        h3 { class: "text-sm font-medium text-gray-300", "Metadata" }
        div {}
        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Title" }
        input {
            r#type: "text",
            class: "input w-full mt-1",
            value: "{title.read()}",
            oninput: move |ev: FormEvent| { title.set(ev.value()); },
        }
        div {}
        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Author" }
        input {
            r#type: "text",
            class: "input w-full mt-1",
            value: "{author.read()}",
            oninput: move |ev: FormEvent| { author.set(ev.value()); },
        }
        div {}
        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Language" }
        input {
            r#type: "text",
            class: "input w-full mt-1",
            value: "{language.read()}",
            oninput: move |ev: FormEvent| { language.set(ev.value()); },
        }
        div {}
        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Tags" }
        input {
            r#type: "text",
            class: "input w-full mt-1",
            value: "{tags.read()}",
            oninput: move |ev: FormEvent| { tags.set(ev.value()); },
            placeholder: "comma, separated, tags",
        }
        div {}
        label { class: "text-xs text-gray-500 uppercase tracking-wide", "Destination" }
        input {
            r#type: "text",
            class: "input w-full mt-1",
            value: "{destination.read()}",
            oninput: move |ev: FormEvent| { destination.set(ev.value()); },
            placeholder: placeholder_text,
        }

        {if let Some(folder) = item.suggested_folder.clone() {
            rsx! {
                div { class: "flex items-center gap-1 mb-1" }
                {render_icon_view(Icon::MapPin)}
                span { class: "text-xs text-blue-400", "{folder}" }
                span { class: "text-xs text-gray-500", "(suggested)" }
            }
        } else { rsx! {} }}

        div { class: "flex gap-2 mt-4 pt-4 border-t border-gray-800" }
        Button {
            variant: ButtonVariant::Primary,
            on_click: move |_: MouseEvent| {
                let item_for_ipc = item_for_click.clone();
                let id = item_for_ipc.id.clone();
                let new_title = {
                    let t = title.read();
                    if t.is_empty() { None } else { Some(t.clone()) }
                };
                let new_author = {
                    let a = author.read();
                    if a.is_empty() { None } else { Some(a.clone()) }
                };
                let new_language = {
                    let l = language.read();
                    if l.is_empty() { None } else { Some(l.clone()) }
                };
                let new_tags: Vec<String> = tags
                    .read()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                let dest = destination.read().clone();
                let toasts_save = toasts;
                spawn_local(async move {
                    let result = crate::ipc::tauri_invoke(
                        "inbox_edit_metadata",
                        serde_wasm_bindgen::to_value(&serde_json::json!({
                            "id": id,
                            "title": new_title,
                            "author": new_author,
                            "language": new_language,
                            "tags": new_tags,
                            "custom": item_for_ipc.metadata.custom,
                        }))
                        .unwrap(),
                    )
                    .await;
                    if serde_wasm_bindgen::from_value::<()>(result).is_err() {
                        toasts_save.error("Metadata", "Could not save the updated metadata");
                    }
                });
                spawn_local(async move {
                    let result = crate::ipc::tauri_invoke(
                        "inbox_move",
                        serde_wasm_bindgen::to_value(&serde_json::json!({
                            "id": item_for_ipc.id,
                            "destination": dest,
                        }))
                        .unwrap(),
                    )
                    .await;
                    if serde_wasm_bindgen::from_value::<()>(result).is_err() {
                        toasts_save.error("Destination", "Could not set the destination folder");
                    }
                });
            },
            {"Apply Metadata"}
        }
        Button {
            variant: ButtonVariant::Ghost,
            on_click: move |_: MouseEvent| {
                title.set(reset_title.clone());
                author.set(reset_author.clone());
                language.set(reset_language.clone());
                tags.set(reset_tags.clone());
                destination.set(reset_dest.clone());
            },
            {"Reset"}
        }
    }
}
