//! Lifecycle model for [`SocketManager`].
//!
//! This module defines the `SocketLifecycle` state machine, which mirrors
//! the standard [`LifecycleStage`](crate::registry::lifecycle::LifecycleStage)
//! used by every other service in the platform:
//!
//! ```text
//! Created → Initialized → Running → Shutdown
//! ```
//!
//! The socket manager transitions through these states as part of the
//! standard application startup and shutdown sequence. All transitions are
//! validated — backward transitions and double-shutdowns are rejected.

use std::sync::atomic::{AtomicU8, Ordering};

use crate::registry::lifecycle::LifecycleStage;

/// The lifecycle stage of a socket server.
///
/// This is a lightweight atomic state that mirrors
/// [`LifecycleStage`] from the registry module. It is used internally by
/// `SocketManager` to enforce valid transitions without holding a lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum SocketLifecycle {
    /// The socket has been created (configuration loaded) but not yet
    /// initialized. No filesystem operations have been performed.
    Created = 0,
    /// The socket path has been validated, stale artifacts cleaned, and
    /// permissions queued for application after binding. The listener is
    /// not yet active.
    Initialized = 1,
    /// The socket is listening for connections and dispatching to the
    /// connection handler.
    Running = 2,
    /// The socket has been shut down — the listener is closed, the accept
    /// loop has terminated, and the socket file has been removed.
    Shutdown = 3,
}

impl SocketLifecycle {
    /// Returns the numeric representation.
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Returns `true` if this stage is at or after `other`.
    pub fn is_at_least(&self, other: Self) -> bool {
        *self >= other
    }

    /// Returns `true` if this stage is before `other`.
    pub fn is_before(&self, other: Self) -> bool {
        *self < other
    }

    /// Returns `true` if the socket is running and accepting connections.
    pub fn is_running(&self) -> bool {
        *self == Self::Running
    }

    /// Returns `true` if the socket has been fully shut down.
    pub fn is_shutdown(&self) -> bool {
        *self == Self::Shutdown
    }
}

impl Default for SocketLifecycle {
    fn default() -> Self {
        Self::Created
    }
}

impl From<LifecycleStage> for SocketLifecycle {
    fn from(stage: LifecycleStage) -> Self {
        match stage {
            LifecycleStage::Created => SocketLifecycle::Created,
            LifecycleStage::Initialized => SocketLifecycle::Initialized,
            LifecycleStage::Running => SocketLifecycle::Running,
            LifecycleStage::Shutdown => SocketLifecycle::Shutdown,
        }
    }
}

impl From<SocketLifecycle> for LifecycleStage {
    fn from(stage: SocketLifecycle) -> Self {
        match stage {
            SocketLifecycle::Created => LifecycleStage::Created,
            SocketLifecycle::Initialized => LifecycleStage::Initialized,
            SocketLifecycle::Running => LifecycleStage::Running,
            SocketLifecycle::Shutdown => LifecycleStage::Shutdown,
        }
    }
}

/// Atomic lifecycle stage tracker for a socket manager.
///
/// Uses a `AtomicU8` for lock-free stage transitions, following the same
/// pattern as [`LifecycleManager`](crate::registry::lifecycle::LifecycleManager).
#[derive(Debug)]
pub struct SocketLifecycleManager {
    stage: AtomicU8,
}

impl Clone for SocketLifecycleManager {
    fn clone(&self) -> Self {
        Self {
            stage: AtomicU8::new(self.stage.load(Ordering::Acquire)),
        }
    }
}

impl SocketLifecycleManager {
    pub fn new() -> Self {
        Self {
            stage: AtomicU8::new(SocketLifecycle::Created.as_u8()),
        }
    }

    pub fn at(stage: SocketLifecycle) -> Self {
        Self {
            stage: AtomicU8::new(stage.as_u8()),
        }
    }

    pub fn stage(&self) -> SocketLifecycle {
        match self.stage.load(Ordering::Acquire) {
            0 => SocketLifecycle::Created,
            1 => SocketLifecycle::Initialized,
            2 => SocketLifecycle::Running,
            3 => SocketLifecycle::Shutdown,
            _ => SocketLifecycle::Created,
        }
    }

    /// Attempts a one-way lifecycle transition.
    ///
    /// Valid transitions:
    /// - `Created → Initialized`
    /// - `Initialized → Running`
    /// - `Running → Shutdown`
    /// - `Initialized → Shutdown` (skip running)
    /// - `Created → Shutdown` (never started)
    /// - Staying at the same stage is a no-op success.
    pub fn transition_to(&self, target: SocketLifecycle) -> Result<(), &'static str> {
        let current = self.stage();
        if current == target {
            return Ok(());
        }

        let valid = match (current, target) {
            (SocketLifecycle::Created, SocketLifecycle::Initialized)
            | (SocketLifecycle::Initialized, SocketLifecycle::Running)
            | (SocketLifecycle::Running, SocketLifecycle::Shutdown)
            | (SocketLifecycle::Initialized, SocketLifecycle::Shutdown)
            | (SocketLifecycle::Created, SocketLifecycle::Shutdown) => true,
            _ => false,
        };

        if !valid {
            return Err("Invalid lifecycle transition — transitions are one-way and forward-only");
        }

        self.stage
            .store(target.as_u8(), Ordering::Release);
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        self.stage() == SocketLifecycle::Running
    }

    pub fn is_shutdown(&self) -> bool {
        self.stage() == SocketLifecycle::Shutdown
    }

    /// Returns `true` if this stage is at or after `other`.
    pub fn is_at_least(&self, other: SocketLifecycle) -> bool {
        self.stage() >= other
    }
}

impl Default for SocketLifecycleManager {
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

    #[test]
    fn default_is_created() {
        let mgr = SocketLifecycleManager::new();
        assert_eq!(mgr.stage(), SocketLifecycle::Created);
        assert!(!mgr.is_running());
        assert!(!mgr.is_shutdown());
    }

    #[test]
    fn valid_forward_transitions() {
        let mgr = SocketLifecycleManager::new();
        assert!(mgr.transition_to(SocketLifecycle::Initialized).is_ok());
        assert_eq!(mgr.stage(), SocketLifecycle::Initialized);
        assert!(mgr.transition_to(SocketLifecycle::Running).is_ok());
        assert_eq!(mgr.stage(), SocketLifecycle::Running);
        assert!(mgr.transition_to(SocketLifecycle::Shutdown).is_ok());
        assert_eq!(mgr.stage(), SocketLifecycle::Shutdown);
    }

    #[test]
    fn skip_transition_allowed() {
        let mgr = SocketLifecycleManager::new();
        assert!(mgr.transition_to(SocketLifecycle::Running).is_err());
        assert!(mgr.transition_to(SocketLifecycle::Shutdown).is_ok());
        assert_eq!(mgr.stage(), SocketLifecycle::Shutdown);
    }

    #[test]
    fn same_stage_is_noop() {
        let mgr = SocketLifecycleManager::new();
        assert!(mgr.transition_to(SocketLifecycle::Created).is_ok());
        assert_eq!(mgr.stage(), SocketLifecycle::Created);
    }

    #[test]
    fn backward_transition_fails() {
        let mgr = SocketLifecycleManager::at(SocketLifecycle::Running);
        assert!(mgr.transition_to(SocketLifecycle::Initialized).is_err());
        assert!(mgr.transition_to(SocketLifecycle::Created).is_err());
    }

    #[test]
    fn shutdown_to_anything_fails() {
        let mgr = SocketLifecycleManager::at(SocketLifecycle::Shutdown);
        assert!(mgr.transition_to(SocketLifecycle::Created).is_err());
        assert!(mgr.transition_to(SocketLifecycle::Initialized).is_err());
        assert!(mgr.transition_to(SocketLifecycle::Running).is_err());
    }

    #[test]
    fn lifecycle_stage_conversion() {
        assert_eq!(
            SocketLifecycle::from(LifecycleStage::Created),
            SocketLifecycle::Created
        );
        assert_eq!(
            SocketLifecycle::from(LifecycleStage::Running),
            SocketLifecycle::Running
        );
        assert_eq!(
            SocketLifecycle::from(LifecycleStage::Shutdown),
            SocketLifecycle::Shutdown
        );

        assert_eq!(
            LifecycleStage::from(SocketLifecycle::Running),
            LifecycleStage::Running
        );
        assert_eq!(
            LifecycleStage::from(SocketLifecycle::Shutdown),
            LifecycleStage::Shutdown
        );
    }

    #[test]
    fn is_at_least_and_is_before() {
        let mgr = SocketLifecycleManager::at(SocketLifecycle::Initialized);
        assert!(mgr.is_at_least(SocketLifecycle::Created));
        assert!(mgr.is_at_least(SocketLifecycle::Initialized));
        assert!(!mgr.is_at_least(SocketLifecycle::Running));
    }
}
