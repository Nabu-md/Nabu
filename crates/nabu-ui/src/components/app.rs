use crate::components::graph_view::{GraphMode, GraphView};
use crate::components::inbox::Inbox;
use crate::components::layout::left_sidebar::LeftSidebar;
use crate::components::layout::ribbon_bar::RibbonBar;
use crate::components::layout::right_inspector::RightInspector;
use crate::components::layout::tab_bar::TabBar;
use crate::components::note_editor::NoteEditor;
use crate::components::reading_queue::ReadingQueue;
use crate::components::settings::settings_panel::SettingsPanel;
use crate::components::template_editor::TemplateEditor;
use crate::components::trash::Trash;
use crate::components::vault_setup_wizard::VaultSetupWizard;
use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AppScreen {
    Loading,
    VaultSetup,
    MainDashboard,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ViewMode {
    Editor,
    Graph,
    Inbox,
    ReadingQueue,
    Templates,
    Settings,
    Trash,
}

#[component]
pub fn App() -> impl IntoView {
    crate::provide_theme("dark".to_string());
    crate::history::provide_history();

    let history = crate::history::use_history();
    let toasts = crate::components::ui::feedback::use_toast();

    let (screen, set_screen) = signal(AppScreen::Loading);
    let (_vault_path, set_vault_path) = signal(String::new());
    let (view_mode, set_view_mode) = signal(ViewMode::Editor);
    let (show_left_sidebar, set_show_left_sidebar) = signal(true);
    let (show_right_inspector, set_show_right_inspector) = signal(true);
    let (initial_content, _set_initial_content) = signal(
        "# Welcome to Nabu\n\nA powerful markdown note-taking app with graph visualization and AI dictation.\n\n- [[Graph View]]\n- [[Settings]]\n- Task: - [ ] Explore features".to_string()
    );

    // On mount, check if a vault already exists via Tauri IPC
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("check_vault_exists", empty_args).await;
        if let Ok(path_val) = serde_wasm_bindgen::from_value::<Option<String>>(result) {
            if let Some(path) = path_val {
                if !path.trim().is_empty() {
                    set_vault_path.set(path);
                    set_screen.set(AppScreen::MainDashboard);
                    return;
                }
            }
        }
        set_screen.set(AppScreen::VaultSetup);
    });

    let handle_vault_selected = move |path: String| {
        set_vault_path.set(path);
        set_screen.set(AppScreen::MainDashboard);
    };

    // RibbonBar expects optional Arc<dyn Fn> callbacks over signals.
    let on_view_mode_change: std::sync::Arc<dyn Fn(ViewMode) + Send + Sync + 'static> =
        std::sync::Arc::new(move |mode: ViewMode| set_view_mode.set(mode));
    let on_show_sidebar_change: std::sync::Arc<dyn Fn(bool) + Send + Sync + 'static> =
        std::sync::Arc::new(move |show: bool| set_show_left_sidebar.set(show));

    view! {
        // Loading screen
        {move || if screen.get() == AppScreen::Loading {
            (view! {
                <div class="flex h-screen w-screen items-center justify-center bg-gray-950 text-gray-100">
                    <div class="flex items-center space-x-2 text-blue-400">
                        <div class="w-6 h-6 border-2 border-blue-400 border-t-transparent rounded-full animate-spin"></div>
                        <span class="text-sm">"Loading..."</span>
                    </div>
                </div>
            }).into_any()
        } else {
            view! {}.into_any()
        }}

        // Vault Setup Wizard (only shown when no vault exists)
        {move || if screen.get() == AppScreen::VaultSetup {
            (view! {
                <VaultSetupWizard on_vault_selected=handle_vault_selected />
            }).into_any()
        } else {
            view! {}.into_any()
        }}

        // Main Dashboard (only shown when vault is configured)
        {move || if screen.get() == AppScreen::MainDashboard {
            (view! {
                <div class="app flex h-screen w-screen bg-gray-950 text-gray-100 overflow-hidden font-sans select-none">
                    // Left Ribbon Bar
                    <div class="flex-none">
                        <RibbonBar
                            set_view_mode=on_view_mode_change.clone()
                            set_show_sidebar=on_show_sidebar_change.clone()
                        />
                    </div>

                    // Left Sidebar (Vault File Explorer)
                    {move || if show_left_sidebar.get() {
                        view! {
                            <div class="flex-none">
                                <LeftSidebar />
                            </div>
                        }.into_any()
                    } else {
                        view! {}.into_any()
                    }}

                    // Main Content Area
                    <div class="flex-1 flex flex-col h-screen overflow-hidden bg-gray-900">
                        // Top Tab Bar
                        <div class="flex-none">
                            <TabBar />
                        </div>

                        // View Mode Switcher / Navigation Controls
                        <div class="flex items-center px-4 py-1.5 bg-gray-800/60 border-b border-gray-700/50 text-xs space-x-2">
                            <button
                                class=move || format!("px-2.5 py-1 rounded transition-colors {}", if view_mode.get() == ViewMode::Editor { "bg-blue-600 text-white font-medium" } else { "text-gray-400 hover:text-gray-200 hover:bg-gray-700/50" })
                                on:click=move |_| set_view_mode.set(ViewMode::Editor)
                            >
                                "📝 Editor"
                            </button>
                            <button
                                class=move || format!("px-2.5 py-1 rounded transition-colors {}", if view_mode.get() == ViewMode::Graph { "bg-blue-600 text-white font-medium" } else { "text-gray-400 hover:text-gray-200 hover:bg-gray-700/50" })
                                on:click=move |_| set_view_mode.set(ViewMode::Graph)
                            >
                                "🕸️ Graph"
                            </button>
                            <button
                                class=move || format!("px-2.5 py-1 rounded transition-colors {}", if view_mode.get() == ViewMode::Settings { "bg-blue-600 text-white font-medium" } else { "text-gray-400 hover:text-gray-200 hover:bg-gray-700/50" })
                                on:click=move |_| set_view_mode.set(ViewMode::Settings)
                            >
                                "⚙️ Settings"
                            </button>
                            <button
                                class=move || format!("px-2.5 py-1 rounded transition-colors {}", if view_mode.get() == ViewMode::ReadingQueue { "bg-blue-600 text-white font-medium" } else { "text-gray-400 hover:text-gray-200 hover:bg-gray-700/50" })
                                on:click=move |_| set_view_mode.set(ViewMode::ReadingQueue)
                            >
                                "📚 Reading Queue"
                            </button>
                            <button
                                class=move || format!("px-2.5 py-1 rounded transition-colors {}", if view_mode.get() == ViewMode::Templates { "bg-blue-600 text-white font-medium" } else { "text-gray-400 hover:text-gray-200 hover:bg-gray-700/50" })
                                on:click=move |_| set_view_mode.set(ViewMode::Templates)
                            >
                                "📋 Templates"
                            </button>
                            <button
                                class=move || format!("px-2.5 py-1 rounded transition-colors {}", if view_mode.get() == ViewMode::Trash { "bg-blue-600 text-white font-medium" } else { "text-gray-400 hover:text-gray-200 hover:bg-gray-700/50" })
                                on:click=move |_| set_view_mode.set(ViewMode::Trash)
                                title="Trash (deleted items)"
                            >
                                "🗑️ Trash"
                            </button>

                            <div class="flex-1"></div>

                            <button
                                class=move || format!("px-2 py-1 rounded transition-colors {}", if history.can_undo.get() { "text-gray-200 hover:bg-gray-700/50" } else { "text-gray-600 cursor-default" })
                                on:click=move |_| crate::history::undo(history, toasts)
                                title="Undo (Cmd/Ctrl+Z)"
                                aria-label="Undo"
                                disabled=move || !history.can_undo.get()
                            >
                                "↶"
                            </button>
                            <button
                                class=move || format!("px-2 py-1 rounded transition-colors {}", if history.can_redo.get() { "text-gray-200 hover:bg-gray-700/50" } else { "text-gray-600 cursor-default" })
                                on:click=move |_| crate::history::redo(history, toasts)
                                title="Redo (Cmd/Ctrl+Shift+Z or Ctrl+Y)"
                                aria-label="Redo"
                                disabled=move || !history.can_redo.get()
                            >
                                "↷"
                            </button>

                            <button
                                class="px-2 py-1 text-gray-400 hover:text-gray-200 rounded hover:bg-gray-700/50"
                                on:click=move |_| set_show_left_sidebar.update(|v| *v = !*v)
                                title="Toggle Left Sidebar"
                            >
                                "📁"
                            </button>
                            <button
                                class="px-2 py-1 text-gray-400 hover:text-gray-200 rounded hover:bg-gray-700/50"
                                on:click=move |_| set_show_right_inspector.update(|v| *v = !*v)
                                title="Toggle Right Inspector"
                            >
                                "📋"
                            </button>
                        </div>

                        // Main View Container
                        <div class="flex-1 overflow-auto p-4">
                            {move || match view_mode.get() {
                                ViewMode::Editor => view! {
                                    <div class="max-w-4xl mx-auto h-full">
                                        <NoteEditor initial_content=initial_content.get() />
                                    </div>
                                }.into_any(),
                                ViewMode::Graph => view! {
                                    <div class="w-full h-full flex items-center justify-center">
                                        <GraphView _mode=GraphMode::Default />
                                    </div>
                                }.into_any(),
                                ViewMode::Settings => view! {
                                    <div class="max-w-4xl mx-auto h-full">
                                        <SettingsPanel />
                                    </div>
                                }.into_any(),
                                ViewMode::Inbox => view! {
                                    <div class="max-w-7xl mx-auto h-full">
                                        <Inbox />
                                    </div>
                                }.into_any(),
                                ViewMode::ReadingQueue => view! {
                                    <div class="max-w-7xl mx-auto h-full">
                                        <ReadingQueue />
                                    </div>
                                }.into_any(),
                                ViewMode::Templates => view! {
                                    <div class="max-w-7xl mx-auto h-full">
                                        <TemplateEditor
                                            templates=vec![]
                                            folder_templates=vec![]
                                            on_save=Callback::new(|_| {})
                                            on_delete=Callback::new(|_| {})
                                            on_assign=Callback::new(|_| {})
                                            on_unassign=Callback::new(|_| {})
                                        />
                                    </div>
                                }.into_any(),
                                ViewMode::Trash => view! {
                                    <div class="max-w-7xl mx-auto h-full">
                                        <Trash />
                                    </div>
                                }.into_any(),
                            }}
                        </div>
                    </div>

                    // Right Inspector Sidebar
                    {move || if show_right_inspector.get() {
                        view! {
                            <div class="flex-none">
                                <RightInspector />
                            </div>
                        }.into_any()
                    } else {
                        view! {}.into_any()
                    }}
                </div>
            }).into_any()
        } else {
            view! {}.into_any()
        }}
    }
}
