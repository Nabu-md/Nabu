//! Lifecycle Management — service lifecycle stages and transitions.
//!
//! This module defines the lifecycle stages that services and the application
//! context can be in, along with transition validation to prevent invalid
//! state changes.
//!
//! # Lifecycle Stages
//!
//! ```text
//! Created → Initialized → Running → Shutdown
//!                              ↓
//!                          (stopped)
//! ```
//!
//! Transitions are one-way — a service cannot move backward through stages.
//! This ensures predictable initialization and teardown ordering.

use std::sync::atomic::{AtomicU8, Ordering};

/// The lifecycle stage of a service or the application context.
///
/// Stages are ordered and one-way: once a stage is reached, the only valid
/// transition is to a later stage. This prevents re-initialization and
/// double-shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LifecycleStage {
    /// The service has been created but not initialized.
    Created = 0,
    /// The service has been initialized (dependencies resolved, resources
    /// allocated) but is not yet processing requests.
    Initialized = 1,
    /// The service is fully operational and processing requests.
    Running = 2,
    /// The service has been shut down and resources released.
    Shutdown = 3,
}

impl LifecycleStage {
    /// Returns the numeric representation of this stage.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Returns `true` if this stage is at or after the given stage.
    pub fn is_at_least(&self, other: LifecycleStage) -> bool {
        *self >= other
    }

    /// Returns `true` if this stage is before the given stage.
    pub fn is_before(&self, other: LifecycleStage) -> bool {
        *self < other
    }
}

/// Error returned when an invalid lifecycle transition is attempted.
#[derive(Debug, Clone)]
pub struct LifecycleError {
    /// The current stage of the service.
    pub current: LifecycleStage,
    /// The attempted target stage.
    pub target: LifecycleStage,
    /// A human-readable description of the error.
    pub message: String,
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Invalid lifecycle transition: {:?} → {:?}: {}",
            self.current, self.target, self.message
        )
    }
}

impl std::error::Error for LifecycleError {}

/// Manages lifecycle stage transitions for a service or application context.
///
/// Uses an atomic counter internally for lock-free stage transitions.
///
/// # Example
///
/// ```ignore
/// use nabu_core::registry::lifecycle::{LifecycleManager, LifecycleStage};
///
/// let mut mgr = LifecycleManager::new();
/// assert_eq!(mgr.stage(), LifecycleStage::Created);
///
/// mgr.transition_to(LifecycleStage::Initialized);
/// assert_eq!(mgr.stage(), LifecycleStage::Initialized);
/// ```
#[derive(Debug)]
pub struct LifecycleManager {
    stage: AtomicU8,
}

impl LifecycleManager {
    /// Creates a new lifecycle manager at the `Created` stage.
    pub fn new() -> Self {
        Self {
            stage: AtomicU8::new(LifecycleStage::Created as u8),
        }
    }

    /// Creates a new lifecycle manager at the given initial stage.
    pub fn at(stage: LifecycleStage) -> Self {
        Self {
            stage: AtomicU8::new(stage as u8),
        }
    }

    /// Returns the current lifecycle stage.
    pub fn stage(&self) -> LifecycleStage {
        match self.stage.load(Ordering::Acquire) {
            0 => LifecycleStage::Created,
            1 => LifecycleStage::Initialized,
            2 => LifecycleStage::Running,
            3 => LifecycleStage::Shutdown,
            _ => unreachable!("Invalid lifecycle stage value"),
        }
    }

    /// Attempts to transition to the given target stage.
    ///
    /// Returns `Ok(())` if the transition is valid, or a [`LifecycleError`]
    /// describing why the transition is not allowed.
    ///
    /// Valid transitions:
    /// - `Created` → `Initialized`
    /// - `Initialized` → `Running`
    /// - `Running` → `Shutdown`
    /// - `Initialized` → `Shutdown` (skip running)
    /// - `Created` → `Shutdown` (never started)
    ///
    /// Invalid transitions (will return an error):
    /// - Any backward transition (e.g. `Running` → `Initialized`)
    /// - Double transitions (e.g. `Shutdown` → anything)
    pub fn transition_to(&self, target: LifecycleStage) -> Result<(), LifecycleError> {
        let current = self.stage();

        // Always allow staying at the same stage
        if current == target {
            return Ok(());
        }

        // Validate transition
        let valid = match (current, target) {
            // Forward transitions
            (LifecycleStage::Created, LifecycleStage::Initialized)
            | (LifecycleStage::Initialized, LifecycleStage::Running)
            | (LifecycleStage::Running, LifecycleStage::Shutdown)
            // Skip transitions (allowed for cleanup)
            | (LifecycleStage::Initialized, LifecycleStage::Shutdown)
            | (LifecycleStage::Created, LifecycleStage::Shutdown) => true,

            // All other transitions are invalid
            _ => false,
        };

        if !valid {
            return Err(LifecycleError {
                current,
                target,
                message: format!(
                    "Cannot transition from {:?} to {:?}. Transitions are one-way and must go forward.",
                    current, target
                ),
            });
        }

        self.stage.store(target as u8, Ordering::Release);
        Ok(())
    }

    /// Returns `true` if the service is at or past the given stage.
    pub fn is_at_least(&self, stage: LifecycleStage) -> bool {
        self.stage() >= stage
    }

    /// Returns `true` if the service has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.stage() == LifecycleStage::Shutdown
    }

    /// Returns `true` if the service is running.
    pub fn is_running(&self) -> bool {
        self.stage() == LifecycleStage::Running
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for services that have a lifecycle.
///
/// Implement this trait for services that need to perform initialization,
/// startup, or shutdown logic.
pub trait Lifecycle: Send + Sync {
    /// Returns the name of this service for logging/debugging.
    fn name(&self) -> &'static str;

    /// Initializes the service.
    ///
    /// Called after all dependencies have been resolved and registered.
    /// Services should allocate resources and validate configuration here.
    fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Starts the service.
    ///
    /// Called after initialization is complete. Services should begin
    /// processing requests. The default implementation is a no-op.
    fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    /// Shuts down the service.
    ///
    /// Called when the application is shutting down. Services should release
    /// resources and stop background tasks. The default implementation is a
    /// no-op.
    fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_stage_is_created() {
        let mgr = LifecycleManager::new();
        assert_eq!(mgr.stage(), LifecycleStage::Created);
    }

    #[test]
    fn valid_transitions_succeed() {
        let mgr = LifecycleManager::new();

        assert!(mgr.transition_to(LifecycleStage::Initialized).is_ok());
        assert_eq!(mgr.stage(), LifecycleStage::Initialized);

        assert!(mgr.transition_to(LifecycleStage::Running).is_ok());
        assert_eq!(mgr.stage(), LifecycleStage::Running);

        assert!(mgr.transition_to(LifecycleStage::Shutdown).is_ok());
        assert_eq!(mgr.stage(), LifecycleStage::Shutdown);
    }

    #[test]
    fn skip_transitions_succeed() {
        // Created → Shutdown (skip initialize and running)
        let mgr = LifecycleManager::new();
        assert!(mgr.transition_to(LifecycleStage::Shutdown).is_ok());
        assert_eq!(mgr.stage(), LifecycleStage::Shutdown);

        // Initialized → Shutdown (skip running)
        let mgr = LifecycleManager::at(LifecycleStage::Initialized);
        assert!(mgr.transition_to(LifecycleStage::Shutdown).is_ok());
    }

    #[test]
    fn backward_transition_fails() {
        let mgr = LifecycleManager::at(LifecycleStage::Running);

        let err = mgr.transition_to(LifecycleStage::Initialized).unwrap_err();
        assert_eq!(err.current, LifecycleStage::Running);
        assert_eq!(err.target, LifecycleStage::Initialized);
    }

    #[test]
    fn shutdown_to_anything_fails() {
        let mgr = LifecycleManager::at(LifecycleStage::Shutdown);

        assert!(mgr.transition_to(LifecycleStage::Created).is_err());
        assert!(mgr.transition_to(LifecycleStage::Initialized).is_err());
        assert!(mgr.transition_to(LifecycleStage::Running).is_err());
    }

    #[test]
    fn staying_at_same_stage_is_noop() {
        let mgr = LifecycleManager::at(LifecycleStage::Initialized);
        assert!(mgr.transition_to(LifecycleStage::Initialized).is_ok());
        assert_eq!(mgr.stage(), LifecycleStage::Initialized);
    }

    #[test]
    fn is_running_and_is_shutdown() {
        let mgr = LifecycleManager::at(LifecycleStage::Running);
        assert!(mgr.is_running());
        assert!(!mgr.is_shutdown());

        mgr.transition_to(LifecycleStage::Shutdown).unwrap();
        assert!(mgr.is_shutdown());
        assert!(!mgr.is_running());
    }

    #[test]
    fn is_at_least() {
        let mgr = LifecycleManager::at(LifecycleStage::Initialized);
        assert!(mgr.is_at_least(LifecycleStage::Created));
        assert!(mgr.is_at_least(LifecycleStage::Initialized));
        assert!(!mgr.is_at_least(LifecycleStage::Running));
    }

    #[test]
    fn lifecycle_trait_default_impls() {
        struct TestService;

        impl Lifecycle for TestService {
            fn name(&self) -> &'static str {
                "test_service"
            }
        }

        let svc = TestService;
        assert_eq!(svc.name(), "test_service");
        assert!(svc.initialize().is_ok());
        assert!(svc.start().is_ok());
        assert!(svc.shutdown().is_ok());
    }

    #[test]
    fn lifecycle_error_display() {
        let err = LifecycleError {
            current: LifecycleStage::Running,
            target: LifecycleStage::Initialized,
            message: "Cannot go backward".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("Running"));
        assert!(msg.contains("Initialized"));
        assert!(msg.contains("Cannot go backward"));
    }
}
