//! EventBus → Tauri bridge.
//!
//! Forwards every published platform event from the internal [`EventBus`] to
//! the Tauri frontend over a single, unified channel: `nabu-event`.
//!
//! The [`EventBus`] remains the **single source of truth** for platform events.
//! This module only *observes* it and extends it with a frontend listener — it
//! never bypasses the EventBus and never publishes events itself.
//!
//! ## Architecture
//!
//! ```text
//! Platform Service
//!     ↓  publish(kind, PipelineEvent)
//! EventBus
//!     ↓  (one subscriber per known kind)
//! EventBusBridge subscriber  ←  this module
//!     ↓  emit_str("nabu-event", envelope)
//! Tauri
//!     ↓  listen("nabu-event")
//! Frontend
//! ```
//!
//! One handler is registered per known event kind (the EventBus dispatches by
//! kind string). All handlers share a single forwarding function, so there is no
//! duplicated serialization or forwarding logic. The registration is
//! idempotent — a process-level guard prevents duplicate listeners.

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Emitter};

use nabu_core::event_bus::{kinds, EventBus, PipelineEvent};
use nabu_core::registry::context::ApplicationContext;

/// The single unified channel every frontend subscriber listens on.
pub const FRONTEND_EVENT_CHANNEL: &str = "nabu-event";

/// Every known platform event kind subscribed to for frontend forwarding.
///
/// The EventBus only supports per-kind subscription, so the bridge subscribes
/// to the canonical set of platform event kinds. Adding a *new* event kind to
/// the platform therefore requires adding it here — this keeps the bridge an
/// explicit, auditable observer rather than silently dropping new event types.
const ALL_EVENT_KINDS: &[&str] = &[
    kinds::ITEM_CAPTURED,
    kinds::ITEM_PROCESSING_STARTED,
    kinds::ITEM_PROCESSING_PROGRESS,
    kinds::ITEM_PROCESSING_COMPLETED,
    kinds::ITEM_PROCESSING_FAILED,
    kinds::ITEM_STORED,
    kinds::INDEX_UPDATED,
    kinds::GRAPH_UPDATED,
    kinds::ITEM_CANCELLED,
    kinds::ITEM_RETRIED,
    kinds::CAPABILITY_STATE_CHANGED,
    // Shared plugin event contract kinds — forwarded to frontend for
    // plugin platform UI (plugin list, warnings, request/response, etc.)
    kinds::PLUGIN_LOADED,
    kinds::PLUGIN_UNLOADED,
    kinds::PLUGIN_REGISTERED,
    kinds::PLUGIN_UNREGISTERED,
    kinds::PLUGIN_STARTED,
    kinds::PLUGIN_STOPPED,
    kinds::CAPABILITY_REGISTERED,
    kinds::CAPABILITY_REMOVED,
    kinds::PLUGIN_WARNING,
    kinds::PLUGIN_ERROR,
    // Plugin request/response events (forwarded for plugin platform UI).
    kinds::PLUGIN_REQUEST,
    kinds::PLUGIN_RESPONSE,
    // Synchronization status-change events (forwarded for sync dashboard / UI).
    kinds::SYNC_STATUS_CHANGED,
    // --- Diagnostic event kinds ---
    // Published by the DiagnosticPlatform when editors request on-demand
    // diagnostics. These events enable async subscribers (background panels,
    // lint lists) to receive diagnostic updates without an explicit IPC request.
    kinds::DIAGNOSTIC_BATCH_PUBLISHED,
    kinds::DIAGNOSTIC_BATCH_CLEARED,
    kinds::DIAGNOSTIC_BATCH_REMOVED,
    // --- Conversation persistence event kinds ---
    // Forwarded so frontend UI components can react to conversation changes.
    kinds::THREAD_SAVED,
    kinds::THREAD_UPDATED,
    kinds::THREAD_DELETED,
];

/// A single structured payload broadcast on the `nabu-event` channel.
///
/// Carries enough envelope metadata (event type, timestamp) for frontend
/// consumers to route or timestamp the event without opening the payload,
/// while the full serialized `PipelineEvent` lives in `payload`.
#[derive(serde::Serialize)]
pub struct FrontendEvent {
    /// Canonical event kind (e.g. `"item.stored"`).
    pub event_type: String,
    /// ISO-8601 timestamp when the platform produced the event, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    /// The full serialized platform event.
    pub payload: serde_json::Value,
}

/// Process-wide guard ensuring the bridge is registered exactly once.
///
/// Prevents duplicate listeners if `register_event_bridge` is ever invoked
/// more than once (the EventBus cannot distinguish "the bridge subscriber"
/// from other subscribers, so a static flag is the only reliable guard).
static BRIDGE_REGISTERED: AtomicBool = AtomicBool::new(false);

/// Subscribes the application context's [`EventBus`] to forward all published
/// platform events to Tauri over the `nabu-event` channel.
///
/// This must be called **once** during startup (inside the Tauri `setup`
/// closure), before services begin publishing events. It is safe to call
/// multiple times — subsequent calls are detected and skipped.
pub fn register_event_bridge(ctx: &ApplicationContext, app_handle: AppHandle) {
    if BRIDGE_REGISTERED.swap(true, Ordering::SeqCst) {
        tracing::warn!("EventBus→Tauri bridge already registered; skipping");
        return;
    }

    let bus: &EventBus<PipelineEvent> = ctx.event_bus();

    for kind in ALL_EVENT_KINDS {
        // Each subscription captures its own clone of AppHandle (cheap — it
        // is a thin Arc-like handle). The EventBus retains the handler for
        // the lifetime of the bus, so the Subscription handle can be dropped
        // without unsubscribing (see EventBus::subscribe docs).
        let handle = app_handle.clone();
        let _subscription = bus.subscribe(kind, move |event: &PipelineEvent| {
            forward_to_tauri(&handle, event);
        });
    }

    tracing::info!(
        kinds = ALL_EVENT_KINDS.len(),
        channel = FRONTEND_EVENT_CHANNEL,
        "EventBus→Tauri bridge registered"
    );
}

/// Serializes a single platform event and emits it over `nabu-event`.
///
/// The event is serialized to a [`serde_json::Value`] exactly **once**; that
/// value is then embedded in the [`FrontendEvent`] envelope and stringified
/// for emission, avoiding duplicate serialization of the event payload.
fn forward_to_tauri(app: &AppHandle, event: &PipelineEvent) {
    let payload = match serde_json::to_value(event) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialize platform event for frontend");
            return;
        }
    };

    let envelope = FrontendEvent {
        event_type: event.kind().to_string(),
        timestamp: event.timestamp().map(|ts| ts.to_rfc3339()),
        payload,
    };

    let envelope_json = match serde_json::to_string(&envelope) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "Failed to serialize frontend event envelope");
            return;
        }
    };

    if let Err(e) = app.emit_str(FRONTEND_EVENT_CHANNEL, envelope_json) {
        tracing::error!(
            error = %e,
            event_type = %envelope.event_type,
            "Failed to emit nabu-event to frontend"
        );
    } else {
        tracing::trace!(event_type = %envelope.event_type, "Emitted nabu-event");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_event_serializes() {
        let envelope = FrontendEvent {
            event_type: "item.stored.test".to_string(),
            timestamp: Some("2024-01-01T00:00:00Z".to_string()),
            payload: serde_json::json!({ "hello": "world" }),
        };
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains("\"event_type\":\"item.stored.test\""));
        assert!(json.contains("\"timestamp\":\"2024-01-01T00:00:00Z\""));
        assert!(json.contains("\"payload\""));
        // Re-parse to confirm validity.
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["event_type"], "item.stored.test");
        assert_eq!(parsed["timestamp"], "2024-01-01T00:00:00Z");
        assert_eq!(parsed["payload"]["hello"], "world");
    }

    #[test]
    fn all_event_kinds_are_known_constants() {
        // Guard: every kind in the bridge list must be non-empty and
        // distinct (no accidental duplicates in the subscription list).
        let mut seen = std::collections::HashSet::new();
        for kind in ALL_EVENT_KINDS {
            assert!(!kind.is_empty(), "event kind must not be empty");
            assert!(seen.insert(*kind), "duplicate event kind in bridge: {}", kind);
        }
    }
}
