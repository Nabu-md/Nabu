use std::path::{Path, PathBuf};
use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportOptions {
    pub include_graph: bool,
    pub theme: String,
}

pub struct ExportEngine {
    vault_root: PathBuf,
}

impl ExportEngine {
    pub fn new(vault_root: PathBuf) -> Self {
        Self { vault_root }
    }

    pub fn export_to_html(&self, note_path: &Path, output_path: &Path) -> Result<()> {
        let content = std::fs::read_to_string(note_path)?;
        let html = crate::parser::parse_markdown_to_html(&content);
        
        let full_html = format!(
            "<html><body>{}</body></html>",
            html
        );
        
        std::fs::write(output_path, full_html)?;
        Ok(())
    }
}
