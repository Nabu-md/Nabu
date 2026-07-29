//! Plugin Architecture Foundation — metadata, capabilities, and lifecycle
//! infrastructure for future Nabu plugins.
//!
//! This module provides the **architectural foundation** for plugins without
//! implementing any third-party plugin loading. All the infrastructure that
//! future plugins will use is defined here as metadata types, registries,
//! and validation logic.
//!
//! # Core Concepts
//!
//! - **PluginManifest** — describes what a plugin is and what it provides.
//! - **Capability** — a named service type (OCR, LLM, Export, Capture, etc.)
//!   that a plugin or built-in component can provide.
//! - **CapabilityRegistry** — central registry of all capabilities provided
//!   by both built-in services and future plugins.
//! - **DependencyGraph** — resolves capability dependencies between services.
//! - **Version** — semantic versioning for compatibility negotiation.
//! - **FeatureFlag** — runtime toggles for optional capabilities.
//! - **PluginLifecycle** — lifecycle hooks for plugin-aware components.
//!
//! # Design Principles
//!
//! 1. **No external code loading** — this is metadata-only infrastructure.
//! 2. **Works for built-in services today** — capabilities describe existing
//!    services (OCR, embeddings, LLM, etc.) before any plugin exists.
//! 3. **Forward-compatible** — when third-party plugins are added later,
//!    they use the same types and registries as built-in components.
//! 4. **Thread-safe** — all registries use interior mutability for shared access.

pub mod capability;
pub mod dependencies;
pub mod feature_flags;
pub mod hooks;
pub mod manifest;
pub mod version;

pub use capability::CapabilityRegistry;
pub use dependencies::DependencyGraph;
pub use feature_flags::FeatureFlags;
pub use hooks::PluginLifecycle;
pub use manifest::PluginManifest;
pub use version::Version;

use std::collections::HashMap;

/// Unique identifier for a plugin or built-in component.
pub type PluginId = String;

/// A permission that a plugin may request.
pub type Permission = String;

/// Standard capability identifiers recognized by the system.
pub mod capabilities {
    /// Full-text search capability.
    pub const SEARCH: &str = "nabu:search";
    /// Embedding/vector generation capability.
    pub const EMBEDDINGS: &str = "nabu:embeddings";
    /// Large Language Model inference.
    pub const LLM: &str = "nabu:llm";
    /// Optical Character Recognition.
    pub const OCR: &str = "nabu:ocr";
    /// Speech-to-text / Whisper transcription.
    pub const STT: &str = "nabu:stt";
    /// Export to various formats.
    pub const EXPORT: &str = "nabu:export";
    /// Import from various formats.
    pub const IMPORT: &str = "nabu:import";
    /// Knowledge capture from external sources.
    pub const CAPTURE: &str = "nabu:capture";
    /// Processing pipeline processor.
    pub const PROCESSOR: &str = "nabu:processor";
    /// Graph/relationship engine.
    pub const GRAPH: &str = "nabu:graph";
    /// Storage persistence layer.
    pub const STORAGE: &str = "nabu:storage";
    /// Event communication bus.
    pub const EVENT_BUS: &str = "nabu:event_bus";
    /// Theme provider.
    pub const THEME: &str = "nabu:theme";
    /// Content provider (fetching from URLs, APIs).
    pub const CONTENT_PROVIDER: &str = "nabu:content_provider";
    /// Workflow automation.
    pub const WORKFLOW: &str = "nabu:workflow";
    /// View/canvas rendering.
    pub const VIEW: &str = "nabu:view";
}

/// Standard permissions recognized by the plugin system.
pub mod permissions {
    /// Permission to read vault files.
    pub const READ_VAULT: &str = "nabu:read_vault";
    /// Permission to write vault files.
    pub const WRITE_VAULT: &str = "nabu:write_vault";
    /// Permission to access the network.
    pub const NETWORK: &str = "nabu:network";
    /// Permission to access the file system.
    pub const FILE_SYSTEM: &str = "nabu:file_system";
    /// Permission to read clipboard content.
    pub const CLIPBOARD_READ: &str = "nabu:clipboard_read";
    /// Permission to access microphone/audio.
    pub const MICROPHONE: &str = "nabu:microphone";
    /// Permission to access the camera.
    pub const CAMERA: &str = "nabu:camera";
    /// Permission to execute user-provided code.
    pub const EXECUTE: &str = "nabu:execute";
}
