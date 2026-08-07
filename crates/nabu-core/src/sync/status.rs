//! Synchronization status — the strongly-typed state of a synchronized folder.
//!
//! [`SyncStatus`] replaces raw string status values with a closed enum so
//! that every synchronization provider reports status through the same
//! vocabulary. The enum is `#[non_exhaustive]` so that future phases can
//! add variants (e.g. `Throttled`, `RateLimited`) without breaking
//! external consumers.
//!
//! ## Ownership
//!
//! `SyncStatus` is a value type — it is `Copy`, `Send`, and `Sync`.
//! It is owned by [`crate::sync::SyncFolder`] and copied into
//! [`crate::sync::SyncProgress`] reports. No service holds a reference to it.
//!
//! ## Lifecycle expectations
//!
//! ```text
//! NotConfigured ──▶ Idle ──▶ Syncing ──▶ UpToDate
//!    │              │         │            │
//!    │              │         │            ▼
//!    │              │         │         Conflict ◀── (on conflict)
//!    │              │         │            │
//!    │              │         │            ▼
//!    │              │         │         Error
//!    │              │         │            │
//!    │              │         └────────────┘  (any state → Error)
//!    │              │                          (on failure)
//!    │              └─────────────────────────▶ Paused (on demand)
//!    │                                          │
//!    │                                          ▼
//!    └───────────────────────────────────────▶ Offline (on disconnect)
//! ```
//!
//! Transitions are advisory — the platform does **not** enforce a state
//! machine. Providers are free to move between states as their backend
//! dictates. The [`SyncStatus::can_transition_to`] helper provides a
//! recommendation but is not a hard constraint.

use serde::{Deserialize, Serialize};

/// The synchronization state of a [`crate::sync::SyncFolder`].
///
/// Every synchronization provider translates its internal state into one of
/// these variants before reporting via the EventBus or IPC.
///
/// # Serialization
///
/// Variants serialize to `snake_case` strings (e.g. `"up_to_date"`,
/// `"not_configured"`). The `#[non_exhaustive]` attribute ensures that
/// new variants can be added in future phases without breaking
/// deserialization for existing consumers.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    /// The folder exists but no synchronization provider has been configured
    /// for it yet. This is the default state for a newly created
    /// [`crate::sync::SyncFolder`].
    NotConfigured,

    /// The folder is configured and idle — no sync operation is in progress.
    Idle,

    /// A synchronization operation is currently in progress.
    /// Progress details are available via [`crate::sync::SyncProgress`].
    Syncing,

    /// The folder is fully synchronized with the remote — no pending
    /// changes on either side.
    UpToDate,

    /// Changes have been detected (locally or remotely) but have not yet been
    /// synchronized. The folder is queued for the next sync cycle.
    Pending,

    /// One or more file conflicts were detected during the last or current
    /// synchronization. The [`crate::sync::ConflictResolution`] strategy
    /// determines how such conflicts are resolved.
    Conflict,

    /// An error occurred during synchronization. The error message is
    /// available in [`crate::sync::SyncFolder::error`] or the active
    /// [`crate::sync::SyncProgress`].
    Error,

    /// The sync provider is offline — the remote endpoint is unreachable,
    /// the network is disconnected, or the device is in a low-power state.
    Offline,

    /// Synchronization has been paused by the user or the system (e.g.
    /// battery-saver mode, explicit pause).
    Paused,
}

impl std::fmt::Display for SyncStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl Default for SyncStatus {
    fn default() -> Self {
        SyncStatus::NotConfigured
    }
}

impl SyncStatus {
    /// Returns a human-readable label suitable for UI or logging.
    pub fn label(&self) -> &'static str {
        match self {
            SyncStatus::NotConfigured => "not configured",
            SyncStatus::Idle => "idle",
            SyncStatus::Syncing => "syncing",
            SyncStatus::UpToDate => "up to date",
            SyncStatus::Pending => "pending",
            SyncStatus::Conflict => "conflict",
            SyncStatus::Error => "error",
            SyncStatus::Offline => "offline",
            SyncStatus::Paused => "paused",
        }
    }

    /// Returns `true` if the status represents an error or failure condition.
    pub fn is_error(&self) -> bool {
        matches!(self, SyncStatus::Error | SyncStatus::Offline)
    }

    /// Returns `true` if the status represents an active (in-progress) state.
    pub fn is_active(&self) -> bool {
        matches!(self, SyncStatus::Syncing)
    }

    /// Returns `true` if the status represents a terminal / steady state
    /// (no ongoing operation).
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            SyncStatus::UpToDate
                | SyncStatus::Pending
                | SyncStatus::Conflict
                | SyncStatus::Idle
                | SyncStatus::NotConfigured
                | SyncStatus::Paused
        )
    }

    /// Returns `true` if the folder is in a state where automatic
    /// synchronization can proceed (i.e. not paused, offline, or in error).
    pub fn is_available(&self) -> bool {
        matches!(
            self,
            SyncStatus::Idle | SyncStatus::UpToDate | SyncStatus::Pending
        )
    }

    /// Returns `true` if a transition from `current` to `target` is
    /// recommended.
    ///
    /// This is an *advisory* check — providers may deviate if their backend
    /// dictates different state semantics. The platform does not enforce
    /// transitions.
    pub fn can_transition_to(current: SyncStatus, target: SyncStatus) -> bool {
        if current == target {
            return true;
        }
        match (current, target) {
            // Error and Offline can recover to any non-error state.
            (SyncStatus::Error, _) | (SyncStatus::Offline, _) => true,
            // Syncing can transition to any completion state.
            (SyncStatus::Syncing, SyncStatus::UpToDate)
            | (SyncStatus::Syncing, SyncStatus::Pending)
            | (SyncStatus::Syncing, SyncStatus::Conflict)
            | (SyncStatus::Syncing, SyncStatus::Error)
            | (SyncStatus::Syncing, SyncStatus::Offline) => true,
            // Idle can start syncing, go pending, or be paused.
            (SyncStatus::Idle, SyncStatus::Syncing)
            | (SyncStatus::Idle, SyncStatus::Pending)
            | (SyncStatus::Idle, SyncStatus::Paused)
            | (SyncStatus::Idle, SyncStatus::Offline) => true,
            // UpToDate can go to pending, syncing, error, offline, or paused.
            (SyncStatus::UpToDate, SyncStatus::Pending)
            | (SyncStatus::UpToDate, SyncStatus::Syncing)
            | (SyncStatus::UpToDate, SyncStatus::Error)
            | (SyncStatus::UpToDate, SyncStatus::Offline)
            | (SyncStatus::UpToDate, SyncStatus::Paused) => true,
            // Pending can sync, go idle, error, offline, or pause.
            (SyncStatus::Pending, SyncStatus::Syncing)
            | (SyncStatus::Pending, SyncStatus::Idle)
            | (SyncStatus::Pending, SyncStatus::Error)
            | (SyncStatus::Pending, SyncStatus::Offline)
            | (SyncStatus::Pending, SyncStatus::Paused) => true,
            // Conflict can be resolved (→ Idle/Syncing) or go error/offline.
            (SyncStatus::Conflict, SyncStatus::Syncing)
            | (SyncStatus::Conflict, SyncStatus::Idle)
            | (SyncStatus::Conflict, SyncStatus::Error)
            | (SyncStatus::Conflict, SyncStatus::Offline)
            | (SyncStatus::Conflict, SyncStatus::Paused) => true,
            // Paused can resume or go offline.
            (SyncStatus::Paused, SyncStatus::Idle)
            | (SyncStatus::Paused, SyncStatus::Syncing)
            | (SyncStatus::Paused, SyncStatus::Offline) => true,
            // NotConfigured can be configured (→ Idle).
            (SyncStatus::NotConfigured, SyncStatus::Idle) => true,
            // All other transitions are not recommended.
            _ => false,
        }
    }
}

#[cfg(test)]
mod sync_model {
    use super::*;

    #[test]
    fn sync_model_status_default_is_not_configured() {
        assert_eq!(SyncStatus::default(), SyncStatus::NotConfigured);
    }

    #[test]
    fn sync_model_status_label() {
        assert_eq!(SyncStatus::NotConfigured.label(), "not configured");
        assert_eq!(SyncStatus::Idle.label(), "idle");
        assert_eq!(SyncStatus::Syncing.label(), "syncing");
        assert_eq!(SyncStatus::UpToDate.label(), "up to date");
        assert_eq!(SyncStatus::Pending.label(), "pending");
        assert_eq!(SyncStatus::Conflict.label(), "conflict");
        assert_eq!(SyncStatus::Error.label(), "error");
        assert_eq!(SyncStatus::Offline.label(), "offline");
        assert_eq!(SyncStatus::Paused.label(), "paused");
    }

    #[test]
    fn sync_model_status_is_error() {
        assert!(SyncStatus::Error.is_error());
        assert!(SyncStatus::Offline.is_error());
        assert!(!SyncStatus::Idle.is_error());
        assert!(!SyncStatus::UpToDate.is_error());
    }

    #[test]
    fn sync_model_status_is_active() {
        assert!(SyncStatus::Syncing.is_active());
        assert!(!SyncStatus::Idle.is_active());
        assert!(!SyncStatus::UpToDate.is_active());
    }

    #[test]
    fn sync_model_status_is_terminal() {
        assert!(SyncStatus::UpToDate.is_terminal());
        assert!(SyncStatus::Idle.is_terminal());
        assert!(SyncStatus::NotConfigured.is_terminal());
        assert!(SyncStatus::Paused.is_terminal());
        assert!(!SyncStatus::Syncing.is_terminal());
        assert!(!SyncStatus::Error.is_terminal());
    }

    #[test]
    fn sync_model_status_is_available() {
        assert!(SyncStatus::Idle.is_available());
        assert!(SyncStatus::UpToDate.is_available());
        assert!(SyncStatus::Pending.is_available());
        assert!(!SyncStatus::Syncing.is_available());
        assert!(!SyncStatus::Error.is_available());
        assert!(!SyncStatus::Offline.is_available());
        assert!(!SyncStatus::Paused.is_available());
    }

    #[test]
    fn sync_model_status_same_state_transition() {
        assert!(SyncStatus::can_transition_to(SyncStatus::Idle, SyncStatus::Idle));
        assert!(SyncStatus::can_transition_to(SyncStatus::Syncing, SyncStatus::Syncing));
    }

    #[test]
    fn sync_model_status_syncing_to_completion_states() {
        assert!(SyncStatus::can_transition_to(SyncStatus::Syncing, SyncStatus::UpToDate));
        assert!(SyncStatus::can_transition_to(SyncStatus::Syncing, SyncStatus::Pending));
        assert!(SyncStatus::can_transition_to(SyncStatus::Syncing, SyncStatus::Conflict));
        assert!(SyncStatus::can_transition_to(SyncStatus::Syncing, SyncStatus::Error));
        assert!(SyncStatus::can_transition_to(SyncStatus::Syncing, SyncStatus::Offline));
    }

    #[test]
    fn sync_model_status_error_to_any_is_allowed() {
        // Error should be able to recover to any state (except itself, which is trivially allowed).
        assert!(SyncStatus::can_transition_to(SyncStatus::Error, SyncStatus::Idle));
        assert!(SyncStatus::can_transition_to(SyncStatus::Error, SyncStatus::Syncing));
        assert!(SyncStatus::can_transition_to(SyncStatus::Error, SyncStatus::UpToDate));
        assert!(SyncStatus::can_transition_to(SyncStatus::Error, SyncStatus::Offline));
    }

    #[test]
    fn sync_model_status_offline_to_any_is_allowed() {
        assert!(SyncStatus::can_transition_to(SyncStatus::Offline, SyncStatus::Idle));
        assert!(SyncStatus::can_transition_to(SyncStatus::Offline, SyncStatus::UpToDate));
    }

    #[test]
    fn sync_model_status_invalid_transition() {
        // UpToDate -> NotConfigured is not a natural transition.
        assert!(!SyncStatus::can_transition_to(SyncStatus::UpToDate, SyncStatus::NotConfigured));
    }

    #[test]
    fn sync_model_status_serialization() {
        let cases = vec![
            (SyncStatus::NotConfigured, "\"not_configured\""),
            (SyncStatus::Idle, "\"idle\""),
            (SyncStatus::Syncing, "\"syncing\""),
            (SyncStatus::UpToDate, "\"up_to_date\""),
            (SyncStatus::Pending, "\"pending\""),
            (SyncStatus::Conflict, "\"conflict\""),
            (SyncStatus::Error, "\"error\""),
            (SyncStatus::Offline, "\"offline\""),
            (SyncStatus::Paused, "\"paused\""),
        ];

        for (status, expected) in cases {
            let json = serde_json::to_string(&status).unwrap();
            assert_eq!(json, expected, "serialization mismatch for {:?}", status);

            let back: SyncStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(back, status, "deserialization mismatch for {:?}", status);
        }
    }

    #[test]
    fn sync_model_status_deserializes_unknown_variant_fails() {
        // An unknown status string should fail deserialization (not silently
        // become a default). This ensures type safety — providers that emit
        // unknown status values are caught early.
        let result: Result<SyncStatus, _> = serde_json::from_str("\"bogus_status\"");
        assert!(result.is_err());
    }
}
