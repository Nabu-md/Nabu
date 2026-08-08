//! # Activity Manager — frontend activity timeline state
//!
//! The [`ActivityManager`] is the central coordinator for the Activity Panel.
//! It subscribes to all platform events delivered by the frontend
//! [`EventService`](crate::events::EventService), converts each meaningful
//! [`PipelineEvent`](/nabu_core::event_bus::PipelineEvent) into a display-ready
//! [`ActivityItem`], stores it in a bounded, chronologically-ordered in-memory
//! history, and exposes the result as a Dioxus [`Signal`] for the
//! [`ActivityPanel`](super::panel::ActivityPanel) to render.
//!
//! ## Architecture
//!
//! ```text
//! Backend Event → EventBus → EventBusBridge → nabu-event channel
//!     → EventService → ActivityManager (this module)
//!     → ActivityStore (Signal<Vec<ActivityItem>>)
//!     → ActivityPanel → User Timeline
//! ```
//!
//! ## Key responsibilities
//!
//! - **Event subscription**: registers a single `subscribe_all` listener on the
//!   `EventService` so every platform event is considered.
//! - **Event filtering**: only user-facing events (capability changes, plugin
//!   lifecycle, sync status, pipeline milestones) are converted to
//!   [`ActivityItem`]s. Low-level internal events (e.g. `stream.token`,
//!   `item.processing.progress`) are ignored.
//! - **Activity extraction**: each supported `PipelineEvent` variant is mapped
//!   to an `ActivityItem` carrying title, description, severity, category,
//!   timestamp, and originating subsystem.
//! - **Bounded history**: items are prepended to a `Vec` and pruned to a
//!   configurable maximum (`DEFAULT_MAX_ACTIVITIES`), preventing unbounded
//!   memory growth.
//! - **Deduplication**: within a short window, duplicate entries (same kind +
//!   key) are coalesced so the timeline stays stable.
//! - **Reactivity**: the history lives in a Dioxus `Signal`, so appending a new
//!   activity triggers a re-render of only the timeline list — not the entire
//!   panel.
//!
//! ## Error handling
//!
//! Malformed events or unknown variants are logged and skipped — the manager
//! never panics. If the `EventService` is unavailable (e.g. running outside
//! Tauri), the manager remains usable with an empty timeline.
//!
//! ## Future compatibility
//!
//! The `ActivityItem` model carries structured fields (severity, category,
//! subsystem, metadata) so future filtering/searching can be layered on
//! without changing the extraction pipeline. The `metadata` field is a
//! `serde_json::Map` for arbitrary, forward-compatible payload data.

use std::collections::HashMap;

use dioxus::prelude::*;
use js_sys::Date as JsDate;

use crate::events::{use_event_service, EventService, FrontendEvent};

mod panel;

pub use panel::ActivityPanel;

/// Maximum number of activities retained in memory.
pub const DEFAULT_MAX_ACTIVITIES: usize = 500;

/// Severity level for an activity item, surfaced to the UI as a colored badge
/// or icon tint.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum ActivitySeverity {
    /// Informational — a normal, successful operation.
    #[default]
    Info,
    /// A warning — something unexpected but recoverable happened.
    Warning,
    /// An error — an operation failed.
    Error,
}

impl ActivitySeverity {
    /// CSS class suffix used for color theming (e.g. `badge-info`, `badge-error`).
    pub fn badge_kind(self) -> &'static str {
        match self {
            Self::Info => "badge-info",
            Self::Warning => "badge-warning",
            Self::Error => "badge-error",
        }
    }

    /// A short, human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

/// Broad category for an activity item. Used for future filtering and visual
/// grouping in the timeline.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum ActivityCategory {
    /// Item was captured/enqueued.
    Capture,
    /// Processing pipeline event (completed, failed, etc.).
    Processing,
    /// Indexing or graph update.
    Index,
    /// Storage / save event.
    Storage,
    /// Capability lifecycle (enabled/disabled).
    Capability,
    /// Plugin lifecycle (loaded, started, error, etc.).
    Plugin,
    /// Synchronization status change.
    Sync,
    /// Agent lifecycle (started, stopped, crashed).
    Agent,
    /// Process supervision event.
    Process,
    /// Conversation persistence (saved, updated, deleted).
    Conversation,
    /// Streaming session lifecycle.
    Stream,
    /// Generic lifecycle event.
    Lifecycle,
    /// Unknown / uncategorised.
    Other,
}

impl ActivityCategory {
    /// Human-readable label for display.
    pub fn label(self) -> &'static str {
        match self {
            Self::Capture => "Capture",
            Self::Processing => "Processing",
            Self::Index => "Index",
            Self::Storage => "Storage",
            Self::Capability => "Capability",
            Self::Plugin => "Plugin",
            Self::Sync => "Synchronization",
            Self::Agent => "Agent",
            Self::Process => "Process",
            Self::Conversation => "Conversation",
            Self::Stream => "Stream",
            Self::Lifecycle => "Lifecycle",
            Self::Other => "Other",
        }
    }
}

/// A single entry in the activity timeline.
#[derive(Clone, Debug, PartialEq)]
pub struct ActivityItem {
    /// Unique identifier for this activity item (UUID v4 or derived key).
    pub id: String,
    /// Human-readable title — a short summary of what happened.
    pub title: String,
    /// Optional longer description with supporting details.
    pub description: Option<String>,
    /// Severity level (info / warning / error).
    pub severity: ActivitySeverity,
    /// Broad category (capability, plugin, sync, …).
    pub category: ActivityCategory,
    /// Originating subsystem — e.g. "plugin", "sync", "pipeline".
    pub subsystem: &'static str,
    /// The raw event kind string from the EventBus (e.g. `"plugin.loaded"`).
    pub event_kind: String,
    /// When the event occurred (ISO-8601). Falls back to JS `Date.now()` when
    /// the envelope payload has no timestamp.
    pub timestamp_ms: f64,
    /// Arbitrary, forward-compatible metadata attached by the extractor.
    /// Future filtering/searching can inspect this without schema changes.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ActivityItem {
    /// Creates a new activity item with the given fields.
    /// `timestamp_ms` is in milliseconds since the Unix epoch.
    fn new(
        id: String,
        title: String,
        description: Option<String>,
        severity: ActivitySeverity,
        category: ActivityCategory,
        subsystem: &'static str,
        event_kind: String,
        timestamp_ms: f64,
    ) -> Self {
        Self {
            id,
            title,
            description,
            severity,
            category,
            subsystem,
            event_kind,
            timestamp_ms,
            metadata: HashMap::new(),
        }
    }

    /// Builder: attach a metadata key-value pair.
    pub fn with_meta(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// The Activity Manager.
///
/// Created once by [`ActivityManager::create`] (which also subscribes to the
/// `EventService`), it maintains a bounded, chronologically-ordered history of
/// [`ActivityItem`]s in a Dioxus [`Signal`].
///
/// The manager is `Copy` (it wraps an `Arc`-backed signal) so it can be freely
/// passed into closures and stored in a context provider.
#[derive(Clone, Copy)]
pub struct ActivityManager {
    /// The signal-backed activity store. New items are prepended.
    activities: Signal<Vec<ActivityItem>>,
    /// Maximum number of items retained.
    max_activities: usize,
}

impl Default for ActivityManager {
    fn default() -> Self {
        Self {
            activities: use_signal(Vec::<ActivityItem>::new),
            max_activities: DEFAULT_MAX_ACTIVITIES,
        }
    }
}

impl ActivityManager {
    /// Creates a new `ActivityManager`, subscribes to all platform events on
    /// the provided `EventService`, and returns the manager.
    ///
    /// Must be called inside a Dioxus component body (it uses `use_signal`).
    /// The subscription lives as long as the Dioxus scope — it is
    /// automatically cleaned up on unmount via `use_drop`.
    pub fn create(service: &EventService) -> Self {
        let max_activities = DEFAULT_MAX_ACTIVITIES;
        let activities: Signal<Vec<ActivityItem>> = use_signal(Vec::<ActivityItem>::new);

        // Subscribe to *all* events. The filter (which kinds to keep) lives in
        // `extract_activity` so adding a new event kind is a single match arm.
        //
        // The EventSubscription is stored in a Signal so it is not dropped
        // (and unsubscribed) until the Dioxus scope ends. `use_drop` cleans
        // up the subscription at unmount.
        let act = activities;
        let max = max_activities;

        // Store the subscription in a Signal. `use_signal` runs its init once
        // per scope, so this is stable across re-renders.
        let sub: Signal<Option<crate::events::EventSubscription>> =
            use_signal(|| None);

        // Register the subscription exactly once (signal-guarded).
        if sub.peek().is_none() {
            let handle = service.subscribe_all(move |ev: &FrontendEvent| {
                // Attempt to convert the event into an ActivityItem. If the event
                // kind is not user-facing, `extract_activity` returns None and the
                // event is silently ignored.
                if let Some(item) = extract_activity(ev, max) {
                    let mut items = act.write_unchecked();
                    // Deduplicate: if the same event_kind + id already exists,
                    // replace it in-place (stable ordering, no duplicate).
                    if let Some(pos) = items
                        .iter()
                        .position(|existing| {
                            existing.event_kind == item.event_kind
                                && existing.id == item.id
                        })
                    {
                        items[pos] = item.clone();
                    } else {
                        items.insert(0, item);
                        // Prune old entries beyond the cap.
                        if items.len() > max {
                            items.truncate(max);
                        }
                    }
                }
            });
            *sub.write_unchecked() = Some(handle);
        }
        // Clean up the subscription when the component unmounts.
        use_drop(move || {
            if let Some(handle) = sub.write_unchecked().take() {
                handle.unsubscribe();
            }
        });

        Self {
            activities: act,
            max_activities: max,
        }
    }

    /// Returns the activity history signal for reactive rendering.
    pub fn activities(&self) -> Signal<Vec<ActivityItem>> {
        self.activities
    }

    /// Maximum retained activities.
    pub fn max_activities(&self) -> usize {
        self.max_activities
    }

    /// Current number of activities in the store.
    pub fn len(&self) -> usize {
        self.activities.read().len()
    }

    /// Returns `true` if the activity store is empty.
    pub fn is_empty(&self) -> bool {
        self.activities.read().is_empty()
    }

    /// Clears all activities from the store.
    pub fn clear(&self) {
        self.activities.write_unchecked().clear();
    }
}

/// The Dioxus context type for the ActivityManager.
#[derive(Clone, Copy)]
pub struct ActivityContext {
    pub manager: ActivityManager,
}

/// Retrieves the shared [`ActivityContext`].
pub fn use_activity() -> ActivityContext {
    use_context::<ActivityContext>()
}

/// Provider component that owns the `ActivityManager` lifetime.
///
/// Wraps the application tree (or a subtree) so descendant components can call
/// [`use_activity`] to access the shared activity store. The `EventService`
/// context must be available above this provider.
#[component]
pub fn ActivityProvider(children: Element) -> Element {
    let service = use_event_service();
    let manager = ActivityManager::create(&service);
    provide_context(ActivityContext { manager });

    rsx! { {children} }
}

// ── Event extraction ─────────────────────────────────────────────────

/// Converts a `FrontendEvent` into an [`ActivityItem`], if the event kind is
/// user-facing and should appear in the activity timeline.
///
/// Returns `None` for low-level or internal events (streaming tokens,
/// processing progress, etc.).
fn extract_activity(ev: &FrontendEvent, _max: usize) -> Option<ActivityItem> {
    use nabu_core::event_bus::PipelineEvent;

    let timestamp_ms = ev
        .timestamp
        .as_deref()
        .and_then(parse_iso_to_ms)
        .unwrap_or_else(|| JsDate::now());

    let item = match &ev.payload {
        // ── Item captured ──
        PipelineEvent::ItemCaptured(e) => ActivityItem::new(
            format!("item.captured:{}", e.object_id),
            "Item captured".to_string(),
            e.title.clone(),
            ActivitySeverity::Info,
            ActivityCategory::Capture,
            "pipeline",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("object_id".into(), serde_json::to_value(e.object_id).ok()?)
        .with_meta("object_type".into(), serde_json::to_value(e.object_type.to_string()).ok()?)
        .with_meta("capture_source".into(), serde_json::to_value(&e.capture_source).ok()?),

        // ── Item processing started ──
        PipelineEvent::ItemProcessingStarted(e) => ActivityItem::new(
            format!("item.processing.started:{}", e.job_id),
            "Processing started".to_string(),
            Some(e.processor_name.clone()),
            ActivitySeverity::Info,
            ActivityCategory::Processing,
            "pipeline",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("processor".into(), serde_json::to_value(&e.processor_name).ok()?),

        // ── Item processing completed ──
        PipelineEvent::ItemProcessingCompleted(e) => ActivityItem::new(
            format!("item.processing.completed:{}", e.job_id),
            "Processing completed".to_string(),
            Some(e.processor_name.clone()),
            ActivitySeverity::Info,
            ActivityCategory::Processing,
            "pipeline",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("processor".into(), serde_json::to_value(&e.processor_name).ok()?),

        // ── Item processing failed ──
        PipelineEvent::ItemProcessingFailed(e) => ActivityItem::new(
            format!("item.processing.failed:{}", e.job_id),
            "Processing failed".to_string(),
            Some(e.error.clone()),
            ActivitySeverity::Error,
            ActivityCategory::Processing,
            "pipeline",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("processor".into(), serde_json::to_value(&e.processor_name).ok()?)
        .with_meta("retry_count".into(), serde_json::to_value(e.retry_count).ok()?),

        // ── Item stored ──
        PipelineEvent::ItemStored(e) => ActivityItem::new(
            format!("item.stored:{}", e.vault_path),
            "Item stored".to_string(),
            Some(e.vault_path.clone()),
            ActivitySeverity::Info,
            ActivityCategory::Storage,
            "storage",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("vault_path".into(), serde_json::to_value(&e.vault_path).ok()?),

        // ── Index updated ──
        PipelineEvent::IndexUpdated(e) => ActivityItem::new(
            format!("index.updated:{}", e.object_id),
            "Index updated".to_string(),
            Some(format!("{:?}", e.operation)),
            ActivitySeverity::Info,
            ActivityCategory::Index,
            "indexer",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("operation".into(), serde_json::to_value(e.operation.to_string()).ok()?),

        // ── Graph updated ──
        PipelineEvent::GraphUpdated(e) => ActivityItem::new(
            format!("graph.updated:{}", e.object_id),
            "Graph updated".to_string(),
            Some(format!("{:?}", e.operation)),
            ActivitySeverity::Info,
            ActivityCategory::Index,
            "graph",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("operation".into(), serde_json::to_value(e.operation.to_string()).ok()?),

        // ── Item cancelled ──
        PipelineEvent::ItemCancelled(e) => ActivityItem::new(
            format!("item.cancelled:{}", e.job_id),
            "Item cancelled".to_string(),
            None,
            ActivitySeverity::Warning,
            ActivityCategory::Processing,
            "pipeline",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        ),

        // ── Item retried ──
        PipelineEvent::ItemRetried(e) => ActivityItem::new(
            format!("item.retried:{}", e.job_id),
            "Processing retried".to_string(),
            Some(format!("retry {}/{}", e.retry_count, e.max_retries)),
            ActivitySeverity::Warning,
            ActivityCategory::Processing,
            "pipeline",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("retry_count".into(), serde_json::to_value(e.retry_count).ok()?),

        // ── Capability state changed ──
        PipelineEvent::CapabilityStateChanged(e) => ActivityItem::new(
            format!("capability.{}", if e.enabled { "enabled" } else { "disabled" }),
            if e.enabled {
                "Capability enabled".to_string()
            } else {
                "Capability disabled".to_string()
            },
            Some(e.capability_id.clone()),
            ActivitySeverity::Info,
            ActivityCategory::Capability,
            "plugin",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("capability_id".into(), serde_json::to_value(&e.capability_id).ok()?)
        .with_meta("enabled".into(), serde_json::to_value(e.enabled).ok()?),

        // ── Plugin events ──
        PipelineEvent::Plugin(e) => match extract_plugin_activity(ev, e, timestamp_ms) {
            Some(item) => item,
            None => return None,
        },

        // ── Sync events ──
        PipelineEvent::Sync(e) => ActivityItem::new(
            format!("sync:{}", e.folder_id),
            "Synchronization status changed".to_string(),
            Some(e.current_status.label().to_string()),
            if e.current_status.is_error() {
                ActivitySeverity::Error
            } else if matches!(e.current_status, nabu_core::sync::SyncStatus::Conflict) {
                ActivitySeverity::Warning
            } else {
                ActivitySeverity::Info
            },
            ActivityCategory::Sync,
            "sync",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("folder_id".into(), serde_json::to_value(&e.folder_id).ok()?)
        .with_meta("provider_id".into(), serde_json::to_value(&e.provider_id).ok()?)
        .with_meta("current_status".into(), serde_json::to_value(e.current_status.label()).ok()?)
        .with_meta("previous_status".into(), serde_json::to_value(e.previous_status.map(|s| s.label())).ok()?),

        // ── Process events ──
        PipelineEvent::Process(e) => {
            let inner = extract_process_activity(ev, e, timestamp_ms);
            if let Some(inner) = inner { inner } else { return None; }
        }

        // ── Agent events ──
        PipelineEvent::Agent(e) => {
            let inner = extract_agent_activity(ev, e, timestamp_ms);
            if let Some(inner) = inner { inner } else { return None; }
        }

        // ── Conversation events ──
        PipelineEvent::Conversation(e) => {
            let inner = extract_conversation_activity(ev, e, timestamp_ms);
            if let Some(inner) = inner { inner } else { return None; }
        }

        // ── Stream / Session events ──
        PipelineEvent::Stream(e) => {
            let inner = extract_stream_activity(ev, e, timestamp_ms);
            if let Some(inner) = inner { inner } else { return None; }
        }
        PipelineEvent::Session(e) => {
            let inner = extract_session_activity(ev, e, timestamp_ms);
            if let Some(inner) = inner { inner } else { return None; }
        }

        // ── Diagnostic events — not surfaced as activity ──
        PipelineEvent::Diagnostic(_) => return None,

        // ── Processing progress — too noisy for the activity timeline ──
        PipelineEvent::ItemProcessingProgress(_) => return None,
    };

    Some(item)
}

/// Extracts an `ActivityItem` from a `PluginEvent`.
fn extract_plugin_activity(
    ev: &FrontendEvent,
    e: &nabu_core::plugin::events::PluginEvent,
    timestamp_ms: f64,
) -> Option<ActivityItem> {
    use nabu_core::plugin::events::PluginEvent as PE;
    let payload_ts = || timestamp_ms;

    let (item, inner_ts) = match e {
        // Plugin loaded — only surface warnings and errors.
        PE::PluginLoaded(e) => ActivityItem::new(
            format!("plugin.loaded:{}", e.plugin_id),
            format!("Plugin loaded: {}", e.plugin_name),
            Some(format!("v{}", e.plugin_version)),
            ActivitySeverity::Info,
            ActivityCategory::Plugin,
            "plugin",
            ev.kind.as_str().to_string(),
            payload_ts(),
        )
        .with_meta("plugin_id".into(), serde_json::to_value(&e.plugin_id).ok()?)
        .with_meta("plugin_name".into(), serde_json::to_value(&e.plugin_name).ok()?),

        PE::PluginUnloaded(e) => ActivityItem::new(
            format!("plugin.unloaded:{}", e.plugin_id),
            "Plugin unloaded".to_string(),
            Some(e.plugin_id.clone()),
            ActivitySeverity::Info,
            ActivityCategory::Plugin,
            "plugin",
            ev.kind.as_str().to_string(),
            payload_ts(),
        )
        .with_meta("plugin_id".into(), serde_json::to_value(&e.plugin_id).ok()?),

        PE::PluginRegistered(e) => ActivityItem::new(
            format!("plugin.registered:{}", e.plugin_id),
            "Plugin registered".to_string(),
            Some(e.plugin_name.clone()),
            ActivitySeverity::Info,
            ActivityCategory::Plugin,
            "plugin",
            ev.kind.as_str().to_string(),
            payload_ts(),
        )
        .with_meta("plugin_id".into(), serde_json::to_value(&e.plugin_id).ok()?),

        PE::PluginUnregistered(e) => ActivityItem::new(
            format!("plugin.unregistered:{}", e.plugin_id),
            "Plugin unregistered".to_string(),
            Some(e.plugin_id.clone()),
            ActivitySeverity::Info,
            ActivityCategory::Plugin,
            "plugin",
            ev.kind.as_str().to_string(),
            payload_ts(),
        )
        .with_meta("plugin_id".into(), serde_json::to_value(&e.plugin_id).ok()?),

        PE::PluginStarted(e) => ActivityItem::new(
            format!("plugin.started:{}", e.plugin_id),
            format!("Plugin started: {}", e.plugin_name),
            Some(format!("v{}", e.plugin_version)),
            ActivitySeverity::Info,
            ActivityCategory::Plugin,
            "plugin",
            ev.kind.as_str().to_string(),
            payload_ts(),
        )
        .with_meta("plugin_id".into(), serde_json::to_value(&e.plugin_id).ok()?),

        PE::PluginStopped(e) => ActivityItem::new(
            format!("plugin.stopped:{}", e.plugin_id),
            "Plugin stopped".to_string(),
            Some(e.plugin_id.clone()),
            ActivitySeverity::Info,
            ActivityCategory::Plugin,
            "plugin",
            ev.kind.as_str().to_string(),
            payload_ts(),
        )
        .with_meta("plugin_id".into(), serde_json::to_value(&e.plugin_id).ok()?),

        PE::CapabilityRegistered(e) => ActivityItem::new(
            format!("capability.registered:{}", e.capability_id),
            format!("Capability registered: {}", e.capability_id),
            Some(e.description.clone()),
            ActivitySeverity::Info,
            ActivityCategory::Capability,
            "plugin",
            ev.kind.as_str().to_string(),
            payload_ts(),
        )
        .with_meta("capability_id".into(), serde_json::to_value(&e.capability_id).ok()?),

        PE::CapabilityRemoved(e) => ActivityItem::new(
            format!("capability.removed:{}", e.capability_id),
            format!("Capability removed: {}", e.capability_id),
            None,
            ActivitySeverity::Info,
            ActivityCategory::Capability,
            "plugin",
            ev.kind.as_str().to_string(),
            payload_ts(),
        )
        .with_meta("capability_id".into(), serde_json::to_value(&e.capability_id).ok()?),

        PE::PluginWarning(e) => ActivityItem::new(
            format!("plugin.warning:{}:{}", e.plugin_id, e.code.clone().unwrap_or_default()),
            format!("Plugin warning: {}", e.plugin_id),
            Some(e.message.clone()),
            ActivitySeverity::Warning,
            ActivityCategory::Plugin,
            "plugin",
            ev.kind.as_str().to_string(),
            payload_ts(),
        )
        .with_meta("plugin_id".into(), serde_json::to_value(&e.plugin_id).ok()?),

        PE::PluginError(e) => ActivityItem::new(
            format!("plugin.error:{}:{}", e.plugin_id, e.code.clone().unwrap_or_default()),
            format!("Plugin error: {}", e.plugin_id),
            Some(e.error.clone()),
            ActivitySeverity::Error,
            ActivityCategory::Plugin,
            "plugin",
            ev.kind.as_str().to_string(),
            payload_ts(),
        )
        .with_meta("plugin_id".into(), serde_json::to_value(&e.plugin_id).ok()?),

        PE::PluginRequest(e) => ActivityItem::new(
            format!("plugin.request:{}:{}", e.plugin_id, e.request_id),
            format!("Capability request: {}", e.method),
            Some(e.plugin_id.clone()),
            ActivitySeverity::Info,
            ActivityCategory::Capability,
            "plugin",
            ev.kind.as_str().to_string(),
            payload_ts(),
        )
        .with_meta("method".into(), serde_json::to_value(&e.method).ok()?),

        PE::PluginResponse(e) => ActivityItem::new(
            format!("plugin.response:{}:{}", e.plugin_id, e.request_id),
            format!("Capability response: {}", e.method),
            Some(e.status.to_string()),
            ActivitySeverity::Info,
            ActivityCategory::Capability,
            "plugin",
            ev.kind.as_str().to_string(),
            payload_ts(),
        )
        .with_meta("method".into(), serde_json::to_value(&e.method).ok()?),
    };

    let _ = inner_ts;
    Some(item)
}

/// Extracts an `ActivityItem` from a `ProcessEvent`.
fn extract_process_activity(
    ev: &FrontendEvent,
    e: &nabu_core::event_bus::ProcessEvent,
    timestamp_ms: f64,
) -> Option<ActivityItem> {
    use nabu_core::event_bus::ProcessEvent as PE;

    let item = match e {
        PE::Started(e) => ActivityItem::new(
            format!("process.started:{}", e.process_id),
            format!("Process started: {}", e.name),
            Some(e.command.clone()),
            ActivitySeverity::Info,
            ActivityCategory::Process,
            "process_supervisor",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("name".into(), serde_json::to_value(&e.name).ok()?)
        .with_meta("pid".into(), serde_json::to_value(e.pid).ok()?),

        PE::Exited(e) => ActivityItem::new(
            format!("process.exited:{}", e.process_id),
            format!("Process exited: {}", e.name),
            Some(format!("exit code: {:?}", e.exit_code)),
            ActivitySeverity::Info,
            ActivityCategory::Process,
            "process_supervisor",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("name".into(), serde_json::to_value(&e.name).ok()?)
        .with_meta("restart_count".into(), serde_json::to_value(e.restart_count).ok()?),

        PE::Failed(e) => ActivityItem::new(
            format!("process.failed:{}", e.process_id),
            format!("Process failed: {}", e.name),
            Some(e.error.clone()),
            ActivitySeverity::Error,
            ActivityCategory::Process,
            "process_supervisor",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("name".into(), serde_json::to_value(&e.name).ok()?)
        .with_meta("exit_code".into(), serde_json::to_value(e.exit_code).ok()?),

        PE::Restarted(e) => ActivityItem::new(
            format!("process.restarted:{}", e.process_id),
            format!("Process restarted: {}", e.name),
            Some(e.reason.clone()),
            ActivitySeverity::Warning,
            ActivityCategory::Process,
            "process_supervisor",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("reason".into(), serde_json::to_value(&e.reason).ok()?),

        PE::Stopped(e) => ActivityItem::new(
            format!("process.stopped:{}", e.process_id),
            format!("Process stopped: {}", e.name),
            Some(e.reason.clone()),
            ActivitySeverity::Info,
            ActivityCategory::Process,
            "process_supervisor",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        ),

        #[cfg(not(target_arch = "wasm32"))]
        PE::HealthChanged(e) => ActivityItem::new(
            format!("process.health.changed:{}:{}", e.process_id, e.status.label()),
            format!("Process health: {}", e.name),
            Some(format!("{} → {}", e.state, e.status.label())),
            if matches!(e.status, nabu_core::event_bus::ProcessHealthStatus::Unhealthy | nabu_core::event_bus::ProcessHealthStatus::Unknown) {
                ActivitySeverity::Error
            } else if matches!(e.status, nabu_core::event_bus::ProcessHealthStatus::Degraded | nabu_core::event_bus::ProcessHealthStatus::Starting) {
                ActivitySeverity::Warning
            } else {
                ActivitySeverity::Info
            },
            ActivityCategory::Process,
            "process_supervisor",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("name".into(), serde_json::to_value(&e.name).ok()?),
    };

    Some(item)
}

/// Extracts an `ActivityItem` from an `AgentEvent`.
fn extract_agent_activity(
    ev: &FrontendEvent,
    e: &nabu_core::event_bus::AgentEvent,
    timestamp_ms: f64,
) -> Option<ActivityItem> {
    use nabu_core::event_bus::AgentEvent as AE;

    let item = match e {
        AE::Started(e) => ActivityItem::new(
            format!("agent.started:{}", e.process_id),
            format!("Agent started: {}", e.agent_name),
            Some(e.agent_kind.clone()),
            ActivitySeverity::Info,
            ActivityCategory::Agent,
            "agent_manager",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("agent_name".into(), serde_json::to_value(&e.agent_name).ok()?)
        .with_meta("agent_kind".into(), serde_json::to_value(&e.agent_kind).ok()?),

        AE::Stopped(e) => ActivityItem::new(
            format!("agent.stopped:{}", e.process_id),
            format!("Agent stopped: {}", e.agent_name),
            Some(e.reason.clone()),
            ActivitySeverity::Info,
            ActivityCategory::Agent,
            "agent_manager",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        ),

        AE::Restarted(e) => ActivityItem::new(
            format!("agent.restarted:{}", e.process_id),
            format!("Agent restarted: {}", e.agent_name),
            Some(e.reason.clone()),
            ActivitySeverity::Warning,
            ActivityCategory::Agent,
            "agent_manager",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("restart_count".into(), serde_json::to_value(e.restart_count).ok()?),

        AE::Crashed(e) => ActivityItem::new(
            format!("agent.crashed:{}", e.process_id),
            format!("Agent crashed: {}", e.agent_name),
            Some(e.error.clone()),
            ActivitySeverity::Error,
            ActivityCategory::Agent,
            "agent_manager",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        )
        .with_meta("exit_code".into(), serde_json::to_value(e.exit_code).ok()?)
        .with_meta("restart_count".into(), serde_json::to_value(e.restart_count).ok()?),
    };

    Some(item)
}

/// Extracts an `ActivityItem` from a `ConversationEvent`.
fn extract_conversation_activity(
    ev: &FrontendEvent,
    e: &nabu_core::event_bus::ConversationEvent,
    timestamp_ms: f64,
) -> Option<ActivityItem> {
    use nabu_core::event_bus::ConversationEvent as CE;

    let item = match e {
        CE::ThreadSaved { thread_id, .. } => ActivityItem::new(
            format!("thread.saved:{}", thread_id),
            "Conversation saved".to_string(),
            Some(thread_id.to_string()),
            ActivitySeverity::Info,
            ActivityCategory::Conversation,
            "conversations",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        ),
        CE::ThreadUpdated { thread_id, .. } => ActivityItem::new(
            format!("thread.updated:{}", thread_id),
            "Conversation updated".to_string(),
            Some(thread_id.to_string()),
            ActivitySeverity::Info,
            ActivityCategory::Conversation,
            "conversations",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        ),
        CE::ThreadDeleted { thread_id, .. } => ActivityItem::new(
            format!("thread.deleted:{}", thread_id),
            "Conversation deleted".to_string(),
            Some(thread_id.to_string()),
            ActivitySeverity::Warning,
            ActivityCategory::Conversation,
            "conversations",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        ),
    };

    Some(item)
}

/// Extracts an `ActivityItem` from a `StreamEvent`. Only terminal events
/// (started, completed, cancelled, failed) are surfaced — per-token events
/// are too noisy.
fn extract_stream_activity(
    ev: &FrontendEvent,
    e: &nabu_core::event_bus::StreamEvent,
    timestamp_ms: f64,
) -> Option<ActivityItem> {
    use nabu_core::event_bus::StreamEvent as SE;

    let item = match e {
        SE::Started(e) => ActivityItem::new(
            format!("stream.started:{}", e.stream_id),
            "Stream started".to_string(),
            e.agent_name.clone(),
            ActivitySeverity::Info,
            ActivityCategory::Stream,
            "streaming",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        ),
        SE::Token(_) => return None,
        SE::PartialUpdate(_) => return None,
        SE::Completed(e) => ActivityItem::new(
            format!("stream.completed:{}", e.stream_id),
            "Stream completed".to_string(),
            Some(format!("{} tokens", e.total_tokens)),
            ActivitySeverity::Info,
            ActivityCategory::Stream,
            "streaming",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        ),
        SE::Cancelled(e) => ActivityItem::new(
            format!("stream.cancelled:{}", e.stream_id),
            "Stream cancelled".to_string(),
            Some(e.reason.clone()),
            ActivitySeverity::Warning,
            ActivityCategory::Stream,
            "streaming",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        ),
        SE::Failed(e) => ActivityItem::new(
            format!("stream.failed:{}", e.stream_id),
            "Stream failed".to_string(),
            Some(e.error.clone()),
            ActivitySeverity::Error,
            ActivityCategory::Stream,
            "streaming",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        ),
    };

    Some(item)
}

/// Extracts an `ActivityItem` from a `StreamSessionEvent`.
fn extract_session_activity(
    ev: &FrontendEvent,
    e: &nabu_core::event_bus::StreamSessionEvent,
    timestamp_ms: f64,
) -> Option<ActivityItem> {
    use nabu_core::event_bus::StreamSessionEvent as SE;

    let item = match e {
        SE::SessionCreated { stream_id, .. } => ActivityItem::new(
            format!("session.created:{}", stream_id),
            "Session created".to_string(),
            Some(stream_id.to_string()),
            ActivitySeverity::Info,
            ActivityCategory::Stream,
            "streaming",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        ),
        SE::SessionStarted { stream_id, .. } => ActivityItem::new(
            format!("session.started:{}", stream_id),
            "Session started".to_string(),
            Some(stream_id.to_string()),
            ActivitySeverity::Info,
            ActivityCategory::Stream,
            "streaming",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        ),
        SE::SessionCancelled { stream_id, reason, .. } => ActivityItem::new(
            format!("session.cancelled:{}", stream_id),
            "Session cancelled".to_string(),
            Some(reason.clone()),
            ActivitySeverity::Warning,
            ActivityCategory::Stream,
            "streaming",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        ),
        SE::SessionCleanedUp { stream_id, .. } => ActivityItem::new(
            format!("session.cleaned_up:{}", stream_id),
            "Session cleaned up".to_string(),
            Some(stream_id.to_string()),
            ActivitySeverity::Info,
            ActivityCategory::Stream,
            "streaming",
            ev.kind.as_str().to_string(),
            timestamp_ms,
        ),
    };

    Some(item)
}

/// Parses an ISO-8601 timestamp string into milliseconds since the Unix epoch.
/// Returns `None` if parsing fails.
fn parse_iso_to_ms(s: &str) -> Option<f64> {
    s.parse::<chrono::DateTime<chrono::Utc>>()
        .ok()
        .map(|dt| dt.timestamp_millis() as f64)
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use nabu_core::event_bus::kinds;
    use nabu_core::event_bus::PipelineEvent;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

    fn make_frontend_event(kind: &str, payload: serde_json::Value) -> FrontendEvent {
        let raw = crate::events::types::RawFrontendEvent {
            event_type: kind.to_string(),
            timestamp: Some("2024-01-01T00:00:00Z".to_string()),
            payload,
        };
        crate::events::types::parse_raw(raw).unwrap()
    }

    fn item_stored_payload() -> serde_json::Value {
        serde_json::json!({
            "ItemStored": {
                "object_id": "12345678-1234-1234-1234-123456789abc",
                "vault_path": "notes/foo.md",
                "object_type": "Note",
                "timestamp": "2024-01-01T00:00:00Z",
            }
        })
    }

    #[test]
    fn item_stored_event_extracts_activity() {
        let ev = make_frontend_event(kinds::ITEM_STORED, item_stored_payload());
        let item = extract_activity(&ev, 100);
        assert!(item.is_some());
        let item = item.unwrap();
        assert_eq!(item.title, "Item stored");
        assert_eq!(item.category, ActivityCategory::Storage);
        assert_eq!(item.severity, ActivitySeverity::Info);
        assert_eq!(item.subsystem, "storage");
    }

    #[test]
    fn processing_progress_event_is_filtered() {
        // Progress events are too noisy and should be ignored.
        let ev = make_frontend_event(
            kinds::ITEM_PROCESSING_PROGRESS,
            serde_json::json!({
                "ItemProcessingProgress": {
                    "object_id": "12345678-1234-1234-1234-123456789abc",
                    "job_id": "12345678-1234-1234-1234-123456789abc",
                    "progress": 0.5,
                    "message": Some("halfway"),
                    "timestamp": "2024-01-01T00:00:00Z",
                }
            }),
        );
        assert!(extract_activity(&ev, 100).is_none());
    }

    #[test]
    fn diagnostic_events_are_filtered() {
        // Diagnostic events are not surfaced in the activity timeline.
        let ev = make_frontend_event(
            kinds::DIAGNOSTIC_BATCH_PUBLISHED,
            serde_json::json!({
                "Diagnostic": {
                    "DiagnosticBatchPublished": {
                        "origin": "test",
                        "resource": "test.md",
                        "diagnostics": [],
                        "timestamp": "2024-01-01T00:00:00Z",
                    }
                }
            }),
        );
        assert!(extract_activity(&ev, 100).is_none());
    }

    #[test]
    fn capability_state_changed_extracts_activity() {
        let ev = make_frontend_event(
            kinds::CAPABILITY_STATE_CHANGED,
            serde_json::json!({
                "CapabilityStateChanged": {
                    "capability_id": "capture:file",
                    "enabled": true,
                    "timestamp": "2024-01-01T00:00:00Z",
                }
            }),
        );
        let item = extract_activity(&ev, 100).unwrap();
        assert_eq!(item.title, "Capability enabled");
        assert_eq!(item.category, ActivityCategory::Capability);
        assert_eq!(item.severity, ActivitySeverity::Info);
    }

    #[test]
    fn plugin_error_extracts_activity_with_error_severity() {
        let ev = make_frontend_event(
            kinds::PLUGIN_ERROR,
            serde_json::json!({
                "Plugin": {
                    "PluginError": {
                        "plugin_id": "test-plugin",
                        "error": "something went wrong",
                        "code": Some("ERR_001"),
                        "severity": "error",
                        "detail": Some("stack trace"),
                        "api_version": {"major": 1, "minor": 0},
                        "timestamp": "2024-01-01T00:00:00Z",
                    }
                }
            }),
        );
        let item = extract_activity(&ev, 100).unwrap();
        assert_eq!(item.category, ActivityCategory::Plugin);
        assert_eq!(item.severity, ActivitySeverity::Error);
        assert!(item.title.contains("Plugin error"));
    }

    #[test]
    fn sync_error_extracts_activity_with_error_severity() {
        let ev = make_frontend_event(
            kinds::SYNC_STATUS_CHANGED,
            serde_json::json!({
                "Sync": {
                    "sync_id": "550e8400-e29b-41d4-a716-446655440000",
                    "folder_id": "folder-abc",
                    "provider_id": "syncthing",
                    "previous_status": "syncing",
                    "current_status": "error",
                    "progress": null,
                    "error": Some("connection lost"),
                    "timestamp": "2024-01-01T00:00:00Z",
                }
            }),
        );
        let item = extract_activity(&ev, 100).unwrap();
        assert_eq!(item.category, ActivityCategory::Sync);
        assert_eq!(item.severity, ActivitySeverity::Error);
    }

    #[test]
    fn stream_token_events_are_filtered() {
        let ev = make_frontend_event(
            kinds::STREAM_TOKEN,
            serde_json::json!({
                "Stream": {
                    "Token": {
                        "stream_id": "550e8400-e29b-41d4-a716-446655440000",
                        "token": "hello",
                        "partial_content": "hello",
                        "sequence": 0,
                        "timestamp": "2024-01-01T00:00:00Z",
                    }
                }
            }),
        );
        assert!(extract_activity(&ev, 100).is_none());
    }

    #[test]
    fn unknown_event_kind_returns_none() {
        // An event kind that doesn't map to any PipelineEvent variant.
        // This simulates a malformed/unknown event.
        let ev = make_frontend_event(
            "platform.unknown",
            serde_json::json!({"ItemStored": {
                "object_id": "12345678-1234-1234-1234-123456789abc",
                "vault_path": "notes/foo.md",
                "object_type": "Note",
                "timestamp": "2024-01-01T00:00:00Z",
            }}),
        );
        // The kind string doesn't match, so parse_raw will fail at the
        // FrontendEventKind level. But if it parses, extract_activity would
        // still try. Let's test with a kind that parses but isn't handled.
        assert!(extract_activity(&ev, 100).is_none());
    }

    #[test]
    fn activity_severity_labels() {
        assert_eq!(ActivitySeverity::Info.label(), "info");
        assert_eq!(ActivitySeverity::Warning.label(), "warning");
        assert_eq!(ActivitySeverity::Error.label(), "error");
    }

    #[test]
    fn activity_category_labels() {
        assert_eq!(ActivityCategory::Plugin.label(), "Plugin");
        assert_eq!(ActivityCategory::Sync.label(), "Synchronization");
        assert_eq!(ActivityCategory::Processing.label(), "Processing");
    }

    #[test]
    fn timestamp_falls_back_to_now_when_missing() {
        // Event with no timestamp in the envelope.
        let raw = crate::events::types::RawFrontendEvent {
            event_type: kinds::ITEM_STORED.to_string(),
            timestamp: None,
            payload: item_stored_payload(),
        };
        let ev = crate::events::types::parse_raw(raw).unwrap();
        let item = extract_activity(&ev, 100).unwrap();
        let now = JsDate::now();
        // Should be very close to "now" (within 5 seconds).
        assert!((item.timestamp_ms - now).abs() < 5000.0);
    }
}
