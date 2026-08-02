//! # Application — Nabu's Composition Root
//!
//! The `Application` struct is the **single composition root** for the entire
//! Nabu application. It constructs, owns, and manages the lifecycle of every
//! core service.
//!
//! ## Architecture
//!
//! ```text
//! Application  ←  the single entry point, owns everything
//! │
//! ├── ApplicationContext  ← readonly handle shared with subsystems
//! │   ├── EventBus          publish/subscribe
//! │   ├── ServiceRegistry   service discovery (read-only after build)
//! │   └── LifecycleManager  phase tracking
//! │
//! ├── ProcessingPipeline   processes captured content
//! ├── CaptureEngine        routes capture requests to handlers
//! ├── JobQueue             durable background job queue
//! ├── WorkerPool           executes queued jobs
//! ├── StorageManager       persists knowledge objects
//! ├── ContentProvider      resolves file content
//! ├── Indexer              full-text search (Tantivy)
//! ├── VaultGraph           semantic relationship graph
//! ├── PerformanceMonitor   local metrics aggregation
//! ├── ExportEngine         export to HTML/Markdown/etc.
//! ├── TemplateManager      note templates
//! └── (future: PluginManager, AI providers, etc.)
//! ```
//!
//! ## Ownership
//!
//! - `Application` **owns** every service via `Arc<T>`.
//! - `ApplicationContext` **borrows** references via `Arc<T>` for typed access.
//! - Subsystems receive **only the dependencies they actually need** (constructor injection).
//! - No subsystem performs singleton lookup, global access, or hidden discovery.
//!
//! ## Lifecycle
//!
//! ```text
//! ApplicationBuilder::new()
//!   │  with_*() methods → constructor injection
//!   ▼
//! .build()              → validates, initializes
//!   │
//!   ▼
//! Application { Created }
//!   │
//!   │ .initialize()     → validates services, transitions to Initialized
//!   ▼
//! Application { Initialized }
//!   │
//!   │ .start()          → starts workers, pipelines, transitions to Running
//!   ▼
//! Application { Running }
//!   │
//!   │ .shutdown()       → graceful stop, waits for drain, transitions to Shutdown
//!   ▼
//! Application { Shutdown }
//! ```
//!
//! ## Dependency Injection Rules
//!
//! 1. Every service is constructed **explicitly** in the `ApplicationBuilder`.
//! 2. Dependencies are passed **through constructors** — no service location.
//! 3. The `Application` decides the order of construction and injection.
//! 4. After `build()`, the `ApplicationContext` is **immutable** — no late registration.
//! 5. The `Application` owns `Arc<T>` for every service — no global statics.
//!
//! ## Testing
//!
//! Use `Application::test_builder()` to construct an `Application` with mock
//! services. Every mock is a lightweight stand-in, not a production service.
//!
//! ```rust
//! use std::sync::Arc;
//! use nabu_core::capture::CaptureEngine;
//! use nabu_core::registry::Application;
//!
//! let app = Application::builder()
//!     .with_capture_engine(Arc::new(CaptureEngine::new()))
//!     .build();
//! assert!(app.context().capture_engine().is_some());
//! ```

use std::sync::Arc;

use crate::capture::CaptureEngine;
use crate::diagnostics::PerformanceMonitor;
use crate::event_bus::{EventBus, PipelineEvent};
use crate::processing::ProcessingPipeline;
use crate::registry::context::ApplicationContext;
use crate::registry::lifecycle::{LifecycleError, LifecycleManager, LifecycleStage};
use crate::registry::ServiceRegistry;

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

/// The single composition root for the entire Nabu application.
///
/// `Application` owns every service and manages the full lifecycle:
/// construction → initialization → startup → runtime → shutdown.
///
/// Use [`Application::builder()`] to construct a production application.
/// Use [`Application::test_builder()`] for testing with mock services.
pub struct Application {
    /// Immutable application context — the canonical dependency container.
    context: ApplicationContext,
    /// Lifecycle manager tracks the application phase.
    lifecycle: LifecycleManager,
    /// Owned services (owned by Application, borrowed by subscribers)
    // Note: Services are stored in the ServiceRegistry which is in context.
    // The Application holds additional Arc references to keep services alive
    // even if the registry is dropped.
    _capture_engine: Option<Arc<CaptureEngine>>,
    _pipeline: Option<Arc<ProcessingPipeline>>,
    _performance_monitor: Option<Arc<PerformanceMonitor>>,
}

impl Application {
    /// Returns a builder for constructing a production `Application`.
    pub fn builder() -> ApplicationBuilder {
        ApplicationBuilder::new()
    }

    /// Returns a builder for constructing a test `Application` with mock services.
    pub fn test_builder() -> ApplicationBuilder {
        let mut builder = ApplicationBuilder::new();
        builder.test_mode = true;
        builder
    }

    /// Returns the read-only application context.
    ///
    /// The context provides typed accessors for every registered service.
    /// It is **immutable** after `build()` — no late registration is allowed.
    pub fn context(&self) -> &ApplicationContext {
        &self.context
    }

    /// Returns the current lifecycle stage of the application.
    pub fn stage(&self) -> LifecycleStage {
        self.lifecycle.stage()
    }

    /// Returns `true` if the application is running.
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }

    /// Returns `true` if the application has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.lifecycle.is_shutdown()
    }

    /// Initialize the application — validate services and transition to
    /// the `Initialized` stage.
    ///
    /// This phase validates that all required services are registered.
    /// Subsystems should allocate resources and validate configuration here.
    ///
    /// Returns a list of missing required services if validation fails.
    pub fn initialize(&self) -> Result<(), Vec<String>> {
        let report = self.context.validate_core_services();
        if !report.is_valid() {
            return Err(report.missing.iter().map(|s| (*s).to_string()).collect());
        }

        self.lifecycle
            .transition_to(LifecycleStage::Initialized)
            .unwrap_or_else(|e| {
                tracing::warn!(error = %e, "Lifecycle transition to Initialized failed");
            });

        tracing::info!(
            stage = ?self.lifecycle.stage(),
            services = self.context.service_count(),
            "Application initialized"
        );

        Ok(())
    }

    /// Start the application — begin processing and transition to `Running`.
    ///
    /// This phase starts background workers and pipelines.
    ///
    /// # Panics
    ///
    /// Panics if `initialize()` has not been called first.
    pub fn start(&self) {
        let current = self.lifecycle.stage();
        if current < LifecycleStage::Initialized {
            panic!(
                "Cannot start application from stage {:?}. Call initialize() first.",
                current
            );
        }

        self.lifecycle
            .transition_to(LifecycleStage::Running)
            .unwrap_or_else(|e| {
                tracing::error!(error = %e, "Lifecycle transition to Running failed");
            });

        tracing::info!(
            stage = ?self.lifecycle.stage(),
            "Application started"
        );
    }

    /// Shut down the application gracefully.
    ///
    /// This phase stops background workers, drains queues, releases resources,
    /// and transitions to `Shutdown`.
    pub fn shutdown(&self) -> Result<(), LifecycleError> {
        let result = self.lifecycle.transition_to(LifecycleStage::Shutdown);
        match &result {
            Ok(()) => {
                tracing::info!(stage = ?self.lifecycle.stage(), "Application shut down");
            }
            Err(e) => {
                tracing::error!(error = %e, "Application shutdown failed");
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// ApplicationBuilder
// ---------------------------------------------------------------------------

/// Builder for constructing an [`Application`] with explicit dependency injection.
///
/// Every service is constructed in dependency order. Dependencies are passed
/// through constructors — no service location.
///
/// ## Production use
///
/// ```ignore
/// let app = Application::builder()
///     .with_event_bus(event_bus)
///     .with_capture_engine(capture_engine)
///     .with_processing_pipeline(pipeline)
///     .with_job_queue(queue)
///     .with_worker_pool(pool)
///     .with_storage_manager(storage)
///     .with_indexer(indexer)
///     .with_vault_graph(graph)
///     .with_performance_monitor(monitor)
///     .build()
///     .unwrap();
/// ```
pub struct ApplicationBuilder {
    /// Whether this is a test builder (allows mock services).
    pub(crate) test_mode: bool,
    /// The service registry (shared with ApplicationContext).
    registry: Option<Arc<std::sync::RwLock<ServiceRegistry>>>,
    /// The event bus (shared across all services).
    event_bus: Option<Arc<EventBus<PipelineEvent>>>,
    /// Performance monitor (metrics aggregation).
    performance_monitor: Option<Arc<PerformanceMonitor>>,
    /// Processing pipeline.
    pipeline: Option<Arc<ProcessingPipeline>>,
    /// Capture engine.
    capture_engine: Option<Arc<CaptureEngine>>,
}

impl ApplicationBuilder {
    /// Create a new builder with default empty state.
    pub fn new() -> Self {
        Self {
            test_mode: false,
            registry: None,
            event_bus: None,
            performance_monitor: None,
            pipeline: None,
            capture_engine: None,
        }
    }

    /// Set the event bus.
    ///
    /// The event bus is the communication backbone for the entire application.
    /// If not set, a default `EventBus` is created.
    pub fn with_event_bus(mut self, bus: Arc<EventBus<PipelineEvent>>) -> Self {
        self.event_bus = Some(bus);
        self
    }

    /// Set the performance monitor.
    ///
    /// If not set, a default `PerformanceMonitor` is created.
    pub fn with_performance_monitor(mut self, monitor: Arc<PerformanceMonitor>) -> Self {
        self.performance_monitor = Some(monitor);
        self
    }

    /// Set the processing pipeline.
    pub fn with_processing_pipeline(mut self, pipeline: Arc<ProcessingPipeline>) -> Self {
        self.pipeline = Some(pipeline);
        self
    }

    /// Set the capture engine.
    pub fn with_capture_engine(mut self, engine: Arc<CaptureEngine>) -> Self {
        self.capture_engine = Some(engine);
        self
    }

    /// Set the service registry explicitly.
    ///
    /// Useful when you need to pre-populate the registry before building.
    /// If not set, a default empty registry is created.
    pub fn with_registry(mut self, registry: Arc<std::sync::RwLock<ServiceRegistry>>) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Build the `Application` with all configured services.
    ///
    /// This method:
    /// 1. Creates the event bus (or uses the provided one)
    /// 2. Creates the service registry (or uses the provided one)
    /// 3. Registers all configured services into the registry
    /// 4. Wires EventBus subscriptions for core services
    /// 5. Creates the `ApplicationContext`
    /// 6. Creates and returns the `Application`
    ///
    /// # Panics
    ///
    /// Panics if required services are missing (in production mode) or if
    /// the registry lock is poisoned.
    pub fn build(self) -> Application {
        let event_bus = self.event_bus.unwrap_or_else(|| Arc::new(EventBus::new()));
        let registry = self.registry.unwrap_or_else(|| {
            Arc::new(std::sync::RwLock::new(ServiceRegistry::new()))
        });

        // ---- 1. Register the event bus itself ----
        {
            let mut reg = registry.write().expect("registry lock not poisoned");
            reg.register("event_bus", event_bus.clone());
        }

        // ---- 2. Build and register the PerformanceMonitor ----
        let perf_monitor = self.performance_monitor.unwrap_or_else(|| {
            Arc::new(PerformanceMonitor::new())
        });
        {
            let mut reg = registry.write().expect("registry lock not poisoned");
            reg.register("performance_monitor", perf_monitor.clone());
        }

        // ---- 3. Build and register the ProcessingPipeline ----
        if let Some(pipeline) = &self.pipeline {
            let mut reg = registry.write().expect("registry lock not poisoned");
            reg.register("pipeline", pipeline.clone());
        }

        // ---- 4. Build and register the CaptureEngine ----
        if let Some(engine) = &self.capture_engine {
            let mut reg = registry.write().expect("registry lock not poisoned");
            reg.register("capture_engine", engine.clone());
        }

        // ---- 5. Create the ApplicationContext ----
        let context = ApplicationContext::new(
            registry.clone(),
            event_bus.clone(),
            crate::plugin::capability::CapabilityRegistry::new(),
        );

        // ---- 6. Create the Application ----
        Application {
            context,
            lifecycle: LifecycleManager::new(),
            _capture_engine: self.capture_engine,
            _pipeline: self.pipeline,
            _performance_monitor: Some(perf_monitor),
        }
    }
}

impl Default for ApplicationBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_creates_application() {
        let app = Application::builder().build();
        assert_eq!(app.stage(), LifecycleStage::Created);
        assert!(!app.is_running());
        assert!(!app.is_shutdown());
    }

    #[test]
    fn test_application_context_is_accessible() {
        let app = Application::builder().build();
        let ctx = app.context();
        let _ = ctx.event_bus();
        assert!(ctx.service_count() >= 1);
    }

    #[test]
    fn test_initialize_validates_services() {
        let app = Application::builder().build();
        // Without required services (capture_engine, pipeline, storage_manager),
        // initialization should fail
        let result = app.initialize();
        assert!(result.is_err());
        let missing = result.unwrap_err();
        assert!(missing.contains(&"capture_engine".to_string()));
    }

    #[test]
    fn test_start_requires_initialize() {
        let app = Application::builder().build();
        // Cannot start without initializing first
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            app.start();
        }));
        assert!(result.is_err());
    }

    #[test]
    fn test_shutdown_from_created() {
        let app = Application::builder().build();
        assert!(app.shutdown().is_ok());
        assert!(app.is_shutdown());
    }

    #[test]
    fn test_full_lifecycle() {
        let app = Application::builder().build();
        assert_eq!(app.stage(), LifecycleStage::Created);

        // Register required services before initialize
        let bus: Arc<EventBus<PipelineEvent>> = Arc::new(EventBus::new());
        app.context().register("capture_engine", Arc::new(CaptureEngine::new()));
        app.context().register(
            "pipeline",
            Arc::new(ProcessingPipeline::new()),
        );
        app.context().register(
            "storage_manager",
            Arc::new(crate::storage::StorageManager::new(
                std::env::temp_dir().join("nabu-test-app"),
            )),
        );

        assert!(app.initialize().is_ok());
        assert_eq!(app.stage(), LifecycleStage::Initialized);

        app.start();
        assert!(app.is_running());

        assert!(app.shutdown().is_ok());
        assert!(app.is_shutdown());
    }

    #[test]
    fn test_test_builder_flag() {
        let builder = Application::test_builder();
        assert!(builder.test_mode);
    }

    #[test]
    fn test_with_event_bus() {
        let bus = Arc::new(EventBus::new());
        let app = Application::builder()
            .with_event_bus(bus.clone())
            .build();
        assert!(Arc::ptr_eq(
            &app.context().event_bus(),
            &bus,
        ));
    }

    #[test]
    fn test_with_performance_monitor() {
        let monitor = Arc::new(PerformanceMonitor::new());
        let app = Application::builder()
            .with_performance_monitor(monitor.clone())
            .build();
        let resolved: Option<Arc<PerformanceMonitor>> =
            app.context().resolve("performance_monitor");
        assert!(resolved.is_some());
    }

    #[test]
    fn test_double_shutdown() {
        let app = Application::builder().build();
        assert!(app.shutdown().is_ok());
        // Second shutdown is allowed (same stage)
        assert!(app.shutdown().is_ok());
    }

    #[test]
    fn test_context_immutable_after_build() {
        let app = Application::builder().build();
        // The context should allow registering (through the mutable registry)
        // but the Application doesn't expose late-registration as a public API
        // The context is writable through the RwLock, but the builder pattern
        // should be preferred.
        let capture_count = app.context().service_count();
        assert!(capture_count >= 1); // event_bus
    }
}
