//! # Plugin Foundation
//!
//! The complete architectural foundation for Nabu's future plugin ecosystem.
//!
//! ## Architecture
//!
//! ```text
//! PluginManager
//! │
//! ├── CapabilityRegistry   — what the system and plugins can provide
//! ├── FeatureRegistry     — local feature flags (experimental, beta, etc.)
//! ├── PermissionEvaluator — validates requested permissions
//! ├── DependencyGraph     — resolves plugin dependency graphs
//! └── PluginEvent contract — shared, versionable event types for EventBus
//!```
//!
//! PluginManifest
//! ├── Version            — semver parsing and compatibility
//! ├── PluginDependency   — required and optional dependencies
//! ├── PluginCapability   — capabilities the plugin provides
//! ├── PluginPermission   — permissions the plugin requests
//! └── PluginEntryType    — the type of plugin (Wasm, Lua, etc.)
//!
//! PluginLifecycle
//! └── Discovered → Validated → Installed → Enabled → Disabled → Upgraded → Unloaded
//!
//! ## Event Flow
//!
//! ```text
//! Plugin
//!   │  (creates shared event)
//!   ▼
//! PluginEvent                    ── implements ──▶ PluginEventContract
//!   │  (publish_plugin_event wraps in PipelineEvent::Plugin)
//!   ▼
//! EventBus<PipelineEvent>        (single source of truth for platform events)
//!   │
//!   ▼
//! Platform Services              (Indexers, Graph, Frontend bridge, etc.)
//! ```
//!
//! Plugins communicate exclusively through the shared event contract defined
//! in [`events`] — never raw `PipelineEvent` values. The
//! [`publish_plugin_event`] helper is the canonical publishing entry point.
//!
//! ## Key Design Decisions
//!
//! 1. **No code execution** — the foundation validates metadata only.
//!    Plugin runtime loading will be added in a future phase.
//!
//! 2. **Strict validation** — manifests are validated structurally and
//!    semantically before registration.
//!
//! 3. **Semantic versioning** — all compatibility checks follow semver rules.
//!    Major version 0 is treated as unstable (requires exact match).
//!
//! 4. **Dependency graph** — circular dependencies are detected and rejected.
//!    Topological ordering is computed for installation.
//!
//! 5. **Permission model** — plugins declare what permissions they need.
//!    The model is foundation-only; runtime enforcement comes later.
//!
//! 6. **Feature flags** — experimental features are gated behind flags.
//!    All flags are local; nothing is ever sent to external services.
//!
//! 7. **Shared event contracts** — plugins communicate through strongly-typed,
//!    versionable event models (`PluginEvent`) that implement the
//!    `PluginEventContract` trait. Events are published through the EventBus
//!    via the `publish_plugin_event` helper, never as raw `PipelineEvent`
//!    values. All event types derive `Serialize`/`Deserialize` and use
//!    `#[serde(default)]` for forward-compatible deserialization.
//!
//! ## Future Compatibility
//!
//! The architecture naturally supports:
//! - Multiple plugin runtimes (Wasm, Lua, native, external)
//! - Capability-based extension discovery
//! - Lifecycle hooks for every transition
//! - Version negotiation and migration
//! - Sandboxed permission enforcement
//! - Staged feature rollout
//! - Dynamic, WASM, remote, and marketplace plugins communicating through
//!   the shared event contract
//! - AI plugins with request/response event patterns

pub mod capability;
pub mod dependency;
pub mod events;
pub mod features;
pub mod lifecycle;
pub mod manager;
pub mod manifest;
pub mod permissions;
pub mod provider;
pub mod version;

// Re-exports
pub use capability::CapabilityRegistry;
pub use events::{
    CapabilityRegisteredEvent, CapabilityRemovedEvent, PluginApiVersion, PluginErrorEvent,
    PluginEvent, PluginEventContract, PluginEventError, PluginEventSeverity,
    PluginLoadedEvent, PluginRequestEvent, PluginResponseEvent, PluginResponseStatus,
    PluginUnloadedEvent, PluginWarningEvent, publish_plugin_event,
};
pub use features::{FeatureFlag, FeatureRegistry, FeatureStage};
pub use lifecycle::{PluginLifecycle, PluginLifecycleEvent, PluginStage};
pub use manager::{
    InstallationReport, ManagerError, ManagerReport, PluginManager, RegistrationIssue,
};
pub use provider::{CapabilityProvider, ProviderError, SharedProvider};
pub use manifest::{
    CompatibilityCheck, ManifestError, PluginDependency, PluginEntryType, PluginFeatureFlag,
    PluginManifest, PluginPermission,
};
pub use permissions::{
    Permission, PermissionCheck, PermissionEvaluator, PermissionSet, PermissionValidation,
    RiskLevel,
};
pub use version::{CompatibilityResult, Version, VersionError, VersionRequirement};
