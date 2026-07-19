# ADR-004 — Typed Event Bus

**Status:** Accepted
**Date:** 2026-07-19
**Phase:** Phase 1.5

---

## Context

The main process needs to notify internal background workers (vector indexing,
vault watching, widget coordination, dictation) about asynchronous state
changes — e.g. a vault was opened, an index was rebuilt, a widget was
registered. Before Phase 1.5 there was no first-class mechanism for this; such
signaling, where it existed, was ad-hoc and untyped.

Two communication paths must stay distinct:

1. **Synchronous renderer ↔ main** requests — must remain on the typed IPC
   layer (`shared/ipc`).
2. **Asynchronous internal main-process** notifications — need a lightweight,
   decoupled bus.

Mixing the two would couple the renderer to internal main-process churn and
weaken the IPC contract.

## Decision

Introduce a **typed event bus** in `src/shared/events/`:

- `bus.ts` — a generic `EventBus<Events extends EventMap>` with
  `publish`, `subscribe` (returns an unsubscribe closure), `unsubscribe`,
  `once`, and `clear`. It is **synchronous**, deterministic (dispatch in
  registration order; a throwing listener does not block siblings; the first
  error is re-thrown after all listeners run), and has **zero imports** — no
  Electron, no React, no other `shared` modules.
- `events.ts` — `AppEvents`, the canonical event map: each event has a name, a
  payload contract, a publisher, and subscriber(s) recorded in
  `EVENT_OWNERSHIP`. Eight events are defined (e.g. `VaultOpened`,
  `VaultClosed`, `IndexUpdated`, `WidgetRegistered`, `DictationFinished`).
- `index.ts` — the `appEventBus` singleton, imported by main-process services
  only.

The bus is used **only** for internal, asynchronous main-process background
notifications. The renderer never imports `appEventBus`.

## Rationale

- A typed map gives compile-time safety for event names and payloads, matching
  the rigor already applied to IPC.
- Keeping `bus.ts` dependency-free guarantees it can be imported anywhere in
  main without pulling in Electron/React and without circular dependencies.
- Synchronous, deterministic dispatch makes publisher behavior predictable and
  testable.
- Separating the bus from IPC preserves the IPC contract as the sole
  renderer↔main channel.

## Alternatives Considered

- **Reuse Electron's `webContents` events / `ipcMain.emit`** — rejected; would
  entangle internal signaling with the renderer-facing transport and add
  Electron coupling to `shared`.
- **An external pub/sub library (e.g. EventEmitter-based npm package)** —
  rejected; unnecessary dependency; a tiny in-house bus meets the need and
  stays dependency-free.
- **Make the bus async (Promise-based)** — rejected; internal notifications are
  fire-and-forget and synchronous dispatch is simpler and sufficient.

## Consequences

- Services publish domain events without knowing their subscribers.
- The renderer is fully isolated from the bus (verified: zero
  `shared/events` imports in `renderer`/`preload`).
- Some events (`SearchCompleted`, `NoteSaved`, `NoteDeleted`) are declared and
  reserved but not yet published, because no suitable internal async origin
  exists yet without changing behavior; the registry is complete and ready.

## Future Implications

- Later phases wire subscribers (e.g. VectorManager reacting to `IndexUpdated`)
  as natural internal origins appear — without altering the bus design.
- The bus must never become a back-channel for renderer communication; that
  remains IPC's exclusive role.
