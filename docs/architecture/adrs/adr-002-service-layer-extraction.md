# ADR-002 — Service Layer Extraction

**Status:** Accepted
**Date:** 2026-07-19
**Phase:** Phase 1.3

---

## Context

Before Phase 1.3, domain logic (vault lifecycle, search, PDF handling, widget
management, dictation, indexing, settings, etc.) was embedded directly inside
the main-process bootstrap (`src/main/index.ts`) and the IPC handler file
(`src/main/ipc.ts`). This created:

- oversized, hard-to-test bootstrap and IPC files;
- duplicated logic between IPC handlers and direct callers;
- no clear ownership of workflows.

A dedicated service layer was needed to give each domain a single owner and to
make the main process composable and testable.

## Decision

Extract domain logic into `src/main/services/*`, one file per service, each
owning a well-defined set of workflows and exposing a small public API. The
bootstrap (`index.ts`) and IPC layer (`ipc.ts`) become **thin coordinators**
that delegate to services; they retain only wiring, window lifecycle, and
handler registration.

Services established (non-exhaustive):

- `vault-service.ts` — vault open/close/create/switch/scan, recents, restore
- `search-service.ts` — search orchestration and indexing coordination
- `pdf-service.ts` — PDF load/render/annotation coordination
- `widget-service.ts` / `widget-manager.ts` — widget lifecycle & windows
- `dictation-service.ts` — dictation workflow
- `state.ts` (StateManager) — in-memory index/AST state
- `vector.ts` (VectorManager) — embeddings & semantic index
- `vault-registry.ts`, `settings.ts`, `watcher.ts`, `parser.ts`,
  `composer.ts`, `bases.ts`, `ocr-manager.ts`, `whisper.ts`, and others

Extraction is **pure**: no behavior was redesigned, no algorithms changed, no
new abstractions (e.g. DI) introduced. Thin wrappers were left behind where
callers expected the old surface.

## Rationale

- Single ownership per domain removes ambiguity about where a workflow lives.
- Smaller files are easier to read, review, and unit-test.
- The IPC layer stays focused on transport, not business rules.
- Pure extraction preserves runtime behavior exactly, de-risking the migration.

## Alternatives Considered

- **Keep logic in `ipc.ts`** — rejected; IPC file was already ~75 KB and
  unmaintainable.
- **Introduce a service container / DI framework** — rejected; would be a
  behavioral change and out of scope for a recovery/cleanup phase.
- **Group services by feature rather than by domain** — rejected; domain
  ownership maps more directly to IPC channels and workflows.

## Consequences

- `ipc.ts` and `index.ts` are now coordinators, not logic hosts.
- Each service depends on `shared` and (optionally) other services, never on
  the renderer.
- Service-to-service calls use direct imports (relative within `services/`),
  not the IPC boundary.

## Future Implications

- Phase 2 can modernize IPC by mapping each handler 1:1 onto the service that
  already owns it.
- New domains are added as new service files, not by extending `ipc.ts`.
- Service ownership is the basis for the layer-ownership rules in ADR-005.
