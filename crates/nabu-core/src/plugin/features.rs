//! Feature Flags — integrated plugin feature flags for the application.
//!
//! Support:
//!   - Experimental features
//!   - Staged rollout
//!   - Disabled capabilities
//!   - Compatibility switches
//!
//! All local.

use std::collections::{HashMap, HashSet};

/// A feature flag for the plugin system.
///
/// Feature flags control which plugin capabilities are enabled.
/// All flags are local — nothing is ever sent to external servers.
#[derive(Debug, Clone, PartialEq)]
pub struct FeatureFlag {
    /// Unique flag name (e.g., "plugin.wasm.alpha", "graph.advanced").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Whether this flag is enabled by default.
    pub enabled_by_default: bool,
    /// Current enabled state (may differ from default).
    pub enabled: bool,
    /// Stage of this feature (stable, beta, alpha, experimental).
    pub stage: FeatureStage,
}

/// The maturity stage of a feature flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FeatureStage {
    /// Fully stable and supported.
    Stable,
    /// Feature-complete but may have minor issues.
    Beta,
    /// In early testing, may change significantly.
    Alpha,
    /// Experimental, may be removed without notice.
    Experimental,
}

impl FeatureStage {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
            Self::Alpha => "alpha",
            Self::Experimental => "experimental",
        }
    }
}

/// Central feature flag registry for the application.
///
/// Manages the lifecycle and state of all feature flags.
/// All operations are local — nothing is ever sent to external services.
#[derive(Debug, Clone, Default)]
pub struct FeatureRegistry {
    flags: HashMap<String, FeatureFlag>,
    /// Set of overridden flag names (user explicitly set them).
    overrides: HashSet<String>,
}

impl FeatureRegistry {
    pub fn new() -> Self {
        Self {
            flags: HashMap::new(),
            overrides: HashSet::new(),
        }
    }

    /// Register a feature flag.
    pub fn register(
        &mut self,
        name: &str,
        description: &str,
        stage: FeatureStage,
        enabled_by_default: bool,
    ) {
        self.flags.insert(
            name.to_string(),
            FeatureFlag {
                name: name.to_string(),
                description: description.to_string(),
                enabled_by_default,
                enabled: enabled_by_default,
                stage,
            },
        );
    }

    /// Check if a feature flag is enabled.
    pub fn is_enabled(&self, name: &str) -> bool {
        self.flags.get(name).map(|f| f.enabled).unwrap_or(false)
    }

    /// Enable a feature flag.
    pub fn enable(&mut self, name: &str) {
        if let Some(flag) = self.flags.get_mut(name) {
            flag.enabled = true;
            self.overrides.insert(name.to_string());
        }
    }

    /// Disable a feature flag.
    pub fn disable(&mut self, name: &str) {
        if let Some(flag) = self.flags.get_mut(name) {
            flag.enabled = false;
            self.overrides.insert(name.to_string());
        }
    }

    /// Reset a flag to its default state.
    pub fn reset(&mut self, name: &str) {
        if let Some(flag) = self.flags.get_mut(name) {
            flag.enabled = flag.enabled_by_default;
            self.overrides.remove(name);
        }
    }

    /// List all registered feature flags.
    pub fn list(&self) -> Vec<&FeatureFlag> {
        let mut flags: Vec<&FeatureFlag> = self.flags.values().collect();
        flags.sort_by(|a, b| a.name.cmp(&b.name));
        flags
    }

    /// List flags by stage.
    pub fn by_stage(&self, stage: FeatureStage) -> Vec<&FeatureFlag> {
        self.flags.values().filter(|f| f.stage == stage).collect()
    }

    /// List flags that have been overridden from their default.
    pub fn overridden(&self) -> Vec<&FeatureFlag> {
        self.flags
            .values()
            .filter(|f| self.overrides.contains(&f.name))
            .collect()
    }

    /// Number of registered flags.
    pub fn count(&self) -> usize {
        self.flags.len()
    }

    /// Register standard plugin system feature flags.
    pub fn register_standard_flags(&mut self) {
        self.register(
            "plugin.wasm",
            "Enable WebAssembly plugin support",
            FeatureStage::Experimental,
            false,
        );
        self.register(
            "plugin.lua",
            "Enable Lua plugin support",
            FeatureStage::Experimental,
            false,
        );
        self.register(
            "plugin.external",
            "Enable external process plugin support",
            FeatureStage::Alpha,
            false,
        );
        self.register(
            "plugin.dev_mode",
            "Enable plugin development mode (verbose logging, hot-reload)",
            FeatureStage::Beta,
            false,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_check() {
        let mut fr = FeatureRegistry::new();
        fr.register("test.feature", "A test feature", FeatureStage::Stable, true);
        assert!(fr.is_enabled("test.feature"));
    }

    #[test]
    fn enable_disable() {
        let mut fr = FeatureRegistry::new();
        fr.register("test.feature", "Test", FeatureStage::Stable, false);
        assert!(!fr.is_enabled("test.feature"));
        fr.enable("test.feature");
        assert!(fr.is_enabled("test.feature"));
        fr.disable("test.feature");
        assert!(!fr.is_enabled("test.feature"));
    }

    #[test]
    fn reset_restores_default() {
        let mut fr = FeatureRegistry::new();
        fr.register("test.feature", "Test", FeatureStage::Stable, false);
        fr.enable("test.feature");
        assert!(fr.is_enabled("test.feature"));
        fr.reset("test.feature");
        assert!(!fr.is_enabled("test.feature"));
    }

    #[test]
    fn unknown_flag_is_disabled() {
        let fr = FeatureRegistry::new();
        assert!(!fr.is_enabled("nonexistent"));
    }

    #[test]
    fn standard_flags() {
        let mut fr = FeatureRegistry::new();
        fr.register_standard_flags();
        assert_eq!(fr.count(), 4);
    }
}
