/// SQLite storage implementation for the Storage Manager.
///
/// This module provides a SQLite-backed implementation of the StorageProvider
/// trait, managing the metadata database at `.nabu/db/metadata.db`.
///
/// # Design Decisions
///
/// - **Thread Safety**: Each method opens a fresh connection to avoid `Sync` issues
///   with `rusqlite::Connection`. This is a valid approach for SQLite.
/// - **Transactions**: All write operations use implicit transactions. Future phases
///   may add explicit transaction support for batch operations.
/// - **JSON Storage**: `ObjectType` and `custom_metadata` are stored as JSON strings
///   to allow schema evolution without migrations.
/// - **Metadata Only**: Only `ObjectMetadata` is persisted; `ObjectContent` is not
///   stored in the metadata database (content is stored separately in the vault).
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension};
use std::path::PathBuf;
use uuid::Uuid;

use super::provider::StorageProvider;
use super::schema::{
    CREATE_KNOWLEDGE_OBJECTS_TABLE, CREATE_SCHEMA_VERSION_TABLE, CREATE_VAULTS_TABLE,
    CURRENT_SCHEMA_VERSION, INSERT_KNOWLEDGE_OBJECT, INSERT_SCHEMA_VERSION,
    SELECT_KNOWLEDGE_OBJECT_BY_ID,
};
use crate::models::knowledge_object::{KnowledgeObject, ObjectContent, ObjectMetadata, ObjectType};

/// SQLite-backed storage provider.
///
/// Manages the metadata database for knowledge objects and vault metadata.
/// The database is stored at `.nabu/db/metadata.db` within the vault directory.
///
/// # Example
///
/// ```ignore
/// use std::path::PathBuf;
/// use nabu_core::storage::SQLiteStorage;
///
/// let storage = SQLiteStorage::new(PathBuf::from("/path/to/vault"));
/// storage.initialize()?;
/// // ... use storage to save/retrieve objects
/// ```
pub struct SQLiteStorage {
    /// Path to the vault directory.
    vault_path: PathBuf,
    /// Path to the database file.
    db_path: PathBuf,
}

impl SQLiteStorage {
    /// Create a new SQLiteStorage instance.
    ///
    /// The database will be located at `{vault_path}/.nabu/db/metadata.db`.
    pub fn new(vault_path: PathBuf) -> Self {
        let db_path = vault_path.join(".nabu").join("db").join("metadata.db");
        Self {
            vault_path,
            db_path,
        }
    }

    /// Get a new database connection.
    ///
    /// This opens a fresh connection to the database.
    fn connect(&self) -> Result<Connection> {
        Connection::open(&self.db_path)
            .map_err(|e| anyhow::anyhow!("Failed to open database at {:?}: {}", self.db_path, e))
    }

    /// Initialize the database schema.
    ///
    /// This method creates the necessary tables if they do not exist.
    /// It should only be called after the database file is created.
    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute(CREATE_KNOWLEDGE_OBJECTS_TABLE, [])
            .map_err(|e| anyhow::anyhow!("Failed to create knowledge_objects table: {}", e))?;
        conn.execute(CREATE_VAULTS_TABLE, [])
            .map_err(|e| anyhow::anyhow!("Failed to create vaults table: {}", e))?;
        conn.execute(CREATE_SCHEMA_VERSION_TABLE, [])
            .map_err(|e| anyhow::anyhow!("Failed to create schema_version table: {}", e))?;
        conn.execute(
            INSERT_SCHEMA_VERSION,
            rusqlite::params![CURRENT_SCHEMA_VERSION, chrono::Utc::now().to_rfc3339()],
        )
        .map_err(|e| anyhow::anyhow!("Failed to insert schema version: {}", e))?;
        Ok(())
    }
}

impl StorageProvider for SQLiteStorage {
    /// Initialize the storage backend.
    ///
    /// Creates the `.nabu/db` directory if it does not exist and
    /// initializes the database schema. Never overwrites an existing database.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The database file exists but has an incompatible schema
    /// - The directory cannot be created due to permissions
    /// - The database cannot be initialized
    fn initialize(&self) -> Result<()> {
        // Check if database already exists
        if self.db_path.exists() {
            // Database exists, just open and verify connection
            let conn = self.connect()?;

            // Verify schema version exists
            let version: Option<i64> = conn
                .query_row(
                    "SELECT version FROM schema_version WHERE version = 1",
                    [],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|e| anyhow::anyhow!("Failed to query schema version: {}", e))?;

            if version.is_none() {
                anyhow::bail!("Database exists but schema version is not 1");
            }

            // Database is valid, return success
            return Ok(());
        }

        // Create the .nabu/db directory
        let db_dir = self.vault_path.join(".nabu").join("db");
        std::fs::create_dir_all(&db_dir).map_err(|e| {
            anyhow::anyhow!("Failed to create database directory {:?}: {}", db_dir, e)
        })?;

        // Create and initialize the database
        let conn = self.connect()?;

        Self::init_schema(&conn)
            .map_err(|e| anyhow::anyhow!("Failed to initialize database schema: {}", e))?;

        Ok(())
    }

    /// Check if the storage backend is initialized.
    fn is_initialized(&self) -> bool {
        self.db_path.exists()
    }

    /// Get the path to the database file.
    fn db_path(&self) -> &PathBuf {
        &self.db_path
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
    fn save_object(&self, object: &KnowledgeObject) -> Result<()> {
        let conn = self.connect()?;

        let custom_json = serde_json::to_string(&object.metadata.custom)
            .map_err(|e| anyhow::anyhow!("Failed to serialize custom metadata: {}", e))?;

        conn.execute(
            INSERT_KNOWLEDGE_OBJECT,
            rusqlite::params![
                object.id.to_string(),
                object.vault_id,
                serde_json::to_string(&object.object_type)?,
                object.created_at,
                object.modified_at,
                object.metadata.title,
                object.metadata.author,
                object.metadata.language,
                object.metadata.source_url,
                object.metadata.source_file,
                object.metadata.mime_type,
                object.metadata.page_count,
                object.metadata.word_count,
                object.metadata.created,
                object.metadata.modified,
                custom_json,
            ],
        )
        .map_err(|e| anyhow::anyhow!("Failed to save knowledge object: {}", e))?;

        Ok(())
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
    fn get_object(&self, id: &str) -> Result<Option<KnowledgeObject>> {
        let conn = self.connect()?;

        let result = conn
            .query_row(SELECT_KNOWLEDGE_OBJECT_BY_ID, [id], |row| {
                let id_str: String = row.get(0)?;
                let id = Uuid::parse_str(&id_str).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                let object_type: String = row.get(2)?;
                let object_type: ObjectType = serde_json::from_str(&object_type).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        2,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;

                let custom_json: String = row.get(15)?;
                let custom: std::collections::HashMap<String, serde_json::Value> =
                    serde_json::from_str(&custom_json).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            15,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;

                Ok(KnowledgeObject {
                    id,
                    vault_id: row.get(1)?,
                    object_type,
                    created_at: row.get(3)?,
                    modified_at: row.get(4)?,
                    content: ObjectContent::PlainText, // Content not stored in metadata DB
                    metadata: ObjectMetadata {
                        title: row.get(5)?,
                        author: row.get(6)?,
                        language: row.get(7)?,
                        source_url: row.get(8)?,
                        source_file: row.get(9)?,
                        mime_type: row.get(10)?,
                        page_count: row.get(11)?,
                        word_count: row.get(12)?,
                        created: row.get(13)?,
                        modified: row.get(14)?,
                        custom,
                    },
                })
            })
            .optional()
            .map_err(|e| anyhow::anyhow!("Failed to query knowledge object: {}", e))?;

        Ok(result)
    }
}

impl Clone for SQLiteStorage {
    fn clone(&self) -> Self {
        Self {
            vault_path: self.vault_path.clone(),
            db_path: self.db_path.clone(),
        }
    }
}
