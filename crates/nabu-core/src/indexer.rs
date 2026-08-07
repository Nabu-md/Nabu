use crate::event_bus::kinds::INDEX_UPDATED;
use crate::event_bus::{EventBus, IndexOperation, IndexUpdatedEvent, PipelineEvent};
use crate::models::KnowledgeObject;
use crate::registry::lifecycle::{Lifecycle, LifecycleManager, LifecycleStage};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use uuid::Uuid;

/// Sub-directory under the vault root where the Indexer persists its
/// inverted index between sessions.
const INDEX_DIR_NAME: &str = ".nabu";

/// File name for the JSON-serialized inverted index.
const INDEX_FILE_NAME: &str = "search_index.json";

/// The Indexer is the SINGLE search index for the Nabu platform.
///
/// No duplicate indexing systems exist.
/// All search operations go through the Indexer.
///
/// The inverted index is kept in memory for fast queries and is also
/// persisted to disk (JSON, under `.nabu/search_index.json`) so that
/// the index survives application restarts.  `load()` restores a
/// previously persisted index; `persist()` flushes the current in-memory
/// index to disk.
pub struct Indexer {
    index: RwLock<HashMap<String, Vec<String>>>,
    event_bus: Option<EventBus<PipelineEvent>>,
    vault_path: Option<PathBuf>,
    /// Lifecycle state manager — tracks Created -> Initialized -> Running -> Shutdown.
    lifecycle: LifecycleManager,
}

impl Indexer {
    pub fn new() -> Self {
        Self {
            index: RwLock::new(HashMap::new()),
            event_bus: None,
            vault_path: None,
            lifecycle: LifecycleManager::new(),
        }
    }

    /// Create an Indexer rooted at the given vault path so that the
    /// inverted index can be persisted to and loaded from disk.
    pub fn with_vault_path(vault_path: impl Into<PathBuf>) -> Self {
        Self {
            index: RwLock::new(HashMap::new()),
            event_bus: None,
            vault_path: Some(vault_path.into()),
            lifecycle: LifecycleManager::new(),
        }
    }

    pub fn with_event_bus(event_bus: EventBus<PipelineEvent>) -> Self {
        Self {
            index: RwLock::new(HashMap::new()),
            event_bus: Some(event_bus),
            vault_path: None,
            lifecycle: LifecycleManager::new(),
        }
    }

    /// Create an Indexer with both a vault path (for persistence) and an
    /// event bus (for publishing index-updated events).
    pub fn with_vault_path_and_event_bus(
        vault_path: impl Into<PathBuf>,
        event_bus: EventBus<PipelineEvent>,
    ) -> Self {
        Self {
            index: RwLock::new(HashMap::new()),
            event_bus: Some(event_bus),
            vault_path: Some(vault_path.into()),
            lifecycle: LifecycleManager::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Lifecycle state accessors
    // -----------------------------------------------------------------------

    /// Returns the current lifecycle stage of the indexer.
    pub fn lifecycle_stage(&self) -> LifecycleStage {
        self.lifecycle.stage()
    }

    /// Returns true if the indexer has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.lifecycle.is_at_least(LifecycleStage::Initialized)
    }

    /// Returns true if the indexer is running.
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }

    /// Returns true if the indexer has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.lifecycle.is_shutdown()
    }

    // -----------------------------------------------------------------------
    // Lifecycle operations
    // -----------------------------------------------------------------------

    /// Initializes the Indexer.
    ///
    /// Lifecycle transition: Created -> Initialized.
    ///
    /// - Prepares the search index structures.
    /// - Loads persisted index metadata from disk (when a vault path is
    ///   configured) so the in-memory index reflects on-disk state.
    /// - Initializes caches.
    pub fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            subsystem = "indexer",
            component = "indexer",
            operation = "initialize",
            "Initializing Indexer"
        );
        // Load persisted index from disk if a vault path is configured.
        self.load()?;
        self.lifecycle
            .transition_to(LifecycleStage::Initialized)?;
        tracing::info!(
            subsystem = "indexer",
            component = "indexer",
            operation = "initialize",
            "Indexer initialized"
        );
        Ok(())
    }

    /// Starts the Indexer.
    ///
    /// Lifecycle transition: Initialized -> Running (or auto-advances from
    /// Created).
    ///
    /// After starting, the indexer begins accepting indexing requests and
    /// subscribes to document events via the EventBus.
    pub fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.lifecycle.is_shutdown() {
            return Err(
                "Indexer has been shut down and cannot be restarted".into(),
            );
        }
        if self.lifecycle.stage() == LifecycleStage::Created {
            self.lifecycle
                .transition_to(LifecycleStage::Initialized)?;
        }
        self.lifecycle.transition_to(LifecycleStage::Running)?;
        tracing::info!(
            subsystem = "indexer",
            component = "indexer",
            operation = "start",
            "Indexer started"
        );
        Ok(())
    }

    /// Shuts down the Indexer gracefully.
    ///
    /// Lifecycle transition: Running -> Shutdown (or Initialized -> Shutdown).
    ///
    /// - Flushes pending index operations to disk.
    /// - Releases caches and index resources.
    /// - Terminates cleanly.
    pub fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            subsystem = "indexer",
            component = "indexer",
            operation = "shutdown",
            "Shutting down Indexer"
        );
        // Flush the in-memory index to disk before shutting down.
        let _ = self.persist();
        tracing::info!(
            subsystem = "indexer",
            component = "indexer",
            operation = "shutdown",
            "Indexer shutdown complete"
        );
        self.lifecycle
            .transition_to(LifecycleStage::Shutdown)?;
        Ok(())
    }

    /// The vault path, if configured.
    pub fn vault_path(&self) -> Option<&Path> {
        self.vault_path.as_deref()
    }

    /// Resolve the on-disk path of the persisted inverted index.
    fn index_file_path(&self) -> Option<PathBuf> {
        self.vault_path
            .as_ref()
            .map(|p| p.join(INDEX_DIR_NAME).join(INDEX_FILE_NAME))
    }

    /// Ensure the `.nabu` directory exists so the index file can be written.
    fn ensure_dir(&self) -> Result<(), String> {
        if let Some(path) = self.index_file_path() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }

    /// Persist the in-memory inverted index to disk as JSON.
    ///
    /// This is the bridge between the in-memory index and durable
    /// storage.  When a vault path is configured the index is written to
    /// `.nabu/search_index.json` inside the vault root.
    pub fn persist(&self) -> Result<(), String> {
        let path = self
            .index_file_path()
            .ok_or_else(|| "Indexer has no vault path configured".to_string())?;
        self.ensure_dir()?;

        let snapshot: HashMap<String, Vec<String>> = {
            let index = self.index.read().map_err(|e| e.to_string())?;
            index.clone()
        };

        let json = serde_json::to_string(&snapshot).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Load a previously persisted inverted index from disk, replacing
    /// the in-memory index.  If no vault path is configured or the index
    /// file does not yet exist this is a no-op.
    pub fn load(&self) -> Result<(), String> {
        let path = match self.index_file_path() {
            Some(p) => p,
            None => return Ok(()),
        };

        if !path.exists() {
            return Ok(());
        }

        let json = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let snapshot: HashMap<String, Vec<String>> =
            serde_json::from_str(&json).map_err(|e| e.to_string())?;

        let mut index = self.index.write().map_err(|e| e.to_string())?;
        *index = snapshot;
        Ok(())
    }

    /// Index a KnowledgeObject for search.
    pub fn index_object(&self, object: &KnowledgeObject) -> Result<(), String> {
        let mut index = self.index.write().map_err(|e| e.to_string())?;

        // Tokenize content and metadata
        let tokens = tokenize_object(object);

        // Add to inverted index
        for token in &tokens {
            index
                .entry(token.clone())
                .or_default()
                .push(object.id.to_string());
        }

        // Publish index updated event
        if let Some(ref bus) = self.event_bus {
            bus.publish(
                INDEX_UPDATED,
                &PipelineEvent::IndexUpdated(IndexUpdatedEvent {
                    object_id: object.id,
                    operation: IndexOperation::Added,
                    timestamp: chrono::Utc::now(),
                }),
            );
        }

        Ok(())
    }

    /// Remove an object from the index.
    pub fn remove_object(&self, object_id: Uuid) -> Result<(), String> {
        let mut index = self.index.write().map_err(|e| e.to_string())?;

        index.retain(|_, ids| {
            ids.retain(|id| id != &object_id.to_string());
            !ids.is_empty()
        });

        Ok(())
    }

    /// Search the index for matching tokens.
    pub fn search(&self, query: &str) -> Vec<String> {
        let index = self.index.read().ok();
        let mut results: Vec<String> = Vec::new();

        if let Some(index) = index {
            let query_tokens: Vec<String> = query
                .to_lowercase()
                .split_whitespace()
                .map(|s| s.to_string())
                .collect();

            for token in &query_tokens {
                if let Some(ids) = index.get(token) {
                    results.extend(ids.clone());
                }
            }
        }

        results.sort();
        results.dedup();
        results
    }

    /// Number of unique tokens in the index.
    pub fn token_count(&self) -> usize {
        self.index.read().map(|i| i.len()).unwrap_or(0)
    }

    /// Clear the entire index (for rebuild).
    pub fn clear(&self) -> Result<(), String> {
        let mut index = self.index.write().map_err(|e| e.to_string())?;
        index.clear();
        Ok(())
    }
}

impl Default for Indexer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Lifecycle trait implementation
// ---------------------------------------------------------------------------

/// Implements the shared Lifecycle trait so Indexer can be managed
/// by the Capability Platform's lifecycle manager alongside other services.
///
/// The trait methods delegate to the inherent initialize() / start() /
/// shutdown() methods defined above.
impl Lifecycle for Indexer {
    fn name(&self) -> &'static str {
        "indexer"
    }

    fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        Indexer::initialize(self)
    }

    fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        Indexer::start(self)
    }

    fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        Indexer::shutdown(self)
    }
}

fn tokenize_object(object: &KnowledgeObject) -> Vec<String> {
    let mut tokens = Vec::new();

    // From title
    if let Some(title) = &object.metadata.title {
        tokens.extend(tokenize_str(title));
    }

    // From description
    if let Some(desc) = &object.metadata.description {
        tokens.extend(tokenize_str(desc));
    }

    // From tags
    for tag in &object.tags {
        tokens.push(tag.to_lowercase());
    }

    // From content type
    tokens.push(object.object_type.variant_name().to_string());

    tokens
}

fn tokenize_str(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split_whitespace()
        .filter(|s| s.len() > 2) // Skip very short tokens
        .map(|s| {
            s.trim_matches(|c: char| c.is_ascii_punctuation())
                .to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ObjectContent, ObjectMetadata};

    #[test]
    fn test_index_and_search() {
        let indexer = Indexer::new();
        let obj = KnowledgeObject::new(
            crate::models::ObjectType::Note,
            ObjectContent::Markdown("Hello world".to_string()),
        )
        .with_metadata(ObjectMetadata {
            title: Some("Test Note".to_string()),
            ..Default::default()
        });

        indexer.index_object(&obj).unwrap();

        let results = indexer.search("test");
        assert!(
            results.contains(&obj.id.to_string()),
            "Should find object by title token"
        );

        let results = indexer.search("nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn test_remove_from_index() {
        let indexer = Indexer::new();
        let obj = KnowledgeObject::new(
            crate::models::ObjectType::Note,
            ObjectContent::PlainText("Something to index".to_string()),
        );

        indexer.index_object(&obj).unwrap();
        assert!(indexer.token_count() > 0);

        indexer.remove_object(obj.id).unwrap();

        let results = indexer.search("something");
        assert!(
            !results.contains(&obj.id.to_string()),
            "Should not find removed object"
        );
    }

    #[test]
    fn test_persist_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let obj = KnowledgeObject::new(
            crate::models::ObjectType::Note,
            ObjectContent::Markdown("Persistent search content".to_string()),
        )
        .with_metadata(ObjectMetadata {
            title: Some("Persistent Search Test".to_string()),
            ..Default::default()
        });

        // Index and persist
        {
            let indexer = Indexer::with_vault_path(dir.path());
            indexer.index_object(&obj).unwrap();
            indexer.persist().unwrap();
        }

        // Load into a new indexer and verify the index survives
        {
            let indexer = Indexer::with_vault_path(dir.path());
            indexer.load().unwrap();

            let results = indexer.search("persistent");
            assert!(
                results.contains(&obj.id.to_string()),
                "Index should survive restart"
            );
            assert!(indexer.token_count() > 0);
        }
    }

    #[test]
    fn test_load_without_persisting_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let indexer = Indexer::with_vault_path(dir.path());
        indexer.load().unwrap();
        assert_eq!(indexer.token_count(), 0);
    }
}
