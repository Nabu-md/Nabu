use crate::components::graph_view::{GraphMode, GraphView};
use crate::components::inbox::Inbox;
use crate::components::layout::left_sidebar::LeftSidebar;
use crate::components::layout::ribbon_bar::RibbonBar;
use crate::components::layout::right_inspector::RightInspector;
use crate::components::layout::tab_bar::TabBar;
use crate::components::note_editor::NoteEditor;
use crate::components::reading_queue::ReadingQueue;
use crate::components::recovery::RecoveryBanner;
use crate::components::recovery::RecoveryManager;
use crate::components::recovery::SaveStatusIndicator;
use crate::components::recovery::VersionHistory;
use crate::components::settings::settings_panel::SettingsPanel;
use crate::components::template_editor::TemplateEditor;
use crate::components::trash::Trash;
use crate::components::vault_setup_wizard::VaultSetupWizard;
use crate::components::workspace::{open_tab, provide_workspace};
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
    History,
    Recovery,
}

/// Maps a persisted session view-mode string back to a [`ViewMode`].
fn parse_view_mode(mode: &str) -> ViewMode {
    match mode.to_lowercase().as_str() {
        "graph" => ViewMode::Graph,
        "inbox" => ViewMode::Inbox,
        "readingqueue" | "reading_queue" => ViewMode::ReadingQueue,
        "templates" => ViewMode::Templates,
        "settings" => ViewMode::Settings,
        "trash" => ViewMode::Trash,
        "history" => ViewMode::History,
        "recovery" => ViewMode::Recovery,
        _ => ViewMode::Editor,
    }
}

/// Renders the persisted-session form of the current workspace state.
fn session_state_from(
    view_mode: ViewMode,
    active_note: Option<String>,
    editor_cursor: u32,
    editor_scroll: u32,
    show_left_sidebar: bool,
    show_right_inspector: bool,
) -> crate::components::recovery::session::SessionState {
    use crate::components::recovery::session::SessionState;
    let saved_at = js_sys::Date::new_0()
        .to_iso_string()
        .as_string()
        .unwrap_or_default();
    SessionState {
        version: 1,
        saved_at: Some(saved_at),
        view_mode: Some(format!("{:?}", view_mode).to_lowercase()),
        active_note: active_note.clone(),
        open_tabs: active_note.into_iter().collect(),
        split_panes: vec![],
        cursor_pos: Some(editor_cursor),
        scroll_top: Some(editor_scroll),
        left_sidebar: Some(show_left_sidebar),
        right_inspector: Some(show_right_inspector),
        window_layout: None,
    }
}

#[component]
pub fn App() -> impl IntoView {
    crate::provide_theme("dark".to_string());
    crate::history::provide_history();
    crate::components::recovery::save_status::provide_save_status();
    // Capture the workspace context at render time so async tasks and raw
    // event callbacks never call `expect_context` without a reactive owner.
    let workspace = provide_workspace();

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

    // ── Phase 11.3: session persistence + crash recovery ───────────────
    // Workspace signals captured into the persisted session.
    let (active_note, set_active_note) = signal(Option::<String>::None);

    // Phase 12.1: keep the editor's `active_note` in sync with the shared
    // workspace tabs state. Clicking a tab (or opening a note from the tree)
    // sets `workspace.active_path`; without this the editor would stay on the
    // old note. Syncing `None` too means closing the last tab (or deleting the
    // only open note) clears the editor instead of leaving it autosaving a
    // trashed file back into existence. The editor's `on_active_note` guard
    // and `open_tab` converge, so the mount loop terminates.
    Effect::new(move |_| {
        let path = workspace.active_path.get();
        if active_note.get() != path {
            set_active_note.set(path);
        }
    });
    let (editor_cursor, set_editor_cursor) = signal(0u32);
    let (editor_scroll, set_editor_scroll) = signal(0u32);
    let pending_recovery = RwSignal::new(None::<crate::components::recovery::session::RecoveryStatus>);
    let set_pending_recovery = pending_recovery;

    // On mount, ask the backend whether the previous run crashed.
    spawn_local(async move {
        let empty_args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke("recovery_check", empty_args).await;
        if let Ok(status) =
            serde_wasm_bindgen::from_value::<crate::components::recovery::session::RecoveryStatus>(result)
        {
            if status.crashed {
                // Never silently discard recoverable work — surface a banner.
                set_pending_recovery.set(Some(status));
            } else if let Some(session) = status.session {
                // Clean shutdown with a saved session: restore automatically.
                if let Some(mode) = session.view_mode.as_deref() {
                    set_view_mode.set(parse_view_mode(mode));
                }
            if let Some(note) = session.active_note.clone() {
                set_active_note.set(Some(note.clone()));
                // Open the restored note in a workspace tab too.
                open_tab(workspace, &note);
            }
                if let Some(cursor) = session.cursor_pos {
                    set_editor_cursor.set(cursor);
                }
                if let Some(scroll) = session.scroll_top {
                    set_editor_scroll.set(scroll);
                }
                if let Some(left) = session.left_sidebar {
                    set_show_left_sidebar.set(left);
                }
                if let Some(right) = session.right_inspector {
                    set_show_right_inspector.set(right);
                }
            }
        }
    });

    // Persist the session (debounced) whenever any workspace field changes.
    let (session_dirty, set_session_dirty) = signal(0u32);
    Effect::new(move |_| {
        let _ = view_mode.get();
        let _ = show_left_sidebar.get();
        let _ = show_right_inspector.get();
        let _ = active_note.get();
        let _ = editor_cursor.get();
        let _ = editor_scroll.get();
        set_session_dirty.update(|v| *v = v.wrapping_add(1));
    });
    Effect::new(move |_| {
        let _ = session_dirty.get();
        set_timeout(
            move || {
                let state = session_state_from(
                    view_mode.get(),
                    active_note.get(),
                    editor_cursor.get(),
                    editor_scroll.get(),
                    show_left_sidebar.get(),
                    show_right_inspector.get(),
                );
                crate::components::recovery::session::session_save(&state);
            },
            std::time::Duration::from_millis(800),
        );
    });

    // Saving the session right before the window unloads catches the latest
    // cursor / scroll position.
    let beforeunload_handle = window_event_listener_untyped("beforeunload", move |_| {
        let state = session_state_from(
            view_mode.get(),
            active_note.get(),
            editor_cursor.get(),
            editor_scroll.get(),
            show_left_sidebar.get(),
            show_right_inspector.get(),
        );
        crate::components::recovery::session::session_save(&state);
    });
    on_cleanup(move || beforeunload_handle.remove());

    // Restore-session action from the recovery banner.
    let restore_session = Callback::new(move |session: crate::components::recovery::session::SessionState| {
        if let Some(mode) = session.view_mode.as_deref() {
            set_view_mode.set(parse_view_mode(mode));
        }
        if let Some(note) = session.active_note.clone() {
            set_active_note.set(Some(note.clone()));
            open_tab(workspace, &note);
        }
        if let Some(cursor) = session.cursor_pos {
            set_editor_cursor.set(cursor);
        }
        if let Some(scroll) = session.scroll_top {
            set_editor_scroll.set(scroll);
        }
        if let Some(left) = session.left_sidebar {
            set_show_left_sidebar.set(left);
        }
        if let Some(right) = session.right_inspector {
            set_show_right_inspector.set(right);
        }
        toasts.success("Session restored", "Your previous workspace is back.");
    });

    let inspect_recovery = Callback::new(move |_| {
        set_view_mode.set(ViewMode::Recovery);
    });

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
                    // Crash recovery banner (only when a previous session is pending)
                    <div class="absolute top-16 left-1/2 -translate-x-1/2 z-50 w-full max-w-3xl px-4">
                        <RecoveryBanner
                            recovery=pending_recovery
                            on_restore=restore_session
                            on_inspect=inspect_recovery
                        />
                    </div>
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
                            <button
                                class=move || format!("px-2.5 py-1 rounded transition-colors {}", if view_mode.get() == ViewMode::History { "bg-blue-600 text-white font-medium" } else { "text-gray-400 hover:text-gray-200 hover:bg-gray-700/50" })
                                on:click=move |_| set_view_mode.set(ViewMode::History)
                                title="Version history"
                            >
                                "🕘 History"
                            </button>
                            <button
                                class=move || format!("px-2.5 py-1 rounded transition-colors {}", if view_mode.get() == ViewMode::Recovery { "bg-blue-600 text-white font-medium" } else { "text-gray-400 hover:text-gray-200 hover:bg-gray-700/50" })
                                on:click=move |_| set_view_mode.set(ViewMode::Recovery)
                                title="Recovery Manager"
                            >
                                "🛟 Recovery"
                            </button>

                            <div class="flex-1"></div>

                            <SaveStatusIndicator />

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
                                        <NoteEditor
                                            note_path=active_note.get().unwrap_or_default()
                                            initial_content=initial_content.get()
                                            on_active_note=Callback::new(move |p: String| {
                                                if active_note.get().as_deref() != Some(p.as_str()) {
                                                    set_active_note.set(Some(p.clone()));
                                                    // Opening from a session restore / editor mount
                                                    // should surface in the tab bar.
                                                    open_tab(workspace, &p);
                                                }
                                            })
                                            on_cursor=Callback::new(move |c: u32| set_editor_cursor.set(c))
                                            on_scroll=Callback::new(move |s: u32| set_editor_scroll.set(s))
                                        />
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
                                ViewMode::History => view! {
                                    <div class="max-w-7xl mx-auto h-full">
                                        <VersionHistory />
                                    </div>
                                }.into_any(),
                                ViewMode::Recovery => view! {
                                    <div class="max-w-7xl mx-auto h-full">
                                        <RecoveryManager />
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
