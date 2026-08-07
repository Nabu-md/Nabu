//! Plugin Manager (Foundation) — the central coordinator for plugin lifecycle.
//!
//! This is a **foundation implementation only**.
//!
//! Responsibilities:
//! - Manifest discovery (future: from disk, from registry)
//! - Manifest validation
//! - Dependency analysis
//! - Compatibility checking
//! - Capability registration
//! - Coordinating [`CapabilityProvider`] registration, validation, unregistration,
//!   and lifecycle — forwarding exposed capabilities through the [`CapabilityRegistry`].
//!
//! No plugin code is loaded or executed.
//! Plugin loading will be implemented in a future phase.
//!
//! # Provider & Capability Integration (complete)
//!
//! The `PluginManager`, [`CapabilityProvider`], and [`CapabilityRegistry`]
//! are fully wired:
//!
//! ```text
//! Plugin ─▶ implements ─▶ CapabilityProvider
//!                             │  (id / capabilities / initialize / shutdown)
//!                             ▼
//!                    PluginManager.register_provider()
//!                             │  validates, stages, commits atomically
//!                             ▼
//!                         CapabilityRegistry      (single source of truth)
//!                             │
//!                             ▼
//!                          Capability Platform
//! ```
//!
//! ## Ownership contract
//!
//! - [`CapabilityProvider`] **exposes** capabilities (it does not own them).
//! - [`PluginManager`] is the **sole coordinator**: it validates duplicates,
//!   stages capability registration against a registry copy, commits atomically
//!   (no partially-registered providers), and owns provider tracking.
//! - [`CapabilityRegistry`] **owns** all registered capability state
//!   (definitions, provider mapping, enabled set). Providers mutate it only
//!   through its public API; the manager mutates it only through that API's
//!   removal helpers.
//!
//! ## What a plugin author must do
//!
//! To participate in the platform, implement `CapabilityProvider` and hand it
//! to the manager:
//!
//! ```ignore
//! let pm = PluginManager::for_application().with_event_bus(event_bus);
//! let provider: Arc<dyn CapabilityProvider> = Arc::new(MyPlugin::new());
//! pm.register_provider(provider)?;        // registers capabilities atomically
//! pm.initialize_providers();              // at lifecycle start
//! pm.unregister_provider(id)?;            // removes provider + capabilities
//! ```
//!
//! No parallel lifecycle or registry access is required — the manager exposes
//! `initialize_providers` / `shutdown_providers` (wired into its `Lifecycle`
//! impl) and routes all mutations through the registry.
//!
//! Register/unregister operations also announce `CapabilityRegistered` /
//! `CapabilityRemoved` events through the `EventBus` (plus
//! `PluginLoaded` / `PluginUnloaded` / `PluginError` lifecycle events) when
//! the manager is constructed with an EventBus.

use std::collections::HashMap;
use std::sync::Arc;

use crate::event_bus::{EventBus, PipelineEvent};
use crate::plugin::capability::CapabilityRegistry;
use crate::plugin::dependency::{validate_dependencies, DependencyReport};
use crate::plugin::events::{
    CapabilityRegisteredEvent, CapabilityRemovedEvent, PluginErrorEvent, PluginEvent,
    PluginEventSeverity, PluginLoadedEvent, PluginUnloadedEvent, publish_plugin_event,
};
use crate::plugin::features::FeatureRegistry;
use crate::plugin::lifecycle::{PluginLifecycle, PluginLifecycleEvent, PluginStage};
use crate::plugin::manifest::{CompatibilityCheck, PluginManifest};
use crate::plugin::permissions::PermissionEvaluator;
use crate::plugin::provider::{CapabilityProvider, ProviderError};
use crate::plugin::version::Version;
use crate::registry::lifecycle::Lifecycle;

/// The central PluginManager responsible for the plugin lifecycle.
///
/// This is a **foundation** implementation.
/// Runtime plugin loading and execution will be added in a future phase.
///
/// Current capabilities:
/// - Register and validate plugin manifests
/// - Track plugin lifecycle stage
/// - Resolve dependency graphs
/// - Check Nabu version compatibility
/// - Register capabilities
/// - Track feature flags and permissions
/// - Coordinate [`CapabilityProvider`] registration, lifecycle, and removal
pub struct PluginManager {
    /// Registered plugin manifests (id → manifest).
    manifests: HashMap<String, PluginManifest>,
    /// Lifecycle states for each plugin.
    lifecycles: HashMap<String, PluginLifecycle>,
    /// The capability registry.
    capability_registry: CapabilityRegistry,
    /// The feature flag registry.
    feature_registry: FeatureRegistry,
    /// The permission evaluator.
    permission_evaluator: PermissionEvaluator,
    /// The current Nabu version for compatibility checks.
    nabu_version: Version,
    /// Registered capability providers (id → provider).
    ///
    /// Providers are stored as `Arc<dyn CapabilityProvider>` so the
    /// `PluginManager` depends on the trait abstraction, not concrete
    /// plugin implementations.
    providers: HashMap<String, Arc<dyn CapabilityProvider>>,
    /// Optional platform [`EventBus`] used to announce provider registration
    /// and removal as [`CapabilityRegisteredEvent`] /
    /// [`CapabilityRemovedEvent`] events.
    ///
    /// When present, `register_provider` / `unregister_provider` publish
    /// through the existing EventBus (via `publish_plugin_event`) rather than
    /// emitting events directly. When `None` (the default), no events are
    /// emitted and all registration is silent — this keeps existing
    /// constructions untouched.
    event_bus: Option<EventBus<PipelineEvent>>,
}

impl std::fmt::Debug for PluginManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginManager")
            .field("manifest_count", &self.manifests.len())
            .field("lifecycle_count", &self.lifecycles.len())
            .field("capability_count", &self.capability_registry.capability_count())
            .field("provider_count", &self.providers.len())
            .field("nabu_version", &self.nabu_version)
            .field("event_bus_attached", &self.event_bus.is_some())
            .finish()
    }
}

impl PluginManager {
    /// Create a PluginManager configured for the running Nabu application.
    ///
    /// Uses the compile-time `CARGO_PKG_VERSION` of nabu-core for compatibility
    /// checks. Falls back to `0.1.0` if the version string cannot be parsed.
    ///
    /// This is the canonical constructor for production use — it requires no
    /// arguments and performs no plugin discovery or loading.
    pub fn for_application() -> Self {
        let version = Version::parse(crate::APPLICATION_VERSION)
            .unwrap_or_else(|_| Version::new(0, 1, 0));
        Self::new(version)
    }

    /// Create a new PluginManager with the given Nabu version.
    pub fn new(nabu_version: Version) -> Self {
        let mut capability_registry = CapabilityRegistry::new();
        capability_registry.register_builtin();

        let mut feature_registry = FeatureRegistry::new();
        feature_registry.register_standard_flags();

        Self {
            manifests: HashMap::new(),
            lifecycles: HashMap::new(),
            capability_registry,
            feature_registry,
            permission_evaluator: PermissionEvaluator::new(),
            nabu_version,
            providers: HashMap::new(),
            event_bus: None,
        }
    }

    /// Create a PluginManager with custom registries (useful for testing).
    pub fn with_registries(
        nabu_version: Version,
        capability_registry: CapabilityRegistry,
        feature_registry: FeatureRegistry,
    ) -> Self {
        Self {
            manifests: HashMap::new(),
            lifecycles: HashMap::new(),
            capability_registry,
            feature_registry,
            permission_evaluator: PermissionEvaluator::new(),
            nabu_version,
            providers: HashMap::new(),
            event_bus: None,
        }
    }

    /// Attach the platform [`EventBus`] so that provider registration and
    /// removal announce `CapabilityRegistered` / `CapabilityRemoved` events.
    ///
    /// This is a builder-style setter that returns `self` for chaining. If no
    /// EventBus is attached, all provider operations remain silent but fully
    /// functional. Attaching an EventBus never changes registration semantics
    /// — it only adds platform event publication.
    pub fn with_event_bus(mut self, event_bus: EventBus<PipelineEvent>) -> Self {
        self.event_bus = Some(event_bus);
        self
    }

    // -----------------------------------------------------------------------
    // Registration & Discovery
    // -----------------------------------------------------------------------

    /// Register a plugin manifest for discovery and validation.
    ///
    /// Returns a list of validation errors (empty = valid).
    pub fn register_manifest(&mut self, manifest: PluginManifest) -> Vec<RegistrationIssue> {
        let mut issues = Vec::new();

        // 1. Validate manifest structure
        let validation_errors = manifest.validate();
        if !validation_errors.is_empty() {
            issues.push(RegistrationIssue::ValidationFailed {
                plugin_id: manifest.id.clone(),
                errors: validation_errors.iter().map(|e| format!("{}", e)).collect(),
            });
            return issues;
        }

        // 2. Check plugin ID uniqueness
        if self.manifests.contains_key(&manifest.id) {
            issues.push(RegistrationIssue::DuplicatePluginId {
                plugin_id: manifest.id.clone(),
            });
            return issues;
        }

        // 3. Check Nabu version compatibility
        match manifest.check_nabu_compatibility(&self.nabu_version) {
            CompatibilityCheck::Incompatible { reason } => {
                issues.push(RegistrationIssue::IncompatibleVersion {
                    plugin_id: manifest.id.clone(),
                    reason,
                });
                return issues;
            }
            CompatibilityCheck::Untested { reason } => {
                issues.push(RegistrationIssue::UntestedVersion {
                    plugin_id: manifest.id.clone(),
                    reason,
                });
                // Continue — untested is a warning, not a blocker
            }
            CompatibilityCheck::Compatible => {}
        }

        // 4. Validate requested permissions
        let permission_names: Vec<String> = manifest
            .permissions
            .iter()
            .map(|p| p.name.clone())
            .collect();
        let permission_validations = self
            .permission_evaluator
            .validate_requested(&permission_names);
        let invalid_permissions: Vec<String> = permission_validations
            .iter()
            .filter(|v| !v.valid)
            .map(|v| v.permission.clone())
            .collect();
        if !invalid_permissions.is_empty() {
            issues.push(RegistrationIssue::UnknownPermissions {
                plugin_id: manifest.id.clone(),
                permissions: invalid_permissions,
            });
            return issues;
        }

        // All checks passed — register the manifest
        let plugin_id = manifest.id.clone();
        self.manifests.insert(plugin_id.clone(), manifest);
        self.lifecycles
            .insert(plugin_id.clone(), PluginLifecycle::new());

        // Transition to Validated
        if let Some(lc) = self.lifecycles.get_mut(&plugin_id) {
            let _ = lc.transition_to(
                PluginStage::Validated,
                PluginLifecycleEvent::Validated {
                    plugin_id: plugin_id.clone(),
                },
            );
        }

        issues
    }

    // -----------------------------------------------------------------------
    // Provider Registration
    // -----------------------------------------------------------------------

    /// Register a [`CapabilityProvider`] with the PluginManager.
    ///
    /// This is the primary entry point for the provider abstraction. Any
    /// future plugin type — built-in, native shared library, WASM, scripting,
    /// remote — will be wrapped as `Arc<dyn CapabilityProvider>` and passed
    /// to this method.
    ///
    /// ## Coordination
    ///
    /// The PluginManager is the sole coordinator for provider registration:
    ///
    /// 1. It validates that no provider with the same ID is already tracked.
    /// 2. It stages the provider's capabilities against a **copy** of the
    ///    [`CapabilityRegistry`] via `provider.register_capabilities`.
    /// 3. Only if every capability registers cleanly is the staged copy
    ///    committed (atomically; the registry is the single source of truth).
    /// 4. Only on a successful commit is the provider tracked for later
    ///    lifecycle operations and queries.
    /// 5. If an [`EventBus`] is attached, a `PluginLoadedEvent` is published
    ///    and a `CapabilityRegistered` event is published for every capability
    ///    that this provider now supplies.
    ///
    /// The provider never mutates the live registry directly — it registers
    /// against a staged copy that the PluginManager commits. A failure at any
    /// point leaves the manager and registry completely unchanged (no
    /// partially-registered providers, no orphaned capabilities).
    ///
    /// This method is **metadata-only**: it does NOT load or execute plugin
    /// code, scan directories, or call the provider's `initialize` hook.
    /// Initialization happens later through
    /// [`initialize_providers`](Self::initialize_providers).
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::DuplicateProvider`] if a provider with the
    /// same ID is already registered. Returns
    /// [`ProviderError::DuplicateCapability`] if a capability exposed by
    /// this provider is already registered in the registry (by this or
    /// another provider).
    pub fn register_provider(
        &mut self,
        provider: Arc<dyn CapabilityProvider>,
    ) -> Result<(), ProviderError> {
        let provider_id = provider.id().to_string();

        // 1. Check for duplicate provider ID.
        if self.providers.contains_key(&provider_id) {
            return Err(ProviderError::DuplicateProvider { provider_id });
        }

        // 2. Stage registration against a copy of the registry so a failure
        //    never leaves a partially-registered provider or orphaned
        //    capabilities behind.
        let mut staged = self.capability_registry.clone();
        provider.register_capabilities(&mut staged)?;

        // 3. Determine which capabilities this provider now owns (used to
        //    emit events and to separate the provider's surface from the rest
        //    of the registry).
        let newly_registered: Vec<String> = provider
            .capabilities()
            .iter()
            .map(|c| c.id())
            .filter(|id| staged.provider(id) == Some(provider_id.as_str()))
            .collect();

        // 4. Commit — swap the live registry so the commit is atomic.
        self.capability_registry = staged;

        // 5. Track the provider.
        self.providers.insert(provider_id.clone(), provider.clone());

        // 6. Announce the newly registered capabilities through the EventBus.
        self.emit_capability_registered(&provider_id, &newly_registered);

        // 7. Announce the provider lifecycle event through the EventBus.
        self.emit_plugin_loaded(&provider_id, &provider);

        Ok(())
    }

    /// Unregister a previously registered [`CapabilityProvider`].
    ///
    /// This is the inverse of [`register_provider`](Self::register_provider)
    /// and completes the provider lifecycle: the provider's capabilities are
    /// removed from the [`CapabilityRegistry`] and the provider is no longer
    /// tracked or exported by the manager.
    ///
    /// ## Coordination
    ///
    /// The PluginManager remains the sole coordinator:
    ///
    /// 1. The provider's `shutdown` hook is invoked (release resources).
    /// 2. All capabilities owned by that provider are removed from the
    ///    [`CapabilityRegistry`] via
    ///    [`CapabilityRegistry::remove_by_provider`] — the registry owns the
    ///    mutation.
    /// 3. The provider is dropped from the manager's provider table.
    /// 4. If an [`EventBus`] is attached, a `CapabilityRemoved` event is
    ///    published for every removed capability, and a `PluginUnloadedEvent`
    ///    is published for the provider. If the `shutdown` hook failed,
    ///    a `PluginErrorEvent` is published with code `SHUTDOWN_FAILED`.
    ///
    /// The removal is atomic from the consumer's perspective: the provider
    /// table and the registry are updated together. A `shutdown` hook failure
    /// is reported via [`ProviderError::ShutdownFailed`] but does **not**
    /// prevent the unregistration from completing.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::UnknownProvider`] if no provider with the
    /// given ID is registered. Returns [`ProviderError::ShutdownFailed`] if
    /// the provider's `shutdown` hook failed (removal still completes).
    pub fn unregister_provider(&mut self, provider_id: &str) -> Result<(), ProviderError> {
        let provider = match self.providers.remove(provider_id) {
            Some(p) => p,
            None => {
                return Err(ProviderError::UnknownProvider {
                    provider_id: provider_id.to_string(),
                })
            }
        };

        // Attempt the shutdown hook; a failure is surfaced but non-fatal.
        let shutdown_result = provider.shutdown();

        // If the EventBus is attached, publish a PluginErrorEvent for the
        // shutdown failure (if any) through the shared event contract.
        if let Err(ref err) = shutdown_result {
            if let Some(ref bus) = self.event_bus {
                self.emit_provider_error(bus, provider_id, err);
            }
        }

        // Remove every capability owned by this provider — the registry owns
        // the mutation.
        let removed = self.capability_registry.remove_by_provider(provider_id);

        self.emit_capability_removed(&removed);

        // 7. Announce the provider removal through the EventBus.
        self.emit_plugin_unloaded(provider_id);

        shutdown_result
    }

    /// Validate a [`CapabilityProvider`] without mutating any state.
    ///
    /// This is a lightweight, side-effect-free check run by the coordinator
    /// before a provider participates in the platform. It verifies:
    ///
    /// - the provider ID is non-empty and not already registered
    /// - the provider exposes at least a stable identity
    /// - each capability has a well-formed `namespace:name` identifier
    /// - no two capabilities declared by this provider share an identifier
    /// - no declared capability already exists in the registry (owned by
    ///   this or another provider)
    ///
    /// Registration via [`register_provider`](Self::register_provider) already
    /// enforces these checks atomically. This method exists so a caller can
    /// inspect a provider's readiness *before* committing to registration.
    pub fn validate_provider(
        &self,
        provider: &dyn CapabilityProvider,
    ) -> Result<(), ProviderError> {
        let provider_id = provider.id();

        if provider_id.is_empty() {
            return Err(ProviderError::InvalidCapability {
                capability_id: String::new(),
                provider_id: provider_id.to_string(),
                reason: "provider ID must be non-empty".into(),
            });
        }

        if self.providers.contains_key(provider_id) {
            return Err(ProviderError::DuplicateProvider {
                provider_id: provider_id.to_string(),
            });
        }

        let mut seen: HashMap<String, ()> = HashMap::new();
        for cap in provider.capabilities() {
            let cap_id = cap.id();
            // Well-formed identifier.
            if cap.namespace.is_empty() || cap.name.is_empty() {
                return Err(ProviderError::InvalidCapability {
                    capability_id: cap_id,
                    provider_id: provider_id.to_string(),
                    reason: "capability namespace and name must be non-empty".into(),
                });
            }
            // No duplicates declared within this provider.
            if seen.contains_key(&cap_id) {
                return Err(ProviderError::DuplicateCapability {
                    capability_id: cap_id,
                    provider_id: provider_id.to_string(),
                });
            }
            seen.insert(cap_id.clone(), ());
            // No collision with an existing registered capability.
            if self.capability_registry.has(&cap_id) {
                return Err(ProviderError::DuplicateCapability {
                    capability_id: cap_id,
                    provider_id: provider_id.to_string(),
                });
            }
        }

        Ok(())
    }

    /// Invoke the `initialize` hook on every registered provider.
    ///
    /// Provider initialization is deliberately a separate phase from
    /// [`register_provider`](Self::register_provider) (which is metadata-only).
    /// This method lets the coordinator initialize providers once they are
    /// registered — typically as part of the application lifecycle's start.
    ///
    /// Every provider is attempted, so a single provider's init failure does
    /// not block the others. Returns the list of `(provider_id, error)`
    /// failures; an empty list means every provider initialized successfully.
    ///
    /// If an [`EventBus`] is attached, a `PluginErrorEvent` with code
    /// `INIT_FAILED` is published for each initialization failure through
    /// the shared plugin event contract.
    pub fn initialize_providers(&self) -> Vec<(String, ProviderError)> {
        let mut failures = Vec::new();
        for (id, provider) in &self.providers {
            if let Err(err) = provider.initialize() {
                if let Some(ref bus) = self.event_bus {
                    self.emit_provider_error(bus, id, &err);
                }
                failures.push((id.clone(), err));
            }
        }
        failures
    }

    /// Invoke the `shutdown` hook on every registered provider.
    ///
    /// The counterpart to [`initialize_providers`](Self::initialize_providers).
    /// Typically called during the application lifecycle's shutdown phase so
    /// providers can release resources. Every provider is attempted; returns
    /// the list of `(provider_id, error)` failures.
    ///
    /// If an [`EventBus`] is attached, a `PluginErrorEvent` with code
    /// `SHUTDOWN_FAILED` is published for each shutdown failure through
    /// the shared plugin event contract.
    pub fn shutdown_providers(&self) -> Vec<(String, ProviderError)> {
        let mut failures = Vec::new();
        for (id, provider) in &self.providers {
            if let Err(err) = provider.shutdown() {
                if let Some(ref bus) = self.event_bus {
                    self.emit_provider_error(bus, id, &err);
                }
                failures.push((id.clone(), err));
            }
        }
        failures
    }

    /// Publish `CapabilityRegistered` events for the given capability IDs.
    fn emit_capability_registered(&self, provider_id: &str, capability_ids: &[String]) {
        if let Some(bus) = &self.event_bus {
            for cap_id in capability_ids {
                let event = PluginEvent::CapabilityRegistered(
                    CapabilityRegisteredEvent::new(cap_id, provider_id, ""),
                );
                publish_plugin_event(bus, &event);
            }
        }
    }

    /// Publish `CapabilityRemoved` events for the given capability IDs.
    fn emit_capability_removed(&self, capability_ids: &[String]) {
        if let Some(bus) = &self.event_bus {
            for cap_id in capability_ids {
                let event =
                    PluginEvent::CapabilityRemoved(CapabilityRemovedEvent::new(cap_id));
                publish_plugin_event(bus, &event);
            }
        }
    }

    /// Publish a `PluginLoadedEvent` for a provider that was just registered.
    ///
    /// Uses the provider's identity (id, name, version) and the version
    /// string from its `Version` type. This is the canonical lifecycle event
    /// for provider registration — every provider that is successfully
    /// tracked by the `PluginManager` emits this event when an `EventBus`
    /// is attached.
    fn emit_plugin_loaded(
        &self,
        provider_id: &str,
        provider: &Arc<dyn CapabilityProvider>,
    ) {
        if let Some(bus) = &self.event_bus {
            let event = PluginEvent::PluginLoaded(PluginLoadedEvent::new(
                provider_id,
                provider.name(),
                &provider.version().to_string(),
            ));
            publish_plugin_event(bus, &event);
        }
    }

    /// Publish a `PluginUnloadedEvent` for a provider that was just removed.
    ///
    /// This is the lifecycle counterpart to [`emit_plugin_loaded`](Self::emit_plugin_loaded)
    /// — emitted after the provider is unregistered and its capabilities are
    /// removed from the registry.
    fn emit_plugin_unloaded(&self, provider_id: &str) {
        if let Some(bus) = &self.event_bus {
            let event =
                PluginEvent::PluginUnloaded(PluginUnloadedEvent::new(provider_id));
            publish_plugin_event(bus, &event);
        }
    }

    /// Publish a `PluginErrorEvent` for a provider lifecycle failure.
    /// Translates a `ProviderError` into a structured `PluginErrorEvent`
    /// with the appropriate severity and error code, then publishes it
    /// through the shared plugin event contract. This ensures that every
    /// provider initialization or shutdown failure is observable on the
    /// `plugin.error` EventBus kind.
    fn emit_provider_error(
        &self,
        bus: &EventBus<PipelineEvent>,
        provider_id: &str,
        err: &ProviderError,
    ) {
        let (severity, code, error_msg) = match err {
            ProviderError::InitializationFailed { reason, .. } => {
                (PluginEventSeverity::Error, "INIT_FAILED", reason.to_string())
            }
            ProviderError::ShutdownFailed { reason, .. } => {
                (PluginEventSeverity::Error, "SHUTDOWN_FAILED", reason.to_string())
            }
            ProviderError::DuplicateProvider { .. } => (
                PluginEventSeverity::Warning,
                "DUPLICATE_PROVIDER",
                err.to_string(),
            ),
            ProviderError::DuplicateCapability { .. } => (
                PluginEventSeverity::Warning,
                "DUPLICATE_CAPABILITY",
                err.to_string(),
            ),
            ProviderError::InvalidCapability { reason, .. } => (
                PluginEventSeverity::Error,
                "INVALID_CAPABILITY",
                reason.to_string(),
            ),
            ProviderError::RegistrationFailed { reason, .. } => (
                PluginEventSeverity::Error,
                "REGISTRATION_FAILED",
                reason.to_string(),
            ),
            ProviderError::UnknownProvider { .. } => (
                PluginEventSeverity::Warning,
                "UNKNOWN_PROVIDER",
                err.to_string(),
            ),
        };

        let mut event = PluginErrorEvent::new(provider_id, &error_msg);
        event.severity = severity;
        event.code = Some(code.to_string());
        publish_plugin_event(bus, &PluginEvent::PluginError(event));
    }

    // -----------------------------------------------------------------------
    // Installation
    // -----------------------------------------------------------------------

    /// Attempt to install all registered plugins.
    ///
    /// Resolves dependencies, checks compatibility, and registers capabilities.
    /// This is still metadata-only — no plugin code is executed.
    pub fn install_all(&mut self) -> InstallationReport {
        let manifests: Vec<PluginManifest> = self.manifests.values().cloned().collect();
        let dep_report = validate_dependencies(&manifests);

        let mut installed = Vec::new();
        let mut failed = Vec::new();

        if dep_report.has_critical_issues() {
            return InstallationReport {
                success: false,
                installed: vec![],
                failed: manifests.iter().map(|m| m.id.clone()).collect(),
                dependency_report: dep_report,
                reason: Some("Critical dependency issues detected".into()),
            };
        }

        // Install in topological order if available
        let order = dep_report
            .topological
            .clone()
            .unwrap_or_else(|| manifests.iter().map(|m| m.id.clone()).collect());

        for plugin_id in &order {
            let succeeded = self.install_single(plugin_id);
            if succeeded {
                installed.push(plugin_id.clone());
            } else {
                failed.push(plugin_id.clone());
            }
        }

        InstallationReport {
            success: failed.is_empty(),
            installed,
            failed,
            dependency_report: dep_report,
            reason: None,
        }
    }

    fn install_single(&mut self, plugin_id: &str) -> bool {
        let manifest = match self.manifests.get(plugin_id) {
            Some(m) => m.clone(),
            None => return false,
        };

        // Verify lifecycle is at Validated
        let can_install = self
            .lifecycles
            .get(plugin_id)
            .map(|lc| lc.is_at_least(PluginStage::Validated))
            .unwrap_or(false);

        if !can_install {
            return false;
        }

        // Register plugin capabilities
        for cap in &manifest.capabilities {
            self.capability_registry.register(
                crate::plugin::capability::Capability::new(
                    &manifest.id.replace('.', "_"),
                    &cap.id,
                    &cap.description,
                ),
                &manifest.id,
            );
        }

        // Update lifecycle
        if let Some(lc) = self.lifecycles.get_mut(plugin_id) {
            let _ = lc.transition_to(
                PluginStage::Installed,
                PluginLifecycleEvent::Installed {
                    plugin_id: plugin_id.into(),
                },
            );
        }

        true
    }

    // -----------------------------------------------------------------------
    // Enable / Disable
    // -----------------------------------------------------------------------

    /// Enable an installed plugin.
    pub fn enable(&mut self, plugin_id: &str) -> Result<(), ManagerError> {
        let can_enable = self
            .lifecycles
            .get(plugin_id)
            .map(|lc| lc.is_at_least(PluginStage::Installed))
            .unwrap_or(false);

        if !can_enable {
            return Err(ManagerError::NotInstalled(plugin_id.to_string()));
        }

        // Enable this plugin's capabilities
        if let Some(manifest) = self.manifests.get(plugin_id) {
            for cap in &manifest.capabilities {
                let cap_id = format!("{}:{}", manifest.id.replace('.', "_"), cap.id);
                self.capability_registry.enable(&cap_id);
            }
        }

        if let Some(lc) = self.lifecycles.get_mut(plugin_id) {
            let _ = lc.transition_to(
                PluginStage::Enabled,
                PluginLifecycleEvent::Enabled {
                    plugin_id: plugin_id.into(),
                },
            );
        }

        Ok(())
    }

    /// Disable an enabled plugin.
    pub fn disable(&mut self, plugin_id: &str) -> Result<(), ManagerError> {
        let is_enabled = self
            .lifecycles
            .get(plugin_id)
            .map(|lc| lc.stage() == PluginStage::Enabled)
            .unwrap_or(false);

        if !is_enabled {
            return Err(ManagerError::NotEnabled(plugin_id.to_string()));
        }

        // Disable this plugin's capabilities
        if let Some(manifest) = self.manifests.get(plugin_id) {
            for cap in &manifest.capabilities {
                let cap_id = format!("{}:{}", manifest.id.replace('.', "_"), cap.id);
                self.capability_registry.disable(&cap_id);
            }
        }

        if let Some(lc) = self.lifecycles.get_mut(plugin_id) {
            let _ = lc.transition_to(
                PluginStage::Disabled,
                PluginLifecycleEvent::Disabled {
                    plugin_id: plugin_id.into(),
                },
            );
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Query
    // -----------------------------------------------------------------------

    /// Get a registered manifest.
    pub fn manifest(&self, plugin_id: &str) -> Option<&PluginManifest> {
        self.manifests.get(plugin_id)
    }

    /// Get a plugin's lifecycle tracker.
    pub fn lifecycle(&self, plugin_id: &str) -> Option<&PluginLifecycle> {
        self.lifecycles.get(plugin_id)
    }

    /// Get the lifecycle stage of a plugin.
    pub fn stage(&self, plugin_id: &str) -> Option<PluginStage> {
        self.lifecycles.get(plugin_id).map(|lc| lc.stage())
    }

    /// List all registered plugin IDs.
    pub fn list_plugins(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.manifests.keys().cloned().collect();
        ids.sort();
        ids
    }

    /// List plugins at a specific lifecycle stage.
    pub fn plugins_at_stage(&self, stage: PluginStage) -> Vec<String> {
        self.lifecycles
            .iter()
            .filter(|(_, lc)| lc.stage() == stage)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Number of registered plugins.
    pub fn plugin_count(&self) -> usize {
        self.manifests.len()
    }

    // -----------------------------------------------------------------------
    // Provider Access
    // -----------------------------------------------------------------------

    /// Get a registered capability provider by ID.
    pub fn provider(&self, id: &str) -> Option<&Arc<dyn CapabilityProvider>> {
        self.providers.get(id)
    }

    /// List all registered provider IDs, sorted.
    pub fn list_providers(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.providers.keys().cloned().collect();
        ids.sort();
        ids
    }

    /// Number of registered capability providers.
    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    /// Reference to the capability registry.
    pub fn capability_registry(&self) -> &CapabilityRegistry {
        &self.capability_registry
    }

    /// Mutable reference to the capability registry.
    pub fn capability_registry_mut(&mut self) -> &mut CapabilityRegistry {
        &mut self.capability_registry
    }

    /// Reference to the feature registry.
    pub fn feature_registry(&self) -> &FeatureRegistry {
        &self.feature_registry
    }

    /// Mutable reference to the feature registry.
    pub fn feature_registry_mut(&mut self) -> &mut FeatureRegistry {
        &mut self.feature_registry
    }

    /// The current Nabu version used for compatibility checks.
    pub fn nabu_version(&self) -> &Version {
        &self.nabu_version
    }

    /// Run dependency analysis on all registered manifests.
    pub fn analyze_dependencies(&self) -> DependencyReport {
        let manifests: Vec<PluginManifest> = self.manifests.values().cloned().collect();
        validate_dependencies(&manifests)
    }

    /// Generate a report of all known plugins and their state.
    pub fn report(&self) -> ManagerReport {
        let manifests: Vec<PluginManifest> = self.manifests.values().cloned().collect();
        let dep_report = validate_dependencies(&manifests);

        let plugin_states: HashMap<String, PluginStage> = self
            .lifecycles
            .iter()
            .map(|(id, lc)| (id.clone(), lc.stage()))
            .collect();

        ManagerReport {
            plugin_count: self.manifests.len(),
            provider_count: self.providers.len(),
            capability_count: self.capability_registry.capability_count(),
            enabled_capabilities: self.capability_registry.enabled_count(),
            feature_flag_count: self.feature_registry.count(),
            plugins: plugin_states,
            dependency_report: dep_report,
            nabu_version: self.nabu_version.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Lifecycle
// ---------------------------------------------------------------------------

/// Implements the shared [`Lifecycle`] trait so that `PluginManager` can
/// participate in the Capability Platform's lifecycle management (Created →
/// Initialized → Running → Shutdown) alongside other services.
///
/// The foundation `PluginManager` performs no plugin discovery, loading, or
/// execution during these transitions. Each phase is a lightweight
/// bookkeeping step that prepares the manager for future plugin phases:
///
/// - **initialize**: validates internal registries and confirms the Nabu
///   version is available for compatibility checks.
/// - **start**: marks the manager as ready to accept plugin manifests.
/// - **shutdown**: no-op cleanup (plugins are metadata-only at this stage).
///
/// Future phases that add discovery/loading will enhance these methods
/// without changing the `ApplicationContext` integration.
impl Lifecycle for PluginManager {
    fn name(&self) -> &'static str {
        "plugin_manager"
    }

    /// Initializes the PluginManager.
    ///
    /// Validates that internal registries are populated. No plugin discovery
    /// or loading occurs at this stage — that belongs to a future phase.
    fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            subsystem = "plugin",
            component = "manager",
            operation = "initialize",
            plugin_count = self.plugin_count(),
            capabilities = self.capability_registry().capability_count(),
            nabu_version = %self.nabu_version,
            "PluginManager initialized"
        );
        Ok(())
    }

    /// Starts the PluginManager.
    ///
    /// The manager is now ready to accept plugin manifests and capability
    /// registrations. Providers already registered with the manager are
    /// initialized through their `initialize` hook so they integrate
    /// naturally with the application lifecycle. No plugins are loaded or
    /// executed — plugin execution will be implemented in a future phase.
    fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let init_failures = self.initialize_providers();
        if !init_failures.is_empty() {
            tracing::warn!(
                subsystem = "plugin",
                component = "manager",
                operation = "start",
                failed = init_failures.len(),
                total = self.provider_count(),
                "Some providers failed to initialize"
            );
        }
        tracing::info!(
            subsystem = "plugin",
            component = "manager",
            operation = "start",
            plugin_count = self.plugin_count(),
            "PluginManager started — ready for plugin registration"
        );
        Ok(())
    }

    /// Shuts down the PluginManager.
    ///
    /// Provider hooks are invoked through `shutdown_providers` so registered
    /// providers release resources as part of the application lifecycle.
    /// Since no plugin code is loaded at the foundation stage, this is
    /// otherwise a lightweight cleanup pass.
    fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        let shutdown_failures = self.shutdown_providers();
        if !shutdown_failures.is_empty() {
            tracing::warn!(
                subsystem = "plugin",
                component = "manager",
                operation = "shutdown",
                failures = shutdown_failures.len(),
                "Some providers failed to shut down"
            );
        }
        tracing::info!(
            subsystem = "plugin",
            component = "manager",
            operation = "shutdown",
            plugin_count = self.plugin_count(),
            "PluginManager shut down"
        );
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Issues that can occur during plugin registration.
#[derive(Debug, Clone, PartialEq)]
pub enum RegistrationIssue {
    ValidationFailed {
        plugin_id: String,
        errors: Vec<String>,
    },
    DuplicatePluginId {
        plugin_id: String,
    },
    IncompatibleVersion {
        plugin_id: String,
        reason: String,
    },
    UntestedVersion {
        plugin_id: String,
        reason: String,
    },
    UnknownPermissions {
        plugin_id: String,
        permissions: Vec<String>,
    },
}

/// Report from a batch installation attempt.
#[derive(Debug, Clone)]
pub struct InstallationReport {
    pub success: bool,
    pub installed: Vec<String>,
    pub failed: Vec<String>,
    pub dependency_report: DependencyReport,
    pub reason: Option<String>,
}

/// Summary report of the plugin manager state.
#[derive(Debug, Clone)]
pub struct ManagerReport {
    pub plugin_count: usize,
    pub provider_count: usize,
    pub capability_count: usize,
    pub enabled_capabilities: usize,
    pub feature_flag_count: usize,
    pub plugins: HashMap<String, PluginStage>,
    pub dependency_report: DependencyReport,
    pub nabu_version: Version,
}

/// Errors that can occur during plugin manager operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ManagerError {
    NotInstalled(String),
    NotEnabled(String),
    AlreadyInstalled(String),
    UnknownPlugin(String),
}

impl std::fmt::Display for ManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled(id) => write!(f, "Plugin '{}' is not installed", id),
            Self::NotEnabled(id) => write!(f, "Plugin '{}' is not enabled", id),
            Self::AlreadyInstalled(id) => write!(f, "Plugin '{}' is already installed", id),
            Self::UnknownPlugin(id) => write!(f, "Unknown plugin: '{}'", id),
        }
    }
}

impl std::error::Error for ManagerError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manifest::PluginEntryType;

    fn test_manifest(id: &str) -> PluginManifest {
        PluginManifest {
            id: id.to_string(),
            name: format!("Plugin {}", id),
            version: Version::new(1, 0, 0),
            author: "test".into(),
            description: "A test plugin".into(),
            min_nabu_version: Version::new(0, 1, 0),
            max_tested_version: None,
            manifest_version: 1,
            capabilities: vec![],
            dependencies: vec![],
            optional_dependencies: vec![],
            feature_flags: vec![],
            permissions: vec![],
            entry_type: PluginEntryType::Wasm,
        }
    }

    #[test]
    fn register_valid_manifest() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let issues = pm.register_manifest(test_manifest("com.example.test"));
        assert!(issues.is_empty());
        assert_eq!(pm.plugin_count(), 1);
    }

    #[test]
    fn duplicate_plugin_rejected() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        pm.register_manifest(test_manifest("com.example.test"));
        let issues = pm.register_manifest(test_manifest("com.example.test"));
        assert!(!issues.is_empty());
        assert!(matches!(
            issues[0],
            RegistrationIssue::DuplicatePluginId { .. }
        ));
    }

    #[test]
    fn incompatible_version_rejected() {
        let mut pm = PluginManager::new(Version::new(0, 0, 5));
        let manifest = PluginManifest {
            min_nabu_version: Version::new(1, 0, 0),
            ..test_manifest("com.example.test")
        };
        let issues = pm.register_manifest(manifest);
        assert!(!issues.is_empty());
        assert!(matches!(
            issues[0],
            RegistrationIssue::IncompatibleVersion { .. }
        ));
    }

    #[test]
    fn install_and_enable() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        pm.register_manifest(test_manifest("com.example.test"));
        let report = pm.install_all();
        assert!(report.success);
        assert_eq!(report.installed.len(), 1);
        assert_eq!(pm.stage("com.example.test"), Some(PluginStage::Installed));

        pm.enable("com.example.test").unwrap();
        assert_eq!(pm.stage("com.example.test"), Some(PluginStage::Enabled));
    }

    #[test]
    fn enable_uninstalled_plugin_fails() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let result = pm.enable("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn list_and_query() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        pm.register_manifest(test_manifest("p1"));
        pm.register_manifest(test_manifest("p2"));
        let list = pm.list_plugins();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"p1".to_string()));
    }

    #[test]
    fn report_generated() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        pm.register_manifest(test_manifest("com.example.test"));
        let report = pm.report();
        assert_eq!(report.plugin_count, 1);
        assert!(report.capability_count >= 10);
    }

    // --- Provider coordination unit tests ------------------------------------

    use crate::plugin::capability::Capability;

    #[derive(Debug)]
    struct UnitProvider {
        id: String,
        version: Version,
        caps: Vec<Capability>,
    }

    impl UnitProvider {
        fn new(id: &str, caps: Vec<Capability>) -> Self {
            Self {
                id: id.into(),
                version: Version::new(1, 0, 0),
                caps,
            }
        }
    }

    impl CapabilityProvider for UnitProvider {
        fn id(&self) -> &str {
            &self.id
        }
        fn name(&self) -> &str {
            &self.id
        }
        fn version(&self) -> &Version {
            &self.version
        }
        fn capabilities(&self) -> Vec<Capability> {
            self.caps.clone()
        }
        fn initialize(&self) -> Result<(), ProviderError> {
            // Record init through interior state on the Arc wrapper.
            Ok(())
        }
        fn shutdown(&self) -> Result<(), ProviderError> {
            Ok(())
        }
    }

    fn shared(p: UnitProvider) -> Arc<dyn CapabilityProvider> {
        Arc::new(p)
    }

    #[test]
    fn validate_provider_accepts_valid() {
        let pm = PluginManager::new(Version::new(1, 0, 0));
        let result = pm.validate_provider(&*shared(UnitProvider::new(
            "com.example.ok",
            vec![Capability::new("ns", "a", "A")],
        )));
        assert!(result.is_ok());
    }

    #[test]
    fn validate_provider_rejects_duplicate_provider() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        pm.register_provider(shared(UnitProvider::new(
            "com.example.dup",
            vec![Capability::new("ns", "a", "A")],
        )))
        .unwrap();
        let result = pm.validate_provider(&UnitProvider::new(
            "com.example.dup",
            vec![],
        ));
        assert!(matches!(result, Err(ProviderError::DuplicateProvider { .. })));
    }

    #[test]
    fn validate_provider_rejects_capability_collision() {
        let pm = PluginManager::new(Version::new(1, 0, 0));
        // "nabu:event_bus" is a built-in capability.
        let result = pm.validate_provider(&UnitProvider::new(
            "com.example.conflict",
            vec![Capability::new("nabu", "event_bus", "Conflicts with builtin")],
        ));
        assert!(matches!(
            result,
            Err(ProviderError::DuplicateCapability { .. })
        ));
    }

    #[test]
    fn validate_provider_rejects_malformed_capability() {
        let pm = PluginManager::new(Version::new(1, 0, 0));
        let result = pm.validate_provider(&UnitProvider::new(
            "com.example.bad",
            vec![Capability {
                namespace: String::new(),
                name: "x".into(),
                description: String::new(),
                required: false,
            }],
        ));
        assert!(matches!(result, Err(ProviderError::InvalidCapability { .. })));
    }

    #[test]
    fn unregister_provider_removes_it_and_its_capabilities() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let provider = Arc::new(UnitProvider::new(
            "com.example.unreg",
            vec![
                Capability::new("unreg", "a", "A"),
                Capability::new("unreg", "b", "B"),
            ],
        ));
        pm.register_provider(provider).unwrap();
        assert_eq!(pm.provider_count(), 1);
        assert!(pm.capability_registry().has("unreg:a"));

        pm.unregister_provider("com.example.unreg").unwrap();

        assert_eq!(pm.provider_count(), 0);
        assert!(pm.provider("com.example.unreg").is_none());
        assert!(!pm.capability_registry().has("unreg:a"));
        assert!(!pm.capability_registry().has("unreg:b"));
    }

    #[test]
    fn unregister_unknown_provider_errors() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let result = pm.unregister_provider("does.not.exist");
        assert!(matches!(result, Err(ProviderError::UnknownProvider { .. })));
    }

    #[test]
    fn can_reuse_provider_id_after_unregister() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        pm.register_provider(shared(UnitProvider::new(
            "com.example.reuse",
            vec![Capability::new("r", "a", "A")],
        )))
        .unwrap();
        pm.unregister_provider("com.example.reuse").unwrap();
        // The ID is freed; a new instance may register.
        pm.register_provider(shared(UnitProvider::new(
            "com.example.reuse",
            vec![Capability::new("r", "b", "B")],
        )))
        .unwrap();
        assert_eq!(pm.provider_count(), 1);
        assert!(pm.capability_registry().has("r:b"));
    }

    #[test]
    fn atomic_registration_leaves_no_partial_capabilities() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        // First provider owns "keep:feature".
        pm.register_provider(shared(UnitProvider::new(
            "com.example.owner",
            vec![Capability::new("keep", "feature", "Owned")],
        )))
        .unwrap();
        let before = pm.capability_registry().clone();

        // Register a failing provider whose first cap registers fine but whose
        // second cap collides with a built-in — must be all-or-nothing.
        let result = pm.register_provider(shared(UnitProvider::new(
            "com.example.fail",
            vec![
                Capability::new("fresh", "new", "New (would-be)"),
                Capability::new("nabu", "event_bus", "Collides"),
            ],
        )));
        assert!(result.is_err());

        let after = pm.capability_registry().clone();
        assert_eq!(before.capability_count(), after.capability_count());
        assert!(!after.has("fresh:new"));
        assert_eq!(pm.provider_count(), 1); // only the original owner survived
    }

    #[test]
    fn initialize_and_shutdown_hooks_run_via_manager() {
        use std::sync::atomic::{AtomicBool, Ordering};

        #[derive(Debug)]
        struct Tracked {
            id: String,
            version: Version,
            init: AtomicBool,
            shut: AtomicBool,
        }
        impl CapabilityProvider for Tracked {
            fn id(&self) -> &str {
                &self.id
            }
            fn name(&self) -> &str {
                &self.id
            }
            fn version(&self) -> &Version {
                &self.version
            }
            fn capabilities(&self) -> Vec<Capability> {
                vec![]
            }
            fn initialize(&self) -> Result<(), ProviderError> {
                self.init.store(true, Ordering::SeqCst);
                Ok(())
            }
            fn shutdown(&self) -> Result<(), ProviderError> {
                self.shut.store(true, Ordering::SeqCst);
                Ok(())
            }
        }

        let provider = Arc::new(Tracked {
            id: "com.example.lifecycle".into(),
            version: Version::new(1, 0, 0),
            init: AtomicBool::new(false),
            shut: AtomicBool::new(false),
        });
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        pm.register_provider(provider.clone()).unwrap();

        assert_eq!(pm.initialize_providers(), vec![]);
        assert!(provider.init.load(Ordering::SeqCst));

        assert_eq!(pm.shutdown_providers(), vec![]);
        assert!(provider.shut.load(Ordering::SeqCst));
    }

    #[test]
    fn event_bus_receives_registration_events() {
        use crate::event_bus::{EventBus, PipelineEvent};

        let bus: EventBus<PipelineEvent> = EventBus::new();
        let registered = Arc::new(std::sync::Mutex::new(Vec::new()));
        let removed = Arc::new(std::sync::Mutex::new(Vec::new()));
        bus.subscribe(
            crate::event_bus::kinds::CAPABILITY_REGISTERED,
            {
                let registered = registered.clone();
                move |pe: &PipelineEvent| {
                    if let PipelineEvent::Plugin(crate::plugin::events::PluginEvent::CapabilityRegistered(ref e)) = pe {
                        registered.lock().unwrap().push(e.capability_id.clone());
                    }
                }
            },
        );
        bus.subscribe(
            crate::event_bus::kinds::CAPABILITY_REMOVED,
            {
                let removed = removed.clone();
                move |pe: &PipelineEvent| {
                    if let PipelineEvent::Plugin(PluginEvent::CapabilityRemoved(ref e)) = pe {
                        removed.lock().unwrap().push(e.capability_id.clone());
                    }
                }
            },
        );

        let mut pm = PluginManager::new(Version::new(1, 0, 0)).with_event_bus(bus);
        pm.register_provider(shared(UnitProvider::new(
            "com.example.events",
            vec![
                Capability::new("ev", "a", "A"),
                Capability::new("ev", "b", "B"),
            ],
        )))
        .unwrap();

        {
            let regs = registered.lock().unwrap();
            assert_eq!(regs.len(), 2);
            assert!(regs.contains(&"ev:a".to_string()));
            assert!(regs.contains(&"ev:b".to_string()));
        }

        pm.unregister_provider("com.example.events").unwrap();

        {
            let rems = removed.lock().unwrap();
            assert_eq!(rems.len(), 2);
            assert!(rems.contains(&"ev:a".to_string()));
            assert!(rems.contains(&"ev:b".to_string()));
        }
    }
}
