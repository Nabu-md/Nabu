//! # Nabu App Shell — Dioxus root component and routing entry
//!
//! Phase 0 is intentionally minimal:
//! - [`App`] wraps the entire tree in all seven context providers.
//! - [`AppRouter`] checks for a configured vault on startup and renders
//!   a loading screen, vault-setup prompt, or the routed dashboard.
//! - [`Dashboard`] is a placeholder; real dashboard widgets arrive in P0.2/P0.3.
//! - [`RootLayout`] wraps routes with a persistent header shell.
//!
//! View components, ribbon bar, sidebar, inspector, session recovery, and
//! global shortcuts are **not** migrated in Phase 0.

use crate::components::contexts::{
    HistoryProvider, NavProvider, SaveStatusProvider, TaskProvider, ThemeProvider,
    ToastProvider, WorkspaceProvider,
};
use crate::components::ui::icons::{Icon, IconEl};
use dioxus::prelude::*;

/// Top-level routes. Phase 0 ships only the dashboard placeholder; view-level
/// routes will be added as views are migrated in P0.3.
#[derive(Routable, Clone, PartialEq)]
#[rustfmt::skip]
pub enum AppRoute {
    #[layout(RootLayout)]
    #[route("/")]
    Dashboard {},
}

/// Root layout — wraps every route with a persistent header shell.
/// The `<Outlet>` renders the active route component.
#[component]
fn RootLayout() -> Element {
    rsx! {
        div {
            class: "flex h-screen w-screen flex-col bg-gray-950 text-gray-100 font-sans overflow-hidden",
            // Header
            div {
                class: "flex-none border-b border-gray-800 px-4 py-3 flex items-center gap-3",
                IconEl {
                    icon: Icon::Dashboard,
                    class: "w-5 h-5 text-blue-400",
                }
                span { class: "font-semibold text-lg", "Nabu" }
            }
            // Route content
            Outlet::<AppRoute> {}
        }
    }
}

/// The root component function passed to `dioxus::web::launch::launch_cfg`.
///
/// Wraps the entire tree in all context providers, then delegates to
/// [`AppRouter`] for vault-state-aware routing.
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

    // Run the vault check once on mount.  The effect captures signal copies
    // (Signals are Copy) and spawns an async task that updates them when the
    // IPC round-trip completes.  Because the callback reads no signals, the
    // effect fires only once.
    use_effect(move || {
        spawn(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            match crate::ipc::tauri_invoke_safe("check_vault_exists", args).await {
                Some(result) => {
                    match serde_wasm_bindgen::from_value::<Option<String>>(result) {
                        Ok(Some(path)) if !path.is_empty() => {
                            vault_state.set(VaultCheckState::MainDashboard);
                        }
                        Ok(_) => {
                            vault_state.set(VaultCheckState::VaultSetup);
                        }
                        Err(_) => {
                            vault_state.set(VaultCheckState::Error);
                            vault_error.set(
                                "Unexpected response from check_vault_exists".into(),
                            );
                        }
                    }
                }
                None => {
                    vault_state.set(VaultCheckState::Error);
                    vault_error.set(
                        "Failed to contact Tauri backend (check_vault_exists)".into(),
                    );
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
                Router::<AppRoute> {}
            }
        }
    }
}

/// Minimal dashboard placeholder — actual dashboard widgets are migrated in
/// P0.2 / P0.3.
#[component]
fn Dashboard() -> Element {
    rsx! {
        div {
            class: "flex-1 overflow-y-auto p-6",
            div {
                class: "max-w-4xl mx-auto",
                h1 { class: "text-2xl font-bold mb-4", "Dashboard" }
                p { class: "opacity-70",
                    "Phase 0 root shell is live. View components arrive in P0.3."
                }
            }
        }
    }
}
