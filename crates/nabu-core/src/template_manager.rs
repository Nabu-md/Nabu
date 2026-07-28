use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::models::template::Template;

pub struct TemplateManager {
    root: PathBuf,
}

impl TemplateManager {
    pub fn new(vault_root: &Path) -> Self {
        Self {
            root: vault_root.join(".nabu/templates"),
        }
    }

    pub fn load_template(&self, name: &str) -> Result<Template> {
        let path = self.root.join(format!("{}.md", name));
        let content = std::fs::read_to_string(path)?;
        
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() < 3 {
            // Legacy template: no frontmatter
            return Ok(Template {
                name: name.to_string(),
                description: None,
                icon: None,
                default_folder: None,
                frontmatter_defaults: HashMap::new(),
                property_presets: HashMap::new(),
                body: content.trim().to_string(),
                object_type: None,
            });
        }
        let frontmatter = parts[1];
        let body = parts[2].trim().to_string();
        
        let mut template: Template = serde_yaml::from_str(frontmatter)?;
        template.body = body;
        Ok(template)
    }
}
