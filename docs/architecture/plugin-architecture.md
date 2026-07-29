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
├── mod.rs             # Root: re-exports, standard constants (capabilities, permissions)
├── manifest.rs        # PluginManifest, PluginMetadata, compatibility validation
├── capability.rs      # CapabilityRegistry — what services are available
├── dependencies.rs    # DependencyGraph — resolution, cycle detection, topological sort
├── feature_flags.rs   # FeatureFlags — runtime toggles, gating, change notification
├── hooks.rs           # PluginLifecycle — stage machine (discovered → ... → unloaded)
└── version.rs         # Version, VersionReq — semantic versioning + parsing
```

---

## Core Types

### PluginManifest (`manifest.rs`)

The canonical description of a plugin or built-in component:

| Field | Type | Description |
|-------|------|-------------|
| `id` | `PluginId` | Unique identifier (e.g., `"my-ocr-engine"`) |
| `metadata` | `PluginMetadata` | Name, description, author, homepage, license, tags |
| `version` | `Version` | Current semantic version |
| `min_nabu_version` | `Version` | Minimum Nabu version required |
| `capabilities` | `HashSet<CapabilityId>` | What this plugin provides |
| `dependencies` | `Vec<PluginDependency>` | What this plugin requires |
| `permissions` | `HashSet<Permission>` | Runtime permissions requested |
| `features` | `Vec<PluginFeature>` | Toggleable optional features |

**Validation:** `manifest.validate()` checks internal consistency (non-empty ID,
valid capability namespacing).

**Compatibility:** `manifest.check_compatibility(nabu_version)` returns a
`Compatibility` result that distinguishes `Compatible`, `CompatibleWithWarnings`,
and `Incompatible`.

### CapabilityRegistry (`capability.rs`)

Central index of available capabilities:

```rust
let registry = CapabilityRegistry::new();

// Built-in capabilities registered at startup
registry.register_builtin_capabilities(&nabu_version);

// Look up providers
let ocr_providers = registry.get_providers("nabu:ocr");
let first_ocr = registry.get_first_provider("nabu:ocr");

// Check availability
let status = registry.check_capability("nabu:llm");
// Returns Available | Disabled | Unavailable | VersionMismatch

// Validate dependencies
let issues = registry.check_dependencies(&plugin_dependencies);
```

### DependencyGraph (`dependencies.rs`)

Directed graph for resolving capability dependencies:

- **Cycle detection** via DFS with three-color marking
- **Topological sort** via Kahn's algorithm
- **Version validation** against `CapabilityRegistry`

```rust
let graph = DependencyGraph::new();
graph.add_node(DependencyNode { id, dependencies, provides, enabled });
graph.add_node(...);

// Check for cycles
if let Some(cycle) = graph.detect_cycles() { ... }

// Resolve initialization order
let result = graph.resolve(&capability_registry);
// result.order — topological order (dependencies first)
// result.errors  — missing/version-mismatch errors
// result.warnings — optional dependency notices
```

### PluginLifecycle (`hooks.rs`)

State machine governing plugin/component lifecycle:

```
Discovered → Registered → Initialized → Started → Stopped → Unloaded
```

| Stage | Meaning |
|-------|---------|
| `Discovered` | Plugin was found but not yet processed |
| `Registered` | Manifest validated and registered |
| `Initialized` | Dependencies resolved, setup complete |
| `Started` | Actively providing services |
| `Stopped` | Gracefully stopped, no longer active |
| `Unloaded` | Fully cleaned up, terminal stage |

```rust
let lc = PluginLifecycle::new("my-ocr");

// Register hooks
lc.on_started(|id, stage| { println!("{} is now {}", id, stage); });

// Progress
lc.boot().unwrap();    // Discovered → ... → Started
lc.shutdown().unwrap(); // Started → ... → Unloaded
```

The `LifecycleManager` batches operations:
```rust
let manager = LifecycleManager::new();
manager.register(PluginLifecycle::new("plugin-a"));
manager.register(PluginLifecycle::new("plugin-b"));
manager.boot_all();     // Boot all discovered plugins
// later...
manager.shutdown_all(); // Graceful shutdown
```

### FeatureFlags (`feature_flags.rs`)

Runtime toggle framework:

```rust
let flags = FeatureFlags::new();
flags.register(FeatureFlag::new("nabu:experimental_ocr", "Experimental OCR", "...")
    .with_default(false)
    .experimental());

flags.is_enabled("nabu:experimental_ocr"); // false
flags.set_enabled("nabu:experimental_ocr", true); // toggle at runtime

// Scoped overrides for testing
flags.set_override("nabu:verbose_logging", Some(true));
flags.clear_overrides();

// Change notifications
flags.on_change(|id, enabled| { println!("Flag '{}' = {}", id, enabled); });
```

### Version (`version.rs`)

Semantic versioning with requirement operators:

| Operator | Name | Example | Meaning |
|----------|------|---------|---------|
| `*` | Any | `*` | All versions |
| `=x.y.z` | Exact | `=1.2.3` | Exactly 1.2.3 |
| `^x.y.z` | Compatible | `^1.2.3` | 1.y.z where y ≥ 2 |
| `~x.y.z` | Patch | `~1.2.3` | 1.2.x where x ≥ 3 |
| `>=x.y.z` | Minimum | `>=1.0.0` | At least 1.0.0 |

---

## Standard Capabilities

Defined as constants in `plugin::capabilities`:

| Constant | Value | Description |
|----------|-------|-------------|
| `SEARCH` | `"nabu:search"` | Full-text search |
| `EMBEDDINGS` | `"nabu:embeddings"` | Vector embeddings |
| `LLM` | `"nabu:llm"` | Language model inference |
| `OCR` | `"nabu:ocr"` | Optical character recognition |
| `STT` | `"nabu:stt"` | Speech-to-text |
| `EXPORT` | `"nabu:export"` | Export to formats |
| `IMPORT` | `"nabu:import"` | Import from formats |
| `CAPTURE` | `"nabu:capture"` | Knowledge capture |
| `PROCESSOR` | `"nabu:processor"` | Processing pipeline |
| `GRAPH` | `"nabu:graph"` | Relationship graph |
| `STORAGE` | `"nabu:storage"` | Storage layer |
| `EVENT_BUS` | `"nabu:event_bus"` | Event bus |
| `THEME` | `"nabu:theme"` | Theme provider |
| `CONTENT_PROVIDER` | `"nabu:content_provider"` | Content fetching |
| `WORKFLOW` | `"nabu:workflow"` | Workflow automation |
| `VIEW` | `"nabu:view"` | View rendering |

---

## Standard Permissions

Defined as constants in `plugin::permissions`:

| Constant | Value | Description |
|----------|-------|-------------|
| `READ_VAULT` | `"nabu:read_vault"` | Read vault files |
| `WRITE_VAULT` | `"nabu:write_vault"` | Write vault files |
| `NETWORK` | `"nabu:network"` | Network access |
| `FILE_SYSTEM` | `"nabu:file_system"` | File system access |
| `CLIPBOARD_READ` | `"nabu:clipboard_read"` | Read clipboard |
| `MICROPHONE` | `"nabu:microphone"` | Microphone access |
| `CAMERA` | `"nabu:camera"` | Camera access |
| `EXECUTE` | `"nabu:execute"` | Code execution |

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

`ApplicationContext` owns the `CapabilityRegistry`, `FeatureFlags`,
`LifecycleManager`, and `DependencyGraph` alongside the `ServiceRegistry`.
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
5. **Initialization** — Create instances, call lifecycle hooks
6. **Runtime** — Services available via `CapabilityRegistry` lookups
7. **Shutdown** — Call `PluginLifecycle::shutdown()`, unregister capabilities

No architectural changes needed — only the plugin loader itself.
