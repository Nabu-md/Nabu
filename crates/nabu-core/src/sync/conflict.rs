//! Conflict resolution strategy model.
//!
//! [`ConflictResolution`] is the canonical, provider-agnostic enumeration of
//! conflict-resolution strategies. Every synchronization provider — whether
//! Syncthing, iCloud, Git, WebDAV, or a custom implementation — translates
//! its native conflict handling into one of these variants.
//!
//! This module also defines [`ConflictEntry`], a lightweight record describing
//! a specific file-level conflict that was detected. The entry carries just
//! enough metadata for the platform to surface the conflict to the user or
//! to a future conflict-resolution engine, without embedding resolution logic.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// The strategy to use when a synchronization conflict is detected.
///
/// Conflicts arise when the same file (or path) has been modified both
/// locally and remotely before the next sync cycle. This enum models the
/// *possible* strategies — the actual resolution algorithm lives in the
/// provider or a future conflict-resolution engine.
///
/// # Serialization
///
/// Variants serialize to `snake_case` strings (e.g. `"keep_local"`,
/// `"newest_wins"`). The `#[non_exhaustive]` attribute ensures that future
/// strategies can be added without breaking consumers.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    /// Keep the local version and discard (or back up) the remote version.
    KeepLocal,

    /// Keep the remote version and discard (or back up) the local version.
    KeepRemote,

    /// Attempt a three-way merge of the local and remote versions.
    /// The merge may fail — in that case the conflict remains unresolved.
    Merge,

    /// Prompt the user to choose which version to keep.
    /// Providers should surface the available choices through the
    /// EventBus or the UI before applying any resolution.
    AskUser,

    /// Mark the conflict as manual — require explicit user intervention
    /// through the UI or a CLI command. The conflict is not auto-resolved.
    Manual,

    /// Keep whichever version is newer (based on modification timestamp).
    /// If timestamps are equal, falls back to `AskUser`.
    NewestWins,
}

impl Default for ConflictResolution {
    fn default() -> Self {
        ConflictResolution::AskUser
    }
}

impl ConflictResolution {
    /// Returns a human-readable label.
    pub fn label(&self) -> &'static str {
        match self {
            ConflictResolution::KeepLocal => "keep local",
            ConflictResolution::KeepRemote => "keep remote",
            ConflictResolution::Merge => "merge",
            ConflictResolution::AskUser => "ask user",
            ConflictResolution::Manual => "manual",
            ConflictResolution::NewestWins => "newest wins",
        }
    }

    /// Returns `true` if this strategy does not require user interaction
    /// and can be applied automatically by the provider.
    pub fn is_automatic(&self) -> bool {
        matches!(
            self,
            ConflictResolution::KeepLocal
                | ConflictResolution::KeepRemote
                | ConflictResolution::Merge
                | ConflictResolution::NewestWins
        )
    }

    /// Returns `true` if this strategy requires the provider to prompt the
    /// user before resolving the conflict.
    pub fn requires_user_input(&self) -> bool {
        matches!(self, ConflictResolution::AskUser | ConflictResolution::Manual)
    }
}

/// A single file-level conflict detected during synchronization.
///
/// `ConflictEntry` is a *record* — it describes a conflict that was observed
/// but does not perform the resolution. A future conflict-resolution engine
/// consumes entries like this to apply the configured [`ConflictResolution`]
/// strategy.
///
/// # Ownership
///
/// `ConflictEntry` is a value type (`Clone`, `Send`, `Sync`). It is intended
/// to be collected into a `Vec` by providers and published through the
/// EventBus as part of a sync-status or sync-progress report.
///
/// # Future compatibility
///
/// All fields use `#[serde(default)]` so that future phases can add
/// metadata (e.g. conflict diff preview, local/remote checksums, author
/// attribution) without breaking deserialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConflictEntry {
    /// The relative path of the conflicting file, relative to the sync folder
    /// root.
    pub path: String,

    /// The provider-specific identifier of the sync folder this conflict
    /// belongs to.
    pub folder_id: String,

    /// The strategy configured for this folder at the time the conflict
    /// was detected.
    #[serde(default)]
    pub strategy: ConflictResolution,

    /// When the conflict was first detected.
    pub detected_at: Option<DateTime<Utc>>,

    /// A human-readable description of the conflict (e.g. "both sides
    /// modified line 12").
    pub description: Option<String>,

    /// The proposed resolution, if one has been determined. `None` means
    /// the conflict is still unresolved and awaiting a strategy decision.
    pub resolved_strategy: Option<ConflictResolution>,

    /// When the conflict was resolved, if applicable.
    pub resolved_at: Option<DateTime<Utc>>,
}

impl Default for ConflictEntry {
    fn default() -> Self {
        ConflictEntry {
            path: String::new(),
            folder_id: String::new(),
            strategy: ConflictResolution::default(),
            detected_at: None,
            description: None,
            resolved_strategy: None,
            resolved_at: None,
        }
    }
}

impl ConflictEntry {
    /// Creates a new conflict entry for the given path and folder.
    pub fn new(path: impl Into<String>, folder_id: impl Into<String>) -> Self {
        ConflictEntry {
            path: path.into(),
            folder_id: folder_id.into(),
            strategy: ConflictResolution::default(),
            detected_at: Some(Utc::now()),
            description: None,
            resolved_strategy: None,
            resolved_at: None,
        }
    }

    /// Sets the conflict description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Sets the configured resolution strategy.
    pub fn with_strategy(mut self, strategy: ConflictResolution) -> Self {
        self.strategy = strategy;
        self
    }

    /// Marks the conflict as resolved with the given strategy and timestamp.
    pub fn resolved_at(mut self, strategy: ConflictResolution, at: DateTime<Utc>) -> Self {
        self.resolved_strategy = Some(strategy);
        self.resolved_at = Some(at);
        self
    }

    /// Returns `true` if the conflict has been resolved.
    pub fn is_resolved(&self) -> bool {
        self.resolved_strategy.is_some() && self.resolved_at.is_some()
    }
}

#[cfg(test)]
mod sync_model {
    use super::*;

    #[test]
    fn sync_model_conflict_resolution_default_is_ask_user() {
        assert_eq!(
            ConflictResolution::default(),
            ConflictResolution::AskUser
        );
    }

    #[test]
    fn sync_model_conflict_resolution_label() {
        assert_eq!(ConflictResolution::KeepLocal.label(), "keep local");
        assert_eq!(ConflictResolution::KeepRemote.label(), "keep remote");
        assert_eq!(ConflictResolution::Merge.label(), "merge");
        assert_eq!(ConflictResolution::AskUser.label(), "ask user");
        assert_eq!(ConflictResolution::Manual.label(), "manual");
        assert_eq!(ConflictResolution::NewestWins.label(), "newest wins");
    }

    #[test]
    fn sync_model_conflict_resolution_is_automatic() {
        assert!(ConflictResolution::KeepLocal.is_automatic());
        assert!(ConflictResolution::KeepRemote.is_automatic());
        assert!(ConflictResolution::Merge.is_automatic());
        assert!(ConflictResolution::NewestWins.is_automatic());
        assert!(!ConflictResolution::AskUser.is_automatic());
        assert!(!ConflictResolution::Manual.is_automatic());
    }

    #[test]
    fn sync_model_conflict_resolution_requires_user_input() {
        assert!(ConflictResolution::AskUser.requires_user_input());
        assert!(ConflictResolution::Manual.requires_user_input());
        assert!(!ConflictResolution::KeepLocal.requires_user_input());
        assert!(!ConflictResolution::KeepRemote.requires_user_input());
        assert!(!ConflictResolution::Merge.requires_user_input());
        assert!(!ConflictResolution::NewestWins.requires_user_input());
    }

    #[test]
    fn sync_model_conflict_resolution_serialization() {
        let cases = vec![
            (ConflictResolution::KeepLocal, "\"keep_local\""),
            (ConflictResolution::KeepRemote, "\"keep_remote\""),
            (ConflictResolution::Merge, "\"merge\""),
            (ConflictResolution::AskUser, "\"ask_user\""),
            (ConflictResolution::Manual, "\"manual\""),
            (ConflictResolution::NewestWins, "\"newest_wins\""),
        ];

        for (strategy, expected) in cases {
            let json = serde_json::to_string(&strategy).unwrap();
            assert_eq!(json, expected);

            let back: ConflictResolution = serde_json::from_str(&json).unwrap();
            assert_eq!(back, strategy);
        }
    }

    #[test]
    fn sync_model_conflict_entry_new_and_defaults() {
        let entry = ConflictEntry::new("note.md", "folder-abc");
        assert_eq!(entry.path, "note.md");
        assert_eq!(entry.folder_id, "folder-abc");
        assert_eq!(entry.strategy, ConflictResolution::AskUser);
        assert!(entry.detected_at.is_some());
        assert!(entry.description.is_none());
        assert!(entry.resolved_strategy.is_none());
        assert!(entry.resolved_at.is_none());
        assert!(!entry.is_resolved());
    }

    #[test]
    fn sync_model_conflict_entry_builder_methods() {
        let now = Utc::now();
        let entry = ConflictEntry::new("doc.md", "f1")
            .with_description("both sides modified")
            .with_strategy(ConflictResolution::NewestWins)
            .resolved_at(ConflictResolution::NewestWins, now);

        assert_eq!(entry.description.as_deref(), Some("both sides modified"));
        assert_eq!(entry.strategy, ConflictResolution::NewestWins);
        assert!(entry.is_resolved());
        assert_eq!(entry.resolved_at, Some(now));
    }

    #[test]
    fn sync_model_conflict_entry_default() {
        let entry = ConflictEntry::default();
        assert!(entry.path.is_empty());
        assert!(entry.folder_id.is_empty());
        assert_eq!(entry.strategy, ConflictResolution::default());
        assert!(!entry.is_resolved());
    }

    #[test]
    fn sync_model_conflict_entry_round_trip() {
        let entry = ConflictEntry::new("path/to/file.md", "folder-123")
            .with_description("conflict on line 5")
            .with_strategy(ConflictResolution::Merge);

        let json = serde_json::to_string(&entry).unwrap();
        let back: ConflictEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry.path, back.path);
        assert_eq!(entry.folder_id, back.folder_id);
        assert_eq!(entry.strategy, back.strategy);
        assert_eq!(entry.description, back.description);
    }

    #[test]
    fn sync_model_conflict_entry_forward_compatible() {
        // Simulate a future version with extra fields.
        let json = r#"{
            "path": "file.txt",
            "folder_id": "f1",
            "strategy": "keep_local",
            "detected_at": null,
            "description": "desc",
            "resolved_strategy": null,
            "resolved_at": null,
            "future_checksum": "abc123",
            "future_author": "user@example.com"
        }"#;
        let entry: ConflictEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.path, "file.txt");
        assert_eq!(entry.folder_id, "f1");
        assert_eq!(entry.strategy, ConflictResolution::KeepLocal);
    }

    #[test]
    fn sync_model_conflict_entry_empty_deserializes() {
        let entry: ConflictEntry = serde_json::from_str("{}").unwrap();
        assert_eq!(entry, ConflictEntry::default());
    }
}
