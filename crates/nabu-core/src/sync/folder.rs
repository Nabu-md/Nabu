//! Synchronization folder model.
//!
//! [`SyncFolder`] represents a local folder that is synchronized with a
//! remote backend by a synchronization provider. It is intentionally
//! provider-agnostic: the `provider_id` field is an opaque identifier that
//! future providers (Syncthing, iCloud, Git, WebDAV, …) will set to their
//! own well-known string.
//!
//! [`SyncConfig`] holds the per-folder synchronization configuration. It is
//! referenced by [`SyncFolder`] and may also be embedded in capability
//! manifests or workspace settings.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::sync::conflict::ConflictResolution;
use crate::sync::error::{SyncError, SyncResult};
use crate::sync::status::SyncStatus;

// ---------------------------------------------------------------------------
// SyncScheduleMode — when to trigger automatic synchronization
// ---------------------------------------------------------------------------

/// The schedule mode for automatic synchronization of a folder.
///
/// This enum captures *how* a folder is scheduled for synchronization
/// without coupling to any specific scheduler implementation. Future
/// providers translate their native scheduling into one of these variants.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncScheduleMode {
    /// Synchronization runs on a fixed interval (see
    /// [`SyncConfig::sync_interval_seconds`]).
    Interval,

    /// Synchronization uses a cron-like expression (see
    /// [`SyncConfig::cron_schedule`]).
    Cron,

    /// Synchronization is triggered manually (on-demand only).
    #[default]
    OnDemand,

    /// Synchronization runs continuously (e.g. real-time file-watching via
    /// inotify / FSEvents).
    Continuous,
}

impl SyncScheduleMode {
    /// Returns a human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            SyncScheduleMode::Interval => "interval",
            SyncScheduleMode::Cron => "cron",
            SyncScheduleMode::OnDemand => "on demand",
            SyncScheduleMode::Continuous => "continuous",
        }
    }
}

// ---------------------------------------------------------------------------
// SyncConfig — per-folder synchronization configuration
// ---------------------------------------------------------------------------

/// Per-folder synchronization configuration.
///
/// `SyncConfig` is embedded in [`SyncFolder`] and may also be referenced
/// directly from capability manifests or workspace-level settings.
///
/// # Future compatibility
///
/// All fields use `#[serde(default)]` so that future phases can add
/// configuration options (e.g. `encryption_metadata`, `bandwidth_schedule`,
/// `version_retention`) without breaking deserialization of existing
/// serialized data.
///
/// # Ownership
///
/// `SyncConfig` is a value type (`Clone`, `Send`, `Sync`). It contains no
/// interior mutability and no shared state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncConfig {
    /// Interval in seconds between automatic sync cycles.
    /// `None` means synchronization is not time-based (e.g. on-demand or
    /// continuous). When `Some(n)`, the provider should sync at most every
    /// `n` seconds.
    pub sync_interval_seconds: Option<u64>,

    /// Bandwidth limit in kilobytes per second. `None` means no limit.
    pub bandwidth_limit_kbps: Option<u64>,

    /// Glob patterns to exclude from synchronization (e.g. `"*.tmp"`,
    /// `".git/**"`). Patterns use the provider's glob syntax; the platform
    /// does not interpret them.
    pub ignore_patterns: Vec<String>,

    /// Specific paths within the folder that should be synchronized
    /// (selective sync). An empty list means "sync everything not excluded
    /// by `ignore_patterns`". Non-empty means "sync only these paths, minus
    /// `ignore_patterns`".
    pub selective_sync_paths: Vec<String>,

    /// Whether at-rest encryption is enabled for this folder's sync
    /// metadata. The actual encryption implementation is provider-specific;
    /// this flag is a shared signal for the platform.
    #[serde(default)]
    pub encryption_enabled: bool,

    /// Whether version history is maintained for synced files. When enabled,
    /// the provider retains a configurable number of historical versions
    /// for conflict recovery and undo.
    #[serde(default)]
    pub version_history_enabled: bool,

    /// The default conflict resolution strategy for this folder.
    /// Individual conflicts may override this via [`ConflictResolution`].
    #[serde(default)]
    pub conflict_resolution: ConflictResolution,

    /// The schedule mode for automatic synchronization.
    #[serde(default)]
    pub schedule_mode: SyncScheduleMode,

    /// A cron-style expression for scheduling (used when `schedule_mode`
    /// is [`SyncScheduleMode::Cron`]). The format is provider-dependent;
    /// the platform stores it opaquely.
    pub cron_schedule: Option<String>,
}

impl Default for SyncConfig {
    fn default() -> Self {
        SyncConfig {
            sync_interval_seconds: None,
            bandwidth_limit_kbps: None,
            ignore_patterns: Vec::new(),
            selective_sync_paths: Vec::new(),
            encryption_enabled: false,
            version_history_enabled: false,
            conflict_resolution: ConflictResolution::default(),
            schedule_mode: SyncScheduleMode::default(),
            cron_schedule: None,
        }
    }
}

impl SyncConfig {
    /// Creates a new `SyncConfig` with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the sync interval in seconds.
    pub fn with_interval(mut self, seconds: u64) -> Self {
        self.sync_interval_seconds = Some(seconds);
        self.schedule_mode = SyncScheduleMode::Interval;
        self
    }

    /// Sets the bandwidth limit in KB/s.
    pub fn with_bandwidth_limit(mut self, kbps: u64) -> Self {
        self.bandwidth_limit_kbps = Some(kbps);
        self
    }

    /// Adds a glob pattern to the ignore list.
    pub fn with_ignore_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.ignore_patterns.push(pattern.into());
        self
    }

    /// Sets the selective sync paths.
    pub fn with_selective_sync(mut self, paths: Vec<String>) -> Self {
        self.selective_sync_paths = paths;
        self
    }

    /// Enables or disables encryption.
    pub fn with_encryption(mut self, enabled: bool) -> Self {
        self.encryption_enabled = enabled;
        self
    }

    /// Enables or disables version history.
    pub fn with_version_history(mut self, enabled: bool) -> Self {
        self.version_history_enabled = enabled;
        self
    }

    /// Sets the default conflict resolution strategy.
    pub fn with_conflict_resolution(mut self, strategy: ConflictResolution) -> Self {
        self.conflict_resolution = strategy;
        self
    }

    /// Sets the schedule mode to continuous.
    pub fn with_continuous_schedule(mut self) -> Self {
        self.schedule_mode = SyncScheduleMode::Continuous;
        self.sync_interval_seconds = None;
        self
    }

    /// Sets the schedule mode to cron with the given expression.
    pub fn with_cron_schedule(mut self, expr: impl Into<String>) -> Self {
        self.schedule_mode = SyncScheduleMode::Cron;
        self.cron_schedule = Some(expr.into());
        self
    }

    /// Returns `true` if selective sync is active (i.e. the
    /// `selective_sync_paths` list is non-empty).
    pub fn is_selective(&self) -> bool {
        !self.selective_sync_paths.is_empty()
    }

    /// Validates the configuration for internal consistency.
    ///
    /// # Invariants
    ///
    /// - `sync_interval_seconds`, if `Some`, must be > 0.
    /// - `bandwidth_limit_kbps`, if `Some`, must be > 0.
    /// - If `schedule_mode` is `Cron`, `cron_schedule` must be `Some` and non-empty.
    /// - If `schedule_mode` is `Interval`, `sync_interval_seconds` must be `Some` and > 0.
    pub fn validate(&self) -> SyncResult<()> {
        if let Some(interval) = self.sync_interval_seconds {
            if interval == 0 {
                return Err(SyncError::invalid_progress(
                    "sync_interval_seconds must be greater than zero",
                ));
            }
        }

        if let Some(bw) = self.bandwidth_limit_kbps {
            if bw == 0 {
                return Err(SyncError::invalid_progress(
                    "bandwidth_limit_kbps must be greater than zero",
                ));
            }
        }

        if self.schedule_mode == SyncScheduleMode::Cron {
            if self.cron_schedule.is_none()
                || self.cron_schedule.as_deref().is_some_and(|s| s.is_empty())
            {
                return Err(SyncError::invalid_progress(
                    "cron_schedule must be set when schedule_mode is Cron",
                ));
            }
        }

        if self.schedule_mode == SyncScheduleMode::Interval {
            if self.sync_interval_seconds.is_none()
                || self.sync_interval_seconds == Some(0)
            {
                return Err(SyncError::invalid_progress(
                    "sync_interval_seconds must be set and > 0 when schedule_mode is Interval",
                ));
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SyncFolder — the canonical folder model
// ---------------------------------------------------------------------------

/// A local folder that is synchronized with a remote backend.
///
/// `SyncFolder` is the canonical model for a synchronized folder. Every
/// synchronization provider (Syncthing, iCloud, Git, WebDAV, …) produces
/// `SyncFolder` values when it discovers or creates a folder to sync, and
/// translates its native folder representation into this type.
///
/// # Ownership
///
/// `SyncFolder` is a plain data struct (`Clone`, `Send`, `Sync`). It owns
/// all of its data — no `Arc` or `Rc` is needed. The platform may store
/// collections of `SyncFolder` values in hash maps keyed by `id`.
///
/// # Lifecycle expectations
///
/// 1. **Created** — a `SyncFolder` is constructed with a unique `id`, a
///    local `path`, a `display_name`, and a `provider_id`. Its initial
///    `status` is [`SyncStatus::NotConfigured`].
/// 2. **Configured** — the provider sets a [`SyncConfig`] and transitions
///    the status to [`SyncStatus::Idle`].
/// 3. **Syncing** — the provider transitions the status to
///    [`SyncStatus::Syncing`] and reports [`SyncProgress`] via the EventBus.
/// 4. **Steady state** — the status settles at [`SyncStatus::UpToDate`],
///    [`SyncStatus::Conflict`], [`SyncStatus::Error`], etc.
///
/// # Future compatibility
///
/// All fields use `#[serde(default)]` so that future phases can add
/// metadata (e.g. `encryption_metadata`, `group_id` for folder groups,
/// `schedule` overrides, `remote_id`) without breaking deserialization.
/// This mirrors the pattern used by [`ServiceHealth`] and
/// [`CapabilityRegistry`].
///
/// [`ServiceHealth`]: crate::registry::health::ServiceHealth
/// [`CapabilityRegistry`]: crate::plugin::capability::CapabilityRegistry
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SyncFolder {
    /// Unique identifier for this synced folder.
    pub id: Uuid,

    /// The local filesystem path of the folder to synchronize.
    ///
    /// This is a `String` (not `PathBuf`) for cross-platform serialization
    /// and wasm-friendliness, matching the convention used by
    /// [`ObjectMetadata::vault_path`](crate::models::ObjectMetadata).
    pub local_path: String,

    /// Human-readable name shown in the UI.
    pub display_name: String,

    /// Provider identifier — an opaque string that identifies which
    /// synchronization provider manages this folder (e.g. `"syncthing"`,
    /// `"icloud"`, `"webdav"`). The platform does not interpret this value.
    pub provider_id: String,

    /// Per-folder synchronization configuration.
    #[serde(default)]
    pub config: SyncConfig,

    /// Current synchronization status. Defaults to
    /// [`SyncStatus::NotConfigured`].
    #[serde(default)]
    pub status: SyncStatus,

    /// The most recent successful sync timestamp, if any.
    pub last_sync: Option<DateTime<Utc>>,

    /// Error message when `status` is [`SyncStatus::Error`], or `None`
    /// otherwise. Providers should clear this when transitioning away from
    /// the error state.
    pub error: Option<String>,
}

impl SyncFolder {
    /// Creates a new `SyncFolder` with the given id, local path, display
    /// name, and provider identifier.
    ///
    /// The initial status is [`SyncStatus::NotConfigured`] and no error
    /// message is set. Call [`SyncFolder::validate`] to check that required
    /// fields are populated.
    pub fn new(
        local_path: impl Into<String>,
        display_name: impl Into<String>,
        provider_id: impl Into<String>,
    ) -> Self {
        SyncFolder {
            id: Uuid::new_v4(),
            local_path: local_path.into(),
            display_name: display_name.into(),
            provider_id: provider_id.into(),
            config: SyncConfig::default(),
            status: SyncStatus::default(),
            last_sync: None,
            error: None,
        }
    }

    /// Creates a new `SyncFolder` with an explicit (provided) id.
    ///
    /// Use this when reconstructing a folder from persisted state. For new
    /// folders, prefer [`SyncFolder::new`] which generates a v4 UUID.
    pub fn with_id(
        id: Uuid,
        local_path: impl Into<String>,
        display_name: impl Into<String>,
        provider_id: impl Into<String>,
    ) -> Self {
        SyncFolder {
            id,
            local_path: local_path.into(),
            display_name: display_name.into(),
            provider_id: provider_id.into(),
            config: SyncConfig::default(),
            status: SyncStatus::default(),
            last_sync: None,
            error: None,
        }
    }

    /// Sets the synchronization configuration.
    pub fn with_config(mut self, config: SyncConfig) -> Self {
        self.config = config;
        self
    }

    /// Sets the synchronization status.
    pub fn with_status(mut self, status: SyncStatus) -> Self {
        self.status = status;
        self
    }

    /// Sets the last-sync timestamp.
    pub fn with_last_sync(mut self, ts: DateTime<Utc>) -> Self {
        self.last_sync = Some(ts);
        self
    }

    /// Sets an error message (typically used when transitioning to
    /// [`SyncStatus::Error`]).
    pub fn with_error(mut self, msg: impl Into<String>) -> Self {
        self.error = Some(msg.into());
        self
    }

    /// Clears the error message and transitions to [`SyncStatus::Idle`].
    pub fn clear_error(mut self) -> Self {
        self.error = None;
        self.status = SyncStatus::Idle;
        self
    }

    /// Validates that the folder has all required fields populated.
    ///
    /// # Invariants
    ///
    /// - `id` must not be nil (i.e. not `Uuid::nil()`).
    /// - `local_path` must be non-empty.
    /// - `display_name` must be non-empty.
    /// - `provider_id` must be non-empty.
    /// - The embedded [`SyncConfig`] must pass its own validation.
    pub fn validate(&self) -> SyncResult<()> {
        if self.id == Uuid::nil() {
            return Err(SyncError::invalid_folder(
                self.id.to_string(),
                "id must not be nil (use SyncFolder::new for v4 UUIDs)",
            ));
        }

        if self.local_path.is_empty() {
            return Err(SyncError::invalid_folder(
                self.id.to_string(),
                "local_path must be non-empty",
            ));
        }

        if self.display_name.is_empty() {
            return Err(SyncError::invalid_folder(
                self.id.to_string(),
                "display_name must be non-empty",
            ));
        }

        if self.provider_id.is_empty() {
            return Err(SyncError::invalid_folder(
                self.id.to_string(),
                "provider_id must be non-empty",
            ));
        }

        self.config.validate()?;

        Ok(())
    }

    /// Returns `true` if the folder is in a healthy state (i.e. not in
    /// `Error`, `Offline`, or `NotConfigured` status).
    pub fn is_healthy(&self) -> bool {
        !matches!(
            self.status,
            SyncStatus::Error | SyncStatus::Offline | SyncStatus::NotConfigured
        )
    }

    /// Returns `true` if the folder is currently syncing.
    pub fn is_syncing(&self) -> bool {
        self.status == SyncStatus::Syncing
    }
}

impl Default for SyncFolder {
    fn default() -> Self {
        SyncFolder {
            id: Uuid::nil(),
            local_path: String::new(),
            display_name: String::new(),
            provider_id: String::new(),
            config: SyncConfig::default(),
            status: SyncStatus::default(),
            last_sync: None,
            error: None,
        }
    }
}

#[cfg(test)]
mod sync_model {
    use super::*;

    #[test]
    fn sync_model_folder_new_generates_uuid() {
        let folder = SyncFolder::new("/path/to/vault", "My Vault", "syncthing");
        assert_ne!(folder.id, Uuid::nil());
        assert!(!folder.id.to_string().is_empty());
        assert_eq!(folder.local_path, "/path/to/vault");
        assert_eq!(folder.display_name, "My Vault");
        assert_eq!(folder.provider_id, "syncthing");
        assert_eq!(folder.status, SyncStatus::NotConfigured);
        assert!(folder.last_sync.is_none());
        assert!(folder.error.is_none());
    }

    #[test]
    fn sync_model_folder_with_id_preserves_id() {
        let id = Uuid::new_v4();
        let folder = SyncFolder::with_id(id, "/data", "Data", "icloud");
        assert_eq!(folder.id, id);
    }

    #[test]
    fn sync_model_folder_builder_methods() {
        let now = Utc::now();
        let config = SyncConfig::new().with_interval(300);
        let folder = SyncFolder::new("/vault", "Vault", "webdav")
            .with_config(config.clone())
            .with_status(SyncStatus::UpToDate)
            .with_last_sync(now)
            .with_error("something went wrong");

        assert_eq!(folder.config, config);
        assert_eq!(folder.status, SyncStatus::UpToDate);
        assert_eq!(folder.last_sync, Some(now));
        assert_eq!(folder.error.as_deref(), Some("something went wrong"));
    }

    #[test]
    fn sync_model_folder_clear_error() {
        let folder = SyncFolder::new("/vault", "Vault", "git")
            .with_status(SyncStatus::Error)
            .with_error("failed");
        assert_eq!(folder.status, SyncStatus::Error);
        assert!(folder.error.is_some());

        let cleared = folder.clear_error();
        assert_eq!(cleared.status, SyncStatus::Idle);
        assert!(cleared.error.is_none());
    }

    #[test]
    fn sync_model_folder_validate_ok() {
        let folder = SyncFolder::new("/vault", "Vault", "syncthing");
        assert!(folder.validate().is_ok());
    }

    #[test]
    fn sync_model_folder_validate_rejects_nil_id() {
        let folder = SyncFolder::default();
        assert!(folder.validate().is_err());
    }

    #[test]
    fn sync_model_folder_validate_rejects_empty_path() {
        let mut folder = SyncFolder::new("/vault", "Vault", "syncthing");
        folder.local_path = String::new();
        assert!(folder.validate().is_err());
    }

    #[test]
    fn sync_model_folder_validate_rejects_empty_display_name() {
        let mut folder = SyncFolder::new("/vault", "Vault", "syncthing");
        folder.display_name = String::new();
        assert!(folder.validate().is_err());
    }

    #[test]
    fn sync_model_folder_validate_rejects_empty_provider_id() {
        let mut folder = SyncFolder::new("/vault", "Vault", "syncthing");
        folder.provider_id = String::new();
        assert!(folder.validate().is_err());
    }

    #[test]
    fn sync_model_folder_is_healthy() {
        let healthy = SyncFolder::new("/vault", "Vault", "syncthing")
            .with_status(SyncStatus::UpToDate);
        assert!(healthy.is_healthy());

        let unhealthy = SyncFolder::new("/vault", "Vault", "syncthing")
            .with_status(SyncStatus::Error)
            .with_error("fail");
        assert!(!unhealthy.is_healthy());
    }

    #[test]
    fn sync_model_folder_is_syncing() {
        let syncing = SyncFolder::new("/vault", "Vault", "syncthing")
            .with_status(SyncStatus::Syncing);
        assert!(syncing.is_syncing());

        let idle = SyncFolder::new("/vault", "Vault", "syncthing")
            .with_status(SyncStatus::Idle);
        assert!(!idle.is_syncing());
    }

    #[test]
    fn sync_model_folder_round_trip() {
        let now = Utc::now();
        let folder = SyncFolder::new("/vault", "Vault", "syncthing")
            .with_config(SyncConfig::new().with_interval(600))
            .with_status(SyncStatus::UpToDate)
            .with_last_sync(now);

        let json = serde_json::to_string(&folder).unwrap();
        let back: SyncFolder = serde_json::from_str(&json).unwrap();
        assert_eq!(folder.id, back.id);
        assert_eq!(folder.local_path, back.local_path);
        assert_eq!(folder.display_name, back.display_name);
        assert_eq!(folder.provider_id, back.provider_id);
        assert_eq!(folder.config, back.config);
        assert_eq!(folder.status, back.status);
        assert_eq!(folder.last_sync, back.last_sync);
    }

    #[test]
    fn sync_model_folder_forward_compatible() {
        let json = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "local_path": "/vault",
            "display_name": "Vault",
            "provider_id": "syncthing",
            "config": {},
            "status": "up_to_date",
            "last_sync": null,
            "error": null,
            "future_field": "ignored"
        }"#;
        let folder: SyncFolder = serde_json::from_str(json).unwrap();
        assert_eq!(folder.display_name, "Vault");
        assert_eq!(folder.provider_id, "syncthing");
        assert_eq!(folder.status, SyncStatus::UpToDate);
    }

    #[test]
    fn sync_model_folder_empty_deserializes() {
        let folder: SyncFolder = serde_json::from_str("{}").unwrap();
        assert_eq!(folder.id, Uuid::nil());
        assert_eq!(folder.local_path, String::new());
        assert_eq!(folder.config, SyncConfig::default());
        assert_eq!(folder.status, SyncStatus::NotConfigured);
    }

    // ---- SyncConfig tests ----

    #[test]
    fn sync_model_config_default() {
        let config = SyncConfig::default();
        assert!(config.sync_interval_seconds.is_none());
        assert!(config.bandwidth_limit_kbps.is_none());
        assert!(config.ignore_patterns.is_empty());
        assert!(config.selective_sync_paths.is_empty());
        assert!(!config.encryption_enabled);
        assert!(!config.version_history_enabled);
        assert_eq!(config.conflict_resolution, ConflictResolution::AskUser);
        assert_eq!(config.schedule_mode, SyncScheduleMode::OnDemand);
        assert!(config.cron_schedule.is_none());
    }

    #[test]
    fn sync_model_config_builder_methods() {
        let config = SyncConfig::new()
            .with_interval(300)
            .with_bandwidth_limit(100)
            .with_ignore_pattern("*.tmp")
            .with_selective_sync(vec!["docs".into()])
            .with_encryption(true)
            .with_version_history(true)
            .with_conflict_resolution(ConflictResolution::NewestWins);

        assert_eq!(config.sync_interval_seconds, Some(300));
        assert_eq!(config.bandwidth_limit_kbps, Some(100));
        assert_eq!(config.ignore_patterns, vec!["*.tmp".to_string()]);
        assert_eq!(config.selective_sync_paths, vec!["docs".to_string()]);
        assert!(config.encryption_enabled);
        assert!(config.version_history_enabled);
        assert_eq!(config.conflict_resolution, ConflictResolution::NewestWins);
        assert_eq!(config.schedule_mode, SyncScheduleMode::Interval);
    }

    #[test]
    fn sync_model_config_continuous_schedule() {
        let config = SyncConfig::new().with_continuous_schedule();
        assert_eq!(config.schedule_mode, SyncScheduleMode::Continuous);
        assert!(config.sync_interval_seconds.is_none());
    }

    #[test]
    fn sync_model_config_cron_schedule() {
        let config = SyncConfig::new().with_cron_schedule("0 * * * *");
        assert_eq!(config.schedule_mode, SyncScheduleMode::Cron);
        assert_eq!(config.cron_schedule.as_deref(), Some("0 * * * *"));
    }

    #[test]
    fn sync_model_config_is_selective() {
        let selective = SyncConfig::new().with_selective_sync(vec!["a".into()]);
        assert!(selective.is_selective());

        let full = SyncConfig::new();
        assert!(!full.is_selective());
    }

    #[test]
    fn sync_model_config_validate_ok() {
        assert!(SyncConfig::default().validate().is_ok());
        assert!(SyncConfig::new().with_interval(60).validate().is_ok());
        assert!(SyncConfig::new()
            .with_cron_schedule("0 * * * *")
            .validate()
            .is_ok());
    }

    #[test]
    fn sync_model_config_validate_interval_zero() {
        let config = SyncConfig {
            sync_interval_seconds: Some(0),
            schedule_mode: SyncScheduleMode::Interval,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn sync_model_config_validate_bandwidth_zero() {
        let config = SyncConfig {
            bandwidth_limit_kbps: Some(0),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn sync_model_config_validate_cron_missing_schedule() {
        let config = SyncConfig {
            schedule_mode: SyncScheduleMode::Cron,
            cron_schedule: None,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn sync_model_config_validate_cron_empty_schedule() {
        let config = SyncConfig {
            schedule_mode: SyncScheduleMode::Cron,
            cron_schedule: Some(String::new()),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn sync_model_config_validate_interval_missing_seconds() {
        let config = SyncConfig {
            schedule_mode: SyncScheduleMode::Interval,
            sync_interval_seconds: None,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn sync_model_config_round_trip() {
        let config = SyncConfig::new()
            .with_interval(600)
            .with_bandwidth_limit(500)
            .with_ignore_pattern("*.tmp")
            .with_selective_sync(vec!["docs".into(), "images".into()])
            .with_encryption(true)
            .with_version_history(true)
            .with_conflict_resolution(ConflictResolution::KeepLocal);

        let json = serde_json::to_string(&config).unwrap();
        let back: SyncConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, back);
    }

    #[test]
    fn sync_model_config_forward_compatible() {
        let json = r#"{
            "sync_interval_seconds": 300,
            "bandwidth_limit_kbps": 100,
            "ignore_patterns": ["*.tmp"],
            "selective_sync_paths": [],
            "encryption_enabled": true,
            "version_history_enabled": false,
            "conflict_resolution": "merge",
            "schedule_mode": "interval",
            "cron_schedule": null,
            "future_encryption_metadata": {"algorithm": "aes-256-gcm"}
        }"#;
        let config: SyncConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.sync_interval_seconds, Some(300));
        assert_eq!(config.encryption_enabled, true);
        assert_eq!(config.conflict_resolution, ConflictResolution::Merge);
    }

    #[test]
    fn sync_model_config_empty_deserializes() {
        let config: SyncConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config, SyncConfig::default());
    }

    // ---- SyncScheduleMode tests ----

    #[test]
    fn sync_model_schedule_mode_default() {
        assert_eq!(SyncScheduleMode::default(), SyncScheduleMode::OnDemand);
    }

    #[test]
    fn sync_model_schedule_mode_label() {
        assert_eq!(SyncScheduleMode::Interval.label(), "interval");
        assert_eq!(SyncScheduleMode::Cron.label(), "cron");
        assert_eq!(SyncScheduleMode::OnDemand.label(), "on demand");
        assert_eq!(SyncScheduleMode::Continuous.label(), "continuous");
    }

    #[test]
    fn sync_model_schedule_mode_serialization() {
        let cases = vec![
            (SyncScheduleMode::Interval, "\"interval\""),
            (SyncScheduleMode::Cron, "\"cron\""),
            (SyncScheduleMode::OnDemand, "\"on_demand\""),
            (SyncScheduleMode::Continuous, "\"continuous\""),
        ];
        for (mode, expected) in cases {
            let json = serde_json::to_string(&mode).unwrap();
            assert_eq!(json, expected);
            let back: SyncScheduleMode = serde_json::from_str(&json).unwrap();
            assert_eq!(back, mode);
        }
    }
}
