use crate::event_bus::kinds::ITEM_STORED;
use crate::event_bus::{EventBus, ItemStoredEvent, PipelineEvent};
use crate::models::{CustomPropertyValue, KnowledgeObject, ObjectContent, ObjectMetadata, ObjectType, ProcessingState};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use uuid::Uuid;

/// Sub-directory under the vault root where StorageManager keeps its
/// JSON sidecar metadata index (one file per object).
const INDEX_DIR_NAME: &str = ".nabu";

/// The StorageManager is the SINGLE storage owner for persistent data.
///
/// No subsystem writes independently to persistent storage.
/// All persistence flows through StorageManager.
///
/// Responsibilities:
/// - Save KnowledgeObjects to the vault (Markdown + JSON sidecar metadata)
/// - Load KnowledgeObjects from the vault
/// - Publish ItemStored events after successful persistence
/// - Delete/rename objects
///
/// The vault uses a Markdown-first storage layout: human-readable `.md` /
/// `.txt` / `.html` / `.json` content files live at the vault root while a
/// per-object JSON sidecar (under `.nabu/`) stores the structured
/// `ObjectMetadata` and `ObjectType` so the object can be loaded back
/// without re-parsing file content.  An in-memory `HashMap` cache is
/// retained for fast lookups so that the existing API (`load`, `exists`,
/// `load_by_type`, `list_objects`) keeps working without breaking callers
/// that rely on in-process state.
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

    /// Ensure the vault root and `.nabu` index directory exist.
    fn ensure_dirs(&self) -> Result<(), String> {
        let vault = self.vault_path.as_path();
        std::fs::create_dir_all(vault).map_err(|e| e.to_string())?;
        let index_dir = vault.join(INDEX_DIR_NAME);
        std::fs::create_dir_all(&index_dir).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Compute the vault-relative path for a KnowledgeObject.
    ///
    /// Falls back to `Inbox/<title>.md` when no explicit `vault_path` is set.
    fn resolve_vault_path(&self, object: &KnowledgeObject) -> Result<String, String> {
        if let Some(p) = &object.metadata.vault_path {
            if !p.is_empty() {
                return Ok(p.clone());
            }
        }
        let title = object
            .metadata
            .title
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("untitled");
        let ext = content_extension_for(&object.content);
        Ok(format!("Inbox/{}.{}", slugify(title), ext))
    }

    /// The absolute path of the JSON sidecar for a given object id.
    fn sidecar_path(&self, id: Uuid) -> PathBuf {
        self.vault_path
            .join(INDEX_DIR_NAME)
            .join(format!("{}.json", id))
    }

    /// The absolute path of the content file for a vault-relative path.
    fn content_path(&self, vault_rel: &str) -> PathBuf {
        self.vault_path.join(vault_rel)
    }

    /// Serialize the sidecar metadata for a KnowledgeObject.
    ///
    /// The `resolved_vault_path` parameter provides the canonical vault-relative
    /// path computed by [`resolve_vault_path`](Self::resolve_vault_path),
    /// ensuring that round-tripping through the JSON sidecar preserves the
    /// content file location even when the object itself did not carry an
    /// explicit `vault_path` in its metadata.
    fn serialize_sidecar(object: &KnowledgeObject, resolved_vault_path: &str) -> Result<String, String> {
        let sidecar = Sidecar {
            id: object.id,
            object_type: object.object_type.clone(),
            title: object.metadata.title.clone(),
            description: object.metadata.description.clone(),
            tags: object.tags.clone(),
            source_url: object.metadata.source_url.clone(),
            authors: object.metadata.authors.clone(),
            publication_date: object.metadata.publication_date,
            site_name: object.metadata.site_name.clone(),
            language: object.metadata.language.clone(),
            file_size: object.metadata.file_size,
            mime_type: object.metadata.mime_type.clone(),
            original_filename: object.metadata.original_filename.clone(),
            vault_path: Some(resolved_vault_path.to_string()),
            created_at: object.created_at,
            updated_at: object.updated_at,
            content_ext: content_extension_for(&object.content).to_string(),
            custom_properties: object.custom_properties.clone(),
        };
        serde_json::to_string_pretty(&sidecar).map_err(|e| e.to_string())
    }

    /// Save a KnowledgeObject to storage.
    ///
    /// The object's content is written to the vault as a Markdown / text /
    /// HTML / JSON file (Markdown-first) and a small JSON sidecar is written
    /// under `.nabu/` recording the structured metadata so the object can be
    /// loaded back without re-parsing.  The in-memory cache is updated and
    /// an `ItemStored` event is published on success.
    ///
    /// Returns the vault path where the object was saved.
    pub fn save(&self, object: &KnowledgeObject) -> Result<String, String> {
        self.ensure_dirs()?;
        let vault_rel = self.resolve_vault_path(object)?;
        let content_path = self.content_path(&vault_rel);

        // Write content file (Markdown-first).
        if let Some(parent) = content_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        match &object.content {
            ObjectContent::Markdown(s)
            | ObjectContent::RichHtml(s)
            | ObjectContent::PlainText(s)
            | ObjectContent::Uri(s) => {
                std::fs::write(&content_path, s).map_err(|e| e.to_string())?;
            }
            ObjectContent::Binary {
                data, mime_type, ..
            } => {
                // Binary data goes into a `.bin` sidecar; content file is empty
                // placeholder so the vault-rel path still resolves.
                let bin_path = self.sidecar_path(object.id).with_extension("bin");
                std::fs::write(&bin_path, data).map_err(|e| e.to_string())?;
                let _ = mime_type;
            }
        }

        // Write JSON sidecar with structured metadata.
        let sidecar = Self::serialize_sidecar(object, &vault_rel)?;
        std::fs::write(self.sidecar_path(object.id), sidecar).map_err(|e| e.to_string())?;

        // Update in-memory cache.
        {
            let mut store = self.store.write().map_err(|e| e.to_string())?;
            store.insert(object.id, object.clone());
        }

        // Publish stored event.
        if let Some(ref bus) = self.event_bus {
            bus.publish(
                ITEM_STORED,
                &PipelineEvent::ItemStored(ItemStoredEvent {
                    object_id: object.id,
                    vault_path: vault_rel.clone(),
                    object_type: object.object_type.clone(),
                    timestamp: chrono::Utc::now(),
                }),
            );
        }

        Ok(vault_rel)
    }

    /// Load a KnowledgeObject by ID.
    ///
    /// Checks the in-memory cache first; if not present, reconstructs the
    /// object from the persisted JSON sidecar and content file on disk.
    pub fn load(&self, id: Uuid) -> Option<KnowledgeObject> {
        // Fast path: in-memory cache.
        if let Ok(store) = self.store.read() {
            if let Some(obj) = store.get(&id) {
                return Some(obj.clone());
            }
        }

        // Slow path: reconstruct from disk.
        let sidecar_str = std::fs::read_to_string(self.sidecar_path(id)).ok()?;
        let sidecar: Sidecar = serde_json::from_str(&sidecar_str).ok()?;
        Self::sidecar_to_object(&sidecar, self.vault_path.as_path())
    }

    /// Load all KnowledgeObjects of a given type.
    ///
    /// Reads the `.nabu` sidecar directory and filters by `object_type`.
    pub fn load_by_type(&self, object_type: ObjectType) -> Vec<KnowledgeObject> {
        // Fast path: serve from cache when populated.
        {
            let store = self.store.read().ok();
            if let Some(s) = store {
                if !s.is_empty() {
                    return s
                        .values()
                        .filter(|o| o.object_type == object_type)
                        .cloned()
                        .collect();
                }
            }
        }

        // Slow path: read sidecars from disk.
        let index_dir = self.vault_path.join(INDEX_DIR_NAME);
        let mut results = Vec::new();
        let entries = match std::fs::read_dir(&index_dir) {
            Ok(e) => e,
            Err(_) => return results,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let data = std::fs::read_to_string(&path).ok();
            let id_str = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let id = Uuid::parse_str(id_str).ok();
            if let (Some(_), Some(data)) = (id, data) {
                if let Ok(sidecar) = serde_json::from_str::<Sidecar>(&data) {
                    if sidecar.object_type == object_type {
                        if let Some(obj) =
                            Self::sidecar_to_object(&sidecar, self.vault_path.as_path())
                        {
                            results.push(obj);
                        }
                    }
                }
            }
        }
        results
    }

    /// Reconstruct a KnowledgeObject from a deserialized sidecar + vault root.
    fn sidecar_to_object(sidecar: &Sidecar, vault_root: &Path) -> Option<KnowledgeObject> {
        let content = if let Some(rel) = &sidecar.vault_path {
            let abs = vault_root.join(rel);
            let raw = std::fs::read_to_string(&abs).ok()?;
            match sidecar.content_ext.as_str() {
                "html" => ObjectContent::RichHtml(raw),
                "txt" => ObjectContent::PlainText(raw),
                "json" => {
                    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
                    ObjectContent::Binary {
                        mime_type: "application/json".to_string(),
                        data: serde_json::to_vec(&v).ok()?,
                        filename: None,
                    }
                }
                _ => ObjectContent::Markdown(raw),
            }
        } else {
            ObjectContent::Markdown(String::new())
        };

        let metadata = ObjectMetadata {
            title: sidecar.title.clone(),
            source_url: sidecar.source_url.clone(),
            authors: sidecar.authors.clone(),
            publication_date: sidecar.publication_date,
            site_name: sidecar.site_name.clone(),
            language: sidecar.language.clone(),
            file_size: sidecar.file_size,
            mime_type: sidecar.mime_type.clone(),
            ocr_confidence: None,
            original_filename: sidecar.original_filename.clone(),
            vault_path: sidecar.vault_path.clone(),
            description: sidecar.description.clone(),
        };

        Some(KnowledgeObject {
            id: sidecar.id,
            object_type: sidecar.object_type.clone(),
            content,
            metadata,
            custom_properties: sidecar.custom_properties.clone(),
            tags: sidecar.tags.clone(),
            relations: vec![],
            processing_state: ProcessingState::Completed,
            content_hash: None,
            created_at: sidecar.created_at,
            updated_at: sidecar.updated_at,
        })
    }

    /// Delete a KnowledgeObject by ID.
    ///
    /// Removes the object from the in-memory cache and deletes its
    /// persisted sidecar and content file from disk.
    pub fn delete(&self, id: Uuid) -> Result<(), String> {
        // Remove sidecar.
        let _ = std::fs::remove_file(self.sidecar_path(id));

        // Remove from cache and track content path for file deletion.
        let content_rel;
        {
            let mut store = self.store.write().map_err(|e| e.to_string())?;
            let removed = store.remove(&id);
            content_rel = removed.and_then(|o| o.metadata.vault_path);
        }

        // Remove content file if it exists.
        if let Some(rel) = content_rel {
            let _ = std::fs::remove_file(self.content_path(&rel));
        }

        Ok(())
    }

    /// Count of stored objects (in-memory cache size).
    pub fn count(&self) -> usize {
        self.store.read().map(|s| s.len()).unwrap_or(0)
    }

    /// The vault path.
    pub fn vault_path(&self) -> &PathBuf {
        &self.vault_path
    }

    /// Check if an object exists in the cache or on disk.
    pub fn exists(&self, id: Uuid) -> bool {
        if let Ok(store) = self.store.read() {
            if store.contains_key(&id) {
                return true;
            }
        }
        self.sidecar_path(id).exists()
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
        // Try cache first.
        {
            let store = self.store.read().map_err(|e| e.to_string())?;
            if !store.is_empty() {
                let mut objects: Vec<KnowledgeObject> = store.values().cloned().collect();
                if let Some(source_file) = source_file {
                    objects
                        .retain(|o| o.metadata.original_filename.as_deref() == Some(source_file));
                }
                objects.truncate(limit);
                return Ok(objects);
            }
        }

        // Fall back to disk.
        let index_dir = self.vault_path.join(INDEX_DIR_NAME);
        let mut objects = Vec::new();
        let entries = std::fs::read_dir(&index_dir).map_err(|e| e.to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            if let Ok(sidecar) = serde_json::from_str::<Sidecar>(&data) {
                if let Some(obj) = Self::sidecar_to_object(&sidecar, self.vault_path.as_path()) {
                    objects.push(obj);
                }
            }
        }

        if let Some(source_file) = source_file {
            objects.retain(|o| o.metadata.original_filename.as_deref() == Some(source_file));
        }
        objects.truncate(limit);
        Ok(objects)
    }

    /// Rebuild the in-memory cache from disk, loading all persisted
    /// KnowledgeObjects.  This is called during application startup so
    /// that the cache reflects the on-disk vault state.
    pub fn reload_from_disk(&self) -> Result<usize, String> {
        let index_dir = self.vault_path.join(INDEX_DIR_NAME);
        let mut count = 0usize;
        let mut store = self.store.write().map_err(|e| e.to_string())?;

        store.clear();

        if !index_dir.exists() {
            return Ok(0);
        }

        for entry in std::fs::read_dir(&index_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let data = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
            if let Ok(sidecar) = serde_json::from_str::<Sidecar>(&data) {
                if let Some(obj) = Self::sidecar_to_object(&sidecar, self.vault_path.as_path()) {
                    store.insert(obj.id, obj);
                    count += 1;
                }
            }
        }

        Ok(count)
    }
}

/// Best-effort slugify for generating safe filenames from titles.
fn slugify(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if c.is_whitespace() || c == '-' || c == '_' {
            out.push('-');
        }
    }
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}

/// Determine the file extension to use for a given content variant.
fn content_extension_for(content: &ObjectContent) -> &'static str {
    match content {
        ObjectContent::Markdown(_) => "md",
        ObjectContent::RichHtml(_) => "html",
        ObjectContent::PlainText(_) => "txt",
        ObjectContent::Uri(_) => "uri",
        ObjectContent::Binary { .. } => "bin",
    }
}

/// Intermediate serializable form of a KnowledgeObject sidecar.
#[derive(serde::Serialize, serde::Deserialize)]
struct Sidecar {
    id: Uuid,
    object_type: ObjectType,
    title: Option<String>,
    description: Option<String>,
    tags: Vec<String>,
    source_url: Option<String>,
    authors: Vec<String>,
    publication_date: Option<chrono::DateTime<chrono::Utc>>,
    site_name: Option<String>,
    language: Option<String>,
    file_size: Option<u64>,
    mime_type: Option<String>,
    original_filename: Option<String>,
    vault_path: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    content_ext: String,
    /// Extensible custom properties (classification, inbox status, filing
    /// suggestions, duplicate info, …). Defaults to empty for sidecars written
    /// before this field existed, so older vaults keep loading.
    #[serde(default)]
    custom_properties: HashMap<String, CustomPropertyValue>,
}

/// Compute a content hash for change detection.
#[allow(dead_code)]
fn content_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = StorageManager::new(dir.path());
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown("Hello".to_string()),
        )
        .with_metadata(ObjectMetadata {
            title: Some("Hello World".to_string()),
            ..Default::default()
        });

        mgr.save(&obj).unwrap();
        let loaded = mgr.load(obj.id).unwrap();

        assert_eq!(loaded.id, obj.id);
        assert_eq!(loaded.object_type, ObjectType::Note);
        assert_eq!(loaded.content, ObjectContent::Markdown("Hello".to_string()));
    }

    #[test]
    fn test_save_and_load_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown("Persisted content".to_string()),
        )
        .with_metadata(ObjectMetadata {
            title: Some("Persisted Note".to_string()),
            ..Default::default()
        });

        {
            let mgr = StorageManager::new(dir.path());
            mgr.save(&obj).unwrap();
        }

        // New manager instance — should reconstruct from disk.
        {
            let mgr = StorageManager::new(dir.path());
            assert!(mgr.exists(obj.id));
            let loaded = mgr.load(obj.id).unwrap();
            assert_eq!(loaded.id, obj.id);
            assert_eq!(
                loaded.content,
                ObjectContent::Markdown("Persisted content".to_string())
            );
        }
    }

    #[test]
    fn test_delete() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = StorageManager::new(dir.path());
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::PlainText("Delete me".to_string()),
        );

        mgr.save(&obj).unwrap();
        assert!(mgr.exists(obj.id));

        mgr.delete(obj.id).unwrap();
        assert!(!mgr.exists(obj.id));
    }

    #[test]
    fn test_reload_from_disk() {
        let dir = tempfile::tempdir().unwrap();

        let obj = KnowledgeObject::new(
            ObjectType::Article,
            ObjectContent::Markdown("Reload me".to_string()),
        )
        .with_metadata(ObjectMetadata {
            title: Some("Reload Test".to_string()),
            ..Default::default()
        });

        {
            let mgr = StorageManager::new(dir.path());
            mgr.save(&obj).unwrap();
        }

        {
            let mgr = StorageManager::new(dir.path());
            let count = mgr.reload_from_disk().unwrap();
            assert!(count >= 1);
            let loaded = mgr.load(obj.id).unwrap();
            assert_eq!(loaded.object_type, ObjectType::Article);
        }
    }

    #[test]
    fn test_load_by_type_from_disk() {
        let dir = tempfile::tempdir().unwrap();

        let note = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown("Note content".to_string()),
        )
        .with_metadata(ObjectMetadata {
            title: Some("Note One".to_string()),
            ..Default::default()
        });
        let bookmark = KnowledgeObject::new(
            ObjectType::Bookmark,
            ObjectContent::Uri("https://example.com".to_string()),
        )
        .with_metadata(ObjectMetadata {
            title: Some("Bookmark".to_string()),
            ..Default::default()
        });

        {
            let mgr = StorageManager::new(dir.path());
            mgr.save(&note).unwrap();
            mgr.save(&bookmark).unwrap();
        }

        {
            let mgr = StorageManager::new(dir.path());
            let notes = mgr.load_by_type(ObjectType::Note);
            assert_eq!(notes.len(), 1);
            let bookmarks = mgr.load_by_type(ObjectType::Bookmark);
            assert_eq!(bookmarks.len(), 1);
        }
    }
}
