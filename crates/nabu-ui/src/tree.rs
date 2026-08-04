use crate::components::ui::icons::{render_icon_view, Icon};
use leptos::prelude::*;
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
                }).collect::<Vec<_>>()}
            </ul>
        </div>
    }
}

#[component]
pub fn TreeNodeView(node: TreeNode) -> impl IntoView {
    let (expanded, set_expanded) = signal(false);

    view! {
        <li class="tree-node">
            <div on:click=move |_| set_expanded.update(|e| *e = !*e)>
                {move || if node.is_folder { if expanded.get() { render_icon_view(Icon::ChevronDown) } else { render_icon_view(Icon::ChevronRight) } } else { view! {}.into_any() }}
                {node.name.clone()}
            </div>
            {move || if node.is_folder && expanded.get() {
                view! {
                    <ul class="pl-4">
                        {node.children.clone().into_iter().map(|child| {
                            view! { <TreeNodeView node=child /> }
                        }).collect::<Vec<_>>()}
                    </ul>
                }.into_any()
            } else {
                view! {}.into_any()
            }}
        </li>
    }
}
