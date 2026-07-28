/// Database schema definitions for the Storage Manager.
///
/// This module contains the SQL schema for the metadata database.
/// The schema is intentionally minimal for Phase 1.3A foundation.

/// SQL statement to create the knowledge_objects table.
/// This table stores metadata for all knowledge objects in the system.
pub const CREATE_KNOWLEDGE_OBJECTS_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS knowledge_objects (
        id TEXT PRIMARY KEY NOT NULL,
        vault_id TEXT NOT NULL,
        object_type TEXT NOT NULL,
        created_at TEXT NOT NULL,
        modified_at TEXT NOT NULL,
        title TEXT,
        author TEXT,
        language TEXT,
        source_url TEXT,
        source_file TEXT,
        mime_type TEXT,
        page_count INTEGER,
        word_count INTEGER,
        source_created TEXT,
        source_modified TEXT,
        custom_metadata TEXT
    )
"#;

/// SQL statement to create the vaults table.
/// This table stores vault metadata.
pub const CREATE_VAULTS_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS vaults (
        id TEXT PRIMARY KEY NOT NULL,
        name TEXT NOT NULL,
        path TEXT NOT NULL,
        created_at TEXT NOT NULL
    )
"#;

/// SQL statement to create the schema version table.
/// This table tracks the database schema version for future migrations.
pub const CREATE_SCHEMA_VERSION_TABLE: &str = r#"
    CREATE TABLE IF NOT EXISTS schema_version (
        version INTEGER PRIMARY KEY,
        applied_at TEXT NOT NULL
    )
"#;

/// The current schema version.
pub const CURRENT_SCHEMA_VERSION: i64 = 1;

/// SQL statement to insert the initial schema version.
pub const INSERT_SCHEMA_VERSION: &str = r#"
    INSERT OR IGNORE INTO schema_version (version, applied_at) VALUES (?, ?)
"#;