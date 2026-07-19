# Phase 6.2 — Workflow Repair & Placeholder Replacement (Prompt A)

**Date:** 2026-07-19
**Scope:** Highest-priority broken workflow / placeholder repair (Prompt A of 3)
**Mode:** Code (repair only — no architecture, service-boundary, IPC-contract, or feature-design changes)

---

## 1. Workflow Repair Report

### Repaired workflow: Graph View — "Blocks" mode (Req 38.6, High impact)

**Previous state (broken placeholder):** The Graph View exposed a third mode toggle,
"Blocks", but its implementation was a documented placeholder. Selecting it always
rendered an empty canvas with the message *"Block references are being processed…"*
(or *"Use block references (`^id`) to populate this view"*). No nodes, no edges,
no interaction — the workflow could not complete.

**Repaired behavior (end-to-end):**

| Step | Layer | Action |
|------|-------|--------|
| User Action | Renderer | Click **Blocks** radio in the graph toolbar → `dispatch({ type: 'GRAPH_MODE_CHANGED', payload: 'blocks' })` |
| Renderer | `GraphView.tsx` | `graphMode === 'blocks'` branch builds nodes/edges from `blockNodes`/`blockEdges` state |
| Command/Effect | `GraphView.tsx` | `useEffect` seeds the graph from `state.extendedIndex.blockRefs` (block *definitions*) synchronously, then asynchronously scans note raw content |
| IPC | `note:get-raw` | For each vault note, `window.electron.note.getRaw(path)` returns raw markdown (existing, unchanged contract) |
| Service | `extended-indexing.ts` (existing) | `blockRefs` already records `^id` definitions during indexing |
| Persistence/Index | `state.ts` (existing) | `extendedIndex` is built and pushed to the renderer via `index:build` |
| UI Update | `GraphView.tsx` | Block nodes drawn as squares, note nodes as circles; edges drawn; click opens the owning note; empty/loading states shown gracefully |

The graph now shows:
- One node per note that **defines** a block (`^id` marker) and one node per **block** (`path#^id`), with a hierarchy edge note → block.
- **Cross-note reference edges** for every `[[Note#^id]]` (or `![[Note#^id]]`) link found in any note, resolved to the target block node (wiki-link basename resolution, consistent with the rest of the app).
- Clicking a block or note node opens the owning note in the editor and closes the graph (same UX as Files mode).
- A loading message while scanning, and a clear instructional empty state when no blocks are defined.

**Validation:** Unit tests added in `tests/unit/graph-utils.test.ts` cover `computeBlockGraph` (definition nodes, hierarchy edges, cross-note reference edges, undefined-block filtering, empty input) and `extractBlockRefLinks` (single, embed-style, dedupe, none). All 23 graph-utils tests pass.

---

## 2. Placeholder Replacement Report

### Placeholder: Graph "Blocks" mode (CHANGELOG: *"placeholder for future block reference visualization"*)

| Field | Detail |
|-------|--------|
| **Previous implementation** | `GraphView.tsx` lines 136–147 returned empty `nodes`/`links` for both the "no block refs" and "has block refs" branches; `renderBlocksPlaceholder()` rendered only a static text overlay. `computeTagGraph` existed for Tags mode but no equivalent for blocks. |
| **Replacement** | Added `computeBlockGraph()` and `extractBlockRefLinks()` to `src/shared/graph-utils.ts` (pure, I/O-free, matching the existing `computeTagGraph` style). Wired them into `GraphView.tsx`: a new `useEffect` seeds block-definition nodes from `state.extendedIndex.blockRefs` and asynchronously derives cross-note reference edges by scanning raw note content via the **existing** `note:get-raw` IPC (with a `useRef` cache to avoid refetching). Block nodes render as squares; clicking opens the owning note. |
| **Rationale** | The data needed (block definitions) was already produced by the existing indexer; only the visualization layer was missing. Reusing `note:get-raw` (an existing, unchanged IPC contract) and the existing `extendedIndex` payload avoids any new service, new IPC channel, or architecture change — fully within Phase 6.2 scope. The implementation is deterministic and unit-tested. |

No other placeholders were found in the highest-priority set. The Phase 6.1 matrix reported all 83 features as *Working*; the only concrete placeholder discovered in code/CHANGELOG was the Blocks graph mode, which is now fully implemented.

---

## 3. Deferred Placeholder Report

No placeholders were intentionally deferred.

The Blocks-mode implementation is complete for the data currently available. One **non-blocking limitation** is documented here for transparency (not a deferred placeholder, since the workflow is fully functional):

- **Incremental refresh:** The block-reference scan runs when the user switches into Blocks mode (and re-runs if `extendedIndex` or the vault file list changes). It does not live-recompute on every keystroke. This matches the existing Files/Tags modes, which also rebuild from the last-pushed index. If live block-reference updates during editing become a requirement, that would be a Phase 6.3/feature enhancement, not a repair.

All other features inspected (vault, file tree, notes, PDF, search, settings, dictation, widgets, kanban, properties, tags, backlinks, task lists, embeds, OCR, templates, daily/random notes, vector search, watcher, slash commands, find/replace, block refs parsing, DOCX import, workspace, tabs, theme, setup wizard, command/quick switcher, format converter, file recovery, etc.) are implemented and contain no placeholder/TODO/stub code in the highest-priority tier.

---

## 4. Files Modified

| File | Change |
|------|--------|
| `src/shared/graph-utils.ts` | Added `BLOCK_REF_LINK_RE`, `BlockGraphNode`, `BlockGraphEdge`, `computeBlockGraph()`, `extractBlockRefLinks()` (pure graph-building utilities for Req 38.6). |
| `src/renderer/src/features/graph/GraphView.tsx` | Replaced the empty blocks branch with real node/edge construction; added `blockNodes`/`blockEdges`/`blockGraphLoading` state; added an async effect that seeds from `extendedIndex.blockRefs` and scans raw content via `note:get-raw`; added `isBlock` to `D3Node`; render block nodes as squares; updated `handleClick` for blocks mode (opens owning note); replaced `renderBlocksPlaceholder` with loading/empty states; updated header comment. |
| `tests/unit/graph-utils.test.ts` | Added `extractBlockRefLinks` and `computeBlockGraph` test suites (10 new tests). |
| `CHANGELOG.md` | Updated the Phase 12 "Blocks View mode" entry from "placeholder" to a description of the implemented block-reference visualization. |

No IPC contracts, service boundaries, shared schemas, or renderer architecture were modified. The only IPC used (`note:get-raw`) is pre-existing.

---

## 5. Verification Summary

### Build status (Gate A)
- `npm run typecheck:web` → **0 errors, 0 warnings** (the renderer + shared layer, which contains all changes).
- `npm run typecheck:node` → 31 `error TS` are present **on the unmodified baseline** (verified via `git stash`): all are `Cannot find module 'electron'` / `Property 'resourcesPath' does not exist on type 'Process'` in `src/main/**` and `src/preload/**` — a pre-existing environment issue (the `electron` native module/type declarations are not resolvable in this sandbox; `electron/package.json` is absent). **None are introduced by this phase's changes**, which touch only `src/shared` and `src/renderer`.

### Runtime status
- `npm run dev` cannot launch in this sandbox because `electron-vite` fails to resolve `electron/package.json` (same missing-electron environment issue, unrelated to the code changes).
- Logic verification was performed via the unit test suite instead:
  - `npx vitest run tests/unit/graph-utils.test.ts` → **23 passed** (including 10 new tests for the repaired workflow).
  - Full suite: **703 passed**, 11 skipped. The single failing file (`recent-vaults.test.ts`) fails only because it transitively imports `electron` from `src/main/services/settings.ts` — the same pre-existing environment limitation, not a regression.

### Workflow validation (Blocks mode)
- Definition nodes + hierarchy edges: covered by `computeBlockGraph` tests.
- Cross-note reference edges (resolved by basename): covered by `computeBlockGraph` tests.
- Link extraction (`[[Note#^id]]`, `![[Note#^id]]`, dedupe, none): covered by `extractBlockRefLinks` tests.
- Graceful empty/loading UI: implemented and type-checked.
- Click-to-open owning note: implemented, reuses the existing `FILE_LOADED` + `GRAPH_VIEW_TOGGLE` dispatch path used by Files mode.

**Gate A (build passes):** ✅ for the modified layers. The only `typecheck` errors are pre-existing and environment-specific (missing `electron` module in the sandbox), confirmed present before any change.

---

## Success Criteria Checklist
- [x] Highest-priority workflow (Graph Blocks mode) restored.
- [x] The one identified placeholder (Blocks mode) replaced with a real implementation reusing existing services/IPC.
- [x] Remaining placeholders: none in the highest-priority tier; non-blocking limitation documented.
- [x] Gate A passes for all modified code (web typecheck clean; node errors pre-existing/env-only).
- [x] No duplicate implementations, no temporary code, no broken execution paths introduced.
- [x] Deliverables produced (this report).

**Prompt A complete.** Awaiting Prompt B.
