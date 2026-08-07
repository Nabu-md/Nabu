//! Managed Process — the internal record for a single supervised subprocess.
//!
//! [`ManagedProcess`] is the authoritative in-memory record for a single
//! process under the [`ProcessSupervisor`](super::ProcessSupervisor). It
//! holds observable state (PID, exit code, restart count, timestamps) and
//! the control handles (broadcast sender for stop signaling, monitoring
//! task handle) needed for supervision.
//!
//! The supervisor stores each `ManagedProcess` behind an `Arc<Mutex<>>`
//! so that:
//! - The supervisor can read snapshots of process state.
//! - The monitoring task (a separate tokio task) can update state.
//! - The `tokio::process::Child` is owned exclusively by the monitoring
//!   task (it is `Send` but not `Sync`).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use super::config::ProcessConfig;
use super::errors::ProcessResult;
use super::errors::ProcessSupervisorError;
use super::state::ProcessState;
use super::ProcessId;

/// A process under management of the [`ProcessSupervisor`](super::ProcessSupervisor).
///
/// Each record tracks the observable lifecycle state of a single managed
/// process, along with the control handles needed for supervision:
///
/// - `stop_tx` — broadcast sender that the monitoring task listens on.
///   Calling `supervisor.stop(id)` sends a `()` on this channel to
///   signal the monitoring task to kill the child and exit.
/// - `monitor_handle` — join handle for the monitoring tokio task. Used
///   during shutdown to wait for the task to finish.
///
/// The `Child` itself is **not** stored here — it lives inside the
/// monitoring task (which is `Send` but not `Sync`). Only the PID and
/// exit code are recorded in this struct.
pub struct ManagedProcess {
    /// Stable unique identifier assigned at spawn time.
    pub id: ProcessId,

    /// Human-readable name from the process configuration.
    pub name: String,

    /// The configuration used to spawn (and restart) this process.
    pub config: ProcessConfig,

    /// Current lifecycle state of the process.
    pub state: ProcessState,

    /// OS process ID, when the process is running.
    pub pid: Option<u32>,

    /// Exit code of the most recent exit, when available.
    /// `None` means the process was terminated by a signal or hasn't exited yet.
    pub exit_code: Option<i32>,

    /// Number of times the supervisor has restarted this process.
    pub restart_count: u32,

    /// When the process was first started (most recent spawn).
    pub started_at: Option<DateTime<Utc>>,

    /// When the process last exited.
    pub exited_at: Option<DateTime<Utc>>,

    /// Last error message, if any.
    pub last_error: Option<String>,

    /// Broadcast sender for stopping the monitoring task.
    ///
    /// The monitoring task subscribes to this channel. When `stop()` is
    /// called, a `()` is sent, the monitoring task intercepts it, kills
    /// the child, cleans up, and exits.
    pub stop_tx: Option<broadcast::Sender<()>>,

    /// Handle to the tokio task that monitors this process.
    ///
    /// Used during shutdown to wait for the task to complete after sending
    /// a stop signal.
    pub monitor_handle: Option<JoinHandle<()>>,
}

impl ManagedProcess {
    /// Create a new `ManagedProcess` record in the `Created` state.
    pub fn new(id: ProcessId, config: ProcessConfig) -> Self {
        Self {
            id,
            name: config.name.clone(),
            config,
            state: ProcessState::Created,
            pid: None,
            exit_code: None,
            restart_count: 0,
            started_at: None,
            exited_at: None,
            last_error: None,
            stop_tx: None,
            monitor_handle: None,
        }
    }

    /// Set the lifecycle state, validating the transition.
    ///
    /// Returns `Err` if the transition is invalid.
    pub fn transition_state(&mut self, new_state: ProcessState) -> ProcessResult<()> {
        if ProcessState::can_transition(self.state, new_state) {
            self.state = new_state;
            Ok(())
        } else {
            Err(ProcessSupervisorError::InvalidStateTransition {
                from: self.state,
                to: new_state,
            })
        }
    }
}

/// A serializable, immutable snapshot of a managed process's observable state.
///
/// This is the public-facing type returned by queries on the
/// [`ProcessSupervisor`](super::ProcessSupervisor). It deliberately excludes
/// internal control handles (`stop_tx`, `monitor_handle`) so it can be
/// safely serialized for IPC responses and health reports.
///
/// ## Future compatibility
///
/// All fields use `#[serde(default)]` so that future phases can add new
/// fields (memory usage, CPU percentage, uptime) without breaking
/// deserialization of older snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProcessSnapshot {
    /// The unique identifier of the managed process.
    pub id: ProcessId,

    /// The human-readable name from the process configuration.
    pub name: String,

    /// The command that was (or will be) executed.
    pub command: String,

    /// Command-line arguments.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,

    /// The current lifecycle state of the process.
    pub state: ProcessState,

    /// The OS process ID, when the process is running.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,

    /// Exit code of the most recent exit, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,

    /// Number of times the supervisor has restarted this process.
    pub restart_count: u32,

    /// The restart policy configured for this process.
    pub restart_policy: super::RestartPolicy,

    /// When the process was first started (most recent spawn).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,

    /// When the process last exited.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exited_at: Option<DateTime<Utc>>,

    /// Last error message, if any (e.g. spawn failure reason).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,

    /// Whether the process is currently running.
    #[serde(skip)]
    #[serde(default)]
    pub is_running: bool,

    /// Whether the process is in a terminal state.
    #[serde(skip)]
    #[serde(default)]
    pub is_terminal: bool,
}

impl ProcessSnapshot {
    /// Returns the full command line as a display string.
    pub fn command_line(&self) -> String {
        let mut parts = vec![self.command.clone()];
        parts.extend(self.args.iter().cloned());
        parts.join(" ")
    }

    /// Returns `true` if the process is currently running.
    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// Returns `true` if the process is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        self.is_terminal
    }
}

impl Default for ProcessSnapshot {
    fn default() -> Self {
        Self {
            id: ProcessId::nil(),
            name: String::new(),
            command: String::new(),
            args: Vec::new(),
            state: ProcessState::Created,
            pid: None,
            exit_code: None,
            restart_count: 0,
            restart_policy: super::RestartPolicy::default(),
            started_at: None,
            exited_at: None,
            last_error: None,
            is_running: false,
            is_terminal: false,
        }
    }
}

impl ManagedProcess {
    /// Produce a serializable [`ProcessSnapshot`] from this record.
    pub fn snapshot(&self) -> ProcessSnapshot {
        ProcessSnapshot {
            id: self.id,
            name: self.name.clone(),
            command: self.config.command.clone(),
            args: self.config.args.clone(),
            state: self.state,
            pid: self.pid,
            exit_code: self.exit_code,
            restart_count: self.restart_count,
            restart_policy: self.config.restart_policy,
            started_at: self.started_at,
            exited_at: self.exited_at,
            last_error: self.last_error.clone(),
            is_running: self.state.is_running(),
            is_terminal: self.state.is_terminal(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process_supervisor::config::ProcessConfig;
    use crate::process_supervisor::policy::RestartPolicy;

    #[test]
    fn new_process_starts_in_created() {
        let id = ProcessId::new_v4();
        let config = ProcessConfig::new("test", "echo");
        let proc = ManagedProcess::new(id, config);

        assert_eq!(proc.id, id);
        assert_eq!(proc.state, ProcessState::Created);
        assert_eq!(proc.restart_count, 0);
        assert!(proc.pid.is_none());
        assert!(proc.exit_code.is_none());
        assert!(proc.last_error.is_none());
    }

    #[test]
    fn valid_state_transition_succeeds() {
        let id = ProcessId::new_v4();
        let config = ProcessConfig::new("test", "echo");
        let mut proc = ManagedProcess::new(id, config);

        assert!(proc.transition_state(ProcessState::Starting).is_ok());
        assert_eq!(proc.state, ProcessState::Starting);

        assert!(proc.transition_state(ProcessState::Running).is_ok());
        assert_eq!(proc.state, ProcessState::Running);

        assert!(proc.transition_state(ProcessState::Exited).is_ok());
        assert_eq!(proc.state, ProcessState::Exited);
    }

    #[test]
    fn invalid_state_transition_fails() {
        let id = ProcessId::new_v4();
        let config = ProcessConfig::new("test", "echo");
        let mut proc = ManagedProcess::new(id, config);

        // Created → Running is invalid (skip Starting)
        let err = proc.transition_state(ProcessState::Running).unwrap_err();
        assert!(matches!(
            err,
            ProcessSupervisorError::InvalidStateTransition { .. }
        ));
        assert_eq!(proc.state, ProcessState::Created); // unchanged
    }

    #[test]
    fn same_state_transition_is_noop() {
        let id = ProcessId::new_v4();
        let config = ProcessConfig::new("test", "echo");
        let mut proc = ManagedProcess::new(id, config);

        assert!(proc.transition_state(ProcessState::Created).is_ok());
        assert_eq!(proc.state, ProcessState::Created);
    }

    #[test]
    fn snapshot_is_serializable() {
        let id = ProcessId::new_v4();
        let config = ProcessConfig::new("test", "echo")
            .with_args(vec!["hello".to_string()])
            .with_restart_policy(RestartPolicy::Always);
        let mut proc = ManagedProcess::new(id, config);
        proc.transition_state(ProcessState::Running).unwrap();
        proc.pid = Some(12345);

        let snapshot = proc.snapshot();

        let json = serde_json::to_string(&snapshot).unwrap();
        let back: ProcessSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(back.id, snapshot.id);
        assert_eq!(back.name, snapshot.name);
        assert_eq!(back.state, ProcessState::Running);
        assert_eq!(back.pid, Some(12345));
        assert!(back.is_running);
        assert!(!back.is_terminal);
    }

    #[test]
    fn snapshot_default() {
        let snapshot = ProcessSnapshot::default();
        assert_eq!(snapshot.state, ProcessState::Created);
        assert!(!snapshot.is_running);
        assert!(!snapshot.is_terminal);
        assert!(snapshot.args.is_empty());
    }

    #[test]
    fn snapshot_command_line() {
        let id = ProcessId::new_v4();
        let config = ProcessConfig::new("srv", "/bin/server")
            .with_args(vec!["--port".to_string(), "8080".to_string()]);
        let proc = ManagedProcess::new(id, config);
        let snapshot = proc.snapshot();

        assert_eq!(snapshot.command_line(), "/bin/server --port 8080");
    }
}
