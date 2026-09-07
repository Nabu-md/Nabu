---
type: ADR
id: "0114"
title: "Mounted workspaces unified graph"
status: active
date: 2026-05-07
---

## Context

Nabu users can already register multiple vaults, but switching vaults historically replaced the active graph. That model breaks down when separate Git repositories represent different workspaces that still need to reference each other: search, quick-open, wikilink navigation, and note lists should see one graph, while Git status, folders, saved views, and sync controls remain scoped to the repository currently in focus.

The app also needs a stable way to disambiguate same-named notes across repositories without writing machine-specific paths into Markdown. A full storage migration or database-backed graph would conflict with Nabu's filesystem-first model and make separate Git histories harder to reason about.

## Decision

**Nabu treats the registered vault list as an installation-local mounted-workspace set and annotates loaded entries with workspace provenance.**

Specifically:

1. `vaults.json` persists workspace identity (`label`, stable `alias`, color, mount flag) and the default workspace path for newly created notes.
2. `useVaultLoader` loads entries from every available mounted workspace and attaches `WorkspaceIdentity` to each `VaultEntry` before React consumes the combined graph.
3. Active-vault switching remains the focus control for Git, folder tree, saved views, watchers, repair, and other per-repository operations.
4. Wikilinks stay Markdown-first. Same-workspace links remain vault-relative; cross-workspace canonical links are prefixed with the target workspace alias.
5. Note reads and writes for absolute paths can resolve the deepest registered vault root at the Tauri boundary when no explicit `vaultPath` is supplied, preserving path-containment validation across mounted workspaces.

## Alternatives considered

- **Mounted workspace provenance on `VaultEntry` with alias-prefixed links** (chosen): preserves filesystem/Git independence while letting UI graph surfaces operate across repositories.
- **Merge separate repositories into one vault**: avoids cross-root resolution, but forces users to collapse unrelated Git histories and permissions into one repo.
- **Persist absolute paths in wikilinks**: disambiguates locally, but makes notes non-portable and leaks machine paths into user data.
- **Store a global graph database**: could make cross-workspace queries faster, but violates the cache-is-disposable rule and adds a new source of truth.

## Consequences

- Search, quick-open, note lists, and wikilink navigation can operate across mounted workspaces.
- UI surfaces that show ambiguous note names should display compact workspace provenance only when more than one workspace is present.
- New notes and Type files are created in the configured default workspace, falling back to the active workspace if the default is unavailable or unmounted.
- Backend command boundaries must continue validating every disk operation against a registered root; mounted workspaces do not loosen filesystem access.
- Future per-workspace features should distinguish graph-wide behavior from active-repository behavior before adding state or commands.
