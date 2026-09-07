---
type: ADR
id: "0058"
title: "Claude Code first-launch onboarding gate"
status: superseded
superseded_by: "0062"
date: 2026-04-12
---

## Context

Nabu's AI features depend on the `claude` CLI being installed on the user's machine. New users arriving with no prior context could open the app, try AI-powered workflows, and get silent failures with no explanation.

A dedicated first-launch prompt was needed to:
- Surface whether the `claude` CLI is already present.
- Guide users to the install page if it is missing.
- Not block experienced users who want to skip the check.

The existing `useOnboarding` hook already handles vault setup and resolves to a `ready` state, but it had no mechanism for a post-vault, pre-app step.

## Decision

**A one-time `ClaudeCodeOnboardingPrompt` is shown immediately after vault onboarding resolves to `ready`, before the main app shell renders. Dismissal is persisted in `localStorage` via `useClaudeCodeOnboarding`, so the gate appears exactly once per install.**

## Options considered

- **Option A** (chosen): Full-screen gate after vault onboarding, dismissed once and persisted in `localStorage`. Pros: cannot be missed on first launch, reuses `useClaudeCodeStatus` for live detection, zero impact on returning users. Cons: adds one extra render phase to the boot sequence.
- **Option B**: Inline banner inside the main app. Pros: less intrusive. Cons: easy to ignore, harder to surface install link prominently.
- **Option C**: Check at feature use time (show error when AI action fails). Pros: no new screen. Cons: poor UX — silent failure or cryptic error at the moment the user needs AI.

## Consequences

- The app boot sequence now has four phases: loading → welcome (if needed) → Claude Code check (once) → main shell.
- `useClaudeCodeOnboarding(enabled)` takes a boolean so the gate is skipped entirely in note windows and before vault onboarding completes.
- The dismissal key (`nabu:claude-code-onboarding-dismissed`) must be pre-set in Playwright storage state so smoke tests bypass the gate.
- Re-evaluation warranted if the `claude` CLI gains an in-app auto-install path, making the manual prompt unnecessary.
