//! # Nabu App Shell — Dioxus root component and routing entry
//!
//! Composes all seven context providers, then delegates to [`AppRouter`] for
//! vault-state-aware routing. Once a vault is configured, the
//! [`WorkspaceLayout`] (ribbon, sidebars, tab bar, navbar, view content, and
//! overlay surfaces) takes over.
//!
//! View switching within the workspace is driven by [`NavContext::view_mode`];
//! actual view content is rendered by [`ViewContent`].

use crate::components::contexts::{use_nav, NavContext, use_workspace};
use crate::components::contexts::{
    HistoryProvider, NavProvider, SaveStatusProvider, ThemeProvider, WorkspaceProvider,
};
use crate::components::layout::WorkspaceLayout;
use crate::components::navigation::{
    CommandPalette, QuickSwitcher, ShortcutReference, ViewMode,
    ArchivePage, CalendarPage, Dashboard, HomeScreen, SearchPage, SmartFoldersPage,
};
use crate::components::ui::feedback::{TaskProvider, ToastProvider};
use crate::components::ui::icons::{render_icon, Icon};
use crate::components::ui::notifications::NotificationManager;
use dioxus::prelude::*;
use wasm_bindgen_futures::spawn_local;

// ── App shell ───────────────────────────────────────────────────

/// The root component function passed to `dioxus::web::launch::launch_cfg`.
#[allow(non_snake_case)]
pub fn App() -> Element {
    rsx! {
        crate::events::EventServiceProvider {
            ThemeProvider { initial_theme: "dark".to_string() }
            ToastProvider {
                TaskProvider {
                    NotificationManager {}
                    HistoryProvider {
                        SaveStatusProvider {
                            WorkspaceProvider {
                                NavProvider {
                                    CommandPalette {}
                                    QuickSwitcher {}
                                    ShortcutReference {}
                                    crate::components::app::AppRouter {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Vault-state-aware router ─────────────────────────────────────

/// Vault check lifecycle.
#[derive(Clone, Copy, PartialEq, Debug)]
enum VaultCheckState {
    Loading,
    VaultSetup,
    Error,
    MainDashboard,
}

/// Checks vault state on mount and renders either a loading screen, a
/// vault-setup prompt, or the `WorkspaceLayout`.
#[component]
pub fn AppRouter() -> Element {
    let mut vault_state = use_signal(|| VaultCheckState::Loading);
    let mut vault_error = use_signal(|| String::new());
    let mut nav = use_nav();

    use_effect(move || {
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            match crate::ipc::tauri_invoke_safe("check_vault_exists", args).await {
                Some(result) => {
                    match serde_wasm_bindgen::from_value::<Option<String>>(result) {
                        Ok(Some(path)) if !path.is_empty() => {
                            vault_state.set(VaultCheckState::MainDashboard);
                            let name = path
                                .rsplit('/')
                                .next()
                                .filter(|n| !n.is_empty())
                                .unwrap_or("Vault")
                                .to_string();
                            nav.vault_name.set(name);
                        }
                        Ok(_) => {
                            vault_state.set(VaultCheckState::VaultSetup);
                        }
                        Err(_) => {
                            vault_state.set(VaultCheckState::Error);
                            vault_error
                                .set("Unexpected response from check_vault_exists".into());
                        }
                    }
                }
                None => {
                    vault_state.set(VaultCheckState::Error);
                    vault_error
                        .set("Failed to contact Tauri backend (check_vault_exists)".into());
                }
            }
        });
    });

    let state = *vault_state.read();
    match state {
        VaultCheckState::Loading => rsx! {
            div { class: "flex h-screen w-screen items-center justify-center bg-gray-950 text-gray-100",
                div { class: "flex flex-col items-center gap-4",
                    div { class: "w-6 h-6 animate-spin rounded-full border-2 border-blue-500 border-t-transparent" }
                    div { "Opening Nabu..." }
                }
            }
        },
        VaultCheckState::Error => rsx! {
            div { class: "flex h-screen w-screen items-center justify-center bg-gray-950 text-red-300",
                div { "{vault_error.read()}" }
            }
        },
        VaultCheckState::VaultSetup => rsx! {
            div { class: "flex h-screen w-screen items-center justify-center bg-gray-950 text-gray-100",
                div { class: "flex flex-col items-center gap-4 max-w-md text-center",
                    div {
                        class: "flex h-8 w-8 items-center justify-center text-gray-400",
                        {render_icon(Icon::Folder, Some("w-8 h-8 text-gray-400"))}
                    },
                    div { class: "text-xl font-semibold", "No vault configured" },
                    div { class: "text-sm opacity-70",
                        "Please create or select a vault to begin using Nabu."
                    },
                    button {
                        class: "mt-4 rounded-md bg-blue-600 px-4 py-2 text-sm font-medium hover:bg-blue-700",
                        onclick: move |_| {},
                        "Open Settings"
                    }
                }
            }
        },
        VaultCheckState::MainDashboard => rsx! {
            WorkspaceLayout {}
        },
    }
}

// ── View content (switching) ─────────────────────────────────────

/// Switches views based on [`NavContext::view_mode`].
#[component]
pub fn ViewContent() -> Element {
    let nav: NavContext = use_nav();
    let mode = *nav.view_mode.read();

    match mode {
        ViewMode::Dashboard => rsx! { Dashboard {} },
        ViewMode::Editor => {
            let ws = use_workspace();
            if ws.active_path.read().is_some() {
                rsx! {
                    div { class: "max-w-4xl mx-auto h-full",
                        crate::components::note_editor::NoteEditor {}
                    }
                }
            } else {
                rsx! { HomeScreen {} }
            }
        }
        ViewMode::Graph => rsx! {
            div { class: "w-full h-full flex items-center justify-center",
                div { class: "text-sm text-gray-400",
                    {render_icon(Icon::Network, Some("w-4 h-4 inline mr-1"))}
                    "Graph view placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::Search => rsx! { SearchPage {} },
        ViewMode::Settings => rsx! {
            div { class: "max-w-4xl mx-auto h-full",
                crate::components::settings::settings_panel::SettingsPanel {}
            }
        },
        ViewMode::Inbox => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                crate::components::inbox::Inbox {}
            }
        },
        ViewMode::ReadingQueue => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                div { class: "text-sm text-gray-400",
                    {render_icon(Icon::BookOpen, Some("w-4 h-4 inline mr-1"))}
                    "Reading queue placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::Templates => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                crate::components::template_editor::TemplateEditor {}
            }
        },
        ViewMode::Trash => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                div { class: "text-sm text-gray-400",
                    {render_icon(Icon::Trash2, Some("w-4 h-4 inline mr-1"))}
                    "Trash placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::History => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                crate::components::recovery::version_history::VersionHistory {}
            }
        },
        ViewMode::Recovery => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                crate::components::recovery::recovery_manager::RecoveryManager {}
            }
        },
        ViewMode::Calendar => rsx! { CalendarPage {} },
        ViewMode::Archive => rsx! { ArchivePage {} },
        ViewMode::SmartFolders => rsx! { SmartFoldersPage {} },
        ViewMode::Canvas => rsx! {
            div { class: "w-full h-full",
                div { class: "text-sm text-gray-400",
                    {render_icon(Icon::Palette, Some("w-4 h-4 inline mr-1"))}
                    "Canvas placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::Reader => rsx! {
            div { class: "w-full h-full",
                div { class: "text-sm text-gray-400",
                    {render_icon(Icon::BookText, Some("w-4 h-4 inline mr-1"))}
                    "Reader mode placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::Comparison => rsx! {
            div { class: "w-full h-full",
                div { class: "text-sm text-gray-400",
                    {render_icon(Icon::Comparison, Some("w-4 h-4 inline mr-1"))}
                    "Comparison view placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::Statistics => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                crate::components::statistics::StatisticsView {}
            }
        },
        ViewMode::Activity => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                crate::components::activity::ActivityPanel {}
            }
        },
    }
}
