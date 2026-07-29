//! Capability registry for describing and discovering services.
//!
//! The [`CapabilityRegistry`] is the central index of what capabilities
//! are available in the system — whether provided by built-in components
//! or by future plugins. It works together with [`PluginManifest`](super::PluginManifest)
//! to provide a uniform view of all services.
//!
//! # Design
//!
//! Each capability has an identifier (e.g., `"nabu:ocr"`, `"nabu:llm"`),
//! a provider name, and optional metadata. The registry supports:
//!
//! - Registering capabilities from built-in services
//! - Looking up providers for a given capability
//! - Discovering all capabilities in a category
//! - Checking whether a required capability is available
//! - Finding all providers of a specific capability

use std::collections::{HashMap, HashSet};
use std::sync::RwLock;

use crate::plugin::PluginId;
use crate::plugin::manifest::PluginDependency;
use crate::plugin::version::{Version, VersionReq};

/// Describes a single capability provided by a component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capability {
    /// The capability identifier (e.g., "nabu:ocr").
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Longer description of what this capability does.
    pub description: String,
    /// The plugin or component providing this capability.
    pub provider_id: PluginId,
    /// Version of the provider that implements this capability.
    pub provider_version: Version,
    /// Whether this capability is currently active/enabled.
    pub enabled: bool,
    /// Optional category for grouping related capabilities.
    pub category: Option<String>,
}

impl Capability {
    /// Creates a new capability descriptor.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        description: impl Into<String>,
        provider_id: impl Into<String>,
        provider_version: Version,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: description.into(),
            provider_id: provider_id.into(),
            provider_version,
            enabled: true,
            category: None,
        }
    }

    /// Sets the category for this capability.
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }
}

/// Result of a capability lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityStatus {
    /// The capability is available and enabled.
    Available,
    /// The capability is registered but currently disabled.
    Disabled,
    /// The capability is not registered at all.
    Unavailable,
    /// The capability exists but has a version mismatch.
    VersionMismatch { required: VersionReq, actual: Version },
}

/// Registry of all capabilities in the system.
///
/// Thread-safe and usable from any component. Built-in services register
/// themselves at startup via [`CapabilityRegistry::register`]. Future
/// third-party plugins will use the same path.
#[derive(Debug)]
pub struct CapabilityRegistry {
    /// Map from capability ID to all providers.
    capabilities: RwLock<HashMap<String, Vec<Capability>>>,
    /// Map from provider ID to the capabilities they provide.
    providers: RwLock<HashMap<PluginId, HashSet<String>>>,
}

impl CapabilityRegistry {
    /// Creates a new empty capability registry.
    pub fn new() -> Self {
        Self {
            capabilities: RwLock::new(HashMap::new()),
            providers: RwLock::new(HashMap::new()),
        }
    }

    /// Registers a capability with the registry.
    ///
    /// If the same capability ID is registered by multiple providers,
    /// both are tracked. Use [`get_providers`](Self::get_providers)
    /// to discover all providers for a given capability.
    pub fn register(&self, capability: Capability) {
        let mut caps = self.capabilities.write().expect("capability registry lock");
        let mut provs = self.providers.write().expect("provider registry lock");

        caps.entry(capability.id.clone())
            .or_default()
            .push(capability.clone());

        provs.entry(capability.provider_id.clone())
            .or_default()
            .insert(capability.id);
    }

    /// Unregisters all capabilities for a given provider.
    ///
    /// Used when a plugin is unloaded or disabled.
    pub fn unregister_provider(&self, provider_id: &str) {
        let mut caps = self.capabilities.write().expect("capability registry lock");
        let mut provs = self.providers.write().expect("provider registry lock");

        if let Some(removed_caps) = provs.remove(provider_id) {
            for cap_id in removed_caps {
                if let Some(providers) = caps.get_mut(&cap_id) {
                    providers.retain(|c| c.provider_id != provider_id);
                    if providers.is_empty() {
                        caps.remove(&cap_id);
                    }
                }
            }
        }
    }

    /// Returns all providers registered for the given capability.
    pub fn get_providers(&self, capability_id: &str) -> Vec<Capability> {
        let caps = self.capabilities.read().expect("capability registry lock");
        caps.get(capability_id)
            .map(|providers| {
                providers
                    .iter()
                    .filter(|c| c.enabled)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Returns the first enabled provider for a capability, if any.
    pub fn get_first_provider(&self, capability_id: &str) -> Option<Capability> {
        let caps = self.capabilities.read().expect("capability registry lock");
        caps.get(capability_id)
            .and_then(|providers| providers.iter().find(|c| c.enabled).cloned())
    }

    /// Checks whether a capability is available and optionally matches
    /// a version requirement.
    pub fn check_capability(&self, capability_id: &str) -> CapabilityStatus {
        let caps = self.capabilities.read().expect("capability registry lock");
        match caps.get(capability_id) {
            None => CapabilityStatus::Unavailable,
            Some(providers) => {
                if providers.iter().any(|c| c.enabled) {
                    CapabilityStatus::Available
                } else {
                    CapabilityStatus::Disabled
                }
            }
        }
    }

    /// Returns all capabilities of a given category.
    pub fn get_by_category(&self, category: &str) -> Vec<Capability> {
        let caps = self.capabilities.read().expect("capability registry lock");
        caps.values()
            .flatten()
            .filter(|c| c.category.as_deref() == Some(category))
            .cloned()
            .collect()
    }

    /// Returns all capabilities provided by a specific provider.
    pub fn get_provider_capabilities(&self, provider_id: &str) -> Vec<Capability> {
        let caps = self.capabilities.read().expect("capability registry lock");
        caps.values()
            .flatten()
            .filter(|c| c.provider_id == provider_id)
            .cloned()
            .collect()
    }

    /// Returns all registered capability IDs.
    pub fn all_capability_ids(&self) -> Vec<String> {
        let caps = self.capabilities.read().expect("capability registry lock");
        caps.keys().cloned().collect()
    }

    /// Returns all registered provider IDs.
    pub fn all_providers(&self) -> Vec<PluginId> {
        let provs = self.providers.read().expect("provider registry lock");
        provs.keys().cloned().collect()
    }

    /// Enables or disables a specific capability for a provider.
    pub fn set_enabled(&self, capability_id: &str, provider_id: &str, enabled: bool) -> bool {
        let mut caps = self.capabilities.write().expect("capability registry lock");
        if let Some(providers) = caps.get_mut(capability_id) {
            for cap in providers.iter_mut() {
                if cap.provider_id == provider_id {
                    cap.enabled = enabled;
                    return true;
                }
            }
        }
        false
    }

    /// Returns the number of registered capabilities.
    pub fn capability_count(&self) -> usize {
        let caps = self.capabilities.read().expect("capability registry lock");
        caps.values().map(|v| v.len()).sum()
    }

    /// Returns the number of registered providers.
    pub fn provider_count(&self) -> usize {
        let provs = self.providers.read().expect("provider registry lock");
        provs.len()
    }

    /// Registers a set of built-in capabilities that are always available.
    ///
    /// This is called once at startup to register Nabu's own services.
    pub fn register_builtin_capabilities(&self, nabu_version: &Version) {
        // Event bus is always available
        self.register(Capability::new(
            crate::plugin::capabilities::EVENT_BUS,
            "Event Bus",
            "Typed event communication bus",
            "nabu-core",
            nabu_version.clone(),
        ));

        // Storage is always available
        self.register(Capability::new(
            crate::plugin::capabilities::STORAGE,
            "Storage Manager",
            "File-based object storage with markdown persistence",
            "nabu-core",
            nabu_version.clone(),
        ));

        // Capture is always available
        self.register(Capability::new(
            crate::plugin::capabilities::CAPTURE,
            "Capture Engine",
            "Captures knowledge from clipboard, files, bookmarks, and more",
            "nabu-core",
            nabu_version.clone(),
        ));

        // Processing pipeline
        self.register(Capability::new(
            crate::plugin::capabilities::PROCESSOR,
            "Processing Pipeline",
            "Processes captured knowledge through registered processors",
            "nabu-core",
            nabu_version.clone(),
        ));

        // Graph
        self.register(Capability::new(
            crate::plugin::capabilities::GRAPH,
            "Vault Graph",
            "Knowledge relationship graph",
            "nabu-core",
            nabu_version.clone(),
        ));

        // Export
        self.register(Capability::new(
            crate::plugin::capabilities::EXPORT,
            "Export Engine",
            "Exports knowledge to various formats",
            "nabu-core",
            nabu_version.clone(),
        ));

        // Search
        self.register(Capability::new(
            crate::plugin::capabilities::SEARCH,
            "Search",
            "Tantivy-based full-text search",
            "nabu-core",
            nabu_version.clone(),
        ));

        // Import
        self.register(Capability::new(
            crate::plugin::capabilities::IMPORT,
            "Import Engine",
            "Imports knowledge from external sources",
            "nabu-core",
            nabu_version.clone(),
        ));

        // Content provider
        self.register(Capability::new(
            crate::plugin::capabilities::CONTENT_PROVIDER,
            "Content Provider",
            "Fetches content from URLs and APIs",
            "nabu-core",
            nabu_version.clone(),
        ));

        // Theme
        self.register(Capability::new(
            crate::plugin::capabilities::THEME,
            "Theme Manager",
            "Manages UI themes and styling",
            "nabu-core",
            nabu_version.clone(),
        ));
    }

    /// Validates that a set of dependencies can be satisfied.
    ///
    /// Returns a list of missing or incompatible dependencies.
    pub fn check_dependencies(&self, dependencies: &[PluginDependency]) -> Vec<String> {
        let mut issues = Vec::new();

        for dep in dependencies {
            let status = self.check_capability(&dep.id);
            match status {
                CapabilityStatus::Available => {
                    // Check version requirement
                    if let Some(provider) = self.get_first_provider(&dep.id) {
                        if !dep.version_req.matches(&provider.provider_version) {
                            issues.push(format!(
                                "Capability '{}' version mismatch: required {} but provider '{}' has version {}",
                                dep.id, dep.version_req, provider.provider_id, provider.provider_version
                            ));
                        }
                    }
                }
                CapabilityStatus::Disabled => {
                    if !dep.optional {
                        issues.push(format!("Required capability '{}' is disabled", dep.id));
                    }
                }
                CapabilityStatus::Unavailable => {
                    if !dep.optional {
                        issues.push(format!("Required capability '{}' is not available", dep.id));
                    }
                }
                CapabilityStatus::VersionMismatch { required, actual } => {
                    issues.push(format!(
                        "Capability '{}' has version {} but version {} is required",
                        dep.id, actual, required
                    ));
                }
            }
        }

        issues
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::CapabilityStatusExt;

    fn test_registry() -> CapabilityRegistry {
        let reg = CapabilityRegistry::new();
        reg.register(Capability::new(
            "nabu:ocr",
            "OCR Engine",
            "Optical character recognition",
            "test-ocr",
            Version::new(1, 0, 0),
        ));
        reg.register(Capability::new(
            "nabu:llm",
            "LLM Service",
            "Language model inference",
            "test-llm",
            Version::new(2, 0, 0),
        ));
        reg
    }

    #[test]
    fn register_and_lookup() {
        let reg = test_registry();
        let providers = reg.get_providers("nabu:ocr");
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].provider_id, "test-ocr");
    }

    #[test]
    fn unregister_provider() {
        let reg = test_registry();
        reg.unregister_provider("test-llm");
        assert!(reg.get_providers("nabu:llm").is_empty());
        assert_eq!(reg.get_providers("nabu:ocr").len(), 1);
    }

    #[test]
    fn check_available() {
        let reg = test_registry();
        assert_eq!(reg.check_capability("nabu:ocr"), CapabilityStatus::Available);
        assert_eq!(reg.check_capability("nabu:unknown"), CapabilityStatus::Unavailable);
    }

    #[test]
    fn enable_disable() {
        let reg = test_registry();
        assert!(reg.set_enabled("nabu:ocr", "test-ocr", false));
        assert_eq!(reg.check_capability("nabu:ocr"), CapabilityStatus::Disabled);
        assert!(reg.get_providers("nabu:ocr").is_empty());

        // Re-enable
        assert!(reg.set_enabled("nabu:ocr", "test-ocr", true));
        assert_eq!(reg.check_capability("nabu:ocr"), CapabilityStatus::Available);
    }

    #[test]
    fn multiple_providers() {
        let reg = CapabilityRegistry::new();
        reg.register(Capability::new(
            "nabu:ocr",
            "OCR Engine A",
            "First OCR provider",
            "provider-a",
            Version::new(1, 0, 0),
        ));
        reg.register(Capability::new(
            "nabu:ocr",
            "OCR Engine B",
            "Second OCR provider",
            "provider-b",
            Version::new(2, 0, 0),
        ));

        let providers = reg.get_providers("nabu:ocr");
        assert_eq!(providers.len(), 2);
    }

    #[test]
    fn builtin_registration() {
        let reg = CapabilityRegistry::new();
        let nabu_version = Version::new(0, 1, 0);
        reg.register_builtin_capabilities(&nabu_version);

        assert!(reg.capability_count() >= 9);
        assert!(reg.check_capability("nabu:event_bus").is_available());
        assert!(reg.check_capability("nabu:storage").is_available());
    }

    #[test]
    fn check_dependencies_all_satisfied() {
        let reg = test_registry();
        let deps = vec![
            PluginDependency::required("nabu:ocr", VersionReq::any()),
            PluginDependency::required("nabu:llm", VersionReq::any()),
        ];
        let issues = reg.check_dependencies(&deps);
        assert!(issues.is_empty());
    }

    #[test]
    fn check_dependencies_missing() {
        let reg = test_registry();
        let deps = vec![PluginDependency::required("nabu:not_real", VersionReq::any())];
        let issues = reg.check_dependencies(&deps);
        assert!(!issues.is_empty());
        assert!(issues[0].contains("not available"));
    }

    #[test]
    fn provider_capabilities_list() {
        let reg = test_registry();
        let caps = reg.get_provider_capabilities("test-ocr");
        assert_eq!(caps.len(), 1);
        assert_eq!(caps[0].id, "nabu:ocr");
    }
}

/// Extension trait to check `CapabilityStatus` conveniently via pattern matching.
pub trait CapabilityStatusExt {
    fn is_available(&self) -> bool;
}

impl CapabilityStatusExt for CapabilityStatus {
    fn is_available(&self) -> bool {
        matches!(self, CapabilityStatus::Available)
    }
}
