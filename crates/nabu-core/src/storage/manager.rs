use crate::event_bus::kinds::ITEM_STORED;
use crate::event_bus::{EventBus, ItemStoredEvent, PipelineEvent};
use crate::models::{KnowledgeObject, ObjectType};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use uuid::Uuid;

/// The StorageManager is the SINGLE storage owner for persistent data.
///
/// No subsystem writes independently to persistent storage.
/// All persistence flows through StorageManager.
///
/// Responsibilities:
/// - Save KnowledgeObjects to the vault (Markdown + SQLite metadata)
/// - Load KnowledgeObjects from the vault
/// - Publish ItemStored events after successful persistence
/// - Delete/rename objects
///
/// Currently uses in-memory storage as a stub.
/// In production, this would use an embedded SQLite database
/// (via rusqlite or sea-orm) combined with the Markdown file system.
pub struct StorageManager {
    store: RwLock<HashMap<Uuid, KnowledgeObject>>,
    vault_path: PathBuf,
    event_bus: Option<EventBus<PipelineEvent>>,
}

impl StorageManager {
    /// Create a new storage manager rooted at the given vault path.
    pub fn new(vault_path: impl Into<PathBuf>) -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            vault_path: vault_path.into(),
            event_bus: None,
        }
    }

    /// Create with event bus for publishing store events.
    pub fn with_event_bus(
        vault_path: impl Into<PathBuf>,
        event_bus: EventBus<PipelineEvent>,
    ) -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            vault_path: vault_path.into(),
            event_bus: Some(event_bus),
        }
    }

    /// Save a KnowledgeObject to storage.
    ///
    /// Returns the vault path where the object was saved.
    /// Publishes ItemStored event on success.
    pub fn save(&self, object: &KnowledgeObject) -> Result<String, String> {
        let vault_path = object.metadata.vault_path.clone().unwrap_or_else(|| {
            format!(
                "Inbox/{}.md",
                object.metadata.title.as_deref().unwrap_or("untitled")
            )
        });

        // Store in memory
        {
            let mut store = self.store.write().map_err(|e| e.to_string())?;
            store.insert(object.id, object.clone());
        }

        // Publish stored event
        if let Some(ref bus) = self.event_bus {
            bus.publish(
                ITEM_STORED,
                &PipelineEvent::ItemStored(ItemStoredEvent {
                    object_id: object.id,
                    vault_path: vault_path.clone(),
                    object_type: object.object_type.clone(),
                    timestamp: chrono::Utc::now(),
                }),
            );
        }

        Ok(vault_path)
    }

    /// Load a KnowledgeObject by ID.
    pub fn load(&self, id: Uuid) -> Option<KnowledgeObject> {
        let store = self.store.read().ok()?;
        store.get(&id).cloned()
    }

    /// Load all KnowledgeObjects of a given type.
    pub fn load_by_type(&self, object_type: ObjectType) -> Vec<KnowledgeObject> {
        if let Ok(store) = self.store.read() {
            store
                .values()
                .filter(|o| o.object_type == object_type)
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Delete a KnowledgeObject by ID.
    pub fn delete(&self, id: Uuid) -> Result<(), String> {
        let mut store = self.store.write().map_err(|e| e.to_string())?;
        store.remove(&id);
        Ok(())
    }

    /// Count of stored objects.
    pub fn count(&self) -> usize {
        self.store.read().map(|s| s.len()).unwrap_or(0)
    }

    /// The vault path.
    pub fn vault_path(&self) -> &PathBuf {
        &self.vault_path
    }

    /// Check if an object exists.
    pub fn exists(&self, id: Uuid) -> bool {
        self.store
            .read()
            .map(|s| s.contains_key(&id))
            .unwrap_or(false)
    }

    /// List all stored objects, optionally filtered by source file.
    ///
    /// Enumerates the vault through the single storage owner.
    pub fn list_objects(
        &self,
        _vault_id: &str,
        source_file: Option<&str>,
        limit: usize,
    ) -> Result<Vec<KnowledgeObject>, String> {
        let store = self.store.read().map_err(|e| e.to_string())?;

        let mut objects: Vec<KnowledgeObject> = store.values().cloned().collect();

        if let Some(source_file) = source_file {
            objects.retain(|o| o.metadata.original_filename.as_deref() == Some(source_file));
        }

        objects.truncate(limit);
        Ok(objects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ObjectContent;

    #[test]
    fn test_save_and_load() {
        let mgr = StorageManager::new("/tmp/test-vault");
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown("Hello".to_string()),
        );

        mgr.save(&obj).unwrap();
        let loaded = mgr.load(obj.id).unwrap();

        assert_eq!(loaded.id, obj.id);
        assert_eq!(loaded.object_type, ObjectType::Note);
    }

    #[test]
    fn test_delete() {
        let mgr = StorageManager::new("/tmp/test-vault");
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::PlainText("Delete me".to_string()),
        );

        mgr.save(&obj).unwrap();
        assert!(mgr.exists(obj.id));

        mgr.delete(obj.id).unwrap();
        assert!(!mgr.exists(obj.id));
    }
}
