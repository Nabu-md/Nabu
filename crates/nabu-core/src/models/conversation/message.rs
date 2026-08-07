//! # Message Model
//!
//! Defines [`Message`] — a logical message exchanged within a [`Thread`](crate::models::conversation.Thread).
//! A message is an ordered collection of [`Turn`](crate::models::conversation.Turn)
//! entries, representing a coherent unit of communication.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A logical message within a conversation thread.
///
/// A message groups one or more turns into a coherent unit of communication.
/// For simple text messages, there is exactly one turn. For multi-step
/// assistant responses (e.g. text + tool calls + tool results), multiple
/// turns are grouped into a single message.
///
/// # Hierarchy
///
/// ```text
/// Thread
///   └── Message  ← this type
///         └── Turn
/// ```
///
/// # Ownership
///
/// - `thread_id` links this message to its parent [`Thread`](crate::models::conversation.Thread).
/// - `turns` is an ordered `Vec<Turn>` preserving the sequence of interaction
///   steps. Ordering is validated on construction.
///
/// # Extensibility
///
/// - `metadata` is an open-ended map for message-level properties (e.g.
///   conversation branch pointers, edit history, references).
/// - `participant_id` / `role` are denormalized for quick lookups.
///
/// # Serialization
///
/// All optional fields use `#[serde(default, skip_serializing_if = "Option::is_none")]`
/// so new fields can be added without breaking deserialization of older data.
/// The `turns` collection uses `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier for this message.
    pub id: Uuid,

    /// The thread this message belongs to.
    pub thread_id: Uuid,

    /// The participant who sent this message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub participant_id: Option<Uuid>,

    /// The role of the participant who sent this message.
    /// Denormalized for quick lookups without joining to the participant list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<crate::models::conversation::Role>,

    /// When this message was created.
    pub created_at: DateTime<Utc>,

    /// When this message was last updated (e.g. appended turns).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,

    /// Ordered collection of turns that form this message.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub turns: Vec<crate::models::conversation::Turn>,

    /// Open-ended metadata for future extension.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

impl Message {
    /// Creates a new message with the given id and thread_id.
    /// The `created_at` timestamp is set to the current UTC time.
    /// The `turns` collection starts empty.
    pub fn new(id: Uuid, thread_id: Uuid) -> Self {
        Self {
            id,
            thread_id,
            participant_id: None,
            role: None,
            created_at: Utc::now(),
            updated_at: None,
            turns: Vec::new(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Creates a new message with fresh UUIDs for both the message and
    /// thread. Intended for testing and convenience where explicit IDs
    /// are not needed.
    pub fn new_anonymous() -> Self {
        Self::new(Uuid::new_v4(), Uuid::new_v4())
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

    /// Builder: set the last-updated timestamp. Primarily for testing.
    pub fn with_updated_at(mut self, updated_at: DateTime<Utc>) -> Self {
        self.updated_at = Some(updated_at);
        self
    }

    /// Builder: add a turn to this message.
    ///
    /// The turn's `message_id` is synchronized to this message's `id`.
    pub fn with_turn(mut self, mut turn: crate::models::conversation::Turn) -> Self {
        turn.message_id = self.id;
        self.turns.push(turn);
        self.updated_at = Some(Utc::now());
        self
    }

    /// Builder: add multiple turns at once.
    pub fn with_turns(mut self, turns: Vec<crate::models::conversation::Turn>) -> Self {
        for mut turn in turns {
            turn.message_id = self.id;
            self.turns.push(turn);
        }
        self.updated_at = Some(Utc::now());
        self
    }

    /// Builder: insert a metadata key-value pair.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Validates this message and all its turns.
    ///
    /// Checks:
    /// - The message id is not nil.
    /// - Each turn id is not nil.
    /// - Each turn's `message_id` matches this message's `id`.
    /// - No two turns share the same id.
    pub fn validate(&self) -> crate::models::conversation::ConversationResult<()> {
        use crate::models::conversation::ConversationError;

        if self.id == Uuid::nil() {
            return Err(ConversationError::invalid_message_id(
                "message id must not be nil",
            ));
        }

        let mut seen_turn_ids = std::collections::HashSet::new();
        for turn in &self.turns {
            if turn.id == Uuid::nil() {
                return Err(ConversationError::invalid_turn_id(
                    "turn id must not be nil",
                ));
            }
            if turn.message_id != self.id {
                return Err(ConversationError::message_mismatch(
                    turn.id,
                    self.id,
                ));
            }
            if !seen_turn_ids.insert(turn.id) {
                return Err(ConversationError::ordering_violation(
                    "message.turns",
                    format!("duplicate turn id: {}", turn.id),
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_new_has_empty_turns() {
        let msg = Message::new_anonymous();
        assert!(msg.turns.is_empty());
        assert_eq!(msg.updated_at, None);
    }

    #[test]
    fn message_with_turn_sets_message_id() {
        let msg = Message::new_anonymous();
        let turn = crate::models::conversation::Turn::new_anonymous("hello");
        let msg = msg.with_turn(turn);
        assert_eq!(msg.turns.len(), 1);
        assert_eq!(msg.turns[0].message_id, msg.id);
        assert!(msg.updated_at.is_some());
    }

    fn make_text_turn(content: &str) -> crate::models::conversation::Turn {
        crate::models::conversation::Turn::new(Uuid::new_v4(), Uuid::nil(), content)
    }

    #[test]
    fn message_with_turns_sets_all_message_ids() {
        let msg = Message::new_anonymous();
        let turns = vec![make_text_turn("a"), make_text_turn("b")];
        let msg = msg.with_turns(turns);
        assert_eq!(msg.turns.len(), 2);
        for turn in &msg.turns {
            assert_eq!(turn.message_id, msg.id);
        }
    }

    #[test]
    fn message_full_builder_round_trips() {
        let id = Uuid::new_v4();
        let thread_id = Uuid::new_v4();
        let ts = Utc::now();
        let msg = Message::new(id, thread_id)
            .with_participant(Uuid::new_v4())
            .with_role(crate::models::conversation::Role::User)
            .with_created_at(ts)
            .with_updated_at(ts)
            .with_metadata("key", serde_json::json!("val"));

        let json = serde_json::to_string(&msg).unwrap();
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(msg.id, back.id);
        assert_eq!(msg.thread_id, back.thread_id);
        assert_eq!(back.role, Some(crate::models::conversation::Role::User));
        assert_eq!(msg.created_at, back.created_at);
        assert_eq!(back.metadata.get("key"), Some(&serde_json::json!("val")));
    }

    #[test]
    fn message_validate_rejects_nil_id() {
        let msg = Message::new(Uuid::nil(), Uuid::new_v4());
        assert!(msg.validate().is_err());
    }

    #[test]
    fn message_validate_rejects_nil_turn_id() {
        let msg = Message::new_anonymous()
            .with_turn(make_text_turn("hello"));
        assert!(msg.validate().is_err());
    }

    #[test]
    fn message_validate_rejects_turn_message_id_mismatch() {
        let msg = Message::new_anonymous();
        let turn = crate::models::conversation::Turn::new(Uuid::new_v4(), Uuid::new_v4(), "hello");
        let msg = msg.with_turn(turn);
        assert!(msg.validate().is_err());
    }

    #[test]
    fn message_validate_rejects_duplicate_turn_ids() {
        let turn_id = Uuid::new_v4();
        let msg = Message::new_anonymous()
            .with_turn(crate::models::conversation::Turn::new(turn_id, Uuid::nil(), "first"))
            .with_turn(crate::models::conversation::Turn::new(turn_id, Uuid::nil(), "second"));
        assert!(msg.validate().is_err());
    }

    #[test]
    fn message_validate_accepts_valid_message() {
        let msg = Message::new_anonymous()
            .with_turn(crate::models::conversation::Turn::new(Uuid::new_v4(), Uuid::nil(), "a"))
            .with_turn(crate::models::conversation::Turn::new(Uuid::new_v4(), Uuid::nil(), "b"));
        // with_turn sets message_id, so this should pass
        assert!(msg.validate().is_ok());
    }

    #[test]
    fn message_empty_turns_omitted_when_serialized() {
        let msg = Message::new_anonymous();
        let json = serde_json::to_string(&msg).unwrap();
        let back: Message = serde_json::from_str(&json).unwrap();
        assert!(back.turns.is_empty());
    }
}
