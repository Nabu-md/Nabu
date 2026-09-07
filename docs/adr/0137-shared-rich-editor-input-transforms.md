---
type: ADR
id: "0137"
title: "Shared rich-editor input transforms"
status: active
date: 2026-06-07
---

## Context

Nabu has several Markdown-style conveniences in the rich BlockNote editor:
typed arrows become ligatures, completed inline math becomes a math node, and
completed `==highlight==` syntax becomes the durable highlight mark.

These features were added incrementally as separate `beforeinput` extensions.
Each extension repeated the same lifecycle work: reading the live ProseMirror
view, skipping IME composition, guarding stale views, dispatching transactions,
preventing native input only after a successful transform, and recovering known
BlockNote/ProseMirror transform failures. The syntax matchers differed, but the
execution shell was parallel enough that each new Markdown affordance risked a
slightly different edge-case policy.

## Decision

**Nabu routes rich-editor Markdown input transforms through one shared
`beforeinput` execution path.**

`src/components/richEditorInputTransform.ts` owns the common lifecycle,
dispatch, and recoverable-error behavior. Feature files such as
`arrowLigaturesExtension.ts`, `mathInputExtension.ts`, and
`markdownHighlightInputExtension.ts` expose small transform objects that only
decide whether the current input event should produce a transaction.

`src/components/richEditorInputTransformExtension.ts` composes the Markdown
transform set used by the main editor and hidden editor probe.

## Options Considered

- **Shared transform primitive with feature-owned matchers** (chosen): removes
  duplicate listener, dispatch, composition, and recovery code while keeping each
  syntax rule local and testable.
- **One monolithic Markdown input extension**: reduces listener count, but mixes
  unrelated syntax rules in one file and makes each future input affordance
  harder to test independently.
- **Keep one extension per feature**: preserves local ownership, but keeps the
  repeated edge-case shell and invites divergence as more transforms are added.

## Consequences

- `Editor` mounts one Markdown input-transform extension rather than separate
  arrow, math, and highlight `beforeinput` listeners.
- Feature-specific files remain responsible for their syntax matching and
  transaction construction only.
- Recoverable transform errors use the same telemetry event and fallback policy
  across Markdown input transforms.
- New rich-editor Markdown input affordances should plug into the shared
  transform primitive instead of adding another capture-phase `beforeinput`
  extension.
