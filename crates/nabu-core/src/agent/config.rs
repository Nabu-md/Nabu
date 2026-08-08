//! # Agent Configuration
//!
//! [`AgentConfig`] describes a managed **agent process** — an external
//! subprocess that communicates over stdin/stdout using JSON-RPC.
//!
//! An `AgentConfig` is *declarative*: it specifies what to spawn and how the
//! supervisor should treat it, but does not perform any I/O itself. The
//! [`AgentManager`](super::AgentManager) translates an `AgentConfig` into a
//! `ProcessConfig` before delegating to the `ProcessSupervisor`.
//!
//! ## Relationship to ProcessConfig
//!
//! `AgentConfig` is a superset of [`ProcessConfig`](crate::process_supervisor::ProcessConfig).
//! It adds agent-specific metadata (agent type, JSON-RPC router name, transport
//! configuration) while embedding the process configuration as a field.
//!
//! ```text
//! AgentConfig
//! ├── name:          AgentName    (logical name, e.g. "mcp-filesystem")
//! ├── kind:          AgentKind    (ACP | MCP | Plugin | Custom)
//! ├── process:       ProcessConfig (command, args, env, restart_policy)
//! ├── transport:     StdioTransportConfig (optional overrides)
//! └── jsonrpc:       JsonRpcConfig (optional method allow-list)
//! ```

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::process_supervisor::{ProcessConfig, RestartPolicy};
use crate::registry::lifecycle::LifecycleStage;

/// A stable, human-readable identifier for a managed agent.
///
/// This is the primary key used by the [`AgentManager`](super::AgentManager)
/// when registering, starting, and querying agents. Unlike a [`ProcessId`]
/// (which is a UUID assigned per-process spawn), an `AgentName` is stable
/// across restarts and identifies the *agent definition*, not a particular
/// running instance.
pub type AgentName = String;

/// The category of an agent, used for filtering and routing.
///
/// This enum is `#[non_exhaustive]` so that future phases can add agent
/// types (e.g. `ACP`, `MCP`) without breaking downstream consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum AgentKind {
    /// A generic external process managed by the AgentManager.
    ///
    /// This is the default — used for any subprocess that does not fit
    /// a more specific category.
    Custom,

    /// A Language Server Protocol (LSP) server or similar
    /// stdin/stdout-based service.
    LspServer,

    /// A search or indexing daemon.
    SearchDaemon,

    /// An OCR worker process.
    OcrWorker,

    /// A synchronization service.
    SyncService,

    /// An embedding generation service.
    EmbeddingProvider,

    /// A file system or tooling MCP server.
    ToolServer,
}

impl Default for AgentKind {
    fn default() -> Self {
        Self::Custom
    }
}

impl std::fmt::Display for AgentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Custom => write!(f, "custom"),
            Self::LspServer => write!(f, "lsp_server"),
            Self::SearchDaemon => write!(f, "search_daemon"),
            Self::OcrWorker => write!(f, "ocr_worker"),
            Self::SyncService => write!(f, "sync_service"),
            Self::EmbeddingProvider => write!(f, "embedding_provider"),
            Self::ToolServer => write!(f, "tool_server"),
        }
    }
}

/// Optional JSON-RPC configuration for an agent.
///
/// This allows the AgentManager to be aware of what JSON-RPC methods an
/// agent supports, primarily for documentation and health-check purposes.
/// The AgentManager itself does **not** implement protocol negotiation,
/// request routing, or method dispatch — those are the responsibility of
/// future protocol implementations (ACP, MCP, etc.).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct JsonRpcConfig {
    /// The JSON-RPC protocol version the agent speaks.
    ///
    /// Defaults to `"2.0"` if not specified.
    #[serde(default = "json_rpc_version_default")]
    pub version: String,

    /// Whether the agent expects batched requests.
    #[serde(default)]
    pub supports_batching: bool,

    /// The set of method names the agent can handle.
    ///
    /// This is informational — the AgentManager does not validate or route
    /// requests based on this list. Future protocol layers may use it for
    /// capability discovery.
    #[serde(default)]
    pub supported_methods: Vec<String>,

    /// Optional timeout for JSON-RPC method calls (in milliseconds).
    #[serde(default)]
    pub request_timeout_ms: Option<u64>,
}

fn json_rpc_version_default() -> String {
    "2.0".to_string()
}

/// Optional overrides for the stdio transport used with an agent.
///
/// If `None`, the default transport configuration is used.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StdioTransportConfig {
    /// Maximum line length in bytes before the reader returns an error.
    pub max_message_bytes: Option<usize>,

    /// How long the reader waits for stdin data before checking the shutdown
    /// signal (milliseconds).
    pub read_poll_interval_ms: Option<u64>,

    /// Shutdown timeout (milliseconds).
    pub shutdown_timeout_ms: Option<u64>,

    /// Whether to flush stdout after every response.
    pub flush_after_write: Option<bool>,
}

/// Describes a managed agent process.
///
/// `AgentConfig` is the primary input to
/// [`AgentManager::register`](super::AgentManager::register).
/// It encapsulates everything needed to spawn and supervise an external
/// agent process:
///
/// - Process specification (executable, args, env, working directory)
/// - Restart behavior (via [`RestartPolicy`])
/// - Agent categorization (via [`AgentKind`])
/// - Optional JSON-RPC and transport hints
///
/// The config is `Clone` so the manager can retain a copy for restarts
/// without borrowing from the caller.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// The agent's name — a stable, human-readable identifier used as the
    /// primary key in the agent registry.
    pub name: AgentName,

    /// The kind/category of agent (Custom, LSP, SearchDaemon, etc.).
    #[serde(default)]
    pub kind: AgentKind,

    /// The process configuration — executable, args, env, working dir,
    /// restart policy.
    pub process: ProcessConfig,

    /// Optional JSON-RPC configuration hints.
    #[serde(default)]
    pub jsonrpc: Option<JsonRpcConfig>,

    /// Optional stdio transport overrides.
    #[serde(default)]
    pub transport: Option<StdioTransportConfig>,
}

impl AgentConfig {
    /// Create a new `AgentConfig` with the given agent name and process command.
    ///
    /// Uses the default [`RestartPolicy`] (which is [`RestartPolicy::OnFailure`]).
    pub fn new(name: impl Into<AgentName>, command: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: AgentKind::default(),
            process: ProcessConfig::new(name.into(), command),
            jsonrpc: None,
            transport: None,
        }
    }

    /// Create a config for a long-running daemon process that should always
    /// be running.
    pub fn persistent(name: impl Into<AgentName>, command: impl Into<String>) -> Self {
        let name = name.into();
        let process = ProcessConfig::persistent(name.clone(), command);
        Self {
            name: name.clone(),
            kind: AgentKind::default(),
            process,
            jsonrpc: None,
            transport: None,
        }
    }

    /// Create a config for a one-shot process (no restarts).
    pub fn one_shot(name: impl Into<AgentName>, command: impl Into<String>) -> Self {
        let name = name.into();
        let process = ProcessConfig::one_shot(name.clone(), command);
        Self {
            name: name.clone(),
            kind: AgentKind::default(),
            process,
            jsonrpc: None,
            transport: None,
        }
    }

    /// Set the agent kind.
    pub fn with_kind(mut self, kind: AgentKind) -> Self {
        self.kind = kind;
        self
    }

    /// Set command-line arguments on the underlying process config.
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.process.args = args;
        self
    }

    /// Add a single command-line argument.
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.process.args.push(arg.into());
        self
    }

    /// Set environment variables on the underlying process config.
    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.process.env = env;
        self
    }

    /// Add a single environment variable.
    pub fn with_env_var(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.process.env.insert(key.into(), value.into());
        self
    }

    /// Set the restart policy.
    pub fn with_restart_policy(mut self, policy: RestartPolicy) -> Self {
        self.process.restart_policy = policy;
        self
    }

    /// Set whether to kill the process on shutdown.
    pub fn with_kill_on_shutdown(mut self, kill: bool) -> Self {
        self.process.kill_on_shutdown = kill;
        self
    }

    /// Set the grace period (milliseconds) before SIGKILL during stop.
    pub fn with_grace_period_ms(mut self, ms: u64) -> Self {
        self.process.grace_period_ms = ms;
        self
    }

    /// Set the working directory.
    pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
        self.process.working_dir = Some(dir);
        self
    }

    /// Set JSON-RPC configuration hints.
    pub fn with_jsonrpc(mut self, jsonrpc: JsonRpcConfig) -> Self {
        self.jsonrpc = Some(jsonrpc);
        self
    }

    /// Set stdio transport overrides.
    pub fn with_transport(mut self, transport: StdioTransportConfig) -> Self {
        self.transport = Some(transport);
        self
    }
}

/// Metadata about a registered agent, tracked by the [`AgentManager`](super::AgentManager).
///
/// This is separate from the runtime `ProcessSnapshot` — it describes the
/// *agent definition* rather than a particular running instance. The `lifecycle_stage`
/// field tracks the agent's management state within the AgentManager (not the
/// underlying process state, which is tracked separately by the supervisor).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMetadata {
    /// The agent's stable name (same as `AgentConfig::name`).
    pub name: AgentName,

    /// The agent kind.
    pub kind: AgentKind,

    /// JSON-RPC configuration hints, if any.
    #[serde(default)]
    pub jsonrpc: Option<JsonRpcConfig>,

    /// Stdio transport overrides, if any.
    #[serde(default)]
    pub transport: Option<StdioTransportConfig>,

    /// The management lifecycle stage of this agent definition.
    #[serde(default)]
    pub lifecycle_stage: LifecycleStage,

    /// When the agent was first registered.
    pub registered_at: chrono::DateTime<chrono::Utc>,

    /// When the agent was last started (if running).
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,

    /// When the agent was last stopped (if stopped/exited).
    pub stopped_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Number of times the agent has been started.
    pub start_count: u32,

    /// Number of times the agent has crashed and been restarted.
    pub crash_count: u32,

    /// Last error, if any.
    pub last_error: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_config_has_defaults() {
        let config = AgentConfig::new("test-agent", "echo");
        assert_eq!(config.name, "test-agent");
        assert_eq!(config.kind, AgentKind::Custom);
        assert_eq!(config.process.name, "test-agent");
        assert_eq!(config.process.command, "echo");
        assert_eq!(config.process.restart_policy, RestartPolicy::OnFailure);
        assert!(config.jsonrpc.is_none());
        assert!(config.transport.is_none());
    }

    #[test]
    fn persistent_uses_always_policy() {
        let config = AgentConfig::persistent("test", "echo");
        assert_eq!(config.process.restart_policy, RestartPolicy::Always);
    }

    #[test]
    fn one_shot_uses_never_policy() {
        let config = AgentConfig::one_shot("test", "echo");
        assert_eq!(config.process.restart_policy, RestartPolicy::Never);
    }

    #[test]
    fn builder_methods_chain() {
        let jsonrpc = JsonRpcConfig {
            version: "2.0".to_string(),
            supports_batching: true,
            supported_methods: vec!["ping".to_string()],
            request_timeout_ms: Some(5000),
        };

        let config = AgentConfig::new("test", "echo")
            .with_kind(AgentKind::LspServer)
            .with_args(vec!["hello".to_string()])
            .with_arg("--flag".to_string())
            .with_env_var("FOO", "bar")
            .with_restart_policy(RestartPolicy::Always)
            .with_kill_on_shutdown(false)
            .with_grace_period_ms(10_000)
            .with_working_dir(PathBuf::from("/tmp"))
            .with_jsonrpc(jsonrpc);

        assert_eq!(config.kind, AgentKind::LspServer);
        assert_eq!(config.process.args, vec!["hello", "--flag"]);
        assert_eq!(config.process.env.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(config.process.restart_policy, RestartPolicy::Always);
        assert_eq!(config.process.kill_on_shutdown, false);
        assert_eq!(config.process.grace_period_ms, 10_000);
        assert_eq!(config.process.working_dir, Some(PathBuf::from("/tmp")));
        assert!(config.jsonrpc.is_some());
        assert_eq!(config.jsonrpc.as_ref().unwrap().version, "2.0");
    }

    #[test]
    fn config_serializes_and_deserializes() {
        let config = AgentConfig::new("mcp-server", "/usr/local/bin/mcp")
            .with_kind(AgentKind::ToolServer)
            .with_args(vec!["--port".to_string(), "8080".to_string()])
            .with_env_var("RUST_LOG", "debug".to_string())
            .with_restart_policy(RestartPolicy::limited_retries(3))
            .with_working_dir(PathBuf::from("/home/user"));

        let json = serde_json::to_string(&config).unwrap();
        let back: AgentConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(back.name, config.name);
        assert_eq!(back.kind, config.kind);
        assert_eq!(back.process.command, config.process.command);
        assert_eq!(back.process.args, config.process.args);
        assert_eq!(back.process.env.get("RUST_LOG"), Some(&"debug".to_string()));
        assert_eq!(back.process.restart_policy, config.process.restart_policy);
    }

    #[test]
    fn agent_kind_serializes_to_snake_case() {
        let json = serde_json::to_string(&AgentKind::LspServer).unwrap();
        assert_eq!(json, "\"lsp_server\"");
    }

    #[test]
    fn agent_kind_deserializes() {
        let kind: AgentKind = serde_json::from_str("\"search_daemon\"").unwrap();
        assert_eq!(kind, AgentKind::SearchDaemon);
    }

    #[test]
    fn jsonrpc_config_default() {
        let config = JsonRpcConfig::default();
        assert_eq!(config.version, "2.0");
        assert!(!config.supports_batching);
        assert!(config.supported_methods.is_empty());
        assert!(config.request_timeout_ms.is_none());
    }

    #[test]
    fn stdio_transport_config_default() {
        let config = StdioTransportConfig::default();
        assert!(config.max_message_bytes.is_none());
        assert!(config.read_poll_interval_ms.is_none());
        assert!(config.shutdown_timeout_ms.is_none());
        assert!(config.flush_after_write.is_none());
    }

    #[test]
    fn transport_config_serializes() {
        let config = StdioTransportConfig {
            max_message_bytes: Some(1024),
            read_poll_interval_ms: Some(100),
            shutdown_timeout_ms: Some(5000),
            flush_after_write: Some(true),
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: StdioTransportConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.max_message_bytes, Some(1024));
        assert_eq!(back.read_poll_interval_ms, Some(100));
        assert_eq!(back.shutdown_timeout_ms, Some(5000));
        assert_eq!(back.flush_after_write, Some(true));
    }
}
