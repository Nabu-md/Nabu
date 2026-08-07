//! # Thread Model
//!
//! Defines [`Thread`] — the top-level container for a complete conversation.
//! A thread owns participants and an ordered collection of messages, each
//! of which in turn contains ordered turns.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A complete conversation thread.
///
/// A `Thread` is the root aggregate of the conversation model. It contains:
///
/// - A set of participants (human or non-human) who take part in the
///   conversation.
/// - An ordered collection of [`Message`] entries.
///
/// # Hierarchy
///
/// ```text
/// Thread  ← this type
/// ├── Message
/// │      ├── Turn
/// │      └── Turn
/// └── ...
/// ```
///
/// # Ownership
///
/// The thread owns its messages and participants. Messages reference the
/// thread by `thread_id` (a denormalized foreign key), and turns reference
/// their parent message by `message_id`. This allows individual messages
/// and turns to be validated and operated on in isolation.
///
/// # Extensibility
///
/// - `metadata` is an open-ended map for thread-level properties (title,
///   tags, conversation type, model preferences, etc.).
/// - `participants` supports arbitrary participants (plugins, services,
///   automation, external systems) via [`Participant`].
/// - All optional fields use `#[serde(default, skip_serializing_if = "...")]`
///   so new fields can be added without breaking deserialization.
///
/// # Serialization
///
/// `Thread` derives [`Serialize`] and [`Deserialize`]. The `messages`
/// collection uses `#[serde(default, skip_serializing_if = "Vec::is_empty")]`
/// and `participants` uses the same pattern, so an empty thread serializes
/// cleanly and can gain fields in future versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    /// Unique identifier for this thread.
    pub id: Uuid,

    /// Human-readable title for the conversation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// When this thread was created.
    pub created_at: DateTime<Utc>,

    /// When this thread was last updated (e.g. new message added).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,

    /// Participants in this conversation (human and non-human).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub participants: Vec<crate::models::conversation::Participant>,

    /// Ordered collection of messages belonging to this thread.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<crate::models::conversation::Message>,

    /// Open-ended metadata for future extension.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

impl Thread {
    /// Creates a new thread with a fresh UUID and the current UTC timestamp.
    pub fn new() -> Self {
        Self::with_id(Uuid::new_v4())
    }

    /// Creates a new thread with the given explicit id.
    pub fn with_id(id: Uuid) -> Self {
        let now = Utc::now();
        Self {
            id,
            title: None,
            created_at: now,
            updated_at: None,
            participants: Vec::new(),
            messages: Vec::new(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Builder: set the title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
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

    /// Builder: add a participant to this thread.
    ///
    /// If a participant with the same id already exists, the later one is kept
    /// (replace semantics).
    pub fn with_participant(mut self, participant: crate::models::conversation::Participant) -> Self {
        if let Some(pos) = self
            .participants
            .iter()
            .position(|p| p.id == participant.id)
        {
            self.participants[pos] = participant;
        } else {
            self.participants.push(participant);
        }
        self.updated_at = Some(Utc::now());
        self
    }

    /// Builder: add a message to this thread.
    ///
    /// The message's `thread_id` is synchronized to this thread's `id`.
    pub fn with_message(mut self, mut message: crate::models::conversation::Message) -> Self {
        message.thread_id = self.id;
        self.messages.push(message);
        self.updated_at = Some(Utc::now());
        self
    }

    /// Builder: add multiple messages at once.
    pub fn with_messages(
        mut self,
        messages: Vec<crate::models::conversation::Message>,
    ) -> Self {
        for mut message in messages {
            message.thread_id = self.id;
            self.messages.push(message);
        }
        self.updated_at = Some(Utc::now());
        self
    }

    /// Builder: insert a metadata key-value pair.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Validates this thread and all its messages and turns.
    ///
    /// Checks:
    /// - The thread id is not nil.
    /// - Each message id is not nil and references the correct thread_id.
    /// - No two messages share the same id.
    /// - Each message's internal validation passes (see [`Message::validate`]).
    pub fn validate(&self) -> crate::models::conversation::ConversationResult<()> {
        use crate::models::conversation::ConversationError;

        if self.id == Uuid::nil() {
            return Err(ConversationError::invalid_thread_id(
                "thread id must not be nil",
            ));
        }

        let mut seen_message_ids = std::collections::HashSet::new();
        for message in &self.messages {
            if message.id == Uuid::nil() {
                return Err(ConversationError::invalid_message_id(
                    "message id must not be nil",
                ));
            }
            if message.thread_id != self.id {
                return Err(ConversationError::thread_mismatch(
                    message.id,
                    self.id,
                ));
            }
            if !seen_message_ids.insert(message.id) {
                return Err(ConversationError::ordering_violation(
                    "thread.messages",
                    format!("duplicate message id: {}", message.id),
                ));
            }
            message.validate()?;
        }

        Ok(())
    }
}

impl Default for Thread {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::conversation::Role;
    use crate::models::conversation::TurnContent;

    #[test]
    fn thread_default_creates_with_fresh_id() {
        let t1 = Thread::new();
        let t2 = Thread::new();
        assert_ne!(t1.id, t2.id);
        assert!(t1.title.is_none());
        assert!(t1.messages.is_empty());
        assert!(t1.participants.is_empty());
    }

    #[test]
    fn thread_with_id_uses_explicit_id() {
        let id = Uuid::new_v4();
        let t = Thread::with_id(id);
        assert_eq!(t.id, id);
    }

    #[test]
    fn thread_with_message_sets_thread_id() {
        let t = Thread::new();
        let msg = crate::models::conversation::Message::new_anonymous();
        let t = t.with_message(msg);
        assert_eq!(t.messages.len(), 1);
        assert_eq!(t.messages[0].thread_id, t.id);
    }

    #[test]
    fn thread_with_messages_sets_all_thread_ids() {
        let t = Thread::new();
        let msgs = vec![
            crate::models::conversation::Message::new_anonymous(),
            crate::models::conversation::Message::new_anonymous(),
        ];
        let t = t.with_messages(msgs);
        assert_eq!(t.messages.len(), 2);
        for msg in &t.messages {
            assert_eq!(msg.thread_id, t.id);
        }
    }

    #[test]
    fn thread_with_participant_dedups_by_id() {
        let id = Uuid::new_v4();
        let p1 = crate::models::conversation::Participant::new(id, Role::User)
            .with_name("Alice");
        let p2 = crate::models::conversation::Participant::new(id, Role::User)
            .with_name("Alice Updated");
        let t = Thread::new()
            .with_participant(p1)
            .with_participant(p2);
        assert_eq!(t.participants.len(), 1);
        assert_eq!(t.participants[0].name, Some("Alice Updated".to_string()));
    }

    #[test]
    fn thread_full_builder_round_trips() {
        let id = Uuid::new_v4();
        let ts = Utc::now();
        let msg = crate::models::conversation::Message::new(Uuid::new_v4(), id)
            .with_turn(crate::models::conversation::Turn::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                "hello",
            ));
        let t = Thread::with_id(id)
            .with_title("My Conversation")
            .with_created_at(ts)
            .with_updated_at(ts)
            .with_participant(
                crate::models::conversation::Participant::new(Uuid::new_v4(), Role::User)
                    .with_name("Alice"),
            )
            .with_message(msg)
            .with_metadata("tag", serde_json::json!("test"));

        let json = serde_json::to_string(&t).unwrap();
        let back: Thread = serde_json::from_str(&json).unwrap();
        assert_eq!(t.id, back.id);
        assert_eq!(back.title, Some("My Conversation".to_string()));
        assert_eq!(t.created_at, back.created_at);
        assert_eq!(back.metadata.get("tag"), Some(&serde_json::json!("test")));
        assert_eq!(back.participants.len(), 1);
        assert_eq!(back.messages.len(), 1);
    }

    #[test]
    fn thread_validate_rejects_nil_id() {
        let t = Thread::with_id(Uuid::nil());
        assert!(t.validate().is_err());
    }

    #[test]
    fn thread_validate_rejects_message_thread_mismatch() {
        // message belongs to a different thread
        let msg = crate::models::conversation::Message::new(Uuid::new_v4(), Uuid::new_v4());
        let t = Thread::new().with_message(msg);
        assert!(t.validate().is_err());
    }

    #[test]
    fn thread_validate_rejects_duplicate_message_ids() {
        let msg_id = Uuid::new_v4();
        let m1 = crate::models::conversation::Message::new(msg_id, Uuid::nil());
        let m2 = crate::models::conversation::Message::new(msg_id, Uuid::nil());
        let t = Thread::new().with_message(m1).with_message(m2);
        assert!(t.validate().is_err());
    }

    #[test]
    fn thread_validate_accepts_valid_thread() {
        let t = Thread::new()
            .with_title("Test")
            .with_participant(
                crate::models::conversation::Participant::new(Uuid::new_v4(), Role::User)
                    .with_name("Alice"),
            )
            .with_message(
                crate::models::conversation::Message::new_anonymous()
                    .with_turn(crate::models::conversation::Turn::new(
                        Uuid::new_v4(),
                        Uuid::new_v4(),
                        "Hello",
                    )),
            );
        assert!(t.validate().is_ok());
    }

    #[test]
    fn thread_empty_collections_omitted_when_serialized() {
        let t = Thread::new();
        let json = serde_json::to_string(&t).unwrap();
        let back: Thread = serde_json::from_str(&json).unwrap();
        assert!(back.messages.is_empty());
        assert!(back.participants.is_empty());
        assert!(back.metadata.is_empty());
    }

    #[test]
    fn thread_title_omitted_when_none() {
        let t = Thread::new();
        let json = serde_json::to_string(&t).unwrap();
        assert!(!json.contains("\"title\""));
    }

    #[test]
    fn thread_participants_and_messages_round_trip() {
        let participant_id = Uuid::new_v4();
        let msg = crate::models::conversation::Message::new_anonymous()
            .with_participant(participant_id)
            .with_role(Role::User)
            .with_turn(crate::models::conversation::Turn::new(
                Uuid::new_v4(),
                Uuid::new_v4(),
                crate::models::conversation::TurnContent::text("Hello"),
            ));

        let t = Thread::new()
            .with_title("Test Thread")
            .with_participant(
                crate::models::conversation::Participant::new(participant_id, Role::User)
                    .with_name("Alice"),
            )
            .with_message(msg);

        let json = serde_json::to_string(&t).unwrap();
        let back: Thread = serde_json::from_str(&json).unwrap();
        assert_eq!(back.messages.len(), 1);
        assert_eq!(back.messages[0].turns.len(), 1);
        assert_eq!(
            back.messages[0].turns[0].content.as_text(),
            Some("Hello")
        );
        assert_eq!(back.participants.len(), 1);
        assert_eq!(back.participants[0].name, Some("Alice".to_string()));
    }
}
