# Nabu 🚀

**The lightning-fast, local-first knowledge base built in pure Rust.**

Nabu is a Markdown-native, open-source (AGPL-3.0) desktop knowledge base that bridges the gap between clean Markdown text and interactive web software. Your data stays **100% portable, plain-text, and token-efficient** — no lock-in. By leveraging **Tauri v2** and **Dioxus 0.6** (WASM/CSR), Nabu delivers a responsive native desktop experience with a single Rust codebase across the entire application: backend, engine, and UI.

---

## ⬇️ Download

Nabu ships as a native desktop app built with Tauri. Prebuilt binaries for macOS are published on the [GitHub Releases](https://github.com/Nabu/Nabu/releases) page:

| Platform | Architecture | Build |
|----------|--------------|-------|
| macOS 13+ | **Apple Silicon** (arm64) | `Nabu-<version>-aarch64.dmg` |
| macOS 13+ | **Intel** (x86_64) | `Nabu-<version>-x86_64.dmg` |

> Artifacts are attached on each tagged release. While a release isn't published yet, you can build from source (see **Building from Source** below). Universal (thin) single-binary DMGs are also produced when both architectures are built.

---

## 📦 Architecture

```
crates/
├── nabu-core/       # Core engine: vault logic, models, processing pipeline,
│                    #   storage, indexer, graph, capability platform, diagnostics
├── nabu-ui/         # Dioxus 0.6 CSR frontend, compiled to WASM (cdylib)
src-tauri/           # Tauri v2 desktop shell, IPC commands & event bridge
```

- **`nabu-core`** — The single authoritative Rust core. One `EventBus`, one `StorageManager`, one `WorkerPool`, one `Indexer`, one `VaultGraph`. Content flows `Capture → Queue → Workers → Pipeline → Storage → Indexer + Graph`. Lives in its own Cargo workspace crate.
- **`nabu-ui`** — Standalone Dioxus 0.6.3 CSR frontend (its own Cargo workspace) compiled to WASM.
- **`src-tauri`** — Tauri v2 backend providing IPC commands (settings, file capture, dictation, statistics, version history, recovery, templates, inbox, capability platform) plus a unified `EventBus → frontend` bridge.

## 🔧 Building from Source

### Prerequisites
- Rust (latest stable)
- Node.js ≥ 20 + npm (Tailwind CSS pipeline)
- Tauri CLI v2 (`cargo install tauri-cli --version ^2`)
- Xcode Command Line Tools (`xcode-select --install`)

> A `notify`-style native watcher is used for the filesystem; no Node/Electron runtime is required.

### Compile Check

```bash
cd "crates/nabu-ui"
cargo check
```

This checks the Dioxus frontend in isolation. The root workspace (`cargo check` from project root) compiles `nabu-core` + `src-tauri`.

### Dev Mode

```bash
npm install
npm run css:build     # generate ./generated/tailwind.css
cargo tauri dev
```

### Release Build

```bash
cargo tauri build     # produces dmg/app (macOS), plus x86_64 / arm64 variants
```

---

## 🚀 Features

### Core & Editing
- **Vault Management** — setup wizard, native OS folder pickers, vault creation/opening, last-vault restore.
- **FileTree & Navigation** — recursive, reactive file tree with context menus, keyboard shortcuts, and a command palette.
- **Note Editor** — live Markdown preview with interactive task-checkbox support, toggle blocks, tables, and wiki-links (`[[Note]]`).
- **Tag Parsing** — real-time tag extraction and indexing.
- **Full-Text Search** — in-memory index over processed KnowledgeObjects, plus related-note backlinks.
- **Relationship Graph** — `VaultGraph` adjacency model with canvas visualization.
- **Dynamic Theme Engine** — reactive dark/light mode persisted to settings.
- **Template Management** — frontmatter templates with variable substitution (`{{title}}`, `{{date}}`, `{{time}}`).

### Capture, Inbox & Recovery
- **Capture Handlers** — clipboard, screenshot, file-drop, folder-watch, and browser/Safari ingestion.
- **Inbox & Queue** — batch-approve/reject/retry/delete, priority & status, provenance tracking, dedup, progress.
- **Dictation Pill** — floating scratchpad / dictation / file-drop zone with clipboard cache (Whisper-backed).
- **Version History & Recovery** — snapshot browsing, diff view, restore, duplicate, session recovery with undo/redo.
- **Statistics & Insights** — vault-wide metrics, writing streaks, growth histograms.

### Hardware & Native
- **macOS Vision OCR**, **PDF Annotation**, **Whisper.cpp** audio dictation.

### Capability Platform
- **Process Supervision** — managed subprocess lifecycle with health checks and restart policies.
- **Synchronization** — sync folder/status/conflict models with provider-agnostic scheduling.
- **Diagnostics & Proofing** — diagnostic data model (categories, severities, suggestions) streaming diagnostics to the editor.
- **Live Event Bus → UI Bridge** — backend `EventBus` events delivered to the frontend over a single `nabu-event` channel.
- **Capabilities** — runtime capability registry, enable/disable/list IPC.
- **Plugin Foundation** — plugin manager, manifests, permissions, dependency & feature registries, provider + event contracts.
- **Health & Metrics** — live service health reporting (status, stage, counts) over IPC.
- **Graceful Shutdown** — coordinated teardown persists the index and vault graph.

---

## 🖥️ IPC Commands

| Command                | Args                          | Returns         |
|------------------------|---------------------------------|-----------------|
| `get_settings`         | —                               | `AppSettings`   |
| `settings_get`         | `{ key: String }`               | `serde_json::Value` |
| `settings_set`         | `{ key, value }`                | `Result<(), String>` |
| `settings_set_all`     | `AppSettings` (serialized)      | `Result<(), String>` |
| `open_settings`        | —                               | `Result<(), String>` |
| `toggle_dictation_pill`| —                              | `Result<(), String>` |
| `capture_file_drop`    | `{ filename, mime_type, data }` | `String` (id)     |
| `start_dictation`      | —                               | `Result<(), String>` |
| `statistics_get`       | `{}`                            | `VaultStatistics` |
| `versions_all`         | `{}`                            | `Vec<NoteSummary>` |
| `versions_list`        | `{ path: String }`              | `Vec<VersionMeta>` |
| `versions_get`         | `{ path, id }`                  | `String` (content) |
| `versions_diff`        | `{ path, id_a, id_b }`          | `Vec<DiffRow>` |
| `versions_restore`     | `{ path, id }`                 | `()`                |
| `versions_duplicate`   | `{ path, id, dest }`           | `()`                |
| `snapshot_create`      | `{ path }`                      | `VersionMeta`      |
| `template_list`        | `{}`                            | `Vec<Template>`    |
| `template_save`        | `{ template: Template }`        | `()`                |
| `template_delete`      | `{ name }`                      | `()`                |
| `template_duplicate`   | `{ name }`                      | `Template`         |
| `template_set_favourite`| `{ name, favourite }`         | `()`                |
| `notes_index` / `notes_search` | `{}` / `{ query }` | results / indexed notes |
| `graph_data` / `note_links` | `{}` / `{ path }`       | graph / links       |
| `capability_list` / `enable` / `disable` | —            | `Vec<Capability>` / `Result<(), String>` |
| `health_check`         | `{}`                            | `ServiceHealth`    |
| `inbox_*`              | Various                        | Various                                        |

*(Full surface is registered in `src-tauri/src/lib.rs` — 60+ commands including recovery, history, trash, folders, smart folders, calendar, queue, canvas, and platform integrations.)*

---

## 📜 License

Copyright © 2026 Nabu Labs. Released under the **GNU Affero General Public License v3.0**.

This is free software. See `LICENSE` for details.

## 🆘 Support

- **Documentation:** See `AGENTS.md` for agent guidelines and migration notes.
- **Issues:** [GitHub Issues](https://github.com/Nabu/Nabu/issues)
- **Community:** Join the discussion on [GitHub Discussions](https://github.com/Nabu/Nabu/discussions)

## 🗺️ Roadmap

| Status | Now / Next | Notes |
|--------|-----------|-------|
| ✅ **Current** | Full Dioxus UI, settings panel, dictation pill, inbox, templates, version history, recovery, statistics, graph view, capability platform | Core + capability foundation shipped |
| 🔜 **Next** | Wrapping the Capability Platform roadmap | Finish & publish tagged releases with installers |
| ⏳ **Planned** | Multi-vault tabs, advanced search, custom HTML apps/dashboards | |
| ⏳ **Planned** | Public plugin marketplace | |