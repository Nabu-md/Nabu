//! Process Health — health monitoring model for managed subprocesses.
//!
//! [`ProcessHealthStatus`] represents the observable health of a single
//! supervised process by mapping its [`ProcessState`] and runtime attributes
//! (PID, exit code, restart count) into a health vocabulary that the platform's
//! health reporting system can consume.
//!
//! ## Health Model
//!
//! | State      | Health        | Notes                                |
//! |------------|---------------|--------------------------------------|
//! | Running    | Healthy       | Process is alive and running         |
//! | Starting   | Starting      | Process is booting up                |
//! | Restarting | Starting      | Process is being restarted           |
//! | Running +  | Degraded      | Non-fatal error or past restarts     |
//! | Exited     | Unhealthy     | Process has exited (needs attention) |
//! | Failed     | Unhealthy     | Process has failed                   |
//! | Stopped    | Stopped       | Process was stopped gracefully       |
//! | Created    | Unknown       | Process created but not yet started  |
//!
//! ## Integration
//!
//! Health changes are published through the EventBus as
//! [`ProcessHealthChangedEvent`] events. The [`ProcessHealth`] struct
//! provides a serializable snapshot suitable for IPC and health reports.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::state::ProcessState;
use super::ProcessId;

/// Health status of a managed process.
///
/// This is a higher-level abstraction over [`ProcessState`] that expresses
/// the process's health from the perspective of the platform health system.
/// It is derived from the process state and runtime attributes — it is not
/// independently tracked.
///
/// ## Lifecycle of a health check
///
/// ```text
/// Running            → Healthy
/// Starting/Restarting → Starting
/// Exited/Failed       → Unhealthy
/// Stopped            → Stopped
/// Running + restarts  → Degraded
/// Created             → Unknown
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessHealthStatus {
    /// The process is running normally.
    ///
    /// Mapped from [`ProcessState::Running`] with no error history (no
    /// previous restarts and no `last_error`).
    Healthy,

    /// The process is in the process of starting or restarting.
    ///
    /// Mapped from [`ProcessState::Starting`] or [`ProcessState::Restarting`].
    Starting,

    /// The process is running but experiencing issues.
    ///
    /// Mapped from [`ProcessState::Running`] when the process has been
    /// restarted previously (restart_count > 0) or has a non-fatal error
    /// in `last_error`.
    Degraded,

    /// The process has exited or failed and is not running.
    ///
    /// Mapped from [`ProcessState::Exited`] or [`ProcessState::Failed`]
    /// before the restart policy has been evaluated or when no restart
    /// is permitted.
    Unhealthy,

    /// The process has been stopped and will not be restarted.
    ///
    /// Mapped from [`ProcessState::Stopped`].
    Stopped,

    /// Health could not be determined — typically because the process
    /// is in the `Created` state (registered but not yet started).
    ///
    /// Mapped from [`ProcessState::Created`].
    Unknown,
}

impl Default for ProcessHealthStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

impl ProcessHealthStatus {
    /// Returns a human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Starting => "starting",
            Self::Degraded => "degraded",
            Self::Unhealthy => "unhealthy",
            Self::Stopped => "stopped",
            Self::Unknown => "unknown",
        }
    }

    /// Returns `true` if the process is in a healthy state
    /// (either [`Healthy`](Self::Healthy) or [`Degraded`](Self::Degraded)).
    pub fn is_healthy(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }

    /// Returns `true` if the process is currently running.
    ///
    /// Only [`Healthy`](Self::Healthy) and [`Degraded`](Self::Degraded)
    /// indicate a running process.
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Healthy | Self::Degraded)
    }
}

impl std::fmt::Display for ProcessHealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// A serializable health snapshot for a single managed process.
///
/// Constructed from a [`ManagedProcess`](super::managed::ManagedProcess)
/// record via [`ProcessHealth::from_managed`](Self::from_managed).
/// All fields use `#[serde(default)]` for forward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessHealth {
    /// The managed process ID.
    pub process_id: ProcessId,

    /// Human-readable name from the process configuration.
    pub name: String,

    /// The current health status.
    #[serde(default)]
    pub status: ProcessHealthStatus,

    /// The current process state.
    #[serde(default)]
    pub state: ProcessState,

    /// The OS process ID, when the process is running.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,

    /// Exit code of the most recent exit, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,

    /// Number of times the supervisor has restarted this process.
    #[serde(default)]
    pub restart_count: u32,

    /// Last error message, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,

    /// When the process was last started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,

    /// When the process last exited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exited_at: Option<DateTime<Utc>>,

    /// When this health snapshot was generated.
    pub checked_at: DateTime<Utc>,
}

impl ProcessHealth {
    /// Derive a [`ProcessHealth`] snapshot from a [`ManagedProcess`].
    ///
    /// The health status is computed from the process state and runtime
    /// attributes — it is never independently tracked.
    pub fn from_managed(managed: &super::managed::ManagedProcess) -> Self {
        let status = compute_health(
            &managed.state,
            managed.restart_count,
            managed.last_error.as_deref(),
        );

        Self {
            process_id: managed.id,
            name: managed.name.clone(),
            status,
            state: managed.state,
            pid: managed.pid,
            exit_code: managed.exit_code,
            restart_count: managed.restart_count,
            last_error: managed.last_error.clone(),
            started_at: managed.started_at,
            exited_at: managed.exited_at,
            checked_at: Utc::now(),
        }
    }

    /// Returns `true` if the process is in a healthy state
    /// (see [`ProcessHealthStatus::is_healthy`]).
    pub fn is_healthy(&self) -> bool {
        self.status.is_healthy()
    }

    /// Returns `true` if the process is currently running
    /// (see [`ProcessHealthStatus::is_running`]).
    pub fn is_running(&self) -> bool {
        self.status.is_running()
    }
}

/// Computes the [`ProcessHealthStatus`] from the process state and attributes.
///
/// - `Running` with 0 restarts and no error → `Healthy`
/// - `Running` with >0 restarts or error → `Degraded`
/// - `Starting` / `Restarting` → `Starting`
/// - `Exited` / `Failed` → `Unhealthy`
/// - `Stopped` → `Stopped`
/// - `Created` → `Unknown`
pub(crate) fn compute_health(
    state: &ProcessState,
    restart_count: u32,
    last_error: Option<&str>,
) -> ProcessHealthStatus {
    match state {
        ProcessState::Running => {
            if restart_count > 0 || last_error.is_some() {
                ProcessHealthStatus::Degraded
            } else {
                ProcessHealthStatus::Healthy
            }
        }
        ProcessState::Starting | ProcessState::Restarting => ProcessHealthStatus::Starting,
        ProcessState::Stopping => ProcessHealthStatus::Stopped,
        ProcessState::Exited | ProcessState::Failed => ProcessHealthStatus::Unhealthy,
        ProcessState::Stopped => ProcessHealthStatus::Stopped,
        ProcessState::Created => ProcessHealthStatus::Unknown,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process_supervisor::config::ProcessConfig;
    use crate::process_supervisor::managed::ManagedProcess;
    use crate::process_supervisor::policy::RestartPolicy;

    #[test]
    fn health_status_labels() {
        assert_eq!(ProcessHealthStatus::Healthy.label(), "healthy");
        assert_eq!(ProcessHealthStatus::Starting.label(), "starting");
        assert_eq!(ProcessHealthStatus::Degraded.label(), "degraded");
        assert_eq!(ProcessHealthStatus::Unhealthy.label(), "unhealthy");
        assert_eq!(ProcessHealthStatus::Stopped.label(), "stopped");
        assert_eq!(ProcessHealthStatus::Unknown.label(), "unknown");
    }

    #[test]
    fn is_healthy_for_healthy_and_degraded() {
        assert!(ProcessHealthStatus::Healthy.is_healthy());
        assert!(ProcessHealthStatus::Degraded.is_healthy());
        assert!(!ProcessHealthStatus::Starting.is_healthy());
        assert!(!ProcessHealthStatus::Unhealthy.is_healthy());
        assert!(!ProcessHealthStatus::Stopped.is_healthy());
        assert!(!ProcessHealthStatus::Unknown.is_healthy());
    }

    #[test]
    fn is_running_for_healthy_and_degraded() {
        assert!(ProcessHealthStatus::Healthy.is_running());
        assert!(ProcessHealthStatus::Degraded.is_running());
        assert!(!ProcessHealthStatus::Starting.is_running());
        assert!(!ProcessHealthStatus::Unhealthy.is_running());
        assert!(!ProcessHealthStatus::Stopped.is_running());
        assert!(!ProcessHealthStatus::Unknown.is_running());
    }

    #[test]
    fn default_health_status_is_unknown() {
        assert_eq!(ProcessHealthStatus::default(), ProcessHealthStatus::Unknown);
    }

    #[test]
    fn compute_health_running_is_healthy() {
        assert_eq!(
            compute_health(&ProcessState::Running, 0, None),
            ProcessHealthStatus::Healthy
        );
    }

    #[test]
    fn compute_health_running_with_restarts_is_degraded() {
        assert_eq!(
            compute_health(&ProcessState::Running, 1, None),
            ProcessHealthStatus::Degraded
        );
    }

    #[test]
    fn compute_health_running_with_error_is_degraded() {
        assert_eq!(
            compute_health(&ProcessState::Running, 0, Some("warning")),
            ProcessHealthStatus::Degraded
        );
    }

    #[test]
    fn compute_health_starting() {
        assert_eq!(
            compute_health(&ProcessState::Starting, 0, None),
            ProcessHealthStatus::Starting
        );
        assert_eq!(
            compute_health(&ProcessState::Restarting, 0, None),
            ProcessHealthStatus::Starting
        );
    }

    #[test]
    fn compute_health_exited_is_unhealthy() {
        assert_eq!(
            compute_health(&ProcessState::Exited, 0, None),
            ProcessHealthStatus::Unhealthy
        );
    }

    #[test]
    fn compute_health_failed_is_unhealthy() {
        assert_eq!(
            compute_health(&ProcessState::Failed, 0, Some("error")),
            ProcessHealthStatus::Unhealthy
        );
    }

    #[test]
    fn compute_health_stopped() {
        assert_eq!(
            compute_health(&ProcessState::Stopped, 0, None),
            ProcessHealthStatus::Stopped
        );
    }

    #[test]
    fn compute_health_created_is_unknown() {
        assert_eq!(
            compute_health(&ProcessState::Created, 0, None),
            ProcessHealthStatus::Unknown
        );
    }

    #[test]
    fn process_health_from_managed() {
        let id = ProcessId::new_v4();
        let config = ProcessConfig::new("test", "echo").with_restart_policy(RestartPolicy::Always);
        let mut managed = ManagedProcess::new(id, config);
        managed.state = ProcessState::Running;
        managed.pid = Some(12345);
        managed.started_at = Some(Utc::now());

        let health = ProcessHealth::from_managed(&managed);

        assert_eq!(health.process_id, id);
        assert_eq!(health.name, "test");
        assert_eq!(health.status, ProcessHealthStatus::Healthy);
        assert_eq!(health.state, ProcessState::Running);
        assert_eq!(health.pid, Some(12345));
        assert_eq!(health.restart_count, 0);
        assert!(health.started_at.is_some());
        assert!(health.checked_at <= Utc::now());
    }

    #[test]
    fn process_health_from_managed_degraded() {
        let id = ProcessId::new_v4();
        let config = ProcessConfig::new("test", "echo");
        let mut managed = ManagedProcess::new(id, config);
        managed.state = ProcessState::Running;
        managed.restart_count = 3;
        managed.last_error = Some("previous crash".to_string());

        let health = ProcessHealth::from_managed(&managed);
        assert_eq!(health.status, ProcessHealthStatus::Degraded);
    }

    #[test]
    fn process_health_serializes() {
        let id = ProcessId::new_v4();
        let config = ProcessConfig::new("test", "echo");
        let managed = ManagedProcess::new(id, config);

        let health = ProcessHealth::from_managed(&managed);
        let json = serde_json::to_string(&health).unwrap();
        let back: ProcessHealth = serde_json::from_str(&json).unwrap();

        assert_eq!(back.process_id, health.process_id);
        assert_eq!(back.name, health.name);
        assert_eq!(back.status, health.status);
    }

    #[test]
    fn process_health_status_serializes_to_snake_case() {
        let json = serde_json::to_string(&ProcessHealthStatus::Healthy).unwrap();
        assert_eq!(json, "\"healthy\"");

        let json = serde_json::to_string(&ProcessHealthStatus::Unknown).unwrap();
        assert_eq!(json, "\"unknown\"");

        let json = serde_json::to_string(&ProcessHealthStatus::Degraded).unwrap();
        assert_eq!(json, "\"degraded\"");
    }

    #[test]
    fn process_health_ignores_unknown_fields() {
        let json = r#"{
            "process_id": "00000000-0000-0000-0000-000000000000",
            "name": "test",
            "status": "healthy",
            "state": "running",
            "pid": 12345,
            "exit_code": null,
            "restart_count": 0,
            "last_error": null,
            "started_at": null,
            "exited_at": null,
            "checked_at": "2024-01-01T00:00:00Z",
            "future_field": "ignored"
        }"#;
        let health: ProcessHealth = serde_json::from_str(json).unwrap();
        assert_eq!(health.name, "test");
        assert_eq!(health.status, ProcessHealthStatus::Healthy);
    }
}
