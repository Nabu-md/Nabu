/// SQLite storage implementation for the Storage Manager.
///
/// This module provides a SQLite-backed implementation of the StorageProvider
/// trait, managing the metadata database at `.nabu/db/metadata.db`.

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};
use std::path::PathBuf;

use super::schema::{
    CREATE_KNOWLEDGE_OBJECTS_TABLE, CREATE_SCHEMA_VERSION_TABLE, CREATE_VAULTS_TABLE,
    CURRENT_SCHEMA_VERSION, INSERT_SCHEMA_VERSION,
};
use super::provider::StorageProvider;

/// SQLite-backed storage provider.
///
/// Manages the metadata database for knowledge objects and vault metadata.
/// The database is stored at `.nabu/db/metadata.db` within the vault directory.
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

    /// Get the path to the database file.
    pub fn db_path(&self) -> &PathBuf {
        &self.db_path
    }

    /// Get a new database connection.
    ///
    /// This opens a fresh connection to the database.
    pub fn connect(&self) -> Result<Connection> {
        Connection::open(&self.db_path)
            .context(format!("Failed to open database at: {:?}", self.db_path))
    }

    /// Initialize the database schema.
    ///
    /// This method creates the necessary tables if they do not exist.
    /// It should only be called after the database file is created.
    fn init_schema(conn: &Connection) -> Result<()> {
        conn.execute(CREATE_KNOWLEDGE_OBJECTS_TABLE, [])
            .context("Failed to create knowledge_objects table")?;

        conn.execute(CREATE_VAULTS_TABLE, [])
            .context("Failed to create vaults table")?;

        conn.execute(CREATE_SCHEMA_VERSION_TABLE, [])
            .context("Failed to create schema_version table")?;

        conn.execute(INSERT_SCHEMA_VERSION, rusqlite::params![CURRENT_SCHEMA_VERSION, chrono::Utc::now().to_rfc3339()])
            .context("Failed to insert schema version")?;

        Ok(())
    }
}

impl StorageProvider for SQLiteStorage {
    /// Initialize the storage backend.
    ///
    /// Creates the `.nabu/db` directory if it does not exist and
    /// initializes the database schema. Never overwrites an existing database.
    fn initialize(&self) -> Result<()> {
        // Check if database already exists
        if self.db_path.exists() {
            // Database exists, just open and verify connection
            let conn = self.connect()?;
            
            // Verify schema version exists
            let version: Option<i64> = conn.query_row(
                "SELECT version FROM schema_version WHERE version = 1",
                [],
                |row| row.get(0),
            ).optional().context("Failed to query schema version")?;

            if version.is_none() {
                anyhow::bail!("Database exists but schema version is not 1");
            }

            // Database is valid, return success
            return Ok(());
        }

        // Create the .nabu/db directory
        let db_dir = self.vault_path.join(".nabu").join("db");
        std::fs::create_dir_all(&db_dir)
            .context(format!("Failed to create database directory: {:?}", db_dir))?;

        // Create and initialize the database
        let conn = self.connect()?;

        Self::init_schema(&conn)
            .context("Failed to initialize database schema")?;

        Ok(())
    }

    /// Check if the storage backend is initialized.
    fn is_initialized(&self) -> bool {
        self.db_path.exists()
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