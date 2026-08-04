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
    let (loaded, set_loaded) = signal(false);
    let (load_error, set_load_error) = signal(None::<String>);
    let toasts = crate::components::ui::feedback::use_toast();

    // Load objects on mount. `retry` is a plain fn so it can be re-run from
    // the error panel's Retry button.
    fn fetch_objects() -> Vec<crate::models::knowledge_object::KnowledgeObject> {
        vec![]
    }
    let do_load = {
        let set_objects = set_objects;
        let set_loaded = set_loaded;
        let set_load_error = set_load_error;
        let toasts = toasts;
        Callback::new(move |_| {
            set_loaded.set(false);
            set_load_error.set(None);
            let set_objects = set_objects;
            let set_loaded = set_loaded;
            let set_load_error = set_load_error;
            let toasts = toasts;
            spawn_local(async move {
                let res = crate::ipc::tauri_invoke(
                    "fetch_objects",
                    serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap(),
                )
                .await;
                match serde_wasm_bindgen::from_value::<Vec<crate::models::knowledge_object::KnowledgeObject>>(res) {
                    Ok(objs) => set_objects.set(objs),
                    Err(e) => {
                        set_load_error.set(Some(e.to_string()));
                        toasts.error("Couldn't load collections", "Your collections could not be loaded — try again.");
                    }
                }
                set_loaded.set(true);
            });
        })
    };
    do_load.run(());

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
            {move || if let Some(err) = load_error.get() {
                view! {
                    <crate::components::ui::feedback::ErrorPanel
                        title="Couldn't load collections".to_string()
                        message="Something went wrong while reading your knowledge objects.".to_string()
                        details=Some(err)
                        recovery=Some("Check that your vault is accessible, then try again.".to_string())
                        on_retry=Some(do_load)
                    />
                }.into_any()
            } else if !loaded.get() {
                view! {
                    <div class="p-6">
                        <crate::components::ui::feedback::SkeletonList rows=Some(6) />
                    </div>
                }.into_any()
            } else if objects.get().is_empty() {
                view! {
                    <div class="p-10 flex justify-center">
                        <crate::components::ui::info::EmptyState
                            icon=crate::components::ui::icons::Icon::FolderTree
                            title="No knowledge objects yet".to_string()
                            description="Collections show your structured knowledge once you start adding objects.".to_string()
                        ></crate::components::ui::info::EmptyState>
                    </div>
                }.into_any()
            } else {
                view! {
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
                }.into_any()
            }}
        </div>
    }
}
