//! # Capability Management — Settings page
//!
//! Provides the UI for viewing, enabling, and disabling registered
//! capabilities. Integrates with the existing Settings panel architecture and
//! reuses the shared Dioxus component library (`Switch`, `StatusDot`,
//! `Spinner`, `Badge`, `ErrorPanel`, `EmptyState`).
//!
//! ## Architecture
//!
//! ```text
//! CapabilityManagementPage         (root — loads + stores state)
//! ├── CapabilityList              (renders the grid of capabilities)
//! │   └── CapabilityRow           (one per capability: metadata + toggle)
//! ├── RefreshButton               (re-fetches from backend)
//! └── State overlays             (loading / error / empty)
//! ```
//!
//! ## IPC interactions
//!
//! - `capability_list_with_state` — fetches all registered capabilities with
//!   their current enabled/disabled state and provider. Called on mount and on
//!   refresh.
//! - `capability_enable` — enables a capability by ID. Optimistic UI update
//!   (the toggle flips immediately); rollback on failure.
//! - `capability_disable` — disables a capability by ID (same optimistic
//!   pattern).
//!
//! ## EventBus integration
//!
//! The backend publishes `CapabilityStateChanged` events on the `nabu-event`
//! channel whenever a capability's enabled state changes. The page subscribes
//! via `use_event_listener` so that state changes originating elsewhere
//! (e.g. plugin lifecycle, other windows) are reflected in the UI without a
//! manual refresh.
//!
//! ## State flow
//!
//! ```text
//! [mount/refresh] → capability_list_with_state IPC → Signal<CapabilitySummary>
//!        ↓
//!    Render CapabilityList (one CapabilityRow per entry)
//!        ↓
//!  User toggles → capability_enable/disable IPC
//!        ↓
//!  Optimistic Signal update + status = Pending
//!        ↓
//!  IPC resolves → status = Enabled/Disabled (or Error on failure)
//!  ─────────────────────────────────────────
//!  EventBus event → CapabilityStateChanged → Signal update + status sync
//! ```

use crate::components::ui::feedback::{
    ErrorPanel, LoadingBlock, Spinner, SpinnerSize, StatusDot, StatusKind,
};
use crate::components::ui::icons::{render_icon_view, Icon};
use crate::components::ui::info::EmptyState;
use crate::components::ui::selection::Switch;
use crate::events::{use_event_listener, FrontendEventKind};
use crate::models::capability::{CapabilityStatus, CapabilitySummary};
use dioxus::prelude::*;
use nabu_core::event_bus::PipelineEvent;
use serde_wasm_bindgen::to_value;
use wasm_bindgen_futures::spawn_local;

// ── CapabilityManagementPage ─────────────────────────────────────────────────

/// Root component for the Capability Management settings page.
///
/// Maintains the list of capabilities, loading/error state, and a refresh
/// counter. Subscribes to `CapabilityStateChanged` events so that state changes
/// originating from other parts of the app are reflected reactively.
#[component]
pub fn CapabilityManagementPage() -> Element {
    // The current list of capability summaries.
    let caps = use_signal(Vec::<CapabilitySummary>::new);
    // True while an initial load or refresh is in progress.
    let loading = use_signal(|| false);
    // Error message from the last IPC failure, if any.
    let error = use_signal(|| None::<String>);
    // Bumped on every successful load to track sequence (future-proofing).
    let _seq = use_signal(|| 0u64);

    // ── EventBus subscription ──────────────────────────────────────────────
    //
    // `CapabilityStateChanged` events keep the UI in sync with state changes
    // that originate outside this page (e.g. plugin activation, other windows).
    // We reconcile by updating the matching summary's `enabled` / `status`
    // fields rather than re-fetching the full list.
    let caps_ev = caps.clone();
    use_event_listener(FrontendEventKind::CapabilityStateChanged, move |ev: &crate::events::FrontendEvent| {
        if let PipelineEvent::CapabilityStateChanged(evt) = &ev.payload {
            let id = evt.capability_id.clone();
            let enabled = evt.enabled;
            caps_ev.write_unchecked().iter_mut().for_each(|c| {
                if c.id() == id {
                    c.enabled = enabled;
                    c.status = if enabled {
                        CapabilityStatus::Enabled
                    } else {
                        CapabilityStatus::Disabled
                    };
                }
            });
        }
    });

    // ── Initial load ───────────────────────────────────────────────────────
    {
        let caps = caps.clone();
        let loading = loading.clone();
        let error = error.clone();
        let seq = _seq.clone();
        use_effect(move || {
            load_capabilities(caps, loading, error, seq);
        });
    }

    let error_msg = error.read().clone();
    let is_loading = *loading.read();
    let caps_len = caps.read().len();
    let caps_view = caps.clone();

    rsx! {
        div { class: "capability-management flex flex-col gap-4",
            // Header with refresh button
            div {
                class: "flex items-center justify-between",
                h2 { class: "text-xl font-bold", "Capabilities" }
                RefreshButton {
                    caps: caps_view,
                    loading: loading,
                    error: error,
                    seq: _seq,
                }
            }

            // Status summary bar
            {capability_summary_bar(caps.clone())}

            // Main content: loading / error / list / empty
            if let Some(err) = error_msg {
                ErrorPanel {
                    title: "Failed to load capabilities",
                    message: "{err}",
                    on_retry: Some(EventHandler::new({
                        let caps = caps.clone();
                        let loading = loading.clone();
                        let error = error.clone();
                        let seq = _seq.clone();
                        move |_| {
                            spawn_local(async move {
                                load_capabilities(caps, loading, error, seq);
                            });
                        }
                    })),
                    recovery: Some("Check that the Tauri backend is running.".to_string()),
                }
            } else if is_loading && caps_len == 0 {
                LoadingBlock { size: SpinnerSize::Lg, label: "Loading capabilities\u{2026}" }
            } else if caps_len == 0 && !is_loading {
                EmptyState {
                    icon: Some(Icon::Layers),
                    title: "No capabilities registered",
                    description: "No capabilities are currently available on this platform.",
                }
            } else {
                CapabilityList { caps: caps }
            }
        }
    }
}

// ── RefreshButton ────────────────────────────────────────────────────────────

/// Refresh button — re-fetches the capability list from the backend.
#[component]
fn RefreshButton(
    caps: Signal<Vec<CapabilitySummary>>,
    loading: Signal<bool>,
    error: Signal<Option<String>>,
    seq: Signal<u64>,
) -> Element {
    let is_loading = *loading.read();
    let icon = if is_loading { Icon::Loader } else { Icon::RefreshCw };
    let label = if is_loading { " Refreshing" } else { " Refresh" };

    rsx! {
        button {
            r#type: "button",
            class: "btn btn-ghost btn-sm",
            disabled: is_loading,
            "aria-label": "Refresh capability list",
            title: "Refresh",
            onclick: move |_| {
                let caps2 = caps.clone();
                let loading2 = loading.clone();
                let error2 = error.clone();
                let seq2 = seq.clone();
                spawn_local(async move {
                    load_capabilities(caps2, loading2, error2, seq2);
                });
            },
            {render_icon_view(icon)}
            "{label}"
        }
    }
}

// ── Summary bar ─────────────────────────────────────────────────────────────

/// Header summary bar showing counts of enabled / disabled / total capabilities.
fn capability_summary_bar(caps: Signal<Vec<CapabilitySummary>>) -> Element {
    let list = caps.read().clone();
    let total = list.len();
    let enabled_count = list.iter().filter(|c| c.enabled).count();
    let disabled_count = total - enabled_count;

    let aria_label = format!(
        "Capability summary: {total} total, {enabled_count} enabled, {disabled_count} disabled"
    );
    let enabled_label = format!("Enabled: {enabled_count}");
    let disabled_label = format!("Disabled: {disabled_count}");
    let total_text = format!("\u{2014} {total} total");

    rsx! {
        div {
            class: "flex items-center gap-4 text-sm text-gray-400",
            "aria-label": "{aria_label}",
        }
        StatusDot { kind: StatusKind::Success, label: "{enabled_label}" }
        "{enabled_count} enabled"
        StatusDot { kind: StatusKind::Warning, label: "{disabled_label}" }
        "{disabled_count} disabled"
        span { class: "text-gray-400", "{total_text}" }
    }
}

// ── CapabilityList ───────────────────────────────────────────────────────────

/// Renders the list of capabilities as a table-like layout.
#[component]
fn CapabilityList(caps: Signal<Vec<CapabilitySummary>>) -> Element {
    let list = caps.read().clone();
    if list.is_empty() {
        return rsx! {};
    }

    rsx! {
        div {
            class: "capability-list border border-gray-700 rounded-lg overflow-hidden",
            // Table header
            div {
                class: "flex items-center gap-4 px-4 py-2 bg-gray-800 border-b border-gray-700 text-xs font-medium text-gray-400 uppercase",
                div { class: "w-[300px]", "Capability" }
                div { class: "flex-1", "Description" }
                div { class: "w-32", "Provider" }
                div { class: "w-24 text-center", "Status" }
                div { class: "w-32 text-center", "Enabled" }
            }
            // Capability rows
            div { class: "divide-y divide-gray-700",
                for cap in list {
                    {CapabilityRow { cap: cap.clone() }}
                }
            }
        }
    }
}

// ── CapabilityRow ────────────────────────────────────────────────────────────

/// A single capability row with metadata, status indicator, and enable/disable
/// toggle.
#[component]
fn CapabilityRow(cap: CapabilitySummary) -> Element {
    let id = cap.id();
    let display_name = format!("{}:{}", cap.capability.namespace, cap.capability.name);
    let description = cap.capability.description.clone();
    let required = cap.capability.required;
    let provider = cap.provider.clone();
    let enabled = cap.enabled;
    let status = cap.status.clone();

    // Signals for this row's toggle (local optimistic state)
    let toggle_enabled = use_signal(|| enabled);
    let toggle_disabled = use_signal(|| false);
    let toggle_error = use_signal(|| None::<String>);

    let status_label = status.label().to_string();
    let status_kind = match &status {
        CapabilityStatus::Enabled => StatusKind::Success,
        CapabilityStatus::Disabled => StatusKind::Warning,
        CapabilityStatus::Pending(_) => StatusKind::Info,
        CapabilityStatus::Error(_) => StatusKind::Error,
        CapabilityStatus::Unknown => StatusKind::Neutral,
    };

    let switch_label = format!("Enable \"{display_name}\"");
    let is_pending = matches!(status, CapabilityStatus::Pending(_));
    let is_disabled = *toggle_disabled.read() || is_pending;

    rsx! {
        div {
            class: "capability-row flex items-center gap-4 px-4 py-3",
            // Capability name + required badge
            div { class: "w-[300px] flex items-center gap-2",
                div { class: "font-medium text-gray-100", "{display_name}" }
                if required {
                    span { class: "badge badge-info", "Required" }
                }
            }
            // Description
            div { class: "flex-1 text-sm text-gray-400", "{description}" }
            // Provider
            div { class: "w-32 text-sm text-gray-500 truncate", title: "{provider}", "{provider}" }
            // Status
            div { class: "w-24 flex items-center justify-center" }
            StatusDot { kind: status_kind, label: "{status_label}" }
            // Enabled toggle
            div { class: "w-32 flex items-center justify-center" }
            Switch {
                checked: toggle_enabled,
                label: switch_label,
                disabled: is_disabled,
                on_change: move |new_val: bool| {
                    toggle_error.set(None);
                    handle_toggle(
                        id.clone(),
                        new_val,
                        toggle_enabled.clone(),
                        toggle_disabled.clone(),
                        toggle_error.clone(),
                    );
                },
            }
            {toggle_error.read().as_ref().map(|e| rsx! {
                span { class: "text-xs text-red-400", "{e}" }
            })}
        }
    }
}

// ── Toggle handler ───────────────────────────────────────────────────────────

/// Handles an enable/disable toggle with optimistic UI update and error
/// rollback.
fn handle_toggle(
    cap_id: String,
    enable: bool,
    toggle_enabled: Signal<bool>,
    toggle_disabled: Signal<bool>,
    toggle_error: Signal<Option<String>>,
) {
    let toasts = crate::components::ui::feedback::use_toast();
    let mut action = if enable { "Enable" } else { "Disable" };

    toggle_disabled.set(true);
    toggle_error.set(None);

    spawn_local(async move {
        let cmd = if enable { "capability_enable" } else { "capability_disable" };
        let args = to_value(&serde_json::json!({ "capability_id": cap_id.clone() })).unwrap();
        let result = crate::ipc::tauri_invoke_safe(cmd, args).await;

        toggle_disabled.set(false);

        match result {
            Some(Ok(())) => {
                toggle_enabled.set(enable);
                let msg = format!("{}d", &action[..1]);
                let _ = msg;
                toasts.success(
                    format!("Capability {}ed", if enable { "en" } else { "dis" }),
                    format!("The capability has been {}.", if enable { "activated" } else { "deactivated" }),
                );
            }
            Some(Err(e)) => {
                toggle_enabled.set(!enable);
                toggle_error.set(Some(e.to_string()));
                toasts.error(
                    format!("{} failed", action),
                    "Could not update capability state.".to_string(),
                );
            }
            None => {
                toggle_enabled.set(!enable);
                toggle_error.set(Some("IPC unavailable".to_string()));
                toasts.error(
                    format!("{} failed", action),
                    "Could not reach the backend.".to_string(),
                );
            }
        }
    });

    // Suppress unused variable warning for `action`
    let _ = &mut action;
}

// ── Load helper ──────────────────────────────────────────────────────────────

/// Fetches capabilities from the backend and writes them into the `caps`
/// signal. Clears `error` on success, sets it on failure. Advances `seq` on
/// success so stale responses can be detected (future-proofing).
fn load_capabilities(
    caps: Signal<Vec<CapabilitySummary>>,
    loading: Signal<bool>,
    error: Signal<Option<String>>,
    seq: Signal<u64>,
) {
    loading.set(true);
    error.set(None);

    spawn_local(async move {
        let args = to_value(&serde_json::json!({})).unwrap();
        let result = crate::ipc::tauri_invoke_safe("capability_list_with_state", args).await;

        match result {
            Some(Ok(val)) => {
                match serde_wasm_bindgen::from_value::<Vec<CapabilitySummary>>(val) {
                    Ok(list) => {
                        caps.set(list);
                        error.set(None);
                        seq.with_mut(|s| *s = s.wrapping_add(1));
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to parse capabilities: {e}")));
                    }
                }
            }
            Some(Err(e)) => {
                error.set(Some(format!("Backend error: {e:?}")));
            }
            None => {
                error.set(Some(
                    "Could not contact the Tauri backend. Ensure Nabu is running.".to_string(),
                ));
            }
        }
        loading.set(false);
    });
}
