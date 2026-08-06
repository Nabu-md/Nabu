//! # Nabu App Shell — Dioxus root component and routing entry
//!
//! Composes all seven context providers, then delegates to [`AppRouter`] for
//! vault-state-aware routing. Once a vault is configured, the
//! [`WorkspaceLayout`] (ribbon, sidebars, tab bar, navbar, view content, and
//! overlay surfaces) takes over.
//!
//! View switching within the workspace is driven by [`NavContext::view_mode`];
//! actual view content is rendered by [`ViewContent`] which delegates to
//! placeholder components for each view (migrated in later phases).

use crate::components::contexts::{
    HistoryProvider, NavProvider, SaveStatusProvider, ThemeProvider, WorkspaceProvider,
};
use crate::components::layout::WorkspaceLayout;
use crate::components::navigation::{KeyboardShortcuts, ViewMode};
use crate::components::navigation::{
    ArchivePage, CalendarPage, CommandPalette, Dashboard, HomeScreen, QuickSwitcher,
    SearchPage, ShortcutReference, SmartFoldersPage,
};
use crate::components::ui::feedback::{TaskProvider, ToastProvider};
use crate::components::ui::icons::{Icon, IconEl};
use crate::components::contexts::{use_nav, NavContext, use_workspace};
use dioxus::prelude::*;

// ── Routes ──────────────────────────────────────────────────────

/// Top-level routes. Phase 0 ships only the dashboard; view switching happens
/// inside [`WorkspaceLayout`] via the `view_mode` signal.
#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum AppRoute {
    #[route("/")]
    Dashboard {},
}

// ── App shell ───────────────────────────────────────────────────

/// The root component function passed to `dioxus::web::launch::launch_cfg`.
#[allow(non_snake_case)]
pub fn App() -> Element {
    rsx! {
        ThemeProvider { initial_theme: "dark".to_string() }
        ToastProvider {
            TaskProvider {
                HistoryProvider {
                    SaveStatusProvider {
                        WorkspaceProvider {
                            NavProvider {
                                KeyboardShortcuts {}
                                AppRouter {}
                            }
                        }
                    }
                }
            }
        }
    }
}

// ── Vault-state-aware router ─────────────────────────────────────

/// Vault check lifecycle — mirrors the LePtOS `AppScreen` enum.
#[derive(Clone, Copy, PartialEq, Debug)]
enum VaultCheckState {
    Loading,
    VaultSetup,
    Error,
    MainDashboard,
}

/// Checks vault state on mount and renders either a loading screen, a
/// vault-setup prompt, or the routed dashboard.
#[component]
pub fn AppRouter() -> Element {
    let mut vault_state = use_signal(|| VaultCheckState::Loading);
    let mut vault_error = use_signal(String::new);

    use_effect(move || {
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            match crate::ipc::tauri_invoke_safe("check_vault_exists", args).await {
                Some(result) => {
                    match serde_wasm_bindgen::from_value::<Option<String>>(result) {
                        Ok(Some(path)) if !path.is_empty() => {
                            vault_state.set(VaultCheckState::MainDashboard);
                            // Derive the vault display name for breadcrumbs / home.
                            let name = path
                                .rsplit('/')
                                .next()
                                .filter(|n| !n.is_empty())
                                .unwrap_or("Vault")
                                .to_string();
                            use_nav().vault_name.set(name);
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
        VaultCheckState::Loading => {
            rsx! {
                div { class: "flex h-screen w-screen items-center justify-center bg-gray-950 text-gray-100",
                    div { class: "flex flex-col items-center gap-4",
                        div { class: "w-6 h-6 animate-spin rounded-full border-2 border-blue-500 border-t-transparent" }
                        div { "Opening Nabu…" }
                    }
                }
            }
        }
        VaultCheckState::Error => {
            rsx! {
                div { class: "flex h-screen w-screen items-center justify-center bg-gray-950 text-red-300",
                    div { "{vault_error.read()}" }
                }
            }
        }
        VaultCheckState::VaultSetup => {
            rsx! {
                div { class: "flex h-screen w-screen items-center justify-center bg-gray-950 text-gray-100",
                    div { class: "flex flex-col items-center gap-4 max-w-md text-center",
                        IconEl {
                            icon: Icon::Folder,
                            class: "w-8 h-8 text-gray-400",
                        }
                        div { class: "text-xl font-semibold", "No vault configured" }
                        div { class: "text-sm opacity-70",
                            "Please create or select a vault to begin using Nabu."
                        }
                        button {
                            class: "mt-4 rounded-md bg-blue-600 px-4 py-2 text-sm font-medium hover:bg-blue-700",
                            onclick: move |_| {
                                // TODO: open settings → vault path picker
                            },
                            "Open Settings"
                        }
                    }
                }
            }
        }
        VaultCheckState::MainDashboard => {
            rsx! {
                WorkspaceLayout {}
            }
        }
    }
}

// ── View content (switching) ─────────────────────────────────────

/// Switches views based on [`NavContext::view_mode`].
///
/// Feature screens are rendered as placeholders; future phases replace them
/// with migrated views. The view-switching framework itself (reading the
/// signal, matching, and rendering the right container) is what this phase
/// delivers.
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
                        div { class: "text-sm text-gray-400",
                            {IconEl { icon: Icon::FilePen, class: "w-4 h-4 inline mr-1" }}
                            "Note editor placeholder — migrated in a later phase."
                        }
                    }
                }
            } else {
                rsx! { HomeScreen {} }
            }
        }
        ViewMode::Graph => rsx! {
            div { class: "w-full h-full flex items-center justify-center",
                div { class: "text-sm text-gray-400",
                    {IconEl { icon: Icon::Network, class: "w-4 h-4 inline mr-1" }}
                    "Graph view placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::Search => rsx! { SearchPage {} },
        ViewMode::Settings => rsx! {
            div { class: "max-w-4xl mx-auto h-full",
                div { class: "text-sm text-gray-400",
                    {IconEl { icon: Icon::Settings, class: "w-4 h-4 inline mr-1" }}
                    "Settings placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::Inbox => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                div { class: "text-sm text-gray-400",
                    {IconEl { icon: Icon::Inbox, class: "w-4 h-4 inline mr-1" }}
                    "Inbox placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::ReadingQueue => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                div { class: "text-sm text-gray-400",
                    {IconEl { icon: Icon::BookOpen, class: "w-4 h-4 inline mr-1" }}
                    "Reading queue placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::Templates => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                div { class: "text-sm text-gray-400",
                    {IconEl { icon: Icon::ClipboardList, class: "w-4 h-4 inline mr-1" }}
                    "Templates placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::Trash => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                div { class: "text-sm text-gray-400",
                    {IconEl { icon: Icon::Trash2, class: "w-4 h-4 inline mr-1" }}
                    "Trash placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::History => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                div { class: "text-sm text-gray-400",
                    {IconEl { icon: Icon::History, class: "w-4 h-4 inline mr-1" }}
                    "Version history placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::Recovery => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                div { class: "text-sm text-gray-400",
                    {IconEl { icon: Icon::LifeBuoy, class: "w-4 h-4 inline mr-1" }}
                    "Recovery manager placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::Calendar => rsx! { CalendarPage {} },
        ViewMode::Archive => rsx! { ArchivePage {} },
        ViewMode::SmartFolders => rsx! { SmartFoldersPage {} },
        ViewMode::Canvas => rsx! {
            div { class: "w-full h-full",
                div { class: "text-sm text-gray-400",
                    {IconEl { icon: Icon::Palette, class: "w-4 h-4 inline mr-1" }}
                    "Canvas placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::Reader => rsx! {
            div { class: "w-full h-full",
                div { class: "text-sm text-gray-400",
                    {IconEl { icon: Icon::BookText, class: "w-4 h-4 inline mr-1" }}
                    "Reader mode placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::Comparison => rsx! {
            div { class: "w-full h-full",
                div { class: "text-sm text-gray-400",
                    {IconEl { icon: Icon::Comparison, class: "w-4 h-4 inline mr-1" }}
                    "Comparison view placeholder — migrated in a later phase."
                }
            }
        },
        ViewMode::Statistics => rsx! {
            div { class: "max-w-7xl mx-auto h-full",
                div { class: "text-sm text-gray-400",
                    {IconEl { icon: Icon::TrendingUp, class: "w-4 h-4 inline mr-1" }}
                    "Statistics placeholder — migrated in a later phase."
                }
            }
        },
    }
}
