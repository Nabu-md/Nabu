//! Synchronization progress model.
//!
//! [`SyncProgress`] is a lightweight, provider-agnostic snapshot of an
//! in-flight synchronization operation. Providers report progress through
//! the EventBus (and ultimately to the Tauri IPC bridge) by constructing a
//! `SyncProgress` value from their backend's native progress data.
//!
//! # Design
//!
//! - `Option<u64>` for totals allows providers that don't know the total
//!   up-front (e.g. streaming, streaming-diff) to report partial progress.
//! - `percentage` is `Option<f64>` so that providers can report a
//!   pre-computed percentage when they have one, or let the platform
//!   derive it from the item or byte counts.
//! - `estimated_remaining_seconds` is `Option<u64>` — using seconds (rather
//!   than `Duration`) keeps the type trivially Serde-serializable and
//!   wasm-friendly.
//!
//! # Thread safety
//!
//! `SyncProgress` is a value type (`Clone`, `Send`, `Sync`). It contains no
//! interior mutability and no shared state — it is a plain data record.

use serde::{Deserialize, Serialize};

use crate::sync::error::{SyncError, SyncResult};
use crate::sync::status::SyncStatus;

/// A progress snapshot for an in-flight synchronization operation.
///
/// # Fields
///
/// | Field                        | Meaning                                      |
/// |------------------------------|----------------------------------------------|
/// | `operation`                  | Human-readable description of the current step |
/// | `completed_items`            | Number of files/items processed so far       |
/// | `total_items`                | Total files/items to process (if known)      |
/// | `bytes_transferred`          | Bytes transferred so far                     |
/// | `total_bytes`                | Total bytes to transfer (if known)           |
/// | `percentage`                 | Overall completion (0.0–100.0), if known  |
/// | `estimated_remaining_seconds`| ETA in seconds, if known                    |
/// | `status`                     | Current [`SyncStatus`]                       |
///
/// # Future compatibility
///
/// All fields use `#[serde(default)]` so that future phases can add new
/// fields (e.g. `transfer_rate_kbps`, `files_skipped`, `conflicts_found`)
/// without breaking deserialization of existing serialized data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncProgress {
    /// Human-readable description of the current operation
    /// (e.g. "uploading note.md", "scanning for changes").
    pub operation: String,

    /// Number of items (files) that have been processed so far.
    pub completed_items: u64,

    /// Total number of items to process, if known.
    /// `None` when the provider cannot determine the total up-front.
    pub total_items: Option<u64>,

    /// Total bytes transferred so far.
    pub bytes_transferred: u64,

    /// Total bytes to transfer, if known.
    /// `None` when the provider cannot determine the total up-front.
    pub total_bytes: Option<u64>,

    /// Overall percentage of completion (0.0–100.0), if known.
    /// `None` when progress cannot be determined.
    pub percentage: Option<f64>,

    /// Estimated remaining time in seconds, if known.
    pub estimated_remaining_seconds: Option<u64>,

    /// The current synchronization status of the folder.
    pub status: SyncStatus,
}

impl Default for SyncProgress {
    fn default() -> Self {
        SyncProgress {
            operation: String::new(),
            completed_items: 0,
            total_items: None,
            bytes_transferred: 0,
            total_bytes: None,
            percentage: None,
            estimated_remaining_seconds: None,
            status: SyncStatus::default(),
        }
    }
}

impl SyncProgress {
    /// Creates a new `SyncProgress` with the given operation label and
    /// a default (`NotConfigured`) status.
    pub fn new(operation: impl Into<String>) -> Self {
        SyncProgress {
            operation: operation.into(),
            ..Default::default()
        }
    }

    /// Sets the current operation label.
    pub fn with_operation(mut self, op: impl Into<String>) -> Self {
        self.operation = op.into();
        self
    }

    /// Sets the item counts (completed and optionally total).
    pub fn with_items(mut self, completed: u64, total: Option<u64>) -> Self {
        self.completed_items = completed;
        self.total_items = total;
        self
    }

    /// Sets the byte counts (transferred and optionally total).
    pub fn with_bytes(mut self, transferred: u64, total: Option<u64>) -> Self {
        self.bytes_transferred = transferred;
        self.total_bytes = total;
        self
    }

    /// Sets the percentage directly (0.0–100.0).
    /// Use this when the provider has its own progress calculation.
    pub fn with_percentage(mut self, pct: f64) -> Self {
        self.percentage = Some(pct);
        self
    }

    /// Sets the estimated remaining time in seconds.
    pub fn with_eta(mut self, seconds: u64) -> Self {
        self.estimated_remaining_seconds = Some(seconds);
        self
    }

    /// Sets the current [`SyncStatus`].
    pub fn with_status(mut self, status: SyncStatus) -> Self {
        self.status = status;
        self
    }

    /// Derives a percentage from the item counts, if both are available.
    /// Returns `None` when `total_items` is `None` or zero.
    pub fn percentage_from_items(&self) -> Option<f64> {
        let total = self.total_items?;
        if total == 0 {
            return Some(0.0);
        }
        Some((self.completed_items as f64 / total as f64) * 100.0)
    }

    /// Derives a percentage from the byte counts, if both are available.
    /// Returns `None` when `total_bytes` is `None` or zero.
    pub fn percentage_from_bytes(&self) -> Option<f64> {
        let total = self.total_bytes?;
        if total == 0 {
            return Some(0.0);
        }
        Some((self.bytes_transferred as f64 / total as f64) * 100.0)
    }

    /// Computes the overall percentage, using the explicitly-set value if
    /// present, otherwise deriving from bytes or items.
    pub fn computed_percentage(&self) -> Option<f64> {
        if let Some(p) = self.percentage {
            return Some(p);
        }
        if let Some(p) = self.percentage_from_bytes() {
            return Some(p);
        }
        self.percentage_from_items()
    }

    /// Validates the internal consistency of this progress record.
    ///
    /// Returns `Ok(())` when all invariants hold, or a [`SyncError`]
    /// describing the first inconsistency found.
    ///
    /// # Invariants
    ///
    /// - `percentage`, if present, must be in the range [0.0, 100.0].
    /// - `completed_items` must not exceed `total_items` (when total is known).
    /// - `bytes_transferred` must not exceed `total_bytes` (when total is known).
    pub fn validate(&self) -> SyncResult<()> {
        // Percentage range
        if let Some(p) = self.percentage {
            if !(0.0..=100.0).contains(&p) {
                return Err(SyncError::invalid_progress(format!(
                    "percentage {} is out of range [0.0, 100.0]",
                    p
                )));
            }
        }

        // completed_items <= total_items
        if let Some(total) = self.total_items {
            if self.completed_items > total {
                return Err(SyncError::invalid_progress(format!(
                    "completed_items ({}) exceeds total_items ({})",
                    self.completed_items, total
                )));
            }
        }

        // bytes_transferred <= total_bytes
        if let Some(total) = self.total_bytes {
            if self.bytes_transferred > total {
                return Err(SyncError::invalid_progress(format!(
                    "bytes_transferred ({}) exceeds total_bytes ({})",
                    self.bytes_transferred, total
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod sync_model {
    use super::*;

    #[test]
    fn sync_model_progress_default() {
        let p = SyncProgress::default();
        assert!(p.operation.is_empty());
        assert_eq!(p.completed_items, 0);
        assert!(p.total_items.is_none());
        assert_eq!(p.bytes_transferred, 0);
        assert!(p.total_bytes.is_none());
        assert!(p.percentage.is_none());
        assert!(p.estimated_remaining_seconds.is_none());
        assert_eq!(p.status, SyncStatus::NotConfigured);
    }

    #[test]
    fn sync_model_progress_new() {
        let p = SyncProgress::new("uploading");
        assert_eq!(p.operation, "uploading");
        assert_eq!(p.status, SyncStatus::NotConfigured);
    }

    #[test]
    fn sync_model_progress_builder_methods() {
        let p = SyncProgress::new("scanning")
            .with_items(5, Some(10))
            .with_bytes(1024, Some(4096))
            .with_percentage(50.0)
            .with_eta(30)
            .with_status(SyncStatus::Syncing);

        assert_eq!(p.operation, "scanning");
        assert_eq!(p.completed_items, 5);
        assert_eq!(p.total_items, Some(10));
        assert_eq!(p.bytes_transferred, 1024);
        assert_eq!(p.total_bytes, Some(4096));
        assert_eq!(p.percentage, Some(50.0));
        assert_eq!(p.estimated_remaining_seconds, Some(30));
        assert_eq!(p.status, SyncStatus::Syncing);
    }

    #[test]
    fn sync_model_progress_percentage_from_items() {
        let p = SyncProgress::new("test").with_items(3, Some(10));
        assert_eq!(p.percentage_from_items(), Some(30.0));
    }

    #[test]
    fn sync_model_progress_percentage_from_items_no_total() {
        let p = SyncProgress::new("test").with_items(3, None);
        assert_eq!(p.percentage_from_items(), None);
    }

    #[test]
    fn sync_model_progress_percentage_from_items_zero_total() {
        let p = SyncProgress::new("test").with_items(0, Some(0));
        assert_eq!(p.percentage_from_items(), Some(0.0));
    }

    #[test]
    fn sync_model_progress_percentage_from_items_complete() {
        let p = SyncProgress::new("test").with_items(10, Some(10));
        assert_eq!(p.percentage_from_items(), Some(100.0));
    }

    #[test]
    fn sync_model_progress_percentage_from_bytes() {
        let p = SyncProgress::new("test").with_bytes(512, Some(2048));
        assert_eq!(p.percentage_from_bytes(), Some(25.0));
    }

    #[test]
    fn sync_model_progress_percentage_from_bytes_no_total() {
        let p = SyncProgress::new("test").with_bytes(512, None);
        assert_eq!(p.percentage_from_bytes(), None);
    }

    #[test]
    fn sync_model_progress_computed_percentage_uses_explicit() {
        let p = SyncProgress::new("test")
            .with_items(3, Some(10))
            .with_bytes(512, Some(2048))
            .with_percentage(99.0);
        assert_eq!(p.computed_percentage(), Some(99.0));
    }

    #[test]
    fn sync_model_progress_computed_percentage_derived_from_bytes() {
        let p = SyncProgress::new("test")
            .with_bytes(512, Some(2048))
            .with_items(3, Some(10));
        assert_eq!(p.computed_percentage(), Some(25.0));
    }

    #[test]
    fn sync_model_progress_computed_percentage_derived_from_items() {
        let p = SyncProgress::new("test")
            .with_items(3, Some(10));
        assert_eq!(p.computed_percentage(), Some(30.0));
    }

    #[test]
    fn sync_model_progress_computed_percentage_none_when_all_unknown() {
        let p = SyncProgress::new("test").with_items(3, None);
        assert_eq!(p.computed_percentage(), None);
    }

    #[test]
    fn sync_model_progress_validate_ok() {
        let p = SyncProgress::new("test")
            .with_items(5, Some(10))
            .with_bytes(512, Some(1024))
            .with_percentage(50.0);
        assert!(p.validate().is_ok());
    }

    #[test]
    fn sync_model_progress_validate_empty_ok() {
        let p = SyncProgress::default();
        assert!(p.validate().is_ok());
    }

    #[test]
    fn sync_model_progress_validate_percentage_out_of_range() {
        let p = SyncProgress::new("test").with_percentage(150.0);
        assert!(p.validate().is_err());
    }

    #[test]
    fn sync_model_progress_validate_percentage_negative() {
        let p = SyncProgress::new("test").with_percentage(-1.0);
        assert!(p.validate().is_err());
    }

    #[test]
    fn sync_model_progress_validate_completed_exceeds_total_items() {
        let p = SyncProgress::new("test").with_items(11, Some(10));
        assert!(p.validate().is_err());
    }

    #[test]
    fn sync_model_progress_validate_bytes_exceeds_total() {
        let p = SyncProgress::new("test").with_bytes(2049, Some(2048));
        assert!(p.validate().is_err());
    }

    #[test]
    fn sync_model_progress_round_trip() {
        let p = SyncProgress::new("uploading doc.md")
            .with_items(5, Some(10))
            .with_bytes(1024, Some(4096))
            .with_percentage(50.0)
            .with_eta(30)
            .with_status(SyncStatus::Syncing);

        let json = serde_json::to_string(&p).unwrap();
        let back: SyncProgress = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn sync_model_progress_forward_compatible() {
        let json = r#"{
            "operation": "test",
            "completed_items": 5,
            "total_items": 10,
            "bytes_transferred": 100,
            "total_bytes": 200,
            "percentage": 50.0,
            "estimated_remaining_seconds": 30,
            "status": "syncing",
            "future_field": "ignored"
        }"#;
        let p: SyncProgress = serde_json::from_str(json).unwrap();
        assert_eq!(p.operation, "test");
        assert_eq!(p.completed_items, 5);
    }

    #[test]
    fn sync_model_progress_empty_deserializes() {
        let p: SyncProgress = serde_json::from_str("{}").unwrap();
        assert_eq!(p, SyncProgress::default());
    }

    #[test]
    fn sync_model_progress_validate_ok_at_boundaries() {
        let p1 = SyncProgress::new("test").with_percentage(0.0);
        assert!(p1.validate().is_ok());

        let p2 = SyncProgress::new("test").with_percentage(100.0);
        assert!(p2.validate().is_ok());
    }

    #[test]
    fn sync_model_progress_computed_percentage_boundary_zero() {
        let p = SyncProgress::new("test").with_items(0, Some(0));
        assert_eq!(p.computed_percentage(), Some(0.0));
    }
}
