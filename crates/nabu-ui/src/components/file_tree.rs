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

use crate::components::contexts::{
    close_tab, open_tab, refresh_tree, rename_tab_prefix, use_workspace, WorkspaceContext,
};
use crate::components::ui::dialog::ConfirmDialog;
use crate::components::ui::feedback::{set_timeout, use_toast, ToastAction, ToastContext, ToastKind};
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::info::EmptyState;
use crate::components::ui::feedback::SkeletonList;
use crate::components::ui::menu::{MenuItem, MenuSeparator};
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

/// The file stem (name without `.md`) for a vault-relative path.
fn file_stem(path: &str) -> String {
    path.rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(".md")
        .to_string()
}

/// The folder part of a vault-relative path ("" for root).
fn parent_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => path[..i].to_string(),
        None => String::new(),
    }
}

/// Display name for a node (extension stripped for notes).
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

/// Loads the full vault tree via the `tree_list` IPC command.
async fn load_tree() -> Vec<TreeNode> {
    let empty = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
    let result = crate::ipc::tauri_invoke("tree_list", empty).await;
    serde_wasm_bindgen::from_value::<Vec<TreeNode>>(result).unwrap_or_default()
}

// ── Action helpers (async, IPC-backed) ──────────────────────────────

/// Commits an inline rename via the reversible rename commands.
fn do_rename(mut ctx: TreeContext, workspace: WorkspaceContext, old_path: String, is_folder: bool) {
    let new_name = ctx.rename_value.read().trim().to_string();
    *ctx.renaming.write_unchecked() = None;
    if new_name.is_empty() || new_name == display_name(&old_path) {
        return;
    }
    let parent = parent_dir(&old_path);
    let mut new_path = if parent.is_empty() {
        new_name.clone()
    } else {
        format!("{parent}/{new_name}")
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
        "to":   new_path.clone(),
    }))
    .unwrap();
    spawn_local(async move {
        let result = crate::ipc::tauri_invoke(cmd, args).await;
        if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
            rename_tab_prefix(workspace, &old_path, &new_path);
            refresh_tree(workspace);
            toasts.success("Renamed", new_path);
        } else {
            toasts.error("Rename", "Could not rename that item");
        }
    });
}

/// Moves the given vault-relative paths into `dest_folder` ("" = vault root).
fn do_move_items(
    mut ctx: TreeContext,
    workspace: WorkspaceContext,
    items: Vec<String>,
    dest_folder: String,
) {
    if items.is_empty() {
        return;
    }
    // Guard against moving a folder into itself or a descendant.
    let invalid = items.iter().any(|src| {
        dest_folder == *src
            || dest_folder.starts_with(&format!("{src}/"))
            || dest_folder == parent_dir(src)
    });
    if invalid {
        ctx.toasts
            .error("Move", "That item is already in the destination folder");
        return;
    }
    let toasts = ctx.toasts;
    let items_json = items.clone();
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "items":       items_json,
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
                    format!("{dest_folder}/{name}")
                };
                rename_tab_prefix(workspace, old_path, &new_path);
            }
            refresh_tree(workspace);
            toasts.success("Moved", "Items moved");
        } else {
            toasts.error("Move", "Could not move the items");
        }
    });
}

/// Duplicates a note/folder via the reversible duplicate command.
fn do_duplicate(mut ctx: TreeContext, workspace: WorkspaceContext, path: String) {
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
                toasts.success(
                    "Duplicated",
                    format!("Created {}", display_name(&new_path)),
                );
            }
            Err(_) => toasts.error("Duplicate", "Could not duplicate that item"),
        }
    });
}

/// Archives a note/folder into the reserved `archive/` folder (reversible).
fn do_archive(mut ctx: TreeContext, workspace: WorkspaceContext, path: String) {
    let toasts = ctx.toasts;
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path.clone() })).unwrap();
    spawn_local(async move {
        let result = crate::ipc::tauri_invoke("archive_note", args).await;
        if serde_wasm_bindgen::from_value::<()>(result).is_ok() {
            close_tab(workspace, &path);
            refresh_tree(workspace);
            toasts.success(
                "Archived",
                format!("Moved {} to the Archive", display_name(&path)),
            );
        } else {
            toasts.error("Archive", "Could not archive that item");
        }
    });
}

/// Moves items to the trash (reversible), showing a single undo toast.
fn do_delete(
    mut ctx: TreeContext,
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
            let args =
                serde_wasm_bindgen::to_value(&serde_json::json!({ "path": p })).unwrap();
            if serde_wasm_bindgen::from_value::<()>(
                crate::ipc::tauri_invoke("note_delete", args).await,
            )
            .is_ok()
            {
                ok += 1;
                close_tab(workspace, p);
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

/// Copies `text` to the OS clipboard and shows a confirmation toast.
fn copy_text(text: String, label: String, toasts: ToastContext) {
    spawn_local(async move {
        let Some(window) = web_sys::window() else {
            return;
        };
        let _ = window.navigator().clipboard().write_text(&text);
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
fn do_create(mut ctx: TreeContext, workspace: WorkspaceContext) {
    let Some((parent, is_folder)) = ctx.creating.read().clone() else {
        return;
    };
    let name = ctx.create_value.read().trim().to_string();
    *ctx.creating.write_unchecked() = None;
    *ctx.create_value.write_unchecked() = String::new();
    if name.is_empty() {
        return;
    }
    let path = if parent.is_empty() {
        name.clone()
    } else {
        format!("{parent}/{name}")
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
    let ws_refresh = workspace.refresh_tree;
    let nodes_load = nodes;
    let loaded_load = tree_loaded;
    use_effect(move || {
        let _ = ws_refresh.read();
        spawn_local(async move {
            let loaded = load_tree().await;
            *nodes_load.write_unchecked() = loaded;
            *loaded_load.write_unchecked() = true;
        });
    });

    // Reveal a note in the tree (from a tab's "Reveal" action): expand the
    // ancestor folders and select the note. This is an app-lifetime listener.
    let expanded_reveal = expanded;
    let selected_reveal = selected;
    use_effect(move || {
        let Some(window) = web_sys::window() else {
            return;
        };
        let expanded_c = expanded_reveal;
        let selected_c = selected_reveal;
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
                    expanded_c.write_unchecked().insert(parent.clone());
                    parent = parent_dir(&parent);
                }
                *selected_c.write_unchecked() = vec![path];
            },
        ));
        let _ = window.add_event_listener_with_callback(
            "nabu:reveal-note",
            handler.as_ref().unchecked_ref(),
        );
        handler.forget(); // app-lifetime listener
    });

    // Root-level drag handlers: dropping on empty tree space moves to the
    // vault root.
    let ctx_drag = ctx;
    let ws_drag = workspace;
    let container_dragover = move |ev: DragEvent| {
        if ctx_drag.dragging.read().is_some() {
            ev.prevent_default();
            let web = ev.data().as_web_event();
            web.prevent_default();
            *ctx_drag.drop_target.write_unchecked() = Some(String::new());
        }
    };
    let container_drop = move |ev: DragEvent| {
        let web = ev.data().as_web_event();
        web.prevent_default();
        let Some(src) = ctx_drag.dragging.read().clone() else {
            return;
        };
        *ctx_drag.dragging.write_unchecked() = None;
        *ctx_drag.drop_target.write_unchecked() = None;
        do_move_items(ctx_drag, ws_drag, vec![src], String::new());
    };
    let container_dragend = move |_ev: DragEvent| {
        *ctx_drag.dragging.write_unchecked() = None;
        *ctx_drag.drop_target.write_unchecked() = None;
    };

    // Cmd/Ctrl+A selects all visible nodes; Escape clears the selection.
    let ctx_key = ctx;
    let tree_keydown = move |ev: KeyboardEvent| {
        let web = ev.data().as_web_event();
        if (web.meta_key() || web.ctrl_key()) && web.key().eq_ignore_ascii_case("a") {
            web.prevent_default();
            let nodes_snapshot = ctx_key.nodes.read().clone();
            let expanded_snapshot = ctx_key.expanded.read().clone();
            *ctx_key.selected.write_unchecked() = visible_paths(&nodes_snapshot, &expanded_snapshot);
        } else if web.key() == "Escape" {
            *ctx_key.selected.write_unchecked() = Vec::new();
            *ctx_key.menu.write_unchecked() = None;
            *ctx_key.renaming.write_unchecked() = None;
        }
    };

    // ── Render ─────────────────────────────────────────────────────

    rsx! {
        div {
            class: "file-tree flex flex-col h-full min-h-0",
            tabindex: "0",
            onkeydown: tree_keydown,
            ondragover: container_dragover,
            ondrop: container_drop,
            ondragend: container_dragend,

            // ── Header ──
            div { class: "file-tree-header flex items-center justify-between px-2 pt-1.5 pb-1",
                span { class: "text-xs font-semibold uppercase tracking-wider text-gray-500", "Notes" }
                button {
                    class: "btn btn-sm btn-ghost",
                    title: "New note",
                    "aria-label": "New note",
                    onclick: move |_| {
                        *ctx.creating.write_unchecked() = Some((String::new(), false));
                        *ctx.create_value.write_unchecked() = String::new();
                    },
                    "+ Note",
                }
                button {
                    class: "btn btn-sm btn-ghost",
                    title: "New folder",
                    "aria-label": "New folder",
                    onclick: move |_| {
                        *ctx.creating.write_unchecked() = Some((String::new(), true));
                        *ctx.create_value.write_unchecked() = String::new();
                    },
                    "+ Folder",
                }
            }

            // ── Inline creation input ──
            {
                let creating = (*ctx.creating.read()).clone();
                if creating.is_some() {
                    let is_folder = creating.as_ref().unwrap().1;
                    let ctx_c = ctx;
                    let ws_c = workspace;
                    rsx! {
                        div {
                            class: "px-2 py-1 flex items-center gap-1",
                            input {
                                class: "input flex-1 text-xs py-0",
                                placeholder: if is_folder { "Folder name…" } else { "Note name…" },
                                value: "{ctx_c.create_value.read()}",
                                oninput: move |ev: FormEvent| {
                                    *ctx_c.create_value.write_unchecked() = ev.value();
                                },
                                onkeydown: move |ev: KeyboardEvent| {
                                    let web = ev.data().as_web_event();
                                    if web.key() == "Enter" {
                                        do_create(ctx_c, ws_c);
                                    } else if web.key() == "Escape" {
                                        *ctx_c.creating.write_unchecked() = None;
                                        *ctx_c.create_value.write_unchecked() = String::new();
                                    }
                                },
                                onclick: move |ev: MouseEvent| {
                                    ev.data().as_web_event().stop_propagation();
                                },
                                ondblclick: move |ev: MouseEvent| {
                                    ev.data().as_web_event().stop_propagation();
                                },
                            }
                        }
                    }
                } else {
                    rsx! {}
                }
            }

            // ── Drop-into-root hint while dragging ──
            {
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
            }

            // ── Tree body ──
            {
                if !*tree_loaded.read() {
                    rsx! {
                        SkeletonList { rows: 6 }
                    }
                } else if ctx.nodes.read().is_empty() {
                    rsx! {
                        EmptyState {
                            icon: Some(Icon::FolderOpen),
                            title: "Your vault is empty".to_string(),
                            description: Some("Create your first note to start building your knowledge base.".to_string()),
                        }
                        button {
                            r#type: "button",
                            class: "btn btn-sm mt-2 mx-2",
                            onclick: move |_| {
                                *ctx.creating.write_unchecked() = Some((String::new(), false));
                                *ctx.create_value.write_unchecked() = String::new();
                            },
                            {render_icon_view(Icon::Plus)}
                            " New Note",
                        }
                    }
                } else {
                    let nodes_snapshot: Vec<TreeNode> =
                        ctx.nodes.read().iter().cloned().collect();
                    rsx! {
                        ul {
                            class: "tree-list flex-1 overflow-y-auto min-h-0 py-1",
                            for node in nodes_snapshot {
                                TreeNodeView { node: node }
                            }
                        }
                    }
                }
            }

            // ── Batch action bar ──
            {
                let count = ctx.selected.read().len();
                if count >= 2 {
                    let toasts_b = ctx.toasts;
                    let ws_b = workspace;
                    let ctx_b = ctx;
                    rsx! {
                        div {
                            class: "batch-bar flex items-center gap-1 px-2 py-1 border-t border-gray-700 bg-gray-900",
                            span { class: "text-xs text-gray-300 font-medium mr-1", "{count} selected" }
                            button { class: "btn btn-sm", onclick: move |_| {
                                let sel: Vec<String> = ctx_b.selected.read().iter().cloned()
                                    .filter(|p| p.ends_with(".md")).collect();
                                for p in &sel {
                                    open_tab(ws_b, p);
                                }
                            }, "Open" }
                            button { class: "btn btn-sm", onclick: move |_| {
                                *ctx_b.move_picker.write_unchecked() = Some(ctx_b.selected.read().clone());
                            }, "Move to…" }
                            button { class: "btn btn-sm", onclick: move |_| {
                                let text = ctx_b.selected.read().join("\n");
                                copy_text(text, "Copy Paths".to_string(), toasts_b);
                            }, "Copy Paths" }
                            button { class: "btn btn-sm btn-danger", onclick: move |_| {
                                *ctx_b.confirm_targets.write_unchecked() = ctx_b.selected.read().clone();
                                *ctx_b.confirm_open.write_unchecked() = true;
                            }, "Delete" }
                            button { class: "btn btn-sm btn-ghost", "aria-label": "Clear selection", onclick: move |_| {
                                *ctx_b.selected.write_unchecked() = Vec::new();
                            } }
                            {render_icon_view(Icon::X)}
                        }
                    }
                } else {
                    rsx! {}
                }
            }

            // ── Move-to folder picker ──
            {
                let picker = (*ctx.move_picker.read()).clone();
                if let Some(items) = picker {
                    let items_count = items.len();
                    let ctx_p = ctx;
                    let ws_p = workspace;
                    let nodes_for_folders = ctx.nodes.read().clone();
                    let mut folders = Vec::new();
                    collect_folders(&nodes_for_folders, 0, &mut folders);
                    let items_vr = items.clone();
                    rsx! {
                        div {
                            class: "dialog-overlay",
                            onclick: move |_| { *ctx_p.move_picker.write_unchecked() = None; },
                            div {
                                class: "menu move-picker",
                                onclick: move |ev: MouseEvent| {
                                    ev.data().as_web_event().stop_propagation();
                                },
                                div {
                                    class: "px-2 py-1 text-xs font-medium text-gray-400",
                                    "Move {items_count} item(s) to…",
                                }
                                MenuItem {
                                    icon: Some(Icon::Folder),
                                    label: "Vault root".to_string(),
                                    on_select: Callback::new(move |_| {
                                        *ctx_p.move_picker.write_unchecked() = None;
                                        do_move_items(ctx_p, ws_p, items_vr.clone(), String::new());
                                    }),
                                }
                                MenuSeparator {}
                                for (folder_path, depth) in &folders {
                                    {
                                        let fp = folder_path.clone();
                                        let items_f = items.clone();
                                        rsx! {
                                            MenuItem {
                                                key: "{fp}",
                                                icon: Some(Icon::Folder),
                                                label: format!("{}{}", "  ".repeat(*depth), fp),
                                                on_select: Callback::new(move |_| {
                                                    *ctx_p.move_picker.write_unchecked() = None;
                                                    do_move_items(ctx_p, ws_p, items_f.clone(), fp.clone());
                                                }),
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
            }

            // ── Confirmation dialog for batch delete ──
            {
                let n = ctx.confirm_targets.read().len();
                let confirm_msg = if n == 1 {
                    "This item will be moved to Trash. You can restore it or undo.".to_string()
                } else {
                    format!("These {n} items will be moved to Trash. You can restore them or undo.")
                };
                let ctx_confirm = ctx;
                let ws_confirm = workspace;
                let hist_confirm = history;
                let confirm_action = move |_| {
                    let targets = ctx_confirm.confirm_targets.read().clone();
                    do_delete(ctx_confirm, ws_confirm, hist_confirm, targets);
                };
                let confirm_action_cb: Option<EventHandler<()>> = Some(Callback::new(confirm_action));
                if *ctx.confirm_open.read() {
                    rsx! {
                        ConfirmDialog {
                            open: confirm_open,
                            title: "Move to Trash?".to_string(),
                            message: confirm_msg,
                            confirm_label: Some("Move to Trash"),
                            cancel_label: Some("Cancel"),
                            danger: true,
                            on_confirm: confirm_action_cb,
                        }
                    }
                } else {
                    rsx! {}
                }
            }

            // ── Context menu ──
            {
                let menu_state = (*ctx.menu.read()).clone();
                if let Some(m) = menu_state {
                    let path = m.path.clone();
                    let is_folder = m.is_folder;
                    let parent = if is_folder { path.clone() } else { parent_dir(&path) };
                    let ctx_m = ctx;
                    let ws_m = workspace;

                    // Rename
                    let path_rename = path.clone();
                    let rename_cb = Callback::new(move |_| {
                        *ctx_m.rename_value.write_unchecked() = display_name(&path_rename);
                        *ctx_m.renaming.write_unchecked() = Some(path_rename.clone());
                        *ctx_m.menu.write_unchecked() = None;
                    });

                    // New note
                    let parent_note = parent.clone();
                    let new_note_cb = Callback::new(move |_| {
                        *ctx_m.creating.write_unchecked() = Some((parent_note.clone(), false));
                        *ctx_m.create_value.write_unchecked() = String::new();
                        ctx_m.expanded.write_unchecked().insert(parent_note.clone());
                        *ctx_m.menu.write_unchecked() = None;
                    });

                    // New folder
                    let parent_folder = parent.clone();
                    let new_folder_cb = Callback::new(move |_| {
                        *ctx_m.creating.write_unchecked() = Some((parent_folder.clone(), true));
                        *ctx_m.create_value.write_unchecked() = String::new();
                        ctx_m.expanded.write_unchecked().insert(parent_folder.clone());
                        *ctx_m.menu.write_unchecked() = None;
                    });

                    // Delete
                    let path_del = path.clone();
                    let del_cb = Callback::new(move |_| {
                        *ctx_m.confirm_targets.write_unchecked() = vec![path_del.clone()];
                        *ctx_m.confirm_open.write_unchecked() = true;
                        *ctx_m.menu.write_unchecked() = None;
                    });

                    // Duplicate
                    let path_dup = path.clone();
                    let dup_cb = Callback::new(move |_| {
                        do_duplicate(ctx_m, ws_m, path_dup.clone());
                        *ctx_m.menu.write_unchecked() = None;
                    });

                    // Move to…
                    let path_mv = path.clone();
                    let mv_cb = Callback::new(move |_| {
                        *ctx_m.move_picker.write_unchecked() = Some(vec![path_mv.clone()]);
                        *ctx_m.menu.write_unchecked() = None;
                    });

                    // Archive
                    let path_arch = path.clone();
                    let arch_cb = Callback::new(move |_| {
                        do_archive(ctx_m, ws_m, path_arch.clone());
                        *ctx_m.menu.write_unchecked() = None;
                    });

                    // Copy path
                    let path_copy = path.clone();
                    let copy_path_cb = Callback::new(move |_| {
                        copy_text(path_copy.clone(), "Copy Path".to_string(), ctx_m.toasts);
                        *ctx_m.menu.write_unchecked() = None;
                    });

                    // Copy wikilink
                    let path_wiki = path.clone();
                    let copy_wiki_cb = Callback::new(move |_| {
                        let link = format!("[[{}]]", file_stem(&display_name(&path_wiki)));
                        copy_text(link, "Copy Wikilink".to_string(), ctx_m.toasts);
                        *ctx_m.menu.write_unchecked() = None;
                    });

                    // Reveal
                    let path_reveal = path.clone();
                    let reveal_cb = Callback::new(move |_| {
                        reveal_in_fm(path_reveal.clone());
                        *ctx_m.menu.write_unchecked() = None;
                    });

                    // Open in new window (future)
                    let new_window_cb = Callback::new(move |_| {
                        ctx_m.toasts.info(
                            "Coming soon",
                            "Opening a note in a new window is on the roadmap.",
                        );
                        *ctx_m.menu.write_unchecked() = None;
                    });

                    rsx! {
                        div {
                            class: "fixed inset-0 z-40",
                            onclick: move |_| { *ctx_m.menu.write_unchecked() = None; },
                            oncontextmenu: move |ev: MouseEvent| {
                                ev.data().as_web_event().prevent_default();
                                *ctx_m.menu.write_unchecked() = None;
                            },
                        }
                        div {
                            class: "menu fixed z-50",
                            role: "menu",
                            style: "left: {m.x}px; top: {m.y}px;",
                            onclick: move |_| { *ctx_m.menu.write_unchecked() = None; },
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
            }
        }
    }
}

// ── Recursive tree node ─────────────────────────────────────────────────

/// One recursive tree row.
#[component]
fn TreeNodeView(node: TreeNode) -> Element {
    let ctx: TreeContext = use_context();
    let workspace = use_workspace();
    let path = node.path.clone();
    let name = node.name.clone();
    let is_folder = node.is_folder;
    let children = node.children.clone();

    // Ref to the inline-rename input element — captured via `onmounted`.
    let input_ref: Rc<std::cell::RefCell<Option<web_sys::HtmlInputElement>>> =
        use_hook(|| Rc::new(std::cell::RefCell::new(None)));

    // Single / Cmd+Click / Shift+Click selection + open / expand.
    let ctx_click = ctx;
    let ws_click = workspace;
    let path_click = path.clone();
    let on_row_click = move |ev: MouseEvent| {
        let web = ev.data().as_web_event();
        if web.meta_key() || web.ctrl_key() {
            let mut sel = ctx_click.selected.read().clone();
            if sel.contains(&path_click) {
                sel.retain(|p| *p != path_click);
            } else {
                sel.push(path_click.clone());
            }
            *ctx_click.selected.write_unchecked() = sel;
            *ctx_click.anchor.write_unchecked() = Some(path_click.clone());
            return;
        }
        if web.shift_key() {
            let nodes_snapshot = ctx_click.nodes.read().clone();
            let expanded_snapshot = ctx_click.expanded.read().clone();
            let flat = visible_paths(&nodes_snapshot, &expanded_snapshot);
            if let Some(a) = ctx_click.anchor.read().clone() {
                if let (Some(ai), Some(bi)) = (
                    flat.iter().position(|p| *p == a),
                    flat.iter().position(|p| *p == path_click.as_str()),
                ) {
                    let (lo, hi) = if ai < bi { (ai, bi) } else { (bi, ai) };
                    *ctx_click.selected.write_unchecked() = flat[lo..=hi].to_vec();
                    return;
                }
            }
            *ctx_click.selected.write_unchecked() = vec![path_click.clone()];
            *ctx_click.anchor.write_unchecked() = Some(path_click.clone());
            return;
        }
        *ctx_click.selected.write_unchecked() = vec![path_click.clone()];
        *ctx_click.anchor.write_unchecked() = Some(path_click.clone());
        if is_folder {
            {
                let mut set = ctx_click.expanded.write_unchecked();
                if set.contains(&path_click) {
                    set.remove(&path_click);
                } else {
                    set.insert(path_click.clone());
                }
            }
        } else {
            open_tab(ws_click, &path_click);
        }
    };

    // Double-click → inline rename (Finder / VS Code behaviour).
    let path_dbl = path.clone();
    let on_row_dblclick = move |ev: MouseEvent| {
        let web = ev.data().as_web_event();
        web.stop_propagation();
        if ctx.renaming.read().is_none() {
            *ctx.rename_value.write_unchecked() = display_name(&path_dbl);
            *ctx.renaming.write_unchecked() = Some(path_dbl.clone());
        }
    };

    // Right-click → select + open the tree context menu at the cursor.
    let path_menu = path.clone();
    let on_contextmenu = move |ev: MouseEvent| {
        let web = ev.data().as_web_event();
        web.prevent_default();
        web.stop_propagation();
        if !ctx.selected.read().contains(&path_menu) {
            *ctx.selected.write_unchecked() = vec![path_menu.clone()];
            *ctx.anchor.write_unchecked() = Some(path_menu.clone());
        }
        *ctx.menu.write_unchecked() = Some(MenuState {
            x: web.client_x() as f64,
            y: web.client_y() as f64,
            path: path_menu.clone(),
            is_folder,
        });
    };

    // ── Drag source ──
    let path_drag = path.clone();
    let on_dragstart = move |ev: DragEvent| {
        let web = ev.data().as_web_event();
        *ctx.dragging.write_unchecked() = Some(path_drag.clone());
        if let Some(dt) = web.data_transfer() {
            let _ = dt.set_data("application/x-nabu-note", &path_drag);
            let _ = dt.set_data("text/plain", &path_drag);
            let _ = dt.effect_allowed();
        }
    };
    let on_dragend = move |_ev: DragEvent| {
        *ctx.dragging.write_unchecked() = None;
        *ctx.drop_target.write_unchecked() = None;
    };

    // ── Drop target: folders accept drops into themselves; notes into their
    // parent folder. ──
    let path_over = path.clone();
    let on_dragover = move |ev: DragEvent| {
        if ctx.dragging.read().is_none() {
            return;
        }
        let web = ev.data().as_web_event();
        web.prevent_default();
        web.stop_propagation();
        let dest = if is_folder {
            path_over.clone()
        } else {
            parent_dir(&path_over)
        };
        *ctx.drop_target.write_unchecked() = Some(dest);
    };
    let path_drop = path.clone();
    let on_drop = move |ev: DragEvent| {
        let web = ev.data().as_web_event();
        web.prevent_default();
        web.stop_propagation();
        let Some(src) = ctx.dragging.read().clone() else {
            return;
        };
        *ctx.dragging.write_unchecked() = None;
        *ctx.drop_target.write_unchecked() = None;
        let dest = if is_folder {
            path_drop.clone()
        } else {
            parent_dir(&path_drop)
        };
        do_move_items(ctx, ws_click, vec![src], dest);
    };

    // Focus the inline rename input when this row enters rename mode.
    let path_focus = path.clone();
    let input_ref_focus = input_ref.clone();
    use_effect(move || {
        if ctx.renaming.read().as_deref() == Some(path_focus.as_str()) {
            let input_ref_f = input_ref_focus.clone();
            set_timeout(
                move || {
                    if let Some(el) = input_ref_f.borrow().as_ref() {
                        let _ = el.focus();
                        let _ = el.select();
                    }
                },
                10,
            );
        }
    });

    // Row classes (reactive: re-renders when signals change).
    let selected_now = ctx.selected.read().contains(&path);
    let is_dragging_now = ctx.dragging.read().as_deref() == Some(&path.as_str());
    let is_drop_now = ctx.drop_target.read().as_deref() == Some(&path.as_str());
    let row_class = format!(
        "tree-row flex items-center gap-1 px-1 py-0.5 mx-1 rounded cursor-pointer select-none text-sm{}{}{}",
        if selected_now { " tree-row-selected" } else { "" },
        if is_dragging_now { " tree-row-dragging" } else { "" },
        if is_drop_now { " tree-row-drop-target" } else { "" },
    );

    let chevron_state = if is_folder && ctx.expanded.read().contains(&path) {
        "open"
    } else {
        "closed"
    };
    let has_children = is_folder && !children.is_empty();
    let aria_expanded = if is_folder {
        if chevron_state == "open" { "true" } else { "false" }
    } else {
        ""
    };

    rsx! {
        li {
            class: "tree-node",
            draggable: "true",
            ondragstart: on_dragstart,
            ondragend: on_dragend,
            ondragover: on_dragover,
            ondrop: on_drop,

            div {
                class: "{row_class}",
                onclick: on_row_click,
                ondblclick: on_row_dblclick,
                oncontextmenu: on_contextmenu,
                role: "treeitem",
                "aria-selected": if selected_now { "true" } else { "false" },
                "aria-expanded": aria_expanded,

                // Chevron
                span {
                    class: "tree-chevron w-4 text-center text-xs text-gray-500",
                    "aria-hidden": "true",
                }
                if is_folder {
                    if chevron_state == "open" {
                        {render_icon_view(Icon::ChevronDown)}
                    } else {
                        {render_icon_view(Icon::ChevronRight)}
                    }
                } else {
                    span { "•" }
                }

                // Icon
                span { class: "tree-icon", "aria-hidden": "true" }
                {render_icon_view(if is_folder { Icon::Folder } else { Icon::FileText })}

                // Label or rename input
                {
                    let renaming = ctx.renaming.read().as_deref() == Some(path.as_str());
                    if renaming {
                        let ctx_r = ctx;
                        let ws_r = workspace;
                        let input_ref_r = input_ref.clone();
                        rsx! {
                            input {
                                class: "input flex-1 text-xs py-0",
                                onmounted: move |ev: MountedEvent| {
                                    let web = ev.data().as_web_event();
                                    if let Ok(input) =
                                        web.dyn_into::<web_sys::HtmlInputElement>()
                                    {
                                        *input_ref_r.borrow_mut() = Some(input);
                                    }
                                },
                                value: "{ctx_r.rename_value.read()}",
                                oninput: move |ev: FormEvent| {
                                    *ctx_r.rename_value.write_unchecked() = ev.value();
                                },
                                onkeydown: move |ev: KeyboardEvent| {
                                    let web = ev.data().as_web_event();
                                    if web.key() == "Enter" {
                                        do_rename(ctx_r, ws_r, path.clone(), is_folder);
                                    } else if web.key() == "Escape" {
                                        *ctx_r.renaming.write_unchecked() = None;
                                    }
                                },
                                onclick: move |ev: MouseEvent| {
                                    ev.data().as_web_event().stop_propagation();
                                },
                                ondblclick: move |ev: MouseEvent| {
                                    ev.data().as_web_event().stop_propagation();
                                },
                            }
                        }
                    } else {
                        rsx! {
                            span { class: "tree-name truncate flex-1 min-w-0", title: "{path}", "{name}" }
                        }
                    }
                }
            }

            // Children (recursive)
            {
                if is_folder && has_children && chevron_state == "open" {
                    rsx! {
                        ul {
                            class: "tree-children ml-3 border-l border-gray-800",
                            for child in &children {
                                TreeNodeView { node: child.clone() }
                            }
                        }
                    }
                } else {
                    rsx! {}
                }
            }
        }
    }
}
