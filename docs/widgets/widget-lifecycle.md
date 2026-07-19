# Widget Lifecycle

Permanent technical documentation for the Nabu widget subsystem. This document
describes the implementation currently in the repository (post Phases 3.1–3.3).

---

## Overview

Nabu has two distinct widget surfaces:

1. **The clipboard/dictation widget** — an always-on-top, transparent, frameless
   `BrowserWindow` (`#/widget` route) driven by `WidgetManager` on the main
   process. It shows clipboard history or a dictation waveform.
2. **In-app widget UI** — React components rendered inside the main window:
   - `ActivityTimeline` (activity log panel)
   - `DictationWidget` (the component rendered inside the widget window)

Ownership is split by layer:

- **Main process** owns the widget *window lifecycle* and the authoritative
  in-memory `WidgetState` (`WidgetManager`).
- **Renderer** owns the widget *rendered state* through a single
  `widgetService` module, which is the only source the widget UI reads from.

The widget UI never owns widget state. It only reads from `widgetService` and
invokes widget-specific actions exposed by that service.

---

## Lifecycle

The widget subsystem follows one deterministic lifecycle:

```
Create
  ↓
Register
  ↓
Render
  ↓
Update
  ↓
Persist
  ↓
Restore
  ↓
Remove
  ↓
Cleanup
```

### Create
`WidgetManager.createWidgetWindow()` lazily creates the `BrowserWindow` on the
first `show()`. It loads the `#/widget` route, wires the `closed` handler, and
publishes `WidgetRegistered` on the app event bus.

### Register
`WidgetManager` is the registry owner. `registerWidgetsIPC()` registers all
`widget:*`, `kanban:*`, and `clipboard:history:*` IPC handlers. The
`WidgetRegistered` event is published once the window exists.

### Render
The renderer consumes widget state via `widgetService`:
- `ActivityTimeline` renders from `useWidgetActivity()`.
- `DictationWidget` renders from `useWidgetDictation()`.

### Update
All updates route through `WidgetManager` methods (`show`, `switchMode`,
`setModel`, `setMicPermission`, `setShortcut`, `insertTextAtCursor`). The
manager pushes state to the widget window via IPC channels
(`widget:mode-changed`, `widget:dictation-*`, `widget:insert-text`). The
renderer `widgetService` subscribes to those channels and updates its local
state, which re-renders the UI.

### Persist
- Clipboard entries: `ClipboardHistory` service (file-backed).
- Shortcut: `settings.json` (`clipboardShortcut`), written by
  `widget:set-shortcut` (which calls `setShortcut` + `saveSettings`) and by the
  Settings panel.
- No duplicate persistence: the IPC handler is the single authoritative write
  path for the shortcut.

### Restore
`WidgetManager.initialize()` is the single Persist+Restore entry point. It
calls `loadSettings()` and `setEnabled(true, settings.clipboardShortcut)`,
restoring the persisted shortcut on startup. No other caller duplicates this
sequence.

### Remove
`WidgetManager.remove()` (and `destroy()`, which delegates to it) hides and
closes the widget window, releasing all resources. This is the single
authoritative removal path.

### Cleanup
The `closed` handler nulls the window reference; `fnMonitor` wiring is torn
down with the process. Renderer subscriptions are cleaned up by React effect
teardown (`subscribeActivity` / `subscribeDictation` return unsubscribe fns).

There is **exactly one deterministic lifecycle owner**: `WidgetManager`.

---

## Ownership

| Concern | Owner |
|---------|-------|
| Lifecycle owner | `WidgetManager` (`src/main/services/widget-manager.ts`) |
| Registry owner | `WidgetManager` (in-memory `WidgetState` + `WidgetRegistered` event) |
| Persistence owner | `ClipboardHistory` (clipboard entries) + `settings` service (`clipboardShortcut`) |
| Rendering owner (state) | `widgetService` (`src/renderer/src/features/widgets/widgetService.ts`) |
| Rendering owner (UI) | `ActivityTimeline` / `DictationWidget` (read-only consumers) |
| IPC handlers | `registerWidgetsIPC()` (`src/main/ipc/widgets.ts`) — thin delegators |
| Preload bridge | `src/preload/index.ts` — typed, contract-derived |

---

## Persistence

### Save flow
1. `widget:set-shortcut` IPC → `widgetManager.setShortcut(shortcut)` (in-memory)
   + `saveSettings({ ...current, clipboardShortcut })` (disk).
2. Clipboard entries are written by `ClipboardHistory` as they occur.

### Load flow
1. On startup, `WidgetManager.initialize()` → `loadSettings()` reads
   `clipboardShortcut` and calls `setEnabled(true, shortcut)`.
2. Clipboard history is read on demand via `clipboard:history-get`.

### Removal flow
- `WidgetManager.remove()` / `destroy()` closes the window. The persisted
  shortcut remains in `settings.json` (it is a user preference, not a
  per-instance widget record), so the widget restores on next launch. Clipboard
  history is pruned by `ClipboardHistory` independently.

---

## Rendering

### Registry → renderer flow
```
WidgetManager (main, authoritative state)
        │  IPC channels (widget:mode-changed, widget:dictation-*,
        │              widget:insert-text, activity:log)
        ▼
widgetService (renderer, single source of truth for rendered state)
        │  hooks: useWidgetActivity(), useWidgetDictation()
        ▼
ActivityTimeline / DictationWidget (read-only UI)
```

### State synchronization
- `widgetService` subscribes once to each widget channel (idempotent
  `ensure*Subscription`).
- Activity entries are prepended and capped at 100 (mirrors prior reducer
  behavior). External edits are recorded via `recordExternalActivity()`.
- Dictation state transitions: `mode-changed`/`dictation-starting` → `listening`;
  `dictation-complete` → `complete`; `dictation-error` → `error`.
- Equivalent application states always produce identical rendered state because
  the UI reads from one owner, not from divergent sources.

### Rendering ownership
The UI is a pure function of `widgetService` state. It never mutates widget
state and never reaches into the global app context or the raw Electron bridge.

---

## Maintenance Guide

### How to add a widget
1. Add the window/state in `WidgetManager` (main process) if it needs a
   dedicated window, or add a renderer component under
   `src/renderer/src/features/widgets/`.
2. Expose any required IPC channels via the typed contract registry
   (`src/shared/contracts`, `src/shared/ipc`) and register handlers in
   `registerWidgetsIPC()`.
3. If the renderer needs state, extend `widgetService` (add a state slice +
   `subscribe*` + `use*` hook). Do **not** add widget state to the global app
   reducer.
4. Render the component from the `use*` hook only.

### Lifecycle expectations
- Every transition (create/register/render/update/persist/restore/remove) must
  flow through `WidgetManager` on the main side and `widgetService` on the
  renderer side.
- `WidgetManager.initialize()` is the only startup restore path.

### Ownership rules
- `WidgetManager` owns lifecycle + in-memory state.
- `widgetService` owns rendered state on the renderer.
- IPC handlers are thin delegators — no lifecycle logic inside them.
- UI components are read-only consumers of `widgetService` hooks.

### Persistence requirements
- Persist user preferences (e.g. shortcut) via `settings`.
- Persist widget content (e.g. clipboard history) via its dedicated service.
- Never persist from the renderer UI; persistence is owned by the main process.
- Ensure no duplicate write paths for the same persisted value.
