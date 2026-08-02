//! # Recovery & Data Protection — frontend
//!
//! User-facing recovery tooling (Phase 11.3):
//!
//! - [`save_status`] — autosave status context + indicator
//! - [`session`] — session persistence + crash-recovery IPC helpers
//! - [`diff_view`] — side-by-side diff viewer
//! - [`version_history`] — browse / preview / restore / duplicate snapshots
//! - [`recovery_manager`] — snapshot browser across the vault
//! - [`recovery_banner`] — crash-recovery prompt on the dashboard

pub mod diff_view;
pub mod recovery_banner;
pub mod recovery_manager;
pub mod save_status;
pub mod session;
pub mod version_history;

pub use recovery_banner::RecoveryBanner;
pub use recovery_manager::RecoveryManager;
pub use save_status::{SaveStatus, SaveStatusContext, SaveStatusIndicator};
pub use session::{RecoveryStatus, SessionState};
pub use version_history::VersionHistory;
