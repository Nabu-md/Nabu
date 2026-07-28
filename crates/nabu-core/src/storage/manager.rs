/// Storage Manager - the permanent persistence layer for knowledge objects.
///
/// The Storage Manager is responsible for:
/// - Initializing storage
/// - Opening the database
/// - Delegating persistence to the configured provider
/// - Exposing typed storage operations
/// - Subscribing to [`ItemProcessed`] events to persist knowledge objects
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
use std::sync::Arc;

use super::provider::StorageProvider;
use super::sqlite::SQLiteStorage;
use crate::event_bus::{
    EVENT_ITEM_PROCESSED, EVENT_ITEM_STORED, EventBus, ItemProcessed, ItemStored,
};
use crate::models::knowledge_object::KnowledgeObject;

/// The Storage Manager is the single source of truth for structured object metadata.
///
/// It manages the storage backend and provides a clean API for persistence operations.
/// Currently uses SQLite as the only implementation, but the StorageProvider trait
/// allows for future backends (PostgreSQL, Cloud Sync, in-memory testing store).
///
/// The manager subscribes to [`ItemProcessed`] events on the provided event bus
/// and automatically persists knowledge objects when they are processed.
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
    provider: SQLiteStorage,
    event_bus: Arc<EventBus>,
}

impl StorageManager {
    /// Create a new StorageManager for the given vault path.
    ///
    /// The metadata database will be located at `{vault_path}/.nabu/db/metadata.db`.
    ///
    /// The manager automatically subscribes to [`ItemProcessed`] events on the
    /// provided event bus and persists knowledge objects as they are processed.
    pub fn new(vault_path: PathBuf, event_bus: Arc<EventBus>) -> Arc<Self> {
        let provider = SQLiteStorage::new(vault_path);
        let manager = Arc::new(Self {
            provider,
            event_bus: event_bus.clone(),
        });

        let mgr = manager.clone();
        event_bus.subscribe(EVENT_ITEM_PROCESSED, move |event: &ItemProcessed| {
            let save_result = mgr.save_object(&event.knowledge_object);
            if let Err(e) = save_result {
                // Log the error but do not panic; storage failures should not
                // crash other subscribers. In a production system, this would
                // be reported to an error tracking service.
                eprintln!("StorageManager failed to save object {}: {}", event.id, e);
                return;
            }
            let stored_event = ItemStored::from(&event.knowledge_object);
            mgr.event_bus.publish(EVENT_ITEM_STORED, &stored_event);
        });

        manager
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

    /// Delete a knowledge object by its UUID.
    ///
    /// # Errors
    ///
    /// Returns an error if the storage backend is not initialized or the delete fails.
    pub fn delete_object(&self, id: &str) -> Result<()> {
        self.provider.delete_object(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::EventBus;
    use crate::models::knowledge_object::{
        KnowledgeObject, ObjectContent, ObjectMetadata, ObjectType,
    };
    use std::collections::HashMap;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn create_test_object(id: Uuid) -> KnowledgeObject {
        KnowledgeObject {
            id,
            object_type: ObjectType::Note,
            vault_id: "test-vault".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            modified_at: "2024-06-01T00:00:00Z".to_string(),
            content: ObjectContent::Markdown,
            metadata: ObjectMetadata {
                title: Some("Test Note".to_string()),
                author: Some("Test Author".to_string()),
                language: Some("en".to_string()),
                source_url: None,
                source_file: Some("/path/to/note.md".to_string()),
                mime_type: Some("text/markdown".to_string()),
                page_count: None,
                word_count: Some(100),
                created: None,
                modified: None,
                custom: HashMap::new(),
            },
        }
    }

    fn create_test_object_with_custom(
        id: Uuid,
        custom: HashMap<String, serde_json::Value>,
    ) -> KnowledgeObject {
        KnowledgeObject {
            id,
            object_type: ObjectType::Document,
            vault_id: "test-vault".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            modified_at: "2024-06-01T00:00:00Z".to_string(),
            content: ObjectContent::PlainText,
            metadata: ObjectMetadata {
                title: Some("Test Document".to_string()),
                author: None,
                language: None,
                source_url: Some("https://example.com".to_string()),
                source_file: None,
                mime_type: Some("text/html".to_string()),
                page_count: Some(10),
                word_count: Some(500),
                created: Some("2024-01-01T00:00:00Z".to_string()),
                modified: Some("2024-05-15T00:00:00Z".to_string()),
                custom,
            },
        }
    }

    // ========================================================================
    // Database Initialization Tests
    // ========================================================================

    #[test]
    fn storage_manager_initializes_new_database() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let vault_path = temp_dir.path().to_path_buf();
        let bus = Arc::new(EventBus::new());

        let manager = StorageManager::new(vault_path.clone(), bus);
        manager.initialize().expect("Failed to initialize storage");

        assert!(manager.is_initialized());
        let db_path = manager.db_path();
        assert!(db_path.ends_with("metadata.db"));
        assert!(db_path.exists());
    }

    #[test]
    fn storage_manager_does_not_overwrite_existing_database() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let vault_path = temp_dir.path().to_path_buf();
        let bus = Arc::new(EventBus::new());

        // First initialization
        let manager1 = StorageManager::new(vault_path.clone(), bus.clone());
        manager1.initialize().expect("Failed to initialize storage");

        // Get the database path
        let db_path = manager1.db_path().clone();

        // Second initialization should succeed without error
        let manager2 = StorageManager::new(vault_path.clone(), bus);
        manager2
            .initialize()
            .expect("Failed to re-initialize storage");

        // Database should still exist
        assert!(db_path.exists());
    }

    #[test]
    fn storage_manager_creates_nabu_db_directory() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let vault_path = temp_dir.path().to_path_buf();
        let bus = Arc::new(EventBus::new());

        let manager = StorageManager::new(vault_path.clone(), bus);
        manager.initialize().expect("Failed to initialize storage");

        let nabu_db_path = vault_path.join(".nabu").join("db");
        assert!(nabu_db_path.exists());
        assert!(nabu_db_path.is_dir());
    }

    // ========================================================================
    // Save/Retrieve Tests
    // ========================================================================

    #[test]
    fn save_object_persists_knowledge_object() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let vault_path = temp_dir.path().to_path_buf();
        let bus = Arc::new(EventBus::new());

        let manager = StorageManager::new(vault_path, bus);
        manager.initialize().expect("Failed to initialize storage");

        let object = create_test_object(Uuid::new_v4());
        manager.save_object(&object).expect("Failed to save object");

        // Verify the object can be retrieved
        let retrieved = manager
            .get_object(&object.id.to_string())
            .expect("Failed to get object");
        assert!(retrieved.is_some());
    }

    #[test]
    fn get_object_returns_none_for_nonexistent_id() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let vault_path = temp_dir.path().to_path_buf();
        let bus = Arc::new(EventBus::new());

        let manager = StorageManager::new(vault_path, bus);
        manager.initialize().expect("Failed to initialize storage");

        let retrieved = manager
            .get_object("nonexistent-id")
            .expect("Failed to get object");
        assert!(retrieved.is_none());
    }

    #[test]
    fn save_and_get_object_round_trip_preserves_metadata() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let vault_path = temp_dir.path().to_path_buf();
        let bus = Arc::new(EventBus::new());

        let manager = StorageManager::new(vault_path, bus);
        manager.initialize().expect("Failed to initialize storage");

        let original = create_test_object(Uuid::new_v4());
        manager
            .save_object(&original)
            .expect("Failed to save object");

        let retrieved = manager
            .get_object(&original.id.to_string())
            .expect("Failed to get object");
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(original.id, retrieved.id);
        assert_eq!(original.vault_id, retrieved.vault_id);
        assert_eq!(original.object_type, retrieved.object_type);
        assert_eq!(original.created_at, retrieved.created_at);
        assert_eq!(original.modified_at, retrieved.modified_at);
        assert_eq!(original.metadata.title, retrieved.metadata.title);
        assert_eq!(original.metadata.author, retrieved.metadata.author);
        assert_eq!(original.metadata.language, retrieved.metadata.language);
        assert_eq!(
            original.metadata.source_file,
            retrieved.metadata.source_file
        );
        assert_eq!(original.metadata.mime_type, retrieved.metadata.mime_type);
        assert_eq!(original.metadata.word_count, retrieved.metadata.word_count);
    }

    // ========================================================================
    // Update Tests
    // ========================================================================

    #[test]
    fn update_object_replaces_existing_object() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let vault_path = temp_dir.path().to_path_buf();
        let bus = Arc::new(EventBus::new());

        let manager = StorageManager::new(vault_path, bus);
        manager.initialize().expect("Failed to initialize storage");

        let id = Uuid::new_v4();
        let original = create_test_object(id);
        manager
            .save_object(&original)
            .expect("Failed to save object");

        // Update the object
        let mut updated = create_test_object(id);
        updated.metadata.title = Some("Updated Title".to_string());
        updated.modified_at = "2024-07-01T00:00:00Z".to_string();

        manager
            .update_object(&updated)
            .expect("Failed to update object");

        // Verify the update
        let retrieved = manager
            .get_object(&id.to_string())
            .expect("Failed to get object");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.metadata.title, Some("Updated Title".to_string()));
        assert_eq!(retrieved.modified_at, "2024-07-01T00:00:00Z");
    }

    // ========================================================================
    // Custom Metadata Tests
    // ========================================================================

    #[test]
    fn save_object_preserves_custom_metadata() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let vault_path = temp_dir.path().to_path_buf();
        let bus = Arc::new(EventBus::new());

        let manager = StorageManager::new(vault_path, bus);
        manager.initialize().expect("Failed to initialize storage");

        let mut custom = HashMap::new();
        custom.insert("ocr_confidence".to_string(), serde_json::json!(0.95));
        custom.insert("entities".to_string(), serde_json::json!(["Nabu", "Rust"]));

        let id = Uuid::new_v4();
        let object = create_test_object_with_custom(id, custom);
        manager.save_object(&object).expect("Failed to save object");

        let retrieved = manager
            .get_object(&id.to_string())
            .expect("Failed to get object");
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(
            retrieved.metadata.custom.get("ocr_confidence"),
            Some(&serde_json::json!(0.95))
        );
        assert_eq!(
            retrieved.metadata.custom.get("entities"),
            Some(&serde_json::json!(["Nabu", "Rust"]))
        );
    }

    // ========================================================================
    // Object Type Tests
    // ========================================================================

    #[test]
    fn save_object_preserves_all_object_types() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let vault_path = temp_dir.path().to_path_buf();
        let bus = Arc::new(EventBus::new());

        let manager = StorageManager::new(vault_path, bus);
        manager.initialize().expect("Failed to initialize storage");

        // Test various object types
        let object_types = vec![
            ObjectType::Note,
            ObjectType::Document,
            ObjectType::Pdf,
            ObjectType::Image,
            ObjectType::Person,
            ObjectType::Project,
            ObjectType::Custom("plugin_type".to_string()),
        ];

        for object_type in object_types {
            let id = Uuid::new_v4();
            let object = KnowledgeObject {
                id,
                object_type,
                vault_id: "test-vault".to_string(),
                created_at: "2024-01-01T00:00:00Z".to_string(),
                modified_at: "2024-06-01T00:00:00Z".to_string(),
                content: ObjectContent::PlainText,
                metadata: ObjectMetadata {
                    title: Some("Test".to_string()),
                    author: None,
                    language: None,
                    source_url: None,
                    source_file: None,
                    mime_type: None,
                    page_count: None,
                    word_count: None,
                    created: None,
                    modified: None,
                    custom: HashMap::new(),
                },
            };

            manager.save_object(&object).expect("Failed to save object");

            let retrieved = manager
                .get_object(&id.to_string())
                .expect("Failed to get object");
            assert!(retrieved.is_some());
            let retrieved = retrieved.unwrap();
            assert_eq!(object.object_type, retrieved.object_type);
        }
    }

    // ========================================================================
    // Multiple Objects Tests
    // ========================================================================

    #[test]
    fn save_multiple_objects_and_retrieve_all() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let vault_path = temp_dir.path().to_path_buf();
        let bus = Arc::new(EventBus::new());

        let manager = StorageManager::new(vault_path, bus);
        manager.initialize().expect("Failed to initialize storage");

        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::new_v4()).collect();

        // Save all objects
        for id in &ids {
            let object = create_test_object(*id);
            manager.save_object(&object).expect("Failed to save object");
        }

        // Retrieve all objects
        for id in &ids {
            let retrieved = manager
                .get_object(&id.to_string())
                .expect("Failed to get object");
            assert!(retrieved.is_some(), "Object {} should exist", id);
        }
    }

    // ========================================================================
    // Error Handling Tests
    // ========================================================================

    #[test]
    fn get_object_with_invalid_uuid_format_returns_none() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let vault_path = temp_dir.path().to_path_buf();
        let bus = Arc::new(EventBus::new());

        let manager = StorageManager::new(vault_path, bus);
        manager.initialize().expect("Failed to initialize storage");

        // Invalid UUID format should return None (not found) rather than error
        let result = manager.get_object("not-a-valid-uuid");
        // The query will succeed but return no rows, which is None
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
