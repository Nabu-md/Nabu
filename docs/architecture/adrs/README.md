# Architecture Decision Records (ADRs)

This directory holds the Architecture Decision Records (ADRs) that capture the
major architectural decisions made during **Phase 1** of the Nabu Recovery
Program.

Each ADR follows a consistent format:

- **Status** — Proposed / Accepted / Deprecated / Superseded
- **Context** — the forces and situation that motivated the decision
- **Decision** — the decision that was made
- **Rationale** — why this decision was the right one
- **Alternatives Considered** — options that were evaluated and rejected
- **Consequences** — what becomes easier or harder as a result
- **Future Implications** — how this decision shapes later phases

---

## Index

| ID | Title | File |
|----|-------|------|
| ADR-001 | Architecture Folder Layout | [adr-001-folder-layout.md](adr-001-folder-layout.md) |
| ADR-002 | Service Layer Extraction | [adr-002-service-layer-extraction.md](adr-002-service-layer-extraction.md) |
| ADR-003 | Shared Contracts & Typed IPC Registry | [adr-003-shared-contracts-ipc.md](adr-003-shared-contracts-ipc.md) |
| ADR-004 | Typed Event Bus | [adr-004-typed-event-bus.md](adr-004-typed-event-bus.md) |
| ADR-005 | Layer Ownership Rules | [adr-005-layer-ownership-rules.md](adr-005-layer-ownership-rules.md) |

---

## Alias Convention (established during Phase 1)

The approved path-alias strategy, defined in `tsconfig.node.json`,
`tsconfig.web.json`, and `electron.vite.config.ts`:

| Alias | Resolves to | Available in |
|-------|-------------|--------------|
| `@main/*` | `src/main/*` | main, preload |
| `@shared/*` | `src/shared/*` | main, preload, renderer |
| `@renderer/*` | `src/renderer/src/*` | renderer, preload |

No `@services/*` alias is defined; intra-layer relative imports (e.g.
`./state`, `../ipc`) are retained because they are conventional and readable
within a single layer.
