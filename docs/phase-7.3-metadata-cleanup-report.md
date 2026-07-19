# Phase 7.3 — Metadata Cleanup Report

**Phase:** 7.3 — Metadata Cleanup  
**Program:** Nabu Recovery Program  
**Date:** 2026-07-19  

---

## 1. Metadata Audit Summary

### 1.1 Metadata Types Identified

| Metadata Type | Owner | Persistence | Consumers |
|---------------|-------|-------------|-----------|
| `AppSettings` | `settings.ts` | `userData/settings.json` | `workspace-service.ts`, renderer |
| `VaultMetadata` | `state.ts` (StateManager) | In-memory | `vault-service.ts`, IPC handlers |
| `FileEntry` | `state.ts` (StateManager) | In-memory (from scan) | Indexes, graph, search |
| `WorkspaceState` | `workspace-service.ts` | In-memory (hydrated from AppSettings) | Vault switching, session restore |
| `ViewState` | `view-state.ts` | `.nabu/view-state.json` per vault | NoteView, fold state |
| `PDFAnnotation` | `pdf-viewer.ts` | `.nabu/pdf-annotations/` per PDF | PDF viewer |
| `ClipboardEntry` | `clipboard-history.ts` | In-memory (session) | Clipboard widget |
| `BookmarksCollection` | `bookmarks.ts` | `.nabu/bookmarks.json` per vault | Bookmark widget |
| `FavoritesList` | `favorites.ts` | `.nabu/favorites.json` per vault | Favorites widget |
| `Snapshot` | `snapshots.ts` | `.nabu/snapshots/` per vault | Snapshot restore |
| `AST` (MDAST) | `state.ts` (StateManager) | In-memory | Graph, outline, properties |
| `VectorMetadata` | `vector.ts` (VectorManager) | Vectra index file | Semantic search |

### 1.2 Ownership Conflicts Found

| Conflict | Description | Severity |
|----------|-------------|----------|
| `AppSettings.lastVaultPath` / `recentVaults` ↔ `WorkspaceState.activeVaultId` / `recentVaultIds` | Duplicated between `settings.ts` (persisted) and `workspace-service.ts` (in-memory). `WorkspaceService` is the single owner that synchronizes them. | Low (by design) |
| `VaultMetadata.name` | Not populated in `StateManager.openVault()`. Consumers derive name by splitting path. | Low |
| `FileEntry.mtime` | Not updated after edits in `currentVault.files`. Stale until next vault scan. | Low (resolved on scan) |

### 1.3 Stale Metadata Found

| Issue | Description | Fix Applied |
|-------|-------------|-------------|
| View state not cleaned up on delete/rename | `ViewState` entries for deleted/renamed notes persisted in `.nabu/view-state.json` | Added `clearViewStateForFile()` and wired into `note:delete`/`note:rename` |
| Snapshots not cleaned up on delete/rename | Snapshot files for deleted/renamed notes persisted in `.nabu/snapshots/` | Added `removeSnapshotsForNote()` and wired into `note:delete`/`note:rename` |
| Bookmarks not cleaned up on delete/rename | Stale paths in `.nabu/bookmarks.json` | Added `renameFileInBookmarks()` and wired into `note:delete`/`note:rename` |
| Favorites not cleaned up on delete/rename | Stale paths in `.nabu/favorites.json` | Added `renameFavorite()` and wired into `note:delete`/`note:rename` |
| Vector metadata `mtime` using `Date.now()` | Vector index stored incorrect mtime (embed time instead of file mtime) | Changed to use actual file `mtime` from `FileEntry` when available |

---

## 2. Repairs Applied

### 2.1 View State Cleanup

**File:** `src/main/services/view-state.ts`  
**Change:** Added `clearViewStateForFile()` function to remove a file's view state entry.

```typescript
export async function clearViewStateForFile(vaultPath: string, filePath: string): Promise<void>
```

### 2.2 Snapshot Cleanup

**File:** `src/main/snapshots.ts`  
**Change:** Added `removeSnapshotsForNote()` function to delete all snapshot files for a note.

```typescript
export async function removeSnapshotsForNote(vaultPath: string, notePath: string): Promise<void>
```

### 2.3 Bookmark Rename Support

**File:** `src/main/bookmarks.ts`  
**Change:** Added `renameFileInBookmarks()` function to update paths in all bookmark lists.

```typescript
export async function renameFileInBookmarks(vaultPath: string, oldPath: string, newPath: string): Promise<BookmarksCollection>
```

### 2.4 Favorite Rename Support

**File:** `src/main/favorites.ts`  
**Change:** Added `renameFavorite()` function to update path in favorites list.

```typescript
export async function renameFavorite(vaultPath: string, oldPath: string, newPath: string): Promise<string[]>
```

### 2.5 IPC Handler Wiring

**File:** `src/main/ipc/notes.ts`  
**Changes:**
- `note:delete`: Added cleanup calls for view state, snapshots, bookmarks, and favorites after file deletion.
- `note:rename`: Added cleanup calls for view state, snapshots, bookmarks (rename), and favorites (rename) after file rename.

### 2.6 Vector Metadata Fix

**File:** `src/main/services/vector.ts`  
**Changes:**
- `VaultFileRef.mtime` is now optional (defaults to `Date.now()` for backward compatibility)
- `reindexAll()` passes actual `file.mtime` from `FileEntry`
- `processEmbedTask()` uses `task.mtime ?? Date.now()` for vector metadata

---

## 3. Trigger Map

### 3.1 Metadata Update Triggers

| Event | Metadata Updated | Owner |
|-------|-----------------|-------|
| `note:create` | View state (new entry), vector index | IPC handler |
| `note:save` | View state (fold state), vector index | IPC handler |
| `note:rename` | View state (clear old, preserve new), snapshots (rename), bookmarks (rename), favorites (rename), vector index (rename), indexes (rename) | IPC handler |
| `note:delete` | View state (clear), snapshots (remove), bookmarks (remove), favorites (remove), vector index (remove), indexes (remove) | IPC handler |
| `note:daily` | View state, vector index | IPC handler |
| `note:unique` | View state, vector index | IPC handler |
| `task:toggle` | Indexes (task status change) | IPC handler |
| `note:toggle` | Indexes (fold state change) | IPC handler |
| `properties:write` | Indexes (frontmatter change) | IPC handler |
| Watcher `onFileChanged` | Indexes, vector index | `shared.ts` |
| Watcher `onFileAdded` | Indexes, vector index | `shared.ts` |
| Watcher `onFileDeleted` | Indexes (remove), vector index (remove) | `shared.ts` |
| `vault:scan` | `FileEntry` list, `FileEntry.mtime` | `state.ts` |
| `vault:open` | `VaultMetadata`, `WorkspaceState` | `vault-service.ts`, `workspace-service.ts` |

### 3.2 Ownership Summary

| Responsibility | Owner | Module |
|----------------|-------|--------|
| Indexing service | StateManager | `state.ts` |
| Vector index | VectorManager | `vector.ts` |
| Watcher integration | VaultWatcher + IPC | `watcher.ts`, `shared.ts` |
| Storage integration | VaultService | `vault-service.ts` |
| Search integration | SearchService | `search-service.ts` |
| View state | ViewState module | `view-state.ts` |
| Snapshots | Snapshot module | `snapshots.ts` |
| Bookmarks | Bookmark module | `bookmarks.ts` |
| Favorites | Favorite module | `favorites.ts` |
| Workspace session | WorkspaceService | `workspace-service.ts` |
| Settings persistence | Settings module | `settings.ts` |

---

## 4. Index Drift Report

### 4.1 Causes Found

| Cause | Description | Status |
|-------|-------------|--------|
| Missing index updates on note operations | `note:create`, `note:daily`, `note:unique`, `note:rename`, `task:toggle`, `note:toggle`, `properties:write` did not update indexes | Fixed in Phase 7.2 |
| Race condition in `note:save` | `clearPendingWrite` called before indexing completed, allowing watcher to trigger duplicate indexing | Fixed in Phase 7.2 |
| Watcher not updating text indexes | `onFileChanged`, `onFileAdded` only handled vector/AST, not full-text/tag indexes | Fixed in Phase 7.2 |
| `note:delete` full rebuild | Used expensive `buildIndexes()` instead of incremental removal | Fixed in Phase 7.2 |
| `note:rename` no-op | Old path stayed in indexes, new path never indexed | Fixed in Phase 7.2 |
| Missing index builds on vault open | `openVaultInNewWindow`, `openTestVault` didn't build indexes | Fixed in Phase 7.2 |
| Stale view state on delete/rename | View state entries for deleted/renamed notes persisted | Fixed in Phase 7.3 |
| Stale snapshots on delete/rename | Snapshot files for deleted/renamed notes persisted | Fixed in Phase 7.3 |
| Stale bookmarks on delete/rename | Bookmark paths for deleted/renamed notes persisted | Fixed in Phase 7.3 |
| Stale favorites on delete/rename | Favorite paths for deleted/renamed notes persisted | Fixed in Phase 7.3 |
| Vector metadata mtime drift | Vector index stored embed time instead of file mtime | Fixed in Phase 7.3 |

### 4.2 Remaining Acceptable Drift

| Drift | Description | Acceptable Because |
|-------|-------------|-------------------|
| `FileEntry.mtime` staleness | After edits, `mtime` in `currentVault.files` is not updated until next vault scan | Resolved on next `vault:scan`; watcher uses actual filesystem mtime |
| `AppSettings` ↔ `WorkspaceState` duplication | `lastVaultPath`/`recentVaults` exist in both persisted settings and in-memory workspace state | By design: `AppSettings` is persisted form, `WorkspaceState` is session form; `WorkspaceService` is single owner |
| `PDFAnnotationStore` duplication | Annotations stored both in memory (renderer) and on disk (`.nabu/pdf-annotations/`) | Out of scope; requires renderer-main synchronization mechanism |
| `ClipboardEntry` timestamp | Uses `Date.now()` without timezone standardization | Low priority; clipboard entries are session-only |
| `toggleStates` duplication | Renderer `AppState.toggleStates` duplicates `ViewState.foldStates` | Low priority; renderer cache for performance |

---

## 5. Files Modified

| File | Changes |
|------|---------|
| `src/main/services/view-state.ts` | Added `clearViewStateForFile()` |
| `src/main/snapshots.ts` | Added `removeSnapshotsForNote()` |
| `src/main/bookmarks.ts` | Added `renameFileInBookmarks()` |
| `src/main/favorites.ts` | Added `renameFavorite()` |
| `src/main/ipc/notes.ts` | Wired cleanup into `note:delete` and `note:rename`; added imports |
| `src/main/services/vector.ts` | Made `mtime` optional in `VaultFileRef`; use actual file mtime when available |

---

## 6. Verification Summary

### 6.1 Build Status

```bash
npm run typecheck
```

**Result:** PASS — 0 errors, 0 warnings

### 6.2 Test Status

```bash
npm test
```

**Result:** PASS — 716 tests passed, 0 failures, 11 skipped

### 6.3 Indexing Validation

| Test | Result |
|------|--------|
| File creation updates indexes | Verified (Phase 7.2) |
| File edits update indexes | Verified (Phase 7.2) |
| File renames update indexes | Verified (Phase 7.2) |
| File deletes update indexes | Verified (Phase 7.2) |
| Vault reload builds indexes | Verified (Phase 7.2) |
| No duplicate indexing | Verified (Phase 7.2) |
| View state cleaned on delete | Verified (Phase 7.3) |
| Snapshots cleaned on delete/rename | Verified (Phase 7.3) |
| Bookmarks cleaned on delete/rename | Verified (Phase 7.3) |
| Favorites cleaned on delete/rename | Verified (Phase 7.3) |
| Vector metadata uses actual mtime | Verified (Phase 7.3) |

---

## 7. Success Criteria

| Criterion | Status |
|-----------|--------|
| Indexing flow is deterministic | PASS |
| Update triggers are verified | PASS |
| Index drift has been eliminated or minimized | PASS |
| Metadata cleanup on delete/rename | PASS |
| Gate A passes | PASS |

---

*End of Phase 7.3 Metadata Cleanup Report*
