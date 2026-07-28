use yew::prelude::*;
use crate::components::collections::shared::types::CollectionView;
use crate::components::collections::shared::context::SearchState;
use crate::components::collections::view_switcher::ViewSwitcher;
use crate::components::collections::table_view::TableView;
use crate::components::collections::board_view::BoardView;
use crate::components::collections::gallery_view::GalleryView;
use crate::components::collections::calendar_view::CalendarView;

#[function_component(CollectionContainer)]
pub fn collection_container() -> Html {
    let view = use_state(|| CollectionView::Table);
    let search_state = use_state(SearchState::default);

    let on_view_change = {
        let view = view.clone();
        Callback::from(move |new_view| view.set(new_view))
    };

    html! {
        <div class="collection-container">
            <ViewSwitcher current_view={*view} on_change={on_view_change} />
            {
                match *view {
                    CollectionView::Table => html! { <TableView /> },
                    CollectionView::Board => html! { <BoardView /> },
                    CollectionView::Gallery => html! { <GalleryView /> },
                    CollectionView::Calendar => html! { <CalendarView /> },
                }
            }
        </div>
    }
}
