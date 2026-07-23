use std::path::Path;

use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettings {
    pub theme: String,
    pub last_vault_path: String,
    #[serde(default)]
    pub recent_vaults: Vec<RecentVaultEntry>,
}

/// Metadata describing a single file in a vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub name: String,
}

/// Represents a recognized text region.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRegion {
    pub text: String,
}

/// Result of OCR processing for an image.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OcrResult {
    pub regions: Vec<OcrRegion>,
}

#[derive(Debug, Error)]
pub enum OcrError {
    #[error("image missing")]
    Missing,
}

/// Native OCR engine abstraction.
///
/// The real implementation may use platform-native bindings once confirmed.
pub trait OcrEngine {
    fn extract_text(&self, image_path: &str) -> Result<OcrResult, OcrError>;
}

/// Defensive no-op engine used when native OCR is unavailable.
#[derive(Debug, Default)]
pub struct NoopOcrEngine;

impl OcrEngine for NoopOcrEngine {
    fn extract_text(&self, image_path: &str) -> Result<OcrResult, OcrError> {
        let path = Path::new(image_path);
        if path.as_os_str().is_empty() || !path.exists() {
            return Err(OcrError::Missing);
        }

        // The production engine will actually perform OCR here.
        Ok(OcrResult::default())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn noop_engine_compiles() {
        let engine = NoopOcrEngine::default();
        let tmp = std::env::temp_dir().join("nabu-ocr-noop-test.txt");
        fs::write(&tmp, b"placeholder").unwrap();

        let result = engine.extract_text(tmp.to_str().unwrap()).unwrap();
        assert!(result.regions.is_empty());

        let _ = fs::remove_file(tmp);
    }
}
