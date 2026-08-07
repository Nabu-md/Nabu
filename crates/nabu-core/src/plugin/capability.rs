//! Capability Registry — describes what the system and plugins can provide.
//!
//! Capabilities are typed identifiers that plugins declare and the application
//! uses to discover available extensions. Each capability represents a
//! well-defined extension point in the Nabu platform.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt;

/// A unique capability identifier.
///
/// Capabilities follow the format `{namespace}:{name}` (e.g., `nabu:capture`,
/// `plugin_xyz:ocr_provider`).
///
/// # Serialization
///
/// All fields are persistent data (identifiers, metadata, and configuration).
/// There are no runtime-only fields. This type is fully serializable via Serde
/// `Serialize`/`Deserialize` derives, enabling future capability manifests,
/// workspace persistence, and synchronization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Capability {
    /// The namespace (e.g., "nabu", "plugin_xyz", "community").
    pub namespace: String,
    /// The capability name (e.g., "capture", "ocr_provider", "exporter").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Whether this capability is required for basic operation.
    pub required: bool,
}

impl Capability {
    pub fn new(namespace: &str, name: &str, description: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            required: false,
        }
    }

    pub fn required(namespace: &str, name: &str, description: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            required: true,
        }
    }

    /// Full identifier string: `{namespace}:{name}`.
    pub fn id(&self) -> String {
        format!("{}:{}", self.namespace, self.name)
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.namespace, self.name)
    }
}

// ---------------------------------------------------------------------------
// Built-in capability constants (nabu:*)
// ---------------------------------------------------------------------------

/// Returns the set of built-in capabilities that Nabu provides.
pub fn builtin_capabilities() -> Vec<Capability> {
    vec![
        Capability::required("nabu", "event_bus", "Publish/subscribe messaging backbone"),
        Capability::required("nabu", "storage", "Knowledge object persistence"),
        Capability::required("nabu", "capture", "Content capture and ingestion"),
        Capability::required("nabu", "processor", "Content processing pipeline"),
        Capability::required("nabu", "graph", "Semantic relationship graph"),
        Capability::new("nabu", "export", "Document export to various formats"),
        Capability::new("nabu", "search", "Full-text search engine"),
        Capability::new("nabu", "ocr", "Optical character recognition"),
        Capability::new("nabu", "ai", "AI provider integration"),
        Capability::new("nabu", "embedding", "Vector embedding generation"),
        Capability::new("nabu", "template", "Note template management"),
        Capability::new("nabu", "sync", "Vault synchronization"),
        Capability::new("nabu", "watch", "File system watching"),
        Capability::new("nabu", "plugin", "Plugin management lifecycle"),
    ]
}

// ---------------------------------------------------------------------------
// Capability Registry
// ---------------------------------------------------------------------------

/// Thread-safe registry of capabilities that the system and plugins provide.
///
/// The capability registry is the single source of truth for what the
/// Nabu platform can do. Plugins declare capabilities when they register,
/// and the application uses this registry to discover available extensions.
///
/// # Serialization
///
/// All fields are persistent data (capability definitions, provider mappings,
/// and enabled sets). There are no runtime-only resources. This type is
/// fully serializable. The `#[serde(default)]` attribute ensures forward
/// compatibility — future versions may add fields without breaking
/// deserialization of existing serialized data.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CapabilityRegistry {
    /// Map from capability ID to capability definition.
    capabilities: HashMap<String, Capability>,
    /// Map from capability ID to the plugin that provides it.
    providers: HashMap<String, String>,
    /// Set of capability IDs that are currently enabled.
    enabled: HashSet<String>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self {
            capabilities: HashMap::new(),
            providers: HashMap::new(),
            enabled: HashSet::new(),
        }
    }

    /// Register a single capability.
    pub fn register(&mut self, capability: Capability, provider: &str) {
        let id = capability.id();
        self.capabilities.insert(id.clone(), capability);
        self.providers.insert(id, provider.to_string());
    }

    /// Register a set of built-in capabilities.
    pub fn register_builtin(&mut self) {
        for cap in builtin_capabilities() {
            let cap_name = cap.name.clone();
            let id = cap.id();
            self.capabilities.insert(id.clone(), cap);
            self.providers.insert(id, "nabu".to_string());
            self.enable(&format!("{}:{}", "nabu", cap_name));
        }
    }

    /// Check if a capability is registered.
    pub fn has(&self, id: &str) -> bool {
        self.capabilities.contains_key(id)
    }

    /// Get a capability by ID.
    pub fn get(&self, id: &str) -> Option<&Capability> {
        self.capabilities.get(id)
    }

    /// Get the provider of a capability.
    pub fn provider(&self, id: &str) -> Option<&str> {
        self.providers.get(id).map(|s| s.as_str())
    }

    /// Enable a capability.
    ///
    /// Prefer [`enable_checked`](Self::enable_checked) for IPC-driven state
    /// changes, which validates the transition and rejects duplicates.
    pub fn enable(&mut self, id: &str) {
        if self.capabilities.contains_key(id) {
            self.enabled.insert(id.to_string());
        }
    }

    /// Disable a capability.
    ///
    /// Prefer [`disable_checked`](Self::disable_checked) for IPC-driven state
    /// changes, which validates the transition and rejects duplicates.
    pub fn disable(&mut self, id: &str) {
        self.enabled.remove(id);
    }

    /// Enable a capability with full validation of the state transition.
    ///
    /// This is the canonical entry point for runtime capability activation
    /// (e.g. from the PluginManager or the `capability_enable` IPC command).
    /// It is the *single* operation that mutates a capability's enabled state:
    /// all transitions must route through the registry.
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - the capability is **not registered**: `"Capability '<id>' not found in registry"`
    /// - the capability is **already enabled**: `"Capability '<id>' is already enabled"`
    pub fn enable_checked(&mut self, id: &str) -> Result<(), String> {
        if !self.capabilities.contains_key(id) {
            tracing::warn!(capability = %id, "Enable failed: capability not registered");
            return Err(format!("Capability '{}' not found in registry", id));
        }
        if self.enabled.contains(id) {
            tracing::warn!(capability = %id, "Enable failed: already enabled");
            return Err(format!("Capability '{}' is already enabled", id));
        }
        self.enabled.insert(id.to_string());
        tracing::info!(capability = %id, "Capability enabled");
        Ok(())
    }

    /// Disable a capability with full validation of the state transition.
    ///
    /// This is the canonical entry point for runtime capability deactivation
    /// and the inverse of [`enable_checked`](Self::enable_checked).
    ///
    /// # Errors
    ///
    /// Returns `Err` when:
    /// - the capability is **not registered**: `"Capability '<id>' not found in registry"`
    /// - the capability is **already disabled**: `"Capability '<id>' is already disabled"`
    pub fn disable_checked(&mut self, id: &str) -> Result<(), String> {
        if !self.capabilities.contains_key(id) {
            tracing::warn!(capability = %id, "Disable failed: capability not registered");
            return Err(format!("Capability '{}' not found in registry", id));
        }
        if !self.enabled.contains(id) {
            tracing::warn!(capability = %id, "Disable failed: already disabled");
            return Err(format!("Capability '{}' is already disabled", id));
        }
        self.enabled.remove(id);
        tracing::info!(capability = %id, "Capability disabled");
        Ok(())
    }

    /// Check if a capability is enabled.
    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.contains(id)
    }

    /// List all capability IDs.
    pub fn list(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.capabilities.keys().cloned().collect();
        ids.sort();
        ids
    }

    /// List all enabled capability IDs.
    pub fn list_enabled(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.enabled.iter().cloned().collect();
        ids.sort();
        ids
    }

    /// Number of registered capabilities.
    pub fn capability_count(&self) -> usize {
        self.capabilities.len()
    }

    /// Number of enabled capabilities.
    pub fn enabled_count(&self) -> usize {
        self.enabled.len()
    }

    /// Get all capabilities matching a namespace.
    pub fn by_namespace(&self, namespace: &str) -> Vec<&Capability> {
        self.capabilities
            .values()
            .filter(|c| c.namespace == namespace)
            .collect()
    }

    /// Check if a namespace has a specific capability.
    pub fn namespace_has(&self, namespace: &str, name: &str) -> bool {
        let id = format!("{}:{}", namespace, name);
        self.capabilities.contains_key(&id)
    }

    /// Get the set of capability IDs that a provider offers.
    pub fn provider_capabilities(&self, provider: &str) -> Vec<String> {
        self.providers
            .iter()
            .filter(|(_, p)| p.as_str() == provider)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Remove a single capability by its identifier, if it is registered.
    ///
    /// Removes the capability definition, its provider mapping, and its
    /// enabled state. Returns `true` if a capability was removed.
    ///
    /// This is the inverse of [`register`](Self::register). Like the rest of
    /// the registry API it is a single, atomic mutation.
    pub fn remove(&mut self, id: &str) -> bool {
        let removed = self.capabilities.remove(id).is_some();
        if removed {
            self.providers.remove(id);
            self.enabled.remove(id);
        }
        removed
    }

    /// Remove every capability owned by the given provider.
    ///
    /// Returns the IDs of the capabilities that were removed, sorted. This is
    /// used when a provider is unregistered from the [`PluginManager`] — the
    /// manager delegates the actual removal here so the registry remains the
    /// single owner of registered capability state.
    ///
    /// [`PluginManager`]: crate::plugin::PluginManager
    pub fn remove_by_provider(&mut self, provider: &str) -> Vec<String> {
        let mut removed: Vec<String> = self
            .providers
            .iter()
            .filter(|(_, p)| p.as_str() == provider)
            .map(|(id, _)| id.clone())
            .collect();
        removed.sort();
        for id in &removed {
            self.capabilities.remove(id);
            self.providers.remove(id);
            self.enabled.remove(id);
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_check() {
        let mut cr = CapabilityRegistry::new();
        cr.register(
            Capability::new("nabu", "test", "Test capability"),
            "test_provider",
        );
        assert!(cr.has("nabu:test"));
        assert_eq!(cr.provider("nabu:test"), Some("test_provider"));
    }

    #[test]
    fn enable_disable() {
        let mut cr = CapabilityRegistry::new();
        cr.register(Capability::new("test", "feature", "A feature"), "provider");
        assert!(!cr.is_enabled("test:feature"));
        cr.enable("test:feature");
        assert!(cr.is_enabled("test:feature"));
        cr.disable("test:feature");
        assert!(!cr.is_enabled("test:feature"));
    }

    #[test]
    fn enable_checked_transitions_and_validates() {
        let mut cr = CapabilityRegistry::new();
        cr.register(Capability::new("test", "feature", "A feature"), "provider");

        // Not-yet-enabled -> Ok and state flips.
        assert!(cr.enable_checked("test:feature").is_ok());
        assert!(cr.is_enabled("test:feature"));

        // Duplicate enable is rejected without a state change.
        let err = cr.enable_checked("test:feature").unwrap_err();
        assert!(err.contains("already enabled"));
        assert!(cr.enabled_count() == 1);
    }

    #[test]
    fn enable_checked_rejects_unknown_capability() {
        let mut cr = CapabilityRegistry::new();
        let err = cr.enable_checked("unknown:thing").unwrap_err();
        assert!(err.contains("not found"));
        assert!(!cr.is_enabled("unknown:thing"));
    }

    #[test]
    fn disable_checked_transitions_and_validates() {
        let mut cr = CapabilityRegistry::new();
        cr.register(Capability::new("test", "feature", "A feature"), "provider");
        cr.enable("test:feature");
        assert!(cr.is_enabled("test:feature"));

        // Active -> Ok and state flips.
        assert!(cr.disable_checked("test:feature").is_ok());
        assert!(!cr.is_enabled("test:feature"));

        // Duplicate disable is rejected without a state change.
        let err = cr.disable_checked("test:feature").unwrap_err();
        assert!(err.contains("already disabled"));
        assert!(!cr.is_enabled("test:feature"));
    }

    #[test]
    fn disable_checked_rejects_unknown_capability() {
        let mut cr = CapabilityRegistry::new();
        let err = cr.disable_checked("unknown:thing").unwrap_err();
        assert!(err.contains("not found"));
    }

    #[test]
    fn builtin_capabilities_have_valid_ids() {
        let caps = builtin_capabilities();
        for cap in &caps {
            assert!(cap.id().contains(':'));
            assert!(!cap.namespace.is_empty());
            assert!(!cap.name.is_empty());
        }
    }

    #[test]
    fn register_builtin() {
        let mut cr = CapabilityRegistry::new();
        cr.register_builtin();
        assert!(cr.has("nabu:event_bus"));
        assert!(cr.has("nabu:storage"));
        assert!(cr.has("nabu:capture"));
        assert!(cr.capability_count() >= 10);
    }

    #[test]
    fn by_namespace_filters() {
        let mut cr = CapabilityRegistry::new();
        cr.register_builtin();
        let nabu_caps = cr.by_namespace("nabu");
        assert!(nabu_caps.len() >= 10);
    }

    #[test]
    fn remove_single_capability() {
        let mut cr = CapabilityRegistry::new();
        cr.register(Capability::new("ns", "a", "A"), "p1");
        cr.register(Capability::new("ns", "b", "B"), "p2");
        cr.enable("ns:a");

        assert!(cr.remove("ns:a"));
        assert!(!cr.has("ns:a"));
        assert!(!cr.is_enabled("ns:a"));
        assert_eq!(cr.provider("ns:a"), None);
        // Unrelated capability is untouched.
        assert!(cr.has("ns:b"));

        // Removing an unknown ID reports false.
        assert!(!cr.remove("ns:missing"));
    }

    #[test]
    fn remove_by_provider_removes_only_owned() {
        let mut cr = CapabilityRegistry::new();
        cr.register(Capability::new("ns", "a", "A"), "p1");
        cr.register(Capability::new("ns", "b", "B"), "p1");
        cr.register(Capability::new("ns", "c", "C"), "p2");
        cr.enable("ns:a");

        let removed = cr.remove_by_provider("p1");
        assert_eq!(removed, vec!["ns:a".to_string(), "ns:b".to_string()]);
        assert!(!cr.has("ns:a"));
        assert!(!cr.has("ns:b"));
        assert!(!cr.is_enabled("ns:a"));
        // Capabilities owned by other providers remain.
        assert!(cr.has("ns:c"));
        assert_eq!(cr.provider_capabilities("p1"), Vec::<String>::new());
    }

    #[test]
    fn remove_by_provider_unknown_returns_empty() {
        let mut cr = CapabilityRegistry::new();
        cr.register_builtin();
        assert_eq!(cr.remove_by_provider("no_such_provider"), Vec::<String>::new());
        assert!(cr.has("nabu:event_bus"));
    }
}

#[cfg(test)]
mod capability_serialization {
    use super::*;
    use serde_json;

    #[test]
    fn capability_round_trips() {
        let cap = Capability::new("nabu", "capture", "Content capture and ingestion");
        let json = serde_json::to_string(&cap).expect("serialize");
        let back: Capability = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cap, back);
        assert_eq!(cap.id(), back.id());
    }

    #[test]
    fn capability_required_flag_preserved() {
        let cap = Capability::required("nabu", "event_bus", "Messaging backbone");
        assert!(cap.required);
        let json = serde_json::to_string(&cap).expect("serialize");
        let back: Capability = serde_json::from_str(&json).expect("deserialize");
        assert!(back.required);
        assert_eq!(cap, back);
    }

    #[test]
    fn capability_builtin_round_trips() {
        let caps = builtin_capabilities();
        for cap in &caps {
            let json = serde_json::to_string(cap).expect("serialize");
            let back: Capability = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(cap, &back);
            assert_eq!(cap.id(), back.id());
        }
    }

    #[test]
    fn capability_unicode_round_trip() {
        let cap = Capability {
            namespace: "plugin_\u{e9}".into(),
            name: "ocr".into(),
            description: "Optical character recognition".into(),
            required: false,
        };
        let json = serde_json::to_string(&cap).expect("serialize");
        let back: Capability = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cap, back);
    }

    #[test]
    fn registry_round_trips() {
        let mut registry = CapabilityRegistry::new();
        registry.register(
            Capability::new("nabu", "test", "Test capability"),
            "test_provider",
        );
        registry.register(
            Capability::required("nabu", "core", "Core capability"),
            "nabu",
        );
        registry.register_builtin();
        registry.enable("nabu:event_bus");
        registry.enable("nabu:capture");

        let json = serde_json::to_string(&registry).expect("serialize");
        let back: CapabilityRegistry = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(registry, back);
        assert_eq!(registry.capability_count(), back.capability_count());
        assert_eq!(registry.list(), back.list());
        assert_eq!(registry.list_enabled(), back.list_enabled());
    }

    #[test]
    fn registry_deserializes_empty() {
        let json = "{}";
        let registry: CapabilityRegistry = serde_json::from_str(json).expect("deserialize");
        assert_eq!(registry.capability_count(), 0);
        assert!(registry.list().is_empty());
    }

    #[test]
    fn capability_ignores_future_fields() {
        let json = r#"{"namespace":"nabu","name":"capture","description":"test","required":false,"future_field":"value"}"#;
        let cap: Capability = serde_json::from_str(json).expect("deserialize");
        assert_eq!(cap.namespace, "nabu");
        assert_eq!(cap.name, "capture");
        assert!(!cap.required);
    }

    #[test]
    fn capability_missing_field_yields_error() {
        let json = r#"{"namespace":"nabu","name":"capture"}"#;
        let result: Result<Capability, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }
}
