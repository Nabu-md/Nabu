//! # Synchronization Domain Models
//!
//! Canonical, provider-agnostic models for the Nabu synchronization
//! capability. Every future synchronization provider — Syncthing, iCloud,
//! OneDrive, Dropbox, Google Drive, WebDAV, Git, or any custom backend
//! — translates its native representation into these shared types.
//!
//! ## Core types
//!
//! | Type                  | File             | Kind  | Purpose                                      |
//! |-----------------------|------------------|-------|----------------------------------------------|
//! | [`SyncFolder`]        | [`folder`]       | struct | A local folder paired with a remote backend |
//! | [`SyncStatus`]        | [`status`]       | enum   | Strongly-typed synchronization state        |
//! | [`ConflictResolution`]| [`conflict`]     | enum   | Conflict-resolution strategy vocabulary     |
//! | [`SyncProgress`]      | [`progress`]     | struct | In-flight operation progress snapshot       |
//!
//! ## Supporting types
//!
//! | Type                  | File             | Purpose                                     |
//! |-----------------------|------------------|---------------------------------------------|
//! | [`SyncConfig`]        | [`folder`]       | Per-folder synchronization configuration    |
//! | [`SyncScheduleMode`]  | [`folder`]       | Schedule mode (interval, cron, on-demand…)   |
//! | [`ConflictEntry`]     | [`conflict`]     | A specific file-level conflict record       |
//! | [`SyncError`]         | [`error`]        | Structured validation errors                |
//!
//! ## Provider independence
//!
//! None of these types reference a specific synchronization backend. The
//! `provider_id` field on [`SyncFolder`] is an opaque `String` that
//! providers set to their own identifier (e.g. `"syncthing"`, `"icloud"`).
//! The platform never matches on specific provider IDs — providers do.
//!
//! ## Thread safety
//!
//! All types in this module are plain data (`Clone`, `Send`, `Sync`). They
//! contain no interior mutability, no `Arc`/`Rc`, and no shared state.
//! They are safe to pass across thread boundaries and can be stored in
//! shared collections guarded by `RwLock`.
//!
//! ## Lifecycle
//!
//! ```text
//! SyncFolder (created)
//!     │  status = NotConfigured
//!     ▼
//! provider.configure(SyncConfig)
//!     │  status = Idle
//!     ▼
//! provider.sync() ──▶ SyncProgress (via EventBus)
//!     │  status = Syncing → UpToDate | Conflict | Error | Offline | Paused
//!     ▼
//! (on conflict) ──▶ ConflictEntry ──▶ ConflictResolution (strategy)
//! ```
//!
//! ## Future compatibility
//!
//! - All struct fields use `#[serde(default)]` so new fields can be added
//!   without breaking deserialization (mirrors the pattern used by
//!   [`ServiceHealth`](crate::registry::health::ServiceHealth) and
//!   [`CapabilityRegistry`](crate::plugin::capability::CapabilityRegistry)).
//! - All enums use `#[non_exhaustive]` so new variants can be added without
//!   breaking external exhaustive matches.
//! - Validation is performed by explicit `validate()` methods that return
//!   [`SyncError`] — never panics.

pub mod conflict;
pub mod error;
pub mod folder;
pub mod progress;
pub mod status;

pub use conflict::{ConflictEntry, ConflictResolution};
pub use error::{SyncError, SyncResult};
pub use folder::{SyncConfig, SyncFolder, SyncScheduleMode};
pub use progress::SyncProgress;
pub use status::SyncStatus;
