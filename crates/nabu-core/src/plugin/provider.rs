//! Capability Provider Trait
//!
//! This module defines the [`CapabilityProvider`] trait — the foundational
//! abstraction that allows any object (a built-in module, a native shared
//! library, a WASM plugin, a scripting plugin, a remote provider, etc.) to
//! contribute capabilities to the Nabu Capability Platform.
//!
//! ## Architecture
//!
//! ```text
//! Plugin
//!   │  (implements)
//!   ▼
//! CapabilityProvider  ──▶  PluginManager.register_provider()
//!   │                         │
//!   │  register_capabilities()│
//!   ▼                         ▼
//! CapabilityRegistry   ◄──  PluginManager coordinates via public API
//!   │
//!   ▼
//! Capability Platform
//! ```
//!
//! The `PluginManager` depends on this trait (the *abstraction*), never on
//! concrete plugin implementations. Every future plugin type — built-in,
//! native, WASM, scripting, remote, or bundled packs — will implement this
//! single contract to integrate with the platform.
//!
//! ## Ownership Model
//!
//! | Layer              | Responsibility                                      |
//! |--------------------|-----------------------------------------------------|
//! | Provider           | Exposes capabilities; registers them through the registry |
//! | PluginManager      | Coordinates registration; tracks provider identity  |
//! | CapabilityRegistry | **Owns** registered capabilities (single source of truth) |
//!
//! The provider does **not** own registered capabilities. It declares what it
//! can provide, and the registry owns the registered state. Providers interact
//! with the registry only through its public API — they never mutate internal
//! collections directly.
//!
//! ## Registration Workflow
//!
//! 1. A provider is constructed (by whatever plugin system loads it).
//! 2. The provider is passed to [`PluginManager::register_provider`] as
//!    `Arc<dyn CapabilityProvider>`.
//! 3. The PluginManager calls `provider.register_capabilities(&mut registry)`.
//! 4. The registry checks for duplicate capability IDs and stores the
//!    capability + provider mapping.
//! 5. The PluginManager stores the provider for later lifecycle operations.
//!
//! Registration is **metadata-only**: no plugin code is executed, no
//! directories are scanned, and no capability initialization occurs.
//!
//! ## Extension Guidance for Future Plugin Authors
//!
//! To create a new provider, implement `CapabilityProvider`:
//!
//! ```
//! use std::sync::Arc;
//! use nabu_core::plugin::capability::{Capability, CapabilityRegistry};
//! use nabu_core::plugin::version::Version;
//! use nabu_core::plugin::provider::{CapabilityProvider, ProviderError};
//!
//! struct MyProvider;
//!
//! impl CapabilityProvider for MyProvider {
//!     fn id(&self) -> &str { "com.example.my-plugin" }
//!     fn name(&self) -> &str { "My Plugin" }
//!     fn version(&self) -> &Version { &Version::new(1, 0, 0) }
//!     fn description(&self) -> &str { "An example provider" }
//!
//!     fn capabilities(&self) -> Vec<Capability> {
//!         vec![
//!             Capability::new("myplugin", "ocr", "Custom OCR engine"),
//!         ]
//!     }
//! }
//! ```
//!
//! The default `register_capabilities` implementation iterates
//! `capabilities()` and calls `registry.register()` for each, checking for
//! duplicates. Override `register_capabilities` only if you need custom
//! registration logic (e.g. conditional capabilities, lazy registration).
//!
//! `initialize` and `shutdown` are optional hooks with default no-op
//! implementations. Override them when your provider needs to set up or tear
//! down resources.
//!
//! ## Thread Safety
//!
//! The trait requires `Send + Sync`, making providers safe to share across
//! threads. This supports future concurrent registration phases. Providers
//! must not rely on global mutable state.
//!
//! ## API Stability
//!
//! The trait is designed to be **extensible without breaking changes**. New
//! default methods may be added in future versions — existing implementors
//! will continue to compile. Methods without defaults are part of the stable
//! ABI and will not be removed or have their signatures changed.

use std::sync::Arc;

use crate::plugin::capability::{Capability, CapabilityRegistry};
use crate::plugin::version::Version;

// ---------------------------------------------------------------------------
// ProviderError
// ---------------------------------------------------------------------------

/// Structured errors returned by capability provider operations.
///
/// All methods on [`CapabilityProvider`] that can fail return
/// `Result<_, ProviderError>`. This enum covers the categories of failure
/// called out by the platform specification:
///
/// - [`DuplicateProvider`](ProviderError::DuplicateProvider) — a provider with
///   the same ID is already registered.
/// - [`DuplicateCapability`](ProviderError::DuplicateCapability) — a capability
///   with the same identifier is already registered (by this or another
///   provider).
/// - [`InvalidCapability`](ProviderError::InvalidCapability) — a capability
///   definition is structurally invalid.
/// - [`RegistrationFailed`](ProviderError::RegistrationFailed) — registration
///   with the capability registry failed for another reason.
/// - [`InitializationFailed`](ProviderError::InitializationFailed) — the
///   provider's `initialize` hook returned an error.
/// - [`ShutdownFailed`](ProviderError::ShutdownFailed) — the provider's
///   `shutdown` hook returned an error.
///
/// This type implements [`std::error::Error`] so it can be used with the
/// standard Rust error-handling ecosystem.
#[derive(Debug, Clone, PartialEq)]
pub enum ProviderError {
    /// A provider with the given ID is already registered.
    DuplicateProvider {
        provider_id: String,
    },
    /// A capability with the given ID is already registered — either by this
    /// provider or by another.
    DuplicateCapability {
        capability_id: String,
        provider_id: String,
    },
    /// A capability definition failed validation.
    InvalidCapability {
        capability_id: String,
        provider_id: String,
        reason: String,
    },
    /// Registration with the capability registry failed for a reason not
    /// covered by the variants above.
    RegistrationFailed {
        provider_id: String,
        reason: String,
    },
    /// The provider's `initialize` hook returned an error.
    InitializationFailed {
        provider_id: String,
        reason: String,
    },
    /// The provider's `shutdown` hook returned an error.
    ShutdownFailed {
        provider_id: String,
        reason: String,
    },
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DuplicateProvider { provider_id } => {
                write!(f, "Provider '{}' is already registered", provider_id)
            }
            Self::DuplicateCapability {
                capability_id,
                provider_id,
            } => {
                write!(
                    f,
                    "Capability '{}' from provider '{}' is already registered",
                    capability_id, provider_id
                )
            }
            Self::InvalidCapability {
                capability_id,
                provider_id,
                reason,
            } => {
                write!(
                    f,
                    "Capability '{}' from provider '{}' is invalid: {}",
                    capability_id, provider_id, reason
                )
            }
            Self::RegistrationFailed {
                provider_id,
                reason,
            } => {
                write!(
                    f,
                    "Capability registration failed for provider '{}': {}",
                    provider_id, reason
                )
            }
            Self::InitializationFailed {
                provider_id,
                reason,
            } => {
                write!(
                    f,
                    "Initialization failed for provider '{}': {}",
                    provider_id, reason
                )
            }
            Self::ShutdownFailed {
                provider_id,
                reason,
            } => {
                write!(
                    f,
                    "Shutdown failed for provider '{}': {}",
                    provider_id, reason
                )
            }
        }
    }
}

impl std::error::Error for ProviderError {}

// ---------------------------------------------------------------------------
// CapabilityProvider trait
// ---------------------------------------------------------------------------

/// The foundational abstraction for any object capable of supplying
/// capabilities to the Nabu Capability Platform.
///
/// Any type that can contribute capabilities to the application — a
/// built-in module, a native shared library plugin, a WASM plugin, a
/// scripting plugin, a remote provider, or a bundled capability pack —
/// implements this trait. The [`PluginManager`] interacts with providers
/// through this abstraction rather than concrete plugin implementations.
///
/// # Responsibilities
///
/// - **Provider identity** — a stable, unique identifier ([`id`](Self::id))
///   and optional version ([`version`](Self::version)).
/// - **Provider metadata** — a human-readable name ([`name`](Self::name))
///   and description ([`description`](Self::description)).
/// - **Capability enumeration** — declare which capabilities the provider
///   offers ([`capabilities`](Self::capabilities)).
/// - **Capability registration** — register capabilities with the
///   [`CapabilityRegistry`] through its public API
///   ([`register_capabilities`](Self::register_capabilities)).
/// - **Optional lifecycle hooks** — initialization and shutdown hooks
///   ([`initialize`](Self::initialize), [`shutdown`](Self::shutdown)) that
///   default to no-ops.
///
/// # Design Principles
///
/// - **Minimal surface** — only identity, metadata, capability enumeration,
///   and registration are required. Lifecycle hooks are optional.
/// - **No panics** — every fallible method returns `Result<_, ProviderError>`.
/// - **Thread-safe** — the trait requires `Send + Sync` so that future
///   phases can register providers concurrently without redesign.
/// - **No global state** — providers must not rely on global mutable state.
/// - **Registry-owned** — the provider never owns registered capabilities.
///   The [`CapabilityRegistry`] is the single source of truth.
/// - **No execution** — registration is metadata-only. No plugin code is
///   loaded or executed, no directories are scanned, and no capability is
///   initialized during registration.
///
/// # Future Compatibility
///
/// The trait is designed so that future plugin types can implement it
/// without requiring breaking changes. New methods with default
/// implementations may be added in future versions. Methods without
/// defaults constitute the stable ABI and will not change.
///
/// # Object Safety
///
/// The trait is object-safe: it can be used as `Arc<dyn CapabilityProvider>`
/// or `Box<dyn CapabilityProvider>`, which is how the `PluginManager` stores
/// heterogeneous providers.
pub trait CapabilityProvider: Send + Sync + std::fmt::Debug {
    // -----------------------------------------------------------------------
    // Identity & Metadata
    // -----------------------------------------------------------------------

    /// Returns the unique identifier of this provider.
    ///
    /// This is used by the `PluginManager` to detect duplicate providers and
    /// to map capabilities back to their origin. The identifier should be
    /// globally unique — reverse domain notation is recommended
    /// (e.g. `"com.example.my-plugin"`, `"nabu"` for built-in providers).
    fn id(&self) -> &str;

    /// Returns the human-readable name of this provider.
    ///
    /// Used for display in UI and logs. This is a human-facing label, not
    /// an identifier.
    fn name(&self) -> &str;

    /// Returns the version of this provider.
    ///
    /// Used for compatibility checks and version negotiation.
    fn version(&self) -> &Version;

    /// Returns a brief description of this provider.
    ///
    /// Defaults to an empty string. Override to provide a human-readable
    /// description of the provider's purpose.
    fn description(&self) -> &str {
        ""
    }

    // -----------------------------------------------------------------------
    // Capability Enumeration
    // -----------------------------------------------------------------------

    /// Returns the capabilities this provider offers.
    ///
    /// Each returned [`Capability`] is a typed identifier with a namespace,
    /// name, and description. The `PluginManager` uses this list to register
    /// capabilities through the [`CapabilityRegistry`].
    ///
    /// This method should be lightweight — it should return declaration
    /// metadata, not perform capability initialization or side effects.
    fn capabilities(&self) -> Vec<Capability>;

    // -----------------------------------------------------------------------
    // Capability Registration
    // -----------------------------------------------------------------------

    /// Registers this provider's capabilities with the given
    /// [`CapabilityRegistry`].
    ///
    /// This is the canonical entry point for capability registration. The
    /// provider delegates to the registry's public API — it does not bypass
    /// the registry or mutate internal collections directly.
    ///
    /// The default implementation iterates [`capabilities`](Self::capabilities)
    /// and calls `registry.register()` for each, rejecting duplicates via
    /// `registry.has()`. Providers with custom registration logic (e.g.
    /// conditional or lazy capabilities) may override this method.
    ///
    /// # Errors
    ///
    /// Returns [`ProviderError::DuplicateCapability`] if a capability with
    /// the same identifier is already registered in the registry. Returns
    /// [`ProviderError::RegistrationFailed`] for other failures.
    fn register_capabilities(
        &self,
        registry: &mut CapabilityRegistry,
    ) -> Result<(), ProviderError> {
        let provider_id = self.id();
        for cap in self.capabilities() {
            let cap_id = cap.id();
            if registry.has(&cap_id) {
                return Err(ProviderError::DuplicateCapability {
                    capability_id: cap_id,
                    provider_id: provider_id.to_string(),
                });
            }
            registry.register(cap, provider_id);
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Optional Lifecycle Hooks
    // -----------------------------------------------------------------------

    /// Performs provider initialization.
    ///
    /// This is an optional hook — the default implementation is a no-op.
    /// Providers may override this to set up resources, establish
    /// connections, or perform other setup.
    ///
    /// This hook is **not** called during `register_provider`. It is a
    /// separate phase that the `PluginManager` may invoke after
    /// capability registration succeeds and the provider has been stored.
    ///
    /// # Errors
    ///
    /// Return [`ProviderError::InitializationFailed`] if setup fails.
    fn initialize(&self) -> Result<(), ProviderError> {
        Ok(())
    }

    /// Performs provider shutdown.
    ///
    /// This is the inverse of [`initialize`](Self::initialize). The default
    /// implementation is a no-op. Providers should release resources and
    /// close connections here.
    ///
    /// # Errors
    ///
    /// Return [`ProviderError::ShutdownFailed`] if cleanup fails.
    fn shutdown(&self) -> Result<(), ProviderError> {
        Ok(())
    }
}

/// A type alias for a shared, thread-safe capability provider.
///
/// This is the concrete type that the `PluginManager` stores and interacts
/// with. Future plugin types are wrapped in `Arc<dyn CapabilityProvider>`
/// before being passed to `PluginManager::register_provider`.
pub type SharedProvider = Arc<dyn CapabilityProvider>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::version::Version;

    /// A simple test provider for unit testing the trait default methods.
    #[derive(Debug)]
    struct TestProvider {
        id: String,
        name: String,
        version: Version,
        description: String,
        caps: Vec<Capability>,
    }

    impl CapabilityProvider for TestProvider {
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
    }

    fn make_provider(id: &str, caps: Vec<Capability>) -> TestProvider {
        TestProvider {
            id: id.to_string(),
            name: format!("{} Provider", id),
            version: Version::new(1, 0, 0),
            description: "A test provider".to_string(),
            caps,
        }
    }

    #[test]
    fn default_register_capabilities_registers_all() {
        let mut registry = CapabilityRegistry::new();
        let provider = make_provider(
            "test.plugin",
            vec![
                Capability::new("test", "ocr", "OCR capability"),
                Capability::new("test", "export", "Export capability"),
            ],
        );

        let result = provider.register_capabilities(&mut registry);
        assert!(result.is_ok());
        assert!(registry.has("test:ocr"));
        assert!(registry.has("test:export"));
        assert_eq!(registry.provider("test:ocr"), Some("test.plugin"));
        assert_eq!(registry.provider("test:export"), Some("test.plugin"));
    }

    #[test]
    fn default_register_capabilities_rejects_duplicates() {
        let mut registry = CapabilityRegistry::new();
        // Pre-register a capability with the same ID
        registry.register(
            Capability::new("test", "ocr", "Existing capability"),
            "existing_provider",
        );

        let provider = make_provider(
            "test.plugin",
            vec![Capability::new("test", "ocr", "OCR capability")],
        );

        let result = provider.register_capabilities(&mut registry);
        assert!(result.is_err());
        match result.unwrap_err() {
            ProviderError::DuplicateCapability { capability_id, provider_id } => {
                assert_eq!(capability_id, "test:ocr");
                assert_eq!(provider_id, "test.plugin");
            }
            other => panic!("Expected DuplicateCapability, got {:?}", other),
        }
    }

    #[test]
    fn default_register_capabilities_rejects_partial_duplicate() {
        let mut registry = CapabilityRegistry::new();
        let provider = make_provider(
            "test.plugin",
            vec![
                Capability::new("test", "ocr", "OCR capability"),
                Capability::new("test", "export", "Export capability"),
                Capability::new("test", "ocr", "Duplicate within provider"),
            ],
        );

        // The first and third capabilities have the same ID.
        // Registration should fail on the duplicate.
        let result = provider.register_capabilities(&mut registry);
        assert!(result.is_err());
    }

    #[test]
    fn register_capabilities_empty_provider() {
        let mut registry = CapabilityRegistry::new();
        let provider = make_provider("test.empty", vec![]);

        let result = provider.register_capabilities(&mut registry);
        assert!(result.is_ok());
        assert_eq!(registry.capability_count(), 0);
    }

    #[test]
    fn default_lifecycle_hooks_are_no_ops() {
        let provider = make_provider("test.lifecycle", vec![]);
        assert!(provider.initialize().is_ok());
        assert!(provider.shutdown().is_ok());
    }

    #[test]
    fn description_defaults_to_empty() {
        let provider = TestProvider {
            id: "test".to_string(),
            name: "Test".to_string(),
            version: Version::new(1, 0, 0),
            description: String::new(),
            caps: vec![],
        };
        // No override of description() — should return ""
        assert_eq!(provider.description(), "");
    }

    #[test]
    fn provider_metadata_accessors() {
        let provider = make_provider("com.example.test", vec![]);
        assert_eq!(provider.id(), "com.example.test");
        assert_eq!(provider.version(), &Version::new(1, 0, 0));
        assert_eq!(provider.name(), "com.example.test Provider");
        assert_eq!(provider.description(), "A test provider");
    }

    #[test]
    fn provider_error_display() {
        let err = ProviderError::DuplicateProvider {
            provider_id: "test.plugin".into(),
        };
        assert!(format!("{}", err).contains("test.plugin"));

        let err = ProviderError::DuplicateCapability {
            capability_id: "test:ocr".into(),
            provider_id: "test.plugin".into(),
        };
        assert!(format!("{}", err).contains("test:ocr"));

        let err = ProviderError::InvalidCapability {
            capability_id: "test:bad".into(),
            provider_id: "test.plugin".into(),
            reason: "bad namespace".into(),
        };
        assert!(format!("{}", err).contains("bad namespace"));

        let err = ProviderError::RegistrationFailed {
            provider_id: "test.plugin".into(),
            reason: "registry locked".into(),
        };
        assert!(format!("{}", err).contains("registry locked"));

        let err = ProviderError::InitializationFailed {
            provider_id: "test.plugin".into(),
            reason: "config missing".into(),
        };
        assert!(format!("{}", err).contains("config missing"));

        let err = ProviderError::ShutdownFailed {
            provider_id: "test.plugin".into(),
            reason: "timeout".into(),
        };
        assert!(format!("{}", err).contains("timeout"));
    }

    #[test]
    fn provider_error_implements_error() {
        let err = ProviderError::DuplicateProvider {
            provider_id: "test".into(),
        };
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn trait_is_object_safe() {
        // Verify that the trait can be used as a trait object.
        let provider: Arc<dyn CapabilityProvider> = Arc::new(make_provider(
            "test",
            vec![Capability::new("test", "cap", "A capability")],
        ));
        assert_eq!(provider.id(), "test");
        assert!(!provider.capabilities().is_empty());
    }

    #[test]
    fn register_capabilities_through_trait_object() {
        let mut registry = CapabilityRegistry::new();
        let provider: Arc<dyn CapabilityProvider> = Arc::new(make_provider(
            "obj.plugin",
            vec![Capability::new("obj", "feature", "A feature")],
        ));

        let result = provider.register_capabilities(&mut registry);
        assert!(result.is_ok());
        assert!(registry.has("obj:feature"));
        assert_eq!(registry.provider("obj:feature"), Some("obj.plugin"));
    }

    #[test]
    fn custom_register_capabilities_override() {
        /// A provider that overrides register_capabilities to skip
        /// capabilities that are already registered.
        #[derive(Debug)]
        struct OverriddenProvider {
            id: String,
            name: String,
            version: Version,
            caps: Vec<Capability>,
        }

        impl CapabilityProvider for OverriddenProvider {
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
                    if !registry.has(&cap_id) {
                        registry.register(cap, self.id());
                    }
                }
                Ok(())
            }
        }

        let mut registry = CapabilityRegistry::new();
        registry.register(
            Capability::new("ns", "existing", "Pre-registered"),
            "other",
        );

        let provider = OverriddenProvider {
            id: "override.plugin".to_string(),
            name: "Override".to_string(),
            version: Version::new(1, 0, 0),
            caps: vec![
                Capability::new("ns", "existing", "Should be skipped"),
                Capability::new("ns", "new", "Should be registered"),
            ],
        };

        let result = provider.register_capabilities(&mut registry);
        assert!(result.is_ok());
        assert!(registry.has("ns:existing"));
        assert_eq!(registry.provider("ns:existing"), Some("other"));
        assert!(registry.has("ns:new"));
        assert_eq!(registry.provider("ns:new"), Some("override.plugin"));
    }
}
