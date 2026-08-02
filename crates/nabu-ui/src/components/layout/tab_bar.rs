use crate::components::ui::button::{Button, ButtonSize, ButtonVariant};
use crate::components::ui::nav::{TabDef, Tabs};
use leptos::prelude::*;

#[component]
pub fn TabBar() -> impl IntoView {
    let active = RwSignal::new("note-1".to_string());
    let tabs = vec![
        TabDef::new("note-1", "Note 1"),
        TabDef::new("note-2", "Note 2"),
    ];
    view! {
        <div class="flex border-b border-gray-700 bg-gray-900 h-9 items-center px-1">
            <Tabs tabs=tabs active=active />
            <Button
                variant=ButtonVariant::Ghost
                size=ButtonSize::Sm
                title="New note"
                aria_label="New note"
            >
                "+"
            </Button>
        </div>
    }
}
