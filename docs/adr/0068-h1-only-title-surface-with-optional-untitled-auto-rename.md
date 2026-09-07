---
type: ADR
id: "0068"
title: "H1-only title surface with optional untitled auto-rename"
status: active
date: 2026-04-17
supersedes: "0055"
---

## Context

ADR-0055 removed the legacy title row and made the editor body the only title surface. That ADR also kept one strong filename behavior from ADR-0044: untitled notes would auto-rename from their first H1 on save.

That always-on rename rule turned out to be too rigid. Some users want the H1 to drive the displayed title immediately, but prefer to keep the synthetic `untitled-*` filename stable until they explicitly rename it from the breadcrumb bar. The product needed to preserve the H1-only editing model without forcing every installation into automatic filename changes.

## Decision

**Nabu keeps the editor body as the only title surface, but untitled-note auto-rename from the first H1 becomes an installation-local setting (`initial_h1_auto_rename_enabled`) that defaults to enabled.**

When the setting is enabled, untitled notes continue to auto-rename on save as soon as a real H1 title exists. When disabled, Nabu still treats the H1 as the canonical display title, but it leaves the filename unchanged until the user explicitly renames it through the breadcrumb controls.

## Options considered

- **Option A** (chosen): Keep the current auto-rename behavior as the default, but make it an installation-local preference. Pros: preserves the fast path for most users while allowing opt-out for users who want stable temporary filenames. Cons: different installs can behave differently.
- **Option B**: Keep auto-rename mandatory, as assumed by ADR-0055. Pros: one simple filename policy. Cons: surprises users who want title editing without immediate file renames.
- **Option C**: Turn auto-rename off for everyone. Pros: filenames only change on explicit user action. Cons: leaves more `untitled-*` files around and adds friction to the common case.

## Consequences

- App settings now persist `initial_h1_auto_rename_enabled` in installation-local settings storage.
- The save pipeline consults that setting before scheduling untitled-file renames.
- Disabling untitled auto-rename does not restore any legacy title field or alternate title UI. H1 remains the only editor title surface.
- When the setting is off, display title and filename can diverge for longer: the note may show a human H1 while the file remains `untitled-*` until explicit rename.
- Settings UI and command/search affordances now expose this filename policy as a user preference rather than a hardcoded rule.
- Re-evaluate if users later need this policy to be per-vault instead of installation-local, or if longer-lived untitled filenames create too much Finder/git noise.
