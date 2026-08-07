//! Process Configuration — describes how a subprocess should be spawned.
//!
//! [`ProcessConfig`] is the declarative specification of a managed
//! subprocess. It is serializable so that configurations can be persisted,
//! transmitted over IPC, or constructed from user input.
//!
//! The supervisor never mutates a `ProcessConfig` after spawning — the
//! config is cloned for each restart so the original configuration is
//! preserved.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::policy::RestartPolicy;

/// A unique, human-readable name for a process.
///
/// Used in event payloads, log messages, and queries. Multiple processes
/// can share the same name (e.g. multiple MCP server instances), but each
/// will have a unique [`ProcessId`](super::ProcessId).
pub type ProcessName = String;

/// Describes how a subprocess should be spawned and managed.
///
/// This is the primary input to [`ProcessSupervisor::spawn`](super::ProcessSupervisor::spawn).
/// It is `Clone` so the supervisor can retain a copy for restart decisions
/// without borrowing from the caller.
///
/// ## Serialization
///
/// All fields use `#[serde(default)]` where appropriate so that future
/// versions can add new optional fields without breaking deserialization
/// of existing configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConfig {
    /// Human-readable name for this process (e.g. `"mcp-filesystem"`,
    /// `"whisper-worker"`).
    pub name: ProcessName,

    /// The executable to spawn (resolved via `PATH` or a full path).
    pub command: String,

    /// Command-line arguments passed to the executable (after the command).
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables for the process.
    ///
    /// If empty, the child inherits the parent's environment.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Working directory for the process.
    ///
    /// If `None`, the process inherits the supervisor's working directory.
    #[serde(default)]
    pub working_dir: Option<PathBuf>,

    /// Restart policy applied when the process exits or fails.
    ///
    /// Defaults to [`RestartPolicy::OnFailure`].
    #[serde(default)]
    pub restart_policy: RestartPolicy,

    /// Whether to kill the child process when the supervisor shuts down.
    ///
    /// Defaults to `true`. When `false`, the child is left running (detached)
    /// on supervisor shutdown — useful for processes that should outlive the
    /// supervisor.
    #[serde(default = "default_kill_on_shutdown")]
    pub kill_on_shutdown: bool,

    /// Grace period (in milliseconds) before sending SIGKILL during stop.
    ///
    /// The supervisor first sends SIGKILL (via `tokio::process::Child::kill`)
    /// since it cannot send SIGTERM without additional platform-specific
    /// code. This field is reserved for future use when graceful SIGTERM
    /// support is added.
    ///
    /// Defaults to `5_000` (5 seconds).
    #[serde(default = "default_grace_period_ms")]
    pub grace_period_ms: u64,
}

fn default_kill_on_shutdown() -> bool {
    true
}

fn default_grace_period_ms() -> u64 {
    5_000
}

impl ProcessConfig {
    /// Create a new `ProcessConfig` with the given command and name.
    ///
    /// Uses [`RestartPolicy::default`] (which is `OnFailure`).
    pub fn new(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: command.into(),
            args: Vec::new(),
            env: HashMap::new(),
            working_dir: None,
            restart_policy: RestartPolicy::default(),
            kill_on_shutdown: true,
            grace_period_ms: 5_000,
        }
    }

    /// Create a `ProcessConfig` with a `Never` restart policy.
    pub fn one_shot(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            restart_policy: RestartPolicy::Never,
            ..Self::new(name, command)
        }
    }

    /// Create a `ProcessConfig` with an `Always` restart policy.
    ///
    /// Suitable for long-running daemons that should always be running.
    pub fn persistent(name: impl Into<String>, command: impl Into<String>) -> Self {
        Self {
            restart_policy: RestartPolicy::Always,
            ..Self::new(name, command)
        }
    }

    /// Set the command-line arguments.
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Add a single argument.
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Set environment variables.
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    /// Add a single environment variable.
    pub fn with_env_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set the restart policy.
    pub fn with_restart_policy(mut self, policy: RestartPolicy) -> Self {
        self.restart_policy = policy;
        self
    }

    /// Set whether to kill the process on shutdown.
    pub fn with_kill_on_shutdown(mut self, kill: bool) -> Self {
        self.kill_on_shutdown = kill;
        self
    }

    /// Set the grace period for shutdown (milliseconds).
    pub fn with_grace_period_ms(mut self, ms: u64) -> Self {
        self.grace_period_ms = ms;
        self
    }

    /// Set the working directory.
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.working_dir = Some(dir);
        self
    }

    /// Returns the full command line as a display string.
    pub fn command_line(&self) -> String {
        let mut parts = vec![self.command.clone()];
        parts.extend(self.args.iter().cloned());
        parts.join(" ")
    }
}

impl Default for ProcessConfig {
    /// Returns a config with empty name and command.
    ///
    /// Useful for testing or when the config will be fully populated via
    /// builder methods.
    fn default() -> Self {
        Self::new("", "")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn new_config_has_defaults() {
        let config = ProcessConfig::new("test", "echo");
        assert_eq!(config.name, "test");
        assert_eq!(config.command, "echo");
        assert!(config.args.is_empty());
        assert!(config.env.is_empty());
        assert_eq!(config.restart_policy, RestartPolicy::OnFailure);
        assert_eq!(config.kill_on_shutdown, true);
        assert_eq!(config.grace_period_ms, 5_000);
    }

    #[test]
    fn one_shot_uses_never_policy() {
        let config = ProcessConfig::one_shot("test", "echo");
        assert_eq!(config.restart_policy, RestartPolicy::Never);
    }

    #[test]
    fn persistent_uses_always_policy() {
        let config = ProcessConfig::persistent("test", "echo");
        assert_eq!(config.restart_policy, RestartPolicy::Always);
    }

    #[test]
    fn builder_methods_chain() {
        let config = ProcessConfig::new("test", "echo")
            .with_args(vec!["hello".to_string(), "world".to_string()])
            .with_arg("--flag".to_string())
            .with_env_var("FOO", "bar")
            .with_restart_policy(RestartPolicy::Always)
            .with_kill_on_shutdown(false)
            .with_grace_period_ms(10_000)
            .with_working_dir(PathBuf::from("/tmp"));

        assert_eq!(config.args, vec!["hello", "world", "--flag"]);
        assert_eq!(config.env.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(config.restart_policy, RestartPolicy::Always);
        assert_eq!(config.kill_on_shutdown, false);
        assert_eq!(config.grace_period_ms, 10_000);
        assert_eq!(config.working_dir, Some(PathBuf::from("/tmp")));
    }

    #[test]
    fn command_line_display() {
        let config = ProcessConfig::new("test", "echo")
            .with_args(vec!["hello".to_string(), "world".to_string()]);

        assert_eq!(config.command_line(), "echo hello world");
    }

    #[test]
    fn config_serializes_and_deserializes() {
        let config = ProcessConfig::new("mcp-server", "/usr/local/bin/mcp")
            .with_arg("--port".to_string())
            .with_arg("8080".to_string())
            .with_env_var("RUST_LOG", "debug".to_string())
            .with_restart_policy(RestartPolicy::limited_retries(3))
            .with_working_dir(PathBuf::from("/home/user"));

        let json = serde_json::to_string(&config).unwrap();
        let back: ProcessConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(back.name, config.name);
        assert_eq!(back.command, config.command);
        assert_eq!(back.args, config.args);
        assert_eq!(back.env.get("RUST_LOG"), Some(&"debug".to_string()));
        assert_eq!(back.restart_policy, config.restart_policy);
        assert_eq!(back.working_dir, config.working_dir);
    }

    #[test]
    fn config_default_is_valid() {
        let config = ProcessConfig::default();
        assert!(config.name.is_empty());
        assert!(config.command.is_empty());
        assert!(config.args.is_empty());
        assert!(config.env.is_empty());
        assert_eq!(config.restart_policy, RestartPolicy::OnFailure);
    }

    #[test]
    fn config_with_env_replaces_env() {
        let mut env = HashMap::new();
        env.insert("FOO".to_string(), "bar".to_string());

        let config = ProcessConfig::new("test", "echo").with_env(env);
        assert_eq!(config.env.len(), 1);
        assert_eq!(config.env.get("FOO"), Some(&"bar".to_string()));
    }
}
