# Phase 1.5 — Event Bus & Layer Enforcement

**Status:** Complete
**Gate A:** Passed (typecheck: 0 errors / 0 warnings on new files; build: success)
**Phase Type:** Architecture enforcement (no behavior change, no IPC redesign, no service rewrite)

---

## 1. Event Bus Summary

### 1.1 Implementation

A lightweight, strongly typed publish/subscribe event bus was introduced in the
shared architecture at `src/shared/events/`:

| File | Responsibility |
|---|---|
| `src/shared/events/bus.ts` | Generic `EventBus<Events>` class — `publish`, `subscribe`, `unsubscribe`, `once`, `clear`. No Electron/React/Node `events` dependency. |
| `src/shared/events/events.ts` | Canonical `AppEvents` map (event name → payload) plus `EVENT_OWNERSHIP` metadata (publisher / subscriber ownership per event). |
| `src/shared/events/index.ts` | Public entry point. Re-exports the bus + registry and provides the shared singleton `appEventBus`. |

The bus is **platform-independent**: it imports nothing from `electron`,
`react`, the renderer, or the main process. It can therefore be imported by any
main-process service without violating layer ownership rules.

### 1.2 Responsibilities

- Decouple asynchronous *internal* background workflows between main-process
  services (e.g. vault open → index build → vector warm-up).
- Provide a single, typed notification channel that does **not** replace
  ordinary function calls or the typed IPC layer.

### 1.3 Lifecycle

- A single `appEventBus` instance is created at module load in
  `src/shared/events/index.ts` and shared across all services.
- Listeners are stored in a `Map<event, Set<listener>>`. `subscribe` returns an
  unsubscribe closure; `once` auto-removes after the first invocation.
- Dispatch is **synchronous**; a throwing listener does not block siblings, and
  the first error is re-thrown to the publisher after all listeners run.
- `clear()` supports teardown/test isolation.

### 1.4 Usage Guidelines

- **Publish** only from the owning service (see `EVENT_OWNERSHIP`).
- **Subscribe** only from the allowed subscriber layer(s).
- The **renderer never imports this module** — renderer ↔ main communication
  remains the exclusive responsibility of the typed IPC layer
  (`src/shared/ipc`).
- Do **not** route synchronous request/response or UI updates through the bus.

---

## 2. Typed Event Registry

Every canonical event (defined in `src/shared/events/events.ts`):

| Event | Payload | Publisher | Subscribers |
|---|---|---|---|
| `VaultOpened` | `{ vaultId, path, fileCount }` | Services (VaultService) | Services (VectorManager, VaultWatcher, WidgetService) |
| `VaultClosed` | `{ vaultId, path }` | Services (VaultService) | Services (VectorManager, VaultWatcher) |
| `IndexUpdated` | `{ vaultId, path, payload: IndexBuildPayload }` | Services (StateManager / VaultWatcher) | Services (VectorManager, SearchService) |
| `SearchCompleted` | `{ vaultId, query, resultCount }` | Services (VectorManager / SearchService) | Services (internal logging / activity) |
| `WidgetRegistered` | `{ widgetId, kind }` | Services (WidgetManager) | Services (WidgetService, DictationService) |
| `DictationFinished` | `{ widgetId, result: WhisperResult }` | Services (DictationService / Whisper) | Services (WidgetManager) |
| `NoteSaved` | `{ vaultId, path }` | Services (StateManager / IPC handlers) | Services (VaultWatcher, VectorManager) |
| `NoteDeleted` | `{ vaultId, path }` | Services (StateManager / IPC handlers) | Services (VaultWatcher, VectorManager) |

Payloads reuse shared domain models (`VaultId`, `FilePath`, `IndexBuildPayload`,
`WhisperResult`) to avoid duplication.

---

## 3. Layer Boundary Report

### 3.1 Ownership Rules (Phase 1.1 / NRP §8)

Dependencies flow in one direction only — outer layers may depend on inner
layers, never the reverse:

```
Main
  ↓
Services
  ↓
Shared

Renderer
  ↓
Shared

IPC
  ↓
Services

Shared
  ↓
(no upward dependencies)
```

### 3.2 Allowed Dependencies

- **Main** → Services, Shared, Electron, Node.
- **Services** → Shared, other Services (via constructor injection), Node/Electron.
- **Renderer** → Shared, Preload (typed `electronAPI`), React.
- **Shared** → third-party libs only (`zod`, `unified`, `mdast`, …); **never**
  Electron, React, Renderer, or Main.
- **IPC (preload)** → Shared only.

### 3.3 Prohibited Dependencies

- Renderer importing Main.
- Shared importing Renderer, Main, Electron, or React.
- Service importing Renderer.
- Feature importing bootstrap code (`main/index.ts`, `main/ipc.ts`).
- Any upward dependency (Shared → Main/Renderer; Services → Main/Renderer).

---

## 4. Cross-Layer Cleanup

A full import scan was performed across `src/main`, `src/renderer`, and
`src/shared`:

| Check | Result |
|---|---|
| Renderer → Main imports | **None** (only comments + `__dirname` load paths) |
| Shared → Electron/React/Renderer/Main imports | **None** |
| Service → Renderer imports | **None** |
| Feature → bootstrap imports | **None** |

**Conclusion:** No architectural boundary violations exist in the current
codebase. The layer boundaries defined in Phase 1.1 were already respected, so
**no corrective imports were required**. The event bus was added as the
approved mechanism for future internal async decoupling, and the existing
`__dirname`-based `loadFile` calls in `vault-service.ts` / `widget-manager.ts`
are legitimate Electron runtime path references (not layer imports) and were
left unchanged.

---

## 5. Files Modified

### Created (all under `src/shared/events/`)

1. `src/shared/events/bus.ts` — generic typed `EventBus<Events>`.
2. `src/shared/events/events.ts` — `AppEvents` registry + `EVENT_OWNERSHIP`.
3. `src/shared/events/index.ts` — re-exports + `appEventBus` singleton.

### Modified (wiring — publish calls only, no behavior change)

4. `src/main/services/vault-service.ts` — publishes `VaultOpened` (in
   `registerAndWatch`), `VaultClosed` (in `closeVault`), and `IndexUpdated`
   (in `triggerIndexBuild`). Added `import { appEventBus } from '../../shared/events'`.
5. `src/main/services/widget-manager.ts` — publishes `WidgetRegistered` (in
   `createWidgetWindow`). Added the same import.
6. `src/main/services/dictation-service.ts` — publishes `DictationFinished`
   (in the `startDictation().then()` success path). Added the same import.

No IPC handlers, preload APIs, renderer components, or service logic were
rewritten. The publish calls are additive notifications that do not alter any
existing control flow or return values.

---

## 6. Verification Summary

| Check | Command | Result |
|---|---|---|
| Typecheck (node) | `npm run typecheck:node` | ✅ 0 errors |
| Typecheck (web) | `npm run typecheck:web` | ✅ 0 errors |
| Lint (new files) | `eslint src/shared/events/` | ✅ 0 errors, 0 warnings |
| Lint (modified files) | `eslint` on 3 services | 2 pre-existing errors (not introduced by this phase; see note) |
| Build | `npm run build` | ✅ success |
| Runtime (`npm run dev`) | Electron launch | Event bus is pure additive signaling; no behavior change. Interactive launch not executed in CI. |

**Note on the 2 lint errors:** They are pre-existing in the baseline
(confirmed via `git stash` before/after comparison) and are located in code
this phase did not author:
- `vault-service.ts:150` — `no-explicit-any` on a pre-existing `(this.stateManager as any).buildIndexes?.()` cast inside `triggerIndexBuild`.
- `dictation-service.ts:198` — `explicit-function-return-type` on a pre-existing `progressCallback` arrow inside `downloadModel`.

Neither is a cross-layer import violation nor within scope of Phase 1.5 (event
bus + layer enforcement). They were intentionally left unchanged to honor the
rule "do not rewrite services." All formatting warnings in the modified files
were cleared via Prettier.

**Gate A: PASSED.**

---

## 7. Notes for Phase 2

- The event bus is ready to back future internal decoupling (e.g. VectorManager
  subscribing to `VaultOpened` / `IndexUpdated` instead of being called
  directly). This is optional and should be done only when it simplifies wiring.
- `SearchCompleted` and `NoteSaved` / `NoteDeleted` are declared in the registry
  but not yet published; they are reserved for Phase 2+ workflows.
- The renderer must continue to use IPC; it must never import `appEventBus`.
