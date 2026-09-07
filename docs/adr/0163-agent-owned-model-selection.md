---
type: ADR
id: "0163"
title: "Agent-owned model discovery and selection"
status: active
date: 2026-07-19
---

## Context

Nabu can run several independently installed CLI agents. Some agents expose a model catalog, some document stable aliases, and others do not offer a reliable machine-readable model interface. Treating all agents as if they shared one catalog would mix incompatible identifiers, couple the renderer to CLI-specific behavior, and risk silently sending a removed model after an agent upgrade.

## Decision

**Each CLI adapter owns its model capabilities, while the AI workspace persists only the selected opaque model id for that agent and conversation.**

The native `get_ai_agent_model_catalog` command returns capabilities only for adapters that can make a reliable promise. Codex discovers its visible catalog through `codex debug models`; Claude Code exposes its documented stable aliases. Discovery failures omit that adapter's selector instead of blocking the workspace. Agents without a verified catalog remain on their normal default behavior.

The renderer always presents an explicit Agent default choice. It stores a preference per agent in installation-local storage and records the active model id with AI workspace conversation metadata. Starting a new chat applies that agent's preference. Switching agents restores the destination agent's preference. Direct API/local model targets remain a separate target kind and never consume a CLI-agent model id.

At execution time, the model id travels through the shared agent request but only the selected adapter translates it into its own CLI flag. Empty values are omitted so the CLI chooses its default. If a saved model disappears from the latest catalog, Nabu clears it before the next run, uses Agent default, and adds a visible transcript marker.

## Alternatives considered

- **One hard-coded catalog for every agent:** predictable to render, but becomes stale and assigns identifiers to adapters that may not accept them.
- **Free-form model text:** supports unknown models, but makes typos and removed identifiers fail only after a prompt is sent.
- **Store one global model preference:** simpler persistence, but model identifiers are agent-specific and would leak across incompatible adapters.
- **Reuse direct API model targets:** conflates a provider request made by Nabu with a CLI agent that owns its authentication, tools, and execution policy.

## Consequences

- Adapter capability discovery can grow independently as more CLIs expose stable interfaces.
- Missing or failed discovery degrades to the existing agent-default execution path.
- Conversation metadata gains an optional model id but still stores no prompts, transcripts, credentials, or note content.
- Analytics records only agent id, default-vs-explicit choice, surface availability, and fallback reason; model names are excluded.
