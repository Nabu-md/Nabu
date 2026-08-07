# Nabu 🚀

**The lightning-fast, local-first knowledge base built in pure Rust.**

Nabu is a Markdown-native, open-source (AGPL-3.0) desktop knowledge base that bridges the gap between clean Markdown text and interactive web software. By leveraging Tauri v2 and Dioxus 0.6.3 (WASM), Nabu delivers a high-performance desktop experience while keeping your data 100% portable, plain-text, and token-efficient.

---

## 📦 Architecture

```
crates/
├── nabu-core/       # Core engine: Vault logic, AST, Indexing, Graph, FFI
├── nabu-ui/         # Dioxus 0.6.3 WASM UI components (cdylib)
src-tauri/           # Tauri v2 desktop shell & IPC commands
```

- **`nabu-core`** — Rust core with models, processing pipeline, storage, indexer, and graph engine.
- **`nabu-ui`** — Standalone Dioxus 0.6.3 CSR frontend compiled to WASM. Lives in its own Cargo workspace.
- **`src-tauri`** — Tauri backend providing IPC commands for settings, file capture, dictation, statistics, version history, templates, and inbox processing.

## 🔧 Building from Source

### Prerequisites
- Rust (latest stable)
- Node.js ≥ 20 + npm (for WASM tooling)
- Tauri CLI v2 (`cargo install tauri-cli --version ^2`)
- Xcode Command Line Tools (`xcode-select --install`)

### Compile Check

```bash
cd "crates/nabu-ui"
cargo check
```

This checks the Dioxus frontend in isolation. The root workspace (`cargo check` from project root) compiles `nabu-core` + `src-tauri`.

### Dev Mode

```bash
npm install
cargo tauri dev
```

### Release Build

```bash
cargo tauri build
```

## 🚀 Features

- **Vault Management:** Setup wizard, native OS folder pickers, vault creation/opening.
- **Capture Handlers:** Clipboard, screenshot, file-drop, folder-watch, and browser/Safari reader ingestion.
- **Template Management:** Create notes from vault templates with variable substitution (`{{title}}`, `{{date}}`, `{{tags}}`).
- **FileTree Navigation:** Recursive, reactive file tree with context menus.
- **Note Editor:** Live markdown preview editor with interactive task checkbox support.
- **Tag Parsing:** Real-time tag extraction and indexing.
- **Full-Text Search:** In-memory indexer over processed KnowledgeObjects.
- **Relationship Graph:** `VaultGraph` adjacency model with Canvas visualization.
- **Backlink Resolution:** Graph traversal for interconnected note discovery.
- **Dynamic Theme Engine:** Reactive dark/light mode switching persisted to settings.
- **Dictation Pill:** Floating scratchpad / dictation / file-drop zone with clipboard cache.
- **Statistics & Insights:** Vault-wide metrics, writing streaks, growth histograms.
- **Version History:** Snapshot browsing, diff view, restore, and duplicate with undo.
- **Hardware Superpowers:** macOS Vision OCR, PDF Annotation, and Whisper.cpp-based audio dictation (Phase 2).

## 🖥️ IPC Commands

| Command                | Args                          | Returns         |
|------------------------|---------------------------------|-----------------|
| `get_settings`         | —                               | `AppSettings`   |
| `settings_get`         | `{ key: String }`               | `serde_json::Value` |
| `settings_set`         | `{ key, value }`                | `Result<(), String>` |
| `settings_set_all`     | `AppSettings` (serialized)      | `Result<(), String>` |
| `open_settings`        | —                               | `Result<(), String>` |
| `toggle_dictation_pill`| —                              | `Result<(), String>` |
| `capture_file_drop`    | `{ filename, mime_type, data }` | `String` (id)   |
| `start_dictation`      | —                               | `Result<(), String>` |
| `statistics_get`       | `{}`                            | `VaultStatistics` |
| `versions_all`         | `{}`                            | `Vec<NoteSummary>` |
| `versions_list`        | `{ path: String }`              | `Vec<VersionMeta>` |
| `versions_get`         | `{ path, id }`                  | `String` (content) |
| `versions_diff`        | `{ path, id_a, id_b }`          | `Vec<DiffRow>` |
| `versions_restore`     | `{ path, id }`                  | `()` |
| `versions_duplicate`   | `{ path, id, dest }`            | `()` |
| `snapshot_create`      | `{ path }`                      | `VersionMeta` |
| `template_list`        | `{}`                            | `Vec<Template>` |
| `template_save`        | `{ template: Template }`        | `()` |
| `template_delete`      | `{ name }`                      | `()` |
| `template_duplicate`   | `{ name }`                      | `Template` |
| `template_set_favourite`| `{ name, favourite }`          | `()` |
| `inbox_*`              | Various                         | Various |

## 📜 License

Copyright © 2024 Nabu Labs. Released under the **GNU Affero General Public License v3.0**.

This is free software. See `LICENSE` for details.

## 🆘 Support

- **Documentation:** See `AGENTS.md` for agent guidelines and migration notes.
- **Issues:** [GitHub Issues](https://github.com/Nabu/Nabu/issues)
- **Community:** Join the discussion on [GitHub Discussions](https://github.com/Nabu/Nabu/discussions)

## Roadmap

| Version | Features |
|---------|---------|
| v1 (current) | Full Dioxus UI, settings panel (15 tabs), dictation pill, inbox, templates, version history, statistics, graph view |
| v2 | Multi-vault tabs, advanced search, custom HTML apps/dashboards |
| v3 | Plugin API, community marketplace |
