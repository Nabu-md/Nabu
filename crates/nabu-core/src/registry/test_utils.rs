//! Test utilities for the ServiceRegistry.
//!
//! Provides mock service factories and test registries for writing
//! unit and integration tests against the service registry.
//!
//! # Usage
//!
//! ```rust
//! use nabu_core::registry::test_utils::MockRegistryBuilder;
//!
//! let ctx = MockRegistryBuilder::new()
//!     .with_capture_engine()
//!     .with_storage_manager()
//!     .build();
//!
//! assert!(ctx.is_initialized());
//! assert!(ctx.capture_engine().is_some());
//! ```

use std::sync::{Arc, RwLock};

use crate::event_bus::EventBus;
use crate::diagnostics::PerformanceMonitor;
use crate::registry::context::ApplicationContext;
use crate::registry::ServiceRegistry;

/// A builder that creates an `ApplicationContext` pre-populated with
/// mock implementations of common services for testing.
///
/// By default, only the event bus is registered. Call `.with_*()` methods
/// to add mock services.
pub struct MockRegistryBuilder {
    event_bus: Option<Arc<EventBus>>,
    registry: Option<Arc<RwLock<ServiceRegistry>>>,
    with_capture: bool,
    with_storage: bool,
    with_pipeline: bool,
    with_queue: bool,
    with_pool: bool,
    with_graph: bool,
    with_indexer: bool,
    with_perf_monitor: bool,
    auto_initialize: bool,
}

impl MockRegistryBuilder {
    /// Create a new builder with default settings (event bus only).
    pub fn new() -> Self {
        Self {
            event_bus: None,
            registry: None,
            with_capture: false,
            with_storage: false,
            with_pipeline: false,
            with_queue: false,
            with_pool: false,
            with_graph: false,
            with_indexer: false,
            with_perf_monitor: false,
            auto_initialize: true,
        }
    }

    /// Register a mock capture engine.
    pub fn with_capture_engine(mut self) -> Self {
        self.with_capture = true;
        self
    }

    /// Register a mock storage manager.
    pub fn with_storage_manager(mut self) -> Self {
        self.with_storage = true;
        self
    }

    /// Register a mock processing pipeline.
    pub fn with_pipeline(mut self) -> Self {
        self.with_pipeline = true;
        self
    }

    /// Register a mock job queue.
    pub fn with_queue(mut self) -> Self {
        self.with_queue = true;
        self
    }

    /// Register a mock worker pool.
    pub fn with_pool(mut self) -> Self {
        self.with_pool = true;
        self
    }

    /// Register a mock vault graph.
    pub fn with_graph(mut self) -> Self {
        self.with_graph = true;
        self
    }

    /// Register a mock indexer.
    pub fn with_indexer(mut self) -> Self {
        self.with_indexer = true;
        self
    }

    /// Register a mock performance monitor.
    pub fn with_performance_monitor(mut self) -> Self {
        self.with_perf_monitor = true;
        self
    }

    /// Register all available mock services.
    pub fn with_all_services(mut self) -> Self {
        self.with_capture = true;
        self.with_storage = true;
        self.with_pipeline = true;
        self.with_queue = true;
        self.with_pool = true;
        self.with_graph = true;
        self.with_indexer = true;
        self.with_perf_monitor = true;
        self
    }

    /// Set whether to auto-initialize the context after registration.
    pub fn auto_initialize(mut self, auto: bool) -> Self {
        self.auto_initialize = auto;
        self
    }

    /// Build the context with the requested mock services.
    pub fn build(self) -> ApplicationContext {
        let event_bus = self.event_bus.unwrap_or_else(|| Arc::new(EventBus::new()));
        let registry = self.registry.unwrap_or_else(|| Arc::new(RwLock::new(ServiceRegistry::new())));

        // Register event bus
        {
            let mut reg = registry.write().unwrap();
            reg.register("event_bus", event_bus.clone());
        }

        let mut ctx = ApplicationContext::new(registry, event_bus, CapabilityRegistryPlaceholder::new_registry());

        // Register requested mock services
        if self.with_capture {
            ctx.register("capture_engine", Arc::new(MockCaptureEngine));
        }
        if self.with_storage {
            ctx.register("storage_manager", Arc::new(MockStorageManager));
        }
        if self.with_pipeline {
            ctx.register("pipeline", Arc::new(MockProcessingPipeline));
        }
        if self.with_queue {
            ctx.register("job_queue", Arc::new(MockJobQueue));
        }
        if self.with_pool {
            ctx.register("worker_pool", Arc::new(MockWorkerPool));
        }
        if self.with_graph {
            ctx.register("vault_graph", Arc::new(RwLock::new(MockVaultGraph)));
        }
        if self.with_indexer {
            ctx.register("indexer", Arc::new(std::sync::Mutex::new(MockIndexer)));
        }
        if self.with_perf_monitor {
            ctx.register("performance_monitor", Arc::new(PerformanceMonitor::new()));
        }

        // Auto-initialize if requested
        if self.auto_initialize {
            ctx.validate_core_services(); // validate, don't transition (may not have all services)
            // The mock tests should call initialize() explicitly if they want lifecycle transitions
        }

        ctx
    }
}

impl Default for MockRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Mock service implementations
// ---------------------------------------------------------------------------

/// Mock capture engine for testing.
pub struct MockCaptureEngine;
impl MockCaptureEngine {
    pub fn new() -> Self { Self }
}

/// Mock storage manager for testing.
pub struct MockStorageManager;
impl MockStorageManager {
    pub fn new() -> Self { Self }
}

/// Mock processing pipeline for testing.
pub struct MockProcessingPipeline;
impl MockProcessingPipeline {
    pub fn new() -> Self { Self }
}

/// Mock job queue for testing.
pub struct MockJobQueue;
impl MockJobQueue {
    pub fn new() -> Self { Self }
}

/// Mock worker pool for testing.
pub struct MockWorkerPool;
impl MockWorkerPool {
    pub fn new() -> Self { Self }
}

/// Mock vault graph for testing.
pub struct MockVaultGraph;

/// Mock indexer for testing.
pub struct MockIndexer;

// ---------------------------------------------------------------------------
// CapabilityRegistry placeholder (avoids dependency on plugin crate internals)
// ---------------------------------------------------------------------------

struct CapabilityRegistryPlaceholder;

impl CapabilityRegistryPlaceholder {
    fn new_registry() -> crate::plugin::capability::CapabilityRegistry {
        crate::plugin::capability::CapabilityRegistry::new()
    }
}

// ---------------------------------------------------------------------------
// Safety markers — mock types are Send + Sync where needed
// ---------------------------------------------------------------------------

unsafe impl Send for MockCaptureEngine {}
unsafe impl Sync for MockCaptureEngine {}
unsafe impl Send for MockStorageManager {}
unsafe impl Sync for MockStorageManager {}
unsafe impl Send for MockProcessingPipeline {}
unsafe impl Sync for MockProcessingPipeline {}
unsafe impl Send for MockJobQueue {}
unsafe impl Sync for MockJobQueue {}
unsafe impl Send for MockWorkerPool {}
unsafe impl Sync for MockWorkerPool {}
unsafe impl Send for MockVaultGraph {}
unsafe impl Sync for MockVaultGraph {}
unsafe impl Send for MockIndexer {}
unsafe impl Sync for MockIndexer {}
