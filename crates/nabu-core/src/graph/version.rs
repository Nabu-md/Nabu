use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Current graph schema version.
/// Increment when the graph persistence format changes in a
/// backwards-incompatible way.
pub const CURRENT_GRAPH_SCHEMA_VERSION: u32 = 1;

/// The application name for provenance metadata.
pub const APPLICATION_NAME: &str = "nabu";

/// Current application version.
pub const APPLICATION_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Graph version metadata persisted alongside graph data.
///
/// This metadata enables:
/// - Automatic detection of out-of-date graph files
/// - Schema migration decisions
/// - Provenance tracking for rebuilds
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphVersion {
    /// Graph schema format version
    pub schema_version: u32,

    /// Application version that wrote this graph
    pub app_version: String,

    /// Application name
    pub app_name: String,

    /// Monotonically increasing generation counter.
    /// Increments on every full graph rebuild.
    pub generation: u64,

    /// When this graph was built
    pub built_at: DateTime<Utc>,

    /// When this graph was last saved
    pub saved_at: DateTime<Utc>,

    /// Number of times this graph has been rebuilt from scratch
    pub rebuild_count: u64,

    /// Source of the last build (e.g., "canonical", "recovery", "upgrade")
    pub build_source: BuildSource,

    /// Content hash of the serialized graph (hex-encoded SHA-256).
    /// Used for integrity verification.
    pub checksum: Option<String>,
}

impl GraphVersion {
    /// Create a new initial graph version.
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            schema_version: CURRENT_GRAPH_SCHEMA_VERSION,
            app_version: APPLICATION_VERSION.to_string(),
            app_name: APPLICATION_NAME.to_string(),
            generation: 1,
            built_at: now,
            saved_at: now,
            rebuild_count: 0,
            build_source: BuildSource::Initial,
            checksum: None,
        }
    }

    /// Create a version marking a full rebuild.
    pub fn rebuilt(build_source: BuildSource) -> Self {
        let now = Utc::now();
        Self {
            schema_version: CURRENT_GRAPH_SCHEMA_VERSION,
            app_version: APPLICATION_VERSION.to_string(),
            app_name: APPLICATION_NAME.to_string(),
            generation: 1, // Reset on rebuild
            built_at: now,
            saved_at: now,
            rebuild_count: 0, // Will be set by recovery logic
            build_source,
            checksum: None,
        }
    }

    /// Increment the generation counter.
    pub fn increment_generation(&mut self) {
        self.generation += 1;
    }

    /// Update the saved timestamp and optionally the checksum.
    pub fn mark_saved(&mut self, checksum: Option<String>) {
        self.saved_at = Utc::now();
        self.checksum = checksum;
    }

    /// Check if this version is compatible with the current schema.
    pub fn is_compatible(&self) -> bool {
        self.schema_version == CURRENT_GRAPH_SCHEMA_VERSION
    }

    /// Check if this version is older than the current schema.
    pub fn is_outdated(&self) -> bool {
        self.schema_version < CURRENT_GRAPH_SCHEMA_VERSION
    }

    /// Check if this version is newer than the current schema (future format).
    pub fn is_from_future(&self) -> bool {
        self.schema_version > CURRENT_GRAPH_SCHEMA_VERSION
    }
}

impl Default for GraphVersion {
    fn default() -> Self {
        Self::new()
    }
}

/// Describes how the graph was built.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum BuildSource {
    /// Initial graph build
    Initial,
    /// Rebuilt from canonical Markdown sources
    Canonical,
    /// Rebuilt due to corruption recovery
    Recovery,
    /// Rebuilt due to schema upgrade
    SchemaUpgrade,
    /// Rebuilt due to application version upgrade
    AppUpgrade,
    /// Rebuilt manually by the user
    Manual,
    /// Unknown build source
    Unknown,
}

/// Result of a version compatibility check.
#[derive(Debug, Clone, PartialEq)]
pub enum VersionCompatibility {
    /// Version is compatible — can load directly
    Compatible,
    /// Version is outdated — needs migration
    Outdated {
        schema_version: u32,
        current_version: u32,
    },
    /// Version is from the future — incompatible
    FutureVersion {
        schema_version: u32,
        current_version: u32,
    },
    /// No version metadata found (fresh start)
    Missing,
}

/// Check compatibility between a loaded version and the current schema.
pub fn check_compatibility(loaded: &Option<GraphVersion>) -> VersionCompatibility {
    match loaded {
        None => VersionCompatibility::Missing,
        Some(version) => {
            if version.is_compatible() {
                VersionCompatibility::Compatible
            } else if version.is_outdated() {
                VersionCompatibility::Outdated {
                    schema_version: version.schema_version,
                    current_version: CURRENT_GRAPH_SCHEMA_VERSION,
                }
            } else {
                VersionCompatibility::FutureVersion {
                    schema_version: version.schema_version,
                    current_version: CURRENT_GRAPH_SCHEMA_VERSION,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_version_is_compatible() {
        let v = GraphVersion::new();
        assert!(v.is_compatible());
        assert!(!v.is_outdated());
        assert!(!v.is_from_future());
    }

    #[test]
    fn test_outdated_version_detection() {
        let mut v = GraphVersion::new();
        v.schema_version = 0;
        assert!(v.is_outdated());
        assert!(!v.is_compatible());
    }

    #[test]
    fn test_future_version_detection() {
        let mut v = GraphVersion::new();
        v.schema_version = CURRENT_GRAPH_SCHEMA_VERSION + 1;
        assert!(v.is_from_future());
        assert!(!v.is_compatible());
    }

    #[test]
    fn test_compatibility_check() {
        assert_eq!(check_compatibility(&None), VersionCompatibility::Missing);

        let compatible = GraphVersion::new();
        assert_eq!(
            check_compatibility(&Some(compatible)),
            VersionCompatibility::Compatible
        );

        let mut outdated = GraphVersion::new();
        outdated.schema_version = 0;
        match check_compatibility(&Some(outdated)) {
            VersionCompatibility::Outdated { .. } => {} // expected
            other => panic!("Expected Outdated, got {:?}", other),
        }
    }

    #[test]
    fn test_generation_increment() {
        let mut v = GraphVersion::new();
        assert_eq!(v.generation, 1);
        v.increment_generation();
        assert_eq!(v.generation, 2);
    }
}
