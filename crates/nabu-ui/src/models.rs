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
