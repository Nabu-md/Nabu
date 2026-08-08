//! # Agent Process Management — Coordinating External Agent Subprocesses
//!
//! This module provides the **AgentManager** — a reusable, higher-level
//! abstraction for managing multiple long-running external **agent processes**.
//!
//! It sits between the application layer and the
//! [`ProcessSupervisor`](crate::process_supervisor::ProcessSupervisor),
//! adding agent-level abstractions (named agents, agent metadata, lifecycle
//! tracking) on top of the supervisor's process-level supervision.
//!
//! ## Architecture
//!
//! ```text
//! Application
//!     │
//!     ▼
//! AgentManager         ← this module
//!     │
//!     ▼
//! ProcessSupervisor   (spawn/stop/restart/monitor)
//!     │
//!     ▼
//! Agent Process        (external subprocess)
//!     │
//!     ▼
//! stdin/stdout
//!     │
//!     ▼
//! JSON-RPC              (crate::rpc)
//! ```
//!
//! The AgentManager is **not** a protocol implementation. It does not implement
//! ACP, MCP, request routing, or tool calling. It manages the *process lifecycle*:
//! registration, spawning, monitoring, restart, and graceful shutdown.
//!
//! ## Key Types
//!
//! | Type | Role |
//! |------|------|
//! | [`AgentManager`] | Central coordinator — register, start, stop, restart agents. |
//! | [`AgentConfig`] | Declarative specification of an agent (command, args, restart policy, etc.). |
//! | [`AgentProcess`] | Runtime record for a single agent (metadata + ProcessId). |
//! | [`AgentRegistry`] | Thread-safe registry tracking all registered agents. |
//! | [`AgentSnapshot`] | Serializable snapshot for IPC/health queries. |
//! | [`AgentProcessState`] | Agent-level management state (Registered, Starting, Running, Stopping, Stopped). |
//! | [`AgentManagerError`] | Structured error type for all manager operations. |
//!
//! ## Lifecycle
//!
//! The `AgentManager` implements the [`Lifecycle`] trait:
//!
//! ```text
//! Created → Initialized → Running → Shutdown
//!    │          │            │         │
//!    │          │            │         └─ All agents stopped, supervisor shut down
//!    │          │            │
//!    │          │            └─ Agents can be registered and started
//!    │          │
//!    │          └─ Agent configs can be registered
//!    │
//!    └─ AgentManager created (no agents registered)
//! ```
//!
//! ## Restart Behavior
//!
//! When an agent's underlying process exits unexpectedly, the restart
//! decision is delegated to the [`ProcessSupervisor`](crate::process_supervisor::ProcessSupervisor),
//! which consults the configured [`RestartPolicy`]. The `AgentManager`
//! records the crash in the agent's metadata (`crash_count`) and publishes
//! a `AgentCrashedEvent` through the [`EventBus`](crate::event_bus::EventBus).
//!
//! ## Thread Safety
//!
//! `AgentManager` is `Send + Sync` and designed to be shared as
//! `Arc<AgentManager>` across threads. All interior mutability is protected
//! by:
//! - `RwLock` for the agent registry's process map
//! - `Mutex` for individual agent process records
//! - `AtomicU8` for lifecycle stage transitions
//! - `Arc<RwLock<>>` / `Arc<EventBus>` for the supervisor and event bus
//!
//! ## Future Compatibility
//!
//! The manager is designed to support future:
//! - ACP agents
//! - MCP servers
//! - AI provider integrations
//! - Plugin hosts
//! - OCR workers
//! - Indexing workers
//! - Synchronization services
//! - Remote agent transports
//!
//! Future agents will call `manager.register(config)` and `manager.start_agent(name)`
//! rather than managing processes directly.
//!
//! [`Lifecycle`]: crate::registry::lifecycle::Lifecycle

#![cfg(not(target_arch = "wasm32"))]

pub mod config;
pub mod errors;
pub mod manager;
pub mod process;
pub mod registry;

// Re-export public types at the module level for ergonomic access.
pub use config::{
    AgentConfig, AgentKind, AgentMetadata, AgentName, JsonRpcConfig, StdioTransportConfig,
};
pub use errors::{AgentManagerError, AgentResult};
pub use manager::{AgentManager, AgentManagerSummary};
pub use process::{AgentProcess, AgentProcessState, AgentSnapshot};
pub use registry::{AgentRegistry, RegistryError, RegistryResult};

// Re-export ProcessId from the event_bus module for convenience.
pub use crate::event_bus::ProcessId;
