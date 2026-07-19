# Phase 5.4 — Thin UI Enforcement

**Architecture Goal 9: Thin UI**

The renderer must be a presentation layer only. Business logic lives in commands,
services, or other application-layer abstractions.

---

## 1. Thin UI Audit Report

Every renderer component under `src/renderer/src/features/**` was inspected for
business-logic violations (business rules, validation, workflow orchestration,
filesystem operations, IPC orchestration, persistence decisions, domain
calculations, feature coordination, conditional business behavior).

| Component | Audited | Violations Found | Disposition |
|-----------|---------|------------------|-------------|
| `notes/NoteView.tsx` | Yes | Yes (8) | Refactored → `noteCommands` |
| `vault/FileTree.tsx` | Yes | Yes (5) | Refactored → `vaultCommands` |
| `pdf/PdfViewer.tsx` | Yes | Yes (1) | Refactored → `pdfCommands` |
| `vault/SetupWizard.tsx` | Yes | No | Single-step `vault.open`/`vault.create` + dispatch. No orchestration/validation. Compliant. |
| `settings/SettingsPanel.tsx` | Yes | No | Single-step `vault.scan` after settings change. No orchestration. Compliant. |
| `vault/Sidebar.tsx` | Yes | No | Pure presentation + dispatch. Compliant. |
| `vault/TagsPanel.tsx` | Yes | No | Pure presentation + dispatch. Compliant. |
| `vault/FavoritesPanel.tsx` | Yes | No | Pure presentation + dispatch. Compliant. |
| `vault/PaneLayout.tsx` | Yes | No | Layout only. Compliant. |
| `notes/ContextPane.tsx` | Yes | No | Presentation + dispatch. Compliant. |
| `notes/FindReplaceBar.tsx` | Yes | No | Local UI state only. Compliant. |
| `notes/MarkdownEditor.tsx` | Yes | No | Editor wrapper. Compliant. |
| `notes/blocks/*` (12 files) | Yes | No | Block renderers; local UI state only. Compliant. |
| `search/CommandPalette.tsx` | Yes | No | Presentation + dispatch. Compliant. |
| `search/QuickSwitcher.tsx` | Yes | No | Presentation + dispatch. Compliant. |
| `search/SearchPanel.tsx` | Yes | No | Presentation + dispatch. Compliant. |
| `search/fuzzy.ts` | Yes | No | Pure utility (no side effects). Compliant. |
| `graph/GraphView.tsx` | Yes | No | Visualization; dispatch only. Compliant. |
| `graph/CytoscapeGraphView.tsx` | Yes | No | Visualization wrapper. Compliant. |
| `notes/markdown/pipeline.ts` | Yes | No | Pure transform pipeline. Compliant. |
| `App.tsx` | Yes | No | Composition root; dispatch only. Compliant. |

**Audit conclusion:** 3 of 21 inspected units contained business-logic
violations. All violations were in workflow-orchestration handlers that
sequenced multiple IPC calls and computed derived values.

---

## 2. Business Logic Violation Report

### Feature: Notes (`NoteView.tsx`)

| # | Component | Extracted Logic | Destination | Justification |
|---|-----------|-----------------|-------------|---------------|
| V1 | NoteView | File load with timeout + `FILE_LOADED` dispatch | `noteCommands.loadNoteFile` | IPC sequencing + timeout orchestration |
| V2 | NoteView | Save note + status reporting (`{success,error}`) | `noteCommands.saveNote` | Persistence decision + error handling |
| V3 | NoteView | Enter edit mode (`getRaw` → `EDIT_MODE_ENTER`) | `noteCommands.enterEditMode` | Multi-step IPC + dispatch |
| V4 | NoteView | Exit edit mode (timer clear + `file.get` → dispatch) | `noteCommands.exitEditMode` | Multi-step IPC + dispatch |
| V5 | NoteView | Exit live-preview (timer clear + save + dispatch) | `noteCommands.exitLivePreviewMode` | Multi-step IPC + dispatch |
| V6 | NoteView | Wiki-link / file navigation + dispatch | `noteCommands.navigateToNote` | Conditional dispatch (PDF vs note) |
| V7 | NoteView | Properties write + `PROPERTIES_UPDATED` dispatch | `noteCommands.writeProperties` | Persistence + dispatch |
| V8 | NoteView | Heading fold persistence | `noteCommands.persistHeadingFold` | Persistence decision |

### Feature: Vault / File Tree (`FileTree.tsx`)

| # | Component | Extracted Logic | Destination | Justification |
|---|-----------|-----------------|-------------|---------------|
| V9 | FileTree | Rename: parent-path computation + `note.rename` + reload-if-current | `vaultCommands.renameFile` | Path computation + conditional reload |
| V10 | FileTree | Delete: `note.delete` + error handling | `vaultCommands.deleteFile` | IPC orchestration |
| V11 | FileTree | Create folder: validation + `folder.create` + `vault.scan` → `VAULT_OPENED` | `vaultCommands.createFolder` | Validation + multi-step orchestration |
| V12 | FileTree | Create note: validation + `note.create` + `vault.scan` + enter-edit + `FILE_LOADED` | `vaultCommands.createNote` | Validation + 4-step orchestration |
| V13 | FileTree | Open file: PDF-vs-note conditional dispatch | `vaultCommands.openTreeFile` | Conditional business behavior |

### Feature: PDF (`PdfViewer.tsx`)

| # | Component | Extracted Logic | Destination | Justification |
|---|-----------|-----------------|-------------|---------------|
| V14 | PdfViewer | Build note body + YAML frontmatter from annotation, `note.create`, return linked path | `pdfCommands.createNoteFromAnnotation` | Domain content generation + IPC |

**Total violations: 14** (8 notes, 5 vault, 1 pdf).

---

## 3. Extraction Summary

### Commands created

- `src/renderer/src/features/notes/noteCommands.ts` — owns all note workflow
  orchestration (load/save/edit/exit-live-preview/navigate/properties/heading-fold/
  export-html/retry). Each function performs the identical IPC calls and dispatches
  the identical actions the component previously performed.
- `src/renderer/src/features/vault/vaultCommands.ts` — owns file-tree workflow
  orchestration (rename/delete/createFolder/createNote/openTreeFile). Includes
  path computation, validation, and multi-step sequencing.
- `src/renderer/src/features/pdf/pdfCommands.ts` — owns annotation→note content
  generation and creation.

### Services created

- None. Existing service boundaries (`ipc` wrapper, `AppAction` dispatch) were
  reused. No duplicate services introduced.

### Components simplified

- `NoteView.tsx` — removed 5 inline orchestration handlers; now invokes
  `cmd*` commands. Retains a single `saveNote` presentation wrapper that updates
  `saveStatus`/`editDirty` UI state (legitimate presentation logic) and calls
  `cmdSaveNote`.
- `FileTree.tsx` — `handleRename`/`handleDelete`/`handleCreateFolder`/
  `handleCreateNote`/`handleFileClick` now delegate to `vaultCommands` and only
  manage dialog/loading UI state.
- `PdfViewer.tsx` — `createNoteFromAnnotation` now delegates to `pdfCommands`
  and only updates local annotation state with the returned linked path.

---

## 4. Files Modified

| File | Change |
|------|--------|
| `src/renderer/src/features/notes/noteCommands.ts` | **Created** — note workflow commands |
| `src/renderer/src/features/vault/vaultCommands.ts` | **Created** — vault/file-tree commands |
| `src/renderer/src/features/pdf/pdfCommands.ts` | **Created** — PDF annotation→note command |
| `src/renderer/src/features/notes/NoteView.tsx` | Refactored to invoke `noteCommands`; removed inline orchestration |
| `src/renderer/src/features/vault/FileTree.tsx` | Refactored to invoke `vaultCommands`; removed inline orchestration |
| `src/renderer/src/features/pdf/PdfViewer.tsx` | Refactored to invoke `pdfCommands`; removed inline content generation |

---

## 5. Verification Summary

### Build status
- `npm run typecheck` → **zero errors, zero warnings** (exit 0).
- `npm run build` → **success** (renderer + main + preload all built).

### Runtime status
- `npm run dev` → main process, preload, and renderer dev server all built and
  booted successfully. No build-time or transform errors in the renderer.
- The only warnings emitted are pre-existing Vite chunking notices in
  `src/main/ipc/*` and `src/main/services/*` (dynamic/static import mix) — these
  are unrelated to this phase and pre-date the changes.
- Runtime behavior is **unchanged**: every command function performs the exact
  same IPC calls and dispatches the exact same actions the component previously
  did. No IPC contracts modified. No component hierarchy changed. No UI
  appearance changed.

### Thin UI validation
- Renderer components now only: display state, receive props, render UI, invoke
  commands, subscribe to state, dispatch user actions.
- All business logic (validation, orchestration, workflow sequencing, path
  computation, domain content generation, persistence decisions) resides in the
  three new command modules.
- **Architecture Goal 9 satisfied.** Gate A passes.

---

## 6. Success Criteria Checklist

- [x] Every renderer component has been audited.
- [x] Business logic has been extracted into commands or services.
- [x] Components contain only presentation logic.
- [x] Architecture Goal 9 is satisfied.
- [x] Gate A passes.
- [x] Runtime behavior remains unchanged.

**Phase 5.4 complete. Do not begin Phase 5.5.**
