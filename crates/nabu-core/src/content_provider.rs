//! ContentProvider abstraction for loading document text without owning it.
//!
//! # Architecture
//!
//! This module bridges the gap between Markdown-as-canonical (Principle 1) and
//! the Indexer's need for searchable text (Principle 6).
//!
//! ## Who owns content?
//!
//! - **Markdown files on disk** own the canonical content (Principle 1).
//! - **KnowledgeObject** carries only a content *descriptor* (format type), not
//!   the actual bytes. It is intentionally a lightweight runtime representation.
//! - **ContentProvider** reads content on demand when the Indexer or a processor
//!   needs it. It is *not* a storage layer — it's a loading abstraction.
//! - **No subsystem** ever stores document text. Content is always read from the
//!   canonical Markdown file when needed and discarded afterward.
//!
//! ## Why not put content in KnowledgeObject?
//!
//! Putting full Markdown text inside every KnowledgeObject would:
//! - Violate Principle 1 (duplicate canonical data into memory)
//! - Double memory usage (every KO in memory would carry the full document)
//! - Break Obsidian/Finder compatibility (the vault file IS the source)
//! - Force IPC to serialize massive payloads
//!
//! ## Usage
//!
//! ```ignore
//! use nabu_core::content_provider::{ContentProvider, FilesystemContentProvider};
//!
//! let provider = FilesystemContentProvider;
//! let text = provider.load_text(&knowledge_object);
//! ```

use crate::models::knowledge_object::KnowledgeObject;
use std::borrow::Cow;
use std::fmt::Debug;

/// Trait for loading text content of a KnowledgeObject on demand.
///
/// Implementations read from the canonical source (typically the filesystem)
/// without storing or duplicating the content.
///
/// # Architecture
///
/// - Markdown is the canonical source of truth (Principle 1).
/// - ContentProvider loads text *at indexing time only*.
/// - Content is never owned by KnowledgeObject.
/// - Structured content (JSON) is serialized inline.
/// - Binary content returns empty.
pub trait ContentProvider: Debug + Send + Sync {
    /// Load the full text content of a knowledge object.
    ///
    /// For `Markdown` / `PlainText` / `Html` content with a `source_file`,
    /// this reads from the filesystem. For `Structured` content, the JSON
    /// value is serialised inline. For `Binary` content, returns empty.
    fn load_text(&self, object: &KnowledgeObject) -> String;
}

/// Reads text content from the filesystem using the KnowledgeObject's
/// `metadata.source_file` path.
///
/// This is the default provider for vault-backed objects where the
/// Markdown file exists on disk.
///
/// # Behaviour by content type
///
/// | Content type | Behaviour |
/// |-------------|-----------|
/// | `Markdown`  | Reads `source_file` from disk, returns full text |
/// | `PlainText` | Reads `source_file` from disk, returns full text |
/// | `Html`      | Reads `source_file` from disk, returns full text |
/// | `Structured`| Serialises the JSON value inline (no disk read) |
/// | `Binary`    | Returns empty string |
#[derive(Debug, Clone, Copy)]
pub struct FilesystemContentProvider;

impl ContentProvider for FilesystemContentProvider {
    fn load_text(&self, object: &KnowledgeObject) -> String {
        match &object.content {
            crate::models::knowledge_object::ObjectContent::Structured(json) => {
                // Structured content is serialised inline — it's not a
                // user-authored Markdown file on disk.
                serde_json::to_string(json).unwrap_or_default()
            }
            crate::models::knowledge_object::ObjectContent::Binary => {
                // Binary content has no text representation.
                String::new()
            }
            // Markdown, PlainText, Html: content lives on disk.
            // Read from source_file if available.
            _ => {
                match &object.metadata.source_file {
                    Some(path) => {
                        std::fs::read_to_string(path).unwrap_or_else(|_| {
                            // File may not exist yet (in-flight capture) or
                            // may have been deleted. Return empty — the
                            // indexer handles this gracefully.
                            String::new()
                        })
                    }
                    None => {
                        // No source file — content was never persisted to disk.
                        // This can happen for transient or in-memory objects.
                        String::new()
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::knowledge_object::{
        KnowledgeObject, ObjectContent, ObjectMetadata, ObjectType,
    };
    use std::collections::HashMap;
    use uuid::Uuid;

    fn create_object(path: Option<&str>, content: ObjectContent) -> KnowledgeObject {
        KnowledgeObject {
            id: Uuid::new_v4(),
            object_type: ObjectType::Note,
            vault_id: "test-vault".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            modified_at: "2024-06-01T00:00:00Z".to_string(),
            content,
            metadata: ObjectMetadata {
                source_file: path.map(|s| s.to_string()),
                ..Default::default()
            },
        }
    }

    #[test]
    fn filesystem_provider_returns_empty_for_missing_file() {
        let provider = FilesystemContentProvider;
        let obj = create_object(
            Some("/tmp/nonexistent-file-for-testing.md"),
            ObjectContent::Markdown,
        );
        let text = provider.load_text(&obj);
        // File doesn't exist, should return empty string gracefully
        assert_eq!(text, "");
    }

    #[test]
    fn filesystem_provider_returns_empty_for_binary() {
        let provider = FilesystemContentProvider;
        let obj = create_object(Some("/path/to/file.bin"), ObjectContent::Binary);
        let text = provider.load_text(&obj);
        assert_eq!(text, "");
    }

    #[test]
    fn filesystem_provider_returns_empty_for_no_source_file() {
        let provider = FilesystemContentProvider;
        let obj = create_object(None, ObjectContent::PlainText);
        let text = provider.load_text(&obj);
        assert_eq!(text, "");
    }

    #[test]
    fn filesystem_provider_serializes_structured_content() {
        let provider = FilesystemContentProvider;
        let obj = create_object(
            None,
            ObjectContent::Structured(serde_json::json!({"key": "value"})),
        );
        let text = provider.load_text(&obj);
        assert!(text.contains("key"));
        assert!(text.contains("value"));
    }

    #[test]
    fn load_text_with_existing_file() {
        let provider = FilesystemContentProvider;
        // Create a temp file
        let dir = std::env::temp_dir().join("nabu-content-provider-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test-note.md");
        std::fs::write(&path, b"# Hello, world!\n\nThis is a test.").unwrap();

        let obj = create_object(
            Some(path.to_str().unwrap()),
            ObjectContent::Markdown,
        );
        let text = provider.load_text(&obj);
        assert_eq!(text, "# Hello, world!\n\nThis is a test.");

        // Cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_text_plain_text_from_file() {
        let provider = FilesystemContentProvider;
        let dir = std::env::temp_dir().join("nabu-content-provider-test-pt");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("plain.txt");
        std::fs::write(&path, b"Plain text content").unwrap();

        let obj = create_object(
            Some(path.to_str().unwrap()),
            ObjectContent::PlainText,
        );
        let text = provider.load_text(&obj);
        assert_eq!(text, "Plain text content");

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
