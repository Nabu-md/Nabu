# Nabu 🚀

**The lightning-fast, local-first knowledge base built in pure Rust.**

<<<<<<< Updated upstream
Nabu is a Markdown-native, open-source (AGPL-3.0) knowledge base that bridges the gap between clean Markdown text and interactive web software. By leveraging Tauri v2 and Leptos WASM, Nabu delivers a high-performance desktop experience while keeping your data 100% portable, plain-text, and token-efficient.

---

## 🚀 Download & Installation

Nabu is available as a pre-built Universal App for macOS (Intel & Apple Silicon M-Series).

### 1. Download
Grab the latest `Nabu-Universal.dmg` installer from the [Releases Page](../../releases).

### 2. Install
1. Open the downloaded `.dmg` file and drag **Nabu** into your **Applications** folder.
2. Open your **Applications** folder.
3. **Right-Click (or Ctrl + Click)** on `Nabu.app` and choose **Open**.
4. Click **Open** on the security prompt.

> **Note on macOS Unsigned Developer Notice:**  
> Nabu is 100% free, open-source software. To keep Nabu free and independent, we do not pay Apple for a $99/year Developer License. Right-clicking to open Nabu for the first time lets macOS know you trust the app. You only need to do this once!

---

## 🛠️ For Developers (Building from Source)

<details>
<summary>Click here if you want to compile Nabu locally</summary>

### Prerequisites
- Rust (latest stable; MSRV `1.97.1`)
- Node.js ≥ 20 + npm
- Trunk (`cargo install trunk`)
- Tauri CLI v2 (`cargo install tauri-cli --version ^2`)
- Xcode Command Line Tools (`xcode-select --install`)

Run `scripts/check-env.sh` any time to verify your toolchain — it reports
missing tools, targets, and npm dependencies with exact install commands.

### Dev Mode (hot-reload)
```bash
git clone https://github.com/Nabu-md/Nabu.git
cd Nabu
npm install
cargo tauri dev
```

### Release Build
```bash
scripts/build.sh               # native release for this machine
scripts/build.sh --universal   # macOS universal DMG (Intel + Apple Silicon)
```

The script detects your platform/architecture, validates prerequisites, and
prints the produced artifacts. Full instructions, prerequisites, expected
outputs, and troubleshooting live in **[docs/BUILDING.md](docs/BUILDING.md)**.

</details>
=======
Nabu is a local-first, open-source (AGPL-3.0) desktop application that bridges the gap between clean Markdown text and rich web software. While your knowledge base stays 100% portable, plain text, and token-efficient (`.md`) on your hard drive, Nabu renders your notes into a powerful, interactive canvas.
>>>>>>> Stashed changes

---

## 💡 The Core Differentiator: HTML-Native App Blocks

<<<<<<< Updated upstream
Nabu treats Markdown as a launchpad for **lightweight, interactive software**. Because Nabu renders via a secure, sandboxed Leptos WASM layer, you can seamlessly embed full HTML, CSS, and JavaScript directly inside your plain-text notes.

### What this unlocks for your workflow:
- **AI-Generated Cockpits:** Instantly run interactive dashboards, simulators, or testing scripts right in your notes.
- **Modular Custom Tools:** Turn a standard note into a personalized Kanban board, tracker, or visualizer using standard web technologies.
- **Zero "Token Tax":** Since Nabu stores standard Markdown on disk, local AI agents and LLMs read your files with 1.0x token efficiency — no bloated proprietary widget code.
=======
Traditional markdown editors treat text as static documents. Nabu treats Markdown as a launchpad for **lightweight, interactive software**. Because Nabu renders directly to a secure, sandboxed DOM layer, you can seamlessly embed full HTML, CSS, and JavaScript right inside your plain-text notes.

### What this unlocks for your workflow:

- **AI-Generated Cockpits:** Ask Claude Code to build a custom project dashboard, a real-time system architecture simulator, or an API testing script. It drops the HTML code into your vault, and Nabu runs it instantly as a live app block.
- **Modular Custom Tools:** Turn a standard note into a personalized Kanban board, a live freelance invoice tracker, or a local git commit visualizer — no complex plugin SDK or proprietary widget formats required. If it runs in a browser, it runs in your note.
- **Zero "Token Tax":** Because the structure is standard Markdown on disk, local AI agents and LLMs read your files with 1.0x token efficiency. They waste zero tokens reading bloated presentation code, while you enjoy a rich graphical interface.
>>>>>>> Stashed changes

---

## 🔥 Features

<<<<<<< Updated upstream
### Foundation & File System
1. **Setup Wizard:** Secure vault directory initialization and path validation.
2. **Vault Dialogs:** Create or open a vault through native OS folder pickers.
3. **Capture Handlers:** Clipboard, screenshot, file-drop, folder-watch, and browser/Safari reader ingestion.
4. **Template Management:** Create notes from vault templates (Meeting Note, Bug Report, Project Brief).
5. **FileTree Navigation:** Recursive, reactive Leptos-based file navigation.

### Editor & Knowledge Base
6. **Note Editor:** Live markdown preview editor with interactive task checkbox support.
7. **Tag Parsing:** Real-time tag extraction and indexing.
8. **Full-Text Search:** In-memory `Indexer` over processed KnowledgeObjects.
9. **Relationship Graph:** `VaultGraph` adjacency model with Canvas visualization.
10. **Backlink Resolution:** Graph traversal for interconnected note discovery.
11. **Dynamic Theme Engine:** Reactive dark/light mode switching persisted to disk.

### Graph & Sandbox
12. **Graph View:** Canvas-based relationship visualization (no external graph crate).
13. **Tag/Blocks Graph View:** Filtered graph traversal modes.
14. **App Block Sandbox:** Secure `iframe` isolation for interactive widgets.

### Hardware Superpowers (Phase 2)
15. **macOS Native Features:** Fully functional backend FFI for macOS Vision OCR, PDF Annotation, and Whisper.cpp-based Audio Dictation.
=======
- **100% Data Ownership:** Your files live in local folders as raw `.md`. Open them in VS Code, version them with Git, or process them with grep. Zero walled gardens.
- **External Change Detection:** Edit a file via the command line or an external IDE. Nabu's native file-watcher hot-reloads the visual canvas instantly.
- **Developer Essentials Ecosystem:** Out-of-the-box D3 graph views for `[[wiki-links]]`, full-text fuzzy search, frontmatter tag filtering, backlinks, and dark mode.
>>>>>>> Stashed changes

---

## 🛠 Architecture

<<<<<<< Updated upstream
```text
crates/
├── nabu-core/       # Core Engine: Vault logic, AST, Indexing, Graph, FFI
├── nabu-ui/         # Leptos WASM UI components
src-tauri/           # Tauri v2 Desktop Shell & IPC Commands
```

## License
Copyright © 2024 Nabu Labs. Released under the **GNU Affero General Public License v3.0**.
=======
Nabu's desktop client core is entirely open-source. Developers can fully audit how their private data, repository structures, and system prompts are handled.

---

## 🚀 Getting Started

1. [Download the latest DMG from nabu.app](https://nabu.app).
2. Run the Setup Wizard to point Nabu to an existing Markdown folder or your project repository.
3. Drop an interactive HTML snippet into any note and watch your workspace come alive.

### From source

```bash
# Prerequisites: Node.js 20+, npm 9+
git clone https://github.com/Nabu/Nabu.git
cd nabu
npm install
npm run dev
```

## Usage

1. **Launch Nabu.** The setup wizard appears.
2. **Create a new vault** (a folder on your machine) or **open an existing one** (any folder with `.md` files).
3. **Navigate** your file tree in the sidebar. Create folders and notes with the `+` buttons.
4. **Search** with the filter bar or `Cmd+Shift+F` to focus it.
5. **Toggle graph view** — see `[[wiki-link]]` relationships between notes.
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

Nabu is distributed as a **universal DMG** (Intel + Apple Silicon) on the [releases page](https://github.com/Nabu/Nabu/releases).

Since Nabu is fully open-source and community-funded, the DMG is **not Apple code-signed**. On first launch:

1. macOS will display a warning: *"Nabu cannot be opened because the developer cannot be verified"*
2. **Right-click** the app in Finder → **Open** → **Open Anyway**
3. You'll only need to do this once

### Building your own DMG

```bash
# Prerequisites: Node.js 20+
npm install
npm run build:mac

# Output: dist/Nabu-1.0.0.dmg
```

The DMG is unsigned by design — no Apple Developer account required.

## Vault Compatibility

Nabu works with any folder of `.md` files, including existing Obsidian vaults:
- `[[Wiki-links]]` are resolved case-insensitively by filename
- YAML frontmatter `tags:` field is parsed (both inline `[a, b]` and block list formats)
- Toggle blocks (`> [!faq]-`) and task lists (`- [ ]`) are rendered as interactive elements
- Task checkbox toggling persists to disk
- `.nabu/` cache directory is auto-generated and git-ignored

## License

Copyright © 2024 Nabu Labs. Released under the **GNU Affero General Public License v3.0**.

This is free software. See `LICENSE` for details.

**Why AGPL?** Nabu is built for the community. The AGPL ensures that any modified version offered as a network service must also release its source code — protecting the open ecosystem while still being permissive enough for personal and commercial use.

## Paid Add-ons

- **Nabu Cloud Sync** — E2E encrypted sync across devices. Available at [nabu.app](https://nabu.app).
- **Nabu Cloud Teams** — Shared vaults with team-wide AI context.

Sync and Teams are separate paid services that fund ongoing development. The open-source app works fully without them.

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
>>>>>>> Stashed changes
