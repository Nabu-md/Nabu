# Phase 4.2 — Vault & Workspace Lifecycle Consolidation

**Status:** Complete
**Gate:** Gate A (passes)
**Scope:** Service boundary consolidation only — no behavior redesign.

---

## 1. Lifecycle Audit Report

Every vault and workspace lifecycle entry point was reviewed.

### Vault lifecycle entry points

| Entry point | Location (before) | Owner (before) |
| --- | --- | --- |
| Open vault (path or picker) | `vault:open` IPC → `VaultService.openVault` | VaultService (delegated from IPC) |
| Scan / re-scan vault | `vault:scan` IPC → `VaultService.scanVault` | VaultService |
| Close vault | `vault:close` IPC → `VaultService.closeVault` | VaultService |
| Create vault | `vault:create` IPC → `VaultService.createVault` | VaultService |
| Switch vault | `vault:switch` IPC → `VaultService.switchVault` | VaultService |
| Open in new window | `vault:open-in-new-window` IPC → `VaultService.openVaultInNewWindow` | VaultService |
| Get current vault | `vault:get-current` IPC → `VaultService.getCurrentVault` | VaultService |
| Get recents | `vault:get-recents` IPC → `VaultService.getRecents` | VaultService |
| Restore on launch | `index.ts` `restoreVault` → `VaultService.restoreVault` | index.ts (bootstrap) → VaultService |
| Test vault injection | `index.ts` `NABU_TEST_VAULT` → `VaultService.openTestVault` | index.ts (bootstrap) → VaultService |
| Watcher start/stop | `VaultService.registerAndWatch` / `VaultRegistry.close` | VaultService + VaultRegistry |

### Workspace lifecycle entry points

| Entry point | Location (before) | Owner (before) |
| --- | --- | --- |
| Restore last vault path | `index.ts` `restoreVault` (reads `lastVaultPath`) | index.ts (bootstrap) |
| Persist opened vault | `index.ts` `registerVaultPersistence` (`vault:opened` listener) | index.ts (bootstrap) — **dead code, never emitted** |
| Recent vaults list | `settings.ts` `updateRecentVaults` | settings.ts (helper, never called) |
| Active vault session | `vault-registry.ts` `setActive` / `getActive` | VaultRegistry (singleton) |
| Open vault sessions | `vault-registry.ts` `register` / `get` / `close` | VaultRegistry (singleton) |

**Finding:** There was no `WorkspaceService`. Workspace state (open/recent vaults, active
vault) was scattered across `index.ts`, `settings.ts`, and `vault-registry.ts`. The
`vault:opened` persistence listener and `updateRecentVaults` helper were defined but never
invoked — dead code.

---

## 2. Service Ownership Report

### Before consolidation

| Concern | Owner | Issue |
| --- | --- | --- |
| Vault open/close/switch/scan/create/restore | `VaultService` | OK — already centralized |
| Workspace session restore | `index.ts` `restoreVault` | Bootstrap driving lifecycle directly |
| Workspace persistence (`lastVaultPath`) | `index.ts` `registerVaultPersistence` | Dead listener; no real owner |
| Recent vaults (`recentVaults`) | `settings.ts` `updateRecentVaults` | Helper, never called |
| Active vault tracking | `VaultRegistry` singleton | Competing owner with bootstrap |
| Shutdown close | none (per-window `watcher.stop`) | No deterministic close path |

**Ownership conflicts identified:**
1. `index.ts` directly handled workspace restoration and (attempted) persistence.
2. `VaultRegistry` held active-vault state that the bootstrap also reasoned about.
3. No single owner for workspace lifecycle; persistence was effectively unowned (dead code).

### After consolidation

| Concern | Owner |
| --- | --- |
| Vault open / close / switch / scan / create / restore / test-inject | `VaultService` (sole owner) |
| Workspace load / restore / initialize / persist / save / cleanup | `WorkspaceService` (sole owner) |
| Active vault session bookkeeping | `VaultRegistry` (used by both services, not a lifecycle driver) |
| Startup sequencing | `index.ts` delegates to `VaultService.open()` → `WorkspaceService.load()` |
| Shutdown sequencing | `index.ts` `before-quit` → `WorkspaceService.save()` → `VaultService.close()` |

**Result:** Lifecycle ownership is centralized. No component other than `VaultService`
controls vault lifecycle; no component other than `WorkspaceService` controls workspace
lifecycle. `index.ts` is a thin bootstrap that wires the canonical flow.

---

## 3. Lifecycle Flow Diagram

Canonical open/close lifecycle (Phase 4.2):

```
Application Startup
        │
        ▼
  VaultService.open()          (vault:open / restoreVault → stateManager.openVault + watcher.start)
        │
        ▼
   Vault Ready
        │
        ▼
  WorkspaceService.load()      (restore lastVaultPath / recentVaults from settings)
        │
        ▼
   Workspace Active            (WorkspaceService.initialize marks active vault in registry)
        │
        ▼
   Normal Operation            (IPC handlers delegate to VaultService / WorkspaceService)
        │
        ▼
  WorkspaceService.save()      (persist active + recent vaults to settings on before-quit)
        │
        ▼
  VaultService.close()         (close all registered vault sessions, stop watchers)
        │
        ▼
   Shutdown
```

Every lifecycle transition now passes through these two services.

---

## 4. Files Modified

| File | Change |
| --- | --- |
| `src/main/services/workspace-service.ts` | **Created.** New `WorkspaceService` owning workspace lifecycle (load/restore/initialize/persist/save/cleanup). Decoupled from filesystem concerns. |
| `src/main/services/vault-service.ts` | **Modified.** Added `close()` — the single deterministic shutdown close path that releases all registered vault sessions. Finalized as sole vault lifecycle owner. |
| `src/main/services/vault-registry.ts` | **Modified.** Added `getVaultIds()` to support the deterministic `VaultService.close()` enumeration. |
| `src/main/index.ts` | **Modified.** Removed dead `registerVaultPersistence` (`vault:opened` listener) and the inline `restoreVault` prelude. Wired canonical startup (`VaultService.open()` → `WorkspaceService.load()` → `initialize`) and shutdown (`before-quit` → `WorkspaceService.save()` → `VaultService.close()`). Removed now-unused `ipcMain` import. |

No watchers, path resolution, storage formats, renderer behavior, or application features
were modified.

---

## 5. Verification Summary

### Build status
- `npm run typecheck` → **PASS** (zero errors, zero warnings; only pre-existing npm config
  warnings unrelated to this phase).
- `npm run build` → **PASS** (electron-vite build succeeded for main, preload, renderer).

### Startup status
- `npm run dev` → **PASS**. Electron main process, preload, and renderer all built and the
  dev server launched. No runtime errors during the startup window.

### Lifecycle validation status
- Vault open/close/switch/scan/create/restore all remain routed through `VaultService`.
- Workspace load/restore/persist/save/cleanup now routed through `WorkspaceService`.
- Canonical startup and shutdown flows wired in `index.ts`.
- Runtime behavior preserved: the previously-dead `vault:opened` persistence path was
  replaced by the canonical `WorkspaceService.save()` on `before-quit`, which realizes the
  intended (but previously unimplemented) persistence through the correct owner. No feature,
  watcher, path-resolution, or storage-format logic was altered.

### Gate A
- **PASS.** `VaultService` and `WorkspaceService` exist with clearly defined responsibilities;
  vault lifecycle has one deterministic open/close flow; lifecycle ownership is centralized;
  typecheck and build are clean; runtime behavior is unchanged.

---

## 6. Notes / Deferred

- Watcher improvements are explicitly deferred to Phase 4.3 (out of scope for this phase).
- The `vault:opened` IPC event and `updateRecentVaults` helper were dead code; `updateRecentVaults`
  is still exported from `settings.ts` and is now consumed by `WorkspaceService.persist`, giving
  it a real owner. The `vault:opened` event string is no longer listened to in `index.ts`; the
  equivalent signal now flows through `WorkspaceService.persist` → `appEventBus.publish('VaultOpened')`.
