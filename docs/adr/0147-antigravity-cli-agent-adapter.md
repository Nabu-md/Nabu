---
type: ADR
id: "0147"
title: "Antigravity CLI agent adapter"
status: active
date: 2026-06-28
supersedes:
  - "0091"
  - "0097"
---

## Context

ADR-0091 and ADR-0097 made Gemini CLI the Google-backed CLI agent path for durable external MCP setup and app-managed AI panel sessions. Antigravity CLI is now the maintained successor path, with official one-shot execution through `agy -p ... --cwd`, dedicated Antigravity MCP configuration in `~/.gemini/config/mcp_config.json` or workspace `.agents/mcp_config.json`, and continued support for `GEMINI.md` / `AGENTS.md` workspace rules during Gemini CLI migration.

Keeping `gemini` as Nabu's product-facing local-agent id would leave users pointed at deprecated setup docs and the old `~/.gemini/settings.json` MCP shape.

## Decision

Nabu replaces the app-managed Gemini CLI agent with an Antigravity CLI adapter.

The product-facing local agent id is `antigravity`. Existing stored `gemini` default-agent values and legacy status payloads normalize to `antigravity` so older settings do not get dropped.

The desktop backend:

- discovers `agy` through PATH, login-shell lookup, and common local install locations, including `~/.local/bin/agy`
- runs `agy -p <prompt> --cwd <vault>` from the active vault
- writes transient workspace MCP config to `.agents/mcp_config.json` for app-managed sessions
- maps Safe mode to `--sandbox=true --toolPermission=proceed-in-sandbox`
- maps Power User mode to `--sandbox=false --toolPermission=always-proceed`
- avoids `--dangerously-skip-permissions`
- streams line-oriented stdout into Nabu AI panel text events and reports setup/auth failures with Antigravity-specific recovery copy

Durable external MCP setup now writes the standard Nabu MCP entry to `~/.gemini/config/mcp_config.json` for Antigravity CLI instead of the legacy Gemini CLI `~/.gemini/settings.json` path. The optional vault-root `GEMINI.md` compatibility shim remains valid because Antigravity CLI continues to parse Gemini-compatible context files during migration.

## Consequences

- Users see Antigravity CLI in agent pickers, onboarding, command registry, and install links.
- Older `default_ai_agent: "gemini"` settings continue to select the Google-backed CLI path after migration.
- Existing standalone Gemini CLI global config is no longer mutated by Nabu's durable MCP setup.
- Antigravity end-to-end native QA requires `agy` to be installed and authenticated; local tests use fake executables for deterministic command/config coverage.
- Future Antigravity CLI flag changes must update the adapter tests and this ADR rather than silently falling back to dangerous bypass flags.
