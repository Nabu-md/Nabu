//! Calendar View component.
//!
//! Production-ready calendar view with date-based grouping,
//! filtering, and sorting.
//! Views are projections of existing KnowledgeObjects — views never own data.

use leptos::prelude::*;
use crate::models::knowledge_object::KnowledgeObject;

#[derive(Clone, PartialEq, Default)]
pub struct CalendarFilter {
    pub query: String,
    pub object_type: Option<String>,
    pub view_mode: CalendarViewMode,
}

#[derive(Clone, PartialEq, Default)]
pub enum CalendarViewMode {
    #[default]
    Month,
    Week,
    Day,
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub objects: Vec<KnowledgeObject>,
    pub filter: CalendarFilter,
    pub on_filter_change: Callback<CalendarFilter>,
}

#[function_component(CalendarView)]
pub fn calendar_view(props: &Props) -> Html {
    let filtered = move || {
        let f = &props.filter;
        let mut result = props.objects.clone();

        if !f.query.is_empty() {
            let q = f.query.to_lowercase();
            result.retain(|obj| {
                obj.metadata.title.as_ref().map_or(false, |t| t.to_lowercase().contains(&q))
            });
        }

        if let Some(ref ot) = f.object_type {
            result.retain(|obj| obj.object_type.to_string() == *ot);
        }

        result
    };

    let grouped_by_date = move || {
        let items = filtered();
        let mut groups: std::collections::HashMap<String, Vec<KnowledgeObject>> = std::collections::HashMap::new();

        for obj in items {
            let date_key = obj.metadata.created.as_ref()
                .and_then(|d| d.get(0..10).map(|s| s.to_string()))
                .unwrap_or_else(|| "no-date".to_string());
            groups.entry(date_key).or_default().push(obj);
        }

        let mut sorted_keys: Vec<String> = groups.keys().cloned().collect();
        sorted_keys.sort();
        sorted_keys
    };

    view! {
        <div class="calendar-view p-4 overflow-y-auto h-full">
            // Calendar header
            <div class="flex items-center justify-between mb-4">
                <h2 class="text-lg font-semibold text-gray-200">"Calendar"</h2>
                <div class="flex gap-2">
                    <button
                        class=move || format!("px-3 py-1 text-xs rounded border {}",
                            if props.filter.get().view_mode == CalendarViewMode::Month { "bg-blue-700 border-blue-500 text-blue-100" } else { "border-gray-600 text-gray-400" })
                        on:click=move |_| props.on_filter_change.emit(CalendarFilter {
                            view_mode: CalendarViewMode::Month,
                            ..props.filter.get()
                        })
                    >
                        "Month"
                    </button>
                    <button
                        class=move || format!("px-3 py-1 text-xs rounded border {}",
                            if props.filter.get().view_mode == CalendarViewMode::Week { "bg-blue-700 border-blue-500 text-blue-100" } else { "border-gray-600 text-gray-400" })
                        on:click=move |_| props.on_filter_change.emit(CalendarFilter {
                            view_mode: CalendarViewMode::Week,
                            ..props.filter.get()
                        })
                    >
                        "Week"
                    </button>
                    <button
                        class=move || format!("px-3 py-1 text-xs rounded border {}",
                            if props.filter.get().view_mode == CalendarViewMode::Day { "bg-blue-700 border-blue-500 text-blue-100" } else { "border-gray-600 text-gray-400" })
                        on:click=move |_| props.on_filter_change.emit(CalendarFilter {
                            view_mode: CalendarViewMode::Day,
                            ..props.filter.get()
                        })
                    >
                        "Day"
                    </button>
                </div>
            </div>

            // Search and filter bar
            <div class="flex gap-3 mb-4">
                <input
                    type="text"
                    placeholder="Search..."
                    value={props.filter.get().query.clone()}
                    on:input=move |ev: InputEvent| {
                        let input: web_sys::HtmlInputElement = ev.target_unchecked_into();
                        let mut f = props.filter.get();
                        f.query = input.value();
                        props.on_filter_change.emit(f);
                    }
                    class="flex-1 bg-gray-800 text-gray-100 rounded px-3 py-1.5 text-sm border border-gray-700 focus:border-blue-500 focus:outline-none"
                />
            </div>

            // Calendar grid
            <div class="grid grid-cols-7 gap-1">
                // Day headers
                {for ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"].iter().map(|day| {
                    view! {
                        <div class="text-center text-xs text-gray-500 py-2 font-medium">{day}</div>
                    }
                })}

                // Date cells
                {move || {
                    let keys = grouped_by_date();
                    if keys.is_empty() {
                        view! {
                            <div class="col-span-full flex items-center justify-center h-64 text-gray-500">
                                "No items to display"
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            {for keys.iter().map(|date_key| {
                                let items = filtered().iter()
                                    .filter(|obj| {
                                        obj.metadata.created.as_ref()
                                            .map_or(false, |d| d.starts_with(date_key))
                                    })
                                    .count();
                                view! {
                                    <div class="col-span-1 bg-gray-800 rounded border border-gray-700 p-2 min-h-[80px]">
                                        <div class="text-xs text-gray-400 mb-1">{date_key}</div>
                                        <div class="text-xs text-gray-500">{format!("{} items", items)}</div>
                                        {move || {
                                            let day_items = filtered().iter()
                                                .filter(|obj| {
                                                    obj.metadata.created.as_ref()
                                                        .map_or(false, |d| d.starts_with(date_key))
                                                })
                                                .take(3)
                                                .collect::<Vec<_>>();
                                            if !day_items.is_empty() {
                                                view! {
                                                    <div class="mt-1 space-y-1">
                                                        {for day_items.iter().map(|obj| {
                                                            let title = obj.metadata.title.clone().unwrap_or_default();
                                                            view! {
                                                                <div class="text-xs text-blue-400 truncate" title={title.clone()}>
                                                                    {title}
                                                                </div>
                                                            }
                                                        })}
                                                    </div>
                                                }.into_any()
                                            } else {
                                                view! {}.into_any()
                                            }
                                        }}
                                    </div>
                                }
                            })}
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}
