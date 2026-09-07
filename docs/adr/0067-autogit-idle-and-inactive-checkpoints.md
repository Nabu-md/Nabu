---
type: ADR
id: "0067"
title: "AutoGit idle and inactive checkpoints"
status: active
date: 2026-04-17
---

## Context

Nabu already had explicit git actions in the status bar (ADR-0032) and a remote-aware manual commit flow (ADR-0059), but git-backed vaults still depended on the user remembering to create checkpoints. That worked for deliberate commits, yet it left a gap for ordinary writing sessions where the app had already saved all note content but no git checkpoint had been recorded.

The new checkpointing behavior needed to stay conservative:

- never run for non-git vaults
- never commit unsaved editor buffers
- reuse the same remote detection and local-only fallback as the manual commit flow
- avoid drift between timer-driven checkpoints and the status-bar quick commit action

## Decision

**Nabu introduces installation-local AutoGit settings plus a dedicated `useAutoGit` hook that triggers a shared `useCommitFlow.runAutomaticCheckpoint()` path after configurable idle or inactive thresholds.** The checkpoint runs only when the current vault is git-backed, there are pending saved changes (or local commits waiting to push), and no unsaved edits remain.

`useCommitFlow.runAutomaticCheckpoint()` is now the single checkpoint runner for both AutoGit and the status-bar quick commit action. That shared path generates deterministic automatic commit messages (`Updated N note(s)` / `Updated N file(s)`), commits locally when no remote exists, and can also do a push-only retry when commits already exist locally.

## Options considered

- **Option A** (chosen): A shared checkpoint runner used by both AutoGit timers and the quick commit action. Pros: one git policy, one message generator, one remote-handling path. Cons: adds another cross-cutting settings-driven hook.
- **Option B**: A separate background AutoGit implementation. Pros: could evolve independently from the manual commit flow. Cons: high risk of drift in commit messages, push behavior, and remote handling.
- **Option C**: Commit on every save. Pros: simplest trigger model. Cons: far too noisy for git history, especially with Nabu's autosave model.

## Consequences

- App settings now persist `autogit_enabled`, `autogit_idle_threshold_seconds`, and `autogit_inactive_threshold_seconds` in installation-local settings storage.
- `useAutoGit` tracks editor activity plus app focus/visibility state and triggers checkpoints after the configured thresholds.
- Automatic checkpoints are blocked while unsaved edits exist, so AutoGit only records content that is already flushed through the normal save pipeline.
- The bottom-bar quick commit action now reuses the same checkpoint runner after forcing a save, keeping manual and automatic checkpoint behavior aligned.
- Vaults without a remote still benefit: AutoGit uses the existing local-only commit behavior from ADR-0059 instead of treating missing remotes as an error.
- Re-evaluate if users need per-vault policy instead of installation-local policy, or if timer-driven checkpoints create too much git noise in real-world use.
