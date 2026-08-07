//! # Shared Plugin Event Contracts
//!
//! This module defines the stable, versionable event contract layer that
//! plugins use to communicate with the Nabu platform core and other services
//! through the existing [`EventBus`].
//!
//! ## Architecture
//!
//! ```text
//! Plugin
//!   │  (creates a strongly-typed event)
//!   ▼
//! PluginEvent            ── implements ──▶ PluginEventContract
//!   │  (to_pipeline_event)
//!   ▼
//! PipelineEvent::Plugin(PluginEvent)
//!   │  (EventBus::publish by kind)
//!   ▼
//! EventBus<PipelineEvent>
//!   │  (delivers to subscribers by kind string)
//!   ▼
//! Platform Services & Frontend Event Bridge
//! ```
//!
//! Plugins should **never** emit raw `PipelineEvent` messages directly.
//! Instead, they create a [`PluginEvent`] (which implements
//! [`PluginEventContract`]) and publish it through the
//! [`publish_plugin_event`] helper, which wraps it in a
//! `PipelineEvent::Plugin(...)` variant and forwards it to the EventBus.
//!
//! ## Ownership
//!
//! | Layer              | Responsibility                                      |
//! |--------------------|-----------------------------------------------------|
//! | Plugin             | Creates shared events, publishes via the helper     |
//! | Shared contracts   | Define event schema (`PluginEvent`, the trait)      |
//! | EventBus           | Transports events (publishes/subscribes by kind)    |
//! | Platform services  | Consumes events (subscribes to kind strings)        |
//! | Frontend bridge    | Observes EventBus, forwards to Tauri `nabu-event`   |
//!
//! ## Versioning Strategy
//!
//! Each event carries an [`PluginApiVersion`] representing the version of
//! the plugin event contract schema it was serialized against.
//!
//! - **Major version** changes represent breaking schema changes (removed
//!   fields, changed types, removed enum variants). A plugin built against
//!   API major version 1 will not be understood by a platform that only
//!   supports major version 2.
//! - **Minor version** changes are backward-compatible and additive (new
//!   optional fields, new enum variants). A platform supporting minor version
//!   2 can safely consume events serialized against minor version 1.
//! - All event structs use `#[serde(default)]` so that fields introduced in
//!   future versions are silently defaulted during deserialization of older
//!   payloads — ensuring forward compatibility.
//!
//! ## Extension Guidelines
//!
//! To add a new shared plugin event:
//!
//! 1. Add a new variant to [`PluginEvent`] (and a corresponding event struct).
//! 2. Add a kind constant to [`crate::event_bus::kinds`].
//! 3. Add a match arm in `PluginEvent::kind()` and `PipelineEvent::kind()`.
//! 4. Add the kind to `event_bridge::ALL_EVENT_KINDS` if frontend forwarding
//!    is desired.
//! 5. Add the variant to `PluginEvent::from_kind_json()` if JSON round-trip
//!    deserialization is needed.
//!
//! New fields on existing structs should be `Option<T>` with
//! `#[serde(default)]` and `#[serde(skip_serializing_if = "Option::is_none")]`
//! so that older serialized payloads remain valid.
//!
//! ## Thread Safety
//!
//! All shared event types are `Send + Sync + Clone`. The [`EventBus`] is
//! `Clone` and wraps its state in `Arc<Mutex<>>`, making it safe to share
//! across threads. Plugins may create and publish events from any thread.
//!
//! ## Serialization
//!
//! All shared event types derive [`Serialize`] and [`Deserialize`] via
//! Serde. No alternative serialization mechanism is introduced. Events are
//! serialized to JSON when forwarded to the frontend.

use crate::event_bus::kinds;
use crate::event_bus::{EventBus, PipelineEvent};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Version and metadata types
// ---------------------------------------------------------------------------

/// The version of the plugin event contract API.
///
/// Every shared plugin event carries this version so the platform can
/// determine whether it understands the event schema.
///
/// - **Major**: breaking changes (incompatible schema evolution).
/// - **Minor**: additive, backward-compatible changes.
///
/// `PluginApiVersion::CURRENT` is the latest version understood by this
/// platform. Future versions of the contract will increment these numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PluginApiVersion {
    /// Major API version — incompatible changes.
    pub major: u32,
    /// Minor API version — additive, compatible changes.
    pub minor: u32,
}

impl Default for PluginApiVersion {
    /// Defaults to the current API version so that events constructed
    /// without an explicit version are tagged with the running platform's
    /// contract version.
    fn default() -> Self {
        Self::CURRENT
    }
}

impl PluginApiVersion {
    /// The current version of the plugin event contract API.
    pub const CURRENT: Self = Self { major: 1, minor: 0 };

    /// The minimum API version this platform supports.
    ///
    /// Events with a higher major version are rejected during validation.
    pub const MIN_SUPPORTED: Self = Self { major: 1, minor: 0 };

    /// Returns `true` if this version is compatible with the running
    /// platform.
    ///
    /// Compatibility is determined by the **major** version only. Minor
    /// version differences are always compatible because all event structs
    /// use `#[serde(default)]` — fields introduced in newer minor versions
    /// are silently defaulted during deserialization of older payloads, and
    /// fields that newer platforms don't understand are preserved in
    /// `serde_json::Value` payloads (for request/response events).
    ///
    /// A matching major version guarantees no breaking schema changes
    /// (no removed fields, no changed field types, no removed enum variants).
    pub fn is_compatible(&self) -> bool {
        self.major == Self::CURRENT.major
    }
}

impl std::fmt::Display for PluginApiVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Severity levels for [`PluginWarningEvent`] and [`PluginErrorEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum PluginEventSeverity {
    /// Informational — non-critical, no action required.
    #[serde(rename = "info")]
    #[default]
    Info = 0,
    /// A warning — unexpected but recoverable condition.
    #[serde(rename = "warning")]
    Warning = 1,
    /// An error — operation failed, but the plugin may continue.
    #[serde(rename = "error")]
    Error = 2,
    /// Critical — the plugin is in an unrecoverable state.
    #[serde(rename = "critical")]
    Critical = 3,
}

impl std::fmt::Display for PluginEventSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Critical => "critical",
        };
        write!(f, "{}", label)
    }
}

/// Status of a [`PluginResponseEvent`] — mirrors the outcome of the
/// corresponding request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PluginResponseStatus {
    /// The request completed successfully.
    #[serde(rename = "success")]
    #[default]
    Success,
    /// The request failed — see the `error` field for details.
    #[serde(rename = "error")]
    Error,
    /// The request was cancelled before completion.
    #[serde(rename = "cancelled")]
    Cancelled,
}

/// The category of a plugin event, used for broad filtering and routing.
///
/// Every [`PluginEvent`] can be classified into one of these categories.
/// This enum allows consumers (e.g. the EventBus frontend bridge, log
/// aggregators, analytics sinks) to subscribe to or filter by broad event
/// groupings without matching on individual event variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PluginEventCategory {
    /// Lifecycle events: registration, loading, starting, stopping, unloading.
    #[serde(rename = "lifecycle")]
    #[default]
    Lifecycle,
    /// Capability events: registration and removal of capabilities.
    #[serde(rename = "capability")]
    Capability,
    /// Diagnostic events: warnings and errors from plugins.
    #[serde(rename = "diagnostic")]
    Diagnostic,
    /// Request/response events: capability invocations between plugins
    /// and the platform.
    #[serde(rename = "request_response")]
    RequestResponse,
}

impl PluginEventCategory {
    /// Short, kebab-case machine identifier matching the serde token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lifecycle => "lifecycle",
            Self::Capability => "capability",
            Self::Diagnostic => "diagnostic",
            Self::RequestResponse => "request_response",
        }
    }
}

impl std::fmt::Display for PluginEventCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// The source of a plugin event — who originated it.
///
/// Plugins and the platform both produce events. This enum distinguishes
/// the origin so consumers can filter or attribute events appropriately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum EventSource {
    /// The event was produced by the Nabu platform core itself (e.g. the
    /// PluginManager, CapabilityRegistry, or runtime supervision layer).
    #[serde(rename = "platform")]
    #[default]
    Platform,
    /// The event was produced by a plugin.
    #[serde(rename = "plugin")]
    Plugin,
    /// The event was produced by the user (e.g. an explicit enable/disable
    /// action reflected through the plugin event system).
    #[serde(rename = "user")]
    User,
    /// The event was produced by an external system (e.g. a remote plugin
    /// host or marketplace service).
    #[serde(rename = "external")]
    External,
}

impl EventSource {
    /// Short, kebab-case machine identifier matching the serde token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::Plugin => "plugin",
            Self::User => "user",
            Self::External => "external",
        }
    }
}

impl std::fmt::Display for EventSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// The lifecycle state of a plugin as managed by the [`PluginManager`].
///
/// This mirrors the [`PluginStage`](crate::plugin::lifecycle::PluginStage)
/// enum but is a strongly-typed, serializable value suitable for inclusion in
/// event payloads. It allows event consumers (especially the frontend bridge)
/// to reason about a plugin's current state without importing lifecycle types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default)]
pub enum PluginLifecycleState {
    /// Plugin manifest has been discovered on disk.
    #[serde(rename = "discovered")]
    Discovered,
    /// Plugin manifest has been registered (accepted by the PluginManager).
    #[serde(rename = "registered")]
    Registered,
    /// Plugin manifest has been validated.
    #[serde(rename = "validated")]
    Validated,
    /// Plugin has been installed (dependencies resolved, capabilities registered).
    #[serde(rename = "installed")]
    Installed,
    /// Plugin is enabled and ready to provide capabilities.
    #[serde(rename = "enabled")]
    Enabled,
    /// Plugin has been disabled (capabilities are no longer available).
    #[serde(rename = "disabled")]
    Disabled,
    /// Plugin has been upgraded to a new version.
    #[serde(rename = "upgraded")]
    Upgraded,
    /// Plugin runtime has been started (execution activated).
    #[serde(rename = "started")]
    Started,
    /// Plugin runtime is actively running (started and executing).
    #[serde(rename = "running")]
    Running,
    /// Plugin runtime has been stopped (execution deactivated).
    #[serde(rename = "stopped")]
    Stopped,
    /// Plugin has been unregistered (manifest removed from the PluginManager).
    #[serde(rename = "unregistered")]
    Unregistered,
    /// Plugin has been unloaded (fully removed from memory).
    #[serde(rename = "unloaded")]
    #[default]
    Unloaded,
}

impl PluginLifecycleState {
    /// Short, kebab-case machine identifier matching the serde token.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Discovered => "discovered",
            Self::Registered => "registered",
            Self::Validated => "validated",
            Self::Installed => "installed",
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::Upgraded => "upgraded",
            Self::Started => "started",
            Self::Running => "running",
            Self::Stopped => "stopped",
            Self::Unregistered => "unregistered",
            Self::Unloaded => "unloaded",
        }
    }

    /// Convert from a [`PluginStage`](crate::plugin::lifecycle::PluginStage).
    pub fn from_stage(stage: crate::plugin::lifecycle::PluginStage) -> Self {
        match stage {
            crate::plugin::lifecycle::PluginStage::Discovered => Self::Discovered,
            crate::plugin::lifecycle::PluginStage::Validated => Self::Validated,
            crate::plugin::lifecycle::PluginStage::Installed => Self::Installed,
            crate::plugin::lifecycle::PluginStage::Enabled => Self::Enabled,
            crate::plugin::lifecycle::PluginStage::Disabled => Self::Disabled,
            crate::plugin::lifecycle::PluginStage::Upgraded => Self::Upgraded,
            crate::plugin::lifecycle::PluginStage::Unloaded => Self::Unloaded,
        }
    }
}

impl std::fmt::Display for PluginLifecycleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Event data structs
// ---------------------------------------------------------------------------

/// Published when a plugin has been successfully loaded.
///
/// A plugin is considered "loaded" once its manifest has been validated,
/// dependencies resolved, and capabilities registered — regardless of
/// whether it is enabled.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginLoadedEvent {
    /// Unique plugin identifier (reverse-domain notation).
    pub plugin_id: String,
    /// Human-readable plugin name.
    pub plugin_name: String,
    /// Version of the plugin's own manifest (semver).
    pub plugin_version: String,
    /// Version of the plugin event contract API this event conforms to.
    pub api_version: PluginApiVersion,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
}

impl Default for PluginLoadedEvent {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            plugin_name: String::new(),
            plugin_version: String::new(),
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

impl PluginLoadedEvent {
    /// Create a new `PluginLoadedEvent`.
    pub fn new(plugin_id: &str, plugin_name: &str, plugin_version: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            plugin_name: plugin_name.to_string(),
            plugin_version: plugin_version.to_string(),
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

/// Published when a plugin has been unloaded (removed from memory).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginUnloadedEvent {
    /// Unique plugin identifier.
    pub plugin_id: String,
    /// Version of the plugin event contract API this event conforms to.
    pub api_version: PluginApiVersion,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
}

impl Default for PluginUnloadedEvent {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

impl PluginUnloadedEvent {
    pub fn new(plugin_id: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

/// Published when a plugin's capability has been registered in the
/// [`CapabilityRegistry`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CapabilityRegisteredEvent {
    /// Full capability identifier (`namespace:name`).
    pub capability_id: String,
    /// The plugin ID that provides this capability.
    pub provider: String,
    /// Human-readable capability description.
    pub description: String,
    /// Version of the plugin event contract API this event conforms to.
    pub api_version: PluginApiVersion,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
}

impl Default for CapabilityRegisteredEvent {
    fn default() -> Self {
        Self {
            capability_id: String::new(),
            provider: String::new(),
            description: String::new(),
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

impl CapabilityRegisteredEvent {
    pub fn new(capability_id: &str, provider: &str, description: &str) -> Self {
        Self {
            capability_id: capability_id.to_string(),
            provider: provider.to_string(),
            description: description.to_string(),
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

/// Published when a capability has been removed from the
/// [`CapabilityRegistry`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CapabilityRemovedEvent {
    /// Full capability identifier (`namespace:name`).
    pub capability_id: String,
    /// Version of the plugin event contract API this event conforms to.
    pub api_version: PluginApiVersion,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
}

impl Default for CapabilityRemovedEvent {
    fn default() -> Self {
        Self {
            capability_id: String::new(),
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

impl CapabilityRemovedEvent {
    pub fn new(capability_id: &str) -> Self {
        Self {
            capability_id: capability_id.to_string(),
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

/// A non-fatal warning published by a plugin to signal an unexpected
/// but recoverable condition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginWarningEvent {
    /// Unique plugin identifier.
    pub plugin_id: String,
    /// Human-readable warning message.
    pub message: String,
    /// Optional machine-readable warning code.
    pub code: Option<String>,
    /// Severity level.
    pub severity: PluginEventSeverity,
    /// Version of the plugin event contract API this event conforms to.
    pub api_version: PluginApiVersion,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
}

impl Default for PluginWarningEvent {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            message: String::new(),
            code: None,
            severity: PluginEventSeverity::Warning,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

impl PluginWarningEvent {
    pub fn new(plugin_id: &str, message: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            message: message.to_string(),
            code: None,
            severity: PluginEventSeverity::Warning,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }

    pub fn with_code(plugin_id: &str, message: &str, code: &str) -> Self {
        Self {
            code: Some(code.to_string()),
            ..Self::new(plugin_id, message)
        }
    }
}

/// A fatal or recoverable error published by a plugin.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginErrorEvent {
    /// Unique plugin identifier.
    pub plugin_id: String,
    /// Human-readable error message.
    pub error: String,
    /// Optional machine-readable error code.
    pub code: Option<String>,
    /// Severity level.
    pub severity: PluginEventSeverity,
    /// Optional stack trace or diagnostic detail.
    pub detail: Option<String>,
    /// Version of the plugin event contract API this event conforms to.
    pub api_version: PluginApiVersion,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
}

impl Default for PluginErrorEvent {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            error: String::new(),
            code: None,
            severity: PluginEventSeverity::Error,
            detail: None,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

impl PluginErrorEvent {
    pub fn new(plugin_id: &str, error: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            error: error.to_string(),
            code: None,
            severity: PluginEventSeverity::Error,
            detail: None,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }

    pub fn critical(plugin_id: &str, error: &str) -> Self {
        Self {
            severity: PluginEventSeverity::Critical,
            ..Self::new(plugin_id, error)
        }
    }
}

/// A request from a plugin to the platform for a capability or service.
///
/// Requests and responses are correlated by `request_id`. The `method`
/// identifies the capability operation being invoked, and `params` carries
/// the request parameters as arbitrary JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginRequestEvent {
    /// Unique plugin identifier.
    pub plugin_id: String,
    /// Correlation ID matching this request to its response.
    pub request_id: Uuid,
    /// The capability method being invoked (e.g. `"nabu:storage.read"`).
    pub method: String,
    /// Request parameters as arbitrary JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
    /// Version of the plugin event contract API this event conforms to.
    pub api_version: PluginApiVersion,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
}

impl Default for PluginRequestEvent {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            request_id: Uuid::nil(),
            method: String::new(),
            params: None,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

impl PluginRequestEvent {
    pub fn new(plugin_id: &str, request_id: Uuid, method: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            request_id,
            method: method.to_string(),
            params: None,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }

    pub fn with_params(plugin_id: &str, request_id: Uuid, method: &str, params: serde_json::Value) -> Self {
        Self {
            params: Some(params),
            ..Self::new(plugin_id, request_id, method)
        }
    }
}

/// A response to a [`PluginRequestEvent`].
///
/// The `request_id` matches the originating request. When `status` is
/// `Error`, the `error` field contains a human-readable message and
/// `result` is `None`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginResponseEvent {
    /// Unique plugin identifier.
    pub plugin_id: String,
    /// Correlation ID matching the originating request.
    pub request_id: Uuid,
    /// The capability method being responded to.
    pub method: String,
    /// The result of the request, if successful.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error message, present when `status` is `Error`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Status of the response.
    pub status: PluginResponseStatus,
    /// Version of the plugin event contract API this event conforms to.
    pub api_version: PluginApiVersion,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
}

impl Default for PluginResponseEvent {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            request_id: Uuid::nil(),
            method: String::new(),
            result: None,
            error: None,
            status: PluginResponseStatus::Success,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

impl PluginResponseEvent {
    pub fn new(plugin_id: &str, request_id: Uuid, method: &str, status: PluginResponseStatus) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            request_id,
            method: method.to_string(),
            result: None,
            error: None,
            status,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }

    pub fn success(plugin_id: &str, request_id: Uuid, method: &str, result: serde_json::Value) -> Self {
        Self {
            request_id,
            method: method.to_string(),
            result: Some(result),
            status: PluginResponseStatus::Success,
            ..Self::new(plugin_id, request_id, method, PluginResponseStatus::Success)
        }
    }

    pub fn error(plugin_id: &str, request_id: Uuid, method: &str, error: &str) -> Self {
        Self {
            error: Some(error.to_string()),
            status: PluginResponseStatus::Error,
            ..Self::new(plugin_id, request_id, method, PluginResponseStatus::Error)
        }
    }

    pub fn cancelled(plugin_id: &str, request_id: Uuid, method: &str) -> Self {
        Self {
            status: PluginResponseStatus::Cancelled,
            ..Self::new(plugin_id, request_id, method, PluginResponseStatus::Cancelled)
        }
    }
}

// ---------------------------------------------------------------------------
// Additional event data structs
// ---------------------------------------------------------------------------

/// Published when a plugin's manifest has been accepted and registered with
/// the [`PluginManager`].
///
/// Registration is a metadata-only operation — no plugin code is loaded or
/// executed. This event signals that the plugin is known to the platform, its
/// dependencies have been resolved, and its capabilities have been registered
/// in the [`CapabilityRegistry`].
///
/// This is **distinct** from [`PluginLoadedEvent`]: a plugin may be *registered*
/// (manifest on disk, validated) without being *loaded* (runtime entry point
/// activated).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginRegisteredEvent {
    /// Unique plugin identifier (reverse-domain notation).
    pub plugin_id: String,
    /// Human-readable plugin name.
    pub plugin_name: String,
    /// Version of the plugin's own manifest (semver).
    pub plugin_version: String,
    /// The lifecycle state the plugin is transitioning into.
    pub state: PluginLifecycleState,
    /// Version of the plugin event contract API this event conforms to.
    pub api_version: PluginApiVersion,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
}

impl Default for PluginRegisteredEvent {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            plugin_name: String::new(),
            plugin_version: String::new(),
            state: PluginLifecycleState::Registered,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

impl PluginRegisteredEvent {
    /// Create a new `PluginRegisteredEvent`.
    pub fn new(plugin_id: &str, plugin_name: &str, plugin_version: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            plugin_name: plugin_name.to_string(),
            plugin_version: plugin_version.to_string(),
            state: PluginLifecycleState::Registered,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }

    /// Create a `PluginRegisteredEvent` with only the plugin ID set.
    ///
    /// This is primarily useful for tests that need to verify validation
    /// rejects empty `plugin_name` or `plugin_version`.
    pub fn plugin_id_only(plugin_id: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            plugin_name: String::new(),
            plugin_version: String::new(),
            state: PluginLifecycleState::Registered,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }

    /// Builder: set the lifecycle state.
    pub fn with_state(mut self, state: PluginLifecycleState) -> Self {
        self.state = state;
        self
    }

    /// Validate that required fields are populated.
    pub fn validate(&self) -> Result<(), PluginEventError> {
        if self.plugin_id.is_empty() {
            return Err(PluginEventError::InvalidPluginId(self.plugin_id.clone()));
        }
        if self.plugin_name.is_empty() {
            return Err(PluginEventError::MalformedPayload(
                "plugin_name must not be empty".to_string(),
            ));
        }
        if self.plugin_version.is_empty() {
            return Err(PluginEventError::MalformedPayload(
                "plugin_version must not be empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Published when a plugin has been unregistered (removed from the
/// [`PluginManager`]).
///
/// This fires when a plugin's manifest is removed from the platform — either
/// through explicit unregistration or through plugin removal. All capabilities
/// provided by the plugin will have been removed from the
/// [`CapabilityRegistry`] before this event is published.
///
/// This is **distinct** from [`PluginUnloadedEvent`]: a plugin may be
/// *unregistered* (manifest removed) independently of being *unloaded*
/// (runtime entry point removed from memory).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginUnregisteredEvent {
    /// Unique plugin identifier.
    pub plugin_id: String,
    /// The lifecycle state the plugin is transitioning into.
    pub state: PluginLifecycleState,
    /// Version of the plugin event contract API this event conforms to.
    pub api_version: PluginApiVersion,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
}

impl Default for PluginUnregisteredEvent {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            state: PluginLifecycleState::Unregistered,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

impl PluginUnregisteredEvent {
    /// Create a new `PluginUnregisteredEvent`.
    pub fn new(plugin_id: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            state: PluginLifecycleState::Unregistered,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }

    /// Builder: set the lifecycle state.
    pub fn with_state(mut self, state: PluginLifecycleState) -> Self {
        self.state = state;
        self
    }

    /// Validate that required fields are populated.
    pub fn validate(&self) -> Result<(), PluginEventError> {
        if self.plugin_id.is_empty() {
            return Err(PluginEventError::InvalidPluginId(self.plugin_id.clone()));
        }
        Ok(())
    }
}

/// Published when a plugin's runtime has been started (activated for
/// execution).
///
/// This event is published after a plugin's runtime entry point has been
/// successfully loaded and initialized. The plugin is now able to receive
/// [`PluginRequestEvent`] invocations and publish its own events.
///
/// This is **distinct** from [`PluginLoadedEvent`]: loading is the metadata
/// stage; starting is the execution stage. A plugin may be loaded without
/// being started (e.g. if it is disabled). This is also distinct from
/// [`PluginRegisteredEvent`]: registration precedes loading; starting
/// succeeds loading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginStartedEvent {
    /// Unique plugin identifier (reverse-domain notation).
    pub plugin_id: String,
    /// Human-readable plugin name.
    pub plugin_name: String,
    /// Version of the plugin's own manifest (semver).
    pub plugin_version: String,
    /// The lifecycle state the plugin is transitioning into.
    pub state: PluginLifecycleState,
    /// Version of the plugin event contract API this event conforms to.
    pub api_version: PluginApiVersion,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
}

impl Default for PluginStartedEvent {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            plugin_name: String::new(),
            plugin_version: String::new(),
            state: PluginLifecycleState::Started,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

impl PluginStartedEvent {
    /// Create a new `PluginStartedEvent`.
    pub fn new(plugin_id: &str, plugin_name: &str, plugin_version: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            plugin_name: plugin_name.to_string(),
            plugin_version: plugin_version.to_string(),
            state: PluginLifecycleState::Started,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }

    /// Create a `PluginStartedEvent` with only the plugin ID and version set.
    ///
    /// This is primarily useful for tests that need to verify validation
    /// rejects empty `plugin_name`.
    pub fn plugin_id_version_only(plugin_id: &str, plugin_version: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            plugin_name: String::new(),
            plugin_version: plugin_version.to_string(),
            state: PluginLifecycleState::Started,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }

    /// Builder: set the lifecycle state.
    pub fn with_state(mut self, state: PluginLifecycleState) -> Self {
        self.state = state;
        self
    }

    /// Validate that required fields are populated.
    pub fn validate(&self) -> Result<(), PluginEventError> {
        if self.plugin_id.is_empty() {
            return Err(PluginEventError::InvalidPluginId(self.plugin_id.clone()));
        }
        if self.plugin_name.is_empty() {
            return Err(PluginEventError::MalformedPayload(
                "plugin_name must not be empty".to_string(),
            ));
        }
        if self.plugin_version.is_empty() {
            return Err(PluginEventError::MalformedPayload(
                "plugin_version must not be empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Published when a plugin's runtime has been stopped (deactivated from
/// execution).
///
/// This fires when a plugin's runtime entry point has been cleanly shut down.
/// After this event, the plugin can no longer receive [`PluginRequestEvent`]
/// invocations, though its manifest may still be registered (allowing it to
/// be started again without re-registration).
///
/// This is **distinct** from [`PluginUnloadedEvent`]: unloading is the
/// memory-management stage; stopping is the execution stage. A plugin may be
/// stopped without being unloaded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginStoppedEvent {
    /// Unique plugin identifier.
    pub plugin_id: String,
    /// The lifecycle state the plugin is transitioning into.
    pub state: PluginLifecycleState,
    /// Version of the plugin event contract API this event conforms to.
    pub api_version: PluginApiVersion,
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
}

impl Default for PluginStoppedEvent {
    fn default() -> Self {
        Self {
            plugin_id: String::new(),
            state: PluginLifecycleState::Stopped,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }
}

impl PluginStoppedEvent {
    /// Create a new `PluginStoppedEvent`.
    pub fn new(plugin_id: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            state: PluginLifecycleState::Stopped,
            api_version: PluginApiVersion::CURRENT,
            timestamp: Utc::now(),
        }
    }

    /// Builder: set the lifecycle state.
    pub fn with_state(mut self, state: PluginLifecycleState) -> Self {
        self.state = state;
        self
    }

    /// Validate that required fields are populated.
    pub fn validate(&self) -> Result<(), PluginEventError> {
        if self.plugin_id.is_empty() {
            return Err(PluginEventError::InvalidPluginId(self.plugin_id.clone()));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PluginEvent enum — the shared event model
// ---------------------------------------------------------------------------

/// All shared plugin event types.
///
/// This is the strongly-typed event model that every future plugin uses to
/// communicate with the Nabu platform. Plugins create a variant, then publish
/// it through [`publish_plugin_event`] which wraps it in a
/// `PipelineEvent::Plugin(...)` for the EventBus.
///
/// Each variant wraps a dedicated event struct that derives
/// [`Serialize`] and [`Deserialize`] and uses `#[serde(default)]` for
/// forward-compatible deserialization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum PluginEvent {
    /// A plugin was registered (manifest accepted).
    PluginRegistered(PluginRegisteredEvent),
    /// A plugin was unregistered (manifest removed).
    PluginUnregistered(PluginUnregisteredEvent),
    /// A plugin was loaded (runtime entry point activated).
    PluginLoaded(PluginLoadedEvent),
    /// A plugin was unloaded (runtime entry point removed from memory).
    PluginUnloaded(PluginUnloadedEvent),
    /// A plugin was started (execution activated).
    PluginStarted(PluginStartedEvent),
    /// A plugin was stopped (execution deactivated).
    PluginStopped(PluginStoppedEvent),
    /// A capability was registered.
    CapabilityRegistered(CapabilityRegisteredEvent),
    /// A capability was removed.
    CapabilityRemoved(CapabilityRemovedEvent),
    /// A plugin emitted a warning.
    PluginWarning(PluginWarningEvent),
    /// A plugin emitted an error.
    PluginError(PluginErrorEvent),
    /// A plugin made a request to a platform capability.
    PluginRequest(PluginRequestEvent),
    /// A platform capability responded to a plugin request.
    PluginResponse(PluginResponseEvent),
}

// ---------------------------------------------------------------------------
// Inherent methods on PluginEvent
// ---------------------------------------------------------------------------

impl PluginEvent {
    /// Returns the event kind string used for EventBus subscription.
    ///
    /// This matches the corresponding constant in
    /// [`crate::event_bus::kinds`].
    pub fn kind(&self) -> &'static str {
        match self {
            Self::PluginRegistered(_) => kinds::PLUGIN_REGISTERED,
            Self::PluginUnregistered(_) => kinds::PLUGIN_UNREGISTERED,
            Self::PluginLoaded(_) => kinds::PLUGIN_LOADED,
            Self::PluginUnloaded(_) => kinds::PLUGIN_UNLOADED,
            Self::PluginStarted(_) => kinds::PLUGIN_STARTED,
            Self::PluginStopped(_) => kinds::PLUGIN_STOPPED,
            Self::CapabilityRegistered(_) => kinds::CAPABILITY_REGISTERED,
            Self::CapabilityRemoved(_) => kinds::CAPABILITY_REMOVED,
            Self::PluginWarning(_) => kinds::PLUGIN_WARNING,
            Self::PluginError(_) => kinds::PLUGIN_ERROR,
            Self::PluginRequest(_) => kinds::PLUGIN_REQUEST,
            Self::PluginResponse(_) => kinds::PLUGIN_RESPONSE,
        }
    }

    /// Returns the plugin ID that originated this event, if applicable.
    ///
    /// Request/response events are platform-originated and return `None`
    /// for the originating plugin (the provider's ID is available via the
    /// `plugin_id` field on the inner struct, which represents the plugin
    /// that receives the response).
    pub fn plugin_id(&self) -> &str {
        match self {
            Self::PluginRegistered(e) => &e.plugin_id,
            Self::PluginUnregistered(e) => &e.plugin_id,
            Self::PluginLoaded(e) => &e.plugin_id,
            Self::PluginUnloaded(e) => &e.plugin_id,
            Self::PluginStarted(e) => &e.plugin_id,
            Self::PluginStopped(e) => &e.plugin_id,
            Self::CapabilityRegistered(e) => &e.provider,
            Self::CapabilityRemoved(_) => "",
            Self::PluginWarning(e) => &e.plugin_id,
            Self::PluginError(e) => &e.plugin_id,
            Self::PluginRequest(e) => &e.plugin_id,
            Self::PluginResponse(e) => &e.plugin_id,
        }
    }

    /// Returns the event contract API version this event conforms to.
    pub fn api_version(&self) -> PluginApiVersion {
        match self {
            Self::PluginRegistered(e) => e.api_version,
            Self::PluginUnregistered(e) => e.api_version,
            Self::PluginLoaded(e) => e.api_version,
            Self::PluginUnloaded(e) => e.api_version,
            Self::PluginStarted(e) => e.api_version,
            Self::PluginStopped(e) => e.api_version,
            Self::CapabilityRegistered(e) => e.api_version,
            Self::CapabilityRemoved(e) => e.api_version,
            Self::PluginWarning(e) => e.api_version,
            Self::PluginError(e) => e.api_version,
            Self::PluginRequest(e) => e.api_version,
            Self::PluginResponse(e) => e.api_version,
        }
    }

    /// Returns when the event was created.
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            Self::PluginRegistered(e) => e.timestamp,
            Self::PluginUnregistered(e) => e.timestamp,
            Self::PluginLoaded(e) => e.timestamp,
            Self::PluginUnloaded(e) => e.timestamp,
            Self::PluginStarted(e) => e.timestamp,
            Self::PluginStopped(e) => e.timestamp,
            Self::CapabilityRegistered(e) => e.timestamp,
            Self::CapabilityRemoved(e) => e.timestamp,
            Self::PluginWarning(e) => e.timestamp,
            Self::PluginError(e) => e.timestamp,
            Self::PluginRequest(e) => e.timestamp,
            Self::PluginResponse(e) => e.timestamp,
        }
    }

    /// Returns the event category for broad filtering and routing.
    ///
    /// This classifies each event into one of the [`PluginEventCategory`]
    /// variants. Consumers (e.g. the EventBus frontend bridge, log aggregators)
    /// can use this to route or filter events by broad groupings without
    /// matching on individual variants.
    pub fn category(&self) -> PluginEventCategory {
        match self {
            Self::PluginRegistered(_)
            | Self::PluginUnregistered(_)
            | Self::PluginLoaded(_)
            | Self::PluginUnloaded(_)
            | Self::PluginStarted(_)
            | Self::PluginStopped(_) => PluginEventCategory::Lifecycle,
            Self::CapabilityRegistered(_)
            | Self::CapabilityRemoved(_) => PluginEventCategory::Capability,
            Self::PluginWarning(_)
            | Self::PluginError(_) => PluginEventCategory::Diagnostic,
            Self::PluginRequest(_)
            | Self::PluginResponse(_) => PluginEventCategory::RequestResponse,
        }
    }

    /// Convert this shared event into a [`PipelineEvent`] for EventBus
    /// integration.
    ///
    /// This is the bridge that allows `PluginEvent` to flow through the
    /// existing `EventBus<PipelineEvent>` without plugins ever touching
    /// `PipelineEvent` directly.
    pub fn to_pipeline_event(&self) -> PipelineEvent {
        PipelineEvent::Plugin(self.clone())
    }

    /// Deserialize a [`PluginEvent`] from a kind string and a JSON payload.
    ///
    /// This is useful for the IPC layer and for plugins that receive events
    /// over a wire protocol. The `kind` must match one of the plugin event
    /// kinds in [`crate::event_bus::kinds`].
    ///
    /// # Errors
    ///
    /// Returns [`PluginEventError::UnknownEventType`] if the kind does not
    /// match a known plugin event type, and
    /// [`PluginEventError::MalformedPayload`] if the JSON cannot be
    /// deserialized into the corresponding struct.
    pub fn from_kind_json(kind: &str, json: &str) -> Result<Self, PluginEventError> {
        // First, try deserializing as a full PluginEvent (which includes
        // the enum variant wrapper, e.g. {"PluginLoaded":{...}}).
        if let Ok(event) = serde_json::from_str::<PluginEvent>(json) {
            if event.kind() == kind {
                return Ok(event);
            }
            // Kind mismatch — fall through to try as raw struct.
        }

        // Otherwise, deserialize based on the kind string into the specific
        // event struct (the JSON payload without the enum wrapper).
        match kind {
            k if k == kinds::PLUGIN_REGISTERED => {
                serde_json::from_str::<PluginRegisteredEvent>(json)
                    .map(Self::PluginRegistered)
                    .map_err(|e| PluginEventError::MalformedPayload(e.to_string()))
            }
            k if k == kinds::PLUGIN_UNREGISTERED => {
                serde_json::from_str::<PluginUnregisteredEvent>(json)
                    .map(Self::PluginUnregistered)
                    .map_err(|e| PluginEventError::MalformedPayload(e.to_string()))
            }
            k if k == kinds::PLUGIN_LOADED => {
                serde_json::from_str::<PluginLoadedEvent>(json)
                    .map(Self::PluginLoaded)
                    .map_err(|e| PluginEventError::MalformedPayload(e.to_string()))
            }
            k if k == kinds::PLUGIN_UNLOADED => {
                serde_json::from_str::<PluginUnloadedEvent>(json)
                    .map(Self::PluginUnloaded)
                    .map_err(|e| PluginEventError::MalformedPayload(e.to_string()))
            }
            k if k == kinds::PLUGIN_STARTED => {
                serde_json::from_str::<PluginStartedEvent>(json)
                    .map(Self::PluginStarted)
                    .map_err(|e| PluginEventError::MalformedPayload(e.to_string()))
            }
            k if k == kinds::PLUGIN_STOPPED => {
                serde_json::from_str::<PluginStoppedEvent>(json)
                    .map(Self::PluginStopped)
                    .map_err(|e| PluginEventError::MalformedPayload(e.to_string()))
            }
            k if k == kinds::CAPABILITY_REGISTERED => {
                serde_json::from_str::<CapabilityRegisteredEvent>(json)
                    .map(Self::CapabilityRegistered)
                    .map_err(|e| PluginEventError::MalformedPayload(e.to_string()))
            }
            k if k == kinds::CAPABILITY_REMOVED => {
                serde_json::from_str::<CapabilityRemovedEvent>(json)
                    .map(Self::CapabilityRemoved)
                    .map_err(|e| PluginEventError::MalformedPayload(e.to_string()))
            }
            k if k == kinds::PLUGIN_WARNING => {
                serde_json::from_str::<PluginWarningEvent>(json)
                    .map(Self::PluginWarning)
                    .map_err(|e| PluginEventError::MalformedPayload(e.to_string()))
            }
            k if k == kinds::PLUGIN_ERROR => {
                serde_json::from_str::<PluginErrorEvent>(json)
                    .map(Self::PluginError)
                    .map_err(|e| PluginEventError::MalformedPayload(e.to_string()))
            }
            k if k == kinds::PLUGIN_REQUEST => {
                serde_json::from_str::<PluginRequestEvent>(json)
                    .map(Self::PluginRequest)
                    .map_err(|e| PluginEventError::MalformedPayload(e.to_string()))
            }
            k if k == kinds::PLUGIN_RESPONSE => {
                serde_json::from_str::<PluginResponseEvent>(json)
                    .map(Self::PluginResponse)
                    .map_err(|e| PluginEventError::MalformedPayload(e.to_string()))
            }
            _ => Err(PluginEventError::UnknownEventType(kind.to_string())),
        }
    }

    /// Validate that the event's required fields are populated.
    ///
    /// This checks for empty plugin IDs, empty method names, and unsupported
    /// API versions. It does not validate the semantic correctness of
    /// event-specific fields (e.g. whether a referenced capability exists).
    ///
    /// # Errors
    ///
    /// Returns [`PluginEventError`] if validation fails.
    pub fn validate(&self) -> Result<(), PluginEventError> {
        let api = self.api_version();
        if api.major != PluginApiVersion::CURRENT.major {
            return Err(PluginEventError::UnsupportedVersion(api));
        }

        match self {
            Self::PluginRegistered(e) => {
                if e.plugin_id.is_empty() {
                    return Err(PluginEventError::InvalidPluginId(e.plugin_id.clone()));
                }
                e.validate()?;
            }
            Self::PluginUnregistered(e) => {
                if e.plugin_id.is_empty() {
                    return Err(PluginEventError::InvalidPluginId(e.plugin_id.clone()));
                }
                e.validate()?;
            }
            Self::PluginLoaded(e) => {
                if e.plugin_id.is_empty() {
                    return Err(PluginEventError::InvalidPluginId(e.plugin_id.clone()));
                }
            }
            Self::PluginUnloaded(e) => {
                if e.plugin_id.is_empty() {
                    return Err(PluginEventError::InvalidPluginId(e.plugin_id.clone()));
                }
            }
            Self::PluginStarted(e) => {
                if e.plugin_id.is_empty() {
                    return Err(PluginEventError::InvalidPluginId(e.plugin_id.clone()));
                }
                e.validate()?;
            }
            Self::PluginStopped(e) => {
                if e.plugin_id.is_empty() {
                    return Err(PluginEventError::InvalidPluginId(e.plugin_id.clone()));
                }
                e.validate()?;
            }
            Self::CapabilityRegistered(e) => {
                if e.capability_id.is_empty() {
                    return Err(PluginEventError::MalformedPayload(
                        "capability_id must not be empty".to_string(),
                    ));
                }
            }
            Self::CapabilityRemoved(e) => {
                if e.capability_id.is_empty() {
                    return Err(PluginEventError::MalformedPayload(
                        "capability_id must not be empty".to_string(),
                    ));
                }
            }
            Self::PluginWarning(e) => {
                if e.plugin_id.is_empty() {
                    return Err(PluginEventError::InvalidPluginId(e.plugin_id.clone()));
                }
                if e.message.is_empty() {
                    return Err(PluginEventError::MalformedPayload(
                        "warning message must not be empty".to_string(),
                    ));
                }
            }
            Self::PluginError(e) => {
                if e.plugin_id.is_empty() {
                    return Err(PluginEventError::InvalidPluginId(e.plugin_id.clone()));
                }
                if e.error.is_empty() {
                    return Err(PluginEventError::MalformedPayload(
                        "error message must not be empty".to_string(),
                    ));
                }
            }
            Self::PluginRequest(e) => {
                if e.plugin_id.is_empty() {
                    return Err(PluginEventError::InvalidPluginId(e.plugin_id.clone()));
                }
                if e.method.is_empty() {
                    return Err(PluginEventError::MalformedPayload(
                        "request method must not be empty".to_string(),
                    ));
                }
            }
            Self::PluginResponse(e) => {
                if e.plugin_id.is_empty() {
                    return Err(PluginEventError::InvalidPluginId(e.plugin_id.clone()));
                }
                if e.method.is_empty() {
                    return Err(PluginEventError::MalformedPayload(
                        "response method must not be empty".to_string(),
                    ));
                }
                if e.status == PluginResponseStatus::Error && e.error.is_none() {
                    return Err(PluginEventError::MalformedPayload(
                        "error response must include an error message".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// PluginEventContract trait
// ---------------------------------------------------------------------------

/// The shared event contract that all plugin-generated events must implement.
///
/// This trait abstracts over the concrete [`PluginEvent`] enum and any
/// future custom event types that plugins may define. It provides a
/// uniform interface for:
///
/// - **Event kind**: the string used for EventBus subscription
/// - **Plugin identity**: which plugin originated the event
/// - **API version**: the event contract schema version
/// - **Timestamp**: when the event was created
/// - **EventBus integration**: conversion to a [`PipelineEvent`]
///
/// All methods are side-effect-free and return owned or borrowed values that
/// are safe to use across threads (`Send + Sync + Clone`).
///
/// # Example
///
/// ```ignore
/// use nabu_core::event_bus::EventBus;
/// use nabu_core::plugin::events::{PluginEvent, PluginEventContract, publish_plugin_event};
///
/// let event = PluginEvent::PluginLoaded(
///     nabu_core::plugin::events::PluginLoadedEvent::new("com.example.my-plugin", "My Plugin", "1.0.0")
/// );
///
/// publish_plugin_event(&event_bus, &event);
/// ```
pub trait PluginEventContract: Send + Sync + Clone + Serialize {
    /// Returns the event kind string used for EventBus subscription.
    fn kind(&self) -> &'static str;

    /// Returns the event category for broad filtering and routing.
    fn category(&self) -> PluginEventCategory;

    /// Returns the plugin ID that originated this event.
    fn plugin_id(&self) -> &str;

    /// Returns the event contract API version.
    fn api_version(&self) -> PluginApiVersion;

    /// Returns when the event was created.
    fn timestamp(&self) -> DateTime<Utc>;

    /// Convert this event into a [`PipelineEvent`] for EventBus publication.
    ///
    /// This is the integration point with the existing EventBus architecture:
    /// the returned `PipelineEvent` is published to the EventBus under the
    /// event's [`kind`](Self::kind).
    fn to_pipeline_event(&self) -> PipelineEvent;
}

impl PluginEventContract for PluginEvent {
    fn kind(&self) -> &'static str {
        self.kind()
    }

    fn category(&self) -> PluginEventCategory {
        self.category()
    }

    fn plugin_id(&self) -> &str {
        self.plugin_id()
    }

    fn api_version(&self) -> PluginApiVersion {
        self.api_version()
    }

    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp()
    }

    fn to_pipeline_event(&self) -> PipelineEvent {
        self.to_pipeline_event()
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Structured errors returned by plugin event operations.
///
/// This type covers the four categories of failure called out in the
/// contract specification:
///
/// - [`UnsupportedVersion`](PluginEventError::UnsupportedVersion) — the event
///   uses an API version this platform does not understand.
/// - [`MalformedPayload`](PluginEventError::MalformedPayload) — the serialized
///   payload is invalid or missing required fields.
/// - [`SerializationError`](PluginEventError::SerializationError) — Serde
///   serialization or deserialization failed.
/// - [`UnknownEventType`](PluginEventError::UnknownEventType) — the kind
///   string does not correspond to a known plugin event type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PluginEventError {
    /// The event uses an unsupported or incompatible API version.
    UnsupportedVersion(PluginApiVersion),
    /// The event payload is malformed (missing or invalid fields).
    MalformedPayload(String),
    /// Serialization or deserialization of the event failed.
    SerializationError(String),
    /// An unknown event type was encountered.
    UnknownEventType(String),
    /// The plugin ID is invalid (e.g. empty).
    InvalidPluginId(String),
}

impl std::fmt::Display for PluginEventError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion(v) => write!(
                f,
                "unsupported event version: API version {} is not supported by this platform (supports up to {})",
                v, PluginApiVersion::CURRENT
            ),
            Self::MalformedPayload(detail) => {
                write!(f, "malformed plugin event payload: {}", detail)
            }
            Self::SerializationError(detail) => {
                write!(f, "plugin event serialization failure: {}", detail)
            }
            Self::UnknownEventType(kind) => {
                write!(f, "unknown plugin event type: {}", kind)
            }
            Self::InvalidPluginId(id) => {
                write!(f, "invalid plugin ID: '{}'", id)
            }
        }
    }
}

impl std::error::Error for PluginEventError {}

// ---------------------------------------------------------------------------
// EventBus publishing helper
// ---------------------------------------------------------------------------

/// Publish a shared plugin event through the EventBus.
///
/// This is the canonical entry point for plugins to emit events. It:
///
/// 1. Converts the event to a [`PipelineEvent`] via
///    [`PluginEventContract::to_pipeline_event`].
/// 2. Publishes it to the [`EventBus`] under the event's [`kind`](PluginEventContract::kind).
///
/// Plugins should **never** call `EventBus::publish` directly with raw
/// `PipelineEvent` values — they should always go through this helper or
/// construct a [`PluginEvent`] and publish it.
///
/// # Example
///
/// ```ignore
/// use nabu_core::plugin::events::{PluginEvent, PluginWarningEvent, publish_plugin_event};
///
/// let event = PluginEvent::PluginWarning(
///     PluginWarningEvent::new("com.example.plugin", "disk space low")
/// );
/// publish_plugin_event(&event_bus, &event);
/// ```
pub fn publish_plugin_event(
    event_bus: &EventBus<PipelineEvent>,
    event: &impl PluginEventContract,
) {
    let kind = event.kind();
    let pipeline_event = event.to_pipeline_event();
    event_bus.publish(kind, &pipeline_event);
}

// ---------------------------------------------------------------------------
// CapabilityRegistry forward reference
// ---------------------------------------------------------------------------
// The `PluginEventContract` trait references `PipelineEvent` (from the
// `event_bus` module) for EventBus integration. The `CapabilityRegisteredEvent`
// and `CapabilityRemovedEvent` events correspond to state changes in the
// `CapabilityRegistry` (defined in `crate::plugin::capability`).
// This module depends on `event_bus` (for `PipelineEvent`, `EventBus`, `kinds`)
// and `event_bus` depends on this module (for the `PluginEvent` variant of
// `PipelineEvent`). This mutual dependency is safe within a single Rust crate.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // --- PluginApiVersion ---

    #[test]
    fn api_version_current() {
        assert_eq!(PluginApiVersion::CURRENT.major, 1);
        assert_eq!(PluginApiVersion::CURRENT.minor, 0);
    }

    #[test]
    fn api_version_default_is_current() {
        assert_eq!(PluginApiVersion::default(), PluginApiVersion::CURRENT);
    }

    #[test]
    fn api_version_compatibility() {
        assert!(PluginApiVersion::CURRENT.is_compatible());
        assert!(PluginApiVersion { major: 1, minor: 0 }.is_compatible());
        assert!(PluginApiVersion { major: 1, minor: 5 }.is_compatible());
        assert!(!PluginApiVersion { major: 2, minor: 0 }.is_compatible());
        assert!(!PluginApiVersion { major: 0, minor: 9 }.is_compatible());
    }

    #[test]
    fn api_version_serialization() {
        let v = PluginApiVersion::CURRENT;
        let json = serde_json::to_string(&v).unwrap();
        let back: PluginApiVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(v, back);
    }

    // --- Severity ---

    #[test]
    fn severity_serialization() {
        let sev = PluginEventSeverity::Critical;
        let json = serde_json::to_string(&sev).unwrap();
        assert!(json.contains("critical"));
        let back: PluginEventSeverity = serde_json::from_str(&json).unwrap();
        assert_eq!(sev, back);
    }

    // --- PluginEvent kind() ---

    #[test]
    fn plugin_event_kind_matches_constants() {
        let event = PluginEvent::PluginLoaded(PluginLoadedEvent::new("test", "Test", "1.0.0"));
        assert_eq!(event.kind(), kinds::PLUGIN_LOADED);

        let event = PluginEvent::PluginUnloaded(PluginUnloadedEvent::new("test"));
        assert_eq!(event.kind(), kinds::PLUGIN_UNLOADED);

        let event = PluginEvent::CapabilityRegistered(CapabilityRegisteredEvent::new("ns:cap", "test", "desc"));
        assert_eq!(event.kind(), kinds::CAPABILITY_REGISTERED);

        let event = PluginEvent::CapabilityRemoved(CapabilityRemovedEvent::new("ns:cap"));
        assert_eq!(event.kind(), kinds::CAPABILITY_REMOVED);

        let event = PluginEvent::PluginWarning(PluginWarningEvent::new("test", "warning"));
        assert_eq!(event.kind(), kinds::PLUGIN_WARNING);

        let event = PluginEvent::PluginError(PluginErrorEvent::new("test", "error"));
        assert_eq!(event.kind(), kinds::PLUGIN_ERROR);

        let event = PluginEvent::PluginRequest(PluginRequestEvent::new(
            "test", Uuid::nil(), "method"
        ));
        assert_eq!(event.kind(), kinds::PLUGIN_REQUEST);

        let event = PluginEvent::PluginResponse(PluginResponseEvent::new(
            "test", Uuid::nil(), "method", PluginResponseStatus::Success
        ));
        assert_eq!(event.kind(), kinds::PLUGIN_RESPONSE);
    }

    // --- Serialization round-trips ---

    #[test]
    fn plugin_loaded_event_round_trips() {
        let event = PluginEvent::PluginLoaded(PluginLoadedEvent::new(
            "com.example.test",
            "Test Plugin",
            "1.2.3",
        ));
        let json = serde_json::to_string(&event).unwrap();
        let back: PluginEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn plugin_unloaded_event_round_trips() {
        let event = PluginEvent::PluginUnloaded(PluginUnloadedEvent::new("com.example.test"));
        let json = serde_json::to_string(&event).unwrap();
        let back: PluginEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn capability_registered_event_round_trips() {
        let event = PluginEvent::CapabilityRegistered(CapabilityRegisteredEvent::new(
            "myplugin:ocr",
            "myplugin",
            "OCR provider",
        ));
        let json = serde_json::to_string(&event).unwrap();
        let back: PluginEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn capability_removed_event_round_trips() {
        let event = PluginEvent::CapabilityRemoved(CapabilityRemovedEvent::new("myplugin:ocr"));
        let json = serde_json::to_string(&event).unwrap();
        let back: PluginEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn plugin_warning_event_round_trips() {
        let event = PluginEvent::PluginWarning(
            PluginWarningEvent::with_code("com.example.test", "disk low", "LOW_DISK"),
        );
        let json = serde_json::to_string(&event).unwrap();
        let back: PluginEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn plugin_error_event_round_trips() {
        let event = PluginEvent::PluginError(PluginErrorEvent::critical(
            "com.example.test",
            "plugin panicked",
        ));
        let json = serde_json::to_string(&event).unwrap();
        let back: PluginEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn plugin_request_event_round_trips() {
        let event = PluginEvent::PluginRequest(PluginRequestEvent::with_params(
            "com.example.test",
            Uuid::nil(),
            "nabu:storage.read",
            serde_json::json!({ "path": "test.md" }),
        ));
        let json = serde_json::to_string(&event).unwrap();
        let back: PluginEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn plugin_response_event_round_trips() {
        let event = PluginEvent::PluginResponse(PluginResponseEvent::success(
            "com.example.test",
            Uuid::nil(),
            "nabu:storage.read",
            serde_json::json!({ "content": "hello" }),
        ));
        let json = serde_json::to_string(&event).unwrap();
        let back: PluginEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    // --- Forward compatibility: missing fields default ---

    #[test]
    fn plugin_loaded_event_defaults_missing_fields() {
        // Serialize a minimal event — only the required fields.
        let minimal = r#"{"plugin_id":"test","plugin_name":"Test","plugin_version":"1.0.0"}"#;
        let back: PluginLoadedEvent = serde_json::from_str(minimal).unwrap();
        assert_eq!(back.api_version, PluginApiVersion::CURRENT);
    }

    #[test]
    fn plugin_event_ignores_unknown_fields() {
        let json = r#"{"plugin_id":"test","plugin_name":"Test","plugin_version":"1.0.0","future_field":"value"}"#;
        let back: PluginLoadedEvent = serde_json::from_str(json).unwrap();
        assert_eq!(back.plugin_id, "test");
    }

    // --- from_kind_json ---

    #[test]
    fn from_kind_json_round_trips() {
        let event = PluginEvent::PluginLoaded(PluginLoadedEvent::new("test", "Test", "1.0.0"));
        let json = serde_json::to_string(&event).unwrap();
        let back = PluginEvent::from_kind_json(event.kind(), &json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn from_kind_json_unknown_kind() {
        let result = PluginEvent::from_kind_json("unknown.event", "{}");
        assert!(matches!(
            result,
            Err(PluginEventError::UnknownEventType(_))
        ));
    }

    #[test]
    fn from_kind_json_malformed_payload() {
        // Missing required field (plugin_id) for PluginLoadedEvent.
        // With #[serde(default)] this will succeed with defaults.
        // Let's test with truly malformed JSON instead.
        let result = PluginEvent::from_kind_json(kinds::PLUGIN_LOADED, "{not valid json}");
        assert!(matches!(result, Err(PluginEventError::MalformedPayload(_))));
    }

    // --- validate() ---

    #[test]
    fn validate_accepts_valid_event() {
        let event = PluginEvent::PluginLoaded(PluginLoadedEvent::new("test", "Test", "1.0.0"));
        assert!(event.validate().is_ok());
    }

    #[test]
    fn validate_rejects_empty_plugin_id() {
        let event = PluginEvent::PluginLoaded(PluginLoadedEvent::new("", "Test", "1.0.0"));
        assert!(matches!(
            event.validate(),
            Err(PluginEventError::InvalidPluginId(_))
        ));
    }

    #[test]
    fn validate_rejects_unsupported_major_version() {
        let event = PluginEvent::PluginLoaded(PluginLoadedEvent {
            api_version: PluginApiVersion { major: 99, minor: 0 },
            ..PluginLoadedEvent::new("test", "Test", "1.0.0")
        });
        assert!(matches!(
            event.validate(),
            Err(PluginEventError::UnsupportedVersion(_))
        ));
    }

    // --- PluginEventContract trait ---

    #[test]
    fn trait_methods_work_through_trait_object() {
        let event = PluginEvent::PluginLoaded(PluginLoadedEvent::new("test", "Test", "1.0.0"));

        fn check_contract<E: PluginEventContract>(event: &E) {
            assert!(!event.kind().is_empty());
            assert_eq!(event.plugin_id(), "test");
            assert_eq!(event.api_version(), PluginApiVersion::CURRENT);
        }

        check_contract(&event);
    }

    // --- EventBus integration ---

    #[test]
    fn publish_plugin_event_delivers_to_subscriber() {
        let bus = EventBus::<PipelineEvent>::new();
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let event = PluginEvent::PluginLoaded(PluginLoadedEvent::new("test", "Test", "1.0.0"));
        let kind = event.kind();

        bus.subscribe(kind, move |pe: &PipelineEvent| {
            if let PipelineEvent::Plugin(e) = pe {
                received_clone.lock().unwrap().push(e.clone());
            }
        });

        publish_plugin_event(&bus, &event);

        let stored = received.lock().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0], event);
    }

    #[test]
    fn publish_plugin_event_uses_correct_kind() {
        let bus = EventBus::<PipelineEvent>::new();
        let received = Arc::new(std::sync::Mutex::new(0usize));
        let received_clone = received.clone();

        let event = PluginEvent::PluginWarning(PluginWarningEvent::new("test", "warn"));
        bus.subscribe(event.kind(), move |_pe: &PipelineEvent| {
            *received_clone.lock().unwrap() += 1;
        });

        // Subscribe a different kind to verify the event doesn't leak.
        bus.subscribe(kinds::PLUGIN_LOADED, |_pe: &PipelineEvent| {});

        publish_plugin_event(&bus, &event);

        assert_eq!(*received.lock().unwrap(), 1);
    }

    // --- Error serialization ---

    #[test]
    fn plugin_event_error_serialization() {
        let err = PluginEventError::UnsupportedVersion(PluginApiVersion { major: 2, minor: 0 });
        let json = serde_json::to_string(&err).unwrap();
        let back: PluginEventError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);

        let err = PluginEventError::UnknownEventType("plugin.unknown".to_string());
        let json = serde_json::to_string(&err).unwrap();
        let back: PluginEventError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
    }

    #[test]
    fn plugin_event_error_display() {
        let err = PluginEventError::UnknownEventType("bad.kind".into());
        let msg = format!("{}", err);
        assert!(msg.contains("plugin.unknown") || msg.contains("bad.kind"));
    }

    // --- PluginEventCategory ---

    #[test]
    fn event_category_lifecycle_classifies_lifecycle_events() {
        let events = [
            PluginEvent::PluginRegistered(PluginRegisteredEvent::new("p", "P", "1.0.0")),
            PluginEvent::PluginUnregistered(PluginUnregisteredEvent::new("p")),
            PluginEvent::PluginLoaded(PluginLoadedEvent::new("p", "P", "1.0.0")),
            PluginEvent::PluginUnloaded(PluginUnloadedEvent::new("p")),
            PluginEvent::PluginStarted(PluginStartedEvent::new("p", "P", "1.0.0")),
            PluginEvent::PluginStopped(PluginStoppedEvent::new("p")),
        ];
        for e in &events {
            assert_eq!(e.category(), PluginEventCategory::Lifecycle);
        }
    }

    #[test]
    fn event_category_capability_classifies_capability_events() {
        let events = [
            PluginEvent::CapabilityRegistered(CapabilityRegisteredEvent::new("ns:cap", "p", "d")),
            PluginEvent::CapabilityRemoved(CapabilityRemovedEvent::new("ns:cap")),
        ];
        for e in &events {
            assert_eq!(e.category(), PluginEventCategory::Capability);
        }
    }

    #[test]
    fn event_category_diagnostic_classifies_warning_and_error() {
        let events = [
            PluginEvent::PluginWarning(PluginWarningEvent::new("p", "warn")),
            PluginEvent::PluginError(PluginErrorEvent::new("p", "err")),
        ];
        for e in &events {
            assert_eq!(e.category(), PluginEventCategory::Diagnostic);
        }
    }

    #[test]
    fn event_category_request_response_classifies_request_and_response() {
        let events = [
            PluginEvent::PluginRequest(PluginRequestEvent::new("p", Uuid::nil(), "m")),
            PluginEvent::PluginResponse(PluginResponseEvent::new(
                "p",
                Uuid::nil(),
                "m",
                PluginResponseStatus::Success,
            )),
        ];
        for e in &events {
            assert_eq!(e.category(), PluginEventCategory::RequestResponse);
        }
    }

    #[test]
    fn event_category_serialization() {
        let cat = PluginEventCategory::Capability;
        let json = serde_json::to_string(&cat).unwrap();
        let back: PluginEventCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(cat, back);
        assert!(json.contains("\"capability\""));
    }

    #[test]
    fn event_category_default_is_lifecycle() {
        assert_eq!(PluginEventCategory::default(), PluginEventCategory::Lifecycle);
    }

    #[test]
    fn event_category_as_str() {
        assert_eq!(PluginEventCategory::Lifecycle.as_str(), "lifecycle");
        assert_eq!(PluginEventCategory::Capability.as_str(), "capability");
        assert_eq!(PluginEventCategory::Diagnostic.as_str(), "diagnostic");
        assert_eq!(
            PluginEventCategory::RequestResponse.as_str(),
            "request_response"
        );
    }

    // --- EventSource ---

    #[test]
    fn event_source_serialization() {
        let src = EventSource::Plugin;
        let json = serde_json::to_string(&src).unwrap();
        let back: EventSource = serde_json::from_str(&json).unwrap();
        assert_eq!(src, back);
        assert!(json.contains("\"plugin\""));
    }

    #[test]
    fn event_source_default_is_platform() {
        assert_eq!(EventSource::default(), EventSource::Platform);
    }

    #[test]
    fn event_source_as_str() {
        assert_eq!(EventSource::Platform.as_str(), "platform");
        assert_eq!(EventSource::Plugin.as_str(), "plugin");
        assert_eq!(EventSource::User.as_str(), "user");
        assert_eq!(EventSource::External.as_str(), "external");
    }

    // --- PluginLifecycleState ---

    #[test]
    fn lifecycle_state_serialization_round_trip() {
        let states = [
            PluginLifecycleState::Discovered,
            PluginLifecycleState::Registered,
            PluginLifecycleState::Validated,
            PluginLifecycleState::Installed,
            PluginLifecycleState::Enabled,
            PluginLifecycleState::Disabled,
            PluginLifecycleState::Upgraded,
            PluginLifecycleState::Started,
            PluginLifecycleState::Running,
            PluginLifecycleState::Stopped,
            PluginLifecycleState::Unregistered,
            PluginLifecycleState::Unloaded,
        ];
        for state in &states {
            let json = serde_json::to_string(state).unwrap();
            let back: PluginLifecycleState = serde_json::from_str(&json).unwrap();
            assert_eq!(*state, back);
        }
    }

    #[test]
    fn lifecycle_state_default_is_unloaded() {
        assert_eq!(PluginLifecycleState::default(), PluginLifecycleState::Unloaded);
    }

    #[test]
    fn lifecycle_state_as_str() {
        assert_eq!(PluginLifecycleState::Discovered.as_str(), "discovered");
        assert_eq!(PluginLifecycleState::Registered.as_str(), "registered");
        assert_eq!(PluginLifecycleState::Validated.as_str(), "validated");
        assert_eq!(PluginLifecycleState::Installed.as_str(), "installed");
        assert_eq!(PluginLifecycleState::Enabled.as_str(), "enabled");
        assert_eq!(PluginLifecycleState::Disabled.as_str(), "disabled");
        assert_eq!(PluginLifecycleState::Upgraded.as_str(), "upgraded");
        assert_eq!(PluginLifecycleState::Started.as_str(), "started");
        assert_eq!(PluginLifecycleState::Running.as_str(), "running");
        assert_eq!(PluginLifecycleState::Stopped.as_str(), "stopped");
        assert_eq!(
            PluginLifecycleState::Unregistered.as_str(),
            "unregistered"
        );
        assert_eq!(PluginLifecycleState::Unloaded.as_str(), "unloaded");
    }

    #[test]
    fn lifecycle_state_from_stage_round_trip() {
        let stage = crate::plugin::lifecycle::PluginStage::Enabled;
        let state = PluginLifecycleState::from_stage(stage);
        assert_eq!(state, PluginLifecycleState::Enabled);

        let stage = crate::plugin::lifecycle::PluginStage::Installed;
        let state = PluginLifecycleState::from_stage(stage);
        assert_eq!(state, PluginLifecycleState::Installed);
    }

    // --- PluginRegisteredEvent ---

    #[test]
    fn registered_event_round_trips() {
        let event = PluginEvent::PluginRegistered(PluginRegisteredEvent::new(
            "com.example.test",
            "Test Plugin",
            "1.0.0",
        ));
        let json = serde_json::to_string(&event).unwrap();
        let back: PluginEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn registered_event_kind_matches_constant() {
        let event = PluginEvent::PluginRegistered(PluginRegisteredEvent::new(
            "test", "Test", "1.0.0",
        ));
        assert_eq!(event.kind(), kinds::PLUGIN_REGISTERED);
    }

    #[test]
    fn registered_event_plugin_id() {
        let event = PluginEvent::PluginRegistered(PluginRegisteredEvent::new(
            "com.example.test", "Test", "1.0.0",
        ));
        assert_eq!(event.plugin_id(), "com.example.test");
    }

    #[test]
    fn registered_event_default_state_is_registered() {
        let event = PluginRegisteredEvent::new("test", "Test", "1.0.0");
        assert_eq!(event.state, PluginLifecycleState::Registered);
    }

    #[test]
    fn registered_event_with_state_builder() {
        let event = PluginRegisteredEvent::new("test", "Test", "1.0.0")
            .with_state(PluginLifecycleState::Enabled);
        assert_eq!(event.state, PluginLifecycleState::Enabled);
    }

    #[test]
    fn registered_event_validate_rejects_empty_plugin_id() {
        let event = PluginRegisteredEvent::new("", "Test", "1.0.0");
        assert!(matches!(
            event.validate(),
            Err(PluginEventError::InvalidPluginId(_))
        ));
    }

    #[test]
    fn registered_event_validate_rejects_empty_plugin_name() {
        let event = PluginRegisteredEvent::plugin_id_only("test");
        assert!(matches!(
            event.validate(),
            Err(PluginEventError::MalformedPayload(_))
        ));
    }

    #[test]
    fn registered_event_validate_rejects_empty_version() {
        let event = PluginRegisteredEvent::new("test", "Test", "");
        assert!(matches!(
            event.validate(),
            Err(PluginEventError::MalformedPayload(_))
        ));
    }

    #[test]
    fn registered_event_validate_accepts_valid() {
        let event = PluginRegisteredEvent::new("test", "Test", "1.0.0");
        assert!(event.validate().is_ok());
    }

    // --- PluginUnregisteredEvent ---

    #[test]
    fn unregistered_event_round_trips() {
        let event = PluginEvent::PluginUnregistered(PluginUnregisteredEvent::new(
            "com.example.test",
        ));
        let json = serde_json::to_string(&event).unwrap();
        let back: PluginEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn unregistered_event_kind_matches_constant() {
        let event = PluginEvent::PluginUnregistered(PluginUnregisteredEvent::new("test"));
        assert_eq!(event.kind(), kinds::PLUGIN_UNREGISTERED);
    }

    #[test]
    fn unregistered_event_plugin_id() {
        let event = PluginEvent::PluginUnregistered(PluginUnregisteredEvent::new("test"));
        assert_eq!(event.plugin_id(), "test");
    }

    #[test]
    fn unregistered_event_default_state_is_unregistered() {
        let event = PluginUnregisteredEvent::new("test");
        assert_eq!(event.state, PluginLifecycleState::Unregistered);
    }

    #[test]
    fn unregistered_event_with_state_builder() {
        let event = PluginUnregisteredEvent::new("test")
            .with_state(PluginLifecycleState::Unloaded);
        assert_eq!(event.state, PluginLifecycleState::Unloaded);
    }

    #[test]
    fn unregistered_event_validate_rejects_empty_plugin_id() {
        let event = PluginUnregisteredEvent::new("");
        assert!(matches!(
            event.validate(),
            Err(PluginEventError::InvalidPluginId(_))
        ));
    }

    #[test]
    fn unregistered_event_validate_accepts_valid() {
        let event = PluginUnregisteredEvent::new("test");
        assert!(event.validate().is_ok());
    }

    // --- PluginStartedEvent ---

    #[test]
    fn started_event_round_trips() {
        let event = PluginEvent::PluginStarted(PluginStartedEvent::new(
            "com.example.test",
            "Test Plugin",
            "1.0.0",
        ));
        let json = serde_json::to_string(&event).unwrap();
        let back: PluginEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn started_event_kind_matches_constant() {
        let event = PluginEvent::PluginStarted(PluginStartedEvent::new(
            "test", "Test", "1.0.0",
        ));
        assert_eq!(event.kind(), kinds::PLUGIN_STARTED);
    }

    #[test]
    fn started_event_plugin_id() {
        let event = PluginEvent::PluginStarted(PluginStartedEvent::new(
            "com.example.test", "Test", "1.0.0",
        ));
        assert_eq!(event.plugin_id(), "com.example.test");
    }

    #[test]
    fn started_event_default_state_is_started() {
        let event = PluginStartedEvent::new("test", "Test", "1.0.0");
        assert_eq!(event.state, PluginLifecycleState::Started);
    }

    #[test]
    fn started_event_with_state_builder() {
        let event = PluginStartedEvent::new("test", "Test", "1.0.0")
            .with_state(PluginLifecycleState::Running);
        assert_eq!(event.state, PluginLifecycleState::Running);
    }

    #[test]
    fn started_event_validate_rejects_empty_plugin_id() {
        let event = PluginStartedEvent::new("", "Test", "1.0.0");
        assert!(matches!(
            event.validate(),
            Err(PluginEventError::InvalidPluginId(_))
        ));
    }

    #[test]
    fn started_event_validate_rejects_empty_plugin_name() {
        let event = PluginStartedEvent::plugin_id_version_only("test", "1.0.0");
        assert!(matches!(
            event.validate(),
            Err(PluginEventError::MalformedPayload(_))
        ));
    }

    #[test]
    fn started_event_validate_rejects_empty_version() {
        let event = PluginStartedEvent::new("test", "Test", "");
        assert!(matches!(
            event.validate(),
            Err(PluginEventError::MalformedPayload(_))
        ));
    }

    #[test]
    fn started_event_validate_accepts_valid() {
        let event = PluginStartedEvent::new("test", "Test", "1.0.0");
        assert!(event.validate().is_ok());
    }

    // --- PluginStoppedEvent ---

    #[test]
    fn stopped_event_round_trips() {
        let event = PluginEvent::PluginStopped(PluginStoppedEvent::new("com.example.test"));
        let json = serde_json::to_string(&event).unwrap();
        let back: PluginEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, back);
    }

    #[test]
    fn stopped_event_kind_matches_constant() {
        let event = PluginEvent::PluginStopped(PluginStoppedEvent::new("test"));
        assert_eq!(event.kind(), kinds::PLUGIN_STOPPED);
    }

    #[test]
    fn stopped_event_plugin_id() {
        let event = PluginEvent::PluginStopped(PluginStoppedEvent::new("test"));
        assert_eq!(event.plugin_id(), "test");
    }

    #[test]
    fn stopped_event_default_state_is_stopped() {
        let event = PluginStoppedEvent::new("test");
        assert_eq!(event.state, PluginLifecycleState::Stopped);
    }

    #[test]
    fn stopped_event_with_state_builder() {
        let event = PluginStoppedEvent::new("test")
            .with_state(PluginLifecycleState::Unloaded);
        assert_eq!(event.state, PluginLifecycleState::Unloaded);
    }

    #[test]
    fn stopped_event_validate_rejects_empty_plugin_id() {
        let event = PluginStoppedEvent::new("");
        assert!(matches!(
            event.validate(),
            Err(PluginEventError::InvalidPluginId(_))
        ));
    }

    #[test]
    fn stopped_event_validate_accepts_valid() {
        let event = PluginStoppedEvent::new("test");
        assert!(event.validate().is_ok());
    }

    // --- New event kind() matches constants ---

    #[test]
    fn all_plugin_event_kinds_match_constants() {
        let events = [
            PluginEvent::PluginRegistered(PluginRegisteredEvent::new("t", "T", "1")),
            PluginEvent::PluginUnregistered(PluginUnregisteredEvent::new("t")),
            PluginEvent::PluginLoaded(PluginLoadedEvent::new("t", "T", "1")),
            PluginEvent::PluginUnloaded(PluginUnloadedEvent::new("t")),
            PluginEvent::PluginStarted(PluginStartedEvent::new("t", "T", "1")),
            PluginEvent::PluginStopped(PluginStoppedEvent::new("t")),
            PluginEvent::CapabilityRegistered(CapabilityRegisteredEvent::new("ns:c", "t", "d")),
            PluginEvent::CapabilityRemoved(CapabilityRemovedEvent::new("ns:c")),
            PluginEvent::PluginWarning(PluginWarningEvent::new("t", "w")),
            PluginEvent::PluginError(PluginErrorEvent::new("t", "e")),
            PluginEvent::PluginRequest(PluginRequestEvent::new("t", Uuid::nil(), "m")),
            PluginEvent::PluginResponse(PluginResponseEvent::new(
                "t",
                Uuid::nil(),
                "m",
                PluginResponseStatus::Success,
            )),
        ];

        let expected = [
            kinds::PLUGIN_REGISTERED,
            kinds::PLUGIN_UNREGISTERED,
            kinds::PLUGIN_LOADED,
            kinds::PLUGIN_UNLOADED,
            kinds::PLUGIN_STARTED,
            kinds::PLUGIN_STOPPED,
            kinds::CAPABILITY_REGISTERED,
            kinds::CAPABILITY_REMOVED,
            kinds::PLUGIN_WARNING,
            kinds::PLUGIN_ERROR,
            kinds::PLUGIN_REQUEST,
            kinds::PLUGIN_RESPONSE,
        ];

        for (event, expected_kind) in events.iter().zip(expected.iter()) {
            assert_eq!(event.kind(), *expected_kind, "kind mismatch for event");
        }
    }

    #[test]
    fn from_kind_json_round_trips_all_events() {
        let events: Vec<PluginEvent> = vec![
            PluginEvent::PluginRegistered(PluginRegisteredEvent::new("t", "T", "1.0.0")),
            PluginEvent::PluginUnregistered(PluginUnregisteredEvent::new("t")),
            PluginEvent::PluginLoaded(PluginLoadedEvent::new("t", "T", "1.0.0")),
            PluginEvent::PluginUnloaded(PluginUnloadedEvent::new("t")),
            PluginEvent::PluginStarted(PluginStartedEvent::new("t", "T", "1.0.0")),
            PluginEvent::PluginStopped(PluginStoppedEvent::new("t")),
            PluginEvent::PluginWarning(PluginWarningEvent::new("t", "w")),
            PluginEvent::PluginError(PluginErrorEvent::new("t", "e")),
            PluginEvent::PluginRequest(PluginRequestEvent::new("t", Uuid::nil(), "m")),
            PluginEvent::PluginResponse(PluginResponseEvent::new(
                "t",
                Uuid::nil(),
                "m",
                PluginResponseStatus::Success,
            )),
        ];

        for event in &events {
            let json = serde_json::to_string(event).unwrap();
            let back = PluginEvent::from_kind_json(event.kind(), &json).unwrap();
            assert_eq!(*event, back, "round-trip failed for kind: {}", event.kind());
        }
    }

    #[test]
    fn validate_rejects_empty_plugin_id_all_lifecycle_events() {
        let events = [
            PluginEvent::PluginRegistered(PluginRegisteredEvent::new("", "T", "1")),
            PluginEvent::PluginUnregistered(PluginUnregisteredEvent::new("")),
            PluginEvent::PluginLoaded(PluginLoadedEvent::new("", "T", "1")),
            PluginEvent::PluginUnloaded(PluginUnloadedEvent::new("")),
            PluginEvent::PluginStarted(PluginStartedEvent::new("", "T", "1")),
            PluginEvent::PluginStopped(PluginStoppedEvent::new("")),
        ];

        for event in &events {
            assert!(
                matches!(event.validate(), Err(PluginEventError::InvalidPluginId(_))),
                "expected InvalidPluginId for {:?}",
                event
            );
        }
    }

    #[test]
    fn validate_rejects_unsupported_version_all_events() {
        let bad_version = PluginApiVersion { major: 99, minor: 0 };
        let events = [
            PluginEvent::PluginRegistered(PluginRegisteredEvent {
                api_version: bad_version,
                ..PluginRegisteredEvent::new("t", "T", "1")
            }),
            PluginEvent::PluginUnregistered(PluginUnregisteredEvent {
                api_version: bad_version,
                ..PluginUnregisteredEvent::new("t")
            }),
            PluginEvent::PluginStarted(PluginStartedEvent {
                api_version: bad_version,
                ..PluginStartedEvent::new("t", "T", "1")
            }),
            PluginEvent::PluginStopped(PluginStoppedEvent {
                api_version: bad_version,
                ..PluginStoppedEvent::new("t")
            }),
        ];

        for event in &events {
            assert!(
                matches!(event.validate(), Err(PluginEventError::UnsupportedVersion(_))),
                "expected UnsupportedVersion for {:?}",
                event
            );
        }
    }

    #[test]
    fn from_kind_json_all_new_kinds_round_trip() {
        let cases = [
            (
                kinds::PLUGIN_REGISTERED,
                serde_json::json!({
                    "plugin_id": "test",
                    "plugin_name": "Test",
                    "plugin_version": "1.0.0",
                    "state": "registered"
                }),
            ),
            (
                kinds::PLUGIN_UNREGISTERED,
                serde_json::json!({
                    "plugin_id": "test",
                    "state": "unregistered"
                }),
            ),
            (
                kinds::PLUGIN_STARTED,
                serde_json::json!({
                    "plugin_id": "test",
                    "plugin_name": "Test",
                    "plugin_version": "1.0.0",
                    "state": "started"
                }),
            ),
            (
                kinds::PLUGIN_STOPPED,
                serde_json::json!({
                    "plugin_id": "test",
                    "state": "stopped"
                }),
            ),
        ];

        for (kind, payload) in cases {
            let json = serde_json::to_string(&payload).unwrap();
            let event = PluginEvent::from_kind_json(kind, &json).unwrap();
            assert_eq!(event.kind(), kind);
        }
    }

    // --- PluginEventContract trait with new events ---

    #[test]
    fn contract_category_method_works_through_trait() {
        let event = PluginEvent::PluginRegistered(PluginRegisteredEvent::new(
            "test", "Test", "1.0.0",
        ));

        fn check_contract<E: PluginEventContract>(event: &E) {
            assert_eq!(event.category(), PluginEventCategory::Lifecycle);
        }

        check_contract(&event);
    }

    #[test]
    fn all_event_types_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PluginEventCategory>();
        assert_send_sync::<EventSource>();
        assert_send_sync::<PluginLifecycleState>();
        assert_send_sync::<PluginRegisteredEvent>();
        assert_send_sync::<PluginUnregisteredEvent>();
        assert_send_sync::<PluginStartedEvent>();
        assert_send_sync::<PluginStoppedEvent>();
    }

    // --- Forward compatibility: missing fields default ---

    #[test]
    fn registered_event_defaults_missing_fields() {
        let minimal = r#"{"plugin_id":"test","plugin_name":"Test","plugin_version":"1.0.0"}"#;
        let back: PluginRegisteredEvent = serde_json::from_str(minimal).unwrap();
        assert_eq!(back.api_version, PluginApiVersion::CURRENT);
        assert_eq!(back.state, PluginLifecycleState::Registered);
    }

    #[test]
    fn started_event_defaults_missing_fields() {
        let minimal = r#"{"plugin_id":"test","plugin_name":"Test","plugin_version":"1.0.0"}"#;
        let back: PluginStartedEvent = serde_json::from_str(minimal).unwrap();
        assert_eq!(back.api_version, PluginApiVersion::CURRENT);
        assert_eq!(back.state, PluginLifecycleState::Started);
    }

    #[test]
    fn stopped_event_defaults_missing_fields() {
        let minimal = r#"{"plugin_id":"test"}"#;
        let back: PluginStoppedEvent = serde_json::from_str(minimal).unwrap();
        assert_eq!(back.api_version, PluginApiVersion::CURRENT);
        assert_eq!(back.state, PluginLifecycleState::Stopped);
    }

    #[test]
    fn unregistered_event_defaults_missing_fields() {
        let minimal = r#"{"plugin_id":"test"}"#;
        let back: PluginUnregisteredEvent = serde_json::from_str(minimal).unwrap();
        assert_eq!(back.api_version, PluginApiVersion::CURRENT);
        assert_eq!(back.state, PluginLifecycleState::Unregistered);
    }

    // --- EventBus integration for new events ---

    #[test]
    fn registered_event_publishes_through_event_bus() {
        let bus = EventBus::<PipelineEvent>::new();
        let received = Arc::new(std::sync::Mutex::new(Vec::new()));
        let received_clone = received.clone();

        let event = PluginEvent::PluginRegistered(PluginRegisteredEvent::new(
            "test", "Test", "1.0.0",
        ));
        let kind = event.kind();

        bus.subscribe(kind, move |pe: &PipelineEvent| {
            if let PipelineEvent::Plugin(e) = pe {
                received_clone.lock().unwrap().push(e.clone());
            }
        });

        publish_plugin_event(&bus, &event);

        let stored = received.lock().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0], event);
    }

    #[test]
    fn started_event_publishes_through_event_bus() {
        let bus = EventBus::<PipelineEvent>::new();
        let received = Arc::new(std::sync::Mutex::new(false));
        let received_clone = received.clone();

        let event = PluginEvent::PluginStarted(PluginStartedEvent::new(
            "test", "Test", "1.0.0",
        ));

        bus.subscribe(event.kind(), move |pe: &PipelineEvent| {
            if let PipelineEvent::Plugin(e) = pe {
                assert_eq!(e.plugin_id(), "test");
                *received_clone.lock().unwrap() = true;
            }
        });

        publish_plugin_event(&bus, &event);

        assert!(*received.lock().unwrap());
    }
}
