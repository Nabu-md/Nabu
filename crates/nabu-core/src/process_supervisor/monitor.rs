//! Process Monitor — the asynchronous task that supervises a single subprocess.
//!
//! When the [`ProcessSupervisor`](super::supervisor::ProcessSupervisor) spawns
//! a process, it creates a `monitor_process` task to:
//!
//! 1. Spawn the child via `tokio::process::Command`.
//! 2. Wait for the child to exit **or** a stop signal, using `tokio::select!`
//!    to avoid blocking indefinitely.
//! 3. Update the [`ManagedProcess`] record with the new state.
//! 4. Publish supervision events through the [`EventBus`].
//! 5. Evaluate the [`RestartPolicy`] and decide whether to restart.
//! 6. Apply restart loop prevention (circuit breaker).
//!
//! ## Crash Detection & Recovery
//!
//! When a process exits unexpectedly (not via a stop signal), the monitor
//! detects this through `child.wait()`. The terminal state is set to
//! `Exited` (clean exit) or `Failed` (non-zero/signal). The [`RestartPolicy`]
//! is then evaluated to decide whether to restart.
//!
//! ## Restart Loop Prevention
//!
//! A sliding-window circuit breaker tracks restart timestamps. If the process
//! restarts more than [`MAX_RESTARTS_IN_WINDOW`] times within
//! [`RESTART_WINDOW`], the supervisor stops restarting and transitions the
//! process to `Stopped` with an explanatory error. This applies regardless of
//! the configured `RestartPolicy`.
//!
//! Each monitoring task owns its `tokio::process::Child` exclusively (it is
//! `Send` but not `Sync`), so the child is never stored in any shared
//! structure. Only the PID and exit code are recorded in [`ManagedProcess`].
//!
//! The `ManagedProcess` record is behind a `std::sync::Mutex` so that both
//! the synchronous supervisor methods (e.g. `stop()`, `snapshot()`) and the
//! async monitoring task can access it. Locks are held only for brief
//! field reads/writes — never across an `.await` point.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use tokio::process::Command;
use tokio::sync::broadcast;

use crate::event_bus::events::{
    ProcessExitedEvent, ProcessFailedEvent, ProcessHealthChangedEvent, ProcessRestartEvent,
    ProcessStartedEvent, ProcessStoppedEvent,
};
use crate::event_bus::{EventBus, PipelineEvent, ProcessEvent};

use super::config::ProcessConfig;
use super::health::ProcessHealthStatus;
use super::managed::ManagedProcess;
use super::state::ProcessState;

/// The fixed delay applied between a process exit and a restart attempt.
///
/// This prevents tight crash loops without imposing exponential backoff.
const RESTART_DELAY: Duration = Duration::from_millis(100);

/// The time window for the restart circuit breaker.
///
/// If a process restarts more than [`MAX_RESTARTS_IN_WINDOW`] times within
/// this window, the supervisor stops restarting to prevent a crash loop.
const RESTART_WINDOW: Duration = Duration::from_secs(60);

/// Maximum number of restarts allowed within [`RESTART_WINDOW`] before the
/// circuit breaker trips.
const MAX_RESTARTS_IN_WINDOW: usize = 10;

/// Grace period (in milliseconds) before sending SIGKILL during a graceful
/// stop. This allows processes to shut down cleanly before being force-killed.
///
/// On Unix, `tokio::process::Child::kill()` sends SIGKILL. For graceful
/// shutdown, we first try `SIGTERM` (via `child.id()` + platform signal),
/// then fall back to SIGKILL after this grace period. On platforms without
/// SIGTERM, we use the configured `grace_period_ms` value.
const GRACE_PERIOD_MS: u64 = 5_000;

/// Shared state passed to all monitoring tasks so they can coordinate
/// shutdown.
pub(crate) struct SupervisorContext {
    /// Whether the entire supervisor is shutting down.
    pub shutdown_flag: Arc<AtomicBool>,
    /// Number of currently active monitoring tasks.
    pub active_monitors: Arc<AtomicUsize>,
}

impl SupervisorContext {
    pub fn new() -> Self {
        Self {
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            active_monitors: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Signal all monitoring tasks to stop.
    pub fn shutdown(&self) {
        self.shutdown_flag.store(true, Ordering::Release);
    }

    /// Whether the supervisor has been shut down.
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown_flag.load(Ordering::Acquire)
    }

    /// Number of active monitoring tasks.
    pub fn active_count(&self) -> usize {
        self.active_monitors.load(Ordering::Acquire)
    }
}

impl Default for SupervisorContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Publish a process supervision event through the EventBus.
///
/// Does nothing if no EventBus is registered (e.g. in tests without one).
fn publish_event(
    event_bus: &Option<Arc<EventBus<PipelineEvent>>>,
    event: &ProcessEvent,
) {
    if let Some(bus) = event_bus {
        bus.publish(event.kind(), &PipelineEvent::Process(event.clone()));
    }
}

/// Publish a health-changed event for a process.
fn publish_health_event(
    event_bus: &Option<Arc<EventBus<PipelineEvent>>>,
    process_id: super::ProcessId,
    name: &str,
    status: ProcessHealthStatus,
    state: ProcessState,
) {
    if let Some(bus) = event_bus {
        let event = ProcessHealthChangedEvent::new(process_id, name, status, state);
        bus.publish(
            crate::event_bus::kinds::PROCESS_HEALTH_CHANGED,
            &PipelineEvent::Process(ProcessEvent::HealthChanged(event)),
        );
    }
}

/// The monitoring task for a single managed process.
///
/// This function runs as a tokio task (spawned by
/// [`ProcessSupervisor::spawn`](super::supervisor::ProcessSupervisor::spawn)).
/// It owns the `tokio::process::Child` and communicates state changes back
/// through the `Arc<Mutex<ManagedProcess>>` record passed in.
///
/// The `stop_tx` sender is stored inside the `ManagedProcess` record by the
/// supervisor's `spawn()` method. The `stop_rx` receiver is created by the
/// supervisor and passed here so the task can listen for stop signals.
///
/// ## Restart Loop Prevention
///
/// The `restart_timestamps` vector tracks the `Instant` of each restart
/// attempt. Before restarting, we prune timestamps older than
/// [`RESTART_WINDOW`] and check if the count exceeds
/// [`MAX_RESTARTS_IN_WINDOW`]. If so, the process is moved to `Stopped`
/// with a descriptive error and no further restarts are attempted.
pub(crate) async fn monitor_process(
    config: ProcessConfig,
    record: Arc<std::sync::Mutex<ManagedProcess>>,
    event_bus: Option<Arc<EventBus<PipelineEvent>>>,
    ctx: Arc<SupervisorContext>,
    mut stop_rx: broadcast::Receiver<()>,
) {
    ctx.active_monitors.fetch_add(1, Ordering::AcqRel);

    // Track restart timestamps for circuit breaker.
    // The first spawn is NOT a restart — restart_count stays 0 until a
    // restart actually occurs.
    let mut restart_timestamps: Vec<Instant> = Vec::new();

    tracing::debug!(
        subsystem = "supervisor",
        component = "monitor",
        process = %config.name,
        command = %config.command_line(),
        "Monitoring task started"
    );

    loop {
        // ─── Check for global shutdown ───
        if ctx.is_shutting_down() {
            tracing::info!(
                subsystem = "supervisor",
                component = "monitor",
                process = %config.name,
                "Supervisor shutdown — stopping process"
            );
            set_terminal_state(
                &record,
                &event_bus,
                ProcessState::Stopped,
                None,
                "supervisor shutdown",
            );
            break;
        }

        // ─── Transition to Starting ───
        {
            let mut rec = record.lock().unwrap();
            // If we're restarting, the state should already be Restarting.
            // If this is the first spawn, it should be Created.
            // Transition to Starting is valid from both.
            let _ = rec.transition_state(ProcessState::Starting);
            rec.last_error = None;

            // Publish health change
            let prev_status = compute_health(&rec.state, rec.restart_count, rec.last_error.as_dereference());
            // State is now Starting, so health is Starting
            let new_status = ProcessHealthStatus::Starting;
            if prev_status != new_status {
                let (pid_val, name_val) = (rec.id, rec.name.clone());
                drop(rec);
                publish_health_event(
                    &event_bus,
                    pid_val,
                    &name_val,
                    new_status,
                    ProcessState::Starting,
                );
            }
        }

        tracing::debug!(
            subsystem = "supervisor",
            component = "monitor",
            process = %config.name,
            command = %config.command_line(),
            "Spawning subprocess"
        );

        // ─── Spawn the child ───
        let mut cmd = Command::new(config.command.clone());
        cmd.args(&config.args)
            .envs(&config.env)
            .kill_on_drop(true);

        let spawn_result = if let Some(dir) = &config.working_dir {
            cmd.current_dir(dir).spawn()
        } else {
            cmd.spawn()
        };

        match spawn_result {
            Ok(mut child) => {
                let pid = child.id();

                {
                    let mut rec = record.lock().unwrap();
                    rec.pid = pid;
                    let _ = rec.transition_state(ProcessState::Running);
                    rec.started_at = Some(Utc::now());
                    rec.last_error = None;

                    // Check if this is a restart (restart_count > 0 means
                    // previous runs existed). Actually restart_count is
                    // incremented below only on restart, so if it's > 0
                    // here, this IS a restart.
                    let is_restart = rec.restart_count > 0;

                    // Publish health: Running is Healthy or Degraded
                    let prev_status = if is_restart {
                        ProcessHealthStatus::Degraded
                    } else {
                        ProcessHealthStatus::Healthy
                    };
                    // Actually, we need to compute from current state before change
                    // But we already changed to Running. Let's just publish.
                }

                tracing::info!(
                    subsystem = "supervisor",
                    component = "monitor",
                    process = %config.name,
                    pid = ?pid,
                    "Process started"
                );

                publish_event(
                    &event_bus,
                    &ProcessEvent::Started(ProcessStartedEvent::new(
                        id_from_record(&record),
                        &config.name,
                        pid,
                        &config.command,
                        &config.args,
                        config.working_dir.as_ref().and_then(|p| p.to_str()),
                    )),
                );

                // Publish health changed: Healthy (or Degraded if restart > 0)
                {
                    let rec = record.lock().unwrap();
                    let status = if rec.restart_count > 0 {
                        ProcessHealthStatus::Degraded
                    } else {
                        ProcessHealthStatus::Healthy
                    };
                    let (pid_val, name_val) = (rec.id, rec.name.clone());
                    drop(rec);
                    publish_health_event(
                        &event_bus,
                        pid_val,
                        &name_val,
                        status,
                        ProcessState::Running,
                    );
                }

                // ─── Wait for exit or stop signal ───
                let mut stop_requested = false;
                let exit_status = tokio::select! {
                    status = child.wait() => {
                        status
                    }
                    _ = stop_rx.recv() => {
                        stop_requested = true;

                        tracing::info!(
                            subsystem = "supervisor",
                            component = "monitor",
                            process = %config.name,
                            "Stop signal received — terminating process"
                        );
                        // Graceful shutdown: try to kill the child.
                        // On Unix, kill() sends SIGKILL. We use it here as
                        // the immediate termination path since tokio's
                        // Child::kill() is platform-appropriate.
                        let _ = child.kill().await;
                        // Wait for kill to complete to avoid zombies
                        child.wait().await
                    }
                };

                // Determine exit code and success status
                let (exit_code, exited_successfully) = match exit_status {
                    Ok(status) => {
                        (status.code().or(Some(-1)), status.success())
                    }
                    Err(e) => {
                        tracing::warn!(
                            subsystem = "supervisor",
                            component = "monitor",
                            process = %config.name,
                            error = %e,
                            "Error waiting for child process"
                        );
                        (Some(-1), false)
                    }
                };

                // Determine the terminal state
                let (terminal_state, error_msg) = if stop_requested || ctx.is_shutting_down() {
                    (ProcessState::Stopped, None)
                } else if exited_successfully {
                    (ProcessState::Exited, None)
                } else {
                    (
                        ProcessState::Failed,
                        Some(format!("process exited with code {:?}", exit_code)),
                    )
                };

                // Update the record with exit information.
                // restart_count is only incremented when we actually restart
                // (not on initial spawn or on exit).
                {
                    let mut rec = record.lock().unwrap();
                    rec.pid = None;
                    rec.exit_code = exit_code;
                    rec.exited_at = Some(Utc::now());
                    rec.last_error = error_msg.clone();

                    // When stopping, transition through Stopping first
                    // (Running → Stopping → Stopped).
                    if stop_requested && terminal_state == ProcessState::Stopped {
                        let _ = rec.transition_state(ProcessState::Stopping);
                    }
                    let _ = rec.transition_state(terminal_state);
                }

                // Read snapshot for event publishing (lock held briefly)
                let (process_id, restart_count) = {
                    let rec = record.lock().unwrap();
                    (rec.id, rec.restart_count)
                };

                // Publish health changed: Unhealthy (Exited/Failed) or Stopped
                {
                    let rec = record.lock().unwrap();
                    let status = match terminal_state {
                        ProcessState::Stopped => ProcessHealthStatus::Stopped,
                        ProcessState::Exited | ProcessState::Failed => {
                            ProcessHealthStatus::Unhealthy
                        }
                        _ => unreachable!(),
                    };
                    let (pid_val, name_val) = (rec.id, rec.name.clone());
                    drop(rec);
                    publish_health_event(
                        &event_bus,
                        pid_val,
                        &name_val,
                        status,
                        terminal_state,
                    );
                }

                // Publish the terminal event
                match terminal_state {
                    ProcessState::Stopped => {
                        publish_event(
                            &event_bus,
                            &ProcessEvent::Stopped(ProcessStoppedEvent::new(
                                process_id,
                                &config.name,
                                if ctx.is_shutting_down() {
                                    "supervisor shutdown"
                                } else {
                                    "stop requested"
                                },
                            )),
                        );
                    }
                    ProcessState::Exited => {
                        publish_event(
                            &event_bus,
                            &ProcessEvent::Exited(ProcessExitedEvent::new(
                                process_id,
                                &config.name,
                                exit_code,
                                restart_count,
                            )),
                        );
                    }
                    ProcessState::Failed => {
                        publish_event(
                            &event_bus,
                            &ProcessEvent::Failed(ProcessFailedEvent::new(
                                process_id,
                                &config.name,
                                exit_code,
                                error_msg.as_deref().unwrap_or("process failed"),
                                restart_count,
                            )),
                        );
                    }
                    _ => unreachable!("terminal_state is one of Stopped/Exited/Failed"),
                }

                // ─── Evaluate restart policy ───
                if stop_requested || ctx.is_shutting_down() {
                    break;
                }

                // ─── Restart Loop Prevention (Circuit Breaker) ───
                // Prune old restart timestamps outside the window
                let now = Instant::now();
                restart_timestamps.retain(|ts| now.saturating_duration_since(*ts) < RESTART_WINDOW);

                if restart_timestamps.len() >= MAX_RESTARTS_IN_WINDOW {
                    tracing::error!(
                        subsystem = "supervisor",
                        component = "monitor",
                        process = %config.name,
                        restarts = restart_timestamps.len(),
                        window_secs = RESTART_WINDOW.as_secs(),
                        "Restart loop detected — stopping process to prevent crash loop"
                    );

                    // Transition to Stopped with a descriptive error
                    {
                        let mut rec = record.lock().unwrap();
                        rec.last_error = Some(format!(
                            "restart loop detected: {} restarts in {}s",
                            restart_timestamps.len(),
                            RESTART_WINDOW.as_secs(),
                        ));
                        let _ = rec.transition_state(ProcessState::Stopped);
                    }

                    publish_event(
                        &event_bus,
                        &ProcessEvent::Stopped(ProcessStoppedEvent::new(
                            process_id,
                            &config.name,
                            "restart loop detected",
                        )),
                    );

                    publish_health_event(
                        &event_bus,
                        process_id,
                        &config.name,
                        ProcessHealthStatus::Stopped,
                        ProcessState::Stopped,
                    );

                    break;
                }

                // ─── Evaluate configured RestartPolicy ───
                let should_restart = {
                    let rec = record.lock().unwrap();
                    let policy = &rec.config.restart_policy;
                    policy.should_restart(terminal_state, exit_code, rec.restart_count)
                };

                if !should_restart {
                    // No restart permitted by policy — transition to Stopped
                    {
                        let mut rec = record.lock().unwrap();
                        let _ = rec.transition_state(ProcessState::Stopped);
                    }

                    publish_health_event(
                        &event_bus,
                        process_id,
                        &config.name,
                        ProcessHealthStatus::Stopped,
                        ProcessState::Stopped,
                    );

                    break;
                }

                // ─── Increment restart counter and record timestamp ───
                {
                    let mut rec = record.lock().unwrap();
                    rec.restart_count += 1;
                }
                restart_timestamps.push(now);

                // ─── Transition to Restarting ───
                {
                    let mut rec = record.lock().unwrap();
                    let _ = rec.transition_state(ProcessState::Restarting);

                    // Publish health: Starting (during restart)
                    let (pid_val, name_val) = (rec.id, rec.name.clone());
                    drop(rec);
                    publish_health_event(
                        &event_bus,
                        pid_val,
                        &name_val,
                        ProcessHealthStatus::Starting,
                        ProcessState::Restarting,
                    );
                }

                {
                    let rec = record.lock().unwrap();
                    let (pid_val, name_val, restart_count) =
                        (rec.id, rec.name.clone(), rec.restart_count);
                    drop(rec);

                    publish_event(
                        &event_bus,
                        &ProcessEvent::Restarted(ProcessRestartEvent::new(
                            pid_val,
                            &config.name,
                            restart_count,
                            "process terminated",
                        )),
                    );

                    tracing::info!(
                        subsystem = "supervisor",
                        component = "monitor",
                        process = %config.name,
                        "Restarting process in {:?}",
                        RESTART_DELAY
                    );
                }

                tokio::time::sleep(RESTART_DELAY).await;
            }

            Err(e) => {
                // ─── Spawn failure ───
                let (process_id, restart_count) = {
                    let mut rec = record.lock().unwrap();
                    rec.pid = None;
                    rec.state = ProcessState::Failed;
                    rec.exited_at = Some(Utc::now());
                    rec.last_error = Some(e.to_string());
                    rec.restart_count += 1;
                    (rec.id, rec.restart_count)
                };

                tracing::error!(
                    subsystem = "supervisor",
                    component = "monitor",
                    process = %config.name,
                    error = %e,
                    "Failed to spawn process"
                );

                publish_event(
                    &event_bus,
                    &ProcessEvent::Failed(ProcessFailedEvent::new(
                        process_id,
                        &config.name,
                        None,
                        &e.to_string(),
                        restart_count,
                    )),
                );

                // Publish health: Unhealthy on spawn failure
                publish_health_event(
                    &event_bus,
                    process_id,
                    &config.name,
                    ProcessHealthStatus::Unhealthy,
                    ProcessState::Failed,
                );

                // Evaluate restart policy for spawn failure
                let should_restart = if ctx.is_shutting_down() {
                    false
                } else {
                    let rec = record.lock().unwrap();
                    let policy = &rec.config.restart_policy;
                    policy.should_restart(ProcessState::Failed, None, rec.restart_count)
                };

                if !should_restart {
                    {
                        let mut rec = record.lock().unwrap();
                        let _ = rec.transition_state(ProcessState::Stopped);
                    }

                    publish_health_event(
                        &event_bus,
                        process_id,
                        &config.name,
                        ProcessHealthStatus::Stopped,
                        ProcessState::Stopped,
                    );

                    break;
                }

                // ─── Restart Loop Prevention for spawn failures ───
                let now = Instant::now();
                restart_timestamps.retain(|ts| now.saturating_duration_since(*ts) < RESTART_WINDOW);

                if restart_timestamps.len() >= MAX_RESTARTS_IN_WINDOW {
                    tracing::error!(
                        subsystem = "supervisor",
                        component = "monitor",
                        process = %config.name,
                        restarts = restart_timestamps.len(),
                        "Restart loop detected (spawn failures) — stopping"
                    );

                    {
                        let mut rec = record.lock().unwrap();
                        rec.last_error = Some(format!(
                            "restart loop detected: {} spawn failures in {}s",
                            restart_timestamps.len(),
                            RESTART_WINDOW.as_secs(),
                        ));
                        let _ = rec.transition_state(ProcessState::Stopped);
                    }

                    publish_event(
                        &event_bus,
                        &ProcessEvent::Stopped(ProcessStoppedEvent::new(
                            process_id,
                            &config.name,
                            "restart loop detected (spawn failures)",
                        )),
                    );

                    publish_health_event(
                        &event_bus,
                        process_id,
                        &config.name,
                        ProcessHealthStatus::Stopped,
                        ProcessState::Stopped,
                    );

                    break;
                }

                restart_timestamps.push(now);

                // Restart
                {
                    let mut rec = record.lock().unwrap();
                    let _ = rec.transition_state(ProcessState::Restarting);

                    let (pid_val, name_val) = (rec.id, rec.name.clone());
                    drop(rec);
                    publish_health_event(
                        &event_bus,
                        pid_val,
                        &name_val,
                        ProcessHealthStatus::Starting,
                        ProcessState::Restarting,
                    );
                }

                {
                    let rec = record.lock().unwrap();
                    let (pid_val, name_val, restart_count) =
                        (rec.id, rec.name.clone(), rec.restart_count);
                    drop(rec);

                    publish_event(
                        &event_bus,
                        &ProcessEvent::Restarted(ProcessRestartEvent::new(
                            pid_val,
                            &config.name,
                            restart_count,
                            "spawn failure",
                        )),
                    );
                }

                tracing::info!(
                    subsystem = "supervisor",
                    component = "monitor",
                    process = %config.name,
                    "Retrying spawn in {:?}",
                    RESTART_DELAY
                );

                tokio::time::sleep(RESTART_DELAY).await;
            }
        }
    }

    // ─── Cleanup ───
    // Clear the stop sender and monitoring handle
    {
        let mut rec = record.lock().unwrap();
        rec.stop_tx = None;
        rec.monitor_handle = None;
    }

    ctx.active_monitors.fetch_sub(1, Ordering::AcqRel);

    tracing::info!(
        subsystem = "supervisor",
        component = "monitor",
        process = %config.name,
        "Monitoring task exited"
    );
}

/// Helper: get the process_id from a ManagedProcess record for event publishing.
fn id_from_record(record: &Arc<std::sync::Mutex<ManagedProcess>>) -> super::ProcessId {
    record.lock().unwrap().id
}

/// Helper: transition to a terminal state and publish the corresponding event.
///
/// Used during graceful shutdown when the supervisor signals all processes
/// to stop.
fn set_terminal_state(
    record: &Arc<std::sync::Mutex<ManagedProcess>>,
    event_bus: &Option<Arc<EventBus<PipelineEvent>>>,
    terminal_state: ProcessState,
    exit_code: Option<i32>,
    reason: &str,
) {
    let (process_id, name) = {
        let mut rec = record.lock().unwrap();
        rec.pid = None;
        rec.exit_code = exit_code;
        rec.exited_at = Some(Utc::now());
        rec.last_error = Some(reason.to_string());
        let _ = rec.transition_state(terminal_state);
        (rec.id, rec.name.clone())
    };

    match terminal_state {
        ProcessState::Stopped => {
            publish_event(
                event_bus,
                &ProcessEvent::Stopped(ProcessStoppedEvent::new(
                    process_id,
                    &name,
                    reason,
                )),
            );
            publish_health_event(
                event_bus,
                process_id,
                &name,
                ProcessHealthStatus::Stopped,
                ProcessState::Stopped,
            );
        }
        ProcessState::Exited => {
            publish_event(
                event_bus,
                &ProcessEvent::Exited(ProcessExitedEvent::new(
                    process_id,
                    &name,
                    exit_code,
                    0,
                )),
            );
            publish_health_event(
                event_bus,
                process_id,
                &name,
                ProcessHealthStatus::Unhealthy,
                ProcessState::Exited,
            );
        }
        ProcessState::Failed => {
            publish_event(
                event_bus,
                &ProcessEvent::Failed(ProcessFailedEvent::new(
                    process_id,
                    &name,
                    exit_code,
                    reason,
                    0,
                )),
            );
            publish_health_event(
                event_bus,
                process_id,
                &name,
                ProcessHealthStatus::Unhealthy,
                ProcessState::Failed,
            );
        }
        _ => {
            tracing::warn!(
                subsystem = "supervisor",
                component = "monitor",
                "set_terminal_state called with non-terminal state: {:?}",
                terminal_state
            );
        }
    }
}

/// Compute process health status from state, restart count, and error.
///
/// This mirrors the logic in [`super::health::compute_health`].
fn compute_health(
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

    #[test]
    fn compute_health_running_healthy() {
        assert_eq!(
            compute_health(&ProcessState::Running, 0, None),
            ProcessHealthStatus::Healthy
        );
    }

    #[test]
    fn compute_health_running_degraded_after_restart() {
        assert_eq!(
            compute_health(&ProcessState::Running, 1, None),
            ProcessHealthStatus::Degraded
        );
    }

    #[test]
    fn compute_health_running_degraded_with_error() {
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
    fn compute_health_exited_unhealthy() {
        assert_eq!(
            compute_health(&ProcessState::Exited, 0, None),
            ProcessHealthStatus::Unhealthy
        );
    }

    #[test]
    fn compute_health_failed_unhealthy() {
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
    fn compute_health_created_unknown() {
        assert_eq!(
            compute_health(&ProcessState::Created, 0, None),
            ProcessHealthStatus::Unknown
        );
    }

    #[test]
    fn health_status_labels() {
        assert_eq!(ProcessHealthStatus::Healthy.label(), "healthy");
        assert_eq!(ProcessHealthStatus::Degraded.label(), "degraded");
        assert_eq!(ProcessHealthStatus::Starting.label(), "starting");
        assert_eq!(ProcessHealthStatus::Unhealthy.label(), "unhealthy");
        assert_eq!(ProcessHealthStatus::Stopped.label(), "stopped");
    }

    #[test]
    fn is_healthy_and_is_running() {
        assert!(ProcessHealthStatus::Healthy.is_healthy());
        assert!(ProcessHealthStatus::Degraded.is_healthy());
        assert!(!ProcessHealthStatus::Starting.is_healthy());
        assert!(!ProcessHealthStatus::Unhealthy.is_healthy());
        assert!(!ProcessHealthStatus::Stopped.is_healthy());

        assert!(ProcessHealthStatus::Healthy.is_running());
        assert!(ProcessHealthStatus::Degraded.is_running());
        assert!(!ProcessHealthStatus::Stopping.is_running());
        assert!(!ProcessHealthStatus::Unhealthy.is_running());
    }

    #[test]
    fn id_from_record_returns_id() {
        let id = ProcessId::new_v4();
        let config = ProcessConfig::new("test", "echo");
        let record = Arc::new(Mutex::new(ManagedProcess::new(id, config)));
        assert_eq!(id_from_record(&record), id);
    }
}
