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
    /// Lifecycle-managed services — stored as `Arc<dyn Lifecycle + Send + Sync>`.
    ///
    /// These are services that implement the [`Lifecycle`] trait and should
    /// have their `initialize()`, `start()`, and `shutdown()` methods called
    /// during the corresponding application lifecycle phases.
    /// Keys are stored in registration order so shutdown can iterate in
    /// reverse (consumers before providers).
    lifecycle_services: Vec<String>,
    /// Back-reference to lifecycle services for calling trait methods.
    lifecycle_refs: HashMap<String, Arc<dyn Lifecycle + Send + Sync>>,
}

impl ServiceRegistry {
    /// Creates a new, empty service registry.
    pub fn new() -> Self {
        Self {
            singletons: HashMap::new(),
            factories: HashMap::new(),
            categories: HashMap::new(),
            lifecycle_services: Vec::new(),
            lifecycle_refs: HashMap::new(),
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

    /// Registers a singleton service that implements [`Lifecycle`].
    ///
    /// The service is stored both as a general singleton (for resolution via
    /// [`resolve`](Self::resolve)) and in a lifecycle-specific map so that
    /// its `initialize()`, `start()`, and `shutdown()` methods are called
    /// automatically during the corresponding application lifecycle phases.
    ///
    /// Services should be registered in dependency order (providers first,
    /// consumers later) so that shutdown — which iterates in reverse order —
    /// shuts down consumers before their providers.
    ///
    /// If a service with the same key already exists, it is replaced.
    pub fn register_lifecycle<T: Lifecycle + Send + Sync + 'static>(
        &mut self,
        key: &str,
        service: Arc<T>,
    ) {
        let key = key.to_string();
        self.singletons.insert(key.clone(), service.clone());
        self.factories.remove(&key);
        if !self.lifecycle_services.contains(&key) {
            self.lifecycle_services.push(key.clone());
        }
        self.lifecycle_refs
            .insert(key, service);
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

        // Remove from lifecycle tracking
        self.lifecycle_services.retain(|k| k != key);
        self.lifecycle_refs.remove(key);

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

    // -----------------------------------------------------------------------
    // Lifecycle-managed service registry
    // -----------------------------------------------------------------------

    /// Returns the keys of all registered lifecycle-managed services, in
    /// registration order (providers first, consumers later).
    pub fn lifecycle_service_keys(&self) -> Vec<String> {
        self.lifecycle_services.clone()
    }

    /// Returns the number of registered lifecycle-managed services.
    pub fn lifecycle_service_count(&self) -> usize {
        self.lifecycle_services.len()
    }

    /// Returns `true` if a service is registered as lifecycle-managed under
    /// the given key.
    pub fn has_lifecycle_service(&self, key: &str) -> bool {
        self.lifecycle_services.iter().any(|k| k == key)
    }

    /// Shuts down all lifecycle-managed services in **reverse registration
    /// order** (consumers before providers).
    ///
    /// Each service's `shutdown()` method is called; errors are logged and
    /// collected in the returned vector. A failure in one service does not
    /// prevent subsequent services from being shut down.
    ///
    /// This is safe to call even if some services are not in the `Running`
    /// stage — the [`Lifecycle`] trait's default `shutdown()` is
    /// idempotent.
    pub fn shutdown_all_lifecycle_services(&self) -> Vec<String> {
        let mut errors: Vec<String> = Vec::new();

        for key in self.lifecycle_services.iter().rev() {
            if let Some(svc) = self.lifecycle_refs.get(key) {
                tracing::info!(
                    service = %key,
                    "Shutting down lifecycle-managed service"
                );
                if let Err(e) = svc.shutdown() {
                    let msg = format!("{}: {}", key, e);
                    tracing::error!(error = %msg, "Lifecycle shutdown failed for service");
                    errors.push(msg);
                }
            }
        }

        if !errors.is_empty() {
            tracing::warn!(
                count = errors.len(),
                "{} lifecycle service(s) failed to shut down cleanly",
                errors.len()
            );
        } else {
            tracing::info!("All lifecycle services shut down cleanly");
        }

        errors
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

    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A mock Lifecycle service that tracks shutdown calls.
    #[derive(Debug, Clone)]
    struct MockLifecycleService {
        name: &'static str,
        shutdown_count: Arc<AtomicUsize>,
    }

    impl MockLifecycleService {
        fn new(name: &'static str) -> Self {
            Self {
                name,
                shutdown_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn shutdown_count(&self) -> usize {
            self.shutdown_count.load(Ordering::SeqCst)
        }
    }

    impl Lifecycle for MockLifecycleService {
        fn name(&self) -> &'static str {
            self.name
        }

        fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
            self.shutdown_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

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

    #[test]
    fn register_lifecycle_tracks_service() {
        let mut registry = ServiceRegistry::new();
        let svc = Arc::new(MockLifecycleService::new("svc_a"));
        registry.register_lifecycle("svc_a", svc);

        assert!(registry.has("svc_a"));
        assert!(registry.has_lifecycle_service("svc_a"));
        assert_eq!(registry.lifecycle_service_count(), 1);
        assert_eq!(registry.lifecycle_service_keys(), vec!["svc_a".to_string()]);

        let resolved: Option<Arc<MockLifecycleService>> = registry.resolve("svc_a");
        assert!(resolved.is_some());
    }

    #[test]
    fn register_lifecycle_replaces_existing() {
        let mut registry = ServiceRegistry::new();
        registry.register_lifecycle("svc_a", Arc::new(MockLifecycleService::new("svc_a")));
        registry.register_lifecycle("svc_a", Arc::new(MockLifecycleService::new("svc_a_v2")));

        assert_eq!(registry.lifecycle_service_count(), 1);
        assert_eq!(registry.lifecycle_service_keys(), vec!["svc_a".to_string()]);
    }

    #[test]
    fn unregister_removes_lifecycle_service() {
        let mut registry = ServiceRegistry::new();
        registry.register_lifecycle("svc_a", Arc::new(MockLifecycleService::new("svc_a")));
        registry.register_lifecycle("svc_b", Arc::new(MockLifecycleService::new("svc_b")));

        assert_eq!(registry.lifecycle_service_count(), 2);
        assert!(registry.unregister("svc_a"));
        assert_eq!(registry.lifecycle_service_count(), 1);
        assert!(!registry.has_lifecycle_service("svc_a"));
        assert!(registry.has_lifecycle_service("svc_b"));
        assert!(!registry.has("svc_a"));
    }

    #[test]
    fn shutdown_all_lifecycle_services_calls_shutdown() {
        let mut registry = ServiceRegistry::new();
        let svc_a = Arc::new(MockLifecycleService::new("svc_a"));
        let svc_b = Arc::new(MockLifecycleService::new("svc_b"));
        let svc_c = Arc::new(MockLifecycleService::new("svc_c"));

        registry.register_lifecycle("svc_a", svc_a.clone());
        registry.register_lifecycle("svc_b", svc_b.clone());
        registry.register_lifecycle("svc_c", svc_c.clone());

        let errors = registry.shutdown_all_lifecycle_services();

        assert!(errors.is_empty());
        assert_eq!(svc_a.shutdown_count(), 1);
        assert_eq!(svc_b.shutdown_count(), 1);
        assert_eq!(svc_c.shutdown_count(), 1);
    }

    #[test]
    fn shutdown_all_lifecycle_services_reverse_order() {
        let mut registry = ServiceRegistry::new();

        // Track the order of shutdown calls
        let shutdown_order = Arc::new(std::sync::Mutex::new(Vec::new()));

        struct OrderedService {
            name: String,
            order: Arc<std::sync::Mutex<Vec<String>>>,
        }

        impl Lifecycle for OrderedService {
            fn name(&self) -> &'static str {
                "ordered"
            }
            fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
                self.order.lock().unwrap().push(self.name.clone());
                Ok(())
            }
        }

        let svc_a = Arc::new(OrderedService {
            name: "a".to_string(),
            order: shutdown_order.clone(),
        });
        let svc_b = Arc::new(OrderedService {
            name: "b".to_string(),
            order: shutdown_order.clone(),
        });
        let svc_c = Arc::new(OrderedService {
            name: "c".to_string(),
            order: shutdown_order.clone(),
        });

        registry.register_lifecycle("svc_a", svc_a);
        registry.register_lifecycle("svc_b", svc_b);
        registry.register_lifecycle("svc_c", svc_c);

        let errors = registry.shutdown_all_lifecycle_services();
        assert!(errors.is_empty());

        let order = shutdown_order.lock().unwrap();
        assert_eq!(*order, vec!["c".to_string(), "b".to_string(), "a".to_string()]);
    }

    #[test]
    fn shutdown_all_lifecycle_services_empty_registry() {
        let registry = ServiceRegistry::new();
        let errors = registry.shutdown_all_lifecycle_services();
        assert!(errors.is_empty());
    }

    #[test]
    fn lifecycle_service_keys_preserve_order() {
        let mut registry = ServiceRegistry::new();
        registry.register_lifecycle("first", Arc::new(MockLifecycleService::new("first")));
        registry.register_lifecycle("second", Arc::new(MockLifecycleService::new("second")));
        registry.register_lifecycle("third", Arc::new(MockLifecycleService::new("third")));

        assert_eq!(
            registry.lifecycle_service_keys(),
            vec!["first".to_string(), "second".to_string(), "third".to_string()]
        );
    }
}
