# Nabu Renderer Architecture Documentation

## 1. Renderer Overview

### Architecture

The Nabu renderer is a React-based single-page application that runs in the Electron renderer process. It follows a **feature-oriented organization** with clear **ownership boundaries** between components, commands, and state.

### Design Principles

1. **Feature-Oriented Organization**: Code is organized by feature domain rather than technical layer. Each feature owns its components, hooks, and commands.

2. **Single Source of Truth**: The `shared/store.ts` module is the exclusive owner of application state. All state mutations flow through the `appReducer`.

3. **Thin UI Architecture** (Architecture Goal 9): Components are presentation-only. Business logic and IPC orchestration live in command modules.

4. **Unidirectional Data Flow**: User actions → Commands → IPC → State updates → Component re-renders.

5. **Explicit Ownership**: Each module, component, and hook has a clearly defined owner responsible for its data and behavior.

### Ownership Model

The renderer uses a hierarchical ownership model:

```
App (root composition)
├── shared/store.ts (state owner)
├── shared/ipc.ts (IPC boundary)
├── shared/commands/ (command registry)
├── shared/components/ (shared UI)
└── features/
    ├── vault/ (vault management)
    ├── notes/ (note viewing/editing)
    ├── graph/ (knowledge graph)
    ├── search/ (search UI)
    ├── settings/ (app settings)
    └── widgets/ (activity timeline, dictation)
```

---

## 2. Folder Structure

```
src/renderer/src/

├── App.tsx                    # Root component, IPC wiring, layout
├── main.tsx                   # React entry point
├── index.html                 # HTML template
├── assets/                    # Static assets (CSS, SVG)
│   ├── base.css
│   ├── main.css
│   └── *.svg
│
├── features/                  # Feature modules (ownership boundaries)
│   ├── vault/                 # Vault management feature
│   │   ├── Sidebar.tsx        # Icon ribbon + panel container
│   │   ├── FileTree.tsx       # File tree with context menu
│   │   ├── TagsPanel.tsx      # Tag hierarchy view
│   │   ├── FavoritesPanel.tsx # Favorited notes list
│   │   ├── SetupWizard.tsx    # Vault creation/opening flow
│   │   ├── PaneLayout.tsx     # Layout configuration
│   │   └── vaultCommands.ts   # Vault workflow orchestration
│   │
│   ├── notes/                 # Note viewing/editing feature
│   │   ├── NoteView.tsx       # Main note renderer
│   │   ├── MarkdownEditor.tsx # CodeMirror editor wrapper
│   │   ├── ContextPane.tsx    # Related notes sidebar
│   │   ├── FindReplaceBar.tsx # Editor find/replace UI
│   │   ├── noteCommands.ts    # Note workflow orchestration
│   │   └── blocks/            # Note block components
│   │       ├── CodeBlock.tsx
│   │       ├── MermaidBlock.tsx
│   │       ├── EmbedBlock.tsx
│   │       ├── WikiLink.tsx
│   │       ├── ToggleBlock.tsx
│   │       ├── TaskList.tsx
│   │       ├── PropertiesView.tsx
│   │       ├── SlashCommands.tsx
│   │       ├── InlineTagChip.tsx
│   │       ├── OCRTextPanel.tsx
│   │       └── PagePreview.tsx
│   │
│   ├── graph/                 # Knowledge graph feature
│   │   ├── GraphView.tsx      # Main graph visualization
│   │   └── CytoscapeGraphView.tsx
│   │
│   ├── pdf/                   # PDF viewer feature
│   │   ├── PdfViewer.tsx      # PDF rendering component
│   │   └── pdfCommands.ts     # PDF workflow orchestration
│   │
│   ├── search/                # Search UI feature
│   │   ├── SearchPanel.tsx    # Advanced search panel
│   │   ├── QuickSwitcher.tsx  # Cmd+O note navigation
│   │   ├── CommandPalette.tsx # Cmd+P command palette
│   │   └── fuzzy.ts           # Shared fuzzy matching utility
│   │
│   ├── settings/              # Settings feature
│   │   └── SettingsPanel.tsx  # App settings modal
│   │
│   └── widgets/               # Widget feature
│       ├── ActivityTimeline.tsx
│       ├── DictationWidget.tsx
│       └── widgetService.ts   # Widget state owner
│
├── shared/                    # Shared infrastructure
│   ├── store.ts               # App state (AppContext, appReducer)
│   ├── ipc.ts                 # Typed IPC wrapper
│   ├── commands/              # Command registry
│   │   ├── registry.ts        # registerCommand, getCommands
│   │   └── feature-registrations.ts # Feature toggle commands
│   └── components/            # Shared UI components
│       ├── FavoriteToggle.tsx
│       ├── OutlinePanel.tsx
│       ├── SandboxedHtml.tsx
│       └── icons.tsx
│
└── _scratch.ts                # Development scratch file
```

### Directory Purpose

| Directory | Purpose |
|-----------|---------|
| `features/` | Feature modules with clear ownership boundaries. Each feature owns its components, commands, and state interactions. |
| `shared/store.ts` | Single source of truth for application state. Contains `AppState`, `AppAction`, `appReducer`, and `AppContext`. |
| `shared/ipc.ts` | The ONLY boundary between renderer and preload bridge. Normalizes weak contract types. |
| `shared/commands/` | Command registry for the Command Palette. Features register commands here. |
| `shared/components/` | UI components shared across multiple features. Not owned by any single feature. |

---

## 3. Feature Ownership

### Vault Feature (`features/vault/`)

| Item | Owner |
|------|-------|
| **Components** | `Sidebar.tsx`, `FileTree.tsx`, `TagsPanel.tsx`, `FavoritesPanel.tsx`, `SetupWizard.tsx`, `PaneLayout.tsx` |
| **Commands** | `vaultCommands.ts` |
| **State** | Reads: `state.vault`, `state.tagIndex`, `state.selectedTags`, `state.openTabs` |
| **Dependencies** | `shared/store.ts`, `shared/ipc.ts`, `shared/components/icons.tsx` |

**Owned Components:**
- `Sidebar` - Icon ribbon with expandable panels
- `FileTree` - File tree with search, context menu, create/rename/delete
- `TagsPanel` - Hierarchical tag view with filtering
- `FavoritesPanel` - Favorited notes list
- `SetupWizard` - Vault creation/opening flow
- `PaneLayout` - Layout configuration

**Owned Commands:**
- `renameFile()` - Rename a file with path computation
- `deleteFile()` - Delete a file via IPC
- `createFolder()` - Create folder with validation
- `createNote()` - Create note from template
- `openTreeFile()` - Open file from tree

### Notes Feature (`features/notes/`)

| Item | Owner |
|------|-------|
| **Components** | `NoteView.tsx`, `MarkdownEditor.tsx`, `ContextPane.tsx`, `FindReplaceBar.tsx` |
| **Block Components** | `CodeBlock.tsx`, `MermaidBlock.tsx`, `EmbedBlock.tsx`, `WikiLink.tsx`, `ToggleBlock.tsx`, `TaskList.tsx`, `PropertiesView.tsx`, `SlashCommands.tsx`, `InlineTagChip.tsx`, `OCRTextPanel.tsx`, `PagePreview.tsx` |
| **Commands** | `noteCommands.ts` |
| **State** | Reads/writes: `state.currentFile`, `state.currentAST`, `state.editMode`, `state.toggleStates`, `state.contextPaneOpen`, `state.contextResults` |
| **Dependencies** | `shared/store.ts`, `shared/ipc.ts`, `shared/components/SandboxedHtml.tsx` |

**Owned Components:**
- `NoteView` - Main note renderer with toolbar, edit mode, live preview
- `MarkdownEditor` - CodeMirror 6 wrapper
- `ContextPane` - Related notes sidebar
- `FindReplaceBar` - Editor find/replace UI

**Owned Block Components:**
- `CodeBlock` - Syntax-highlighted code
- `MermaidBlock` - Mermaid diagram rendering
- `EmbedBlock` - `![[target]]` transclusion
- `WikiLink` - `[[target]]` navigation
- `ToggleBlock` - Collapsible toggle sections
- `TaskList` - Interactive task checkboxes
- `PropertiesView` - YAML frontmatter editor
- `SlashCommands` - Inline autocomplete menu
- `InlineTagChip` - Clickable inline tags
- `OCRTextPanel` - Extracted text from images
- `PagePreview` - Hover preview popover

**Owned Commands:**
- `loadNoteFile()` - Load AST via IPC
- `saveNote()` - Persist note content
- `enterEditMode()` - Enter edit mode with raw content
- `exitEditMode()` - Exit edit mode, reload AST
- `exitLivePreviewMode()` - Save and exit live preview
- `navigateToNote()` - Navigate to file or PDF
- `writeProperties()` - Write YAML frontmatter
- `persistHeadingFold()` - Persist heading fold state
- `exportNoteHtml()` - Export to HTML
- `retryLoadNote()` - Force reload note

### Graph Feature (`features/graph/`)

| Item | Owner |
|------|-------|
| **Components** | `GraphView.tsx`, `CytoscapeGraphView.tsx` |
| **Commands** | None (state-driven) |
| **State** | Reads: `state.graphEdges`, `state.extendedIndex`, `state.graphMode`, `state.vault` |
| **Dependencies** | `shared/store.ts`, `@shared/graph-utils` |

**Owned Components:**
- `GraphView` - D3-based graph visualization (files/tags/blocks modes)
- `CytoscapeGraphView` - Alternative graph view

### PDF Feature (`features/pdf/`)

| Item | Owner |
|------|-------|
| **Components** | `PdfViewer.tsx` |
| **Commands** | `pdfCommands.ts` |
| **State** | Reads: `state.vault` |
| **Dependencies** | `shared/store.ts`, `shared/ipc.ts` |

**Owned Components:**
- `PdfViewer` - PDF rendering with lazy page loading, annotations

**Owned Commands:**
- `createNoteFromAnnotation()` - Create backlinked note from annotation

### Search Feature (`features/search/`)

| Item | Owner |
|------|-------|
| **Components** | `SearchPanel.tsx`, `QuickSwitcher.tsx`, `CommandPalette.tsx` |
| **Utilities** | `fuzzy.ts` |
| **State** | Reads: `state.vault`, `state.extendedIndex`, `state.recentNotes` |
| **Dependencies** | `shared/store.ts`, `shared/commands/registry.ts` |

**Owned Components:**
- `SearchPanel` - Advanced search with operators
- `QuickSwitcher` - Cmd+O fuzzy note navigation
- `CommandPalette` - Cmd+P command palette

**Owned Utilities:**
- `fuzzySearch()` - Fuzzy matching algorithm
- `matchScore()` - Field-level scoring

### Settings Feature (`features/settings/`)

| Item | Owner |
|------|-------|
| **Components** | `SettingsPanel.tsx` |
| **Commands** | None (direct IPC) |
| **State** | Reads/writes: `state.theme`, `state.settingsPanelOpen` |
| **Dependencies** | `shared/store.ts`, `shared/ipc.ts` |

**Owned Components:**
- `SettingsPanel` - Vault, theme, feature toggles, dictation settings

### Widgets Feature (`features/widgets/`)

| Item | Owner |
|------|-------|
| **Components** | `ActivityTimeline.tsx`, `DictationWidget.tsx` |
| **Services** | `widgetService.ts` |
| **State** | Widget state is owned by `widgetService.ts` (not AppState) |
| **Dependencies** | `shared/ipc.ts` |

**Owned Components:**
- `ActivityTimeline` - Recent file changes
- `DictationWidget` - Audio dictation widget

**Owned Services:**
- `widgetService.ts` - Widget state owner (activity log, dictation state)

---

## 4. Component Tree

### Root Component Hierarchy

```
App.tsx
├── ErrorBoundary
│   └── (conditional rendering)
│       ├── SetupWizard (when showSetup=true)
│       └── main layout
│           ├── Sidebar
│           │   ├── FileTree
│           │   ├── TagsPanel
│           │   ├── OutlinePanel (shared)
│           │   └── FavoritesPanel
│           │
│           ├── main.note-container
│           │   ├── note-toolbar
│           │   │   ├── NoteIcon
│           │   │   ├── GraphIcon
│           │   │   └── EditIcon / EyeIcon
│           │   │
│           │   └── (conditional content)
│           │       ├── PdfViewer (when pdfViewOpen)
│           │       ├── GraphView (when graphViewOpen)
│           │       └── NoteView (default)
│           │           ├── MarkdownEditor (when editMode)
│           │           │   └── FindReplaceBar (optional)
│           │           └── renderNode (AST rendering)
│           │               ├── ToggleBlock
│           │               ├── TaskList
│           │               ├── WikiLink
│           │               ├── CodeBlock
│           │               ├── MermaidBlock
│           │               ├── EmbedBlock
│           │               ├── Callout
│           │               ├── PropertiesView
│           │               ├── KanbanBlock
│           │               ├── InlineTagChip
│           │               └── PagePreview
│           │
│           ├── ContextPane
│           │
│           ├── ActivityTimeline
│           │
│           ├── SearchPanel (when searchPanelOpen)
│           ├── QuickSwitcher (when quickSwitcherOpen)
│           └── CommandPalette (when commandPaletteOpen)
```

### Feature Entry Points

| Feature | Entry Point | Rendered When |
|---------|-------------|--------------|
| Vault | `Sidebar` | Always (left sidebar) |
| Notes | `NoteView` | `graphViewOpen=false, pdfViewOpen=false` |
| Graph | `GraphView` | `graphViewOpen=true` |
| PDF | `PdfViewer` | `pdfViewOpen=true` |
| Search | `SearchPanel` | `searchPanelOpen=true` |
| Quick Switcher | `QuickSwitcher` | `quickSwitcherOpen=true` |
| Command Palette | `CommandPalette` | `commandPaletteOpen=true` |
| Settings | `SettingsPanel` | `settingsPanelOpen=true` |
| Setup | `SetupWizard` | `showSetup=true` |

---

## 5. State Flow

### State Architecture

The renderer uses a **single store pattern** with React Context:

```
┌─────────────────────────────────────────────────────────────┐
│                     AppContext (React Context)               │
│                                                              │
│  ┌─────────────────────────────────────────────────────┐  │
│  │                     AppState                           │  │
│  │  - Vault state (openVaults, activeVaultId, vault)      │  │
│  │  - Tab state (openTabs, activeTabId, paneLayout)       │  │
│  │  - Derived state (currentFile, currentAST, editMode)   │  │
│  │  - UI state (graphViewOpen, searchPanelOpen, etc.)     │  │
│  │  - Index state (graphEdges, tagIndex, fullTextIndex)   │  │
│  └─────────────────────────────────────────────────────┘  │
│                       ▲                                      │
│                       │ dispatch                             │
│                       │                                      │
│  ┌─────────────────────────────────────────────────────┐  │
│  │                     appReducer                         │  │
│  │  - Pure function, only writer of AppState             │  │
│  │  - Handles 30+ action types                           │  │
│  │  - syncActiveAliases() derives currentFile/currentAST   │  │
│  └─────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### Canonical State Flow

```
User Action
    │
    ▼
Command (from registry or feature command module)
    │
    ▼
IPC Call (via shared/ipc.ts)
    │
    ▼
Main Process Handler
    │
    ▼
State Owner (appReducer or widgetService)
    │
    ▼
Subscribers (useAppContext hook)
    │
    ▼
Component Re-render
```

### State Update Examples

**Opening a Note:**
1. User clicks file in `FileTree`
2. `FileTree` calls `cmdOpenTreeFile()` from `vaultCommands.ts`
3. `vaultCommands.openTreeFile()` calls `ipc.file.get()`
4. Returns AST, dispatches `FILE_LOADED` action
5. `appReducer` updates `currentFile` and `currentAST`
6. `NoteView` re-renders with new AST

**Toggling Edit Mode:**
1. User clicks edit button in `NoteView` toolbar
2. `NoteView` calls `cmdEnterEditMode()` from `noteCommands.ts`
3. `noteCommands.enterEditMode()` calls `ipc.note.getRaw()`
4. Dispatches `EDIT_MODE_ENTER` action
5. `appReducer` updates `editMode=true` and `currentRaw`
6. `NoteView` switches to `MarkdownEditor`

**Graph Node Click:**
1. User clicks node in `GraphView`
2. `GraphView` calls `window.electron.file.get()` directly
3. Dispatches `FILE_LOADED` action
4. `appReducer` updates state
5. `NoteView` re-renders

---

## 6. Thin UI Architecture

### Architecture Goal 9 Compliance

The renderer satisfies the Thin UI Architecture goal by ensuring:

1. **Components are presentation-only**: Components receive props and dispatch actions, but do not contain business logic.

2. **Commands own workflow orchestration**: Each feature has a `*Commands.ts` module that handles IPC calls and state updates.

3. **Services own feature-specific state**: `widgetService.ts` owns widget state, not `AppState`.

### Responsibilities

| Layer | Responsibilities |
|-------|-----------------|
| **Components** | - Render UI based on state<br>- Handle user interactions (clicks, keyboard)<br>- Dispatch actions to state<br>- Call command functions for workflows |
| **Commands** | - IPC orchestration<br>- Path computation<br>- Validation<br>- Multi-step workflows<br>- Dispatch state updates |
| **Services** | - Feature-specific state (widget state)<br>- IPC subscription management<br>- Expose hooks for components |
| **Store** | - State shape definition<br>- Action type definitions<br>- Pure reducer function<br>- Context and hook |

### Example: Note Save Flow

```tsx
// NoteView.tsx (presentation)
const handleSave = useCallback(() => {
  cmdSaveNote(state.currentFile, state.currentRaw, dispatch)
}, [state.currentFile, state.currentRaw])

// noteCommands.ts (orchestration)
export async function saveNote(filePath: string, content: string): Promise<SaveResult> {
  const result = await ipc.note.save(filePath, content)
  return { success: result.success, error: result.error ?? null }
}
```

---

## 7. Shared Infrastructure

### Shared UI Components

Located in `shared/components/`:

| Component | Purpose | Used By |
|-----------|---------|---------|
| `FavoriteToggle` | Star button for favorite state | `NoteView`, `FileTree` |
| `OutlinePanel` | Heading outline for current note | `Sidebar` |
| `SandboxedHtml` | Secure iframe for HTML content | `NoteView` (embeds) |
| `icons.tsx` | SVG icon components | All features |

**When to use shared components:**
- Used by 2+ features
- No feature-specific business logic
- Generic UI patterns (buttons, panels, icons)

### Reusable Hooks

| Hook | Location | Purpose |
|------|----------|---------|
| `useAppContext` | `shared/store.ts` | Access AppState and dispatch |
| `useWidgetActivity` | `features/widgets/widgetService.ts` | Subscribe to activity log |
| `useWidgetDictation` | `features/widgets/widgetService.ts` | Subscribe to dictation state |

### Contexts

| Context | Location | Purpose |
|---------|----------|---------|
| `AppContext` | `shared/store.ts` | Application state distribution |

### Utilities

| Utility | Location | Purpose |
|---------|----------|---------|
| `ipc` | `shared/ipc.ts` | Typed IPC boundary |
| `registerCommand` | `shared/commands/registry.ts` | Command registration |
| `fuzzySearch` | `features/search/fuzzy.ts` | Fuzzy matching |

---

## 8. Maintenance Guidelines

### Adding New Features

1. **Create feature directory** under `features/`
2. **Create entry point component** in the feature directory
3. **Create command module** if the feature needs IPC orchestration
4. **Register commands** in `shared/commands/feature-registrations.ts` if needed
5. **Add to App.tsx** conditional rendering
6. **Add state** to `shared/store.ts` if shared state is needed

### Adding Components

1. **Place in feature directory** if feature-specific
2. **Place in `shared/components/`** if used by multiple features
3. **Keep presentation-only** - no business logic
4. **Use `useAppContext`** for state access
5. **Call command functions** for workflows

### State Ownership Rules

1. **AppState** (`shared/store.ts`) owns all shared application state
2. **Feature state** that is not shared should use local component state
3. **Widget state** is owned by `widgetService.ts`
4. **Only `appReducer`** may write to `AppState`
5. **Derived state** is computed in `syncActiveAliases()` after tab mutations

### Command Placement

1. **Feature commands** go in `features/{feature}/{feature}Commands.ts`
2. **Global commands** go in `shared/commands/registry.ts`
3. **Feature toggle commands** go in `shared/commands/feature-registrations.ts`

### Feature Ownership Rules

1. **Each feature owns its components** - no cross-feature component imports
2. **Shared components** are in `shared/components/`
3. **Features may read shared state** but should not write to other features' state
4. **Commands may dispatch actions** for their feature's state
5. **IPC calls** go through `shared/ipc.ts`

### Dependency Rules

```
features/* → shared/* (allowed)
shared/* → features/* (NOT allowed)
features/* → features/* (NOT allowed, except shared components)
```

---

## 9. Verification

### Typecheck

Run `npm run typecheck` to verify TypeScript compilation.

### Development Server

Run `npm run dev` to start the development server.

### Gate A Verification

Gate A passes when:
- TypeScript compiles without errors
- Development server starts successfully
- All features render correctly

---

## 10. Summary

This documentation describes the Nabu renderer architecture as of Phase 5.6. The architecture follows:

- **Feature-oriented organization** with clear ownership boundaries
- **Single source of truth** for state in `shared/store.ts`
- **Thin UI components** that delegate to command modules
- **Unidirectional data flow** from user action to state update
- **Shared infrastructure** for cross-cutting concerns

All documented components, commands, and state flows match the current implementation.