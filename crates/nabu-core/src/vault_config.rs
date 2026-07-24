use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    pub name: String,
    pub path: PathBuf,
}

impl VaultConfig {
    pub fn initialize_vault(path: PathBuf, name: String) -> Result<()> {
        if !path.is_dir() {
            return anyhow::bail!("Path is not a directory: {:?}", path);
        }
        let nabu_dir = path.join(".nabu");
        if nabu_dir.exists() {
            return anyhow::bail!("Vault already initialized at: {:?}", path);
        }
        std::fs::create_dir_all(nabu_dir.join("index"))?;
        
        let config = VaultConfig { name, path: path.clone() };
        let config_path = nabu_dir.join("config.json");
        std::fs::write(config_path, serde_json::to_string_pretty(&config)?)?;
        
        Ok(())
    }
}
