use leptos::prelude::*;
use crate::components::collections::shared::types::CollectionView;

#[derive(Props, PartialEq)]
pub struct Props {
    pub current_view: CollectionView,
    pub on_change: Callback<CollectionView>,
}

#[component]
pub fn ViewSwitcher(props: &Props) -> impl IntoView {
    let views = [
        CollectionView::Table,
        CollectionView::Board,
        CollectionView::Gallery,
        CollectionView::Calendar,
    ];

    view! {
        <div class="view-switcher flex items-center gap-1 p-2 bg-gray-800 border-b border-gray-700">
            {move || views.iter().map(move |view| {
                let view = *view;
                let class = if props.current_view == view {
                    "view-btn bg-blue-600 text-white"
                } else {
                    "view-btn text-gray-400 hover:text-gray-200"
                };
                let on_click = move |_| {
                    props.on_change.call(view);
                };
                view! {
                    <button class=class on_click=on_click>
                        {format!("{:?}", view)}
                    </button>
                }
            })}
        </div>
    }
}
