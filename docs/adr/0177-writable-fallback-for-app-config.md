---
type: ADR
id: "0177"
title: "Writable fallback for app config"
status: active
date: 2026-08-19
---

## Context

ADR-0145 made the XDG config directory Nabu's preferred Unix location and retained the platform config directory as a read fallback. That assumes the current desktop account can write the preferred target. A prior administrator launch, restore, or package action can leave `$HOME/.config` or a Nabu config file owned by another account. In that state the app can still read settings, but every save fails. The first-launch telemetry dialog then appears inert because dismissing it depends on persisting the choice.

Running the app as an administrator bypasses the filesystem ownership problem, but is not an acceptable operating requirement. It also does not repair the normal account's durable settings path.

## Decision

**The native app keeps the XDG location as its first choice, but selects the first app config target that the current process can write.**

For each app config file, the shared Rust resolver:

1. Checks whether an existing target can be opened for writing, or whether a new target can be created in its namespace directory.
2. Uses the platform config directory when the XDG target is not writable. On macOS this is `~/Library/Application Support/com.nabu.app/`.
3. Moves the selected writable root to the front of the read order for that file. This prevents an older, unread-only XDG file from shadowing a value saved to the fallback.
4. Keeps the original XDG path as the final write attempt when no candidate is writable, so the actual save operation returns its normal filesystem error.

The renderer's settings save contract reports success or failure. First-launch consent actions are disabled while a save is pending; if persistence still fails, the dialog remains open, restores both actions, and displays a localized retry message.

This decision extends ADR-0145. It does not migrate, delete, change ownership, or rewrite an existing config file.

## Options considered

- **Probe the preferred target and fall back to the platform config root** (chosen): preserves XDG for normal installations while recovering automatically from ownership or permission damage.
- **Always switch macOS to Application Support**: follows the native convention but can strand newer XDG-backed settings and breaks the explicit portability decision in ADR-0145.
- **Change ownership or permissions automatically**: mutates user filesystem security policy and may require elevation, so Nabu must not do this silently.
- **Only show the backend error**: makes the failure understandable but leaves a normal user unable to finish onboarding when a writable per-user platform root is available.

## Consequences

- A normal macOS account can complete first launch even when its Nabu XDG target was created by an administrator.
- Existing writable XDG installations keep the same paths and behavior.
- A stale unwritable file may remain on disk; Nabu intentionally does not alter its ownership or permissions.
- The consent dialog no longer looks frozen when all candidate config locations fail.
- Config consumers continue to use `src-tauri/src/app_config.rs`; they must not duplicate path or permission fallback logic.
