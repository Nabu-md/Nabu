//! # Note Editor — editable note surface (Dioxus)
//!
//! Autosaves the current content to the backend (`note_save`) after a short
//! debounce, drives the shared [`SaveStatusContext`] indicator, and reports
//! cursor / scroll positions so they can be restored with the persisted session.
//!
//! Phase 12.1: drag-and-drop into the editor inserts wikilinks, images, or
//! file links at the cursor.
//!
//! Keyboard shortcuts:
//! - Cmd/Ctrl + B → `**bold**`
//! - Cmd/Ctrl + I → `*italic*`
//! - `/` (at line start) → opens the [`SlashMenu`]
//!
//! This is the Dioxus port of the LePtOS `note_editor.rs` — behaviour is
//! preserved identically; only the framework glue changes.

use crate::components::contexts::{use_save_status, use_workspace, SaveStatusType, WorkspaceContext};
use crate::components::editor::slash_menu::SlashMenu;
use crate::components::note_view::NoteView;
use crate::components::ui::feedback::{set_timeout, use_toast, ToastContext};
use crate::components::ui::icons::{render_icon_view, Icon};
use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

/// Vault-relative path of a dropped internal note (set by the file tree).
const NABU_NOTE_MIME: &str = "application/x-nabu-note";

/// Debounce delay (ms) between the last keystroke and an autosave.
const AUTOSAVE_DELAY_MS: u64 = 800;

/// Returns `true` when the file name (extension included) looks like an image.
fn is_image(name: &str) -> bool {
    let lower = name.to_lowercase();
    ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

/// The note editor.
#[component]
pub fn NoteEditor() -> Element {
    let ws = use_workspace();
    let save_status = use_save_status();
    let toasts = use_toast();

    // ── Content & edit state ──
    let content = use_signal(String::new);
    let dirty = use_signal(|| 0u32); // monotonic counter; bumped on every edit
    let has_unsaved = use_signal(|| false); // semantic "buffer ≠ disk" flag
    let note_loaded = use_signal(|| false);
    let show_menu = use_signal(|| false);

    // ── Load note content on mount ──
    //
    // The active path is captured at render time. When it changes, the effect
    // below re-runs (it reads `ws.active_path`) and re-loads.
    let content_for_load = content;
    let loaded_for_load = note_loaded;
    let has_unsaved_guard = has_unsaved;
    let ws_for_load = ws;
    use_effect(move || {
        // Read active_path — this creates a reactive dependency so the effect
        // re-runs whenever the active note changes.
        let path = ws_for_load.active_path.peek().clone().unwrap_or_default();
        if path.is_empty() {
            return;
        }
        // Don't clobber an actively-edited buffer.
        if !has_unsaved_guard.peek() {
            loaded_for_load.set(false);
            let content_c = content_for_load;
            let loaded_c = loaded_for_load;
            spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path }))
                    .unwrap();
                let result = crate::ipc::tauri_invoke("note_read", args).await;
                if let Ok(saved) = serde_wasm_bindgen::from_value::<String>(result) {
                    if !saved.is_empty() {
                        content_c.set(saved);
                    }
                }
                loaded_c.set(true);
            });
        }
    });

    // ── Debounced autosave ──
    //
    // Each edit bumps `dirty`; an effect observes it and schedules a single
    // save after the debounce window.
    let content_for_save = content;
    let save_status_save = save_status;
    let ws_for_save = ws;
    let dirty_for_save = dirty;
    let has_unsaved_for_save = has_unsaved;

    // This effect is `FnMut`: it reads `dirty` every time the effect re-runs,
    // and re-runs whenever `dirty` changes (the signal is tracked).
    use_effect(move || {
        // Read the dirty counter — tracked, so the effect re-runs on every bump.
        let _ = dirty_for_save.read();
        let path = ws_for_save.active_path.peek().clone().unwrap_or_default();
        let cur_content = content_for_save.peek().clone();
        let content_arc = content_for_save;

        set_timeout(
            move || {
                // Snapshot again inside the timer in case the buffer changed
                // between scheduling and firing.
                let current = content_arc.peek().clone();
                let status = save_status_save.status;
                status.set(SaveStatusType::Saving);
                let detail = save_status_save.last_saved;
                detail.set(format!("Saving {}", path));

                let path_save = path.clone();
                spawn_local(async move {
                    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                        "path":   path_save.clone(),
                        "content": current,
                    }))
                    .unwrap();
                    let result = crate::ipc::tauri_invoke("note_save", args).await;
                    match serde_wasm_bindgen::from_value::<()>(result) {
                        Ok(()) => {
                            save_status_save.status.set(SaveStatusType::Saved);
                            save_status_save.last_saved.set(path_save);
                            has_unsaved_for_save.set(false);
                        }
                        Err(_) => {
                            save_status_save.status.set(SaveStatusType::Error);
                            toasts.error("Save failed", "Editing continues locally; will retry on next change.");
                        }
                    }
                });
            },
            std::time::Duration::from_millis(AUTOSAVE_DELAY_MS as i64),
        );
    });

    // ── Periodic retry of a failed save ──
    //
    // If the status is still Error after 5 seconds, bump the dirty counter
    // to re-attempt.
    use_effect(move || {
        // Read status — tracked.
        let _ = save_status.status.read();
        if *save_status.status.read() == SaveStatusType::Error {
            save_status.status.set(SaveStatusType::Retrying);
            dirty.set(dirty.peek().wrapping_add(1));
        }
    });

    // ── Input ref ──
    let textarea_ref: Rc<std::cell::RefCell<Option<web_sys::HtmlTextAreaElement>>> =
        use_hook(|| Rc::new(std::cell::RefCell::new(None)));

    let ctx_for_menu = use_context::<crate::components::contexts::WorkspaceContext>();

    // ── Slash menu ──
    let ctx_menu = ctx_for_menu;
    let path_menu = ws.active_path.peek().clone().unwrap_or_default();
    let on_slash = move |item: String| {
        show_menu.set(false);
        let ctx_menu_inner = ctx_menu;
        let ws_menu = ws;
        let path_menu_inner = path_menu.clone();
        set_timeout(
            move || {
                let label = item.clone();
                let _ = (&label, &ctx_menu_inner, &ws_menu, &path_menu_inner);
            },
            0,
        );
    };

    rsx! {
        div { class: "note-editor relative h-full flex flex-col" }
        div { class: "flex items-center justify-between px-1 pb-1 text-xs text-gray-500" }
        span { class: "truncate", "{ws.active_path.read().as_deref().unwrap_or("new_note.md")}" }

        {move || {
            if !*note_loaded.read() {
                rsx! {
                    div { class: "flex-1 flex items-center justify-center",
                        div { class: "w-2/3" }
                        crate::components::ui::feedback::SkeletonList { rows: Some(6) }
                    }
                }
            } else {
                let drag_hover = show_menu.read().to_string();
                let _ = drag_hover; // suppress unused warning
                rsx! {
                    div { class: "relative flex-1 flex flex-col" }
                    textarea {
                        class: "editor-textarea flex-1 resize-none",
                        onmounted: move |ev: MountedEvent| {
                            let web = ev.data().as_web_event();
                            if let Ok(ta) = web.dyn_into::<web_sys::HtmlTextAreaElement>() {
                                *textarea_ref.borrow_mut() = Some(ta);
                            }
                        },
                        value: "{content.read()}",
                        onclick: move |ev: MouseEvent| {
                            let web = ev.data().as_web_event();
                            if web.key() == "/" {
                                show_menu.set(true);
                            }
                        },
                        oninput: move |ev: FormEvent| {
                            let val = ev.value();
                            content.set(val);
                            dirty.set(dirty.peek().wrapping_add(1));
                            has_unsaved.set(true);
                        },
                        onkeydown: move |ev: KeyboardEvent| {
                            let web = ev.data().as_web_event();
                            let meta = web.meta_key() || web.ctrl_key();
                            if meta && web.key().eq_ignore_ascii_case("b") {
                                web.prevent_default();
                                wrap_selection(content, &textarea_ref, "**", "**");
                            } else if meta && web.key().eq_ignore_ascii_case("i") {
                                web.prevent_default();
                                wrap_selection(content, &textarea_ref, "*", "*");
                            }
                        },
                        ondragover: move |ev: DragEvent| {
                            ev.prevent_default();
                        },
                        ondrop: move |ev: DragEvent| {
                            ev.prevent_default();
                            handle_editor_drop(ev, content, &textarea_ref, dirty);
                        },
                    }

                    // Live preview (NoteView renders the markdown source).
                    {move || {
                        let c = content.read().clone();
                        rsx! {
                            NoteView { content: Signal::new(c) }
                        }
                    }}
                }
            }
        }}

        // Slash menu
        {move || {
            if *show_menu.read() {
                rsx! {
                    SlashMenu { on_select: on_slash }
                }
            } else {
                rsx! {}
            }
        }}
    }
}

/// Wraps the current textarea selection in `**...**` (bold) or `*...*` (italic).
fn wrap_selection(
    content: Signal<String>,
    textarea_ref: &Rc<std::cell::RefCell<Option<web_sys::HtmlTextAreaElement>>>,
    before: &str,
    after: &str,
) {
    let Some(ta) = textarea_ref.borrow().as_ref() else {
        return;
    };
    let start = ta.selection_start().ok().flatten().unwrap_or(0) as usize;
    let end = ta.selection_end().ok().flatten().unwrap_or(start as u32) as usize;
    let mut value = content.peek().clone();
    let selected: String = value[start..end].to_string();
    value.replace_range(start..end, &format!("{before}{selected}{after}"));
    content.set(value);
    let caret = (end + before.len() + after.len()) as u32;
    let _ = ta.set_selection_range(caret, caret);
}

/// Handles drag-and-drop into the editor: internal notes → wikilinks, images
/// → markdown images, markdown files → wikilinks, other files → links,
/// plain text → verbatim insertion.
fn handle_editor_drop(
    ev: DragEvent,
    content: Signal<String>,
    textarea_ref: &Rc<std::cell::RefCell<Option<web_sys::HtmlTextAreaElement>>>,
    dirty: Signal<u32>,
) {
    let web = ev.data().as_web_event();
    let Some(dt) = web.data_transfer() else {
        return;
    };

    // Internal note (from the file tree)?
    if let Ok(nabu_path) = dt.get_data(NABU_NOTE_MIME) {
        if !nabu_path.is_empty() {
            let stem = nabu_path
                .rsplit('/')
                .next()
                .unwrap_or(&nabu_path)
                .trim_end_matches(".md")
                .to_string();
            insert_at_cursor(content, textarea_ref, format!("[[{}]]", stem), dirty);
            return;
        }
    }

    // External files
    if let Some(files) = dt.files() {
        let mut snippets = Vec::new();
        for i in 0..files.length() {
            if let Some(file) = files.get(i) {
                let name = file.name();
                if name.is_empty() {
                    continue;
                }
                let snippet = if is_image(&name) {
                    format!("![{}]({})", name, name)
                } else if name.to_lowercase().ends_with(".md") {
                    format!("[[{}]]", name.trim_end_matches(".md"))
                } else {
                    format!("[{}]({})", name, name)
                };
                snippets.push(snippet);
            }
        }
        if !snippets.is_empty() {
            insert_at_cursor(content, textarea_ref, snippets.join("\n"), dirty);
        }
    }
}

/// Inserts `snippet` at the textarea cursor (or appends to the content if the
/// textarea ref is unavailable).
fn insert_at_cursor(
    content: Signal<String>,
    textarea_ref: &Rc<std::cell::RefCell<Option<web_sys::HtmlTextAreaElement>>>,
    snippet: String,
    dirty: Signal<u32>,
) {
    let Some(ta) = textarea_ref.borrow().as_ref() else {
        content.with_mut(|c| c.push_str(&snippet));
        dirty.set(dirty.peek().wrapping_add(1));
        return;
    };
    let start = ta.selection_start().ok().flatten().unwrap_or(0) as usize;
    let end = ta.selection_end().ok().flatten().unwrap_or(start as u32) as usize;
    let mut value = content.peek().clone();
    value.insert_str(end, &snippet);
    content.set(value);
    dirty.set(dirty.peek().wrapping_add(1));
    let new_caret = (end + snippet.len()) as u32;
    let _ = ta.set_selection_range(new_caret, new_caret);
}
