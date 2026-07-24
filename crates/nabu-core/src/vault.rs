use crate::view_state::ViewStateManager;
use std::path::PathBuf;

pub struct VaultSession {
    pub vault_id: String,
    pub vault_path: PathBuf,
    pub view_state_manager: ViewStateManager,
    pub is_active: bool,
}

impl VaultSession {
    pub fn new(vault_id: String, vault_path: PathBuf) -> Self {
        let view_state_manager = ViewStateManager::new(vault_path.clone());
        Self {
            vault_id,
            vault_path,
            view_state_manager,
            is_active: true,
        }
    }
}
