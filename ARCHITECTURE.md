# Architecture

Nabu is an Electron desktop app with three layers: **main process** (system), **preload bridge** (secure IPC), and **renderer process** (UI). Data flows in one direction through the pipeline.

```
┌─────────────────────────────────────────────────────────────┐
│                     Renderer Process (React 19)              │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌─────────┐  │
│  │  App.tsx  │   │ NoteView │   │  Blocks  │   │ Graph   │  │
│  │ (state)  │──▶│ (render) │──▶│ (custom) │   │ (d3-f)  │  │
│  └────┬─────┘   └──────────┘   └──────────┘   └─────────┘  │
│       │                                                      │
│  ┌────▼─────┐   ┌──────────┐   ┌──────────┐                │
│  │ FileTree │   │ Sidebar  │   │ Settings  │                │
│  └──────────┘   └──────────┘   └──────────┘                │
└──────────────────────┬──────────────────────────────────────┘
                       │  contextBridge (preload)
┌──────────────────────▼──────────────────────────────────────┐
│                    Main Process (Electron)                    │
│                                                              │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌─────────┐  │
│  │  IPC.ts  │   │ Parser   │   │ State    │   │ Watcher │  │
│  │ handlers │──▶│ (remark) │──▶│ Manager  │──▶│(chokidar│  │
│  └──────────┘   └──────────┘   └──────────┘   └─────────┘  │
│       │                                                      │
│  ┌────▼─────┐   ┌──────────┐   ┌──────────┐                │
│  │ Vector   │   │ Settings │   │ Templates│                │
│  │ Index    │   │ (JSON)   │   │ Engine   │                │
│  └──────────┘   └──────────┘   └──────────┘                │
└──────────────────────────────────────────────────────────────┘
```

## Data Flow

1. **File System → Watcher:** chokidar detects file changes (create, modify, delete) in the vault directory. Changes are debounced and emitted as events.

2. **Watcher → State Manager:** The StateManager receives file events, updates the AST cache (parsed markdown), the full-text index, and the tag index. A `PendingWriteLock` prevents races between file writes and user edits.

3. **State → IPC → Renderer:** State changes are pushed to the renderer via IPC. The renderer's `App.tsx` dispatches actions through a `useReducer` and React Context.

4. **Renderer IPC → Main:** User actions (edit note, toggle task, rename file) go through `contextBridge` → IPC handlers → StateManager → File System.

## Key Modules

### Main Process (`src/main/`)

- **`index.ts`** — App lifecycle, window creation, application menu.
- **`ipc.ts`** — Registers 14+ Zod-validated IPC handlers. Every message between processes is validated against a schema before processing.
- **`parser.ts`** — Markdown → AST pipeline using unified/remark. Parses frontmatter, wiki-links, toggle blocks, and task lists into a structured tree.
- **`state.ts`** — Central state: vault file tree, AST cache (LRU), full-text index, tag index, file hash map. All mutations go through the `PendingWriteLock`.
- **`watcher.ts`** — chokidar-based file watcher with configurable debounce, restart on error, and filtering for `.md` and `.nabu/` cache files.
- **`settings.ts`** — Persists user settings (theme, vault path, preferences) as JSON in Electron's `userData`.
- **`vector.ts`** — ONNX-based vector index for semantic context search. Uses the bundled `bge-micro-v2` model.
- **`templates.ts`** — Template engine that substitutes `{{title}}`, `{{date}}`, `{{time}}` variables in template files.

### Preload (`src/preload/`)

- **`index.ts`** — `contextBridge.exposeInMainWorld` exposing a typed `electronAPI` object. Each method maps to an IPC channel validated by Zod.
- **`index.d.ts`** — Type declarations for the global `electronAPI` used by the renderer.

### Renderer (`src/renderer/`)

- **`App.tsx`** — Root component: manages app state via `useReducer`, wires IPC listeners, renders the layout (sidebar + content area).
- **`components/SetupWizard.tsx`** — First-launch flow for creating or opening a vault.
- **`components/FileTree.tsx`** — Recursive tree of files and folders with context menus for rename, delete, create.
- **`components/NoteView.tsx`** — Renders note content from the parsed AST. Supports inline editing (`Cmd+E`), auto-save (1s debounce), and backlinks panel.
- **`components/blocks/`** — Custom renderers for remark AST node types:
  - `CodeBlock.tsx` — Syntax-highlighted code with copy button
  - `TaskList.tsx` — Interactive checkboxes that persist to disk
  - `ToggleBlock.tsx` — Collapsible toggle sections (`> [!faq]-`)
  - `WikiLink.tsx` — Clickable `[[page-name]]` links with fuzzy navigation
- **`components/GraphView.tsx`** — d3-force directed graph on HTML Canvas. Nodes are notes, edges are wiki-links. Drag, pan, zoom.
- **`components/SettingsPanel.tsx`** — Theme switching (dark/light/system), vault path management.
- **`components/TagsPanel.tsx`** — Tag browsing and filtering from YAML frontmatter.
- **`components/Sidebar.tsx`** — Layout shell: file tree, search, tag panel, settings toggle.
- **`components/ContextPane.tsx`** — Backlinks and context for the active note.
- **`components/ActivityTimeline.tsx`** — Recent file changes and activity log.
- **`components/Versions.tsx`** — Electron and app version display.

### Shared (`src/shared/`)

- **`channels.ts`** — IPC channel enum (string constants for every channel).
- **`schemas.ts`** — Zod v4 schemas for every IPC message. One schema per channel.
- **`types.ts`** — TypeScript interfaces and types used across both processes.
- **`graph.ts`** — Pure function for building a graph from wiki-link relationships. Idempotent, no side effects.
- **`indexing.ts`** — Pure functions for building full-text and tag indexes from parsed notes.

## Security Architecture

### Sandboxed HTML Rendering

User-authored HTML content (embedded in markdown notes) is rendered inside a **sandboxed iframe**. The iframe uses the `sandbox` attribute with minimal permissions:

```html
<iframe sandbox="allow-scripts" src="about:blank"></iframe>
```

- `nodeIntegration` is **disabled** — no access to Node.js APIs.
- `contextIsolation` is **enabled** — no access to Electron internals.
- The iframe communicates with Nabu only via `window.postMessage`.

### IPC Security

- Every IPC message is validated against a **Zod schema** before the handler executes.
- Channel names are string enums — no dynamic channel routing.
- The preload script exposes only a curated set of functions via `contextBridge`. The renderer never has direct access to `ipcRenderer`.

## Tech Decisions

- **Electron** over Tauri for mature file-watching and simpler native module bundling.
- **Tailwind CSS v4** over CSS-in-JS for zero runtime cost and consistent design tokens.
- **Zod v4** over JSON Schema for cleaner TypeScript inference and dual-process validation.
- **useReducer + Context** over Redux for simplicity — the state shape is not complex enough to warrant a store library.
- **d3-force** over a graph library for full control over the physics simulation and rendering.
