---
type: ADR
id: "0118"
title: "Entry-scoped note windows without vault index scans"
status: active
date: 2026-05-14
---

## Context

ADR-0031 kept secondary note windows on the full `App` shell so they would inherit the same editor capabilities as the primary window. That decision also accepted a full vault load per secondary window as the simpler trade-off.

In practice, repeated note-window opens were paying that full-vault scan cost even when the window only needed one known note path. The startup path loaded the vault index and passed related entries into the editor even though note-window mode renders a single-note surface. That extra work increased window-open cost and made every secondary window depend on repository-wide entry hydration for a workflow that is intentionally scoped to one note.

Nabu still wants note windows to reuse the main App architecture rather than reviving a separate `NoteWindow` shell. The missing decision was how far the shared vault loader should go when the window contract is already narrowed to a single entry.

## Decision

**Secondary note windows continue to render the full `App` shell, but they no longer load the full vault index during startup.** In note-window mode, Nabu skips the shared vault-entry scan, reloads only the requested note entry, and scopes editor entry props to that active note instead of passing repository-wide visible entries.

This keeps the architectural benefit of ADR-0031 (one window architecture, one editor surface) while changing the data-loading contract for secondary windows from vault-scoped to entry-scoped.

## Alternatives considered

- **Full `App` shell with entry-scoped loading** (chosen): preserves feature parity in the shared shell while removing unnecessary full-vault scans for a one-note window. Trade-off: note windows should not assume vault-index-derived context is available by default.
- **Full `App` shell with full vault scan**: simplest continuation of ADR-0031, but repeats avoidable repository-wide I/O every time a note window opens.
- **Dedicated `NoteWindow` shell**: could be lighter still, but reintroduces the architectural drift and duplicated feature work that ADR-0031 intentionally removed.

## Consequences

- Opening a secondary note window no longer requires `list_vault`/full entry hydration before the editor can render the requested note.
- Repeated note-window opens avoid redundant vault scans and stay aligned with the product contract that these windows are single-note work surfaces.
- Features inside note-window mode must treat vault-index-derived data as opt-in; they cannot assume related entries are already loaded just because the full `App` shell is mounted.
- ADR-0031 remains directionally valid for shared window architecture, but its original "full vault load per secondary window" trade-off is no longer the operating model.
- Re-evaluate if note windows later need immediate repository-wide browsing context, or if future profiling shows the remaining single-entry reload path is still too expensive.
