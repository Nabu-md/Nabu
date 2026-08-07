//! Integration tests for the CapabilityProvider trait and PluginManager
//! provider registration workflow (Phase 6.1.2).
//!
//! These tests validate the observable behavior of:
//! - Provider registration through the PluginManager
//! - Capability registration flowing through the CapabilityRegistry
//! - Registry integrity (no duplicate registrations)
//! - Duplicate provider handling
//! - Provider lifecycle hooks (initialize / shutdown)
//! - Integration with the existing manifest-based registration path


mod plugin_integration {
    use super::*;

    use std::sync::Arc;

    use nabu_core::plugin::capability::{builtin_capabilities, Capability, CapabilityRegistry};
    use nabu_core::plugin::manager::PluginManager;
    use nabu_core::plugin::provider::{CapabilityProvider, ProviderError};
    use nabu_core::plugin::version::Version;

    // ===========================================================================
    // Test Provider Implementations
    // ===========================================================================

    /// A minimal test provider with a fixed set of capabilities.
    #[derive(Debug)]
    struct StaticProvider {
        id: String,
        name: String,
        version: Version,
        description: String,
        caps: Vec<Capability>,
        initialized: std::sync::atomic::AtomicBool,
        shut_down: std::sync::atomic::AtomicBool,
    }

    impl StaticProvider {
        fn new(id: &str, name: &str, caps: Vec<Capability>) -> Self {
            Self {
                id: id.to_string(),
                name: name.to_string(),
                version: Version::new(1, 0, 0),
                description: format!("Test provider: {}", name),
                caps,
                initialized: std::sync::atomic::AtomicBool::new(false),
                shut_down: std::sync::atomic::AtomicBool::new(false),
            }
        }
    }

    impl CapabilityProvider for StaticProvider {
        fn id(&self) -> &str {
            &self.id
        }
        fn name(&self) -> &str {
            &self.name
        }
        fn version(&self) -> &Version {
            &self.version
        }
        fn description(&self) -> &str {
            &self.description
        }
        fn capabilities(&self) -> Vec<Capability> {
            self.caps.clone()
        }

        fn initialize(&self) -> Result<(), ProviderError> {
            self.initialized
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        fn shutdown(&self) -> Result<(), ProviderError> {
            self.shut_down
                .store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    /// A provider that overrides `register_capabilities` with custom logic
    /// — e.g. registering capabilities with a custom namespace prefix.
    #[derive(Debug)]
    struct CustomRegistrationProvider {
        id: String,
        name: String,
        version: Version,
        caps: Vec<Capability>,
    }

    impl CapabilityProvider for CustomRegistrationProvider {
        fn id(&self) -> &str {
            &self.id
        }
        fn name(&self) -> &str {
            &self.name
        }
        fn version(&self) -> &Version {
            &self.version
        }
        fn capabilities(&self) -> Vec<Capability> {
            self.caps.clone()
        }

        fn register_capabilities(
            &self,
            registry: &mut CapabilityRegistry,
        ) -> Result<(), ProviderError> {
            for cap in self.capabilities() {
                let cap_id = cap.id();
                if registry.has(&cap_id) {
                    return Err(ProviderError::DuplicateCapability {
                        capability_id: cap_id,
                        provider_id: self.id().to_string(),
                    });
                }
                registry.register(cap, self.id());
            }
            Ok(())
        }
    }

    /// A provider whose `initialize` hook always fails.
    #[derive(Debug)]
    struct FailingInitProvider {
        id: String,
        version: Version,
    }

    impl CapabilityProvider for FailingInitProvider {
        fn id(&self) -> &str {
            &self.id
        }
        fn name(&self) -> &str {
            "Failing"
        }
        fn version(&self) -> &Version {
            &self.version
        }
        fn capabilities(&self) -> Vec<Capability> {
            vec![Capability::new("failing", "init", "Failing init provider")]
        }

        fn initialize(&self) -> Result<(), ProviderError> {
            Err(ProviderError::InitializationFailed {
                provider_id: self.id.clone(),
                reason: "simulated init failure".to_string(),
            })
        }
    }

    // ===========================================================================
    // Provider Registration Tests
    // ===========================================================================

    #[test]
    fn provider_registration_succeeds() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let provider = Arc::new(StaticProvider::new(
            "com.example.ocr",
            "OCR Provider",
            vec![Capability::new("ocr", "provider", "A custom OCR engine")],
        ));

        let result = pm.register_provider(provider);
        assert!(result.is_ok());
        assert_eq!(pm.provider_count(), 1);
        assert!(pm.list_providers().contains(&"com.example.ocr".to_string()));
    }

    #[test]
    fn provider_registration_tracks_provider() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let provider = Arc::new(StaticProvider::new(
            "com.example.exporter",
            "Exporter",
            vec![],
        ));

        pm.register_provider(provider).unwrap();

        let fetched = pm.provider("com.example.exporter");
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id(), "com.example.exporter");
        assert_eq!(fetched.unwrap().name(), "Exporter");
    }

    #[test]
    fn provider_registration_multiple_providers() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        pm.register_provider(Arc::new(StaticProvider::new(
            "com.example.ocr",
            "OCR",
            vec![Capability::new("ocr", "provider", "OCR")],
        )))
        .unwrap();
        pm.register_provider(Arc::new(StaticProvider::new(
            "com.example.ai",
            "AI",
            vec![Capability::new("ai", "provider", "AI")],
        )))
        .unwrap();

        assert_eq!(pm.provider_count(), 2);
        let ids = pm.list_providers();
        assert!(ids.contains(&"com.example.ocr".to_string()));
        assert!(ids.contains(&"com.example.ai".to_string()));
    }

    #[test]
    fn register_duplicate_provider_rejected() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let provider = Arc::new(StaticProvider::new(
            "com.example.duplicate",
            "Duplicate",
            vec![Capability::new("dup", "feature", "Feature")],
        ));

        // First registration succeeds
        assert!(pm.register_provider(provider.clone()).is_ok());
        assert_eq!(pm.provider_count(), 1);

        // Second registration with the same provider ID is rejected
        let result = pm.register_provider(provider);
        assert!(result.is_err());
        match result.unwrap_err() {
            ProviderError::DuplicateProvider { provider_id } => {
                assert_eq!(provider_id, "com.example.duplicate");
            }
            other => panic!("Expected DuplicateProvider, got {:?}", other),
        }

        // Provider count remains 1 — the duplicate was not stored
        assert_eq!(pm.provider_count(), 1);
    }

    #[test]
    fn register_duplicate_provider_different_instance_same_id() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));

        pm.register_provider(Arc::new(StaticProvider::new(
            "com.example.same",
            "First",
            vec![Capability::new("ns", "cap1", "Cap 1")],
        )))
        .unwrap();

        // A completely different provider instance with the same ID
        let result = pm.register_provider(Arc::new(StaticProvider::new(
            "com.example.same",
            "Second",
            vec![Capability::new("ns", "cap2", "Cap 2")],
        )));

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ProviderError::DuplicateProvider { .. }));
        assert_eq!(pm.provider_count(), 1);
    }

    // ===========================================================================
    // Capability Registration Tests
    // ===========================================================================

    #[test]
    fn provider_capabilities_registered_in_registry() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let provider = Arc::new(StaticProvider::new(
            "com.example.ocr",
            "OCR",
            vec![
                Capability::new("ocr", "tesseract", "Tesseract OCR engine"),
                Capability::new("ocr", "google_vision", "Google Vision OCR"),
            ],
        ));

        pm.register_provider(provider).unwrap();

        let cr = pm.capability_registry();
        assert!(cr.has("ocr:tesseract"));
        assert!(cr.has("ocr:google_vision"));

        // The provider ID is recorded as the capability's provider
        assert_eq!(cr.provider("ocr:tesseract"), Some("com.example.ocr"));
        assert_eq!(cr.provider("ocr:google_vision"), Some("com.example.ocr"));
    }

    #[test]
    fn provider_capabilities_appear_alongside_builtin() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        // Built-in capabilities are already registered by PluginManager::new
        let builtin_count = pm.capability_registry().capability_count();

        let provider = Arc::new(StaticProvider::new(
            "com.example.ext",
            "Extension",
            vec![Capability::new("ext", "feature", "An extension feature")],
        ));
        pm.register_provider(provider).unwrap();

        let cr = pm.capability_registry();
        // The new capability is registered alongside built-ins
        assert!(cr.has("nabu:event_bus")); // built-in
        assert!(cr.has("ext:feature")); // provider-provided
        assert_eq!(cr.capability_count(), builtin_count + 1);
    }

    #[test]
    fn provider_provider_capability_lookup() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let provider = Arc::new(StaticProvider::new(
            "com.example.lookup",
            "Lookup",
            vec![
                Capability::new("lookup", "search", "Search capability"),
                Capability::new("lookup", "suggest", "Suggestion capability"),
            ],
        ));

        pm.register_provider(provider).unwrap();

        let cr = pm.capability_registry();
        let provider_caps = cr.provider_capabilities("com.example.lookup");
        assert_eq!(provider_caps.len(), 2);
        assert!(provider_caps.contains(&"lookup:search".to_string()));
        assert!(provider_caps.contains(&"lookup:suggest".to_string()));
    }

    #[test]
    fn empty_provider_registers_no_capabilities() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let before_count = pm.capability_registry().capability_count();

        let provider = Arc::new(StaticProvider::new(
            "com.example.empty",
            "Empty",
            vec![],
        ));
        pm.register_provider(provider).unwrap();

        assert_eq!(pm.provider_count(), 1);
        assert_eq!(
            pm.capability_registry().capability_count(),
            before_count // unchanged
        );
    }

    // ===========================================================================
    // Registry Integration Tests
    // ===========================================================================

    #[test]
    fn provider_capabilities_are_in_registry_after_registration() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let caps = vec![
            Capability::new("test", "cap_a", "Capability A"),
            Capability::new("test", "cap_b", "Capability B"),
            Capability::new("test", "cap_c", "Capability C"),
        ];
        let provider = Arc::new(StaticProvider::new(
            "com.example.three",
            "Three",
            caps,
        ));

        pm.register_provider(provider).unwrap();

        let cr = pm.capability_registry();
        assert!(cr.has("test:cap_a"));
        assert!(cr.has("test:cap_b"));
        assert!(cr.has("test:cap_c"));
    }

    #[test]
    fn duplicate_capability_from_different_provider_rejected() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));

        // Register a provider that claims "shared:feature"
        pm.register_provider(Arc::new(StaticProvider::new(
            "com.example.first",
            "First",
            vec![Capability::new("shared", "feature", "Shared feature")],
        )))
        .unwrap();

        // Register another provider that also claims "shared:feature"
        let result = pm.register_provider(Arc::new(StaticProvider::new(
            "com.example.second",
            "Second",
            vec![Capability::new("shared", "feature", "Different provider, same cap")],
        )));

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProviderError::DuplicateCapability { capability_id, .. }
                if capability_id == "shared:feature"
        ));

        // The second provider was NOT stored
        assert_eq!(pm.provider_count(), 1);
        assert!(pm.provider("com.example.second").is_none());
    }

    #[test]
    fn capability_from_rejected_provider_does_not_pollute_registry() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));

        // First provider registers "ns:feature"
        pm.register_provider(Arc::new(StaticProvider::new(
            "com.example.first",
            "First",
            vec![Capability::new("ns", "feature", "Feature A")],
        )))
        .unwrap();

        // Second provider tries to register both a new capability and a duplicate.
        // Because register_capabilities is atomic (returns Err before partially
        // committing), the new capability should also NOT be in the registry.
        let result = pm.register_provider(Arc::new(StaticProvider::new(
            "com.example.second",
            "Second",
            vec![
                Capability::new("ns", "feature", "Duplicate"), // duplicate!
                Capability::new("ns", "new_cap", "New cap"),  // would-be new
            ],
        )));

        assert!(result.is_err());

        // The duplicate was rejected, and the would-be-new capability was NOT
        // registered (registration is all-or-nothing within the provider's
        // register_capabilities call).
        let cr = pm.capability_registry();
        assert!(cr.has("ns:feature"));
        assert_eq!(cr.provider("ns:feature"), Some("com.example.first"));
        assert!(!cr.has("ns:new_cap"));
    }

    #[test]
    fn custom_register_capabilities_override_works() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let provider = Arc::new(CustomRegistrationProvider {
            id: "com.example.custom".to_string(),
            name: "Custom".to_string(),
            version: Version::new(2, 1, 3),
            caps: vec![Capability::new("custom", "engine", "Custom engine")],
        });

        pm.register_provider(provider).unwrap();

        let cr = pm.capability_registry();
        assert!(cr.has("custom:engine"));
        assert_eq!(cr.provider("custom:engine"), Some("com.example.custom"));
    }

    #[test]
    fn provider_registration_does_not_affect_manifest_registration() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));

        // Register via the old manifest path
        use nabu_core::plugin::manifest::{PluginEntryType, PluginManifest};
        let manifest = PluginManifest {
            id: "com.example.manifest".to_string(),
            name: "Manifest Plugin".to_string(),
            version: Version::new(1, 0, 0),
            author: "test".into(),
            description: "A manifest plugin".into(),
            min_nabu_version: Version::new(0, 1, 0),
            max_tested_version: None,
            manifest_version: 1,
            capabilities: vec![nabu_core::plugin::manifest::PluginCapability {
                id: "manifest_cap".to_string(),
                description: "From manifest".to_string(),
                version: Version::new(1, 0, 0),
            }],
            dependencies: vec![],
            optional_dependencies: vec![],
            feature_flags: vec![],
            permissions: vec![],
            entry_type: PluginEntryType::Wasm,
        };
        pm.register_manifest(manifest);
        pm.install_all();

        // Also register via the new provider path
        pm.register_provider(Arc::new(StaticProvider::new(
            "com.example.provider",
            "Provider Plugin",
            vec![Capability::new("provider_ns", "provider_cap", "From provider")],
        )))
        .unwrap();

        let cr = pm.capability_registry();
        // Manifest-registered capabilities use the plugin_id.replace('.', "_") namespace
        assert!(cr.has("com_example_manifest:manifest_cap"));
        assert!(cr.has("provider_ns:provider_cap"));
        assert_eq!(cr.capability_count() >= 11, true);
    }

    // ===========================================================================
    // Lifecycle Hook Tests
    // ===========================================================================

    #[test]
    fn provider_initialize_hook_called() {
        let provider = Arc::new(StaticProvider::new(
            "com.example.lifecycle",
            "Lifecycle",
            vec![Capability::new("lifecycle", "test", "Test")],
        ));

        let initialized_before = provider
            .initialized
            .load(std::sync::atomic::Ordering::SeqCst);
        assert!(!initialized_before);

        let result = provider.initialize();
        assert!(result.is_ok());

        let initialized_after = provider
            .initialized
            .load(std::sync::atomic::Ordering::SeqCst);
        assert!(initialized_after);
    }

    #[test]
    fn provider_shutdown_hook_called() {
        let provider = Arc::new(StaticProvider::new(
            "com.example.shutdown",
            "Shutdown",
            vec![Capability::new("shutdown", "test", "Test")],
        ));

        let result = provider.shutdown();
        assert!(result.is_ok());

        let shut_down = provider
            .shut_down
            .load(std::sync::atomic::Ordering::SeqCst);
        assert!(shut_down);
    }

    #[test]
    fn provider_initialize_failure_returns_error() {
        let provider = Arc::new(FailingInitProvider {
            id: "com.example.failing".to_string(),
            version: Version::new(1, 0, 0),
        });

        let result = provider.initialize();
        assert!(result.is_err());
        match result.unwrap_err() {
            ProviderError::InitializationFailed { provider_id, reason } => {
                assert_eq!(provider_id, "com.example.failing");
                assert!(reason.contains("simulated"));
            }
            other => panic!("Expected InitializationFailed, got {:?}", other),
        }
    }

    // ===========================================================================
    // Provider Access & Listing Tests
    // ===========================================================================

    #[test]
    fn provider_lookup_returns_none_for_unknown() {
        let pm = PluginManager::new(Version::new(1, 0, 0));
        assert!(pm.provider("nonexistent").is_none());
    }

    #[test]
    fn list_providers_returns_sorted_ids() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        pm.register_provider(Arc::new(StaticProvider::new(
            "com.zeta.third",
            "Zeta",
            vec![Capability::new("z", "cap", "Z")],
        )))
        .unwrap();
        pm.register_provider(Arc::new(StaticProvider::new(
            "com.alpha.first",
            "Alpha",
            vec![Capability::new("a", "cap", "A")],
        )))
        .unwrap();
        pm.register_provider(Arc::new(StaticProvider::new(
            "com.beta.second",
            "Beta",
            vec![Capability::new("b", "cap", "B")],
        )))
        .unwrap();

        let ids = pm.list_providers();
        assert_eq!(ids, vec![
            "com.alpha.first",
            "com.beta.second",
            "com.zeta.third",
        ]);
    }

    #[test]
    fn provider_count_reflects_registrations() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        assert_eq!(pm.provider_count(), 0);

        pm.register_provider(Arc::new(StaticProvider::new(
            "p1",
            "P1",
            vec![Capability::new("p1", "cap", "P1 cap")],
        )))
        .unwrap();
        assert_eq!(pm.provider_count(), 1);

        pm.register_provider(Arc::new(StaticProvider::new(
            "p2",
            "P2",
            vec![Capability::new("p2", "cap", "P2 cap")],
        )))
        .unwrap();
        assert_eq!(pm.provider_count(), 2);
    }

    // ===========================================================================
    // ManagerReport Tests
    // ===========================================================================

    #[test]
    fn report_includes_provider_count() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        assert_eq!(pm.report().provider_count, 0);

        pm.register_provider(Arc::new(StaticProvider::new(
            "com.example.report",
            "Report",
            vec![Capability::new("r", "cap", "R cap")],
        )))
        .unwrap();

        let report = pm.report();
        assert_eq!(report.provider_count, 1);
        assert!(report.capability_count > 0);
    }

    // ===========================================================================
    // ProviderError Display Tests
    // ===========================================================================

    #[test]
    fn provider_error_display_is_human_readable() {
        let errors = vec![
            ProviderError::DuplicateProvider {
                provider_id: "p".into(),
            },
            ProviderError::DuplicateCapability {
                capability_id: "ns:cap".into(),
                provider_id: "p".into(),
            },
            ProviderError::InvalidCapability {
                capability_id: "ns:bad".into(),
                provider_id: "p".into(),
                reason: "bad".into(),
            },
            ProviderError::RegistrationFailed {
                provider_id: "p".into(),
                reason: "oops".into(),
            },
            ProviderError::InitializationFailed {
                provider_id: "p".into(),
                reason: "no config".into(),
            },
            ProviderError::ShutdownFailed {
                provider_id: "p".into(),
                reason: "timeout".into(),
            },
        ];

        for err in errors {
            let msg = format!("{}", err);
            assert!(!msg.is_empty(), "Error display must not be empty");
            // Each message should contain the provider or capability ID
            assert!(
                msg.contains("p"),
                "Error message should contain provider ID: {}",
                msg
            );
        }
    }

    // ===========================================================================
    // Combined Provider + Manifest Tests
    // ===========================================================================

    #[test]
    fn provider_capability_count_after_registration() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));
        let builtin_count = pm.capability_registry().capability_count();

        pm.register_provider(Arc::new(StaticProvider::new(
            "com.example.counted",
            "Counted",
            vec![
                Capability::new("c1", "a", "A"),
                Capability::new("c1", "b", "B"),
                Capability::new("c1", "c", "C"),
            ],
        )))
        .unwrap();

        assert_eq!(
            pm.capability_registry().capability_count(),
            builtin_count + 3
        );
    }

    #[test]
    fn provider_with_namespace_matching_builtin_rejected() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));

        // A provider trying to register a capability that matches a built-in
        // capability should be rejected.
        let result = pm.register_provider(Arc::new(StaticProvider::new(
            "com.example.conflict",
            "Conflict",
            vec![Capability::new("nabu", "event_bus", "Conflicting!")],
        )));

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProviderError::DuplicateCapability { capability_id, .. }
                if capability_id == "nabu:event_bus"
        ));

        // The provider was NOT stored
        assert_eq!(pm.provider_count(), 0);
    }

    #[test]
    fn register_provider_then_lookup_via_registry() {
        let mut pm = PluginManager::new(Version::new(1, 0, 0));

        let caps = builtin_capabilities();
        let custom = Capability::new("custom", "extension", "A custom extension");

        pm.register_provider(Arc::new(StaticProvider::new(
            "com.example.registry",
            "Registry Test",
            vec![custom],
        )))
        .unwrap();

        // Verify the capability is discoverable through the registry
        let cr = pm.capability_registry();
        assert!(cr.has("custom:extension"));
        assert_eq!(cr.provider("custom:extension"), Some("com.example.registry"));

        // Verify the provider is discoverable through the PluginManager
        let provider = pm.provider("com.example.registry");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().id(), "com.example.registry");
        assert_eq!(provider.unwrap().name(), "Registry Test");

        // The builtin capabilities are still present
        assert!(cr.has("nabu:event_bus"));
        assert!(cr.has("nabu:storage"));
        assert!(cr.has("nabu:capture"));
        assert!(cr.has("nabu:graph"));

        // Ensure no unused variable warning
        let _ = caps;
    }
}
