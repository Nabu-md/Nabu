use crate::event_bus::EventBus;
use crate::pipeline_migration::PipelineEvent;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

/// The ServiceRegistry is a dependency injection container.
///
/// All services in the platform are registered here by well-known string keys.
/// No service locator anti-pattern exists — services are explicitly registered
/// and retrieved through typed accessors.
pub struct ServiceRegistry {
    services: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    /// Register a service by key.
    pub fn register<T: Send + Sync + 'static>(&mut self, key: &str, service: T) {
        self.services.insert(key.to_string(), Box::new(service));
    }

    /// Register an Arc-wrapped service.
    pub fn register_arc<T: Send + Sync + 'static>(&mut self, key: &str, service: Arc<T>) {
        self.services.insert(key.to_string(), Box::new(service));
    }

    /// Get a service by key.
    pub fn get<T: Send + Sync + 'static>(&self, key: &str) -> Option<&T> {
        self.services
            .get(key)
            .and_then(|s| s.downcast_ref::<T>())
    }

    /// Get a service by key, returning an Arc.
    pub fn get_arc<T: Send + Sync + 'static>(&self, key: &str) -> Option<Arc<T>> {
        self.services
            .get(key)
            .and_then(|s| s.downcast_ref::<Arc<T>>())
            .cloned()
    }

    /// Check if a service is registered.
    pub fn has(&self, key: &str) -> bool {
        self.services.contains_key(key)
    }

    /// Remove a service by key.
    pub fn remove(&mut self, key: &str) {
        self.services.remove(key);
    }

    /// List all registered service keys.
    pub fn keys(&self) -> Vec<String> {
        self.services.keys().cloned().collect()
    }

    /// Number of registered services.
    pub fn count(&self) -> usize {
        self.services.len()
    }

    /// Validate that all required services are registered.
    pub fn validate_required(&self, required: &[&str]) -> Result<(), Vec<String>> {
        let missing: Vec<String> = required
            .iter()
            .filter(|key| !self.has(key))
            .map(|key| key.to_string())
            .collect();

        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Standard service keys used across the platform.
pub mod keys {
    pub const EVENT_BUS: &str = "event_bus";
    pub const CAPTURE_ENGINE: &str = "capture_engine";
    pub const PIPELINE: &str = "pipeline";
    pub const STORAGE_MANAGER: &str = "storage_manager";
    pub const JOB_QUEUE: &str = "job_queue";
    pub const WORKER_POOL: &str = "worker_pool";
    pub const INDEXER: &str = "indexer";
    pub const VAULT_GRAPH: &str = "vault_graph";
    pub const EXECUTOR_REGISTRY: &str = "executor_registry";
    pub const PIPELINE_EXECUTOR: &str = "pipeline_executor";
}

/// Standard service categories for grouping.
pub mod categories {
    pub const CAPTURE_HANDLERS: &str = "capture_handlers";
    pub const PROCESSORS: &str = "processors";
    pub const AI_PROVIDERS: &str = "ai_providers";
    pub const OCR_PROVIDERS: &str = "ocr_providers";
    pub const EMBEDDING_PROVIDERS: &str = "embedding_providers";
    pub const EXPORTERS: &str = "exporters";
    pub const STORAGE_PROVIDERS: &str = "storage_providers";
    pub const CONTENT_PROVIDERS: &str = "content_providers";
}

/// Required services that must be registered for platform startup.
pub const REQUIRED_SERVICES: &[&str] = &[
    keys::EVENT_BUS,
    keys::CAPTURE_ENGINE,
    keys::PIPELINE,
    keys::STORAGE_MANAGER,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_get() {
        let mut registry = ServiceRegistry::new();
        registry.register("test", 42u32);

        let value: &u32 = registry.get("test").unwrap();
        assert_eq!(*value, 42);
    }

    #[test]
    fn test_validate_required() {
        let mut registry = ServiceRegistry::new();
        registry.register("event_bus", "bus");

        let result = registry.validate_required(&["event_bus", "missing"]);
        assert!(result.is_err());

        let missing = result.unwrap_err();
        assert_eq!(missing, vec!["missing"]);
    }
}
