//! # Participant Model
//!
//! Defines [`Participant`] — a metadata-bearing description of who or what
//! sent a message or took a turn in a conversation. This is deliberately
//! lightweight and extensible, keeping the canonical conversation model
//! transport-agnostic.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Metadata describing a participant in a conversation.
///
/// A `Participant` captures identity-level information without presuming the
/// participant is human. Plugins, services, automation workflows, and external
/// systems all have `Participant` entries.
///
/// # Extensibility
///
/// - `id` is a UUID that can be a real identity when available, or a generated
///   stable identifier for non-human participants.
/// - `agent_id` optionally identifies a specific plugin/service/agent when
///   `role` is [`Role::Plugin`] or [`Role::Service`].
/// - `model` optionally records the AI model name when `role` is
///   [`Role::Assistant`].
/// - `name` is a human-readable display label.
/// - `metadata` is an open-ended map for future participant-specific fields
///   (avatar, preferences, capabilities, etc.).
///
/// # Serialization
///
/// Uses `#[serde(default)]` on the metadata map and `skip_serializing_if`
/// on optional fields so new fields can be added without breaking existing
/// serialized data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Participant {
    /// Stable unique identifier for this participant within the thread.
    pub id: Uuid,

    /// The role this participant plays in the conversation.
    pub role: crate::models::conversation::Role,

    /// Human-readable display name (e.g. "User", "GPT-4", "CodeSearchPlugin").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Agent/plugin/service identifier when `role` is `Plugin` or `Service`.
    /// This is an opaque string chosen by the plugin/service.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,

    /// AI model identifier when `role` is `Assistant` (e.g. "gpt-4", "claude-3-7-sonnet").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Open-ended metadata for future extension (avatar URLs, preferences,
    /// capabilities, etc.).
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

impl Participant {
    /// Creates a new participant with the given id and role.
    pub fn new(id: Uuid, role: crate::models::conversation::Role) -> Self {
        Self {
            id,
            role,
            name: None,
            agent_id: None,
            model: None,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Fluent builder: set the display name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Fluent builder: set the agent/plugin/service identifier.
    pub fn with_agent_id(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// Fluent builder: set the AI model identifier.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Fluent builder: insert a metadata key-value pair.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

impl std::fmt::Display for Participant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.name {
            Some(name) => write!(f, "{}", name),
            None => write!(f, "{}({})", self.role, self.id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::conversation::Role;

    #[test]
    fn participant_basics_round_trip() {
        let id = Uuid::new_v4();
        let p = Participant::new(id, Role::User)
            .with_name("Alice")
            .with_model("gpt-4");

        let json = serde_json::to_string(&p).unwrap();
        let back: Participant = serde_json::from_str(&json).unwrap();
        assert_eq!(p.id, back.id);
        assert_eq!(p.role, back.role);
        assert_eq!(p.name, back.name);
        assert_eq!(p.model, back.model);
    }

    #[test]
    fn participant_empty_metadata_not_serialized() {
        let p = Participant::new(Uuid::new_v4(), Role::Plugin).with_name("Search");
        let json = serde_json::to_string(&p).unwrap();
        let back: Participant = serde_json::from_str(&json).unwrap();
        assert!(back.metadata.is_empty());
    }

    #[test]
    fn participant_display_uses_name_when_present() {
        let p = Participant::new(Uuid::new_v4(), Role::User).with_name("Alice");
        assert_eq!(format!("{}", p), "Alice");
    }

    #[test]
    fn participant_display_falls_back_to_role_and_id() {
        let id = Uuid::new_v4();
        let p = Participant::new(id, Role::User);
        let display = format!("{}", p);
        assert!(display.contains("user"));
        assert!(display.contains(&id.to_string()));
    }

    #[test]
    fn participant_plugin_with_agent_id_round_trips() {
        let id = Uuid::new_v4();
        let p = Participant::new(id, Role::Plugin)
            .with_name("CodeSearch")
            .with_agent_id("codesearch-plugin-v1");

        let json = serde_json::to_string(&p).unwrap();
        let back: Participant = serde_json::from_str(&json).unwrap();
        assert_eq!(back.agent_id, Some("codesearch-plugin-v1".to_string()));
    }
}
