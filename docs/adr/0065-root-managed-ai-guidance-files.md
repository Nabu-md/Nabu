---
type: ADR
id: "0065"
title: "Root-managed AI guidance files with Claude shim"
status: active
date: 2026-04-14
---

## Context

Nabu now supports multiple local CLI agents, but vault-level guidance still carried legacy assumptions. Existing vault bootstrap and repair flows centered on `config/agents.md`, while modern coding agents expect instructions at the vault root. That mismatch made managed guidance harder to reason about, left Claude Code compatibility implicit, and gave the UI no reliable way to distinguish between Nabu-managed files that can be repaired and user-authored custom guidance that must be preserved.

## Decision

**Nabu manages vault AI guidance at the vault root.** `AGENTS.md` is the canonical shared guidance file, `CLAUDE.md` is a compatibility shim that points Claude Code back to `AGENTS.md`, and Nabu classifies both files as `managed`, `missing`, `broken`, or `custom` so repair flows restore only Nabu-managed guidance without overwriting custom user files.

## Options considered

- **Root `AGENTS.md` as canonical plus a root `CLAUDE.md` shim** (chosen): matches current agent expectations, keeps one source of truth for shared instructions, and makes repair status explicit.
- **Keep managed guidance under `config/agents.md`**: preserves the older structure, but hides a user-facing integration contract behind legacy config paths and keeps Claude compatibility indirect.
- **Maintain separate full instruction files for each agent**: simple per tool, but duplicates instructions and increases drift risk whenever guidance changes.

## Consequences

- New and repaired vaults now seed `AGENTS.md` and `CLAUDE.md` at the vault root.
- Legacy `config/agents.md` content is migrated forward when safe, then the obsolete file is removed.
- The status bar and command palette can expose a first-class restore action because backend guidance state is normalized.
- Custom root guidance files are preserved instead of being silently overwritten by repair flows.
- Nabu keeps a single shared guidance document even while supporting multiple CLI agents.
- Re-evaluate if supported agents stop relying on root-level files or if future agent integrations require materially different vault instructions instead of a shared source of truth.
