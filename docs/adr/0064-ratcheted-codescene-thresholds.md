---
type: ADR
id: "0064"
title: "Ratcheted CodeScene thresholds as the quality gate baseline"
status: active
date: 2026-04-14
---

## Context

ADR-0018 established CodeScene code-health gates so Nabu could block regressions before code reached `main`. Since then, the codebase has improved materially and the tracked baseline in `.codescene-thresholds` has been ratcheted above the original 9.50 / 9.31 minimums.

Leaving ADR-0018 active would make the architecture record stale: the enforced thresholds are now stricter than the decision document says, and the current workflow intentionally tightens them as the project's sustained health improves.

## Decision

**Supersede ADR-0018 and treat `.codescene-thresholds` as the ratcheted policy baseline for Nabu's CodeScene gate.** The current required minimums are `HOTSPOT_THRESHOLD=9.84` and `AVERAGE_THRESHOLD=9.45`. Thresholds move upward only when the repository can sustain a stricter baseline without immediately regressing.

## Options considered

- **Ratchet the enforced thresholds and document the new baseline** (chosen): keeps the ADRs aligned with the real gate, preserves the Boy Scout Rule, and makes code-health expectations stricter as the codebase improves.
- **Keep ADR-0018 active and treat higher thresholds as an implementation detail**: lower documentation churn, but the active ADR would no longer describe the actual CI and hook policy.
- **Remove numeric thresholds from ADRs entirely**: more durable on paper, but loses the explicit quality bar that developers are expected to maintain.

## Consequences

- `.codescene-thresholds` is now the authoritative location for the current numeric gate values.
- ADRs must be superseded again if Nabu makes another meaningful policy jump in CodeScene thresholds.
- Pre-push and related quality checks now enforce a stricter floor than ADR-0018 described.
- The quality gate remains intentionally one-way: relaxing thresholds would require an explicit architectural reversal, not a quiet config edit.
