//! Lifecycle hooks for the plugin architecture.
//!
//! Provides [`PluginLifecycle`] — a state machine that governs how both
//! built-in components and future plugins transition through their
//! lifecycle stages:
//!
//! ```text
//! Discovered → Registered → Initialized → Started → Stopped → Unloaded
//! ```
//!
//! Lifecycle hooks are callbacks that fire at each stage. Built-in
//! services participate in this lifecycle today; third-party plugins
//! will use the same infrastructure in the future.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, RwLock};

use crate::plugin::PluginId;

/// The six stages of the plugin lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum LifecycleStage {
    /// The plugin has been discovered but not yet processed.
    Discovered = 0,
    /// The plugin's manifest has been validated and registered.
    Registered = 1,
    /// The plugin has been initialized with its dependencies resolved.
    Initialized = 2,
    /// The plugin is actively running and providing services.
    Started = 3,
    /// The plugin has been stopped and is no longer providing services.
    Stopped = 4,
    /// The plugin has been fully unloaded and cleaned up.
    Unloaded = 5,
}

impl LifecycleStage {
    /// Returns the name of this stage as a string.
    pub fn name(&self) -> &'static str {
        match self {
            LifecycleStage::Discovered => "discovered",
            LifecycleStage::Registered => "registered",
            LifecycleStage::Initialized => "initialized",
            LifecycleStage::Started => "started",
            LifecycleStage::Stopped => "stopped",
            LifecycleStage::Unloaded => "unloaded",
        }
    }

    /// Returns the next valid stage in the lifecycle.
    pub fn next(&self) -> Option<LifecycleStage> {
        match self {
            LifecycleStage::Discovered => Some(LifecycleStage::Registered),
            LifecycleStage::Registered => Some(LifecycleStage::Initialized),
            LifecycleStage::Initialized => Some(LifecycleStage::Started),
            LifecycleStage::Started => Some(LifecycleStage::Stopped),
            LifecycleStage::Stopped => Some(LifecycleStage::Unloaded),
            LifecycleStage::Unloaded => None,
        }
    }

    /// Returns `true` if this stage allows the component to provide services.
    pub fn is_active(&self) -> bool {
        matches!(self, LifecycleStage::Started)
    }

    /// Returns `true` if this stage is terminal (cannot progress).
    pub fn is_terminal(&self) -> bool {
        matches!(self, LifecycleStage::Unloaded)
    }
}

impl fmt::Display for LifecycleStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Error returned when an invalid lifecycle transition is attempted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleError {
    /// The plugin or component that attempted the transition.
    pub plugin_id: PluginId,
    /// The current stage of the component.
    pub current_stage: LifecycleStage,
    /// The stage that was attempted.
    pub attempted_stage: LifecycleStage,
    /// Optional error message.
    pub message: String,
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Plugin '{}': cannot transition from {} to {}: {}",
            self.plugin_id, self.current_stage, self.attempted_stage, self.message
        )
    }
}

impl std::error::Error for LifecycleError {}

/// Type alias for lifecycle hook functions.
///
/// Hooks receive the plugin ID and the current lifecycle stage.
type LifecycleHook = Box<dyn Fn(&str, LifecycleStage) + Send + Sync>;

/// Per-plugin lifecycle state and hooks.
///
/// Each plugin or built-in component has its own [`PluginLifecycle`] instance
/// that tracks its current stage and provides hooks for each transition.
///
/// # Thread Safety
///
/// Uses an atomic u8 for the stage to allow lock-free reads.
/// Hooks are stored behind a read-write lock.
pub struct PluginLifecycle {
    /// Unique identifier for this plugin/component.
    id: PluginId,
    /// Current lifecycle stage (atomic for lock-free reads).
    stage: AtomicU8,
    /// Hooks registered for each lifecycle transition.
    hooks: RwLock<HashMap<LifecycleStage, Vec<LifecycleHook>>>,
    /// Whether this lifecycle has been finalized (prevent further transitions).
    finalized: RwLock<bool>,
}

impl PluginLifecycle {
    /// Creates a new lifecycle starting at the `Discovered` stage.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            stage: AtomicU8::new(LifecycleStage::Discovered as u8),
            hooks: RwLock::new(HashMap::new()),
            finalized: RwLock::new(false),
        }
    }

    /// Creates a new lifecycle starting at a specific stage.
    pub fn at_stage(id: impl Into<String>, stage: LifecycleStage) -> Self {
        Self {
            id: id.into(),
            stage: AtomicU8::new(stage as u8),
            hooks: RwLock::new(HashMap::new()),
            finalized: RwLock::new(false),
        }
    }

    /// Returns the current lifecycle stage.
    pub fn current_stage(&self) -> LifecycleStage {
        match self.stage.load(Ordering::Acquire) {
            0 => LifecycleStage::Discovered,
            1 => LifecycleStage::Registered,
            2 => LifecycleStage::Initialized,
            3 => LifecycleStage::Started,
            4 => LifecycleStage::Stopped,
            5 => LifecycleStage::Unloaded,
            _ => LifecycleStage::Discovered,
        }
    }

    /// Returns the plugin ID associated with this lifecycle.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Attempts to transition to the next lifecycle stage.
    ///
    /// Returns `Ok(())` on success, or a [`LifecycleError`] if the
    /// transition is invalid.
    pub fn advance(&self) -> Result<LifecycleStage, LifecycleError> {
        let current = self.current_stage();
        let next = current.next().ok_or_else(|| LifecycleError {
            plugin_id: self.id.clone(),
            current_stage: current,
            attempted_stage: current, // No valid next stage
            message: "Already at terminal stage".to_string(),
        })?;

        self.transition_to(next)
    }

    /// Attempts to transition to a specific lifecycle stage.
    ///
    /// Only allows forward transitions to the immediate next stage.
    /// Returns an error for invalid transitions.
    pub fn transition_to(&self, target: LifecycleStage) -> Result<LifecycleStage, LifecycleError> {
        let current = self.current_stage();

        // Check if finalized
        {
            let finalized = self.finalized.read().expect("lifecycle finalized lock");
            if *finalized {
                return Err(LifecycleError {
                    plugin_id: self.id.clone(),
                    current_stage: current,
                    attempted_stage: target,
                    message: "Lifecycle is finalized".to_string(),
                });
            }
        }

        // Check that the transition is valid (must be exactly the next stage)
        let expected_next = current.next();
        match expected_next {
            Some(next) if next == target => {
                self.stage.store(target as u8, Ordering::Release);

                // Fire hooks for the new stage
                let hooks = self.hooks.read().expect("lifecycle hooks lock");
                if let Some(hooks_for_stage) = hooks.get(&target) {
                    for hook in hooks_for_stage {
                        hook(&self.id, target);
                    }
                }

                Ok(target)
            }
            Some(_) => Err(LifecycleError {
                plugin_id: self.id.clone(),
                current_stage: current,
                attempted_stage: target,
                message: format!(
                    "Can only transition to {}, not {}",
                    current.next().map(|s| s.name()).unwrap_or("<none>"),
                    target.name()
                ),
            }),
            None => Err(LifecycleError {
                plugin_id: self.id.clone(),
                current_stage: current,
                attempted_stage: target,
                message: "Cannot transition from terminal stage".to_string(),
            }),
        }
    }

    /// Registers a hook that fires when the lifecycle reaches the given stage.
    pub fn on_stage<F>(&self, stage: LifecycleStage, hook: F)
    where
        F: Fn(&str, LifecycleStage) + Send + Sync + 'static,
    {
        let mut hooks = self.hooks.write().expect("lifecycle hooks lock");
        hooks.entry(stage).or_default().push(Box::new(hook));
    }

    /// Registers a hook that fires when the lifecycle enters `Discovered`.
    pub fn on_discovered<F>(&self, hook: F)
    where
        F: Fn(&str, LifecycleStage) + Send + Sync + 'static,
    {
        self.on_stage(LifecycleStage::Discovered, hook);
    }

    /// Registers a hook that fires when the lifecycle enters `Registered`.
    pub fn on_registered<F>(&self, hook: F)
    where
        F: Fn(&str, LifecycleStage) + Send + Sync + 'static,
    {
        self.on_stage(LifecycleStage::Registered, hook);
    }

    /// Registers a hook that fires when the lifecycle enters `Initialized`.
    pub fn on_initialized<F>(&self, hook: F)
    where
        F: Fn(&str, LifecycleStage) + Send + Sync + 'static,
    {
        self.on_stage(LifecycleStage::Initialized, hook);
    }

    /// Registers a hook that fires when the lifecycle enters `Started`.
    pub fn on_started<F>(&self, hook: F)
    where
        F: Fn(&str, LifecycleStage) + Send + Sync + 'static,
    {
        self.on_stage(LifecycleStage::Started, hook);
    }

    /// Registers a hook that fires when the lifecycle enters `Stopped`.
    pub fn on_stopped<F>(&self, hook: F)
    where
        F: Fn(&str, LifecycleStage) + Send + Sync + 'static,
    {
        self.on_stage(LifecycleStage::Stopped, hook);
    }

    /// Registers a hook that fires when the lifecycle enters `Unloaded`.
    pub fn on_unloaded<F>(&self, hook: F)
    where
        F: Fn(&str, LifecycleStage) + Send + Sync + 'static,
    {
        self.on_stage(LifecycleStage::Unloaded, hook);
    }

    /// Finalizes the lifecycle, preventing further transitions.
    ///
    /// This is called during shutdown to prevent race conditions.
    pub fn finalize(&self) {
        let mut finalized = self.finalized.write().expect("lifecycle finalized lock");
        *finalized = true;
    }

    /// Runs a full lifecycle progression: Discovered → ... → Started.
    ///
    /// Useful for quick initialization of built-in components.
    pub fn boot(&self) -> Result<(), LifecycleError> {
        self.advance()?; // Discovered → Registered
        self.advance()?; // Registered → Initialized
        self.advance()?; // Initialized → Started
        Ok(())
    }

    /// Runs a full shutdown progression: Started → ... → Unloaded.
    pub fn shutdown(&self) -> Result<(), LifecycleError> {
        self.advance()?; // Started → Stopped
        self.advance()?; // Stopped → Unloaded
        Ok(())
    }
}

impl fmt::Debug for PluginLifecycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginLifecycle")
            .field("id", &self.id)
            .field("stage", &self.current_stage())
            .field("finalized", &self.finalized)
            .finish()
    }
}

impl fmt::Display for PluginLifecycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PluginLifecycle({}: {})", self.id, self.current_stage())
    }
}

/// Manager for multiple plugin lifecycles.
///
/// Provides batch operations like booting all plugins or shutting down.
#[derive(Debug, Default)]
pub struct LifecycleManager {
    lifecycles: RwLock<HashMap<PluginId, Arc<PluginLifecycle>>>,
}

impl LifecycleManager {
    /// Creates a new empty lifecycle manager.
    pub fn new() -> Self {
        Self {
            lifecycles: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a lifecycle for a plugin or component.
    pub fn register(&self, lifecycle: PluginLifecycle) {
        let mut lifecycles = self.lifecycles.write().expect("lifecycle manager lock");
        lifecycles.insert(lifecycle.id().to_string(), Arc::new(lifecycle));
    }

    /// Returns the lifecycle for a plugin, if registered.
    pub fn get(&self, id: &str) -> Option<Arc<PluginLifecycle>> {
        let lifecycles = self.lifecycles.read().expect("lifecycle manager lock");
        lifecycles.get(id).cloned()
    }

    /// Advances a specific plugin to the next stage.
    pub fn advance(&self, id: &str) -> Result<LifecycleStage, LifecycleError> {
        let lifecycles = self.lifecycles.read().expect("lifecycle manager lock");
        if let Some(lc) = lifecycles.get(id) {
            lc.advance()
        } else {
            Err(LifecycleError {
                plugin_id: id.to_string(),
                current_stage: LifecycleStage::Discovered,
                attempted_stage: LifecycleStage::Discovered,
                message: "Plugin not registered".to_string(),
            })
        }
    }

    /// Boots all plugins that are in the `Discovered` stage.
    ///
    /// Returns a list of errors for plugins that failed to boot.
    pub fn boot_all(&self) -> Vec<LifecycleError> {
        let lifecycles = self.lifecycles.read().expect("lifecycle manager lock");
        let mut errors = Vec::new();
        for lc in lifecycles.values() {
            if lc.current_stage() == LifecycleStage::Discovered {
                if let Err(e) = lc.boot() {
                    errors.push(e);
                }
            }
        }
        errors
    }

    /// Shuts down all plugins gracefully.
    ///
    /// Returns a list of errors for plugins that failed to shut down.
    pub fn shutdown_all(&self) -> Vec<LifecycleError> {
        let lifecycles = self.lifecycles.read().expect("lifecycle manager lock");
        let mut errors = Vec::new();
        for lc in lifecycles.values() {
            if lc.current_stage() == LifecycleStage::Started {
                if let Err(e) = lc.shutdown() {
                    errors.push(e);
                }
            }
        }
        errors
    }

    /// Returns the number of registered lifecycles.
    pub fn count(&self) -> usize {
        let lifecycles = self.lifecycles.read().expect("lifecycle manager lock");
        lifecycles.len()
    }

    /// Returns all registered lifecycle IDs.
    pub fn all_ids(&self) -> Vec<PluginId> {
        let lifecycles = self.lifecycles.read().expect("lifecycle manager lock");
        lifecycles.keys().cloned().collect()
    }

    /// Returns all lifecycles at a given stage.
    pub fn at_stage(&self, stage: LifecycleStage) -> Vec<Arc<PluginLifecycle>> {
        let lifecycles = self.lifecycles.read().expect("lifecycle manager lock");
        lifecycles
            .values()
            .filter(|lc| lc.current_stage() == stage)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_stage() {
        let lc = PluginLifecycle::new("test-plugin");
        assert_eq!(lc.current_stage(), LifecycleStage::Discovered);
    }

    #[test]
    fn forward_transition() {
        let lc = PluginLifecycle::new("test");
        assert_eq!(lc.advance().unwrap(), LifecycleStage::Registered);
        assert_eq!(lc.current_stage(), LifecycleStage::Registered);
    }

    #[test]
    fn invalid_transition() {
        let lc = PluginLifecycle::new("test");
        // Directly trying to go from Discovered to Started is invalid
        assert!(lc.transition_to(LifecycleStage::Started).is_err());
        assert_eq!(lc.current_stage(), LifecycleStage::Discovered);
    }

    #[test]
    fn full_boot_sequence() {
        let lc = PluginLifecycle::new("test");
        assert!(lc.boot().is_ok());
        assert_eq!(lc.current_stage(), LifecycleStage::Started);
    }

    #[test]
    fn full_shutdown_sequence() {
        let lc = PluginLifecycle::new("test");
        lc.boot().unwrap();
        assert!(lc.shutdown().is_ok());
        assert_eq!(lc.current_stage(), LifecycleStage::Unloaded);
    }

    #[test]
    fn cannot_advance_from_unloaded() {
        let lc = PluginLifecycle::new("test");
        lc.boot().unwrap();
        lc.shutdown().unwrap();
        assert!(lc.advance().is_err());
    }

    #[test]
    fn hooks_fire_on_transition() {
        let lc = PluginLifecycle::new("hook-test");
        use std::sync::atomic::{AtomicBool, Ordering};
        let fired = std::sync::Arc::new(AtomicBool::new(false));
        let fired_clone = fired.clone();

        lc.on_registered(move |_id, _stage| {
            fired_clone.store(true, Ordering::SeqCst);
        });

        lc.advance().unwrap();
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn finalized_prevents_transitions() {
        let lc = PluginLifecycle::new("finalized");
        lc.finalize();
        assert!(lc.advance().is_err());
    }

    #[test]
    fn stage_properties() {
        assert!(LifecycleStage::Started.is_active());
        assert!(!LifecycleStage::Discovered.is_active());
        assert!(!LifecycleStage::Unloaded.is_active());
        assert!(LifecycleStage::Unloaded.is_terminal());
        assert!(!LifecycleStage::Started.is_terminal());
    }

    #[test]
    fn lifecycle_manager() {
        let manager = LifecycleManager::new();

        let lc1 = PluginLifecycle::new("plugin-a");
        let lc2 = PluginLifecycle::new("plugin-b");

        manager.register(lc1);
        manager.register(lc2);

        assert_eq!(manager.count(), 2);
        assert!(manager.all_ids().contains(&"plugin-a".to_string()));

        // Boot all
        let errors = manager.boot_all();
        assert!(errors.is_empty());

        assert_eq!(
            manager.get("plugin-a").unwrap().current_stage(),
            LifecycleStage::Started
        );
    }

    #[test]
    fn lifecycle_manager_shutdown() {
        let manager = LifecycleManager::new();

        let lc = PluginLifecycle::new("bootable");
        manager.register(lc);
        manager.boot_all();

        let errors = manager.shutdown_all();
        assert!(errors.is_empty());

        assert_eq!(
            manager.get("bootable").unwrap().current_stage(),
            LifecycleStage::Unloaded
        );
    }

    #[test]
    fn at_stage_filter() {
        let manager = LifecycleManager::new();
        manager.register(PluginLifecycle::new("a"));
        manager.register(PluginLifecycle::new("b"));
        manager.register(PluginLifecycle::at_stage("c", LifecycleStage::Started));

        assert_eq!(manager.at_stage(LifecycleStage::Discovered).len(), 2);
        assert_eq!(manager.at_stage(LifecycleStage::Started).len(), 1);
    }
}
