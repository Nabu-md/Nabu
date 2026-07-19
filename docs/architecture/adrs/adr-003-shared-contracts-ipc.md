# ADR-003 — Shared Contracts & Typed IPC Registry

**Status:** Accepted
**Date:** 2026-07-19
**Phase:** Phase 1.4

---

## Context

Inter-process communication in Nabu was historically string-keyed: channel
names were scattered as literals across `ipc.ts`, the preload bridge, and
renderer call sites, and payloads were validated ad-hoc (or not at all). This
led to:

- no single source of truth for channel names or payload shapes;
- easy typos and silent contract drift between processes;
- duplicated Zod schemas in multiple places.

A canonical, strongly-typed contract layer was required so that every IPC
channel and its request/response payloads are defined exactly once and shared
by all three processes.

## Decision

Establish `src/shared/` as the single home for all cross-process contracts:

- `src/shared/models/` — domain types with **no runtime behavior**, no Electron
  and no React imports.
- `src/shared/schemas/` — Zod schemas for domain models and IPC payloads; it
  re-exports the existing `../schemas` to avoid duplicate definitions and adds
  the few channel payloads previously validated ad-hoc.
- `src/shared/validation/` — side-effect-free validation helpers
  (`validatePayload`, `formatZodError`, `zodErrorToValidationErrors`, …).
- `src/shared/contracts/` — canonical contract definitions (channels, request/
  response pairings, ownership).
- `src/shared/ipc/` — the **typed IPC registry**: a single enumeration of
  channel names plus their typed request/response shapes, consumed by main,
  preload, and renderer alike.

The registry is the canonical definition of every channel. Handlers, the
preload bridge, and renderer invocations all reference the same types, so a
payload-shape change is caught at compile time everywhere.

## Rationale

- One definition per channel eliminates drift and typos.
- Zod schemas double as runtime validation and as the source of inferred
  TypeScript types.
- Keeping contracts in `shared/` (process-agnostic) means all three processes
  import the same module rather than re-declaring it.
- Re-exporting the existing `schemas.ts` avoids a risky, behavior-changing
  rewrite while still centralizing the contract.

## Alternatives Considered

- **Generate IPC types from a separate IDL / codegen step** — rejected; adds a
  build dependency and is heavier than the TypeScript-native registry we have.
- **Keep schemas co-located with each service** — rejected; would reintroduce
  duplication and drift between main and renderer.
- **Use runtime-only validation without shared types** — rejected; loses
  compile-time safety, the primary goal.

## Consequences

- Adding or changing an IPC channel is a single-edit operation in
  `shared/ipc` (+ `shared/schemas` if payloads change).
- The renderer can call IPC with full type inference through the preload
  bridge.
- `shared/` must remain free of Electron/React imports to stay consumable by
  all processes (enforced by ADR-005).

## Future Implications

- Phase 2 (IPC modernization) extends this registry as the backbone for a
  cleaner handler/invocation API.
- Any new cross-process capability must be registered here first; no process
  may invent a channel name locally.
