//! Plugin Lifecycle Hooks — lifecycle events and contracts for plugins.
//!
//! Defines the lifecycle stages a plugin goes through:
//!
//! ```text
//! Discovered → Validated → Installed → Enabled → Disabled → Upgraded → Unloaded
//! ```
//!
//! No plugin code is executed during these transitions. Only metadata
//! validation and dependency resolution occur.

/// The lifecycle stage of a plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PluginStage {
    /// Plugin manifest has been discovered on disk.
    Discovered = 0,
    /// Plugin manifest has been validated.
    Validated = 1,
    /// Plugin has been installed (dependencies resolved, capabilities registered).
    Installed = 2,
    /// Plugin is enabled and ready to provide capabilities.
    Enabled = 3,
    /// Plugin has been disabled (capabilities are no longer available).
    Disabled = 4,
    /// Plugin has been upgraded to a new version.
    Upgraded = 5,
    /// Plugin has been unloaded (fully removed from memory).
    Unloaded = 6,
}

impl PluginStage {
    pub fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Events that occur during plugin lifecycle transitions.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginLifecycleEvent {
    /// Plugin manifest was discovered at the given path.
    Discovered {
        plugin_id: String,
        path: String,
        version: String,
    },
    /// Plugin manifest was validated successfully.
    Validated { plugin_id: String },
    /// Plugin manifest validation failed.
    ValidationFailed {
        plugin_id: String,
        errors: Vec<String>,
    },
    /// Plugin was installed (dependencies resolved).
    Installed { plugin_id: String },
    /// Plugin installation failed.
    InstallFailed { plugin_id: String, reason: String },
    /// Plugin was enabled.
    Enabled { plugin_id: String },
    /// Plugin was disabled.
    Disabled { plugin_id: String },
    /// Plugin was upgraded from an old version to a new version.
    Upgraded {
        plugin_id: String,
        old_version: String,
        new_version: String,
    },
    /// Plugin was unloaded.
    Unloaded { plugin_id: String },
}

/// Trait for components that observe plugin lifecycle events.
///
/// Implement this trait to react to plugin lifecycle transitions
/// without executing plugin code. Examples:
/// - Logging plugin state changes
/// - Updating the capability registry
/// - Notifying the UI of plugin state
/// - Running pre/post hooks
pub trait PluginLifecycleObserver: Send + Sync {
    /// Called when a lifecycle event occurs.
    fn on_event(&self, event: &PluginLifecycleEvent);

    /// Return the name of this observer for debugging.
    fn name(&self) -> &'static str;
}

/// Manages the lifecycle of a plugin from discovery to unload.
///
/// Tracks current stage and validates transitions.
/// No plugin code is executed — only metadata operations.
#[derive(Debug, Clone)]
pub struct PluginLifecycle {
    /// Current stage of the plugin.
    stage: PluginStage,
    /// List of lifecycle events that have occurred.
    history: Vec<PluginLifecycleEvent>,
}

impl PluginLifecycle {
    /// Create a new lifecycle tracker starting at Discovered.
    pub fn new() -> Self {
        Self {
            stage: PluginStage::Discovered,
            history: Vec::new(),
        }
    }

    /// Create a new lifecycle tracker at a custom starting stage.
    pub fn at(stage: PluginStage) -> Self {
        Self {
            stage,
            history: Vec::new(),
        }
    }

    /// Current lifecycle stage.
    pub fn stage(&self) -> PluginStage {
        self.stage
    }

    /// History of lifecycle events.
    pub fn history(&self) -> &[PluginLifecycleEvent] {
        &self.history
    }

    /// Transition to a new stage.
    ///
    /// Returns an error if the transition is invalid (e.g., skipping stages
    /// or going backward in the lifecycle).
    pub fn transition_to(
        &mut self,
        target: PluginStage,
        event: PluginLifecycleEvent,
    ) -> Result<(), LifecycleTransitionError> {
        // Allow same stage (no-op)
        if self.stage == target {
            return Ok(());
        }

        // Allow forward transitions only
        if target < self.stage {
            return Err(LifecycleTransitionError::BackwardTransition {
                current: self.stage,
                target,
            });
        }

        // Record the event
        self.history.push(event);
        self.stage = target;
        Ok(())
    }

    /// Returns `true` if the plugin is in or past the given stage.
    pub fn is_at_least(&self, stage: PluginStage) -> bool {
        self.stage >= stage
    }

    /// Returns `true` if the plugin is enabled.
    pub fn is_enabled(&self) -> bool {
        self.stage == PluginStage::Enabled
    }

    /// Returns `true` if the plugin has been unloaded.
    pub fn is_unloaded(&self) -> bool {
        self.stage == PluginStage::Unloaded
    }
}

impl Default for PluginLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

/// Error returned when an invalid lifecycle transition is attempted.
#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleTransitionError {
    BackwardTransition {
        current: PluginStage,
        target: PluginStage,
    },
}

impl std::fmt::Display for LifecycleTransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackwardTransition { current, target } => {
                write!(
                    f,
                    "Cannot transition from {:?} to {:?}: lifecycle is one-way forward",
                    current, target
                )
            }
        }
    }
}

impl std::error::Error for LifecycleTransitionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_starts_at_discovered() {
        let lc = PluginLifecycle::new();
        assert_eq!(lc.stage(), PluginStage::Discovered);
    }

    #[test]
    fn forward_transition_succeeds() {
        let mut lc = PluginLifecycle::new();
        assert!(lc
            .transition_to(
                PluginStage::Validated,
                PluginLifecycleEvent::Validated {
                    plugin_id: "test".into()
                },
            )
            .is_ok());
        assert_eq!(lc.stage(), PluginStage::Validated);
    }

    #[test]
    fn backward_transition_fails() {
        let mut lc = PluginLifecycle::at(PluginStage::Enabled);
        let result = lc.transition_to(
            PluginStage::Installed,
            PluginLifecycleEvent::Disabled {
                plugin_id: "test".into(),
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn full_lifecycle() {
        let mut lc = PluginLifecycle::new();
        assert!(lc
            .transition_to(
                PluginStage::Validated,
                PluginLifecycleEvent::Validated {
                    plugin_id: "p".into()
                }
            )
            .is_ok());
        assert!(lc
            .transition_to(
                PluginStage::Installed,
                PluginLifecycleEvent::Installed {
                    plugin_id: "p".into()
                }
            )
            .is_ok());
        assert!(lc
            .transition_to(
                PluginStage::Enabled,
                PluginLifecycleEvent::Enabled {
                    plugin_id: "p".into()
                }
            )
            .is_ok());
        assert!(lc
            .transition_to(
                PluginStage::Disabled,
                PluginLifecycleEvent::Disabled {
                    plugin_id: "p".into()
                }
            )
            .is_ok());
        assert!(lc
            .transition_to(
                PluginStage::Unloaded,
                PluginLifecycleEvent::Unloaded {
                    plugin_id: "p".into()
                }
            )
            .is_ok());
        assert!(lc.is_unloaded());
    }

    #[test]
    fn history_records_events() {
        let mut lc = PluginLifecycle::new();
        lc.transition_to(
            PluginStage::Validated,
            PluginLifecycleEvent::Validated {
                plugin_id: "p".into(),
            },
        )
        .unwrap();
        lc.transition_to(
            PluginStage::Installed,
            PluginLifecycleEvent::Installed {
                plugin_id: "p".into(),
            },
        )
        .unwrap();
        assert_eq!(lc.history().len(), 2);
    }
}
