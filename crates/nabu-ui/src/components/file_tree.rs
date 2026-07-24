use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;


use leptos::prelude::*;
use std::path::PathBuf;
use std::collections::HashSet;

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

#[component]
pub fn FileTree(nodes: Vec<TreeNode>, on_select: Callback<PathBuf>) -> impl IntoView {
    let active_file = RwSignal::new(None);
    let expanded_folders = RwSignal::new(HashSet::new());
    provide_context(FileTreeContext { active_file, expanded_folders });

    let (new_file_input, set_new_file_input) = signal(false);
    let (new_folder_input, set_new_folder_input) = signal(false);
    let (name_input, set_name_input) = signal("".to_string());

    view! {
        <div class="file-tree">
            <div class="actions">
                <button on:click=move |_| set_new_file_input.set(true)>"+ New File"</button>
                <button on:click=move |_| set_new_folder_input.set(true)>"+ New Folder"</button>
            </div>
            
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
    
    view! {
        <li class="tree-node">
            <div on:click={
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
            }>
                {let path = path.clone();
                 move || if is_folder { 
                    if context.expanded_folders.get().contains(&path) { "▼ " } else { "▶ " }
                } else { "  " }}
                {name.clone()}
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
