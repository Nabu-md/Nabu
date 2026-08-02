use crate::components::editor::slash_menu::SlashMenu;
use crate::components::note_view::NoteView;
use crate::components::recovery::save_status::{SaveStatus, use_save_status};
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

/// Vault-relative path of a dropped internal note (set by the file tree).
const NABU_NOTE_MIME: &str = "application/x-nabu-note";

/// File extensions treated as embeddable images.
fn is_image(name: &str) -> bool {
    let lower = name.to_lowercase();
    ["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp", "avif"].iter().any(|ext| lower.ends_with(ext))
}

/// Debounce delay (ms) between the last keystroke and an autosave.
const AUTOSAVE_DELAY_MS: u64 = 800;

/// The note editor.
///
/// Autosaves the current content to the backend (`note_save`) after a short
/// debounce, drives the shared [`SaveStatus`] indicator, and reports cursor /
/// scroll positions so they can be restored with the persisted session.
#[component]
pub fn NoteEditor(
    /// Vault-relative path of the note being edited.
    #[prop(optional)]
    note_path: Option<String>,
    /// Initial content (used only when the note has no saved content).
    #[prop(optional)]
    initial_content: Option<String>,
    /// Reports the active note path so the app can persist it in the session.
    #[prop(optional)]
    on_active_note: Option<Callback<String>>,
    /// Reports cursor position (char offset) for session restore.
    #[prop(optional)]
    on_cursor: Option<Callback<u32>>,
    /// Reports scroll offset for session restore.
    #[prop(optional)]
    on_scroll: Option<Callback<u32>>,
) -> impl IntoView {
    // A restored session may not have an active note; treat an empty string
    // the same as "no path" so the editor falls back to the default note.
    let path = note_path
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| "new_note.md".to_string());
    let (content, set_content) = signal(initial_content.unwrap_or_default());
    let (show_menu, set_show_menu) = signal(false);
    let save_status = use_save_status();

    // Report the active note path so the app can persist it in the session.
    if let Some(cb) = on_active_note {
        cb.run(path.clone());
    }

    // Load the note content from disk when the editor mounts (the active note
    // may have been restored from a previous session). Clone `path` into a
    // local so the async block owns it while `path` stays usable below.
    let path_read = path.clone();
    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path_read.clone() }))
            .unwrap();
        let result = crate::ipc::tauri_invoke("note_read", args).await;
        if let Ok(saved) = serde_wasm_bindgen::from_value::<String>(result) {
            if !saved.is_empty() {
                set_content.set(saved);
            }
        }
    });

    // Debounced autosave. Each keystroke bumps a dirty counter; an effect
    // observes it and schedules a single save after the debounce window.
    let (dirty, set_dirty) = signal(0u32);
    // `path` is used by several `move` closures and the view below, so clone
    // it for the Effect (which must be `move`/`'static`).
    let path_effect = path.clone();
    Effect::new(move |_| {
        let _ = dirty.get();
        // Clone into a local each run so the `set_timeout` closure can own it
        // while the Effect closure itself stays re-runnable (FnMut) on each
        // dirty bump — otherwise the inner closure would move `path_effect`
        // out and the Effect would be FnOnce.
        let path_save_effect = path_effect.clone();
        set_timeout(
            move || {
                let current = content.get_untracked();
                save_status.status.set(SaveStatus::Saving);
                save_status.detail.set(format!("Saving {}", path_save_effect.clone()));
                let path_save = path_save_effect.clone();
                spawn_local(async move {
                    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                        "path": path_save.clone(),
                        "content": current,
                    }))
                    .unwrap();
                    let result = crate::ipc::tauri_invoke("note_save", args).await;
                    match serde_wasm_bindgen::from_value::<()>(result) {
                        Ok(()) => {
                            save_status.status.set(SaveStatus::Saved);
                            save_status.detail.set(format!("Saved {}", path_save));
                        }
                        Err(_) => {
                            // Surface a retry state; the retry tick below will
                            // re-attempt automatically.
                            save_status.status.set(SaveStatus::Failed);
                            save_status
                                .detail
                                .set("Save failed — editing continues locally".to_string());
                        }
                    }
                });
            },
            std::time::Duration::from_millis(AUTOSAVE_DELAY_MS),
        );
    });

    // Periodic retry of a failed save: if the status is still Failed after a
    // few seconds, bump the dirty counter to re-attempt. `set_interval`
    // returns `()` in leptos 0.7.8; use the handle variant so the interval
    // can be stopped on cleanup.
    let retry_tick = set_interval_with_handle(
        move || {
            if save_status.status.get_untracked() == SaveStatus::Failed {
                save_status.status.set(SaveStatus::Retrying);
                set_dirty.update(|v| *v = v.wrapping_add(1));
            }
        },
        std::time::Duration::from_secs(5),
    );
    on_cleanup(move || {
        if let Ok(handle) = retry_tick {
            handle.clear();
        }
    });

    // Capture cursor / scroll position for session persistence.
    let report_position = move |ta: &web_sys::HtmlTextAreaElement| {
        if let Some(cb) = on_cursor {
            // `selection_start()` returns `Result<Option<u32>, JsValue>`.
            let pos = ta.selection_start().ok().flatten().unwrap_or(0);
            cb.run(pos);
        }
        if let Some(cb) = on_scroll {
            // `scroll_top()` is `i32`; the session stores `u32`.
            cb.run(ta.scroll_top().max(0) as u32);
        }
    };

    // ── Phase 12.1: drag-and-drop into the editor ────────────────────────
    // Dragging an internal note inserts a `[[wikilink]]` at the cursor;
    // dragging an image file inserts `![name](name)` markdown; any other
    // file inserts a `[name](name)` link; plain text is inserted verbatim.
    let ta_ref = NodeRef::<leptos::html::Textarea>::new();
    let (drag_hover, set_drag_hover) = signal(false);

    let insert_at_cursor = move |snippet: String| {
        let Some(ta) = ta_ref.get() else {
            set_content.update(|c| c.push_str(&snippet));
            return;
        };
        let start = ta.selection_start().ok().flatten().unwrap_or(0) as usize;
        let end = ta.selection_end().ok().flatten().unwrap_or(start as u32) as usize;
        let mut value = content.get_untracked();
        value.insert_str(end, &snippet);
        set_content.set(value.clone());
        set_dirty.update(|v| *v = v.wrapping_add(1));
        let new_caret = (end + snippet.len()) as u32;
        let _ = ta.set_selection_range(new_caret, new_caret);
    };

    // Wrap the current selection in markdown markers (Cmd/Ctrl+B → **bold**,
    // Cmd/Ctrl+I → *italic*). Mirrors the ShortcutReference registry.
    let wrap_selection = move |before: &str, after: &str| {
        let Some(ta) = ta_ref.get() else { return };
        let start = ta.selection_start().ok().flatten().unwrap_or(0) as usize;
        let end = ta.selection_end().ok().flatten().unwrap_or(start as u32) as usize;
        let mut value = content.get_untracked();
        let selected: String = value[start..end].to_string();
        value.replace_range(start..end, &format!("{before}{selected}{after}"));
        set_content.set(value);
        set_dirty.update(|v| *v = v.wrapping_add(1));
        // Put the cursor after the closing marker.
        let caret = (end + before.len() + after.len()) as u32;
        let _ = ta.set_selection_range(caret, caret);
    };

    let on_editor_keydown = move |ev: web_sys::KeyboardEvent| {
        let meta = ev.meta_key() || ev.ctrl_key();
        if !meta {
            return;
        }
        let shift = ev.shift_key();
        let key = ev.key();
        if !shift && key.eq_ignore_ascii_case("b") {
            ev.prevent_default();
            wrap_selection("**", "**");
        } else if !shift && key.eq_ignore_ascii_case("i") {
            ev.prevent_default();
            wrap_selection("*", "*");
        }
    };

    let on_editor_dragover = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        set_drag_hover.set(true);
    };
    let on_editor_dragleave = move |_ev: web_sys::DragEvent| set_drag_hover.set(false);
    let on_editor_drop = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        set_drag_hover.set(false);
        let Some(dt) = ev.data_transfer() else { return };

        // Internal note (from the file tree)? Insert a wikilink.
        if let Ok(nabu_path) = dt.get_data(NABU_NOTE_MIME) {
            if !nabu_path.is_empty() {
                let stem = nabu_path
                    .rsplit('/')
                    .next()
                    .unwrap_or(&nabu_path)
                    .trim_end_matches(".md")
                    .to_string();
                insert_at_cursor(format!("[[{}]]", stem));
                return;
            }
        }

        // External files? Insert markdown per kind.
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
                insert_at_cursor(snippets.join("\n"));
                return;
            }
        }

        // Plain text (external drag of selected text).
        if let Ok(text) = dt.get_data("text/plain") {
            if !text.is_empty() {
                insert_at_cursor(text);
            }
        }
    };

    view! {
        <div
            class="note-editor relative h-full flex flex-col"
            on:keydown=move |ev| if ev.key() == "/" { set_show_menu.set(true) }
            on:dragover=on_editor_dragover
            on:dragleave=on_editor_dragleave
            on:drop=on_editor_drop
        >
            <div class="flex items-center justify-between px-1 pb-1 text-xs text-gray-500">
                <span class="truncate">{path.clone()}</span>
                <span class="text-gray-600">"Markdown"</span>
            </div>
            {move || if drag_hover.get() {
                view! {
                    <div class="editor-drop-overlay absolute inset-0 z-20 flex items-center justify-center pointer-events-none rounded border-2 border-dashed border-blue-500 bg-blue-500/10">
                        <span class="text-sm font-medium text-blue-300 bg-gray-900/80 px-3 py-1 rounded">"Drop to insert link, image or text"</span>
                    </div>
                }.into_any()
            } else {
                view! {}.into_any()
            }}
            <textarea
                node_ref=ta_ref
                prop:value=content
                on:keydown=on_editor_keydown
                on:input=move |ev| {
                    let value = event_target_value(&ev);
                    set_content.set(value);
                    set_dirty.update(|v| *v = v.wrapping_add(1));
                }
                on:select=move |ev: web_sys::Event| {
                    if let Some(ta) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok()) {
                        report_position(&ta);
                    }
                }
                on:keyup=move |ev: web_sys::KeyboardEvent| {
                    if let Some(ta) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok()) {
                        report_position(&ta);
                    }
                }
                on:scroll=move |ev: web_sys::Event| {
                    if let Some(ta) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok()) {
                        report_position(&ta);
                    }
                }
                class="editor-textarea flex-1 resize-none"
            />
            {move || if show_menu.get() {
                view! { <SlashMenu on_select=Callback::new(move |_item| { set_show_menu.set(false); /* Insert item logic */ }) /> }.into_any()
            } else {
                view! {}.into_any()
            }}
            <NoteView content=content />
        </div>
    }
}
