use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeObject {
    pub id: Uuid,
    pub object_type: ObjectType,
    pub vault_id: String,
    pub created_at: String,
    pub modified_at: String,
    pub content: ObjectContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectType {
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
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectContent {
    Markdown,
    PlainText,
    Html,
    Binary,
    Structured(serde_json::Value),
}
