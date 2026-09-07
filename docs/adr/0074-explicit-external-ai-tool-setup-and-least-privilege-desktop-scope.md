---
type: ADR
id: "0074"
title: "Explicit external AI tool setup and least-privilege desktop scope"
status: active
date: 2026-04-22
---

## Context

Nabu's first MCP integration optimized for zero setup: desktop startup auto-registered the Nabu MCP server in Claude Code and Cursor config files, the Tauri asset protocol allowed every local path, and app-managed Codex sessions launched with the CLI's dangerous bypass flag. That made the product feel convenient, but it also widened trust by default in places that users could not see or consent to clearly.

The product direction now favors least-privilege defaults. Fresh installs should not silently edit third-party config files, external AI tool setup must be intentional and reversible, and the desktop shell should only expose the filesystem paths that the active vault actually needs.

## Decision

**Nabu now treats external AI tool wiring as an explicit user action and keeps the desktop shell scoped to the active vault.**

- The app still spawns its local MCP WebSocket bridge on desktop startup, but it no longer auto-registers third-party MCP config files.
- External MCP registration is exposed through a keyboard-accessible setup flow reachable from the command palette and status surfaces. Confirming the flow upserts Nabu's MCP entry for the current vault; cancel leaves external config untouched; disconnect removes Nabu's entry again.
- The Tauri asset protocol remains enabled for local vault images, but its static config scope is empty. Nabu grants recursive asset access only to the active vault at runtime when that vault is reloaded.
- App-managed Codex sessions use the CLI's normal approval and sandbox path by default instead of opting into the dangerous bypass mode automatically.

## Options considered

- **Explicit setup + runtime vault-only scope** (chosen): aligns with least-privilege defaults, keeps command-palette discoverability, preserves image loading and external-tool support, and makes every privileged step visible and reversible.
- **Keep startup auto-registration and global asset scope**: lowest friction, but it silently mutates third-party config and leaves the desktop shell effectively open to every local file path.
- **Disable external MCP registration entirely**: safest on paper, but it removes a valuable workflow for Claude Code, Cursor, and other MCP-compatible tools that Nabu intentionally supports.

## Consequences

- Fresh installs no longer modify `~/.claude/mcp.json` or `~/.cursor/mcp.json` until the user confirms setup.
- Switching vaults does not silently retarget external MCP clients; users reconnect explicitly when they want a different vault exposed.
- Desktop asset access is constrained to the active vault instead of all filesystem paths, while note images and attachments continue to load normally.
- The command palette and status bar now expose an explicit external AI tools setup/remove flow that supports keyboard-only QA.
- Codex agent sessions are safer by default, at the cost of relying on the CLI's normal approval path instead of bypassing it automatically.
