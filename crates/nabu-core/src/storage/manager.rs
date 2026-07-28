/// Storage Manager - the permanent persistence layer for knowledge objects.
///
/// The Storage Manager is responsible for:
/// - Initializing storage
/// - Opening the database
/// - Delegating persistence to the configured provider
/// - Exposing typed storage operations
///
/// # Architectural Responsibilities
///
/// The Storage Manager serves as the single source of truth for structured
/// object metadata. It manages the storage backend and provides a clean API
/// for persistence operations.
///
/// # Thread Safety
///
/// The Storage Manager is thread-safe and can be used from multiple threads
/// concurrently. Each operation opens a fresh database connection.
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
///
/// # Example
///
/// ```ignore
/// use std::path::PathBuf;
/// use nabu_core::storage::StorageManager;
///
/// let manager = StorageManager::new(PathBuf::from("/path/to/vault"));
/// manager.initialize()?;
/// // ... use manager to save/retrieve objects
/// ```
pub struct StorageManager {
    /// The storage provider implementation.
    provider: SQLiteStorage,
}

impl StorageManager {
    /// Create a new StorageManager for the given vault path.
    ///
    /// The metadata database will be located at `{vault_path}/.nabu/db/metadata.db`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let manager = StorageManager::new(PathBuf::from("/path/to/vault"));
    /// ```
    pub fn new(vault_path: PathBuf) -> Self {
        let provider = SQLiteStorage::new(vault_path);
        Self { provider }
    }

    /// Initialize the storage backend.
    ///
    /// Creates the database and schema if they do not exist.
    /// Never overwrites an existing database.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The database file exists but has an incompatible schema
    /// - The directory cannot be created due to permissions
    /// - The database cannot be initialized
    pub fn initialize(&self) -> Result<()> {
        self.provider.initialize()
    }

    /// Check if the storage backend is initialized.
    pub fn is_initialized(&self) -> bool {
        self.provider.is_initialized()
    }

    /// Get the path to the metadata database.
    pub fn db_path(&self) -> &PathBuf {
        self.provider.db_path()
    }

    /// Save a knowledge object to the database.
    ///
    /// Uses INSERT OR REPLACE to handle duplicate IDs safely.
    /// Serializes the object to JSON for storage.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The storage backend is not initialized
    /// - Serialization of the object fails
    /// - The database write operation fails
    pub fn save_object(&self, object: &KnowledgeObject) -> Result<()> {
        self.provider.save_object(object)
    }

    /// Retrieve a knowledge object by its UUID.
    ///
    /// Returns `None` if the object does not exist.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The storage backend is not initialized
    /// - Deserialization of the object fails
    /// - The database read operation fails
    pub fn get_object(&self, id: &str) -> Result<Option<KnowledgeObject>> {
        self.provider.get_object(id)
    }

    /// Update an existing knowledge object.
    ///
    /// The object identity (id) is preserved.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The storage backend is not initialized
    /// - Serialization of the object fails
    /// - The database write operation fails
    pub fn update_object(&self, object: &KnowledgeObject) -> Result<()> {
        self.provider.update_object(object)
    }
}
