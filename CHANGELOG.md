# Changelog

All notable changes to Nemo are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] — 2026-07-04

### Added

- **Setup wizard** — first-launch flow to create or open a vault
- **File tree** — recursive tree with folder/note creation, rename, delete via context menu
- **Inline editing** — `Cmd+E` to edit, 1-second auto-save debounce, `Cmd+S` to save
- **Graph view** — d3-force directed graph of `[[wiki-link]]` relationships with drag, pan, zoom
- **Full-text search** — every word indexed, results sorted by relevance
- **Tag filtering** — YAML frontmatter `tags:` → tag panel → filter file tree
- **Backlinks** — every note shows which other notes link to it with a snippet
- **Templates** — create notes from templates stored in `_templates/` (Meeting Note, Bug Report, Project Brief)
- **Themes** — dark, light, system — follows macOS preference
- **Export** — export any note as HTML or print to PDF
- **External edit detection** — chokidar watches for external file changes, hot-reloads the visual canvas
- **HTML-native app blocks** — embed raw HTML, CSS, and JavaScript inside notes via sandboxed iframe
- **Custom remark plugins** — wiki-links, toggle blocks, task lists all parsed from standard markdown
- **Vector index** — ONNX-based semantic search using bundled bge-micro-v2 model
- **Property-based tests** — 278+ tests including fast-check invariants for graph, indexing, and templates
- **Unsigned DMG distribution** — universal binary for Intel + Apple Silicon, no Apple Developer account required

### Architecture

- Three-tier Electron architecture: main process ↔ preload bridge ↔ React renderer
- Zod v4 schemas for bidirectional IPC validation (14+ channels)
- CRDT-ready sync foundation in the private nemo-cloud monorepo
