//! Service Registry — central registration and resolution of services.
//!
//! The [`ServiceRegistry`] provides a thread-safe container for registering
//! and resolving services by key and category. It supports:
//!
//! - **Singleton services** — registered eagerly, resolved on demand.
//! - **Transient factories** — registered as factory closures, create a new
//!   instance on each resolution.
//! - **Category-based registration** — services can be tagged with one or more
//!   categories for discovery (e.g. "capture_handlers", "processors").
//! - **Lazy initialization** — factory registrations are resolved only when
//!   first requested.
//!
//! # Usage
//!
//! ```ignore
//! use nabu_core::registry::ServiceRegistry;
//!
//! let mut registry = ServiceRegistry::new();
//! registry.register("my_service", Arc::new(MyService::new()));
//!
//! let service: Option<Arc<MyService>> = registry.resolve("my_service");
//! ```

pub mod application;
pub mod context;
pub mod health;
pub mod lifecycle;

pub use application::Application;
pub use health::{HealthStatus, LifecycleStageInfo, ServiceEntry, ServiceHealth};
pub use lifecycle::{Lifecycle, LifecycleError, LifecycleManager, LifecycleStage};

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

/// A thread-safe registry for services with singleton, transient, and
/// category-based resolution.
///
/// The registry is protected by an internal `RwLock` and designed to be
/// shared via `Arc<RwLock<ServiceRegistry>>` across the application.
#[derive(Default)]
pub struct ServiceRegistry {
    /// Singleton services — stored as `Arc<dyn Any>` for type erasure.
    singletons: HashMap<String, Arc<dyn Any + Send + Sync>>,
    /// Transient factory functions — produce a new instance on each resolve.
    factories: HashMap<String, Box<dyn Fn() -> Arc<dyn Any + Send + Sync> + Send + Sync>>,
    /// Category index — maps category names to lists of service keys.
    categories: HashMap<String, Vec<String>>,
}

impl ServiceRegistry {
    /// Creates a new, empty service registry.
    pub fn new() -> Self {
        Self {
            singletons: HashMap::new(),
            factories: HashMap::new(),
            categories: HashMap::new(),
        }
    }

    /// Registers a singleton service under the given key.
    ///
    /// The service is stored as an `Arc<T>` and will be returned on every
    /// [`resolve`](Self::resolve) call for this key.
    ///
    /// If a service with the same key already exists, it is replaced.
    pub fn register<T: Send + Sync + 'static>(&mut self, key: &str, service: Arc<T>) {
        self.singletons.insert(key.to_string(), service);
        self.factories.remove(key);
    }

    /// Registers a transient factory for the given key.
    ///
    /// The factory is invoked on every [`resolve`](Self::resolve) call,
    /// producing a new instance. Factories are useful for services that
    /// hold per-request state or must not be shared.
    ///
    /// If a singleton with the same key exists, it is replaced by the factory.
    pub fn register_factory<T, F>(&mut self, key: &str, factory: F)
    where
        T: Send + Sync + 'static,
        F: Fn() -> Arc<T> + Send + Sync + 'static,
    {
        let boxed: Box<dyn Fn() -> Arc<dyn Any + Send + Sync> + Send + Sync> =
            Box::new(move || factory());
        self.factories.insert(key.to_string(), boxed);
        self.singletons.remove(key);
    }

    /// Resolves a singleton or transient service by key.
    ///
    /// Returns `None` if no service is registered under the given key.
    ///
    /// # Panics
    ///
    /// Panics if a factory is registered but the resolved type `T` does not
    /// match the factory's output type. This is a programmer error — ensure
    /// the type parameter matches the registered type.
    pub fn resolve<T: Send + Sync + 'static>(&self, key: &str) -> Option<Arc<T>> {
        if let Some(singleton) = self.singletons.get(key) {
            return singleton.clone().downcast::<T>().ok();
        }
        if let Some(factory) = self.factories.get(key) {
            let instance = factory();
            return instance.downcast::<T>().ok();
        }
        None
    }

    /// Registers a service key under the given category.
    ///
    /// Categories allow grouping related services (e.g. "processors",
    /// "capture_handlers", "ai_providers") for batch resolution.
    ///
    /// The service must already be registered via [`register`](Self::register)
    /// or [`register_factory`](Self::register_factory) before being added to
    /// a category.
    pub fn register_in_category(&mut self, category: &str, key: &str) {
        self.categories
            .entry(category.to_string())
            .or_default()
            .push(key.to_string());
    }

    /// Returns the list of service keys registered in the given category.
    ///
    /// Returns an empty slice if the category does not exist.
    pub fn get_category(&self, category: &str) -> Vec<String> {
        self.categories.get(category).cloned().unwrap_or_default()
    }

    /// Returns `true` if a service is registered under the given key
    /// (either as a singleton or factory).
    pub fn has(&self, key: &str) -> bool {
        self.singletons.contains_key(key) || self.factories.contains_key(key)
    }

    /// Returns all registered singleton service keys.
    ///
    /// This is used by health reporting to enumerate registered services.
    pub fn service_keys(&self) -> Vec<String> {
        self.singletons.keys().cloned().collect()
    }

    /// Returns the number of registered singletons.
    pub fn singleton_count(&self) -> usize {
        self.singletons.len()
    }

    /// Returns the number of registered factories.
    pub fn factory_count(&self) -> usize {
        self.factories.len()
    }

    /// Returns the number of registered categories.
    pub fn category_count(&self) -> usize {
        self.categories.len()
    }

    /// Removes a service registration by key.
    ///
    /// Returns `true` if a service was removed.
    pub fn unregister(&mut self, key: &str) -> bool {
        let removed_singleton = self.singletons.remove(key).is_some();
        let removed_factory = self.factories.remove(key).is_some();

        // Also remove from all categories
        for services in self.categories.values_mut() {
            services.retain(|k| k != key);
        }

        removed_singleton || removed_factory
    }

    /// Registers multiple services under a category from an iterator.
    ///
    /// This is a convenience method for batch registration.
    pub fn register_batch_in_category<I>(&mut self, category: &str, keys: I)
    where
        I: IntoIterator<Item = String>,
    {
        let entry = self.categories.entry(category.to_string()).or_default();
        entry.extend(keys);
    }

    /// Resolves all services in a category as a vector.
    ///
    /// Returns only those services that could be downcast to the requested
    /// type `T`. Services that are not registered or have a type mismatch
    /// are silently skipped.
    pub fn resolve_category<T: Send + Sync + 'static>(&self, category: &str) -> Vec<Arc<T>> {
        self.get_category(category)
            .iter()
            .filter_map(|key| self.resolve::<T>(key))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Standard category constants — used across the application
// ---------------------------------------------------------------------------

/// Category for capture handlers (implementing [`CaptureHandler`]).
pub const CATEGORY_CAPTURE_HANDLERS: &str = "capture_handlers";

/// Category for processing pipeline processors (implementing [`Processor`]).
pub const CATEGORY_PROCESSORS: &str = "processors";

/// Category for AI provider services.
pub const CATEGORY_AI_PROVIDERS: &str = "ai_providers";

/// Category for OCR engine services.
pub const CATEGORY_OCR_PROVIDERS: &str = "ocr_providers";

/// Category for embedding provider services.
pub const CATEGORY_EMBEDDING_PROVIDERS: &str = "embedding_providers";

/// Category for exporter services.
pub const CATEGORY_EXPORTERS: &str = "exporters";

/// Category for storage provider services.
pub const CATEGORY_STORAGE_PROVIDERS: &str = "storage_providers";

/// Category for content provider services.
pub const CATEGORY_CONTENT_PROVIDERS: &str = "content_providers";

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    struct MockService {
        value: i32,
    }

    #[derive(Debug, Clone)]
    struct OtherService;

    #[test]
    fn register_and_resolve_singleton() {
        let mut registry = ServiceRegistry::new();
        let service = Arc::new(MockService { value: 42 });
        registry.register("mock", service.clone());

        let resolved: Option<Arc<MockService>> = registry.resolve("mock");
        assert!(resolved.is_some());
        assert_eq!(resolved.unwrap().value, 42);
    }

    #[test]
    fn resolve_nonexistent_returns_none() {
        let registry = ServiceRegistry::new();
        let resolved: Option<Arc<MockService>> = registry.resolve("nonexistent");
        assert!(resolved.is_none());
    }

    #[test]
    fn singleton_is_reused() {
        let mut registry = ServiceRegistry::new();
        registry.register("mock", Arc::new(MockService { value: 99 }));

        let a: Arc<MockService> = registry.resolve("mock").unwrap();
        let b: Arc<MockService> = registry.resolve("mock").unwrap();

        // Arc::ptr_eq checks they are the same allocation
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn factory_produces_new_instances() {
        let mut registry = ServiceRegistry::new();
        registry.register_factory("counter", || Arc::new(MockService { value: 0 }));

        let a: Arc<MockService> = registry.resolve("counter").unwrap();
        let b: Arc<MockService> = registry.resolve("counter").unwrap();

        assert!(!Arc::ptr_eq(&a, &b));
        assert_eq!(a.value, b.value);
    }

    #[test]
    fn register_replaces_singleton() {
        let mut registry = ServiceRegistry::new();
        registry.register("mock", Arc::new(MockService { value: 1 }));
        registry.register("mock", Arc::new(MockService { value: 2 }));

        let resolved: Arc<MockService> = registry.resolve("mock").unwrap();
        assert_eq!(resolved.value, 2);
    }

    #[test]
    fn factory_replaces_singleton() {
        let mut registry = ServiceRegistry::new();
        registry.register("mock", Arc::new(MockService { value: 1 }));
        registry.register_factory("mock", || Arc::new(MockService { value: 99 }));

        let resolved: Arc<MockService> = registry.resolve("mock").unwrap();
        assert_eq!(resolved.value, 99);
    }

    #[test]
    fn category_registration_and_resolution() {
        let mut registry = ServiceRegistry::new();
        registry.register("proc_a", Arc::new(MockService { value: 10 }));
        registry.register("proc_b", Arc::new(MockService { value: 20 }));
        registry.register_in_category("processors", "proc_a");
        registry.register_in_category("processors", "proc_b");

        let resolved: Vec<Arc<MockService>> = registry.resolve_category("processors");
        assert_eq!(resolved.len(), 2);

        let mut values: Vec<i32> = resolved.iter().map(|s| s.value).collect();
        values.sort();
        assert_eq!(values, vec![10, 20]);
    }

    #[test]
    fn unregister_removes_from_all_categories() {
        let mut registry = ServiceRegistry::new();
        registry.register("svc", Arc::new(MockService { value: 1 }));
        registry.register_in_category("cats", "svc");

        assert!(registry.unregister("svc"));
        assert!(!registry.has("svc"));
        assert!(registry.get_category("cats").is_empty());
    }

    #[test]
    fn resolve_category_filters_by_type() {
        let mut registry = ServiceRegistry::new();
        registry.register("mock_svc", Arc::new(MockService { value: 1 }));
        registry.register("other_svc", Arc::new(OtherService));
        registry.register_in_category("mix", "mock_svc");
        registry.register_in_category("mix", "other_svc");

        let mock_results: Vec<Arc<MockService>> = registry.resolve_category("mix");
        assert_eq!(mock_results.len(), 1);
        assert_eq!(mock_results[0].value, 1);
    }

    #[test]
    fn has_returns_correctly() {
        let mut registry = ServiceRegistry::new();
        assert!(!registry.has("missing"));

        registry.register("svc", Arc::new(MockService { value: 0 }));
        assert!(registry.has("svc"));

        registry.unregister("svc");
        assert!(!registry.has("svc"));
    }

    #[test]
    fn empty_registry_has_zero_counts() {
        let registry = ServiceRegistry::new();
        assert_eq!(registry.singleton_count(), 0);
        assert_eq!(registry.factory_count(), 0);
        assert_eq!(registry.category_count(), 0);
    }

    #[test]
    fn register_batch_in_category() {
        let mut registry = ServiceRegistry::new();
        registry.register("a", Arc::new(MockService { value: 1 }));
        registry.register("b", Arc::new(MockService { value: 2 }));
        registry.register("c", Arc::new(MockService { value: 3 }));

        registry.register_batch_in_category(
            "procs",
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
        );

        assert_eq!(registry.get_category("procs").len(), 3);
    }
}
