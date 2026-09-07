# Nabu

**The local-first, privacy-first knowledge base built in pure Rust.**

Nabu is a Markdown-native desktop knowledge base for thinkers who demand speed, sovereignty, and polish. Write in clean Markdown. Think with interactive canvases, live backlinks, and a relationship graph. Everything runs **on your machine** — no cloud, no telemetry, no network calls unless you explicitly make them.

Built on a single Rust core (`nabu-core`) with a Dioxus 0.6 frontend compiled to WebAssembly and wrapped in Tauri v2 for native desktop performance. Zero JavaScript. Zero Electron. One codebase, one language, one source of truth.

---

## Quick Start

Download a prebuilt DMG from the [GitHub Releases](https://github.com/Nabu/Nabu/releases) page:

| Platform | Architecture | Build |
|----------|--------------|-------|
| macOS 13+ | Apple Silicon (arm64) | `Nabu-<version>-aarch64.dmg` |
| macOS 13+ | Intel (x86_64) | `Nabu-<version>-x86_64.dmg` |
| Windows 10+ | x86_64 | `Nabu-<version>-x64.msi` |
| Linux | x86_64 | `Nabu-<version>-x86_64.AppImage` |

> A universal binary DMG is also published when both macOS architectures are built.

---

## Why Nabu

| | Nabu |
|---|---|
| **Privacy** | All processing is local. Your notes never leave your machine unless you tell them to. |
| **Speed** | Native performance via Rust + WebAssembly. Startup in ~400ms, search in <10ms. |
| **Portability** | Plain Markdown on disk. No proprietary formats. Open your vault in any editor. |
| **Extensibility** | Capability platform with a plugin foundation. Extend Nabu with Rust plugins. |
| **Resilience** | Automatic version history, crash recovery, and undo/redo on every note. |

---

## Core Features

### Knowledge Management
- **Setup wizard** — first-launch flow to create or open a vault with native folder pickers
- **Recursive file tree** — reactive navigation with context menus, keyboard shortcuts, and command palette
- **Markdown editor** — live preview with task-checkbox support, toggle blocks, tables, and wiki-links (`[[Note]]`)
- **Tag parsing** — real-time extraction from frontmatter with tag-based filtering
- **Full-text search** — in-memory index with relevance ranking and backlink discovery
- **Relationship graph** — interactive canvas visualization of your vault's link structure
- **Template management** — frontmatter templates with variable substitution (`{{title}}`, `{{date}}`, `{{time}}`)
- **Theme engine** — reactive dark/light/system modes persisted to settings

### Capture & Ingestion
- **Clipboard capture** — automatically ingest from system clipboard
- **Screenshot ingestion** — capture and embed images directly
- **File drop** — drag-and-drop files into the editor
- **Folder watch** — monitor directories for new content
- **Dictation pill** — floating scratchpad for voice input via Whisper.cpp

### Version Control & Recovery
- **Snapshot history** — immutable version snapshots for every save, browseable with diff view
- **Restore & duplicate** — restore any past version or duplicate to a new path
- **Session recovery** — workspace state persisted and restored across launches
- **Crash detection** — `.running` marker detects unclean shutdowns and offers recovery
- **Undo/Redo** — full history stack with per-operation reversibility

### Capability Platform
- **Plugin foundation** — manifest-based plugin system with permissions, dependencies, and feature contracts
- **Process supervision** — managed subprocess lifecycle with health checks and restart policies
- **Synchronization** — provider-agnostic sync folder/status models with conflict detection
- **Diagnostics pipeline** — streaming diagnostics (lsp, linters, custom providers) to the editor
- **Live event bus** — unified `EventBus` with pub/sub, bridged to the frontend in real-time
- **Capability registry** — runtime enable/disable/list of platform capabilities over IPC
- **Health & metrics** — live service health reporting and runtime metrics exposed over IPC
- **Graceful shutdown** — coordinated teardown persists index and vault graph before exit

### Native Integrations
- **macOS Vision OCR** — automatic text extraction from images
- **PDF annotation** — dedicated viewer with highlight-to-note conversion
- **Whisper.cpp dictation** — local speech-to-text with configurable model sizes

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    src-tauri/ (Tauri v2)                  │
│  ── IPC commands ──────── 60+ Tauri invoke handlers     │
│  ── Event bridge ──────── EventBus → frontend (nabu-event)  │
└────────┬─────────────────────────────────────────────────┘
         │ WASM / cdylib
┌────────┴─────────────────────────────────────────────────┐
│                    crates/nabu-ui/ (Dioxus 0.6)          │
│  ── CSR frontend ──── Compiled to WASM, no JS runtime    │
│  ── Reactive state ── Dioxus signals + event subscriptions│
└────────┬─────────────────────────────────────────────────┘
         │ Rust calls
┌────────┴─────────────────────────────────────────────────┐
│                    crates/nabu-core/ (Rust core)          │
│                                                         │
│  Capture Engine ──▶ Worker Pool ──▶ Processing Pipeline │
│                                                         │
│  Storage Manager (KnowledgeObjects + Sidecar cache)     │
│  Indexer (full-text + backlinks)                        │
│  VaultGraph (adjacency model for relationship canvas)  │
│  EventBus (unified pub/sub across all services)         │
│                                                         │
│  Capability Platform: Plugin Manager, Process          │
│  Supervisor, Sync Layer, Diagnostics Platform           │
└─────────────────────────────────────────────────────────┘
```

### Data Flow

```
Capture (clipboard, file, folder)
    │
    ▼
CaptureEngine → WorkQueue → WorkerPool
    │
    ▼
Processing Pipeline (Harper, OCR, transformers)
    │
    ▼
StorageManager (writes .md + .json sidecar)
    │
    ▼
EventBus publishes ITEM_STORED
    ├─▶ Indexer (full-text index)
    └─▶ VaultGraph (relationship graph)
```

### Building from Source

#### Prerequisites
- Rust 1.75+ (stable)
- Tauri CLI v2 (`cargo install tauri-cli`)
- System dependencies: `libwebkit2gtk-dev`, `libssl-dev`, `pkg-config` (Linux)
- Node.js 20+ (CSS pipeline only — no Node runtime in the app)

#### Development

```bash
# Terminal 1 — Tailwind CSS watch
npm install
npm run css:watch

# Terminal 2 — Tauri dev server (hot-reloads UI changes)
cargo tauri dev
```

#### Production Build

```bash
cargo tauri build
```

Output: `src-tauri/target/release/bundle/` — signed DMG (macOS), MSI (Windows), AppImage (Linux).

#### Compile Check (CI)

```bash
# Check the Rust core
cargo check --workspace

# Check the Dioxus frontend (standalone workspace)
cd crates/nabu-ui
cargo check
```

---


## Documentation

| Resource | Description |
|----------|-------------|
| `AGENTS.md` | Agent guidelines and architecture notes |
| `docs/` | Architecture decision records and design documents |
| `crates/nabu-core/src/` | Rust doc comments (run `cargo doc` for full API reference) |

---

## Community

- **Issues:** [GitHub Issues](https://github.com/Nabu/Nabu/issues) — bug reports and feature requests
- **Discussions:** [GitHub Discussions](https://github.com/Nabu/Nabu/discussions) — Q&A and community chat
- **Contributing:** See `CONTRIBUTING.md`

---


## Built with open source

Nabu stands on the shoulders of these projects. Full transparency, full credit.

| Project | License | What it does |
|---|---|---|
| [anydoc](https://crates.io/crates/anydoc) | MIT | Document-to-Markdown conversion (PDF, DOCX, PPTX, XLSX, ODT, RTF, EPUB, CSV) |
| [Harper](https://github.com/automattic/harper) | Apache-2.0 | Offline English grammar checking |
| [FluidVoice](https://github.com/altic-dev/FluidVoice) | GPLv3 | On-device macOS dictation (Parakeet, Whisper, Nemotron, Apple Speech models) — separate-process integration via Apple Events/clipboard |
| [Tolaria](https://github.com/refactoringhq/tolaria) | AGPL-3.0 | Vault tools for AI agents (MCP server) and app skeleton |
| Tauri | Apache-2.0 | Desktop app framework |
| React | MIT | UI library |
| BlockNote | MPL-2.0 | Rich text editor |
| Tailwind CSS | MIT | Styling engine |
| shadcn/ui | MIT | UI component patterns |
| vis-network | Apache-2.0 | Graph view visualization |

Additional Rust/frontend dependency licenses are enumerated in [NOTICE.md](NOTICE.md).

## Security

If you believe you have found a security issue, please report it privately as described in [SECURITY.md](./SECURITY.md).

## License

Nabu is licensed under AGPL-3.0-or-later. The Nabu name and logo remain covered by the project’s trademark policy.

---

*The name "Nabu" is inspired by the ancient Mesopotamian deity of writing and knowledge. This project is not affiliated with any commercial entity.*
