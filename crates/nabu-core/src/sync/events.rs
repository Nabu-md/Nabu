//! # Synchronization Event Pipeline
//!
//! Strongly-typed event types and the publishing helper that every
//! synchronization provider uses to report status changes through the existing
//! [`EventBus`].
//!
//! ## Architecture
//!
//! ```text
//! Synchronization Provider
//!     │  creates SyncStatusChanged
//!     ▼
//! SyncStatusChanged  ──▶  publish_sync_status_changed()
//!     │  wraps in PipelineEvent::Sync(...) + EventBus::publish by kind string
//!     ▼
//! EventBus<PipelineEvent>   (single source of truth for all platform events)
//!     │  subscribes "sync.status.changed" kind
//!     ▼
//! EventBusBridge subscriber  (src-tauri/src/event_bridge.rs)
//!     │  serializes + emit_str("nabu-event")
//!     ▼
//! Tauri / nabu-event channel
//!     │  listen("nabu-event")
//!     ▼
//! Frontend EventService  (crates/nabu-ui/src/events/)
//!     │  dispatches to typed subscribers
//!     ▼
//! Application Components
//! ```
//!
//! ## Single EventBus principle
//!
//! This module does **not** create a separate event bus, channel, or
//! dispatcher. Sync status changes are transported alongside pipeline, plugin,
//! diagnostic, process-supervision, and capability events through the **same**
//! `EventBus<PipelineEvent>`. The EventBus dispatches by a *kind string*, so
//! subscribers register for `"sync.status.changed"` just as they register for
//! `"item.stored"` or `"plugin.loaded"`.
//!
//! ## Publishing flow
//!
//! 1. A provider constructs a [`SyncStatusChanged`] with the folder ID,
//!    previous status, current status, optional progress, and its
//!    `provider_id`.
//! 2. The provider calls [`publish_sync_status_changed`], passing the
//!    `EventBus<PipelineEvent>` handle and the event.
//! 3. The helper wraps the event in `PipelineEvent::Sync(...)` and calls
//!    `EventBus::publish` under the
//!    [`SYNC_STATUS_CHANGED`](crate::event_bus::kinds::SYNC_STATUS_CHANGED)
//!    kind string.
//! 4. All subscribers registered for that kind are invoked (collected and
//!    dispatched outside the EventBus lock, so handlers may publish additional
//!    events without deadlocking — the same guarantee as every other event
//!    type).
//!
//! ## Frontend forwarding
//!
//! The EventBus→Tauri bridge (`src-tauri/src/event_bridge.rs`) subscribes to
//! `SYNC_STATUS_CHANGED` among its list of known kinds. When a sync event is
//! published, the bridge serializes it into a `FrontendEvent` envelope
//! (`{ event_type, timestamp, payload }`) and broadcasts it on the
//! `nabu-event` channel. The frontend's `EventService` deserializes the
//! envelope back into a typed `PipelineEvent::Sync(...)` and fans it out to
//! components.
//!
//! ## Extension guidance for future providers
//!
//! Every future synchronization provider — Syncthing, iCloud, Dropbox, Git,
//! WebDAV, OneDrive, Google Drive, or any custom backend — should:
//!
//! 1. Hold an `Arc<EventBus<PipelineEvent>>` (obtained from the
//!    `ApplicationContext` at startup).
//! 2. Construct a [`SyncStatusChanged`] whenever its internal state changes.
//! 3. Call [`publish_sync_status_changed`] to publish the event.
//!
//! No new event types, bus wiring, or IPC transport are needed — the
//! provider-agnostic pipeline is established here. If a *new* kind of sync
//! event is needed (e.g. a `SyncConflictDetected` event), add it as a new
//! variant of the sync event enum **without adding a new bus** — reuse the
//! same `PipelineEvent::Sync(...)` wrapper and the existing kind-registration
//! pattern (add a kind constant, a `kind()` match arm, and the kind to the
//! bridge's `ALL_EVENT_KINDS` list).
//!
//! ## Thread safety
//!
//! [`SyncStatusChanged`] is a plain data type (`Clone`, `Send`, `Sync`). It
//! contains no interior mutability and no shared state. Multiple providers
//! running on different threads or async tasks can publish concurrently —
//! the `EventBus` wraps its subscriber list in `Arc<Mutex<…>>` and releases
//! the lock before invoking any handler, so concurrent publication is safe.
//! Event payloads are passed by reference to handlers, who clone what they
//! need.
//!
//! ## Serialization
//!
//! All sync event types derive [`Serialize`] and [`Deserialize`].
//! [`SyncStatusChanged`] uses `#[serde(default)]` so that future fields can be
//! added without breaking deserialization of payloads serialized by older
//! providers. The `SyncStatus` and `SyncProgress` fields are reused from the
//! existing sync domain models — future providers require no additional
//! serialization work.
//!
//! ## Error handling
//!
//! Publication never panics. The [`publish_sync_status_changed`] helper logs
//! a warning (via `tracing::warn`) if the event fails validation, so
//! malformed events are surfaced without crashing the provider. Serialization
//! failures during IPC forwarding are handled by the bridge module (which
//! logs and skips the offending event).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::event_bus::kinds;
use crate::event_bus::{EventBus, PipelineEvent};
use crate::sync::error::{SyncError, SyncResult};
use crate::sync::progress::SyncProgress;
use crate::sync::status::SyncStatus;

// ---------------------------------------------------------------------------
// SyncStatusChanged
// ---------------------------------------------------------------------------

/// A synchronization state-change event published through the [`EventBus`].
///
/// This is the **single canonical event** that every synchronization provider
/// (Syncthing, iCloud, Git, WebDAV, etc.) emits whenever the synchronization
/// status of a folder changes. Providers wrap this event in
/// `PipelineEvent::Sync(...)` via [`publish_sync_status_changed`] and publish
/// it under the
/// [`SYNC_STATUS_CHANGED`](crate::event_bus::kinds::SYNC_STATUS_CHANGED)
/// kind.
///
/// # Fields
///
/// | Field               | Meaning                                          |
/// |---------------------|--------------------------------------------------|
/// | `sync_id`           | Unique identifier for this status transition    |
/// | `folder_id`         | The folder whose status changed                 |
/// | `provider_id`       | Originating provider (opaque, e.g. `"syncthing"`)|
/// | `previous_status`   | The folder's status before this change          |
/// | `current_status`    | The folder's status after this change           |
/// | `progress`          | Optional progress snapshot (present during sync)|
/// | `error`             | Error message if the status is [`SyncStatus::Error`] |
/// | `timestamp`         | When the event was produced                     |
///
/// # Provider independence
///
/// The event is provider-agnostic: it carries the `provider_id` as an opaque
/// string so the platform and frontend can attribute the event without
/// hard-coding any specific backend. The platform never matches on the value
/// — providers do (if they need to).
///
/// # Future compatibility
///
/// All fields use `#[serde(default)]` so that future phases can add metadata
/// (e.g. `conflict_detected`, `bandwidth_usage`, `sync_direction`) without
/// breaking deserialization of payloads serialized by older providers.
///
/// [`EventBus`]: crate::event_bus::EventBus
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncStatusChanged {
    /// Unique identifier for this particular status transition.
    ///
    /// Generated by the provider when the event is created. This distinguishes
    /// individual transitions so subscribers can deduplicate or correlate
    /// follow-up events.
    pub sync_id: Uuid,

    /// The identifier of the folder whose synchronization status changed.
    ///
    /// This corresponds to `SyncFolder::id` but is stored as a `String` so
    /// that providers which use non-UUID folder identifiers (e.g. Syncthing's
    /// device+folder ID strings) can still emit events without coercion.
    /// Providers using `SyncFolder` should set this to `folder.id.to_string()`.
    pub folder_id: String,

    /// The provider that originated this event.
    ///
    /// An opaque identifier (e.g. `"syncthing"`, `"icloud"`, `"git"`,
    /// `"webdav"`). The platform does not interpret this value — it is
    /// forwarded to the frontend for attribution and display.
    pub provider_id: String,

    /// The folder's synchronization status **before** this change.
    ///
    /// `None` when the provider has no prior status (e.g. the folder was just
    /// discovered).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_status: Option<SyncStatus>,

    /// The folder's synchronization status **after** this change.
    pub current_status: SyncStatus,

    /// Optional progress snapshot, present when a sync operation is in flight.
    ///
    /// This carries the provider's last known [`SyncProgress`] at the moment
    /// the status changed. When `current_status` is not [`SyncStatus::Syncing`],
    /// this is typically `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<SyncProgress>,

    /// Error message when `current_status` is [`SyncStatus::Error`].
    ///
    /// Cleared (set to `None`) when the status transitions away from
    /// `Error`. Providers should populate this with a human-readable message
    /// that can be displayed to the user.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// When the event was produced.
    ///
    /// Used by the EventBus→Tauri bridge to attach a top-level timestamp to
    /// the frontend event envelope, and by subscribers for ordering and
    /// diagnostics.
    pub timestamp: DateTime<Utc>,
}

impl Default for SyncStatusChanged {
    fn default() -> Self {
        SyncStatusChanged {
            sync_id: Uuid::nil(),
            folder_id: String::new(),
            provider_id: String::new(),
            previous_status: None,
            current_status: SyncStatus::NotConfigured,
            progress: None,
            error: None,
            timestamp: Utc::now(),
        }
    }
}

impl SyncStatusChanged {
    /// Creates a new `SyncStatusChanged` event.
    ///
    /// The `sync_id` is auto-generated as a v4 UUID. The `previous_status`
    /// is optional — omit it (or pass `None`) when there is no prior state
    /// (e.g. a newly discovered folder).
    ///
    /// # Example
    ///
    /// ```ignore
    /// use nabu_core::sync::{SyncStatusChanged, SyncStatus, publish_sync_status_changed};
    /// use nabu_core::event_bus::{EventBus, PipelineEvent};
    ///
    /// let event = SyncStatusChanged::new(
    ///     "folder-abc",
    ///     "syncthing",
    ///     SyncStatus::Idle,                         // current status
    /// ).with_previous(SyncStatus::Syncing);        // previous status
    ///
    /// publish_sync_status_changes(&event_bus, &event);
    /// ```
    pub fn new(
        folder_id: impl Into<String>,
        provider_id: impl Into<String>,
        current_status: SyncStatus,
    ) -> Self {
        SyncStatusChanged {
            sync_id: Uuid::new_v4(),
            folder_id: folder_id.into(),
            provider_id: provider_id.into(),
            previous_status: None,
            current_status,
            progress: None,
            error: None,
            timestamp: Utc::now(),
        }
    }

    /// Sets the previous status before this transition.
    pub fn with_previous(mut self, status: SyncStatus) -> Self {
        self.previous_status = Some(status);
        self
    }

    /// Sets optional progress information for this transition.
    pub fn with_progress(mut self, progress: SyncProgress) -> Self {
        self.progress = Some(progress);
        self
    }

    /// Sets an error message (typically used when transitioning to
    /// [`SyncStatus::Error`]).
    pub fn with_error(mut self, err: impl Into<String>) -> Self {
        self.error = Some(err.into());
        self
    }

    /// Sets the timestamp for this event.
    pub fn with_timestamp(mut self, ts: DateTime<Utc>) -> Self {
        self.timestamp = ts;
        self
    }

    /// Returns `true` if the event's `error` field is set (i.e. the event
    /// carries an error message worth surfacing).
    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }

    /// Validates that the event has the minimum required fields populated.
    ///
    /// # Invariants
    ///
    /// - `folder_id` must be non-empty.
    /// - `provider_id` must be non-empty.
    /// - `sync_id` must not be nil (i.e. not `Uuid::nil()`).
    /// - If `progress` is present, it must pass its own `validate()`.
    ///
    /// Returns [`SyncError`] on failure — never panics.
    pub fn validate(&self) -> SyncResult<()> {
        if self.folder_id.is_empty() {
            return Err(SyncError::invalid_folder(
                self.folder_id.clone(),
                "folder_id must not be empty",
            ));
        }

        if self.provider_id.is_empty() {
            return Err(SyncError::invalid_folder(
                self.folder_id.clone(),
                "provider_id must not be empty",
            ));
        }

        if self.sync_id == Uuid::nil() {
            return Err(SyncError::invalid_folder(
                self.folder_id.clone(),
                "sync_id must not be nil (use SyncStatusChanged::new)",
            ));
        }

        if let Some(ref progress) = self.progress {
            progress.validate()?;
        }

        Ok(())
    }

    /// Returns the event kind string used for EventBus subscription.
    ///
    /// This matches the constant
    /// [`kinds::SYNC_STATUS_CHANGED`](crate::event_bus::kinds::SYNC_STATUS_CHANGED).
    pub fn kind(&self) -> &'static str {
        kinds::SYNC_STATUS_CHANGED
    }

    /// Returns when this event was produced.
    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    /// Convert this event into a [`PipelineEvent`] for EventBus publication.
    ///
    /// The returned `PipelineEvent::Sync(...)` is published to the EventBus
    /// under the event's [`kind`](Self::kind).
    pub fn to_pipeline_event(&self) -> PipelineEvent {
        PipelineEvent::Sync(self.clone())
    }
}

// ---------------------------------------------------------------------------
// Publishing helper
// ---------------------------------------------------------------------------

/// Publish a synchronization status-change event through the [`EventBus`].
///
/// This is the canonical publishing entry point for all synchronization
/// providers. It validates the event and, if valid, wraps it in
/// `PipelineEvent::Sync(...)` and publishes it under the
/// [`SYNC_STATUS_CHANGED`](crate::event_bus::kinds::SYNC_STATUS_CHANGED)
/// kind. If validation fails, a warning is logged and the event is silently
/// dropped — publication never panics.
///
/// # Arguments
///
/// * `event_bus` — a reference to the shared `EventBus<PipelineEvent>`.
/// * `event` — the `SyncStatusChanged` event to publish.
///
/// # Example
///
/// ```ignore
/// use nabu_core::event_bus::{EventBus, PipelineEvent};
/// use nabu_core::sync::{SyncStatusChanged, SyncStatus, publish_sync_status_changed};
///
/// let event_bus = EventBus::<PipelineEvent>::new();
///
/// let event = SyncStatusChanged::new(
///     "folder-abc",
///     "syncthing",
///     SyncStatus::Syncing,
/// ).with_previous(SyncStatus::Idle);
///
/// publish_sync_status_changed(&event_bus, &event);
/// ```
///
/// [`EventBus`]: crate::event_bus::EventBus
pub fn publish_sync_status_changed(
    event_bus: &EventBus<PipelineEvent>,
    event: &SyncStatusChanged,
) {
    if let Err(e) = event.validate() {
        tracing::warn!(
            error = %e,
            kind = kinds::SYNC_STATUS_CHANGED,
            "Dropping malformed SyncStatusChanged event (validation failed)"
        );
        return;
    }

    let kind = event.kind();
    let pipeline_event = event.to_pipeline_event();
    event_bus.publish(kind, &pipeline_event);
}

// ---------------------------------------------------------------------------
// SyncSubscriber
// ---------------------------------------------------------------------------

/// A subscriber that listens for [`SyncStatusChanged`] events on the
/// [`EventBus`] and forwards them to an IPC bridge callback.
///
/// The subscriber's responsibilities are deliberately narrow:
///
/// 1. **Receive** `SyncStatusChanged` events from the EventBus (by subscribing
///    to the `SYNC_STATUS_CHANGED` kind).
/// 2. **Validate** each event payload — malformed events are logged and
///    skipped, never forwarded.
/// 3. **Forward** valid events to a caller-supplied callback (which is
///    typically the IPC bridge's `forward_to_tauri` function).
/// 4. **Avoid duplicate forwarding** — the subscriber is registered exactly
///    once per `register` call; the callback is invoked at most once per
///    published event.
///
/// # Thread safety
///
/// The subscriber holds the EventBus subscription handle and an `Arc` to the
/// forwarding callback. The callback is `Fn + Send + Sync + 'static`, so it
/// can be invoked from any thread. The subscriber itself is `Send + Sync`.
///
/// # Example
///
/// ```ignore
/// use nabu_core::sync::SyncSubscriber;
/// use nabu_core::event_bus::{EventBus, PipelineEvent};
///
/// let bus = EventBus::<PipelineEvent>::new();
///
/// let subscriber = SyncSubscriber::new(|event| {
///     // Forward to IPC bridge...
/// });
/// subscriber.register(&bus);
/// ```
pub struct SyncSubscriber {
    callback: Arc<dyn Fn(&SyncStatusChanged) + Send + Sync>,
}

impl SyncSubscriber {
    /// Creates a new `SyncSubscriber` with the given forwarding callback.
    ///
    /// The callback is invoked once per valid `SyncStatusChanged` event
    /// published on the EventBus. It receives a reference to the validated
    /// event and may forward it to the IPC bridge, log it, or perform any
    /// other side effect.
    pub fn new(callback: Arc<dyn Fn(&SyncStatusChanged) + Send + Sync>) -> Self {
        SyncSubscriber { callback }
    }

    /// Registers this subscriber on the given EventBus.
    ///
    /// Subscribes to the `SYNC_STATUS_CHANGED` kind. The returned
    /// [`Subscription`](crate::event_bus::Subscription) handle can be used
    /// to unsubscribe later, though in practice the subscription lives for
    /// the lifetime of the EventBus.
    ///
    /// # Errors
    ///
    /// Returns `Err(SyncError)` if subscription fails (e.g. the EventBus is
    /// in an inconsistent state — this should never happen in practice).
    pub fn register(&self, bus: &EventBus<PipelineEvent>) -> SyncResult<crate::event_bus::Subscription> {
        let callback = self.callback.clone();
        let subscription = bus.subscribe(kinds::SYNC_STATUS_CHANGED, move |event: &PipelineEvent| {
            if let PipelineEvent::Sync(sync_event) = event {
                // Validate the event payload before forwarding.
                if sync_event.validate().is_ok() {
                    callback(sync_event);
                } else {
                    tracing::warn!(
                        event_kind = kinds::SYNC_STATUS_CHANGED,
                        "Dropping invalid SyncStatusChanged event in subscriber"
                    );
                }
            }
        });

        Ok(subscription)
    }
}

impl<F> From<F> for SyncSubscriber
where
    F: Fn(&SyncStatusChanged) + Send + Sync + 'static,
{
    fn from(f: F) -> Self {
        Self::new(Arc::new(f))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn sync_model_event_new_sets_required_fields() {
        let event = SyncStatusChanged::new("folder-abc", "syncthing", SyncStatus::Syncing);
        assert_ne!(event.sync_id, Uuid::nil());
        assert_eq!(event.folder_id, "folder-abc");
        assert_eq!(event.provider_id, "syncthing");
        assert_eq!(event.current_status, SyncStatus::Syncing);
        assert!(event.previous_status.is_none());
        assert!(event.progress.is_none());
        assert!(event.error.is_none());
        assert!(event.timestamp <= Utc::now());
    }

    #[test]
    fn sync_model_event_builder_methods() {
        let progress = SyncProgress::new("uploading").with_status(SyncStatus::Syncing);
        let event = SyncStatusChanged::new("f1", "icloud", SyncStatus::UpToDate)
            .with_previous(SyncStatus::Syncing)
            .with_progress(progress.clone())
            .with_error("connection lost")
            .with_timestamp(Utc::now());

        assert_eq!(event.previous_status, Some(SyncStatus::Syncing));
        assert_eq!(event.progress, Some(progress));
        assert_eq!(event.error.as_deref(), Some("connection lost"));
        assert!(event.has_error());
    }

    #[test]
    fn sync_model_event_default() {
        let event = SyncStatusChanged::default();
        assert_eq!(event.sync_id, Uuid::nil());
        assert!(event.folder_id.is_empty());
        assert!(event.provider_id.is_empty());
        assert_eq!(event.current_status, SyncStatus::NotConfigured);
        assert!(event.previous_status.is_none());
        assert!(event.progress.is_none());
        assert!(event.error.is_none());
    }

    #[test]
    fn sync_model_event_has_error() {
        let with_error = SyncStatusChanged::new("f1", "s", SyncStatus::Error).with_error("fail");
        assert!(with_error.has_error());

        let without_error = SyncStatusChanged::new("f1", "s", SyncStatus::Idle);
        assert!(!without_error.has_error());
    }

    #[test]
    fn sync_model_event_kind_matches_constant() {
        let event = SyncStatusChanged::new("f1", "s", SyncStatus::Idle);
        assert_eq!(event.kind(), kinds::SYNC_STATUS_CHANGED);
    }

    #[test]
    fn sync_model_event_validate_ok() {
        let event = SyncStatusChanged::new("f1", "syncthing", SyncStatus::Idle);
        assert!(event.validate().is_ok());
    }

    #[test]
    fn sync_model_event_validate_rejects_empty_folder_id() {
        let event = SyncStatusChanged::default()
            .with_previous(SyncStatus::Idle);
        assert!(event.validate().is_err());
    }

    #[test]
    fn sync_model_event_validate_rejects_empty_provider_id() {
        let event = SyncStatusChanged::new("f1", "", SyncStatus::Idle);
        assert!(event.validate().is_err());
    }

    #[test]
    fn sync_model_event_validate_rejects_nil_sync_id() {
        let event = SyncStatusChanged {
            sync_id: Uuid::nil(),
            folder_id: "f1".to_string(),
            provider_id: "s".to_string(),
            current_status: SyncStatus::Idle,
            ..Default::default()
        };
        assert!(event.validate().is_err());
    }

    #[test]
    fn sync_model_event_validate_invalid_progress() {
        let bad_progress = SyncProgress::new("test").with_percentage(150.0);
        let event = SyncStatusChanged::new("f1", "s", SyncStatus::Syncing)
            .with_progress(bad_progress);
        assert!(event.validate().is_err());
    }

    #[test]
    fn sync_model_event_validate_ok_with_valid_progress() {
        let progress = SyncProgress::new("uploading").with_percentage(50.0);
        let event = SyncStatusChanged::new("f1", "s", SyncStatus::Syncing)
            .with_progress(progress);
        assert!(event.validate().is_ok());
    }

    #[test]
    fn sync_model_event_serialization_round_trip() {
        let progress = SyncProgress::new("uploading")
            .with_items(5, Some(10))
            .with_status(SyncStatus::Syncing);

        let event = SyncStatusChanged::new("folder-xyz", "syncthing", SyncStatus::Syncing)
            .with_previous(SyncStatus::Idle)
            .with_progress(progress.clone())
            .with_error("partial failure");

        let json = serde_json::to_string(&event).unwrap();
        let back: SyncStatusChanged = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn sync_model_event_forward_compatible() {
        let json = r#"{
            "sync_id": "550e8400-e29b-41d4-a716-446655440000",
            "folder_id": "folder-123",
            "provider_id": "syncthing",
            "previous_status": "idle",
            "current_status": "syncing",
            "progress": null,
            "error": null,
            "timestamp": "2024-01-01T00:00:00Z",
            "future_field": "ignored"
        }"#;
        let event: SyncStatusChanged = serde_json::from_str(json).unwrap();
        assert_eq!(event.folder_id, "folder-123");
        assert_eq!(event.provider_id, "syncthing");
        assert_eq!(event.current_status, SyncStatus::Syncing);
        assert_eq!(event.previous_status, Some(SyncStatus::Idle));
    }

    #[test]
    fn sync_model_event_empty_deserializes() {
        let event: SyncStatusChanged = serde_json::from_str("{}").unwrap();
        assert_eq!(event.current_status, SyncStatus::NotConfigured);
        assert!(event.folder_id.is_empty());
        assert!(event.provider_id.is_empty());
    }

    #[test]
    fn sync_model_event_timestamp_accessor() {
        let ts = Utc::now();
        let event = SyncStatusChanged::new("f1", "s", SyncStatus::Idle)
            .with_timestamp(ts);
        assert_eq!(event.timestamp(), ts);
    }

    #[test]
    fn sync_model_event_to_pipeline_event() {
        let event = SyncStatusChanged::new("f1", "s", SyncStatus::Idle);
        let pipeline = event.to_pipeline_event();
        assert!(matches!(pipeline, PipelineEvent::Sync(_)));
    }

    #[test]
    fn publish_sync_status_changed_delivers_to_subscriber() {
        let bus = EventBus::<PipelineEvent>::new();
        let received = Arc::new(AtomicUsize::new(0));

        let cb = received.clone();
        let sub = SyncSubscriber::new(Arc::new(move |_event: &SyncStatusChanged| {
            cb.fetch_add(1, Ordering::SeqCst);
        }));
        sub.register(&bus).unwrap();

        let event = SyncStatusChanged::new("folder-1", "syncthing", SyncStatus::Syncing)
            .with_previous(SyncStatus::Idle);
        publish_sync_status_changed(&bus, &event);

        assert_eq!(received.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn publish_sync_status_changed_drops_invalid_event() {
        let bus = EventBus::<PipelineEvent>::new();
        let received = Arc::new(AtomicUsize::new(0));

        let cb = received.clone();
        let sub = SyncSubscriber::new(Arc::new(move |_event: &SyncStatusChanged| {
            cb.fetch_add(1, Ordering::SeqCst);
        }));
        sub.register(&bus).unwrap();

        // Invalid: empty folder_id.
        let bad_event = SyncStatusChanged {
            sync_id: Uuid::nil(),
            folder_id: String::new(),
            provider_id: String::new(),
            current_status: SyncStatus::Syncing,
            ..Default::default()
        };
        publish_sync_status_changed(&bus, &bad_event);

        assert_eq!(received.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn sync_subscriber_validates_before_forwarding() {
        let bus = EventBus::<PipelineEvent>::new();
        let received = Arc::new(AtomicUsize::new(0));

        let cb = received.clone();
        let sub = SyncSubscriber::new(Arc::new(move |_event: &SyncStatusChanged| {
            cb.fetch_add(1, Ordering::SeqCst);
        }));
        sub.register(&bus).unwrap();

        // Valid event is forwarded.
        let good = SyncStatusChanged::new("f1", "s", SyncStatus::Idle);
        bus.publish(kinds::SYNC_STATUS_CHANGED, &PipelineEvent::Sync(good));

        // Invalid event (nil sync_id) is NOT forwarded by the subscriber.
        let bad = SyncStatusChanged {
            sync_id: Uuid::nil(),
            folder_id: "f1".to_string(),
            provider_id: "s".to_string(),
            current_status: SyncStatus::Idle,
            ..Default::default()
        };
        bus.publish(kinds::SYNC_STATUS_CHANGED, &PipelineEvent::Sync(bad));

        assert_eq!(received.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn sync_subscriber_can_be_constructed_from_closure() {
        let bus = EventBus::<PipelineEvent>::new();
        let received = Arc::new(AtomicUsize::new(0));

        let cb = received.clone();
        let sub = SyncSubscriber::from(move |_event: &SyncStatusChanged| {
            cb.fetch_add(1, Ordering::SeqCst);
        });
        sub.register(&bus).unwrap();

        let event = SyncStatusChanged::new("f1", "s", SyncStatus::Idle);
        publish_sync_status_changed(&bus, &event);
        assert_eq!(received.load(Ordering::SeqCst), 1);
    }
}
