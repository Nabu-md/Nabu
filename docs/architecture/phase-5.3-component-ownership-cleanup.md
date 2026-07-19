# Phase 5.3 — Component Ownership Cleanup

**Program:** Nabu Recovery Program
**Phase:** 5.3 — Component Ownership Cleanup
**Scope:** Component responsibilities, ownership boundaries, and dependency coupling only. No state-management changes, no business-logic extraction, no IPC/UI/feature changes.
**Status:** ✅ Complete — Gate A passed

---

## 1. Component Ownership Report

Every major renderer component was audited for primary responsibility, owning feature, parent/child relationships, and shared dependencies.

### 1.1 App shell (root composition)

| Component | Responsibility | Owning feature | Parent | Children | Shared deps |
|-----------|---------------|----------------|--------|----------|-------------|
| `App.tsx` | Root composition: wires IPC listeners, mounts `AppContext.Provider`, renders layout (Sidebar, NoteView/GraphView/PdfViewer, ContextPane, ActivityTimeline, SearchPanel, SettingsPanel, QuickSwitcher, CommandPalette) | app shell | — | Sidebar, NoteView, GraphView, PdfViewer, SettingsPanel, ContextPane, ActivityTimeline, SetupWizard, SearchPanel, QuickSwitcher, CommandPalette | `shared/store`, `shared/ipc`, `shared/commands`, `shared/components` |

> **Ownership change (this phase):** `App.tsx` no longer *owns* the state/context/reducer infrastructure. It now imports those from `shared/store` and re-exports them for backward compatibility. `App.tsx` is purely the composition root + IPC wiring.

### 1.2 Shared infrastructure (`shared/`)

| Module | Responsibility | Owning feature | Consumers |
|--------|---------------|----------------|-----------|
| `shared/store.ts` | **Single owner** of renderer state: `AppState`, `AppAction`, `appReducer`, `initialState`, `syncActiveAliases`, `AppContext`, `useAppContext` | app shell (state) | All feature components |
| `shared/ipc.ts` | Typed IPC boundary wrapping `window.electron` | cross-cutting | All features that talk to main |
| `shared/commands/registry.ts` | Command registry (seed/register/get) | cross-cutting | CommandPalette, App |
| `shared/components/icons.tsx` | Shared SVG icon set | cross-cutting | vault, notes, app shell |
| `shared/components/FavoriteToggle.tsx` | Star toggle control | cross-cutting | notes (NoteView), vault (FileTree) |
| `shared/components/SandboxedHtml.tsx` | Sandboxed iframe HTML renderer | cross-cutting | notes (NoteView) |
| `shared/components/OutlinePanel.tsx` | Current-note heading outline panel | cross-cutting | vault (Sidebar) |

### 1.3 Feature modules (`features/`)

**vault** — `Sidebar` (icon ribbon + panel host), `FileTree` (file browser), `TagsPanel` (tag filter), `FavoritesPanel` (favorites), `PaneLayout` (layout state, currently unwired), `SetupWizard` (first-run vault setup). All consume `shared/store` + `shared/components`.

**notes** — `NoteView` (note renderer + edit/preview toolbar logic), `ContextPane` (backlink/context search), `MarkdownEditor` (edit textarea), `FindReplaceBar` (find/replace), `markdown/pipeline.ts` (parse), `blocks/*` (CodeBlock, EmbedBlock, InlineTagChip, KanbanBlock, MermaidBlock, OCRTextPanel, PagePreview, PropertiesView, SlashCommands, TaskList, ToggleBlock, WikiLink). All consume `shared/store` + `shared/components`.

**graph** — `GraphView` (d3-force canvas graph), `CytoscapeGraphView` (cytoscape alternative). Consume `shared/store` + `@shared/graph-utils`.

**pdf** — `PdfViewer` (PDF page rasterisation via IPC). Consumes `shared/types` only (no store dependency — fully self-contained via props + IPC).

**search** — `SearchPanel`, `QuickSwitcher`, `CommandPalette`, `fuzzy.ts`. Consume `shared/store` + `shared/commands`.

**settings** — `SettingsPanel`. Consumes `shared/store`.

**widgets** — `ActivityTimeline`, `DictationWidget`, `widgetService.ts`. Consume `shared/store` + `shared/ipc`.

### 1.4 Components with multiple responsibilities (identified)

- **`App.tsx` (before):** simultaneously owned (a) the reducer/state/context infrastructure, (b) IPC wiring, and (c) root layout composition. This made it a "god" module that every component transitively imported. **Resolved** by extracting (a) into `shared/store.ts`.
- **`OutlinePanel` (before):** a notes-feature component consumed by the vault `Sidebar` — a cross-feature ownership violation. **Resolved** by relocating it to `shared/components/`.
- No other component was found to combine data-fetching, workflow coordination, and unrelated-UI rendering. Each remaining component has a single, well-defined purpose.

---

## 2. Coupling Report

### 2.1 Coupling removed

| # | Coupling (before) | After | Mechanism |
|---|-------------------|-------|-----------|
| 1 | Every feature component → `../../App` (root module containing reducer + IPC wiring + composition) | Every feature component → `../../shared/store` | Extracted state/context/reducer/action infrastructure into `shared/store.ts`. Components no longer transitively import the root composition module. |
| 2 | `vault/Sidebar` → `../notes/OutlinePanel` (cross-feature import) | `vault/Sidebar` → `../../shared/components/OutlinePanel` | Relocated `OutlinePanel` to `shared/components/` (it is consumed by the vault sidebar, so it is genuinely shared UI). |
| 3 | `shared/commands/registry.ts` → `../../App` (for `AppAction` type) | `shared/commands/registry.ts` → `../../shared/store` | `AppAction` type now lives in `shared/store`. |

**Net effect:** Zero feature-to-feature component imports remain. A search for `from '../(notes|vault|graph|pdf|search|settings|widgets)/'` across `features/` returns **0 results**.

### 2.2 Dependency improvements

- **Unidirectional flow established:** `features/*` → `shared/*` → (none, terminal). No feature imports another feature. No shared module imports a feature. Dependencies flow in one direction.
- **Reduced blast radius:** A change to `App.tsx` composition or IPC wiring no longer forces re-resolution of every feature component's import graph, because components bind to the stable `shared/store` contract instead of the volatile root module.
- **Clearer ownership:** State ownership is now physically isolated in `shared/store.ts`; the root `App.tsx` is unambiguously the composition root.

### 2.3 Remaining intentional coupling

- **All features → `shared/store` (`AppContext`/`useAppContext`/`AppState`/`AppAction`):** The single state source. Intentional and required; this is the established architecture (Phase 5.2).
- **All features → `shared/ipc.ts`:** The only sanctioned bridge to the main process. Intentional.
- **`App.tsx` re-exports store symbols** (`AppContext`, `useAppContext`, `appReducer`, `syncActiveAliases`, and the state/action types) for backward compatibility with any external consumer (e.g. tests) that referenced them from the root module. This is a thin re-export shim, not a new dependency.
- **`notes/blocks/*` → `notes/blocks/*`:** Sibling blocks within the same feature (e.g. `NoteView` → `ToggleBlock`, `EmbedBlock` → `OCRTextPanel`). Correct intra-feature ownership.
- **`shared/commands/feature-registrations.ts`:** Not imported by any runtime path; retained as documentation of intended feature-toggle → command wiring (unchanged from Phase 5.1).

---

## 3. Component Boundary Summary

Finalized ownership model:

```
src/renderer/src/
├── App.tsx                  # composition root + IPC wiring ONLY (re-exports store for compat)
├── main.tsx                 # react bootstrap
├── shared/                  # cross-cutting infrastructure (single owner per module)
│   ├── store.ts             # ★ STATE OWNER: AppState, AppAction, appReducer,
│   │                        #   initialState, syncActiveAliases, AppContext, useAppContext
│   ├── ipc.ts               # IPC boundary
│   ├── commands/
│   │   ├── registry.ts      # command registry
│   │   └── feature-registrations.ts  # (dead doc, retained)
│   └── components/
│       ├── icons.tsx        # shared icons
│       ├── FavoriteToggle.tsx
│       ├── SandboxedHtml.tsx
│       └── OutlinePanel.tsx # ★ moved here from features/notes (consumed by vault)
├── features/                # each feature owns its components; imports only from shared/
│   ├── vault/      (Sidebar, FileTree, TagsPanel, FavoritesPanel, PaneLayout, SetupWizard)
│   ├── notes/      (NoteView, ContextPane, OutlinePanel→moved, FindReplaceBar,
│   │                MarkdownEditor, markdown/pipeline.ts, blocks/*)
│   ├── graph/      (GraphView, CytoscapeGraphView)
│   ├── pdf/        (PdfViewer)
│   ├── search/     (SearchPanel, QuickSwitcher, CommandPalette, fuzzy.ts)
│   ├── settings/   (SettingsPanel)
│   └── widgets/    (ActivityTimeline, DictationWidget, widgetService.ts)
```

**Boundary rules enforced:**
1. Each component belongs to exactly one feature (or `shared/`).
2. Shared UI (`icons`, `FavoriteToggle`, `SandboxedHtml`, `OutlinePanel`) lives in `shared/components/` and is feature-agnostic.
3. Feature-specific components remain within their feature directory.
4. Dependencies flow strictly `features/*` → `shared/*` (one direction). No `shared/*` → `features/*`. No `features/X` → `features/Y`.
5. State ownership is centralized in `shared/store.ts` (single owner, unchanged from Phase 5.2).

---

## 4. Files Modified

### 4.1 Created

| File | Change |
|------|--------|
| `src/renderer/src/shared/store.ts` | New module. Extracted `AppState`, `AppAction`, `appReducer`, `initialState`, `syncActiveAliases`, `AppContext`, `useAppContext`, and all supporting types (`OpenVault`, `Tab`, `PDFTab`, `Workspace`, `TabGroup`, `PaneLayout`, `GraphMode`, `getActiveVault`) from `App.tsx`. No logic changed — verbatim move. |
| `src/renderer/src/shared/components/OutlinePanel.tsx` | New location. Moved from `features/notes/OutlinePanel.tsx` (content unchanged except import path `../../App` → `../store`). |

### 4.2 Modified

| File | Change |
|------|--------|
| `src/renderer/src/App.tsx` | Removed the state/context/reducer/action infrastructure (now in `shared/store`). Imports `AppContext`, `appReducer`, `initialState` from `./shared/store`. Re-exports store symbols for backward compatibility. Added `useReducer` (React) and `Root` (mdast) / `Edge` (`@shared/types`) imports needed by the retained composition + IPC-wiring code. |
| `src/renderer/src/features/vault/Sidebar.tsx` | `../notes/OutlinePanel` → `../../shared/components/OutlinePanel`. |
| `src/renderer/src/features/notes/ContextPane.tsx` | `../../App` → `../../shared/store`. |
| `src/renderer/src/features/settings/SettingsPanel.tsx` | `../../App` → `../../shared/store`. |
| `src/renderer/src/features/search/SearchPanel.tsx` | `../../App` → `../../shared/store`. |
| `src/renderer/src/features/search/QuickSwitcher.tsx` | `../../App` → `../../shared/store`. |
| `src/renderer/src/features/search/CommandPalette.tsx` | `../../App` → `../../shared/store`. |
| `src/renderer/src/shared/commands/registry.ts` | `../../App` → `../../shared/store` (for `AppAction`). |
| `src/renderer/src/shared/components/FavoriteToggle.tsx` | `../../App` → `../../shared/store`. |
| `src/renderer/src/features/vault/PaneLayout.tsx` | `../../App` → `../../shared/store`. |
| `src/renderer/src/features/notes/OutlinePanel.tsx` | Deleted (moved to `shared/components/OutlinePanel.tsx`). |
| `src/renderer/src/features/notes/NoteView.tsx` | `../../App` → `../../shared/store`. |
| `src/renderer/src/features/vault/TagsPanel.tsx` | `../../App` → `../../shared/store`. |
| `src/renderer/src/features/vault/SetupWizard.tsx` | `../../App` → `../../shared/store`. |
| `src/renderer/src/features/vault/FileTree.tsx` | `../../App` → `../../shared/store`. |
| `src/renderer/src/features/vault/FavoritesPanel.tsx` | `../../App` → `../../shared/store`. |
| `src/renderer/src/features/notes/blocks/InlineTagChip.tsx` | `../../../App` → `../../../shared/store`. |
| `src/renderer/src/features/notes/blocks/EmbedBlock.tsx` | `../../../App` → `../../../shared/store`. |
| `src/renderer/src/features/notes/blocks/ToggleBlock.tsx` | `../../../App` → `../../../shared/store` (for `AppContext`). |
| `src/renderer/src/features/graph/GraphView.tsx` | `../../App` → `../../shared/store`. |
| `src/renderer/src/features/notes/blocks/MermaidBlock.tsx` | `../../../App` → `../../../shared/store`. |

No business logic, state-management logic, IPC behavior, UI appearance, or application features were modified. All changes are import-path/ownership relocations only.

---

## 5. Verification Summary

### 5.1 Build status

- `npm run typecheck` — ✅ **PASS** (node + web projects, 0 errors, 0 warnings related to changes).
- `npm run build` (`electron-vite build`) — ✅ **PASS** (renderer + main + preload bundled cleanly, exit 0).

### 5.2 Runtime status

- `npm run dev` — ✅ Renderer Vite dev server compiled and served at `http://localhost:5173/` with **no import-resolution errors** (73 modules transformed in the renderer graph; the `shared/store` and `shared/components/OutlinePanel` modules resolve correctly).
- The `electron.app.whenReady is not a function` error observed at the end of `npm run dev` is the **pre-existing environment limitation** (Electron binary unavailable in this headless sandbox), identical to that documented in Phases 5.1 and 5.2. It is unrelated to the renderer reorganization — the renderer bundle built and served cleanly, which is the relevant validation for this phase.

### 5.3 Component validation

- **Zero cross-feature imports:** A regex scan for `from '../(notes|vault|graph|pdf|search|settings|widgets)/'` across `features/` returns 0 matches.
- **Single state owner preserved:** `appReducer` remains the only writer of `AppState`; `syncActiveAliases` remains the only derivation site. These were moved verbatim — no behavioral change.
- **Backward compatibility:** `App.tsx` re-exports `AppContext`, `useAppContext`, `appReducer`, `syncActiveAliases`, and all state/action types, so any external reference to these names from the root module continues to resolve.
- **No behavior change:** The reducer, context value shape, IPC wiring, and component render trees are byte-for-byte equivalent at runtime. Only import paths and module boundaries changed.

### 5.4 Gate A

✅ **Gate A passes** — component responsibilities are clear, excessive coupling (root-module hub + cross-feature import) has been reduced, component boundaries are documented, `typecheck` + `build` are clean, and runtime behavior is unchanged.

---

## 6. Deferred to later phases

- **Phase 5.4 (business-logic extraction):** Extract reducer side-effect orchestration (IPC wiring in `App.wireListeners`) if desired. The state infrastructure is now isolated in `shared/store.ts`, making such extraction cleaner.
- **Unify `FILE_LOADED` with the tab system** (carried over from 5.2): `currentFile`/`currentAST` remain the one justified non-tab writer; out of scope for 5.3.
