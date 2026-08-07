//! Restart Policy — configurable restart behavior for managed subprocesses.
//!
//! [`RestartPolicy`] determines whether the
//! [`ProcessSupervisor`](super::ProcessSupervisor) should restart a managed
//! process after it exits or fails. The policy is evaluated by the
//! supervisor's monitoring task whenever a process terminates.
//!
//! ## Policies
//!
//! | Policy            | Restart behavior                         |
//! |-------------------|------------------------------------------|
//! | `Never`           | Never restart — always goes to `Stopped`.|
//! | `Always`          | Always restart, regardless of exit status.|
//! | `OnFailure`       | Restart only when the process fails       |
//! |                   | (non-zero exit code or termination by    |
//! |                   | signal). Successful exits go to `Stopped`.|
//! | `LimitedRetries`  | Like `OnFailure` but caps the number of   |
//! |                   | restarts at `max_restarts`.               |
//!
//! ## No scheduling / backoff
//!
//! This phase does **not** implement exponential backoff, jitter, or
//! scheduling delays. All restarts happen immediately after the process
//! exits. A small fixed delay (currently 100 ms) is applied between exit
//! and restart to avoid tight crash loops, but no configurable scheduling
//! is provided.
//!
//! ## Future compatibility
//!
//! The enum is `#[non_exhaustive]` so that future phases can add policies
//! (e.g. `ExponentialBackoff`, `CronSchedule`) without breaking downstream
//! consumers.

use serde::{Deserialize, Serialize};

use super::state::ProcessState;

/// Determines whether the [`ProcessSupervisor`](super::ProcessSupervisor)
/// should restart a managed process after it terminates.
///
/// Each policy is evaluated by [`RestartPolicy::should_restart`], which
/// takes the process's exit code, the number of restarts already attempted,
/// and the terminal state (`Exited` or `Failed`) to decide whether to restart.
///
/// ## Extensibility
///
/// This enum is `#[non_exhaustive]` so that future phases can add new
/// policies (e.g. `ExponentialBackoff`, `CronSchedule`) without breaking
/// existing consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum RestartPolicy {
    /// Never restart a process after it exits or fails.
    ///
    /// The process transitions to `Stopped` after `Exited` or `Failed`.
    Never,

    /// Always restart a process, regardless of exit status.
    ///
    /// Useful for long-running daemons and services that must stay alive
    /// (e.g. MCP servers, search daemons, sync services).
    Always,

    /// Restart only when the process exits unsuccessfully (non-zero exit
    /// code or terminated by a signal).
    ///
    /// A clean exit (code 0) goes to `Stopped`.
    OnFailure,

    /// Restart on failure, up to `max_restarts` times.
    ///
    /// Behaves like [`OnFailure`](RestartPolicy::OnFailure) but enforces a
    /// hard cap on the number of restart attempts. Once `max_restarts` is
    /// reached, the process transitions to `Stopped` permanently.
    LimitedRetries {
        /// Maximum number of restart attempts before giving up.
        max_restarts: u32,
    },
}

impl RestartPolicy {
    /// Creates a `Never` restart policy.
    pub fn never() -> Self {
        Self::Never
    }

    /// Creates an `Always` restart policy.
    pub fn always() -> Self {
        Self::Always
    }

    /// Creates an `OnFailure` restart policy.
    pub fn on_failure() -> Self {
        Self::OnFailure
    }

    /// Creates a `LimitedRetries` policy with the given maximum.
    pub fn limited_retries(max_restarts: u32) -> Self {
        Self::LimitedRetries { max_restarts }
    }

    /// The default restart policy used when none is specified.
    ///
    /// Defaults to `OnFailure` — the most common and safe choice for
    /// long-running services that should not restart on clean exit.
    pub fn default() -> Self {
        Self::OnFailure
    }

    /// Determines whether a process should be restarted.
    ///
    /// ## Parameters
    ///
    /// * `state` — The terminal state of the process (`Exited` or `Failed`).
    ///   Transitions to this state are the trigger for restart evaluation.
    /// * `exit_code` — The process's exit code, if available. `None`
    ///   indicates the process was terminated by a signal.
    /// * `restart_count` — How many times the supervisor has already
    ///   restarted this process.
    ///
    /// ## Returns
    ///
    /// `true` if the supervisor should attempt a restart, `false` if the
    /// process should transition to `Stopped`.
    pub fn should_restart(
        &self,
        state: ProcessState,
        exit_code: Option<i32>,
        restart_count: u32,
    ) -> bool {
        match self {
            Self::Never => false,
            Self::Always => true,
            Self::OnFailure => !is_successful_exit(state, exit_code),
            Self::LimitedRetries { max_restarts } => {
                let is_failure = !is_successful_exit(state, exit_code);
                is_failure && restart_count < *max_restarts
            }
        }
    }

    /// Returns the maximum number of restarts allowed, or `None` if
    /// unbounded.
    pub fn max_restarts(&self) -> Option<u32> {
        match self {
            Self::Never => Some(0),
            Self::Always => None,
            Self::OnFailure => None,
            Self::LimitedRetries { max_restarts } => Some(*max_restarts),
        }
    }
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self::default()
    }
}

/// Returns `true` if the process exited successfully (clean exit with code 0,
/// or a successful `Exited` state).
///
/// A process that was terminated by a signal (`exit_code == None`) or
/// exited with a non-zero code is considered to have failed.
fn is_successful_exit(state: ProcessState, exit_code: Option<i32>) -> bool {
    match state {
        ProcessState::Exited => exit_code.map_or(false, |code| code == 0),
        ProcessState::Failed => false,
        // Non-terminal states should not reach this method, but if they do,
        // treat as failure.
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_never_restarts() {
        let policy = RestartPolicy::Never;
        let result = policy.should_restart(ProcessState::Exited, Some(0), 0);
        assert!(!result);

        let result = policy.should_restart(ProcessState::Failed, None, 0);
        assert!(!result);

        let result = policy.should_restart(ProcessState::Failed, Some(1), 99);
        assert!(!result);
    }

    #[test]
    fn always_restarts_regardless() {
        let policy = RestartPolicy::Always;

        // Clean exit — still restarts
        assert!(policy.should_restart(ProcessState::Exited, Some(0), 0));
        // Failed exit — still restarts
        assert!(policy.should_restart(ProcessState::Failed, Some(1), 0));
        // Signal kill — still restarts
        assert!(policy.should_restart(ProcessState::Failed, None, 0));
        // Even after many restarts
        assert!(policy.should_restart(ProcessState::Failed, Some(1), 999));
    }

    #[test]
    fn on_failure_restarts_only_on_failure() {
        let policy = RestartPolicy::OnFailure;

        // Clean exit (code 0) → no restart
        assert!(!policy.should_restart(ProcessState::Exited, Some(0), 0));

        // Non-zero exit → restart
        assert!(policy.should_restart(ProcessState::Exited, Some(1), 0));

        // Failed state → restart
        assert!(policy.should_restart(ProcessState::Failed, Some(1), 0));

        // Signal termination → restart
        assert!(policy.should_restart(ProcessState::Failed, None, 0));

        // Still restarts regardless of count
        assert!(policy.should_restart(ProcessState::Failed, Some(1), 999));
    }

    #[test]
    fn limited_retries_restarts_on_failure_up_to_limit() {
        let policy = RestartPolicy::limited_retries(3);

        // Clean exit → no restart
        assert!(!policy.should_restart(ProcessState::Exited, Some(0), 0));

        // First failure → restart
        assert!(policy.should_restart(ProcessState::Failed, Some(1), 0));

        // Second failure → restart
        assert!(policy.should_restart(ProcessState::Failed, Some(1), 1));

        // Third failure → restart
        assert!(policy.should_restart(ProcessState::Failed, Some(1), 2));

        // Fourth failure (restart_count == 3 == max) → no restart
        assert!(!policy.should_restart(ProcessState::Failed, Some(1), 3));

        // Past limit → no restart
        assert!(!policy.should_restart(ProcessState::Failed, Some(1), 99));
    }

    #[test]
    fn limited_retries_restarts_on_signal_failure() {
        let policy = RestartPolicy::limited_retries(2);

        // Signal termination (no exit code) → treated as failure → restart
        assert!(policy.should_restart(ProcessState::Failed, None, 0));
        assert!(policy.should_restart(ProcessState::Failed, None, 1));
        // At limit → no restart
        assert!(!policy.should_restart(ProcessState::Failed, None, 2));
    }

    #[test]
    fn max_restarts_returns_correct_limits() {
        assert_eq!(RestartPolicy::Never.max_restarts(), Some(0));
        assert_eq!(RestartPolicy::Always.max_restarts(), None);
        assert_eq!(RestartPolicy::OnFailure.max_restarts(), None);
        assert_eq!(
            RestartPolicy::limited_retries(5).max_restarts(),
            Some(5)
        );
    }

    #[test]
    fn default_is_on_failure() {
        assert_eq!(RestartPolicy::default(), RestartPolicy::OnFailure);
    }

    #[test]
    fn policy_serializes_correctly() {
        let json = serde_json::to_string(&RestartPolicy::Never).unwrap();
        assert!(json.contains("never"));

        let json = serde_json::to_string(&RestartPolicy::Always).unwrap();
        assert!(json.contains("always"));

        let json = serde_json::to_string(&RestartPolicy::OnFailure).unwrap();
        assert!(json.contains("on_failure"));

        let json = serde_json::to_string(&RestartPolicy::limited_retries(3)).unwrap();
        let back: RestartPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(back, RestartPolicy::limited_retries(3));
    }

    #[test]
    fn is_successful_exit_logic() {
        // Exited with code 0 → success
        assert!(is_successful_exit(ProcessState::Exited, Some(0)));
        // Exited with non-zero → failure
        assert!(!is_successful_exit(ProcessState::Exited, Some(1)));
        // Exited with None (signal) → failure
        assert!(!is_successful_exit(ProcessState::Exited, None));
        // Failed → failure
        assert!(!is_successful_exit(ProcessState::Failed, Some(0)));
        assert!(!is_successful_exit(ProcessState::Failed, None));
        // Non-terminal → failure (shouldn't normally be called)
        assert!(!is_successful_exit(ProcessState::Running, Some(0)));
    }
}
