//! Plugin Manager (Foundation) — the central coordinator for plugin lifecycle.
//!
//! This is a **foundation implementation only**.
//!
//! Responsibilities:
//! - Manifest discovery (future: from disk, from registry)
//! - Manifest validation
//! - Dependency analysis
//! - Compatibility checking
//! - Capability registration
//!
//! No plugin code is loaded or executed.
//! Plugin loading will be implemented in a future phase.

use std::collections::HashMap;

use crate::plugin::capability::CapabilityRegistry;
use crate::plugin::dependency::{validate_dependencies, DependencyReport};
use crate::plugin::features::FeatureRegistry;
use crate::plugin::lifecycle::{PluginLifecycle, PluginLifecycleEvent, PluginStage};
use crate::plugin::manifest::{CompatibilityCheck, PluginManifest};
use crate::plugin::permissions::PermissionEvaluator;
use crate::plugin::version::Version;

/// The central PluginManager responsible for the plugin lifecycle.
///
/// This is a **foundation** implementation.
/// Runtime plugin loading and execution will be added in a future phase.
///
/// Current capabilities:
/// - Register and validate plugin manifests
/// - Track plugin lifecycle stage
/// - Resolve dependency graphs
/// - Check Nabu version compatibility
/// - Register capabilities
/// - Track feature flags and permissions
#[derive(Debug)]
pub struct PluginManager {
    /// Registered plugin manifests (id → manifest).
    manifests: HashMap<String, PluginManifest>,
    /// Lifecycle states for each plugin.
    lifecycles: HashMap<String, PluginLifecycle>,
    /// The capability registry.
    capability_registry: CapabilityRegistry,
    /// The feature flag registry.
    feature_registry: FeatureRegistry,
    /// The permission evaluator.
    permission_evaluator: PermissionEvaluator,
    /// The current Nabu version for compatibility checks.
    nabu_version: Version,
}

impl PluginManager {
    /// Create a new PluginManager with the given Nabu version.
    pub fn new(nabu_version: Version) -> Self {
        let mut capability_registry = CapabilityRegistry::new();
        capability_registry.register_builtin();

        let mut feature_registry = FeatureRegistry::new();
        feature_registry.register_standard_flags();

        Self {
            manifests: HashMap::new(),
            lifecycles: HashMap::new(),
            capability_registry,
            feature_registry,
            permission_evaluator: PermissionEvaluator::new(),
            nabu_version,
        }
    }

    /// Create a PluginManager with custom registries (useful for testing).
    pub fn with_registries(
        nabu_version: Version,
        capability_registry: CapabilityRegistry,
        feature_registry: FeatureRegistry,
    ) -> Self {
        Self {
            manifests: HashMap::new(),
            lifecycles: HashMap::new(),
            capability_registry,
            feature_registry,
            permission_evaluator: PermissionEvaluator::new(),
            nabu_version,
        }
    }

    // -----------------------------------------------------------------------
    // Registration & Discovery
    // -----------------------------------------------------------------------

    /// Register a plugin manifest for discovery and validation.
    ///
    /// Returns a list of validation errors (empty = valid).
    pub fn register_manifest(&mut self, manifest: PluginManifest) -> Vec<RegistrationIssue> {
        let mut issues = Vec::new();

        // 1. Validate manifest structure
        let validation_errors = manifest.validate();
        if !validation_errors.is_empty() {
            issues.push(RegistrationIssue::ValidationFailed {
                plugin_id: manifest.id.clone(),
                errors: validation_errors.iter().map(|e| format!("{}", e)).collect(),
            });
            return issues;
        }

        // 2. Check plugin ID uniqueness
        if self.manifests.contains_key(&manifest.id) {
            issues.push(RegistrationIssue::DuplicatePluginId {
                plugin_id: manifest.id.clone(),
            });
            return issues;
        }

        // 3. Check Nabu version compatibility
        match manifest.check_nabu_compatibility(&self.nabu_version) {
            CompatibilityCheck::Incompatible { reason } => {
                issues.push(RegistrationIssue::IncompatibleVersion {
                    plugin_id: manifest.id.clone(),
                    reason,
                });
                return issues;
            }
            CompatibilityCheck::Untested { reason } => {
                issues.push(RegistrationIssue::UntestedVersion {
                    plugin_id: manifest.id.clone(),
                    reason,
                });
                // Continue — untested is a warning, not a blocker
            }
            CompatibilityCheck::Compatible => {}
        }

        // 4. Validate requested permissions
        let permission_names: Vec<String> = manifest
            .permissions
            .iter()
            .map(|p| p.name.clone())
            .collect();
        let permission_validations = self
            .permission_evaluator
            .validate_requested(&permission_names);
        let invalid_permissions: Vec<String> = permission_validations
            .iter()
            .filter(|v| !v.valid)
            .map(|v| v.permission.clone())
            .collect();
        if !invalid_permissions.is_empty() {
            issues.push(RegistrationIssue::UnknownPermissions {
                plugin_id: manifest.id.clone(),
                permissions: invalid_permissions,
            });
            return issues;
        }

        // All checks passed — register the manifest
        let plugin_id = manifest.id.clone();
        self.manifests.insert(plugin_id.clone(), manifest);
        self.lifecycles
            .insert(plugin_id.clone(), PluginLifecycle::new());

        // Transition to Validated
        if let Some(lc) = self.lifecycles.get_mut(&plugin_id) {
            let _ = lc.transition_to(
                PluginStage::Validated,
                PluginLifecycleEvent::Validated {
                    plugin_id: plugin_id.clone(),
                },
            );
        }

        issues
    }

    // -----------------------------------------------------------------------
    // Installation
    // -----------------------------------------------------------------------

    /// Attempt to install all registered plugins.
    ///
    /// Resolves dependencies, checks compatibility, and registers capabilities.
    /// This is still metadata-only — no plugin code is executed.
    pub fn install_all(&mut self) -> InstallationReport {
        let manifests: Vec<PluginManifest> = self.manifests.values().cloned().collect();
        let dep_report = validate_dependencies(&manifests);

        let mut installed = Vec::new();
        let mut failed = Vec::new();

        if dep_report.has_critical_issues() {
            return InstallationReport {
                success: false,
                installed: vec![],
                failed: manifests.iter().map(|m| m.id.clone()).collect(),
                dependency_report: dep_report,
                reason: Some("Critical dependency issues detected".into()),
            };
        }

        // Install in topological order if available
        let order = dep_report
            .topological
            .clone()
            .unwrap_or_else(|| manifests.iter().map(|m| m.id.clone()).collect());

        for plugin_id in &order {
            let succeeded = self.install_single(plugin_id);
            if succeeded {
                installed.push(plugin_id.clone());
            } else {
                failed.push(plugin_id.clone());
            }
        }

        InstallationReport {
            success: failed.is_empty(),
            installed,
            failed,
            dependency_report: dep_report,
            reason: None,
        }
    }

    fn install_single(&mut self, plugin_id: &str) -> bool {
        let manifest = match self.manifests.get(plugin_id) {
            Some(m) => m.clone(),
            None => return false,
        };

        // Verify lifecycle is at Validated
        let can_install = self
            .lifecycles
            .get(plugin_id)
            .map(|lc| lc.is_at_least(PluginStage::Validated))
            .unwrap_or(false);

        if !can_install {
            return false;
        }

        // Register plugin capabilities
        for cap in &manifest.capabilities {
            self.capability_registry.register(
                crate::plugin::capability::Capability::new(
                    &manifest.id.replace('.', "_"),
                    &cap.id,
                    &cap.description,
                ),
                &manifest.id,
            );
        }

        // Update lifecycle
        if let Some(lc) = self.lifecycles.get_mut(plugin_id) {
            let _ = lc.transition_to(
                PluginStage::Installed,
                PluginLifecycleEvent::Installed {
                    plugin_id: plugin_id.into(),
                },
            );
        }

        true
    }

    // -----------------------------------------------------------------------
    // Enable / Disable
    // -----------------------------------------------------------------------

    /// Enable an installed plugin.
    pub fn enable(&mut self, plugin_id: &str) -> Result<(), ManagerError> {
        let can_enable = self
            .lifecycles
            .get(plugin_id)
            .map(|lc| lc.is_at_least(PluginStage::Installed))
            .unwrap_or(false);

        if !can_enable {
            return Err(ManagerError::NotInstalled(plugin_id.to_string()));
        }

        // Enable this plugin's capabilities
        if let Some(manifest) = self.manifests.get(plugin_id) {
            for cap in &manifest.capabilities {
                let cap_id = format!("{}:{}", manifest.id.replace('.', "_"), cap.id);
                self.capability_registry.enable(&cap_id);
            }
        }

        if let Some(lc) = self.lifecycles.get_mut(plugin_id) {
            let _ = lc.transition_to(
                PluginStage::Enabled,
                PluginLifecycleEvent::Enabled {
                    plugin_id: plugin_id.into(),
                },
            );
        }

        Ok(())
    }

    /// Disable an enabled plugin.
    pub fn disable(&mut self, plugin_id: &str) -> Result<(), ManagerError> {
        let is_enabled = self
            .lifecycles
            .get(plugin_id)
            .map(|lc| lc.stage() == PluginStage::Enabled)
            .unwrap_or(false);

        if !is_enabled {
            return Err(ManagerError::NotEnabled(plugin_id.to_string()));
        }

        // Disable this plugin's capabilities
        if let Some(manifest) = self.manifests.get(plugin_id) {
            for cap in &manifest.capabilities {
                let cap_id = format!("{}:{}", manifest.id.replace('.', "_"), cap.id);
                self.capability_registry.disable(&cap_id);
            }
        }

        if let Some(lc) = self.lifecycles.get_mut(plugin_id) {
            let _ = lc.transition_to(
                PluginStage::Disabled,
                PluginLifecycleEvent::Disabled {
                    plugin_id: plugin_id.into(),
                },
            );
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Query
    // -----------------------------------------------------------------------

    /// Get a registered manifest.
    pub fn manifest(&self, plugin_id: &str) -> Option<&PluginManifest> {
        self.manifests.get(plugin_id)
    }

    /// Get a plugin's lifecycle tracker.
    pub fn lifecycle(&self, plugin_id: &str) -> Option<&PluginLifecycle> {
        self.lifecycles.get(plugin_id)
    }

    /// Get the lifecycle stage of a plugin.
    pub fn stage(&self, plugin_id: &str) -> Option<PluginStage> {
        self.lifecycles.get(plugin_id).map(|lc| lc.stage())
    }

    /// List all registered plugin IDs.
    pub fn list_plugins(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.manifests.keys().cloned().collect();
        ids.sort();
        ids
    }

    /// List plugins at a specific lifecycle stage.
    pub fn plugins_at_stage(&self, stage: PluginStage) -> Vec<String> {
        self.lifecycles
            .iter()
            .filter(|(_, lc)| lc.stage() == stage)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Number of registered plugins.
    pub fn plugin_count(&self) -> usize {
        self.manifests.len()
    }

    /// Reference to the capability registry.
    pub fn capability_registry(&self) -> &CapabilityRegistry {
        &self.capability_registry
    }

    /// Mutable reference to the capability registry.
    pub fn capability_registry_mut(&mut self) -> &mut CapabilityRegistry {
        &mut self.capability_registry
    }

    /// Reference to the feature registry.
    pub fn feature_registry(&self) -> &FeatureRegistry {
        &self.feature_registry
    }

    /// Mutable reference to the feature registry.
    pub fn feature_registry_mut(&mut self) -> &mut FeatureRegistry {
        &mut self.feature_registry
    }

    /// The current Nabu version used for compatibility checks.
    pub fn nabu_version(&self) -> &Version {
        &self.nabu_version
    }

    /// Run dependency analysis on all registered manifests.
    pub fn analyze_dependencies(&self) -> DependencyReport {
        let manifests: Vec<PluginManifest> = self.manifests.values().cloned().collect();
        validate_dependencies(&manifests)
    }

    /// Generate a report of all known plugins and their state.
    pub fn report(&self) -> ManagerReport {
        let manifests: Vec<PluginManifest> = self.manifests.values().cloned().collect();
        let dep_report = validate_dependencies(&manifests);

        let plugin_states: HashMap<String, PluginStage> = self
            .lifecycles
            .iter()
            .map(|(id, lc)| (id.clone(), lc.stage()))
            .collect();

        ManagerReport {
            plugin_count: self.manifests.len(),
            capability_count: self.capability_registry.capability_count(),
            enabled_capabilities: self.capability_registry.enabled_count(),
            feature_flag_count: self.feature_registry.count(),
            plugins: plugin_states,
            dependency_report: dep_report,
            nabu_version: self.nabu_version.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Issues that can occur during plugin registration.
#[derive(Debug, Clone, PartialEq)]
pub enum RegistrationIssue {
    ValidationFailed {
        plugin_id: String,
        errors: Vec<String>,
    },
    DuplicatePluginId {
        plugin_id: String,
    },
    IncompatibleVersion {
        plugin_id: String,
        reason: String,
    },
    UntestedVersion {
        plugin_id: String,
        reason: String,
    },
    UnknownPermissions {
        plugin_id: String,
        permissions: Vec<String>,
    },
}

/// Report from a batch installation attempt.
#[derive(Debug, Clone)]
pub struct InstallationReport {
    pub success: bool,
    pub installed: Vec<String>,
    pub failed: Vec<String>,
    pub dependency_report: DependencyReport,
    pub reason: Option<String>,
}

/// Summary report of the plugin manager state.
#[derive(Debug, Clone)]
pub struct ManagerReport {
    pub plugin_count: usize,
    pub capability_count: usize,
    pub enabled_capabilities: usize,
    pub feature_flag_count: usize,
    pub plugins: HashMap<String, PluginStage>,
    pub dependency_report: DependencyReport,
    pub nabu_version: Version,
}

/// Errors that can occur during plugin manager operations.
#[derive(Debug, Clone, PartialEq)]
pub enum ManagerError {
    NotInstalled(String),
    NotEnabled(String),
    AlreadyInstalled(String),
    UnknownPlugin(String),
}

impl std::fmt::Display for ManagerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotInstalled(id) => write!(f, "Plugin '{}' is not installed", id),
            Self::NotEnabled(id) => write!(f, "Plugin '{}' is not enabled", id),
            Self::AlreadyInstalled(id) => write!(f, "Plugin '{}' is already installed", id),
            Self::UnknownPlugin(id) => write!(f, "Unknown plugin: '{}'", id),
        }
    }
}

impl std::error::Error for ManagerError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::manifest::PluginEntryType;

    fn test_manifest(id: &str) -> PluginManifest {
        PluginManifest {
            id: id.to_string(),
            name: format!("Plugin {}", id),
            version: Version::new(1, 0, 0),
            author: "test".into(),
            description: "A test plugin".into(),
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
    fn register_valid_manifest() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let issues = pm.register_manifest(test_manifest("com.example.test"));
        assert!(issues.is_empty());
        assert_eq!(pm.plugin_count(), 1);
    }

    #[test]
    fn duplicate_plugin_rejected() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        pm.register_manifest(test_manifest("com.example.test"));
        let issues = pm.register_manifest(test_manifest("com.example.test"));
        assert!(!issues.is_empty());
        assert!(matches!(
            issues[0],
            RegistrationIssue::DuplicatePluginId { .. }
        ));
    }

    #[test]
    fn incompatible_version_rejected() {
        let mut pm = PluginManager::new(Version::new(0, 0, 5));
        let manifest = PluginManifest {
            min_nabu_version: Version::new(1, 0, 0),
            ..test_manifest("com.example.test")
        };
        let issues = pm.register_manifest(manifest);
        assert!(!issues.is_empty());
        assert!(matches!(
            issues[0],
            RegistrationIssue::IncompatibleVersion { .. }
        ));
    }

    #[test]
    fn install_and_enable() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        pm.register_manifest(test_manifest("com.example.test"));
        let report = pm.install_all();
        assert!(report.success);
        assert_eq!(report.installed.len(), 1);
        assert_eq!(pm.stage("com.example.test"), Some(PluginStage::Installed));

        pm.enable("com.example.test").unwrap();
        assert_eq!(pm.stage("com.example.test"), Some(PluginStage::Enabled));
    }

    #[test]
    fn enable_uninstalled_plugin_fails() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let result = pm.enable("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn list_and_query() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        pm.register_manifest(test_manifest("p1"));
        pm.register_manifest(test_manifest("p2"));
        let list = pm.list_plugins();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"p1".to_string()));
    }

    #[test]
    fn report_generated() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        pm.register_manifest(test_manifest("com.example.test"));
        let report = pm.report();
        assert_eq!(report.plugin_count, 1);
        assert!(report.capability_count >= 10);
    }
}
