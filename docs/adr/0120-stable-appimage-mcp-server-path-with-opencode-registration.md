# 0120. Stable AppImage MCP Server Path With OpenCode Registration

Status: active

Date: 2026-05-14

## Context

Domenico Lupinetti's PR #600 identified two gaps in durable external MCP setup:

- Linux AppImage launches expose bundled resources through a mount path that can change between app starts, so external clients can keep a stale `mcp-server/index.js` path.
- OpenCode uses `~/.config/opencode/opencode.json` with a different MCP schema from Claude Code, Cursor, Gemini, and generic `mcpServers` clients.

ADR-0119 made durable MCP registration vault-neutral, so PR #600 could not be merged directly: its registered entries still carried `VAULT_PATH`. The stable-path and OpenCode work is still valid, but it has to preserve the current mounted-workspace resolution model.

## Decision

On Linux AppImage startup, Nabu extracts the bundled `mcp-server/` directory to `~/.local/share/nabu/mcp-server/`. The extracted directory is version-gated by a `.nabu-version` marker. Extraction runs on first launch or after an app version change, uses a staging directory plus rename, and uses a process lock so concurrent app launches do not write the stable directory at the same time.

Durable external registration prefers the stable extracted server directory when it is ready. Otherwise it falls back to the packaged resource resolver.

OpenCode is added to durable MCP registration and removal. Nabu writes an OpenCode-specific entry under the top-level `mcp` key using:

- `type: "local"`
- `command: [node, index.js]`
- `enabled: true`
- `environment.WS_UI_PORT = "9711"`

OpenCode registration remains vault-neutral. It does not write `VAULT_PATH`; the Node MCP server resolves active mounted workspaces from Nabu state per ADR-0119.

## Consequences

Linux AppImage users can register external MCP clients once and keep a valid `index.js` path across restarts and updates.

OpenCode participates in the same connect, disconnect, and status flow as Claude Code, Cursor, Gemini, and generic MCP clients while preserving its own config schema.

The stable path fixes the packaging lifecycle without reintroducing static vault pinning.
