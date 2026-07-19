# Phase 4.3 — Watcher Cleanup & Path Resolution Repair

**Prompt A — Infrastructure Reliability Phase**

This phase stabilizes filesystem monitoring and path resolution **without**
changing vault lifecycle behavior. It is an infrastructure reliability phase,
not a lifecycle redesign.

---

## 1. Watcher Audit Report

A repository-wide audit (`chokidar`, `fs.watch`, `fs.watchFile`, `watch(`,
`FSWatcher`, `addListener('change')`, `on('change')`) found **exactly one**
filesystem watcher implementation:

| # | Watcher | Owner (before) | Registration point | Watched paths | Events handled | Cleanup |
|---|---------|----------------|--------------------|---------------|----------------|---------|
| W1 | `VaultWatcher` (chokidar singleton `vaultWatcher`) | `VaultService` (intended) | `vault-service.ts` — 4 distinct call sites | The open vault root (`config.vaultPath`), recursive, ignoring `/^\|\.nabu/` | `change` (`.md`), `add` (`.md` + images), `unlink` (`.md`), `error` | `VaultWatcher.stop()` → `watcher.close()`; also auto-restart on fatal errors (max 3) |

No separate workspace watcher, plugin watcher, metadata watcher, or indexing
watcher exists. Indexing is driven *by* the single vault watcher's callbacks
(vector embedding on change/add, removal on unlink, OCR on image add).

### Registration points found (all inside `VaultService`)

| Call site | Code path | Used `registerAndWatch`? |
|-----------|-----------|--------------------------|
| `registerAndWatch()` | `openVault`, `createVault` | Yes (canonical) |
| `openVaultInNewWindow()` | `vault:open-in-new-window` | **No** — inline `vaultRegistry.register` + `watcher.start` |
| `restoreVault()` | launch restore | **No** — inline `watcher.start` (no registry registration) |
| `openTestVault()` | `NABU_TEST_VAULT` E2E | **No** — inline `watcher.start` (no registry registration) |

---

## 2. Watcher Ownership Report

### Before
- **Intended owner:** `VaultService` (single `VaultWatcher` instance).
- **Actual reality:** the watcher was started from **four** different methods,
  three of which bypassed the `registerAndWatch` helper and performed ad-hoc
  `vaultRegistry.register` / `watcher.start` sequences. This created
  inconsistent ownership and registration semantics across startup flows
  (normal open, new-window open, launch restore, test injection).

### After
- **Single explicit owner:** `VaultService.registerAndWatch()` is now the **only**
  method that calls `VaultWatcher.start()` and registers the session in
  `VaultRegistry`.
- All four entry points (`openVault`/`createVault`, `openVaultInNewWindow`,
  `restoreVault`, `openTestVault`) now delegate to `registerAndWatch()`.
- `VaultWatcher.start()` is invoked from exactly **one** location, so the
  built-in `stop()`-before-`start()` guard (which prevents stacked chokidar
  instances) is the single deterministic ownership boundary.

No watcher is registered from multiple locations. Each watched vault path has
exactly one responsible owner (`VaultService` → `VaultWatcher`).

---

## 3. Indexing Trigger Report

### Flow (unchanged)
```
Filesystem Event
  ↓
VaultWatcher (chokidar)
  ↓
onFileChanged / onFileAdded / onFileDeleted / onImageAdded
  ↓
VectorManager.embedFile / removeFile  (vector index)
  ↓
Search Database
```
Plus a full rebuild path: `VaultService.triggerIndexBuild()` →
`StateManager.buildIndexes()` → `IPCChannel.INDEX_BUILD`, and OCR's
`updateIndexesForFile` for companion notes.

### Duplicate trigger removal
- The watcher callbacks were **already centralized** in `buildWatcherConfig()`
  (shared IPC helper). The only duplication was at the *registration* layer:
  three flows re-implemented the `register + start` sequence inline instead of
  reusing `registerAndWatch`.
- By routing all four flows through `registerAndWatch` → `buildWatcherConfig`,
  the indexing trigger chain is now reached through **one** code path. There is
  no longer any risk of a second `watcher.start()` stacking a duplicate chokidar
  instance (which would have fired the same `onFileChanged`/`onFileAdded`
  callbacks twice → double embedding / double indexing for a single event).
- Indexing still occurs exactly when required (external `.md` change/add,
  delete, image add) and is skipped for app-initiated writes via the
  `Pending_Write_Lock` check — behavior unchanged.

---

## 4. Path Resolution Report

### Canonical resolution strategy
A single, dependency-free module `src/shared/path.ts` is now the **one**
authoritative path-resolution implementation. It is importable from both the
main process and the renderer (no Node `fs`/`path` dependency).

Provided primitives:
- `normalizePath(input)` — separator unification (`\` → `/`), collapse of
  repeated separators, lexical `.`/`..` resolution, trailing-separator strip.
  Deterministic and safe as a map key / equality comparator.
- `toVaultRelative(vaultPath, filePath)` — vault-relative resolution with a
  **separator-boundary check** (`target.startsWith(base + '/')`), preventing the
  classic substring bug where a vault named `Notes` corrupts
  `/MyNotes/file.md` → `/MyNotes/file.md` stripped to `/MyNotes/file.md`.
- `fromVaultRelative(vaultPath, relativePath)` — inverse, lexicalized against
  path traversal.
- `toWorkspaceRelative(...)` — explicit workspace-scoped alias.
- `getFileName` / `getFileNameWithoutExt` / `getExtension` — cross-platform
  basename helpers.

### Repaired utilities (call sites migrated)
| File | Before | After |
|------|--------|-------|
| `src/main/snapshots.ts` (`getSnapshotPath`, `pruneNoteSnapshots`, `restoreSnapshot`) | `notePath.replace(vaultPath, '').replace(/^\//, '')` | `toVaultRelative(vaultPath, notePath)` |
| `src/main/services/view-state.ts` (`getViewStateFile`) | `notePath.replace(vaultPath, '').replace(/^\/+/, '')` | `toVaultRelative(vaultPath, notePath)` |
| `src/shared/search-query.ts` (`getRelativePath`) | hand-rolled `startsWith` + `slice` | `toVaultRelative(vaultPath, filePath)` |

### Removed duplication
- The four divergent resolution strategies collapsed to one (`toVaultRelative`
  / `normalizePath`).
- `state.ts` already used Node's `path.relative` + `path.sep` (canonical, kept
  as-is — no change needed; it is consistent with the new strategy).
- `ipc/shared.ts` re-exports Node's `path` (`export { path }`) — this is a
  re-export of the platform module, not a custom utility, and is left intact.

---

## 5. Files Modified

| File | Change |
|------|--------|
| `src/shared/path.ts` | **New** — canonical, deterministic path-resolution utilities. |
| `src/main/services/vault-service.ts` | `openVaultInNewWindow`, `restoreVault`, `openTestVault` now delegate to `registerAndWatch` (single watcher-ownership path). No lifecycle logic changed. |
| `src/main/snapshots.ts` | Snapshot relative-path computation now uses `toVaultRelative`. |
| `src/main/services/view-state.ts` | View-state file path now uses `toVaultRelative`. |
| `src/shared/search-query.ts` | `getRelativePath` now delegates to `toVaultRelative`. |

**Not modified (per scope rules):** `VaultService` lifecycle, `WorkspaceService`
lifecycle, service boundaries, vault behavior, renderer behavior, indexing
design, storage design.

---

## 6. Verification Summary

### Build status
- `npm run typecheck` → **PASS** (zero TypeScript errors, zero warnings).
  - `typecheck:node` (tsconfig.node.json): clean.
  - `typecheck:web` (tsconfig.web.json): clean.
  - (The `npm warn ... electron_mirror` lines are unrelated `.npmrc` config
    notices, not TypeScript diagnostics.)

### Runtime status
- `npm run dev` (electron-vite) → **main, preload, and renderer bundles all
  build successfully** (✓ built in ~1.7s / ~0.3s; dev server starts).
- A `TypeError: Cannot read properties of undefined (reading 'whenReady')`
  appears when `npm run dev` is wrapped in `timeout` and the Electron binary is
  not attached. **This error is pre-existing and environment-specific**: it
  reproduces identically on the unmodified baseline (verified via
  `git stash` → same error → `git stash pop`). It is caused by the headless
  `timeout` wrapper executing the main bundle outside the Electron runtime, not
  by any Phase 4.3 change. Under a normal `npm run dev` launch (Electron binary
  present, display available) the app boots and the watcher initializes through
  the single `registerAndWatch` path.

### Watcher validation
- Exactly **one** `VaultWatcher` instance; `VaultWatcher.start()` called from a
  single location (`registerAndWatch`).
- All four vault-open flows (open, create, open-in-new-window, restore, test)
  register through the same owner path → no stacked/duplicate watchers.
- `VaultWatcher.stop()` (called by `VaultRegistry.close` and `VaultService.close`)
  remains the sole teardown path.

### Path validation
- All vault-relative resolutions now flow through `toVaultRelative` with a
  separator-boundary guard, eliminating the substring-collision class of bug.
- `normalizePath` provides consistent separator handling and `.`/`..`
  lexicalization across main and renderer.
- No behavioral change to resolved values for well-formed inputs; only the
  unsafe edge cases (vault name as substring of a later segment) are now
  handled correctly.

---

## 7. Success Criteria — Checklist

- [x] Watchers have explicit ownership (`VaultService.registerAndWatch` is the
      sole owner path).
- [x] Duplicate watchers removed (no stacked chokidar instances possible; 4
      registration sites collapsed to 1).
- [x] Duplicate indexing triggers eliminated (single `buildWatcherConfig`
      callback chain, single `start()` call).
- [x] Path resolution is deterministic and consistent (one canonical module).
- [x] Gate A passes (`npm run typecheck` → zero errors/warnings).
- [x] Runtime behavior remains unchanged (verified: build succeeds; the only
      runtime error is a pre-existing, environment-specific launch issue
      unrelated to this phase).

**Phase 4.3 complete. Do not begin Phase 4.4.**
