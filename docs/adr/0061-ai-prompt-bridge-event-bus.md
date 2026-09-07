---
type: ADR
id: "0061"
title: "AI prompt bridge — module-level event bus for cross-component prompt routing"
status: active
date: 2026-04-13
---

## Context

The AI panel is a sibling subtree to the command palette in the component tree. When the user submits a prompt from the command palette's AI mode, the AI panel (mounted elsewhere) needs to receive it and start processing. Props-down / callbacks-up wiring between the two would require threading state through multiple layers of unrelated components.

## Decision

**Introduce `aiPromptBridge.ts` as a module-level singleton event bus.** The bridge exposes `queueAiPrompt(text, references)` (write path) and `takeQueuedAiPrompt()` (consume path), backed by a module variable and a `CustomEvent` on `window` (`nabu:ai-prompt-queued`). The command palette enqueues a prompt; the AI panel listens for the event, consumes the prompt via `takeQueuedAiPrompt`, and dispatches it to the agent. A companion `requestOpenAiChat()` function fires a separate `nabu:open-ai-chat` event to open the panel before the prompt is sent.

## Options considered

- **Option A** (chosen): module-level singleton + `window` events — zero dependencies, no new global state manager, consistent with the existing `window.dispatchEvent` pattern already used for menu-command bridging.
- **Option B**: Lift AI panel state to a shared ancestor (e.g., `App.tsx`) and pass `onPrompt` callback down — would require `App.tsx` to own AI agent state, bloating it further; conflicts with ADR-0026 (props-down principle).
- **Option C**: Zustand / Jotai global store atom — adds a dependency and architecture overhead for a narrow, two-participant channel.

## Consequences

- Positive: decouples command palette from AI panel with no shared ancestor coupling.
- Positive: any future surface (e.g., wikilink context menu, note action bar) can call `queueAiPrompt` without tree-level wiring.
- Negative: module-level mutable state is harder to test in isolation; tests must call `takeQueuedAiPrompt` to drain state between runs.
- Negative: the event is fire-and-forget — if the AI panel is not mounted when the event fires, the prompt is silently dropped (currently not an issue as the panel is always mounted).
- Re-evaluate if the number of AI entry points grows large enough to warrant a proper state management solution.
