//! Health reporting model for the application lifecycle.
//!
//! This module provides the [`ServiceHealth`] struct — a structured, serializable
//! summary of the application's runtime state. Health information is collected
//! directly from the [`LifecycleManager`] (the single source of truth for
//! lifecycle state) and the [`ServiceRegistry`], with no duplicate state
//! maintained.
//!
//! ## Usage
//!
//! ```ignore
//! let ctx: ApplicationContext = /* ... */;
//! let health: ServiceHealth = ctx.health_check();
//! assert_eq!(health.overall_status, HealthStatus::Healthy);
//! ```
//!
//! ## Future Compatibility
//!
//! Every field uses `#[serde(default)]` so that future additions
//! (uptime, version, performance metrics, memory usage, dependency health,
//! plugin status, capability status) can be added without breaking
//! backward-compatible deserialization.
//!
//! [`LifecycleManager`]: crate::registry::lifecycle::LifecycleManager
//! [`ServiceRegistry`]: crate::registry::ServiceRegistry

use serde::{Deserialize, Serialize};

use crate::registry::lifecycle::LifecycleStage;

// ---------------------------------------------------------------------------
// ServiceStatus — per-service health (used by check_health)
// ---------------------------------------------------------------------------

/// Health status of a single service key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceStatus {
    /// Service is registered and functional.
    Healthy,
    /// Service is not registered.
    NotFound,
}

impl ServiceStatus {
    /// Returns a human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::NotFound => "not_found",
        }
    }
}

// ---------------------------------------------------------------------------
// HealthStatus — overall health summary
// ---------------------------------------------------------------------------

/// Overall health status of the application.
///
/// Aggregated from individual service lifecycle stages and lifecycle
/// transitions. This is the single value a caller needs to determine
/// whether the platform is ready to accept work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    /// All lifecycle-managed services are at least initialized and none
    /// have failed or been shut down.
    Healthy,
    /// The application is operational but one or more services are not in
    /// their expected state (e.g. a service is shut down or failed).
    Degraded,
    /// One or more services have failed, or health information could not
    /// be fully obtained.
    Unhealthy,
    /// Health could not be determined — typically because the application
    /// has not yet been initialized.
    Unknown,
}

impl Default for HealthStatus {
    fn default() -> Self {
        HealthStatus::Unknown
    }
}

impl HealthStatus {
    /// Returns a human-readable label suitable for logging.
    pub fn label(&self) -> &'static str {
        match self {
            HealthStatus::Healthy => "healthy",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Unhealthy => "unhealthy",
            HealthStatus::Unknown => "unknown",
        }
    }
}

// ---------------------------------------------------------------------------
// LifecycleStageInfo — serializable lifecycle stage
// ---------------------------------------------------------------------------

/// Serializable representation of a service's lifecycle stage.
///
/// This mirrors [`crate::registry::lifecycle::LifecycleStage`] but is
/// independently serializable for IPC responses. The two types are kept
/// separate so that the core lifecycle module can remain free of serde
/// coupling, while the health model is fully serializable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleStageInfo {
    /// The service has been created but not initialized.
    Created,
    /// The service has been initialized but is not yet running.
    Initialized,
    /// The service is fully operational.
    Running,
    /// The service has been shut down.
    Shutdown,
}

/// Alias — the health-report stage type used by [`ServiceHealth`] and
/// [`ServiceEntry`].  Both names refer to the same type so that consumers
/// may use whichever is most descriptive in context.
pub type ServiceLifecycleStageInfo = LifecycleStageInfo;

impl Default for LifecycleStageInfo {
    fn default() -> Self {
        LifecycleStageInfo::Created
    }
}

impl From<LifecycleStage> for LifecycleStageInfo {
    fn from(stage: LifecycleStage) -> Self {
        match stage {
            LifecycleStage::Created => LifecycleStageInfo::Created,
            LifecycleStage::Initialized => LifecycleStageInfo::Initialized,
            LifecycleStage::Running => LifecycleStageInfo::Running,
            LifecycleStage::Shutdown => LifecycleStageInfo::Shutdown,
        }
    }
}

impl std::fmt::Display for LifecycleStageInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LifecycleStageInfo::Created => write!(f, "created"),
            LifecycleStageInfo::Initialized => write!(f, "initialized"),
            LifecycleStageInfo::Running => write!(f, "running"),
            LifecycleStageInfo::Shutdown => write!(f, "shutdown"),
        }
    }
}

// ---------------------------------------------------------------------------
// ServiceEntry — per-service lifecycle snapshot
// ---------------------------------------------------------------------------

/// Lifecycle information for a single registered service.
///
/// Each entry reflects the service's current lifecycle stage as reported
/// by its own [`LifecycleManager`] — no duplicate tracking is performed.
///
/// [`LifecycleManager`]: crate::registry::lifecycle::LifecycleManager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    /// The service key (e.g. `"capture_engine"`, `"storage_manager"`).
    pub name: String,
    /// The service's current lifecycle stage.
    #[serde(default)]
    pub stage: LifecycleStageInfo,
    /// Whether the service is in a non-error state.
    #[serde(default)]
    pub healthy: bool,
}

// ---------------------------------------------------------------------------
// ServiceHealth — the full health report
// ---------------------------------------------------------------------------

/// Structured health report for the application's core services.
///
/// `ServiceHealth` is the canonical health API for the Capability Platform. It
/// is collected directly from the [`LifecycleManager`] (the single source of
/// truth for lifecycle state) and the [`ServiceRegistry`]:
///
/// - The overall lifecycle stage comes from the `ApplicationContext`'s
///   `LifecycleManager` — never a duplicate flag.
/// - Per-service stages come from each service's own `LifecycleManager`.
/// - Service counts come from the `ServiceRegistry`.
///
/// No separate health state is maintained — the report is computed on each
/// call from the live state of the lifecycle managers.
///
/// ## Future Compatibility
///
/// All fields use `#[serde(default)]` so that future phases can add new
/// fields (uptime, version, performance metrics, memory usage, dependency
/// health, plugin status, capability status) without breaking serialization
/// compatibility.
///
/// [`LifecycleManager`]: crate::registry::lifecycle::LifecycleManager
/// [`ServiceRegistry`]: crate::registry::ServiceRegistry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceHealth {
    /// Overall health status, aggregated from per-service lifecycle stages.
    #[serde(default)]
    pub overall_status: HealthStatus,

    /// The current lifecycle stage of the application context, sourced
    /// directly from its `LifecycleManager`.
    #[serde(default)]
    pub lifecycle_stage: LifecycleStageInfo,

    /// Whether the application context has been initialized (stage >= Initialized).
    #[serde(default)]
    pub initialized: bool,

    /// Whether the application context is running (stage == Running).
    #[serde(default)]
    pub running: bool,

    /// Whether the last startup attempt succeeded. This is `true` when the
    /// context has at least reached the `Initialized` stage.
    #[serde(default)]
    pub startup_success: bool,

    /// Total number of registered services (singletons + factories).
    #[serde(default)]
    pub registered_services: usize,

    /// All registered singleton service keys (sorted).
    #[serde(default)]
    pub service_names: Vec<String>,

    /// Per-service lifecycle status for known lifecycle-managed services.
    /// Only services that expose a `lifecycle_stage()` accessor are included.
    #[serde(default)]
    pub services: Vec<ServiceEntry>,

    /// Number of lifecycle-managed services currently at the `Running` stage.
    #[serde(default)]
    pub running_service_count: usize,

    /// Number of lifecycle-managed services currently at the `Shutdown` stage.
    #[serde(default)]
    pub stopped_service_count: usize,

    /// Number of lifecycle-managed services that are not healthy
    /// (i.e. still at `Created` with no initialization, or in an error state).
    #[serde(default)]
    pub failed_service_count: usize,

    /// Number of registered capabilities in the `CapabilityRegistry`.
    #[serde(default)]
    pub capability_count: usize,

    /// Error information if health collection encountered partial failures.
    /// `None` when the health report is complete and consistent.
    #[serde(default)]
    pub error: Option<String>,

    // ── Reserved for future phases ──────────────────────────────────
    // These fields are intentionally omitted but the model is designed
    // to accommodate them via serde defaults:
    //
    // pub uptime: Option<Duration>,
    // pub version: Option<String>,
    // pub metrics: Option<PerformanceMetrics>,
    // pub memory_usage: Option<MemoryUsage>,
    // pub dependency_health: Option<Vec<DependencyHealth>>,
    // pub plugin_status: Option<Vec<PluginStatus>>,
    // pub capability_status: Option<Vec<CapabilityStatus>>,
}

impl Default for ServiceHealth {
    fn default() -> Self {
        ServiceHealth {
            overall_status: HealthStatus::Unknown,
            lifecycle_stage: LifecycleStageInfo::Created,
            initialized: false,
            running: false,
            startup_success: false,
            registered_services: 0,
            service_names: Vec::new(),
            services: Vec::new(),
            running_service_count: 0,
            stopped_service_count: 0,
            failed_service_count: 0,
            capability_count: 0,
            error: None,
        }
    }
}

impl ServiceHealth {
    /// Returns the overall status as a short label suitable for logging.
    pub fn status_label(&self) -> &'static str {
        self.overall_status.label()
    }

    /// Returns `true` if all registered lifecycle-managed services are
    /// healthy and the context is at least initialized.
    pub fn is_healthy(&self) -> bool {
        self.overall_status == HealthStatus::Healthy && self.initialized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // LifecycleStageInfo conversion and serialization
    // -----------------------------------------------------------------------

    #[test]
    fn lifecycle_stage_info_from_all_stages() {
        assert_eq!(
            LifecycleStageInfo::from(LifecycleStage::Created),
            LifecycleStageInfo::Created
        );
        assert_eq!(
            LifecycleStageInfo::from(LifecycleStage::Initialized),
            LifecycleStageInfo::Initialized
        );
        assert_eq!(
            LifecycleStageInfo::from(LifecycleStage::Running),
            LifecycleStageInfo::Running
        );
        assert_eq!(
            LifecycleStageInfo::from(LifecycleStage::Shutdown),
            LifecycleStageInfo::Shutdown
        );
    }

    #[test]
    fn lifecycle_stage_info_serializes_correctly() {
        assert_eq!(
            serde_json::to_string(&LifecycleStageInfo::Created).unwrap(),
            "\"created\""
        );
        assert_eq!(
            serde_json::to_string(&LifecycleStageInfo::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::to_string(&LifecycleStageInfo::Shutdown).unwrap(),
            "\"shutdown\""
        );
    }

    // -----------------------------------------------------------------------
    // HealthStatus
    // -----------------------------------------------------------------------

    #[test]
    fn health_status_default_is_unknown() {
        assert_eq!(HealthStatus::default(), HealthStatus::Unknown);
    }

    #[test]
    fn health_status_label() {
        assert_eq!(HealthStatus::Healthy.label(), "healthy");
        assert_eq!(HealthStatus::Degraded.label(), "degraded");
        assert_eq!(HealthStatus::Unhealthy.label(), "unhealthy");
        assert_eq!(HealthStatus::Unknown.label(), "unknown");
    }

    #[test]
    fn health_status_serializes_correctly() {
        assert_eq!(
            serde_json::to_string(&HealthStatus::Healthy).unwrap(),
            "\"healthy\""
        );
        assert_eq!(
            serde_json::to_string(&HealthStatus::Unhealthy).unwrap(),
            "\"unhealthy\""
        );
    }

    // -----------------------------------------------------------------------
    // ServiceEntry
    // -----------------------------------------------------------------------

    #[test]
    fn service_entry_serializes() {
        let entry = ServiceEntry {
            name: "test_service".to_string(),
            stage: LifecycleStageInfo::Running,
            healthy: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("test_service"));
        assert!(json.contains("running"));
    }

    // -----------------------------------------------------------------------
    // ServiceHealth — default and serialization
    // -----------------------------------------------------------------------

    #[test]
    fn service_health_default() {
        let health = ServiceHealth::default();
        assert_eq!(health.overall_status, HealthStatus::Unknown);
        assert_eq!(health.lifecycle_stage, LifecycleStageInfo::Created);
        assert!(!health.initialized);
        assert!(!health.running);
        assert!(!health.startup_success);
        assert_eq!(health.registered_services, 0);
        assert!(health.services.is_empty());
        assert!(health.error.is_none());
    }

    #[test]
    fn service_health_status_label() {
        assert_eq!(ServiceHealth::default().status_label(), "unknown");

        let mut h = ServiceHealth::default();
        h.overall_status = HealthStatus::Healthy;
        assert_eq!(h.status_label(), "healthy");

        h.overall_status = HealthStatus::Degraded;
        assert_eq!(h.status_label(), "degraded");

        h.overall_status = HealthStatus::Unhealthy;
        assert_eq!(h.status_label(), "unhealthy");
    }

    #[test]
    fn service_health_is_healthy() {
        let mut h = ServiceHealth::default();
        h.overall_status = HealthStatus::Healthy;
        h.initialized = true;
        assert!(h.is_healthy());

        h.initialized = false;
        assert!(!h.is_healthy());

        h.overall_status = HealthStatus::Unknown;
        h.initialized = true;
        assert!(!h.is_healthy());
    }

    // -----------------------------------------------------------------------
    // ServiceHealth — serde round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn service_health_serializes_and_deserializes() {
        let health = ServiceHealth {
            overall_status: HealthStatus::Healthy,
            lifecycle_stage: LifecycleStageInfo::Running,
            initialized: true,
            running: true,
            startup_success: true,
            registered_services: 8,
            service_names: vec!["event_bus".to_string(), "capture_engine".to_string()],
            services: vec![
                ServiceEntry {
                    name: "capture_engine".to_string(),
                    stage: LifecycleStageInfo::Running,
                    healthy: true,
                },
                ServiceEntry {
                    name: "storage_manager".to_string(),
                    stage: LifecycleStageInfo::Running,
                    healthy: true,
                },
            ],
            running_service_count: 2,
            stopped_service_count: 0,
            failed_service_count: 0,
            capability_count: 14,
            error: None,
        };

        let json = serde_json::to_string(&health).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("running"));
        assert!(json.contains("capture_engine"));

        // Round-trip: deserialize back
        let deserialized: ServiceHealth = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.overall_status, HealthStatus::Healthy);
        assert_eq!(deserialized.lifecycle_stage, LifecycleStageInfo::Running);
        assert!(deserialized.initialized);
        assert!(deserialized.running);
        assert_eq!(deserialized.registered_services, 8);
        assert_eq!(deserialized.services.len(), 2);
        assert_eq!(deserialized.capability_count, 14);
        assert!(deserialized.error.is_none());
    }

    // -----------------------------------------------------------------------
    // ServiceHealth — forward-compatible deserialization
    // -----------------------------------------------------------------------

    #[test]
    fn service_health_ignores_unknown_fields() {
        // Simulates a future version that adds new fields — deserialization
        // with #[serde(default)] should not fail.
        let json = r#"{
            "overall_status": "healthy",
            "lifecycle_stage": "running",
            "initialized": true,
            "running": true,
            "startup_success": true,
            "registered_services": 5,
            "service_names": ["a", "b"],
            "services": [],
            "running_service_count": 0,
            "stopped_service_count": 0,
            "failed_service_count": 0,
            "capability_count": 10,
            "error": null,
            "uptime_ms": 12345,
            "version": "0.1.0"
        }"#;
        let deserialized: ServiceHealth = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized.registered_services, 5);
        assert!(deserialized.initialized);
    }

    #[test]
    fn service_health_deserializes_missing_fields_with_defaults() {
        // An empty JSON object should deserialize to the default.
        let deserialized: ServiceHealth = serde_json::from_str("{}").unwrap();
        assert_eq!(deserialized.overall_status, HealthStatus::Unknown);
        assert_eq!(deserialized.lifecycle_stage, LifecycleStageInfo::Created);
        assert!(!deserialized.initialized);
        assert_eq!(deserialized.registered_services, 0);
    }
}
