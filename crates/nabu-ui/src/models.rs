//! UI-side data model projections.
//!
//! The UI renders projections of existing domain types; it never owns data.
//! Types that exist in `nabu-core` are re-exported so there is a single source
//! of truth. Types that only exist in the UI layer (template contract, graph
//! edge, and the relation-type picker contract) are defined here.

pub mod graph {
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    /// Relation types selectable in the relation editor.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum RelationType {
        BelongsTo,
        WorksOn,
        RelatedTo,
        CreatedBy,
        References,
        MemberOf,
        DependsOn,
    }

    /// A directed edge between two knowledge objects.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct GraphEdge {
        pub id: Uuid,
        pub source: Uuid,
        pub target: Uuid,
        pub relation: RelationType,
    }

    /// One node in the knowledge graph (mirrors the backend `GraphNode`).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct GraphNodeData {
        pub path: String,
        pub title: String,
        pub folder: String,
        pub modified_at: String,
        pub tags: Vec<String>,
        pub backlink_count: usize,
        pub outgoing_count: usize,
        pub degree: usize,
    }

    /// One edge in the knowledge graph (mirrors the backend `GraphEdgeData`).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct GraphEdgeData {
        pub source: String,
        pub target: String,
        pub broken: bool,
    }

    /// Full graph payload (mirrors the backend `GraphData`).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct GraphData {
        pub nodes: Vec<GraphNodeData>,
        pub edges: Vec<GraphEdgeData>,
        pub orphan_count: usize,
        pub cluster_count: usize,
    }

    /// One backlink hit (mirrors the backend `BacklinkEntry`).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct BacklinkEntry {
        pub path: String,
        pub title: String,
        pub folder: String,
        pub snippet: String,
        pub match_start: usize,
        pub match_end: usize,
        pub count: usize,
    }

    /// One outgoing link (mirrors the backend `OutgoingLink`).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct OutgoingLink {
        /// `internal` (resolves to a note), `broken`, or `external` (URL).
        pub kind: String,
        /// Raw link text or URL.
        pub target: String,
        /// Resolved note path when `internal`.
        pub path: Option<String>,
        /// How many times this target is linked.
        pub count: usize,
    }

    /// One unlinked mention (mirrors the backend `MentionEntry`).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct MentionEntry {
        pub title: String,
        pub path: String,
        pub snippet: String,
        pub match_start: usize,
        pub match_end: usize,
        pub score: u32,
    }

    /// Backlinks, outgoing links and unlinked mentions for one note (mirrors
    /// the backend `NoteLinks`).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct NoteLinks {
        pub backlinks: Vec<BacklinkEntry>,
        pub outgoing: Vec<OutgoingLink>,
        pub mentions: Vec<MentionEntry>,
        /// Frontmatter tags of the inspected note.
        pub tags: Vec<String>,
    }
}

pub mod knowledge_object {
    pub use nabu_core::models::knowledge_object::*;
}

pub mod template {
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    /// A note template with per-folder assignment and property presets.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Template {
        pub name: String,
        pub description: Option<String>,
        pub icon: Option<String>,
        pub default_folder: Option<String>,
        pub frontmatter_defaults: HashMap<String, String>,
        pub property_presets: HashMap<String, serde_json::Value>,
        pub body: String,
        pub object_type: Option<String>,
    }
}
