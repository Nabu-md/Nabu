//! # Universal Undo/Redo — Command History
//!
//! A centralized, command-based history system that makes reversible user
//! actions predictable across the entire application: editor operations,
//! filesystem operations, metadata changes and workspace operations.
//!
//! ## Design
//!
//! Every reversible action is recorded as a [`HistoryEntry`] carrying:
//!
//! - a unique identifier and timestamp
//! - an operation type ([`HistoryOp`])
//! - the affected object identifiers / paths
//! - the previous and new state (JSON snapshots)
//! - the undo and redo actions (closures)
//!
//! Entries are pushed onto a [`HistoryManager`], which owns an undo stack and
//! a redo stack. Pushing a new entry invalidates the redo stack (linear
//! history); undoing pops from the undo stack and pushes onto the redo stack;
//! redoing does the reverse. The manager supports a configurable maximum
//! depth with oldest-entry pruning.
//!
//! ## Registration
//!
//! The manager is registered as a singleton service under the
//! `history_manager` key and resolved through the [`ApplicationContext`]
//! (see `registry::context`). Backend commands push entries after performing
//! an operation; the frontend drives `undo` / `redo` over IPC.
//!
//! [`ApplicationContext`]: crate::registry::context::ApplicationContext

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// The type of operation a history entry represents.
///
/// Used for labels, filtering and future persistence/grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryOp {
    /// A note was created.
    NoteCreate,
    /// A note was renamed.
    NoteRename,
    /// A note was moved between folders.
    NoteMove,
    /// A note was duplicated.
    NoteDuplicate,
    /// A note was deleted (moved to trash).
    NoteDelete,
    /// A note was restored from trash.
    NoteRestore,
    /// A folder was created.
    FolderCreate,
    /// A folder was renamed.
    FolderRename,
    /// A folder was moved.
    FolderMove,
    /// A folder was deleted.
    FolderDelete,
    /// Metadata (properties, tags, aliases, links, templates) changed.
    Metadata,
    /// An editor / document operation.
    Editor,
    /// A workspace operation (tabs, panes, sidebar, view settings).
    Workspace,
    /// Any other reversible action.
    Other,
}

impl HistoryOp {
    /// Human-readable default label for the operation.
    pub fn label(self) -> &'static str {
        match self {
            HistoryOp::NoteCreate => "Create Note",
            HistoryOp::NoteRename => "Rename Note",
            HistoryOp::NoteMove => "Move Note",
            HistoryOp::NoteDuplicate => "Duplicate Note",
            HistoryOp::NoteDelete => "Delete Note",
            HistoryOp::NoteRestore => "Restore Note",
            HistoryOp::FolderCreate => "Create Folder",
            HistoryOp::FolderRename => "Rename Folder",
            HistoryOp::FolderMove => "Move Folder",
            HistoryOp::FolderDelete => "Delete Folder",
            HistoryOp::Metadata => "Metadata",
            HistoryOp::Editor => "Edit",
            HistoryOp::Workspace => "Workspace",
            HistoryOp::Other => "Action",
        }
    }
}

/// The undo / redo action of a history entry.
///
/// A closure that performs the operation (redo) or reverses it (undo).
/// Returning `Err` signals the action failed; the manager keeps the entry
/// on the stack so it can be retried rather than silently lost.
pub type HistoryAction = Arc<dyn Fn() -> Result<(), String> + Send + Sync>;

/// A single reversible operation in the history.
#[derive(Clone)]
pub struct HistoryEntry {
    /// Unique identifier of this history entry.
    pub id: Uuid,
    /// When the operation was performed.
    pub timestamp: DateTime<Utc>,
    /// The kind of operation.
    pub op: HistoryOp,
    /// Human-readable description (e.g. "Rename Note to 'Q2.md'").
    pub label: String,
    /// Affected object identifiers / paths.
    pub affected: Vec<String>,
    /// JSON snapshot of the state before the operation.
    pub previous_state: serde_json::Value,
    /// JSON snapshot of the state after the operation.
    pub new_state: serde_json::Value,
    /// Reverses the operation.
    undo: HistoryAction,
    /// Re-applies the operation.
    redo: HistoryAction,
}

impl HistoryEntry {
    /// Creates a new history entry.
    pub fn new(
        op: HistoryOp,
        label: impl Into<String>,
        affected: Vec<String>,
        previous_state: serde_json::Value,
        new_state: serde_json::Value,
        undo: HistoryAction,
        redo: HistoryAction,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            op,
            label: label.into(),
            affected,
            previous_state,
            new_state,
            undo,
            redo,
        }
    }

    /// Runs the undo action.
    pub fn run_undo(&self) -> Result<(), String> {
        (self.undo)()
    }

    /// Runs the redo action.
    pub fn run_redo(&self) -> Result<(), String> {
        (self.redo)()
    }
}

/// The centralized command history.
///
/// Owns an undo stack and a redo stack with a configurable maximum depth.
/// New pushes clear the redo stack (linear history); the oldest entries are
/// pruned when the undo stack exceeds the configured depth.
pub struct HistoryManager {
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
    max_depth: usize,
}

impl Default for HistoryManager {
    /// The default manager uses the standard history depth (100 entries).
    fn default() -> Self {
        Self::with_depth(100)
    }
}

impl HistoryManager {
    /// Creates a manager with the default history depth (100 entries).
    pub fn new() -> Self {
        Self::with_depth(100)
    }

    /// Creates a manager with a custom maximum history depth.
    pub fn with_depth(max_depth: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_depth: max_depth.max(1),
        }
    }

    /// Records a reversible operation, invalidating the redo stack.
    pub fn push(&mut self, entry: HistoryEntry) {
        self.redo_stack.clear();
        self.undo_stack.push(entry);
        self.prune();
    }

    /// Reverses the most recent operation.
    ///
    /// Returns `Ok(Some(label))` when an entry was undone, `Ok(None)` when
    /// there is nothing to undo, and `Err` if the undo action failed (the
    /// entry is restored to the undo stack so it can be retried).
    pub fn undo(&mut self) -> Result<Option<String>, String> {
        let Some(entry) = self.undo_stack.pop() else {
            return Ok(None);
        };
        if let Err(e) = entry.run_undo() {
            self.undo_stack.push(entry);
            return Err(e);
        }
        let label = entry.label.clone();
        self.redo_stack.push(entry);
        Ok(Some(label))
    }

    /// Re-applies the most recently undone operation.
    ///
    /// Returns `Ok(Some(label))` when an entry was redone, `Ok(None)` when
    /// there is nothing to redo, and `Err` if the redo action failed (the
    /// entry is restored to the redo stack so it can be retried).
    pub fn redo(&mut self) -> Result<Option<String>, String> {
        let Some(entry) = self.redo_stack.pop() else {
            return Ok(None);
        };
        if let Err(e) = entry.run_redo() {
            self.redo_stack.push(entry);
            return Err(e);
        }
        let label = entry.label.clone();
        self.undo_stack.push(entry);
        Ok(Some(label))
    }

    /// Returns `true` when there is something to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Returns `true` when there is something to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Label of the next entry that would be undone.
    pub fn undo_label(&self) -> Option<String> {
        self.undo_stack.last().map(|e| e.label.clone())
    }

    /// Label of the next entry that would be redone.
    pub fn redo_label(&self) -> Option<String> {
        self.redo_stack.last().map(|e| e.label.clone())
    }

    /// Number of entries that can be undone.
    pub fn undo_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Number of entries that can be redone.
    pub fn redo_len(&self) -> usize {
        self.redo_stack.len()
    }

    /// Total number of entries in both stacks.
    pub fn len(&self) -> usize {
        self.undo_stack.len() + self.redo_stack.len()
    }

    /// Returns `true` when both stacks are empty.
    pub fn is_empty(&self) -> bool {
        self.undo_stack.is_empty() && self.redo_stack.is_empty()
    }

    /// The configured maximum history depth.
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Updates the maximum history depth, pruning if necessary.
    pub fn set_max_depth(&mut self, depth: usize) {
        self.max_depth = depth.max(1);
        self.prune();
    }

    /// Clears both stacks (history invalidation, e.g. vault switch).
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Trims the undo stack down to the configured depth, removing the
    /// oldest entries first.
    fn prune(&mut self) {
        while self.undo_stack.len() > self.max_depth {
            self.undo_stack.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop_entry(op: HistoryOp, label: &str) -> HistoryEntry {
        HistoryEntry::new(
            op,
            label,
            vec![],
            serde_json::json!({}),
            serde_json::json!({}),
            Arc::new(|| Ok(())),
            Arc::new(|| Ok(())),
        )
    }

    #[test]
    fn empty_history_has_nothing_to_undo() {
        let mut manager = HistoryManager::new();
        assert!(!manager.can_undo());
        assert!(!manager.can_redo());
        assert_eq!(manager.undo().unwrap(), None);
        assert_eq!(manager.redo().unwrap(), None);
        assert!(manager.is_empty());
    }

    #[test]
    fn push_undo_redo_round_trip() {
        let mut manager = HistoryManager::new();
        manager.push(noop_entry(HistoryOp::NoteCreate, "Create Note"));

        assert!(manager.can_undo());
        assert_eq!(manager.undo_label().as_deref(), Some("Create Note"));
        assert_eq!(manager.undo().unwrap(), Some("Create Note".to_string()));
        assert!(!manager.can_undo());
        assert!(manager.can_redo());

        assert_eq!(manager.redo_label().as_deref(), Some("Create Note"));
        assert_eq!(manager.redo().unwrap(), Some("Create Note".to_string()));
        assert!(manager.can_undo());
        assert!(!manager.can_redo());
    }

    #[test]
    fn push_invalidates_redo_stack() {
        let mut manager = HistoryManager::new();
        manager.push(noop_entry(HistoryOp::NoteRename, "Rename Note"));
        manager.undo().unwrap();
        assert!(manager.can_redo());

        // A new operation clears the redo stack (linear history).
        manager.push(noop_entry(HistoryOp::Metadata, "Edit Metadata"));
        assert!(!manager.can_redo());
        assert_eq!(manager.undo_len(), 1);
    }

    #[test]
    fn depth_prunes_oldest_entries() {
        let mut manager = HistoryManager::with_depth(3);
        for i in 0..6 {
            manager.push(noop_entry(HistoryOp::NoteCreate, &format!("Create {}", i)));
        }
        assert_eq!(manager.undo_len(), 3);
        // The three most recent remain; the three oldest were pruned.
        assert_eq!(manager.undo_label().as_deref(), Some("Create 5"));
        manager.undo().unwrap();
        manager.undo().unwrap();
        assert_eq!(manager.undo_label().as_deref(), Some("Create 3"));
    }

    #[test]
    fn set_max_depth_prunes() {
        let mut manager = HistoryManager::new();
        for i in 0..10 {
            manager.push(noop_entry(HistoryOp::NoteCreate, &format!("Create {}", i)));
        }
        assert_eq!(manager.undo_len(), 10);
        manager.set_max_depth(2);
        assert_eq!(manager.undo_len(), 2);
    }

    #[test]
    fn clear_invalidates_history() {
        let mut manager = HistoryManager::new();
        manager.push(noop_entry(HistoryOp::NoteCreate, "A"));
        manager.undo().unwrap();
        assert!(manager.can_redo());
        manager.clear();
        assert!(manager.is_empty());
        assert!(!manager.can_undo());
        assert!(!manager.can_redo());
    }

    #[test]
    fn undo_actions_actually_run() {
        let counter = std::sync::Arc::new(std::sync::Mutex::new(0));
        let undo_counter = counter.clone();
        let redo_counter = counter.clone();

        let entry = HistoryEntry::new(
            HistoryOp::Editor,
            "Type",
            vec![],
            serde_json::json!({"value": 0}),
            serde_json::json!({"value": 1}),
            Arc::new(move || {
                *undo_counter.lock().unwrap() -= 1;
                Ok(())
            }),
            Arc::new(move || {
                *redo_counter.lock().unwrap() += 1;
                Ok(())
            }),
        );

        let mut manager = HistoryManager::new();
        manager.push(entry);
        manager.undo().unwrap();
        assert_eq!(*counter.lock().unwrap(), -1);
        manager.redo().unwrap();
        assert_eq!(*counter.lock().unwrap(), 0);
    }

    #[test]
    fn failed_undo_restores_entry() {
        let mut manager = HistoryManager::new();
        let entry = HistoryEntry::new(
            HistoryOp::Other,
            "Boom",
            vec![],
            serde_json::json!({}),
            serde_json::json!({}),
            Arc::new(|| Err("cannot undo".to_string())),
            Arc::new(|| Ok(())),
        );
        manager.push(entry);
        assert!(manager.undo().is_err());
        // The failed entry is restored so it can be retried.
        assert!(manager.can_undo());
        assert!(!manager.can_redo());
    }

    #[test]
    fn entry_metadata_is_recorded() {
        let entry = HistoryEntry::new(
            HistoryOp::NoteRename,
            "Rename Note",
            vec!["path/to/note.md".to_string()],
            serde_json::json!({"name": "a.md"}),
            serde_json::json!({"name": "b.md"}),
            Arc::new(|| Ok(())),
            Arc::new(|| Ok(())),
        );
        assert_eq!(entry.op, HistoryOp::NoteRename);
        assert_eq!(entry.affected, vec!["path/to/note.md"]);
        assert_eq!(entry.previous_state["name"], "a.md");
        assert_eq!(entry.new_state["name"], "b.md");
        assert_eq!(HistoryOp::NoteRename.label(), "Rename Note");
    }
}
