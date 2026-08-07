use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// The role of a participant in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Role {
    /// The user participating in the conversation.
    User,
    /// An AI assistant participant.
    Assistant,
    /// A system-level instruction or context message.
    System,
    /// A tool/function call result returned to the assistant.
    Tool,
}

/// The content type of a message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentType {
    /// Plain text content (no formatting).
    Text,
    /// Markdown-formatted text content.
    Markdown,
    /// JSON-structured content (e.g. tool call arguments).
    Json,
    /// HTML-formatted text content.
    Html,
}

/// A single message within a conversation turn.
///
/// Messages are the atomic unit of conversation content. Each message has a
/// `role` (user, assistant, system, tool), a `content` body, optional
/// metadata (tool call data, name, etc.), and a timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier for this message.
    pub id: Uuid,
    /// The role of the message sender.
    pub role: Role,
    /// The content type (determines how `content` should be interpreted).
    pub content_type: ContentType,
    /// The message body content.
    pub content: String,
    /// Optional name of the tool or assistant (used for tool messages
    /// and named assistants).
    pub name: Option<String>,
    /// Optional tool call ID — when this message is a response to a tool
    /// call, this references the originating tool_call ID in the
    /// assistant's message.
    pub tool_call_id: Option<String>,
    /// Additional metadata associated with the message (tool call data,
    /// token usage, etc.).
    pub metadata: HashMap<String, serde_json::Value>,
    /// When this message was created.
    pub created_at: DateTime<Utc>,
    /// When this message was last updated.
    pub updated_at: DateTime<Utc>,
}

impl Message {
    /// Creates a new message with the given role and content.
    ///
    /// The `content_type` defaults to `ContentType::Text`.
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self::with_type(role, ContentType::Text, content)
    }

    /// Creates a new message with explicit content type.
    pub fn with_type(
        role: Role,
        content_type: ContentType,
        content: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            role,
            content_type,
            content: content.into(),
            name: None,
            tool_call_id: None,
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Sets the name for this message (e.g. the tool name).
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Sets the tool call ID for this message.
    pub fn with_tool_call_id(mut self, id: impl Into<String>) -> Self {
        self.tool_call_id = Some(id.into());
        self
    }

    /// Sets a metadata entry.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// A single assistant turn in a conversation.
///
/// A turn groups a message with its associated tool calls (and the results
/// of those tool calls). This models the assistant → tool → result cycle
/// common in assistant/AI interactions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Turn {
    /// Optional tool call ID linking this turn to a specific tool invocation.
    /// Used when a turn represents the result of a tool call.
    pub tool_call_id: Option<String>,
    /// Optional name associated with the tool call.
    pub tool_name: Option<String>,
    /// The tool call arguments (as a JSON object) if this turn initiated
    /// a tool call.
    pub tool_arguments: Option<serde_json::Value>,
    /// The result of the tool call, if this turn represents a tool result.
    pub tool_result: Option<String>,
}

impl Turn {
    /// Creates a new empty turn (no tool call or result).
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a turn representing a tool call.
    pub fn tool_call(name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self {
            tool_name: Some(name.into()),
            tool_arguments: Some(arguments),
            tool_call_id: None,
            tool_result: None,
        }
    }

    /// Creates a turn representing a tool result.
    pub fn tool_result(tool_call_id: impl Into<String>, result: impl Into<String>) -> Self {
        Self {
            tool_call_id: Some(tool_call_id.into()),
            tool_result: Some(result.into()),
            tool_name: None,
            tool_arguments: None,
        }
    }
}

/// A conversation thread containing an ordered sequence of messages.
///
/// A thread is the top-level conversation entity. It groups related messages
/// (a conversation between a user and an AI assistant, for example) with
/// metadata such as a title, provider, and creation/update timestamps.
///
/// Threads are the canonical unit of conversation persistence — the
/// [`ConversationStore`](crate::conversations::ConversationStore) manages
/// saving and loading threads to/from disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    /// Unique identifier for this thread (stable across save/reload).
    pub id: Uuid,
    /// Human-readable thread title.
    pub title: String,
    /// Optional provider identifier (e.g. "openai", "anthropic").
    /// Remains `None` until a provider is assigned — the persistence layer
    /// is provider-agnostic and does not depend on this field.
    pub provider: Option<String>,
    /// The ordered sequence of messages in this thread.
    pub messages: Vec<Message>,
    /// Optional turns associated with tool-call cycles in this thread.
    pub turns: Vec<Turn>,
    /// Additional metadata for the thread (model name, parameters, etc.).
    pub metadata: HashMap<String, serde_json::Value>,
    /// When this thread was created.
    pub created_at: DateTime<Utc>,
    /// When this thread was last modified.
    pub updated_at: DateTime<Utc>,
}

impl Thread {
    /// Creates a new thread with the given title and a random UUID.
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            provider: None,
            messages: Vec::new(),
            turns: Vec::new(),
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Adds a message to the thread, updating the `updated_at` timestamp.
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.updated_at = Utc::now();
    }

    /// Adds a turn to the thread, updating the `updated_at` timestamp.
    pub fn add_turn(&mut self, turn: Turn) {
        self.turns.push(turn);
        self.updated_at = Utc::now();
    }

    /// Sets the provider for this thread.
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }

    /// Sets a metadata entry.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_new_defaults() {
        let msg = Message::new(Role::User, "hello");
        assert_eq!(msg.role, Role::User);
        assert_eq!(msg.content, "hello");
        assert_eq!(msg.content_type, ContentType::Text);
        assert!(msg.name.is_none());
        assert!(msg.tool_call_id.is_none());
        assert!(msg.metadata.is_empty());
    }

    #[test]
    fn message_builder_chains() {
        let msg = Message::new(Role::Tool, "result")
            .with_name("calc")
            .with_tool_call_id("call_123")
            .with_metadata("tokens", serde_json::json!(42));

        assert_eq!(msg.name, Some("calc".to_string()));
        assert_eq!(msg.tool_call_id, Some("call_123".to_string()));
        assert_eq!(msg.metadata.get("tokens"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn turn_tool_call_and_result() {
        let tc = Turn::tool_call("calc", serde_json::json!({"x": 1}));
        assert_eq!(tc.tool_name, Some("calc".to_string()));
        assert_eq!(tc.tool_arguments.as_ref().unwrap()["x"], 1);
        assert!(tc.tool_result.is_none());
        assert!(tc.tool_call_id.is_none());

        let tr = Turn::tool_result("call_42", "42");
        assert_eq!(tr.tool_call_id, Some("call_42".to_string()));
        assert_eq!(tr.tool_result, Some("42".to_string()));
    }

    #[test]
    fn thread_adds_messages_and_turns() {
        let mut thread = Thread::new("My Thread");
        let before = thread.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(10));

        let msg = Message::new(Role::User, "hello");
        thread.add_message(msg);
        assert_eq!(thread.messages.len(), 1);
        assert!(thread.updated_at > before);

        let turn = Turn::tool_call("tool", serde_json::json!({}));
        thread.add_turn(turn);
        assert_eq!(thread.turns.len(), 1);
    }

    #[test]
    fn thread_serializes_and_deserializes() {
        let mut thread = Thread::new("Test")
            .with_provider("test-provider")
            .with_metadata("model", serde_json::json!("gpt-4"));

        thread.add_message(Message::new(Role::User, "Hello"));
        thread.add_message(Message::new(Role::Assistant, "Hi there"));

        let json = serde_json::to_string(&thread).unwrap();
        let restored: Thread = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.id, thread.id);
        assert_eq!(restored.title, thread.title);
        assert_eq!(restored.provider, thread.provider);
        assert_eq!(restored.messages.len(), 2);
        assert_eq!(restored.messages[0].content, "Hello");
        assert_eq!(restored.messages[1].content, "Hi there");
        assert_eq!(restored.created_at, thread.created_at);
        assert_eq!(restored.updated_at, thread.updated_at);
    }

    #[test]
    fn role_and_content_type_roundtrip() {
        for role in [Role::User, Role::Assistant, Role::System, Role::Tool] {
            let msg = Message::new(role.clone(), "test");
            let json = serde_json::to_string(&msg).unwrap();
            let restored: Message = serde_json::from_str(&json).unwrap();
            assert_eq!(restored.role, role);
        }

        for ct in [
            ContentType::Text,
            ContentType::Markdown,
            ContentType::Json,
            ContentType::Html,
        ] {
            let msg = Message::with_type(Role::Assistant, ct.clone(), "x");
            let json = serde_json::to_string(&msg).unwrap();
            let restored: Message = serde_json::from_str(&json).unwrap();
            assert_eq!(restored.content_type, ct);
        }
    }
}
