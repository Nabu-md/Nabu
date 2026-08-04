use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Universal runtime model for all knowledge within Nabu.
/// Every subsystem uses KnowledgeObject as its primary domain type.
/// No duplicate domain models exist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeObject {
    /// Unique identifier
    pub id: Uuid,

    /// Object type discriminator
    pub object_type: ObjectType,

    /// Content variant — how this object stores its primary data
    pub content: ObjectContent,

    /// Standard metadata
    pub metadata: ObjectMetadata,

    /// Custom properties (extensible by vault config and plugins)
    pub custom_properties: HashMap<String, CustomPropertyValue>,

    /// Tags applied to this object
    pub tags: Vec<String>,

    /// Relationships to other KnowledgeObjects
    pub relations: Vec<ObjectRelation>,

    /// Processing state
    pub processing_state: ProcessingState,

    /// Content hash for deduplication (SHA-256)
    pub content_hash: Option<String>,

    /// When the object was created
    pub created_at: DateTime<Utc>,

    /// When the object was last modified
    pub updated_at: DateTime<Utc>,
}

impl KnowledgeObject {
    /// Returns the word count of the primary text content (Markdown / plain /
    /// HTML / URI), or `0` when the content carries no text body. Reused by
    /// views and the indexer so the count is derived from canonical content
    /// rather than stored independently.
    pub fn count_words(&self) -> usize {
        match &self.content {
            ObjectContent::Markdown(s)
            | ObjectContent::RichHtml(s)
            | ObjectContent::PlainText(s)
            | ObjectContent::Uri(s) => s.split_whitespace().count(),
            ObjectContent::Binary { .. } => 0,
        }
    }

    /// Reads a custom property typed as a plain text variant (`Text`, `Select`,
    /// `Url`, `Date`) as a borrowed string slice. Mirrors the backend
    /// `custom_text` helper so UI and core share the same coercion rules.
    pub fn custom_property_text(&self, key: &str) -> Option<String> {
        match self.custom_properties.get(key) {
            Some(CustomPropertyValue::Text(s))
            | Some(CustomPropertyValue::Select(s))
            | Some(CustomPropertyValue::Url(s))
            | Some(CustomPropertyValue::Date(s)) => Some(s.clone()),
            _ => None,
        }
    }

    /// Reads a custom property and projects it into a `serde_json::Value`, so
    /// callers that expect JSON semantics (e.g. `.as_str()`) work regardless of
    /// the underlying `CustomPropertyValue` variant. Mirrors the backend
    /// `custom_json` helper.
    pub fn custom_property_json(&self, key: &str) -> Option<serde_json::Value> {
        self.custom_properties
            .get(key)
            .map(|v| v.to_json_value())
    }

    /// True when this object declares a relation (of any type) pointing at
    /// `target_id`. Backlinks/edges are stored on `relations`; the graph
    /// (`VaultGraph`) is the authority for traversal.
    pub fn has_relation(&self, target_id: uuid::Uuid) -> bool {
        self.relations.iter().any(|r| r.target_id == target_id)
    }
}

impl std::fmt::Display for ObjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.variant_name())
    }
}

impl KnowledgeObject {
    pub fn new(object_type: ObjectType, content: ObjectContent) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            object_type,
            content,
            metadata: ObjectMetadata::default(),
            custom_properties: HashMap::new(),
            tags: Vec::new(),
            relations: Vec::new(),
            processing_state: ProcessingState::Pending,
            content_hash: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_metadata(mut self, metadata: ObjectMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_property(mut self, key: impl Into<String>, value: CustomPropertyValue) -> Self {
        self.custom_properties.insert(key.into(), value);
        self
    }
}

/// 22 object types covering all knowledge variants
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ObjectType {
    /// Standard text note
    Note,
    /// Bookmark/URL capture
    Bookmark,
    /// Document (PDF, Word, etc.)
    Document,
    /// Image
    Image,
    /// Screenshot capture
    Screenshot,
    /// Scan (from scanner or camera)
    Scan,
    /// Audio recording
    AudioRecording,
    /// Video recording
    VideoRecording,
    /// GitHub repository
    Repository,
    /// YouTube video reference
    YouTubeVideo,
    /// Article (readability-extracted web content)
    Article,
    /// Email capture
    Email,
    /// Contact/Person
    Contact,
    /// Project entity
    Project,
    /// Task/Action item
    Task,
    /// Event/Appointment
    Event,
    /// Code snippet
    CodeSnippet,
    /// Whiteboard / drawing
    Whiteboard,
    /// Template
    Template,
    /// Attachment (any binary file)
    Attachment,
    /// Collection / folder
    Collection,
    /// Dashboard / workspace
    Dashboard,
}

impl ObjectType {
    pub fn variant_name(&self) -> &'static str {
        match self {
            ObjectType::Note => "note",
            ObjectType::Bookmark => "bookmark",
            ObjectType::Document => "document",
            ObjectType::Image => "image",
            ObjectType::Screenshot => "screenshot",
            ObjectType::Scan => "scan",
            ObjectType::AudioRecording => "audio_recording",
            ObjectType::VideoRecording => "video_recording",
            ObjectType::Repository => "repository",
            ObjectType::YouTubeVideo => "youtube_video",
            ObjectType::Article => "article",
            ObjectType::Email => "email",
            ObjectType::Contact => "contact",
            ObjectType::Project => "project",
            ObjectType::Task => "task",
            ObjectType::Event => "event",
            ObjectType::CodeSnippet => "code_snippet",
            ObjectType::Whiteboard => "whiteboard",
            ObjectType::Template => "template",
            ObjectType::Attachment => "attachment",
            ObjectType::Collection => "collection",
            ObjectType::Dashboard => "dashboard",
        }
    }
}

/// Five content variants for how KnowledgeObject stores its primary data
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ObjectContent {
    /// Markdown body text
    Markdown(String),
    /// Rich HTML content (e.g., extracted articles)
    RichHtml(String),
    /// Raw text (no formatting)
    PlainText(String),
    /// URL reference (bookmarks, videos, repos)
    Uri(String),
    /// Binary data (images, attachments, audio)
    Binary {
        mime_type: String,
        data: Vec<u8>,
        filename: Option<String>,
    },
}

impl ObjectContent {
    pub fn content_type_hint(&self) -> &str {
        match self {
            ObjectContent::Markdown(_) => "text/markdown",
            ObjectContent::RichHtml(_) => "text/html",
            ObjectContent::PlainText(_) => "text/plain",
            ObjectContent::Uri(_) => "text/uri-list",
            ObjectContent::Binary { mime_type, .. } => mime_type.as_str(),
        }
    }
}

/// Standard metadata applicable to all object types
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ObjectMetadata {
    /// Human-readable title
    pub title: Option<String>,
    /// Original source URL (if captured from web)
    pub source_url: Option<String>,
    /// Author(s)
    pub authors: Vec<String>,
    /// Publication date (for articles, documents)
    pub publication_date: Option<DateTime<Utc>>,
    /// Site/domain name (for web captures)
    pub site_name: Option<String>,
    /// Language code (e.g., "en", "fr")
    pub language: Option<String>,
    /// File size in bytes
    pub file_size: Option<u64>,
    /// MIME type
    pub mime_type: Option<String>,
    /// OCR confidence score (0.0–1.0)
    pub ocr_confidence: Option<f64>,
    /// Original filename (for file drops)
    pub original_filename: Option<String>,
    /// Vault path where this object is stored
    pub vault_path: Option<String>,
    /// Extended description / excerpt
    pub description: Option<String>,
    /// Word count of the primary content body (derived, persisted for views).
    pub word_count: Option<usize>,
}

/// Extensible custom property value types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CustomPropertyValue {
    Text(String),
    Number(f64),
    Bool(bool),
    Date(String),
    Select(String),
    MultiSelect(Vec<String>),
    Url(String),
    /// A typed relation to another KnowledgeObject, stored by id. Reuses the
    /// existing relation model rather than introducing a parallel one.
    Relation(uuid::Uuid),
}

impl CustomPropertyValue {
    /// Project this value into a `serde_json::Value` so callers that expect
    /// JSON semantics (e.g. `.as_str()`, `.as_f64()`) work for every variant.
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            CustomPropertyValue::Text(s) => serde_json::Value::String(s.clone()),
            CustomPropertyValue::Number(n) => serde_json::Value::from(*n),
            CustomPropertyValue::Bool(b) => serde_json::Value::Bool(*b),
            CustomPropertyValue::Date(s) => serde_json::Value::String(s.clone()),
            CustomPropertyValue::Select(s) => serde_json::Value::String(s.clone()),
            CustomPropertyValue::MultiSelect(v) => {
                serde_json::Value::Array(v.iter().cloned().map(serde_json::Value::String).collect())
            }
            CustomPropertyValue::Url(s) => serde_json::Value::String(s.clone()),
            CustomPropertyValue::Relation(id) => {
                serde_json::Value::String(id.to_string())
            }
        }
    }
}

/// A relationship between this object and another
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectRelation {
    pub target_id: Uuid,
    pub relation_type: RelationType,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationType {
    References,
    ReferencedBy,
    Parent,
    Child,
    Attached,
    Related,
    Custom(String),
}

/// Processing state of a KnowledgeObject
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProcessingState {
    Pending,
    Processing,
    Completed,
    Failed(String),
    Cancelled,
}

/// Capture source types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum CaptureSource {
    Browser,
    Clipboard,
    Screenshot,
    FileDrop,
    WatchFolder,
    SafariReader,
    YouTube,
    GitHub,
    /// Email capture (`.eml` files, email text, forwarded messages)
    Email,
    Url,
    /// Reader-mode / readability-extracted article content.
    Article,
    Manual,
    Api,
    Plugin,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_metadata_has_word_count_field() {
        let meta = ObjectMetadata::default();
        assert_eq!(meta.word_count, None);

        let meta = ObjectMetadata {
            word_count: Some(42),
            ..Default::default()
        };
        assert_eq!(meta.word_count, Some(42));
    }

    #[test]
    fn custom_property_value_relation_to_json() {
        let id = Uuid::nil();
        let rel = CustomPropertyValue::Relation(id);
        let json = rel.to_json_value();
        assert!(json.is_string());
        assert_eq!(json.as_str(), Some(id.to_string().as_str()));
    }

    #[test]
    fn to_json_value_projects_each_variant() {
        assert_eq!(
            CustomPropertyValue::Text("hi".into()).to_json_value(),
            serde_json::Value::String("hi".into())
        );
        assert_eq!(
            CustomPropertyValue::Number(3.5).to_json_value(),
            serde_json::Value::from(3.5)
        );
        assert_eq!(
            CustomPropertyValue::Bool(true).to_json_value(),
            serde_json::Value::Bool(true)
        );
        assert_eq!(
            CustomPropertyValue::MultiSelect(vec!["a".into(), "b".into()]).to_json_value(),
            serde_json::Value::Array(vec![
                serde_json::Value::String("a".into()),
                serde_json::Value::String("b".into())
            ])
        );
    }

    #[test]
    fn custom_property_json_uses_to_json_value() {
        let mut obj = KnowledgeObject::new(ObjectType::Note, ObjectContent::PlainText("hello world".into()));
        let id = Uuid::nil();
        obj.custom_properties.insert("related".into(), CustomPropertyValue::Relation(id));
        let json = obj.custom_property_json("related");
        assert_eq!(json, Some(serde_json::Value::String(id.to_string())));
        assert_eq!(obj.custom_property_json("missing"), None);
    }
}
