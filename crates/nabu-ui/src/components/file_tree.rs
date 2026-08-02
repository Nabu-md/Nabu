use crate::components::ui::button::{Button, ButtonSize};
use crate::components::ui::dialog::ConfirmDialog;
use crate::components::ui::feedback::{use_toast, ToastKind};
use wasm_bindgen_futures::spawn_local;

use leptos::prelude::*;
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_folder: bool,
    pub children: Vec<TreeNode>,
}

#[derive(Clone, Copy)]
pub struct FileTreeContext {
    pub active_file: RwSignal<Option<PathBuf>>,
    pub expanded_folders: RwSignal<HashSet<PathBuf>>,
}

/// Deletes a vault item by moving it to trash, then shows an undo toast so the
/// deletion can be reversed immediately (Phase 11.2 delete workflow).
///
/// `toasts` and `history` are captured at render time by the caller (never
/// looked up inside the async task, which has no reactive owner).
fn move_to_trash(
    path: PathBuf,
    name: String,
    is_folder: bool,
    toasts: crate::components::ui::feedback::ToastContext,
    history: crate::history::HistoryContext,
) {
    let path_str = path.to_string_lossy().to_string();
    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path_str })).unwrap();
        let result = crate::ipc::tauri_invoke("note_delete", args).await;
        match serde_wasm_bindgen::from_value::<()>(result) {
            Ok(()) => {
                let kind = if is_folder { "Folder" } else { "Note" };
                let toasts_undo = toasts;
                let history_undo = history;
                toasts.push_with_action(
                    ToastKind::Info,
                    format!("{kind} moved to Trash"),
                    format!("{name} can be restored from the Trash screen."),
                    crate::components::ui::feedback::ToastAction::new(
                        "Undo",
                        Callback::new(move |_| crate::history::undo(history_undo, toasts_undo)),
                    ),
                );
            }
            Err(_) => toasts.error("Delete", "Could not move the item to trash"),
        }
    });
}

#[component]
pub fn FileTree(nodes: Vec<TreeNode>, on_select: Callback<PathBuf>) -> impl IntoView {
    let active_file = RwSignal::new(None);
    let expanded_folders = RwSignal::new(HashSet::new());
    provide_context(FileTreeContext {
        active_file,
        expanded_folders,
    });

    let (new_file_input, set_new_file_input) = signal(false);
    let (new_folder_input, set_new_folder_input) = signal(false);
    let (name_input, set_name_input) = signal("".to_string());

    let show_new_file = Callback::new(move |_| set_new_file_input.set(true));
    let show_new_folder = Callback::new(move |_| set_new_folder_input.set(true));

    // Per-node delete confirmation is held at the tree level so a single
    // ConfirmDialog serves every node. (path, name, is_folder)
    let (pending_delete, set_pending_delete) = signal(Option::<(PathBuf, String, bool)>::None);
    provide_context(pending_delete);
    provide_context(set_pending_delete);

    // The ConfirmDialog needs a settable signal; sync it from the pending
    // state so overlay/Escape closes stay consistent.
    let delete_open = RwSignal::new(false);
    Effect::new(move |_| delete_open.set(pending_delete.get().is_some()));
    let delete_message = Memo::new(move |_| {
        pending_delete.get().map(|(_, name, is_folder)| {
            if is_folder {
                format!("The folder '{name}' and everything inside it will be moved to Trash. You can restore it or undo this action.")
            } else {
                format!("The note '{name}' will be moved to Trash. You can restore it or undo this action.")
            }
        }).unwrap_or_default()
    });

    let toasts = use_toast();
    let history = crate::history::use_history();
    let confirm_delete = Callback::new(move |_| {
        if let Some((path, name, is_folder)) = pending_delete.get() {
            move_to_trash(path, name, is_folder, toasts, history);
        }
        set_pending_delete.set(None);
    });
    let cancel_delete = Callback::new(move |_| set_pending_delete.set(None));

    view! {
        <div class="file-tree">
            <div class="actions">
                <Button size=ButtonSize::Sm on_click=show_new_file>"+ New File"</Button>
                <Button size=ButtonSize::Sm on_click=show_new_folder>"+ New Folder"</Button>
            </div>
            <ConfirmDialog
                open=delete_open
                title="Move to Trash?".to_string()
                message="".to_string()
                message_signal=delete_message
                confirm_label="Move to Trash"
                cancel_label="Cancel"
                danger=false
                on_confirm=confirm_delete
                on_cancel=cancel_delete
            />

            {move || if new_file_input.get() || new_folder_input.get() {
                view! {
                    <input type="text"
                        prop:value=name_input
                        on:input=move |ev| set_name_input.set(event_target_value(&ev))
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" {
                                let name = name_input.get();
                                spawn_local(async move {
                                    let _ = crate::ipc::tauri_invoke("note_create_file", serde_wasm_bindgen::to_value(&serde_json::json!({"path": name})).unwrap()).await;
                                });
                                set_new_file_input.set(false);
                                set_new_folder_input.set(false);
                                set_name_input.set("".to_string());
                            }
                        }
                    />
                }.into_any()
            } else {
                view! {}.into_any()
            }}

            <ul>
                {nodes.into_iter().map(|node| {
                    view! { <TreeNodeView node=node on_select=on_select /> }
                }).collect_view()}
            </ul>
        </div>
    }
}

#[component]
fn TreeNodeView(node: TreeNode, on_select: Callback<PathBuf>) -> impl IntoView {
    let context = expect_context::<FileTreeContext>();
    let is_folder = node.is_folder;
    let path = node.path.clone();
    let name = node.name.clone();
    let children = node.children.clone();

    let (_expanded, set_expanded) = signal(false);
    // Request delete confirmation at the tree level.
    let delete_path = path.clone();
    let delete_name = name.clone();
    let delete_is_folder = is_folder;
    let on_delete = Callback::new(move |_| {
        expect_context::<WriteSignal<Option<(PathBuf, String, bool)>>>().set(Some((
            delete_path.clone(),
            delete_name.clone(),
            delete_is_folder,
        )));
    });

    view! {
        <li class="tree-node">
            <div class="group flex items-center justify-between">
                <button
                    class="flex-1 flex items-center gap-1 text-left"
                    on:click={
                        let path = path.clone();
                        move |_| {
                            if is_folder {
                                context.expanded_folders.update(|set| {
                                    if set.contains(&path) {
                                        set.remove(&path);
                                    } else {
                                        set.insert(path.clone());
                                    }
                                });
                                set_expanded.update(|e| *e = !*e);
                            } else {
                                context.active_file.set(Some(path.clone()));
                                on_select.run(path.clone());
                            }
                        }
                    }
                >
                    {let path = path.clone();
                     move || if is_folder {
                        if context.expanded_folders.get().contains(&path) { "▼ " } else { "▶ " }
                    } else { "  " }}
                    {name.clone()}
                </button>
                <button
                    class="text-gray-500 hover:text-red-400 opacity-0 group-hover:opacity-100 transition-opacity px-1"
                    title="Move to Trash"
                    aria-label=format!("Move {} to trash", name.clone())
                    on:click=move |ev| {
                        ev.stop_propagation();
                        on_delete.run(());
                    }
                >
                    "🗑"
                </button>
            </div>
            {let path = path.clone();
             move || if is_folder && context.expanded_folders.get().contains(&path) {
                view! {
                    <ul class="pl-4">
                        {children.clone().into_iter().map(|child| {
                            view! { <TreeNodeView node=child on_select=on_select /> }
                        }).collect_view()}
                    </ul>
                }.into_any()
            } else {
                view! {}.into_any()
            }}
        </li>
    }
}
