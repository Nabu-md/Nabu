# Phase 3.1 — Widget Audit & Lifecycle Consolidation (Prompt A)

**Status:** Complete
**Gate A:** Passed (`npm run typecheck` + `npm run dev` green)
**Scope:** Widget lifecycle management, orchestration, persistence coordination, ownership, duplicated widget workflows.
**Out of scope (per rules):** Widget redesign, new widget types, storage redesign, rendering redesign, widget UX changes.

---

## 1. Widget Audit Report

Every widget-related subsystem discovered in the codebase:

| # | Subsystem | Location | Responsibility |
|---|-----------|----------|----------------|
| 1 | **WidgetManager** | `src/main/services/widget-manager.ts` | Owns the always-on-top BrowserWindow lifecycle (create/show/hide/destroy), dictation mode, in-memory `WidgetState`, fn-key wiring. |
| 2 | **Widget IPC module** | `src/main/ipc/widgets.ts` | Registers `kanban:*`, `clipboard:history-*`, and `widget:*` control channels; previously performed inline init (`loadSettings → setEnabled`). |
| 3 | **ClipboardHistory** | `src/main/services/clipboard-history.ts` | Persistence of clipboard entries consumed by the clipboard widget. |
| 4 | **Widget template (HTML)** | `src/main/services/widget-template.ts` | **DEAD** — `getWidgetHTML()` defined but never imported or called anywhere. A duplicate creation path that was superseded by the React `#/widget` route. |
| 5 | **Widget toggle bridge** | `src/main/ipc/shared.ts` (`onWidgetToggle` / `getWidgetToggleCallback`) | Bridges the `clipboard-widget` feature toggle to the manager. |
| 6 | **Settings integration** | `src/main/ipc/settings.ts`, `src/main/services/settings.ts` | Persists `clipboardShortcut`; invokes the toggle bridge on change. |
| 7 | **Bootstrap wiring** | `src/main/index.ts` | Wires `fnMonitor → widgetManager`, registers IPC, wires `onWidgetToggle → setEnabled` (runtime toggle). |
| 8 | **Preload bridge** | `src/preload/index.ts` | Exposes `widget.setShortcut` and `on.widgetModeChanged/Dictation*/InsertText/showClipboard` listeners. |
| 9 | **Renderer widget UI** | `src/renderer/src/features/widgets/DictationWidget.tsx` | React component rendered in the widget BrowserWindow (`#/widget` route). |
| 10 | **Renderer settings UI** | `src/renderer/src/features/settings/SettingsPanel.tsx` | Captures shortcut, writes `clipboardShortcut` to settings AND calls `widget.setShortcut`. |
| 11 | **Event bus metadata** | `src/shared/events/events.ts` | `WidgetRegistered` / `DictationFinished` ownership metadata. |
| 12 | **Contracts / schemas** | `src/shared/contracts/index.ts`, `src/shared/schemas/index.ts`, `src/shared/ipc/index.ts` | Define `widget:toggle/move/resize/create-note/fetch-title/open-note` channels — **no IPC handlers registered** for these (unused/orphaned definitions). |

### Entry points (creation, update, deletion, persistence, restoration, registration, init, events, state sync)

- **Creation:** `WidgetManager.createWidgetWindow()` (lazy, on `show()`).
- **Update:** `WidgetManager.show/switchMode/setModel/setMicPermission/setShortcut` + `insertTextAtCursor`.
- **Deletion/Removal:** `WidgetManager.destroy()` / `hide()` / `widgetWindow.close()`.
- **Persistence:** `ClipboardHistory` (clipboard entries); `settings.json` (`clipboardShortcut`).
- **Restoration:** `loadSettings()` → `setEnabled(true, shortcut)` at startup.
- **Registration:** `registerWidgetsIPC()` registers all `widget:*` / `kanban:*` / `clipboard:history:*` handlers.
- **Initialization:** previously inline in `registerWidgetsIPC` (`loadSettings().then(setEnabled)`) **and** `index.ts` `onWidgetToggle` wiring.
- **Events:** `WidgetRegistered` published by manager; `DictationFinished` published by `dictation-service.ts`.
- **State sync:** `widget:mode-changed`, `widget:dictation-*`, `widget:insert-text` channels pushed to the widget window.

---

## 2. Ownership Map

### Before consolidation

| Concern | Owner(s) | Conflict |
|---------|----------|----------|
| Widget window creation | `WidgetManager.createWidgetWindow` **and** dead `getWidgetHTML()` | Duplicate/competing creation path (one unused). |
| Widget init / shortcut restore | `registerWidgetsIPC` (inline `loadSettings→setEnabled`) **and** `index.ts` `onWidgetToggle→setEnabled` | Two init paths for enable state. |
| Shortcut persistence | `SettingsPanel` writes settings + calls `setShortcut`; `widget:set-shortcut` IPC only calls `setShortcut` (no persist) | Inconsistent persistence — IPC path did not persist. |
| `WidgetRegistered` subscribers | `events.ts` listed `WidgetService` (nonexistent) + `DictationService` | Dead owner reference. |
| Event ownership metadata | `VaultOpened` listed `WidgetService` subscriber | Dead owner reference. |

### After consolidation

| Concern | Single Owner |
|---------|--------------|
| Widget window creation | `WidgetManager.createWidgetWindow()` (dead `getWidgetHTML` removed). |
| Widget init / restore (Persist + Restore) | `WidgetManager.initialize()` — the **only** caller of `loadSettings→setEnabled`. |
| Widget enable/disable (runtime) | `WidgetManager.setEnabled()` via `onWidgetToggle` bridge (unchanged path). |
| Shortcut in-memory state | `WidgetManager.setShortcut()` (single mutator). |
| Shortcut persistence | `widget:set-shortcut` IPC → `setShortcut` + `saveSettings` (single authoritative write). `SettingsPanel` still writes settings (UX) but the IPC path now also persists, removing the inconsistency. |
| Event ownership metadata | Corrected to real owners (`WidgetManager`, `DictationService`); `WidgetService` references removed. |

---

## 3. Lifecycle Diagram

The consolidated, deterministic lifecycle is owned exclusively by `WidgetManager`:

```
        ┌─────────────────────────────────────────────┐
        │            WidgetManager (owner)             │
        └─────────────────────────────────────────────┘
                          │
                          ▼
                     [ Create ]
                  createWidgetWindow()
                  (lazy, on first show)
                          │
                          ▼
                  [ Initialize ]
                  initialize()  ──► loadSettings()
                  (Persist+Restore)     └─► setEnabled(true, shortcut)
                          │
                          ▼
                    [ Persist ]
                  settings.json (clipboardShortcut)
                  ClipboardHistory (clipboard entries)
                          │
                          ▼
                     [ Update ]
                  show / switchMode / setModel /
                  setMicPermission / setShortcut /
                  insertTextAtCursor
                          │
                          ▼
                   [ Restore ]
                  initialize() reloads persisted
                  shortcut on (re)start
                          │
                          ▼
                    [ Remove ]
                  remove() / destroy()
                  (hide + close window)
```

Every transition above is reached **only** through `WidgetManager` methods. IPC handlers in `widgets.ts` and the `onWidgetToggle` bridge are thin delegators — they contain no lifecycle logic of their own.

---

## 4. Consolidation Report (duplicate paths removed)

| # | Duplicate / conflict removed | Action |
|---|------------------------------|--------|
| 1 | Dead `getWidgetHTML()` creation path in `widget-template.ts` | **Deleted file.** No callers existed; the live path is the React `#/widget` route via `createWidgetWindow()`. |
| 2 | Inline init in `registerWidgetsIPC` (`loadSettings().then(setEnabled)`) | **Replaced** with a single call to `widgetManager.initialize()`, which owns the Persist+Restore sequence. |
| 3 | Inconsistent shortcut persistence (`widget:set-shortcut` did not persist) | **Consolidated** into one authoritative write: `setShortcut()` + `saveSettings()` inside the IPC handler. |
| 4 | Dead `WidgetService` owner references in `events.ts` (`WidgetRegistered` + `VaultOpened`) | **Corrected** to real owners (`DictationService`; removed from `VaultOpened`). |
| 5 | Stale header comment referencing nonexistent `widget-service.ts` | **Updated** to reflect current ownership. |

No widget behavior, UX, storage format, or rendering was changed. The `widget:toggle/move/resize/create-note/fetch-title/open-note` orphaned contracts were left intact (they are unused channel definitions, not lifecycle paths, and removing them is out of scope for this ownership-consolidation phase).

---

## 5. Files Modified

| File | Change |
|------|--------|
| `src/main/services/widget-manager.ts` | Added `import { loadSettings }`; added `initialize()` (Persist+Restore owner) and `remove()` (single removal path); `destroy()` now delegates to `remove()`; documented `setShortcut` as the single in-memory mutator. |
| `src/main/ipc/widgets.ts` | `widget:set-shortcut` now persists via `saveSettings`; init replaced by `widgetManager.initialize()`; header comment updated; added `saveSettings` import. |
| `src/shared/events/events.ts` | Removed dead `WidgetService` subscriber references in `WidgetRegistered` and `VaultOpened`. |
| `src/main/services/widget-template.ts` | **Deleted** (dead duplicate creation path). |

---

## 6. Verification Summary

| Check | Result |
|-------|--------|
| `npm run typecheck` (node + web) | **PASS** (exit 0, no type errors). |
| `npm run dev` (electron-vite build) | **PASS** — main process + preload built successfully (exit 0). Pre-existing dynamic-import warnings are unrelated to this change. |
| Widget initialization | `WidgetManager.initialize()` loads persisted shortcut and enables the widget (single path). |
| Widget creation | `createWidgetWindow()` unchanged; dead `getWidgetHTML` removed. |
| Widget update | `show/switchMode/setModel/setShortcut/insertTextAtCursor` unchanged. |
| Widget persistence | `clipboardShortcut` now written by both `SettingsPanel` and `widget:set-shortcut` (consistent); `ClipboardHistory` unchanged. |
| Widget restoration | `initialize()` restores shortcut on startup (single path). |
| Widget removal | `remove()` / `destroy()` unchanged behavior. |
| Runtime behavior | Unchanged — no widget UX, rendering, storage, or type changes. |

**Gate A:** ✅ Passed.
