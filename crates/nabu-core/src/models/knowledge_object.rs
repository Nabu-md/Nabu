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
}

/// Extensible custom property value types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CustomPropertyValue {
    Text(String),
    Number(f64),
    Bool(bool),
    Date(String),
    Select(String),
    MultiSelect(Vec<String>),
    Url(String),
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
