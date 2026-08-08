//! # Agent Registry — Thread-Safe Registry for Managed Agents
//!
//! [`AgentRegistry`] provides a thread-safe container for tracking all
//! registered agents managed by the [`AgentManager`](super::AgentManager).
//!
//! ## Architecture
//!
//! ```text
//! AgentRegistry (Arc<AgentRegistry>)
//! └── agents: RwLock<HashMap<AgentName, Arc<Mutex<AgentProcess>>>>
//! ```
//!
//! Each agent is stored as an `Arc<Mutex<AgentProcess>>` so that:
//! - The `AgentManager` can read agent state from synchronous API methods.
//! - Background monitoring tasks can update agent state.
//! - Multiple threads can safely access different agents concurrently.
//!
//! The registry owns the `Arc<Mutex<AgentProcess>>` records — it does not
//! own the underlying `tokio::process::Child`. The child process is owned
//! exclusively by the `ProcessSupervisor`'s monitoring task.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use crate::process_supervisor::ProcessId;

use super::config::AgentName;
use super::process::{AgentProcess, AgentProcessState, AgentSnapshot};

/// Error returned by registry operations.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum RegistryError {
    /// The agent with the given name was not found.
    #[error("agent not found: {0}")]
    NotFound(String),

    /// An agent with the given name is already registered.
    #[error("agent already registered: {0}")]
    AlreadyRegistered(String),
}

/// Result type for registry operations.
pub type RegistryResult<T> = Result<T, RegistryError>;

/// A thread-safe registry for tracking managed agents.
///
/// The registry maps agent names to `Arc<Mutex<AgentProcess>>` records.
/// It is `Send + Sync` and designed to be shared via `Arc` across threads.
///
/// ## Usage
///
/// ```ignore
/// use nabu_core::agent::{AgentRegistry, AgentConfig};
/// use std::sync::Arc;
///
/// let registry = Arc::new(AgentRegistry::new());
/// let config = AgentConfig::new("my-agent", "echo");
///
/// let process = AgentProcess::new(config);
/// registry.register(process).unwrap();
/// let snapshot = registry.snapshot("my-agent").unwrap();
/// ```
pub struct AgentRegistry {
    /// All registered agents, keyed by agent name.
    /// Each value is behind its own `Mutex` for fine-grained locking.
    agents: RwLock<HashMap<AgentName, Arc<Mutex<AgentProcess>>>>,
}

impl AgentRegistry {
    /// Create a new, empty agent registry.
    pub fn new() -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new agent.
    ///
    /// Returns `Err(AlreadyRegistered)` if an agent with the same name
    /// already exists. The caller should use [`unregister`](Self::unregister)
    /// first if they want to replace an existing agent.
    ///
    /// # Errors
    ///
    /// - [`RegistryError::AlreadyRegistered`] if the name is already in use.
    pub fn register(&self, process: AgentProcess) -> RegistryResult<()> {
        let name = process.name().to_string();

        let mut agents = self
            .agents
            .write()
            .expect("agent registry lock poisoned");

        if agents.contains_key(&name) {
            return Err(RegistryError::AlreadyRegistered(name));
        }

        agents.insert(name.clone(), Arc::new(Mutex::new(process)));
        tracing::debug!(
            subsystem = "agent_manager",
            component = "registry",
            operation = "register",
            agent = %name,
            "Agent registered"
        );

        Ok(())
    }

    /// Returns `true` if an agent with the given name is registered.
    pub fn has(&self, name: &str) -> bool {
        self.agents
            .read()
            .expect("agent registry lock poisoned")
            .contains_key(name)
    }

    /// Get a reference-counted handle to an agent process.
    ///
    /// Returns `None` if no agent with the given name is registered.
    pub fn get(&self, name: &str) -> Option<Arc<Mutex<AgentProcess>>> {
        self.agents
            .read()
            .expect("agent registry lock poisoned")
            .get(name)
            .cloned()
    }

    /// Unregister and remove an agent from the registry.
    ///
    /// Returns `Err(NotFound)` if no agent with the given name is registered.
    ///
    /// # Safety
    ///
    /// The caller is responsible for ensuring the underlying process has
    /// been stopped before calling this. The registry does not interact
    /// with the `ProcessSupervisor`.
    pub fn unregister(&self, name: &str) -> RegistryResult<()> {
        let mut agents = self
            .agents
            .write()
            .expect("agent registry lock poisoned");

        if agents.remove(name).is_some() {
            tracing::debug!(
                subsystem = "agent_manager",
                component = "registry",
                operation = "unregister",
                agent = %name,
                "Agent unregistered"
            );
            Ok(())
        } else {
            Err(RegistryError::NotFound(name.to_string()))
        }
    }

    /// Returns the number of registered agents.
    pub fn count(&self) -> usize {
        self.agents
            .read()
            .expect("agent registry lock poisoned")
            .len()
    }

    /// Returns the number of agents currently in the `Running` management state.
    pub fn running_count(&self) -> usize {
        let agents = self
            .agents
            .read()
            .expect("agent registry lock poisoned");
        agents
            .values()
            .filter(|proc| proc.lock().expect("agent process lock poisoned").is_running())
            .count()
    }

    /// Returns the number of agents in terminal states.
    pub fn stopped_count(&self) -> usize {
        let agents = self
            .agents
            .read()
            .expect("agent registry lock poisoned");
        agents
            .values()
            .filter(|proc| proc.lock().expect("agent process lock poisoned").is_stopped())
            .count()
    }

    /// Returns a snapshot of all registered agents.
    ///
    /// The snapshot is a point-in-time copy — mutations to the registry
    /// after this call will not be reflected.
    pub fn snapshots(&self) -> Vec<AgentSnapshot> {
        let agents = self
            .agents
            .read()
            .expect("agent registry lock poisoned");
        agents
            .values()
            .map(|proc| {
                let guard = proc.lock().expect("agent process lock poisoned");
                guard.snapshot()
            })
            .collect()
    }

    /// Returns the names of all registered agents.
    pub fn names(&self) -> Vec<String> {
        let agents = self
            .agents
            .read()
            .expect("agent registry lock poisoned");
        agents.keys().cloned().collect()
    }

    /// Returns the process ID of a registered agent, if it has one.
    ///
    /// Returns `None` if the agent is not registered or has no process ID
    /// (e.g. it was stopped).
    pub fn process_id(&self, name: &str) -> Option<ProcessId> {
        let agents = self
            .agents
            .read()
            .expect("agent registry lock poisoned");
        agents.get(name).and_then(|proc| {
            proc.lock().expect("agent process lock poisoned").process_id()
        })
    }

    /// Returns the agent's management state, if registered.
    pub fn state(&self, name: &str) -> Option<AgentProcessState> {
        let agents = self
            .agents
            .read()
            .expect("agent registry lock poisoned");
        agents.get(name).map(|proc| {
            proc.lock().expect("agent process lock poisoned").state()
        })
    }

    /// Returns a snapshot of a specific agent, if registered.
    pub fn snapshot(&self, name: &str) -> Option<AgentSnapshot> {
        let agents = self
            .agents
            .read()
            .expect("agent registry lock poisoned");
        agents.get(name).map(|proc| {
            proc.lock().expect("agent process lock poisoned").snapshot()
        })
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::config::AgentConfig;
    use crate::process_supervisor::RestartPolicy;

    #[test]
    fn new_registry_is_empty() {
        let registry = AgentRegistry::new();
        assert_eq!(registry.count(), 0);
        assert_eq!(registry.running_count(), 0);
        assert_eq!(registry.stopped_count(), 0);
        assert!(registry.names().is_empty());
    }

    #[test]
    fn register_and_retrieve() {
        let registry = AgentRegistry::new();
        let config = AgentConfig::new("test-agent", "echo");
        let process = AgentProcess::new(config);

        let result = registry.register(process);
        assert!(result.is_ok());
        assert_eq!(registry.count(), 1);
        assert!(registry.has("test-agent"));
        assert!(!registry.has("other-agent"));
    }

    #[test]
    fn register_duplicate_fails() {
        let registry = AgentRegistry::new();
        let config = AgentConfig::new("test-agent", "echo");

        registry.register(AgentProcess::new(config.clone())).unwrap();
        let result = registry.register(AgentProcess::new(config));
        assert!(matches!(result, Err(RegistryError::AlreadyRegistered(_))));
    }

    #[test]
    fn unregister_removes_agent() {
        let registry = AgentRegistry::new();
        let config = AgentConfig::new("test-agent", "echo");
        registry.register(AgentProcess::new(config)).unwrap();

        assert!(registry.has("test-agent"));
        registry.unregister("test-agent").unwrap();
        assert!(!registry.has("test-agent"));
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn unregister_nonexistent_fails() {
        let registry = AgentRegistry::new();
        let result = registry.unregister("no-such-agent");
        assert!(matches!(result, Err(RegistryError::NotFound(_))));
    }

    #[test]
    fn get_returns_process_handle() {
        let registry = AgentRegistry::new();
        let config = AgentConfig::new("test-agent", "echo");
        registry.register(AgentProcess::new(config)).unwrap();

        let handle = registry.get("test-agent");
        assert!(handle.is_some());
        let handle = handle.unwrap();
        let proc = handle.lock().expect("lock poisoned");
        assert_eq!(proc.name(), "test-agent");
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let registry = AgentRegistry::new();
        assert!(registry.get("no-such-agent").is_none());
    }

    #[test]
    fn snapshot_returns_agent_snapshot() {
        let registry = AgentRegistry::new();
        let config = AgentConfig::new("test-agent", "echo")
            .with_args(vec!["hello".to_string()])
            .with_restart_policy(RestartPolicy::Always);
        registry.register(AgentProcess::new(config)).unwrap();

        let snapshot = registry.snapshot("test-agent");
        assert!(snapshot.is_some());
        let snapshot = snapshot.unwrap();
        assert_eq!(snapshot.name, "test-agent");
        assert_eq!(snapshot.config.process.args, vec!["hello".to_string()]);
        assert_eq!(snapshot.agent_state, AgentProcessState::Registered);
    }

    #[test]
    fn snapshots_returns_all() {
        let registry = AgentRegistry::new();
        registry.register(AgentProcess::new(AgentConfig::new("a", "echo"))).unwrap();
        registry.register(AgentProcess::new(AgentConfig::new("b", "echo"))).unwrap();

        let snapshots = registry.snapshots();
        assert_eq!(snapshots.len(), 2);

        let names: Vec<&str> = snapshots.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"a"));
        assert!(names.contains(&"b"));
    }

    #[test]
    fn names_returns_all() {
        let registry = AgentRegistry::new();
        registry.register(AgentProcess::new(AgentConfig::new("alpha", "echo"))).unwrap();
        registry.register(AgentProcess::new(AgentConfig::new("beta", "echo"))).unwrap();

        let names = registry.names();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["alpha", "beta"]);
    }

    #[test]
    fn counts_are_correct() {
        let registry = AgentRegistry::new();

        let config1 = AgentConfig::new("agent-1", "echo");
        let config2 = AgentConfig::new("agent-2", "echo");
        let mut proc1 = AgentProcess::new(config1);
        let mut proc2 = AgentProcess::new(config2);
        proc1.mark_started(ProcessId::new_v4());
        proc1.mark_running();
        proc2.mark_stopped(false, None);

        registry.register(proc1).unwrap();
        registry.register(proc2).unwrap();

        assert_eq!(registry.count(), 2);
        assert_eq!(registry.running_count(), 1);
        assert_eq!(registry.stopped_count(), 1);
    }

    #[test]
    fn process_id_returns_none_for_unstarted() {
        let registry = AgentRegistry::new();
        registry.register(AgentProcess::new(AgentConfig::new("test", "echo"))).unwrap();
        assert!(registry.process_id("test").is_none());
    }

    #[test]
    fn process_id_returns_id_after_mark_started() {
        let registry = AgentRegistry::new();
        registry.register(AgentProcess::new(AgentConfig::new("test", "echo"))).unwrap();

        let handle = registry.get("test").unwrap();
        let id = ProcessId::new_v4();
        handle.lock().expect("lock").mark_started(id);

        assert_eq!(registry.process_id("test"), Some(id));
    }

    #[test]
    fn state_returns_agent_state() {
        let registry = AgentRegistry::new();
        registry.register(AgentProcess::new(AgentConfig::new("test", "echo"))).unwrap();

        let handle = registry.get("test").unwrap();
        let id = ProcessId::new_v4();
        {
            let mut proc = handle.lock().expect("lock");
            proc.mark_started(id);
            proc.mark_running();
        }

        assert_eq!(
            registry.state("test"),
            Some(AgentProcessState::Running)
        );
    }

    #[test]
    fn default_registry_is_empty() {
        let registry = AgentRegistry::default();
        assert_eq!(registry.count(), 0);
    }

    #[test]
    fn registry_error_display() {
        let err = RegistryError::NotFound("test".to_string());
        assert!(format!("{}", err).contains("not found"));

        let err = RegistryError::AlreadyRegistered("test".to_string());
        assert!(format!("{}", err).contains("already registered"));
    }
}
