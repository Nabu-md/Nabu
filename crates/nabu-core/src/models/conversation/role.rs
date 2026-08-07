//! # Participant Role
//!
//! Defines the [`Role`] enum for conversation participants. This enum is
//! intentionally minimal and extensible — it does not enumerate AI model
//! variants or plugin types. Instead, it uses an open-ended `Participant`
//! model (see [`crate::models::conversation::Participant`]) for any
//! participant-specific metadata, keeping the canonical representation
//! transport-agnostic.
//!
//! The enum is `#[non_exhaustive]` so future phases can add roles (e.g.
//! `System`, `Workflow`, `Bridge`) without a breaking change.

use serde::{Deserialize, Serialize};

/// The role of a participant in a conversation.
///
/// This enum keeps the canonical roles small and stable. Future participants
/// — plugins, services, automation, external systems — are represented as
/// [`Participant`](crate::models::conversation::Participant) entries with
/// their role set to the most appropriate existing variant or to a new
/// variant added in a future phase.
///
/// # Extensibility
///
/// This enum is `#[non_exhaustive]`. External matchers must include a `_`
/// arm, so new roles can be added without breaking downstream consumers.
///
/// # Serialization
///
/// Serialized as a snake_case string (e.g. `"user"`, `"assistant"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Role {
    /// The human user initiating or participating in the conversation.
    User,

    /// An AI assistant participant. The specific model is identified via
    /// [`Participant::model`](crate::models::conversation::Participant::model).
    Assistant,

    /// A system-level participant that emits instructions, context, or
    /// guardrails. Not a human or an AI assistant.
    System,

    /// A plugin or extension participant. Identified via
    /// [`Participant::agent_id`](crate::models::conversation::Participant::agent_id).
    Plugin,

    /// A service or automation participant (e.g. scheduled workflow, CI agent).
    Service,

    /// Any participant role not covered by a known variant. Future-proofs
    /// deserialization of roles introduced in later phases.
    #[serde(other)]
    Other,
}

impl Role {
    /// Returns the canonical string name for this role variant.
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::Plugin => "plugin",
            Role::Service => "service",
            Role::Other => "other",
        }
    }
}

impl Default for Role {
    fn default() -> Self {
        Role::User
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_serializes_as_snake_case() {
        assert_eq!(serde_json::to_string(&Role::User).unwrap(), "\"user\"");
        assert_eq!(
            serde_json::to_string(&Role::Assistant).unwrap(),
            "\"assistant\""
        );
        assert_eq!(serde_json::to_string(&Role::System).unwrap(), "\"system\"");
        assert_eq!(serde_json::to_string(&Role::Plugin).unwrap(), "\"plugin\"");
        assert_eq!(serde_json::to_string(&Role::Service).unwrap(), "\"service\"");
        assert_eq!(serde_json::to_string(&Role::Other).unwrap(), "\"other\"");
    }

    #[test]
    fn role_deserializes_from_snake_case() {
        assert_eq!(serde_json::from_str::<Role>("\"user\"").unwrap(), Role::User);
        assert_eq!(
            serde_json::from_str::<Role>("\"assistant\"").unwrap(),
            Role::Assistant
        );
        assert_eq!(serde_json::from_str::<Role>("\"system\"").unwrap(), Role::System);
        assert_eq!(serde_json::from_str::<Role>("\"plugin\"").unwrap(), Role::Plugin);
        assert_eq!(serde_json::from_str::<Role>("\"service\"").unwrap(), Role::Service);
    }

    #[test]
    fn role_deserializes_unknown_as_other() {
        let role: Role = serde_json::from_str("\"custom_role\"").unwrap();
        assert_eq!(role, Role::Other);
    }

    #[test]
    fn role_default_is_user() {
        assert_eq!(Role::default(), Role::User);
    }

    #[test]
    fn role_display_matches_as_str() {
        assert_eq!(format!("{}", Role::User), "user");
        assert_eq!(format!("{}", Role::Assistant), "assistant");
        assert_eq!(format!("{}", Role::System), "system");
        assert_eq!(format!("{}", Role::Plugin), "plugin");
        assert_eq!(format!("{}", Role::Service), "service");
        assert_eq!(format!("{}", Role::Other), "other");
    }

    #[test]
    fn role_round_trips_all_variants() {
        for role in [
            Role::User,
            Role::Assistant,
            Role::System,
            Role::Plugin,
            Role::Service,
            Role::Other,
        ] {
            let json = serde_json::to_string(&role).unwrap();
            let back: Role = serde_json::from_str(&json).unwrap();
            assert_eq!(role, back);
        }
    }
}
