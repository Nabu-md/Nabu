use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeSettings {
    pub theme_name: String,
}

pub struct ThemeManager {
    config_path: PathBuf,
}

impl ThemeManager {
    pub fn new(vault_root: PathBuf) -> Self {
        Self { config_path: vault_root.join(".nabu/theme.json") }
    }

    pub fn set_theme(&self, theme: &str) -> Result<()> {
        let settings = ThemeSettings { theme_name: theme.into() };
        std::fs::write(&self.config_path, serde_json::to_string(&settings)?)?;
        Ok(())
    }

    pub fn get_theme(&self) -> Result<String> {
        let content = std::fs::read_to_string(&self.config_path)?;
        let settings: ThemeSettings = serde_json::from_str(&content)?;
        Ok(settings.theme_name)
    }
}
