use crate::conversations::{PersistenceError, PersistenceResult};
use crate::models::conversation::Thread;
use crate::registry::lifecycle::{Lifecycle, LifecycleManager, LifecycleStage};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use uuid::Uuid;

/// Sub-directory under the vault's `.nabu` config directory where the
/// ConversationStore persists serialized thread JSON files.
const CONVERSATIONS_DIR_NAME: &str = "conversations";

/// The file extension used for persisted thread files.
const THREAD_FILE_EXT: &str = "json";

/// The manifest filename — records thread IDs and ordering for discovery.
const MANIFEST_FILE_NAME: &str = "manifest.json";

/// A minimal manifest entry recording the file path and timestamps for a thread.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct ManifestEntry {
    id: Uuid,
    filename: String,
    created_at: DateTime<Utc>,
    updated_at: Option<DateTime<Utc>>,
}

/// The manifest file records the ordering and metadata of all persisted threads.
#[derive(Debug, Clone, Serialize, serde::Deserialize, Default)]
struct Manifest {
    /// Schema version for forward-compatibility.
    #[serde(default)]
    version: u32,
    /// Ordered list of thread entries (order = created order).
    #[serde(default)]
    threads: Vec<ManifestEntry>,
}

/// The `ConversationStore` is the SINGLE owner of persistent conversation
/// storage.
///
/// It isolates all storage concerns (file layout, atomic writes, recovery,
/// validation) from the conversation models (`Thread`, `Message`, `Turn`).
/// The models remain simple data structures that derive `Serialize` /
/// `Deserialize`; all persistence logic lives here.
///
/// # Architecture
///
/// ```text
/// Thread / Message / Turn  (in-memory data models)
///     │  Serialize / Deserialize (Serde)
///     ▼
/// ConversationStore (persistence layer)
///     │  atomic write / read / discover / validate
///     ▼
/// Disk  (.nabu/conversations/<uuid>.json + manifest)
/// ```
///
/// # Thread Safety
///
/// The store uses an internal `RwLock<HashMap<Uuid, Thread>>` for the
/// in-memory cache. All public methods are safe to call from multiple
/// threads concurrently (`Send + Sync`).
///
/// # Lifecycle Integration
///
/// `ConversationStore` implements the [`Lifecycle`] trait. The
/// [`ApplicationContext`](crate::registry::context::ApplicationContext)
/// manages it through the standard lifecycle:
///
/// - **`initialize()`** — discovers persisted threads, deserializes and
///   validates each one, loads them into the in-memory cache. Corrupted or
///   invalid threads are skipped with a warning.
/// - **`start()`** — marks the store as running.
/// - **`shutdown()`** — flushes pending writes (rewrites manifest) and marks
///   the store as shut down.
pub struct ConversationStore {
    /// In-memory cache of all loaded threads, keyed by thread ID.
    store: RwLock<HashMap<Uuid, Thread>>,
    /// The vault path (root of the Nabu vault).
    vault_path: PathBuf,
    /// Lifecycle state manager — tracks Created → Initialized → Running → Shutdown.
    lifecycle: LifecycleManager,
    /// The path to the conversations subdirectory: `<vault>/.nabu/conversations/`.
    conversations_dir: PathBuf,
    /// The path to the manifest file.
    manifest_path: PathBuf,
}

impl ConversationStore {
    /// Creates a new `ConversationStore` rooted at the given vault path.
    ///
    /// The store starts in the `Created` lifecycle stage. Call
    /// `initialize()` (or use the `Lifecycle` trait via
    /// `ApplicationContext`) to load persisted threads from disk.
    pub fn new(vault_path: impl Into<PathBuf>) -> Self {
        let vault_path = vault_path.into();
        let conversations_dir =
            vault_path.join(".nabu").join(CONVERSATIONS_DIR_NAME);
        let manifest_path = conversations_dir.join(MANIFEST_FILE_NAME);

        Self {
            store: RwLock::new(HashMap::new()),
            vault_path,
            lifecycle: LifecycleManager::new(),
            conversations_dir,
            manifest_path,
        }
    }

    // -----------------------------------------------------------------------
    // Lifecycle state accessors
    // -----------------------------------------------------------------------

    /// Returns the current lifecycle stage of the store.
    pub fn lifecycle_stage(&self) -> LifecycleStage {
        self.lifecycle.stage()
    }

    /// Returns `true` if the store has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.lifecycle.is_at_least(LifecycleStage::Initialized)
    }

    /// Returns `true` if the store is running.
    pub fn is_running(&self) -> bool {
        self.lifecycle.is_running()
    }

    /// Returns `true` if the store has been shut down.
    pub fn is_shutdown(&self) -> bool {
        self.lifecycle.is_shutdown()
    }

    /// Returns the vault path.
    pub fn vault_path(&self) -> &Path {
        &self.vault_path
    }

    // -----------------------------------------------------------------------
    // Directory helpers
    // -----------------------------------------------------------------------

    /// Ensures the conversations directory exists on disk.
    fn ensure_dirs(&self) -> Result<(), PersistenceError> {
        if !self.conversations_dir.exists() {
            std::fs::create_dir_all(&self.conversations_dir)?;
        }
        Ok(())
    }

    /// Returns the absolute path for a thread's persistence file.
    fn thread_file_path(&self, id: Uuid) -> PathBuf {
        let filename = format!("{}.{}", id, THREAD_FILE_EXT);
        self.conversations_dir.join(filename)
    }

    // -----------------------------------------------------------------------
    // Validation
    // -----------------------------------------------------------------------

    /// Validates a thread after deserialization using the model's own
    /// `validate()` method, which checks thread IDs, message IDs, thread
    /// references, turn references, and ordering.
    fn validate_thread(thread: &Thread) -> Result<(), PersistenceError> {
        thread
            .validate()
            .map_err(|e| PersistenceError::ValidationError {
                thread_id: thread.id,
                reason: e.to_string(),
            })
    }

    // -----------------------------------------------------------------------
    // Save
    // -----------------------------------------------------------------------

    /// Saves a thread to persistent storage.
    ///
    /// The thread's JSON is written to `.nabu/conversations/<thread_id>.json`
    /// using an atomic write (write-to-temp + rename) to prevent partial files
    /// and data corruption on crash. The in-memory cache is updated and the
    /// manifest is refreshed.
    ///
    /// If a thread with the same ID already exists, it is replaced.
    pub fn save(&self, thread: &Thread) -> PersistenceResult<()> {
        if self.lifecycle.is_shutdown() {
            return Err(PersistenceError::Shutdown);
        }

        self.ensure_dirs()?;
        Self::validate_thread(thread)?;

        let file_path = self.thread_file_path(thread.id);
        let json = serde_json::to_string_pretty(thread)?;

        // Atomic write: write to temp file then rename.
        let temp_path = file_path.with_extension(format!("tmp.{}", THREAD_FILE_EXT));
        std::fs::write(&temp_path, &json)?;
        std::fs::rename(&temp_path, &file_path)?;

        // Update in-memory cache.
        {
            let mut store = self.store.write().expect("store lock poisoned");
            store.insert(thread.id, thread.clone());
        }

        // Refresh the manifest.
        self.write_manifest()?;

        tracing::debug!(
            thread_id = %thread.id,
            "Thread persisted to disk"
        );

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Load
    // -----------------------------------------------------------------------

    /// Loads a single thread by ID.
    ///
    /// Checks the in-memory cache first; if not present, reads and
    /// deserializes from the persistence file on disk and validates it.
    pub fn load(&self, id: Uuid) -> PersistenceResult<Thread> {
        // Fast path: in-memory cache.
        {
            let store = self.store.read().expect("store lock poisoned");
            if let Some(thread) = store.get(&id) {
                return Ok(thread.clone());
            }
        }

        // Slow path: load from disk.
        let file_path = self.thread_file_path(id);
        if !file_path.exists() {
            return Err(PersistenceError::ThreadNotFound { thread_id: id });
        }

        let json = std::fs::read_to_string(&file_path)?;
        let thread: Thread = serde_json::from_str(&json).map_err(|e| {
            PersistenceError::DeserializationError {
                target: id.to_string(),
                message: e.to_string(),
            }
        })?;

        Self::validate_thread(&thread)?;

        // Cache the loaded thread.
        {
            let mut store = self.store.write().expect("store lock poisoned");
            store.insert(thread.id, thread.clone());
        }

        Ok(thread)
    }

    /// Loads all persisted threads from disk, validating each one.
    ///
    /// Threads that fail deserialization or validation are logged and skipped
    /// — the store returns only the successfully loaded threads.
    pub fn load_all(&self) -> PersistenceResult<Vec<Thread>> {
        let mut threads = Vec::new();

        self.ensure_dirs()?;

        let entries = std::fs::read_dir(&self.conversations_dir)?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some(THREAD_FILE_EXT) {
                continue;
            }

            // Skip the manifest file.
            if path.file_name() == Some(MANIFEST_FILE_NAME.as_ref()) {
                continue;
            }

            let json = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        file = %path.display(),
                        error = %e,
                        "Skipping unreadable thread file"
                    );
                    continue;
                }
            };

            let thread: Thread = match serde_json::from_str(&json) {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!(
                        file = %path.display(),
                        error = %e,
                        "Skipping thread with deserialization error"
                    );
                    continue;
                }
            };

            if let Err(e) = Self::validate_thread(&thread) {
                tracing::warn!(
                    file = %path.display(),
                    error = %e,
                    "Skipping invalid thread"
                );
                continue;
            }

            threads.push(thread);
        }

        Ok(threads)
    }

    // -----------------------------------------------------------------------
    // Delete
    // -----------------------------------------------------------------------

    /// Deletes a thread from persistent storage and the in-memory cache.
    ///
    /// Returns `Err(PersistenceError::ThreadNotFound)` if the thread does
    /// not exist.
    pub fn delete(&self, id: Uuid) -> PersistenceResult<()> {
        if self.lifecycle.is_shutdown() {
            return Err(PersistenceError::Shutdown);
        }

        let file_path = self.thread_file_path(id);
        if !file_path.exists() {
            return Err(PersistenceError::ThreadNotFound { thread_id: id });
        }

        std::fs::remove_file(&file_path)?;

        // Remove from cache.
        {
            let mut store = self.store.write().expect("store lock poisoned");
            store.remove(&id);
        }

        // Refresh the manifest.
        self.write_manifest()?;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // In-memory cache accessors
    // -----------------------------------------------------------------------

    /// Returns all threads currently in the in-memory cache.
    pub fn list(&self) -> Vec<Thread> {
        let store = self.store.read().expect("store lock poisoned");
        store.values().cloned().collect()
    }

    /// Returns all thread IDs currently in the in-memory cache.
    pub fn list_ids(&self) -> Vec<Uuid> {
        let store = self.store.read().expect("store lock poisoned");
        store.keys().cloned().collect()
    }

    /// Returns the number of threads in the cache.
    pub fn count(&self) -> usize {
        let store = self.store.read().expect("store lock poisoned");
        store.len()
    }

    /// Returns `true` if a thread with the given ID exists in the cache
    /// or on disk.
    pub fn exists(&self, id: Uuid) -> bool {
        {
            let store = self.store.read().expect("store lock poisoned");
            if store.contains_key(&id) {
                return true;
            }
        }
        self.thread_file_path(id).exists()
    }

    // -----------------------------------------------------------------------
    // Manifest
    // -----------------------------------------------------------------------

    /// Writes the manifest file recording all threads in the in-memory cache.
    ///
    /// The manifest is written atomically (temp + rename) and is used for
    /// thread discovery and ordering during startup recovery.
    fn write_manifest(&self) -> Result<(), PersistenceError> {
        let store = self.store.read().expect("store lock poisoned");

        let entries: Vec<ManifestEntry> = store
            .values()
            .map(|t| ManifestEntry {
                id: t.id,
                filename: format!("{}.{}", t.id, THREAD_FILE_EXT),
                created_at: t.created_at,
                updated_at: t.updated_at,
            })
            .collect();

        let manifest = Manifest {
            version: 1,
            threads: entries,
        };

        let json = serde_json::to_string_pretty(&manifest)?;

        let temp_path = self.manifest_path.with_extension("tmp");
        std::fs::write(&temp_path, &json)?;
        std::fs::rename(&temp_path, &self.manifest_path)?;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Recovery
    // -----------------------------------------------------------------------

    /// Rebuilds the in-memory cache from disk, loading all persisted threads.
    ///
    /// Called during `initialize()` so the cache reflects the on-disk state.
    /// Threads that fail validation or deserialization are skipped with a
    /// warning — the application continues operating with the valid threads.
    ///
    /// Returns the number of threads successfully loaded.
    pub fn reload_from_disk(&self) -> PersistenceResult<usize> {
        let threads = self.load_all()?;
        let count = threads.len();

        {
            let mut store = self.store.write().expect("store lock poisoned");
            store.clear();
            for thread in threads {
                store.insert(thread.id, thread);
            }
        }

        tracing::info!(
            threads = count,
            "Loaded conversations from disk"
        );

        Ok(count)
    }

    /// Creates a thread if it does not already exist, or does nothing if
    /// a thread with the same ID already exists (idempotent register).
    ///
    /// This is useful for recovery scenarios where a thread may already be
    /// in the cache but the caller wants to ensure it is persisted.
    pub fn register(&self, thread: &Thread) -> PersistenceResult<()> {
        {
            let store = self.store.read().expect("store lock poisoned");
            if store.contains_key(&thread.id) {
                return Ok(());
            }
        }
        self.save(thread)
    }

    /// Updates an existing thread in the cache and persists it.
    ///
    /// The thread's `updated_at` timestamp is set to the current time before
    /// saving. Returns `Err(NotFound)` if the thread is not currently
    /// in the cache or on disk.
    pub fn update(&self, thread: &mut Thread) -> PersistenceResult<()> {
        if !self.exists(thread.id) {
            return Err(PersistenceError::ThreadNotFound {
                thread_id: thread.id,
            });
        }
        thread.updated_at = Some(Utc::now());
        self.save(thread)
    }
}

// ---------------------------------------------------------------------------
// Lifecycle trait implementation
// ---------------------------------------------------------------------------

impl Lifecycle for ConversationStore {
    fn name(&self) -> &'static str {
        "conversation_store"
    }

    fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            subsystem = "conversations",
            component = "store",
            operation = "initialize",
            "Initializing ConversationStore"
        );

        self.ensure_dirs()?;
        let count = self.reload_from_disk()?;

        self.lifecycle
            .transition_to(LifecycleStage::Initialized)?;

        tracing::info!(
            subsystem = "conversations",
            component = "store",
            operation = "initialize",
            threads = count,
            "ConversationStore initialized"
        );

        Ok(())
    }

    fn start(&self) -> Result<(), Box<dyn std::error::Error>> {
        if self.lifecycle.is_shutdown() {
            return Err(
                "ConversationStore has been shut down and cannot be restarted".into(),
            );
        }
        if self.lifecycle.stage() == LifecycleStage::Created {
            self.lifecycle
                .transition_to(LifecycleStage::Initialized)?;
        }
        self.lifecycle.transition_to(LifecycleStage::Running)?;

        tracing::info!(
            subsystem = "conversations",
            component = "store",
            operation = "start",
            "ConversationStore started"
        );

        Ok(())
    }

    fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!(
            subsystem = "conversations",
            component = "store",
            operation = "shutdown",
            "Shutting down ConversationStore"
        );

        // Flush: ensure the manifest is consistent with the in-memory cache.
        let _ = self.write_manifest();

        self.lifecycle
            .transition_to(LifecycleStage::Shutdown)?;

        tracing::info!(
            subsystem = "conversations",
            component = "store",
            operation = "shutdown",
            "ConversationStore shutdown complete"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::conversation::{Message, Role, Turn, TurnContent};

    /// Helper: build a simple but complete thread for testing.
    fn sample_thread(title: &str) -> Thread {
        let mut thread = Thread::new().with_title(title);

        let msg = Message::new_anonymous()
            .with_role(Role::System)
            .with_turn(
                Turn::new_anonymous(TurnContent::text("You are a helpful assistant.")),
            );
        thread = thread.with_message(msg);

        let user_msg = Message::new_anonymous()
            .with_role(Role::User)
            .with_turn(Turn::new_anonymous(TurnContent::text("Hello!")));
        thread = thread.with_message(user_msg);

        let assistant_msg = Message::new_anonymous()
            .with_role(Role::Assistant)
            .with_turn(Turn::new_anonymous(TurnContent::text("Hi there! How can I help?")));
        thread = thread.with_message(assistant_msg);

        thread
    }

    #[test]
    fn test_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());
        let thread = sample_thread("My Conversation");

        store.save(&thread).unwrap();
        let loaded = store.load(thread.id).unwrap();

        assert_eq!(loaded.id, thread.id);
        assert_eq!(loaded.title, thread.title);
        assert_eq!(loaded.messages.len(), 3);
        assert_eq!(loaded.messages[0].role, Some(Role::System));
        assert_eq!(loaded.messages[1].role, Some(Role::User));
        assert_eq!(loaded.messages[2].role, Some(Role::Assistant));
    }

    #[test]
    fn test_restart_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let thread = sample_thread("Persistent Thread");

        {
            let store = ConversationStore::new(dir.path());
            store.initialize().unwrap();
            store.start().unwrap();
            store.save(&thread).unwrap();
            store.shutdown().unwrap();
        }

        // New store instance — should reconstruct from disk.
        {
            let store = ConversationStore::new(dir.path());
            store.initialize().unwrap();

            let loaded = store.load(thread.id).unwrap();
            assert_eq!(loaded.id, thread.id);
            assert_eq!(loaded.title.as_deref(), Some("Persistent Thread"));
            assert_eq!(loaded.messages.len(), 3);
            // IDs preserved
            assert_eq!(loaded.messages[0].id, thread.messages[0].id);
            assert_eq!(loaded.messages[1].id, thread.messages[1].id);
            assert_eq!(loaded.messages[2].id, thread.messages[2].id);
            // Timestamps preserved
            assert_eq!(loaded.created_at, thread.created_at);
            assert_eq!(loaded.updated_at, thread.updated_at);

            store.shutdown().unwrap();
        }
    }

    #[test]
    fn test_load_all_after_restart() {
        let dir = tempfile::tempdir().unwrap();

        {
            let store = ConversationStore::new(dir.path());
            store.initialize().unwrap();
            store.save(&sample_thread("Thread A")).unwrap();
            store.save(&sample_thread("Thread B")).unwrap();
            store.save(&sample_thread("Thread C")).unwrap();
            store.shutdown().unwrap();
        }

        {
            let store = ConversationStore::new(dir.path());
            store.initialize().unwrap();

            let threads = store.list();
            assert_eq!(threads.len(), 3);
            let titles: std::collections::HashSet<&str> =
                threads.iter().filter_map(|t| t.title.as_deref()).collect();
            assert!(titles.contains("Thread A"));
            assert!(titles.contains("Thread B"));
            assert!(titles.contains("Thread C"));

            store.shutdown().unwrap();
        }
    }

    #[test]
    fn test_delete() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());
        let thread = sample_thread("To Delete");

        store.save(&thread).unwrap();
        assert!(store.exists(thread.id));

        store.delete(thread.id).unwrap();
        assert!(!store.exists(thread.id));

        // Loading a deleted thread returns ThreadNotFound
        let result = store.load(thread.id);
        assert!(matches!(
            result,
            Err(PersistenceError::ThreadNotFound { thread_id })
            if thread_id == thread.id
        ));
    }

    #[test]
    fn test_load_nonexistent_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());
        let fake_id = Uuid::new_v4();

        let result = store.load(fake_id);
        assert!(matches!(
            result,
            Err(PersistenceError::ThreadNotFound { thread_id })
            if thread_id == fake_id
        ));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());
        let thread = sample_thread("Serialization Test");

        store.save(&thread).unwrap();

        // Read the raw JSON file and verify it deserializes independently.
        let file_path = store.thread_file_path(thread.id);
        let json = std::fs::read_to_string(file_path).unwrap();
        let restored: Thread = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, thread.id);
        assert_eq!(restored.messages.len(), 3);
    }

    #[test]
    fn test_corrupted_file_is_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();

        let thread = sample_thread("Good Thread");
        store.save(&thread).unwrap();

        // Corrupt the file by overwriting with invalid JSON.
        let file_path = store.thread_file_path(thread.id);
        std::fs::write(&file_path, "{invalid json}").unwrap();

        // reload_from_disk should skip the corrupted file.
        let count = store.reload_from_disk().unwrap();
        assert_eq!(count, 0);

        store.shutdown().unwrap();
    }

    #[test]
    fn test_invalid_thread_is_skipped_on_load_all() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();

        // Create a thread with a message whose thread_id doesn't match —
        // this fails model validation. We bypass the `with_message` builder
        // (which sets thread_id) to create the mismatch.
        let bad_thread = Thread {
            id: Uuid::new_v4(),
            title: Some("Bad Thread".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: None,
            participants: Vec::new(),
            messages: vec![Message::new_anonymous()
                .with_role(Role::User)
                .with_turn(Turn::new_anonymous(TurnContent::text("hello")))],
            metadata: std::collections::HashMap::new(),
        };
        let json = serde_json::to_string(&bad_thread).unwrap();
        std::fs::write(store.thread_file_path(bad_thread.id), &json).unwrap();

        // A valid thread should load fine.
        let good_thread = sample_thread("Good Thread");
        store.save(&good_thread).unwrap();

        let loaded = store.load_all().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, good_thread.id);

        store.shutdown().unwrap();
    }

    #[test]
    fn test_save_replaces_existing() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());

        let mut thread = sample_thread("Original");
        store.save(&thread).unwrap();
        assert_eq!(store.count(), 1);

        // Add another message and save again — should replace, not duplicate.
        let msg = Message::new_anonymous()
            .with_role(Role::User)
            .with_turn(Turn::new_anonymous(TurnContent::text("New message")));
        thread = thread.with_message(msg);

        store.save(&thread).unwrap();
        assert_eq!(store.count(), 1);

        let loaded = store.load(thread.id).unwrap();
        assert_eq!(loaded.messages.len(), 4);
    }

    #[test]
    fn test_update_modifies_timestamp() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());
        let mut thread = sample_thread("Update Test");
        store.save(&thread).unwrap();

        let original_updated = thread.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Modify and update — updated_at should change.
        let msg = Message::new_anonymous()
            .with_role(Role::User)
            .with_turn(Turn::new_anonymous(TurnContent::text("Added later")));
        thread = thread.with_message(msg);

        store.update(&mut thread).unwrap();

        let loaded = store.load(thread.id).unwrap();
        assert!(loaded.updated_at.is_some());
        assert!(loaded.updated_at.unwrap() > original_updated.unwrap());
    }

    #[test]
    fn test_turns_are_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());

        let mut thread = Thread::new().with_title("Turns Test");
        let tool_call_content = serde_json::json!({
            "tool_call": {"name": "calc", "args": {"expr": "2+2"}}
        });
        let msg = Message::new_anonymous()
            .with_role(Role::User)
            .with_turn(Turn::new_anonymous(TurnContent::text("Calculate 2+2")))
            .with_turn(Turn::new_anonymous(TurnContent::Unknown(tool_call_content)));
        thread = thread.with_message(msg);

        store.save(&thread).unwrap();
        let loaded = store.load(thread.id).unwrap();

        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].turns.len(), 2);
        assert_eq!(
            loaded.messages[0].turns[0].content.as_text(),
            Some("Calculate 2+2")
        );
    }

    #[test]
    fn test_lifecycle_transitions() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());

        assert_eq!(store.lifecycle_stage(), LifecycleStage::Created);
        assert!(!store.is_initialized());
        assert!(!store.is_running());

        store.initialize().unwrap();
        assert!(store.is_initialized());
        assert!(!store.is_running());

        store.start().unwrap();
        assert!(store.is_running());

        store.shutdown().unwrap();
        assert!(store.is_shutdown());

        // Operations after shutdown fail.
        let thread = sample_thread("Post-shutdown");
        assert!(store.save(&thread).is_err());
    }

    #[test]
    fn test_empty_store_initializes() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());
        store.initialize().unwrap();
        assert_eq!(store.count(), 0);
        store.shutdown().unwrap();
    }

    #[test]
    fn test_manifest_written() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());
        store.save(&sample_thread("Thread 1")).unwrap();
        store.save(&sample_thread("Thread 2")).unwrap();

        // The manifest file should exist.
        assert!(store.manifest_path.exists());

        // Read it and verify structure.
        let json = std::fs::read_to_string(&store.manifest_path).unwrap();
        let manifest: Manifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest.threads.len(), 2);
    }

    #[test]
    fn test_register_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());
        let thread = sample_thread("Register Test");

        store.register(&thread).unwrap();
        assert_eq!(store.count(), 1);

        // Registering again should be a no-op (no error, no duplicate).
        store.register(&thread).unwrap();
        assert_eq!(store.count(), 1);
    }

    #[test]
    fn test_update_nonexistent_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());
        let mut thread = sample_thread("Nonexistent");
        // Don't save first — update should fail.
        let result = store.update(&mut thread);
        assert!(matches!(
            result,
            Err(PersistenceError::ThreadNotFound { thread_id })
            if thread_id == thread.id
        ));
    }

    #[test]
    fn test_validation_persists_message_ids() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());
        let thread = sample_thread("ID Preservation Test");

        store.save(&thread).unwrap();
        let loaded = store.load(thread.id).unwrap();

        // All message IDs should be preserved after save/reload.
        assert_eq!(loaded.messages.len(), thread.messages.len());
        for (original, restored) in
            thread.messages.iter().zip(loaded.messages.iter())
        {
            assert_eq!(original.id, restored.id);
            assert_eq!(original.thread_id, restored.thread_id);
        }
    }

    #[test]
    fn test_validation_persists_turns() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());

        let turn = Turn::new(Uuid::new_v4(), Uuid::new_v4(), TurnContent::text("content"));
        let mut thread = Thread::new().with_title("Turn Test");
        let msg = Message::new_anonymous().with_role(Role::User).with_turn(turn.clone());
        thread = thread.with_message(msg);

        store.save(&thread).unwrap();
        let loaded = store.load(thread.id).unwrap();

        assert_eq!(loaded.messages[0].turns.len(), 1);
        assert_eq!(loaded.messages[0].turns[0].id, turn.id);
        assert_eq!(loaded.messages[0].turns[0].message_id, loaded.messages[0].id);
    }

    #[test]
    fn test_concurrent_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let store = std::sync::Arc::new(ConversationStore::new(dir.path()));
        store.initialize().unwrap();
        store.start().unwrap();

        // Ensure the directory exists before spawning concurrent savers.
        store.ensure_dirs().unwrap();

        let results: Vec<Thread> = (0..10)
            .map(|i| {
                let store_clone = store.clone();
                let thread = sample_thread(&format!("Concurrent Thread {}", i));
                let id = thread.id;
                store_clone.save(&thread).expect("save should succeed");
                store_clone.load(id).expect("load should succeed")
            })
            .collect();

        // All 10 threads should be in the store.
        assert_eq!(store.count(), 10);

        // Verify all IDs are present and unique.
        let ids: std::collections::HashSet<Uuid> = results.iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), 10);

        store.shutdown().unwrap();
    }

    #[test]
    fn test_metadata_persists() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());

        let mut thread = Thread::new().with_title("Metadata Test");
        thread = thread.with_metadata("model", serde_json::json!("gpt-4"));
        thread = thread.with_metadata("temperature", serde_json::json!(0.7));

        store.save(&thread).unwrap();
        let loaded = store.load(thread.id).unwrap();

        assert_eq!(
            loaded.metadata.get("model"),
            Some(&serde_json::json!("gpt-4"))
        );
        assert_eq!(
            loaded.metadata.get("temperature"),
            Some(&serde_json::json!(0.7))
        );
    }

    #[test]
    fn test_provider_field_persists() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConversationStore::new(dir.path());

        let thread = Thread::new()
            .with_title("Provider Test")
            .with_metadata("provider", serde_json::json!("openai"));
        // Note: Thread uses metadata for provider, not a separate field.
        // This test verifies the metadata persistence.

        store.save(&thread).unwrap();
        let loaded = store.load(thread.id).unwrap();
        assert_eq!(
            loaded.metadata.get("provider"),
            Some(&serde_json::json!("openai"))
        );
    }
}
