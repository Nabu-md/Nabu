use leptos::prelude::{component, view, ClassAttribute, ElementChild, CollectView, signal, View, IntoView, IntoAny, Get, OnAttribute, Update};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TreeNode {
    pub id: String,
    pub name: String,
    pub is_folder: bool,
    pub children: Vec<TreeNode>,
}

#[component]
pub fn FileTree(nodes: Vec<TreeNode>) -> impl IntoView {
    view! {
        <div class="file-tree">
            <ul>
                {nodes.into_iter().map(|node| {
                    view! { <TreeNodeView node=node /> }
                }).collect_view()}
            </ul>
        </div>
    }
}

#[component]
fn TreeNodeView(node: TreeNode) -> impl IntoView {
    let (expanded, set_expanded) = signal(false);
    
    view! {
        <li class="tree-node">
            <div on:click=move |_| set_expanded.update(|e| *e = !*e)>
                {move || if node.is_folder { if expanded.get() { "▼ " } else { "▶ " } } else { "  " }}
                {node.name.clone()}
            </div>
            {move || if node.is_folder && expanded.get() {
                view! {
                    <ul class="pl-4">
                        {node.children.clone().into_iter().map(|child| {
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
