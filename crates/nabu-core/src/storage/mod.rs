/// Storage module for the Storage Manager.
///
/// This module provides the persistence layer for knowledge objects,
/// including the StorageManager, StorageProvider trait, and SQLite implementation.

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
    use tempfile::tempdir;

    #[test]
    fn storage_manager_initializes_new_database() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let vault_path = temp_dir.path().to_path_buf();

        let manager = StorageManager::new(vault_path.clone());
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
        let manager1 = StorageManager::new(vault_path.clone());
        manager1.initialize().expect("Failed to initialize storage");

        // Get the database path
        let db_path = manager1.db_path().clone();

        // Second initialization should succeed without error
        let manager2 = StorageManager::new(vault_path.clone());
        manager2.initialize().expect("Failed to re-initialize storage");

        // Database should still exist
        assert!(db_path.exists());
    }

    #[test]
    fn storage_manager_creates_nabu_db_directory() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let vault_path = temp_dir.path().to_path_buf();

        let manager = StorageManager::new(vault_path.clone());
        manager.initialize().expect("Failed to initialize storage");

        let nabu_db_path = vault_path.join(".nabu").join("db");
        assert!(nabu_db_path.exists());
        assert!(nabu_db_path.is_dir());
    }
}