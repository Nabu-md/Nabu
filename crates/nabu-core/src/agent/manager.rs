//! # Agent Manager — Coordinating Agent Process Lifecycle
//!
//! [`AgentManager`] is the reusable, higher-level coordinator for managing
//! multiple external **agent processes**. It sits between the application
//! layer and the [`ProcessSupervisor`](crate::process_supervisor::ProcessSupervisor),
//! adding agent-level abstractions (named agents, agent metadata, lifecycle
//! tracking) on top of the supervisor's process-level supervision.
//!
//! ## Architecture
//!
//! ```text
//! Application
//!     │
//!     ▼
//! AgentManager     ← this module
//!     │
//!     ▼
//! ProcessSupervisor  (spawn/stop/restart/queries)
//!     │
//!     ▼
//! Agent Process      (external subprocess)
//!     │
//!     ▼
//! stdin/stdout
//!     │
//!     ▼
//! JSON-RPC            (crate::rpc)
//! ```
//!
//! The AgentManager is **not** a protocol implementation. It does not implement
//! ACP, MCP, request routing, or tool calling. It manages the *process lifecycle*:
//! registration, spawning, monitoring, restart, and graceful shutdown.
//!
//! ## Responsibilities
//!
//! - **Registering agents** — store `AgentConfig` in a thread-safe `AgentRegistry`.
//! - **Starting agents** — delegate to `ProcessSupervisor::spawn()` to create
//!   the underlying OS process.
//! - **Stopping agents** — call `ProcessSupervisor::stop()` to terminate the process.
//! - **Restarting agents** — call `ProcessSupervisor::stop()` + `spawn()`.
//! - **Querying status** — return `AgentSnapshot` combining agent metadata
//!   with the `ProcessSupervisor`'s `ProcessSnapshot`.
//! - **Coordinating shutdown** — stop all agents and delegate to the supervisor's
//!   `shutdown()`.
//! - **Lifecycle integration** — implements the `Lifecycle` trait.
//! - **EventBus integration** — publishes agent lifecycle events.
//!
//! ## Thread Safety
//!
//! `AgentManager` is `Send + Sync` and designed to be shared as
//! `Arc<AgentManager>` across threads. The supervisor and event bus are
//! already thread-safe (`Arc<RwLock<>>` internally), and the registry
//! uses per-agent `Mutex` locks for fine-grained access.

use std::sync::Arc;

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::event_bus::{EventBus, PipelineEvent};
use crate::process_supervisor::{ProcessConfig, ProcessId, ProcessState, ProcessSupervisor};
use crate::registry::lifecycle::{
    Lifecycle, LifecycleManager, LifecycleStage,
};

use super::config::AgentConfig;
use super::errors::{AgentManagerError, AgentResult};
use super::process::{AgentProcess, AgentProcessState, AgentSnapshot};
use super::registry::AgentRegistry;

/// The fixed delay applied between detecting a process crash and when the
/// supervisor's monitoring task restarts it. The `ProcessSupervisor` already
/// applies its own `RESTART_DELAY` internally.
const MONITOR_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);

/// A reusable manager for coordinating multiple external agent processes.
///
/// `AgentManager` wraps a [`ProcessSupervisor`] and provides agent-level
/// abstractions on top of it:
///
/// - Named agents (stable identifiers across restarts)
/// - Agent metadata (kind, JSON-RPC config, transport config, lifecycle)
/// - Lifecycle integration (implements [`Lifecycle`])
/// - EventBus event publishing
///
/// The manager never executes protocol logic itself — it delegates all
/// process-level operations to the `ProcessSupervisor`.
///
/// ## Usage
///
/// ```no_run
/// use nabu_core::agent::{AgentConfig, AgentManager};
/// use nabu_core::process_supervisor::{ProcessSupervisor, RestartPolicy};
/// use nabu_core::event_bus::{EventBus, PipelineEvent};
/// use nabu_core::registry::lifecycle::Lifecycle;
/// use std::sync::Arc;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// # let event_bus = Arc::new(EventBus::new());
/// let supervisor = ProcessSupervisor::with_event_bus(event_bus.clone());
/// supervisor.initialize()?;
/// supervisor.start()?;
///
/// let manager = AgentManager::new(Arc::new(supervisor), event_bus);
/// manager.initialize()?;
/// manager.start()?;
///
/// // Register an agent
/// let config = AgentConfig::new("mcp-filesystem", "/usr/bin/mcp-filesystem")
///     .with_restart_policy(RestartPolicy::Always);
/// manager.register(config)?;
///
/// // Start the agent
/// let process_id = manager.start_agent("mcp-filesystem")?;
///
/// // Query status
/// let snapshot = manager.snapshot("mcp-filesystem")?;
/// assert!(snapshot.is_running());
///
/// // Shutdown
/// manager.shutdown()?;
/// # Ok(())
/// # }
/// ```
pub struct AgentManager {
    /// The process supervisor that manages the actual OS processes.
    supervisor: Arc<ProcessSupervisor>,

    /// The thread-safe agent registry.
    registry: Arc<AgentRegistry>,

    /// Optional EventBus for publishing agent lifecycle events.
    event_bus: Option<Arc<EventBus<PipelineEvent>>>,

    /// Lifecycle state manager.
    lifecycle: LifecycleManager,
}

impl AgentManager {
    /// Create a new `AgentManager` with the given supervisor and event bus.
    ///
    /// The manager starts in the `Created` lifecycle stage. Call
    /// [`initialize`](Self::initialize) and [`start`](Self::start) before
    /// registering or starting agents.
    pub fn new(
        supervisor: Arc<ProcessSupervisor>,
        event_bus: Arc<EventBus<PipelineEvent>>,
    ) -> Self {
        Self {
            supervisor,
            registry: Arc::new(AgentRegistry::new()),
            event_bus: Some(event_bus),
            lifecycle: LifecycleManager::new(),
        }
    }

    /// Create a new `AgentManager` without an EventBus.
    ///
    /// Use this in contexts where event publishing is not needed (e.g.
    /// unit tests without a running event bus).
    pub fn without_event_bus(supervisor: Arc<ProcessSupervisor>) -> Self {
        Self {
            supervisor,
            registry: Arc::new(AgentRegistry::new()),
            event_bus: None,
            lifecycle: LifecycleManager::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Lifecycle state accessors
    // -----------------------------------------------------------------------

    /// Returns the current lifecycle stage of the manager.
    pub fn lifecycle_stage(&self) -> LifecycleStage {
        self.lifecycle.stage()
    }

    /// Returns `true` if the manager has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.lifecycle.is_at_least(LifecycleStage::Initialized)
    }

    /// Returns `true` if the manager is running and can accept operations.
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }

    /// Returns `true` if the manager has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.lifecycle.is_shutdown()
    }

    /// Returns a reference to the underlying `ProcessSupervisor`.
    pub fn supervisor(&self) -> &Arc<ProcessSupervisor> {
        &self.supervisor
    }

    /// Returns a reference to the agent registry.
    pub fn registry(&self) -> &Arc<AgentRegistry> {
        &self.registry
    }

    /// Returns a reference to the event bus, if one is registered.
    pub fn event_bus(&self) -> Option<&Arc<EventBus<PipelineEvent>>> {
        self.event_bus.as_ref()
    }

    // -----------------------------------------------------------------------
    // Agent registration
    // -----------------------------------------------------------------------

    /// Register a new agent with the manager.
    ///
    /// The agent is registered in the `Registered` state — it has not been
    /// started yet. Call [`start_agent`](Self::start_agent) to spawn the
    /// underlying process.
    ///
    /// # Errors
    ///
    /// - [`AgentManagerError::AlreadyRegistered`] if an agent with the same
    ///   name is already registered.
    /// - [`AgentManagerError::NotReady`] if the manager is not initialized.
    pub fn register(&self, config: AgentConfig) -> AgentResult<()> {
        self.ensure_ready(LifecycleStage::Initialized)?;

        tracing::debug!(
            subsystem = "agent_manager",
            component = "manager",
            operation = "register",
            agent = %config.name,
            kind = %config.kind,
            "Registering agent"
        );

        let agent_name = config.name.clone();
        let process = AgentProcess::new(config);
        self.registry.register(process).map_err(|e| match e {
            super::registry::RegistryError::AlreadyRegistered(name) => {
                AgentManagerError::AlreadyRegistered(name)
            }
            super::registry::RegistryError::NotFound(name) => {
                AgentManagerError::AgentNotFound(name)
            }
        })?;

        tracing::info!(
            subsystem = "agent_manager",
            component = "manager",
            operation = "register",
            agent = %agent_name,
            "Agent registered"
        );

        Ok(())
    }

    /// Returns `true` if an agent with the given name is registered.
    pub fn has_agent(&self, name: &str) -> bool {
        self.registry.has(name)
    }

    /// Returns the names of all registered agents.
    pub fn agent_names(&self) -> Vec<String> {
        self.registry.names()
    }

    /// Returns the number of registered agents.
    pub fn agent_count(&self) -> usize {
        self.registry.count()
    }

    // -----------------------------------------------------------------------
    // Agent lifecycle — start / stop / restart
    // -----------------------------------------------------------------------

    /// Start a registered agent by name.
    ///
    /// This translates the `AgentConfig` into a `ProcessConfig` and delegates
    /// to the `ProcessSupervisor::spawn()` method. The agent's management
    /// state is updated to `Starting`, and the `ProcessId` returned by the
    /// supervisor is recorded in the `AgentProcess` record.
    ///
    /// # Errors
    ///
    /// - [`AgentManagerError::AgentNotFound`] if no agent with the given name
    ///   is registered.
    /// - [`AgentManagerError::NotReady`] if the manager is not running.
    /// - [`AgentManagerError::Supervisor`] if the supervisor fails to spawn
    ///   the process (e.g. executable not found).
    pub fn start_agent(&self, name: &str) -> AgentResult<ProcessId> {
        self.ensure_ready(LifecycleStage::Running)?;

        let process_handle = self
            .registry
            .get(name)
            .ok_or_else(|| AgentManagerError::AgentNotFound(name.to_string()))?;

        // Extract the config for spawning (clone to avoid holding the lock)
        let (process_config, process_id, config_clone) = {
            let proc = process_handle.lock().expect("agent process lock poisoned");
            let config: ProcessConfig = proc.config.process.clone();
            let process_id = proc.process_id;
            (config, process_id, proc.config.clone())
        };

        // Don't start if already running
        if process_id.is_some() {
            tracing::warn!(
                subsystem = "agent_manager",
                component = "manager",
                operation = "start_agent",
                agent = %name,
                "Agent is already started"
            );
            return Err(AgentManagerError::InvalidProcessState {
                name: name.to_string(),
                state: ProcessState::Running,
            });
        }

        tracing::info!(
            subsystem = "agent_manager",
            component = "manager",
            operation = "start_agent",
            agent = %name,
            command = %process_config.command_line(),
            "Starting agent"
        );

        // Delegate to the ProcessSupervisor
        let supervisor_pid = self
            .supervisor
            .spawn(process_config)
            .map_err(AgentManagerError::Supervisor)?;

        // Update the agent's state
        {
            let mut proc = process_handle.lock().expect("agent process lock poisoned");
            proc.mark_started(supervisor_pid);
        }

        // Wait for the process to enter Running state
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            if std::time::Instant::now() >= deadline {
                break;
            }
            let state = self.supervisor.get_state(supervisor_pid);
            if state.is_none_or(|s| s.is_running()) {
                {
                    let mut proc = process_handle.lock().expect("agent process lock poisoned");
                    proc.mark_running();
                }
                break;
            }
            std::thread::sleep(MONITOR_POLL_INTERVAL);
        }

        tracing::info!(
            subsystem = "agent_manager",
            component = "manager",
            operation = "start_agent",
            agent = %name,
            process_id = %supervisor_pid,
            "Agent started"
        );

        // Publish event through EventBus
        self.publish_agent_started(name, supervisor_pid, &config_clone);

        Ok(supervisor_pid)
    }

    /// Stop a running agent by name.
    ///
    /// Delegates to `ProcessSupervisor::stop()` to terminate the underlying
    /// process. The agent's management state is updated to `Stopping`.
    ///
    /// If the agent is not currently running, this is a no-op.
    ///
    /// # Errors
    ///
    /// - [`AgentManagerError::AgentNotFound`] if no agent with the given name
    ///   is registered.
    /// - [`AgentManagerError::NotReady`] if the manager is not running.
    pub fn stop_agent(&self, name: &str) -> AgentResult<()> {
        self.ensure_ready(LifecycleStage::Running)?;

        let process_handle = self
            .registry
            .get(name)
            .ok_or_else(|| AgentManagerError::AgentNotFound(name.to_string()))?;

        let (process_id, _config_name) = {
            let proc = process_handle.lock().expect("agent process lock poisoned");
            (proc.process_id, proc.config.name.clone())
        };

        if let Some(pid) = process_id {
            tracing::info!(
                subsystem = "agent_manager",
                component = "manager",
                operation = "stop_agent",
                agent = %name,
                process_id = %pid,
                "Stopping agent"
            );

            self.supervisor
                .stop(pid)
                .map_err(AgentManagerError::Supervisor)?;
        }

        // Update agent state
        {
            let mut proc = process_handle.lock().expect("agent process lock poisoned");
            proc.mark_stopped(false, None);
        }

        tracing::info!(
            subsystem = "agent_manager",
            component = "manager",
            operation = "stop_agent",
            agent = %name,
            "Agent stopped"
        );

        // Publish event
        if let Some(pid) = process_id {
            self.publish_agent_stopped(name, pid, "user requested stop");
        }

        Ok(())
    }

    /// Restart an agent by name.
    ///
    /// This stops the current process (if running) and spawns a new one using
    /// the same `AgentConfig`. The `ProcessId` changes — callers should use
    /// [`snapshot`](Self::snapshot) to get the current ID.
    ///
    /// # Errors
    ///
    /// - [`AgentManagerError::AgentNotFound`] if no agent with the given name
    ///   is registered.
    /// - [`AgentManagerError::NotReady`] if the manager is not running.
    /// - [`AgentManagerError::RestartNotApplicable`] if the agent is not in
    ///   a valid state for restart.
    pub fn restart_agent(&self, name: &str) -> AgentResult<ProcessId> {
        self.ensure_ready(LifecycleStage::Running)?;

        let process_handle = self
            .registry
            .get(name)
            .ok_or_else(|| AgentManagerError::AgentNotFound(name.to_string()))?;

        let (_current_state, process_id, config_clone) = {
            let proc = process_handle.lock().expect("agent process lock poisoned");
            (proc.state(), proc.process_id, proc.config.clone())
        };

        // Stop the current process if it's running
        if let Some(pid) = process_id {
            let _ = self.supervisor.stop(pid);

            // Wait briefly for the process to terminate
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while std::time::Instant::now() < deadline {
                if self.supervisor.get_state(pid).is_none_or(|s| s.is_terminal()) {
                    break;
                }
                std::thread::sleep(MONITOR_POLL_INTERVAL);
            }
        }

        // Clear the old process ID and mark for restart
        {
            let mut proc = process_handle.lock().expect("agent process lock poisoned");
            proc.process_id = None;
            proc.agent_state = AgentProcessState::Stopping;
            proc.metadata.stopped_at = Some(Utc::now());
        }

        // Spawn a new process
        let process_config: ProcessConfig = config_clone.process.clone();
        let supervisor_pid = self
            .supervisor
            .spawn(process_config)
            .map_err(AgentManagerError::Supervisor)?;

        // Update the agent's state
        {
            let mut proc = process_handle.lock().expect("agent process lock poisoned");
            proc.mark_started(supervisor_pid);
        }

        tracing::info!(
            subsystem = "agent_manager",
            component = "manager",
            operation = "restart_agent",
            agent = %name,
            process_id = %supervisor_pid,
            "Agent restarted"
        );

        // Publish event
        let restart_count = {
            let proc = process_handle.lock().expect("agent process lock poisoned");
            proc.metadata.start_count
        };
        self.publish_agent_restarted(name, supervisor_pid, restart_count, "user requested restart");

        Ok(supervisor_pid)
    }

    /// Restart an agent by name, waiting for the old process to terminate
    /// and the new process to start.
    ///
    /// This is an async version of [`restart_agent`](Self::restart_agent) that
    /// uses `tokio::time::sleep` instead of `std::thread::sleep` for waiting.
    /// It also waits for the new process to appear as `Running` in the
    /// supervisor.
    ///
    /// # Errors
    ///
    /// Same as [`restart_agent`](Self::restart_agent).
    pub async fn restart_agent_async(&self, name: &str) -> AgentResult<ProcessId> {
        self.ensure_ready(LifecycleStage::Running)?;

        let process_handle = self
            .registry
            .get(name)
            .ok_or_else(|| AgentManagerError::AgentNotFound(name.to_string()))?;

        // Extract config and current PID
        let (process_id, config_clone) = {
            let proc = process_handle.lock().expect("agent process lock poisoned");
            (proc.process_id, proc.config.clone())
        };

        // Stop current process if running
        if let Some(pid) = process_id {
            let _ = self.supervisor.stop(pid);
        }

        // Wait for the process to terminate (async)
        if let Some(pid) = process_id {
            let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
            loop {
                if tokio::time::Instant::now() >= deadline {
                    break;
                }
                let state = self.supervisor.get_state(pid);
                if state.is_none_or(|s| s.is_terminal()) {
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }

        // Clear old state
        {
            let mut proc = process_handle.lock().expect("agent process lock poisoned");
            proc.process_id = None;
            proc.agent_state = AgentProcessState::Stopping;
            proc.metadata.stopped_at = Some(Utc::now());
        }

        // Spawn new process
        let process_config: ProcessConfig = config_clone.process.clone();
        let supervisor_pid = self
            .supervisor
            .spawn(process_config)
            .map_err(AgentManagerError::Supervisor)?;

        // Update state
        {
            let mut proc = process_handle.lock().expect("agent process lock poisoned");
            proc.mark_started(supervisor_pid);
        }

        // Wait for the new process to enter Running state
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
        loop {
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            let state = self.supervisor.get_state(supervisor_pid);
            if state.is_none_or(|s| s.is_running()) {
                {
                    let mut proc = process_handle.lock().expect("agent process lock poisoned");
                    proc.mark_running();
                }
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }

        tracing::info!(
            subsystem = "agent_manager",
            component = "manager",
            operation = "restart_agent_async",
            agent = %name,
            process_id = %supervisor_pid,
            "Agent restarted (async)"
        );

        // Publish event
        let restart_count = {
            let proc = process_handle.lock().expect("agent process lock poisoned");
            proc.metadata.start_count
        };
        self.publish_agent_restarted(name, supervisor_pid, restart_count, "user requested restart");

        Ok(supervisor_pid)
    }

    // -----------------------------------------------------------------------
    // Agent queries
    // -----------------------------------------------------------------------

    /// Returns a snapshot of a registered agent, combining agent metadata
    /// with the process supervisor's snapshot.
    ///
    /// Returns [`AgentManagerError::AgentNotFound`] if no agent with the
    /// given name is registered.
    pub fn snapshot(&self, name: &str) -> AgentResult<AgentSnapshot> {
        let snapshot = self
            .registry
            .snapshot(name)
            .ok_or_else(|| AgentManagerError::AgentNotFound(name.to_string()))?;

        // Enrich with the process supervisor's snapshot
        let process_snapshot = snapshot.process_id.and_then(|pid| {
            self.supervisor.get_snapshot(pid)
        });

        let process_state = process_snapshot.as_ref().map(|s| s.state);

        Ok(AgentSnapshot {
            process_snapshot,
            process_state,
            ..snapshot
        })
    }

    /// Returns the current management state of a registered agent.
    ///
    /// Returns `None` if no agent with the given name is registered.
    pub fn agent_state(&self, name: &str) -> Option<AgentProcessState> {
        self.registry.state(name)
    }

    /// Returns the `ProcessId` of a registered agent's underlying process,
    /// if it has one.
    ///
    /// Returns `None` if the agent is not registered or has no process ID.
    pub fn agent_process_id(&self, name: &str) -> Option<ProcessId> {
        self.registry.process_id(name)
    }

    /// Returns `true` if the agent's process is running.
    pub fn is_agent_running(&self, name: &str) -> bool {
        self.registry
            .state(name)
            .map(|s| s.is_running())
            .unwrap_or(false)
    }

    /// Returns a list of all agent snapshots.
    pub fn all_snapshots(&self) -> Vec<AgentSnapshot> {
        let snapshots = self.registry.snapshots();
        snapshots
            .into_iter()
            .map(|mut s| {
                // Enrich with process supervisor snapshot
                s.process_snapshot = s.process_id.and_then(|pid| {
                    self.supervisor.get_snapshot(pid)
                });
                s.process_state = s.process_snapshot.as_ref().map(|ps| ps.state);
                s
            })
            .collect()
    }

    /// Returns the number of registered agents.
    pub fn registered_agent_count(&self) -> usize {
        self.registry.count()
    }

    /// Returns the number of agents currently running.
    pub fn running_agent_count(&self) -> usize {
        self.registry.running_count()
    }

    /// Returns the number of agents in stopped/stopped states.
    pub fn stopped_agent_count(&self) -> usize {
        self.registry.stopped_count()
    }

    /// Unregister an agent from the manager.
    ///
    /// If the agent's process is running, it will be stopped first.
    ///
    /// # Errors
    ///
    /// - [`AgentManagerError::AgentNotFound`] if no agent with the given name
    ///   is registered.
    pub fn unregister_agent(&self, name: &str) -> AgentResult<()> {
        self.ensure_ready(LifecycleStage::Initialized)?;

        // Get the process ID before unregistering
        let process_id = self.registry.process_id(name);

        // Stop the process if it's running
        if let Some(pid) = process_id {
            let _ = self.supervisor.stop(pid);
        }

        self.registry.unregister(name).map_err(|e| match e {
            super::registry::RegistryError::NotFound(_) => {
                AgentManagerError::AgentNotFound(name.to_string())
            }
            super::registry::RegistryError::AlreadyRegistered(_) => {
                AgentManagerError::AlreadyRegistered(name.to_string())
            }
        })?;

        tracing::info!(
            subsystem = "agent_manager",
            component = "manager",
            operation = "unregister_agent",
            agent = %name,
            "Agent unregistered"
        );

        if let Some(pid) = process_id {
            self.publish_agent_stopped(name, pid, "unregistered");
        }
        Ok(())
    }

    /// Returns a summary of the manager's current state.
    pub fn summary(&self) -> AgentManagerSummary {
        let snapshots = self.all_snapshots();
        let total = snapshots.len();
        let running = snapshots.iter().filter(|s| s.is_running()).count();
        let stopped = snapshots.iter().filter(|s| s.is_stopped()).count();

        AgentManagerSummary {
            total_agents: total,
            running_agents: running,
            stopped_agents: stopped,
            lifecycle_stage: self.lifecycle.stage(),
            supervisor_lifecycle: self.supervisor.lifecycle_stage(),
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Ensures the manager is at or past the given lifecycle stage.
    fn ensure_ready(&self, required: LifecycleStage) -> AgentResult<()> {
        let current = self.lifecycle.stage();
        if current < required {
            return Err(AgentManagerError::NotReady { current, required });
        }
        if self.lifecycle.is_shutdown() {
            return Err(AgentManagerError::ShuttingDown);
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // EventBus publishing
    // -----------------------------------------------------------------------

    fn publish_agent_started(&self, name: &str, process_id: ProcessId, config: &AgentConfig) {
        if let Some(bus) = &self.event_bus {
            let pid = self.supervisor.get_pid(process_id);
            let kind_str = config.kind.to_string();
            let event = crate::event_bus::events::AgentStartedEvent::new(
                process_id,
                name,
                &kind_str,
                pid,
            );
            bus.publish(
                crate::event_bus::kinds::AGENT_STARTED,
                &PipelineEvent::Agent(crate::event_bus::events::AgentEvent::Started(event)),
            );
        }
    }

    fn publish_agent_stopped(&self, name: &str, process_id: ProcessId, reason: &str) {
        if let Some(bus) = &self.event_bus {
            let event = crate::event_bus::events::AgentStoppedEvent::new(
                process_id,
                name,
                reason,
            );
            bus.publish(
                crate::event_bus::kinds::AGENT_STOPPED,
                &PipelineEvent::Agent(crate::event_bus::events::AgentEvent::Stopped(event)),
            );
        }
    }

    fn publish_agent_restarted(&self, name: &str, process_id: ProcessId, restart_count: u32, reason: &str) {
        if let Some(bus) = &self.event_bus {
            let event = crate::event_bus::events::AgentRestartedEvent::new(
                process_id,
                name,
                restart_count,
                reason,
            );
            bus.publish(
                crate::event_bus::kinds::AGENT_RESTARTED,
                &PipelineEvent::Agent(crate::event_bus::events::AgentEvent::Restarted(event)),
            );
        }
    }

    #[allow(dead_code)]
    fn publish_agent_crashed(&self, name: &str, process_id: ProcessId, error: String, exit_code: Option<i32>, pid: Option<u32>, restart_count: u32) {
        if let Some(bus) = &self.event_bus {
            let event = crate::event_bus::events::AgentCrashedEvent::new(
                process_id,
                name,
                exit_code,
                &error,
                pid,
                restart_count,
            );
            bus.publish(
                crate::event_bus::kinds::AGENT_CRASHED,
                &PipelineEvent::Agent(crate::event_bus::events::AgentEvent::Crashed(event)),
            );
        }
    }
}

/// A summary of the `AgentManager`'s current state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManagerSummary {
    /// Total number of registered agents.
    pub total_agents: usize,

    /// Number of agents currently running.
    pub running_agents: usize,

    /// Number of agents in stopped/stopped states.
    pub stopped_agents: usize,

    /// Current lifecycle stage of the agent manager.
    pub lifecycle_stage: LifecycleStage,

    /// Current lifecycle stage of the underlying process supervisor.
    pub supervisor_lifecycle: LifecycleStage,
}

// ---------------------------------------------------------------------------
// Lifecycle trait implementation
// ---------------------------------------------------------------------------

impl Lifecycle for AgentManager {
    fn name(&self) -> &'static str {
        "agent_manager"
    }

    /// Initialize the agent manager.
    ///
    /// Lifecycle transition: `Created → Initialized`.
    ///
    /// The supervisor must already be initialized and running for the manager
    /// to start agents.
    fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.lifecycle.is_shutdown() {
            return Err("AgentManager has been shut down".into());
        }

        tracing::info!(
            subsystem = "agent_manager",
            component = "manager",
            operation = "initialize",
            "Initializing AgentManager"
        );

        self.lifecycle
            .transition_to(LifecycleStage::Initialized)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        tracing::info!(
            subsystem = "agent_manager",
            component = "manager",
            operation = "initialize",
            "AgentManager initialized"
        );
        Ok(())
    }

    /// Start the agent manager.
    ///
    /// Lifecycle transition: `Created → Initialized → Running` (or
    /// `Initialized → Running`).
    ///
    /// After starting, the manager accepts `register`, `start_agent`,
    /// `stop_agent`, and `restart_agent` calls.
    fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.lifecycle.is_shutdown() {
            return Err("AgentManager has been shut down and cannot be started".into());
        }

        // Auto-advance Created → Initialized
        if self.lifecycle.stage() == LifecycleStage::Created {
            self.initialize()?;
        }

        self.lifecycle
            .transition_to(LifecycleStage::Running)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        tracing::info!(
            subsystem = "agent_manager",
            component = "manager",
            operation = "start",
            "AgentManager started — accepting agent registrations"
        );
        Ok(())
    }

    /// Shut down the agent manager gracefully.
    ///
    /// Lifecycle transition: `Running → Shutdown`.
    ///
    /// This stops all running agents and delegates to the underlying
    /// `ProcessSupervisor::shutdown()`.
    fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            subsystem = "agent_manager",
            component = "manager",
            operation = "shutdown",
            "Shutting down AgentManager"
        );

        // Stop all running agents
        let snapshots = self.all_snapshots();
        for snapshot in snapshots {
            if let Some(pid) = snapshot.process_id {
                let _ = self.supervisor.stop(pid);
            }

            if let Some(handle) = self.registry.get(&snapshot.name) {
                let mut proc = handle.lock().expect("agent process lock poisoned");
                proc.mark_stopped(false, Some("agent manager shutdown".to_string()));
            }
        }

        // Delegate supervisor shutdown
        self.supervisor.shutdown()?;

        // Transition lifecycle
        self.lifecycle
            .transition_to(LifecycleStage::Shutdown)
            .unwrap_or_else(|e| {
                tracing::warn!(
                    subsystem = "agent_manager",
                    component = "manager",
                    error = %e,
                    "Lifecycle transition to Shutdown failed"
                );
            });

        tracing::info!(
            subsystem = "agent_manager",
            component = "manager",
            operation = "shutdown",
            "AgentManager shut down"
        );

        Ok(())
    }
}

impl Default for AgentManager {
    fn default() -> Self {
        let supervisor = Arc::new(ProcessSupervisor::new());
        let event_bus: Arc<EventBus<PipelineEvent>> = Arc::new(EventBus::new());
        Self::new(supervisor, event_bus)
    }
}

impl std::fmt::Debug for AgentManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentManager")
            .field("agent_count", &self.registry.count())
            .field("lifecycle_stage", &self.lifecycle.stage())
            .field("supervisor_stage", &self.supervisor.lifecycle_stage())
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::EventBus;
    use crate::process_supervisor::RestartPolicy;
    use std::sync::Arc;

    fn test_manager() -> AgentManager {
        let supervisor = Arc::new(ProcessSupervisor::new());
        let event_bus: Arc<EventBus<PipelineEvent>> = Arc::new(EventBus::new());
        let manager = AgentManager::new(supervisor, event_bus);
        // Start the supervisor so spawn() works in tests
        manager.supervisor.initialize().unwrap();
        manager.supervisor.start().unwrap();
        manager
    }

    #[test]
    fn new_manager_starts_in_created() {
        let manager = test_manager();
        assert_eq!(manager.lifecycle_stage(), LifecycleStage::Created);
        assert!(!manager.is_initialized());
        assert!(!manager.is_running());
        assert!(!manager.is_shutdown());
        assert_eq!(manager.agent_count(), 0);
    }

    #[test]
    fn with_event_bus_sets_event_bus() {
        let supervisor = Arc::new(ProcessSupervisor::new());
        let bus: Arc<EventBus<PipelineEvent>> = Arc::new(EventBus::new());
        let manager = AgentManager::new(supervisor, bus.clone());
        assert!(manager.event_bus().is_some());
        assert!(Arc::ptr_eq(manager.event_bus().unwrap(), &bus));
    }

    #[test]
    fn lifecycle_transition_to_initialized() {
        let manager = test_manager();
        assert!(manager.initialize().is_ok());
        assert_eq!(manager.lifecycle_stage(), LifecycleStage::Initialized);
        assert!(manager.is_initialized());
        assert!(!manager.is_running());
    }

    #[test]
    fn lifecycle_full_flow() {
        let manager = test_manager();
        assert_eq!(manager.lifecycle_stage(), LifecycleStage::Created);

        assert!(manager.initialize().is_ok());
        assert_eq!(manager.lifecycle_stage(), LifecycleStage::Initialized);

        assert!(manager.start().is_ok());
        assert_eq!(manager.lifecycle_stage(), LifecycleStage::Running);

        assert!(manager.shutdown().is_ok());
        assert_eq!(manager.lifecycle_stage(), LifecycleStage::Shutdown);
        assert!(manager.is_shutdown());
    }

    #[test]
    fn start_auto_advances_from_created() {
        let manager = test_manager();
        assert!(manager.start().is_ok());
        assert_eq!(manager.lifecycle_stage(), LifecycleStage::Running);
    }

    #[test]
    fn start_after_shutdown_fails() {
        let manager = test_manager();
        assert!(manager.start().is_ok());
        assert!(manager.shutdown().is_ok());
        assert!(manager.start().is_err());
    }

    #[test]
    fn register_requires_initialized() {
        let manager = test_manager();
        // Not initialized yet
        let config = AgentConfig::new("test", "echo");
        let result = manager.register(config);
        assert!(matches!(
            result,
            Err(AgentManagerError::NotReady { .. })
        ));
    }

    #[test]
    fn register_and_query() {
        let manager = test_manager();
        manager.initialize().unwrap();
        manager.start().unwrap();

        let config = AgentConfig::new("test-agent", "echo")
            .with_args(vec!["hello".to_string()])
            .with_restart_policy(RestartPolicy::Never);
        manager.register(config).unwrap();

        assert_eq!(manager.agent_count(), 1);
        assert!(manager.has_agent("test-agent"));
        assert!(!manager.has_agent("other"));

        let names = manager.agent_names();
        assert!(names.contains(&"test-agent".to_string()));
    }

    #[test]
    fn register_duplicate_fails() {
        let manager = test_manager();
        manager.initialize().unwrap();
        manager.start().unwrap();

        let config = AgentConfig::new("test-agent", "echo");
        manager.register(config.clone()).unwrap();
        let result = manager.register(config);
        assert!(result.is_err());
    }

    #[test]
    fn start_nonexistent_agent_fails() {
        let manager = test_manager();
        manager.initialize().unwrap();
        manager.start().unwrap();

        let result = manager.start_agent("no-such-agent");
        assert!(matches!(result, Err(AgentManagerError::AgentNotFound(_))));
    }

    #[test]
    fn snapshot_nonexistent_agent_fails() {
        let manager = test_manager();
        manager.initialize().unwrap();
        manager.start().unwrap();

        let result = manager.snapshot("no-such-agent");
        assert!(matches!(result, Err(AgentManagerError::AgentNotFound(_))));
    }

    #[test]
    fn debug_format_works() {
        let manager = test_manager();
        let debug_str = format!("{:?}", manager);
        assert!(debug_str.contains("AgentManager"));
        assert!(debug_str.contains("agent_count"));
    }

    #[test]
    fn lifecycle_trait_name() {
        let manager = test_manager();
        let manager_ref: &dyn Lifecycle = &manager;
        assert_eq!(manager_ref.name(), "agent_manager");
    }

    #[test]
    fn summary_reflects_state() {
        let manager = test_manager();
        manager.initialize().unwrap();
        manager.start().unwrap();

        let config = AgentConfig::new("test-agent", "echo")
            .with_restart_policy(RestartPolicy::Never);
        manager.register(config).unwrap();

        let summary = manager.summary();
        assert_eq!(summary.total_agents, 1);
    }

    #[test]
    fn without_event_bus_works() {
        let supervisor = Arc::new(ProcessSupervisor::new());
        supervisor.initialize().unwrap();
        supervisor.start().unwrap();
        let manager = AgentManager::without_event_bus(supervisor);
        assert!(manager.event_bus().is_none());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_stop_agent_lifecycle() {
        let manager = test_manager();
        manager.initialize().unwrap();
        manager.start().unwrap();

        let config = AgentConfig::new("sleeper", "sleep")
            .with_arg("30".to_string())
            .with_restart_policy(RestartPolicy::Always);
        manager.register(config).unwrap();

        // Start the agent
        let _pid = manager.start_agent("sleeper").unwrap();
        assert!(manager.has_agent("sleeper"));

        // Wait for the process to start running
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let snapshot = manager.snapshot("sleeper").unwrap();
        assert!(snapshot.is_running());

        // Stop the agent
        manager.stop_agent("sleeper").unwrap();

        // Wait for the process to be stopped
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        manager.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn agent_crashes_and_restarts() {
        let manager = test_manager();
        manager.initialize().unwrap();
        manager.start().unwrap();

        let config = AgentConfig::new("failer", "sh")
            .with_args(vec!["-c".to_string(), "exit 1".to_string()])
            .with_restart_policy(RestartPolicy::Always);
        manager.register(config).unwrap();

        let _pid = manager.start_agent("failer").unwrap();

        // Wait for crash and restart cycle
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // The process should have been restarted at least once
        let snapshot = manager.snapshot("failer").unwrap();
        let restart_count = snapshot.restart_count();
        assert!(
            restart_count > 0,
            "process should have restarted (restart_count = {})",
            restart_count
        );

        manager.stop_agent("failer").ok();
        manager.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn graceful_shutdown_stops_all_agents() {
        let manager = test_manager();
        manager.initialize().unwrap();
        manager.start().unwrap();

        // Register and start multiple agents
        manager.register(
            AgentConfig::new("sleeper-1", "sleep")
                .with_arg("30".to_string())
                .with_restart_policy(RestartPolicy::Never)
        ).unwrap();
        manager.register(
            AgentConfig::new("sleeper-2", "sleep")
                .with_arg("30".to_string())
                .with_restart_policy(RestartPolicy::Never)
        ).unwrap();

        let _pid1 = manager.start_agent("sleeper-1").unwrap();
        let _pid2 = manager.start_agent("sleeper-2").unwrap();

        // Wait for them to start
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        assert_eq!(manager.running_agent_count(), 2);

        // Shutdown should stop all agents
        manager.shutdown().unwrap();

        assert!(manager.is_shutdown());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn event_bus_events_published_on_start_stop() {
        let bus: Arc<EventBus<PipelineEvent>> = Arc::new(EventBus::new());
        let supervisor = Arc::new(ProcessSupervisor::with_event_bus(bus.clone()));
        supervisor.initialize().unwrap();
        supervisor.start().unwrap();

        let manager = AgentManager::new(supervisor, bus.clone());
        manager.initialize().unwrap();
        manager.start().unwrap();

        // Subscribe to agent events
        let started_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let stopped_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

        let started_clone = started_count.clone();
        bus.subscribe(crate::event_bus::kinds::AGENT_STARTED, move |_event: &PipelineEvent| {
            started_clone.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        });

        let stopped_clone = stopped_count.clone();
        bus.subscribe(crate::event_bus::kinds::AGENT_STOPPED, move |_event: &PipelineEvent| {
            stopped_clone.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        });

        manager.register(AgentConfig::new("test", "echo").with_restart_policy(RestartPolicy::Never)).unwrap();
        manager.start_agent("test").unwrap();

        // The event should have been published synchronously
        assert!(started_count.load(std::sync::atomic::Ordering::Acquire) >= 1);

        manager.stop_agent("test").unwrap();
        assert!(stopped_count.load(std::sync::atomic::Ordering::Acquire) >= 1);

        manager.shutdown().unwrap();
    }
}
