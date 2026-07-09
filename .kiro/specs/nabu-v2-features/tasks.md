# Implementation Plan: Nabu v2 Features

## Overview

This plan executes the v2 design in eleven dependency-ordered phases. Each phase is independently shippable: it lands behind its own UI and IPC, concludes with a verification task (full Vitest suite + Playwright e2e where the phase added UI), and does not require a later phase to be useful. Requirement numbers (`N.M`) trace every task back to `requirements.md`; correctness properties from `design.md` are validated in the testing tasks.

Phases 1–5 are additive features built on the v1 architecture. **Phase 6 is core-plugin parity** — 9 Obsidian core plugins ported to Nabu (Requirements 27–35 from the gap analysis). Phase 7 adds format import and the feature toggle system. Phases 8–10 are the architectural lifts (multi-vault, live preview, split panes/workspaces) that depend on Phase 1's shared index but are otherwise independent of each other — except Workspaces (Phase 10), which requires Split Panes. Phase 11 is the hardening and release sweep.

Numbering is a single increasing integer across all phases (it is not reset per phase), matching the `onyx-v1-features/tasks.md` convention.

---

## Tasks

### Phase 1 — Foundations & Index Repair

- [x] 1. Wire `VectorManager.embedFile` into the watcher add/change path
  - Add a single consolidated `VaultSession.handleFile*` (or equivalent) helper to remove the three duplicated watcher-callback sites in `ipc.ts` and `index.ts`.
  - Call `vectorManager.embedFile(filePath)` on add/change; call `removeFile` on delete.
  - Guard each call with `StateManager.hasPendingWrite` so app-initiated saves embed exactly once per logical edit.
  - _Requirements: 1.1, 1.3, 1.9_

- [x] 2. Enqueue embeddings from `StateManager.updateIndexesForFile`
  - After the incremental index update, enqueue an embedding for the saved file via the `VectorManager.AsyncQueue`.
  - Skip empty-content notes (frontmatter-only) to avoid degenerate vectors.
  - _Requirements: 1.2, 1.8_

- [x] 3. Add `context:reindex` and `vector:status` IPC channels
  - New channel `context:reindex` (Zod `ContextReindexSchema`) enqueues embeddings for all current vault files and returns a processed count.
  - New channel `vector:status` returns `{ disabled: boolean, reason: string | null }`.
  - Register both in `channels.ts`, `schemas.ts`, `ipc.ts`, and the preload bridge.
  - _Requirements: 1.5, 1.6, 26.1_

- [x] 4. Surface vector-disabled state in the renderer
  - On app load, query `vector:status`; store `vectorDisabled`/`vectorDisabledReason` in `AppState`.
  - Show a non-blocking notice in the ContextPane when semantic search is disabled, naming the reason.
  - _Requirements: 1.4_

- [x] 5. Make `context:query` honest about an empty/disabled index
  - Return `{ results: [], disabled }` when the index is empty or the model is disabled, instead of silently appearing to find nothing.
  - _Requirements: 1.7_

- [x] 6. Build `src/shared/extended-indexing.ts`
  - Implement `ExtendedSearchIndex` (token positions, line snippets, unified tag index, alias map, property index, blockRefs) and the pure `buildExtendedIndex` + `updateExtendedIndexForFile` functions.
  - Inline `#tag` extraction skipping `code`/`inlineCode`; namespaced-tag parent segments; `aliases` field extraction.
  - Reuse the v1 `SKIP`-on-`yaml`/`toml` rule for full-text positions while still extracting structured fields.
  - _Requirements: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7, 2.8_

- [x] 7. Integrate the Extended_Search_Index into `StateManager`
  - Build it alongside the v1 indexes in `buildIndexes`; update it in `updateIndexesForFile`.
  - Push it to the renderer via the `INDEX_BUILD` channel (extend the payload; keep the v1 inverted index for the fast path).
  - Store it as `extendedIndex` in `AppState`.
  - _Requirements: 2.6, 2.8_

- [x] 8. Phase 1 verification
  - Unit tests: `tests/unit/extended-indexing.test.ts`, extend `tests/integration/ipc.test.ts` for `context:reindex`/`vector:status`.
  - Property tests: incremental-update == full-rebuild (fast-check).
  - E2E: open vault → edit a note → reindex → verify ContextPane returns related notes.
  - Run full Vitest suite; confirm no v1 regressions.
  - _Requirements: 1.1, 2.6, 2.8_
  - **Validates: Requirements 1.1, 1.2, 1.3, 1.5, 2.1, 2.6, 2.8**

---

### Phase 2 — Search & Navigation

- [x] 9. Implement `src/shared/search-query.ts` (AST_Walk_Query)
  - Parse `operator:value` tokens (`path`, `tag`, `line`, `content`, `file`, `property`, `regex:`) and bare terms.
  - AND-combine operators; serve `path:`/`tag:`/`file:`/`property:` from index membership and `line:`/`content:`/regex from positions + line snippets.
  - Return `SearchResult[]` with snippet + match ranges.
  - _Requirements: 3.2, 3.3, 3.4, 3.5, 3.6, 3.8_

- [x] 10. Add `search:query` IPC channel
  - Zod `SearchQuerySchema`/`SearchResponseSchema`; handler dispatches to `search-query.ts` over the active vault's extended index + `getAST`.
  - Register in `channels.ts`, `schemas.ts`, `ipc.ts`, preload.
  - _Requirements: 3.11, 26.1_

- [x] 11. Build `SearchPanel.tsx`
  - Dedicated panel (Cmd+Shift+F and Command Palette entry); renders results with name, relative path, highlighted snippet.
  - Full keyboard navigation (arrows/Enter/Esc); preserves last query in session state; empty/no-results states.
  - _Requirements: 3.1, 3.7, 3.9, 3.10_

- [x] 12. Implement `src/renderer/src/utils/fuzzy.ts`
  - Shared fuzzy ranker for Quick Switcher and Command Palette; name > path > alias weighting; deterministic ordering.
  - _Requirements: 4.2, 5.7_

- [x] 13. Build `QuickSwitcher.tsx`
  - Cmd+O modal; fuzzy-matches name/path/aliases; recents on empty query; debounced ranking; opens in active pane on Enter.
  - Register "Go to note…" command for the palette.
  - _Requirements: 4.1, 4.3, 4.4, 4.5, 4.6, 4.7, 4.8_

- [x] 14. Implement `src/renderer/src/commands/registry.ts`
  - `registerCommand`/`getCommands` extension point; feature modules register at import time.
  - Seed the registry with v1 actions (toggle edit/view, toggle graph, toggle search, open settings, create note) plus reindex, open switcher, daily note, random note.
  - _Requirements: 5.2, 5.3, 5.6_

- [x] 15. Build `CommandPalette.tsx`
  - Cmd+P modal reading `getCommands()`; fuzzy-filter by label/keywords/id; runs command on Enter.
  - Keyboard + mouse accessible.
  - _Requirements: 5.1, 5.4, 5.5_

- [x] 16. Add Outgoing Links panel to NoteView
  - Compute from `graphEdges.filter(e => e.source === currentFile)`; dedupe by target; broken-link indicator; collapsible; hidden when empty; click opens target.
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

- [x] 17. Build `OutlinePanel.tsx`
  - Heading hierarchy from `currentAST`; active-section tracking via IntersectionObserver; click scrolls to heading; empty placeholder; Cmd+Shift+O jump modal.
  - Sidebar panel + Command Palette entry.
  - _Requirements: 7.1, 7.2, 7.3, 7.4, 7.5, 7.6_

- [x] 18. Phase 2 verification
  - Unit tests: `search-query.test.ts`, `fuzzy.test.ts`, command-registry test.
  - Property tests: AND-combination soundness; fuzzy determinism.
  - E2E: operator search returns expected files; Cmd+O opens a note; Cmd+P runs a command; outline navigates.
  - Full Vitest suite green.
  - _Requirements: 3.8, 5.7_
  - **Validates: Requirements 3.1, 3.8, 4.1, 4.2, 5.1, 5.2, 6.1, 7.1**

---

### Phase 3 — Markdown Rendering

- [x] 19. Add `remarkCallouts.ts` plugin
  - Pure plugin producing `callout` nodes from `> [!type]` blockquotes; `-`/`+` collapse suffix; renderer-safe.
  - Add `Callout` type to `shared/types.ts`.
  - _Requirements: 8.1, 8.3, 8.6_

- [x] 20. Render callouts in `renderNode`
  - Map calloutType → icon + colour (note/info/tip/success/warning/danger/error/question/example/quote/abstract); render body as full markdown; unknown type falls back to `note`.
  - Round-trip via `denormalizeNode`.
  - _Requirements: 8.2, 8.4, 8.5, 8.7_

- [x] 21. Add math rendering (KaTeX)
  - Add `remark-math` to the pipeline (main + renderer); `inlineMath`/`math` branches in `renderNode` using `katex.renderToString`; import KaTeX CSS once; graceful parse-error fallback.
  - Round-trip via `denormalizeNode`; do not parse math inside code.
  - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5, 9.6_

- [x] 22. Add `MermaidBlock.tsx`
  - In `renderNode`, route `code` with `lang === 'mermaid'` to `MermaidBlock`; lazy-load `mermaid`; loading state; parse-error message + source; theme-aware re-render; serialise SVG for HTML export.
  - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5, 10.6_

- [x] 23. Add embeds (`remarkEmbeds.ts` + renderer)
  - Recognise `![[target]]`; resolve image → data-URI `<img>` via `asset:read`; note → transcluded AST with depth cap; broken-embed indicator; vault-root path containment in `asset:read`; round-trip.
  - _Requirements: 11.1, 11.2, 11.3, 11.4, 11.5, 11.6, 11.7_

- [x] 24. Add block-reference parsing (`remarkBlockRefs.ts`)
  - Trailing `^id` on blocks; `[[note#^id]]` link form; `blockRefs` index build + incremental update.
  - _Requirements: 20.1, 20.2, 20.5, 20.6_

- [x] 25. Render block-reference clicks
  - Click opens target note and scrolls/highlights the block; broken-reference indicator.
  - _Requirements: 20.3, 20.4_

- [x] 26. Phase 3 verification
  - Unit tests: `remarkCallouts.test.ts`, `remarkEmbeds.test.ts`, `remarkBlockRefs.test.ts`, math round-trip.
  - Property tests: callout round-trip; math round-trip; embed path containment.
  - E2E: a note with callout + math + mermaid + embed + block-ref renders all five.
  - Full Vitest suite green.
  - _Requirements: 8.5, 9.4, 11.6_
  - **Validates: Requirements 8.1, 8.5, 9.1, 10.1, 11.1, 11.6, 20.1**

---

### Phase 4 — Metadata & Properties

- [x] 27. Build `PropertiesView.tsx`
  - Render frontmatter `yaml` node as a two-column table (replaces the silent skip in `renderNode`); inline edit string/number/boolean/list; add/remove property; "Add properties" when none.
  - Use the `yaml` npm library for parse/serialise (preserving unknown keys).
  - _Requirements: 12.1, 12.2, 12.3, 12.5, 12.6_

- [x] 28. Add `properties:read` / `properties:write` IPC channels
  - Zod schemas; handler rewrites only the frontmatter block under the Pending_Write_Lock; invalid YAML is rejected pre-write.
  - Reload + warn on external frontmatter change while the view is open.
  - _Requirements: 12.4, 12.7, 12.8, 13B.1, 13B.2, 13B.3, 26.1_

- [x] 29. Add raw-YAML toggle
  - Allow switching between Properties table and raw-YAML edit at any time.
  - _Requirements: 12.7_

- [x] 30. Implement property search
  - `property:name:value` operator in SearchPanel using `Extended_SearchIndex.propertyIndex`; bare `name:value` for unambiguous keys; list values indexed element-wise.
  - Clicking a property value in PropertiesView filters results.
  - _Requirements: 13.1, 13.2, 13.3, 13.4, 13.5_

- [x] 31. Inline tags + nested tag pane
  - Render inline `#tag` as clickable chips (skip inside code); hierarchical tag pane with namespaced nesting + counts; parent-tag namespace filtering; unified tag index.
  - _Requirements: 14.1, 14.2, 14.3, 14.4, 14.5, 14.6_

- [x] 32. Wire aliases into resolution paths
  - Wiki-link resolver, graph edge builder, and Quick Switcher consult `aliasIndex`; shortest-path tie-break + ambiguity surfacing; incremental alias re-index.
  - _Requirements: 15.1, 15.2, 15.3, 15.4, 15.5, 15.6_

- [x] 33. Edit aliases via Properties view
  - Aliases as editable list; preserve original syntax when unedited; re-index after write.
  - _Requirements: 15B.1, 15B.2, 15B.3_

- [x] 34. Implement auto-properties
  - On `note:create`, inject `created` (ISO) if absent; on `note:save`, update `modified`; settings flag `autoProperties` (default on); create minimal frontmatter when none.
  - Pending_Write_Lock honoured; no re-parse storm.
  - _Requirements: 16.1, 16.2, 16.3, 16.4, 16.5, 16.6_

- [x] 35. Phase 4 verification
  - Unit tests: `properties.test.ts`, `auto-properties.test.ts`, alias-resolution test.
  - Property tests: Properties round-trip; alias resolution consistency; auto-property idempotence.
  - E2E: edit a property in Properties view, reload, verify round-trip; property search returns matches; inline tag chip filters tree.
  - Full Vitest suite green.
  - _Requirements: 12.5, 15.2, 16.3_
  - **Validates: Requirements 12.1, 12.5, 13.1, 14.1, 15.2, 16.1**

---

### Phase 5 — Quick Wins & Utilities

- [x] 36. Implement daily notes
  - `note:daily` command; configurable `dateFormat`/`folder`/`template` in Settings; create-if-missing using template substitution + note-create; register in Command Palette.
  - _Requirements: 17.1, 17.2, 17.3, 17.4, 17.5, 17.6_

- [x] 37. Implement favorites
  - Per-vault `.nabu/favorites.json`; sidebar Favorites section; toolbar toggle + context-menu toggle; cleanup on rename/delete; Command Palette command.
  - _Requirements: 18.1, 18.2, 18.3, 18.5, 18.6_

- [x] 38. Implement bookmarks
  - Named bookmark lists per-vault (`.nabu/bookmarks.json`); sidebar collection; cleanup on rename/delete; Command Palette command.
  - _Requirements: 18.4, 18.5, 18.6_

- [x] 39. Implement random note
  - `note:random` returns one path from the active vault file list (respecting tag filter); empty-vault guard; Command Palette registration.
  - _Requirements: 19.1, 19.2, 19.3, 19.4_

- [x] 40. Implement Note Composer
  - Multi-select merge dialog; `## <name>` headings; union tags; warn on scalar conflicts; archive/delete originals optional; preview before write; Pending_Write_Lock via note-create path.
  - _Requirements: 21.1, 21.2, 21.3, 21.4, 21.5_

- [x] 41. Phase 5 verification
  - Unit tests: daily-note path derivation, favorites/bookmarks persistence + cleanup, composer merge.
  - E2E: open daily note (create then reopen), favorite a note and verify it persists, run random note, compose two notes.
  - Full Vitest suite green.
  - _Requirements: 17.4, 18.5, 21.4_
  - **Validates: Requirements 17.1, 18.1, 19.1, 21.1**

---

### Phase 6 — Core Plugin Parity (9 Obsidian Features)

These 9 features correspond to Obsidian core plugins (R27–35). Each is independent, additive, and ships behind its own toggle. They are grouped into a single phase because none depends on the architectural lifts (multi-vault, live preview, split panes).

- [x] 42. Implement Unique Note Creator (R27)
  - Command "Create unique note" with configurable timestamp format (default `YYYYMMDDHHmmss`); injects filename as `title` frontmatter + H1; uses template system with `_templates/unique-note.md` fallback; Command Palette registration.
  - _Requirements: 27.1, 27.2, 27.3, 27.4, 27.5_

- [x] 43. Implement Page Preview on Hover (R28)
  - On hover over resolved `wikiLink` or `embed` in view mode, show popover with rendered excerpt of target note (same `renderNode` pipeline, truncated); configurable hover delay (default 300ms); "Open" link in popover; no preview on broken links; disabled when user turns off in Settings.
  - _Requirements: 28.1, 28.2, 28.3, 28.4, 28.5, 28.6, 28.7, 28.8_

- [x] 44. Implement File Recovery via Snapshots (R29)
  - Auto-snapshot note to `.nabu/snapshots/<path>/` before each save; per-note cap (50) and per-vault cap (1000); "File recovery" panel listing snapshots with diff preview; "Restore this version" and "Open as new note" actions; pre-restore snapshot before restore; configurable on/off in Settings; `.nabu/snapshots/` added to `.gitignore` automatically; async via AsyncQueue.
  - _Requirements: 29.1, 29.2, 29.3, 29.4, 29.5, 29.6, 29.7, 29.8, 29.9_

- [x] 45. Implement Slash Commands (R30)
  - Typing `/` at line start (or after whitespace) in edit mode shows inline autocomplete menu; fuzzy-filter commands; insert template on selection; registry with heading/bullet list/numbered list/task list/callout/code block/math block/table/hr/embed commands; extensible registry; Esc to dismiss; no trigger inside code blocks; also works in Live Preview (CodeMirror).
  - _Requirements: 30.1, 30.2, 30.3, 30.4, 30.5, 30.6, 30.7_

- [x] 46. Implement Footnotes View (R31)
  - Parse `[^label]` / `[^label]:` footnote syntax; inline references as clickable superscript links; Footnotes sidebar panel listing definitions; click reference scrolls to definition and vice versa; hidden when no footnotes; round-trip on save; no parsing inside code blocks.
  - _Requirements: 31.1, 31.2, 31.3, 31.4, 31.5, 31.6_

- [x] 47. Implement Format Converter — Import from Notion/Roam/Evernote (R32)
  - "Import notes" command (Command Palette + Settings); support Notion JSON/ZIP export, Roam JSON, Evernote ENEX; convert to Nabu markdown preserving headings/lists/bold-italic/code/links/images; map source metadata to frontmatter; dry-run preview before write; filename conflict handling (rename/skip/overwrite); main-process async with progress IPC pushes; modular per-format in `src/main/importers/`.
  - _Requirements: 32.1, 32.2, 32.3, 32.4, 32.5, 32.6, 32.7, 32.8, 32.9_

- [x] 48. Implement Bases — Database Views (R33)
  - Base View renders note collection as sortable/filterable table (each row = note, columns = frontmatter properties); user-selectable columns with drag-and-drop reorder; sort by column asc/desc; filter by column value; inline property editing via `properties:write`; multiple view types (table/board-kanban/gallery-card); query-defined (tag/folder/property condition); config persistable as named base in `.nabu/bases.json`; reflective updates when matching notes change.
  - _Requirements: 33.1, 33.2, 33.3, 33.4, 33.5, 33.6, 33.7, 33.8_

- [x] 49. Implement Web Viewer (R34)
  - External link click opens in embedded browser view (BrowserView or `<webview>`) instead of system browser; navigation controls (back/forward/reload/open in system browser); dismissible (close button/Esc); Settings option "Open links in" (Nabu web viewer vs system browser, default system); clear cookies/session state on close; block `file://` and `localhost` URLs.
  - _Requirements: 34.1, 34.2, 34.3, 34.4, 34.5, 34.6_

- [x] 50. Implement Audio Recorder (R35)
  - "Insert audio recording" command (edit mode + Command Palette); record from default input with timer indicator; save as `.mp3`/`.ogg` to configurable assets dir (default `.nabu/audio/`); insert `![[recording.mp3]]` at cursor; render as `<audio controls>` via asset bridge; non-blocking async recording; graceful error on no microphone / permission denied.
  - _Requirements: 35.1, 35.2, 35.3, 35.4, 35.5, 35.6, 35.7_

- [x] 51. Phase 6 verification
  - Unit tests for each of the 9 features (parser/serializer logic, command registration, persistence).
  - E2E: unique note creates properly named file; page preview appears on hover; file recovery snapshots and restores; slash commands insert correct syntax; footnotes render and navigate; format converter imports a fixture; bases view displays properties; web viewer opens external URL; audio recorder creates and embeds recording.
  - Full Vitest suite green.
  - **Validates: Requirements 27.1, 28.1, 29.1, 30.1, 31.1, 32.1, 33.1, 34.1, 35.1**

---

### Phase 7 — Format Import & Feature Toggles

- [x] 52. Implement format import: PDF via `pdfjs-dist` (R36)
  - Bundle `pdfjs-dist`; `pdf-importer.ts` in `src/main/importers/`; extract text content from PDF pages; convert to markdown (heading detection by font size, paragraph grouping, basic list detection); save as `.md` with frontmatter preserving original filename and `source_format: pdf`; progress reporting; error handling for encrypted/corrupt PDFs.
  - _Requirements: 36.1, 36.4, 36.5, 36.6, 36.8, 36.9_

- [x] 53. Implement format import: DOCX via `mammoth.js` + CSV via `xlsx` (R36)
  - Bundle `mammoth.js` and `xlsx`; `docx-importer.ts` converts styled DOCX to markdown (bold/italic/headings/lists/tables preserved); `csv-importer.ts` renders CSV data as GFM markdown tables; same frontmatter, progress, error conventions as task 52.
  - Invocable from Command Palette ("Import file...") and "File > Import" menu via native file-open dialog filtered by supported extensions.
  - _Requirements: 36.2, 36.3, 36.4, 36.5, 36.6, 36.7, 36.8, 36.9_

- [x] 54. Build the feature toggle registry + Settings UI (R37)
  - Implement `src/shared/feature-toggles.ts` with registry pattern: each toggleable feature registers `{ id, label, description, setup: () => void, teardown: () => void }`.
  - Add `settings:getFeatureToggles` / `settings:setFeatureToggle` IPC channels with Zod schemas.
  - Settings panel gets an "Optional Features" section rendering every registered toggle as a labelled switch.
  - On toggle off: unregister commands from palette, hide panels, remove keyboard shortcuts, call `teardown()`.
  - On toggle on: register commands, show panels, add shortcuts, call `setup()`.
  - _Requirements: 37.1, 37.4, 37.5, 37.6, 37.8_

- [x] 55. Register all optional features in the toggle system (R37)
  - Register: Daily Notes, Templates, Random Note, Unique Note Creator, Slash Commands, Page Preview, Audio Recorder, Word Count, File Recovery, Format Converter, Format Import.
  - Each registration wires the feature's existing Command Palette entries, panels, and shortcuts into the toggle lifecycle.
  - Persist default-off for new features; default-on for established v1 features (templates, word count).
  - Features that create files on activation (Daily Notes, Unique Note Creator) prompt on first enable rather than acting silently.
  - _Requirements: 37.2, 37.3, 37.7, 37.9_

- [x] 56. Phase 7 verification
  - Unit tests: each importer round-trips a fixture; feature toggle registry setup/teardown callbacks fire correctly.
  - E2E: import a PDF, DOCX, and CSV → verify they become .md notes; toggle a feature off → verify its commands disappear from the palette; toggle back on → verify commands reappear.
  - Full Vitest suite green.
  - **Validates: Requirements 36.1, 36.2, 36.3, 37.1, 37.3, 37.4, 37.6**

---

### Phase 8 — Multi-Vault

- [x] 57. Add `recentVaults` to `AppSettings`
  - `Array<{ path, name, lastOpened }>` capped to a bounded size; migration keeps the v1 `lastVaultPath` as the first entry.
  - _Requirements: 22.1, 22.10_

- [ ] 58. Build `Vault_Registry` + `VaultSession`
  - `src/main/vault-registry.ts`; per-vault `StateManager`/`VectorManager`/`VaultWatcher`; make v1 singletons delegate to the active session.
  - _Requirements: 22.2, 22.10_

- [ ] 59. Thread `vaultId` through content IPC handlers
  - Add `vaultId` (defaulting to active when omitted) to content channels; Zod-validate on every channel; dispatch via the registry.
  - _Requirements: 22.3, 22.9, 26.1_

- [ ] 60. Add multi-vault renderer state
  - `openVaults`/`activeVaultId` in `AppState`; sidebar/tree/noteview reflect the active vault; vault switcher (sidebar header + Command Palette).
  - _Requirements: 22.4, 22.5_

- [ ] 61. Implement vault open/close lifecycle
  - Open/create vault → instantiate session; switch active vault → renderer swap; close vault → stop watcher + release state.
  - _Requirements: 22.5, 22.6_

- [ ] 62. Replace "Open in New Window" placeholder
  - Open the chosen vault in a second `BrowserWindow` backed by the same registry.
  - _Requirements: 22.7_

- [ ] 63. Setup wizard + auto-restore for multi-vault
  - Wizard renders when `openVaults.length === 0`; auto-restore reopens the most recently active vault set on launch.
  - _Requirements: 22.8_

- [ ] 64. Phase 8 verification
  - Unit tests: `vault-registry.test.ts` (open/close, id dispatch, recents cap).
  - Property tests: vault isolation; multi-vault IPC validation.
  - E2E: open a second vault, switch active, verify independent indexes; open in new window.
  - Full Vitest suite green.
  - _Requirements: 22.3, 22.6_
  - **Validates: Requirements 22.1, 22.2, 22.3, 22.6, 22.9**

---

### Phase 9 — Live Preview

- [ ] 65. Extract shared `buildProcessor` to `src/shared/markdown.ts`
  - Single source of truth for the plugin pipeline used by main and renderer.
  - _Requirements: 23.3, 23.7_

- [ ] 66. Build the Renderer_Remark_Pipeline
  - `src/renderer/src/markdown/pipeline.ts` using the shared `buildProcessor`; bundle `remark-parse` + plugins + `remark-math` + callouts into the renderer bundle.
  - _Requirements: 23.3_

- [ ] 67. Add CodeMirror 6 editor
  - `@uiw/react-codemirror` + `@codemirror/lang-markdown` + a Nabu theme; replace the `<textarea>` at `NoteView.tsx:835` for Live Preview mode.
  - _Requirements: 23.1, 23.2_

- [ ] 68. Render inline on debounced doc change
  - Re-parse via the renderer pipeline; render visible regions inline (headings, bold, links, wiki-links, code, lists, callouts, math, task checkboxes).
  - _Requirements: 23.4, 23.6, 23.7_

- [ ] 69. Generalise mode-switch + save coupling
  - Toggling out of Live Preview flushes the in-memory doc through `note:save` (no disk refetch); malformed regions fall back to raw source.
  - Auto-save + external-edit detection continue via the Pending_Write_Lock + watcher.
  - _Requirements: 23.5, 23.8, 23.9_

- [ ] 70. Phase 9 verification
  - Unit tests: renderer-pipeline parity with main pipeline on a fixture set.
  - Property tests: Live Preview source preservation (toggle out == source unchanged).
  - E2E: type a wiki-link in Live Preview and see it render inline; switch modes without data loss; external edit reloads.
  - Full Vitest suite green.
  - _Requirements: 23.5, 23.7_
  - **Validates: Requirements 23.1, 23.3, 23.4, 23.5, 23.7**

---

### Phase 10 — Split Panes & Workspaces

- [ ] 71. Convert `currentFile` to `openTabs`/`activeTabId`
  - `Tab = { id, path, ast, raw, mode, scrollTop, cursor }`; keep `currentFile` as a compat alias for `openTabs[activeTabId]?.path`.
  - Generalise per-note flows (save, task toggle, external edit, properties) to take a `tabId`.
  - _Requirements: 24.1, 24.8_

- [ ] 72. Build `PaneLayout.tsx`
  - `paneLayout: 'single'|'split-horizontal'|'split-vertical'|'grid'`; each pane bound to an open tab; independent scroll/mode/cursor; active pane visually distinguished.
  - _Requirements: 24.2, 24.4, 24.5_

- [ ] 73. Pane open/close/drag interactions
  - Cmd-click / "Open in new pane"; close panes individually; drag text between panes inserts at cursor.
  - Command Palette commands: Close tab, Next pane, Move tab.
  - _Requirements: 24.3, 24.6, 24.7_

- [ ] 74. Implement Workspaces
  - Per-vault `.nabu/workspaces.json`; save current `openTabs`+`paneLayout` as a named workspace; load restores (skip + warn on missing notes); switcher in sidebar + Command Palette.
  - _Requirements: 25.1, 25.2, 25.3, 25.4, 25.5, 25.6_

- [ ] 75. Implement folder-based tab groups (Chrome-style)
  - When notes from the same folder are opened, visually group their tabs with a Chrome-style color-coded group label and underline.
  - Tab groups are collapsible (expand/collapse all tabs in a group), reorderable (drag entire group), and inheritable (subfolders nest or inherit parent color).
  - Group colors cycle through a palette (blue, red, green, yellow, purple, orange, cyan, pink) assigned deterministically by folder path.
  - Root-level notes get no group; single-tab groups collapse to a plain tab (no group label shown).
  - Group state persisted per-vault in `.nabu/tab-groups.json` alongside `workspaces.json`.
  - _Requirements: 24.9 [NEW]_

- [ ] 76. Phase 10 verification
  - Unit tests: tab/pane reducer transitions; workspace serialise/deserialise; tab group color assignment + collapse/expand.
  - Property tests: pane independence; workspace restore safety; tab group determinism.
  - E2E: open two panes, edit in one, verify the other is unaffected; open notes from `projects/` → they appear as a blue tab group; save and restore a workspace.
  - Full Vitest suite green.
  - _Requirements: 24.4, 25.5, 24.9_
  - **Validates: Requirements 24.1, 24.4, 24.9, 25.1, 25.5**

---

### Phase 11 — Hardening, E2E & Release

- [ ] 77. Cross-cutting JSDoc + requirement traceability sweep
  - Ensure every new/modified source file cites `Requirements: N.M` in its header; verify no v1 requirement citations were dropped.
  - _Requirements: 26.3_

- [ ] 78. Security review
  - Confirm no v2 feature enables `allow-same-origin`, `nodeIntegration`, or asset access outside the vault root; confirm `contextIsolation` remains on.
  - _Requirements: 26.4_

- [ ] 79. Error-handling sweep
  - Verify no v2 feature silently swallows errors; user-facing failures show messages; developer-facing failures log with context.
  - _Requirements: 26.7_

- [ ] 80. Theme + string audit
  - Verify all new UI reads CSS variables (no hard-coded colours) and all user-facing strings are plain text.
  - _Requirements: 26.6_

- [ ] 81. Documentation update
  - Update README (feature list, roadmap: move multi-vault/advanced-search/live-preview/split-panes out of "v2 planned"), ARCHITECTURE (Vault_Registry, Renderer_Remark_Pipeline, PaneLayout, feature toggle registry, tab groups), CHANGELOG.
  - _Requirements: 26.8_

- [ ] 82. Full regression sweep
  - Run the entire Vitest suite + all Playwright e2e specs; confirm the v1 correctness properties still hold and the v2 properties pass.
  - _Requirements: 26.5_
  - **Validates: Requirements 26.1, 26.2, 26.3, 26.4, 26.5, 26.6, 26.7, 26.8**

---

## Task Dependency Graph

```json
{
  "waves": [
    { "id": 0, "tasks": ["1", "2", "6"], "note": "Phase 1 foundations: vector wiring + extended index module (parallel-safe)" },
    { "id": 1, "tasks": ["3", "4", "5", "7"], "note": "Phase 1 IPC + integration; depends on wave 0" },
    { "id": 2, "tasks": ["8"], "note": "Phase 1 verification gate" },

    { "id": 3, "tasks": ["9", "12", "14", "16", "24"], "note": "Phase 2/3 pure modules: search-query, fuzzy, command registry, outgoing links, block-ref parser (parallel-safe; search-query depends on wave 0's extended index)" },
    { "id": 4, "tasks": ["10", "11", "13", "15", "17", "19", "21", "23", "25"], "note": "Phase 2/3 UI + IPC; depends on wave 3" },
    { "id": 5, "tasks": ["20", "22"], "note": "Phase 3 renderers depending on their plugins (callout render, mermaid)" },
    { "id": 6, "tasks": ["18", "26"], "note": "Phase 2/3 verification gates" },

    { "id": 7, "tasks": ["27", "28", "31", "36", "37", "38", "39", "40"], "note": "Phase 4/5 features (properties, inline tags, daily, favorites, bookmarks, random, composer); properties depends on extended index" },
    { "id": 8, "tasks": ["29", "30", "32", "33", "34"], "note": "Phase 4 metadata follow-ons (raw toggle, property search, aliases, auto-properties)" },
    { "id": 9, "tasks": ["35", "41"], "note": "Phase 4/5 verification gates" },

    { "id": 10, "tasks": ["42", "43", "44", "45", "46", "47", "48", "49", "50"], "note": "Phase 6 core plugins (9 parallel-safe features; each is independent; page preview depends on extended index for snippet extraction)" },
    { "id": 11, "tasks": ["51"], "note": "Phase 6 verification gate" },

    { "id": 12, "tasks": ["52", "53", "54"], "note": "Phase 7 foundations: format importers + feature toggle registry (parallel-safe)" },
    { "id": 13, "tasks": ["55"], "note": "Phase 7 feature registration (depends on toggle registry from wave 12)" },
    { "id": 14, "tasks": ["56"], "note": "Phase 7 verification gate" },

    { "id": 15, "tasks": ["57", "58", "65"], "note": "Phase 8/9 foundations: recentVaults, Vault_Registry, shared buildProcessor (parallel-safe across phases)" },
    { "id": 16, "tasks": ["59", "60", "66"], "note": "Phase 8/9 wiring: vaultId IPC, renderer multi-vault state, renderer pipeline" },
    { "id": 17, "tasks": ["61", "62", "63", "67"], "note": "Phase 8/9 lifecycle + CodeMirror editor" },
    { "id": 18, "tasks": ["68", "69"], "note": "Phase 9 live-render + mode coupling" },
    { "id": 19, "tasks": ["64", "70"], "note": "Phase 8/9 verification gates" },

    { "id": 20, "tasks": ["71"], "note": "Phase 10 foundation: openTabs/activeTabId conversion" },
    { "id": 21, "tasks": ["72", "73"], "note": "Phase 10 PaneLayout + interactions; depends on wave 20" },
    { "id": 22, "tasks": ["74"], "note": "Phase 10 Workspaces; HARD depends on Split Panes (wave 21)" },
    { "id": 23, "tasks": ["75"], "note": "Phase 10 Tab Groups; depends on openTabs system (wave 20) + PaneLayout (wave 21)" },
    { "id": 24, "tasks": ["76"], "note": "Phase 10 verification gate" },

    { "id": 25, "tasks": ["77", "78", "79", "80"], "note": "Phase 11 hardening + docs (parallel-safe)" },
    { "id": 26, "tasks": ["81", "82"], "note": "Phase 11 verify: first traceability/security/docs (wave 25), then full regression sweep" }
  ]
}
```

---

## Notes

- **Phase independence**: Phases 2–5 do not depend on each other (only on Phase 1). Phase 6 (core plugins) is fully independent and can run in parallel with Phases 2–5 or 7. Phases 8, 9, and 10 (Split Panes half) are mutually independent; only Workspaces (task 74) hard-depends on Split Panes. This means teams could parallelise Phases 6, 8, and 9.
- **v1 compatibility shims**: tasks 58, 59, and 71 deliberately keep v1 single-vault / single-note flows working during migration (compat aliases `vault`, `currentFile`; default-when-omitted `vaultId`). Do not remove the shims until Phase 11 confirms no caller depends on them.
- **The vector-search fix is Phase 1, tasks 1–5**: it is the highest-impact, lowest-risk item in v2 because it makes an already-shipped feature work. It should land first and can ship alone.
- **Test mandate**: every phase's final task is a verification gate that runs the full Vitest suite plus phase-specific unit/property/e2e tests, per `Requirement 26.2`/`26.5`. Do not mark a phase complete until its gate passes.
- **Git discipline**: after every individual task is completed (all code written, tests pass), commit with a descriptive message and push. Follow conventional commit format: `type: task description` (e.g. `feat: implement slash commands with inline autocomplete`). Do not batch tasks into a single commit. This ensures the commit log serves as a granular, auditable build history.
- **Feature toggles (Phase 7) should ship early enough** that Phase 6 features (core plugins) can be registered behind them. If Phase 7 is blocked, Phase 6 features can ship with hardcoded defaults (on) and retrofitting toggles later.
- **Naming**: this spec uses the `nabu-` prefix (matching the repo rename in commit `893e2f7`); the legacy `onyx-*` spec folders remain authoritative for v1. Cite v1 requirement numbers only when a v2 task extends a v1 behaviour; otherwise cite v2 `N.M`.
- **Format import (R36)** uses JS-native libraries (pdfjs-dist, mammoth.js, xlsx) bundled with the app, not a Python sidecar. This covers 90%+ of real-world import needs without deployment complexity.
