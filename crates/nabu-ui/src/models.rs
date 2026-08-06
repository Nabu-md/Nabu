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
        #[serde(default)]
        pub category: Option<String>,
        #[serde(default)]
        pub favourite: bool,
        pub frontmatter_defaults: HashMap<String, String>,
        pub property_presets: HashMap<String, serde_json::Value>,
        pub body: String,
        pub object_type: Option<String>,
    }
}

/// Virtual collections for Knowledge Organisation (Phase 13.2).
pub mod organisation {
    use serde::{Deserialize, Serialize};

    /// A persisted smart-folder definition (mirrors the backend `SmartFolder`).
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    pub struct SmartFolder {
        pub id: String,
        pub name: String,
        #[serde(default)]
        pub icon: String,
        pub query: String,
        #[serde(default)]
        pub pinned: bool,
    }

    /// One archived note (mirrors the backend `ArchiveEntry`).
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    pub struct ArchiveEntry {
        pub archive_path: String,
        pub original_path: String,
        pub title: String,
        #[serde(default)]
        pub folder: String,
        #[serde(default)]
        pub modified_at: String,
    }

    /// One dated note for the calendar (mirrors the backend `CalendarEntry`).
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    pub struct CalendarEntry {
        pub path: String,
        pub title: String,
        #[serde(default)]
        pub folder: String,
        pub date: String,
        #[serde(default)]
        pub modified_at: String,
    }
}

/// Property editor data model — UI-layer definitions for metadata fields
/// that are displayed and edited in the Property Editor. These are
/// presentational: the backend stores values as `CustomPropertyValue`
/// inside `KnowledgeObject::custom_properties`; the editor projects them into
/// these typed structs for rendering, then calls back to the parent via
/// `on_change` / `on_validate`.

pub mod properties {
    use serde::{Deserialize, Serialize};

    /// The kind of a property field — determines which input control is rendered.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    pub enum PropertyType {
        Text,
        Number,
        Date,
        Select,
        MultiSelect,
        Url,
    }

    /// A typed property value — mirrors the UI-side projection of
    /// `CustomPropertyValue` (Text, Number, Date, Select, MultiSelect, Url).
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(untagged)]
    pub enum PropertyValue {
        Text(String),
        Number(f64),
        Date(String),
        Select(String),
        MultiSelect(Vec<String>),
        Url(String),
    }

    /// Definition of a single metadata property field.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct PropertyDefinition {
        pub id: String,
        pub display_name: String,
        pub property_type: PropertyType,
        pub description: Option<String>,
        pub default_value: Option<PropertyValue>,
        pub options: Option<Vec<String>>,
    }

    impl PropertyDefinition {
        /// Validates a value against this property definition.
        pub fn validate(&self, value: &PropertyValue) -> bool {
            match (&self.property_type, value) {
                (PropertyType::Text, PropertyValue::Text(_))
                | (PropertyType::Number, PropertyValue::Number(_))
                | (PropertyType::Date, PropertyValue::Date(_))
                | (PropertyType::Select, PropertyValue::Select(_))
                | (PropertyType::MultiSelect, PropertyValue::MultiSelect(_))
                | (PropertyType::Url, PropertyValue::Url(_)) => true,
                (PropertyType::Select, PropertyValue::Select(v)) => self
                    .options
                    .as_ref()
                    .map(|opts| opts.contains(v))
                    .unwrap_or(true),
                (PropertyType::MultiSelect, PropertyValue::MultiSelect(v)) => {
                    if let Some(opts) = &self.options {
                        v.iter().all(|val| opts.contains(val))
                    } else {
                        true
                    }
                }
                (PropertyType::Url, PropertyValue::Url(v)) => {
                    v.is_empty()
                        || v.starts_with("http://")
                        || v.starts_with("https://")
                        || v.starts_with("mailto:")
                }
                _ => false,
            }
        }

        /// Extracts the text representation of a value for display.
        pub fn value_text(value: &PropertyValue) -> String {
            match value {
                PropertyValue::Text(s) | PropertyValue::Date(s) | PropertyValue::Select(s) | PropertyValue::Url(s) => s.clone(),
                PropertyValue::Number(n) => n.to_string(),
                PropertyValue::MultiSelect(v) => v.join(", "),
            }
        }
    }
}
