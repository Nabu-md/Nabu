//! # NotificationManager — EventBus → Toast pipeline
//!
//! Bridges platform events from the [`EventService`](crate::events) to the
//! existing [`ToastProvider`] toast infrastructure in [`feedback`].
//!
//! ## Architecture
//!
//! ```text
//! EventBus (backend) → nabu-event channel → EventService (frontend)
//!                                      → NotificationManager
//!                                      → ToastContext (use_toast)
//!                                      → ToastRegion renders
//! ```
//!
//! The `NotificationManager` is a zero-DOM component: it lives for the
//! lifetime of the app, subscribes to every known event kind via
//! [`use_event_listener`], maps each incoming event to a toast
//! (title + message + [`ToastKind`]), and pushes it through the shared
//! [`ToastContext`]. It also maintains a lightweight in-memory queue for
//! **deduplication** (coalescing rapid repeat events of the same kind) and
//! **expiry cleanup** (auto-pruning stale dedup entries).
//!
//! ## What it does NOT do
//!
//! - Desktop notifications (outside the toast region)
//! - Activity feed / notification history
//! - Progress bars or long-running operation progress
//! - Polling — purely event-driven via the EventBus
//!
//! ## Future extensibility
//!
//! The queue is designed so that adding:
//! - Action buttons (via `ToastAction`)
//! - Persistent notifications (via `persistent: true`)
//! - Notification history (via a retained log)
//! - Grouped notifications
//! - Notification preferences (suppression rules)
//!
//! …requires only extending the `event_to_toast` mapping and/or the
//! `NotificationConfig` table, without restructuring the dispatch pipeline.

use std::collections::HashMap;

use dioxus::prelude::*;

use crate::events::{use_event_listener, FrontendEvent, FrontendEventKind};
use crate::ui::feedback::{set_timeout, ToastContext, ToastKind, use_toast};
use nabu_core::event_bus::PipelineEvent;

// ─── Notification metadata ───────────────────────────────────────────

/// Static metadata describing how a given event kind should be surfaced as a
/// toast.  This is the configuration table that maps a platform event to:
///
/// - A **title** template (human-readable summary)
/// - A **default severity** (Success / Info / Warning / Error)
/// - Whether the toast should be **persistent** (no auto-dismiss)
/// - A **dedup key** — events sharing a key replace each other instead of
///   stacking (e.g. repeated "syncing" status updates)
///
/// Adding a new mapped event kind only requires extending [`NotificationConfig::for_kind`].
struct NotificationConfig {
    title: &'static str,
    severity: ToastKind,
    persistent: bool,
    dedup_key: Option<&'static str>,
}

impl NotificationConfig {
    /// Returns the notification configuration for a given event kind, or `None`
    /// if that kind should not produce a toast (e.g. progress updates that are
    /// too noisy on their own).
    fn for_kind(kind: FrontendEventKind) -> Option<Self> {
        match kind {
            FrontendEventKind::ItemStored => Some(Self {
                title: "Note saved",
                severity: ToastKind::Success,
                persistent: false,
                dedup_key: None,
            }),
            FrontendEventKind::ItemCaptured => Some(Self {
                title: "Item captured",
                severity: ToastKind::Info,
                persistent: false,
                dedup_key: None,
            }),
            FrontendEventKind::ItemProcessingCompleted => Some(Self {
                title: "Processing complete",
                severity: ToastKind::Success,
                persistent: false,
                dedup_key: None,
            }),
            FrontendEventKind::ItemProcessingFailed => Some(Self {
                title: "Processing failed",
                severity: ToastKind::Error,
                persistent: false,
                dedup_key: None,
            }),
            FrontendEventKind::ItemCancelled => Some(Self {
                title: "Item cancelled",
                severity: ToastKind::Warning,
                persistent: false,
                dedup_key: None,
            }),
            FrontendEventKind::ItemRetried => Some(Self {
                title: "Retrying",
                severity: ToastKind::Info,
                persistent: false,
                dedup_key: None,
            }),
            FrontendEventKind::CapabilityStateChanged => Some(Self {
                title: "Capability updated",
                severity: ToastKind::Info,
                persistent: false,
                dedup_key: None,
            }),
            FrontendEventKind::SyncStatusChanged => Some(Self {
                title: "Sync status changed",
                severity: ToastKind::Info,
                persistent: false,
                dedup_key: Some("sync.status"),
            }),
            // Events that are too frequent/noisy to toast individually:
            // progress updates, processing-started, and index/graph updates.
            FrontendEventKind::ItemProcessingProgress
            | FrontendEventKind::ItemProcessingStarted
            | FrontendEventKind::IndexUpdated
            | FrontendEventKind::GraphUpdated => None,
        }
    }
}

// ─── Notification queue (dedup + expiry) ───────────────────────────────

/// In-memory queue for deduplication and expiry management.
///
/// The queue tracks active "replaceable" toasts by dedup key. When a new
/// event arrives with the same key, the previous toast (if still visible) is
/// dismissed before the new one is shown. Entries auto-expire after
/// [`DEDUP_EXPIRY_MS`] so that genuinely distinct events with the same key
/// are not permanently suppressed.
struct NotificationQueue {
    /// Active dedup entries: key → expiry timestamp (ms since epoch).
    entries: HashMap<String, f64>,
}

/// How long a dedup entry lives before it expires (ms).
const DEDUP_TIMEOUT_MS: f64 = 5_000.0;

impl NotificationQueue {
    fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Check whether a dedup key is currently "locked" (i.e. a toast with
    /// that key was shown within the last `DEDUP_TIMEOUT_MS` milliseconds).
    /// If the entry has expired, remove it and return `false`.
    fn is_locked(&mut self, key: &str) -> bool {
        let now = js_sys::Date::now();
        if let Some(&expires_at) = self.entries.get(key) {
            if now >= expires_at {
                self.entries.remove(key);
                false
            } else {
                true
            }
        } else {
            false
        }
    }

    /// Register a dedup key for the standard expiry window.
    fn lock(&mut self, key: String) {
        let now = js_sys::Date::now();
        self.entries.insert(key, now + DEDUP_TIMEOUT_MS);
    }

    /// Remove a dedup key immediately.
    fn unlock(&mut self, key: &str) {
        self.entries.remove(key);
    }

    /// Drop all expired entries (garbage collection).
    fn prune(&mut self) {
        let now = js_sys::Date::now();
        self.entries.retain(|_, expires_at| *expires_at > now);
    }
}

// ─── Toast record tracking ───────────────────────────────────────────

/// Internal record of a live event-driven toast, keyed by its ToastContext
/// ID so the queue can dismiss replaceable toasts.
struct ToastRecord {
    id: String,
    dedup_key: Option<String>,
}

impl ToastRecord {
    fn new(id: String, dedup_key: Option<String>) -> Self {
        Self { id, dedup_key }
    }
}

// ─── Event → Toast mapping ─────────────────────────────────────────────

/// Maps a single [`FrontendEvent`] payload to a human-readable message
/// suitable for display in a toast.
fn event_message(payload: &PipelineEvent) -> Option<String> {
    match payload {
        PipelineEvent::ItemStored(e) => {
            Some(format!("Saved to {}", e.vault_path))
        }
        PipelineEvent::ItemCaptured(e) => {
            let source = format!("{:?}", e.capture_source);
            match &e.title {
                Some(t) => Some(format!("{} ({})", t, source)),
                None => Some(source),
            }
        }
        PipelineEvent::ItemProcessingCompleted(e) => {
            Some(format!("Processed by {}", e.processor_name))
        }
        PipelineEvent::ItemProcessingFailed(e) => {
            let retry_note = if e.will_retry {
                format!(", will retry (attempt {})", e.retry_count + 1)
            } else {
                String::new()
            };
            Some(format!("{} failed: {}{}", e.processor_name, e.error, retry_note))
        }
        PipelineEvent::ItemCancelled(e) => {
            Some(format!("Cancelled (job {})", e.job_id))
        }
        PipelineEvent::ItemRetried(e) => {
            Some(format!("Retrying (attempt {}/{})", e.retry_count, e.max_retries))
        }
        PipelineEvent::CapabilityStateChanged(e) => {
            let state = if e.enabled { "enabled" } else { "disabled" };
            Some(format!("{} {}", e.capability_id, state))
        }
        PipelineEvent::Sync(e) => {
            match e.current_status {
                nabu_core::sync::SyncStatus::Syncing => {
                    if let Some(ref progress) = e.progress {
                        Some(format!("{}: {} in progress", e.provider_id, progress.operation))
                    } else {
                        Some(format!("{}: syncing", e.provider_id))
                    }
                }
                nabu_core::sync::SyncStatus::UpToDate => {
                    Some(format!("{}: up to date", e.provider_id))
                }
                nabu_core::sync::SyncStatus::Idle => {
                    Some(format!("{}: idle", e.provider_id))
                }
                nabu_core::sync::SyncStatus::Pending => {
                    Some(format!("{}: pending changes", e.provider_id))
                }
                nabu_core::sync::SyncStatus::Error => {
                    let err = e.error.clone().unwrap_or_else(|| "unknown error".to_string());
                    Some(format!("{}: {}", e.provider_id, err))
                }
                nabu_core::sync::SyncStatus::NotConfigured => {
                    Some(format!("{}: not configured", e.provider_id))
                }
                nabu_core::sync::SyncStatus::Conflict => {
                    Some(format!("{}: conflicts detected", e.provider_id))
                }
            }
        }
        PipelineEvent::ItemProcessingProgress(_) => None,
        PipelineEvent::ItemProcessingStarted(e) => {
            Some(format!("Started: {}", e.processor_name))
        }
        PipelineEvent::IndexUpdated(e) => {
            let op = match e.operation {
                nabu_core::event_bus::IndexOperation::Added => "added",
                nabu_core::event_bus::IndexOperation::Updated => "updated",
                nabu_core::event_bus::IndexOperation::Removed => "removed",
            };
            Some(format!("Index {} updated", op))
        }
        PipelineEvent::GraphUpdated(e) => {
            let op = match e.operation {
                nabu_core::event_bus::GraphOperation::NodeAdded => "node added",
                nabu_core::event_bus::GraphOperation::NodeUpdated => "node updated",
                nabu_core::event_bus::GraphOperation::NodeRemoved => "node removed",
                nabu_core::event_bus::GraphOperation::EdgeAdded => "edge added",
                nabu_core::event_bus::GraphOperation::EdgeRemoved => "edge removed",
            };
            Some(format!("Graph updated: {}", op))
        }
        PipelineEvent::Plugin(_)
        | PipelineEvent::Process(_)
        | PipelineEvent::Agent(_)
        | PipelineEvent::Diagnostic(_)
        | PipelineEvent::Conversation(_)
        | PipelineEvent::Stream(_)
        | PipelineEvent::Session(_) => {
            // These event kinds are not in the FrontendEventKind set yet,
            // so they won't reach this function in the current phase.
            // Return a generic message if they ever do.
            Some(payload.kind().to_string())
        }
    }
}

// ─── NotificationManager ───────────────────────────────────────────────

/// Context passed into every event listener closure, capturing the shared
/// signals and toast context needed for dispatch.
struct NotificationState {
    toast_ctx: ToastContext,
    queue: Signal<NotificationQueue>,
    active_toasts: Signal<Vec<ToastRecord>>,
}

fn subscribe_all_kinds(state: NotificationState) {
    // We must call `use_event_listener` for every kind unconditionally to
    // satisfy the hooks rule. For kinds with no config, the callback is
    // a no-op.
    for &kind in FrontendEventKind::ALL {
        let config = NotificationConfig::for_kind(kind);
        let state = &state;

        use_event_listener(kind, move |ev: &FrontendEvent| {
            if let Some(config) = config {
                handle_event(ev, state, config);
            }
        });
    }
}

/// Zero-visibility component that wires platform events to toasts.
///
/// Place a single `<NotificationManager />` inside the `ToastProvider` subtree
/// (typically right after the app root's context providers).  It subscribes to
/// all event kinds that produce toasts, maps each event to a
/// [`NotificationConfig`], and pushes the resulting toast via
/// [`use_toast`].  Replaceable toasts (those with a `dedup_key`) are managed
/// through the internal queue to avoid stacking.
#[component]
pub fn NotificationManager() -> Element {
    let toast_ctx = use_toast();

    // The queue is scope-stable (created once by `use_signal`).
    let queue = use_signal(NotificationQueue::new);
    let active_toasts = use_signal(Vec::<ToastRecord>::new);

    let state = NotificationState {
        toast_ctx,
        queue,
        active_toasts,
    };

    // Subscribe to every event kind. `use_event_listener` is called
    // unconditionally for each kind (hooks rule compliance).
    subscribe_all_kinds(state);

    // Schedule periodic GC of expired dedup entries.
    {
        let queue_clone = state.queue.clone();
        let scheduled = use_signal(|| false);
        if !*scheduled.read() {
            *scheduled.write_unchecked() = true;
            set_timeout(
                move || {
                    queue_clone.write_unchecked().prune();
                    // Reschedule for the next cycle.
                    let q2 = queue_clone.clone();
                    set_timeout(
                        move || {
                            q2.write_unchecked().prune();
                        },
                        5_000,
                    );
                },
                5_000,
            );
        }
    }

    rsx! {}
}

/// Maps a single [`FrontendEvent`] to a toast and pushes it through the
/// [`ToastContext`], respecting dedup keys.
fn handle_event(
    ev: &FrontendEvent,
    state: &NotificationState,
    config: NotificationConfig,
) {
    // Build the message from the payload.
    let message = event_message(&ev.payload);
    let title = config.title.to_string();

    // Check dedup: if this kind has a dedup key and a toast with that key is
    // still locked, dismiss the old one first.
    let dedup_key = config.dedup_key.map(|k| k.to_string());

    if let Some(ref key) = dedup_key {
        let mut q = state.queue.write_unchecked();
        if q.is_locked(key) {
            // Dismiss existing toasts with this dedup key.
            let old_records: Vec<ToastRecord> = state
                .active_toasts
                .write_unchecked()
                .extract_if(.., |r| r.dedup_key.as_deref() == Some(key))
                .collect();
            for rec in old_records {
                state.toast_ctx.dismiss(&rec.id);
            }
            q.unlock(key);
        } else {
            // Lock the key for the expiry window.
            q.lock(key.clone());
        }
    }

    // Push the new toast.
    let toast_id = uuid::Uuid::new_v4().to_string();
    let record = ToastRecord::new(toast_id.clone(), dedup_key.clone());

    // Track the record.
    state.active_toasts.write_unchecked().push(record);

    // Push to the ToastContext.
    if let Some(ref msg) = message {
        state.toast_ctx.push(config.severity, title, msg);
    } else {
        // No message — still push a title-only toast.
        state.toast_ctx.push(config.severity, title, String::new());
    }

    // Schedule auto-cleanup: when the toast auto-dismisses (5 s), remove it
    // from the active_toasts list.
    if !config.persistent {
        let toast_id_cleanup = toast_id.clone();
        let active_cleanup = state.active_toasts.clone();
        set_timeout(
            move || {
                active_cleanup.write_unchecked().retain(|r| r.id != toast_id_cleanup);
            },
            5_000,
        );
    }
}

// ─── Convenience wrapper for App-level usage ───────────────────────────

/// Wraps the app tree with both a `ToastProvider` and `NotificationManager`.
///
/// This is the canonical way to ensure event-driven toasts are wired:
/// place your app's root children inside this provider, and all platform
/// events will automatically surface as toasts.
#[component]
pub fn NotificationHost(children: Element) -> Element {
    rsx! {
        crate::ui::feedback::ToastProvider {
            NotificationManager {}
            {children}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nabu_core::event_bus::PipelineEvent;
    use nabu_core::sync::{SyncStatus, SyncStatusChanged};

    // ─── NotificationConfig tests ───────────────────────────────────────

    #[test]
    fn config_excludes_noisy_events() {
        assert!(NotificationConfig::for_kind(FrontendEventKind::ItemProcessingProgress).is_none());
        assert!(NotificationConfig::for_kind(FrontendEventKind::ItemProcessingStarted).is_none());
        assert!(NotificationConfig::for_kind(FrontendEventKind::IndexUpdated).is_none());
        assert!(NotificationConfig::for_kind(FrontendEventKind::GraphUpdated).is_none());
    }

    #[test]
    fn config_includes_signal_events() {
        for &kind in FrontendEventKind::ALL {
            let config = NotificationConfig::for_kind(kind);
            // Every kind either has a config or is explicitly excluded.
            if config.is_none() {
                match kind {
                    FrontendEventKind::ItemProcessingProgress
                    | FrontendEventKind::ItemProcessingStarted
                    | FrontendEventKind::IndexUpdated
                    | FrontendEventKind::GraphUpdated => {}
                    _ => panic!("unexpected None config for kind: {kind:?}"),
                }
            }
        }
    }

    #[test]
    fn config_severity_for_error_kind() {
        let config = NotificationConfig::for_kind(FrontendEventKind::ItemProcessingFailed).unwrap();
        assert_eq!(config.severity, ToastKind::Error);
    }

    #[test]
    fn config_severity_for_success_kind() {
        let config = NotificationConfig::for_kind(FrontendEventKind::ItemStored).unwrap();
        assert_eq!(config.severity, ToastKind::Success);
    }

    #[test]
    fn config_severity_for_warning_kind() {
        let config = NotificationConfig::for_kind(FrontendEventKind::ItemCancelled).unwrap();
        assert_eq!(config.severity, ToastKind::Warning);
    }

    #[test]
    fn config_severity_for_info_kind() {
        let config = NotificationConfig::for_kind(FrontendEventKind::ItemCaptured).unwrap();
        assert_eq!(config.severity, ToastKind::Info);
    }

    #[test]
    fn config_dedup_key_for_sync() {
        let config = NotificationConfig::for_kind(FrontendEventKind::SyncStatusChanged).unwrap();
        assert_eq!(config.dedup_key, Some("sync.status"));
    }

    #[test]
    fn config_no_dedup_for_item_stored() {
        let config = NotificationConfig::for_kind(FrontendEventKind::ItemStored).unwrap();
        assert!(config.dedup_key.is_none());
    }

    #[test]
    fn config_no_dedup_for_processing_failed() {
        let config = NotificationConfig::for_kind(FrontendEventKind::ItemProcessingFailed).unwrap();
        assert!(config.dedup_key.is_none());
    }

    // ─── event_message tests ────────────────────────────────────────────

    #[test]
    fn message_for_item_stored() {
        let event = PipelineEvent::ItemStored(nabu_core::event_bus::ItemStoredEvent {
            object_id: uuid::Uuid::nil(),
            vault_path: "notes/foo.md".to_string(),
            object_type: nabu_core::models::ObjectType::Note,
            timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
        });
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("notes/foo.md"));
    }

    #[test]
    fn message_for_item_captured_with_title() {
        let event = PipelineEvent::ItemCaptured(nabu_core::event_bus::ItemCapturedEvent {
            object_id: uuid::Uuid::nil(),
            object_type: nabu_core::models::ObjectType::Note,
            capture_source: nabu_core::models::CaptureSource::Clipboard,
            title: Some("My Note Title".to_string()),
            timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
            job_id: None,
        });
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("My Note Title"));
        assert!(msg.contains("Clipboard"));
    }

    #[test]
    fn message_for_item_captured_without_title() {
        let event = PipelineEvent::ItemCaptured(nabu_core::event_bus::ItemCapturedEvent {
            object_id: uuid::Uuid::nil(),
            object_type: nabu_core::models::ObjectType::Note,
            capture_source: nabu_core::models::CaptureSource::Url,
            title: None,
            timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
            job_id: None,
        });
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("Url"));
    }

    #[test]
    fn message_for_processing_completed() {
        let event = PipelineEvent::ItemProcessingCompleted(nabu_core::event_bus::ItemProcessingCompletedEvent {
            object_id: uuid::Uuid::nil(),
            job_id: uuid::Uuid::nil(),
            processor_name: "ocr".to_string(),
            timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
        });
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("ocr"));
    }

    #[test]
    fn message_for_processing_failed_no_retry() {
        let event = PipelineEvent::ItemProcessingFailed(nabu_core::event_bus::ItemProcessingFailedEvent {
            object_id: uuid::Uuid::nil(),
            job_id: uuid::Uuid::nil(),
            processor_name: "transcriber".to_string(),
            error: "audio format not supported".to_string(),
            retry_count: 3,
            will_retry: false,
            timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
        });
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("transcriber failed"));
        assert!(msg.contains("audio format not supported"));
        assert!(!msg.contains("will retry"));
    }

    #[test]
    fn message_for_processing_failed_with_retry() {
        let event = PipelineEvent::ItemProcessingFailed(nabu_core::event_bus::ItemProcessingFailedEvent {
            object_id: uuid::Uuid::nil(),
            job_id: uuid::Uuid::nil(),
            processor_name: "transcriber".to_string(),
            error: "timeout".to_string(),
            retry_count: 1,
            will_retry: true,
            timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
        });
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("will retry"));
    }

    #[test]
    fn message_for_item_cancelled() {
        let event = PipelineEvent::ItemCancelled(nabu_core::event_bus::ItemCancelledEvent {
            object_id: uuid::Uuid::nil(),
            job_id: uuid::Uuid::nil(),
            timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
        });
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("Cancelled"));
    }

    #[test]
    fn message_for_item_retried() {
        let event = PipelineEvent::ItemRetried(nabu_core::event_bus::ItemRetriedEvent {
            object_id: uuid::Uuid::nil(),
            job_id: uuid::Uuid::nil(),
            retry_count: 2,
            max_retries: 5,
            timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
        });
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("2/5"));
    }

    #[test]
    fn message_for_capability_enabled() {
        let event = PipelineEvent::CapabilityStateChanged(nabu_core::event_bus::CapabilityStateEvent {
            capability_id: "capture:file".to_string(),
            enabled: true,
            timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
        });
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("capture:file"));
        assert!(msg.contains("enabled"));
    }

    #[test]
    fn message_for_capability_disabled() {
        let event = PipelineEvent::CapabilityStateChanged(nabu_core::event_bus::CapabilityStateEvent {
            capability_id: "capture:file".to_string(),
            enabled: false,
            timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
        });
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("disabled"));
    }

    #[test]
    fn message_for_sync_up_to_date() {
        let sync_event = SyncStatusChanged::new("folder-1", "syncthing", SyncStatus::UpToDate);
        let event = PipelineEvent::Sync(sync_event);
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("up to date"));
    }

    #[test]
    fn message_for_sync_syncing() {
        let sync_event = SyncStatusChanged::new("folder-1", "syncthing", SyncStatus::Syncing);
        let event = PipelineEvent::Sync(sync_event);
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("syncing"));
    }

    #[test]
    fn message_for_sync_error() {
        let sync_event = SyncStatusChanged::new("folder-1", "icloud", SyncStatus::Error)
            .with_error("connection refused");
        let event = PipelineEvent::Sync(sync_event);
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("connection refused"));
    }

    #[test]
    fn message_for_sync_not_configured() {
        let sync_event = SyncStatusChanged::new("folder-1", "webdav", SyncStatus::NotConfigured);
        let event = PipelineEvent::Sync(sync_event);
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("not configured"));
    }

    #[test]
    fn message_for_sync_conflict() {
        let sync_event = SyncStatusChanged::new("folder-1", "git", SyncStatus::Conflict);
        let event = PipelineEvent::Sync(sync_event);
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("conflicts"));
    }

    #[test]
    fn message_for_sync_pending() {
        let sync_event = SyncStatusChanged::new("folder-1", "git", SyncStatus::Pending);
        let event = PipelineEvent::Sync(sync_event);
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("pending"));
    }

    #[test]
    fn message_for_sync_idle() {
        let sync_event = SyncStatusChanged::new("folder-1", "syncthing", SyncStatus::Idle);
        let event = PipelineEvent::Sync(sync_event);
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("idle"));
    }

    #[test]
    fn message_for_sync_syncing_with_progress() {
        let progress = nabu_core::sync::SyncProgress::new("uploading note.md")
            .with_status(SyncStatus::Syncing);
        let sync_event = SyncStatusChanged::new("folder-1", "syncthing", SyncStatus::Syncing)
            .with_progress(progress);
        let event = PipelineEvent::Sync(sync_event);
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("uploading note.md"));
    }

    #[test]
    fn message_for_sync_error_without_message() {
        let sync_event = SyncStatusChanged::new("folder-1", "syncthing", SyncStatus::Error);
        let event = PipelineEvent::Sync(sync_event);
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("unknown error"));
    }

    #[test]
    fn message_for_progress_is_none() {
        let event = PipelineEvent::ItemProcessingProgress(nabu_core::event_bus::ItemProcessingProgressEvent {
            object_id: uuid::Uuid::nil(),
            job_id: uuid::Uuid::nil(),
            progress: 0.5,
            message: None,
            timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
        });
        assert_eq!(event_message(&event), None);
    }

    #[test]
    fn message_for_index_updated() {
        let event = PipelineEvent::IndexUpdated(nabu_core::event_bus::IndexUpdatedEvent {
            object_id: uuid::Uuid::nil(),
            operation: nabu_core::event_bus::IndexOperation::Added,
            timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
        });
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("added"));
    }

    #[test]
    fn message_for_graph_updated() {
        let event = PipelineEvent::GraphUpdated(nabu_core::event_bus::GraphUpdatedEvent {
            object_id: uuid::Uuid::nil(),
            operation: nabu_core::event_bus::GraphOperation::NodeAdded,
            timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
        });
        let msg = event_message(&event).unwrap();
        assert!(msg.contains("node added"));
    }

    // ─── NotificationQueue tests ────────────────────────────────────────

    #[test]
    fn queue_is_unlocked_initially() {
        let mut q = NotificationQueue::new();
        assert!(!q.is_locked("test-key"));
    }

    #[test]
    fn queue_locks_and_unlocks() {
        let mut q = NotificationQueue::new();
        q.lock("test-key".to_string());
        assert!(q.is_locked("test-key"));
        q.unlock("test-key");
        assert!(!q.is_locked("test-key"));
    }

    #[test]
    fn queue_multiple_keys_independent() {
        let mut q = NotificationQueue::new();
        q.lock("key-a".to_string());
        q.lock("key-b".to_string());
        assert!(q.is_locked("key-a"));
        assert!(q.is_locked("key-b"));
        q.unlock("key-a");
        assert!(!q.is_locked("key-a"));
        assert!(q.is_locked("key-b"));
    }

    #[test]
    fn queue_prune_is_safe_on_empty() {
        let mut q = NotificationQueue::new();
        q.prune();
        assert!(q.entries.is_empty());
    }

    // ─── ToastRecord tests ───────────────────────────────────────────────

    #[test]
    fn toast_record_stores_dedup_key() {
        let rec = ToastRecord::new("id-1".to_string(), Some("sync.status".to_string()));
        assert_eq!(rec.dedup_key.as_deref(), Some("sync.status"));
    }

    #[test]
    fn toast_record_no_dedup_key() {
        let rec = ToastRecord::new("id-2".to_string(), None);
        assert!(rec.dedup_key.is_none());
    }
}
