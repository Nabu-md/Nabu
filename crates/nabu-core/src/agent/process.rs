//! # Agent Process — Runtime Representation of a Managed Agent
//!
//! [`AgentProcess`] is the runtime record for a single managed agent within
//! the [`AgentManager`](super::AgentManager). It wraps a
//! [`ManagedProcess`](crate::process_supervisor::ManagedProcess) record
//! (provided by the [`ProcessSupervisor`](crate::process_supervisor::ProcessSupervisor))
//! and adds agent-specific metadata: the agent's logical name, kind, and
//! lifecycle stage within the manager.
//!
//! ## Architecture
//!
//! ```text
//! AgentManager (owns Arc<AgentProcess> records)
//!   │
//!   ├── AgentProcess  ← this module
//!   │     ├── metadata: Arc<AgentMetadata>  (immutable config + tracking)
//!   │     ├── lifecycle: LifecycleManager  (agent-level lifecycle)
//!   │     └── supervisor: ProcessSupervisor (delegates process management)
//!   │
//!   └── ProcessSupervisor (manages the actual child process)
//!         └── ManagedProcess (internal record, state + pid + restart_count)
//! ```
//!
//! The `AgentProcess` does **not** own the underlying `tokio::process::Child` —
//! that is exclusively owned by the supervisor's monitoring task. The
//! `AgentProcess` holds only a `ProcessId` that can be used to query the
//! supervisor for the current `ProcessSnapshot`.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::process_supervisor::{ProcessId, ProcessSnapshot, ProcessState};
use crate::registry::lifecycle::{LifecycleError, LifecycleManager, LifecycleStage};

use super::config::{AgentConfig, AgentMetadata};

/// The runtime state of a registered agent within the `AgentManager`.
///
/// This is the agent-level lifecycle, distinct from the underlying process
/// state tracked by the `ProcessSupervisor`. The agent lifecycle tracks
/// the *management* lifecycle — when an agent is registered, started,
/// stopped, or removed — while the process state tracks the *OS process*
/// lifecycle.
///
/// ```text
/// Registered → Starting → Running → Stopping → Stopped
///    │           │          │          │         │
///    │           │          │          │         └─ terminal (agent removed)
///    │           │          │          └─ stop requested
///    │           │          │
///    │           │          └─ process running
///    │           │
///    │           └─ process spawning
///    │
///    └─ agent registered but not started
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AgentProcessState {
    /// The agent has been registered but not yet started.
    Registered,

    /// The agent has been started and the supervisor is spawning the process.
    Starting,

    /// The agent's process is running.
    Running,

    /// The agent's process is being stopped.
    Stopping,

    /// The agent has been stopped and is no longer managed.
    Stopped,
}

impl Default for AgentProcessState {
    fn default() -> Self {
        Self::Registered
    }
}

impl AgentProcessState {
    /// Returns `true` if the agent's process is running.
    pub fn is_running(&self) -> bool {
        *self == Self::Running
    }

    /// Returns `true` if the agent is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        *self == Self::Stopped
    }

    /// Returns `true` if the agent is in a starting/restarting state.
    pub fn is_starting(&self) -> bool {
        matches!(self, Self::Starting)
    }

    /// Returns a human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Registered => "registered",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
        }
    }
}

impl std::fmt::Display for AgentProcessState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// The runtime record for a single managed agent.
///
/// An `AgentProcess` binds together:
/// - The agent's [`AgentConfig`] (declarative specification).
/// - The agent's [`AgentMetadata`] (tracked state: lifecycle, start count, etc.).
/// - The [`ProcessId`] assigned by the [`ProcessSupervisor`](crate::process_supervisor::ProcessSupervisor)
///   when the underlying process was spawned.
///
/// The `AgentProcess` is stored in the `AgentManager`'s registry behind an
/// `Arc<Mutex<>>` so that both the manager's synchronous API and background
/// monitoring tasks can access it safely.
pub struct AgentProcess {
    /// The agent's configuration (immutable after registration).
    pub config: AgentConfig,

    /// Mutable metadata tracked by the manager.
    pub metadata: AgentMetadata,

    /// The `ProcessId` assigned by the `ProcessSupervisor` when the process
    /// was last spawned. `None` before the first spawn or after the process
    /// has been stopped.
    pub process_id: Option<ProcessId>,

    /// The agent's management lifecycle stage.
    pub agent_state: AgentProcessState,

    /// Lifecycle manager for the agent-level lifecycle transitions.
    pub lifecycle: LifecycleManager,
}

impl AgentProcess {
    /// Create a new `AgentProcess` record in the `Registered` state.
    ///
    /// The agent is registered but not yet started — `process_id` is `None`.
    pub fn new(config: AgentConfig) -> Self {
        let now = Utc::now();
        let metadata = AgentMetadata {
            name: config.name.clone(),
            kind: config.kind,
            jsonrpc: config.jsonrpc.clone(),
            transport: config.transport.clone(),
            lifecycle_stage: LifecycleStage::Created,
            registered_at: now,
            started_at: None,
            stopped_at: None,
            start_count: 0,
            crash_count: 0,
            last_error: None,
        };

        Self {
            config,
            metadata,
            process_id: None,
            agent_state: AgentProcessState::Registered,
            lifecycle: LifecycleManager::new(),
        }
    }

    /// Transition the agent to a new management state.
    ///
    /// This transitions the internal `LifecycleManager` and updates
    /// `agent_state`. Returns `Err` if the transition is invalid.
    pub fn transition_state(
        &mut self,
        new_state: AgentProcessState,
        stage: LifecycleStage,
    ) -> Result<(), LifecycleError> {
        self.lifecycle.transition_to(stage)?;
        self.agent_state = new_state;
        Ok(())
    }

    /// Mark the agent as started with the given process ID.
    ///
    /// Updates metadata: increments `start_count`, sets `started_at`,
    /// clears `last_error`.
    pub fn mark_started(&mut self, process_id: ProcessId) {
        self.process_id = Some(process_id);
        self.agent_state = AgentProcessState::Starting;
        self.metadata.start_count += 1;
        self.metadata.started_at = Some(Utc::now());
        self.metadata.last_error = None;
    }

    /// Mark the agent as running (process confirmed alive).
    pub fn mark_running(&mut self) {
        self.agent_state = AgentProcessState::Running;
    }

    /// Mark the agent as stopped.
    ///
    /// Updates metadata: sets `stopped_at`. If the stop was due to a crash
    /// (not a requested stop), `crash_count` is incremented.
    pub fn mark_stopped(&mut self, crashed: bool, error: Option<String>) {
        self.agent_state = AgentProcessState::Stopping;
        self.metadata.stopped_at = Some(Utc::now());
        if crashed {
            self.metadata.crash_count += 1;
        }
        if let Some(err) = error {
            self.metadata.last_error = Some(err);
        }
        self.process_id = None;
    }

    /// Mark the agent's underlying process as failed (for the current spawn).
    pub fn mark_failed(&mut self, error: String) {
        self.metadata.last_error = Some(error);
    }

    /// Returns a snapshot of the agent's runtime state for IPC responses.
    ///
    /// This is a read-only, serializable view combining agent metadata
    /// with the process snapshot (if the process is managed by the supervisor).
    pub fn snapshot(&self) -> AgentSnapshot {
        AgentSnapshot {
            name: self.config.name.clone(),
            kind: self.config.kind,
            agent_state: self.agent_state,
            lifecycle_stage: self.lifecycle.stage(),
            process_id: self.process_id,
            config: self.config.clone(),
            metadata: self.metadata.clone(),
        }
    }

    /// Returns a reference to the agent's configuration.
    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    /// Returns the agent's name.
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Returns the `ProcessId` of the underlying process, if any.
    pub fn process_id(&self) -> Option<ProcessId> {
        self.process_id
    }

    /// Returns the agent's current management state.
    pub fn state(&self) -> AgentProcessState {
        self.agent_state
    }

    /// Returns `true` if the agent's process is running.
    pub fn is_running(&self) -> bool {
        self.agent_state.is_running()
    }

    /// Returns `true` if the agent is in a terminal state.
    pub fn is_stopped(&self) -> bool {
        self.agent_state.is_terminal()
    }

    /// Returns the number of times this agent has been started.
    pub fn start_count(&self) -> u32 {
        self.metadata.start_count
    }

    /// Returns the number of crashes detected.
    pub fn crash_count(&self) -> u32 {
        self.metadata.crash_count
    }

    /// Returns the last error message, if any.
    pub fn last_error(&self) -> Option<&str> {
        self.metadata.last_error.as_deref()
    }
}

/// A serializable snapshot of an agent's runtime state.
///
/// This is the public-facing type returned by `AgentManager` queries. It
/// combines agent-level metadata with the process-level snapshot (when
/// available) into a single, serializable structure suitable for IPC responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshot {
    /// The agent's name.
    pub name: String,

    /// The agent kind.
    pub kind: super::config::AgentKind,

    /// The agent's management lifecycle state.
    pub agent_state: AgentProcessState,

    /// The agent's lifecycle stage (`Created → Initialized → Running → Shutdown`).
    pub lifecycle_stage: LifecycleStage,

    /// The `ProcessId` from the `ProcessSupervisor`, if the process is managed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_id: Option<ProcessId>,

    /// The full agent configuration.
    pub config: AgentConfig,

    /// Mutable metadata tracked by the manager.
    pub metadata: AgentMetadata,

    /// A snapshot of the underlying managed process, if available.
    ///
    /// This is populated by the `AgentManager` when it queries the
    /// `ProcessSupervisor` for the process state. It is `None` before the
    /// agent is started or after it has been stopped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_snapshot: Option<ProcessSnapshot>,

    /// The underlying process state, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_state: Option<ProcessState>,
}

impl AgentSnapshot {
    /// Returns the full command line as a display string.
    pub fn command_line(&self) -> String {
        self.config.process.command_line()
    }

    /// Returns `true` if the agent's process is running.
    pub fn is_running(&self) -> bool {
        self.agent_state.is_running()
    }

    /// Returns `true` if the agent is in a terminal state.
    pub fn is_stopped(&self) -> bool {
        self.agent_state.is_terminal()
    }

    /// Returns the number of restarts the underlying process has had.
    pub fn restart_count(&self) -> u32 {
        self.process_snapshot
            .as_ref()
            .map(|s| s.restart_count)
            .unwrap_or(0)
    }

    /// Returns the PID of the underlying process, if running.
    pub fn pid(&self) -> Option<u32> {
        self.process_snapshot.as_ref().and_then(|s| s.pid)
    }

    /// Returns the exit code of the most recent process exit, if available.
    pub fn exit_code(&self) -> Option<i32> {
        self.process_snapshot.as_ref().and_then(|s| s.exit_code)
    }

    /// Returns the number of crashes detected by the agent manager.
    pub fn crash_count(&self) -> u32 {
        self.metadata.crash_count
    }

    /// Returns the number of times the agent has been started.
    pub fn start_count(&self) -> u32 {
        self.metadata.start_count
    }

    /// Returns the last error message, if any.
    pub fn last_error(&self) -> Option<&str> {
        self.metadata.last_error.as_deref()
            .or_else(|| self.process_snapshot.as_ref().and_then(|s| s.last_error.as_deref()))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process_supervisor::policy::RestartPolicy;

    #[test]
    fn new_agent_process_starts_registered() {
        let config = AgentConfig::new("test-agent", "echo");
        let proc = AgentProcess::new(config);

        assert_eq!(proc.state(), AgentProcessState::Registered);
        assert!(!proc.is_running());
        assert!(!proc.is_stopped());
        assert_eq!(proc.start_count(), 0);
        assert_eq!(proc.crash_count(), 0);
        assert!(proc.process_id().is_none());
        assert!(proc.last_error().is_none());
    }

    #[test]
    fn mark_started_sets_process_id() {
        let config = AgentConfig::new("test-agent", "echo");
        let mut proc = AgentProcess::new(config);

        let id = ProcessId::new_v4();
        proc.mark_started(id);

        assert_eq!(proc.process_id(), Some(id));
        assert_eq!(proc.start_count(), 1);
        assert!(proc.metadata.started_at.is_some());

        // Start again — count should increment
        let id2 = ProcessId::new_v4();
        proc.mark_started(id2);
        assert_eq!(proc.start_count(), 2);
    }

    #[test]
    fn mark_running_transitions_state() {
        let config = AgentConfig::new("test-agent", "echo");
        let mut proc = AgentProcess::new(config);

        let id = ProcessId::new_v4();
        proc.mark_started(id);
        proc.mark_running();

        assert!(proc.is_running());
    }

    #[test]
    fn mark_stopped_transitions_state() {
        let config = AgentConfig::new("test-agent", "echo");
        let mut proc = AgentProcess::new(config);

        proc.mark_stopped(true, Some("crash".to_string()));

        assert!(proc.is_stopped());
        assert_eq!(proc.crash_count(), 1);
        assert!(proc.last_error().is_some());
    }

    #[test]
    fn mark_stopped_non_crash() {
        let config = AgentConfig::new("test-agent", "echo");
        let mut proc = AgentProcess::new(config);

        proc.mark_stopped(false, None);

        assert!(proc.is_stopped());
        assert_eq!(proc.crash_count(), 0);
    }

    #[test]
    fn transition_state_advances_lifecycle() {
        let config = AgentConfig::new("test-agent", "echo");
        let mut proc = AgentProcess::new(config);

        assert_eq!(proc.lifecycle.stage(), LifecycleStage::Created);

        proc.transition_state(
            AgentProcessState::Starting,
            LifecycleStage::Initialized,
        ).unwrap();

        assert_eq!(proc.lifecycle.stage(), LifecycleStage::Initialized);
        assert_eq!(proc.state(), AgentProcessState::Starting);
    }

    #[test]
    fn transition_state_backward_fails() {
        let config = AgentConfig::new("test-agent", "echo");
        let mut proc = AgentProcess::new(config);

        proc.transition_state(
            AgentProcessState::Starting,
            LifecycleStage::Initialized,
        ).unwrap();

        // Cannot go backward from Initialized to Created
        assert!(proc.transition_state(
            AgentProcessState::Registered,
            LifecycleStage::Created,
        ).is_err());
    }

    #[test]
    fn snapshot_is_serializable() {
        let config = AgentConfig::new("test-agent", "echo")
            .with_args(vec!["hello".to_string()])
            .with_restart_policy(RestartPolicy::Always);
        let proc = AgentProcess::new(config);
        let snapshot = proc.snapshot();

        let json = serde_json::to_string(&snapshot).unwrap();
        let back: AgentSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(back.name, snapshot.name);
        assert_eq!(back.agent_state, snapshot.agent_state);
        assert_eq!(back.config.args, vec!["hello".to_string()]);
    }

    #[test]
    fn agent_state_labels() {
        assert_eq!(AgentProcessState::Registered.label(), "registered");
        assert_eq!(AgentProcessState::Starting.label(), "starting");
        assert_eq!(AgentProcessState::Running.label(), "running");
        assert_eq!(AgentProcessState::Stopping.label(), "stopping");
        assert_eq!(AgentProcessState::Stopped.label(), "stopped");
    }

    #[test]
    fn agent_state_serializes_to_snake_case() {
        let json = serde_json::to_string(&AgentProcessState::Running).unwrap();
        assert_eq!(json, "\"running\"");

        let json = serde_json::to_string(&AgentProcessState::Registered).unwrap();
        assert_eq!(json, "\"registered\"");
    }

    #[test]
    fn agent_state_deserializes() {
        let state: AgentProcessState = serde_json::from_str("\"stopping\"").unwrap();
        assert_eq!(state, AgentProcessState::Stopping);
    }

    #[test]
    fn agent_state_is_running_and_terminal() {
        assert!(AgentProcessState::Running.is_running());
        assert!(!AgentProcessState::Stopped.is_running());
        assert!(AgentProcessState::Stopped.is_terminal());
        assert!(!AgentProcessState::Running.is_terminal());
        assert!(!AgentProcessState::Starting.is_terminal());
    }

    #[test]
    fn agent_state_is_starting() {
        assert!(AgentProcessState::Starting.is_starting());
        assert!(!AgentProcessState::Running.is_starting());
        assert!(!AgentProcessState::Registered.is_starting());
    }

    #[test]
    fn agent_snapshot_command_line() {
        let config = AgentConfig::new("srv", "/bin/server")
            .with_args(vec!["--port".to_string(), "8080".to_string()]);
        let proc = AgentProcess::new(config);
        let snapshot = proc.snapshot();

        assert_eq!(snapshot.command_line(), "/bin/server --port 8080");
    }

    #[test]
    fn agent_snapshot_default_methods() {
        let config = AgentConfig::new("test", "echo");
        let proc = AgentProcess::new(config);
        let snapshot = proc.snapshot();

        assert_eq!(snapshot.restart_count(), 0);
        assert!(snapshot.pid().is_none());
        assert!(snapshot.exit_code().is_none());
        assert_eq!(snapshot.crash_count(), 0);
        assert_eq!(snapshot.start_count(), 0);
    }
}
