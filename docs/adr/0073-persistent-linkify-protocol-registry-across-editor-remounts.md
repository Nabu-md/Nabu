---
type: ADR
id: "0073"
title: "Persistent linkify protocol registry across editor remounts"
status: active
date: 2026-04-22
---

## Context

Nabu keeps a single editor shell alive while users swap notes and toggle between BlockNote and raw mode. The upstream BlockNote/Tiptap link stack assumes linkify protocol registration is effectively one-shot per editor lifetime, and Tiptap's link extension resets that registry on destroy.

In Nabu's lifecycle, that behavior caused duplicate `linkifyjs: already initialized` warnings during note-open and editor-remount flows. The problem is cross-cutting: BlockNote and Tiptap both participate, and the failure only disappears when protocol registration survives teardown/remount cycles instead of being repeated opportunistically.

## Decision

**Nabu patches the upstream BlockNote and Tiptap link packages so custom linkify protocols are pre-registered once per app runtime and are not reset on editor teardown.** The patched packages coordinate through `globalThis` flags, and Nabu tracks them via `pnpm` patched dependencies rather than ad hoc runtime monkey-patching inside app code.

## Options considered

- **Option A** (chosen): maintain explicit `pnpm` patches for the affected upstream packages, pre-register the needed protocols once, and preserve the registry across remounts. This matches Nabu's persistent editor shell and keeps the behavior deterministic in both dev and packaged builds.
- **Option B**: keep upstream behavior and tolerate or suppress the warnings locally. Lower maintenance, but it leaves editor lifecycle correctness dependent on noisy duplicate initialization and makes future regressions harder to reason about.
- **Option C**: add Nabu-side runtime monkey-patches around editor mount/unmount. Avoids vendoring patches, but spreads dependency-specific lifecycle logic into application code and is more fragile across package upgrades.

## Consequences

- `pnpm-workspace.yaml` now treats the relevant BlockNote and Tiptap link packages as patched dependencies, so upgrades must preserve or consciously replace those patches.
- Editor teardown in Nabu must not assume ownership of the global linkify protocol registry.
- Smoke coverage for note open, editor remount, and raw-mode toggling must stay in place because the failure mode is lifecycle-specific rather than feature-specific.
- Re-evaluate this ADR if upstream BlockNote/Tiptap gains a supported lifecycle-safe protocol-registration model that makes the Nabu patches unnecessary.
