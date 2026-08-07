//! Service Health Registry
//!
//! Provides health status tracking for registered services. Each service
//! registered in the [`ServiceRegistry`](crate::registry::ServiceRegistry)
//! can be queried for its health state through the types defined here.
//!
//! This module supports the `ApplicationContext` health-check and validation
//! pipeline. It is a lightweight, read-only view — health state is derived
//! from whether a service key is present and responsive, not from active
//! polling.

use std::sync::atomic::{AtomicU8, Ordering};

/// Health status of a registered service.
///
/// This mirrors the variants in [`ApplicationContext::check_health`](crate::registry::context::ApplicationContext::check_health)
/// but is intended for standalone health reporting and external inspection
/// (e.g. the Tauri event bridge, diagnostics).
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

/// A coarse-grained health summary across the service registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// All required services are healthy.
    Healthy,
    /// One or more services are unhealthy or not initialized.
    Degraded { unhealthy: usize, not_initialized: usize, missing: usize },
    /// The registry is unavailable or in a critical state.
    Critical(String),
}

/// Lifecycle information for a single registered service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleStageInfo {
    /// The service key.
    pub key: String,
    /// Whether the service has been initialized.
    pub initialized: bool,
    /// Whether the service is currently running.
    pub running: bool,
    /// Whether the service has been shut down.
    pub shutdown: bool,
}

/// A single entry in the health registry, representing a registered service
/// and its current health state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceEntry {
    /// The service key.
    pub key: String,
    /// Current health status.
    pub health: ServiceHealth,
    /// Whether the service is registered in a category.
    pub categories: Vec<String>,
}

/// Thread-safe health tracker for a single service.
///
/// Uses an atomic for lock-free health reads.
#[derive(Debug)]
pub struct ServiceHealthTracker {
    health: AtomicU8,
    error_detail: std::sync::Mutex<Option<String>>,
}

impl ServiceHealthTracker {
    pub fn new() -> Self {
        Self {
            health: AtomicU8::new(0),
            error_detail: std::sync::Mutex::new(None),
        }
    }

    pub fn mark_healthy(&self) {
        self.health.store(0, Ordering::SeqCst);
        *self.error_detail.lock().unwrap() = None;
    }

    pub fn mark_unhealthy(&self, detail: String) {
        self.health.store(2, Ordering::SeqCst);
        *self.error_detail.lock().unwrap() = Some(detail);
    }

    pub fn mark_not_initialized(&self) {
        self.health.store(1, Ordering::SeqCst);
        *self.error_detail.lock().unwrap() = None;
    }

    pub fn mark_not_found(&self) {
        self.health.store(3, Ordering::SeqCst);
        *self.error_detail.lock().unwrap() = None;
    }

    pub fn health(&self) -> ServiceHealth {
        match self.health.load(Ordering::SeqCst) {
            0 => ServiceHealth::Healthy,
            1 => ServiceHealth::NotInitialized,
            2 => {
                let detail = self.error_detail.lock().unwrap().clone();
                ServiceHealth::Unhealthy(detail.unwrap_or_default())
            }
            _ => ServiceHealth::NotFound,
        }
    }
}

impl Default for ServiceHealthTracker {
    fn default() -> Self {
        Self::new()
    }
}
