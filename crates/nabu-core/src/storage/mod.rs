/// Storage module for the Storage Manager.
///
/// This module provides the persistence layer for knowledge objects,
/// including the StorageManager, StorageProvider trait, and SQLite implementation.
///
/// # Architecture
///
/// The storage layer is designed with the following principles:
///
/// - **Abstraction**: `StorageProvider` trait allows future backends
/// - **Thread Safety**: Each operation opens a fresh connection
/// - **Schema Evolution**: JSON storage for flexible fields
/// - **No Overwrites**: Existing databases are never modified on initialization
///
/// # Database Location
///
/// The metadata database is stored at `.nabu/db/metadata.db` within the vault directory.
mod manager;
mod provider;
mod schema;
mod sqlite;

pub use manager::StorageManager;
pub use provider::StorageProvider;
pub use sqlite::SQLiteStorage;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_bus::EventBus;
    use crate::models::knowledge_object::{
        KnowledgeObject, ObjectContent, ObjectMetadata, ObjectType,
    };
    use std::collections::HashMap;
    use std::sync::Arc;
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

        let manager = StorageManager::new(vault_path.clone(), Arc::new(EventBus::new()));
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

        // First initialization
        let manager1 = StorageManager::new(vault_path.clone(), Arc::new(EventBus::new()));
        manager1.initialize().expect("Failed to initialize storage");

        // Get the database path
        let db_path = manager1.db_path().clone();

        // Second initialization should succeed without error
        let manager2 = StorageManager::new(vault_path.clone(), Arc::new(EventBus::new()));
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

        let manager = StorageManager::new(vault_path.clone(), Arc::new(EventBus::new()));
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

        let manager = StorageManager::new(vault_path, Arc::new(EventBus::new()));
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

        let manager = StorageManager::new(vault_path, Arc::new(EventBus::new()));
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

        let manager = StorageManager::new(vault_path, Arc::new(EventBus::new()));
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

        let manager = StorageManager::new(vault_path, Arc::new(EventBus::new()));
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

        let manager = StorageManager::new(vault_path, Arc::new(EventBus::new()));
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

        let manager = StorageManager::new(vault_path, Arc::new(EventBus::new()));
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

        let manager = StorageManager::new(vault_path, Arc::new(EventBus::new()));
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

        let manager = StorageManager::new(vault_path, Arc::new(EventBus::new()));
        manager.initialize().expect("Failed to initialize storage");

        // Invalid UUID format should return None (not found) rather than error
        let result = manager.get_object("not-a-valid-uuid");
        // The query will succeed but return no rows, which is None
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }
}
