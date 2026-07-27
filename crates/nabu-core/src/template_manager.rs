use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub struct TemplateManager {
    root: PathBuf,
}

impl TemplateManager {
    pub fn new(vault_root: &Path) -> Self {
        Self {
            root: vault_root.join(".nabu/templates"),
        }
    }

    pub fn get_template(&self, name: &str) -> Result<String> {
        let path = self.root.join(format!("{}.md", name));
        std::fs::read_to_string(path).context("Failed to read template")
    }
}
