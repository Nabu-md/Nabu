/// Storage Manager - the permanent persistence layer for knowledge objects.
///
/// The Storage Manager is responsible for:
/// - Initializing storage
/// - Opening the database
/// - Delegating persistence to the configured provider
/// - Exposing typed storage operations

use anyhow::Result;
use std::path::PathBuf;

use super::provider::StorageProvider;
use super::sqlite::SQLiteStorage;

/// The Storage Manager is the single source of truth for structured object metadata.
///
/// It manages the storage backend and provides a clean API for persistence operations.
/// Currently uses SQLite as the only implementation, but the StorageProvider trait
/// allows for future backends (PostgreSQL, Cloud Sync, in-memory testing store).
pub struct StorageManager {
    /// The SQLite storage instance.
    sqlite: SQLiteStorage,
}

impl StorageManager {
    /// Create a new StorageManager for the given vault path.
    ///
    /// The metadata database will be located at `{vault_path}/.nabu/db/metadata.db`.
    pub fn new(vault_path: PathBuf) -> Self {
        let sqlite = SQLiteStorage::new(vault_path);
        Self { sqlite }
    }

    /// Initialize the storage backend.
    ///
    /// Creates the database and schema if they do not exist.
    /// Never overwrites an existing database.
    pub fn initialize(&self) -> Result<()> {
        self.sqlite.initialize()
    }

    /// Check if the storage backend is initialized.
    pub fn is_initialized(&self) -> bool {
        self.sqlite.is_initialized()
    }

    /// Get the path to the metadata database.
    pub fn db_path(&self) -> &PathBuf {
        self.sqlite.db_path()
    }
}