//! Plugin manifest and metadata types.
//!
//! Defines [`PluginManifest`], [`PluginMetadata`], and compatibility
//! validation that describe what a plugin (or built-in component) is,
//! what it requires, and what it provides.
//!
//! This is **metadata only** — no plugin loading occurs.

use std::collections::HashSet;
use std::fmt;

use crate::plugin::capabilities;
use crate::plugin::version::{Version, VersionReq};

use super::Permission;
use super::PluginId;

/// Unique identifier for standard capability categories.
pub type CapabilityId = String;

/// Error returned when a manifest validation check fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// The plugin ID is empty or invalid.
    InvalidId(String),
    /// The version string could not be parsed.
    InvalidVersion(String),
    /// A required dependency is missing.
    MissingDependency(String),
    /// Incompatible version for a dependency.
    VersionMismatch { dependency: String, required: VersionReq, actual: Version },
    /// An unknown capability was declared.
    UnknownCapability(String),
    /// The manifest references another manifest that does not exist.
    MissingReference(String),
    /// A required field is missing.
    MissingField(String),
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::InvalidId(id) => write!(f, "Invalid plugin ID: '{}'", id),
            ManifestError::InvalidVersion(v) => write!(f, "Invalid version string: '{}'", v),
            ManifestError::MissingDependency(dep) => write!(f, "Missing required dependency: '{}'", dep),
            ManifestError::VersionMismatch { dependency, required, actual } => {
                write!(f, "Version mismatch for '{}': required {} but found {}", dependency, required, actual)
            }
            ManifestError::UnknownCapability(cap) => write!(f, "Unknown capability: '{}'", cap),
            ManifestError::MissingReference(ref_id) => write!(f, "Missing reference: '{}'", ref_id),
            ManifestError::MissingField(field) => write!(f, "Missing required field: '{}'", field),
        }
    }
}

impl std::error::Error for ManifestError {}

/// Metadata about a plugin's author and origin.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginMetadata {
    /// Human-readable name.
    pub name: String,
    /// Short description of what the plugin does.
    pub description: String,
    /// Author name or organization.
    pub author: Option<String>,
    /// URL to the plugin's homepage or repository.
    pub homepage: Option<String>,
    /// URL to the plugin's issue tracker.
    pub issues_url: Option<String>,
    /// License identifier (e.g., "MIT", "Apache-2.0").
    pub license: Option<String>,
    /// Arbitrary tags/categories for discovery.
    pub tags: Vec<String>,
}

impl PluginMetadata {
    /// Creates new metadata with only the required fields.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            author: None,
            homepage: None,
            issues_url: None,
            license: None,
            tags: Vec::new(),
        }
    }
}

/// Describes a dependency on another plugin or built-in service.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PluginDependency {
    /// The PluginId or CapabilityId being depended on.
    pub id: String,
    /// Version requirement for this dependency.
    pub version_req: VersionReq,
    /// Whether this dependency is optional.
    pub optional: bool,
}

impl PluginDependency {
    /// Creates a new required dependency.
    pub fn required(id: impl Into<String>, version_req: VersionReq) -> Self {
        Self {
            id: id.into(),
            version_req,
            optional: false,
        }
    }

    /// Creates a new optional dependency.
    pub fn optional(id: impl Into<String>, version_req: VersionReq) -> Self {
        Self {
            id: id.into(),
            version_req,
            optional: true,
        }
    }
}

/// Compatibility validation result for a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compatibility {
    /// Fully compatible — no issues found.
    Compatible,
    /// Compatible with warnings.
    CompatibleWithWarnings(Vec<String>),
    /// Incompatible — contains blocking errors.
    Incompatible(Vec<ManifestError>),
}

impl Compatibility {
    /// Returns `true` if this result is not `Compatible`.
    pub fn has_issues(&self) -> bool {
        matches!(self, Compatibility::CompatibleWithWarnings(_) | Compatibility::Incompatible(_))
    }

    /// Returns `true` if this result is `Incompatible`.
    pub fn is_blocking(&self) -> bool {
        matches!(self, Compatibility::Incompatible(_))
    }

    /// Merges two compatibility results together.
    pub fn merge(self, other: Compatibility) -> Compatibility {
        match (self, other) {
            (Compatibility::Compatible, other) => other,
            (Compatibility::CompatibleWithWarnings(mut w), Compatibility::CompatibleWithWarnings(w2)) => {
                w.extend(w2);
                Compatibility::CompatibleWithWarnings(w)
            }
            (Compatibility::CompatibleWithWarnings(w), Compatibility::Compatible) => {
                Compatibility::CompatibleWithWarnings(w)
            }
            (Compatibility::CompatibleWithWarnings(mut w), Compatibility::Incompatible(mut e)) => {
                w.extend(e.drain(..).map(|e| e.to_string()));
                Compatibility::CompatibleWithWarnings(w)
            }
            (Compatibility::Incompatible(mut e), Compatibility::Incompatible(e2)) => {
                e.extend(e2);
                Compatibility::Incompatible(e)
            }
            (Compatibility::Incompatible(e), _) => Compatibility::Incompatible(e),
        }
    }
}

/// Full plugin manifest describing identity, capabilities, dependencies, and requirements.
///
/// This is the canonical description of what a plugin is and what it does.
/// Built-in components also define manifests so that the system has a uniform
/// view of all capabilities regardless of origin.
///
/// # Examples
///
/// ```
/// use nabu_core::plugin::manifest::PluginManifest;
/// use nabu_core::plugin::PluginMetadata;
/// use nabu_core::plugin::version::{Version, VersionReq};
/// use nabu_core::plugin::capabilities;
///
/// let manifest = PluginManifest::new(
///     "my-ocr-engine",
///     PluginMetadata::new("My OCR Engine", "Provides OCR capabilities"),
///     Version::new(1, 0, 0),
///     Version::new(0, 1, 0), // min Nabu version
/// )
/// .with_capability(capabilities::OCR)
/// .with_dependency(PluginDependency::required(capabilities::STORAGE, VersionReq::any()));
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginManifest {
    /// Unique identifier for this plugin (e.g., "my-ocr-engine").
    pub id: PluginId,
    /// Human-readable metadata.
    pub metadata: PluginMetadata,
    /// Current version of this plugin.
    pub version: Version,
    /// Minimum version of Nabu required to run this plugin.
    pub min_nabu_version: Version,
    /// Capabilities this plugin provides (e.g., "nabu:ocr", "nabu:llm").
    pub capabilities: HashSet<CapabilityId>,
    /// Dependencies on other plugins or built-in capabilities.
    pub dependencies: Vec<PluginDependency>,
    /// Permissions this plugin requests at runtime.
    pub permissions: HashSet<Permission>,
    /// Features that can be toggled on/off.
    pub features: Vec<PluginFeature>,
    /// Whether this plugin is enabled by default.
    pub enabled_by_default: bool,
    /// Compatibility notes for future automated migration.
    pub compatibility_notes: Vec<String>,
}

/// An optional feature that a plugin provides.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PluginFeature {
    /// Feature identifier.
    pub id: String,
    /// Human-readable description.
    pub description: String,
    /// Whether this feature is enabled by default.
    pub default: bool,
}

impl PluginManifest {
    /// Creates a new plugin manifest with the minimum required fields.
    pub fn new(
        id: impl Into<String>,
        metadata: PluginMetadata,
        version: Version,
        min_nabu_version: Version,
    ) -> Self {
        Self {
            id: id.into(),
            metadata,
            version,
            min_nabu_version,
            capabilities: HashSet::new(),
            dependencies: Vec::new(),
            permissions: HashSet::new(),
            features: Vec::new(),
            enabled_by_default: true,
            compatibility_notes: Vec::new(),
        }
    }

    /// Adds a capability to this manifest.
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        self.capabilities.insert(capability.into());
        self
    }

    /// Adds a dependency to this manifest.
    pub fn with_dependency(mut self, dependency: PluginDependency) -> Self {
        self.dependencies.push(dependency);
        self
    }

    /// Adds a permission to this manifest.
    pub fn with_permission(mut self, permission: impl Into<String>) -> Self {
        self.permissions.insert(permission.into());
        self
    }

    /// Adds a feature to this manifest.
    pub fn with_feature(mut self, feature: PluginFeature) -> Self {
        self.features.push(feature);
        self
    }

    /// Validates the manifest structure and internal consistency.
    pub fn validate(&self) -> Result<(), Vec<ManifestError>> {
        let mut errors = Vec::new();

        if self.id.is_empty() {
            errors.push(ManifestError::InvalidId("Plugin ID cannot be empty".into()));
        }

        // Validate capability IDs follow "nabu:*" pattern
        for cap in &self.capabilities {
            if !cap.starts_with("nabu:") {
                errors.push(ManifestError::UnknownCapability(cap.clone()));
            }
        }

        if let Some(nabu_min_feature) = self.features.iter().find(|f| f.id == "min_nabu_version") {
            // Could add version negotiation here
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Checks compatibility with the current Nabu version.
    ///
    /// Returns `Compatible` if the manifest is compatible with the
    /// given Nabu version, or lists the issues found.
    pub fn check_compatibility(&self, nabu_version: &Version) -> Compatibility {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        // Check minimum Nabu version
        if !nabu_version.satisfies_minimum(&self.min_nabu_version) {
            errors.push(ManifestError::VersionMismatch {
                dependency: "nabu".into(),
                required: VersionReq::minimum(self.min_nabu_version.clone()),
                actual: nabu_version.clone(),
            });
        }

        // Check pre-release compatibility
        if self.version.is_pre_release() && nabu_version.is_pre_release() {
            warnings.push(format!(
                "Both plugin ({}) and Nabu ({}) are pre-release versions",
                self.version, nabu_version
            ));
        }

        // Warn about deprecated capabilities
        for cap in &self.capabilities {
            match cap.as_str() {
                c if c.starts_with("nabu:") => {
                    if !VALID_CAPABILITIES.contains(&c) && !deprecated_capabilities().contains(&c) {
                        warnings.push(format!("Unknown capability '{}' — may not be recognized", c));
                    }
                    if deprecated_capabilities().contains(&c) {
                        warnings.push(format!("Capability '{}' is deprecated", c));
                    }
                }
                _ => {}
            }
        }

        if !errors.is_empty() {
            Compatibility::Incompatible(errors)
        } else if !warnings.is_empty() {
            Compatibility::CompatibleWithWarnings(warnings)
        } else {
            Compatibility::Compatible
        }
    }

    /// Returns the `PluginDependency` entries that are required (not optional).
    pub fn required_dependencies(&self) -> impl Iterator<Item = &PluginDependency> {
        self.dependencies.iter().filter(|d| !d.optional)
    }

    /// Returns the `PluginDependency` entries that are optional.
    pub fn optional_dependencies(&self) -> impl Iterator<Item = &PluginDependency> {
        self.dependencies.iter().filter(|d| d.optional)
    }
}

/// Capability IDs that are recognized by the current Nabu version.
const VALID_CAPABILITIES: &[&str] = &[
    "nabu:search",
    "nabu:embeddings",
    "nabu:llm",
    "nabu:ocr",
    "nabu:stt",
    "nabu:export",
    "nabu:import",
    "nabu:capture",
    "nabu:processor",
    "nabu:graph",
    "nabu:storage",
    "nabu:event_bus",
    "nabu:theme",
    "nabu:content_provider",
    "nabu:workflow",
    "nabu:view",
];

/// Returns the set of currently deprecated capabilities.
pub fn deprecated_capabilities() -> &'static [&'static str] {
    &[]
}

/// Builder for constructing a [`PluginManifest`] incrementally.
#[derive(Debug, Default)]
pub struct PluginManifestBuilder {
    id: Option<String>,
    metadata: Option<PluginMetadata>,
    version: Option<Version>,
    min_nabu_version: Option<Version>,
    capabilities: HashSet<CapabilityId>,
    dependencies: Vec<PluginDependency>,
    permissions: HashSet<Permission>,
    features: Vec<PluginFeature>,
    enabled_by_default: bool,
    compatibility_notes: Vec<String>,
}

impl PluginManifestBuilder {
    /// Creates a new empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the plugin ID.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the plugin metadata.
    pub fn metadata(mut self, metadata: PluginMetadata) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Sets the plugin version.
    pub fn version(mut self, version: Version) -> Self {
        self.version = Some(version);
        self
    }

    /// Sets the minimum Nabu version.
    pub fn min_nabu_version(mut self, version: Version) -> Self {
        self.min_nabu_version = Some(version);
        self
    }

    /// Adds a capability.
    pub fn capability(mut self, cap: impl Into<String>) -> Self {
        self.capabilities.insert(cap.into());
        self
    }

    /// Adds a dependency.
    pub fn dependency(mut self, dep: PluginDependency) -> Self {
        self.dependencies.push(dep);
        self
    }

    /// Adds a permission.
    pub fn permission(mut self, perm: impl Into<String>) -> Self {
        self.permissions.insert(perm.into());
        self
    }

    /// Adds a feature.
    pub fn feature(mut self, feature: PluginFeature) -> Self {
        self.features.push(feature);
        self
    }

    /// Builds the manifest, or returns a list of validation errors.
    pub fn build(self) -> Result<PluginManifest, Vec<ManifestError>> {
        let mut errors: Vec<ManifestError> = Vec::new();

        let id = self.id.ok_or_else(|| vec![ManifestError::MissingField("id".into())])?;
        let metadata = self.metadata.ok_or_else(|| vec![ManifestError::MissingField("metadata".into())])?;
        let version = self.version.ok_or_else(|| vec![ManifestError::MissingField("version".into())])?;
        let min_nabu_version = self.min_nabu_version.unwrap_or_else(|| Version::new(0, 1, 0));

        if !errors.is_empty() {
            return Err(errors);
        }

        let manifest = PluginManifest {
            id,
            metadata,
            version,
            min_nabu_version,
            capabilities: self.capabilities,
            dependencies: self.dependencies,
            permissions: self.permissions,
            features: self.features,
            enabled_by_default: self.enabled_by_default,
            compatibility_notes: self.compatibility_notes,
        };

        manifest.validate()?;

        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::capabilities;

    fn test_manifest() -> PluginManifest {
        PluginManifest::new(
            "test-plugin",
            PluginMetadata::new("Test Plugin", "A test plugin"),
            Version::new(1, 0, 0),
            Version::new(0, 1, 0),
        )
        .with_capability(capabilities::OCR)
        .with_capability(capabilities::EXPORT)
    }

    #[test]
    fn manifest_creation() {
        let m = test_manifest();
        assert_eq!(m.id, "test-plugin");
        assert_eq!(m.metadata.name, "Test Plugin");
        assert_eq!(m.version, Version::new(1, 0, 0));
        assert!(m.capabilities.contains(capabilities::OCR));
        assert!(m.capabilities.contains(capabilities::EXPORT));
        assert!(m.enabled_by_default);
    }

    #[test]
    fn manifest_validation_passes() {
        let m = test_manifest();
        assert!(m.validate().is_ok());
    }

    #[test]
    fn manifest_validation_fails_empty_id() {
        let m = PluginManifest::new(
            "",
            PluginMetadata::new("Bad Plugin", "No ID"),
            Version::new(1, 0, 0),
            Version::new(0, 1, 0),
        );
        assert!(m.validate().is_err());
    }

    #[test]
    fn compatibility_check_passes() {
        let m = test_manifest();
        let nabu_version = Version::new(0, 1, 0);
        let compat = m.check_compatibility(&nabu_version);
        assert_eq!(compat, Compatibility::Compatible);
    }

    #[test]
    fn compatibility_check_fails_low_nabu_version() {
        let m = test_manifest();
        let nabu_version = Version::new(0, 0, 1);
        let compat = m.check_compatibility(&nabu_version);
        assert!(compat.is_blocking());
    }

    #[test]
    fn compatibility_warnings_for_unknown_capability() {
        let m = PluginManifest::new(
            "test",
            PluginMetadata::new("Test", "Unknown cap"),
            Version::new(1, 0, 0),
            Version::new(0, 1, 0),
        )
        .with_capability("nabu:unknown_capability");
        let nabu_version = Version::new(0, 1, 0);
        let compat = m.check_compatibility(&nabu_version);
        assert!(!compat.is_blocking()); // unknown cap is a warning, not error
    }

    #[test]
    fn builder_pattern() {
        let manifest = PluginManifestBuilder::new()
            .id("builder-test")
            .metadata(PluginMetadata::new("Builder Test", "Built via builder"))
            .version(Version::new(2, 0, 0))
            .min_nabu_version(Version::new(1, 0, 0))
            .capability(capabilities::LLM)
            .capability(capabilities::EMBEDDINGS)
            .dependency(PluginDependency::required(capabilities::STORAGE, VersionReq::any()))
            .build()
            .unwrap();

        assert_eq!(manifest.id, "builder-test");
        assert_eq!(manifest.version, Version::new(2, 0, 0));
        assert!(manifest.capabilities.contains(capabilities::LLM));
        assert_eq!(manifest.dependencies.len(), 1);
    }

    #[test]
    fn builder_missing_fields() {
        let result = PluginManifestBuilder::new().build();
        assert!(result.is_err());
    }

    #[test]
    fn dependency_required_vs_optional() {
        let dep_required = PluginDependency::required("nabu:storage", VersionReq::any());
        let dep_optional = PluginDependency::optional("nabu:llm", VersionReq::any());

        assert!(!dep_required.optional);
        assert!(dep_optional.optional);
    }
}
