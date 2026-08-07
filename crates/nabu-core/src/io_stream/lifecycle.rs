//! Lifecycle integration for the stdio transport.
//!
//! This module integrates the stdio transport with the standard
//! [`Lifecycle`] trait and [`LifecycleManager`] from the registry module,
//! ensuring consistent lifecycle management across all Nabu services.
//!
//! ## Lifecycle Stages
//!
//! ```text
//! Created → Initialized → Running → Shutdown
//! ```
//!
//! The stdio transport's lifecycle is managed through atomic state transitions:
//!
//! - **Created** — The transport struct exists but I/O has not started.
//! - **Initialized** — Configuration is validated; shutdown signal is fresh.
//! - **Running** — The read loop is actively processing requests from stdin.
//! - **Shutdown** — The read loop has terminated; stdout has been flushed.
//!
//! All transitions are one-way and forward-only, matching the platform
//! convention established by [`LifecycleStage`].

use crate::registry::lifecycle::{Lifecycle, LifecycleManager, LifecycleStage};

/// Lifecycle manager for [`StdioTransport`](super::StdioTransport).
///
/// Wraps the standard [`LifecycleManager`] to provide lifecycle tracking
/// for the stdio transport. This ensures the transport participates in
/// the same `Created → Initialized → Running → Shutdown` state machine
/// as every other service in the Nabu Capability Platform.
///
/// The lifecycle is tracked atomically (lock-free), so `is_running()` and
/// `is_shutdown()` can be called concurrently from multiple threads without
/// blocking the read loop.
#[derive(Debug)]
pub struct TransportLifecycle {
    inner: LifecycleManager,
}

impl TransportLifecycle {
    /// Create a new lifecycle manager, starting at `Created` stage.
    pub fn new() -> Self {
        Self {
            inner: LifecycleManager::new(),
        }
    }

    /// Returns the current lifecycle stage.
    pub fn stage(&self) -> LifecycleStage {
        self.inner.stage()
    }

    /// Transition to the `Initialized` stage.
    ///
    /// Called after configuration is validated and before the read loop
    /// starts. Returns an error if the transport is already running or
    /// shut down.
    pub fn initialize(&self) -> Result<(), LifecycleError> {
        self.inner
            .transition_to(LifecycleStage::Initialized)
            .map_err(|e| LifecycleError::from_registry(e))
    }

    /// Transition to the `Running` stage.
    ///
    /// Called once the read loop task has been spawned. Returns an error
    /// if initialization hasn't happened or the transport is already
    /// shut down.
    pub fn start(&self) -> Result<(), LifecycleError> {
        self.inner
            .transition_to(LifecycleStage::Running)
            .map_err(|e| LifecycleError::from_registry(e))
    }

    /// Transition to the `Shutdown` stage.
    ///
    /// Called after the read loop terminates and stdout is flushed.
    /// Returns an error if the transport hasn't reached `Running` yet.
    pub fn shutdown(&self) -> Result<(), LifecycleError> {
        self.inner
            .transition_to(LifecycleStage::Shutdown)
            .map_err(|e| LifecycleError::from_registry(e))
    }

    /// Returns `true` if the transport is currently running.
    pub fn is_running(&self) -> bool {
        self.inner.is_running()
    }

    /// Returns `true` if the transport has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.inner.is_shutdown()
    }

    /// Returns `true` if the transport is at or past the given stage.
    pub fn is_at_least(&self, stage: LifecycleStage) -> bool {
        self.inner.is_at_least(stage)
    }
}

impl Default for TransportLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper error type for lifecycle transition failures.
///
/// This re-exports the registry [`LifecycleError`] so transport callers
/// don't need to import the registry module directly.
#[derive(Debug)]
pub struct LifecycleError {
    pub inner: crate::registry::lifecycle::LifecycleError,
}

impl LifecycleError {
    pub fn from_registry(e: crate::registry::lifecycle::LifecycleError) -> Self {
        LifecycleError { inner: e }
    }
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl std::error::Error for LifecycleError {}
