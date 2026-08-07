//! # Turn Model
//!
//! Defines [`Turn`] — the finest-grain unit in the conversation hierarchy.
//! A turn represents an individual interaction step within a message.
//!
//! Turn content is represented as the [`TurnContent`] enum rather than a
//! fixed string. This allows future structured content (attachments, tool
//! calls, citations, streaming responses) to be added as new enum variants
//! without restructuring the model.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The content payload of a [`Turn`].
///
/// This enum is intentionally open-ended. The initial `Text` variant covers
/// the vast majority of conversational content. Future phases can add
/// variants for attachments, tool calls, citations, structured outputs,
/// and streaming chunks without breaking the `Turn` model itself.
///
/// # Extensibility
///
/// This enum is `#[non_exhaustive]`. External matchers must include a `_`
/// arm when exhaustive matching is not desired.
///
/// # Serialization
///
/// Uses `#[serde(untagged)]` so the serialized JSON is a flat value rather
/// than a nested tagged enum. For example, `TurnContent::Text("hello".into())`
/// serializes as `"hello"` (a JSON string), not `{"text": "hello"}`.
///
/// Unknown content shapes deserialize into `TurnContent::Unknown` as a
/// `serde_json::Value`, preserving data through round-trips even if the
/// variant was introduced in a newer version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum TurnContent {
    /// Plain-text or Markdown content.
    Text(String),

    /// Structured content that does not match a known variant. The raw
    /// `serde_json::Value` preserves all data for forward compatibility.
    Unknown(serde_json::Value),
}

impl TurnContent {
    /// Creates a new `Text` turn content from anything that implements
    /// `Into<String>`.
    pub fn text(s: impl Into<String>) -> Self {
        TurnContent::Text(s.into())
    }

    /// Returns the text content if this is a `Text` variant, or `None`
    /// otherwise.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            TurnContent::Text(s) => Some(s),
            _ => None,
        }
    }

    /// Returns `true` if this content is the `Text` variant.
    pub fn is_text(&self) -> bool {
        matches!(self, TurnContent::Text(_))
    }

    /// Coerces this content into a string suitable for display or indexing.
    /// For `Text` it returns the string directly. For `Unknown` it returns
    /// the JSON representation.
    pub fn to_display_string(&self) -> String {
        match self {
            TurnContent::Text(s) => s.clone(),
            TurnContent::Unknown(v) => v.to_string(),
        }
    }
}

impl From<String> for TurnContent {
    fn from(s: String) -> Self {
        TurnContent::Text(s)
    }
}

impl From<&str> for TurnContent {
    fn from(s: &str) -> Self {
        TurnContent::Text(s.into())
    }
}

impl std::fmt::Display for TurnContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TurnContent::Text(s) => f.write_str(s),
            TurnContent::Unknown(v) => write!(f, "{}", v),
        }
    }
}

/// A single interaction step within a [`Message`](crate::models::conversation::Message).
///
/// A turn captures one unit of content produced by a single participant.
/// Multiple turns within a message support interleaved generation,
/// tool-call/result pairs, and streaming chunks within a single logical
/// message.
///
/// # Hierarchy
///
/// ```text
/// Thread
///   └── Message
///         └── Turn  ← this type
/// ```
///
/// # Extensibility
///
/// - `content` is a [`TurnContent`] enum, allowing future structured content
///   variants (attachments, tool calls, citations) without model changes.
/// - `metadata` is an open-ended map for turn-level properties.
/// - `participant_id` links the turn to a [`Participant`](crate::models::conversation::Participant)
///   in the parent thread, enabling per-turn attribution.
///
/// # Serialization
///
/// All optional fields use `#[serde(default, skip_serializing_if = "Option::is_none")]`
/// so new fields can be added without breaking deserialization of older data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    /// Unique identifier for this turn within the conversation.
    pub id: Uuid,

    /// The message this turn belongs to.
    pub message_id: Uuid,

    /// The participant who produced this turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub participant_id: Option<Uuid>,

    /// The role of the participant who produced this turn.
    /// This is a denormalized copy for quick lookups without joining
    /// to the participant table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<crate::models::conversation::Role>,

    /// The content payload of this turn.
    pub content: TurnContent,

    /// When this turn was created.
    pub created_at: DateTime<Utc>,

    /// Open-ended metadata for future extension (token counts, model version,
    /// tool call references, streaming state, etc.).
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

impl Turn {
    /// Creates a new turn with the given id, message_id, and text content.
    /// The `created_at` timestamp is set to the current UTC time.
    pub fn new(id: Uuid, message_id: Uuid, content: impl Into<TurnContent>) -> Self {
        Self {
            id,
            message_id,
            participant_id: None,
            role: None,
            content: content.into(),
            created_at: Utc::now(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Creates a new turn with a fresh UUID for both the turn and message.
    /// Intended for testing and convenience where explicit IDs are not needed.
    pub fn new_anonymous(content: impl Into<TurnContent>) -> Self {
        Self::new(Uuid::new_v4(), Uuid::new_v4(), content)
    }

    /// Builder: set the participant ID.
    pub fn with_participant(mut self, participant_id: Uuid) -> Self {
        self.participant_id = Some(participant_id);
        self
    }

    /// Builder: set the role.
    pub fn with_role(mut self, role: crate::models::conversation::Role) -> Self {
        self.role = Some(role);
        self
    }

    /// Builder: set the creation timestamp. Primarily for testing.
    pub fn with_created_at(mut self, created_at: DateTime<Utc>) -> Self {
        self.created_at = created_at;
        self
    }

    /// Builder: insert a metadata key-value pair.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Validates that the turn id is not nil.
    pub fn validate(&self) -> crate::models::conversation::ConversationResult<()> {
        if self.id == Uuid::nil() {
            return Err(crate::models::conversation::ConversationError::invalid_turn_id(
                "turn id must not be nil",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::conversation::Role;

    #[test]
    fn turn_text_round_trips() {
        let turn = Turn::new_anonymous("Hello, world!");
        let json = serde_json::to_string(&turn).unwrap();
        let back: Turn = serde_json::from_str(&json).unwrap();
        assert_eq!(turn.id, back.id);
        assert_eq!(turn.content, back.content);
        assert_eq!(turn.content.as_text(), Some("Hello, world!"));
    }

    #[test]
    fn turn_content_text_serializes_as_plain_string() {
        let content = TurnContent::Text("hello".into());
        let json = serde_json::to_string(&content).unwrap();
        assert_eq!(json, "\"hello\"");
    }

    #[test]
    fn turn_content_from_string() {
        let content: TurnContent = "hello".into();
        assert!(content.is_text());
        assert_eq!(content.as_text(), Some("hello"));
    }

    #[test]
    fn turn_content_unknown_round_trips() {
        let value = serde_json::json!({"tool_call": {"name": "search", "args": {"q": "test"}}});
        let content = TurnContent::Unknown(value.clone());
        let json = serde_json::to_string(&content).unwrap();
        let back: TurnContent = serde_json::from_str(&json).unwrap();
        assert_eq!(content, back);
        assert_eq!(back.to_display_string(), value.to_string());
    }

    #[test]
    fn turn_content_display_text() {
        let content = TurnContent::Text("hello".into());
        assert_eq!(format!("{}", content), "hello");
    }

    #[test]
    fn turn_content_display_unknown() {
        let value = serde_json::json!({"key": "val"});
        let content = TurnContent::Unknown(value.clone());
        assert_eq!(format!("{}", content), value.to_string());
    }

    #[test]
    fn turn_bare_builder_fields_omitted_when_none() {
        let turn = Turn::new_anonymous("hi");
        let json = serde_json::to_string(&turn).unwrap();
        let back: Turn = serde_json::from_str(&json).unwrap();
        assert_eq!(back.participant_id, None);
        assert_eq!(back.role, None);
    }

    #[test]
    fn turn_full_builder_round_trips() {
        let id = Uuid::new_v4();
        let msg_id = Uuid::new_v4();
        let participant_id = Uuid::new_v4();
        let ts = Utc::now();
        let turn = Turn::new(id, msg_id, TurnContent::text("content"))
            .with_participant(participant_id)
            .with_role(Role::Assistant)
            .with_created_at(ts)
            .with_metadata("tokens", serde_json::json!(42));

        let json = serde_json::to_string(&turn).unwrap();
        let back: Turn = serde_json::from_str(&json).unwrap();
        assert_eq!(turn.id, back.id);
        assert_eq!(turn.message_id, back.message_id);
        assert_eq!(back.participant_id, Some(participant_id));
        assert_eq!(back.role, Some(Role::Assistant));
        assert_eq!(turn.created_at, back.created_at);
        assert_eq!(back.metadata.get("tokens"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn turn_validate_rejects_nil_id() {
        let turn = Turn::new(Uuid::nil(), Uuid::new_v4(), "content");
        assert!(turn.validate().is_err());
    }

    #[test]
    fn turn_validate_accepts_valid_id() {
        let turn = Turn::new(Uuid::new_v4(), Uuid::new_v4(), "content");
        assert!(turn.validate().is_ok());
    }
}
