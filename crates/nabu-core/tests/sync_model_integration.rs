//! # Synchronization Model Integration Tests
//!
//! Verifies that the four core synchronization domain models —
//! [`SyncFolder`], [`SyncStatus`], [`ConflictResolution`], and
//! [`SyncProgress`] — behave correctly when used together, support
//! round-trip serialization, reject invalid state combinations, and remain
//! provider-agnostic.
//!
//! These tests exercise the models as an external consumer would import them
//! (via `nabu_core::sync::...`).

use chrono::Utc;
use uuid::Uuid;

use nabu_core::sync::{
    ConflictEntry, ConflictResolution, SyncConfig, SyncFolder, SyncProgress, SyncScheduleMode,
    SyncStatus,
};

// ---------------------------------------------------------------------------
// Re-exports for readability in tests
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// 1. SyncFolder — construction, validation, and builder
// ---------------------------------------------------------------------------

#[test]
fn sync_model_integration_folder_new() {
    let folder = SyncFolder::new("/vault/notes", "Notes", "syncthing");
    assert_ne!(folder.id, Uuid::nil());
    assert_eq!(folder.local_path, "/vault/notes");
    assert_eq!(folder.display_name, "Notes");
    assert_eq!(folder.provider_id, "syncthing");
    assert_eq!(folder.status, SyncStatus::NotConfigured);
    assert!(folder.last_sync.is_none());
    assert!(folder.error.is_none());
    assert!(folder.config.sync_interval_seconds.is_none());
}

#[test]
fn sync_model_integration_folder_with_id() {
    let id = Uuid::new_v4();
    let folder = SyncFolder::with_id(
        id,
        "/data/photos",
        "Photos",
        "icloud",
    );
    assert_eq!(folder.id, id);
}

#[test]
fn sync_model_integration_folder_validate_accepts_valid() {
    let folder = SyncFolder::new("/vault", "Vault", "syncthing");
    assert!(folder.validate().is_ok());
}

#[test]
fn sync_model_integration_folder_validate_rejects_nil_id() {
    let folder = SyncFolder {
        id: Uuid::nil(),
        local_path: "/vault".into(),
        display_name: "Vault".into(),
        provider_id: "syncthing".into(),
        ..Default::default()
    };
    let err = folder.validate().unwrap_err();
    assert_eq!(err.variant_name(), "invalid_folder");
}

#[test]
fn sync_model_integration_folder_validate_rejects_empty_fields() {
    let mut folder = SyncFolder::new("/vault", "Vault", "syncthing");
    folder.local_path = String::new();
    assert!(folder.validate().is_err());

    folder.local_path = "/vault".into();
    folder.display_name = String::new();
    assert!(folder.validate().is_err());

    folder.display_name = "Vault".into();
    folder.provider_id = String::new();
    assert!(folder.validate().is_err());
}

#[test]
fn sync_model_integration_folder_is_healthy() {
    let healthy = SyncFolder::new("/v", "V", "s").with_status(SyncStatus::UpToDate);
    assert!(healthy.is_healthy());

    let error_folder = SyncFolder::new("/v", "V", "s")
        .with_status(SyncStatus::Error)
        .with_error("oops");
    assert!(!error_folder.is_healthy());

    let offline = SyncFolder::new("/v", "V", "s").with_status(SyncStatus::Offline);
    assert!(!offline.is_healthy());

    let not_configured = SyncFolder::new("/v", "V", "s").with_status(SyncStatus::NotConfigured);
    assert!(!not_configured.is_healthy());
}

// ---------------------------------------------------------------------------
// 2. SyncFolder — serde round-trip
// ---------------------------------------------------------------------------

#[test]
fn sync_model_integration_folder_serde_round_trip() {
    let now = Utc::now();
    let config = SyncConfig::new()
        .with_interval(600)
        .with_bandwidth_limit(500)
        .with_ignore_pattern("*.tmp")
        .with_selective_sync(vec!["docs".into()])
        .with_encryption(true)
        .with_version_history(true)
        .with_conflict_resolution(ConflictResolution::NewestWins);

    let folder = SyncFolder::new("/vault", "Vault", "syncthing")
        .with_config(config)
        .with_status(SyncStatus::UpToDate)
        .with_last_sync(now)
        .with_error("transient error");

    let json = serde_json::to_string(&folder).unwrap();
    let back: SyncFolder = serde_json::from_str(&json).unwrap();
    assert_eq!(folder, back);
}

#[test]
fn sync_model_integration_folder_serde_ignores_unknown_fields() {
    let json = r#"{
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "local_path": "/vault",
        "display_name": "Vault",
        "provider_id": "syncthing",
        "config": {
            "sync_interval_seconds": 300,
            "bandwidth_limit_kbps": 100,
            "ignore_patterns": ["*.tmp"],
            "selective_sync_paths": [],
            "encryption_enabled": true,
            "version_history_enabled": false,
            "conflict_resolution": "keep_local",
            "schedule_mode": "interval",
            "cron_schedule": null,
            "future_option": 42
        },
        "status": "conflict",
        "last_sync": null,
        "error": null,
        "provider_specific_field": {"foo": "bar"}
    }"#;
    let folder: SyncFolder = serde_json::from_str(json).unwrap();
    assert_eq!(folder.display_name, "Vault");
    assert_eq!(folder.provider_id, "syncthing");
    assert_eq!(folder.status, SyncStatus::Conflict);
    assert_eq!(folder.config.conflict_resolution, ConflictResolution::KeepLocal);
    assert_eq!(folder.config.encryption_enabled, true);
}

#[test]
fn sync_model_integration_folder_serde_empty_object() {
    let folder: SyncFolder = serde_json::from_str("{}").unwrap();
    assert_eq!(folder, SyncFolder::default());
}

// ---------------------------------------------------------------------------
// 3. SyncStatus — enum behavior and serialization
// ---------------------------------------------------------------------------

#[test]
fn sync_model_integration_status_enum_values() {
    let all = [
        SyncStatus::NotConfigured,
        SyncStatus::Idle,
        SyncStatus::Syncing,
        SyncStatus::UpToDate,
        SyncStatus::Pending,
        SyncStatus::Conflict,
        SyncStatus::Error,
        SyncStatus::Offline,
        SyncStatus::Paused,
    ];

    for status in &all {
        let json = serde_json::to_string(status).unwrap();
        let back: SyncStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(*status, back);
    }
}

#[test]
fn sync_model_integration_status_serialized_strings() {
    let cases = [
        (SyncStatus::NotConfigured, "not_configured"),
        (SyncStatus::Idle, "idle"),
        (SyncStatus::Syncing, "syncing"),
        (SyncStatus::UpToDate, "up_to_date"),
        (SyncStatus::Pending, "pending"),
        (SyncStatus::Conflict, "conflict"),
        (SyncStatus::Error, "error"),
        (SyncStatus::Offline, "offline"),
        (SyncStatus::Paused, "paused"),
    ];

    for (status, expected) in &cases {
        let json = serde_json::to_string(status).unwrap();
        assert_eq!(json, format!("\"{}\"", expected));
    }
}

#[test]
fn sync_model_integration_status_display_matches_label() {
    assert_eq!(format!("{}", SyncStatus::Idle), SyncStatus::Idle.label());
    assert_eq!(format!("{}", SyncStatus::Syncing), SyncStatus::Syncing.label());
    assert_eq!(format!("{}", SyncStatus::UpToDate), SyncStatus::UpToDate.label());
}

#[test]
fn sync_model_integration_status_unknown_variant_fails() {
    let result: Result<SyncStatus, _> = serde_json::from_str("\"unknown_status\"");
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// 4. ConflictResolution — enum behavior and serialization
// ---------------------------------------------------------------------------

#[test]
fn sync_model_integration_conflict_resolution_enum_values() {
    let all = [
        ConflictResolution::KeepLocal,
        ConflictResolution::KeepRemote,
        ConflictResolution::Merge,
        ConflictResolution::AskUser,
        ConflictResolution::Manual,
        ConflictResolution::NewestWins,
    ];

    for strategy in &all {
        let json = serde_json::to_string(strategy).unwrap();
        let back: ConflictResolution = serde_json::from_str(&json).unwrap();
        assert_eq!(*strategy, back);
    }
}

#[test]
fn sync_model_integration_conflict_resolution_serialized_strings() {
    let cases = [
        (ConflictResolution::KeepLocal, "keep_local"),
        (ConflictResolution::KeepRemote, "keep_remote"),
        (ConflictResolution::Merge, "merge"),
        (ConflictResolution::AskUser, "ask_user"),
        (ConflictResolution::Manual, "manual"),
        (ConflictResolution::NewestWins, "newest_wins"),
    ];

    for (strategy, expected) in &cases {
        let json = serde_json::to_string(strategy).unwrap();
        assert_eq!(json, format!("\"{}\"", expected));
    }
}

#[test]
fn sync_model_integration_conflict_resolution_default_is_ask_user() {
    assert_eq!(ConflictResolution::default(), ConflictResolution::AskUser);
}

#[test]
fn sync_model_integration_conflict_resolution_automatic_vs_manual() {
    assert!(ConflictResolution::KeepLocal.is_automatic());
    assert!(ConflictResolution::KeepRemote.is_automatic());
    assert!(ConflictResolution::Merge.is_automatic());
    assert!(ConflictResolution::NewestWins.is_automatic());
    assert!(!ConflictResolution::AskUser.is_automatic());
    assert!(!ConflictResolution::Manual.is_automatic());

    assert!(ConflictResolution::AskUser.requires_user_input());
    assert!(ConflictResolution::Manual.requires_user_input());
}

// ---------------------------------------------------------------------------
// 5. ConflictEntry — struct behavior
// ---------------------------------------------------------------------------

#[test]
fn sync_model_integration_conflict_entry_new() {
    let entry = ConflictEntry::new("docs/note.md", "folder-123");
    assert_eq!(entry.path, "docs/note.md");
    assert_eq!(entry.folder_id, "folder-123");
    assert!(entry.detected_at.is_some());
    assert!(!entry.is_resolved());
}

#[test]
fn sync_model_integration_conflict_entry_round_trip() {
    let entry = ConflictEntry::new("path/to/file.md", "folder-abc")
        .with_description("both modified")
        .with_strategy(ConflictResolution::NewestWins);

    let json = serde_json::to_string(&entry).unwrap();
    let back: ConflictEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(entry, back);
}

#[test]
fn sync_model_integration_conflict_entry_forward_compatible() {
    let json = r#"{
        "path": "file.txt",
        "folder_id": "f1",
        "strategy": "keep_local",
        "detected_at": "2024-01-01T00:00:00Z",
        "description": "desc",
        "resolved_strategy": null,
        "resolved_at": null,
        "future_field": 123
    }"#;
    let entry: ConflictEntry = serde_json::from_str(json).unwrap();
    assert_eq!(entry.path, "file.txt");
    assert_eq!(entry.strategy, ConflictResolution::KeepLocal);
}

// ---------------------------------------------------------------------------
// 6. SyncProgress — struct behavior and validation
// ---------------------------------------------------------------------------

#[test]
fn sync_model_integration_progress_new_and_builder() {
    let progress = SyncProgress::new("uploading")
        .with_items(5, Some(10))
        .with_bytes(1024, Some(4096))
        .with_percentage(50.0)
        .with_eta(30)
        .with_status(SyncStatus::Syncing);

    assert_eq!(progress.operation, "uploading");
    assert_eq!(progress.completed_items, 5);
    assert_eq!(progress.total_items, Some(10));
    assert_eq!(progress.bytes_transferred, 1024);
    assert_eq!(progress.total_bytes, Some(4096));
    assert_eq!(progress.percentage, Some(50.0));
    assert_eq!(progress.estimated_remaining_seconds, Some(30));
    assert_eq!(progress.status, SyncStatus::Syncing);
}

#[test]
fn sync_model_integration_progress_validate() {
    // Valid
    let p = SyncProgress::new("test")
        .with_items(5, Some(10))
        .with_bytes(512, Some(1024))
        .with_percentage(50.0);
    assert!(p.validate().is_ok());

    // Invalid: percentage > 100
    let p = SyncProgress::new("test").with_percentage(150.0);
    assert!(p.validate().is_err());

    // Invalid: completed > total
    let p = SyncProgress::new("test").with_items(11, Some(10));
    assert!(p.validate().is_err());

    // Invalid: bytes > total bytes
    let p = SyncProgress::new("test").with_bytes(2049, Some(2048));
    assert!(p.validate().is_err());
}

#[test]
fn sync_model_integration_progress_computed_percentage() {
    // Explicit percentage takes priority
    let p = SyncProgress::new("test")
        .with_items(3, Some(10))
        .with_bytes(512, Some(2048))
        .with_percentage(99.0);
    assert_eq!(p.computed_percentage(), Some(99.0));

    // Derived from bytes
    let p = SyncProgress::new("test").with_bytes(512, Some(2048));
    assert_eq!(p.computed_percentage(), Some(25.0));

    // Derived from items
    let p = SyncProgress::new("test").with_items(3, Some(10));
    assert_eq!(p.computed_percentage(), Some(30.0));

    // None when all unknown
    let p = SyncProgress::new("test").with_items(3, None);
    assert_eq!(p.computed_percentage(), None);
}

#[test]
fn sync_model_integration_progress_round_trip() {
    let p = SyncProgress::new("syncing")
        .with_items(50, Some(100))
        .with_bytes(5000, Some(10000))
        .with_percentage(50.0)
        .with_eta(120)
        .with_status(SyncStatus::Syncing);

    let json = serde_json::to_string(&p).unwrap();
    let back: SyncProgress = serde_json::from_str(&json).unwrap();
    assert_eq!(p, back);
}

// ---------------------------------------------------------------------------
// 7. SyncConfig — struct behavior
// ---------------------------------------------------------------------------

#[test]
fn sync_model_integration_config_serde_round_trip() {
    let config = SyncConfig::new()
        .with_interval(300)
        .with_bandwidth_limit(100)
        .with_ignore_pattern("*.tmp")
        .with_selective_sync(vec!["docs".into()])
        .with_encryption(true)
        .with_version_history(true)
        .with_conflict_resolution(ConflictResolution::KeepLocal)
        .with_cron_schedule("0 * * * *");

    let json = serde_json::to_string(&config).unwrap();
    let back: SyncConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config, back);
}

#[test]
fn sync_model_integration_config_validate_interval() {
    let config = SyncConfig {
        sync_interval_seconds: Some(0),
        schedule_mode: SyncScheduleMode::Interval,
        ..Default::default()
    };
    assert!(config.validate().is_err());

    let config = SyncConfig::new().with_interval(60);
    assert!(config.validate().is_ok());
}

#[test]
fn sync_model_integration_config_validate_cron() {
    let config = SyncConfig {
        schedule_mode: SyncScheduleMode::Cron,
        cron_schedule: None,
        ..Default::default()
    };
    assert!(config.validate().is_err());

    let config = SyncConfig::new().with_cron_schedule("0 2 * * *");
    assert!(config.validate().is_ok());
}

// ---------------------------------------------------------------------------
// 8. Cross-type: SyncFolder + SyncConfig + SyncStatus
// ---------------------------------------------------------------------------

#[test]
fn sync_model_integration_folder_with_config_and_status() {
    let config = SyncConfig::new()
        .with_interval(600)
        .with_bandwidth_limit(500)
        .with_conflict_resolution(ConflictResolution::Merge)
        .with_encryption(true)
        .with_version_history(true);

    let now = Utc::now();
    let folder = SyncFolder::new("/vault/notes", "Notes", "syncthing")
        .with_config(config)
        .with_status(SyncStatus::UpToDate)
        .with_last_sync(now);

    assert!(folder.validate().is_ok());
    assert_eq!(folder.status, SyncStatus::UpToDate);
    assert!(folder.is_healthy());
    assert!(!folder.is_syncing());
    assert_eq!(folder.config.conflict_resolution, ConflictResolution::Merge);
    assert!(folder.config.encryption_enabled);
    assert!(folder.config.version_history_enabled);
    assert_eq!(folder.last_sync, Some(now));
}

#[test]
fn sync_model_integration_folder_clear_error() {
    let folder = SyncFolder::new("/v", "V", "s")
        .with_status(SyncStatus::Error)
        .with_error("something broke");

    assert_eq!(folder.status, SyncStatus::Error);
    assert!(folder.error.is_some());

    let cleared = folder.clear_error();
    assert_eq!(cleared.status, SyncStatus::Idle);
    assert!(!cleared.error.is_some());
    assert!(cleared.error.is_none());
}

// ---------------------------------------------------------------------------
// 9. Provider independence — no provider-specific strings leak into models
// ---------------------------------------------------------------------------

#[test]
fn sync_model_integration_folder_accepts_arbitrary_provider_id() {
    // The model must not restrict provider_id to known providers.
    for provider_id in &["syncthing", "icloud", "webdav", "git", "custom_thing_42"] {
        let folder = SyncFolder::new("/v", "V", *provider_id);
        assert_eq!(folder.provider_id, *provider_id);
        assert!(folder.validate().is_ok());
    }
}

// ---------------------------------------------------------------------------
// 10. Thread safety — models are Send + Sync
// ---------------------------------------------------------------------------

#[test]
fn sync_model_integration_types_are_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<SyncFolder>();
    assert_send_sync::<SyncConfig>();
    assert_send_sync::<SyncStatus>();
    assert_send_sync::<ConflictResolution>();
    assert_send_sync::<ConflictEntry>();
    assert_send_sync::<SyncProgress>();
    assert_send_sync::<SyncScheduleMode>();
}
