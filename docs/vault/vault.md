# Vault & Workspace Subsystem

**Permanent technical documentation for the Nabu vault and workspace subsystem.**

This document reflects the implementation currently in the repository as of
Phase 4.4 of the Nabu Recovery Program. It covers architecture, lifecycle,
service ownership, watchers, and path resolution. It is the authoritative
reference for future maintenance.

---

## 1. Overview

Nabu organizes user knowledge into **vaults** — directories of Markdown notes
plus a `.nabu/` cache folder (indexes, snapshots, view-state, favorites,
bookmarks, bases). A **workspace** is the user's working session: which vaults
are open/recent and which vault is active.

### Responsibilities

| Concern | Owner |
|---------|-------|
| Vault lifecycle (open / close / create / switch / scan / restore) | `VaultService` |
| Workspace session state (active + recent vaults, persistence) | `WorkspaceService` |
| Open vault sessions (multi-vault registry) | `VaultRegistry` |
| Filesystem monitoring of the open vault | `VaultWatcher` (owned by `VaultService`) |
| In-memory vault state, AST cache, search index | `StateManager` |
| Vector embeddings / semantic search | `VectorManager` |
| IPC transport (thin wrappers) | `src/main/ipc/*` |
| Application bootstrap & lifecycle orchestration | `src/main/index.ts` |

The subsystem is built so that **exactly one deterministic lifecycle owner**
exists. `VaultService` owns vault lifecycle; `WorkspaceService` owns workspace
lifecycle. No other component drives vault or workspace lifecycle directly.

---

## 2. Lifecycle

The canonical lifecycle (established in Phase 4.2 and verified in Phase 4.4):

```
Application Startup
  ↓
VaultService.open()          (vault ready)
  ↓
WorkspaceService.load()     (workspace active)
  ↓
Normal Operation
  ↓
WorkspaceService.save()     (persist session)
  ↓
VaultService.close()        (shutdown)
```

### 2.1 Startup

`src/main/index.ts` (`app.whenReady`):

1. Instantiate core modules: `StateManager`, `VectorManager`, `VaultWatcher`.
2. Register IPC handlers (thin wrappers over the services).
3. Create the main `BrowserWindow`.
4. Construct `VaultService` and `WorkspaceService` (once, shared).
5. On `did-finish-load`:
   - If `NABU_TEST_VAULT` env var is set → `VaultService.openTestVault()`
     (E2E injection, bypasses persisted settings).
   - Otherwise → `restoreVault(vaultService, workspaceService, mainWindow)`:
     - `WorkspaceService.load()` reads `lastVaultPath` / `recentVaults`.
     - `VaultService.restoreVault()` opens the restored vault (or signals the
       renderer to show the picker when none / unreadable).
     - `WorkspaceService.initialize(lastVaultPath)` marks the vault active.

### 2.2 Open

`VaultService.openVault(options)` (handler: `vault:open`):

1. Validate payload (`VaultOpenSchema`).
2. Resolve path (native folder picker if none provided).
3. `StateManager.openVault(path)` → scans vault, builds `FileEntry[]` metadata.
4. `copyDefaultTemplates()` (non-fatal, first-open only).
5. `registerAndWatch(path, vaultMeta)` — **the single watcher-ownership path**:
   - `VaultRegistry.register(...)` + `setActive(...)`.
   - `VaultWatcher.start(buildWatcherConfig(...))`.
   - Publish `VaultOpened` on the app event bus.
6. `triggerIndexBuild()` → `StateManager.buildIndexes()` → `INDEX_BUILD`.
7. `sendToRenderer(NOTES_LOADED, { vaultPath, files })`.

`createVault` and `openVaultInNewWindow` follow the same `registerAndWatch`
path (the latter additionally creates a second `BrowserWindow`).

### 2.3 Close

`VaultService.closeVault(rawPayload)` (handler: `vault:close`):

1. Validate (`VaultCloseSchema`).
2. If `vaultId` provided → `VaultRegistry.close(vaultId)`:
   - `session.watcher.stop()` (tears down chokidar, cancels debounce timers).
   - Deactivate if active; remove session from registry.
3. Else (legacy fallback) → `VaultWatcher.stop()`.
4. Publish `VaultClosed` on the app event bus.

`VaultService.close()` (shutdown): iterates `VaultRegistry.getVaultIds()` and
closes each session, publishing `VaultClosed` per vault. This is the canonical
close step of the shutdown flow.

### 2.4 Reload / Switch

`VaultService.switchVault(rawPayload)` (handler: `vault:switch`):

- If the target vault is already registered → `VaultRegistry.setActive(vaultId)`
  (no re-open, no new watcher — the existing session's watcher stays live).
- If it matches the current vault path → success.
- Otherwise → error (`Vault not found in registry`).

`VaultService.scanVault()` (handler: `vault:scan`) re-runs
`StateManager.openVault(currentVault.path)` and `triggerIndexBuild()` to refresh
metadata and the search index without tearing down the watcher.

> **Watcher transfer on switch:** Because the watcher is owned per registered
> session and `switchVault` only changes *which* session is active (not which
> is open), the watcher for an already-open vault is never stopped and
> re-started. Indexing resumes via the existing watcher callbacks. For a vault
> not yet registered, the caller must `openVault` it first (which registers and
> watches it); there is no implicit close-then-open in `switchVault`.

### 2.5 Shutdown

`src/main/index.ts` (`app.on('before-quit')`):

1. `await WorkspaceService.save()` — persists `lastVaultPath` / `recentVaults`.
2. `VaultService.close()` — closes all registered sessions (stops watchers,
   releases state).

On macOS dock re-activation with no windows, a new `BrowserWindow` is created
(the vault session state persists in the registry/settings).

---

## 3. Service Ownership

### VaultService (`src/main/services/vault-service.ts`)
Owns **all** vault lifecycle business logic: open, close, create, switch, scan,
restore, test-injection, and the single `registerAndWatch` watcher-ownership
path. Constructed once with `StateManager`, `VectorManager`, and the
`VaultWatcher` singleton. Delegates registry bookkeeping to `VaultRegistry` and
watcher config to `buildWatcherConfig` (in `src/main/ipc/shared.ts`).

### WorkspaceService (`src/main/services/workspace-service.ts`)
Owns **workspace** lifecycle and session state only. It does **not** import
Electron, `fs`, or any vault file-handling code. Responsibilities:
`load()` (restore from settings), `initialize()` (mark active in registry),
`persist()` (update active/open/recent + write settings), `getLastVaultPath()`,
`clearLastVaultPath()`, `save()` (persist on shutdown), `cleanup()`.

### Supporting services
- **VaultRegistry** (`src/main/services/vault-registry.ts`) — holds open vault
  sessions (`StateManager` + `VectorManager` + `VaultWatcher` per session),
  tracks the active session, and stops a session's watcher on `close()`.
- **StateManager** (`src/main/services/state.ts`) — in-memory vault state,
  AST cache, search index, `Pending_Write_Lock`, vault scanning.
- **VectorManager** (`src/main/services/vector.ts`) — semantic embeddings and
  the vector search index; receives embed/remove calls from watcher callbacks.
- **VaultWatcher** (`src/main/services/watcher.ts`) — the chokidar filesystem
  watcher; owned exclusively by `VaultService` via `registerAndWatch`.

---

## 4. Watchers

### 4.1 Ownership
There is **exactly one** filesystem watcher: the `VaultWatcher` chokidar
singleton. Its sole owner is `VaultService`, and the **only** code path that
starts it is `VaultService.registerAndWatch()`. No other component registers,
starts, or stops a filesystem watcher.

### 4.2 Registration
`registerAndWatch(vaultPath, vaultMeta)`:
1. `VaultRegistry.register(vaultPath, vaultPath, stateManager, vectorManager, watcher)`
   — registers the session (vault id == vault path).
2. `VaultRegistry.setActive(vaultPath)`.
3. `VaultWatcher.start(buildWatcherConfig(...))` — starts chokidar on
   `vaultPath`, ignoring `/^\|\.nabu/`, with `awaitWriteFinish` (50 ms
   stability threshold).

All four vault-open flows (`openVault`/`createVault`, `openVaultInNewWindow`,
`restoreVault`, `openTestVault`) route through `registerAndWatch`, so the
watcher is started from a single deterministic location. `VaultWatcher.start()`
internally calls `stop()` first, preventing stacked chokidar instances.

### 4.3 Cleanup
- Per-session: `VaultRegistry.close(vaultId)` → `session.watcher.stop()` →
  `VaultWatcher.stop()` closes the chokidar instance and cancels all pending
  per-file debounce timers.
- Shutdown: `VaultService.close()` closes every registered session.
- Fatal watcher errors (EMFILE/ENFILE/fsevents crash) trigger an automatic
  restart sequence (up to 3 attempts, 2 s apart) inside `VaultWatcher`;
  exhaustion forwards to `onError`.

### 4.4 Indexing triggers
```
Filesystem Event
  ↓
VaultWatcher (chokidar)
  ↓
onFileChanged / onFileAdded / onFileDeleted / onImageAdded
  ↓
VectorManager.embedFile / removeFile   (vector index)
  ↓
Search Database
```
Callbacks are centralized in `buildWatcherConfig` (`src/main/ipc/shared.ts`):
- `onFileChanged` → invalidate AST, re-parse, embed (skipped when
  `Pending_Write_Lock` indicates an app-initiated write).
- `onFileAdded` → notify renderer, embed new file.
- `onFileDeleted` → `VectorManager.removeFile`, notify renderer.
- `onImageAdded` → enqueue OCR, create companion note, update indexes.

A full rebuild path also exists: `VaultService.triggerIndexBuild()` →
`StateManager.buildIndexes()` → `IPCChannel.INDEX_BUILD`. Because the watcher
is started exactly once per session, each filesystem event produces exactly one
indexing trigger — no duplicate indexing.

---

## 5. Path Resolution

### 5.1 Canonical strategy
All vault/workspace-relative path resolution flows through the single,
dependency-free module **`src/shared/path.ts`** (importable from both main and
renderer; no Node `fs`/`path` dependency). This replaced four previously
divergent strategies (hand-rolled `getRelativePath`, `notePath.replace(...)`,
`path.relative`).

### 5.2 Vault-relative paths
`toVaultRelative(vaultPath, filePath)` returns the normalized path relative to
the vault root (no leading separator). It uses a **separator-boundary check**
(`target.startsWith(base + '/')`) so a vault named `Notes` does **not** corrupt
`/MyNotes/file.md`. If `filePath` is not under `vaultPath`, the normalized
`filePath` is returned unchanged.

`fromVaultRelative(vaultPath, relativePath)` is the inverse, lexicalized against
path traversal.

### 5.3 Workspace-relative paths
`toWorkspaceRelative(workspaceRoot, filePath)` is an explicit workspace-scoped
alias of `toVaultRelative`, used where intent is a workspace root rather than a
vault root.

### 5.4 Normalization rules
`normalizePath(input)`:
- Unifies `\` → `/` (Windows compatibility).
- Collapses repeated separators (`a//b` → `a/b`).
- Resolves `.` and `..` lexically (no filesystem access).
- Strips a single trailing separator (except root `/`).
- Preserves absolute POSIX leading `/` and Windows drive-letter prefixes.

Result is deterministic and safe as a map key / equality comparator.

### 5.5 Call sites using the canonical module
- `src/main/snapshots.ts` — `getSnapshotPath`, `pruneNoteSnapshots`,
  `restoreSnapshot`.
- `src/main/services/view-state.ts` — `getViewStateFile`.
- `src/shared/search-query.ts` — `getRelativePath`.

`StateManager.scanVault` continues to use Node's `path.relative` + `path.sep`
(which is already canonical and consistent with this strategy).

---

## 6. Maintenance Guide

### 6.1 Adding lifecycle functionality
- **Vault** behavior → add a method to `VaultService`; wire it through a thin
  IPC handler in `src/main/ipc/vault.ts` (validate with a Zod schema from
  `@shared/schemas`, delegate to the service, return a validated response).
- **Workspace** behavior → add a method to `WorkspaceService`; keep it free of
  Electron/`fs`/vault-file imports.
- Never start/stop a vault or watcher from outside `VaultService`.

### 6.2 Watcher ownership rules
- The `VaultWatcher` singleton is owned exclusively by `VaultService`.
- The **only** method that may call `VaultWatcher.start()` is
  `VaultService.registerAndWatch()`.
- Every vault-open flow must go through `registerAndWatch` (never inline
  `vaultRegistry.register` + `watcher.start`).
- Teardown is `VaultRegistry.close(vaultId)` → `session.watcher.stop()`; never
  call `VaultWatcher.stop()` ad hoc from other components.
- Do not add a second filesystem watcher for vault/workspace/plugin/metadata
  monitoring — extend the existing `buildWatcherConfig` callbacks instead.

### 6.3 Path resolution guidelines
- Never use `notePath.replace(vaultPath, '')` or similar substring tricks.
- Always use `toVaultRelative` / `fromVaultRelative` / `normalizePath` from
  `src/shared/path.ts`.
- For workspace roots, use `toWorkspaceRelative` to make intent explicit.
- Keep `src/shared/path.ts` dependency-free so it stays renderer-safe.

### 6.4 Service boundary expectations
- `VaultService` ↔ `WorkspaceService`: coordinated only in `src/main/index.ts`
  (startup/shutdown). They do not import each other.
- `VaultService` ↔ `VaultRegistry`: `VaultService` drives registry
  register/setActive/close; the registry never starts/stops vaults on its own
  beyond stopping a session's watcher on `close()`.
- IPC layer (`src/main/ipc/*`) is a thin transport: validate → delegate →
  return. No lifecycle logic lives there.
- Renderer never touches vault files directly; all access is via IPC.

---

*Document generated for Phase 4.4 — Verification & Documentation. Reflects the
repository state after Phase 4.3 (watcher cleanup & path resolution repair).*
