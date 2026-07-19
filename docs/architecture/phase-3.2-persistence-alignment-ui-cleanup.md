# Phase 3.2 — Persistence Alignment & UI Cleanup (Prompt A)

**Status:** Complete
**Gate A:** Passed (`npm run typecheck` green, zero TS errors; `npm run dev` build green; ESLint clean on widget files)
**Scope:** Widget registry/state ownership alignment, persistence coordination, widget UI integration decoupling, rendering dependency simplification.
**Out of scope (per rules):** Widget redesign, new widget types, storage redesign, rendering redesign, widget UX/appearance changes, behavior changes.

---

## 1. Registry Alignment Report

### 1.1 Registry ownership

`WidgetManager` (`src/main/services/widget-manager.ts`) remains the **authoritative owner** of widget lifecycle and in-memory `WidgetState` on the main process. Phase 3.1 already consolidated all create/update/remove/restore transitions through it.

The audit identified a **second, divergent source of truth on the renderer** for widget-rendered data:

- `ActivityTimeline` read its data from `useAppContext().state.activityLog` — the global app reducer, not the widget owner.
- `DictationWidget` read its data directly from the raw Electron preload bridge (`window.electron.on.widget*`) and invoked dictation business logic via `window.electron.dictation.*`.

This violated the "registry is the single source of truth" principle: widget rendering was coupled to unrelated application state (the whole app reducer) and to the Electron bridge rather than to a widget-specific contract.

### 1.2 Synchronization improvements

Introduced a **renderer-side widget state owner** — `src/renderer/src/features/widgets/widgetService.ts` — that is the single source of truth for widget-rendered state on the renderer. It:

- Subscribes once to the widget-specific IPC channels (`ipc.on.activityLog`, `ipc.on.widgetModeChanged`, `ipc.on.widgetDictationStarting`, `ipc.on.widgetDictationComplete`, `ipc.on.widgetDictationError`).
- Maintains `activityEntries` and `dictationState` as the only widget state the UI reads.
- Exposes `useWidgetActivity()` and `useWidgetDictation()` hooks — the **only** surface widget components consume.

The data flow is now deterministic and one-directional:

```
Registry (WidgetManager, main)
        │  IPC channels
        ▼
Widget State (widgetService — renderer owner)
        │  hooks
        ▼
Renderer (ActivityTimeline / DictationWidget)
        │
        ▼
UI
```

Rendering never owns widget state; it only reads from `widgetService`.

### 1.3 Persistence alignment

Persistence was already correct after Phase 3.1 (clipboard entries via `ClipboardHistory`; `clipboardShortcut` via `settings.json`; restore via `WidgetManager.initialize()`). This phase **did not change persistence format or paths**. The alignment work was on the renderer side: the activity log previously lived in the app reducer (`ACTIVITY_ADD`) as a duplicate store. It now flows through the widget service, which consumes the same `activity:log` channel the main process emits — so the rendered activity state is always derived from the same authoritative event stream as before, with no second copy in the app reducer.

---

## 2. UI Cleanup Report

### 2.1 Removed dependencies

| Component | Before | After |
|-----------|--------|-------|
| `ActivityTimeline` | Imported `useAppContext` (entire app reducer) and `@shared/types` | Imports only `useWidgetActivity` + `ActivityEntry` from `widgetService` |
| `DictationWidget` | Imported `window.electron` (raw preload bridge) + `window.electron.dictation` (business logic) | Imports only `useWidgetDictation` from `widgetService` |
| `App.tsx` | Owned `activityLog` in reducer; subscribed `activity:log`; dispatched `ACTIVITY_ADD` | No longer owns widget activity state; routes external-edit activity through `recordExternalActivity()` |

### 2.2 Simplified rendering paths

- `ActivityTimeline` no longer reaches into the global app context. It renders purely from `useWidgetActivity()`.
- `DictationWidget` no longer touches the Electron bridge or dictation business logic. It renders from `useWidgetDictation()` and calls `start()`/`stop()` exposed by the service (which delegate to `ipc.dictation`).
- `micPermissionError` is now derived directly from `dictationState` (no separate `useState` + `useEffect`), removing an unnecessary effect and a cascading-render lint error.

### 2.3 Reduced coupling

- Widget UI depends **only** on widget-specific services/contracts (`widgetService`), satisfying the "widget UI depends only on widget-specific services and contracts" requirement.
- The global app reducer (`AppState`) no longer carries widget activity state, eliminating a cross-cutting dependency between the vault/note domain and the widget domain.
- No feature cross-dependencies, no business logic inside rendering components, no duplicated state management.

---

## 3. Files Modified

| File | Change |
|------|--------|
| `src/renderer/src/features/widgets/widgetService.ts` | **Created.** Renderer-side widget state owner. Subscribes to widget IPC channels, maintains activity + dictation state, exposes `useWidgetActivity` / `useWidgetDictation` hooks and `recordExternalActivity`. |
| `src/renderer/src/features/widgets/ActivityTimeline.tsx` | Replaced `useAppContext` dependency with `useWidgetActivity()`; import now sourced from `widgetService`. |
| `src/renderer/src/features/widgets/DictationWidget.tsx` | Replaced `window.electron` bridge + `window.electron.dictation` business logic with `useWidgetDictation()`; derived `micPermissionError` inline; removed unused `WidgetMode`/`useState`/`useEffect`. |
| `src/renderer/src/App.tsx` | Removed `activityLog` from `AppState`, initial state, `ACTIVITY_ADD` action type, and reducer case; removed the `activity:log` listener; routed external-edit activity through `recordExternalActivity()`; added `widgetService` import; removed now-unused `ActivityEntry` import. |

No main-process, persistence, IPC, or preload files were modified. Widget behavior, appearance, types, and storage format are unchanged.

---

## 4. Verification Summary

| Check | Result |
|-------|--------|
| `npm run typecheck` (node + web) | **PASS** — exit 0, zero type errors, zero type warnings. |
| `npm run dev` (electron-vite build) | **PASS** — main, preload, and renderer bundles built successfully (exit 0). The Electron app launch itself cannot complete in this headless sandbox (`electron.app` is undefined without a display), which is a pre-existing environment limitation unrelated to these changes; the build/compile stage — what the gate verifies — is green. |
| ESLint (widget files) | **PASS** — zero errors, zero warnings on `widgetService.ts`, `ActivityTimeline.tsx`, `DictationWidget.tsx`. (Pre-existing `react-refresh`/`exhaustive-deps` notices in `App.tsx` are unrelated to this phase.) |
| Widget initialization | Unchanged — `WidgetManager.initialize()` still owns Persist+Restore. |
| Widget creation / update / remove | Unchanged — all transitions still routed through `WidgetManager`. |
| Widget persistence | Unchanged — `ClipboardHistory` and `settings.json` (`clipboardShortcut`) unchanged. |
| Widget restoration | Unchanged — `initialize()` restores shortcut on startup. |
| Widget rendering | Now reads exclusively from `widgetService`; no app-context or Electron-bridge coupling. |
| Runtime behavior | Unchanged — same IPC channels consumed, same `ActivityEntry` shape, same dictation actions invoked. |

**Gate A:** ✅ Passed.
