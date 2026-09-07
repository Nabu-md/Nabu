---
type: ADR
id: "0146"
title: "Cached main-window startup with empty reload recovery"
status: active
date: 2026-06-28
supersedes: "0124"
---

## Context

ADR-0124 allowed secondary note windows to use the cached/incremental `list_vault` path, but kept normal main-window startup on a forced `reload_vault`.

That forced reload invalidates the cache and runs a full filesystem scan before the main window finishes indexing. On large local macOS vaults this can make startup look hung for tens of seconds, with the status bar stuck in the vault reloading state even when a healthy cached index is available.

Nabu still needs the recovery behavior that motivated the fresh reload path: if a startup cache returns an empty result for a vault that should contain notes, the app should recover with a fresh scan instead of leaving the user with an empty graph.

## Decision

**Main-window startup uses the cached/incremental `list_vault` path first, just like secondary note windows. The main window performs a `reload_vault` fallback only when that initial cached result is empty.**

Explicit user reloads, watcher/external-edit refreshes, Git pull refreshes, and other freshness-critical paths continue to call `reload_vault` through the existing reload abstractions.

## Consequences

- Healthy cached starts avoid invalidating the vault cache and do not pay a full rescan just to mount the main window.
- Empty-cache or stale-empty startup regressions still recover through one fresh `reload_vault` pass in the main window.
- Secondary note windows keep the ADR-0124 behavior and do not use startup empty-result recovery, avoiding surprise full scans when opening many note windows.
- The backend cache remains responsible for incremental freshness when `list_vault` returns non-empty cached entries.
- Future startup performance changes must preserve the distinction between initial cached hydration and explicit freshness reloads.
