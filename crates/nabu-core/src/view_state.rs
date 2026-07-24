use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct ViewState {
    pub fold_states: HashMap<String, bool>,
}

pub struct ViewStateManager {
    vault_path: PathBuf,
}

impl ViewStateManager {
    pub fn new(vault_path: PathBuf) -> Self {
        Self { vault_path }
    }

    fn get_view_state_dir(&self) -> PathBuf {
        self.vault_path.join(".nabu").join("view-state")
    }

    fn get_view_state_file(&self, note_path: &str) -> PathBuf {
        // Simple mapping of notePath to filename, similar to TS version
        // In TS: join(viewStateDir, encodeURIComponent(notePath) + '.json')
        // For Rust, let's just use a simple hash or sanitize.
        let safe_name = note_path.replace("/", "_").replace("\\", "_");
        self.get_view_state_dir().join(format!("{}.json", safe_name))
    }

    pub async fn load_view_state(&self, note_path: &str) -> ViewState {
        let path = self.get_view_state_file(note_path);
        if let Ok(content) = fs::read_to_string(path).await {
            if let Ok(state) = serde_json::from_str(&content) {
                return state;
            }
        }
        ViewState::default()
    }

    pub async fn save_view_state(&self, note_path: &str, state: &ViewState) -> anyhow::Result<()> {
        let dir = self.get_view_state_dir();
        if !dir.exists() {
            fs::create_dir_all(&dir).await?;
        }
        let path = self.get_view_state_file(note_path);
        let content = serde_json::to_string_pretty(state)?;
        fs::write(path, content).await?;
        Ok(())
    }
}
