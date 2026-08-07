//! Typed frontend event model.
//!
//! Defines the strongly-typed structures delivered to application components.
//! Event *payloads* reuse the backend `PipelineEvent` enum from `nabu-core` so
//! there is a single canonical serialized representation of every platform
//! event — the frontend never invents its own copy of an event shape.
//!
//! The event *kind* is modelled as a typed enum ([`FrontendEventKind`]) that
//! maps 1:1 onto the backend `EventBus` kind constants
//! (`nabu_core::event_bus::kinds`). Typing the kinds at the subscription
//! boundary keeps new event kinds an explicit, auditable change (mirroring the
//! backend bridge's `ALL_EVENT_KINDS` list) and prevents typos in subscription
//! strings.

use nabu_core::event_bus::kinds;
use nabu_core::event_bus::PipelineEvent;
use serde::Deserialize;

/// Re-export of the backend `EventBus` kind-string constants.
///
/// Kept available so callers can subscribe with a raw `&str` (e.g. for
/// forward-compatible "all events" inspection) while still reusing the
/// canonical kind names defined by the backend `EventBus`.
pub use nabu_core::event_bus::kinds as raw_kinds;

/// The single Tauri channel the backend `EventBusBridge` broadcasts on.
///
/// This constant mirrors `FRONTEND_EVENT_CHANNEL` in
/// `src-tauri/src/event_bridge.rs`. The frontend crate cannot depend on the
/// Tauri host crate, so the value is duplicated here by contract; the two must
/// stay in sync.
pub const FRONTEND_EVENT_CHANNEL: &str = "nabu-event";

/// Every platform event kind the frontend can subscribe to.
///
/// This is a closed, typed mirror of the backend `EventBus` kind constants.
/// When the backend adds a new kind to its bridge list, this enum must be
/// extended to match — keeping the set of deliverable events auditable and
/// typo-proof at the subscription boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrontendEventKind {
    /// A new item has been captured and enqueued.
    ItemCaptured,
    /// Processing of an item has started.
    ItemProcessingStarted,
    /// Processing progress update (0.0–1.0).
    ItemProcessingProgress,
    /// Processing completed successfully.
    ItemProcessingCompleted,
    /// Processing failed.
    ItemProcessingFailed,
    /// Item has been stored permanently.
    ItemStored,
    /// The search index has been updated.
    IndexUpdated,
    /// The knowledge graph has been updated.
    GraphUpdated,
    /// Item has been cancelled.
    ItemCancelled,
    /// Item has been retried.
    ItemRetried,
    /// A capability was enabled or disabled at runtime.
    CapabilityStateChanged,
    /// A synchronization folder's status has changed.
    SyncStatusChanged,
}

impl FrontendEventKind {
    /// The canonical kind string understood by the backend `EventBus`.
    pub const fn as_str(&self) -> &'static str {
        match self {
            FrontendEventKind::ItemCaptured => kinds::ITEM_CAPTURED,
            FrontendEventKind::ItemProcessingStarted => kinds::ITEM_PROCESSING_STARTED,
            FrontendEventKind::ItemProcessingProgress => kinds::ITEM_PROCESSING_PROGRESS,
            FrontendEventKind::ItemProcessingCompleted => kinds::ITEM_PROCESSING_COMPLETED,
            FrontendEventKind::ItemProcessingFailed => kinds::ITEM_PROCESSING_FAILED,
            FrontendEventKind::ItemStored => kinds::ITEM_STORED,
            FrontendEventKind::IndexUpdated => kinds::INDEX_UPDATED,
            FrontendEventKind::GraphUpdated => kinds::GRAPH_UPDATED,
            FrontendEventKind::ItemCancelled => kinds::ITEM_CANCELLED,
            FrontendEventKind::ItemRetried => kinds::ITEM_RETRIED,
            FrontendEventKind::CapabilityStateChanged => kinds::CAPABILITY_STATE_CHANGED,
            FrontendEventKind::SyncStatusChanged => kinds::SYNC_STATUS_CHANGED,
        }
    }

    /// Parse a kind string back into the typed enum. Returns `None` for
    /// unknown kinds (which are logged and ignored by the dispatcher rather
    /// than crashing the frontend).
    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            kinds::ITEM_CAPTURED => FrontendEventKind::ItemCaptured,
            kinds::ITEM_PROCESSING_STARTED => FrontendEventKind::ItemProcessingStarted,
            kinds::ITEM_PROCESSING_PROGRESS => FrontendEventKind::ItemProcessingProgress,
            kinds::ITEM_PROCESSING_COMPLETED => FrontendEventKind::ItemProcessingCompleted,
            kinds::ITEM_PROCESSING_FAILED => FrontendEventKind::ItemProcessingFailed,
            kinds::ITEM_STORED => FrontendEventKind::ItemStored,
            kinds::INDEX_UPDATED => FrontendEventKind::IndexUpdated,
            kinds::GRAPH_UPDATED => FrontendEventKind::GraphUpdated,
            kinds::ITEM_CANCELLED => FrontendEventKind::ItemCancelled,
            kinds::ITEM_RETRIED => FrontendEventKind::ItemRetried,
            kinds::CAPABILITY_STATE_CHANGED => FrontendEventKind::CapabilityStateChanged,
            kinds::SYNC_STATUS_CHANGED => FrontendEventKind::SyncStatusChanged,
            _ => return None,
        })
    }

    /// All known frontend event kinds (mirrors the backend bridge's
    /// `ALL_EVENT_KINDS`). Used for diagnostics and completeness checks.
    pub const ALL: &'static [FrontendEventKind] = &[
        FrontendEventKind::ItemCaptured,
        FrontendEventKind::ItemProcessingStarted,
        FrontendEventKind::ItemProcessingProgress,
        FrontendEventKind::ItemProcessingCompleted,
        FrontendEventKind::ItemProcessingFailed,
        FrontendEventKind::ItemStored,
        FrontendEventKind::IndexUpdated,
        FrontendEventKind::GraphUpdated,
        FrontendEventKind::ItemCancelled,
        FrontendEventKind::ItemRetried,
        FrontendEventKind::CapabilityStateChanged,
        FrontendEventKind::SyncStatusChanged,
    ];
}

impl std::fmt::Display for FrontendEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A platform event as received and understood by the frontend.
///
/// Produced by deserializing the `FrontendEvent` envelope emitted by the
/// backend `EventBusBridge`. The inner `payload` is the strongly-typed
/// `PipelineEvent` — callers pattern-match on it instead of poking at raw
/// JSON.
#[derive(Debug, Clone)]
pub struct FrontendEvent {
    /// Typed event kind (routes the event to the right subscribers).
    pub kind: FrontendEventKind,
    /// ISO-8601 timestamp the platform produced the event, if present.
    pub timestamp: Option<String>,
    /// The strongly-typed payload (reuses the backend `PipelineEvent` model).
    pub payload: PipelineEvent,
}

/// Wire format of the envelope emitted by `EventBusBridge::forward_to_tauri`.
///
/// Mirrors `src-tauri/src/event_bridge.rs::FrontendEvent` exactly:
/// `{ event_type, timestamp, payload }`.
#[derive(Deserialize)]
pub(crate) struct RawFrontendEvent {
    pub event_type: String,
    #[serde(default)]
    pub timestamp: Option<String>,
    pub payload: serde_json::Value,
}

/// Errors that can occur while turning a raw Tauri event into a
/// [`FrontendEvent`]. All are recoverable — the dispatcher logs and skips the
/// offending event rather than ever panicking.
#[derive(Debug)]
pub enum EventError {
    /// A kind string with no matching [`FrontendEventKind`].
    UnknownKind(String),
    /// The envelope or typed payload failed to deserialize.
    MalformedPayload(String),
    /// The `payload` field could not be read from the JS event object.
    PayloadExtraction(String),
    /// Tauri's `listen` call failed (or Tauri is unavailable).
    TauriListen(String),
}

impl std::fmt::Display for EventError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            EventError::UnknownKind(k) => write!(f, "unknown event kind: {k}"),
            EventError::MalformedPayload(msg) => write!(f, "malformed event payload: {msg}"),
            EventError::PayloadExtraction(msg) => write!(f, "could not extract payload: {msg}"),
            EventError::TauriListen(msg) => write!(f, "tauri listen failed: {msg}"),
        }
    }
}

impl std::error::Error for EventError {}

/// Convert a raw envelope into a typed [`FrontendEvent`].
///
/// Pure Rust — operates on an already-parsed [`RawFrontendEvent`]. Kept
/// separate from the JS-value handling in [`crate::events::bindings`] so the
/// deserialization logic is trivially unit-testable without a JS runtime.
pub(crate) fn parse_raw(raw: RawFrontendEvent) -> Result<FrontendEvent, EventError> {
    let kind = FrontendEventKind::from_str(&raw.event_type)
        .ok_or_else(|| EventError::UnknownKind(raw.event_type.clone()))?;

    let payload = serde_json::from_value::<PipelineEvent>(raw.payload)
        .map_err(|e| EventError::MalformedPayload(format!("typed payload: {e}")))?;

    Ok(FrontendEvent {
        kind,
        timestamp: raw.timestamp,
        payload,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a `RawFrontendEvent`-compatible envelope and parse it, exercising
    /// the same deserialization path the live JS listener uses.
    fn parse_envelope(kind: &str, payload: serde_json::Value) -> Result<FrontendEvent, EventError> {
        let envelope = serde_json::json!({
            "event_type": kind,
            "timestamp": "2024-01-01T00:00:00Z",
            "payload": payload,
        });
        let raw: RawFrontendEvent =
            serde_json::from_value(envelope).expect("envelope must deserialize");
        parse_raw(raw)
    }

    #[test]
    fn kind_round_trips_all_variants() {
        for &kind in FrontendEventKind::ALL {
            let s = kind.as_str();
            let back = FrontendEventKind::from_str(s);
            assert_eq!(back, Some(kind), "kind did not round-trip: {s}");
        }
        assert_eq!(FrontendEventKind::from_str("nope.unknown"), None);
    }

    #[test]
    fn item_stored_envelope_parses() {
        // `payload` mirrors `serde_json::to_value(PipelineEvent)` — i.e. the
        // externally-tagged enum form emitted by the backend bridge.
        let payload = serde_json::json!({
            "ItemStored": {
                "object_id": "12345678-1234-1234-1234-123456789abc",
                "vault_path": "notes/foo.md",
                "object_type": "Note",
                "timestamp": "2024-01-01T00:00:00Z",
            }
        });
        let event = parse_envelope(kinds::ITEM_STORED, payload).unwrap();
        assert_eq!(event.kind, FrontendEventKind::ItemStored);
        assert_eq!(event.timestamp.as_deref(), Some("2024-01-01T00:00:00Z"));
        assert!(matches!(event.payload, PipelineEvent::ItemStored(_)));
    }

    #[test]
    fn capability_state_changed_parses() {
        let payload = serde_json::json!({
            "CapabilityStateChanged": {
                "capability_id": "capture:file",
                "enabled": true,
                "timestamp": "2024-01-01T00:00:00Z",
            }
        });
        let event = parse_envelope(kinds::CAPABILITY_STATE_CHANGED, payload).unwrap();
        assert_eq!(event.kind, FrontendEventKind::CapabilityStateChanged);
        assert!(matches!(
            event.payload,
            PipelineEvent::CapabilityStateChanged(_)
        ));
    }

    #[test]
    fn sync_status_changed_parses() {
        let payload = serde_json::json!({
            "Sync": {
                "sync_id": "550e8400-e29b-41d4-a716-446655440000",
                "folder_id": "folder-abc",
                "provider_id": "syncthing",
                "previous_status": "idle",
                "current_status": "syncing",
                "progress": null,
                "error": null,
                "timestamp": "2024-01-01T00:00:00Z",
            }
        });
        let event = parse_envelope(kinds::SYNC_STATUS_CHANGED, payload).unwrap();
        assert_eq!(event.kind, FrontendEventKind::SyncStatusChanged);
        assert!(matches!(event.payload, PipelineEvent::Sync(_)));
    }

    #[test]
    fn unknown_kind_is_rejected() {
        let event = parse_envelope("platform.made.up", serde_json::json!({}));
        assert!(matches!(event, Err(EventError::UnknownKind(_))));
    }

    #[test]
    fn malformed_payload_is_rejected() {
        // A `payload` that does not match any `PipelineEvent` variant.
        let event = parse_envelope(kinds::ITEM_STORED, serde_json::json!("not-an-object"));
        assert!(matches!(event, Err(EventError::MalformedPayload(_))));
    }

    #[test]
    fn timestamp_is_optional() {
        // Envelopes without a timestamp are still valid.
        let raw = RawFrontendEvent {
            event_type: kinds::ITEM_STORED.to_string(),
            timestamp: None,
            payload: serde_json::json!({
                "ItemStored": {
                    "object_id": "12345678-1234-1234-1234-123456789abc",
                    "vault_path": "notes/foo.md",
                    "object_type": "Note",
                    "timestamp": "2024-01-01T00:00:00Z",
                }
            }),
        };
        let event = parse_raw(raw).unwrap();
        assert!(event.timestamp.is_none());
    }
}
