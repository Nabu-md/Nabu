# Plugin Architecture Foundation

## Overview

This document describes Nabu's plugin architecture foundation — a set of
metadata, registration, and lifecycle types designed to make Nabu extensible
without implementing third-party plugin loading.

**Current status:** Infrastructure only. No external code loading is implemented.
All built-in components use the same types that future plugins will use.

---

## Design Principles

1. **No external code loading** — this is metadata-only infrastructure.
2. **Built-in first** — capabilities describe existing services (OCR, embeddings,
   LLM, etc.) before any plugin exists.
3. **Forward-compatible** — when third-party plugins are added later, they use
   the same types and registries as built-in components.
4. **Thread-safe** — all registries use interior mutability (`RwLock`) for safe
   shared access across threads.

---

## Module Structure

```
crates/nabu-core/src/plugin/
├── mod.rs             # Root: re-exports, module wiring
├── manifest.rs        # PluginManifest, PluginMetadata, compatibility validation
├── capability.rs      # CapabilityRegistry, builtin_capabilities()
├── dependency.rs      # DependencyGraph — resolution, cycle detection, topological sort
├── features.rs        # FeatureRegistry, FeatureFlag, FeatureStage
├── lifecycle.rs       # PluginLifecycle, PluginStage, PluginLifecycleEvent
├── manager.rs         # PluginManager — orchestration and installation reports
├── permissions.rs     # Permission, PermissionSet, PermissionEvaluator, RiskLevel
├── version.rs         # Version, VersionRequirement — semantic versioning + parsing
├── events.rs          # PluginEvent, PluginEventContract, shared event types
├── invocation.rs      # PluginInvocationRequest/Response, ExecutionMetadata, errors
└── provider.rs        # CapabilityProvider trait, ProviderError, register_capabilities
```

> **Note:** The code samples in the sections below are illustrative of the
> intent of each module. Always verify against the actual public API in
> `crates/nabu-core/src/plugin/` before using these types.

---

## Core Types

### PluginManifest (`manifest.rs`)

The canonical description of a plugin or built-in component:

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Unique identifier (e.g., `"my-ocr-engine"`) |
| `name` | `String` | Display name |
| `version` | `Version` | Current semantic version |
| `author` | `String` | Author name |
| `description` | `String` | Short description |
| `min_nabu_version` | `Version` | Minimum Nabu version required |
| `max_tested_version` | `Option<Version>` | Highest version tested against |
| `manifest_version` | `u32` | Manifest format version |
| `capabilities` | `Vec<PluginCapability>` | What this plugin provides |
| `dependencies` | `Vec<PluginDependency>` | Required dependencies |
| `optional_dependencies` | `Vec<PluginDependency>` | Optional dependencies |
| `feature_flags` | `Vec<PluginFeatureFlag>` | Toggleable optional features |
| `permissions` | `Vec<PluginPermission>` | Runtime permissions requested |
| `entry_type` | `PluginEntryType` | Wasm, Lua, native, external |

**Validation:** `manifest.validate()` returns `Vec<ManifestError>` — checks internal
consistency (non-empty ID, valid capability namespacing).

**Compatibility:** `manifest.check_nabu_compatibility(&nabu_version)` returns a
`CompatibilityCheck` that distinguishes `Compatible`, `CompatibleWithWarnings`,
and `Incompatible`.

### CapabilityRegistry (`capability.rs`)

Central index of available capabilities:

```rust
let mut registry = CapabilityRegistry::new();

// Built-in capabilities registered at startup
registry.register_builtin();

// Look up capabilities
registry.has("nabu:ocr");              // true
registry.get("nabu:ocr");              // Option<&Capability>
registry.provider("nabu:ocr");         // Option<&str> — "nabu"
registry.list();                        // Vec<String> of all IDs
registry.list_enabled();                // Vec<String> of enabled IDs
registry.namespace_has("nabu", "llm");
registry.provider_capabilities("nabu");
```

### DependencyGraph (`dependency.rs`)

Directed graph for resolving plugin dependencies:

- **Cycle detection** via DFS with three-color marking
- **Topological sort** via Kahn's algorithm
- **Missing-dependency reporting** against the registered manifests

```rust
let mut graph = DependencyGraph::new();
graph.add_plugin(&manifest_a);
graph.add_plugin(&manifest_b);
graph.add_dependency("plugin-a", "plugin-b");

// Check for cycles
let cycles: Vec<Vec<String>> = graph.detect_cycles();

// Missing dependencies / initialization order
let missing: Vec<MissingDependency> = graph.missing_dependencies();
let order: Option<Vec<String>> = graph.topological_order(); // deps first
```

### PluginLifecycle (`lifecycle.rs`)

State machine governing plugin lifecycle:

```
Discovered → Validated → Installed → Enabled → Disabled → Upgraded → Unloaded
```

| Stage | Meaning |
|-------|---------|
| `Discovered` | Plugin was found but not yet processed |
| `Validated` | Manifest validated |
| `Installed` | Dependencies resolved, setup complete |
| `Enabled` | Actively providing services |
| `Disabled` | Disabled at runtime, not active |
| `Upgraded` | Version upgrade applied |
| `Unloaded` | Fully cleaned up, terminal stage |

```rust
let lc = PluginLifecycle::new();
lc.transition_to(PluginStage::Validated); // records a lifecycle event
lc.stage();                               // PluginStage
lc.history();                             // &[PluginLifecycleEvent]
lc.is_enabled();                          // stage >= Enabled
lc.is_unloaded();                         // terminal
```

The `PluginManager` owns the lifecycle of every registered plugin:
```rust
let mut manager = PluginManager::new(nabu_version);
manager.register_manifest(manifest_a);
manager.register_manifest(manifest_b);
let report: InstallationReport = manager.install_all(); // boot + validate + install
manager.enable("plugin-a");   // Result<(), ManagerError>
manager.disable("plugin-a");  // Result<(), ManagerError>
manager.list_plugins();       // Vec<String>
manager.stage("plugin-a");    // Option<PluginStage>
```

### FeatureRegistry (`features.rs`)

Runtime toggle framework:

```rust
let mut flags = FeatureRegistry::new();
flags.register_standard_flags(); // registers the built-in flag set
flags.register(FeatureFlag {
    name: "nabu:experimental_ocr".into(),
    description: "Experimental OCR".into(),
    enabled_by_default: false,
    enabled: false,
    stage: FeatureStage::Experimental,
});

flags.is_enabled("nabu:experimental_ocr"); // false
flags.enable("nabu:experimental_ocr");     // toggle at runtime
flags.disable("nabu:experimental_ocr");
flags.reset("nabu:experimental_ocr");      // back to default

flags.list();      // Vec<&FeatureFlag>
flags.by_stage(FeatureStage::Experimental);
flags.overridden(); // Vec<&FeatureFlag> with non-default values
```

### Version (`version.rs`)

Semantic versioning with requirement operators:

| Variant | Display | Meaning |
|---------|---------|---------|
| `VersionRequirement::Exact(v)` | `==1.2.3` | Exactly 1.2.3 |
| `VersionRequirement::Compatible(v)` | `~1.2.3` | Same major, minor ≥ required |
| `VersionRequirement::Range(min, max)` | `>=1.0.0, <=2.0.0` | Inclusive range |
| `VersionRequirement::GreaterThan(v)` | `>=1.0.0` | At least 1.0.0 (via `VersionRequirement::at_least(1, 0, 0)`) |

---

## Standard Capabilities

Built-in capabilities are returned by `builtin_capabilities()` in
`plugin::capability` and registered via `CapabilityRegistry::register_builtin()`:

| ID | Required | Description |
|----|----------|-------------|
| `nabu:event_bus` | ✅ | Publish/subscribe messaging backbone |
| `nabu:storage` | ✅ | Knowledge object persistence |
| `nabu:capture` | ✅ | Content capture and ingestion |
| `nabu:processor` | ✅ | Content processing pipeline |
| `nabu:graph` | ✅ | Semantic relationship graph |
| `nabu:export` | — | Document export to various formats |
| `nabu:search` | — | Full-text search engine |
| `nabu:ocr` | — | Optical character recognition |
| `nabu:ai` | — | AI provider integration |
| `nabu:embedding` | — | Vector embedding generation |
| `nabu:template` | — | Note template management |
| `nabu:sync` | — | Vault synchronization |
| `nabu:watch` | — | File system watching |
| `nabu:plugin` | — | Plugin management lifecycle |

---

## Standard Permissions

Standard permission definitions are returned by `standard_permissions()` in
`plugin::permissions` (14 built-in definitions):

| Name | Description | Risk |
|------|-------------|------|
| `vault.access` | Read and write access to vault content | High |
| `vault.read` | Read-only access to vault content | Medium |
| `capture.access` | Access the capture pipeline | Medium |
| `filesystem.read` | Read files on the local filesystem | Medium |
| `filesystem.write` | Write files to the local filesystem | High |
| `ai.providers` | Access configured AI providers | High |
| `network.http` | Make HTTP requests to external services | Critical |
| `network.websocket` | Open WebSocket connections | Critical |
| `export.access` | Access the export engine | Low |
| `system.process` | Spawn subprocesses | Critical |
| `system.env` | Read environment variables | Medium |
| `event_bus.subscribe` | Subscribe to event bus topics | Low |
| `event_bus.publish` | Publish to the event bus | Medium |
| `storage.access` | Access the storage layer | High |

---

## Integration with Existing Infrastructure

### Service Registry (Prompt 31)

The `ServiceRegistry` from Prompt 31 handles concrete service registration and
resolution (which instance of `StorageManager`, which instance of `CaptureEngine`).

The plugin architecture's `CapabilityRegistry` operates at a higher level of
abstraction — it describes *what* capabilities exist, not *which concrete service
instance* provides them. The two registries complement each other:

1. **ServiceRegistry** resolves "I need the `StorageManager` instance"
2. **CapabilityRegistry** resolves "Is OCR available in this system?"
3. **DependencyGraph** resolves "What order should I initialize these plugins?"

### ApplicationContext (Prompt 31)

`ApplicationContext` owns the `CapabilityRegistry`, `FeatureRegistry`,
`PluginManager` (which contains `DependencyGraph` + `PluginLifecycle`)
alongside the `ServiceRegistry`.
When future plugin loading is implemented, plugins will register with both
the `CapabilityRegistry` (metadata) and `ServiceRegistry` (concrete instances).

---

## Migration Path for Future Plugins

When third-party plugin loading is implemented in a future prompt, the
architecture is ready:

1. **Discovery** — Scan plugin directories, parse `.toml`/`.json` manifests
2. **Registration** — Register capabilities in `CapabilityRegistry`
3. **Validation** — Check `PluginManifest::validate()` and `check_compatibility()`
4. **Dependency Resolution** — Resolve via `DependencyGraph`
5. **Installation** — `PluginManager::install_all()` drives validate → install
6. **Runtime** — Services available via `CapabilityRegistry` lookups
7. **Shutdown** — `PluginManager::disable(id)` + lifecycle `Unloaded`, unregister capabilities

No architectural changes needed — only the plugin loader itself.

---

## Invocation Pipeline (Phase 6.3.1)

The invocation pipeline routes structured requests from the frontend through the
`PluginManager` and into `CapabilityProvider::invoke`, returning structured
responses. This section documents the complete flow, validation layers, error
normalization, and observability.

### Architecture

```text
Frontend (Dioxus)
  │  (serializes PluginInvocationRequest as JSON)
  ▼
plugin_call IPC command (src-tauri/src/commands.rs)
  │  (deserializes via Tauri's State<ApplicationContext>)
  ▼
ApplicationContext::invoke_plugin() (registry/context.rs)
  │  (acquires RwLock on PluginManager, delegates)
  ▼
PluginManager::invoke_capability() (plugin/manager.rs)
  │  1. Validates request fields (plugin_id, capability, method non-empty)
  │  2. Locates provider by plugin_id in providers HashMap
  │  3. Validates capability is registered in CapabilityRegistry
  │  4. Validates capability is owned by the target provider
  │  5. Validates capability is enabled (unless `required`)
  │  6. Publishes PluginRequestEvent via EventBus (if attached)
  │  7. Dispatches to provider.invoke() inside catch_unwind (panic-safe)
  │  8. Enriches response with ExecutionMetadata (duration, provider, request_id)
  │  9. Propagates API version from request metadata to response
  │  10. Publishes PluginResponseEvent via EventBus (if attached)
  ▼
CapabilityProvider::invoke()
  │  (provider-specific execution — may return success, error, or not supported)
  ▼
PluginInvocationResponse
  │  (serialized by Tauri back to frontend as Result<_, String>)
  ▼
Frontend
```

### Request/Response Models (`invocation.rs`)

| Type | Fields | Purpose |
|------|--------|---------|
| `PluginInvocationRequest` | `plugin_id`, `capability`, `method`, `input` (Option\<Value\>), `metadata` (Option\<InvocationMetadata\>) | Canonical wire format from frontend |
| `InvocationMetadata` | `request_id` (Uuid), `timestamp`, `timeout_ms`, `caller`, `api_version`, `context` | Tracing, timeout, version negotiation |
| `PluginInvocationResponse` | `success`, `status`, `result` (Option\<Value\>), `error` (Option\<Error\>), `execution` (Option\<ExecutionMetadata\>) | Structured result returned to frontend |
| `ExecutionMetadata` | `request_id`, `provider`, `capability`, `duration_ms`, `api_version` | Host-level observability metadata |
| `PluginInvocationError` | `code`, `message`, `detail` (Option) | Machine-parseable error with human-readable message |
| `PluginInvocationStatus` | `Success` / `Error` / `Cancelled` | High-level outcome enum |

All types derive `Serialize` + `Deserialize` and use `#[serde(default)]` for
forward compatibility. All types are `Send + Sync + Clone`.

### Error Codes

| Code | Meaning |
|------|---------|
| `INVALID_REQUEST` | Request fields are empty or malformed |
| `PLUGIN_NOT_FOUND` | No provider registered for the given `plugin_id` |
| `CAPABILITY_NOT_FOUND` | Capability not registered, or owned by a different provider |
| `CAPABILITY_DISABLED` | Capability is registered but not enabled |
| `CAPABILITY_NOT_SUPPORTED` | Provider's default `invoke` (did not override) |
| `PROVIDER_ERROR` | Provider returned a structured error response |
| `PROVIDER_PANIC` | Provider panicked; caught via `catch_unwind` |
| `PROVIDER_TIMEOUT` | Provider exceeded timeout (future async phase) |

### Panic Safety

Provider `invoke` calls are wrapped in `std::panic::catch_unwind`. A panicking
provider never crashes the host process — the panic payload is extracted as a
string and returned as a `PROVIDER_PANIC` structured error with the panic
message in the `detail` field. A `PluginErrorEvent` with severity `Critical`
is also published to the EventBus (if attached) with code `PROVIDER_PANIC`.

### EventBus Observability

When a `PluginManager` is constructed with an `EventBus` (via
`with_event_bus`), every invocation publishes two events:

1. **`PluginRequestEvent`** (kind: `plugin.request`) — published before dispatch,
   with the merged `capability:method` string and the input payload.
2. **`PluginResponseEvent`** (kind: `plugin.response`) — published after
   completion, with the response status, result (on success), or error message
   (on failure).

Both events carry:
- `plugin_id` — the requesting plugin
- `request_id` — correlation UUID (generated by the manager if not provided)
- `method` — `{capability}:{method}` string
- `api_version` — the current `PluginApiVersion::CURRENT`
- `timestamp` — event creation time

The frontend event bridge (`src-tauri/src/event_bridge.rs`) forwards both
`plugin.request` and `plugin.response` kinds to the frontend via the Tauri
`nabu-event` channel.

### Thread Safety

`invoke_capability` takes `&self` (shared reference). The `PluginManager`'s
internal `providers` HashMap is read concurrently via `Arc<dyn
CapabilityProvider>`. Concurrent invocations are safe as long as the provider
itself is `Send + Sync` — which the `CapabilityProvider` trait requires.

### Capabilities and Enabled State

Capabilities are **enabled by default** when registered through
`register_provider` (the manager flips them on in the staged registry copy
before committing). The `invoke_capability` method rejects invocations to
disabled capabilities with `CAPABILITY_DISABLED`, unless the capability is
marked `required` (built-in capabilities are always enabled via
`register_builtin`).
