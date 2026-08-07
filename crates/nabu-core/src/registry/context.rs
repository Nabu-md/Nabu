//! Application Context — the central container for all application services.
//!
//! [`ApplicationContext`] holds the [`ServiceRegistry`], plugin [`CapabilityRegistry`],
//! and provides typed accessor methods for known services. It also manages the service
//! lifecycle (initialize, start, shutdown) and performs validation.
//!
//! # Architecture
//!
//! ```text
//! ApplicationContext
//! ├── registry: ServiceRegistry
//! │   ├── event_bus          → EventBus
//! │   ├── capture_engine     → CaptureEngine
//! │   ├── pipeline           → ProcessingPipeline
//! │   ├── storage_manager    → StorageManager (via SQLiteStorage)
//! │   ├── job_queue          → JobQueue
//! │   ├── worker_pool        → WorkerPool
//! │   ├── vault_graph        → VaultGraph (RwLock)
//! │   ├── indexer            → Indexer (Tantivy)
//! │   ├── capture_handlers:  → [BrowserCaptureHandler, ClipboardHandler, …]
//! │   ├── processors:        → [ContentClassifier, DuplicateDetector, …]
//! │   ├── ai_providers:      → [future]
//! │   ├── ocr_providers:     → [future]
//! │   ├── embedding_providers: → [future]
//! │   ├── exporters:         → [future]
//! │   └── ...
//! ├── capability_registry: CapabilityRegistry
//! │   ├── nabu:event_bus     → built-in
//! │   ├── nabu:storage       → built-in
//! │   ├── nabu:capture       → built-in
//! │   ├── nabu:processor     → built-in
//! │   ├── nabu:graph         → built-in
//! │   ├── nabu:export        → built-in
//! │   ├── nabu:search        → built-in
//! │   └── ...
//! └── lifecycle: LifecycleManager
//!     └── Created → Initialized → Running → Shutdown
//! ```
//!
//! # Thread Safety
//!
//! `ApplicationContext` is `Send + Sync` and designed to be shared as
//! `Arc<ApplicationContext>` across threads.

use std::sync::{Arc, RwLock, RwLockReadGuard};

use chrono::Utc;

use crate::event_bus::{EventBus, PipelineEvent};
use crate::plugin::capability::CapabilityRegistry;

use crate::registry::lifecycle::{Lifecycle, LifecycleError, LifecycleManager, LifecycleStage};
use crate::registry::ServiceRegistry;

use crate::diagnostics::PerformanceMonitor;
use crate::jobs;

// ---------------------------------------------------------------------------
// Forward type aliases — prevents circular crate dependencies.
// Concrete types are resolved at crate level (nabu-core) via the registry.
// ---------------------------------------------------------------------------

/// Health status of a registered service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceHealth {
    /// Service is registered and functional.
    Healthy,
    /// Service is registered but not yet initialized.
    NotInitialized,
    /// Service is registered but in an error state.
    Unhealthy(String),
    /// Service is not registered.
    NotFound,
}

/// Result of validating required services in the context.
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// All required service keys that must be present.
    pub required_services: Vec<&'static str>,
    /// All optional service keys that may be present.
    pub optional_services: Vec<&'static str>,
    /// Keys of services that are present and healthy.
    pub present: Vec<&'static str>,
    /// Keys of required services that are missing.
    pub missing: Vec<&'static str>,
    /// Keys of services that are present but unhealthy.
    pub unhealthy: Vec<(String, String)>,
}

impl ValidationReport {
    /// Returns `true` if all required services are present and healthy.
    pub fn is_valid(&self) -> bool {
        self.missing.is_empty() && self.unhealthy.is_empty()
    }

    /// Returns the total number of registered services.
    pub fn total_count(&self) -> usize {
        self.present.len() + self.missing.len() + self.unhealthy.len()
    }

    /// Returns the number of missing required services.
    pub fn missing_count(&self) -> usize {
        self.missing.len()
    }

    /// Returns a human-readable summary of the validation.
    pub fn summary(&self) -> String {
        // Denominator is the total number of *known* services (present +
        // missing + unhealthy), not just the required list — present may also
        // include optional services that were found.
        let total = self.total_count();
        let mut parts = vec![format!(
            "Validation: {}/{} required services present",
            self.present.len(),
            total,
        )];

        if !self.missing.is_empty() {
            parts.push(format!("Missing: {}", self.missing.join(", ")));
        }
        if !self.unhealthy.is_empty() {
            let issues: Vec<String> = self
                .unhealthy
                .iter()
                .map(|(k, v)| format!("{}: {}", k, v))
                .collect();
            parts.push(format!("Unhealthy: {}", issues.join(", ")));
        }

        parts.join(" | ")
    }
}

// ---------------------------------------------------------------------------
// ApplicationContext
// ---------------------------------------------------------------------------

/// The central application context that holds and coordinates all services.
///
/// `ApplicationContext` is designed to be created once at application startup
/// and shared across the application as `Arc<ApplicationContext>`.
pub struct ApplicationContext {
    /// The service registry containing all registered services.
    pub registry: Arc<RwLock<ServiceRegistry>>,
    /// The central event bus for publish/subscribe communication.
    pub event_bus: Arc<EventBus<PipelineEvent>>,
    /// Plugin capability registry — describes what the system can do.
    ///
    /// Guarded by a `RwLock` so that IPC commands (which hold only a shared
    /// `&ApplicationContext`) can safely enable/disable capabilities against
    /// concurrently-running readers. The lock is the *single synchronization
    /// point* for capability state; every mutation routes through it.
    pub capability_registry: RwLock<CapabilityRegistry>,
    /// Manages service lifecycle (init → start → shutdown).
    lifecycle: LifecycleManager,
}

impl ApplicationContext {
    /// Creates a new `ApplicationContext` with the given registry and event bus.
    ///
    /// Prefer using [`ApplicationContext::builder`] for most use cases.
    pub fn new(
        registry: Arc<RwLock<ServiceRegistry>>,
        event_bus: Arc<EventBus<PipelineEvent>>,
        capability_registry: CapabilityRegistry,
    ) -> Self {
        Self {
            registry,
            event_bus,
            capability_registry: RwLock::new(capability_registry),
            lifecycle: LifecycleManager::new(),
        }
    }

    /// Returns a builder for constructing an `ApplicationContext`.
    pub fn builder() -> ApplicationContextBuilder {
        ApplicationContextBuilder::new()
    }

    // -----------------------------------------------------------------------
    // Registry access
    // -----------------------------------------------------------------------

    /// Returns a reference to the service registry.
    pub fn registry(&self) -> &Arc<RwLock<ServiceRegistry>> {
        &self.registry
    }

    /// Returns a reference to the event bus.
    pub fn event_bus(&self) -> &Arc<EventBus<PipelineEvent>> {
        &self.event_bus
    }

    /// Returns a read guard for the capability registry.
    ///
    /// Acquire this to inspect capability state. Mutations (enable/disable)
    /// must go through [`enable_capability`](Self::enable_capability) /
    /// [`disable_capability`](Self::disable_capability), which take the write
    /// lock and publish state-change events on the EventBus.
    pub fn capability_registry(&self) -> RwLockReadGuard<'_, CapabilityRegistry> {
        self.capability_registry
            .read()
            .expect("capability registry lock poisoned")
    }

    /// Enable a registered capability by its identifier.
    ///
    /// Delegates the state transition to the [`CapabilityRegistry`] (the single
    /// synchronization point for capability state). On success, publishes a
    /// `CapabilityStateChanged` event through the [`EventBus`] so any EventBus
    /// bridge can forward it to the frontend.
    ///
    /// Duplicate or invalid transitions are rejected cleanly — see
    /// [`CapabilityRegistry::enable_checked`].
    pub fn enable_capability(&self, id: &str) -> Result<(), String> {
        {
            let mut reg = self
                .capability_registry
                .write()
                .expect("capability registry lock poisoned");
            reg.enable_checked(id)?;
        }
        let event = crate::event_bus::CapabilityStateEvent {
            capability_id: id.to_string(),
            enabled: true,
            timestamp: Utc::now(),
        };
        self.event_bus.publish(
            crate::event_bus::kinds::CAPABILITY_STATE_CHANGED,
            &crate::event_bus::PipelineEvent::CapabilityStateChanged(event),
        );
        Ok(())
    }

    /// Disable a registered capability by its identifier (the inverse of
    /// [`enable_capability`](Self::enable_capability)).
    ///
    /// Delegates the state transition to the [`CapabilityRegistry`] and publishes
    /// a `CapabilityStateChanged` event on success.
    pub fn disable_capability(&self, id: &str) -> Result<(), String> {
        {
            let mut reg = self
                .capability_registry
                .write()
                .expect("capability registry lock poisoned");
            reg.disable_checked(id)?;
        }
        let event = crate::event_bus::CapabilityStateEvent {
            capability_id: id.to_string(),
            enabled: false,
            timestamp: Utc::now(),
        };
        self.event_bus.publish(
            crate::event_bus::kinds::CAPABILITY_STATE_CHANGED,
            &crate::event_bus::PipelineEvent::CapabilityStateChanged(event),
        );
        Ok(())
    }

    /// Resolves a service from the registry by key.
    ///
    /// Convenience wrapper around `registry.read().unwrap().resolve::<T>(key)`.
    pub fn resolve<T: Send + Sync + 'static>(&self, key: &str) -> Option<Arc<T>> {
        let registry = self.registry.read().expect("registry lock not poisoned");
        registry.resolve::<T>(key)
    }

    /// Resolves all services in a category.
    pub fn resolve_category<T: Send + Sync + 'static>(&self, category: &str) -> Vec<Arc<T>> {
        let registry = self.registry.read().expect("registry lock not poisoned");
        registry.resolve_category::<T>(category)
    }

    /// Registers a singleton service in the registry.
    pub fn register<T: Send + Sync + 'static>(&self, key: &str, service: Arc<T>) {
        let mut registry = self.registry.write().expect("registry lock not poisoned");
        registry.register(key, service);
    }

    /// Registers a service in a category.
    pub fn register_in_category(&self, category: &str, key: &str) {
        let mut registry = self.registry.write().expect("registry lock not poisoned");
        registry.register_in_category(category, key);
    }

    // -----------------------------------------------------------------------
    // Typed accessors — convenience methods for well-known services
    // -----------------------------------------------------------------------

    /// Returns the capture engine if registered.
    pub fn capture_engine(&self) -> Option<Arc<crate::capture::CaptureEngine>> {
        self.resolve("capture_engine")
    }

    /// Returns the processing pipeline if registered.
    pub fn processing_pipeline(&self) -> Option<Arc<crate::processing::ProcessingPipeline>> {
        self.resolve("pipeline")
    }

    /// Returns the job queue if registered.
    pub fn job_queue(&self) -> Option<Arc<jobs::DurableJobQueue>> {
        self.resolve("job_queue")
    }

    /// Returns the worker pool if registered.
    pub fn worker_pool(&self) -> Option<Arc<jobs::WorkerPool>> {
        self.resolve("worker_pool")
    }

    /// Returns the pipeline executor if registered.
    pub fn pipeline_executor(&self) -> Option<Arc<crate::pipeline_migration::PipelineExecutor>> {
        self.resolve("pipeline_executor")
    }

    /// Returns the vault graph if registered.
    pub fn vault_graph(&self) -> Option<Arc<RwLock<crate::graph::VaultGraph>>> {
        self.resolve("vault_graph")
    }

    /// Returns the indexer if registered.
    pub fn indexer(&self) -> Option<Arc<std::sync::Mutex<crate::indexer::Indexer>>> {
        self.resolve("indexer")
    }

    /// Returns the storage manager if registered.
    pub fn storage_manager(&self) -> Option<Arc<crate::storage::StorageManager>> {
        self.resolve("storage_manager")
    }

    /// Returns the universal history manager if registered.
    pub fn history_manager(&self) -> Option<Arc<std::sync::RwLock<crate::history::HistoryManager>>> {
        self.resolve("history_manager")
    }

    /// Returns the performance monitor if registered.
    pub fn performance_monitor(&self) -> Option<Arc<PerformanceMonitor>> {
        self.resolve("performance_monitor")
    }

    // -----------------------------------------------------------------------
    // Service health & validation
    // -----------------------------------------------------------------------

    /// Checks the health of a single service by key.
    pub fn check_health(&self, key: &str) -> ServiceHealth {
        let registry = self.registry.read().expect("registry lock not poisoned");
        if registry.has(key) {
            ServiceHealth::Healthy
        } else {
            ServiceHealth::NotFound
        }
    }

    /// Validates that all required services are present.
    ///
    /// The `required` list defines service keys that MUST be registered for
    /// the application to function. Missing required services are reported
    /// in [`ValidationReport::missing`].
    pub fn validate_services(
        &self,
        required: &[&'static str],
        optional: &[&'static str],
    ) -> ValidationReport {
        let registry = self.registry.read().expect("registry lock not poisoned");
        let mut present = Vec::new();
        let mut missing = Vec::new();
        let unhealthy = Vec::new();

        for key in required {
            if registry.has(key) {
                present.push(*key);
            } else {
                missing.push(*key);
            }
        }

        for key in optional {
            if registry.has(key) {
                present.push(*key);
            }
        }

        ValidationReport {
            required_services: required.to_vec(),
            optional_services: optional.to_vec(),
            present,
            missing,
            unhealthy,
        }
    }

    /// Validates the standard set of core services required for Nabu.
    ///
    /// This is a convenience method that checks all services that should
    /// normally be present in a running Nabu application.
    pub fn validate_core_services(&self) -> ValidationReport {
        self.validate_services(
            &["event_bus", "capture_engine", "pipeline", "storage_manager"],
            &["job_queue", "worker_pool", "vault_graph", "indexer"],
        )
    }

    /// Returns the number of registered services.
    pub fn service_count(&self) -> usize {
        let registry = self.registry.read().expect("registry lock not poisoned");
        registry.singleton_count() + registry.factory_count()
    }

    /// Returns the number of registered categories.
    pub fn category_count(&self) -> usize {
        let registry = self.registry.read().expect("registry lock not poisoned");
        registry.category_count()
    }

    // -----------------------------------------------------------------------
    // Lifecycle
    // -----------------------------------------------------------------------

    /// Returns the current lifecycle stage.
    pub fn lifecycle_stage(&self) -> LifecycleStage {
        self.lifecycle.stage()
    }

    /// Returns `true` if the context has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.lifecycle.stage() >= LifecycleStage::Initialized
    }

    /// Returns `true` if the context is running.
    pub fn is_running(&self) -> bool {
        self.lifecycle.stage() == LifecycleStage::Running
    }

    /// Returns `true` if the context has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.lifecycle.stage() == LifecycleStage::Shutdown
    }

    /// Transitions the context to the `Initialized` stage.
    ///
    /// Validates that all required services are registered, then initializes
    /// every lifecycle-managed service in dependency-safe order:
    ///
    /// StorageManager → WorkerPool → PipelineExecutor → CaptureEngine → Indexer → VaultGraph
    ///
    /// Initialization performs configuration validation, dependency
    /// verification, and resource allocation — no background work begins.
    ///
    /// # Errors
    ///
    /// Returns a list of error descriptions if validation fails (missing
    /// services) or if any service's `initialize()` returns an error.
    /// If any service fails to initialize, the context remains at `Created`
    /// and `start()` will not proceed.
    pub fn initialize(&self) -> Result<(), Vec<String>> {
        let report = self.validate_core_services();
        if !report.is_valid() {
            return Err(report.missing.iter().map(|s| (*s).to_string()).collect());
        }

        tracing::info!("Initializing lifecycle services");

        let mut errors: Vec<String> = Vec::new();

        // Initialize in dependency-safe order:
        // StorageManager → WorkerPool → PipelineExecutor → CaptureEngine → Indexer → VaultGraph

        // --- StorageManager (foundation — all other services depend on it) ---
        if let Some(storage) = self.storage_manager() {
            if let Err(e) = storage.initialize() {
                tracing::error!(error = %e, "Failed to initialize StorageManager");
                errors.push(format!("StorageManager: {}", e));
            }
        }

        // --- WorkerPool (captures and executors dispatch jobs through it) ---
        if let Some(pool) = self.worker_pool() {
            if let Err(e) = pool.initialize() {
                tracing::error!(error = %e, "Failed to initialize WorkerPool");
                errors.push(format!("WorkerPool: {}", e));
            }
        }

        // --- PipelineExecutor (bridges workers to the processing pipeline) ---
        if let Some(executor) = self.pipeline_executor() {
            if let Err(e) = executor.initialize() {
                tracing::error!(error = %e, "Failed to initialize PipelineExecutor");
                errors.push(format!("PipelineExecutor: {}", e));
            }
        }

        // --- CaptureEngine (entry point for all captured content) ---
        if let Some(engine) = self.capture_engine() {
            if let Err(e) = engine.initialize() {
                tracing::error!(error = %e, "Failed to initialize CaptureEngine");
                errors.push(format!("CaptureEngine: {}", e));
            }
        }

        // --- Indexer (full-text search, subscribes to ITEM_STORED) ---
        if let Some(indexer) = self.indexer() {
            if let Ok(idx) = indexer.lock() {
                if let Err(e) = idx.initialize() {
                    tracing::error!(error = %e, "Failed to initialize Indexer");
                    errors.push(format!("Indexer: {}", e));
                }
            }
        }

        // --- VaultGraph (semantic relationships, subscribes to ITEM_STORED) ---
        if let Some(graph) = self.vault_graph() {
            if let Ok(g) = graph.write() {
                if let Err(e) = g.initialize() {
                    tracing::error!(error = %e, "Failed to initialize VaultGraph");
                    errors.push(format!("VaultGraph: {}", e));
                }
            }
        }

        if !errors.is_empty() {
            return Err(errors);
        }

        self.lifecycle
            .transition_to(LifecycleStage::Initialized)
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Lifecycle transition to Initialized failed");
            });

        tracing::info!(
            stage = ?self.lifecycle_stage(),
            services = self.service_count(),
            capabilities = self.capability_registry().capability_count(),
            "Application context initialized"
        );

        Ok(())
    }

    /// Transitions the context to the `Running` stage.
    ///
    /// Starts all lifecycle-managed services in dependency-safe order:
    ///
    /// StorageManager → WorkerPool → PipelineExecutor → CaptureEngine → Indexer → VaultGraph
    ///
    /// If any service fails to start, already-started services are shut
    /// down gracefully via [`shutdown`](Self::shutdown) to avoid leaving
    /// the application in a partially started state.
    ///
    /// # Errors
    ///
    /// Returns a list of error descriptions for any service that failed
    /// to start. On failure, [`shutdown`](Self::shutdown) is called
    /// automatically to roll back progress.
    pub fn start(&self) -> Result<(), Vec<String>> {
        tracing::info!("Starting lifecycle services");

        let mut errors: Vec<String> = Vec::new();

        // Start in dependency-safe order. If a service fails, we stop
        // starting subsequent services and roll back already-started ones.
        // StorageManager → WorkerPool → PipelineExecutor → CaptureEngine → Indexer → VaultGraph

        // --- StorageManager ---
        if let Some(storage) = self.storage_manager() {
            tracing::info!("Starting StorageManager");
            if let Err(e) = storage.start() {
                tracing::error!(error = %e, "Failed to start StorageManager");
                errors.push(format!("StorageManager: {}", e));
            }
        }

        // --- WorkerPool ---
        if errors.is_empty() {
            if let Some(pool) = self.worker_pool() {
                tracing::info!("Starting WorkerPool");
                if let Err(e) = pool.start() {
                    tracing::error!(error = %e, "Failed to start WorkerPool");
                    errors.push(format!("WorkerPool: {}", e));
                }
            }
        }

        // --- PipelineExecutor ---
        if errors.is_empty() {
            if let Some(executor) = self.pipeline_executor() {
                tracing::info!("Starting PipelineExecutor");
                if let Err(e) = executor.start() {
                    tracing::error!(error = %e, "Failed to start PipelineExecutor");
                    errors.push(format!("PipelineExecutor: {}", e));
                }
            }
        }

        // --- CaptureEngine ---
        if errors.is_empty() {
            if let Some(engine) = self.capture_engine() {
                tracing::info!("Starting CaptureEngine");
                if let Err(e) = engine.start() {
                    tracing::error!(error = %e, "Failed to start CaptureEngine");
                    errors.push(format!("CaptureEngine: {}", e));
                }
            }
        }

        // --- Indexer ---
        if errors.is_empty() {
            if let Some(indexer) = self.indexer() {
                if let Ok(idx) = indexer.lock() {
                    tracing::info!("Starting Indexer");
                    if let Err(e) = idx.start() {
                        tracing::error!(error = %e, "Failed to start Indexer");
                        errors.push(format!("Indexer: {}", e));
                    }
                }
            }
        }

        // --- VaultGraph ---
        if errors.is_empty() {
            if let Some(graph) = self.vault_graph() {
                if let Ok(g) = graph.write() {
                    tracing::info!("Starting VaultGraph");
                    if let Err(e) = g.start() {
                        tracing::error!(error = %e, "Failed to start VaultGraph");
                        errors.push(format!("VaultGraph: {}", e));
                    }
                }
            }
        }

        if !errors.is_empty() {
            tracing::error!("Startup failed — rolling back started services");
            let _ = self.shutdown();
            return Err(errors);
        }

        self.lifecycle
            .transition_to(LifecycleStage::Running)
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "Lifecycle transition to Running failed");
            });

        tracing::info!(
            stage = ?self.lifecycle_stage(),
            "ApplicationContext ready — startup complete"
        );

        Ok(())
    }

    /// Transitions the context to the `Shutdown` stage.
    ///
    /// Shuts down all lifecycle-managed services in reverse dependency order:
    ///
    /// VaultGraph → Indexer → CaptureEngine → PipelineExecutor → WorkerPool → StorageManager
    pub fn shutdown(&self) -> Result<(), LifecycleError> {
        // Shut down in reverse dependency order so that consumers stop
        // before their providers.
        // VaultGraph → Indexer → CaptureEngine → PipelineExecutor → WorkerPool → StorageManager

        if let Some(graph) = self.vault_graph() {
            if let Ok(g) = graph.write() {
                tracing::info!("VaultGraph shutting down");
                if let Err(e) = g.shutdown() {
                    tracing::error!(error = %e, "Failed to shut down VaultGraph");
                }
            }
        }

        if let Some(indexer) = self.indexer() {
            if let Ok(idx) = indexer.lock() {
                tracing::info!("Indexer shutting down");
                if let Err(e) = idx.shutdown() {
                    tracing::error!(error = %e, "Failed to shut down Indexer");
                }
            }
        }

        if let Some(engine) = self.capture_engine() {
            tracing::info!("CaptureEngine shutting down");
            if let Err(e) = engine.shutdown() {
                tracing::error!(error = %e, "Failed to shut down CaptureEngine");
            }
        }

        if let Some(executor) = self.pipeline_executor() {
            tracing::info!("PipelineExecutor shutting down");
            if let Err(e) = executor.shutdown() {
                tracing::error!(error = %e, "Failed to shut down PipelineExecutor");
            }
        }

        if let Some(pool) = self.worker_pool() {
            tracing::info!("WorkerPool shutting down");
            if let Err(e) = pool.shutdown() {
                tracing::error!(error = %e, "Failed to shut down WorkerPool");
            }
        }

        if let Some(storage) = self.storage_manager() {
            tracing::info!("StorageManager shutting down");
            if let Err(e) = storage.shutdown() {
                tracing::error!(error = %e, "Failed to shut down StorageManager");
            }
        }

        let result = self.lifecycle.transition_to(LifecycleStage::Shutdown);
        match &result {
            Ok(()) => {
                tracing::info!(stage = ?self.lifecycle_stage(), "Application context shut down");
            }
            Err(e) => {
                tracing::error!(error = %e, "Lifecycle shutdown failed");
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// ApplicationContextBuilder
// ---------------------------------------------------------------------------

/// Builder for constructing an [`ApplicationContext`].
pub struct ApplicationContextBuilder {
    event_bus: Option<Arc<EventBus<PipelineEvent>>>,
    registry: Option<Arc<RwLock<ServiceRegistry>>>,
    capability_registry: Option<CapabilityRegistry>,
}

impl ApplicationContextBuilder {
    /// Creates a new builder with default values.
    pub fn new() -> Self {
        Self {
            event_bus: None,
            registry: None,
            capability_registry: None,
        }
    }

    /// Sets the event bus for the context.
    pub fn with_event_bus(mut self, bus: Arc<EventBus<PipelineEvent>>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    /// Sets the service registry for the context.
    pub fn with_registry(mut self, registry: Arc<RwLock<ServiceRegistry>>) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Sets the capability registry for the context.
    pub fn with_capability_registry(mut self, cr: CapabilityRegistry) -> Self {
        self.capability_registry = Some(cr);
        self
    }

    /// Builds the [`ApplicationContext`].
    pub fn build(self) -> ApplicationContext {
        let event_bus = self.event_bus.unwrap_or_else(|| Arc::new(EventBus::new()));
        let registry = self
            .registry
            .unwrap_or_else(|| Arc::new(RwLock::new(ServiceRegistry::new())));
        let capability_registry = self.capability_registry.unwrap_or_default();

        // Register the event bus in the registry for discoverability
        {
            let mut reg = registry.write().expect("registry lock");
            reg.register("event_bus", event_bus.clone());
        }

        ApplicationContext::new(registry, event_bus, capability_registry)
    }
}

impl Default for ApplicationContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Standard context builder — used by the Tauri app entry point
// ---------------------------------------------------------------------------

/// Builds a standard `ApplicationContext` with Nabu's built-in services
/// registered, capabilities registered, and ready for start.
///
/// This is the primary factory method used by the Tauri application. It
/// produces a context with the event bus, processing pipeline, capture
/// engine, job queue, and all standard handlers/processors registered.
///
/// Callers should attach additional services (storage, indexer, graph, etc.)
/// after calling this method, then call [`ApplicationContext::initialize`]
/// and [`ApplicationContext::start`].
pub fn build_standard_application_context() -> ApplicationContext {
    let event_bus = Arc::new(EventBus::new());
    let registry = Arc::new(RwLock::new(ServiceRegistry::new()));
    let mut capability_registry = CapabilityRegistry::new();

    // Register built-in capabilities
    capability_registry.register_builtin();

    let ctx = ApplicationContext::new(registry, event_bus, capability_registry);

    tracing::info!(
        capabilities = ctx.capability_registry().capability_count(),
        "Built-in capabilities registered"
    );

    ctx
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::EventBus;

    #[test]
    fn builder_creates_default_context() {
        let ctx = ApplicationContext::builder().build();
        assert!(!ctx.is_initialized());
        assert!(!ctx.is_running());
        assert!(!ctx.is_shutdown());
        assert!(ctx.service_count() > 0); // event_bus is auto-registered
    }

    #[test]
    fn lifecycle_transitions() {
        let ctx = ApplicationContext::builder().build();
        assert_eq!(ctx.lifecycle_stage(), LifecycleStage::Created);

        // Register required services so initialization succeeds
        ctx.register(
            "capture_engine",
            Arc::new(crate::capture::CaptureEngine::new()),
        );
        ctx.register(
            "pipeline",
            Arc::new(crate::processing::ProcessingPipeline::new()),
        );

        // StorageManager requires a vault path, so use a mock
        use crate::storage::StorageManager;
        let temp_dir = std::env::temp_dir().join("nabu-test-context");
        let _ = std::fs::create_dir_all(&temp_dir);
        let sm = Arc::new(StorageManager::new(temp_dir.clone()));
        ctx.register("storage_manager", sm);
        let _ = std::fs::remove_dir_all(&temp_dir);

        assert!(ctx.initialize().is_ok());
        assert!(ctx.is_initialized());
        assert!(!ctx.is_running());

        assert!(ctx.start().is_ok());
        assert!(ctx.is_running());
        assert!(ctx.is_initialized());

        ctx.shutdown().unwrap();
        assert!(ctx.is_shutdown());
        assert!(!ctx.is_running());
    }

    #[test]
    fn register_and_resolve_through_context() {
        let ctx = ApplicationContext::builder().build();

        struct MyService {
            value: i32,
        }

        ctx.register("my_svc", Arc::new(MyService { value: 42 }));

        let resolved: Option<Arc<MyService>> = ctx.resolve("my_svc");
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().value, 42);
    }

    #[test]
    fn resolve_nonexistent_returns_none() {
        let ctx = ApplicationContext::builder().build();
        let resolved: Option<Arc<String>> = ctx.resolve("nonexistent");
        assert!(resolved.is_none());
    }

    #[test]
    fn with_custom_event_bus() {
        let bus = Arc::new(EventBus::new());
        let ctx = ApplicationContext::builder()
            .with_event_bus(bus.clone())
            .build();

        assert!(Arc::ptr_eq(&ctx.event_bus, &bus));
    }

    #[test]
    fn health_check() {
        let ctx = ApplicationContext::builder().build();
        assert_eq!(ctx.check_health("event_bus"), ServiceHealth::Healthy);
        assert_eq!(ctx.check_health("nonexistent"), ServiceHealth::NotFound);
    }

    #[test]
    fn validation_detects_missing_services() {
        let ctx = ApplicationContext::builder().build();
        // The builder auto-registers `event_bus`, so include it in the
        // required list to verify it is reported as present.
        let report = ctx.validate_services(&["event_bus", "required_svc"], &["optional_svc"]);
        assert!(!report.is_valid());
        assert_eq!(report.missing.len(), 1);
        assert!(report.present.contains(&"event_bus"));
    }

    #[test]
    fn core_validation_missing() {
        let ctx = ApplicationContext::builder().build();
        let report = ctx.validate_core_services();
        // Only event_bus is auto-registered; capture_engine, pipeline,
        // storage_manager are all missing
        assert!(report.missing_count() > 0);
    }

    #[test]
    fn category_operations_through_context() {
        let ctx = ApplicationContext::builder().build();

        struct TestHandler;
        ctx.register("handler_a", Arc::new(TestHandler));
        ctx.register("handler_b", Arc::new(TestHandler));
        ctx.register_in_category("capture_handlers", "handler_a");
        ctx.register_in_category("capture_handlers", "handler_b");

        let handlers: Vec<Arc<TestHandler>> = ctx.resolve_category("capture_handlers");
        assert_eq!(handlers.len(), 2);
    }

    #[test]
    fn capability_registry_accessible() {
        let ctx = ApplicationContext::builder()
            .with_capability_registry(CapabilityRegistry::new())
            .build();

        assert!(ctx.capability_registry().capability_count() == 0);
    }

    #[test]
    fn standard_context_has_capabilities() {
        let ctx = build_standard_application_context();
        assert!(ctx.capability_registry().capability_count() > 0);
    }

    #[test]
    fn validation_report_summary() {
        let report = ValidationReport {
            required_services: vec!["a", "b"],
            optional_services: vec!["c"],
            present: vec!["a", "c"],
            missing: vec!["b"],
            unhealthy: vec![],
        };
        let summary = report.summary();
        // 2 of 3 known services (a, c present; b missing) are present.
        assert!(summary.contains("2/3"));
        assert!(summary.contains("Missing: b"));
    }
}
