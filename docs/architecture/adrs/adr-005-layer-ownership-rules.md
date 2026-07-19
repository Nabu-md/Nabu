# ADR-005 — Layer Ownership Rules

**Status:** Accepted
**Date:** 2026-07-19
**Phase:** Phase 1.1 → enforced through Phase 1.5, documented in Phase 1.6

---

## Context

With a multi-process layout (ADR-001), a service layer (ADR-002), a shared
contract layer (ADR-003), and an internal event bus (ADR-004), the project
needed explicit, enforceable rules about **which layer may depend on which**.
Without such rules, process boundaries erode: a renderer module might import a
main-only service, or `shared` might silently start importing Electron,
breaking its consumability by all processes.

## Decision

Adopt the following **layer ownership / dependency rules**. Dependencies flow
downward only; upward and cross-process imports are prohibited.

| Layer | May depend on | May NOT depend on |
|-------|---------------|-------------------|
| **Main** (`src/main`) | `services/*`, `shared/*`, `electron` | `renderer` |
| **Preload** (`src/preload`) | `shared/*`, `electron` | `main` logic, `renderer` |
| **Renderer** (`src/renderer`) | `shared/*` (types/schemas/contracts/ipc), React | `main`, `electron` (except preload bridge), services |
| **Services** (`src/main/services`) | `shared/*`, other services | `renderer`, `electron` UI globals (use `electron` APIs only where required) |
| **Shared** (`src/shared`) | third-party libs, other `shared` modules | `main`, `renderer`, `electron`, `react` |

Additional invariants:

- **Synchronous renderer ↔ main communication uses IPC only** (via
  `shared/ipc` + the preload bridge). The event bus (ADR-004) is for internal
  main-process notifications and is never imported by the renderer.
- **`shared/` stays process-agnostic**: zero `electron` and zero `react`
  imports. This is what makes it safe to consume from every process.
- **Intra-layer relative imports are allowed and preferred** for readability
  (e.g. `./state`, `../ipc` within main; `./features/...` within renderer). The
  `@main`, `@shared`, `@renderer` aliases are used for **cross-layer** imports.
- **No `@services` alias** is defined; services reference each other by
  relative path within `src/main/services`.

## Rationale

- Downward-only dependencies keep the build graph acyclic and each process
  independently bundleable.
- A process-agnostic `shared/` is the linchpin that lets main, preload, and
  renderer share one contract without circular process coupling.
- Separating IPC (renderer↔main) from the event bus (internal main) prevents
  the renderer from being coupled to main-process internals.
- Allowing intra-layer relative imports avoids over-aliasing, which would hurt
  local readability without adding safety.

## Alternatives Considered

- **Allow `shared` to import `electron` for convenience** — rejected; would
  break renderer/preload consumption and defeat the contract layer.
- **Mandate aliases for all imports, including intra-layer** — rejected;
  reduces readability and offers no boundary benefit within a single layer.
- **Permit the renderer to subscribe to the event bus** — rejected; would
  couple the renderer to internal main churn and bypass the IPC contract.

## Consequences

- Architecture validation can be performed by scanning imports for prohibited
  edges (the method used in Phase 1.5 and 1.6).
- New modules are placed and imported according to a fixed, checkable matrix.
- Violations are detectable in CI via import linting.

## Future Implications

- These rules are the acceptance criteria for all future architecture
  validation gates.
- If a new process context is added, its allowed dependencies are appended to
  the matrix — never by relaxing an existing rule.
- Tooling (e.g. ESLint import boundaries) may later automate enforcement of
  this ADR.
