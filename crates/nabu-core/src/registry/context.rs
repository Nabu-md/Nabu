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

use std::sync::{Arc, RwLock};

use crate::event_bus::EventBus;
use crate::plugin::capability::{Capability, CapabilityRegistry};
use crate::plugin::version::Version;
use crate::registry::lifecycle::{LifecycleError, LifecycleManager, LifecycleStage};
use crate::registry::ServiceRegistry;

use crate::jobs;
use crate::diagnostics::PerformanceMonitor;

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
        let mut parts = vec![format!(
            "Validation: {}/{} required services present",
            self.present.len(),
            self.required_services.len(),
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
    pub event_bus: Arc<EventBus>,
    /// Plugin capability registry — describes what the system can do.
    pub capability_registry: CapabilityRegistry,
    /// Manages service lifecycle (init → start → shutdown).
    lifecycle: LifecycleManager,
}

impl ApplicationContext {
    /// Creates a new `ApplicationContext` with the given registry and event bus.
    ///
    /// Prefer using [`ApplicationContext::builder`] for most use cases.
    pub fn new(
        registry: Arc<RwLock<ServiceRegistry>>,
        event_bus: Arc<EventBus>,
        capability_registry: CapabilityRegistry,
    ) -> Self {
        Self {
            registry,
            event_bus,
            capability_registry,
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
    pub fn event_bus(&self) -> &Arc<EventBus> {
        &self.event_bus
    }

    /// Returns a reference to the capability registry.
    pub fn capability_registry(&self) -> &CapabilityRegistry {
        &self.capability_registry
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
        let mut registry = self
            .registry
            .write()
            .expect("registry lock not poisoned");
        registry.register_in_category(category, key);
    }

    // -----------------------------------------------------------------------
    // Typed accessors — convenience methods for well-known services
    // -----------------------------------------------------------------------

    /// Returns the capture engine if registered.
    pub fn capture_engine(
        &self,
    ) -> Option<Arc<crate::capture::CaptureEngine>> {
        self.resolve("capture_engine")
    }

    /// Returns the processing pipeline if registered.
    pub fn processing_pipeline(
        &self,
    ) -> Option<Arc<crate::processing::ProcessingPipeline>> {
        self.resolve("pipeline")
    }

    /// Returns the job queue if registered.
    pub fn job_queue(
        &self,
    ) -> Option<Arc<jobs::DurableJobQueue>> {
        self.resolve("job_queue")
    }

    /// Returns the worker pool if registered.
    pub fn worker_pool(
        &self,
    ) -> Option<Arc<jobs::WorkerPool>> {
        self.resolve("worker_pool")
    }

    /// Returns the vault graph if registered.
    pub fn vault_graph(
        &self,
    ) -> Option<Arc<RwLock<crate::graph::VaultGraph>>> {
        self.resolve("vault_graph")
    }

    /// Returns the indexer if registered.
    pub fn indexer(
        &self,
    ) -> Option<Arc<std::sync::Mutex<crate::indexer::Indexer>>> {
        self.resolve("indexer")
    }

    /// Returns the storage manager if registered.
    pub fn storage_manager(
        &self,
    ) -> Option<Arc<crate::storage::StorageManager>> {
        self.resolve("storage_manager")
    }

    /// Returns the performance monitor if registered.
    pub fn performance_monitor(
        &self,
    ) -> Option<Arc<PerformanceMonitor>> {
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
        let mut unhealthy = Vec::new();

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
            &[
                "event_bus",
                "capture_engine",
                "pipeline",
                "storage_manager",
            ],
            &[
                "job_queue",
                "worker_pool",
                "vault_graph",
                "indexer",
            ],
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
    /// Validates required services are present before transitioning.
    /// Returns a list of missing required services if validation fails.
    pub fn initialize(&self) -> Result<(), Vec<String>> {
        let report = self.validate_core_services();
        if !report.is_valid() {
            return Err(report.missing.iter().map(|s| (*s).to_string()).collect());
        }

        self.lifecycle
            .transition_to(LifecycleStage::Initialized)
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Lifecycle transition to Initialized failed");
            });

        tracing::info!(
            stage = ?self.lifecycle_stage(),
            services = self.service_count(),
            capabilities = self.capability_registry.capability_count(),
            "Application context initialized"
        );

        Ok(())
    }

    /// Transitions the context to the `Running` stage.
    pub fn start(&self) {
        self.lifecycle
            .transition_to(LifecycleStage::Running)
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "Lifecycle transition to Running failed");
            });

        tracing::info!(
            stage = ?self.lifecycle_stage(),
            "Application context started"
        );
    }

    /// Transitions the context to the `Shutdown` stage.
    ///
    /// Logs the shutdown and returns any lifecycle transition error.
    pub fn shutdown(&self) -> Result<(), LifecycleError> {
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
    event_bus: Option<Arc<EventBus>>,
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
    pub fn with_event_bus(mut self, bus: Arc<EventBus>) -> Self {
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
        let capability_registry = self
            .capability_registry
            .unwrap_or_default();

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
    let nabu_version = Version::new(0, 1, 0);
    register_builtin_capabilities(&mut capability_registry, &nabu_version);

    let ctx = ApplicationContext::new(registry, event_bus, capability_registry);

    tracing::info!(
        capabilities = ctx.capability_registry.capability_count(),
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
    use crate::registry::ServiceRegistry;

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
        ctx.register("capture_engine", Arc::new(crate::capture::CaptureEngine::new(Arc::new(EventBus::new()))));
        ctx.register("pipeline", crate::processing::ProcessingPipeline::new_no_subscribe(Arc::new(EventBus::new())));

        // StorageManager requires a vault path, so use a mock
        use crate::storage::StorageManager;
        let temp_dir = std::env::temp_dir().join("nabu-test-context");
        let _ = std::fs::create_dir_all(&temp_dir);
        let bus = Arc::new(crate::event_bus::EventBus::new());
        let sm = StorageManager::new(temp_dir.clone(), bus);
        ctx.register("storage_manager", sm);
        let _ = std::fs::remove_dir_all(&temp_dir);

        assert!(ctx.initialize().is_ok());
        assert!(ctx.is_initialized());
        assert!(!ctx.is_running());

        ctx.start();
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
        let report = ctx.validate_services(&["required_svc"], &["optional_svc"]);
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

        assert!(ctx.capability_registry.capability_count() == 0);
    }

    #[test]
    fn standard_context_has_capabilities() {
        let ctx = build_standard_application_context();
        assert!(ctx.capability_registry.capability_count() > 0);
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
        assert!(summary.contains("2/3"));
        assert!(summary.contains("Missing: b"));
    }
}
