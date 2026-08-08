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

/// # Capability Platform — UI-side data model
///
/// The UI layer projects the backend [`Capability`] (defined in
/// `nabu-core::plugin::capability`) into a richer `CapabilitySummary` that
/// carries the runtime state the backend does **not** include in the
/// `capability_list` snapshot: the *enabled* flag and the *provider* name.
///
/// The `Capability` struct is the single source of truth for capability
/// *definitions* (namespace, name, description, required). The enabled state
/// is communicated reactively through `CapabilityStateChanged` events on the
/// EventBus — the UI reconciles the initial snapshot received from
/// `capability_list` with live state-change events.
pub mod capability {
    use nabu_core::plugin::capability::Capability;
    use serde::{Deserialize, Serialize};

    /// Re-export of the backend `Capability` type so callers can reference it
    /// through the UI models module without depending on `nabu-core` directly.
    pub use nabu_core::plugin::capability::Capability;

    /// Per-capability UI status, derived from the EventBus state-change events.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
    pub enum CapabilityStatus {
        /// The initial state before any data has been loaded.
        #[default]
        Unknown,
        /// The capability is currently enabled.
        Enabled,
        /// The capability is currently disabled.
        Disabled,
        /// An enable or disable operation is in flight.
        ///
        /// Carries the direction of the pending transition (`true` = enabling,
        /// `false` = disabling) so the UI can render the correct disabled
        /// toggle while the request is in progress.
        Pending(bool),
        /// The last enable/disable operation failed.  The inner string is the
        /// error message received from the backend.
        Error(String),
    }

    impl CapabilityStatus {
        /// Human-readable label for the status badge.
        pub fn label(&self) -> &'static str {
            match self {
                CapabilityStatus::Unknown => "Unknown",
                CapabilityStatus::Enabled => "Enabled",
                CapabilityStatus::Disabled => "Disabled",
                CapabilityStatus::Pending(true) => "Enabling…",
                CapabilityStatus::Pending(false) => "Disabling…",
                CapabilityStatus::Error(_) => "Error",
            }
        }

        /// Whether the capability is currently considered enabled.
        ///
        /// `Pending` transitions are resolved optimistically — `Pending(true)`
        /// reports `true` so the toggle shows the target state immediately.
        pub fn is_enabled(&self) -> bool {
            match self {
                CapabilityStatus::Enabled | CapabilityStatus::Pending(true) => true,
                CapabilityStatus::Disabled | CapabilityStatus::Pending(false) => false,
                CapabilityStatus::Unknown => false,
                CapabilityStatus::Error(msg) => !msg.is_empty(),
            }
        }
    }

    /// The UI-layer view of a single capability.
    ///
    /// Wraps the backend [`Capability`] definition and enriches it with the
    /// runtime fields the backend does not include in the `capability_list`
    /// snapshot:
    ///
    /// - `enabled` — the current enabled/disabled state, synchronised from
    ///   the backend `CapabilityRegistry` via `CapabilityStateChanged` events.
    /// - `provider` — which plugin or the host application provides this
    ///   capability (e.g. `"nabu"` for built-ins).
    /// - `status` — a finer-grained status badge (enabled / disabled /
    ///   loading / error).
    ///
    /// The struct is deserialisable directly from the `capability_list` IPC
    /// response because `#[serde(default)]` makes the runtime fields optional
    /// — if the backend later includes `provider` and `enabled` in its
    /// serialized form, the fields are picked up automatically; otherwise they
    /// default and are reconciled via events.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct CapabilitySummary {
        /// The backend capability definition (namespace, name, description,
        /// required flag).
        #[serde(flatten)]
        pub capability: Capability,
        /// Whether the capability is currently enabled.
        ///
        /// Defaults to `false` when the backend does not include this field
        /// in the `capability_list` response. The UI reconciles the real
        /// state from `CapabilityStateChanged` events and initial enable-state
        /// probes after listing.
        #[serde(default)]
        pub enabled: bool,
        /// The provider name (e.g. `"nabu"` for built-in capabilities, or a
        /// plugin ID for plugin-provided capabilities).
        #[serde(default)]
        pub provider: String,
        /// Fine-grained runtime status for the status indicator badge.
        #[serde(default)]
        #[serde(skip)]
        pub status: CapabilityStatus,
    }

    impl CapabilitySummary {
        /// Full identifier string: `{namespace}:{name}`.
        pub fn id(&self) -> String {
            self.capability.id()
        }

        /// Convenience constructor for tests and manual construction.
        pub fn from_capability(cap: Capability, provider: &str, enabled: bool) -> Self {
            let status = if enabled {
                CapabilityStatus::Enabled
            } else {
                CapabilityStatus::Disabled
            };
            Self {
                capability: cap,
                enabled,
                provider: provider.to_string(),
                status,
            }
        }
    }

    impl From<Capability> for CapabilitySummary {
        fn from(cap: Capability) -> Self {
            CapabilitySummary {
                capability: cap,
                enabled: false,
                provider: String::new(),
                status: CapabilityStatus::Unknown,
            }
        }
    }
}