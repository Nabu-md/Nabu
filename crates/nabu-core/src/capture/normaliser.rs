use std::fs;
use std::path::Path;

use crate::capture::{CaptureError, IngestionOptions, IngestionRequest};

/// Converts raw file input into a canonical [`IngestionRequest`].
///
/// The normaliser handles:
/// - file validation
/// - MIME type detection
/// - byte reading
///
/// It does not parse, enrich, or transform content. Those responsibilities
/// belong to the [`IngestionPipeline`] and downstream processors.
///
/// The normaliser is source-agnostic: it accepts a `source` identifier so
/// that any handler (file drop, watch folder, API webhook, etc.) can use it
/// without modification.
pub struct Normaliser;

impl Normaliser {
    /// Normalizes a file path into an [`IngestionRequest`].
    ///
    /// # Arguments
    ///
    /// * `source` - The capture source identifier (e.g., "file_drop", "watch_folder").
    /// * `file_path` - Path to the file to normalize.
    /// * `vault_id` - Target vault identifier.
    /// * `source_file` - Original source file path, if applicable.
    /// * `options` - Ingestion options controlling processing behaviour.
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError`] if the file is invalid, unreadable, or if MIME
    /// detection fails.
    pub fn normalize(
        &self,
        source: &str,
        file_path: &Path,
        vault_id: &str,
        source_file: Option<String>,
        options: IngestionOptions,
    ) -> Result<IngestionRequest, CaptureError> {
        self.validate_file(file_path)?;
        let raw_bytes = self.read_file(file_path)?;
        let mime_type = self.detect_mime_type(file_path);

        Ok(IngestionRequest {
            source: source.to_string(),
            raw_bytes,
            mime_type,
            vault_id: vault_id.to_string(),
            source_file,
            options,
        })
    }

    fn validate_file(&self, path: &Path) -> Result<(), CaptureError> {
        if !path.exists() {
            return Err(CaptureError::InvalidFile(format!(
                "File does not exist: {}",
                path.display()
            )));
        }
        if !path.is_file() {
            return Err(CaptureError::InvalidFile(format!(
                "Path is not a file: {}",
                path.display()
            )));
        }
        Ok(())
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, CaptureError> {
        fs::read(path).map_err(|e| CaptureError::ReadFailed(format!("{}: {}", path.display(), e)))
    }

    fn detect_mime_type(&self, path: &Path) -> String {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match extension.as_deref() {
            Some("txt") => "text/plain".to_string(),
            Some("md") => "text/markdown".to_string(),
            Some("html") | Some("htm") => "text/html".to_string(),
            Some("pdf") => "application/pdf".to_string(),
            Some("jpg") | Some("jpeg") => "image/jpeg".to_string(),
            Some("png") => "image/png".to_string(),
            Some("gif") => "image/gif".to_string(),
            Some("webp") => "image/webp".to_string(),
            Some("svg") => "image/svg+xml".to_string(),
            Some("mp3") => "audio/mpeg".to_string(),
            Some("wav") => "audio/wav".to_string(),
            Some("mp4") => "video/mp4".to_string(),
            Some("mov") => "video/quicktime".to_string(),
            Some("zip") => "application/zip".to_string(),
            Some("json") => "application/json".to_string(),
            Some("xml") => "application/xml".to_string(),
            Some("csv") => "text/csv".to_string(),
            Some("doc") => "application/msword".to_string(),
            Some("docx") => {
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                    .to_string()
            }
            Some("xls") => "application/vnd.ms-excel".to_string(),
            Some("xlsx") => {
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string()
            }
            Some("ppt") => "application/vnd.ms-powerpoint".to_string(),
            Some("pptx") => {
                "application/vnd.openxmlformats-officedocument.presentationml.presentation"
                    .to_string()
            }
            _ => "application/octet-stream".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_text_file() {
        let dir = std::env::temp_dir().join("nabu_capture_test");
        let _ = fs::create_dir_all(&dir);
        let file_path = dir.join("test.txt");
        fs::write(&file_path, "Hello, Nabu!").unwrap();

        let normaliser = Normaliser;
        let result = normaliser.normalize(
            "file_drop",
            &file_path,
            "vault-1",
            file_path.to_str().map(|s| s.to_string()),
            IngestionOptions::default(),
        );

        assert!(result.is_ok());
        let request = result.unwrap();
        assert_eq!(request.source, "file_drop");
        assert_eq!(request.mime_type, "text/plain");
        assert_eq!(request.raw_bytes, b"Hello, Nabu!");
        assert_eq!(request.vault_id, "vault-1");
        assert_eq!(
            request.source_file,
            file_path.to_str().map(|s| s.to_string())
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_markdown_file() {
        let dir = std::env::temp_dir().join("nabu_capture_test_md");
        let _ = fs::create_dir_all(&dir);
        let file_path = dir.join("note.md");
        fs::write(&file_path, "# Title\n\nBody").unwrap();

        let normaliser = Normaliser;
        let result = normaliser.normalize(
            "file_drop",
            &file_path,
            "vault-1",
            file_path.to_str().map(|s| s.to_string()),
            IngestionOptions::default(),
        );

        assert!(result.is_ok());
        let request = result.unwrap();
        assert_eq!(request.mime_type, "text/markdown");
        assert_eq!(
            request.source_file,
            file_path.to_str().map(|s| s.to_string())
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_pdf_file() {
        let dir = std::env::temp_dir().join("nabu_capture_test_pdf");
        let _ = fs::create_dir_all(&dir);
        let file_path = dir.join("doc.pdf");
        fs::write(&file_path, b"%PDF-1.4 fake pdf").unwrap();

        let normaliser = Normaliser;
        let result = normaliser.normalize(
            "file_drop",
            &file_path,
            "vault-1",
            file_path.to_str().map(|s| s.to_string()),
            IngestionOptions::default(),
        );

        assert!(result.is_ok());
        let request = result.unwrap();
        assert_eq!(request.mime_type, "application/pdf");
        assert_eq!(
            request.source_file,
            file_path.to_str().map(|s| s.to_string())
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_binary_file() {
        let dir = std::env::temp_dir().join("nabu_capture_test_bin");
        let _ = fs::create_dir_all(&dir);
        let file_path = dir.join("data.bin");
        fs::write(&file_path, [0u8, 255u8, 128u8]).unwrap();

        let normaliser = Normaliser;
        let result = normaliser.normalize(
            "file_drop",
            &file_path,
            "vault-1",
            file_path.to_str().map(|s| s.to_string()),
            IngestionOptions::default(),
        );

        assert!(result.is_ok());
        let request = result.unwrap();
        assert_eq!(request.mime_type, "application/octet-stream");
        assert_eq!(request.raw_bytes, &[0u8, 255u8, 128u8]);
        assert_eq!(
            request.source_file,
            file_path.to_str().map(|s| s.to_string())
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_missing_file() {
        let normaliser = Normaliser;
        let result = normaliser.normalize(
            "file_drop",
            Path::new("/nonexistent/path/file.txt"),
            "vault-1",
            None,
            IngestionOptions::default(),
        );

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CaptureError::InvalidFile(_)));
    }

    #[test]
    fn normalize_directory_path() {
        let dir = std::env::temp_dir().join("nabu_capture_test_dir");
        let _ = fs::create_dir_all(&dir);

        let normaliser = Normaliser;
        let result = normaliser.normalize(
            "file_drop",
            &dir,
            "vault-1",
            None,
            IngestionOptions::default(),
        );

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CaptureError::InvalidFile(_)));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_unicode_path() {
        let dir = std::env::temp_dir().join("nabu_capture_test_unicode_日本語");
        let _ = fs::create_dir_all(&dir);
        let file_path = dir.join("ファイル.txt");
        fs::write(&file_path, "Unicode content").unwrap();

        let normaliser = Normaliser;
        let result = normaliser.normalize(
            "file_drop",
            &file_path,
            "vault-1",
            file_path.to_str().map(|s| s.to_string()),
            IngestionOptions::default(),
        );

        assert!(result.is_ok());
        let request = result.unwrap();
        assert_eq!(request.raw_bytes, b"Unicode content");
        assert_eq!(
            request.source_file,
            file_path.to_str().map(|s| s.to_string())
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_preserves_ingestion_options() {
        let dir = std::env::temp_dir().join("nabu_capture_test_opts");
        let _ = fs::create_dir_all(&dir);
        let file_path = dir.join("opt.txt");
        fs::write(&file_path, "options").unwrap();

        let mut custom = std::collections::HashMap::new();
        custom.insert("key".to_string(), serde_json::json!("value"));

        let options = IngestionOptions {
            create_knowledge_object: false,
            extract_metadata: true,
            custom,
        };

        let normaliser = Normaliser;
        let result = normaliser.normalize(
            "file_drop",
            &file_path,
            "vault-1",
            file_path.to_str().map(|s| s.to_string()),
            options.clone(),
        );

        assert!(result.is_ok());
        let request = result.unwrap();
        assert_eq!(request.options, options);

        let _ = fs::remove_dir_all(&dir);
    }
}
