---
type: ADR
id: "0062"
title: "Selectable CLI AI agents with a shared panel architecture"
status: active
date: 2026-04-13
---

## Context

Nabu's AI panel, onboarding flow, and status surfaces were built around a single CLI dependency: Claude Code. That worked for the first release, but it made every UI and backend seam agent-specific. Adding Codex as a second supported CLI agent would have duplicated large parts of the app: separate availability checks, a second onboarding path, another status badge, and yet another streaming hook.

The product direction is broader than a single vendor. Nabu needs one AI panel that can target multiple local CLI agents while preserving the same MCP-backed vault tooling, the same note-context assembly, and a single install-local preference for which agent should be used by default.

## Decision

**Introduce a shared CLI-agent abstraction for Nabu's AI surfaces.** The frontend now treats agents as a small registry (`claude_code`, `codex`) with labels, install URLs, availability state, and a persisted `default_ai_agent` setting. The AI panel, onboarding gate, command palette, and status bar all read from that shared model. On the backend, `ai_agents.rs` owns agent detection and streaming, dispatching to per-agent adapters: Claude still flows through `claude_cli.rs`, while Codex is launched through `codex exec --json` with Nabu's MCP server injected via transient config flags.

## Options considered

- **Option A** (chosen): shared agent registry + backend adapter layer — one panel, one preference, one onboarding path, and a clear place to add future CLI agents.
- **Option B**: keep the UI Claude-specific and bolt on Codex as a second special case — lowest short-term cost, but every new agent multiplies the number of bespoke checks, prompts, and command handlers.
- **Option C**: split the product into separate per-agent panels — clearer ownership per integration, but fragments the UX and makes command-palette / status-bar interactions inconsistent.

## Consequences

- Positive: new CLI agents can be added by implementing one backend adapter and registering one frontend definition.
- Positive: onboarding and settings now explain the AI capability of the app at the product level rather than assuming Claude Code is the only valid path.
- Positive: the default agent is installation-local, matching ADR-0004's rule that machine-specific tool preferences belong in app settings rather than the vault.
- Negative: event normalization is now Nabu-owned; backend adapters must translate each CLI's stream format into a common event model.
- Negative: some user guidance becomes agent-specific again at the edge, such as install links and authentication errors (`claude` login vs `codex login`).
- Re-evaluate if one agent needs capabilities the shared panel cannot express cleanly, or if Nabu ever moves from CLI subprocesses to a dedicated local SDK/runtime.
