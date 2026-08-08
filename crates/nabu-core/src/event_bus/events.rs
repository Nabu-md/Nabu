use crate::diagnostic::events::DiagnosticEvent;
use crate::models::{CaptureSource, ObjectType};
use crate::plugin::events::PluginEvent;
use crate::sync::SyncStatusChanged;
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
    
    /// A synchronization status-change event flowing through the EventBus.
    ///
    /// Every synchronization provider (Syncthing, iCloud, Git, WebDAV, etc.)
    /// publishes status changes by creating a [`SyncStatusChanged`] and
    /// publishing it via
    /// [`publish_sync_status_changed`](crate::sync::events::publish_sync_status_changed),
    /// which wraps it in this variant.
    Sync(SyncStatusChanged),
    /// A process supervision event flowing through the EventBus.
    ///
    /// Published by the [`ProcessSupervisor`](crate::process_supervisor::ProcessSupervisor)
    /// when a managed subprocess starts, exits, fails, restarts, or stops.
    Process(ProcessEvent),
    /// An agent management event flowing through the EventBus.
    ///
    /// Published by the [`AgentManager`](crate::agent::manager::AgentManager)
    /// when an agent is started, stopped, restarted, or crashes. The
    /// `ProcessSupervisor` publishes [`ProcessEvent`]s for the underlying OS
    /// process; the `AgentManager` publishes `AgentEvent`s for the
    /// higher-level management lifecycle.
    Agent(AgentEvent),
    /// A conversation persistence event flowing through the EventBus.
    ///
    /// Published by the [`ConversationStore`](crate::conversations::ConversationStore)
    /// when a thread is saved, updated, or deleted. The event bridge forwards these
    /// to the frontend so UI components can react to conversation changes.
    Conversation(ConversationEvent),
    /// A streaming event flowing through the EventBus.
    ///
    /// Published by the [`StreamingPipeline`](crate::streaming::StreamingPipeline)
    /// when tokens are produced, a stream starts, completes, is cancelled, or fails.
    /// Every streamed token passes through the EventBus as the single transport,
    /// enabling frontend subscribers to reconstruct responses incrementally.
    Stream(StreamEvent),
    /// A streaming session lifecycle event flowing through the EventBus.
    ///
    /// Published by the [`StreamingSession`](crate::streaming::StreamingSession)
    /// and [`StreamingPipeline`](crate::streaming::StreamingPipeline) to signal
    /// session-level state transitions (created, started, cancelled, cleaned up).
    /// These events are distinct from per-token [`StreamEvent`] variants and
    /// allow subscribers to track session lifecycle independently.
    Session(StreamSessionEvent),
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
    /// A managed process's health status has changed.
    pub const PROCESS_HEALTH_CHANGED: &str = "process.health.changed";

    // --- Agent management event kinds ---
    // Published by the AgentManager through the EventBus when agents are
    // started, stopped, restarted, or crashed. These complement the
    // process-level events above with higher-level management lifecycle events.

    /// An agent has been started.
    pub const AGENT_STARTED: &str = "agent.started";
    /// An agent has been stopped.
    pub const AGENT_STOPPED: &str = "agent.stopped";
    /// An agent has been restarted.
    pub const AGENT_RESTARTED: &str = "agent.restarted";
    /// An agent has crashed (unexpected process exit).
    pub const AGENT_CRASHED: &str = "agent.crashed";

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

    // --- Synchronization event kinds ---
    // Published by synchronization providers (Syncthing, iCloud, Git, WebDAV, etc.)
    // through the EventBus. Providers create `SyncStatusChanged` events and
    // publish via `publish_sync_status_changed`, which wraps them in
    // `PipelineEvent::Sync(...)` for EventBus transport.

    /// A synchronization folder's status has changed.
    pub const SYNC_STATUS_CHANGED: &str = "sync.status.changed";

    // --- Conversation persistence event kinds ---
    // Published by the ConversationStore when threads are saved, updated, or
    // deleted. The EventBus bridge forwards these to the frontend so UI
    // components can react to conversation changes.

    /// A thread was saved to persistent storage.
    pub const THREAD_SAVED: &str = "thread.saved";
    /// A thread was updated in persistent storage.
    pub const THREAD_UPDATED: &str = "thread.updated";
    /// A thread was deleted from persistent storage.
    pub const THREAD_DELETED: &str = "thread.deleted";

    // --- Streaming event kinds ---
    // Published by the streaming pipeline and session manager through the
    // EventBus. These events carry incremental token output from agent
    // processes to frontend subscribers.

    /// A stream was started (session created and publishing beginning).
    pub const STREAM_STARTED: &str = "stream.started";
    /// A token was received and published to the stream.
    pub const STREAM_TOKEN: &str = "stream.token";
    /// A partial content update was published (aggregated tokens so far).
    pub const STREAM_PARTIAL_UPDATE: &str = "stream.partial_update";
    /// A stream was completed normally (all tokens delivered).
    pub const STREAM_COMPLETED: &str = "stream.completed";
    /// A stream was cancelled before completion.
    pub const STREAM_CANCELLED: &str = "stream.cancelled";
    /// A stream failed due to an error.
    pub const STREAM_FAILED: &str = "stream.failed";

    // --- Streaming session lifecycle event kinds ---

    /// A streaming session was created.
    pub const SESSION_CREATED: &str = "session.created";
    /// A streaming session was started.
    pub const SESSION_STARTED: &str = "session.started";
    /// A streaming session was cancelled.
    pub const SESSION_CANCELLED: &str = "session.cancelled";
    /// A streaming session was cleaned up and removed.
    pub const SESSION_CLEANED_UP: &str = "session.cleaned_up";
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
            PipelineEvent::Agent(e) => e.kind(),
            PipelineEvent::Diagnostic(e) => e.kind(),
            PipelineEvent::Sync(e) => e.kind(),
            PipelineEvent::Conversation(e) => e.kind(),
            PipelineEvent::Stream(e) => e.kind(),
            PipelineEvent::Session(e) => e.kind(),
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
            PipelineEvent::Agent(e) => Some(e.timestamp()),
            PipelineEvent::Diagnostic(e) => Some(e.timestamp()),
            PipelineEvent::Sync(e) => Some(e.timestamp()),
            PipelineEvent::Conversation(e) => Some(e.timestamp()),
            PipelineEvent::Stream(e) => Some(e.timestamp()),
            PipelineEvent::Session(e) => Some(e.timestamp()),
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
    Stopped(ProcessStoppedEvent),
    /// A managed process's health status has changed.
    ///
    /// Carries the process ID, name, new health status, and the process
    /// state that triggered the change.
    #[cfg(not(target_arch = "wasm32"))]
    HealthChanged(ProcessHealthChangedEvent),
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
            #[cfg(not(target_arch = "wasm32"))]
            Self::HealthChanged(_) => kinds::PROCESS_HEALTH_CHANGED,
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
            #[cfg(not(target_arch = "wasm32"))]
            Self::HealthChanged(e) => e.timestamp,
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

/// The health status of a managed process, used in
/// [`ProcessHealthChangedEvent`].
///
/// This mirrors [`crate::process_supervisor::health::ProcessHealthStatus`]
/// but is defined here to avoid a circular dependency between the `event_bus`
/// and `process_supervisor` modules. The two types are kept in sync
/// manually — changes to one must be reflected in the other.
///
/// Only available on non-wasm32 targets, where the process supervisor
/// module is compiled.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessHealthStatus {
    /// The process is running normally.
    Healthy,
    /// The process is starting or restarting.
    Starting,
    /// The process is running but experiencing issues.
    Degraded,
    /// The process has exited or failed and is not running.
    Unhealthy,
    /// The process has been stopped and will not be restarted.
    Stopped,
    /// Health could not be determined.
    Unknown,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for ProcessHealthStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ProcessHealthStatus {
    /// Returns a human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Starting => "starting",
            Self::Degraded => "degraded",
            Self::Unhealthy => "unhealthy",
            Self::Stopped => "stopped",
            Self::Unknown => "unknown",
        }
    }

    /// Returns `true` if the process is in a healthy state.
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl std::fmt::Display for ProcessHealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Published when a managed process's health status changes.
///
/// This event is emitted by the [`ProcessSupervisor`](crate::process_supervisor::ProcessSupervisor)
/// whenever a process transitions between health states. Subscribers can
/// listen on the `process.health.changed` kind to receive real-time health
/// updates without polling.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcessHealthChangedEvent {
    /// The unique identifier of the managed process.
    pub process_id: ProcessId,
    /// The human-readable name from the process configuration.
    pub name: String,
    /// The new health status.
    pub status: ProcessHealthStatus,
    /// The process state that produced this health status.
    pub state: crate::process_supervisor::ProcessState,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

#[cfg(not(target_arch = "wasm32"))]
impl ProcessHealthChangedEvent {
    pub fn new(
        process_id: ProcessId,
        name: &str,
        status: ProcessHealthStatus,
        state: crate::process_supervisor::ProcessState,
    ) -> Self {
        Self {
            process_id,
            name: name.to_string(),
            status,
            state,
            timestamp: Utc::now(),
        }
    }
}

/// Convenience conversion from the process_supervisor health status to the
/// event_bus health status.
///
/// These types are kept separate to avoid a circular dependency, but the
/// values map 1:1.
#[cfg(not(target_arch = "wasm32"))]
impl From<crate::process_supervisor::health::ProcessHealthStatus> for ProcessHealthStatus {
    fn from(status: crate::process_supervisor::health::ProcessHealthStatus) -> Self {
        match status {
            crate::process_supervisor::health::ProcessHealthStatus::Healthy => Self::Healthy,
            crate::process_supervisor::health::ProcessHealthStatus::Starting => Self::Starting,
            crate::process_supervisor::health::ProcessHealthStatus::Degraded => Self::Degraded,
            crate::process_supervisor::health::ProcessHealthStatus::Unhealthy => Self::Unhealthy,
            crate::process_supervisor::health::ProcessHealthStatus::Stopped => Self::Stopped,
            crate::process_supervisor::health::ProcessHealthStatus::Unknown => Self::Unknown,
        }
    }
}

// ---------------------------------------------------------------------------
// Conversation persistence events
// ---------------------------------------------------------------------------

/// All conversation persistence events published through the EventBus.
///
/// Published by the [`ConversationStore`](crate::conversations::ConversationStore)
/// when threads are saved, updated, or deleted. The EventBus bridge forwards
/// these to the frontend over the `nabu-event` channel so UI components can
/// react to conversation changes.
///
/// Each variant carries the affected thread's ID and a timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ConversationEvent {
    /// A thread was saved (newly created or replaced).
    ThreadSaved {
        /// The ID of the thread that was saved.
        thread_id: Uuid,
        /// When the event was produced.
        timestamp: DateTime<Utc>,
    },
    /// A thread was updated (existing thread modified in place).
    ThreadUpdated {
        /// The ID of the thread that was updated.
        thread_id: Uuid,
        /// When the event was produced.
        timestamp: DateTime<Utc>,
    },
    /// A thread was deleted.
    ThreadDeleted {
        /// The ID of the thread that was deleted.
        thread_id: Uuid,
        /// When the event was produced.
        timestamp: DateTime<Utc>,
    },
}

impl ConversationEvent {
    /// Returns the event kind string used for EventBus subscription.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::ThreadSaved { .. } => kinds::THREAD_SAVED,
            Self::ThreadUpdated { .. } => kinds::THREAD_UPDATED,
            Self::ThreadDeleted { .. } => kinds::THREAD_DELETED,
        }
    }

    /// Returns the timestamp when this event was produced.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::ThreadSaved { timestamp, .. }
            | Self::ThreadUpdated { timestamp, .. }
            | Self::ThreadDeleted { timestamp, .. } => *timestamp,
        }
    }
}

// ---------------------------------------------------------------------------
// Agent management events
// ---------------------------------------------------------------------------

/// All agent lifecycle events published through the EventBus.
///
/// Published by the [`AgentManager`](crate::agent::manager::AgentManager)
/// when an agent is started, stopped, restarted, or crashes. The
/// `ProcessSupervisor` publishes [`ProcessEvent`]s for the underlying OS
/// process; the `AgentManager` publishes `AgentEvent`s for the
/// higher-level management lifecycle.
///
/// Each variant wraps a dedicated event struct that derives
/// [`Serialize`] and [`Deserialize`] and carries a `timestamp` field
/// for uniform access via [`PipelineEvent::timestamp`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AgentEvent {
    /// An agent has been started.
    ///
    /// Carries the process ID, agent name, and the agent kind.
    Started(AgentStartedEvent),
    /// An agent has been stopped.
    ///
    /// Carries the process ID, agent name, and the reason for the stop.
    Stopped(AgentStoppedEvent),
    /// An agent has been restarted.
    ///
    /// Carries the process ID, agent name, restart count, and the cause.
    Restarted(AgentRestartedEvent),
    /// An agent has crashed (unexpected process exit).
    ///
    /// Carries the process ID, agent name, exit code (when available), and
    /// the error message.
    Crashed(AgentCrashedEvent),
}

impl AgentEvent {
    /// Returns the event kind string used for EventBus subscription.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Started(_) => kinds::AGENT_STARTED,
            Self::Stopped(_) => kinds::AGENT_STOPPED,
            Self::Restarted(_) => kinds::AGENT_RESTARTED,
            Self::Crashed(_) => kinds::AGENT_CRASHED,
        }
    }

    /// Returns the timestamp when this event was produced.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::Started(e) => e.timestamp,
            Self::Stopped(e) => e.timestamp,
            Self::Restarted(e) => e.timestamp,
            Self::Crashed(e) => e.timestamp,
        }
    }
}

/// Published when an agent has been started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentStartedEvent {
    /// The unique identifier assigned to this managed process.
    pub process_id: ProcessId,
    /// The human-readable name of the agent.
    pub agent_name: String,
    /// The kind of agent (e.g. "jsonrpc_stdio").
    pub agent_kind: String,
    /// The PID of the agent process, if available.
    pub pid: Option<u32>,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl AgentStartedEvent {
    pub fn new(process_id: ProcessId, agent_name: &str, agent_kind: &str, pid: Option<u32>) -> Self {
        Self {
            process_id,
            agent_name: agent_name.to_string(),
            agent_kind: agent_kind.to_string(),
            pid,
            timestamp: Utc::now(),
        }
    }
}

/// Published when an agent has been stopped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentStoppedEvent {
    /// The unique identifier assigned to this managed process.
    pub process_id: ProcessId,
    /// The human-readable name of the agent.
    pub agent_name: String,
    /// The reason for the stop (e.g. "manager shutdown", "user requested stop").
    pub reason: String,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl AgentStoppedEvent {
    pub fn new(process_id: ProcessId, agent_name: &str, reason: &str) -> Self {
        Self {
            process_id,
            agent_name: agent_name.to_string(),
            reason: reason.to_string(),
            timestamp: Utc::now(),
        }
    }
}

/// Published when an agent has been restarted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRestartedEvent {
    /// The unique identifier assigned to this managed process.
    pub process_id: ProcessId,
    /// The human-readable name of the agent.
    pub agent_name: String,
    /// How many times this agent has been restarted.
    pub restart_count: u32,
    /// The cause that triggered the restart.
    pub reason: String,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl AgentRestartedEvent {
    pub fn new(
        process_id: ProcessId,
        agent_name: &str,
        restart_count: u32,
        reason: &str,
    ) -> Self {
        Self {
            process_id,
            agent_name: agent_name.to_string(),
            restart_count,
            reason: reason.to_string(),
            timestamp: Utc::now(),
        }
    }
}

/// Published when an agent has crashed (unexpected process exit).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentCrashedEvent {
    /// The unique identifier assigned to this managed process.
    pub process_id: ProcessId,
    /// The human-readable name of the agent.
    pub agent_name: String,
    /// The exit code of the process, if available.
    pub exit_code: Option<i32>,
    /// The error message describing the crash.
    pub error: String,
    /// The PID of the agent process, if available.
    pub pid: Option<u32>,
    /// How many times this agent has been restarted.
    pub restart_count: u32,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl AgentCrashedEvent {
    pub fn new(
        process_id: ProcessId,
        agent_name: &str,
        exit_code: Option<i32>,
        error: &str,
        pid: Option<u32>,
        restart_count: u32,
    ) -> Self {
        Self {
            process_id,
            agent_name: agent_name.to_string(),
            exit_code,
            error: error.to_string(),
            pid,
            restart_count,
            timestamp: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Streaming events
// ---------------------------------------------------------------------------

/// A unique identifier for a streaming session.
///
/// This is a `Uuid` allocated when a streaming session is created. It is
/// stable for the lifetime of the stream and used to correlate all streaming
/// events for a particular response.
pub type StreamId = Uuid;

/// The lifecycle state of a streaming session.
///
/// This mirrors the stream lifecycle documented in the streaming module:
/// `Active` → `Streaming` → `Completed` / `Cancelled` / `Failed`.
/// It is duplicated here to avoid circular dependencies between the `event_bus`
/// and `streaming` modules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamState {
    /// The session has been created but not yet published any tokens.
    Active,
    /// Tokens are being published to the stream.
    Streaming,
    /// The stream completed normally (all tokens delivered).
    Completed,
    /// The stream was cancelled before completion.
    Cancelled,
    /// The stream failed due to an error.
    Failed,
}

impl Default for StreamState {
    fn default() -> Self {
        Self::Active
    }
}

impl StreamState {
    /// Returns `true` if the stream is in an active (non-terminal) state.
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Active | Self::Streaming)
    }

    /// Returns `true` if the stream is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled | Self::Failed)
    }
}

/// All streaming events published through the EventBus.
///
/// Published by the [`StreamingPipeline`](crate::streaming::StreamingPipeline)
/// when tokens are produced, a stream starts, completes, is cancelled, or fails.
/// The `PipelineEvent::Stream` variant wraps a `StreamEvent` for EventBus
/// transport.
///
/// Each variant carries the [`StreamId`] identifying the stream, the
/// [`ProcessId`] of the originating agent (when applicable), and a timestamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum StreamEvent {
    /// A stream was started — the first event in any streaming session.
    ///
    /// Carries the stream ID, conversation/thread ID (when applicable), the
    /// originating agent's process ID, and optional metadata.
    Started(StreamStartedEvent),
    /// A token was received from the agent process.
    ///
    /// Each token is published individually as it arrives, preserving
    /// ordering and enabling sub-10ms latency from agent process to
    /// frontend subscriber.
    Token(StreamTokenEvent),
    /// A partial content update — the accumulated content so far.
    ///
    /// This is a convenience event for subscribers that want to rebuild
    /// the full partial message without manually concatenating tokens.
    PartialUpdate(StreamPartialUpdateEvent),
    /// A stream was completed normally (all tokens delivered).
    ///
    /// This is the terminal event for a successfully completed stream.
    Completed(StreamCompletedEvent),
    /// A stream was cancelled before completion.
    ///
    /// Published when cancellation is requested via
    /// [`StreamSessionHandle::cancel`](crate::streaming::StreamSessionHandle).
    Cancelled(StreamCancelledEvent),
    /// A stream failed due to an error.
    ///
    /// Carries an error message describing the failure. This is the terminal
    /// event for a failed stream.
    Failed(StreamFailedEvent),
}

impl StreamEvent {
    /// Returns the event kind string used for EventBus subscription.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Started(_) => kinds::STREAM_STARTED,
            Self::Token(_) => kinds::STREAM_TOKEN,
            Self::PartialUpdate(_) => kinds::STREAM_PARTIAL_UPDATE,
            Self::Completed(_) => kinds::STREAM_COMPLETED,
            Self::Cancelled(_) => kinds::STREAM_CANCELLED,
            Self::Failed(_) => kinds::STREAM_FAILED,
        }
    }

    /// Returns the timestamp when this event was produced.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::Started(e) => e.timestamp,
            Self::Token(e) => e.timestamp,
            Self::PartialUpdate(e) => e.timestamp,
            Self::Completed(e) => e.timestamp,
            Self::Cancelled(e) => e.timestamp,
            Self::Failed(e) => e.timestamp,
        }
    }

    /// Returns the stream ID for this event.
    pub fn stream_id(&self) -> StreamId {
        match self {
            Self::Started(e) => e.stream_id,
            Self::Token(e) => e.stream_id,
            Self::PartialUpdate(e) => e.stream_id,
            Self::Completed(e) => e.stream_id,
            Self::Cancelled(e) => e.stream_id,
            Self::Failed(e) => e.stream_id,
        }
    }
}

/// Published when a stream was started.
///
/// This is the first event in any streaming session, establishing the
/// stream identity and context before any tokens are delivered.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamStartedEvent {
    /// The unique identifier for this streaming session.
    pub stream_id: StreamId,
    /// The conversation/thread ID this stream is associated with, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<Uuid>,
    /// The agent process that originated this stream, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<crate::event_bus::ProcessId>,
    /// The agent name, if the stream is associated with a named agent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_name: Option<String>,
    /// Open-ended metadata for future extension (model name, endpoint, etc.).
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl StreamStartedEvent {
    /// Create a new `StreamStartedEvent` with the current timestamp.
    pub fn new(
        stream_id: StreamId,
        thread_id: Option<Uuid>,
        agent_id: Option<crate::event_bus::ProcessId>,
        agent_name: Option<String>,
    ) -> Self {
        Self {
            stream_id,
            thread_id,
            agent_id,
            agent_name,
            metadata: std::collections::HashMap::new(),
            timestamp: Utc::now(),
        }
    }

    /// Builder: add a metadata key-value pair.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Published when a single token is received from the agent process.
///
/// Each token is published individually as it arrives through the EventBus,
/// preserving strict ordering and enabling sub-10ms latency from agent
/// process to frontend subscriber.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamTokenEvent {
    /// The streaming session this token belongs to.
    pub stream_id: StreamId,
    /// The incremental token text chunk.
    pub token: String,
    /// The cumulative text accumulated so far in this stream.
    ///
    /// Pre-computed at publication time so subscribers that want the full
    /// partial content can read it directly without concatenating tokens.
    pub partial_content: String,
    /// The sequence number of this token within the stream (0-based).
    ///
    /// Enables subscribers to detect missing or duplicate tokens.
    pub sequence: u64,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl StreamTokenEvent {
    /// Create a new `StreamTokenEvent` with the current timestamp.
    pub fn new(
        stream_id: StreamId,
        token: impl Into<String>,
        partial_content: impl Into<String>,
        sequence: u64,
    ) -> Self {
        Self {
            stream_id,
            token: token.into(),
            partial_content: partial_content.into(),
            sequence,
            timestamp: Utc::now(),
        }
    }
}

/// Published when a partial content update is available.
///
/// This event is published alongside tokens to provide subscribers with
/// the full accumulated content at a given point in time. It is useful
/// for subscribers that do not want to manually reconstruct the message
/// from individual token events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamPartialUpdateEvent {
    /// The streaming session this update belongs to.
    pub stream_id: StreamId,
    /// The full accumulated text so far in this stream.
    pub content: String,
    /// The number of tokens published so far.
    pub token_count: u64,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl StreamPartialUpdateEvent {
    /// Create a new `StreamPartialUpdateEvent` with the current timestamp.
    pub fn new(stream_id: StreamId, content: impl Into<String>, token_count: u64) -> Self {
        Self {
            stream_id,
            content: content.into(),
            token_count,
            timestamp: Utc::now(),
        }
    }
}

/// Published when a stream was completed normally.
///
/// This is the terminal event for a successfully completed stream — all
/// tokens have been delivered and the stream is now in the `Completed` state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamCompletedEvent {
    /// The streaming session that completed.
    pub stream_id: StreamId,
    /// The complete accumulated content of the stream.
    pub full_content: String,
    /// The total number of tokens delivered in this stream.
    pub total_tokens: u64,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl StreamCompletedEvent {
    /// Create a new `StreamCompletedEvent` with the current timestamp.
    pub fn new(
        stream_id: StreamId,
        full_content: impl Into<String>,
        total_tokens: u64,
    ) -> Self {
        Self {
            stream_id,
            full_content: full_content.into(),
            total_tokens,
            timestamp: Utc::now(),
        }
    }
}

/// Published when a stream was cancelled before completion.
///
/// Cancellation is triggered by calling
/// [`StreamSessionHandle::cancel`](crate::streaming::StreamSessionHandle).
/// After this event, no further token events will be published for the stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamCancelledEvent {
    /// The streaming session that was cancelled.
    pub stream_id: StreamId,
    /// The number of tokens delivered before cancellation.
    pub tokens_delivered: u64,
    /// The partial content delivered before cancellation.
    pub partial_content: String,
    /// The reason for the cancellation (e.g. "user requested", "timeout").
    pub reason: String,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl StreamCancelledEvent {
    /// Create a new `StreamCancelledEvent` with the current timestamp.
    pub fn new(
        stream_id: StreamId,
        tokens_delivered: u64,
        partial_content: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            stream_id,
            tokens_delivered,
            partial_content: partial_content.into(),
            reason: reason.into(),
            timestamp: Utc::now(),
        }
    }
}

/// Published when a stream failed due to an error.
///
/// This is the terminal event for a failed stream. After this event,
/// no further token events will be published for the stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StreamFailedEvent {
    /// The streaming session that failed.
    pub stream_id: StreamId,
    /// The number of tokens delivered before the failure.
    pub tokens_delivered: u64,
    /// The partial content delivered before the failure.
    pub partial_content: String,
    /// A human-readable error message describing the failure.
    pub error: String,
    /// When the event was produced.
    pub timestamp: DateTime<Utc>,
}

impl StreamFailedEvent {
    /// Create a new `StreamFailedEvent` with the current timestamp.
    pub fn new(
        stream_id: StreamId,
        tokens_delivered: u64,
        partial_content: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            stream_id,
            tokens_delivered,
            partial_content: partial_content.into(),
            error: error.into(),
            timestamp: Utc::now(),
        }
    }
}

/// All streaming session lifecycle events published through the EventBus.
///
/// These events track the lifecycle of a [`StreamingSession`](crate::streaming::StreamingSession)
/// at the session level — distinct from the per-token [`StreamEvent`] variants.
/// Subscribers can listen for session-level events to manage resource cleanup,
/// UI state, and logging without parsing individual token events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum StreamSessionEvent {
    /// A streaming session was created (but not yet started).
    SessionCreated {
        /// The unique identifier for the streaming session.
        stream_id: StreamId,
        /// The conversation/thread ID this session is associated with, if any.
        thread_id: Option<Uuid>,
        /// When the event was produced.
        timestamp: DateTime<Utc>,
    },
    /// A streaming session was started (tokens are being published).
    SessionStarted {
        /// The unique identifier for the streaming session.
        stream_id: StreamId,
        /// When the event was produced.
        timestamp: DateTime<Utc>,
    },
    /// A streaming session was cancelled.
    SessionCancelled {
        /// The unique identifier for the streaming session.
        stream_id: StreamId,
        /// The reason for the cancellation.
        reason: String,
        /// When the event was produced.
        timestamp: DateTime<Utc>,
    },
    /// A streaming session was cleaned up and removed from the manager.
    SessionCleanedUp {
        /// The unique identifier for the streaming session.
        stream_id: StreamId,
        /// When the event was produced.
        timestamp: DateTime<Utc>,
    },
}

impl StreamSessionEvent {
    /// Returns the event kind string used for EventBus subscription.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::SessionCreated { .. } => kinds::SESSION_CREATED,
            Self::SessionStarted { .. } => kinds::SESSION_STARTED,
            Self::SessionCancelled { .. } => kinds::SESSION_CANCELLED,
            Self::SessionCleanedUp { .. } => kinds::SESSION_CLEANED_UP,
        }
    }

    /// Returns the timestamp when this event was produced.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::SessionCreated { timestamp, .. }
            | Self::SessionStarted { timestamp, .. }
            | Self::SessionCancelled { timestamp, .. }
            | Self::SessionCleanedUp { timestamp, .. } => *timestamp,
        }
    }

    /// Returns the stream ID for this session event.
    pub fn stream_id(&self) -> StreamId {
        match self {
            Self::SessionCreated { stream_id, .. }
            | Self::SessionStarted { stream_id, .. }
            | Self::SessionCancelled { stream_id, .. }
            | Self::SessionCleanedUp { stream_id, .. } => *stream_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::events::AgentEvent;
    use uuid::Uuid;

    fn test_process_id() -> ProcessId {
        Uuid::nil()
    }

    #[test]
    fn agent_started_kind_is_started() {
        let ev = AgentEvent::Started(AgentStartedEvent::new(
            test_process_id(),
            "mcp-filesystem",
            "jsonrpc_stdio",
            Some(1234),
        ));
        assert_eq!(ev.kind(), "agent.started");
    }

    #[test]
    fn agent_stopped_kind_is_stopped() {
        let ev = AgentEvent::Stopped(AgentStoppedEvent::new(
            test_process_id(),
            "mcp-filesystem",
            "user requested stop",
        ));
        assert_eq!(ev.kind(), "agent.stopped");
    }

    #[test]
    fn agent_restarted_kind_is_restarted() {
        let ev = AgentEvent::Restarted(AgentRestartedEvent::new(
            test_process_id(),
            "mcp-filesystem",
            1,
            "crash",
        ));
        assert_eq!(ev.kind(), "agent.restarted");
    }

    #[test]
    fn agent_crashed_kind_is_crashed() {
        let ev = AgentEvent::Crashed(AgentCrashedEvent::new(
            test_process_id(),
            "mcp-filesystem",
            Some(1),
            "panic",
            Some(1234),
            0,
        ));
        assert_eq!(ev.kind(), "agent.crashed");
    }

    #[test]
    fn agent_event_timestamp_is_valid() {
        let created = Utc::now();
        let ev = AgentEvent::Started(AgentStartedEvent::new(
            test_process_id(),
            "mcp-filesystem",
            "jsonrpc_stdio",
            None,
        ));
        let ts = ev.timestamp();
        assert!(ts >= created, "timestamp should be >= time of creation");
    }
}
