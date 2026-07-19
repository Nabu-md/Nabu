# ADR-001 — Architecture Folder Layout

**Status:** Accepted
**Date:** 2026-07-19
**Phase:** Phase 1.1 (recovered & documented in Phase 1.6)

---

## Context

Nabu is an Electron application with three runtime contexts: the **main
process** (Node), the **preload** bridge, and the **renderer** (React). Before
the recovery program, source files were organized ad-hoc, mixing process
concerns and making it difficult to reason about which code runs where, which
modules may depend on which, and where new code belongs.

A clear, enforceable folder layout was needed so that:

- each process context has a single, obvious home;
- shared, process-agnostic code is isolated from Electron/React;
- future contributors can locate and place code without ambiguity.

## Decision

Adopt a four-top-level-source layout under `src/`:

```
src/
  main/        # Electron main process (Node)
    index.ts        # app bootstrap, window lifecycle
    ipc.ts          # IPC handler registration
    services/       # extracted domain services (Vault, Search, PDF, Widget, ...)
    plugins/        # remark plugin re-exports bridging to shared
  preload/      # contextBridge surface exposed to the renderer
    index.ts
    index.d.ts
  renderer/     # React UI
    src/
      App.tsx
      features/      # feature modules (notes, vault, graph, pdf, search, widgets, settings)
      components/    # shared presentational components
      commands/      # command registry + feature registrations
  shared/       # process-agnostic contracts, types, schemas, IPC, events
    models/     # domain types (no runtime behavior)
    schemas/    # Zod validation schemas
    validation/ # validation utilities
    contracts/  # canonical contract definitions
    ipc/        # typed IPC channel registry
    events/     # typed event bus + event map
    plugins/    # shared remark plugins
    *.ts        # standalone shared utilities (graph, indexing, search-query, ...)
```

Path aliases are wired in `tsconfig.node.json`, `tsconfig.web.json`, and
`electron.vite.config.ts`:

- `@main/*` → `src/main/*`
- `@shared/*` → `src/shared/*`
- `@renderer/*` → `src/renderer/src/*`

## Rationale

- A 1:1 mapping between runtime context and folder removes guesswork about
  where a module executes.
- Isolating `shared/` from `electron` and `react` imports guarantees it can be
  consumed by any process without pulling in process-specific globals.
- Aliases make cross-layer imports explicit and stable under file moves,
  reducing brittle relative paths such as `../../../../shared/types`.

## Alternatives Considered

- **Single flat `src/` with naming conventions** — rejected; too easy to
  accidentally import a renderer-only module from main.
- **Per-feature vertical slices spanning all layers** — rejected; would blur
  the process boundary and complicate the Electron build graph.
- **A `common/` + `core/` split instead of `shared/`** — rejected; `shared/`
  already matched the existing de-facto location of cross-process code.

## Consequences

- New code has an obvious home keyed to its runtime context.
- Build tooling (electron-vite) maps aliases per process, keeping the bundle
  graph correct.
- Relative imports within a layer remain acceptable and readable.

## Future Implications

- Phase 2 (IPC modernization) builds on the `shared/ipc` registry location.
- Any new process context (e.g. a worker) would get its own top-level folder
  and alias, not be folded into an existing one.
- The layout is the reference structure for all subsequent architecture
  validation checks.
