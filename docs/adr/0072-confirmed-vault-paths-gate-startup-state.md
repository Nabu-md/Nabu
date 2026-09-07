---
type: ADR
id: "0072"
title: "Confirmed vault paths gate startup state"
status: active
date: 2026-04-22
---

## Context

Nabu's startup path was assuming that any incoming `vaultPath` was authoritative immediately. In practice, boot can pass through transient empty paths and stale paths that no longer correspond to the persisted active vault. That produced two classes of regressions:

1. `useVaultLoader` fired `reload_vault` and `get_modified_files` before a real vault path existed, generating avoidable warnings and backend calls for `""`.
2. On fresh install or other non-persisted startup cases, a missing path could incorrectly render `vault-missing` instead of the intended welcome flow.

Nabu's onboarding and vault-loading surfaces need the same invariant: only a confirmed vault identity should drive startup side effects or missing-vault error UI.

## Decision

**Nabu now treats a vault path as authoritative at startup only after it is confirmed.** Vault-loading side effects no-op until the path is non-empty, and the `vault-missing` onboarding state is shown only when the missing path was the persisted active vault recorded in `load_vault_list`. Otherwise, startup falls back to `welcome`.

## Options considered

- **Option A** (chosen): gate startup effects and missing-vault UI on confirmed vault identity. This keeps boot deterministic, avoids empty-path backend calls, and preserves the product rule that fresh installs should land in Welcome rather than an error state.
- **Option B**: treat any startup `vaultPath` as authoritative immediately. Simpler branching, but it keeps the existing race where transient or stale paths trigger warnings and the wrong onboarding state.
- **Option C**: special-case each startup surface independently. Lower immediate churn, but it would duplicate boot logic and let `useOnboarding` and `useVaultLoader` drift again.

## Consequences

- `useVaultLoader` must guard all startup work behind a real non-empty vault path.
- `useOnboarding` must consult persisted vault state before deciding that a missing path represents a deleted active vault.
- Fresh installs, cleared vault lists, and other startup flows without a confirmed active vault should resolve to `welcome`, even if an initial path probe fails.
- Re-evaluate if Nabu introduces deeper startup routing (for example multiple launch intents or restored workspaces) that needs a richer boot-state model than the current confirmed-path gate.
