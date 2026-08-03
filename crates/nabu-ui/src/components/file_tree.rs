//! # Vault File Tree
//!
//! A real, interactive file tree backed by the `tree_list` command:
//!
//! - loads vault-relative nodes (folders first, alphabetical)
//! - expand / collapse folders, single-click opens notes into a tab
//! - multi-select: Cmd/Ctrl-click toggles, Shift-click range-selects,
//!   Cmd/Ctrl+A selects all visible
//! - inline rename (double-click or context menu → Rename), committed with
//!   Enter, cancelled with Escape, matching Finder / VS Code behaviour
//! - context menu per node: New Note, New Folder, Rename, Delete, Duplicate,
//!   Move, Copy Path, Copy Wikilink, Reveal in File Manager, Open in New
//!   Window (future-compatible)
//! - drag-and-drop: drag a note/folder onto a folder (or the empty tree area)
//!   to move it; internal drops carry `application/x-nabu-note` so the editor
//!   can turn them into wikilinks
//! - a batch action bar (Delete / Move to… / Copy Paths / Open All) appears
//!   once two or more items are selected
//!
//! All destructive operations route through the reversible history layer
//! (note_delete → trash with undo toast) so nothing is ever lost.

use crate::components::ui::dialog::ConfirmDialog;
use crate::components::ui::feedback::{use_toast, ToastAction, ToastContext, ToastKind};
use crate::components::ui::menu::{MenuItem, MenuSeparator};
use crate::components::workspace::{open_tab, refresh_tree, use_workspace, WorkspaceContext};
use leptos::prelude::*;
use std::collections::HashSet;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

/// One node of the vault tree (mirrors the backend `TreeEntry`).
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct TreeNode {
    pub name: String,
    /// Vault-relative path (forward slashes).
    pub path: String,
    pub is_folder: bool,
    pub children: Vec<TreeNode>,
}

/// Context-menu state (position + target) for the single tree-level menu.
#[derive(Clone)]
pub struct MenuState {
    pub x: f64,
    pub y: f64,
    pub path: String,
    pub is_folder: bool,
}

/// Shared tree state, provided by [`FileTree`] so recursive node views can
/// reach it without prop-drilling.
#[derive(Clone, Copy)]
pub struct TreeContext {
    pub nodes: RwSignal<Vec<TreeNode>>,
    pub expanded: RwSignal<HashSet<String>>,
    pub selected: RwSignal<Vec<String>>,
    pub anchor: RwSignal<Option<String>>,
    pub renaming: RwSignal<Option<String>>,
    pub rename_value: RwSignal<String>,
    /// `Some((parent_path, is_folder))` while a new-item input is visible.
    pub creating: RwSignal<Option<(String, bool)>>,
    pub create_value: RwSignal<String>,
    pub menu: RwSignal<Option<MenuState>>,
    pub dragging: RwSignal<Option<String>>,
    pub drop_target: RwSignal<Option<String>>,
    /// `Some(paths)` while the "Move to…" folder picker is open.
    pub move_picker: RwSignal<Option<Vec<String>>>,
    pub confirm_open: RwSignal<bool>,
    pub confirm_targets: RwSignal<Vec<String>>,
    pub toasts: ToastContext,
}

fn file_stem(name: &str) -> String {
    name.trim_end_matches(".md").to_string()
}

/// The folder part of a vault-relative path ("" for root).
fn parent_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => path[..i].to_string(),
        None => String::new(),
    }
}

/// The display name of a node (extension stripped for notes).
fn display_name(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    if path.ends_with(".md") {
        name.trim_end_matches(".md").to_string()
    } else {
        name.to_string()
    }
}

/// Depth-first path list of the nodes currently visible (respects expansion).
fn visible_paths(nodes: &[TreeNode], expanded: &HashSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(nodes: &[TreeNode], expanded: &HashSet<String>, out: &mut Vec<String>) {
        for n in nodes {
            out.push(n.path.clone());
            if n.is_folder && expanded.contains(&n.path) {
                walk(&n.children, expanded, out);
            }
        }
    }
    walk(nodes, expanded, &mut out);
    out
}

/// Flattens the tree into (folder path, depth) pairs for the move picker.
fn collect_folders(nodes: &[TreeNode], depth: usize, out: &mut Vec<(String, usize)>) {
    for n in nodes {
        if n.is_folder {
            out.push((n.path.clone(), depth));
            collect_folders(&n.children, depth + 1, out);
        }
    }
}

async fn load_tree() -> Vec<TreeNode> {
    let empty = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = crate::ipc::tauri_invoke("tree_list", empty).await;
    serde_wasm_bindgen::from_value::<Vec<TreeNode>>(result).unwrap_or_default()
}

/// Commits an inline rename via the reversible rename commands.
/// `workspace` is threaded in (captured at render time) so this stays safe
/// to call from async contexts — never `expect_context` inside `spawn_local`.
fn do_rename(
    ctx: TreeContext,
    workspace: WorkspaceContext,
    old_path: String,
    is_folder: bool,
) {
    let new_name = ctx.rename_value.get().trim().to_string();
    ctx.renaming.set(None);
    if new_name.is_empty() || new_name == display_name(&old_path) {
        return;
    }
    let parent = parent_dir(&old_path);
    let mut new_path = if parent.is_empty() {
        new_name.clone()
    } else {
        format!("{}/{}", parent, new_name)
    };
    if !is_folder && !new_path.ends_with(".md") {
        new_path.push_str(".md");
    }
    if new_path == old_path {
        return;
    }
    let cmd = if is_folder { "folder_rename" } else { "note_rename" };
    let toasts = ctx.toasts;
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "from": old_path.clone(),
        "to": new_path.clone(),
    }))
    .unwrap();
    spawn_local(async move {
        let result = crate::ipc::tauri_invoke(cmd, args).await;
        if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
            // Rewrites the renamed note's tab AND, for folders, every tab
            // pointing at a note inside it, so the editor never autosaves
            // files back at the old location.
            crate::components::workspace::rename_tab_prefix(workspace, &old_path, &new_path);
            refresh_tree(workspace);
            toasts.success("Renamed", new_path);
        } else {
            toasts.error("Rename", "Could not rename that item");
        }
    });
}

/// Moves the given vault-relative paths into `dest_folder` ("" = vault root).
fn do_move_items(
    ctx: TreeContext,
    workspace: WorkspaceContext,
    items: Vec<String>,
    dest_folder: String,
) {
    if items.is_empty() {
        return;
    }
    // Guard: never move an item into itself, into one of its descendants, or
    // into the folder it already lives in (a no-op that would otherwise make
    // the backend resolve a "name (1)" conflict copy).
    let invalid = items.iter().any(|src| {
        dest_folder == *src
            || dest_folder.starts_with(&format!("{src}/"))
            || dest_folder == parent_dir(src)
    });
    if invalid {
        ctx.toasts.error("Move", "That item is already in the destination folder");
        return;
    }
    let toasts = ctx.toasts;
    let items_json = items.clone();
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "items": items_json,
        "dest_folder": dest_folder.clone(),
    }))
    .unwrap();
    spawn_local(async move {
        let result = crate::ipc::tauri_invoke("items_move", args).await;
        if serde_wasm_bindgen::from_value::<Vec<String>>(result).is_ok() {
            // Keep open tabs tracking the moved items instead of leaving them
            // pointing at the now-vanished old paths. `rename_tab_prefix`
            // rewrites the exact path AND any tabs inside a moved folder.
            for old_path in &items {
                let name = old_path.rsplit('/').next().unwrap_or(old_path);
                let new_path = if dest_folder.is_empty() {
                    name.to_string()
                } else {
                    format!("{}/{}", dest_folder, name)
                };
                crate::components::workspace::rename_tab_prefix(workspace, old_path, &new_path);
            }
            refresh_tree(workspace);
            toasts.success("Moved", "Items moved");
        } else {
            toasts.error("Move", "Could not move the items");
        }
    });
}

/// Duplicates a note/folder via the reversible duplicate command.
fn do_duplicate(ctx: TreeContext, workspace: WorkspaceContext, path: String) {
    let toasts = ctx.toasts;
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "from": path.clone(),
        "dest": path.clone(),
    }))
    .unwrap();
    spawn_local(async move {
        let result = crate::ipc::tauri_invoke("note_duplicate", args).await;
        match serde_wasm_bindgen::from_value::<String>(result) {
            Ok(new_path) => {
                refresh_tree(workspace);
                toasts.success("Duplicated", format!("Created {}", display_name(&new_path)));
            }
            Err(_) => toasts.error("Duplicate", "Could not duplicate that item"),
        }
    });
}

/// Archives a note/folder into the reserved `archive/` folder (reversible).
fn do_archive(ctx: TreeContext, workspace: WorkspaceContext, path: String) {
    let toasts = ctx.toasts;
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path.clone() })).unwrap();
    spawn_local(async move {
        let result = crate::ipc::tauri_invoke("archive_note", args).await;
        if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
            // The note moved out of normal navigation — close its tab so the
            // editor never autosaves a recreated file at the old path.
            crate::components::workspace::close_tab(workspace, &path);
            refresh_tree(workspace);
            toasts.success("Archived", format!("Moved {} to the Archive", display_name(&path)));
        } else {
            toasts.error("Archive", "Could not archive that item");
        }
    });
}

/// Moves items to the trash (reversible), showing a single undo toast.
fn do_delete(
    ctx: TreeContext,
    workspace: WorkspaceContext,
    history: crate::history::HistoryContext,
    paths: Vec<String>,
) {
    if paths.is_empty() {
        return;
    }
    let toasts = ctx.toasts;
    spawn_local(async move {
        let mut ok = 0;
        for p in &paths {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": p })).unwrap();
            if serde_wasm_bindgen::from_value::<()>(crate::ipc::tauri_invoke("note_delete", args).await)
                .is_ok()
            {
                ok += 1;
                // Close any open tab pointing at the deleted note so the
                // editor never autosaves a recreated file at the old path.
                crate::components::workspace::close_tab(workspace, p);
            }
        }
        if ok > 0 {
            let label = if ok == 1 { "item" } else { "items" };
            let history_undo = history;
            let toasts_undo = toasts;
            toasts.push_with_action(
                ToastKind::Info,
                format!("{ok} {label} moved to Trash"),
                "You can restore them from the Trash screen.",
                ToastAction::new("Undo", Callback::new(move |_| crate::history::undo(history_undo, toasts_undo))),
            );
            refresh_tree(workspace);
        } else {
            toasts.error("Delete", "Could not move those items to Trash");
        }
    });
}

fn copy_text(text: String, label: String, toasts: ToastContext) {
    spawn_local(async move {
        let Some(window) = web_sys::window() else { return };
        let clipboard = window.navigator().clipboard();
        let _ = clipboard.write_text(&text);
        toasts.success(label, "Copied to clipboard");
    });
}

/// Reveals a vault-relative path in the OS file manager.
fn reveal_in_fm(path: String) {
    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path })).unwrap();
        let _ = crate::ipc::tauri_invoke("reveal_in_file_manager", args).await;
    });
}

/// Commits the inline "new note / new folder" creation.
fn do_create(ctx: TreeContext, workspace: WorkspaceContext) {
    let Some((parent, is_folder)) = ctx.creating.get() else { return };
    let name = ctx.create_value.get().trim().to_string();
    ctx.creating.set(None);
    ctx.create_value.set(String::new());
    if name.is_empty() {
        return;
    }
    let path = if parent.is_empty() {
        name.clone()
    } else {
        format!("{}/{}", parent, name)
    };
    let path = if is_folder {
        path
    } else if path.ends_with(".md") {
        path
    } else {
        format!("{path}.md")
    };
    let cmd = if is_folder { "folder_create" } else { "note_create_file" };
    let toasts = ctx.toasts;
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path.clone() })).unwrap();
    spawn_local(async move {
        let result = crate::ipc::tauri_invoke(cmd, args).await;
        if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
            if !is_folder {
                open_tab(workspace, &path);
            }
            refresh_tree(workspace);
            toasts.success(if is_folder { "Folder created" } else { "Note created" }, path);
        } else {
            toasts.error(if is_folder { "Folder" } else { "Note" }, "Could not create that item");
        }
    });
}

/// The vault file tree.
#[component]
pub fn FileTree() -> impl IntoView {
    let workspace = use_workspace();
    let toasts = use_toast();
    let (tree_loaded, set_tree_loaded) = signal(false);
    let nodes = RwSignal::new(Vec::<TreeNode>::new());
    let expanded = RwSignal::new(HashSet::<String>::new());
    let selected = RwSignal::new(Vec::<String>::new());
    let anchor = RwSignal::new(None::<String>);
    let renaming = RwSignal::new(None::<String>);
    let rename_value = RwSignal::new(String::new());
    let creating = RwSignal::new(None::<(String, bool)>);
    let create_value = RwSignal::new(String::new());
    let menu = RwSignal::new(None::<MenuState>);
    let dragging = RwSignal::new(None::<String>);
    let drop_target = RwSignal::new(None::<String>);
    let move_picker = RwSignal::new(None::<Vec<String>>);
    let confirm_open = RwSignal::new(false);
    let confirm_targets = RwSignal::new(Vec::<String>::new());

    let ctx = TreeContext {
        nodes,
        expanded,
        selected,
        anchor,
        renaming,
        rename_value,
        creating,
        create_value,
        menu,
        dragging,
        drop_target,
        move_picker,
        confirm_open,
        confirm_targets,
        toasts,
    };
    provide_context(ctx);

    // (Re)load the tree on mount and whenever the workspace asks for it
    // (after creates / renames / moves / deletes / duplicates / undo-redo).
    let workspace_refresh = workspace.refresh_tree;
    Effect::new(move |_| {
        let _ = workspace_refresh.get();
        let tree_nodes = nodes;
        spawn_local(async move {
            let loaded = load_tree().await;
            tree_nodes.set(loaded);
            set_tree_loaded.set(true);
        });
    });

    // Reveal a note in the tree (from a tab's "Reveal" action): expand the
    // ancestor folders and select the note.
    let reveal_expanded = expanded;
    let reveal_selected = selected;
    let reveal_handle = window_event_listener_untyped("nabu:reveal-note", move |ev: web_sys::Event| {
        let Some(custom) = ev.dyn_ref::<web_sys::CustomEvent>() else { return };
        let Some(path) = custom.detail().as_string() else { return };
        let mut parent = parent_dir(&path);
        while !parent.is_empty() {
            reveal_expanded.update(|set| {
                set.insert(parent.clone());
            });
            parent = parent_dir(&parent);
        }
        reveal_selected.set(vec![path]);
    });
    on_cleanup(move || reveal_handle.remove());

    // Root-level drag handlers: dropping on empty tree space moves to the
    // vault root.
    let container_dragover = move |ev: web_sys::DragEvent| {
        if dragging.get().is_some() {
            ev.prevent_default();
            drop_target.set(Some(String::new()));
        }
    };
    let container_drop = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        let Some(src) = dragging.get() else { return };
        dragging.set(None);
        drop_target.set(None);
        do_move_items(ctx, workspace, vec![src], String::new());
    };
    let container_dragend = move |_ev: web_sys::DragEvent| {
        dragging.set(None);
        drop_target.set(None);
    };

    // Cmd/Ctrl+A selects all visible nodes; Escape clears the selection.
    let tree_keydown = move |ev: web_sys::KeyboardEvent| {
        if (ev.meta_key() || ev.ctrl_key()) && ev.key().eq_ignore_ascii_case("a") {
            ev.prevent_default();
            selected.set(visible_paths(&nodes.get(), &expanded.get()));
        } else if ev.key() == "Escape" {
            selected.set(Vec::new());
            menu.set(None);
            renaming.set(None);
        }
    };

    let history = crate::history::use_history();
    let confirm_message = Memo::new(move |_| {
        let n = confirm_targets.get().len();
        if n == 1 {
            "This item will be moved to Trash. You can restore it or undo the action.".to_string()
        } else {
            format!("These {n} items will be moved to Trash. You can restore them or undo the action.")
        }
    });
    let confirm_action = Callback::new(move |_| {
        let targets = confirm_targets.get();
        do_delete(ctx, workspace, history, targets);
    });

    view! {
        <div
            class="file-tree flex flex-col h-full min-h-0"
            tabindex="0"
            on:keydown=tree_keydown
            on:dragover=container_dragover
            on:drop=container_drop
            on:dragend=container_dragend
        >
            <div class="file-tree-header flex items-center justify-between px-2 pt-1.5 pb-1">
                <span class="text-xs font-semibold uppercase tracking-wider text-gray-500">"Notes"</span>
                <div class="flex gap-1">
                    <button
                        class="btn btn-sm btn-ghost"
                        title="New note"
                        aria-label="New note"
                        on:click=move |_| {
                            creating.set(Some((String::new(), false)));
                            create_value.set(String::new());
                        }
                    >"+ Note"</button>
                    <button
                        class="btn btn-sm btn-ghost"
                        title="New folder"
                        aria-label="New folder"
                        on:click=move |_| {
                            creating.set(Some((String::new(), true)));
                            create_value.set(String::new());
                        }
                    >"+ Folder"</button>
                </div>
            </div>

            // Inline creation input
            {move || if creating.get().is_some() {
                view! {
                    <div class="px-2 py-1 flex items-center gap-1">
                        <input
                            class="input flex-1 text-xs"
                            placeholder=move || if creating.get().map(|(_, f)| f).unwrap_or(false) { "Folder name…" } else { "Note name…" }
                            prop:value=create_value
                            on:input=move |ev| create_value.set(event_target_value(&ev))
                            on:keydown=move |ev| {
                                if ev.key() == "Enter" {
                                    do_create(ctx, workspace);
                                } else if ev.key() == "Escape" {
                                    creating.set(None);
                                    create_value.set(String::new());
                                }
                            }
                        />
                    </div>
                }.into_any()
            } else {
                view! {}.into_any()
            }}

            // Drop-into-root hint while dragging
            {move || if dragging.get().is_some() {
                view! {
                    <div class=move || format!("tree-root-drop text-[11px] px-2 py-1 border-t border-b mx-1 rounded text-center text-gray-400 {}", if drop_target.get().as_deref() == Some("") { "tree-root-drop-active" } else { "" })>
                        "Drop to move to the vault root"
                    </div>
                }.into_any()
            } else {
                view! {}.into_any()
            }}

            // Tree (skeleton while first load is in flight, empty state for a
            // brand-new vault)
            {move || if !tree_loaded.get() {
                view! {
                    <div class="px-2 py-1">
                        <crate::components::ui::feedback::SkeletonList rows=6 />
                    </div>
                }.into_any()
            } else if nodes.get().is_empty() {
                view! {
                    <div class="px-2 py-3">
                        <crate::components::ui::info::EmptyState
                            icon="🗒️"
                            title="Your vault is empty".to_string()
                            description="Create your first note to start building your knowledge base.".to_string()
                        >
                            <button
                                type="button"
                                class="btn btn-sm mt-2"
                                on:click=move |_| {
                                    creating.set(Some((String::new(), false)));
                                    create_value.set(String::new());
                                }
                            >
                                "➕ New Note"
                            </button>
                        </crate::components::ui::info::EmptyState>
                    </div>
                }.into_any()
            } else {
                view! {
                    <ul class="tree-list flex-1 overflow-y-auto min-h-0 py-1">
                        {move || nodes.get().into_iter().map(|node| {
                            view! { <TreeNodeView node=node /> }
                        }).collect_view()}
                    </ul>
                }.into_any()
            }}

            // Batch action bar
            {move || {
                let count = selected.get().len();
                if count < 2 {
                    return view! {}.into_any();
                }
                let toasts_ctx = toasts;
                let clear = Callback::new(move |_| selected.set(Vec::new()));
                let open_all = Callback::new(move |_| {
                    let sel = selected.get();
                    let ws = workspace;
                    for p in sel {
                        if p.ends_with(".md") {
                            open_tab(ws, &p);
                        }
                    }
                });
                let picker = Callback::new(move |_| {
                    move_picker.set(Some(selected.get()));
                });
                let copy = Callback::new(move |_| {
                    let text = selected.get().join("\n");
                    copy_text(text, "Copy Paths".to_string(), toasts_ctx);
                });
                let del = Callback::new(move |_| {
                    confirm_targets.set(selected.get());
                    confirm_open.set(true);
                });
                view! {
                    <div class="batch-bar flex items-center gap-1 px-2 py-1 border-t border-gray-700 bg-gray-900">
                        <span class="text-xs text-gray-300 font-medium mr-1">{count} selected</span>
                        <button class="btn btn-sm" on:click=move |_| open_all.run(())>"Open"</button>
                        <button class="btn btn-sm" on:click=move |_| picker.run(())>"Move to…"</button>
                        <button class="btn btn-sm" on:click=move |_| copy.run(())>"Copy Paths"</button>
                        <button class="btn btn-sm btn-danger" on:click=move |_| del.run(())>"Delete"</button>
                        <button class="btn btn-sm btn-ghost" on:click=move |_| clear.run(())>"✕"</button>
                    </div>
                }.into_any()
            }}

            // Move-to folder picker
            {move || if let Some(items) = move_picker.get() {
                // `items` stays owned by the if-let here; each consumer gets
                // its own clone so no closure moves the shared Vec.
                let items_root = items.clone();
                let close = Callback::new(move |_| move_picker.set(None));
                let pick_root = Callback::new(move |_| {
                    move_picker.set(None);
                    do_move_items(ctx, workspace, items_root.clone(), String::new());
                });
                let mut folders = Vec::new();
                collect_folders(&nodes.get(), 0, &mut folders);
                let items_count = items.len();
                view! {
                    <div class="dialog-overlay" on:click=move |_| close.run(())>
                        <div class="menu move-picker" on:click=move |ev| ev.stop_propagation()>
                            <div class="px-2 py-1 text-xs font-medium text-gray-400">{format!("Move {} item(s) to…", items_count)}</div>
                            <MenuItem label="📁 Vault root".to_string() on_select=pick_root />
                            <MenuSeparator />
                            {folders.into_iter().map(|(path, depth)| {
                                let path_c = path.clone();
                                let items_c = items.clone();
                                let pick = Callback::new(move |_| {
                                    move_picker.set(None);
                                    do_move_items(ctx, workspace, items_c.clone(), path_c.clone());
                                });
                                view! {
                                    <MenuItem label=format!("{}{} {}", "  ".repeat(depth), path, "📁") on_select=pick />
                                }
                            }).collect_view()}
                        </div>
                    </div>
                }.into_any()
            } else {
                view! {}.into_any()
            }}

            // Confirmation for batch delete
            <ConfirmDialog
                open=confirm_open
                title="Move to Trash?".to_string()
                message="".to_string()
                message_signal=confirm_message
                confirm_label="Move to Trash"
                cancel_label="Cancel"
                danger=true
                on_confirm=confirm_action
            />

            // Context menu
            {move || if let Some(m) = menu.get() {
                let path = m.path.clone();
                let is_folder = m.is_folder;
                let parent = if is_folder { path.clone() } else { parent_dir(&path) };
                let close = Callback::new(move |_| menu.set(None));

                let path_rename = path.clone();
                let rename = Callback::new(move |_| {
                    rename_value.set(display_name(&path_rename));
                    renaming.set(Some(path_rename.clone()));
                    menu.set(None);
                });

                let parent_note = parent.clone();
                let new_note = Callback::new(move |_| {
                    creating.set(Some((parent_note.clone(), false)));
                    create_value.set(String::new());
                    expanded.update(|set| { if !parent_note.is_empty() { set.insert(parent_note.clone()); } });
                    menu.set(None);
                });

                let parent_folder = parent.clone();
                let new_folder = Callback::new(move |_| {
                    creating.set(Some((parent_folder.clone(), true)));
                    create_value.set(String::new());
                    expanded.update(|set| { if !parent_folder.is_empty() { set.insert(parent_folder.clone()); } });
                    menu.set(None);
                });

                let path_del = path.clone();
                let del = Callback::new(move |_| {
                    confirm_targets.set(vec![path_del.clone()]);
                    confirm_open.set(true);
                    menu.set(None);
                });

                let path_dup = path.clone();
                let dup = Callback::new(move |_| {
                    do_duplicate(ctx, workspace, path_dup.clone());
                    menu.set(None);
                });

                let path_mv = path.clone();
                let mv = Callback::new(move |_| {
                    move_picker.set(Some(vec![path_mv.clone()]));
                    menu.set(None);
                });

                let path_arch = path.clone();
                let archive = Callback::new(move |_| {
                    do_archive(ctx, workspace, path_arch.clone());
                    menu.set(None);
                });

                let path_copy = path.clone();
                let copy_path = Callback::new(move |_| {
                    copy_text(path_copy.clone(), "Copy Path".to_string(), toasts);
                    menu.set(None);
                });

                let path_wiki = path.clone();
                let copy_wiki = Callback::new(move |_| {
                    let link = format!("[[{}]]", file_stem(&display_name(&path_wiki)));
                    copy_text(link, "Copy Wikilink".to_string(), toasts);
                    menu.set(None);
                });

                let path_reveal = path.clone();
                let reveal = Callback::new(move |_| {
                    reveal_in_fm(path_reveal.clone());
                    menu.set(None);
                });

                let new_window = Callback::new(move |_| {
                    toasts.info("Coming soon", "Opening a note in a new window is on the roadmap.");
                    menu.set(None);
                });
                view! {
                    <div class="fixed inset-0 z-40" on:click=move |_| close.run(()) on:contextmenu=move |ev: web_sys::MouseEvent| { ev.prevent_default(); menu.set(None); }></div>
                    <div
                        class="menu fixed z-50"
                        role="menu"
                        style=move || format!("left: {}px; top: {}px;", m.x, m.y)
                    >
                        <MenuItem label="New Note".to_string() on_select=new_note />
                        <MenuItem label="New Folder".to_string() on_select=new_folder />
                        <MenuSeparator />
                        <MenuItem label="Rename".to_string() hint="⏎".to_string() on_select=rename />
                        <MenuItem label="Delete".to_string() danger=true on_select=del />
                        <MenuItem label="Duplicate".to_string() on_select=dup />
                        <MenuItem label="Move to…".to_string() on_select=mv />
                        <MenuItem label="Archive".to_string() hint="hides from navigation".to_string() on_select=archive />
                        <MenuSeparator />
                        <MenuItem label="Copy Path".to_string() on_select=copy_path />
                        <MenuItem label="Copy Wikilink".to_string() on_select=copy_wiki />
                        <MenuItem label="Reveal in File Manager".to_string() on_select=reveal />
                        <MenuItem label="Open in New Window".to_string() on_select=new_window />
                    </div>
                }.into_any()
            } else {
                view! {}.into_any()
            }}
        </div>
    }
}

/// One recursive tree row.
#[component]
fn TreeNodeView(node: TreeNode) -> impl IntoView {
    let ctx = expect_context::<TreeContext>();
    let workspace = use_workspace();
    let path = node.path.clone();
    let name = node.name.clone();
    let is_folder = node.is_folder;
    let children = node.children.clone();

    // Single / Cmd+Click / Shift+Click selection + open / expand.
    let path_click = path.clone();
    let on_row_click = move |ev: web_sys::MouseEvent| {
        if ev.meta_key() || ev.ctrl_key() {
            ctx.selected.update(|sel| {
                if sel.contains(&path_click) {
                    sel.retain(|p| *p != path_click);
                } else {
                    sel.push(path_click.clone());
                }
            });
            ctx.anchor.set(Some(path_click.clone()));
            return;
        }
        if ev.shift_key() {
            let flat = visible_paths(&ctx.nodes.get(), &ctx.expanded.get());
            if let Some(a) = ctx.anchor.get() {
                if let (Some(ai), Some(bi)) = (
                    flat.iter().position(|p| *p == a),
                    flat.iter().position(|p| *p == path_click),
                ) {
                    let (lo, hi) = if ai < bi { (ai, bi) } else { (bi, ai) };
                    ctx.selected.set(flat[lo..=hi].to_vec());
                    return;
                }
            }
            ctx.selected.set(vec![path_click.clone()]);
            ctx.anchor.set(Some(path_click.clone()));
            return;
        }
        ctx.selected.set(vec![path_click.clone()]);
        ctx.anchor.set(Some(path_click.clone()));
        if is_folder {
            ctx.expanded.update(|set| {
                if set.contains(&path_click) {
                    set.remove(&path_click);
                } else {
                    set.insert(path_click.clone());
                }
            });
        } else {
            open_tab(workspace, &path_click);
        }
    };

    // Double-click → inline rename (Finder / VS Code behaviour).
    let path_dbl = path.clone();
    let on_row_dblclick = move |ev: web_sys::MouseEvent| {
        ev.stop_propagation();
        if ctx.renaming.get().is_none() {
            ctx.rename_value.set(display_name(&path_dbl));
            ctx.renaming.set(Some(path_dbl.clone()));
        }
    };

    // Right-click → select the node and open the tree context menu.
    let path_menu = path.clone();
    let on_contextmenu = move |ev: web_sys::MouseEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        if !ctx.selected.get().contains(&path_menu) {
            ctx.selected.set(vec![path_menu.clone()]);
            ctx.anchor.set(Some(path_menu.clone()));
        }
        ctx.menu.set(Some(MenuState {
            x: ev.client_x() as f64,
            y: ev.client_y() as f64,
            path: path_menu.clone(),
            is_folder,
        }));
    };

    // Drag source.
    let path_drag = path.clone();
    let on_dragstart = move |ev: web_sys::DragEvent| {
        ctx.dragging.set(Some(path_drag.clone()));
        if let Some(dt) = ev.data_transfer() {
            let _ = dt.set_data("application/x-nabu-note", &path_drag);
            let _ = dt.set_data("text/plain", &path_drag);
            let _ = dt.set_effect_allowed("move");
        }
    };
    let on_dragend = move |_ev: web_sys::DragEvent| {
        ctx.dragging.set(None);
        ctx.drop_target.set(None);
    };

    // Drop target: folders accept drops into themselves; notes into their
    // parent folder.
    let path_over = path.clone();
    let on_dragover = move |ev: web_sys::DragEvent| {
        if ctx.dragging.get().is_none() {
            return;
        }
        ev.prevent_default();
        ev.stop_propagation();
        let dest = if is_folder { path_over.clone() } else { parent_dir(&path_over) };
        ctx.drop_target.set(Some(dest));
    };
    let path_drop = path.clone();
    let on_drop = move |ev: web_sys::DragEvent| {
        ev.prevent_default();
        ev.stop_propagation();
        let Some(src) = ctx.dragging.get() else { return };
        ctx.dragging.set(None);
        ctx.drop_target.set(None);
        let dest = if is_folder { path_drop.clone() } else { parent_dir(&path_drop) };
        do_move_items(ctx, workspace, vec![src], dest);
    };

    // Focus the inline rename input when this row enters rename mode. The
    // effect clones `path` so the `move`/`'static` closure can compare paths
    // without touching the component's shared `path` String.
    let path_focus = path.clone();
    let rename_input_ref = NodeRef::<leptos::html::Input>::new();
    let rename_focus = rename_input_ref;
    Effect::new(move |_| {
        if ctx.renaming.get().as_deref() == Some(path_focus.as_str()) {
            set_timeout(
                move || {
                    if let Some(el) = rename_focus.get() {
                        let _ = el.focus();
                        let _ = el.select();
                    }
                },
                std::time::Duration::from_millis(10),
            );
        }
    });

    let path_row = path.clone();
    let row_class = move || {
        let mut c = "tree-row flex items-center gap-1 px-1 py-0.5 mx-1 rounded cursor-pointer select-none text-sm".to_string();
        if ctx.selected.get().contains(&path_row) {
            c.push_str(" tree-row-selected");
        }
        if ctx.dragging.get().as_deref() == Some(path_row.as_str()) {
            c.push_str(" tree-row-dragging");
        }
        if ctx.drop_target.get().as_deref() == Some(path_row.as_str()) {
            c.push_str(" tree-row-drop-target");
        }
        c
    };

    view! {
        <li
            class="tree-node"
            draggable="true"
            on:dragstart=on_dragstart
            on:dragend=on_dragend
            on:dragover=on_dragover
            on:drop=on_drop
        >
            <div
                class=row_class
                on:click=on_row_click
                on:dblclick=on_row_dblclick
                on:contextmenu=on_contextmenu
                role="treeitem"
                aria-selected={let p = path.clone(); move || ctx.selected.get().contains(&p)}
                aria-expanded={let p = path.clone(); move || is_folder && ctx.expanded.get().contains(&p)}
            >
                <span class="tree-chevron w-4 text-center text-xs text-gray-500" aria-hidden="true">
                    {let p = path.clone();
                     move || if is_folder {
                        if ctx.expanded.get().contains(&p) { "▼" } else { "▶" }
                    } else {
                        "•"
                    }}
                </span>
                <span class="tree-icon" aria-hidden="true">{if is_folder { "📁" } else { "📄" }}</span>
                {let p = path.clone();
                 move || if ctx.renaming.get().as_deref() == Some(p.as_str()) {
                    // Clone `p`/`is_folder` into locals BEFORE the inner `move`
                    // closures so the outer block stays FnMut (the view macro
                    // re-runs it on every reactive update). Never touch the
                    // component-scope `path` String from inside this closure.
                    let path_input = p.clone();
                    let is_folder_input = is_folder;
                    let ctx_input = ctx;
                    let workspace_input = workspace;
                    view! {
                        <input
                            class="input flex-1 text-xs py-0"
                            node_ref=rename_input_ref
                            prop:value=ctx.rename_value
                            on:input=move |ev| ctx.rename_value.set(event_target_value(&ev))
                            on:keydown=move |ev| {
                                if ev.key() == "Enter" {
                                    do_rename(ctx_input, workspace_input, path_input.clone(), is_folder_input);
                                } else if ev.key() == "Escape" {
                                    ctx_input.renaming.set(None);
                                }
                            }
                            on:click=move |ev| ev.stop_propagation()
                            on:dblclick=move |ev| ev.stop_propagation()
                            on:dragstart=move |ev| ev.prevent_default()
                        />
                    }.into_any()
                } else {
                    view! {
                        <span class="tree-name truncate flex-1 min-w-0" title=p.clone()>{name.clone()}</span>
                    }.into_any()
                }}
            </div>
            {let p = path.clone();
             move || if is_folder && ctx.expanded.get().contains(&p) {
                view! {
                    <ul class="tree-children ml-3 border-l border-gray-800">
                        {children.clone().into_iter().map(|child| {
                            view! { <TreeNodeView node=child /> }
                        }).collect_view()}
                    </ul>
                }.into_any()
            } else {
                view! {}.into_any()
            }}
        </li>
    }
}
