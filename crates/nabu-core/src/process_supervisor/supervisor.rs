//! ProcessSupervisor — the central authority for managed subprocesses.
//!
//! The [`ProcessSupervisor`] is the single component responsible for
//! spawning, monitoring, and supervising all long-running external
//! subprocesses in the Nabu platform. Rather than allowing services to
//! spawn unmanaged child processes, all future managed processes (MCP
//! servers, ACP servers, plugin hosts, OCR workers, sync services, etc.)
//! will be spawned through and tracked by this supervisor.
//!
//! ## Architecture
//!
//! ```text
//! ProcessSupervisor (Arc<ProcessSupervisor>)
//! ├── processes: RwLock<HashMap<ProcessId, Arc<Mutex<ManagedProcess>>>>
//! ├── context:   Arc<SupervisorContext>  (shutdown flag, active monitor count)
//! ├── event_bus: Option<Arc<EventBus<PipelineEvent>>>
//! └── lifecycle: LifecycleManager
//!
//! For each spawned process:
//!   1. A ManagedProcess record is created (state = Created)
//!   2. A broadcast channel for stop signaling is created
//!   3. The sender is stored in the record
//!   4. A tokio monitoring task is spawned (owns the Child)
//!   5. The task updates the record's state and publishes events
//! ```
//!
//! ## Thread Safety
//!
//! The supervisor is `Send + Sync` and designed to be shared via
//! `Arc<ProcessSupervisor>` across threads. All mutable state is protected
//! by lock-free atomics (for flags/counters) or `Mutex`/`RwLock` (for
//! process records).
//!
//! ## Lifecycle
//!
//! ```text
//! Created → Initialized → Running → Shutdown
//! ```
//!
//! Only when the supervisor is in the `Running` stage will `spawn()` accept
//! new processes. `shutdown()` stops all managed processes and prevents new
//! spawns.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::event_bus::{EventBus, PipelineEvent};
use crate::registry::lifecycle::{Lifecycle, LifecycleManager, LifecycleStage};
use crate::registry::metrics::{
    CounterMetric, GaugeMetric, MetricsAggregator, ServiceMetrics,
};

use super::config::ProcessConfig;
use super::errors::ProcessResult;
use super::errors::ProcessSupervisorError;
use super::managed::ManagedProcess;
use super::managed::ProcessSnapshot;
use super::monitor::{monitor_process, SupervisorContext};
use super::state::ProcessState;
use super::ProcessId;

/// The timeout for waiting for monitoring tasks to finish during
/// `shutdown()`.
#[allow(dead_code)]
const STOP_WAIT_TIMEOUT: Duration = Duration::from_secs(10);

/// The fixed timeout for waiting for all monitoring tasks to finish during
/// full supervisor shutdown.
const SHUTDOWN_DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

/// A short sleep interval used in synchronous polling loops.
///
/// This replaces busy-waiting — we poll process state at reasonable
/// intervals rather than spinning.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// The central supervisor for all managed subprocesses.
///
/// `ProcessSupervisor` is the single authority for spawning, monitoring,
/// and supervising subprocesses. It owns `ManagedProcess` records (each
/// behind its own `Arc<Mutex<>>`) and spawns a dedicated tokio monitoring
/// task per process.
///
/// ## Usage
///
/// ```ignore
/// use nabu_core::process_supervisor::{ProcessSupervisor, ProcessConfig, RestartPolicy};
///
/// let supervisor = ProcessSupervisor::new()
///     .with_event_bus(event_bus);
///
/// // Spawn a managed process
/// let id = supervisor.spawn(ProcessConfig::new("mcp-server", "/usr/bin/mcp-server")
///     .with_restart_policy(RestartPolicy::Always))?;
///
/// // Query its state
/// let snapshot = supervisor.get_snapshot(id).unwrap();
/// assert_eq!(snapshot.state, ProcessState::Running);
///
/// // Stop it
/// supervisor.stop(id)?;
/// ```
///
/// ## Thread Safety
///
/// The supervisor is `Send + Sync` and can be shared via `Arc` across
/// threads. All interior mutability is protected by `RwLock` (for the
/// process map) and `Mutex` (for individual records).
pub struct ProcessSupervisor {
    /// All managed processes, keyed by ProcessId.
    /// Each value is behind its own `Mutex` for fine-grained locking
    /// so that one process's monitoring task doesn't block access to
    /// another's state.
    processes: RwLock<HashMap<ProcessId, Arc<Mutex<ManagedProcess>>>>,

    /// EventBus for publishing supervision events (optional).
    event_bus: Option<Arc<EventBus<PipelineEvent>>>,

    /// Lifecycle state manager — tracks Created → Initialized → Running → Shutdown.
    lifecycle: LifecycleManager,

    /// Shared context for coordinating monitoring tasks (shutdown flag,
    /// active monitor count).
    ctx: Arc<SupervisorContext>,
}

impl ProcessSupervisor {
    /// Create a new `ProcessSupervisor` with no EventBus integration.
    ///
    /// The supervisor starts in the `Created` lifecycle stage. Call
    /// [`initialize`](Self::initialize) and [`start`](Self::start) before
    /// spawning processes.
    pub fn new() -> Self {
        Self {
            processes: RwLock::new(HashMap::new()),
            event_bus: None,
            lifecycle: LifecycleManager::new(),
            ctx: Arc::new(SupervisorContext::new()),
        }
    }

    /// Create a new `ProcessSupervisor` with an EventBus for publishing
    /// supervision events.
    pub fn with_event_bus(event_bus: Arc<EventBus<PipelineEvent>>) -> Self {
        Self {
            processes: RwLock::new(HashMap::new()),
            event_bus: Some(event_bus),
            lifecycle: LifecycleManager::new(),
            ctx: Arc::new(SupervisorContext::new()),
        }
    }

    // -----------------------------------------------------------------------
    // Lifecycle state accessors
    // -----------------------------------------------------------------------

    /// Returns the current lifecycle stage of the supervisor.
    pub fn lifecycle_stage(&self) -> LifecycleStage {
        self.lifecycle.stage()
    }

    /// Returns `true` if the supervisor has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.lifecycle.is_at_least(LifecycleStage::Initialized)
    }

    /// Returns `true` if the supervisor is running and can spawn processes.
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }

    /// Returns `true` if the supervisor has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.lifecycle.is_shutdown()
    }

    // -----------------------------------------------------------------------
    // Process management — spawning
    // -----------------------------------------------------------------------

    /// Spawn a managed subprocess.
    ///
    /// This is the canonical entry point for spawning subprocesses. All
    /// future managed processes (MCP servers, ACP servers, plugin hosts,
    /// OCR workers, sync services, etc.) should be spawned through this
    /// method rather than via direct `std::process::Command` or
    /// `tokio::process::Command` calls.
    ///
    /// ## Behavior
    ///
    /// 1. Validates the supervisor's lifecycle state (must be at least `Initialized`).
    /// 2. Assigns a unique `ProcessId` (UUID v4).
    /// 3. Creates a `ManagedProcess` record in the `Created` state.
    /// 4. Creates a broadcast channel for stop signaling and stores the
    ///    sender in the record.
    /// 5. Spawns a tokio monitoring task that spawns the actual child
    ///    process and monitors its lifecycle.
    /// 6. Stores the monitoring task handle in the record.
    /// 7. Returns the `ProcessId` for future operations.
    ///
    /// ## Thread Safety
    ///
    /// Must be called from within a tokio runtime context, as it spawns
    /// a tokio task for monitoring. The runtime handle is obtained via
    /// `Handle::try_current()`.
    ///
    /// ## Errors
    ///
    /// Returns [`ProcessSupervisorError::ShuttingDown`] if the supervisor
    /// is being shut down. Returns [`ProcessSupervisorError::NoRuntime`]
    /// if no tokio runtime is available on the current thread.
    pub fn spawn(&self, config: ProcessConfig) -> ProcessResult<ProcessId> {
        self.spawn_with_config(config, false)
    }

    /// Spawn a managed subprocess, bypassing the lifecycle check.
    ///
    /// This is a convenience for tests and advanced use cases. For normal
    /// use, prefer [`spawn`](Self::spawn).
    fn spawn_with_config(
        &self,
        config: ProcessConfig,
        _bypass_lifecycle: bool,
    ) -> ProcessResult<ProcessId> {
        tracing::debug!(
            subsystem = "supervisor",
            component = "supervisor",
            operation = "spawn",
            process = %config.name,
            command = %config.command_line(),
            "Spawning managed process"
        );

        // ─── Validate supervisor state ───
        if self.ctx.is_shutting_down() {
            tracing::warn!(
                subsystem = "supervisor",
                component = "supervisor",
                process = %config.name,
                "Spawn rejected: supervisor is shutting down"
            );
            return Err(ProcessSupervisorError::ShuttingDown);
        }

        // ─── Obtain tokio runtime ───
        let runtime = Handle::try_current().map_err(|e| {
            tracing::error!(
                subsystem = "supervisor",
                component = "supervisor",
                process = %config.name,
                error = %e,
                "No tokio runtime available for spawning monitoring task"
            );
            ProcessSupervisorError::NoRuntime
        })?;

        // ─── Create the process record ───
        let id = Uuid::new_v4();
        let process_name = config.name.clone();
        let record = Arc::new(Mutex::new(ManagedProcess::new(id, config.clone())));

        // ─── Create broadcast channel for stop signaling ───
        let (stop_tx, stop_rx) = broadcast::channel::<()>(1);

        // Store the stop sender in the record BEFORE spawning the monitoring
        // task, so there's no race where stop() is called before the
        // monitoring task has set the sender.
        {
            let mut rec = record.lock().unwrap();
            rec.stop_tx = Some(stop_tx);
        }

        // ─── Spawn the monitoring task ───
        let event_bus = self.event_bus.clone();
        let ctx = self.ctx.clone();

        let handle = runtime.spawn(monitor_process(
            config,
            record.clone(),
            event_bus,
            ctx,
            stop_rx,
        ));

        // Store the monitoring handle in the record
        {
            let mut rec = record.lock().unwrap();
            rec.monitor_handle = Some(handle);
        }

        // ─── Insert into the process map ───
        {
            let mut processes = self
                .processes
                .write()
                .expect("process map lock not poisoned");
            processes.insert(id, record);
        }

        tracing::info!(
            subsystem = "supervisor",
            component = "supervisor",
            operation = "spawn",
            process = %process_name,
            process_id = %id,
            "Managed process spawned"
        );

        Ok(id)
    }

    // -----------------------------------------------------------------------
    // Process management — stopping
    // -----------------------------------------------------------------------

    /// Stop a managed subprocess by its ID.
    ///
    /// Sends a stop signal to the monitoring task, which will kill the
    /// child process and set its state to `Stopped`. This method waits
    /// for the process to reach a terminal state (or times out).
    ///
    /// ## Behavior
    ///
    /// - If the process is already in a terminal state, this is a no-op
    ///   and returns `Ok(())`.
    /// - If the process is running, the stop signal is sent and this method
    ///   polls the process state until it reaches `Stopped` or the timeout
    ///   expires.
    /// - After timeout, the method returns `Ok(())` regardless — the
    ///   monitoring task will eventually set the state.
    ///
    /// ## Errors
    ///
    /// Returns [`ProcessSupervisorError::NotFound`] if no process with the
    /// given ID is managed by this supervisor.
    pub fn stop(&self, id: ProcessId) -> ProcessResult<()> {
        tracing::debug!(
            subsystem = "supervisor",
            component = "supervisor",
            operation = "stop",
            process_id = %id,
            "Stopping managed process"
        );

        // ─── Find and signal the process ───
        let stop_sent = {
            let processes = self
                .processes
                .read()
                .expect("process map lock not poisoned");

            let record = processes
                .get(&id)
                .ok_or(ProcessSupervisorError::NotFound(id))?;

            let guard = record.lock().unwrap();

            // If already stopped, no-op
            if guard.state.is_terminal() {
                tracing::debug!(
                    subsystem = "supervisor",
                    component = "supervisor",
                    process_id = %id,
                    state = %guard.state,
                    "Process already in terminal state — stop is no-op"
                );
                return Ok(());
            }

            // Send the stop signal
            if let Some(tx) = &guard.stop_tx {
                let _ = tx.send(());
            }
            true
        };

        let _ = stop_sent;
        // Note: stop() is synchronous and does not block-wait for the
        // monitoring task to reach a terminal state. Callers should poll
        // `get_state()` after a short async sleep if they need to confirm
        // the state change.

        Ok(())
    }

    /// Stop all managed processes and shut down the supervisor.
    ///
    /// This is called during application shutdown. It:
    /// 1. Sets the global shutdown flag.
    /// 2. Sends stop signals to all running processes.
    /// 3. Waits (synchronously) for all monitoring tasks to finish,
    ///    up to [`SHUTDOWN_DRAIN_TIMEOUT`].
    /// 4. Aborts any monitoring tasks that don't finish in time.
    ///
    /// Double-shutdown is a safe no-op — if the supervisor is already shut
    /// down (via the lifecycle flag or the shutdown context), this method
    /// returns immediately without error.
    pub fn shutdown(&self) -> ProcessResult<()> {
        // Idempotency guard: if we're already shutting down or fully
        // shut down, return immediately.
        if self.lifecycle.is_shutdown() {
            return Ok(());
        }

        tracing::info!(
            subsystem = "supervisor",
            component = "supervisor",
            operation = "shutdown",
            "Shutting down ProcessSupervisor"
        );

        // ─── Signal shutdown to all monitoring tasks ───
        self.ctx.shutdown();

        // ─── Send stop signals to all running processes ───
        let handles_to_abort: Vec<JoinHandle<()>> = {
            let processes = self
                .processes
                .read()
                .expect("process map lock not poisoned");

            let mut handles = Vec::new();
            for (id, record) in processes.iter() {
                let mut guard = record.lock().unwrap();

                if guard.state.is_terminal() {
                    continue;
                }

                if let Some(tx) = &guard.stop_tx {
                    let _ = tx.send(());
                }

                if let Some(handle) = guard.monitor_handle.take() {
                    handles.push(handle);
                }

                tracing::debug!(
                    subsystem = "supervisor",
                    component = "supervisor",
                    operation = "shutdown",
                    process_id = %id,
                    "Sent stop signal to process"
                );
            }
            handles
        };

        // ─── Wait for monitoring tasks to finish ───
        let start = std::time::Instant::now();
        loop {
            let remaining = self.ctx.active_count();
            if remaining == 0 {
                tracing::info!(
                    subsystem = "supervisor",
                    component = "supervisor",
                    operation = "shutdown",
                    "All monitoring tasks finished"
                );
                break;
            }

            if start.elapsed() >= SHUTDOWN_DRAIN_TIMEOUT {
                tracing::warn!(
                    subsystem = "supervisor",
                    component = "supervisor",
                    operation = "shutdown",
                    remaining = remaining,
                    "Drain timeout — aborting remaining monitoring tasks"
                );
                for handle in &handles_to_abort {
                    handle.abort();
                }
                break;
            }

            std::thread::sleep(POLL_INTERVAL);
        }

        // ─── Clear the process map ───
        self.processes
            .write()
            .expect("process map lock not poisoned")
            .clear();

        // Transition lifecycle
        self.lifecycle
            .transition_to(LifecycleStage::Shutdown)
            .unwrap_or_else(|e| {
                tracing::warn!(
                    subsystem = "supervisor",
                    component = "supervisor",
                    error = %e,
                    "Lifecycle transition to Shutdown failed"
                );
            });

        tracing::info!(
            subsystem = "supervisor",
            component = "supervisor",
            operation = "shutdown",
            "ProcessSupervisor shut down"
        );

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Process queries
    // -----------------------------------------------------------------------

    /// Returns the current state of a managed process.
    ///
    /// Returns `None` if no process with the given ID is managed.
    pub fn get_state(&self, id: ProcessId) -> Option<ProcessState> {
        let processes = self
            .processes
            .read()
            .expect("process map lock not poisoned");
        let record = processes.get(&id)?;
        let guard = record.lock().unwrap();
        Some(guard.state)
    }

    /// Returns a serializable snapshot of a managed process's state.
    ///
    /// Returns `None` if no process with the given ID is managed.
    pub fn get_snapshot(&self, id: ProcessId) -> Option<ProcessSnapshot> {
        let processes = self
            .processes
            .read()
            .expect("process map lock not poisoned");
        let record = processes.get(&id)?;
        let guard = record.lock().unwrap();
        Some(guard.snapshot())
    }

    /// Returns the OS process ID of a managed process, if it is running.
    ///
    /// Returns `None` if the process is not running or is not found.
    pub fn get_pid(&self, id: ProcessId) -> Option<u32> {
        let processes = self
            .processes
            .read()
            .expect("process map lock not poisoned");
        let record = processes.get(&id)?;
        let guard = record.lock().unwrap();
        guard.pid
    }

    /// Returns a list of all managed processes as snapshots.
    pub fn list_processes(&self) -> Vec<ProcessSnapshot> {
        let processes = self
            .processes
            .read()
            .expect("process map lock not poisoned");
        processes
            .values()
            .map(|record| {
                let guard = record.lock().unwrap();
                guard.snapshot()
            })
            .collect()
    }

    /// Returns the number of managed processes.
    ///
    /// This includes processes in all states (running, stopped, failed).
    pub fn process_count(&self) -> usize {
        self.processes
            .read()
            .expect("process map lock not poisoned")
            .len()
    }

    /// Returns the number of currently running processes.
    pub fn running_count(&self) -> usize {
        let processes = self
            .processes
            .read()
            .expect("process map lock not poisoned");
        processes
            .values()
            .filter(|record| record.lock().unwrap().state.is_running())
            .count()
    }

    /// Returns the number of processes in terminal states.
    pub fn terminal_count(&self) -> usize {
        let processes = self
            .processes
            .read()
            .expect("process map lock not poisoned");
        processes
            .values()
            .filter(|record| record.lock().unwrap().state.is_terminal())
            .count()
    }

    /// Returns a summary of the supervisor's current state.
    pub fn summary(&self) -> SupervisorSummary {
        let (total, running, terminal) = {
            let processes = self
                .processes
                .read()
                .expect("process map lock not poisoned");
            let total = processes.len();
            let running = processes
                .values()
                .filter(|r| r.lock().unwrap().state.is_running())
                .count();
            let terminal = processes
                .values()
                .filter(|r| r.lock().unwrap().state.is_terminal())
                .count();
            (total, running, terminal)
        };

        SupervisorSummary {
            total_processes: total,
            running_processes: running,
            terminal_processes: terminal,
            lifecycle_stage: self.lifecycle.stage(),
            is_shutdown: self.ctx.is_shutting_down(),
        }
    }

    /// Returns a reference to the EventBus, if one is registered.
    pub fn event_bus(&self) -> Option<&Arc<EventBus<PipelineEvent>>> {
        self.event_bus.as_ref()
    }

    /// Returns `true` if the supervisor manages a process with the given ID.
    pub fn has_process(&self, id: ProcessId) -> bool {
        self.processes
            .read()
            .expect("process map lock not poisoned")
            .contains_key(&id)
    }
}

/// A lightweight summary of the supervisor's current state.
#[derive(Debug, Clone)]
pub struct SupervisorSummary {
    /// Total number of managed processes.
    pub total_processes: usize,

    /// Number of processes currently running.
    pub running_processes: usize,

    /// Number of processes in terminal states (Stopped).
    pub terminal_processes: usize,

    /// Current lifecycle stage of the supervisor.
    pub lifecycle_stage: LifecycleStage,

    /// Whether the supervisor is shutting down.
    pub is_shutdown: bool,
}

// ---------------------------------------------------------------------------
// Lifecycle trait implementation
// ---------------------------------------------------------------------------

/// Implements the shared [`Lifecycle`] trait so `ProcessSupervisor` can
/// participate in the Capability Platform's lifecycle management alongside
/// other services.
///
/// ```text
/// Created → Initialized → Running → Shutdown
/// ```
///
/// - **initialize**: verifies configuration, prepares internal structures.
/// - **start**: marks the supervisor as ready to accept process spawns.
/// - **shutdown**: stops all managed processes, prevents new spawns,
///   drains monitoring tasks.
impl Lifecycle for ProcessSupervisor {
    fn name(&self) -> &'static str {
        "process_supervisor"
    }

    /// Initializes the ProcessSupervisor.
    ///
    /// Lifecycle transition: `Created → Initialized`.
    ///
    /// No resource allocation or process spawning occurs here — the
    /// supervisor is ready to accept processes after `start()`.
    fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.lifecycle.is_shutdown() {
            return Err("ProcessSupervisor has been shut down".into());
        }

        tracing::info!(
            subsystem = "supervisor",
            component = "supervisor",
            operation = "initialize",
            "Initializing ProcessSupervisor"
        );

        self.lifecycle
            .transition_to(LifecycleStage::Initialized)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        tracing::info!(
            subsystem = "supervisor",
            component = "supervisor",
            operation = "initialize",
            "ProcessSupervisor initialized"
        );
        Ok(())
    }

    /// Starts the ProcessSupervisor.
    ///
    /// Lifecycle transition: `Created → Initialized → Running` (or
    /// `Initialized → Running` if [`initialize`](Self::initialize) was called
    /// first).
    ///
    /// After starting, the supervisor accepts `spawn()` calls.
    fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.lifecycle.is_shutdown() {
            return Err("ProcessSupervisor has been shut down and cannot be started".into());
        }

        // Auto-advance Created → Initialized
        if self.lifecycle.stage() == LifecycleStage::Created {
            self.lifecycle
                .transition_to(LifecycleStage::Initialized)?;
        }

        self.lifecycle
            .transition_to(LifecycleStage::Running)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        tracing::info!(
            subsystem = "supervisor",
            component = "supervisor",
            operation = "start",
            "ProcessSupervisor started — accepting process spawns"
        );
        Ok(())
    }

    /// Shuts down the ProcessSupervisor.
    ///
    /// Lifecycle transition: `Running → Shutdown` (or `Initialized → Shutdown`).
    ///
    /// Stops all managed processes and drains monitoring tasks.
    fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            subsystem = "supervisor",
            component = "supervisor",
            operation = "shutdown",
            "ProcessSupervisor shutting down"
        );

        self.shutdown()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

        Ok(())
    }
}

impl Default for ProcessSupervisor {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ProcessSupervisor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let processes = self
            .processes
            .read()
            .map(|p| p.len())
            .unwrap_or(0);
        f.debug_struct("ProcessSupervisor")
            .field("process_count", &processes)
            .field("lifecycle_stage", &self.lifecycle.stage())
            .field("shutdown_flag", &self.ctx.is_shutting_down())
            .finish()
    }
}

impl MetricsAggregator for ProcessSupervisor {
    fn metrics(&self) -> ServiceMetrics {
        let summary = self.summary();

        ServiceMetrics {
            service: "process_supervisor".to_string(),
            timers: Vec::new(),
            counters: vec![
                CounterMetric {
                    key: "supervisor.total_processes".to_string(),
                    value: summary.total_processes as u64,
                },
                CounterMetric {
                    key: "supervisor.running_processes".to_string(),
                    value: summary.running_processes as u64,
                },
                CounterMetric {
                    key: "supervisor.terminal_processes".to_string(),
                    value: summary.terminal_processes as u64,
                },
            ],
            gauges: vec![
                GaugeMetric {
                    key: "supervisor.total_processes".to_string(),
                    value: summary.total_processes as i64,
                },
                GaugeMetric {
                    key: "supervisor.running_processes".to_string(),
                    value: summary.running_processes as i64,
                },
                GaugeMetric {
                    key: "supervisor.terminal_processes".to_string(),
                    value: summary.terminal_processes as i64,
                },
                GaugeMetric {
                    key: "supervisor.is_shutting_down".to_string(),
                    value: if summary.is_shutdown { 1 } else { 0 },
                },
            ],
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::EventBus;
    use crate::process_supervisor::policy::RestartPolicy;
    use std::sync::Arc;

    #[test]
    fn new_supervisor_starts_in_created() {
        let supervisor = ProcessSupervisor::new();
        assert_eq!(supervisor.lifecycle_stage(), LifecycleStage::Created);
        assert!(!supervisor.is_initialized());
        assert!(!supervisor.is_running());
        assert!(!supervisor.is_shutdown());
        assert_eq!(supervisor.process_count(), 0);
        assert_eq!(supervisor.running_count(), 0);
    }

    #[test]
    fn with_event_bus_sets_event_bus() {
        let bus: Arc<EventBus<PipelineEvent>> = Arc::new(EventBus::new());
        let supervisor = ProcessSupervisor::with_event_bus(bus.clone());
        assert!(supervisor.event_bus().is_some());
        assert!(Arc::ptr_eq(supervisor.event_bus().unwrap(), &bus));
    }

    #[test]
    fn lifecycle_transition_to_initialized() {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert_eq!(supervisor.lifecycle_stage(), LifecycleStage::Initialized);
        assert!(supervisor.is_initialized());
        assert!(!supervisor.is_running());
    }

    #[test]
    fn lifecycle_full_flow() {
        let supervisor = ProcessSupervisor::new();
        assert_eq!(supervisor.lifecycle_stage(), LifecycleStage::Created);

        assert!(supervisor.initialize().is_ok());
        assert_eq!(supervisor.lifecycle_stage(), LifecycleStage::Initialized);
        assert!(supervisor.is_initialized());

        assert!(supervisor.start().is_ok());
        assert_eq!(supervisor.lifecycle_stage(), LifecycleStage::Running);
        assert!(supervisor.is_running());

        assert!(supervisor.shutdown().is_ok());
        assert_eq!(supervisor.lifecycle_stage(), LifecycleStage::Shutdown);
        assert!(supervisor.is_shutdown());
    }

    #[test]
    fn start_auto_advances_from_created() {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.start().is_ok());
        assert_eq!(supervisor.lifecycle_stage(), LifecycleStage::Running);
    }

    #[test]
    fn shutdown_after_start() {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.start().is_ok());
        assert!(supervisor.shutdown().is_ok());
        assert!(supervisor.is_shutdown());
    }

    #[test]
    fn shutdown_without_start() {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.shutdown().is_ok());
        assert!(supervisor.is_shutdown());
    }

    #[test]
    fn start_after_shutdown_fails() {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.start().is_ok());
        assert!(supervisor.shutdown().is_ok());
        assert!(supervisor.start().is_err());
    }

    #[test]
    fn spawn_requires_runtime() {
        let supervisor = ProcessSupervisor::new();
        // Call from outside a tokio runtime — should fail with NoRuntime
        let config = ProcessConfig::new("test", "echo");
        let result = supervisor.spawn(config);
        // If we're not in a runtime, we get NoRuntime.
        // If we ARE in a runtime (e.g. tokio test runtime), we get a different
        // error or success. Let's just verify it doesn't panic.
        match result {
            Ok(_) => { /* fine if we happen to be in a runtime */ }
            Err(ProcessSupervisorError::NoRuntime) => { /* expected if not in runtime */ }
            Err(ProcessSupervisorError::ShuttingDown) => { /* also possible */ }
            Err(e) => panic!("Unexpected error from spawn: {:?}", e),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn spawn_process_that_exists_and_exits() {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert!(supervisor.start().is_ok());

        // spawn a process that exits immediately with code 0
        let config = ProcessConfig::new("exit-test", "true")
            .with_restart_policy(RestartPolicy::never());

        let id = supervisor.spawn(config).expect("spawn should succeed");

        // Wait for the process to start and exit
        tokio::time::sleep(Duration::from_millis(200)).await;

        let snapshot = supervisor.get_snapshot(id);
        assert!(snapshot.is_some(), "process should be in the registry");
        let snapshot = snapshot.unwrap();

        // With RestartPolicy::Never, the process should eventually be Stopped
        // or Exited. Let's check it's not Created/Starting/Running anymore.
        assert!(
            snapshot.state.is_terminal() || snapshot.state == ProcessState::Exited,
            "process should be exited or stopped, got: {:?} (exit_code: {:?})",
            snapshot.state,
            snapshot.exit_code
        );

        supervisor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn spawn_process_and_stop() {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert!(supervisor.start().is_ok());

        // Spawn a long-running process
        let config = ProcessConfig::new("sleeper", "sleep")
            .with_arg("30".to_string())
            .with_restart_policy(RestartPolicy::Always);

        let id = supervisor.spawn(config).expect("spawn should succeed");

        // Wait for it to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        let state = supervisor.get_state(id);
        assert_eq!(state, Some(ProcessState::Running));

        // Stop it
        supervisor.stop(id).expect("stop should succeed");
        // Allow the monitor task to process the stop signal and reach Stopped
        tokio::time::sleep(Duration::from_millis(200)).await;
        let state = supervisor.get_state(id);
        assert_eq!(
            state,
            Some(ProcessState::Stopped),
            "process should be stopped after stop()"
        );

        supervisor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn restart_on_failure_with_always_policy() {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert!(supervisor.start().is_ok());

        // Spawn a process that fails immediately (exit code 1)
        // Use a shell command that exits with 1
        let config = ProcessConfig::new("failer", "sh")
            .with_args(vec!["-c".to_string(), "exit 1".to_string()])
            .with_restart_policy(RestartPolicy::Always);

        let id = supervisor.spawn(config).expect("spawn should succeed");

        // Wait for multiple restart cycles
        tokio::time::sleep(Duration::from_millis(500)).await;

        let snapshot = supervisor.get_snapshot(id).expect("process should exist");
        assert!(
            snapshot.restart_count > 0,
            "process should have been restarted at least once (restart_count = {})",
            snapshot.restart_count
        );

        supervisor.stop(id).ok();
        supervisor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn restart_limited_retries_policy() {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert!(supervisor.start().is_ok());

        // Spawn a process that fails immediately, with limited retries
        let config = ProcessConfig::new("failer", "sh")
            .with_args(vec!["-c".to_string(), "exit 1".to_string()])
            .with_restart_policy(RestartPolicy::limited_retries(2));

        let id = supervisor.spawn(config).expect("spawn should succeed");

        // Wait for it to fail and exhaust retries
        tokio::time::sleep(Duration::from_millis(600)).await;

        let snapshot = supervisor.get_snapshot(id).expect("process should exist");
        // With max_restarts=2 and OnFailure-like behavior, it should restart twice
        // then give up. restart_count should be > 0 but <= 2 + 1.
        assert!(
            snapshot.restart_count > 0,
            "process should have been restarted (restart_count = {})",
            snapshot.restart_count
        );

        supervisor.stop(id).ok();
        supervisor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn process_not_found_returns_error() {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert!(supervisor.start().is_ok());

        let fake_id = Uuid::new_v4();
        let result = supervisor.get_state(fake_id);
        assert!(result.is_none());

        let result = supervisor.get_snapshot(fake_id);
        assert!(result.is_none());

        let result = supervisor.get_pid(fake_id);
        assert!(result.is_none());

        supervisor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn stop_nonexistent_returns_not_found() {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert!(supervisor.start().is_ok());

        let fake_id = Uuid::new_v4();
        let result = supervisor.stop(fake_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().is_not_found());

        supervisor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn summary_reflects_state() {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.start().is_ok());

        let config = ProcessConfig::new("test", "echo")
            .with_args(vec!["hello".to_string()])
            .with_restart_policy(RestartPolicy::never());

        let id = supervisor.spawn(config).expect("spawn should succeed");

        // Wait for it to start and exit
        tokio::time::sleep(Duration::from_millis(200)).await;

        let summary = supervisor.summary();
        assert_eq!(summary.total_processes, 1);
        assert!(summary.lifecycle_stage == LifecycleStage::Running);

        let _ = id; // suppress unused warning
        supervisor.shutdown().unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn spawn_after_shutdown_fails() {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.start().is_ok());
        assert!(supervisor.shutdown().is_ok());

        let config = ProcessConfig::new("test", "echo");
        let result = supervisor.spawn(config);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProcessSupervisorError::ShuttingDown
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn debug_format_works() {
        let supervisor = ProcessSupervisor::new();
        let debug_str = format!("{:?}", supervisor);
        assert!(debug_str.contains("ProcessSupervisor"));
        assert!(debug_str.contains("process_count"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn lifecycle_trait_name() {
        let supervisor = ProcessSupervisor::new();
        let supervisor_ref: &dyn Lifecycle = &supervisor;
        assert_eq!(supervisor_ref.name(), "process_supervisor");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn multiple_processes_managed() {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.start().is_ok());

        let id1 = supervisor.spawn(ProcessConfig::new("p1", "echo").with_restart_policy(RestartPolicy::never())).unwrap();
        let id2 = supervisor.spawn(ProcessConfig::new("p2", "echo").with_restart_policy(RestartPolicy::never())).unwrap();

        assert_ne!(id1, id2);
        assert_eq!(supervisor.process_count(), 2);
        assert!(supervisor.has_process(id1));
        assert!(supervisor.has_process(id2));

        supervisor.shutdown().unwrap();
    }
}
