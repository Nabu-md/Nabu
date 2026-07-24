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

    pub fn export_to_html(&self, note_path: &Path, output_path: &Path, template_name: &str) -> Result<()> {
        let content = std::fs::read_to_string(note_path)?;
        let html_content = crate::parser::parse_markdown_to_html(&content);
        
        let mut tera = tera::Tera::new(&format!("{}/.nabu/templates/**/*", self.vault_root.display()))?;
        let mut context = tera::Context::new();
        context.insert("content", &html_content);
        context.insert("title", note_path.file_name().unwrap().to_str().unwrap());
        
        let rendered = tera.render(&format!("{}.html", template_name), &context)?;
        std::fs::write(output_path, rendered)?;
        Ok(())
    }
}
