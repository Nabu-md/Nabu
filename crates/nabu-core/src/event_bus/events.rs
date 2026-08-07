use crate::diagnostic::events::DiagnosticEvent;
use crate::models::{CaptureSource, ObjectType};
use crate::plugin::events::PluginEvent;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// All pipeline lifecycle events emitted through the EventBus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PipelineEvent {
    /// A new item has been captured and enqueued
    ItemCaptured(ItemCapturedEvent),
    /// Processing of an item has started
    ItemProcessingStarted(ItemProcessingStartedEvent),
    /// Processing progress update (0.0–1.0)
    ItemProcessingProgress(ItemProcessingProgressEvent),
    /// Processing completed successfully
    ItemProcessingCompleted(ItemProcessingCompletedEvent),
    /// Processing failed
    ItemProcessingFailed(ItemProcessingFailedEvent),
    /// Item has been stored permanently
    ItemStored(ItemStoredEvent),
    /// Index has been updated
    IndexUpdated(IndexUpdatedEvent),
    /// Graph has been updated
    GraphUpdated(GraphUpdatedEvent),
    /// Item has been cancelled
    ItemCancelled(ItemCancelledEvent),
    /// Item has been retried
    ItemRetried(ItemRetriedEvent),
    /// A capability was enabled or disabled
    CapabilityStateChanged(CapabilityStateEvent),
    /// A shared plugin event flowing through the EventBus.
    ///
    /// Plugins never construct `PipelineEvent` directly — they create a
    /// [`PluginEvent`](crate::plugin::events::PluginEvent) and publish it
    /// via [`publish_plugin_event`](crate::plugin::events::publish_plugin_event),
    /// which wraps it in this variant.
    Plugin(PluginEvent),
    /// A diagnostic event flowing through the EventBus.
    ///
    /// Every diagnostic producer (spell checker, grammar engine, AI assistant,
    /// plugin, LSP adapter, OCR engine, metadata validator, etc.) publishes
    /// diagnostics by creating a [`DiagnosticEvent`] and publishing it via
    /// [`publish_diagnostic_event`](crate::diagnostic::events::publish_diagnostic_event),
    /// which wraps it in this variant.
    Diagnostic(DiagnosticEvent),
    /// A process supervision event flowing through the EventBus.
    ///
    /// Published by the [`ProcessSupervisor`](crate::process_supervisor::ProcessSupervisor)
    /// when a managed subprocess starts, exits, fails, restarts, or stops.
    Process(ProcessEvent),
}

/// Event kind string constants for EventBus subscriptions
pub mod kinds {
    pub const ITEM_CAPTURED: &str = "item.captured";
    pub const ITEM_PROCESSING_STARTED: &str = "item.processing.started";
    pub const ITEM_PROCESSING_PROGRESS: &str = "item.processing.progress";
    pub const ITEM_PROCESSING_COMPLETED: &str = "item.processing.completed";
    pub const ITEM_PROCESSING_FAILED: &str = "item.processing.failed";
    pub const ITEM_STORED: &str = "item.stored";
    pub const INDEX_UPDATED: &str = "index.updated";
    pub const GRAPH_UPDATED: &str = "graph.updated";
    pub const ITEM_CANCELLED: &str = "item.cancelled";
    pub const ITEM_RETRIED: &str = "item.retried";
    /// A capability was enabled or disabled.
    pub const CAPABILITY_STATE_CHANGED: &str = "capability.state.changed";

    // --- Plugin event kinds (shared plugin event contract) ---
    // These constants are defined here (in event_bus::kinds) rather than in
    // plugin::events to keep ALL event kind strings centralized for the
    // EventBus subscription and frontend bridge. The PluginEvent::kind()
    // method in plugin::events references these same constants.

    /// A plugin was loaded.
    pub const PLUGIN_LOADED: &str = "plugin.loaded";
    /// A plugin was unloaded.
    pub const PLUGIN_UNLOADED: &str = "plugin.unloaded";
    /// A capability was registered by a plugin or the platform.
    pub const CAPABILITY_REGISTERED: &str = "capability.registered";
    /// A capability was removed.
    pub const CAPABILITY_REMOVED: &str = "capability.removed";
    /// A plugin emitted a warning.
    pub const PLUGIN_WARNING: &str = "plugin.warning";
    /// A plugin emitted an error.
    pub const PLUGIN_ERROR: &str = "plugin.error";
    /// A plugin made a request to a platform capability.
    pub const PLUGIN_REQUEST: &str = "plugin.request";
    /// A platform capability responded to a plugin request.
    pub const PLUGIN_RESPONSE: &str = "plugin.response";
    /// A plugin was registered (manifest accepted by the PluginManager).
    pub const PLUGIN_REGISTERED: &str = "plugin.registered";
    /// A plugin was unregistered (manifest removed from the PluginManager).
    pub const PLUGIN_UNREGISTERED: &str = "plugin.unregistered";
    /// A plugin started executing (runtime activated).
    pub const PLUGIN_STARTED: &str = "plugin.started";
    /// A plugin stopped executing (runtime deactivated).
    pub const PLUGIN_STOPPED: &str = "plugin.stopped";

    // --- Process supervision event kinds ---
    // These are published by the ProcessSupervisor through the EventBus,
    // following the same pattern as capability and plugin events.

    /// A managed process has started.
    pub const PROCESS_STARTED: &str = "process.started";
    /// A managed process has exited.
    pub const PROCESS_EXITED: &str = "process.exited";
    /// A managed process has failed.
    pub const PROCESS_FAILED: &str = "process.failed";
    /// A managed process has been restarted.
    pub const PROCESS_RESTARTED: &str = "process.restarted";
    /// A managed process has been stopped.
    pub const PROCESS_STOPPED: &str = "process.stopped";

    // --- Diagnostic event kinds ---
    // Published by diagnostic producers (spell checkers, AI assistants,
    // plugins, LSP adapters, OCR engines, metadata validators, etc.)
    // through the EventBus via DiagnosticEvent to PipelineEvent::Diagnostic.

    /// A batch of diagnostics was published (created or updated).
    pub const DIAGNOSTIC_BATCH_PUBLISHED: &str = "diagnostic.batch.published";
    /// All diagnostics from an origin were cleared for a resource.
    pub const DIAGNOSTIC_BATCH_CLEARED: &str = "diagnostic.batch.cleared";
    /// A specific previously-published diagnostic batch was removed.
    pub const DIAGNOSTIC_BATCH_REMOVED: &str = "diagnostic.batch.removed";
}

impl PipelineEvent {
    pub fn kind(&self) -> &'static str {
        match self {
            PipelineEvent::ItemCaptured(_) => kinds::ITEM_CAPTURED,
            PipelineEvent::ItemProcessingStarted(_) => kinds::ITEM_PROCESSING_STARTED,
            PipelineEvent::ItemProcessingProgress(_) => kinds::ITEM_PROCESSING_PROGRESS,
            PipelineEvent::ItemProcessingCompleted(_) => kinds::ITEM_PROCESSING_COMPLETED,
            PipelineEvent::ItemProcessingFailed(_) => kinds::ITEM_PROCESSING_FAILED,
            PipelineEvent::ItemStored(_) => kinds::ITEM_STORED,
            PipelineEvent::IndexUpdated(_) => kinds::INDEX_UPDATED,
            PipelineEvent::GraphUpdated(_) => kinds::GRAPH_UPDATED,
            PipelineEvent::ItemCancelled(_) => kinds::ITEM_CANCELLED,
            PipelineEvent::ItemRetried(_) => kinds::ITEM_RETRIED,
            PipelineEvent::CapabilityStateChanged(_) => kinds::CAPABILITY_STATE_CHANGED,
            PipelineEvent::Plugin(e) => e.kind(),
            PipelineEvent::Process(e) => e.kind(),
            PipelineEvent::Diagnostic(e) => e.kind(),
        }
    }

    /// Returns the event's timestamp, if available.
    ///
    /// Every `PipelineEvent` variant carries a `timestamp: DateTime<Utc>`
    /// field. This accessor exposes it uniformly so that downstream consumers
    /// (notably the EventBus→Tauri bridge) can attach a top-level timestamp to
    /// frontend event envelopes without pattern-matching on every variant.
    pub fn timestamp(&self) -> Option<DateTime<Utc>> {
        match self {
            PipelineEvent::ItemCaptured(e) => Some(e.timestamp),
            PipelineEvent::ItemProcessingStarted(e) => Some(e.timestamp),
            PipelineEvent::ItemProcessingProgress(e) => Some(e.timestamp),
            PipelineEvent::ItemProcessingCompleted(e) => Some(e.timestamp),
            PipelineEvent::ItemProcessingFailed(e) => Some(e.timestamp),
            PipelineEvent::ItemStored(e) => Some(e.timestamp),
            PipelineEvent::IndexUpdated(e) => Some(e.timestamp),
            PipelineEvent::GraphUpdated(e) => Some(e.timestamp),
            PipelineEvent::ItemCancelled(e) => Some(e.timestamp),
            PipelineEvent::ItemRetried(e) => Some(e.timestamp),
            PipelineEvent::CapabilityStateChanged(e) => Some(e.timestamp),
            PipelineEvent::Plugin(e) => Some(e.timestamp()),
            PipelineEvent::Process(e) => Some(e.timestamp()),
            PipelineEvent::Diagnostic(e) => Some(e.timestamp()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCapturedEvent {
    pub object_id: Uuid,
    pub object_type: ObjectType,
    pub capture_source: CaptureSource,
    pub title: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub job_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessingStartedEvent {
    pub object_id: Uuid,
    pub job_id: Uuid,
    pub processor_name: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessingProgressEvent {
    pub object_id: Uuid,
    pub job_id: Uuid,
    pub progress: f64,
    pub message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessingCompletedEvent {
    pub object_id: Uuid,
    pub job_id: Uuid,
    pub processor_name: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemProcessingFailedEvent {
    pub object_id: Uuid,
    pub job_id: Uuid,
    pub processor_name: String,
    pub error: String,
    pub retry_count: u32,
    pub will_retry: bool,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemStoredEvent {
    pub object_id: Uuid,
    pub vault_path: String,
    pub object_type: ObjectType,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexUpdatedEvent {
    pub object_id: Uuid,
    pub operation: IndexOperation,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IndexOperation {
    Added,
    Updated,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphUpdatedEvent {
    pub object_id: Uuid,
    pub operation: GraphOperation,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphOperation {
    NodeAdded,
    NodeUpdated,
    NodeRemoved,
    EdgeAdded,
    EdgeRemoved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemCancelledEvent {
    pub object_id: Uuid,
    pub job_id: Uuid,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemRetriedEvent {
    pub object_id: Uuid,
    pub job_id: Uuid,
    pub retry_count: u32,
    pub max_retries: u32,
    pub timestamp: DateTime<Utc>,
}

/// A capability was enabled or disabled at runtime.
///
/// Published on the `capability.state.changed` kind whenever the
/// [`crate::plugin::capability::CapabilityRegistry`] enables or disables a
/// capability. The EventBus bridge forwards these to the UI as
/// `nabu-event` payloads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityStateEvent {
    /// Full capability identifier (`namespace:name`).
    pub capability_id: String,
    /// Whether the capability is now enabled.
    pub enabled: bool,
    pub timestamp: DateTime<Utc>,
}

impl ItemCapturedEvent {
    pub fn new(
        object_id: Uuid,
        object_type: ObjectType,
        capture_source: CaptureSource,
        title: Option<String>,
        job_id: Option<Uuid>,
    ) -> Self {
        Self {
            object_id,
            object_type,
            capture_source,
            title,
            timestamp: Utc::now(),
            job_id,
        }
    }
}

impl ItemStoredEvent {
    pub fn new(object_id: Uuid, vault_path: String, object_type: ObjectType) -> Self {
        Self {
            object_id,
            vault_path,
            object_type,
            timestamp: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Process supervision events
// ---------------------------------------------------------------------------

/// A unique identifier for a managed subprocess.
///
/// This is a `Uuid` allocated when the [`ProcessSupervisor`] spawns the
/// process. It is stable for the lifetime of the managed process and is
/// used to correlate events, queries, and operations.
pub type ProcessId = Uuid;

/// All process supervision events published through the EventBus.
///
/// Published by the [`ProcessSupervisor`](crate::process_supervisor::ProcessSupervisor)
/// when a managed subprocess transitions through its lifecycle. The
/// `PipelineEvent::Process` variant wraps a `ProcessEvent` for EventBus
/// transport.
///
/// Each variant wraps a dedicated event struct that derives
/// [`Serialize`] and [`Deserialize`] and carries a `timestamp` field
/// for uniform access via [`PipelineEvent::timestamp`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ProcessEvent {
    /// A managed process has started.
    ///
    /// Carries the process ID, name, PID (when available), and command.
    Started(ProcessStartedEvent),
    /// A managed process has exited.
    ///
    /// Carries the process ID, name, and exit code. The restart count
    /// reflects how many times the process has been restarted by the
    /// supervisor.
    Exited(ProcessExitedEvent),
    /// A managed process has failed.
    ///
    /// Carries the process ID, name, exit code (when available), error
    /// message, and restart count.
    Failed(ProcessFailedEvent),
    /// A managed process has been restarted.
    ///
    /// Carries the process ID, name, restart count, and the cause that
    /// triggered the restart.
    Restarted(ProcessRestartEvent),
    /// A managed process has been stopped.
    ///
    /// Carries the process ID, name, and reason for the stop.
    Stopped(ProcessStoppedEvent),
}

impl ProcessEvent {
    /// Returns the event kind string used for EventBus subscription.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Started(_) => kinds::PROCESS_STARTED,
            Self::Exited(_) => kinds::PROCESS_EXITED,
            Self::Failed(_) => kinds::PROCESS_FAILED,
            Self::Restarted(_) => kinds::PROCESS_RESTARTED,
            Self::Stopped(_) => kinds::PROCESS_STOPPED,
        }
    }

    /// Returns the timestamp when this event was produced.
    ///
    /// Delegates to the inner event struct's `timestamp` field, so that
    /// [`PipelineEvent::timestamp`] can access it uniformly across all
    /// variants without pattern-matching each inner struct.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::Started(e) => e.timestamp,
            Self::Exited(e) => e.timestamp,
            Self::Failed(e) => e.timestamp,
            Self::Restarted(e) => e.timestamp,
            Self::Stopped(e) => e.timestamp,
        }
    }
}

/// Published when a managed process has started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessStartedEvent {
    /// The unique identifier assigned to this managed process.
    pub process_id: ProcessId,
    /// The human-readable name from the process configuration.
    pub name: String,
    /// The OS process ID, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// The command that was executed.
    pub command: String,
    /// Command-line arguments.
    pub args: Vec<String>,
    /// Working directory for the process.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl ProcessStartedEvent {
    pub fn new(
        process_id: ProcessId,
        name: &str,
        pid: Option<u32>,
        command: &str,
        args: &[String],
        working_dir: Option<&str>,
    ) -> Self {
        Self {
            process_id,
            name: name.to_string(),
            pid,
            command: command.to_string(),
            args: args.to_vec(),
            working_dir: working_dir.map(|s| s.to_string()),
            timestamp: Utc::now(),
        }
    }
}

/// Published when a managed process has exited (successfully or not).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessExitedEvent {
    /// The unique identifier assigned to this managed process.
    pub process_id: ProcessId,
    /// The human-readable name from the process configuration.
    pub name: String,
    /// The exit code, when available.
    /// `None` indicates the process was terminated by a signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// How many times the supervisor has restarted this process.
    pub restart_count: u32,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl ProcessExitedEvent {
    pub fn new(process_id: ProcessId, name: &str, exit_code: Option<i32>, restart_count: u32) -> Self {
        Self {
            process_id,
            name: name.to_string(),
            exit_code,
            restart_count,
            timestamp: Utc::now(),
        }
    }
}

/// Published when a managed process has failed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessFailedEvent {
    /// The unique identifier assigned to this managed process.
    pub process_id: ProcessId,
    /// The human-readable name from the process configuration.
    pub name: String,
    /// The exit code, when available.
    /// `None` indicates the process was terminated by a signal or never
    /// produced an exit status (e.g. spawn failure).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Human-readable error message describing the failure.
    pub error: String,
    /// How many times the supervisor has restarted this process.
    pub restart_count: u32,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl ProcessFailedEvent {
    pub fn new(
        process_id: ProcessId,
        name: &str,
        exit_code: Option<i32>,
        error: &str,
        restart_count: u32,
    ) -> Self {
        Self {
            process_id,
            name: name.to_string(),
            exit_code,
            error: error.to_string(),
            restart_count,
            timestamp: Utc::now(),
        }
    }
}

/// Published when a managed process has been restarted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessRestartEvent {
    /// The unique identifier assigned to this managed process.
    pub process_id: ProcessId,
    /// The human-readable name from the process configuration.
    pub name: String,
    /// How many times the supervisor has restarted this process (including
    /// the restart that triggered this event).
    pub restart_count: u32,
    /// The reason for the restart (e.g. "process exited", "spawn failure").
    pub reason: String,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl ProcessRestartEvent {
    pub fn new(process_id: ProcessId, name: &str, restart_count: u32, reason: &str) -> Self {
        Self {
            process_id,
            name: name.to_string(),
            restart_count,
            reason: reason.to_string(),
            timestamp: Utc::now(),
        }
    }
}

/// Published when a managed process has been stopped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessStoppedEvent {
    /// The unique identifier assigned to this managed process.
    pub process_id: ProcessId,
    /// The human-readable name from the process configuration.
    pub name: String,
    /// The reason for the stop (e.g. "supervisor shutdown", "user requested stop").
    pub reason: String,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl ProcessStoppedEvent {
    pub fn new(process_id: ProcessId, name: &str, reason: &str) -> Self {
        Self {
            process_id,
            name: name.to_string(),
            reason: reason.to_string(),
            timestamp: Utc::now(),
        }
    }
}
