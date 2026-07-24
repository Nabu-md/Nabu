use serde::{Deserialize, Serialize};
use leptos::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TreeNode {
    pub id: String,
    pub name: String,
    pub path: String,
    pub is_folder: bool,
    pub children: Vec<TreeNode>,
}

#[component]
pub fn TreeRenderer(nodes: Vec<TreeNode>) -> impl IntoView {
    view! {
        <ul>
            {nodes.into_iter().map(|node| {
                let is_folder = node.is_folder;
                let children = node.children;
                view! {
                    <li>
                        {node.name}
                        {if is_folder {
                            view! {
                                <TreeRenderer nodes=children />
                            }.into_view()
                        } else {
                            view! {}.into_view()
                        }}
                    </li>
                }
            }).collect_view()}
        </ul>
    }
}
