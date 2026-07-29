//! Plugin Manifest — strongly typed metadata for plugin discovery and validation.
//!
//! Every plugin MUST provide a manifest that describes its identity,
//! dependencies, capabilities, and compatibility requirements.

use crate::plugin::version::{Version, VersionRequirement};

/// Strongly typed plugin manifest with strict validation.
///
/// Plugins declare everything they need through this manifest.
/// No third-party code is executed during validation.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginManifest {
    /// Unique plugin identifier (reverse domain notation: "com.example.my-plugin").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Plugin version.
    pub version: Version,
    /// Author or organization.
    pub author: String,
    /// Brief description.
    pub description: String,
    /// Minimum supported Nabu API version.
    pub min_nabu_version: Version,
    /// Maximum Nabu version this plugin has been tested against.
    pub max_tested_version: Option<Version>,
    /// Plugin API version this manifest conforms to.
    pub manifest_version: u32,
    /// Capabilities this plugin provides.
    pub capabilities: Vec<PluginCapability>,
    /// Required dependencies — plugin IDs that must be present.
    pub dependencies: Vec<PluginDependency>,
    /// Optional dependencies — plugin IDs that may be present.
    pub optional_dependencies: Vec<PluginDependency>,
    /// Feature flags this plugin requires or supports.
    pub feature_flags: Vec<PluginFeatureFlag>,
    /// Required permissions for this plugin.
    pub permissions: Vec<PluginPermission>,
    /// Entry point type (future: "wasm", "lua", "native", etc.).
    pub entry_type: PluginEntryType,
}

impl PluginManifest {
    /// Validate the manifest for structural correctness.
    ///
    /// Returns a list of validation errors. An empty list means valid.
    pub fn validate(&self) -> Vec<ManifestError> {
        let mut errors = Vec::new();

        if self.id.is_empty() {
            errors.push(ManifestError::EmptyField("id"));
        }
        if self.name.is_empty() {
            errors.push(ManifestError::EmptyField("name"));
        }
        if self.author.is_empty() {
            errors.push(ManifestError::EmptyField("author"));
        }
        if self.description.is_empty() {
            errors.push(ManifestError::EmptyField("description"));
        }
        if self.manifest_version == 0 {
            errors.push(ManifestError::InvalidManifestVersion(self.manifest_version));
        }

        // Check dependency IDs are not empty
        for dep in &self.dependencies {
            if dep.plugin_id.is_empty() {
                errors.push(ManifestError::EmptyDependencyId);
            }
        }
        for dep in &self.optional_dependencies {
            if dep.plugin_id.is_empty() {
                errors.push(ManifestError::EmptyDependencyId);
            }
        }

        errors
    }

    /// Check if this manifest is compatible with the current Nabu version.
    pub fn check_nabu_compatibility(&self, nabu_version: &Version) -> CompatibilityCheck {
        // Check minimum version
        if self.min_nabu_version > *nabu_version {
            return CompatibilityCheck::Incompatible {
                reason: format!(
                    "Plugin '{}' requires Nabu >= {}. Current: {}",
                    self.id, self.min_nabu_version, nabu_version
                ),
            };
        }

        // Check maximum tested version (warning only)
        if let Some(max_tested) = &self.max_tested_version {
            if nabu_version > max_tested && nabu_version.major > max_tested.major {
                return CompatibilityCheck::Untested {
                    reason: format!(
                        "Plugin '{}' was tested up to Nabu {}. Current: {}. May still work.",
                        self.id, max_tested, nabu_version
                    ),
                };
            }
        }

        CompatibilityCheck::Compatible
    }
}

/// A capability that a plugin provides.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginCapability {
    pub id: String,
    pub description: String,
    pub version: Version,
}

/// A dependency on another plugin.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginDependency {
    pub plugin_id: String,
    pub version_requirement: VersionRequirement,
    pub optional: bool,
}

/// A feature flag for a plugin.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginFeatureFlag {
    pub name: String,
    pub enabled_by_default: bool,
    pub description: String,
}

/// A permission that a plugin requests.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginPermission {
    pub name: String,
    pub description: String,
    pub required: bool,
}

/// The type of plugin entry point.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginEntryType {
    /// Native Rust shared library (.dylib/.so/.dll).
    Native,
    /// WebAssembly module.
    Wasm,
    /// Lua script.
    Lua,
    /// Python script (future).
    Python,
    /// External process communicating via IPC.
    External,
}

impl Default for PluginEntryType {
    fn default() -> Self {
        Self::Wasm
    }
}

/// Result of a Nabu version compatibility check.
#[derive(Debug, Clone, PartialEq)]
pub enum CompatibilityCheck {
    /// Plugin is fully compatible.
    Compatible,
    /// Plugin is incompatible with the current Nabu version.
    Incompatible { reason: String },
    /// Plugin has not been tested with this version but may work.
    Untested { reason: String },
}

/// Validation errors for plugin manifests.
#[derive(Debug, Clone, PartialEq)]
pub enum ManifestError {
    EmptyField(&'static str),
    InvalidManifestVersion(u32),
    EmptyDependencyId,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyField(name) => write!(f, "Field '{}' must not be empty", name),
            Self::InvalidManifestVersion(v) => write!(f, "Invalid manifest version: {}", v),
            Self::EmptyDependencyId => write!(f, "Dependency plugin_id must not be empty"),
        }
    }
}

impl std::error::Error for ManifestError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest() -> PluginManifest {
        PluginManifest {
            id: "com.example.my-plugin".to_string(),
            name: "My Plugin".to_string(),
            version: Version::new(1, 0, 0),
            author: "Example Author".to_string(),
            description: "A test plugin".to_string(),
            min_nabu_version: Version::new(0, 1, 0),
            max_tested_version: None,
            manifest_version: 1,
            capabilities: vec![],
            dependencies: vec![],
            optional_dependencies: vec![],
            feature_flags: vec![],
            permissions: vec![],
            entry_type: PluginEntryType::Wasm,
        }
    }

    #[test]
    fn valid_manifest_passes_validation() {
        let manifest = valid_manifest();
        assert!(manifest.validate().is_empty());
    }

    #[test]
    fn empty_id_fails_validation() {
        let manifest = PluginManifest { id: String::new(), ..valid_manifest() };
        let errors = manifest.validate();
        assert!(errors.contains(&ManifestError::EmptyField("id")));
    }

    #[test]
    fn zero_manifest_version_fails() {
        let manifest = PluginManifest { manifest_version: 0, ..valid_manifest() };
        let errors = manifest.validate();
        assert!(errors.contains(&ManifestError::InvalidManifestVersion(0)));
    }

    #[test]
    fn compatibility_requires_min_version() {
        let manifest = valid_manifest();
        let nabu = Version::new(0, 0, 5);
        let result = manifest.check_nabu_compatibility(&nabu);
        assert!(matches!(result, CompatibilityCheck::Incompatible { .. }));
    }

    #[test]
    fn compatibility_ok_when_version_met() {
        let manifest = valid_manifest();
        let nabu = Version::new(1, 0, 0);
        let result = manifest.check_nabu_compatibility(&nabu);
        assert_eq!(result, CompatibilityCheck::Compatible);
    }

    #[test]
    fn untested_warning_when_beyond_max() {
        let manifest = PluginManifest {
            max_tested_version: Some(Version::new(1, 0, 0)),
            ..valid_manifest()
        };
        let nabu = Version::new(2, 0, 0);
        let result = manifest.check_nabu_compatibility(&nabu);
        assert!(matches!(result, CompatibilityCheck::Untested { .. }));
    }
}
