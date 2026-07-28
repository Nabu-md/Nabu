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
use crate::models::knowledge_object::KnowledgeObject;

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

    /// Save a knowledge object to the database.
    ///
    /// Uses INSERT OR REPLACE to handle duplicate IDs safely.
    /// Serializes the object to JSON for storage.
    pub fn save_object(&self, object: &KnowledgeObject) -> Result<()> {
        self.sqlite.save_object(object)
    }

    /// Retrieve a knowledge object by its UUID.
    ///
    /// Returns None if the object does not exist.
    pub fn get_object(&self, id: &str) -> Result<Option<KnowledgeObject>> {
        self.sqlite.get_object(id)
    }

    /// Update an existing knowledge object.
    ///
    /// The object identity (id) is preserved.
    pub fn update_object(&self, object: &KnowledgeObject) -> Result<()> {
        self.sqlite.update_object(object)
    }
}