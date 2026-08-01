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
//! └── DependencyGraph     — resolves plugin dependency graphs
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
//! ```
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
//! ## Future Compatibility
//!
//! The architecture naturally supports:
//! - Multiple plugin runtimes (Wasm, Lua, native, external)
//! - Capability-based extension discovery
//! - Lifecycle hooks for every transition
//! - Version negotiation and migration
//! - Sandboxed permission enforcement
//! - Staged feature rollout

pub mod capability;
pub mod dependency;
pub mod features;
pub mod lifecycle;
pub mod manager;
pub mod manifest;
pub mod permissions;
pub mod version;

// Re-exports
pub use capability::CapabilityRegistry;
pub use features::{FeatureFlag, FeatureRegistry, FeatureStage};
pub use lifecycle::{PluginLifecycle, PluginLifecycleEvent, PluginStage};
pub use manager::{InstallationReport, ManagerError, ManagerReport, PluginManager, RegistrationIssue};
pub use manifest::{CompatibilityCheck, ManifestError, PluginDependency, PluginEntryType, PluginFeatureFlag, PluginManifest, PluginPermission};
pub use permissions::{Permission, PermissionCheck, PermissionEvaluator, PermissionSet, PermissionValidation, RiskLevel};
pub use version::{CompatibilityResult, Version, VersionError, VersionRequirement};
