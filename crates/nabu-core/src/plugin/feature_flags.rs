//! Feature flag framework for runtime capability toggles.
//!
//! Provides a thread-safe [`FeatureFlags`] registry that allows for:
//!
//! - Defining feature flags with default values
//! - Toggling flags at runtime
//! - Grouping related flags into feature groups
//! - Checking if a feature is enabled
//! - Scoped overrides for testing
//! - Observing flag changes via callbacks

use std::collections::HashMap;
use std::sync::RwLock;

/// A single feature flag with metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureFlag {
    /// Unique identifier for this flag.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Description of what this flag controls.
    pub description: String,
    /// Whether this flag is enabled by default.
    pub default: bool,
    /// Current runtime value (may differ from default).
    pub enabled: bool,
    /// Category for grouping related flags.
    pub category: Option<String>,
    /// Whether this flag can be changed at runtime (vs. requiring restart).
    pub runtime_toggleable: bool,
    /// Whether this flag is stable and will not be removed.
    pub stable: bool,
}

impl FeatureFlag {
    /// Creates a new feature flag.
    pub fn new(id: impl Into<String>, name: impl Into<String>, description: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            enabled: true,
            default: true,
            runtime_toggleable: true,
            stable: true,
            category: None,
            id,
            name: name.into(),
            description: description.into(),
        }
    }

    /// Sets whether this flag is enabled by default.
    pub fn with_default(mut self, default: bool) -> Self {
        self.default = default;
        self.enabled = default;
        self
    }

    /// Sets whether this flag can be toggled at runtime.
    pub fn with_runtime_toggle(mut self, runtime: bool) -> Self {
        self.runtime_toggleable = runtime;
        self
    }

    /// Sets the category for this flag.
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Marks this flag as experimental (not stable).
    pub fn experimental(mut self) -> Self {
        self.stable = false;
        self
    }
}

/// Observer callback for feature flag changes.
type FlagChangeCallback = Box<dyn Fn(&str, bool) + Send + Sync>;

/// Thread-safe registry of feature flags with change notifications.
pub struct FeatureFlags {
    flags: RwLock<Vec<FeatureFlag>>,
    overrides: RwLock<HashMap<String, Option<bool>>>,
    callbacks: RwLock<Vec<FlagChangeCallback>>,
}

impl FeatureFlags {
    /// Creates a new empty feature flag registry.
    pub fn new() -> Self {
        Self {
            flags: RwLock::new(Vec::new()),
            overrides: RwLock::new(HashMap::new()),
            callbacks: RwLock::new(Vec::new()),
        }
    }

    /// Registers a feature flag.
    ///
    /// If a flag with the same ID already exists, it is replaced.
    pub fn register(&self, flag: FeatureFlag) {
        let mut flags = self.flags.write().expect("feature flags lock");
        if let Some(existing) = flags.iter_mut().find(|f| f.id == flag.id) {
            *existing = flag;
        } else {
            flags.push(flag);
        }
    }

    /// Registers multiple feature flags at once.
    pub fn register_all(&self, flags: Vec<FeatureFlag>) {
        for flag in flags {
            self.register(flag);
        }
    }

    /// Returns `true` if the given feature flag is enabled.
    ///
    /// Checks overrides first, then the flag's current value,
    /// then its default. Returns `false` if the flag is not found.
    pub fn is_enabled(&self, id: &str) -> bool {
        // Check overrides first
        {
            let overrides = self.overrides.read().expect("feature flags lock");
            if let Some(val) = overrides.get(id) {
                return val.unwrap_or(false);
            }
        }

        let flags = self.flags.read().expect("feature flags lock");
        flags
            .iter()
            .find(|f| f.id == id)
            .map(|f| f.enabled)
            .unwrap_or(false)
    }

    /// Sets a feature flag's enabled state.
    ///
    /// Returns `false` if the flag does not exist or is not runtime-toggleable.
    pub fn set_enabled(&self, id: &str, enabled: bool) -> bool {
        let mut flags = self.flags.write().expect("feature flags lock");
        if let Some(flag) = flags.iter_mut().find(|f| f.id == id) {
            if !flag.runtime_toggleable {
                return false;
            }
            flag.enabled = enabled;
            drop(flags);

            // Notify callbacks
            let callbacks = self.callbacks.read().expect("feature flags lock");
            for cb in callbacks.iter() {
                cb(id, enabled);
            }

            true
        } else {
            false
        }
    }

    /// Sets a runtime override for a feature flag.
    ///
    /// Overrides persist for the session and take precedence over
    /// the flag's stored value. Pass `None` to clear the override.
    pub fn set_override(&self, id: &str, value: Option<bool>) {
        let mut overrides = self.overrides.write().expect("feature flags lock");
        overrides.insert(id.to_string(), value);
    }

    /// Clears all runtime overrides.
    pub fn clear_overrides(&self) {
        let mut overrides = self.overrides.write().expect("feature flags lock");
        overrides.clear();
    }

    /// Returns a snapshot of all registered feature flags.
    pub fn all_flags(&self) -> Vec<FeatureFlag> {
        let flags = self.flags.read().expect("feature flags lock");
        flags.clone()
    }

    /// Returns all flags in a given category.
    pub fn flags_by_category(&self, category: &str) -> Vec<FeatureFlag> {
        let flags = self.flags.read().expect("feature flags lock");
        flags
            .iter()
            .filter(|f| f.category.as_deref() == Some(category))
            .cloned()
            .collect()
    }

    /// Registers a callback that is invoked when a flag changes.
    pub fn on_change<F>(&self, callback: F)
    where
        F: Fn(&str, bool) + Send + Sync + 'static,
    {
        let mut callbacks = self.callbacks.write().expect("feature flags lock");
        callbacks.push(Box::new(callback));
    }

    /// Resets all flags to their default values.
    pub fn reset_to_defaults(&self) {
        let mut flags = self.flags.write().expect("feature flags lock");
        for flag in flags.iter_mut() {
            flag.enabled = flag.default;
        }
    }

    /// Returns the total number of registered flags.
    pub fn flag_count(&self) -> usize {
        let flags = self.flags.read().expect("feature flags lock");
        flags.len()
    }

    /// Returns `true` if all flags in the given list are enabled.
    ///
    /// Useful for checking feature gates.
    pub fn all_enabled(&self, ids: &[&str]) -> bool {
        ids.iter().all(|id| self.is_enabled(id))
    }

    /// Returns `true` if any flag in the given list is enabled.
    pub fn any_enabled(&self, ids: &[&str]) -> bool {
        ids.iter().any(|id| self.is_enabled(id))
    }
}

impl std::fmt::Debug for FeatureFlags {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeatureFlags")
            .field("flags", &self.flags)
            .field("overrides", &self.overrides)
            .field("callbacks", &self.callbacks.read().expect("lock").len())
            .finish()
    }
}

impl Default for FeatureFlags {
    fn default() -> Self {
        Self::new()
    }
}

/// Pre-defined feature flag constants for Nabu capabilities.
pub mod nabu_flags {
    use super::FeatureFlag;

    /// Enables experimental OCR features.
    pub fn experimental_ocr() -> FeatureFlag {
        FeatureFlag::new(
            "nabu:experimental_ocr",
            "Experimental OCR",
            "Enable experimental OCR capabilities",
        )
        .with_default(false)
        .experimental()
        .with_category("experimental")
    }

    /// Enables experimental LLM features.
    pub fn experimental_llm() -> FeatureFlag {
        FeatureFlag::new(
            "nabu:experimental_llm",
            "Experimental LLM",
            "Enable experimental LLM capabilities",
        )
        .with_default(false)
        .experimental()
        .with_category("experimental")
    }

    /// Enables verbose debug logging.
    pub fn verbose_logging() -> FeatureFlag {
        FeatureFlag::new(
            "nabu:verbose_logging",
            "Verbose Logging",
            "Enable verbose debug logging for development",
        )
        .with_default(false)
        .with_runtime_toggle(true)
        .with_category("development")
    }

    /// Enables developer mode tools and diagnostics.
    pub fn developer_mode() -> FeatureFlag {
        FeatureFlag::new(
            "nabu:developer_mode",
            "Developer Mode",
            "Enable developer mode with diagnostics and profiling tools",
        )
        .with_default(false)
        .with_runtime_toggle(false)
        .with_category("development")
    }

    /// Enables performance instrumentation.
    pub fn performance_instrumentation() -> FeatureFlag {
        FeatureFlag::new(
            "nabu:performance_instrumentation",
            "Performance Instrumentation",
            "Enable detailed performance timing and instrumentation",
        )
        .with_default(false)
        .with_runtime_toggle(true)
        .with_category("development")
    }

    /// Creates all default Nabu feature flags.
    pub fn all_default() -> Vec<FeatureFlag> {
        vec![
            experimental_ocr(),
            experimental_llm(),
            verbose_logging(),
            developer_mode(),
            performance_instrumentation(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_check() {
        let flags = FeatureFlags::new();
        flags.register(FeatureFlag::new("test-flag", "Test", "A test flag"));
        assert!(flags.is_enabled("test-flag"));
    }

    #[test]
    fn flag_default_disabled() {
        let flags = FeatureFlags::new();
        flags.register(
            FeatureFlag::new("disabled-flag", "Disabled", "Disabled by default")
                .with_default(false),
        );
        assert!(!flags.is_enabled("disabled-flag"));
    }

    #[test]
    fn toggle_flag() {
        let flags = FeatureFlags::new();
        flags.register(FeatureFlag::new("toggle", "Toggle", "Toggle test"));
        assert!(flags.is_enabled("toggle"));

        assert!(flags.set_enabled("toggle", false));
        assert!(!flags.is_enabled("toggle"));

        assert!(flags.set_enabled("toggle", true));
        assert!(flags.is_enabled("toggle"));
    }

    #[test]
    fn unknown_flag_returns_false() {
        let flags = FeatureFlags::new();
        assert!(!flags.is_enabled("does-not-exist"));
    }

    #[test]
    fn non_runtime_toggleable() {
        let flags = FeatureFlags::new();
        flags.register(
            FeatureFlag::new("restart-required", "Restart Required", "Needs restart")
                .with_runtime_toggle(false)
                .with_default(true),
        );

        // Should fail to toggle at runtime
        assert!(!flags.set_enabled("restart-required", false));
        // But should still be enabled
        assert!(flags.is_enabled("restart-required"));
    }

    #[test]
    fn override_takes_precedence() {
        let flags = FeatureFlags::new();
        flags.register(
            FeatureFlag::new("feature-x", "Feature X", "A feature")
                .with_default(false),
        );
        assert!(!flags.is_enabled("feature-x"));

        flags.set_override("feature-x", Some(true));
        assert!(flags.is_enabled("feature-x"));

        flags.clear_overrides();
        assert!(!flags.is_enabled("feature-x"));
    }

    #[test]
    fn category_filtering() {
        let flags = FeatureFlags::new();
        flags.register_all(vec![
            FeatureFlag::new("a", "A", "Flag A").with_category("cat1"),
            FeatureFlag::new("b", "B", "Flag B").with_category("cat1"),
            FeatureFlag::new("c", "C", "Flag C").with_category("cat2"),
        ]);

        assert_eq!(flags.flags_by_category("cat1").len(), 2);
        assert_eq!(flags.flags_by_category("cat2").len(), 1);
        assert_eq!(flags.flags_by_category("cat3").len(), 0);
    }

    #[test]
    fn all_and_any_checks() {
        let flags = FeatureFlags::new();
        flags.register_all(vec![
            FeatureFlag::new("a", "A", "A").with_default(true),
            FeatureFlag::new("b", "B", "B").with_default(true),
            FeatureFlag::new("c", "C", "C").with_default(false),
        ]);

        assert!(flags.all_enabled(&["a", "b"]));
        assert!(!flags.all_enabled(&["a", "c"]));
        assert!(flags.any_enabled(&["a", "c"]));
        assert!(!flags.any_enabled(&["d"]));
    }

    #[test]
    fn reset_to_defaults() {
        let flags = FeatureFlags::new();
        flags.register(FeatureFlag::new("flag", "Flag", "A flag").with_default(false));
        assert!(!flags.is_enabled("flag"));

        flags.set_enabled("flag", true);
        assert!(flags.is_enabled("flag"));

        flags.reset_to_defaults();
        assert!(!flags.is_enabled("flag"));
    }

    #[test]
    fn flag_count() {
        let flags = FeatureFlags::new();
        assert_eq!(flags.flag_count(), 0);
        flags.register(FeatureFlag::new("f1", "F1", "First"));
        flags.register(FeatureFlag::new("f2", "F2", "Second"));
        assert_eq!(flags.flag_count(), 2);
    }

    #[test]
    fn default_nabu_flags() {
        let flags = FeatureFlags::new();
        let nabu = nabu_flags::all_default();
        flags.register_all(nabu);

        assert_eq!(flags.flag_count(), 5);
        assert!(!flags.is_enabled("nabu:experimental_ocr"));
        assert!(!flags.is_enabled("nabu:verbose_logging"));
        assert!(!flags.is_enabled("nabu:developer_mode"));
    }
}
