use crate::components::editor::slash_menu::SlashMenu;
use crate::components::note_view::NoteView;
use crate::components::recovery::save_status::{SaveStatus, use_save_status};
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

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

    view! {
        <div class="note-editor relative h-full flex flex-col" on:keydown=move |ev| if ev.key() == "/" { set_show_menu.set(true) }>
            <div class="flex items-center justify-between px-1 pb-1 text-xs text-gray-500">
                <span class="truncate">{path.clone()}</span>
                <span class="text-gray-600">"Markdown"</span>
            </div>
            <textarea
                prop:value=content
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
