use std::path::PathBuf;
use crate::view_state::ViewStateManager;
use crate::indexer::Indexer;

pub struct VaultSession {
    pub vault_id: String,
    pub vault_path: PathBuf,
    pub view_state_manager: ViewStateManager,
    pub indexer: Indexer,
    pub is_active: bool,
}

impl VaultSession {
    pub fn new(vault_id: String, vault_path: PathBuf) -> Self {
        let view_state_manager = ViewStateManager::new(vault_path.clone());
        let indexer = Indexer::new(vault_path.join(".nabu/index")).expect("Failed to initialize indexer");
        Self {
            vault_id,
            vault_path,
            view_state_manager,
            indexer,
            is_active: true,
        }
    }
}
