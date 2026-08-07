//! Process State — the strongly typed lifecycle model for managed subprocesses.
//!
//! [`ProcessState`] represents the observable state of a single managed
//! process as it moves through its lifecycle under the
//! [`ProcessSupervisor`](super::ProcessSupervisor). The model is a finite
//! state machine with deterministic, validated transitions.
//!
//! ## Lifecycle
//!
//! ```text
//! Created → Starting → Running → Exited / Failed → ──(restart?)──→ Restarting → Starting
//!                          │                                 │
//!                          └─(no restart)→ Stopped ←────────┘
//! Running → Stopping → Stopped
//! ```
//!
//! The supervisor never allows invalid transitions — every state change is
//! validated by [`ProcessState::can_transition`].
//!
//! ## Extensibility
//!
//! The enum is `#[non_exhaustive]`, allowing future states (e.g.
//! `Paused`, `Suspended`) to be added without breaking downstream consumers.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The lifecycle state of a managed subprocess.
///
/// Each process supervised by [`ProcessSupervisor`](super::ProcessSupervisor)
/// moves through a deterministic set of states. Not every transition is
/// valid — use [`ProcessState::can_transition`] to validate before applying.
///
/// ## Extensibility
///
/// This enum is `#[non_exhaustive]` so that future phases can add states
/// (e.g. `Paused`, `Suspended`) without breaking downstream consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ProcessState {
    /// The process has been configured but not yet spawned.
    ///
    /// This is the initial state when a [`ProcessConfig`](super::ProcessConfig)
    /// is registered with the supervisor.
    Created,

    /// The process is being spawned — the supervisor has invoked `spawn()`
    /// but the OS has not yet confirmed startup.
    Starting,

    /// The process is running.
    Running,

    /// The process is being stopped — a stop signal has been sent and the
    /// supervisor is waiting for it to terminate.
    Stopping,

    /// The process has been stopped (gracefully) and will not be restarted
    /// without explicit action.
    ///
    /// This is a terminal state.
    Stopped,

    /// The process is in the process of being restarted.
    ///
    /// The supervisor has decided to restart the process (based on the
    /// [`RestartPolicy`](super::RestartPolicy)) and is preparing the new
    /// spawn.
    Restarting,

    /// The process exited or failed and may be restarted depending on the
    /// restart policy.
    ///
    /// This is a transient state — the supervisor uses it to evaluate the
    /// restart policy and decide between [`Restarting`] and [`Stopped`].
    ///
    /// [`Restarting`]: ProcessState::Restarting
    /// [`Stopped`]: ProcessState::Stopped
    Failed,

    /// The process has exited.
    ///
    /// This is a transient state — the supervisor uses it to evaluate the
    /// restart policy and decide between [`Restarting`] and [`Stopped`].
    ///
    /// [`Restarting`]: ProcessState::Restarting
    /// [`Stopped`]: ProcessState::Stopped
    Exited,
}

impl ProcessState {
    /// Returns `true` if a transition from `from` to `to` is valid.
    ///
    /// Staying in the same state is always valid (no-op). All other
    /// transitions are checked against the state machine:
    ///
    /// ```text
    /// Created → Starting
    /// Starting → Running | Failed
    /// Running → Exited   | Failed | Stopping
    /// Stopping → Stopped
    /// Exited  → Restarting | Stopped
    /// Failed  → Restarting | Stopped
    /// Restarting → Starting
    /// Stopped → (terminal — nothing out)
    /// ```
    pub fn can_transition(from: Self, to: Self) -> bool {
        if from == to {
            return true;
        }
        match (from, to) {
            // Lifecycle forward
            (Self::Created, Self::Starting) => true,
            (Self::Starting, Self::Running) | (Self::Starting, Self::Failed) => true,
            (Self::Running, Self::Exited)
            | (Self::Running, Self::Failed)
            | (Self::Running, Self::Stopping) => true,
            (Self::Stopping, Self::Stopped) => true,
            // Restart decision
            (Self::Exited, Self::Restarting) | (Self::Exited, Self::Stopped) => true,
            (Self::Failed, Self::Restarting) | (Self::Failed, Self::Stopped) => true,
            // Restart → back to Starting
            (Self::Restarting, Self::Starting) => true,
            _ => false,
        }
    }

    /// Returns `true` if the process is currently running.
    pub fn is_running(&self) -> bool {
        *self == Self::Running
    }

    /// Returns `true` if the process is in a terminal state
    /// (will not be restarted or resumed without explicit action).
    pub fn is_terminal(&self) -> bool {
        *self == Self::Stopped
    }

    /// Returns `true` if the process has exited or failed (but may still
    /// be restarted by the supervisor).
    pub fn is_exited_or_failed(&self) -> bool {
        matches!(self, ProcessState::Exited | ProcessState::Failed)
    }

    /// Returns `true` if the process is in the process of starting or
    /// restarting.
    pub fn is_starting(&self) -> bool {
        matches!(self, ProcessState::Starting | ProcessState::Restarting)
    }

    /// Returns a human-readable label for this state.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Restarting => "restarting",
            Self::Failed => "failed",
            Self::Exited => "exited",
        }
    }
}

impl fmt::Display for ProcessState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

impl Default for ProcessState {
    /// The initial state of any new managed process.
    fn default() -> Self {
        Self::Created
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_created() {
        assert_eq!(ProcessState::default(), ProcessState::Created);
    }

    #[test]
    fn all_forward_transitions_valid() {
        // Created → Starting
        assert!(ProcessState::can_transition(
            ProcessState::Created,
            ProcessState::Starting
        ));

        // Starting → Running | Failed
        assert!(ProcessState::can_transition(
            ProcessState::Starting,
            ProcessState::Running
        ));
        assert!(ProcessState::can_transition(
            ProcessState::Starting,
            ProcessState::Failed
        ));

        // Running → Exited | Failed | Stopping
        assert!(ProcessState::can_transition(
            ProcessState::Running,
            ProcessState::Exited
        ));
        assert!(ProcessState::can_transition(
            ProcessState::Running,
            ProcessState::Failed
        ));
        assert!(ProcessState::can_transition(
            ProcessState::Running,
            ProcessState::Stopping
        ));

        // Stopping → Stopped
        assert!(ProcessState::can_transition(
            ProcessState::Stopping,
            ProcessState::Stopped
        ));

        // Exited → Restarting | Stopped
        assert!(ProcessState::can_transition(
            ProcessState::Exited,
            ProcessState::Restarting
        ));
        assert!(ProcessState::can_transition(
            ProcessState::Exited,
            ProcessState::Stopped
        ));

        // Failed → Restarting | Stopped
        assert!(ProcessState::can_transition(
            ProcessState::Failed,
            ProcessState::Restarting
        ));
        assert!(ProcessState::can_transition(
            ProcessState::Failed,
            ProcessState::Stopped
        ));

        // Restarting → Starting
        assert!(ProcessState::can_transition(
            ProcessState::Restarting,
            ProcessState::Starting
        ));
    }

    #[test]
    fn same_state_transition_is_valid() {
        for state in [
            ProcessState::Created,
            ProcessState::Starting,
            ProcessState::Running,
            ProcessState::Stopping,
            ProcessState::Stopped,
            ProcessState::Restarting,
            ProcessState::Failed,
            ProcessState::Exited,
        ] {
            assert!(
                ProcessState::can_transition(state, state),
                "same-state transition should be valid for {:?}",
                state
            );
        }
    }

    #[test]
    fn backward_transitions_invalid() {
        // Running → Created (backward)
        assert!(!ProcessState::can_transition(
            ProcessState::Running,
            ProcessState::Created
        ));
        // Running → Starting (backward-ish)
        assert!(!ProcessState::can_transition(
            ProcessState::Running,
            ProcessState::Starting
        ));
        // Starting → Created (backward)
        assert!(!ProcessState::can_transition(
            ProcessState::Starting,
            ProcessState::Created
        ));
        // Stopped → anything (terminal)
        assert!(!ProcessState::can_transition(
            ProcessState::Stopped,
            ProcessState::Running
        ));
    }

    #[test]
    fn skipped_transitions_invalid() {
        // Created → Running (skip Starting)
        assert!(!ProcessState::can_transition(
            ProcessState::Created,
            ProcessState::Running
        ));
        // Created → Exited (skip everything)
        assert!(!ProcessState::can_transition(
            ProcessState::Created,
            ProcessState::Exited
        ));
        // Running → Stopped (skip Stopping)
        assert!(!ProcessState::can_transition(
            ProcessState::Running,
            ProcessState::Stopped
        ));
        // Exited → Running (skip Restarting/Starting)
        assert!(!ProcessState::can_transition(
            ProcessState::Exited,
            ProcessState::Running
        ));
    }

    #[test]
    fn state_labels() {
        assert_eq!(ProcessState::Created.label(), "created");
        assert_eq!(ProcessState::Starting.label(), "starting");
        assert_eq!(ProcessState::Running.label(), "running");
        assert_eq!(ProcessState::Stopping.label(), "stopping");
        assert_eq!(ProcessState::Stopped.label(), "stopped");
        assert_eq!(ProcessState::Restarting.label(), "restarting");
        assert_eq!(ProcessState::Failed.label(), "failed");
        assert_eq!(ProcessState::Exited.label(), "exited");
    }

    #[test]
    fn is_running_helper() {
        assert!(ProcessState::Running.is_running());
        assert!(!ProcessState::Stopped.is_running());
        assert!(!ProcessState::Starting.is_running());
    }

    #[test]
    fn is_terminal_helper() {
        assert!(ProcessState::Stopped.is_terminal());
        assert!(!ProcessState::Exited.is_terminal());
        assert!(!ProcessState::Failed.is_terminal());
        assert!(!ProcessState::Running.is_terminal());
    }

    #[test]
    fn is_exited_or_failed_helper() {
        assert!(ProcessState::Exited.is_exited_or_failed());
        assert!(ProcessState::Failed.is_exited_or_failed());
        assert!(!ProcessState::Stopped.is_exited_or_failed());
        assert!(!ProcessState::Running.is_exited_or_failed());
    }

    #[test]
    fn is_starting_helper() {
        assert!(ProcessState::Starting.is_starting());
        assert!(ProcessState::Restarting.is_starting());
        assert!(!ProcessState::Created.is_starting());
        assert!(!ProcessState::Running.is_starting());
    }

    #[test]
    fn state_serializes_to_snake_case() {
        let json = serde_json::to_string(&ProcessState::Created).unwrap();
        assert_eq!(json, "\"created\"");
    }

    #[test]
    fn state_deserializes_from_snake_case() {
        let state: ProcessState = serde_json::from_str("\"running\"").unwrap();
        assert_eq!(state, ProcessState::Running);
    }
}
