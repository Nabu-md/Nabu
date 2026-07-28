use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The universal knowledge object representing any item captured by Nabu.
///
/// Every entity in the system—notes, documents, people, projects, recordings,
/// and plugin-defined types—is represented as a `KnowledgeObject`.
///
/// This struct is intentionally free of business logic. It serves as the
/// foundational serialization contract for IPC, storage, and downstream subsystems.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[non_exhaustive]
pub struct KnowledgeObject {
    /// Unique identifier for this knowledge object.
    pub id: Uuid,
    /// The category of knowledge this object represents.
    pub object_type: ObjectType,
    /// The vault this object belongs to.
    pub vault_id: String,
    /// ISO 8601 timestamp of when this object was created.
    pub created_at: String,
    /// ISO 8601 timestamp of when this object was last modified.
    pub modified_at: String,
    /// The primary content of this knowledge object.
    pub content: ObjectContent,
    /// Non-content metadata describing this knowledge object.
    pub metadata: ObjectMetadata,
}

/// Metadata describing a knowledge object, excluding its primary content.
///
/// Metadata powers search, OCR, AI enrichment, graph enrichment, duplicate
/// detection, timeline extraction, and document management.
///
/// Processors may safely append new fields to `custom` without schema redesign.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[non_exhaustive]
pub struct ObjectMetadata {
    /// Human-readable title of the object.
    pub title: Option<String>,
    /// Author or creator of the object.
    pub author: Option<String>,
    /// Language code (e.g., "en", "fr", "de").
    pub language: Option<String>,
    /// Original source URL if the object was captured from the web.
    pub source_url: Option<String>,
    /// Original source file path if the object was imported from a file.
    pub source_file: Option<String>,
    /// MIME type of the original source (e.g., "application/pdf").
    pub mime_type: Option<String>,
    /// Number of pages, applicable to paginated documents.
    pub page_count: Option<u32>,
    /// Number of words in the object content.
    pub word_count: Option<u32>,
    /// ISO 8601 timestamp of when the source was created.
    pub created: Option<String>,
    /// ISO 8601 timestamp of when the source was last modified.
    pub modified: Option<String>,
    /// Arbitrary plugin-defined metadata.
    ///
    /// Processors may insert any key-value pairs here without requiring
    /// schema changes. Values should be JSON-compatible.
    pub custom: HashMap<String, serde_json::Value>,
}

/// The category of a knowledge object.
///
/// New variants may be added in future phases without breaking serialization.
/// Plugin-defined types should use `Custom(String)`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ObjectType {
    #[default]
    Note,
    Document,
    Pdf,
    Image,
    Receipt,
    Invoice,
    Meeting,
    Person,
    Organisation,
    Project,
    ResearchPaper,
    Book,
    Course,
    Website,
    Bookmark,
    Repository,
    AudioRecording,
    Video,
    Scan,
    Screenshot,
    Attachment,
    Contract,
    Article,
    Resume,
    Presentation,
    Manual,
    Letter,
    /// A plugin-defined or otherwise unrecognised object type.
    Custom(String),
}

/// The primary content format of a knowledge object.
///
/// New variants may be added in future phases without breaking serialization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ObjectContent {
    /// Markdown formatted text.
    #[default]
    Markdown,
    /// Unformatted plain text.
    PlainText,
    /// HTML content.
    Html,
    /// Opaque binary data.
    Binary,
    /// Structured data in JSON format.
    Structured(serde_json::Value),
}

impl std::fmt::Display for ObjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectType::Note => write!(f, "note"),
            ObjectType::Document => write!(f, "document"),
            ObjectType::Pdf => write!(f, "pdf"),
            ObjectType::Image => write!(f, "image"),
            ObjectType::Receipt => write!(f, "receipt"),
            ObjectType::Invoice => write!(f, "invoice"),
            ObjectType::Meeting => write!(f, "meeting"),
            ObjectType::Person => write!(f, "person"),
            ObjectType::Organisation => write!(f, "organisation"),
            ObjectType::Project => write!(f, "project"),
            ObjectType::ResearchPaper => write!(f, "research_paper"),
            ObjectType::Book => write!(f, "book"),
            ObjectType::Course => write!(f, "course"),
            ObjectType::Website => write!(f, "website"),
            ObjectType::Bookmark => write!(f, "bookmark"),
            ObjectType::Repository => write!(f, "repository"),
            ObjectType::AudioRecording => write!(f, "audio_recording"),
            ObjectType::Video => write!(f, "video"),
            ObjectType::Scan => write!(f, "scan"),
            ObjectType::Screenshot => write!(f, "screenshot"),
            ObjectType::Attachment => write!(f, "attachment"),
            ObjectType::Contract => write!(f, "contract"),
            ObjectType::Article => write!(f, "article"),
            ObjectType::Resume => write!(f, "resume"),
            ObjectType::Presentation => write!(f, "presentation"),
            ObjectType::Manual => write!(f, "manual"),
            ObjectType::Letter => write!(f, "letter"),
            ObjectType::Custom(s) => write!(f, "{}", s),
        }
    }
}

impl ObjectContent {
    /// Returns the content as a text string, or an empty string for binary content.
    pub fn as_text(&self) -> &str {
        match self {
            ObjectContent::Markdown => "",
            ObjectContent::PlainText => "",
            ObjectContent::Html => "",
            ObjectContent::Binary => "",
            ObjectContent::Structured(json) => {
                // This is a simplification; in practice, structured content
                // would need to be serialized to a string representation.
                // For now, return an empty string.
                ""
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn object_type_all_variants_serialize() {
        let variants = vec![
            ObjectType::Note,
            ObjectType::Document,
            ObjectType::Pdf,
            ObjectType::Image,
            ObjectType::Receipt,
            ObjectType::Invoice,
            ObjectType::Meeting,
            ObjectType::Person,
            ObjectType::Organisation,
            ObjectType::Project,
            ObjectType::ResearchPaper,
            ObjectType::Book,
            ObjectType::Course,
            ObjectType::Website,
            ObjectType::Bookmark,
            ObjectType::Repository,
            ObjectType::AudioRecording,
            ObjectType::Video,
            ObjectType::Scan,
            ObjectType::Screenshot,
            ObjectType::Attachment,
            ObjectType::Custom("plugin_type".to_string()),
        ];

        for variant in variants {
            let serialized =
                serde_json::to_string(&variant).expect("Failed to serialize ObjectType");
            let deserialized: ObjectType =
                serde_json::from_str(&serialized).expect("Failed to deserialize ObjectType");
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn object_content_all_variants_serialize() {
        let variants = vec![
            ObjectContent::Markdown,
            ObjectContent::PlainText,
            ObjectContent::Html,
            ObjectContent::Binary,
            ObjectContent::Structured(json!({"key": "value"})),
        ];

        for variant in variants {
            let serialized =
                serde_json::to_string(&variant).expect("Failed to serialize ObjectContent");
            let deserialized: ObjectContent =
                serde_json::from_str(&serialized).expect("Failed to deserialize ObjectContent");
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn object_metadata_empty_serializes() {
        let metadata = ObjectMetadata {
            title: None,
            author: None,
            language: None,
            source_url: None,
            source_file: None,
            mime_type: None,
            page_count: None,
            word_count: None,
            created: None,
            modified: None,
            custom: HashMap::new(),
        };

        let serialized =
            serde_json::to_string(&metadata).expect("Failed to serialize empty metadata");
        let deserialized: ObjectMetadata =
            serde_json::from_str(&serialized).expect("Failed to deserialize empty metadata");
        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn object_metadata_populated_round_trip() {
        let mut custom = HashMap::new();
        custom.insert("ocr_confidence".to_string(), json!(0.95));
        custom.insert("entities".to_string(), json!(["Nabu", "Rust"]));

        let metadata = ObjectMetadata {
            title: Some("Test Document".to_string()),
            author: Some("Author Name".to_string()),
            language: Some("en".to_string()),
            source_url: Some("https://example.com/doc".to_string()),
            source_file: Some("/path/to/file.pdf".to_string()),
            mime_type: Some("application/pdf".to_string()),
            page_count: Some(42),
            word_count: Some(12345),
            created: Some("2024-01-01T00:00:00Z".to_string()),
            modified: Some("2024-06-01T00:00:00Z".to_string()),
            custom,
        };

        let serialized = serde_json::to_string(&metadata).expect("Failed to serialize metadata");
        let deserialized: ObjectMetadata =
            serde_json::from_str(&serialized).expect("Failed to deserialize metadata");
        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn object_metadata_custom_survives_round_trip() {
        let mut custom = HashMap::new();
        custom.insert(
            "plugin_data".to_string(),
            json!({"nested": true, "count": 7}),
        );

        let metadata = ObjectMetadata {
            title: None,
            author: None,
            language: None,
            source_url: None,
            source_file: None,
            mime_type: None,
            page_count: None,
            word_count: None,
            created: None,
            modified: None,
            custom,
        };

        let serialized =
            serde_json::to_string(&metadata).expect("Failed to serialize custom metadata");
        let deserialized: ObjectMetadata =
            serde_json::from_str(&serialized).expect("Failed to deserialize custom metadata");
        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn knowledge_object_full_round_trip() {
        let obj = KnowledgeObject {
            id: Uuid::new_v4(),
            object_type: ObjectType::ResearchPaper,
            vault_id: "vault-123".to_string(),
            created_at: "2024-01-01T00:00:00Z".to_string(),
            modified_at: "2024-06-01T00:00:00Z".to_string(),
            content: ObjectContent::Structured(json!({"abstract": "A study of Rust."})),
            metadata: ObjectMetadata {
                title: Some("Rust Research".to_string()),
                author: Some("Jane Doe".to_string()),
                language: Some("en".to_string()),
                source_url: Some("https://arxiv.org/abs/1234".to_string()),
                source_file: None,
                mime_type: Some("application/pdf".to_string()),
                page_count: Some(12),
                word_count: Some(8500),
                created: Some("2024-01-01T00:00:00Z".to_string()),
                modified: Some("2024-05-15T00:00:00Z".to_string()),
                custom: HashMap::new(),
            },
        };

        let serialized = serde_json::to_string(&obj).expect("Failed to serialize KnowledgeObject");
        let deserialized: KnowledgeObject =
            serde_json::from_str(&serialized).expect("Failed to deserialize KnowledgeObject");
        assert_eq!(obj, deserialized);
    }

    #[test]
    fn object_type_custom_string_round_trip() {
        let variant = ObjectType::Custom("my_custom_type".to_string());
        let serialized = serde_json::to_string(&variant).expect("Failed to serialize Custom");
        let deserialized: ObjectType =
            serde_json::from_str(&serialized).expect("Failed to deserialize Custom");
        assert_eq!(variant, deserialized);
    }

    #[test]
    fn object_content_structured_json_round_trip() {
        let payload = json!({
            "frontmatter": {
                "tags": ["rust", "nabu"],
                "date": "2024-06-01"
            },
            "body": "Hello, world!"
        });
        let variant = ObjectContent::Structured(payload.clone());
        let serialized = serde_json::to_string(&variant).expect("Failed to serialize Structured");
        let deserialized: ObjectContent =
            serde_json::from_str(&serialized).expect("Failed to deserialize Structured");
        assert_eq!(variant, deserialized);
    }

    #[test]
    fn object_content_binary_round_trip() {
        let variant = ObjectContent::Binary;
        let serialized = serde_json::to_string(&variant).expect("Failed to serialize Binary");
        let deserialized: ObjectContent =
            serde_json::from_str(&serialized).expect("Failed to deserialize Binary");
        assert_eq!(variant, deserialized);
    }
}
