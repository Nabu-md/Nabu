---
type: ADR
id: "0172"
title: "Local CodeScene safeguards before commits and direct-to-main pushes"
status: active
date: 2026-07-24
---

## Context

ADR-0064 made project-wide Hotspot and Average Code Health a ratcheted release gate. Those scores are calculated from the latest remote CodeScene analysis, so they are valuable repository-level monitors but cannot describe an uncommitted local change. The local pre-push hook can also only read the previous remote analysis until the new commit has reached the repository and CodeScene has analyzed it.

Nabu pushes directly to `main`, so it has no pull-request boundary where a normal change-set review would run. The written Boy Scout rule already requires file-level before/after reviews, but reviewing files one at a time does not provide one final verdict for the complete commit or task.

CodeScene's MCP provides two change-aware safeguards for these boundaries: `pre_commit_code_health_safeguard` reviews modified and staged files, and `analyze_change_set` compares the complete committed change with a base Git reference.

## Decision

**Keep the project-wide CodeScene ratchet from ADR-0064, and add mandatory local MCP safeguards before each commit and before the final direct-to-main push.**

- Continue file-level before/after reviews while editing, preserving the Boy Scout rule.
- Before every commit, run `pre_commit_code_health_safeguard` for the repository. Its quality gates must pass.
- Before the final push, run `analyze_change_set` against `origin/main`. Every affected file must be improved or stable, and the overall quality gate must pass.
- Treat the final change-set analysis as Nabu's local PR-preflight equivalent.
- Keep the remote Hotspot and Average thresholds as the outer repository gate and trend signal.
- Prefer the CodeScene MCP. If it is unavailable, use the supported CodeScene CLI equivalent. Do not silently replace change-aware analysis with the previous remote project score.

## Options considered

- **Add local commit and change-set safeguards while retaining the remote ratchet** (chosen): evaluates the code that is actually about to land and preserves the project-wide trend gate.
- **Rely on the project-wide remote scores only**: remains simple, but the analysis necessarily lags the local direct-to-main push.
- **Rely on individual file score checks only**: preserves the Boy Scout rule, but has no repository-level verdict over the complete commit or task.
- **Move back to pull requests to use a conventional PR preflight**: would provide a natural branch boundary, but reintroduces the solo-development overhead rejected by ADR-0021.

## Consequences

- Code Health regressions receive feedback while the implementing agent still has the task context.
- A multi-commit task receives one final comparison against the exact `origin/main` base it will replace.
- Completion evidence must include file-level before/after results, pre-commit safeguard verdicts, and the final change-set verdict.
- The MCP installation becomes part of the development toolchain and should be kept current.
- These agent-side safeguards complement the Husky and CI gates; they do not weaken or replace them.
- If no supported MCP or CLI safeguard is available, code commits and pushes stop until CodeScene access is restored.
