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
use std::time::Duration;

use chrono::Utc;
use tokio::process::Command;
use tokio::sync::broadcast;

use crate::event_bus::events::{
    ProcessExitedEvent, ProcessFailedEvent, ProcessRestartEvent, ProcessStartedEvent,
    ProcessStoppedEvent,
};
use crate::event_bus::{EventBus, PipelineEvent, ProcessEvent};

use super::config::ProcessConfig;
use super::managed::ManagedProcess;
use super::state::ProcessState;

/// The fixed delay applied between a process exit and a restart attempt.
///
/// This is intentionally simple — no exponential backoff. It prevents
/// tight crash loops without introducing scheduling complexity.
const RESTART_DELAY: Duration = Duration::from_millis(100);

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
fn publish_event(
    event_bus: &Option<Arc<EventBus<PipelineEvent>>>,
    event: &ProcessEvent,
) {
    if let Some(bus) = event_bus {
        bus.publish(event.kind(), &PipelineEvent::Process(event.clone()));
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
pub(crate) async fn monitor_process(
    config: ProcessConfig,
    record: Arc<std::sync::Mutex<ManagedProcess>>,
    event_bus: Option<Arc<EventBus<PipelineEvent>>>,
    ctx: Arc<SupervisorContext>,
    mut stop_rx: broadcast::Receiver<()>,
) {
    ctx.active_monitors.fetch_add(1, Ordering::AcqRel);

    tracing::info!(
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
            let _ = rec.transition_state(ProcessState::Starting);
            rec.last_error = None;
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
        cmd.args(&config.args).envs(&config.env).kill_on_drop(true);

        let spawn_result = if let Some(dir) = &config.working_dir {
            cmd.current_dir(dir).spawn()
        } else {
            cmd.spawn()
        };

        match spawn_result {
            Ok(mut child) => {
                let pid = child.id();

                // Capture snapshot values while holding the lock briefly
                let (process_id, _restart_count) = {
                    let mut rec = record.lock().unwrap();
                    rec.pid = pid;
                    let _ = rec.transition_state(ProcessState::Running);
                    rec.started_at = Some(Utc::now());
                    rec.last_error = None;
                    (rec.id, rec.restart_count)
                };

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
                        process_id,
                        &config.name,
                        pid,
                        &config.command,
                        &config.args,
                        config.working_dir.as_ref().and_then(|p| p.to_str()),
                    )),
                );

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
                            "Stop signal received — killing process"
                        );
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

                // Update the record with exit information
                {
                    let mut rec = record.lock().unwrap();
                    rec.pid = None;
                    rec.exit_code = exit_code;
                    rec.exited_at = Some(Utc::now());
                    rec.restart_count += 1;
                    rec.last_error = error_msg.clone();
                    // When stopping, transition through the Stopping
                    // intermediate state first (Running → Stopping → Stopped).
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

                let should_restart = {
                    let rec = record.lock().unwrap();
                    let policy = &rec.config.restart_policy;
                    policy.should_restart(terminal_state, exit_code, rec.restart_count)
                };

                if !should_restart {
                    break;
                }

                // ─── Restart ───
                {
                    let mut rec = record.lock().unwrap();
                    let _ = rec.transition_state(ProcessState::Restarting);
                }

                let (process_id, restart_count) = {
                    let rec = record.lock().unwrap();
                    (rec.id, rec.restart_count)
                };

                publish_event(
                    &event_bus,
                    &ProcessEvent::Restarted(ProcessRestartEvent::new(
                        process_id,
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

                // Evaluate restart policy for spawn failure
                let should_restart = if ctx.is_shutting_down() {
                    false
                } else {
                    let rec = record.lock().unwrap();
                    let policy = &rec.config.restart_policy;
                    policy.should_restart(ProcessState::Failed, None, rec.restart_count)
                };

                if !should_restart {
                    break;
                }

                // Restart
                {
                    let mut rec = record.lock().unwrap();
                    let _ = rec.transition_state(ProcessState::Restarting);
                }

                let (process_id, restart_count) = {
                    let rec = record.lock().unwrap();
                    (rec.id, rec.restart_count)
                };

                publish_event(
                    &event_bus,
                    &ProcessEvent::Restarted(ProcessRestartEvent::new(
                        process_id,
                        &config.name,
                        restart_count,
                        "spawn failure",
                    )),
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
