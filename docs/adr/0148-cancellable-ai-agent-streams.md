---
type: ADR
id: "0148"
title: "Cancellable AI agent streams"
status: active
date: 2026-06-28
supersedes:
---

## Context

ADR-0133 gave every AI stream a request-scoped event name so concurrent or rapidly reused renderer sessions do not share Tauri event channels. Users can still start long-running CLI-agent work that spawns local subprocesses, and closing or ignoring the frontend stream only suppresses stale renderer callbacks. Without a backend cancellation path, the selected CLI can keep running after the user has decided to stop the response.

The stop control needs to work across the shared JSON-line runtime and line-oriented adapters such as Antigravity, Kiro, and Hermes without giving the renderer access to arbitrary process ids.

## Decision

Nabu treats the request-scoped AI-agent event name as the cancellation handle for app-managed CLI-agent streams.

The desktop backend wraps `stream_ai_agent` execution in an AI-agent stream scope. When an adapter spawns a child process, it registers that child under the current scoped stream id through a shared process registry. The renderer can then call `abort_ai_agent_stream(event_name)` with the same scoped id used for event delivery. The command validates the `ai-agent-stream-*` prefix and safe character set before killing the registered child, and returns `false` when no active child is present.

Direct AI-model streams are not registered in this process registry because they do not spawn local CLI children.

## Consequences

- The stop button can abort an in-flight app-managed CLI process instead of only ignoring late stream events.
- New CLI-agent adapters that spawn subprocesses must register their child after taking stdout/stderr handles and wait through the registered wrapper.
- The renderer never receives or supplies OS process ids; cancellation remains scoped to validated AI-agent stream names.
- A stop can race with process startup or natural completion, so both frontend and backend stop paths are idempotent.
