//! Integration tests for the `ProcessSupervisor` subsystem.
//!
//! These tests exercise the full supervisor lifecycle — from creation
//! through spawning managed processes, evaluating restart policies,
//! and shutting down — using real subprocesses on the host.

use std::time::Duration;

use tokio::runtime::Runtime;

use nabu_core::process_supervisor::{
    ProcessConfig, ProcessId, ProcessState, ProcessSupervisor, RestartPolicy,
};
use nabu_core::registry::lifecycle::{Lifecycle, LifecycleStage};

/// Spawn an echo process that exits immediately with code 0.
fn echo_config(name: &str) -> ProcessConfig {
    ProcessConfig::new(name, "echo").with_arg("hello".to_string())
}

#[test]
fn supervisor_lifecycle_transitions() {
    let supervisor = ProcessSupervisor::new();
    assert_eq!(supervisor.lifecycle_stage(), LifecycleStage::Created);
    assert!(!supervisor.is_initialized());
    assert!(!supervisor.is_running());
    assert!(!supervisor.is_shutdown());

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
fn spawn_requires_runtime() {
    let supervisor = ProcessSupervisor::new();
    assert!(supervisor.initialize().is_ok());
    assert!(supervisor.start().is_ok());

    // No tokio runtime available — spawn should fail
    let result = supervisor.spawn(echo_config("orphan"));
    assert!(result.is_err());
    assert!(
        matches!(result.unwrap_err(), nabu_core::process_supervisor::ProcessSupervisorError::NoRuntime),
        "should be NoRuntime error"
    );

    supervisor.shutdown().unwrap();
}

#[test]
fn spawn_and_wait_for_exit() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert!(supervisor.start().is_ok());

        let id = supervisor.spawn(echo_config("quick")).expect("spawn should succeed");

        // Wait for the process to exit on its own (echo exits immediately)
        tokio::time::sleep(Duration::from_millis(500)).await;

        let snapshot = supervisor.get_snapshot(id).expect("process should exist");
        assert!(
            snapshot.state == ProcessState::Exited || snapshot.state == ProcessState::Stopped,
            "process should have exited or been stopped, got {:?}",
            snapshot.state
        );
        assert!(snapshot.exit_code.is_some());
        assert_eq!(snapshot.exit_code, Some(0));

        supervisor.shutdown().unwrap();
    });
}

#[test]
fn spawn_and_stop_running_process() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert!(supervisor.start().is_ok());

        // Spawn a long-running process
        let config = ProcessConfig::new("sleeper", "sleep")
            .with_arg("30".to_string())
            .with_restart_policy(RestartPolicy::Never);

        let id = supervisor.spawn(config).expect("spawn should succeed");

        // Wait for it to start
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(supervisor.get_state(id), Some(ProcessState::Running));

        // Stop it
        supervisor.stop(id).expect("stop should succeed");

        // Wait for the monitoring task to process the stop signal
        for _ in 0..50 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            if supervisor
                .get_state(id)
                .map(|s| s.is_terminal())
                .unwrap_or(false)
            {
                break;
            }
        }

        assert_eq!(
            supervisor.get_state(id),
            Some(ProcessState::Stopped),
            "process should be stopped after stop()"
        );

        supervisor.shutdown().unwrap();
    });
}

#[test]
fn restart_policy_never_does_not_restart() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert!(supervisor.start().is_ok());

        // Spawn a command that exits with code 1 and never restarts
        let config = ProcessConfig::new("failer", "sh")
            .with_args(vec!["-c".to_string(), "exit 1".to_string()])
            .with_restart_policy(RestartPolicy::Never);

        let id = supervisor.spawn(config).expect("spawn should succeed");

        // Wait for the process to fail
        tokio::time::sleep(Duration::from_millis(500)).await;

        let snapshot = supervisor.get_snapshot(id).expect("process should exist");
        assert_eq!(snapshot.state, ProcessState::Failed);
        assert_eq!(snapshot.restart_count, 1);
        assert!(
            snapshot.exit_code == Some(1),
            "exit code should be 1, got {:?}",
            snapshot.exit_code
        );

        supervisor.shutdown().unwrap();
    });
}

#[test]
fn restart_policy_always_restarts_on_failure() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert!(supervisor.start().is_ok());

        // Spawn a command that always fails
        let config = ProcessConfig::new("failer", "sh")
            .with_args(vec!["-c".to_string(), "exit 1".to_string()])
            .with_restart_policy(RestartPolicy::Always);

        let id = supervisor.spawn(config).expect("spawn should succeed");

        // Wait for at least one restart to occur
        tokio::time::sleep(Duration::from_millis(800)).await;

        let snapshot = supervisor.get_snapshot(id).expect("process should exist");

        // With Always policy, the process should have been restarted at least once
        // It may be in Starting, Running, Exited, Failed, or Restarting state
        assert!(
            snapshot.restart_count >= 1,
            "should have restarted at least once, got restart_count = {}",
            snapshot.restart_count
        );

        supervisor.stop(id).ok();
        supervisor.shutdown().unwrap();
    });
}

#[test]
fn restart_policy_limited_retries_stops_after_limit() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert!(supervisor.start().is_ok());

        // Spawn a command that always fails, with limited retries (2)
        let config = ProcessConfig::new("failer", "sh")
            .with_args(vec!["-c".to_string(), "exit 7".to_string()])
            .with_restart_policy(RestartPolicy::limited_retries(2));

        let id = supervisor.spawn(config).expect("spawn should succeed");

        // Wait for restarts to be exhausted
        tokio::time::sleep(Duration::from_millis(800)).await;

        let snapshot = supervisor.get_snapshot(id).expect("process should exist");

        // After 2 exits (restart_count = 2), the policy says don't restart.
        // The process should be in Failed (terminal, no more restarts)
        assert_eq!(
            snapshot.restart_count, 2,
            "should have restarted exactly 2 times (restart_count = 2)"
        );
        assert!(
            snapshot.state == ProcessState::Failed || snapshot.state == ProcessState::Stopped,
            "process should be in terminal state after retries exhausted, got {:?}",
            snapshot.state
        );

        supervisor.shutdown().unwrap();
    });
}

#[test]
fn stop_nonexistent_process_returns_error() {
    let supervisor = ProcessSupervisor::new();
    assert!(supervisor.initialize().is_ok());
    assert!(supervisor.start().is_ok());

    let fake_id = ProcessId::new_v4();
    let result = supervisor.stop(fake_id);
    assert!(result.is_err());
    assert!(result.unwrap_err().is_not_found());

    supervisor.shutdown().unwrap();
}

#[test]
fn list_processes_returns_all_spawned() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert!(supervisor.start().is_ok());

        let id1 = supervisor
            .spawn(echo_config("proc1"))
            .expect("spawn should succeed");
        let id2 = supervisor
            .spawn(echo_config("proc2"))
            .expect("spawn should succeed");

        // Give processes time to exit
        tokio::time::sleep(Duration::from_millis(300)).await;

        let processes = supervisor.list_processes();
        assert!(
            processes.len() >= 2,
            "should have at least 2 processes, got {}",
            processes.len()
        );

        let ids: Vec<ProcessId> = processes.iter().map(|s| s.id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));

        supervisor.shutdown().unwrap();
    });
}

#[test]
fn process_count_and_running_count() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert!(supervisor.start().is_ok());

        assert_eq!(supervisor.process_count(), 0);
        assert_eq!(supervisor.running_count(), 0);

        let id = supervisor
            .spawn(echo_config("quick"))
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(supervisor.process_count(), 1);
        assert_eq!(supervisor.running_count(), 0); // echo exits quickly

        supervisor.shutdown().unwrap();
    });
}

#[test]
fn shutdown_cleans_up_all_processes() {
    let rt = Runtime::new().unwrap();
    rt.block_on(async {
        let supervisor = ProcessSupervisor::new();
        assert!(supervisor.initialize().is_ok());
        assert!(supervisor.start().is_ok());

        let id1 = supervisor
            .spawn(ProcessConfig::new("sleeper1", "sleep").with_arg("30".to_string()))
            .expect("spawn should succeed");
        let id2 = supervisor
            .spawn(ProcessConfig::new("sleeper2", "sleep").with_arg("30".to_string()))
            .expect("spawn should succeed");

        tokio::time::sleep(Duration::from_millis(100)).await;

        assert_eq!(supervisor.process_count(), 2);
        assert_eq!(supervisor.running_count(), 2);

        // Shutdown should stop all processes
        assert!(supervisor.shutdown().is_ok());
    });
}
