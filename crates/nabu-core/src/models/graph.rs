use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Semantic relation types used by [`VaultGraph`](crate::graph::VaultGraph).
///
/// # Architectural note (Principle 7 — One Graph)
///
/// `RelationType` is the single canonical relation enum for the entire
/// application. All graph operations — semantic edges, entity relations,
/// knowledge object links — use these variants. No subsystem should
/// introduce its own relation types.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RelationType {
    /// Object belongs to a parent container (e.g., folder, collection, project).
    BelongsTo,
    /// Object is actively being worked on.
    WorksOn,
    /// Free-form associative relation between two objects.
    RelatedTo,
    /// Object was created by a person or entity.
    CreatedBy,
    /// Object references or cites another object.
    References,
    /// Object is a member of a group or collection.
    MemberOf,
    /// Object depends on another object (e.g., task dependency).
    DependsOn,
}
