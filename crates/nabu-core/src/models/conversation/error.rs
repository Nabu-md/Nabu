//! # Conversation Domain Errors
//!
//! Structured error types for conversation model validation. Every constructor
//! and validation helper that can fail returns [`ConversationError`] — never
//! panics. The type is fully serializable so it can be transported across IPC
//! boundaries (EventBus → Tauri bridge → frontend).
//!
//! [`ConversationError`] borrows its variant payloads from the strongly-typed
//! domain models rather than re-encoding them as opaque strings.

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Result alias used throughout the conversation domain.
pub type ConversationResult<T> = Result<T, ConversationError>;

/// Structured errors for conversation model validation.
///
/// All variants carry structured data — not raw string messages — so that
/// IPC consumers (the frontend, logging, metrics) can inspect error fields
/// without string parsing.
///
/// # Serialization
///
/// `ConversationError` derives [`Serialize`] and [`Deserialize`] via Serde.
/// Every variant round-trips through JSON correctly.
///
/// # Forward compatibility
///
/// New variants may be added in future phases. External consumers should
/// include a `_ =>` arm when matching exhaustively is not desired.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ConversationError {
    /// A thread identifier was expected but missing or invalid.
    #[error("Invalid thread identifier: {reason}")]
    InvalidThreadId {
        /// Human-readable description of the validation failure.
        reason: String,
    },

    /// A message identifier was expected but missing or invalid.
    #[error("Invalid message identifier: {reason}")]
    InvalidMessageId {
        /// Human-readable description of the validation failure.
        reason: String,
    },

    /// A turn identifier was expected but missing or invalid.
    #[error("Invalid turn identifier: {reason}")]
    InvalidTurnId {
        /// Human-readable description of the validation failure.
        reason: String,
    },

    /// A message references a thread that does not exist or does not match.
    #[error("Message {message_id} does not belong to thread {thread_id}")]
    ThreadMismatch {
        /// The message identifier that has a mismatched thread reference.
        message_id: Uuid,
        /// The thread identifier the message references.
        thread_id: Uuid,
    },

    /// A turn references a message that does not exist or does not match.
    #[error("Turn {turn_id} does not belong to message {message_id}")]
    MessageMismatch {
        /// The turn identifier that has a mismatched message reference.
        turn_id: Uuid,
        /// The message identifier the turn references.
        message_id: Uuid,
    },

    /// A collection ordering constraint was violated (e.g. duplicate turn IDs).
    #[error("Ordering violation in {collection}: {reason}")]
    OrderingViolation {
        /// The collection that has the ordering issue (e.g. "message.turns").
        collection: String,
        /// Human-readable description of the violation.
        reason: String,
    },

    /// A required field is missing from a model.
    #[error("Missing required field '{field}' in {model}")]
    MissingField {
        /// The model type that has the missing field.
        model: String,
        /// The field name that is missing.
        field: String,
    },

    /// The provided content is invalid or cannot be deserialized.
    #[error("Invalid content: {reason}")]
    InvalidContent {
        /// Human-readable description of the content failure.
        reason: String,
    },
}

impl ConversationError {
    /// Creates an [`InvalidThreadId`](ConversationError::InvalidThreadId) error.
    pub fn invalid_thread_id(reason: impl Into<String>) -> Self {
        ConversationError::InvalidThreadId {
            reason: reason.into(),
        }
    }

    /// Creates an [`InvalidMessageId`](ConversationError::InvalidMessageId) error.
    pub fn invalid_message_id(reason: impl Into<String>) -> Self {
        ConversationError::InvalidMessageId {
            reason: reason.into(),
        }
    }

    /// Creates an [`InvalidTurnId`](ConversationError::InvalidTurnId) error.
    pub fn invalid_turn_id(reason: impl Into<String>) -> Self {
        ConversationError::InvalidTurnId {
            reason: reason.into(),
        }
    }

    /// Creates a [`ThreadMismatch`](ConversationError::ThreadMismatch) error.
    pub fn thread_mismatch(message_id: Uuid, thread_id: Uuid) -> Self {
        ConversationError::ThreadMismatch {
            message_id,
            thread_id,
        }
    }

    /// Creates a [`MessageMismatch`](ConversationError::MessageMismatch) error.
    pub fn message_mismatch(turn_id: Uuid, message_id: Uuid) -> Self {
        ConversationError::MessageMismatch {
            turn_id,
            message_id,
        }
    }

    /// Creates an [`OrderingViolation`](ConversationError::OrderingViolation) error.
    pub fn ordering_violation(
        collection: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        ConversationError::OrderingViolation {
            collection: collection.into(),
            reason: reason.into(),
        }
    }

    /// Creates a [`MissingField`](ConversationError::MissingField) error.
    pub fn missing_field(model: impl Into<String>, field: impl Into<String>) -> Self {
        ConversationError::MissingField {
            model: model.into(),
            field: field.into(),
        }
    }

    /// Creates an [`InvalidContent`](ConversationError::InvalidContent) error.
    pub fn invalid_content(reason: impl Into<String>) -> Self {
        ConversationError::InvalidContent {
            reason: reason.into(),
        }
    }

    /// Returns the variant name as a `&'static str` — useful for metrics and
    /// structured logging without serializing the full error payload.
    pub fn variant_name(&self) -> &'static str {
        match self {
            ConversationError::InvalidThreadId { .. } => "invalid_thread_id",
            ConversationError::InvalidMessageId { .. } => "invalid_message_id",
            ConversationError::InvalidTurnId { .. } => "invalid_turn_id",
            ConversationError::ThreadMismatch { .. } => "thread_mismatch",
            ConversationError::MessageMismatch { .. } => "message_mismatch",
            ConversationError::OrderingViolation { .. } => "ordering_violation",
            ConversationError::MissingField { .. } => "missing_field",
            ConversationError::InvalidContent { .. } => "invalid_content",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_thread_id_round_trips() {
        let err = ConversationError::invalid_thread_id("thread id is nil");
        let json = serde_json::to_string(&err).unwrap();
        let back: ConversationError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
        assert_eq!(err.variant_name(), "invalid_thread_id");
    }

    #[test]
    fn invalid_message_id_round_trips() {
        let err = ConversationError::invalid_message_id("message id is nil");
        let json = serde_json::to_string(&err).unwrap();
        let back: ConversationError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
        assert_eq!(err.variant_name(), "invalid_message_id");
    }

    #[test]
    fn invalid_turn_id_round_trips() {
        let err = ConversationError::invalid_turn_id("turn id is nil");
        let json = serde_json::to_string(&err).unwrap();
        let back: ConversationError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
        assert_eq!(err.variant_name(), "invalid_turn_id");
    }

    #[test]
    fn thread_mismatch_round_trips() {
        let msg_id = Uuid::new_v4();
        let thread_id = Uuid::new_v4();
        let err = ConversationError::thread_mismatch(msg_id, thread_id);
        let json = serde_json::to_string(&err).unwrap();
        let back: ConversationError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
        assert_eq!(err.variant_name(), "thread_mismatch");
    }

    #[test]
    fn message_mismatch_round_trips() {
        let turn_id = Uuid::new_v4();
        let msg_id = Uuid::new_v4();
        let err = ConversationError::message_mismatch(turn_id, msg_id);
        let json = serde_json::to_string(&err).unwrap();
        let back: ConversationError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
        assert_eq!(err.variant_name(), "message_mismatch");
    }

    #[test]
    fn ordering_violation_round_trips() {
        let err = ConversationError::ordering_violation("thread.messages", "duplicate id");
        let json = serde_json::to_string(&err).unwrap();
        let back: ConversationError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
        assert_eq!(err.variant_name(), "ordering_violation");
    }

    #[test]
    fn missing_field_round_trips() {
        let err = ConversationError::missing_field("Turn", "content");
        let json = serde_json::to_string(&err).unwrap();
        let back: ConversationError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
        assert_eq!(err.variant_name(), "missing_field");
    }

    #[test]
    fn invalid_content_round_trips() {
        let err = ConversationError::invalid_content("empty content");
        let json = serde_json::to_string(&err).unwrap();
        let back: ConversationError = serde_json::from_str(&json).unwrap();
        assert_eq!(err, back);
        assert_eq!(err.variant_name(), "invalid_content");
    }

    #[test]
    fn error_display_contains_fields() {
        let msg_id = Uuid::new_v4();
        let thread_id = Uuid::new_v4();
        let err = ConversationError::thread_mismatch(msg_id, thread_id);
        let msg = format!("{}", err);
        assert!(msg.contains(&msg_id.to_string()));
        assert!(msg.contains(&thread_id.to_string()));
    }

    #[test]
    fn error_implements_std_error() {
        let err = ConversationError::invalid_thread_id("test");
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn error_is_clone() {
        let err = ConversationError::invalid_thread_id("test");
        let _clone = err.clone();
        assert_eq!(err, _clone);
    }
}
