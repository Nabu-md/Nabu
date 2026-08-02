use crate::components::ui::button::{Button, ButtonSize};
use crate::components::ui::nav::{NavGroup, SidebarItem};
use wasm_bindgen_futures::spawn_local;

use leptos::prelude::*;

#[component]
pub fn LeftSidebar() -> impl IntoView {
    view! {
        <div class="w-64 border-r border-gray-700 bg-gray-900 h-screen flex flex-col gap-2 p-2">
            <Button
                size=ButtonSize::Sm
                on_click=Callback::new(move |_| {
                    spawn_local(async move {
                        let _ = crate::ipc::tauri_invoke(
                            "note_create_file",
                            serde_wasm_bindgen::to_value(&serde_json::json!({"path": "new_note.md"})).unwrap(),
                        ).await;
                    });
                })
            >
                "+ Note"
            </Button>
            <div class="flex-1 overflow-y-auto">
                <NavGroup title="Notes".to_string()>
                    <SidebarItem label="My First Note.md".to_string() active=true icon="📄" />
                    <SidebarItem label="Another Note.md".to_string() icon="📄" />
                </NavGroup>
                <NavGroup title="Archive".to_string()>
                    <SidebarItem label="(empty)".to_string() icon="🗄️" />
                </NavGroup>
            </div>
        </div>
    }
}
