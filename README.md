# Nemo

**A markdown-native knowledge base for developers — on disk, plain `.md`, zero lock-in.**

Nemo is an open-source (AGPL-3.0) desktop app for developers who want their notes in their own control. It renders your local markdown files as a rich, interactive knowledge base with full-text search, tag filtering, a graph view of wiki-link relationships, and an HTML-native viewer that's extensible beyond what markdown alone can do.

## Why Nemo?

- **Plain `.md` on disk.** Your notes are just markdown files in a folder. Open them in any editor, version with git, process with any tool. Zero lock-in.
- **HTML-native rendering.** Since Nemo renders markdown as HTML, you can build dashboards, scrapers, or any HTML-based tool for your vault — all while keeping `.md` files that stay token-efficient for LLM use.
- **Made for developers.** Designed for the Claude Code / developer workflow. Wiki-links, tags, full-text search, graph view — everything you expect from a knowledge base, none of the proprietary format.

## Features

| Feature | Description |
|---|---|
| **Setup wizard** | First-launch flow to create or open a vault |
| **File tree** | Recursive tree with folder/note creation, rename, delete from context menu |
| **Inline editing** | `Cmd+E` to edit, 1-second auto-save debounce, `Cmd+S` to save |
| **Graph view** | d3-force directed graph of [[wiki-link]] relationships with drag, pan, zoom |
| **Full-text search** | Every word indexed. Search across your vault, results sorted by relevance |
| **Tag filtering** | YAML frontmatter `tags:` → tag panel → filter file tree |
| **Backlinks** | Every note shows which other notes link to it with a snippet |
| **Templates** | Create notes from templates stored in `_templates/` (Meeting Note, Bug Report, Project Brief) |
| **Themes** | Dark, light, system — follows your macOS preference |
| **Export** | Export any note as HTML or print to PDF |
| **External edit detection** | Edit a note in another app, Nemo detects the change and re-parses automatically |

## Quick Start

### Download

Download the latest DMG from the [releases page](https://github.com/Nemo-notes/Nemo/releases).

> **Note:** Nemo is fully open-source and community-funded, so the DMG is not Apple code-signed. If macOS blocks the initial launch, right-click the app icon in Finder, select **Open**, and click **Open Anyway**.

### Via Homebrew (recommended for developers)

```bash
brew install --cask nemo
```

### From source

```bash
# Prerequisites: Node.js 20+, npm 9+
git clone https://github.com/Nemo-notes/Nemo.git
cd nemo
npm install
npm run dev
```

## Usage

1. **Launch Nemo.** The setup wizard appears.
2. **Create a new vault** (a folder on your machine) or **open an existing one** (any folder with `.md` files).
3. **Navigate** your file tree in the sidebar. Create folders and notes with the `+` buttons.
4. **Search** with the filter bar or `Cmd+Shift+F` to focus it.
5. **Toggle graph view** — see [[wiki-link]] relationships between notes.
6. **Edit a note** with `Cmd+E`, save with `Cmd+S`.
7. **Tag notes** by adding `tags: [tag1, tag2]` to YAML frontmatter.
8. **Link notes** with `[[Page Name]]` wiki-link syntax.

## Architecture

```
src/
├── main/          # Electron main process
│   ├── index.ts   # App lifecycle, window, menu
│   ├── ipc.ts     # IPC handler registration (14+ Zod-validated channels)
│   ├── state.ts   # StateManager: AST cache, indexes, PendingWriteLock
│   ├── parser.ts  # Markdown → AST pipeline (remark/unified)
│   ├── watcher.ts # chokidar vault watcher with debounce + restart
│   ├── vector.ts  # Vector index for semantic context search
│   ├── settings.ts# Settings persistence (JSON in userData)
│   └── templates.ts # Template variable substitution ({{title}}, {{date}}, {{time}})
├── preload/       # Context bridge (contextBridge.exposeInMainWorld)
│   ├── index.ts   # Typed electron API
│   └── index.d.ts # Global type declarations
├── renderer/      # React 19 UI (Vite + Tailwind v4)
│   └── src/
│       ├── App.tsx          # Root component, state, IPC wiring
│       ├── components/      # All UI components
│       │   ├── SetupWizard  # First-launch vault picker
│       │   ├── FileTree     # Recursive tree with context menus
│       │   ├── NoteView     # Markdown rendering + edit mode + backlinks
│       │   ├── GraphView    # d3-force canvas graph
│       │   ├── SettingsPanel# Theme, vault management
│       │   ├── TagsPanel    # Tag browser + filter
│       │   └── blocks/      # Custom remark AST renderers
│       └── assets/          # CSS with theme vars + Tailwind @theme
└── shared/        # Shared between main and renderer
    ├── channels.ts # IPC channel enum
    ├── schemas.ts  # Zod v4 schemas for all IPC messages
    ├── types.ts    # TypeScript interfaces
    ├── graph.ts    # Pure graph-building from wiki-links
    └── indexing.ts # Pure full-text + tag index builders
```

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop framework | Electron 39 |
| UI | React 19 |
| Styling | Tailwind CSS v4 |
| Markdown parsing | unified / remark / mdast |
| Graph visualization | d3-force on HTML Canvas |
| IPC validation | Zod v4 (bidirectional schemas) |
| State management | useReducer + React Context |
| File watching | chokidar (fsevents on macOS) |
| Testing | Vitest + fast-check (property-based) |
| Build | electron-vite + electron-builder |

## Downloadable Mac App

Nemo is distributed as a **universal DMG** (Intel + Apple Silicon) on the [releases page](https://github.com/Nemo-notes/Nemo/releases).

Since Nemo is fully open-source and community-funded, the DMG is **not Apple code-signed**. On first launch:

1. macOS will display a warning: *"Nemo cannot be opened because the developer cannot be verified"*
2. **Right-click** the app in Finder → **Open** → **Open Anyway**
3. You'll only need to do this once

### Building your own DMG

```bash
# Prerequisites: Node.js 20+
npm install
npm run build:mac

# Output: dist/Nemo-1.0.0.dmg
```

The DMG is unsigned by design — no Apple Developer account required.

## Vault Compatibility

Nemo works with any folder of `.md` files, including existing Obsidian vaults:
- `[[Wiki-links]]` are resolved case-insensitively by filename
- YAML frontmatter `tags:` field is parsed (both inline `[a, b]` and block list formats)
- Toggle blocks (`> [!faq]-`) and task lists (`- [ ]`) are rendered as interactive elements
- Task checkbox toggling persists to disk
- `.nemo/` cache directory is auto-generated and git-ignored

## License

Copyright © 2024 Nemo Labs. Released under the **GNU Affero General Public License v3.0**.

This is free software. See `LICENSE` for details.

**Why AGPL?** Nemo is built for the community. The AGPL ensures that any modified version offered as a network service must also release its source code — protecting the open ecosystem while still being permissive enough for personal and commercial use.

## Paid Add-ons

- **Nemo Sync** — End-to-end encrypted sync across devices. Available at [nemo.app](https://nemo.app).

Sync is a separate paid service that funds ongoing development. The open-source app works fully without it.

## Development

```bash
npm install         # Install dependencies
npm run dev         # Development (hot-reload)
npm run typecheck   # TypeScript check
npm run test        # Run tests (278+ passing)
npm run lint        # Lint
npm run build:mac   # Build universal DMG
npm run build:linux # Build Linux package
```

### Property-Based Tests

The test suite uses [fast-check](https://github.com/dubzzz/fast-check) for property-based testing on:
- Graph building invariants (idempotence, subset properties)
- Full-text index properties (frontmatter exclusion, case insensitivity)
- Tag index properties (presence, uniqueness)
- Template substitution (idempotence)

## Roadmap

| Version | Features |
|---|---|
| v1 (current) | Setup wizard, file tree, note editing, graph view, tags, search, themes, templates, export |
| v2 | Multi-vault tabs, advanced search, custom HTML apps/dashboards |
| v3 | Plugin API, community marketplace |
