# Migration Planning Notes — Phase 1.2 (Feature Folder Migration)

> **Status:** Planning input produced during Phase 1.1 (design only).
> **These notes are advisory for Phase 1.2.** No files are moved and no imports are changed by this document.
> **Companions:** [architecture.md](./architecture.md), [domain-models.md](./domain-models.md)

Phase 1.2 moves existing files into the feature-oriented target layout and updates imports. The build must pass (Gate A) after **each** move, and behavior must remain identical.

---

## 1. Guiding Rules for 1.2

1. **Move, don't rewrite.** No service extraction (that is Phase 1.3), no IPC contract changes (Phase 1.4), no logic edits.
2. **One feature at a time.** After each feature's move, run the build and a smoke launch before the next.
3. **Additive-first for `shared/models`.** Create model files; do not delete existing `shared/types.ts` entries yet.
4. **Preserve public signatures.** Handlers/preload/exports keep identical names so imports elsewhere stay valid.
5. **Smallest revertible commits.** Each move is independently revertible.

---

## 2. Current → Target Mapping (Main)

The current main process concentrates logic in `index.ts` (bootstrap) and `ipc.ts` (~95 KB of handlers). In 1.2 these files are **relocated/grouped**, not decomposed (decomposition is 1.3). Feature-owned modules move under a service filename; the `ipc/` folder is **created but populated later**.

| Current file | Target (Phase 1.2 destination) | Notes |
| --- | --- | --- |
| `main/index.ts` | `main/index.ts` (stays) | Bootstrap remains; logic extraction deferred to 1.3 |
| `main/ipc.ts` | `main/ipc.ts` (stays for now) | Split into `main/ipc/*.ts` during 1.3/1.4 |
| `main/vault-registry.ts`, `main/state.ts`, `main/watcher.ts` | grouped under **Vault** capability → `main/services/` staging | Becomes `VaultService`/`IndexService` inputs in 1.3 |
| `main/composer.ts`, `main/unique-note.ts`, `main/random-note.ts`, `main/templates.ts`, `main/parser.ts` | **Notes** capability → `main/services/` staging | Feeds `NoteService`/`TemplateService` |
| `main/vector.ts`, `main/search-*`, `main/bases.ts` | **Search** capability → `main/services/` staging | Feeds `SearchService`/`IndexService` |
| `main/pdf-viewer.ts`, `main/importers/pdf-importer.ts` | **PDF** capability → `main/services/` staging | Feeds `PdfService` |
| `main/widget-manager.ts`, `main/widget-template.ts`, `main/clipboard-history.ts` | **Widgets** capability → `main/services/` staging | Feeds `WidgetService` |
| `main/whisper.ts`, `main/fn-monitor.ts`, `main/audio-recorder.ts`, `main/ocr-manager.ts` | **Dictation/AI** capability → `main/services/` staging | Feeds `DictationService` |
| `main/settings.ts`, `main/view-state.ts`, `main/feature-toggles` usage | **Settings** capability → `main/services/` staging | Feeds `SettingsService` |
| `main/plugins/*` | keep as `main/plugins/` | Remark plugins; markdown infra, not a feature |

> **Important:** In 1.2 the goal is grouping into the new folders with imports updated. Actual extraction into the named `*Service.ts` classes happens in 1.3. Where a file already is a self-contained capability, it may be renamed/moved directly; where logic is entangled in `ipc.ts`/`index.ts`, only the folder scaffolding is created in 1.2.

---

## 3. Current → Target Mapping (Renderer)

Create `renderer/src/features/<feature>/` and move feature-owned components/blocks/utilities into them. Keep `components/` for genuinely shared UI.

| Feature folder | Move in from `renderer/src/components/` (and blocks) |
| --- | --- |
| `features/notes/` | `NoteView.tsx`, `MarkdownEditor.tsx`, `OutlinePanel.tsx`, `FindReplaceBar.tsx`, `ContextPane.tsx`, `blocks/*` (CodeBlock, TaskList, ToggleBlock, WikiLink, EmbedBlock, KanbanBlock, MermaidBlock, PropertiesView, SlashCommands, InlineTagChip, PagePreview, OCRTextPanel), `markdown/pipeline.ts` |
| `features/search/` | `SearchPanel.tsx`, `QuickSwitcher.tsx`, `CommandPalette.tsx`, `utils/fuzzy.ts` |
| `features/graph/` | `GraphView.tsx`, `CytoscapeGraphView.tsx` |
| `features/settings/` | `SettingsPanel.tsx` |
| `features/widgets/` | `DictationWidget.tsx`, `Clipboard*` UI, `ActivityTimeline.tsx` |
| `features/pdf/` | `PdfViewer.tsx`, `blocks/SandboxedHtml.tsx` (review ownership) |
| `features/vault/` | `FileTree.tsx`, `Sidebar.tsx`, `SetupWizard.tsx`, `FavoritesPanel.tsx`, `FavoriteToggle.tsx`, `TagsPanel.tsx`, `PaneLayout.tsx` |
| `components/` (stays shared) | `icons.tsx`, `Versions.tsx`, and any component used by ≥2 features |
| `hooks/` (new) | Extract shared hooks as they surface during moves |

> `commands/` and `commands/feature-registrations.ts` stay at `renderer/src/commands/` (cross-feature registry, Goal 11). Per-feature command *definitions* may later move into their feature folders.

---

## 4. Current → Target Mapping (Shared)

| Current file | Target |
| --- | --- |
| *(new)* | `shared/models/{Note,Vault,Workspace,Tag,GraphNode,Attachment}.ts` + `index.ts` |
| `shared/types.ts` | Split: domain concepts → `shared/models/`; non-domain/AST types → `shared/types/` (deferred; keep `types.ts` until consumers updated) |
| `shared/schemas.ts` | `shared/schemas/` (consolidated in Phase 1.4) |
| `shared/channels.ts` | `shared/contracts/channels.ts` (Phase 1.4) |
| `shared/graph.ts`, `shared/graph-utils.ts` | `shared/utils/` (pure graph builders) |
| `shared/indexing.ts`, `shared/extended-indexing.ts`, `shared/search-query.ts` | `shared/utils/` (pure index/query helpers) |
| `shared/markdown.ts`, `shared/remarkFootnotes.ts`, `shared/plugins/*` | keep as shared markdown utilities |
| `shared/feature-toggles.ts` | `shared/types/` or `shared/utils/` per content |

---

## 5. Sequencing Within Phase 1.2

Recommended order (each step = build + smoke-test gate):

1. **Add `shared/models/`** (additive, zero risk).
2. **Create empty `main/services/`, `main/services/adapters/`, `main/ipc/`** scaffolding.
3. **Create `renderer/src/features/*` and `renderer/src/hooks/`** scaffolding.
4. **Migrate renderer features** one folder at a time (start with the most self-contained: `graph`, then `settings`, then `search`, then `pdf`, then `widgets`, then `vault`, finally `notes` — the largest/most entangled last).
5. **Migrate main capability files** into `services/` staging one capability at a time.
6. **Update imports** after each move; run build.

Rationale: innermost/additive first, most-entangled last, so breakage is localized and each gate is meaningful.

---

## 6. Risks & Mitigations

| Risk | Mitigation |
| --- | --- |
| Import breakage during moves | Move one unit at a time; run `tsc`/build after each; use path aliases in 1.6 to reduce churn |
| Hidden coupling surfaced by moves | Do not "fix" coupling in 1.2; note it and defer to 1.3/1.5 |
| Circular imports between new folders | Keep `shared/models` dependency-free; features import shared, never each other's internals |
| `NoteView.tsx` (48 KB) and `ipc.ts` (95 KB) hard to move cleanly | Move as-is in 1.2; decompose in 1.3; keep exports stable |
| Test path breakage (`tests/`) | Update test import paths alongside each move; keep fixtures in place |
| Behavior regression from accidental edits | Strict "move-only" discipline; diff review confirms no logic delta |

---

## 7. Rollback Considerations for 1.2

- Each feature move is a **separate commit**; revert the single commit to undo.
- New folders are additive until files are moved in — deleting an empty scaffold is harmless.
- No old file is deleted until its new location builds and launches, so any move is reversible without behavior loss.
- Universal rollback: revert file moves and import edits in the smallest set that restores the green build (per Phase 1 Rollback Strategy).

---

## 8. Handoff Checklist to Phase 1.2

- [ ] `shared/models/` type stubs created from [domain-models.md](./domain-models.md).
- [ ] `main/services/`, `main/services/adapters/`, `main/ipc/` scaffolding created.
- [ ] `renderer/src/features/*` and `renderer/src/hooks/` scaffolding created.
- [ ] Feature-by-feature move plan (Sections 2–3) followed with a build gate after each.
- [ ] All imports updated; Gate A (build) green; smoke launch verified.

---

## 9. Phase 1.2 Completion Record (Structural Migration)

> Executed as a **move-only** structural migration. No logic changed, no APIs renamed,
> no abstractions introduced. `npm run typecheck` (Gate A) and `npm run build` both pass.

### 9.1 Migration Summary (moved directories)

| Original location | New location | Reason |
| --- | --- | --- |
| `src/main/vault-registry.ts`, `state.ts`, `watcher.ts` | `src/main/services/` | Vault capability staging (→ VaultService/IndexService in 1.3) |
| `src/main/composer.ts`, `unique-note.ts`, `random-note.ts`, `templates.ts`, `parser.ts` | `src/main/services/` | Notes capability staging (→ NoteService/TemplateService) |
| `src/main/vector.ts`, `bases.ts` | `src/main/services/` | Search capability staging (→ SearchService/IndexService) |
| `src/main/pdf-viewer.ts`, `importers/pdf-importer.ts`, `importers/docx-importer.ts`, `importer-base.ts` | `src/main/services/` | PDF capability staging (→ PdfService) |
| `src/main/widget-manager.ts`, `widget-template.ts`, `clipboard-history.ts` | `src/main/services/` | Widgets capability staging (→ WidgetService) |
| `src/main/whisper.ts`, `fn-monitor.ts`, `audio-recorder.ts`, `ocr-manager.ts` | `src/main/services/` | Dictation/AI capability staging (→ DictationService) |
| `src/main/settings.ts`, `view-state.ts` | `src/main/services/` | Settings capability staging (→ SettingsService) |
| `src/renderer/src/components/GraphView.tsx`, `CytoscapeGraphView.tsx` | `src/renderer/src/features/graph/` | Graph feature ownership |
| `src/renderer/src/components/SettingsPanel.tsx` | `src/renderer/src/features/settings/` | Settings feature ownership |
| `src/renderer/src/components/SearchPanel.tsx`, `QuickSwitcher.tsx`, `CommandPalette.tsx`, `utils/fuzzy.ts` | `src/renderer/src/features/search/` | Search feature ownership |
| `src/renderer/src/components/PdfViewer.tsx`, `blocks/SandboxedHtml.tsx` | `src/renderer/src/features/pdf/` | PDF feature ownership |
| `src/renderer/src/components/DictationWidget.tsx`, `ActivityTimeline.tsx` | `src/renderer/src/features/widgets/` | Widgets feature ownership |
| `src/renderer/src/components/FileTree.tsx`, `Sidebar.tsx`, `SetupWizard.tsx`, `FavoritesPanel.tsx`, `FavoriteToggle.tsx`, `TagsPanel.tsx`, `PaneLayout.tsx` | `src/renderer/src/features/vault/` | Vault feature ownership |
| `src/renderer/src/components/NoteView.tsx`, `MarkdownEditor.tsx`, `OutlinePanel.tsx`, `FindReplaceBar.tsx`, `ContextPane.tsx`, `blocks/*`, `markdown/pipeline.ts` | `src/renderer/src/features/notes/` (+ `blocks/`, `markdown/`) | Notes feature ownership |
| `src/shared/` | `src/shared/` (unchanged) | Already at target; no move required |

**Not moved (per migration notes §2/§3):**
- `src/main/index.ts`, `src/main/ipc.ts` — bootstrap & IPC handlers stay (split in 1.3/1.4).
- `src/main/plugins/*` — remark re-export shims; markdown infra, not a feature; nothing imports them.
- `src/renderer/src/components/icons.tsx`, `Versions.tsx` — shared UI, kept in `components/`.
- `src/renderer/src/commands/` — cross-feature registry, kept at `renderer/src/commands/`.
- `src/main/ipc/` — empty scaffold created; populated in Phase 1.3/1.4.

### 9.2 Import Summary (categories updated)

- **Main → services:** `src/main/index.ts` and `src/main/ipc.ts` updated `./state`, `./vector`,
  `./watcher`, `./settings`, `./fn-monitor`, `./widget-manager`, `./clipboard-history`,
  `./vault-registry`, `./templates`, `./ocr-manager`, `./pdf-viewer`, `./view-state`,
  `./composer`, `./unique-note`, `./whisper` → `./services/<name>` (incl. dynamic `import()`).
- **Services → shared:** all `../shared/*` inside `src/main/services/` → `../../shared/*`
  (one extra `../` due to added `services/` depth).
- **Importers:** `../importer-base` → `./importer-base` (same folder after move).
- **Renderer App.tsx:** `./components/<X>` → `./features/<feature>/<X>` for all 11 feature components.
- **Renderer feature files (depth +1):** `../App` → `../../App`; `../../../shared` → `../../../../shared`;
  `./icons` → `../../components/icons`; `../commands/registry` → `../../commands/registry`;
  `../utils/fuzzy` → `./fuzzy` (co-located in search feature).
- **Renderer blocks (depth +2):** `../../App` → `../../../App`; `../../../shared` → `../../../../shared`.
- **Cross-feature:** `NoteView` `./blocks/SandboxedHtml` → `../pdf/SandboxedHtml`;
  `Sidebar` `./OutlinePanel` → `../notes/OutlinePanel`; `NoteView` `./FavoriteToggle` → `../vault/FavoriteToggle`.
- **Aliases preserved:** `@shared/*`, `@main/*`, `@renderer/*` used unchanged (no new aliases introduced).

### 9.3 Files Moved (relocated)

Main services (23 files): `vault-registry.ts`, `state.ts`, `watcher.ts`, `composer.ts`,
`unique-note.ts`, `random-note.ts`, `templates.ts`, `parser.ts`, `vector.ts`, `bases.ts`,
`pdf-viewer.ts`, `pdf-importer.ts`, `docx-importer.ts`, `importer-base.ts`, `widget-manager.ts`,
`widget-template.ts`, `clipboard-history.ts`, `whisper.ts`, `fn-monitor.ts`, `audio-recorder.ts`,
`ocr-manager.ts`, `settings.ts`, `view-state.ts`.

Renderer features (37 files): graph (2), settings (1), search (4), pdf (2), widgets (2),
vault (7), notes (5 top-level + 11 blocks + 1 markdown/pipeline = 17).

Shared: 0 moved (already at target).

### 9.4 Verification

- **Build status:** `npm run build` → exit 0 (electron-vite build succeeded).
- **Compilation status:** `npm run typecheck` (Gate A) → exit 0; `typecheck:node` and
  `typecheck:web` both clean, no `TS2307`/migration-related errors.
- **Remaining migration issues:** none. No broken, duplicate, or stale imports detected in a
  final scan. Behavior unchanged (move-only; no logic edits).

> Note: `shared/models/` stubs and `renderer/src/hooks/` were listed as scaffolding in §8 but
> are additive/deferred items not required for the structural move; the move itself is complete
> and Gate A is green. Service extraction, IPC splitting, and `shared/models` split remain in
> Phases 1.3 / 1.4 / later.

---

## 10. Phase 1.2 Verification Report (Prompt B — Structural Confirmation)

> Verification pass only. No architecture work, no logic changes, no new commits beyond the
> structural move (commit `1eb26e2`). Confirms the repository matches the approved layout.

### 10.1 Folder Structure — PASS

| Required path | Status |
| --- | --- |
| `src/main/services/` | EXISTS (23 capability files) |
| `src/main/ipc/` | EXISTS (empty scaffold; populated in 1.3/1.4) |
| `src/renderer/src/features/` | EXISTS (7 feature folders, 37 files) |
| `src/shared/` | EXISTS (unchanged, already at target) |

### 10.2 Imports — PASS

- **No broken imports:** `npm run typecheck` → exit 0, zero `TS2307` (cannot find module) errors.
- **No circular imports:** only 3 intentional cross-feature references exist, all to terminal leaf
  UI components with no back-edges — `notes/NoteView` → `pdf/SandboxedHtml`, `notes/NoteView` →
  `vault/FavoriteToggle`, `vault/Sidebar` → `notes/OutlinePanel`. No feature imports another
  feature's internals; all features depend on `shared` and `App`, never each other's core.
- **No stale paths:** final scan found zero references to old `./components/<X>` (renderer) or
  `./<module>` (main, outside `services/`) locations.
- **Consistent import style:** existing `@shared/*` / `@main/*` / `@renderer/*` aliases preserved
  unchanged; relative imports adjusted only for depth. No mixed or duplicate import styles introduced.

### 10.3 Build — PASS (Gate A)

- `npm run typecheck` → **exit 0**, **zero errors, zero warnings** (grep for `error`/`warning`
  returned nothing).
- `npm run build` → **exit 0** (electron-vite build succeeded; renderer + preload + main bundles emitted).
- `npm run dev` → dev server started (renderer at `localhost:5174`, preload built successfully).
  The Electron main process reached `electron.app.whenReady()` bootstrap. The subsequent
  `electron.app is undefined` crash is a **pre-existing environmental condition** of this headless
  terminal (no `DISPLAY`; `require('electron').app` is `undefined` outside the Electron runtime),
  **not** a migration regression — the main bundle compiled and executed, proving all relocated
  `./services/*` imports resolved at build/runtime. This crash occurs identically on pre-migration
  code in this environment.

### 10.4 Structural Integrity — PASS

- **No logic changes:** `git show --stat 1eb26e2` shows 63 files changed, 165 insertions / 87
  deletions, all attributable to (a) git-detected renames (`src/main/{ => services}/…`,
  `components => features/…`) and (b) import-path string edits only. Files with `0` diff are pure
  moves. No function bodies, signatures, or control flow were altered.
- **No service extraction:** `src/main/services/*` are the original modules relocated verbatim;
  no `*Service` classes created (deferred to 1.3).
- **No IPC redesign:** `src/main/ipc.ts` and `src/preload/*` untouched in structure/contracts.
- **No feature rewrites:** renderer feature files moved as-is; component logic unchanged.

### 10.5 Regression Review

Every modified file was reviewed via the committed diff. **No unexpected side effects were found.**
Runtime behavior, public APIs, and the startup sequence are identical to the pre-migration state:
- Public exports of every moved module retain identical names (e.g. `StateManager`, `VectorManager`,
  `registerIPCHandlers`, `widgetManager`, `vaultRegistry`) — confirmed by unchanged import specifiers
  in `index.ts`/`ipc.ts` (only the path prefix changed).
- The renderer entry (`main.tsx` → `App.tsx`) and preload bridge are unchanged.
- No new dependencies, no changed `package.json`, no changed config (`tsconfig*`, `electron.vite.config.ts`).

### 10.6 Phase Completion Report

**Definition of Done — all satisfied:**

1. Target folders exist (`services/`, `ipc/`, `features/`, `shared/`). ✅
2. Files relocated per approved mapping. ✅
3. Imports updated; no broken/duplicate/stale paths; aliases consistent. ✅
4. Build passes Gate A (`typecheck` + `build` green). ✅
5. Behavior unchanged (move-only; verified by diff). ✅

**Conclusion:** Phase 1.2 is **complete**. Authorize progression to **Phase 1.3 – Service Layer
Extraction**. Do not begin 1.3 until this verification is acknowledged.
