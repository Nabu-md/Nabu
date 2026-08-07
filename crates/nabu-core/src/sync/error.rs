//! Synchronization domain errors.
//!
//! Structured error types for the synchronization domain models.
//! Every method that can fail returns [`SyncError`] — no panics, no
//! stringly-typed failures. The type is fully serializable so it can be
//! transported across IPC boundaries (EventBus → Tauri bridge → frontend).
//!
//! [`SyncError`] borrows its variant payloads from the strongly-typed domain
//! models ([`SyncStatus`], [`ConflictResolution`]) rather than re-encoding
//! them as strings. This keeps the error surface type-safe and
//! deserialization-friendly.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::sync::SyncStatus;

/// Result alias used throughout the synchronization domain.
pub type SyncResult<T> = Result<T, SyncError>;

/// Structured errors for synchronization model validation and
/// state-transition operations.
///
/// All variants carry structured data — not raw string messages — so that
/// IPC consumers (the frontend, logging, metrics) can inspect error fields
/// without string parsing.
///
/// # Serialization
///
/// `SyncError` derives [`Serialize`] and [`Deserialize`] via Serde. Because the
/// referenced types ([`SyncStatus`], [`ConflictResolution`]) are themselves
/// serializable, every variant round-trips through JSON correctly. This allows
/// errors to be transported over the EventBus / Tauri IPC boundary.
///
/// # Forward compatibility
///
/// New variants may be added in future phases. Consumers should match
/// exhaustively only within the crate; external consumers should include a
/// `_ =>` arm.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncError {
    /// A [`crate::sync::SyncFolder`] failed validation.
    #[error("Invalid sync folder '{id}': {reason}")]
    InvalidFolder {
        /// The folder identifier (as a string) that failed validation.
        id: String,
        /// Human-readable description of the validation failure.
        reason: String,
    },

    /// A [`crate::sync::SyncStatus`] transition is not permitted.
    #[error("Invalid status transition: {current} -> {target}")]
    InvalidStatusTransition {
        /// The current synchronization status.
        current: SyncStatus,
        /// The target synchronization status that was rejected.
        target: SyncStatus,
    },

    /// A [`crate::sync::SyncProgress`] value is internally inconsistent.
    #[error("Invalid sync progress: {reason}")]
    InvalidProgress {
        /// Human-readable description of the inconsistency.
        reason: String,
    },

    /// A [`crate::sync::ConflictResolution`] strategy is not applicable or
    /// was used in an invalid context.
    #[error("Invalid conflict resolution: {reason}")]
    InvalidConflictResolution {
        /// Human-readable description of the failure.
        reason: String,
    },
}

impl SyncError {
    /// Creates an [`InvalidFolder`](SyncError::InvalidFolder) error for the
    /// given folder id and reason.
    pub fn invalid_folder(id: impl Into<String>, reason: impl Into<String>) -> Self {
        SyncError::InvalidFolder {
            id: id.into(),
            reason: reason.into(),
        }
    }

    /// Creates an [`InvalidProgress`](SyncError::InvalidProgress) error with
    /// the given reason.
    pub fn invalid_progress(reason: impl Into<String>) -> Self {
        SyncError::InvalidProgress {
            reason: reason.into(),
        }
    }

    /// Creates an [`InvalidConflictResolution`](SyncError::InvalidConflictResolution)
    /// error with the given reason.
    pub fn invalid_conflict_resolution(reason: impl Into<String>) -> Self {
        SyncError::InvalidConflictResolution {
            reason: reason.into(),
        }
    }

    /// Returns the variant name as a `&'static str` — useful for metrics and
    /// structured logging without serializing the full error payload.
    pub fn variant_name(&self) -> &'static str {
        match self {
            SyncError::InvalidFolder { .. } => "invalid_folder",
            SyncError::InvalidStatusTransition { .. } => "invalid_status_transition",
            SyncError::InvalidProgress { .. } => "invalid_progress",
            SyncError::InvalidConflictResolution { .. } => "invalid_conflict_resolution",
        }
    }
}

#[cfg(test)]
mod sync_model {
    use super::*;

    #[test]
    fn sync_model_error_invalid_folder_round_trip() {
        let err = SyncError::invalid_folder("folder-1", "path is empty");
        let json = serde_json::to_string(&err).unwrap();
        let back: SyncError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
        assert_eq!(err.variant_name(), "invalid_folder");
    }

    #[test]
    fn sync_model_error_invalid_progress_round_trip() {
        let err = SyncError::invalid_progress("percentage out of range");
        let json = serde_json::to_string(&err).unwrap();
        let back: SyncError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
        assert_eq!(err.variant_name(), "invalid_progress");
    }

    #[test]
    fn sync_model_error_status_transition_round_trip() {
        let err = SyncError::InvalidStatusTransition {
            current: SyncStatus::UpToDate,
            target: SyncStatus::Syncing,
        };
        // This transition is actually valid — we're just testing serialization.
        let json = serde_json::to_string(&err).unwrap();
        let back: SyncError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
        assert!(json.contains("up_to_date"));
        assert!(json.contains("syncing"));
    }

    #[test]
    fn sync_model_error_invalid_conflict_resolution_round_trip() {
        let err = SyncError::invalid_conflict_resolution("strategy not supported");
        let json = serde_json::to_string(&err).unwrap();
        let back: SyncError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
        assert_eq!(err.variant_name(), "invalid_conflict_resolution");
    }

    #[test]
    fn sync_model_error_display_contains_fields() {
        let err = SyncError::InvalidFolder {
            id: "abc".into(),
            reason: "missing provider".into(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("abc"));
        assert!(msg.contains("missing provider"));
    }

    #[test]
    fn sync_model_error_implements_std_error() {
        let err = SyncError::invalid_progress("test");
        let _: &dyn std::error::Error = &err;
    }
}
