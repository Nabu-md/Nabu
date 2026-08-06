//! # Vault File Tree (Dioxus)
//!
//! A real, interactive file tree backed by the `tree_list` command:
//!
//! - loads vault-relative nodes (folders first, alphabetical)
//! - expand / collapse folders, single-click opens notes into a tab
//! - multi-select: Cmd/Ctrl-click toggles, Shift-click range-selects,
//!   Cmd/Ctrl+A select all visible
//! - inline rename (double-click or context menu → Rename), committed with
//!   Enter, cancelled with Escape
//! - context menu per node: New Note, New Folder, Rename, Delete, Duplicate,
//!   Move, Copy Path, Copy Wikilink, Reveal in File Manager
//! - drag-and-drop: drag a note/folder onto a folder (or the empty tree area)
//!   to move it; internal drops carry `application/x-nabu-note` so the editor
//!   can turn them into wikilinks
//! - a batch action bar (Delete / Move to… / Copy Paths / Open All) appears
//!   once two or more items are selected
//!
//! All destructive operations route through the reversible history layer
//! (note_delete → trash with undo toast) so nothing is ever lost.
//!
//! This is the Dioxus port of the LePtOS `file_tree.rs` — behaviour is
//! preserved identically; only the framework glue changes.

use crate::components::ui::dialog::ConfirmDialog;
use crate::components::ui::feedback::{set_timeout, use_toast, ToastAction, ToastContext, ToastKind};
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::menu::{MenuItem, MenuSeparator};
use crate::components::contexts::{open_tab, refresh_tree, use_workspace, WorkspaceContext};
use crate::history;
use dioxus::prelude::*;
use dioxus::web::WebEventExt;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

// ── Data model ──────────────────────────────────────────────────────

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
    pub nodes: Signal<Vec<TreeNode>>,
    pub expanded: Signal<HashSet<String>>,
    pub selected: Signal<Vec<String>>,
    pub anchor: Signal<Option<String>>,
    pub renaming: Signal<Option<String>>,
    pub rename_value: Signal<String>,
    /// `Some((parent_path, is_folder))` while a new-item input is visible.
    pub creating: Signal<Option<(String, bool)>>,
    pub create_value: Signal<String>,
    pub menu: Signal<Option<MenuState>>,
    pub dragging: Signal<Option<String>>,
    pub drop_target: Signal<Option<String>>,
    /// `Some(paths)` while the "Move to…" folder picker is open.
    pub move_picker: Signal<Option<Vec<String>>>,
    pub confirm_open: Signal<bool>,
    pub confirm_targets: Signal<Vec<String>>,
    pub toasts: ToastContext,
}

// ── Helpers ─────────────────────────────────────────────────────────

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

// ── Action helpers (async, IPC-backed) ──────────────────────────────

/// Commits an inline rename via the reversible rename commands.
fn do_rename(ctx: TreeContext, workspace: WorkspaceContext, old_path: String, is_folder: bool) {
    let new_name = ctx.rename_value.read().trim().to_string();
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
            crate::components::contexts::rename_tab_prefix(workspace, &old_path, &new_path);
            refresh_tree(workspace);
            toasts.success("Renamed", new_path);
        } else {
            toasts.error("Rename", "Could not rename that item");
        }
    });
}

/// Moves the given vault-relative paths into `dest_folder` ("" = vault root).
fn do_move_items(ctx: TreeContext, workspace: WorkspaceContext, items: Vec<String>, dest_folder: String) {
    if items.is_empty() {
        return;
    }
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
            for old_path in &items {
                let name = old_path.rsplit('/').next().unwrap_or(old_path);
                let new_path = if dest_folder.is_empty() {
                    name.to_string()
                } else {
                    format!("{}/{}", dest_folder, name)
                };
                crate::components::contexts::rename_tab_prefix(workspace, old_path, &new_path);
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
            crate::components::contexts::close_tab(workspace, &path);
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
    history: history::HistoryContext,
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
                crate::components::contexts::close_tab(workspace, p);
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
                ToastAction::new("Undo", Callback::new(move |_| {
                    history::undo(history_undo, toasts_undo);
                })),
            );
            refresh_tree(workspace);
        } else {
            toasts.error("Delete", "Could not move those items to Trash");
        }
    });
}

fn copy_text(text: String, label: String, toasts: ToastContext) {
    spawn_local(async move {
        let Some(window) = web_sys::window() else {
            return;
        };
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
    let Some((parent, is_folder)) = ctx.creating.read().clone() else {
        return;
    };
    let name = ctx.create_value.read().trim().to_string();
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
            toasts.success(
                if is_folder { "Folder created" } else { "Note created" },
                path,
            );
        } else {
            toasts.error(
                if is_folder { "Folder" } else { "Note" },
                "Could not create that item",
            );
        }
    });
}

// ── Main component ──────────────────────────────────────────────────

/// The vault file tree.
#[component]
pub fn FileTree() -> Element {
    let workspace = use_workspace();
    let toasts = use_toast();
    let history = history::use_history();

    // Tree state signals
    let nodes = use_signal(Vec::<TreeNode>::new);
    let expanded = use_signal(HashSet::<String>::new);
    let selected = use_signal(Vec::<String>::new);
    let anchor = use_signal(|| None::<String>);
    let renaming = use_signal(|| None::<String>);
    let rename_value = use_signal(String::new);
    let creating = use_signal(|| None::<(String, bool)>);
    let create_value = use_signal(String::new);
    let menu = use_signal(|| None::<MenuState>);
    let dragging = use_signal(|| None::<String>);
    let drop_target = use_signal(|| None::<String>);
    let move_picker = use_signal(|| None::<Vec<String>>);
    let confirm_open = use_signal(|| false);
    let confirm_targets = use_signal(Vec::<String>::new);

    // Track whether the tree has been loaded at least once.
    let tree_loaded = use_signal(|| false);

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

    // (Re)load the tree on mount and whenever the workspace asks for it.
    let workspace_refresh = workspace.refresh_tree;
    let nodes_for_load = nodes;
    let tree_loaded_for_load = tree_loaded;
    use_effect(move || {
        let _ = workspace_refresh.read();
        spawn_local(async move {
            let loaded = load_tree().await;
            nodes_for_load.set(loaded);
            tree_loaded_for_load.set(true);
        });
    });

    // Reveal a note in the tree (from a tab's "Reveal" action): expand the
    // ancestor folders and select the note.
    let expanded_for_reveal = expanded;
    let selected_for_reveal = selected;
    use_effect(move || {
        let Some(window) = web_sys::window() else {
            return;
        };
        let expanded_clone = expanded_for_reveal;
        let selected_clone = selected_for_reveal;
        let handler = Closure::<dyn FnMut(web_sys::Event)>::wrap(Box::new(
            move |ev: web_sys::Event| {
                let Some(custom) = ev.dyn_ref::<web_sys::CustomEvent>() else {
                    return;
                };
                let Some(path) = custom.detail().as_string() else {
                    return;
                };
                let mut parent = parent_dir(&path);
                while !parent.is_empty() {
                    expanded_clone.write_unchecked().insert(parent.clone());
                    parent = parent_dir(&parent);
                }
                selected_clone.set(vec![path]);
            },
        ));
        let _ = window
            .add_event_listener_with_callback("nabu:reveal-note", handler.as_ref().unchecked_ref());
        let handler_boxed: Box<dyn FnMut(web_sys::Event)> = handler;
        handler_boxed.forget(); // app-lifetime listener
    });

    // Root-level drag handlers: dropping on empty tree space moves to the
    // vault root.
    let ctx_for_drag = ctx;
    let ws_for_drag = workspace;
    let container_dragover = move |ev: DragEvent| {
        if ctx_for_drag.dragging.read().is_some() {
            let web = ev.as_web_event();
            web.prevent_default();
            ctx_for_drag.drop_target.set(Some(String::new()));
        }
    };
    let container_drop = move |ev: DragEvent| {
        let web = ev.as_web_event();
        web.prevent_default();
        let Some(src) = ctx_for_drag.dragging.read().clone() else {
            return;
        };
        ctx_for_drag.dragging.set(None);
        ctx_for_drag.drop_target.set(None);
        do_move_items(ctx_for_drag, ws_for_drag, vec![src], String::new());
    };
    let container_dragend = move |_ev: DragEvent| {
        ctx_for_drag.dragging.set(None);
        ctx_for_drag.drop_target.set(None);
    };

    // Cmd/Ctrl+A selects all visible nodes; Escape clears the selection.
    let ctx_for_key = ctx;
    let tree_keydown = move |ev: KeyboardEvent| {
        let web = ev.as_web_event();
        if (web.meta_key() || web.ctrl_key()) && web.key().eq_ignore_ascii_case("a") {
            web.prevent_default();
            let nodes_snapshot = ctx_for_key.nodes.read().clone();
            let expanded_snapshot = ctx_for_key.expanded.read().clone();
            ctx_for_key
                .selected
                .set(visible_paths(&nodes_snapshot, &expanded_snapshot));
        } else if web.key() == "Escape" {
            ctx_for_key.selected.set(Vec::new());
            ctx_for_key.menu.set(None);
            ctx_for_key.renaming.set(None);
        }
    };

    // Confirmation dialog for batch delete
    let confirm_message = use_memo(move || {
        let n = ctx.confirm_targets.read().len();
        if n == 1 {
            "This item will be moved to Trash. You can restore it or undo the action.".to_string()
        } else {
            format!("These {n} items will be moved to Trash. You can restore them or undo the action.")
        }
    });

    let ctx_for_confirm = ctx;
    let ws_for_confirm = workspace;
    let history_for_confirm = history;
    let confirm_action = move |_| {
        let targets = ctx_for_confirm.confirm_targets.read().clone();
        do_delete(ctx_for_confirm, ws_for_confirm, history_for_confirm, targets);
    };

    rsx! {
        div {
            class: "file-tree flex flex-col h-full min-h-0",
            tabindex: "0",
            onkeydown: tree_keydown,
            ondragover: container_dragover,
            ondrop: container_drop,
            ondragend: container_dragend,

            div {
                class: "file-tree-header flex items-center justify-between px-2 pt-1.5 pb-1",
                span { class: "text-xs font-semibold uppercase tracking-wider text-gray-500", "Notes" }
                div { class: "flex gap-1" }
                button {
                    class: "btn btn-sm btn-ghost",
                    title: "New note",
                    aria-label: "New note",
                    onclick: move |_| {
                        ctx.creating.set(Some((String::new(), false)));
                        ctx.create_value.set(String::new());
                    },
                    "+ Note",
                }
                button {
                    class: "btn btn-sm btn-ghost",
                    title: "New folder",
                    aria-label: "New folder",
                    onclick: move |_| {
                        ctx.creating.set(Some((String::new(), true)));
                        ctx.create_value.set(String::new());
                    },
                    "+ Folder",
                }
            }

            // Inline creation input
            {move || {
                let creating = ctx.creating.read().clone();
                if creating.is_some() {
                    let is_folder = creating.unwrap().1;
                    let ctx_clone = ctx;
                    rsx! {
                        div {
                            class: "px-2 py-1 flex items-center gap-1",
                            input {
                                class: "input flex-1 text-xs py-0",
                                placeholder: if is_folder { "Folder name…" } else { "Note name…" },
                                value: "{ctx_clone.create_value.read()}",
                                onchange: move |ev: FormEvent| {
                                    ctx_clone.create_value.set(ev.value());
                                },
                                onkeydown: move |ev: KeyboardEvent| {
                                    let web = ev.as_web_event();
                                    if web.key() == "Enter" {
                                        do_create(ctx_clone, ws_for_drag);
                                    } else if web.key() == "Escape" {
                                        ctx_clone.creating.set(None);
                                        ctx_clone.create_value.set(String::new());
                                    }
                                },
                            }
                        }
                    }
                } else {
                    rsx! {}
                }
            }}

            // Drop-into-root hint while dragging
            {move || {
                if ctx.dragging.read().is_some() {
                    let is_active = ctx.drop_target.read().as_deref() == Some("");
                    let cls = format!(
                        "tree-root-drop text-[11px] px-2 py-1 border-t border-b mx-1 rounded text-center text-gray-400 {}",
                        if is_active { "tree-root-drop-active" } else { "" }
                    );
                    rsx! {
                        div { class: "{cls}", "Drop to move to the vault root" }
                    }
                } else {
                    rsx! {}
                }
            }}

            // Tree (skeleton while first load is in flight, empty state for a
            // brand-new vault)
            {move || {
                if !*tree_loaded.read() {
                    rsx! {
                        div { class: "px-2 py-1" }
                        crate::components::ui::feedback::SkeletonList { rows: Some(6) }
                    }
                } else if ctx.nodes.read().is_empty() {
                    rsx! {
                        div { class: "px-2 py-3" }
                        crate::components::ui::info::EmptyState {
                            icon: Some(Icon::FolderOpen),
                            title: "Your vault is empty".to_string(),
                            description: Some("Create your first note to start building your knowledge base.".to_string()),
                            children: rsx! {
                                button {
                                    r#type: "button",
                                    class: "btn btn-sm mt-2",
                                    onclick: move |_| {
                                        ctx.creating.set(Some((String::new(), false)));
                                        ctx.create_value.set(String::new());
                                    },
                                    {render_icon_view(Icon::Plus)}
                                    " New Note",
                                }
                            },
                        }
                    }
                } else {
                    let nodes_snapshot: Vec<TreeNode> = ctx.nodes.read().iter().cloned().collect();
                    rsx! {
                        ul { class: "tree-list flex-1 overflow-y-auto min-h-0 py-1" }
                        for node in nodes_snapshot {
                            TreeNodeView { node: node }
                        }
                    }
                }
            }}}

            // Batch action bar
            {move || {
                let count = ctx.selected.read().len();
                if count < 2 {
                    return rsx! {};
                }
                let toasts_ctx = ctx.toasts;
                let ws_batch = workspace;
                let ctx_batch = ctx;
                rsx! {
                    div {
                        class: "batch-bar flex items-center gap-1 px-2 py-1 border-t border-gray-700 bg-gray-900",
                        span { class: "text-xs text-gray-300 font-medium mr-1", "{count} selected" }
                        button { class: "btn btn-sm", onclick: move |_| {
                            let sel = ctx_batch.selected.read().iter().cloned().filter(|p| p.ends_with(".md")).collect::<Vec<_>>();
                            for p in &sel {
                                open_tab(ws_batch, p);
                            }
                        }, "Open" }
                        button { class: "btn btn-sm", onclick: move |_| {
                            ctx_batch.move_picker.set(Some(ctx_batch.selected.read().clone()));
                        }, "Move to…" }
                        button { class: "btn btn-sm", onclick: move |_| {
                            let text = ctx_batch.selected.read().join("\n");
                            copy_text(text, "Copy Paths".to_string(), toasts_ctx);
                        }, "Copy Paths" }
                        button { class: "btn btn-sm btn-danger", onclick: move |_| {
                            ctx_batch.confirm_targets.set(ctx_batch.selected.read().clone());
                            ctx_batch.confirm_open.set(true);
                        }, "Delete" }
                        button { class: "btn btn-sm btn-ghost", aria-label: "Clear selection", onclick: move |_| {
                            ctx_batch.selected.set(Vec::new());
                        } }
                        {render_icon_view(Icon::X)}
                    }
                }
            }}}

            // Move-to folder picker
            {move || {
                let picker = ctx.move_picker.read().clone();
                if let Some(items) = picker {
                    let items_count = items.len();
                    let ctx_picker = ctx;
                    let ws_picker = workspace;
                    let nodes_for_folders = ctx.nodes.read().clone();
                    let mut folders = Vec::new();
                    collect_folders(&nodes_for_folders, 0, &mut folders);
                    rsx! {
                        div {
                            class: "dialog-overlay",
                            onclick: move |_| { ctx_picker.move_picker.set(None); },
                            div {
                                class: "menu move-picker",
                                onclick: move |ev: MouseEvent| ev.stop_propagation(),
                                div {
                                    class: "px-2 py-1 text-xs font-medium text-gray-400",
                                    "Move {items_count} item(s) to…",
                                }
                                MenuItem {
                                    icon: Some(Icon::Folder),
                                    label: "Vault root".to_string(),
                                    on_select: move |_| {
                                        ctx_picker.move_picker.set(None);
                                        do_move_items(ctx_picker, ws_picker, items.clone(), String::new());
                                    },
                                }
                                MenuSeparator {}
                                for (path, depth) in &folders {
                                    {
                                        let path_folder = path.clone();
                                        rsx! {
                                            MenuItem {
                                                key: "{path_folder}",
                                                icon: Some(Icon::Folder),
                                                label: format!("{}{}", "  ".repeat(*depth), path_folder),
                                                on_select: move |_| {
                                                    ctx_picker.move_picker.set(None);
                                                    do_move_items(ctx_picker, ws_picker, items.clone(), path_folder.clone());
                                                },
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else {
                    rsx! {}
                }
            }}}

            // Confirmation for batch delete
        }
        ConfirmDialog {
            open: confirm_open,
            title: "Move to Trash?".to_string(),
            message: String::new(),
            message_signal: Some(confirm_message),
            confirm_label: Some("Move to Trash"),
            cancel_label: Some("Cancel"),
            danger: true,
            on_confirm: Some(confirm_action),
        }

        // Context menu
        {move || {
            let menu_state = ctx.menu.read().clone();
            if let Some(m) = menu_state {
                let path = m.path.clone();
                let is_folder = m.is_folder;
                let parent = if is_folder { path.clone() } else { parent_dir(&path) };
                let ctx_menu = ctx;
                let ws_menu = workspace;

                // Rename
                let path_rename = path.clone();
                let rename_cb = move |_| {
                    ctx_menu.rename_value.set(display_name(&path_rename));
                    ctx_menu.renaming.set(Some(path_rename.clone()));
                    ctx_menu.menu.set(None);
                };

                // New note
                let parent_note = parent.clone();
                let new_note_cb = move |_| {
                    ctx_menu.creating.set(Some((parent_note.clone(), false)));
                    ctx_menu.create_value.set(String::new());
                    ctx_menu.expanded.write_unchecked().insert(parent_note.clone());
                    ctx_menu.menu.set(None);
                };

                // New folder
                let parent_folder = parent.clone();
                let new_folder_cb = move |_| {
                    ctx_menu.creating.set(Some((parent_folder.clone(), true)));
                    ctx_menu.create_value.set(String::new());
                    ctx_menu.expanded.write_unchecked().insert(parent_folder.clone());
                    ctx_menu.menu.set(None);
                };

                // Delete
                let path_del = path.clone();
                let del_cb = move |_| {
                    ctx_menu.confirm_targets.set(vec![path_del.clone()]);
                    ctx_menu.confirm_open.set(true);
                    ctx_menu.menu.set(None);
                };

                // Duplicate
                let path_dup = path.clone();
                let dup_cb = move |_| {
                    do_duplicate(ctx_menu, ws_menu, path_dup.clone());
                    ctx_menu.menu.set(None);
                };

                // Move to…
                let path_mv = path.clone();
                let mv_cb = move |_| {
                    ctx_menu.move_picker.set(Some(vec![path_mv.clone()]));
                    ctx_menu.menu.set(None);
                };

                // Archive
                let path_arch = path.clone();
                let arch_cb = move |_| {
                    do_archive(ctx_menu, ws_menu, path_arch.clone());
                    ctx_menu.menu.set(None);
                };

                // Copy path
                let path_copy = path.clone();
                let copy_path_cb = move |_| {
                    copy_text(path_copy.clone(), "Copy Path".to_string(), ctx_menu.toasts);
                    ctx_menu.menu.set(None);
                };

                // Copy wikilink
                let path_wiki = path.clone();
                let copy_wiki_cb = move |_| {
                    let link = format!("[[{}]]", file_stem(&display_name(&path_wiki)));
                    copy_text(link, "Copy Wikilink".to_string(), ctx_menu.toasts);
                    ctx_menu.menu.set(None);
                };

                // Reveal
                let path_reveal = path.clone();
                let reveal_cb = move |_| {
                    reveal_in_fm(path_reveal.clone());
                    ctx_menu.menu.set(None);
                };

                // New window (future)
                let new_window_cb = move |_| {
                    ctx_menu.toasts.info(
                        "Coming soon",
                        "Opening a note in a new window is on the roadmap.",
                    );
                    ctx_menu.menu.set(None);
                };

                rsx! {
                    div {
                        class: "fixed inset-0 z-40",
                        onclick: move |_| { ctx_menu.menu.set(None); },
                        oncontextmenu: move |ev: MouseEvent| {
                            let web = ev.as_web_event();
                            web.prevent_default();
                            ctx_menu.menu.set(None);
                        },
                    }
                    div {
                        class: "menu fixed z-50",
                        role: "menu",
                        style: "left: {m.x}px; top: {m.y}px;",
                        onclick: move |_| { ctx_menu.menu.set(None); },
                        MenuItem { label: "New Note".to_string(), on_select: new_note_cb }
                        MenuItem { label: "New Folder".to_string(), on_select: new_folder_cb }
                        MenuSeparator {}
                        MenuItem { label: "Rename".to_string(), hint: Some("⏎".to_string()), on_select: rename_cb }
                        MenuItem { label: "Delete".to_string(), danger: true, on_select: del_cb }
                        MenuItem { label: "Duplicate".to_string(), on_select: dup_cb }
                        MenuItem { label: "Move to…".to_string(), on_select: mv_cb }
                        MenuItem { label: "Archive".to_string(), hint: Some("hides from navigation".to_string()), on_select: arch_cb }
                        MenuSeparator {}
                        MenuItem { label: "Copy Path".to_string(), on_select: copy_path_cb }
                        MenuItem { label: "Copy Wikilink".to_string(), on_select: copy_wiki_cb }
                        MenuItem { label: "Reveal in File Manager".to_string(), on_select: reveal_cb }
                        MenuItem { label: "Open in New Window".to_string(), on_select: new_window_cb }
                    }
                }
            } else {
                rsx! {}
            }
        }}
    }
}

// ── Recursive tree node ─────────────────────────────────────────────────

/// One recursive tree row.
#[component]
fn TreeNodeView(node: TreeNode) -> Element {
    let ctx = use_context::<TreeContext>();
    let workspace = use_workspace();
    let path = node.path.clone();
    let name = node.name.clone();
    let is_folder = node.is_folder;
    let children = node.children.clone();

    // Rename input ref — captured via `onmounted`.
    let input_ref: Rc<std::cell::RefCell<Option<web_sys::HtmlInputElement>>> =
        use_hook(|| Rc::new(std::cell::RefCell::new(None)));

    // Single / Cmd+Click / Shift+Click selection + open / expand.
    let ctx_click = ctx;
    let ws_click = workspace;
    let on_row_click = move |ev: MouseEvent| {
        let web = ev.as_web_event();
        if web.meta_key() || web.ctrl_key() {
            let mut sel = ctx_click.selected.read().clone();
            if sel.contains(&path) {
                sel.retain(|p| *p != path);
            } else {
                sel.push(path.clone());
            }
            ctx_click.selected.set(sel);
            ctx_click.anchor.set(Some(path.clone()));
            return;
        }
        if web.shift_key() {
            let nodes_snapshot = ctx_click.nodes.read().clone();
            let expanded_snapshot = ctx_click.expanded.read().clone();
            let flat = visible_paths(&nodes_snapshot, &expanded_snapshot);
            if let Some(a) = ctx_click.anchor.read().clone() {
                if let (Some(ai), Some(bi)) = (
                    flat.iter().position(|p| *p == a),
                    flat.iter().position(|p| *p == path),
                ) {
                    let (lo, hi) = if ai < bi { (ai, bi) } else { (bi, ai) };
                    ctx_click.selected.set(flat[lo..=hi].to_vec());
                    return;
                }
            }
            ctx_click.selected.set(vec![path.clone()]);
            ctx_click.anchor.set(Some(path.clone()));
            return;
        }
        ctx_click.selected.set(vec![path.clone()]);
        ctx_click.anchor.set(Some(path.clone()));
        if is_folder {
            ctx_click.expanded.with_mut(|set| {
                if set.contains(&path) {
                    set.remove(&path);
                } else {
                    set.insert(path.clone());
                }
            });
        } else {
            open_tab(ws_click, &path);
        }
    };

    // Double-click → inline rename (Finder / VS Code behaviour).
    let path_dbl = path.clone();
    let on_row_dblclick = move |ev: MouseEvent| {
        let web = ev.as_web_event();
        web.stop_propagation();
        if ctx.renaming.read().is_none() {
            ctx.rename_value.set(display_name(&path_dbl));
            ctx.renaming.set(Some(path_dbl.clone()));
        }
    };

    // Right-click → select the node and open the tree context menu.
    let path_menu = path.clone();
    let on_contextmenu = move |ev: MouseEvent| {
        let web = ev.as_web_event();
        web.prevent_default();
        web.stop_propagation();
        if !ctx.selected.read().contains(&path_menu) {
            ctx.selected.set(vec![path_menu.clone()]);
            ctx.anchor.set(Some(path_menu.clone()));
        }
        ctx.menu.set(Some(MenuState {
            x: web.client_x() as f64,
            y: web.client_y() as f64,
            path: path_menu.clone(),
            is_folder,
        }));
    };

    // Drag source.
    let path_drag = path.clone();
    let on_dragstart = move |ev: DragEvent| {
        let web = ev.as_web_event();
        ctx.dragging.set(Some(path_drag.clone()));
        if let Some(dt) = web.data_transfer() {
            let _ = dt.set_data("application/x-nabu-note", &path_drag);
            let _ = dt.set_data("text/plain", &path_drag);
            let _ = dt.set_effect_allowed("move");
        }
    };
    let on_dragend = move |_ev: DragEvent| {
        ctx.dragging.set(None);
        ctx.drop_target.set(None);
    };

    // Drop target: folders accept drops into themselves; notes into their
    // parent folder.
    let path_over = path.clone();
    let on_dragover = move |ev: DragEvent| {
        if ctx.dragging.read().is_none() {
            return;
        }
        let web = ev.as_web_event();
        web.prevent_default();
        web.stop_propagation();
        let dest = if is_folder { path_over.clone() } else { parent_dir(&path_over) };
        ctx.drop_target.set(Some(dest));
    };
    let path_drop = path.clone();
    let on_drop = move |ev: DragEvent| {
        let web = ev.as_web_event();
        web.prevent_default();
        web.stop_propagation();
        let Some(src) = ctx.dragging.read().clone() else { return };
        ctx.dragging.set(None);
        ctx.drop_target.set(None);
        let dest = if is_folder { path_drop.clone() } else { parent_dir(&path_drop) };
        do_move_items(ctx, ws_click, vec![src], dest);
    };

    // Focus the inline rename input when this row enters rename mode.
    let path_focus = path.clone();
    use_effect(move || {
        if ctx.renaming.read().as_deref() == Some(path_focus.as_str()) {
            let input_ref_clone = input_ref.clone();
            set_timeout(
                move || {
                    if let Some(el) = input_ref_clone.borrow().as_ref() {
                        let _ = el.focus();
                        let _ = el.select();
                    }
                },
                10,
            );
        }
    });

    let path_row = path.clone();
    let row_class = move || {
        let mut c = "tree-row flex items-center gap-1 px-1 py-0.5 mx-1 rounded cursor-pointer select-none text-sm".to_string();
        if ctx.selected.read().contains(&path_row) {
            c.push_str(" tree-row-selected");
        }
        if ctx.dragging.read().as_deref() == Some(path_row.as_str()) {
            c.push_str(" tree-row-dragging");
        }
        if ctx.drop_target.read().as_deref() == Some(path_row.as_str()) {
            c.push_str(" tree-row-drop-target");
        }
        c
    };

    rsx! {
        li {
            class: "tree-node",
            draggable: "true",
            ondragstart: on_dragstart,
            ondragend: on_drag_end_wrapper,
            ondragover: on_dragover,
            ondrop: on_drop,

            div {
                class: row_class,
                onclick: on_row_click,
                ondblclick: on_row_dblclick,
                oncontextmenu: on_context_menu,
                role: "treeitem",
                "aria-selected": "{ctx.selected.read().contains(&path)}",
                "aria-expanded": if is_folder {
                    "{ctx.expanded.read().contains(&path)}"
                } else {
                    ""
                },

                // Chevron / bullet
                {move || {
                    let p = path.clone();
                    let mut c = "tree-chevron w-4 text-center text-xs text-gray-500".to_string();
                    if is_folder && ctx.expanded.read().contains(&p) {
                        c.push_str(" tree-chevron-open");
                    }
                    rsx! {
                        span { class: "{c}", "aria-hidden": "true" }
                        if is_folder {
                            if ctx.expanded.read().contains(&p) {
                                {render_icon_view(Icon::ChevronDown)}
                            } else {
                                {render_icon_view(Icon::ChevronRight)}
                            }
                        } else {
                            span { "•" }
                        }
                    }
                }}}

                // Icon
                span { class: "tree-icon", "aria-hidden": "true" }
                {render_icon_view(if is_folder { Icon::Folder } else { Icon::FileText })}

                // Label or rename input
                {move || {
                    let p = path.clone();
                    if ctx.renaming.read().as_deref() == Some(p.as_str()) {
                        let ctx_rename = ctx;
                        let ws_rename = workspace;
                        let input_ref_clone = input_ref.clone();
                        rsx! {
                            input {
                                class: "input flex-1 text-xs py-0",
                                onmounted: move |ev: MountedEvent| {
                                    let web = ev.data().as_web_event();
                                    if let Ok(input) = web.dyn_into::<web_sys::HtmlInputElement>() {
                                        *input_ref_clone.borrow_mut() = Some(input);
                                    }
                                },
                                value: "{ctx_rename.rename_value.read()}",
                                onchange: move |ev: FormEvent| {
                                    ctx_rename.rename_value.set(ev.value());
                                },
                                onkeydown: move |ev: KeyboardEvent| {
                                    let web = ev.as_web_event();
                                    if web.key() == "Enter" {
                                        do_rename(ctx_rename, ws_rename, p.clone(), is_folder);
                                    } else if web.key() == "Escape" {
                                        ctx_rename.renaming.set(None);
                                    }
                                },
                                onclick: move |ev: MouseEvent| {
                                    ev.as_web_event().stop_propagation();
                                },
                                ondblclick: move |ev: MouseEvent| {
                                    ev.as_web_event().stop_propagation();
                                },
                                ondragstart: move |ev: DragEvent| {
                                    ev.as_web_event().prevent_default();
                                },
                            }
                        }
                    } else {
                        rsx! {
                            span { class: "tree-name truncate flex-1 min-w-0", title: "{p}", "{name}" }
                        }
                    }
                }}}
            }

            // Children (recursive)
            {move || {
                let p = path.clone();
                if is_folder && ctx.expanded.read().contains(&p) {
                    rsx! {
                        ul { class: "tree-children ml-3 border-l border-gray-800" }
                        for child in &children {
                            TreeNodeView { node: child.clone() }
                        }
                    }
                } else {
                    rsx! {}
                }
            }}}
        }
    }
}
