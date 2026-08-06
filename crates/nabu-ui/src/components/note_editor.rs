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

use crate::components::contexts::{use_save_status, use_workspace, SaveStatusType};
use crate::components::editor::slash_menu::SlashMenu;
use crate::components::note_view::NoteView;
use crate::components::ui::feedback::{set_timeout, use_toast, SkeletonList};
use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

/// Vault-relative path of a dropped internal note (set by the file tree).
const NABU_NOTE_MIME: &str = "application/x-nabu-note";

/// Debounce delay (ms) between the last keystroke and an autosave.
const AUTOSAVE_DELAY_MS: u32 = 800;

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
    let dirty = use_signal(|| 0u32);
    let has_unsaved = use_signal(|| false);
    let note_loaded = use_signal(|| false);
    let show_menu = use_signal(|| false);

    // ── Load note content on mount / path change ──
    let content_for_load = content;
    let loaded_for_load = note_loaded;
    let has_unsaved_guard = has_unsaved;
    let ws_for_load = ws;
    use_effect(move || {
        let path = ws_for_load.active_path.peek().clone().unwrap_or_default();
        if path.is_empty() {
            return;
        }
        if !*has_unsaved_guard.peek() {
            *loaded_for_load.write_unchecked() = false;
            let content_c = content_for_load;
            let loaded_c = loaded_for_load;
            spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path }))
                    .unwrap();
                let result = crate::ipc::tauri_invoke("note_read", args).await;
                if let Ok(saved) = serde_wasm_bindgen::from_value::<String>(result) {
                    if !saved.is_empty() {
                        *content_c.write_unchecked() = saved;
                    }
                }
                *loaded_c.write_unchecked() = true;
            });
        }
    });

    // ── Debounced autosave ──
    let content_for_save = content;
    let save_status_save = save_status;
    let ws_for_save = ws;
    let dirty_for_save = dirty;
    let has_unsaved_for_save = has_unsaved;
    let toasts_save = toasts;

    use_effect(move || {
        let _ = dirty_for_save.read();
        let path = ws_for_save.active_path.peek().clone().unwrap_or_default();
        let content_arc = content_for_save;

        set_timeout(
            move || {
                let current = content_arc.peek().clone();
                *save_status_save.status.write_unchecked() = SaveStatusType::Saving;
                *save_status_save.last_saved.write_unchecked() = Some(format!("Saving {}", path));

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
                            *save_status_save.status.write_unchecked() = SaveStatusType::Saved;
                            *save_status_save.last_saved.write_unchecked() = Some(path_save);
                            *has_unsaved_for_save.write_unchecked() = false;
                        }
                        Err(_) => {
                            *save_status_save.status.write_unchecked() = SaveStatusType::Error;
                            toasts_save.error("Save failed", "Editing continues locally; will retry on next change.");
                        }
                    }
                });
            },
            AUTOSAVE_DELAY_MS,
        );
    });

    // ── Periodic retry of a failed save ──
    use_effect(move || {
        let _ = save_status.status.read();
        if *save_status.status.read() == SaveStatusType::Error {
            *dirty.write_unchecked() = dirty.peek().wrapping_add(1);
        }
    });

    // ── Input ref ──
    let textarea_ref: Rc<std::cell::RefCell<Option<web_sys::HtmlTextAreaElement>>> =
        use_hook(|| Rc::new(std::cell::RefCell::new(None)));

    // ── Slash menu callback ──
    let on_slash: EventHandler<String> = Callback::new(move |item: String| {
        *show_menu.write_unchecked() = false;
        let _ = item;
    });

    let active_label = ws.active_path.peek().as_deref().unwrap_or("new_note.md").to_string();

    // Clone refs for each move closure (Rc is not Copy in Rust 2024)
    let textarea_mount = textarea_ref.clone();
    let textarea_key = textarea_ref.clone();
    let textarea_drop = textarea_ref.clone();

    rsx! {
        div {
            class: "note-editor relative h-full flex flex-col",

            div {
                class: "flex items-center justify-between px-1 pb-1 text-xs text-gray-500",
                span { class: "truncate", "{active_label}" }
            }

            {
                if !*note_loaded.read() {
                    rsx! {
                        div { class: "flex-1 flex items-center justify-center" }
                        SkeletonList { rows: 6 }
                    }
                } else {
                    rsx! {
                        div {
                            class: "relative flex-1 flex flex-col",
                            textarea {
                                class: "editor-textarea flex-1 resize-none",
                                onmounted: move |ev: MountedEvent| {
                                    let web = ev.data().as_web_event();
                                    if let Ok(ta) = web.dyn_into::<web_sys::HtmlTextAreaElement>() {
                                        *textarea_mount.borrow_mut() = Some(ta);
                                    }
                                },
                                value: "{content.read()}",
                                oninput: move |ev: FormEvent| {
                                    let val = ev.value();
                                    *content.write_unchecked() = val;
                                    *dirty.write_unchecked() = dirty.peek().wrapping_add(1);
                                    *has_unsaved.write_unchecked() = true;
                                },
                                onkeydown: move |ev: KeyboardEvent| {
                                    let web = ev.data().as_web_event();
                                    let key = web.key();
                                    let meta = web.meta_key() || web.ctrl_key();
                                    if key == "/" {
                                        *show_menu.write_unchecked() = true;
                                    }
                                    if meta && key.eq_ignore_ascii_case("b") {
                                        web.prevent_default();
                                        wrap_selection(content, &textarea_key, "**", "**");
                                    } else if meta && key.eq_ignore_ascii_case("i") {
                                        web.prevent_default();
                                        wrap_selection(content, &textarea_key, "*", "*");
                                    }
                                },
                                ondragover: move |ev: DragEvent| {
                                    ev.prevent_default();
                                },
                                ondrop: move |ev: DragEvent| {
                                    ev.prevent_default();
                                    handle_editor_drop(ev, content, &textarea_drop, dirty);
                                },
                            }

                            // Live preview
                            NoteView { content: content }
                        }
                    }
                }
            }

            // Slash menu
            {
                if *show_menu.read() {
                    rsx! {
                        SlashMenu { on_select: on_slash }
                    }
                } else {
                    rsx! {}
                }
            }
        }
    }
}

/// Wraps the current textarea selection in `**...**` (bold) or `*...*` (italic).
fn wrap_selection(
    content: Signal<String>,
    textarea_ref: &Rc<std::cell::RefCell<Option<web_sys::HtmlTextAreaElement>>>,
    before: &str,
    after: &str,
) {
    let ta_ref = textarea_ref.borrow();
    let Some(ta) = ta_ref.as_ref() else {
        return;
    };
    let start = ta.selection_start().ok().flatten().unwrap_or(0) as usize;
    let end = ta.selection_end().ok().flatten().unwrap_or(start as u32) as usize;
    let mut value = content.peek().clone();
    let selected: String = value[start..end].to_string();
    value.replace_range(start..end, &format!("{before}{selected}{after}"));
    *content.write_unchecked() = value;
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
    let ta_ref = textarea_ref.borrow();
    let Some(ta) = ta_ref.as_ref() else {
        content.write_unchecked().push_str(&snippet);
        *dirty.write_unchecked() = dirty.peek().wrapping_add(1);
        return;
    };
    let start = ta.selection_start().ok().flatten().unwrap_or(0) as usize;
    let end = ta.selection_end().ok().flatten().unwrap_or(start as u32) as usize;
    let mut value = content.peek().clone();
    value.insert_str(end, &snippet);
    *content.write_unchecked() = value;
    *dirty.write_unchecked() = dirty.peek().wrapping_add(1);
    let new_caret = (end + snippet.len()) as u32;
    let _ = ta.set_selection_range(new_caret, new_caret);
}
