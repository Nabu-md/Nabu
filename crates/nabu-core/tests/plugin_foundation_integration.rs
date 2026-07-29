//! Integration tests for Nabu's Plugin Foundation (Prompt 45).
//!
//! These tests verify the complete plugin architecture:
//! - PluginManifest validation and compatibility
//! - Version negotiation
//! - Capability Registry
//! - Dependency graph resolution
//! - Circular dependency detection
//! - Lifecycle management
//! - Feature flags
//! - Permission model
//! - PluginManager registration and installation

use nabu_core::plugin::capability::{builtin_capabilities, Capability, CapabilityRegistry};
use nabu_core::plugin::dependency::{validate_dependencies, DependencyGraph};
use nabu_core::plugin::features::{FeatureRegistry, FeatureStage};
use nabu_core::plugin::lifecycle::{PluginLifecycle, PluginLifecycleEvent, PluginStage};
use nabu_core::plugin::manager::{PluginManager, RegistrationIssue};
use nabu_core::plugin::manifest::{
    CompatibilityCheck, PluginDependency, PluginEntryType, PluginManifest,
};
use nabu_core::plugin::permissions::{PermissionEvaluator, PermissionSet, RiskLevel};
use nabu_core::plugin::version::{Version, VersionRequirement};

// ===========================================================================
// Helper: create a test manifest
// ===========================================================================

fn make_manifest(id: &str) -> PluginManifest {
    PluginManifest {
        id: id.to_string(),
        name: format!("Test Plugin {}", id),
        version: Version::new(1, 0, 0),
        author: "Integration Test".into(),
        description: "A plugin for integration testing".into(),
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

// ===========================================================================
// Version Tests
// ===========================================================================

#[test]
fn version_parse_and_format() {
    let v = Version::parse("3.2.1").unwrap();
    assert_eq!(v.major, 3);
    assert_eq!(v.minor, 2);
    assert_eq!(v.patch, 1);
    assert_eq!(v.to_string(), "3.2.1");
}

#[test]
fn version_pre_release_ordering() {
    let v1 = Version::with_pre(1, 0, 0, "alpha");
    let v2 = Version::new(1, 0, 0);
    let v3 = Version::with_pre(1, 0, 0, "beta");
    assert!(v1 < v3);
    assert!(v3 < v2);
}

#[test]
fn version_compatibility() {
    let app = Version::new(1, 5, 0);
    let req = VersionRequirement::Compatible(Version::new(1, 3, 0));
    assert!(req.is_satisfied_by(&app));
}

#[test]
fn version_incompatible_different_major() {
    let app = Version::new(2, 0, 0);
    let req = VersionRequirement::Compatible(Version::new(1, 0, 0));
    assert!(!req.is_satisfied_by(&app));
}

#[test]
fn version_at_least() {
    let v = Version::new(2, 0, 0);
    let req = VersionRequirement::at_least(1, 5, 0);
    assert!(req.is_satisfied_by(&v));
}

#[test]
fn version_range_inclusive() {
    let v = Version::new(1, 5, 0);
    let req = VersionRequirement::Range(Version::new(1, 0, 0), Version::new(2, 0, 0));
    assert!(req.is_satisfied_by(&v));
}

#[test]
fn version_range_exclusive_upper_bound() {
    let v = Version::new(2, 1, 0);
    let req = VersionRequirement::Range(Version::new(1, 0, 0), Version::new(2, 0, 0));
    assert!(!req.is_satisfied_by(&v));
}

// ===========================================================================
// Manifest Tests
// ===========================================================================

#[test]
fn valid_manifest_passes_validation() {
    let m = make_manifest("com.example.valid");
    let errors = m.validate();
    assert!(errors.is_empty());
}

#[test]
fn empty_id_fails_validation() {
    let m = PluginManifest { id: String::new(), ..make_manifest("test") };
    let errors = m.validate();
    assert!(!errors.is_empty());
}

#[test]
fn compatibility_requires_min_version() {
    let m = PluginManifest {
        min_nabu_version: Version::new(2, 0, 0),
        ..make_manifest("test")
    };
    let result = m.check_nabu_compatibility(&Version::new(1, 0, 0));
    assert!(matches!(result, CompatibilityCheck::Incompatible { .. }));
}

#[test]
fn untested_warning_when_beyond_max() {
    let m = PluginManifest {
        max_tested_version: Some(Version::new(1, 0, 0)),
        ..make_manifest("test")
    };
    let result = m.check_nabu_compatibility(&Version::new(2, 0, 0));
    assert!(matches!(result, CompatibilityCheck::Untested { .. }));
}

// ===========================================================================
// Lifecycle Tests
// ===========================================================================

#[test]
fn lifecycle_full_path() {
    let mut lc = PluginLifecycle::new();
    assert_eq!(lc.stage(), PluginStage::Discovered);

    lc.transition_to(PluginStage::Validated,
        PluginLifecycleEvent::Validated { plugin_id: "p".into() }).unwrap();
    lc.transition_to(PluginStage::Installed,
        PluginLifecycleEvent::Installed { plugin_id: "p".into() }).unwrap();
    lc.transition_to(PluginStage::Enabled,
        PluginLifecycleEvent::Enabled { plugin_id: "p".into() }).unwrap();
    lc.transition_to(PluginStage::Disabled,
        PluginLifecycleEvent::Disabled { plugin_id: "p".into() }).unwrap();
    lc.transition_to(PluginStage::Unloaded,
        PluginLifecycleEvent::Unloaded { plugin_id: "p".into() }).unwrap();

    assert!(lc.is_unloaded());
}

#[test]
fn lifecycle_prevents_backwards_transition() {
    let mut lc = PluginLifecycle::at(PluginStage::Enabled);
    let result = lc.transition_to(PluginStage::Installed,
        PluginLifecycleEvent::Disabled { plugin_id: "p".into() });
    assert!(result.is_err());
}

#[test]
fn lifecycle_tracks_history() {
    let mut lc = PluginLifecycle::new();
    lc.transition_to(PluginStage::Validated,
        PluginLifecycleEvent::Validated { plugin_id: "p".into() }).unwrap();
    lc.transition_to(PluginStage::Installed,
        PluginLifecycleEvent::Installed { plugin_id: "p".into() }).unwrap();
    assert_eq!(lc.history().len(), 2);
}

// ===========================================================================
// Capability Registry Tests
// ===========================================================================

#[test]
fn capability_register_and_resolve() {
    let mut cr = CapabilityRegistry::new();
    cr.register(Capability::new("test", "feature", "A test feature"), "test-provider");
    assert!(cr.has("test:feature"));
    assert_eq!(cr.provider("test:feature"), Some("test-provider"));
}

#[test]
fn capability_enable_disable() {
    let mut cr = CapabilityRegistry::new();
    cr.register(Capability::new("test", "toggle", "Toggle me"), "p");
    assert!(!cr.is_enabled("test:toggle"));
    cr.enable("test:toggle");
    assert!(cr.is_enabled("test:toggle"));
    cr.disable("test:toggle");
    assert!(!cr.is_enabled("test:toggle"));
}

#[test]
fn builtin_capabilities_are_registered() {
    let caps = builtin_capabilities();
    let names: Vec<&str> = caps.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"event_bus"));
    assert!(names.contains(&"storage"));
    assert!(names.contains(&"capture"));
    assert!(names.contains(&"processor"));
    assert!(names.contains(&"graph"));
    assert!(names.contains(&"ai"));
    assert!(names.contains(&"plugin"));
}

#[test]
fn capability_by_namespace_filtering() {
    let mut cr = CapabilityRegistry::new();
    cr.register_builtin();
    let nabu = cr.by_namespace("nabu");
    assert!(nabu.len() >= 10);
}

#[test]
fn capability_provider_lookup() {
    let mut cr = CapabilityRegistry::new();
    cr.register(Capability::new("custom", "ocr", "Custom OCR"), "my-plugin");
    let caps = cr.provider_capabilities("my-plugin");
    assert_eq!(caps, vec!["custom:ocr"]);
}

// ===========================================================================
// Dependency Graph Tests
// ===========================================================================

#[test]
fn empty_graph_valid() {
    let report = validate_dependencies(&[]);
    assert!(report.is_valid());
    assert!(!report.has_critical_issues());
}

#[test]
fn linear_dependency_resolution() {
    let a = PluginManifest { dependencies: vec![PluginDependency {
        plugin_id: "b".into(),
        version_requirement: VersionRequirement::Compatible(Version::new(1, 0, 0)),
        optional: false,
    }], ..make_manifest("a") };
    let b = make_manifest("b");
    let report = validate_dependencies(&[a, b]);
    assert!(report.is_valid());
    assert!(report.topological.is_some());
}

#[test]
fn circular_dependency_detected() {
    let a = PluginManifest {
        dependencies: vec![PluginDependency {
            plugin_id: "b".into(),
            version_requirement: VersionRequirement::Compatible(Version::new(1, 0, 0)),
            optional: false,
        }],
        ..make_manifest("a")
    };
    let b = PluginManifest {
        dependencies: vec![PluginDependency {
            plugin_id: "a".into(),
            version_requirement: VersionRequirement::Compatible(Version::new(1, 0, 0)),
            optional: false,
        }],
        ..make_manifest("b")
    };
    let report = validate_dependencies(&[a, b]);
    assert!(report.has_critical_issues());
    assert!(!report.cycles.is_empty());
}

#[test]
fn missing_dependency_detected() {
    let a = PluginManifest {
        dependencies: vec![PluginDependency {
            plugin_id: "missing".into(),
            version_requirement: VersionRequirement::Compatible(Version::new(1, 0, 0)),
            optional: false,
        }],
        ..make_manifest("a")
    };
    let report = validate_dependencies(&[a]);
    assert!(report.has_critical_issues());
    assert_eq!(report.missing.len(), 1);
}

#[test]
fn self_dependency_detected() {
    let a = PluginManifest {
        dependencies: vec![PluginDependency {
            plugin_id: "a".into(),
            version_requirement: VersionRequirement::Compatible(Version::new(1, 0, 0)),
            optional: false,
        }],
        ..make_manifest("a")
    };
    let report = validate_dependencies(&[a]);
    assert!(report.has_critical_issues());
}

#[test]
fn topological_sort() {
    let a = PluginManifest {
        dependencies: vec![
            PluginDependency { plugin_id: "b".into(), version_requirement: VersionRequirement::Compatible(Version::new(1, 0, 0)), optional: false },
            PluginDependency { plugin_id: "c".into(), version_requirement: VersionRequirement::Compatible(Version::new(1, 0, 0)), optional: false },
        ],
        ..make_manifest("a")
    };
    let b = PluginManifest {
        dependencies: vec![PluginDependency {
            plugin_id: "c".into(),
            version_requirement: VersionRequirement::Compatible(Version::new(1, 0, 0)),
            optional: false,
        }],
        ..make_manifest("b")
    };
    let c = make_manifest("c");
    let report = validate_dependencies(&[a, b, c]);
    let order = report.topological.unwrap();
    // c must come before b, which must come before a
    let pos_c = order.iter().position(|x| x == "c").unwrap();
    let pos_b = order.iter().position(|x| x == "b").unwrap();
    let pos_a = order.iter().position(|x| x == "a").unwrap();
    assert!(pos_c < pos_b);
    assert!(pos_b < pos_a);
}

// ===========================================================================
// Feature Flag Tests
// ===========================================================================

#[test]
fn feature_register_and_check() {
    let mut fr = FeatureRegistry::new();
    fr.register("test.feature", "A test", FeatureStage::Stable, true);
    assert!(fr.is_enabled("test.feature"));
}

#[test]
fn feature_enable_disable_override() {
    let mut fr = FeatureRegistry::new();
    fr.register("test.feature", "Test", FeatureStage::Beta, false);
    fr.enable("test.feature");
    assert!(fr.is_enabled("test.feature"));
    assert_eq!(fr.overridden().len(), 1);
    fr.reset("test.feature");
    assert!(!fr.is_enabled("test.feature"));
}

#[test]
fn feature_stages_filter() {
    let mut fr = FeatureRegistry::new();
    fr.register("stable.feature", "Stable", FeatureStage::Stable, true);
    fr.register("experimental.feature", "Experimental", FeatureStage::Experimental, false);
    assert_eq!(fr.by_stage(FeatureStage::Stable).len(), 1);
    assert_eq!(fr.by_stage(FeatureStage::Experimental).len(), 1);
}

#[test]
fn standard_feature_flags() {
    let mut fr = FeatureRegistry::new();
    fr.register_standard_flags();
    assert_eq!(fr.count(), 4);
    assert!(fr.list().iter().any(|f| f.name == "plugin.wasm"));
    assert!(fr.list().iter().any(|f| f.name == "plugin.dev_mode"));
}

// ===========================================================================
// Permission Model Tests
// ===========================================================================

#[test]
fn permission_grant_and_check() {
    let mut ps = PermissionSet::new();
    ps.grant("vault.access");
    assert!(ps.is_granted("vault.access"));
}

#[test]
fn permission_deny_overrides_grant() {
    let mut ps = PermissionSet::new();
    ps.grant("network.http");
    ps.deny("network.http");
    assert!(!ps.is_granted("network.http"));
}

#[test]
fn standard_permissions_defined() {
    let perms = nabu_core::plugin::permissions::standard_permissions();
    let names: Vec<&str> = perms.iter().map(|p| p.name.as_str()).collect();
    assert!(names.contains(&"vault.read"));
    assert!(names.contains(&"vault.access"));
    assert!(names.contains(&"network.http"));
    assert!(names.contains(&"capture.access"));
    assert!(names.contains(&"ai.providers"));
    assert!(names.contains(&"filesystem.write"));
    assert!(names.contains(&"system.process"));
}

#[test]
fn permission_risk_levels() {
    assert!(RiskLevel::None < RiskLevel::Low);
    assert!(RiskLevel::Low < RiskLevel::Medium);
    assert!(RiskLevel::Medium < RiskLevel::High);
    assert!(RiskLevel::High < RiskLevel::Critical);
}

#[test]
fn permission_evaluator_validates_known() {
    let evaluator = PermissionEvaluator::new();
    let results = evaluator.validate_requested(&[
        "vault.read".into(),
        "unknown.perm".into(),
    ]);
    assert!(results[0].valid);
    assert!(!results[1].valid);
}

#[test]
fn permission_set_merge() {
    let mut ps1 = PermissionSet::new();
    ps1.grant("vault.read");
    let mut ps2 = PermissionSet::new();
    ps2.grant("network.http");
    ps1.merge(&ps2);
    assert!(ps1.is_granted("vault.read"));
    assert!(ps1.is_granted("network.http"));
    assert_eq!(ps1.granted_count(), 2);
}

// ===========================================================================
// PluginManager Integration Tests
// ===========================================================================

#[test]
fn plugin_manager_registers_valid_manifest() {
    let mut pm = PluginManager::new(Version::new(1, 0, 0));
    let issues = pm.register_manifest(make_manifest("com.example.test"));
    assert!(issues.is_empty());
    assert_eq!(pm.plugin_count(), 1);
    assert!(pm.manifest("com.example.test").is_some());
}

#[test]
fn plugin_manager_rejects_duplicate() {
    let mut pm = PluginManager::new(Version::new(1, 0, 0));
    pm.register_manifest(make_manifest("com.example.test"));
    let issues = pm.register_manifest(make_manifest("com.example.test"));
    assert!(!issues.is_empty());
    assert!(matches!(issues[0], RegistrationIssue::DuplicatePluginId { .. }));
}

#[test]
fn plugin_manager_rejects_incompatible_version() {
    let mut pm = PluginManager::new(Version::new(0, 0, 5));
    let issues = pm.register_manifest(PluginManifest {
        min_nabu_version: Version::new(1, 0, 0),
        ..make_manifest("com.example.test")
    });
    assert!(!issues.is_empty());
    assert!(matches!(issues[0], RegistrationIssue::IncompatibleVersion { .. }));
}

#[test]
fn plugin_manager_install_and_enable_lifecycle() {
    let mut pm = PluginManager::new(Version::new(1, 0, 0));
    pm.register_manifest(make_manifest("com.example.plugin"));
    let report = pm.install_all();
    assert!(report.success);
    assert_eq!(report.installed.len(), 1);
    assert_eq!(pm.stage("com.example.plugin"), Some(PluginStage::Installed));

    pm.enable("com.example.plugin").unwrap();
    assert_eq!(pm.stage("com.example.plugin"), Some(PluginStage::Enabled));
}

#[test]
fn plugin_manager_disable_and_check() {
    let mut pm = PluginManager::new(Version::new(1, 0, 0));
    pm.register_manifest(make_manifest("com.example.plugin"));
    pm.install_all();
    pm.enable("com.example.plugin").unwrap();
    pm.disable("com.example.plugin").unwrap();
    assert_eq!(pm.stage("com.example.plugin"), Some(PluginStage::Disabled));
}

#[test]
fn plugin_manager_enable_uninstalled_fails() {
    let mut pm = PluginManager::new(Version::new(1, 0, 0));
    assert!(pm.enable("nonexistent").is_err());
}

#[test]
fn plugin_manager_disable_not_enabled_fails() {
    let mut pm = PluginManager::new(Version::new(1, 0, 0));
    pm.register_manifest(make_manifest("com.example.plugin"));
    pm.install_all();
    assert!(pm.disable("com.example.plugin").is_err());
}

#[test]
fn plugin_manager_list_and_query() {
    let mut pm = PluginManager::new(Version::new(1, 0, 0));
    pm.register_manifest(make_manifest("plugin.a"));
    pm.register_manifest(make_manifest("plugin.b"));
    let list = pm.list_plugins();
    assert_eq!(list.len(), 2);
    assert!(list.contains(&"plugin.a".to_string()));
    assert!(list.contains(&"plugin.b".to_string()));
}

#[test]
fn plugin_manager_report_contains_state() {
    let mut pm = PluginManager::new(Version::new(1, 0, 0));
    pm.register_manifest(make_manifest("com.example.report"));
    let report = pm.report();
    assert_eq!(report.plugin_count, 1);
    assert!(report.capability_count >= 10);
    assert!(report.plugins.contains_key("com.example.report"));
    assert_eq!(report.nabu_version, Version::new(1, 0, 0));
}

#[test]
fn plugin_manager_builtin_capabilities_available() {
    let pm = PluginManager::new(Version::new(1, 0, 0));
    let cr = pm.capability_registry();
    assert!(cr.has("nabu:event_bus"));
    assert!(cr.has("nabu:storage"));
    assert!(cr.has("nabu:capture"));
    assert!(cr.has("nabu:ai"));
}

#[test]
fn plugin_manager_features_available() {
    let pm = PluginManager::new(Version::new(1, 0, 0));
    let fr = pm.feature_registry();
    assert!(!fr.is_enabled("plugin.wasm")); // experimental, disabled by default
}

#[test]
fn plugin_manager_dependency_analysis() {
    let mut pm = PluginManager::new(Version::new(1, 0, 0));
    pm.register_manifest(make_manifest("plugin.a"));
    pm.register_manifest(make_manifest("plugin.b"));
    let dep_report = pm.analyze_dependencies();
    assert!(dep_report.is_valid());
}

#[test]
fn plugin_manager_plugins_at_stage_filtering() {
    let mut pm = PluginManager::new(Version::new(1, 0, 0));
    pm.register_manifest(make_manifest("plugin.a"));
    pm.register_manifest(make_manifest("plugin.b"));
    pm.install_all();
    pm.enable("plugin.a").unwrap();

    let enabled = pm.plugins_at_stage(PluginStage::Enabled);
    let installed = pm.plugins_at_stage(PluginStage::Installed);

    assert_eq!(enabled, vec!["plugin.a"]);
    assert_eq!(installed, vec!["plugin.b"]);
}

// ===========================================================================
// Edge Cases
// ===========================================================================

#[test]
fn empty_manifest_id_rejected() {
    let mut pm = PluginManager::new(Version::new(1, 0, 0));
    let m = PluginManifest { id: String::new(), ..make_manifest("test") };
    let issues = pm.register_manifest(m);
    assert!(!issues.is_empty());
}

#[test]
fn multiple_plugins_install_order() {
    let mut pm = PluginManager::new(Version::new(1, 0, 0));
    pm.register_manifest(make_manifest("plugin.a"));
    pm.register_manifest(make_manifest("plugin.b"));
    pm.register_manifest(make_manifest("plugin.c"));
    let report = pm.install_all();
    assert!(report.success);
    assert_eq!(report.installed.len(), 3);
}
