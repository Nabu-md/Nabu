---
type: ADR
id: "0145"
title: "XDG-backed app config path"
status: active
date: 2026-06-27
---

## Context

Nabu stores installation-local state such as settings, registered workspaces, window state, AI workspace session metadata, and local AI provider secrets outside the vault. The product rule is still that vault-shaped content belongs in the vault, while machine- and installation-specific preferences stay in app config.

Users also want this app config to be portable through dotfile backup workflows. The existing implementation documented `~/.config/com.nabu.app`, but the Rust path resolver used the platform config directory directly and duplicated that decision in settings and vault-list code. That made `$XDG_CONFIG_HOME` support unclear and increased the risk that new app config files would pick a different path.

## Decision

**Nabu resolves app-owned config files through one Rust helper that follows the XDG config location on Unix platforms and keeps the platform config directory as a read fallback.**

All app-owned config JSON remains under the `com.nabu.app` namespace:

```text
${XDG_CONFIG_HOME:-$HOME/.config}/com.nabu.app/settings.json
${XDG_CONFIG_HOME:-$HOME/.config}/com.nabu.app/vaults.json
${XDG_CONFIG_HOME:-$HOME/.config}/com.nabu.app/window-state.json
${XDG_CONFIG_HOME:-$HOME/.config}/com.nabu.app/ai-provider-secrets.json
```

If `XDG_CONFIG_HOME` is missing on Unix platforms, Nabu uses `$HOME/.config`. If `XDG_CONFIG_HOME` is relative, Nabu ignores it and falls back to the next valid config root. Windows keeps using the platform config directory unless the user sets an absolute `XDG_CONFIG_HOME`.

Reads check the preferred Nabu namespace first, then the legacy `com.laputa.app` namespace, and then the previous platform config directory when it differs from the XDG location. Writes always go to the Nabu namespace under the preferred config root. The helper lives in `src-tauri/src/app_config.rs`, and app config consumers should call it instead of joining their own config root.

## Options considered

- **Use the XDG config home on Unix and preserve platform read fallback** (chosen): makes dotfile-backed config explicit without changing the JSON file formats or stranding existing platform-config installs.
- **Always force `~/.config` on every platform**: maximizes visible dotfile portability, but ignores OS conventions for users who have not opted into XDG config.
- **Keep per-module path helpers**: preserves the status quo, but makes future config files easy to place inconsistently.
- **Move installation-local state into the vault**: would sync more data, but would mix device preferences, credentials, and window state into user content.

## Consequences

- Users can back up Nabu's app config JSON alongside other dotfile-managed tools.
- Existing installs keep working because reads still check `com.laputa.app` and the previous platform config directory when no preferred XDG/Nabu file exists.
- Relative `XDG_CONFIG_HOME` values are ignored so Nabu does not write config relative to an arbitrary process working directory.
- New app-owned config files should use the shared Rust helper and document whether they are safe to back up. Secrets stay outside vaults and worktrees, but users who back up their XDG directory need to treat those files as sensitive.
