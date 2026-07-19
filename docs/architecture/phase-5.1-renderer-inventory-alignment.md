# Phase 5.1 — Renderer Inventory & Feature Folder Alignment

**Status:** Complete (Gate A passed)
**Date:** 2026-07-19
**Scope:** Renderer organization only. No component behavior, state management, rendering logic, UI behavior, or IPC behavior was modified.

---

## 1. Renderer Inventory Report

### 1.1 Entry Points

| File | Role | Owner |
|------|------|-------|
| `src/renderer/src/main.tsx` | React root bootstrap | App shell (cross-cutting) |
| `src/renderer/src/App.tsx` | Root component, `useReducer` store, `AppContext`, IPC wiring, layout | App shell (cross-cutting) |
| `src/renderer/src/index.html` | HTML host | App shell |
| `src/renderer/src/env.d.ts` | Vite env typings | App shell |

### 1.2 Cross-Cutting Infrastructure (now `shared/`)

| File | Role |
|------|------|
| `shared/ipc.ts` | Typed IPC boundary wrapping `window.electron` |
| `shared/commands/registry.ts` | Command registry (seed, register, get, unregister) |
| `shared/commands/feature-registrations.ts` | Feature-toggle command wiring (currently uninvoked dead code, retained for documentation of intended wiring) |
| `shared/components/icons.tsx` | Shared SVG icon set (genuinely used by vault, notes, app shell) |
| `shared/components/FavoriteToggle.tsx` | Star toggle control (consumed by notes + vault) |
| `shared/components/SandboxedHtml.tsx` | Sandboxed iframe HTML renderer (consumed by notes) |

### 1.3 Feature Modules (`features/`)

**vault** — `Sidebar`, `FileTree`, `TagsPanel`, `FavoritesPanel`, `PaneLayout`, `SetupWizard`
**notes** — `NoteView`, `ContextPane`, `OutlinePanel`, `FindReplaceBar`, `MarkdownEditor`, `markdown/pipeline.ts`, `blocks/` (CodeBlock, EmbedBlock, InlineTagChip, KanbanBlock, MermaidBlock, OCRTextPanel, PagePreview, PropertiesView, SlashCommands, TaskList, ToggleBlock, WikiLink)
**graph** — `GraphView`, `CytoscapeGraphView`
**pdf** — `PdfViewer`
**search** — `SearchPanel`, `QuickSwitcher`, `CommandPalette`, `fuzzy.ts`
**settings** — `SettingsPanel`
**widgets** — `ActivityTimeline`, `DictationWidget`, `widgetService.ts`

### 1.4 Hooks / Stores / Contexts

- **Root store:** `App.tsx` `useReducer` + `AppContext` (single source of truth for all UI state). Not modified.
- **Feature hooks:** `useAppContext()` (re-exported from `App.tsx`), `useWidgetActivity` / `useWidgetDictation` (in `widgets/widgetService.ts`).
- **No Zustand/Redux/MobX** present in the renderer. State is React `useReducer` + Context + local `useState`/`useRef`.

### 1.5 Components With No Feature (before alignment)

- `components/icons.tsx` — shared, no owner → moved to `shared/components/`
- `components/Versions.tsx` — dead code (returns empty fragment, never imported) → **deleted**
- `commands/*` — cross-cutting → moved to `shared/commands/`
- `ipc.ts` — cross-cutting → moved to `shared/ipc.ts`
- `FavoriteToggle.tsx` — lived in `vault/` but consumed by `notes/` → moved to `shared/components/`
- `SandboxedHtml.tsx` — lived in `pdf/` but consumed by `notes/` → moved to `shared/components/`

---

## 2. Feature Ownership Report

### 2.1 Before → After

| File | Before owner | After owner | Reason |
|------|-------------|-------------|--------|
| `components/icons.tsx` | (none) | `shared/components` | Genuinely shared across vault/notes/shell |
| `components/Versions.tsx` | (none) | — (deleted) | Dead code, never imported |
| `commands/registry.ts` | (none) | `shared/commands` | Cross-cutting command infrastructure |
| `commands/feature-registrations.ts` | (none) | `shared/commands` | Cross-cutting feature-toggle wiring |
| `ipc.ts` | (none) | `shared/ipc` | Single IPC boundary |
| `features/vault/FavoriteToggle.tsx` | vault | `shared/components` | Consumed by notes (NoteView) and vault (FileTree) |
| `features/pdf/SandboxedHtml.tsx` | pdf | `shared/components` | Consumed by notes (NoteView) |
| `App.tsx`, `main.tsx` | (none) | App shell (root) | Entry + root state; relocating would churn 20+ paths for no ownership gain |

### 2.2 Ownership Rule Applied

Every renderer file now has exactly one owning feature or lives in `shared/` as genuinely cross-cutting infrastructure. No feature-specific component remains in `shared/`.

---

## 3. Folder Structure Report (final)

```
src/renderer/src/
├── App.tsx                      # root component + reducer store + context (app shell)
├── main.tsx                     # react bootstrap (app shell)
├── env.d.ts
├── index.html
├── assets/                      # css + svg (app shell)
├── shared/                      # cross-cutting infrastructure
│   ├── ipc.ts
│   ├── commands/
│   │   ├── registry.ts
│   │   └── feature-registrations.ts
│   └── components/
│       ├── icons.tsx
│       ├── FavoriteToggle.tsx
│       └── SandboxedHtml.tsx
├── features/                    # feature-owned modules
│   ├── vault/      (Sidebar, FileTree, TagsPanel, FavoritesPanel, PaneLayout, SetupWizard)
│   ├── notes/      (NoteView, ContextPane, OutlinePanel, FindReplaceBar, MarkdownEditor,
│   │                markdown/pipeline.ts, blocks/*)
│   ├── graph/      (GraphView, CytoscapeGraphView)
│   ├── pdf/        (PdfViewer)
│   ├── search/     (SearchPanel, QuickSwitcher, CommandPalette, fuzzy.ts)
│   ├── settings/   (SettingsPanel)
│   └── widgets/    (ActivityTimeline, DictationWidget, widgetService.ts)
└── utils/          (empty, pre-existing — left untouched)
```

---

## 4. Cross-Feature Coupling Report

### 4.1 Dependencies Removed (by relocation)

| Coupling | Before | After |
|----------|--------|-------|
| `notes/NoteView` → `vault/FavoriteToggle` | cross-feature import | `notes/NoteView` → `shared/components/FavoriteToggle` |
| `notes/NoteView` → `pdf/SandboxedHtml` | cross-feature import | `notes/NoteView` → `shared/components/SandboxedHtml` |
| `vault/Sidebar` → `notes/OutlinePanel` | cross-feature import | **retained** (see 4.3) |
| `search/CommandPalette` → `commands/registry` | root-level import | `search/CommandPalette` → `shared/commands/registry` |
| `App.tsx` → `components/`, `commands/`, `ipc` | root-level imports | `App.tsx` → `shared/...` |

### 4.2 Ownership Improvements

- `FavoriteToggle` and `SandboxedHtml` no longer masquerade as feature-owned; they are now explicitly shared infrastructure, eliminating two false feature boundaries.
- The `shared/` folder gives a single, discoverable home for cross-cutting renderer infrastructure, matching the `src/shared/` pattern already used at the workspace level.

### 4.3 Remaining Intentional Coupling

- **`vault/Sidebar` → `notes/OutlinePanel`**: The sidebar renders the note outline. `notes` is a lower-level feature than `vault`; vault depending on notes is acceptable and documented as intentional. No component redesign was performed (out of scope for 5.1).
- **All features → `App.tsx` (`useAppContext`/`AppContext`)**: The root reducer/context is the single state source. This is the established architecture and is explicitly deferred to Phase 5.2 (state management). Not changed.
- **All features → `shared/ipc.ts`**: The IPC boundary is the only sanctioned bridge to the main process. Intentional.
- **`shared/commands/feature-registrations.ts`**: Not imported by any runtime path; retained as documentation of the intended feature-toggle → command wiring. Will be wired in a later phase.

---

## 5. Files Modified

### 5.1 Moved (relocated, content unchanged except import paths)

| From | To |
|------|----|
| `src/renderer/src/components/` | `src/renderer/src/shared/components/` |
| `src/renderer/src/commands/` | `src/renderer/src/shared/commands/` |
| `src/renderer/src/ipc.ts` | `src/renderer/src/shared/ipc.ts` |
| `src/renderer/src/features/vault/FavoriteToggle.tsx` | `src/renderer/src/shared/components/FavoriteToggle.tsx` |
| `src/renderer/src/features/pdf/SandboxedHtml.tsx` | `src/renderer/src/shared/components/SandboxedHtml.tsx` |

### 5.2 Deleted

| File | Reason |
|------|--------|
| `src/renderer/src/components/Versions.tsx` | Dead code (returns `<></>`), never imported |

### 5.3 Import-path updates (no logic changes)

| File | Change |
|------|--------|
| `App.tsx` | `./components/icons` → `./shared/components/icons`; `./commands/registry` → `./shared/commands/registry`; `./ipc` → `./shared/ipc` |
| `features/vault/Sidebar.tsx` | `../../components/icons` → `../../shared/components/icons` |
| `features/vault/FileTree.tsx` | `../../components/icons` → `../../shared/components/icons` |
| `features/notes/NoteView.tsx` | `../vault/FavoriteToggle` → `../../shared/components/FavoriteToggle`; `../pdf/SandboxedHtml` → `../../shared/components/SandboxedHtml`; `../../components/icons` → `../../shared/components/icons` |
| `features/notes/ContextPane.tsx` | `../../ipc` → `../../shared/ipc` |
| `features/widgets/widgetService.ts` | `../../ipc` → `../../shared/ipc` |
| `features/search/CommandPalette.tsx` | `../../commands/registry` → `../../shared/commands/registry` |
| `shared/commands/registry.ts` | `../App` → `../../App` (depth correction after move) |

---

## 6. Verification Summary

### 6.1 Build Status

- `npm run typecheck` → **PASS** (zero TS errors, zero TS warnings).
  - `typecheck:node` (tsconfig.node.json): clean.
  - `typecheck:web` (tsconfig.web.json): clean.
- `npm run build` → **PASS** (exit 0; renderer bundle `index-*.js` + all feature chunks emitted successfully).

### 6.2 Runtime Status

- `npm run dev` → renderer Vite dev server compiled and served with **no import-resolution errors** (started on `:5174` because `:5173` was already occupied by a prior instance — environmental, not a code issue).
- The Electron main-process boot error observed in the headless sandbox (`electron.app.whenReady` undefined) is an environment limitation (Electron binary not available in this CI-like context) and is **unrelated to renderer reorganization** — the renderer bundle built and served cleanly, which is the relevant validation for this phase.

### 6.3 Renderer Validation

- All feature entry points (`NoteView`, `GraphView`, `PdfViewer`, `SettingsPanel`, `SearchPanel`, `QuickSwitcher`, `CommandPalette`, `Sidebar`, `SetupWizard`, `ActivityTimeline`, `DictationWidget`) resolve through the new `shared/` and `features/` paths.
- No component, hook, store, or IPC surface was altered — runtime behavior is unchanged by construction.

---

## 7. Success Criteria Checklist

- [x] Renderer fully inventoried (entry points, features, shared infra, state sources).
- [x] Feature folders clearly define ownership (`features/*` + `shared/*`).
- [x] Cross-feature coupling reduced (2 false feature boundaries eliminated; shared infra centralized).
- [x] Gate A passes (`typecheck` + `build` clean).
- [x] Runtime behavior unchanged (no logic/behavior modifications).

**Phase 5.2 (state management) and 5.3 (component restructuring) are explicitly deferred and were not started.**
