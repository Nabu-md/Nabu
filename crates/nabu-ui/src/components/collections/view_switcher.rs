use yew::prelude::*;
use crate::components::collections::shared::types::CollectionView;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub current_view: CollectionView,
    pub on_change: Callback<CollectionView>,
}

#[function_component(ViewSwitcher)]
pub fn view_switcher(props: &Props) -> Html {
    let views = [
        CollectionView::Table,
        CollectionView::Board,
        CollectionView::Gallery,
        CollectionView::Calendar,
    ];

    html! {
        <div class="view-switcher">
            { for views.iter().map(|view| {
                let view = *view;
                let on_click = props.on_change.reform(move |_| view);
                let class = if props.current_view == view { "active" } else { "" };
                html! {
                    <button class={class} onclick={on_click}>
                        { format!("{:?}", view) }
                    </button>
                }
            })}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_view_switcher_renders() {
        // Just a basic test to ensure it compiles
    }
}
