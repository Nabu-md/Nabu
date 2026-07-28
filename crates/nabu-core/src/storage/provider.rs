/// Storage provider abstraction for the Storage Manager.
///
/// This trait defines the interface for storage backends, allowing future
/// implementations (PostgreSQL, Cloud Sync, in-memory testing store) without
/// changing higher-level code.

use anyhow::Result;

/// Trait defining the storage provider interface.
///
/// Implementations of this trait provide persistence for knowledge objects
/// and related metadata. The trait is intentionally minimal for Phase 1.3A,
/// focusing only on basic database initialization and connection management.
pub trait StorageProvider: Send + Sync {
    /// Initialize the storage backend.
    ///
    /// This method should create necessary directories and database files
    /// if they do not exist. It should never overwrite existing data.
    fn initialize(&self) -> Result<()>;

    /// Check if the storage backend is initialized and ready.
    fn is_initialized(&self) -> bool;
}