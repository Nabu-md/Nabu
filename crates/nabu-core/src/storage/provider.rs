/// Storage provider abstraction for the Storage Manager.
///
/// This trait defines the interface for storage backends, allowing future
/// implementations (PostgreSQL, Cloud Sync, in-memory testing store) without
/// changing higher-level code.
///
/// # Architectural Responsibilities
///
/// Each storage provider is responsible for:
/// - Initializing the storage backend (creating directories, database files)
/// - Managing the lifecycle of the storage connection
/// - Persisting and retrieving knowledge object metadata
///
/// The trait is designed to be extended in future phases for:
/// - Search indexing integration
/// - Graph persistence
/// - Metadata enrichment
/// - Object versioning
/// - Cloud synchronization
use anyhow::Result;
use std::path::PathBuf;

use crate::models::knowledge_object::{KnowledgeObject, ObjectContent, ObjectMetadata, ObjectType};
use chrono;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Trait defining the storage provider interface.
///
/// Implementations of this trait provide persistence for knowledge objects
/// and related metadata. The trait is thread-safe (`Send + Sync`) to allow
/// concurrent access from multiple threads.
pub trait StorageProvider: Send + Sync {
    /// Initialize the storage backend.
    ///
    /// Creates necessary directories and database files if they do not exist.
    /// Never overwrites existing data.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The database file exists but has an incompatible schema
    /// - The directory cannot be created due to permissions
    /// - The database cannot be initialized
    fn initialize(&self) -> Result<()>;

    /// Check if the storage backend is initialized and ready.
    fn is_initialized(&self) -> bool;

    /// Get the path to the database file.
    fn db_path(&self) -> &PathBuf;

    /// Save a knowledge object to the storage backend.
    ///
    /// Uses INSERT OR REPLACE semantics to handle duplicate IDs safely.
    /// Serializes the object to JSON for storage.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The storage backend is not initialized
    /// - Serialization of the object fails
    /// - The database write operation fails
    fn save_object(&self, object: &KnowledgeObject) -> Result<()>;

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
    fn get_object(&self, id: &str) -> Result<Option<KnowledgeObject>>;

    /// Update an existing knowledge object.
    ///
    /// The object identity (id) is preserved. This is typically an alias
    /// for `save_object` when using INSERT OR REPLACE semantics.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The storage backend is not initialized
    /// - Serialization of the object fails
    /// - The database write operation fails
    fn update_object(&self, object: &KnowledgeObject) -> Result<()> {
        self.save_object(object)
    }

    /// List knowledge objects in a vault, optionally filtered by source file.
    ///
    /// This is used by processors for duplicate detection. It returns a limited
    /// set of objects to avoid expensive full vault scans.
    ///
    /// # Arguments
    ///
    /// * `vault_id` - The vault to query.
    /// * `source_file` - Optional source file path to filter by.
    /// * `limit` - Maximum number of objects to return.
    ///
    /// # Returns
    ///
    /// A list of knowledge objects matching the criteria.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage backend is not initialized or the query fails.
    fn list_objects(&self, vault_id: &str, source_file: Option<&str>, limit: usize) -> Result<Vec<KnowledgeObject>> {
        // Default implementation uses get_object for each ID.
        // Backends should override with efficient queries.
        let mut results = Vec::new();
        // This is a placeholder; actual implementations should query effectively.
        Ok(results)
    }

    /// Delete a knowledge object by its UUID.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage backend is not initialized or the delete fails.
    fn delete_object(&self, id: &str) -> Result<()> {
        // Default implementation does nothing; backends should override.
        Ok(())
    }

}
