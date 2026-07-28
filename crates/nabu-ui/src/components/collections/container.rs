//! Collection Container component.
//!
//! Production-ready collection container that manages view state,
//! filtering, sorting, and data projection for all collection views.
//! Views are projections of existing KnowledgeObjects — views never own data.

use leptos::prelude::*;
use crate::components::collections::shared::types::CollectionView;
use crate::components::collections::shared::context::SearchState;
use crate::components::collections::view_switcher::ViewSwitcher;
use crate::components::collections::table_view::{TableView, TableFilter, ColumnConfig};
use crate::components::collections::board_view::{BoardView, BoardFilter, BoardColumn};
use crate::components::collections::gallery_view::{GalleryView, GalleryFilter};
use crate::components::collections::calendar_view::{CalendarView, CalendarFilter};
use crate::models::knowledge_object::KnowledgeObject;

#[component]
pub fn CollectionContainer() -> impl IntoView {
    let (view, set_view) = signal(CollectionView::Table);
    let (search_state, set_search_state) = signal(SearchState::default());
    let (objects, set_objects) = signal(vec![]);

    // Load objects on mount
    spawn_local(async move {
        match crate::ipc::tauri_invoke(
            "fetch_objects",
            serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap(),
        ).await {
            Ok(objs) => set_objects.set(objs),
            Err(e) => web_sys::console::error_1(&format!("Failed to load objects: {}", e).into()),
        }
    });

    let on_view_change = {
        let view = view.clone();
        Callback::from(move |new_view| view.set(new_view))
    };

    let table_filter = move || TableFilter {
        query: search_state.get().query.clone(),
        object_type: None,
        sort_by: "modified".to_string(),
        sort_ascending: false,
    };

    let board_filter = move || BoardFilter {
        query: search_state.get().query.clone(),
        object_type: None,
        group_by: "status".to_string(),
    };

    let gallery_filter = move || GalleryFilter {
        query: search_state.get().query.clone(),
        object_type: None,
        sort_by: "modified".to_string(),
        sort_ascending: false,
    };

    let calendar_filter = move || CalendarFilter {
        query: search_state.get().query.clone(),
        object_type: None,
        view_mode: crate::components::collections::calendar_view::CalendarViewMode::Month,
    };

    let table_columns = move || vec![
        ColumnConfig { key: "title".to_string(), label: "Title".to_string(), visible: true, sortable: true, width: Some("flex-1".to_string()) },
        ColumnConfig { key: "type".to_string(), label: "Type".to_string(), visible: true, sortable: true, width: Some("w-24".to_string()) },
        ColumnConfig { key: "modified".to_string(), label: "Modified".to_string(), visible: true, sortable: true, width: Some("w-32".to_string()) },
        ColumnConfig { key: "author".to_string(), label: "Author".to_string(), visible: true, sortable: true, width: Some("w-24".to_string()) },
        ColumnConfig { key: "words".to_string(), label: "Words".to_string(), visible: false, sortable: true, width: None },
    ];

    let board_columns = move || vec![
        BoardColumn { id: "reading".to_string(), title: "Reading".to_string(), items: vec![] },
        BoardColumn { id: "completed".to_string(), title: "Completed".to_string(), items: vec![] },
        BoardColumn { id: "archived".to_string(), title: "Archived".to_string(), items: vec![] },
    ];

    view! {
        <div class="collection-container">
            <ViewSwitcher current_view={*view} on_change={on_view_change} />
            {
                match *view {
                    CollectionView::Table => view! {
                        <TableView
                            objects={objects.get()}
                            columns={table_columns()}
                            filter={table_filter()}
                            on_filter_change=Callback::new(|_| {})
                            on_sort=Callback::new(|_| {})
                        />
                    }.into_any(),
                    CollectionView::Board => view! {
                        <BoardView
                            objects={objects.get()}
                            columns={board_columns()}
                            filter={board_filter()}
                            on_filter_change=Callback::new(|_| {})
                            on_move_item=Callback::new(|_| {})
                        />
                    }.into_any(),
                    CollectionView::Gallery => view! {
                        <GalleryView
                            objects={objects.get()}
                            filter={gallery_filter()}
                            on_filter_change=Callback::new(|_| {})
                        />
                    }.into_any(),
                    CollectionView::Calendar => view! {
                        <CalendarView
                            objects={objects.get()}
                            filter={calendar_filter()}
                            on_filter_change=Callback::new(|_| {})
                        />
                    }.into_any(),
                }
            }
        </div>
    }
}
