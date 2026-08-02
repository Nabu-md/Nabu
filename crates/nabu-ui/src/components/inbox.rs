//! Knowledge Inbox UI component.
//!
//! Split-pane interface for reviewing, organising, approving, and processing
//! captured knowledge before final storage. State is driven by EventBus events
//! via Tauri IPC — no polling.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

// ── Types ──────────────────────────────────────────────────────────────────

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
    pub custom: std::collections::HashMap<String, serde_json::Value>,
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

// ── State ──────────────────────────────────────────────────────────────────

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

// ── Inbox Actions ──────────────────────────────────────────────────────────

fn approve_item(id: String) {
    spawn_local(async move {
        let _ =
            crate::ipc::tauri_invoke("inbox_approve", serde_wasm_bindgen::to_value(&id).unwrap())
                .await;
    });
}

fn reject_item(id: String) {
    spawn_local(async move {
        let _ = crate::ipc::tauri_invoke(
            "inbox_reject",
            serde_wasm_bindgen::to_value(&serde_json::json!({"id": id, "reason": "User rejected"}))
                .unwrap(),
        )
        .await;
    });
}

fn retry_item(id: String) {
    spawn_local(async move {
        let _ = crate::ipc::tauri_invoke("inbox_retry", serde_wasm_bindgen::to_value(&id).unwrap())
            .await;
    });
}

fn delete_item(id: String) {
    spawn_local(async move {
        let _ =
            crate::ipc::tauri_invoke("inbox_delete", serde_wasm_bindgen::to_value(&id).unwrap())
                .await;
    });
}

// ── Inbox Component ────────────────────────────────────────────────────────

#[component]
pub fn Inbox() -> impl IntoView {
    let (state, set_state) = signal(InboxState::default());

    // Subscribe to EventBus events via Tauri IPC (no polling)
    spawn_local(async move {
        let _ = crate::ipc::tauri_invoke(
            "inbox_subscribe",
            serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap(),
        )
        .await;
    });

    let filtered_items = move || {
        let s = state.get();
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

    let selected_count = move || state.get().items.iter().filter(|i| i.selected).count();

    let toggle_select_item = move |id: String| {
        let mut s = state.get();
        if let Some(item) = s.items.iter_mut().find(|i| i.id == id) {
            item.selected = !item.selected;
        }
        set_state.set(s);
    };

    let set_preview = move |id: String| {
        let mut s = state.get();
        s.preview_id = Some(id);
        set_state.set(s);
    };

    let batch_approve = move |_| {
        let ids: Vec<String> = state
            .get()
            .items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.id.clone())
            .collect();
        if !ids.is_empty() {
            spawn_local(async move {
                let _ = crate::ipc::tauri_invoke(
                    "inbox_batch_approve",
                    serde_wasm_bindgen::to_value(&ids).unwrap(),
                )
                .await;
            });
        }
    };

    let batch_reject = move |_| {
        let ids: Vec<String> = state
            .get()
            .items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.id.clone())
            .collect();
        if !ids.is_empty() {
            spawn_local(async move {
                let _ = crate::ipc::tauri_invoke(
                    "inbox_batch_reject",
                    serde_wasm_bindgen::to_value(&ids).unwrap(),
                )
                .await;
            });
        }
    };

    let batch_delete = move |_| {
        let ids: Vec<String> = state
            .get()
            .items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.id.clone())
            .collect();
        if !ids.is_empty() {
            spawn_local(async move {
                let _ = crate::ipc::tauri_invoke(
                    "inbox_batch_delete",
                    serde_wasm_bindgen::to_value(&ids).unwrap(),
                )
                .await;
            });
        }
    };

    let batch_retry = move |_| {
        let ids: Vec<String> = state
            .get()
            .items
            .iter()
            .filter(|i| i.selected)
            .map(|i| i.id.clone())
            .collect();
        if !ids.is_empty() {
            spawn_local(async move {
                let _ = crate::ipc::tauri_invoke(
                    "inbox_batch_retry",
                    serde_wasm_bindgen::to_value(&ids).unwrap(),
                )
                .await;
            });
        }
    };

    let on_filter_change = move |ev| {
        let mut s = state.get();
        s.filter = event_target_value(&ev);
        set_state.set(s);
    };

    let on_sort_change = move |field: SortField| {
        let mut s = state.get();
        if s.sort_by == field {
            s.sort_ascending = !s.sort_ascending;
        } else {
            s.sort_by = field;
            s.sort_ascending = true;
        }
        set_state.set(s);
    };

    view! {
        <div class="inbox flex h-full bg-gray-950 text-gray-100 overflow-hidden">
            // Left panel: Queue
            <div class="flex-none w-96 border-r border-gray-800 flex flex-col">
                <div class="flex items-center gap-2 p-3 border-b border-gray-800">
                    <input type="text" placeholder="Search inbox..."
                        class="flex-1 bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none"
                        on:input=on_filter_change />
                </div>
                // Batch actions bar
                {move || {
                    let count = selected_count();
                    if count > 0 {
                        view! {
                            <div class="flex items-center gap-2 px-3 py-2 bg-blue-900/30 border-b border-blue-800/30">
                                <span class="text-xs text-blue-400">{count} selected</span>
                                <button class="px-2 py-1 text-xs bg-green-700 rounded hover:bg-green-600" on:click=batch_approve>"Approve"</button>
                                <button class="px-2 py-1 text-xs bg-red-700 rounded hover:bg-red-600" on:click=batch_reject>"Reject"</button>
                                <button class="px-2 py-1 text-xs bg-yellow-700 rounded hover:bg-yellow-600" on:click=batch_retry>"Retry"</button>
                                <button class="px-2 py-1 text-xs bg-gray-700 rounded hover:bg-gray-600" on:click=batch_delete>"Delete"</button>
                            </div>
                        }.into_any()
                    } else { view! {}.into_any() }
                }}
                // Queue list
                <div class="flex-1 overflow-y-auto">
                    {move || {
                        let items = filtered_items();
                        if items.is_empty() {
                            view! { <div class="flex items-center justify-center h-full text-gray-500 text-sm">"No items in inbox"</div> }.into_any()
                        } else {
                            view! {
                                <div class="divide-y divide-gray-800">
                                    { items.iter().map(|item| {
                                        let id = item.id.clone();
                                        let dbl_id = id.clone();
                                        let title = item.title.clone();
                                        let status = item.status;
                                        let source = item.source.clone();
                                        let object_type = item.object_type.clone();
                                        let selected = item.selected;
                                        let warnings = item.warnings.clone();
                                        let duplicate_info = item.duplicate_info.clone();
                                        let ocr_info = item.ocr_info.clone();
                                        view! {
                                            <div class=move || format!(
                                                "inbox-item px-3 py-2 cursor-pointer hover:bg-gray-800 border-l-2 {} {}",
                                                if selected { "bg-gray-800 border-l-blue-500" } else { "border-l-transparent" },
                                                if warnings.is_empty() { "" } else if duplicate_info.is_some() { "border-l-yellow-500" } else if ocr_info.as_ref().map_or(false, |o| o.warning.is_some()) { "border-l-orange-500" } else { "border-l-green-500" }
                                            )
                                                on:click=move |_| toggle_select_item(id.clone())
                                                on:dblclick=move |_| set_preview(dbl_id.clone())>
                                                <div class="flex items-center justify-between">
                                                    <span class="text-sm font-medium truncate max-w-48">{title}</span>
                                                    <span class="text-xs px-1.5 py-0.5 rounded bg-gray-700 text-gray-300">
                                                        {move || format!("{:?}", status)}
                                                    </span>
                                                </div>
                                                <div class="flex items-center gap-2 mt-1">
                                                    <span class="text-xs text-gray-500">{source}</span>
                                                    <span class="text-xs text-gray-600">"•"</span>
                                                    <span class="text-xs text-gray-500">{object_type}</span>
                                                </div>
                                            </div>
                                        }
                                    }).collect_view()}
                                </div>
                            }.into_any()
                        }
                    }}
                </div>
                // Footer
                <div class="flex items-center justify-between px-3 py-2 border-t border-gray-800 text-xs text-gray-500">
                    <span>{move || format!("{} items", state.get().items.len())}</span>
                    <div class="flex gap-2">
                        <button class="hover:text-gray-300" on:click=move |_| on_sort_change(SortField::Timestamp)>"Sort by Date"</button>
                        <button class="hover:text-gray-300" on:click=move |_| on_sort_change(SortField::Title)>"Sort by Title"</button>
                        <button class="hover:text-gray-300" on:click=move |_| on_sort_change(SortField::Status)>"Sort by Status"</button>
                    </div>
                </div>
            </div>

            // Right panel: Preview
            <div class="flex-1 flex flex-col overflow-hidden">
                {move || {
                    let preview_id = state.get().preview_id.clone();
                    match preview_id {
                        Some(id) => {
                            let state_guard = state.get();
                            let item = state_guard.items.iter().find(|i| i.id == id);
                            match item {
                                Some(item) => view! {
                                    <div class="flex h-full">
                                        <div class="flex-1 overflow-y-auto p-4"><InboxPreview item=item.clone() /></div>
                                        <div class="flex-none w-72 border-l border-gray-800 overflow-y-auto p-4"><InboxMetadataSidebar item=item.clone() /></div>
                                    </div>
                                }.into_any(),
                                None => view! { <div class="flex items-center justify-center h-full text-gray-500">"Select an item to preview"</div> }.into_any(),
                            }
                        }
                        None => view! { <div class="flex items-center justify-center h-full text-gray-500">"Select an item to preview"</div> }.into_any(),
                    }
                }}
            </div>
        </div>
    }
}

// ── Inbox Preview ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum InboxPreviewTab {
    Details,
    Duplicate,
    Timeline,
    Ocr,
    History,
}

#[component]
fn InboxPreview(item: InboxItem) -> impl IntoView {
    let (active_tab, set_active_tab) = signal(InboxPreviewTab::Details);
    let approve_id = item.id.clone();
    let reject_id = item.id.clone();
    let retry_id = item.id.clone();
    let delete_id = item.id.clone();
    let has_duplicate = item.duplicate_info.is_some();
    let has_timeline = item.timeline_info.is_some();
    let has_ocr = item.ocr_info.is_some();

    view! {
        <div class="inbox-preview">
            <div class="flex items-center gap-1 mb-4 border-b border-gray-800 pb-2">
                <button class=move || format!("px-3 py-1 text-sm rounded {}", if active_tab.get() == InboxPreviewTab::Details { "bg-blue-600 text-white" } else { "text-gray-400 hover:text-gray-200" })
                    on:click=move |_| set_active_tab.set(InboxPreviewTab::Details)>"Details"</button>
                <button class=move || format!("px-3 py-1 text-sm rounded {}", if active_tab.get() == InboxPreviewTab::Duplicate { "bg-blue-600 text-white" } else { "text-gray-400 hover:text-gray-200" })
                    on:click=move |_| set_active_tab.set(InboxPreviewTab::Duplicate)>{move || format!("Duplicate{}", if has_duplicate { " ⚠" } else { "" })}</button>
                <button class=move || format!("px-3 py-1 text-sm rounded {}", if active_tab.get() == InboxPreviewTab::Timeline { "bg-blue-600 text-white" } else { "text-gray-400 hover:text-gray-200" })
                    on:click=move |_| set_active_tab.set(InboxPreviewTab::Timeline)>{move || format!("Timeline{}", if has_timeline { " 📅" } else { "" })}</button>
                <button class=move || format!("px-3 py-1 text-sm rounded {}", if active_tab.get() == InboxPreviewTab::Ocr { "bg-blue-600 text-white" } else { "text-gray-400 hover:text-gray-200" })
                    on:click=move |_| set_active_tab.set(InboxPreviewTab::Ocr)>{move || format!("OCR{}", if has_ocr { " 🔍" } else { "" })}</button>
                <button class=move || format!("px-3 py-1 text-sm rounded {}", if active_tab.get() == InboxPreviewTab::History { "bg-blue-600 text-white" } else { "text-gray-400 hover:text-gray-200" })
                    on:click=move |_| set_active_tab.set(InboxPreviewTab::History)>"History"</button>
            </div>
            {move || match active_tab.get() {
                InboxPreviewTab::Details => view! { <InboxDetails item=item.clone() /> }.into_any(),
                InboxPreviewTab::Duplicate => view! { <InboxDuplicateReview item=item.clone() /> }.into_any(),
                InboxPreviewTab::Timeline => view! { <InboxTimelineReview item=item.clone() /> }.into_any(),
                InboxPreviewTab::Ocr => view! { <InboxOcrReview item=item.clone() /> }.into_any(),
                InboxPreviewTab::History => view! { <InboxHistory item=item.clone() /> }.into_any(),
            }}
            <div class="flex items-center gap-2 mt-4 pt-4 border-t border-gray-800">
                <button class="px-3 py-1.5 text-sm bg-green-700 rounded hover:bg-green-600" on:click=move |_| approve_item(approve_id.clone())>"✓ Approve"</button>
                <button class="px-3 py-1.5 text-sm bg-red-700 rounded hover:bg-red-600" on:click=move |_| reject_item(reject_id.clone())>"✗ Reject"</button>
                <button class="px-3 py-1.5 text-sm bg-yellow-700 rounded hover:bg-yellow-600" on:click=move |_| retry_item(retry_id.clone())>"↻ Retry"</button>
                <button class="px-3 py-1.5 text-sm bg-gray-700 rounded hover:bg-gray-600" on:click=move |_| delete_item(delete_id.clone())>"🗑 Delete"</button>
            </div>
        </div>
    }
}

// ── Preview Tabs ───────────────────────────────────────────────────────────

#[component]
fn InboxDetails(item: InboxItem) -> impl IntoView {
    view! {
        <div class="space-y-3">
            <div><label class="text-xs text-gray-500 uppercase tracking-wide">"Title"</label><p class="text-lg font-medium">{item.title.clone()}</p></div>
            <div class="grid grid-cols-2 gap-3">
                <div><label class="text-xs text-gray-500 uppercase tracking-wide">"Type"</label><p class="text-sm">{item.object_type.clone()}</p></div>
                <div><label class="text-xs text-gray-500 uppercase tracking-wide">"Source"</label><p class="text-sm">{item.source.clone()}</p></div>
                <div><label class="text-xs text-gray-500 uppercase tracking-wide">"MIME Type"</label><p class="text-sm">{item.mime_type.clone().unwrap_or_default()}</p></div>
                <div><label class="text-xs text-gray-500 uppercase tracking-wide">"Source File"</label><p class="text-sm text-gray-400 truncate">{item.source_file.clone().unwrap_or_default()}</p></div>
            </div>
            {move || {
                if !item.warnings.is_empty() {
                    view! {
                        <div class="mt-3"><label class="text-xs text-gray-500 uppercase tracking-wide">"Warnings"</label>
                            <ul class="mt-1 space-y-1">{ item.warnings.iter().map(|w| view! { <li class="text-sm text-yellow-400">{w.clone()}</li> }).collect_view()}</ul>
                        </div>
                    }.into_any()
                } else { view! {}.into_any() }
            }}
        </div>
    }
}

#[component]
fn InboxDuplicateReview(item: InboxItem) -> impl IntoView {
    let duplicate = move || item.duplicate_info.clone();
    view! {
        <div class="space-y-3">
            {move || match duplicate() {
                Some(dup) => view! {
                    <div class="space-y-3">
                        <div class="p-3 bg-yellow-900/20 border border-yellow-700/30 rounded-lg">
                            <div class="flex items-center gap-2"><span class="text-yellow-400">"⚠"</span><span class="text-sm font-medium text-yellow-300">Potential Duplicate Detected</span></div>
                            <p class="text-xs text-yellow-400/70 mt-1">{dup.reason.clone().unwrap_or_default()}</p>
                        </div>
                        <div class="grid grid-cols-2 gap-3 text-sm">
                            <div><label class="text-xs text-gray-500">"Confidence"</label><p class="text-gray-300">{dup.confidence}</p></div>
                            <div><label class="text-xs text-gray-500">"Content Hash"</label><p class="text-gray-300 font-mono text-xs truncate">{dup.content_hash.clone().unwrap_or_default()}</p></div>
                            <div><label class="text-xs text-gray-500">"Duplicate Source"</label><p class="text-gray-300 text-xs truncate">{dup.duplicate_source.clone().unwrap_or_default()}</p></div>
                            <div><label class="text-xs text-gray-500">"Candidates"</label><p class="text-gray-300">{dup.candidate_ids.len()} found</p></div>
                        </div>
                        <div class="flex gap-2">
                            <button class="px-3 py-1.5 text-sm bg-green-700 rounded hover:bg-green-600">"Keep Both"</button>
                            <button class="px-3 py-1.5 text-sm bg-blue-700 rounded hover:bg-blue-600">"Replace"</button>
                            <button class="px-3 py-1.5 text-sm bg-gray-700 rounded hover:bg-gray-600">"Ignore"</button>
                        </div>
                    </div>
                }.into_any(),
                None => view! { <div class="text-gray-500 text-sm">No duplicate detected for this item.</div> }.into_any(),
            }}
        </div>
    }
}

#[component]
fn InboxTimelineReview(item: InboxItem) -> impl IntoView {
    let timeline = move || item.timeline_info.clone();
    view! {
        <div class="space-y-3">
            {move || match timeline() {
                Some(tl) => view! {
                    <div class="space-y-3">
                        <div class="grid grid-cols-2 gap-3 text-sm">
                            <div><label class="text-xs text-gray-500">"Document Date"</label><p class="text-gray-300">{tl.document_date.clone().unwrap_or_default()}</p></div>
                            <div><label class="text-xs text-gray-500">"Created Date"</label><p class="text-gray-300">{tl.created_date.clone().unwrap_or_default()}</p></div>
                            <div><label class="text-xs text-gray-500">"Modified Date"</label><p class="text-gray-300">{tl.modified_date.clone().unwrap_or_default()}</p></div>
                            <div><label class="text-xs text-gray-500">"Detected Event Date"</label><p class="text-gray-300">{tl.detected_event_date.clone().unwrap_or_default()}</p></div>
                        </div>
                        <div><label class="text-xs text-gray-500">"Extraction Confidence"</label><p class="text-gray-300">{tl.extraction_confidence.clone().unwrap_or_default()}</p></div>
                    </div>
                }.into_any(),
                None => view! { <div class="text-gray-500 text-sm">No timeline information extracted for this item.</div> }.into_any(),
            }}
        </div>
    }
}

#[component]
fn InboxOcrReview(item: InboxItem) -> impl IntoView {
    let ocr = move || item.ocr_info.clone();
    view! {
        <div class="space-y-3">
            {move || match ocr() {
                Some(ocr) => view! {
                    <div class="space-y-3">
                        <div class="grid grid-cols-2 gap-3 text-sm">
                            <div><label class="text-xs text-gray-500">"Confidence"</label><p class="text-gray-300">{ocr.confidence.map_or("N/A".to_string(), |c| format!("{:.0}%", c * 100.0))}</p></div>
                            <div><label class="text-xs text-gray-500">"Language"</label><p class="text-gray-300">{ocr.recognition_language.clone().unwrap_or_default()}</p></div>
                            <div><label class="text-xs text-gray-500">"Pages Processed"</label><p class="text-gray-300">{ocr.page_count.map_or("N/A".to_string(), |p| p.to_string())}</p></div>
                            <div><label class="text-xs text-gray-500">"Duration"</label><p class="text-gray-300">{ocr.processing_duration_ms.map_or("N/A".to_string(), |d| format!("{}ms", d))}</p></div>
                            <div><label class="text-xs text-gray-500">"Scanned Document"</label><p class="text-gray-300">{ocr.is_scanned.map_or("N/A".to_string(), |s| s.to_string())}</p></div>
                        </div>
                        {move || {
                            if let Some(text) = &ocr.extracted_text {
                                view! { <div><label class="text-xs text-gray-500">"Extracted Text"</label><pre class="mt-1 text-xs text-gray-300 bg-gray-900 p-3 rounded-lg overflow-auto max-h-48 whitespace-pre-wrap">{text.clone()}</pre></div> }.into_any()
                            } else { view! {}.into_any() }
                        }}
                        {move || {
                            if let Some(warn) = &ocr.warning {
                                view! { <div class="text-sm text-orange-400">{"⚠ "}{warn.clone()}</div> }.into_any()
                            } else { view! {}.into_any() }
                        }}
                    </div>
                }.into_any(),
                None => view! { <div class="text-gray-500 text-sm">No OCR information available for this item.</div> }.into_any(),
            }}
        </div>
    }
}

#[component]
fn InboxHistory(item: InboxItem) -> impl IntoView {
    let history = item.processing_history.clone();
    view! {
        <div class="space-y-2">
            {if history.is_empty() {
                view! { <div class="text-gray-500 text-sm">No processing history available.</div> }.into_any()
            } else {
                view! {
                    <div class="space-y-2">
                        { history.iter().map(|entry| {
                            let success = entry.success;
                            let warnings = entry.warnings.clone();
                            let error = entry.error.clone();
                            let processor_name = entry.processor_name.clone();
                            let duration_ms = entry.duration_ms;
                            let timestamp = entry.timestamp.clone();
                            view! {
                                <div class="flex items-start gap-3 p-2 rounded-lg bg-gray-900/50 text-sm">
                                    <span class=move || format!("mt-0.5 {}", if success { "text-green-400" } else { "text-red-400" })>
                                        {if success { "✓" } else { "✗" }}
                                    </span>
                                    <div class="flex-1 min-w-0">
                                        <div class="flex items-center justify-between">
                                            <span class="font-medium text-gray-200">{processor_name}</span>
                                            <span class="text-xs text-gray-500">{duration_ms}ms</span>
                                        </div>
                                        <div class="text-xs text-gray-500 mt-0.5">{timestamp}</div>
                                        {move || {
                                            if !warnings.is_empty() {
                                                view! { <ul class="mt-1 space-y-0.5">{ warnings.iter().map(|w| view! { <li class="text-xs text-yellow-400">{w.clone()}</li> }).collect_view()}</ul> }.into_any()
                                            } else { view! {}.into_any() }
                                        }}
                                        {move || {
                                            if let Some(err) = &error {
                                                view! { <div class="text-xs text-red-400 mt-1">{err.clone()}</div> }.into_any()
                                            } else { view! {}.into_any() }
                                        }}
                                    </div>
                                </div>
                            }
                        }).collect_view()}
                    </div>
                }.into_any()
            }}
        </div>
    }
}

// ── Metadata Sidebar ───────────────────────────────────────────────────────

#[component]
fn InboxMetadataSidebar(item: InboxItem) -> impl IntoView {
    let (title, set_title) = signal(item.metadata.title.clone().unwrap_or_default());
    let (author, set_author) = signal(item.metadata.author.clone().unwrap_or_default());
    let (language, set_language) = signal(item.metadata.language.clone().unwrap_or_default());
    let (tags, set_tags) = signal(item.metadata.tags.join(", "));

    let save_metadata = move |_| {
        let item_for_ipc = item.clone();
        let id = item_for_ipc.id.clone();
        let new_title = if title.get().is_empty() {
            None
        } else {
            Some(title.get())
        };
        let new_author = if author.get().is_empty() {
            None
        } else {
            Some(author.get())
        };
        let new_language = if language.get().is_empty() {
            None
        } else {
            Some(language.get())
        };
        let new_tags: Vec<String> = tags
            .get()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        spawn_local(async move {
            let _ = crate::ipc::tauri_invoke(
                "inbox_edit_metadata",
                serde_wasm_bindgen::to_value(&serde_json::json!({
                    "id": id, "title": new_title, "author": new_author,
                    "language": new_language, "tags": new_tags,
                    "custom": item_for_ipc.metadata.custom,
                }))
                .unwrap(),
            )
            .await;
        });
    };

    view! {
        <div class="space-y-3">
            <h3 class="text-sm font-medium text-gray-300">"Metadata"</h3>
            <div>
                <label class="text-xs text-gray-500 uppercase tracking-wide">"Title"</label>
                <input type="text" value=title on:input=move |ev| set_title.set(event_target_value(&ev))
                    class="w-full bg-gray-800 text-gray-100 rounded px-2 py-1 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none" />
            </div>
            <div>
                <label class="text-xs text-gray-500 uppercase tracking-wide">"Author"</label>
                <input type="text" value=author on:input=move |ev| set_author.set(event_target_value(&ev))
                    class="w-full bg-gray-800 text-gray-100 rounded px-2 py-1 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none" />
            </div>
            <div>
                <label class="text-xs text-gray-500 uppercase tracking-wide">"Language"</label>
                <input type="text" value=language on:input=move |ev| set_language.set(event_target_value(&ev))
                    class="w-full bg-gray-800 text-gray-100 rounded px-2 py-1 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none" />
            </div>
            <div>
                <label class="text-xs text-gray-500 uppercase tracking-wide">"Tags (comma-separated)"</label>
                <input type="text" value=tags on:input=move |ev| set_tags.set(event_target_value(&ev))
                    class="w-full bg-gray-800 text-gray-100 rounded px-2 py-1 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none" />
            </div>
            <button class="w-full px-3 py-1.5 text-sm bg-blue-700 rounded hover:bg-blue-600" on:click=save_metadata>"Save Metadata"</button>
        </div>
    }
}
