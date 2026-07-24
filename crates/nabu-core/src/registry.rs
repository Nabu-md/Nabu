use crate::vault::VaultSession;
use std::collections::HashMap;

pub struct VaultRegistry {
    sessions: HashMap<String, VaultSession>,
}

impl VaultRegistry {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn register_vault(&mut self, vault_id: String, vault_path: std::path::PathBuf) {
        let session = VaultSession::new(vault_id.clone(), vault_path);
        self.sessions.insert(vault_id, session);
    }

    pub fn get_session(&self, vault_id: &str) -> Option<&VaultSession> {
        self.sessions.get(vault_id)
    }
}
