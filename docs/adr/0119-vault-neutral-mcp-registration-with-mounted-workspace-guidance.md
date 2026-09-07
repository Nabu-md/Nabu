# 0119. Vault-Neutral MCP Registration With Mounted Workspace Guidance

Status: active

Date: 2026-05-14

## Context

Nabu used to register external MCP clients with a durable `VAULT_PATH` environment variable. That made the copied config easy to inspect, but it also meant Claude Code, Cursor, Gemini CLI, and generic MCP clients stayed pinned to whichever vault was active at setup time.

Domenico Lupinetti's dynamic-vault MCP proposal in PR #603 pointed in the right direction: MCP clients should follow Nabu's current workspace state instead of requiring users to reconnect after each vault change. The current app model has also moved from one selected vault toward mounted workspaces, so the MCP server needs to operate on every active mounted vault and load local agent guidance from each vault.

## Decision

Durable external MCP registration is vault-neutral. Nabu still writes an explicit stdio MCP entry, but that entry contains only the Node command, `mcp-server/index.js`, and `WS_UI_PORT=9711`. It no longer writes `VAULT_PATH`.

The Node MCP entrypoints resolve vaults at tool-call time:

- Explicit `VAULT_PATH` and `VAULT_PATHS` environment variables continue to win for app-owned bridge launches and legacy/manual launches.
- When those env vars are absent, the MCP server reads Nabu's `vaults.json`.
- `active_vault` is returned first.
- Every workspace in `vaults[]` is included unless it is explicitly marked `mounted: false`.
- Paths are deduplicated and blank paths are ignored.

Vault context now checks each active mounted workspace root for `AGENTS.md` and returns those instructions with the vault summary. The MCP server also exposes `list_vaults` so agents can discover the active workspace set and whether each vault has root guidance.

We are not adding a session-local `switch_vault` tool. A switch tool would create a second source of truth inside the MCP process, while Nabu already owns mounted workspace state.

## Consequences

External MCP config survives vault switches and mounted-workspace changes without reconnecting.

Agents can work across all active mounted vaults and receive the per-vault `AGENTS.md` instructions needed to respect local rules.

Manual users can still override the resolved workspace set with `VAULT_PATH` or `VAULT_PATHS` when they intentionally want a static or scripted MCP session.
