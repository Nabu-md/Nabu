# Nabu Recovery Program (NRP)

**Version:** 0.3 Draft
**Status:** Planning
**Project:** Nabu Desktop Knowledge Management Application
**Audience:** Human Maintainers & AI Coding Agents

---

# Table of Contents

1. Executive Summary
2. Vision
3. Product Principles
4. Architecture Goals
5. Non-Goals
6. Success Metrics
7. Technology Standards
8. Coding Standards
9. AI Operating Manual
10. Verification Gates
11. Phase Locking
12. Permanent Artifacts
13. Overall Roadmap
14. Phase -1 — Audit Validation
15. Phase 0 — Emergency Stabilization
16. Phase 1 — Architecture Migration
17. Phase 2 — IPC Stabilization
18. Phase 3 — Widget System
19. Phase 4 — Vault & Workspace
20. Phase 5 — Renderer & UX
21. Phase 6 — Feature Recovery
22. Phase 7 — Storage, Search & PDF
23. Phase 8 — Technical Debt Elimination
24. Phase 9 — Runtime Validation
25. Phase 10 — Production Readiness
26. Recovery Scoreboard
27. Completion Checklist
28. Estimated Project Size

---

# Executive Summary

The Nabu Recovery Program (NRP) defines the complete engineering roadmap for transforming the existing Nabu repository into a stable, maintainable, production-quality desktop application.

This document is not a bug-fixing checklist.

It is the engineering specification governing every architectural decision, every refactor, every implementation phase, and every AI coding session throughout the recovery effort.

The goal is to eliminate technical debt while preserving the project's vision as a **desktop-first, local-first, privacy-first knowledge management application**.

Every code change performed by either humans or AI must conform to this document.

Nabu is a desktop application that runs entirely on the user's computer. It is not a hosted SaaS product, not a distributed system, and not a cloud-native platform. All architecture decisions must reflect that reality.

---

# Vision

Nabu should become the best local-first knowledge management application for users who value ownership, speed, extensibility, and reliability.

The application should feel:

* Fast
* Stable
* Predictable
* Offline-first
* Privacy-first
* AI-friendly
* Extensible
* Easy to maintain

The long-term goal is a desktop application that is pleasant to use, easy to reason about, and resilient under change.

---

# Product Principles

## 1. Local-First

All core functionality must work without a network connection.

User data should remain on the user's machine unless a future feature explicitly introduces synchronization.

## 2. User Ownership

The user owns their data, their workspace, and their vaults.

The application must not obscure where data lives or how it is stored.

## 3. Simplicity Over Cleverness

Prefer straightforward implementations over abstract or overly generic ones.

If a solution is hard to explain, it is probably too complex for this codebase.

## 4. Explicit Over Implicit

Dependencies, ownership, and data flow should be visible and easy to trace.

Avoid hidden side effects and magical behavior.

## 5. Feature Ownership

Each feature should have a clear owner in the codebase.

A feature should not be spread across unrelated folders without a strong reason.

## 6. Strong Typing

TypeScript should be used to enforce correctness, reduce ambiguity, and improve maintainability.

## 7. Incremental Change

Large refactors must be broken into small, verifiable steps.

Every phase should leave the application in a better and more stable state.

## 8. Desktop-First Design

The application should be optimized for Electron and local filesystem workflows, not browser deployment or cloud hosting.

---

# Architecture Goals

The target architecture is designed to make the application easier to understand, safer to modify, and more resilient to future growth.

## 1. Feature-Based Organization

Organize by business capability rather than technical layer whenever practical.

Each feature should own:

* UI
* state
* commands
* IPC client
* utilities
* tests

## 2. Service Layer

Application behavior belongs inside services.

Services coordinate work.

Services do not render UI.

Services do not manipulate React components.

Services should be focused, testable, and easy to locate.

## 3. Small Domain Models

Domain models represent business concepts only.

Examples include:

* Note
* Vault
* Workspace
* Tag
* Attachment
* GraphNode

Domain models must never depend on:

* Electron
* React
* Browser APIs

## 4. Typed IPC

IPC is treated as a public API.

Every IPC channel must define:

* channel identifier
* request schema
* response schema
* error schema

String literals are prohibited outside the IPC contract layer.

## 5. Internal Event Bus

A lightweight typed event bus is used only for background workflows.

Examples include:

* VaultOpened
* VaultClosed
* NoteSaved
* NoteDeleted
* SearchIndexed
* WidgetRegistered

The event bus is not a replacement for ordinary function calls.

## 6. Layer Separation

The project is divided into three architectural layers.

### Main

Responsible for:

* Electron
* Filesystem
* Database
* AI
* Search
* IPC
* Background work

### Renderer

Responsible for:

* UI
* User interaction
* Presentation
* Local UI state

### Shared

Responsible for:

* Types
* Schemas
* Validation
* Contracts
* Utilities
* Domain models

Cross-layer imports are prohibited except through approved contracts.

## 7. Target Folder Layout

Within the three-layer structure, code is organized by feature. The following layout is the concrete target:

```
src/
  main/
    services/          # One file per domain service
      VaultService.ts
      SearchService.ts
      PdfService.ts
      WidgetService.ts
      DictationService.ts
    ipc/               # One file per feature area, register*IPC() bootstrap
      vault.ts
      notes.ts
      search.ts
      pdf.ts
      settings.ts
      widgets.ts
      dictation.ts
      index.ts         # calls all register*IPC() functions
  renderer/
    features/          # One folder per feature
      notes/
      search/
      graph/
      settings/
      widgets/
    components/        # Shared UI components
    hooks/
  shared/
    models/            # Note, Vault, Workspace, Tag, GraphNode
    schemas/           # Zod schemas
    types/             # TypeScript type definitions
```

## 8. Layer Dependency Flow

Dependencies flow in one direction only — outer layers may depend on inner layers, never the reverse:

```
UI (React components)
       ↓
Application Services (openVault, saveNote, searchNotes…)
       ↓
Domain Models (Note, Vault, Workspace, Search)
       ↓
Infrastructure (FileSystem, SQLite, IPC, AI, PDF)
       ↓
Electron APIs
```

A React component calls a service. The service coordinates domain logic through infrastructure. The service never imports Electron or React. Domain models never import anything.

## 9. Thin Presentation Layer

React components must not contain business logic.

Components are responsible for:

* Displaying state from services and commands
* Calling commands in response to user input
* Managing local UI state (scroll position, input focus, animation)

Components must never:

* Call Electron APIs directly (use IPC through preload contracts)
* Execute filesystem operations (delegate to services)
* Contain domain logic (that belongs in services or commands)
* Manage persistent application state (that belongs in services)

The renderer is a presentation layer. All non-UI behavior lives in services on the main process side, surfaced through typed IPC contracts.

## 10. Infrastructure Adapters

Filesystem, SQLite, AI, PDF, clipboard, and Electron APIs must be isolated behind clear interfaces.

Services depend on interfaces, not directly on Electron or Node.js APIs.

```typescript
// Services use an interface
interface FileSystemAdapter {
  readFile(path: string): Promise<string>
  writeFile(path: string, content: string): Promise<void>
}

// The real implementation uses Electron/Node
class ElectronFileSystemAdapter implements FileSystemAdapter {
  async readFile(path: string): Promise<string> {
    return fs.promises.readFile(path, 'utf-8')
  }
}

// Service never imports fs or electron
class VaultService {
  constructor(private fs: FileSystemAdapter) {}
  // ...
}
```

This does not mean every tiny class needs an interface. The rule applies at the infrastructure boundary: Electron APIs, filesystem, database, AI, and OS-level operations. If it touches a platform API, it goes through an adapter.

## 11. Command-Based Actions

Every user-facing action is represented as a reusable command object.

Each command encapsulates:

* Action metadata (label, shortcut, icon)
* Execution logic (what happens when invoked)
* Undo behavior (where applicable)
* Menu and palette integration

```typescript
interface Command {
  id: string
  label: string
  shortcut?: string
  execute(context: CommandContext): Promise<void>
  undo?(context: CommandContext): Promise<void>
}
```

Commands are how the UI interacts with the application layer. A button click invokes a command. A keyboard shortcut invokes a command. The command palette lists all available commands. This pattern is already established in the codebase — this goal documents it as a permanent architectural rule.

The following architectural patterns will not be introduced unless a future Architecture Decision Record explicitly approves them:

* Full Domain-Driven Design
* CQRS
* Event Sourcing
* Microservices
* Distributed Messaging
* Enterprise Repository Pattern
* Generic Service Locators
* Heavy Dependency Injection Frameworks

The application is a desktop application and should remain appropriately simple.

This recovery program is not an excuse to introduce enterprise architecture where it does not belong.

---

# Success Metrics

The recovery program is complete only when all of the following conditions are satisfied:

* Clean installation
* Clean dependency resolution
* Clean TypeScript compilation
* Clean Electron startup
* Interactive renderer
* No startup exceptions
* No orphaned IPC channels
* No missing exports
* No missing imports
* No circular dependency blockers
* Feature inventory completed
* Technical debt inventory completed
* Architecture documentation completed
* Production packaging succeeds

Additional success indicators include:

* Clear ownership of major subsystems
* Reduced file size and complexity
* Predictable startup behavior
* Stable IPC contracts
* Reliable local data handling
* Easier future feature development

---

# Technology Standards

The recovery program assumes the following technology stack unless a phase explicitly changes it:

* Electron
* React
* TypeScript
* Vite
* Node.js
* Zod for runtime validation
* Zustand or the current established state library
* ESLint
* Prettier

## Technology Rules

1. Use TypeScript in strict mode wherever possible.
2. Use Node APIs only in the main process or approved preload boundaries.
3. Use browser APIs only in the renderer.
4. Use shared schemas and types for cross-process contracts.
5. Avoid introducing new dependencies unless they solve a documented problem.
6. Prefer existing tools and conventions already present in the repository.

---

# Coding Standards

## 1. File Responsibility

Every file should have one clear responsibility.

Avoid files that mix:

* UI and business logic
* IPC and domain logic
* startup and feature logic
* unrelated utilities

## 2. File Size Guidance

Recommended maximums:

* Service: 300 lines
* Component: 250 lines
* IPC module: 250 lines
* Utility module: 200 lines

These are guidelines, not rigid laws, but large files should be treated as a warning sign.

## 3. Naming

Use names that describe intent clearly.

Prefer:

* `VaultService`
* `SearchService`
* `NoteEditor`
* `registerVaultIPC`

Avoid vague names like:

* `manager`
* `helper`
* `utils`
* `misc`

unless the file truly contains generic helpers.

## 4. Imports

Imports should be explicit and minimal.

Avoid circular dependencies.

Avoid deep relative import chains when a clear alias exists.

## 5. Error Handling

Every service should return predictable errors.

Never swallow exceptions silently.

Never return undefined for failed IPC.

Always return structured errors where possible.

## 6. Logging

All logs should pass through a centralized logger.

Never scatter random `console.log` calls throughout the codebase.

Support:

* info
* warn
* error
* debug

## 7. Testing

When practical, changes should be accompanied by tests or verification steps.

At minimum, every phase should include a validation step.

---

# AI Operating Manual

This document is intended to guide both human maintainers and AI coding agents.

## AI Workflow

Every AI coding session must follow the same workflow:

1. Understand the subsystem.
2. Explain the root cause.
3. Produce an implementation plan.
4. Identify affected files.
5. Make the smallest coherent change.
6. Verify compilation.
7. Verify runtime behavior.
8. Report remaining work.
9. Recommend the next phase.

## AI Guardrails

AI agents must:

* stay within the current phase
* avoid unrelated refactors
* preserve behavior unless the phase explicitly changes it
* update types, imports, and documentation together
* keep the application buildable whenever possible
* report risks before making structural changes
* stop and ask for clarification when scope is unclear

## AI Anti-Patterns

AI agents must not:

* rewrite unrelated subsystems
* introduce new architecture without approval
* collapse multiple phases into one large refactor
* hide failures behind temporary hacks
* leave the repository in a broken state without explanation

## Verification Rule

Every meaningful implementation step should be followed by verification.

Verification may include:

* TypeScript compilation
* Electron startup
* IPC contract checks
* manual workflow validation
* targeted tests
* log inspection

---

# Verification Gates

Every phase ends at a verification gate. The gate is a hard stop: if it fails, work must not proceed to the next phase until the failure is resolved.

## Gate A — Basic Integrity

Applies after every phase. All of the following must pass:

* `npm install` resolves without errors
* `npm run typecheck` exits with 0 errors, 0 warnings
* `npm run dev` starts without crash
* Electron main process launches without uncaught exceptions
* Main window opens and displays content
* Renderer reaches interactive state (no white screen, no infinite spinner)
* DevTools console has no uncaught errors or warnings (filtering out intentional debug output)

## Gate B — Feature Integrity

Applies after phases that modify feature code (Phase 6 onwards). In addition to Gate A:

* Feature-specific workflows verified end to end
* No regressions in features gated by previous phases
* IPC channels modified by this phase respond correctly to requests

## Gate Enforcement

If a gate fails:

1. Stop immediately. Do not start the next phase.
2. Document the failure with reproduction steps.
3. Fix within the current phase.
4. Re-run the gate from scratch.
5. Only then proceed.

A phase is not considered complete until its gate passes.

---

# Phase Locking

Once a phase completes and its verification gate passes, the files it owns become locked.

## Rule

No future phase may modify files owned by an earlier completed phase unless:

1. A documented justification exists explaining why the change is necessary.
2. Regression verification is performed against the original phase's acceptance criteria.
3. The change is reviewed and approved.

## Ownership

Each phase defines which files it owns. Ownership is recorded in the phase's deliverable artifacts.

If Phase 3 fixes IPC channel definitions, Phase 8 cannot casually rename those same channels. If Phase 3's change is genuinely wrong, the fix must include a justification and a regression check.

## Purpose

Phase locking prevents AI agents from undoing each other's work across phases. Without it, later phases can silently reintroduce the exact problems earlier phases fixed.

This is not a ban on cross-phase changes. It is a process requirement that cross-phase changes be intentional, documented, and verified.

---

# Permanent Artifacts

Each phase produces a permanent artifact document. These artifacts accumulate over the course of the program so that the project gradually documents itself. Later phases read these artifacts instead of reverse-engineering the code.

| Phase | Artifact | Contents |
|-------|----------|----------|
| -1 | `audit/audit-validation.md` | Audit findings, accuracy check, gaps discovered |
| 0 | `reports/emergency-stabilization.md` | Blockers fixed, build status, startup status |
| 1 | `architecture/architecture.md` | Target folder structure, layer boundaries, ownership rules |
| 1 | `architecture/services.md` | Service inventory, responsibilities, dependencies |
| 1 | `architecture/domain-models.md` | Domain model definitions, relationships |
| 1 | `architecture/adrs/` | Architecture Decision Records |
| 2 | `ipc/ipc.md` | Channel list, request/response schemas, handler ownership |
| 3 | `widgets/widget-lifecycle.md` | Widget lifecycle, registry, persistence rules |
| 4 | `vault/vault.md` | Vault service, workspace resolution, path rules |
| 5 | `renderer/renderer-tree.md` | Component tree, state flow, feature boundaries |
| 6 | `features/feature-matrix.md` | Feature status, ownership, workflow status |
| 7 | `storage/storage.md` | Persistence paths, indexing, search, PDF workflows |
| 8 | `reports/tech-debt.md` | Dead code removed, duplicates consolidated |
| 9 | `reports/runtime-validation.md` | End-to-end workflow results, regression list |
| 10 | `reports/production-readiness.md` | Packaging, security, release checklist |

Artifacts are stored in the repository under a `docs/` directory.

---

# Overall Roadmap

The Nabu Recovery Program is divided into twelve phases.

* Phase -1 — Audit Validation
* Phase 0 — Emergency Stabilization
* Phase 1 — Architecture Migration
* Phase 2 — IPC Stabilization
* Phase 3 — Widget System
* Phase 4 — Vault & Workspace
* Phase 5 — Renderer & UX
* Phase 6 — Feature Recovery
* Phase 7 — Storage, Search & PDF
* Phase 8 — Technical Debt Elimination
* Phase 9 — Runtime Validation
* Phase 10 — Production Readiness

Each phase contains multiple subphases with explicit objectives, guardrails, deliverables, acceptance criteria, rollback plans, and prompt estimates.

## Subphase Sizing Rule

Every subphase must be sized for **1 to 4 prompts**.

If a task would require more than 4 prompts, it must be broken into another subphase before execution.

This rule applies to every phase in the program.

---

# Phase -1 — Audit Validation

## Objective

Validate the existing audit findings and identify anything the audit may have missed.

**Rule:** No production code changes are allowed during this phase.

## Scope

This phase is purely observational. It confirms existing knowledge rather than discovering from scratch.

Previous investigative work (the codebase audit) has already identified entry points, dependency issues, IPC gaps, dead code, startup flow, and critical blockers. This phase validates that work is still accurate.

## Deliverables

* Audit validation report
* List of any findings the audit missed
* Confirmed critical blocker list (handed to Phase 0)

## Risks

* Audit may be partially stale if the codebase changed since it was run
* Subtle dependencies may have been missed during initial investigation

## Dependencies

* Access to the full repository
* Access to the existing audit report

## Subphases

### Subphase -1.1 — Audit Verification (2 prompts)

Verify the existing audit findings against the current state of the repository.

**Definition of done:**

* Every existing audit finding is confirmed or flagged as stale
* Critical blocker list is accurate and complete
* No investigation remaining of entry points, IPC, or dead code

**Non-goals:**

* No production code changes
* No new audit scope beyond verifying existing findings

**Output:**

* Confirmed findings
* Stale or incorrect findings (marked for update)
* Any critical blockers not yet captured

---

### Subphase -1.2 — Gap Discovery (1 prompt)

Check for anything the audit may have missed: hidden entry points, unlisted modules, untraced IPC channels, or stale configuration.

**Definition of done:**

* Gap list is complete (or confirmed empty)
* Updated risk assessment is documented
* Phase 0 blocker list is final

**Non-goals:**

* No production code changes
* No deep dive into any single gap (flag for later phases)

**Output:**

* Gap list (may be empty)
* Updated risk assessment

## Prompt Estimate

**Total:** 3 prompts across 2 subphases

## Acceptance Criteria

* Existing audit findings are validated or corrected
* No major category of issue remains unexplored
* Critical blocker list is ready for Phase 0

## Exit Criteria

The team has a confirmed, accurate understanding of the codebase state.

## Rollback Strategy

No rollback is required because this phase does not change production code.

## Lessons Learned

Document discrepancies between the initial audit and the current repository state.

---

# Phase 0 — Emergency Stabilization

## Objective

Fix known build and startup blockers to reach a verified green baseline before any architecture work begins.

**Rule:** This phase is strictly about reaching a working baseline. No refactoring, no reorganization, no architecture changes. Fix what is broken, nothing more.

## Scope

* Compilation errors that block `npm run dev` or `npm run typecheck`
* Missing or incorrect exports that cause runtime crashes
* Clear TypeScript errors (unused variables, missing returns) that block clean compilation
* Preload wiring and channel alignment issues
* Startup crashes in the Electron main process or renderer
* Any additional blocker discovered during Phase -1

## Out of Scope

* Feature redesign
* Architecture changes
* File reorganization
* Dead code removal (deferred to Phase 8)
* New capabilities

## Deliverables

* Working build (`npm run typecheck` passes with 0 errors)
* Working startup (app launches, window opens, renderer loads)
* Baseline verification gate report
* List of remaining non-blocking issues for later phases

## Subphases

### Subphase 0.1 — Fix Known Build Blockers (2 prompts)

Fix the critical issues identified in the audit that prevent compilation.

**Target issues:**

* WidgetManager export missing (C1)
* Missing `getVaultForPath()` on VaultRegistry (C2) — may be replaced with direct vault path passing
* TypeScript unused-variable errors (TS6133) across whisper.ts, widget-manager.ts, ipc.ts, pdf-viewer.ts, and all renderer files

**Definition of done:**

* All known build-blocking TypeScript errors are resolved
* `npm run typecheck` passes with 0 errors
* No new errors introduced

**Non-goals:**

* No behavior changes beyond fixing errors
* No refactoring or architecture changes
* No dead code removal

**Output:**

* Fixes applied for all known build blockers
* TypeScript compilation passes

---

### Subphase 0.2 — Fix Known Startup Blockers (2 prompts)

Fix issues that prevent the application from launching and reaching an interactive state.

**Target issues:**

* Missing IPC handlers (VAULT_SWITCH, VAULT_GET_RECENTS, VIEW_STATE_GET/SET_FOLD, PROPERTIES_READ, BOOKMARKS_*, NOTE_COMPOSE, NOTE_UNIQUE)
* Preload channel name mismatches (vault:opened-test vs vault:opened)
* Orphaned feature-registrations.ts — wire into startup or document as intentionally deferred
* Runtime require() issues (yaml, path inside function bodies)
* WidgetManager import in index.ts

**Definition of done:**

* All missing IPC handlers registered
* Preload API surface matches main process handlers
* Application starts without uncaught exceptions

**Non-goals:**

* No IPC contract redesign
* No feature rewrites
* No UI changes

**Output:**

* IPC handlers registered for all channels the renderer calls
* Preload API surface matches main process handlers
* Feature toggles wired into startup

---

### Subphase 0.3 — Get Electron Window Opening (2 prompts)

Ensure the full startup path works: Electron launch → window creation → preload load → renderer boot.

**Target issues:**

* Window creation timing and configuration
* Preload script path resolution
* Renderer boot sequence
* pollForVault() retry loop behavior

**Definition of done:**

* Application launches without crashes
* Main window opens and displays content
* Renderer reaches interactive state
* DevTools console shows no uncaught errors

**Non-goals:**

* No window UX improvements (sizing, positioning)
* No feature-level fixes beyond startup

**Output:**

* Application launches without crashes
* Main window opens and displays content
* Renderer reaches interactive state

---

### Subphase 0.4 — Baseline Verification (1 prompt)

Run the Verification Gate (Gate A) and confirm everything is green.

**Gate checks:**

* `npm install` succeeds
* `npm run typecheck` — 0 errors, 0 warnings
* `npm run dev` starts without crash
* Electron main process: no uncaught exceptions
* Window opens, renderer loads
* DevTools console: no errors

**Definition of done:**

* All gate checks pass
* Remaining non-blocking issues are documented for later phases
* Scoreboard updated with baseline metrics

**Non-goals:**

* No fixing non-blocking issues (deferred to later phases)
* No architecture changes

**Output:**

* Baseline gate report
* Green light to proceed to Phase 1

## Prompt Estimate

**Total:** 7 prompts across 4 subphases

## Acceptance Criteria

* Build compiles cleanly (0 errors, 0 warnings)
* Application launches and reaches interactive state
* Console is clean (no uncaught errors)
* Gate A passes

## Exit Criteria

The repository has a verified green baseline. All known blocking issues are fixed. Remaining issues are non-critical and documented for later phases.

## Rollback Strategy

Revert individual fixes if they introduce regressions. Each subphase fix should be granular enough to roll back independently.

## Lessons Learned

Document which fixes were straightforward and which revealed hidden coupling.

---

# Phase 1 — Architecture Migration

**Important Rule:** This phase is structural, not functional.

The goal is to prepare the repository for future work while preserving behavior.

## Objective

Introduce the target architecture without changing user-visible behavior.

## Scope

This phase focuses on folder structure, service boundaries, shared contracts, typed IPC, eventing, and layer separation.

## Out of Scope

* Feature redesign
* UI redesign
* New product capabilities
* Behavior changes unrelated to architecture

## Deliverables

* Feature-based folder structure
* Service layer extraction
* Domain model definitions
* Shared contracts + typed IPC framework
* Internal event bus + layer enforcement
* Architecture validation report

## Risks

* Import breakage during file moves
* Hidden coupling between layers
* Over-extraction of abstractions
* Behavior regressions caused by structural changes

## Dependencies

* Phase -1 audit validation
* Phase 0 emergency stabilization (green build baseline)

## Subphases

### Subphase 1.1 — Design: folder structure, domain models, ownership (1 prompt)

Define the target folder layout (see Architecture Goal 7 for the concrete layout), domain model types, and ownership rules. Combined because these are all design decisions with no implementation risk.

**Definition of done:**

* Target folder tree matches Architecture Goal 7 layout
* Domain model types defined in `src/shared/models/`
* Ownership rules and migration order documented

**Non-goals:**

* No file moves or code changes in this subphase
* No behavioral analysis

**Output:**

* Proposed folder tree matching the target layout: `src/main/services/`, `src/main/ipc/`, `src/renderer/features/`, `src/shared/`
* Domain model type definitions (Note, Vault, Workspace, Tag, GraphNode) in `src/shared/models/`
* Ownership rules and migration order

---

### Subphase 1.2 — Feature folder migration (2 prompts)

Move files into feature-oriented folders following the target layout (`src/main/services/`, `src/main/ipc/`, `src/renderer/features/`). Update all imports to match the new structure.

**Definition of done:**

* All files moved to feature folders per the target layout
* Imports updated consistently across all moved files
* Build passes (Gate A)
* No behavioral regressions

**Non-goals:**

* No service extraction (deferred to Subphase 1.3)
* No IPC contract changes
* No logic modifications

**Output:**

* `src/main/services/` created with service files extracted from current monoliths
* `src/main/ipc/` directory created (files per feature area, to be populated in Phase 2)
* `src/renderer/features/` created with feature subdirectories
* Imports updated consistently across all moved files
* Build verified after each prompt

---

### Subphase 1.3 — Service layer extraction (2 prompts)

Extract business logic embedded in UI or Electron bootstrap code into focused services with clear boundaries, placed in `src/main/services/`.

**Definition of done:**

* Service boundaries defined for all features
* Service files created in `src/main/services/`
* UI and bootstrap code no longer contain business logic
* Build passes (Gate A)

**Non-goals:**

* No behavior changes to extracted logic
* No IPC layer changes
* No UI changes

**Output:**

* Service boundaries defined (VaultService, SearchService, PdfService, WidgetService, DictationService)
* Service files created in `src/main/services/`
* Thin wrappers where needed

---

### Subphase 1.4 — Shared contracts + typed IPC framework (2 prompts)

Centralize shared schemas, types, and validation. Build typed IPC channel registry with request/response/error contracts.

**Definition of done:**

* Shared contracts layer created with types, schemas, and validation
* Typed IPC channel registry defines all channels with request/response/error contracts
* Build passes (Gate A)

**Non-goals:**

* No handler implementation changes
* No preload API changes (deferred to Phase 2)
* No service logic changes

**Output:**

* Shared contracts layer (types, schemas, validation)
* Typed channel registry
* Request/response/error contracts

---

### Subphase 1.5 — Event bus + layer enforcement (2 prompts)

Introduce typed event bus for background workflows. Enforce clean layer boundaries and remove cross-layer imports.

**Definition of done:**

* Typed event bus implementation is operational
* Layer boundary rules are documented and enforced
* All cross-layer imports violating rules are removed
* Build passes (Gate A)

**Non-goals:**

* No changing event subscribers' behavior
* No removing legitimate cross-layer communication

**Output:**

* Event bus implementation and typed events
* Layer boundary rules
* Cross-layer imports cleaned up

---

### Subphase 1.6 — Import cleanup, verification, ADRs (2 prompts)

Normalize import aliases, verify the architecture compiles and preserves behavior, document decisions.

**Definition of done:**

* Import paths are consistent and use aliases where appropriate
* Architecture validation report confirms target structure is intact
* ADRs written for major architecture decisions
* Build and launch verified (Gate A)

**Non-goals:**

* No behavioral changes
* No additional architecture work beyond cleanup

**Output:**

* Clean import paths and alias consistency
* Architecture validation report
* ADRs for major decisions

## Prompt Estimate

**Total:** 11 prompts across 6 subphases

## Acceptance Criteria

* Target architecture established
* No behavioral regressions
* Build still succeeds (Gate A passes)

## Exit Criteria

Target architecture established.
No behavioral regressions.
Build still succeeds.

## Rollback Strategy

Revert file moves, alias changes, and structural edits in the smallest possible set if the migration breaks the build or runtime behavior.

## Lessons Learned

Document the architectural decisions that were necessary and the ones that were rejected.

---

# Phase 2 — IPC Stabilization

## Executive Summary

Audit, type, and stabilize all IPC pathways.

## Objectives

* Eliminate orphaned channels
* Standardize request/response contracts
* Improve error handling
* Align preload and main process APIs

## Scope

* IPC handlers
* Preload APIs
* Channel definitions
* Error contracts
* Splitting monolithic `ipc.ts` into per-feature modules

## Out of Scope

* Feature redesign
* UI redesign
* Storage redesign

## Deliverables

* Per-feature IPC modules in `src/main/ipc/` (vault.ts, notes.ts, search.ts, pdf.ts, settings.ts, widgets.ts, dictation.ts) with `register*IPC()` bootstrap pattern
* Typed IPC registry
* Stable preload surface
* Consistent error handling
* IPC documentation (permanent artifact: `docs/ipc/ipc.md`)

## Risks

* Channel drift
* Duplicate handlers
* Inconsistent payload shapes

## Dependencies

* Phase 1 architecture migration (typed IPC framework)
* Phase 0 emergency stabilization (green build baseline)

## Subphases

### Subphase 2.1 — IPC inventory + channel contracts (2 prompts)

Inventory every IPC channel, handler, and preload exposure. Define typed request/response/error contracts for each channel. Combined because inventory feeds directly into contract definition with no separate decision step.

**Definition of done:**

* Every IPC channel, handler, and preload API is catalogued
* Typed contracts exist for all channels (request, response, error shapes)
* File layout for `src/main/ipc/` with per-feature ownership is defined

**Non-goals:**

* No handler implementation changes in this subphase
* No preload modifications

**Output:**

* Complete channel, handler, and preload API list
* Shared IPC contracts with typed request/response/error definitions
* File layout for `src/main/ipc/` with one file per feature area

---

### Subphase 2.2 — Preload API alignment (2 prompts)

Align all preload APIs with the typed IPC contracts from Phase 2.1.

**Definition of done:**

* All preload APIs match typed IPC contracts
* Renderer-side access is fully typed
* API consistency is verified against main process handlers

**Non-goals:**

* No handler implementation changes
* No new IPC channels

**Output:**

* Correct preload surface matching main process handlers
* Typed renderer-side access
* Verified API consistency

---

### Subphase 2.3 — Handler consolidation + per-feature modules + orphan cleanup (2 prompts)

Consolidate duplicate or overlapping handlers into single owners. Split the monolithic ipc.ts into per-feature modules in `src/main/ipc/`, each exporting a `register*IPC()` function. Remove orphaned channels and stale preload APIs.

**Definition of done:**

* One handler per channel with clear ownership
* `src/main/ipc/` contains per-feature modules with bootstrap pattern
* Orphaned channels and stale preload APIs removed
* Build passes (Gate A)

**Non-goals:**

* No changing handler behavior during consolidation
* No contract changes

**Output:**

* One handler per channel with clear ownership
* `src/main/ipc/` created with per-feature files (vault.ts, notes.ts, search.ts, pdf.ts, settings.ts, widgets.ts, dictation.ts)
* `src/main/ipc/index.ts` bootstrap that calls all `register*IPC()` functions
* Clean IPC graph with dead pathways removed

---

### Subphase 2.4 — Error normalization + verification (2 prompts)

Standardize error handling across all IPC responses. Verify all pathways work end to end.

**Definition of done:**

* All IPC responses use structured error shapes
* Consistent failure responses across all channels
* Channel behavior verified through Gate A

**Non-goals:**

* No contract changes
* No new channels

**Output:**

* Structured error shapes and consistent failure responses
* Verified channel behavior through Gate A

---

### Subphase 2.5 — Documentation (1 prompt)

Document the final IPC surface for future maintenance.

**Definition of done:**

* `docs/ipc/ipc.md` is complete with channel list, schemas, and handler ownership
* Documentation is accurate against the current codebase

**Non-goals:**

* No code changes
* No architecture changes

**Output:**

* `docs/ipc/ipc.md` with channel list, schemas, and handler ownership

## Prompt Estimate

**Total:** 9 prompts across 5 subphases

## Acceptance Criteria

* Every IPC channel is documented
* Every exposed API is typed
* No orphaned handlers remain
* Gate A passes

## Exit Criteria

IPC is stable enough to support feature recovery.

## Rollback Strategy

Restore previous handler registrations if a typed contract breaks runtime behavior.

## Lessons Learned

Document channel ownership and contract patterns.

---

# Phase 3 — Widget System

## Executive Summary

Refactor widget lifecycle, ownership, and persistence.

## Objectives

* Establish a single widget service
* Clarify widget registration and removal
* Stabilize widget persistence
* Improve widget rendering consistency

## Scope

* Widget registry
* Widget lifecycle
* Widget persistence
* Widget UI integration

## Out of Scope

* Full UI redesign
* Unrelated feature changes

## Deliverables

* WidgetService
* Widget registry
* Typed widget events
* Widget lifecycle documentation (permanent artifact: `docs/widgets/widget-lifecycle.md`)

## Risks

* Duplicate widget ownership
* Persistence inconsistencies
* UI state drift

## Dependencies

* Phase 1 architecture migration (service layer)
* Phase 2 IPC stabilization

## Subphases

### Subphase 3.1 — Widget audit + lifecycle consolidation (2 prompts)

Audit all widget-related code, state, and persistence paths. Consolidate creation, update, and removal into one lifecycle owner.

**Definition of done:**

* Widget code, state, and persistence paths are fully mapped
* Widget lifecycle has a single owner with deterministic create/update/remove flow
* Build passes (Gate A)

**Non-goals:**

* No widget redesign or new widget types
* No UI restyling beyond what consolidation requires

**Output:**

* Widget ownership and lifecycle map
* Single widget lifecycle path with reduced duplication

---

### Subphase 3.2 — Persistence alignment + UI cleanup (2 prompts)

Align widget registry behavior with persistence behavior. Clean up widget UI integration and remove coupling to unrelated systems.

**Definition of done:**

* Widget registry and persistence are in sync (save/load matches state)
* Widget UI component is decoupled from unrelated systems
* Build passes (Gate A)

**Non-goals:**

* No widget visual redesign
* No adding new widget types

**Output:**

* Consistent registry state with stable save/load flow
* Cleaner widget UI with predictable rendering

---

### Subphase 3.3 — Verification + documentation (1 prompt)

Verify widget behavior after consolidation. Produce the permanent artifact.

**Definition of done:**

* All widget workflows validated: show, hide, persist, restore
* Widget lifecycle documented in permanent artifact
* Gate A passes

**Non-goals:**

* No functional changes beyond bug fixes found during verification

**Output:**

* Widget workflow validation report
* `docs/widgets/widget-lifecycle.md`

## Prompt Estimate

**Total:** 5 prompts across 3 subphases

## Acceptance Criteria

* Widgets have one clear owner
* Widget lifecycle is predictable
* Widget state persists correctly
* Gate A passes

## Exit Criteria

Widget behavior is stable and maintainable.

## Rollback Strategy

Revert widget lifecycle changes if persistence or rendering breaks.

## Lessons Learned

Document widget ownership and lifecycle rules.

---

# Phase 4 — Vault & Workspace

## Executive Summary

Stabilize vault and workspace loading, ownership, and lifecycle management.

## Objectives

* Clarify active vault handling
* Stabilize workspace resolution
* Improve file watching and indexing triggers
* Reduce ambiguity in workspace state

## Scope

* Vault registry
* Workspace loading
* Path resolution
* Watchers
* Indexing triggers

## Out of Scope

* Sync features
* Cloud storage
* Major UX redesign

## Deliverables

* VaultService
* WorkspaceService
* Deterministic vault lifecycle
* Vault documentation (permanent artifact: `docs/vault/vault.md`)

## Risks

* Path resolution bugs
* Watcher duplication
* State inconsistency

## Dependencies

* Phase 1 architecture migration (service layer)
* Phase 2 IPC stabilization
* Phase 0 emergency stabilization (green build baseline)

## Subphases

### Subphase 4.1 — Vault + workspace inventory (1 prompt)

Inventory vault- and workspace-related code, state, persistence, and path handling in a single pass.

**Definition of done:**

* All vault- and workspace-related code, state, and persistence paths are mapped
* Lifecycle ownership is documented
* Path handling rules are catalogued

**Non-goals:**

* No code changes in this subphase
* No service extraction

**Output:**

* Vault and workspace ownership map
* Lifecycle map
* Path handling map

---

### Subphase 4.2 — Lifecycle consolidation (2 prompts)

Consolidate vault and workspace lifecycle handling into clear service boundaries with deterministic open/close flow.

**Definition of done:**

* VaultService and WorkspaceService exist with clear boundaries
* Vault lifecycle has a single deterministic open/close flow
* Build passes (Gate A)

**Non-goals:**

* No watcher changes (deferred to Subphase 4.3)
* No path resolution changes

**Output:**

* VaultService and WorkspaceService
* Single lifecycle owner
* Deterministic open/close flow

---

### Subphase 4.3 — Watcher cleanup + path resolution repair (2 prompts)

Clean up file watchers, reduce duplicate indexing triggers, and repair path resolution logic.

**Definition of done:**

* File watchers have clear ownership with no duplicates
* Indexing triggers are reduced to minimum necessary
* Path resolution is correct and predictable
* Build passes (Gate A)

**Non-goals:**

* No vault lifecycle changes
* No service restructuring

**Output:**

* Stable watcher behavior with clear ownership
* Correct path resolution rules

---

### Subphase 4.4 — Verification + documentation (1 prompt)

Verify vault and workspace behavior through Gate A and produce the permanent artifact.

**Definition of done:**

* Gate A passes
* Vault lifecycle workflows are validated: open, close, reload
* `docs/vault/vault.md` is complete and accurate

**Non-goals:**

* No additional lifecycle changes beyond bug fixes

**Output:**

* Workflow validation report
* `docs/vault/vault.md`

## Prompt Estimate

**Total:** 6 prompts across 4 subphases

## Acceptance Criteria

* Active vault is unambiguous
* Workspace loading is deterministic
* Watchers behave predictably
* Gate A passes

## Exit Criteria

Vault and workspace behavior is stable enough for broader feature recovery.

## Rollback Strategy

Revert watcher or lifecycle changes that break vault loading.

## Lessons Learned

Document workspace ownership and path resolution rules.

---

# Phase 5 — Renderer & UX

## Executive Summary

Refactor renderer structure and stabilize user-facing interaction flows.

## Objectives

* Improve renderer organization
* Reduce UI coupling
* Stabilize state management
* Make feature boundaries visible in the UI layer

## Scope

* Renderer feature folders
* UI state
* Shared components
* Interaction flows

## Out of Scope

* Major visual redesign
* New product features unrelated to stability

## Deliverables

* Feature-based renderer structure
* Cleaner component ownership
* Stable UI state flows
* Renderer documentation (permanent artifact: `docs/renderer/renderer-tree.md`)

## Risks

* State duplication
* Component coupling
* UI regressions

## Dependencies

* Phase 1 architecture migration (feature-based organization)
* Phase 2 IPC stabilization

## Subphases

### Subphase 5.1 — Renderer inventory + feature folder alignment (2 prompts)

Map the renderer's current structure, components, and state. Align code with feature-based folder ownership in the same pass.

**Definition of done:**

* Complete renderer map with component and state inventory
* Feature folders created with clear ownership boundaries
* Cross-feature coupling is reduced
* Build passes (Gate A)

**Non-goals:**

* No state management changes (deferred to Subphase 5.2)
* No component restructuring (deferred to Subphase 5.3)

**Output:**

* Renderer map with component and state inventory
* Feature folders with clear ownership
* Reduced cross-feature coupling

---

### Subphase 5.2 — State cleanup (2 prompts)

Clean up renderer state management: remove duplication, clarify ownership, make state flow predictable.

**Definition of done:**

* State duplication is eliminated or explicitly justified
* State ownership is clear for each piece of shared state
* State flow is predictable and traceable
* Build passes (Gate A)

**Non-goals:**

* No component restructuring (deferred to next subphase)
* No business logic extraction (deferred to Subphase 5.4)

**Output:**

* Reduced state duplication
* Clear state ownership
* Predictable state flow

---

### Subphase 5.3 — Component ownership cleanup (1 prompt)

Clarify component responsibilities and reduce coupling.

**Definition of done:**

* Component boundaries are clear and documented
* Excessive coupling between components is resolved
* Build passes (Gate A)

**Non-goals:**

* No extracting business logic (deferred to Subphase 5.4)
* No state changes

**Output:**

* Clearer component boundaries
* Reduced coupling between components

---

### Subphase 5.4 — Thin UI enforcement (1 prompt)

Audit all components for business logic violations (Architecture Goal 9). Extract any business logic found into commands or services. Ensure components only display state and call commands.

**Definition of done:**

* All components audited for business logic violations
* Extracted logic moved to commands or services
* No component contains business logic
* Build passes (Gate A)

**Non-goals:**

* No component-level restructuring
* No state changes

**Output:**

* Components audited for business logic
* Extracted logic moved to commands or services
* Violation list with fixes applied

---

### Subphase 5.5 — Interaction verification + regression review (2 prompts)

Verify major interaction flows and check for regressions introduced during cleanup.

**Definition of done:**

* All major interaction flows are verified end to end
* Regressions are catalogued and fixed
* Gate A passes

**Non-goals:**

* No additional refactoring
* No feature changes

**Output:**

* Verified interaction paths
* Regression report with fixes

---

### Subphase 5.6 — Documentation (1 prompt)

Document the final renderer structure and ownership.

**Definition of done:**

* `docs/renderer/renderer-tree.md` covers component tree, ownership, state flow
* Documentation is accurate against the current codebase
* Gate A passes

**Non-goals:**

* No code changes

**Output:**

* `docs/renderer/renderer-tree.md`

## Prompt Estimate

**Total:** 9 prompts across 6 subphases

## Acceptance Criteria

* Renderer structure is feature-based
* UI state is predictable
* Interaction flows remain stable
* Gate A passes

## Exit Criteria

Renderer code is easier to maintain and extend.

## Rollback Strategy

Revert renderer refactors that introduce UI regressions.

## Lessons Learned

Document renderer ownership and state flow conventions.

---

# Phase 6 — Feature Recovery

## Executive Summary

Audit every feature and repair broken or incomplete functionality.

## Objectives

* Identify feature status
* Repair broken workflows
* Remove placeholders where appropriate
* Restore feature completeness

## Scope

* Existing features
* Feature-specific services
* Feature-specific UI
* Feature-specific IPC

## Out of Scope

* New feature design
* Major architecture changes

## Deliverables

* Feature status matrix (permanent artifact: `docs/features/feature-matrix.md`)
* Feature verification report

## Risks

* Hidden feature dependencies
* Incomplete feature ownership
* Regression during repair

## Dependencies

* Phase 4 vault/workspace stability
* Phase 5 renderer cleanup
* Phase 2 IPC stabilization

## Subphases

### Subphase 6.1 — Feature status matrix + triage (2 prompts)

Create the complete feature status matrix (working/broken/incomplete/placeholder/deprecated) and prioritize broken features by user impact.

**Definition of done:**

* Every feature has a clear status in the matrix
* Broken features are prioritized by user impact
* Triage order is documented and ready for execution

**Non-goals:**

* No code changes in this subphase
* No feature redesign discussions

**Output:**

* Feature status matrix
* Triage order and repair priority list

---

### Subphase 6.2 — Workflow repair + placeholder replacement (3 prompts)

Repair the highest-priority broken workflows. Replace placeholder implementations with real code where appropriate.

**Definition of done:**

* Highest-priority broken workflows are restored
* Placeholder implementations are replaced or documented as intentionally deferred
* Build passes (Gate A)

**Non-goals:**

* No adding new features beyond repair
* No redesign of existing features
* No architecture changes

**Output:**

* Restored workflows
* Real feature behavior replacing placeholders
* Clear fallback behavior where full implementation isn't justified

---

### Subphase 6.3 — Feature verification + regression review (2 prompts)

Verify all repaired features end to end. Must pass Gate B. Review for regressions.

**Definition of done:**

* Gate B passes (all repaired features verified end to end)
* Regressions are catalogued and fixed
* Feature matrix updated with current status

**Non-goals:**

* No new repairs beyond what was triaged
* No architecture changes

**Output:**

* Feature validation notes
* Regression report with follow-up fixes

---

### Subphase 6.4 — Documentation + cleanup (1 prompt)

Document feature ownership and status in the permanent artifact. Clean up temporary repair artifacts.

**Definition of done:**

* `docs/features/feature-matrix.md` is complete and accurate
* Temporary repair artifacts are cleaned up
* Gate B passes

**Non-goals:**

* No additional feature work

**Output:**

* `docs/features/feature-matrix.md`
* Cleanup list

## Prompt Estimate

**Total:** 8 prompts across 4 subphases

## Acceptance Criteria

* Each feature has a clear status
* Broken workflows are repaired
* Placeholder behavior is minimized
* Gate B passes

## Exit Criteria

Core features are functional and documented.

## Rollback Strategy

Revert feature-specific changes that break established workflows.

## Lessons Learned

Document recurring feature failure patterns.

---

# Phase 7 — Storage, Search & PDF

## Executive Summary

Stabilize persistence, indexing, search, and PDF-related workflows.

## Objectives

* Improve storage reliability
* Stabilize search indexing
* Repair PDF workflows
* Improve caching and metadata handling

## Scope

* Persistence
* Search
* Indexing
* PDF rendering
* PDF annotations
* Caching

## Out of Scope

* Cloud sync
* External storage services
* Major UX redesign

## Deliverables

* Stable storage layer
* Search indexing verification
* PDF workflow repair
* Metadata consistency
* Storage documentation (permanent artifact: `docs/storage/storage.md`)

## Risks

* Data corruption
* Index drift
* PDF rendering inconsistencies

## Dependencies

* Phase 4 vault/workspace stability
* Phase 2 IPC stabilization

## Subphases

### Subphase 7.1 — Storage + search + PDF inventory (2 prompts)

Inventory all storage, search, indexing, and PDF-related code in a combined pass. These are all read-only discovery tasks that don't depend on each other.

**Definition of done:**

* Storage persistence paths and data formats are fully mapped
* Search indexing paths and query behavior are documented
* PDF rendering and annotation paths are catalogued

**Non-goals:**

* No code changes in this subphase
* No performance analysis

**Output:**

* Storage map with persistence paths and data formats
* Search map with indexing paths and query behavior
* PDF map with rendering and annotation paths

---

### Subphase 7.2 — Indexing repair (2 prompts)

Repair indexing behavior and align it with storage and search expectations.

**Definition of done:**

* Indexing flow is stable with verified update triggers
* Index drift is eliminated or minimized
* Build passes (Gate A)

**Non-goals:**

* No storage schema changes
* No search query behavior changes
* No PDF changes

**Output:**

* Stable indexing flow with verified update triggers
* Reduced index drift

---

### Subphase 7.3 — Metadata cleanup (1 prompt)

Clean up metadata handling across storage, search, and PDF workflows.

**Definition of done:**

* Metadata handling is consistent across all workflows
* Metadata ownership is clear with no duplication
* Build passes (Gate A)

**Non-goals:**

* No storage schema changes
* No indexing changes

**Output:**

* Consistent metadata with clear ownership
* Reduced duplication

---

### Subphase 7.4 — Verification + documentation (1 prompt)

Verify storage, search, and PDF behavior through Gate B. Produce the permanent artifact.

**Definition of done:**

* Gate B passes (storage, search, PDF workflows validated)
* `docs/storage/storage.md` is complete and accurate
* Regression notes are documented

**Non-goals:**

* No additional fixes beyond blocking issues found during verification

**Output:**

* Workflow validation with regression notes
* `docs/storage/storage.md`

## Prompt Estimate

**Total:** 6 prompts across 4 subphases

## Acceptance Criteria

* Storage is reliable
* Search works consistently
* PDF workflows are stable
* Gate B passes

## Exit Criteria

Persistence and retrieval workflows are dependable.

## Rollback Strategy

Revert storage or indexing changes that threaten data integrity.

## Lessons Learned

Document storage and indexing invariants.

---

# Phase 8 — Technical Debt Elimination

## Executive Summary

Remove dead code, duplicate implementations, and obsolete modules.

## Objectives

* Reduce clutter
* Remove unused exports
* Eliminate stale modules
* Simplify maintenance

## Scope

* Dead code
* Duplicate code
* Obsolete utilities
* Unused exports

## Out of Scope

* Behavior changes
* Feature redesign

## Deliverables

* Technical debt cleanup report (permanent artifact: `docs/reports/tech-debt.md`)
* Dead code removal
* Simplified module graph

## Risks

* Removing code still used indirectly
* Over-cleaning during uncertain areas

## Dependencies

* Phase 6 feature recovery
* Phase 7 storage/search/PDF stabilization

## Subphases

### Subphase 8.1 — Dead code + duplicate inventory (1 prompt)

Identify dead code, unused exports, stale modules, and duplicate implementations in a single pass.

**Definition of done:**

* All dead code, unused exports, stale modules, and duplicates are catalogued
* Removal candidates are confirmed safe (not used indirectly)

**Non-goals:**

* No actual code removal in this subphase
* No architecture changes

**Output:**

* Dead code, unused export, stale module list
* Duplicate code map with consolidation candidates

---

### Subphase 8.2 — Removal + export cleanup (2 prompts)

Remove obsolete modules, delete dead code, and clean up unused exports. Consolidate duplicate implementations.

**Definition of done:**

* All dead code from inventory is removed
* Unused exports eliminated, module surfaces cleaned up
* Build passes (Gate A)

**Non-goals:**

* No behavior changes to remaining code
* No refactoring beyond removal

**Output:**

* Removed modules with updated imports
* Smaller, cleaner module surfaces

---

### Subphase 8.3 — Verification + documentation (1 prompt)

Verify cleanup results and document what was removed. Must pass Gate A.

**Definition of done:**

* Build passes (Gate A)
* Removal log is complete and accurate
* All tests/workflows pass

**Non-goals:**

* No additional removal beyond what was inventoried

**Output:**

* Cleanup verification with removal log
* `docs/reports/tech-debt.md`

## Prompt Estimate

**Total:** 4 prompts across 3 subphases

## Acceptance Criteria

* Dead code is removed
* Duplicate logic is reduced
* Repository is easier to navigate
* Gate A passes

## Exit Criteria

The codebase is cleaner and easier to maintain.

## Rollback Strategy

Restore removed code if it is discovered to still be required.

## Lessons Learned

Document patterns that produced technical debt.

---

# Phase 9 — Runtime Validation

## Executive Summary

Exercise complete user workflows and validate the application under realistic use.

## Objectives

* Validate end-to-end workflows
* Confirm stability across major features
* Identify runtime regressions

## Scope

* Startup
* Vault operations
* Note editing
* Search
* Graph
* Widgets
* PDF
* Settings
* Shutdown and restart

## Out of Scope

* New feature development
* Major refactors

## Deliverables

* Runtime validation report (permanent artifact: `docs/reports/runtime-validation.md`)
* Regression list

## Risks

* Hidden runtime failures
* Workflow-specific regressions
* Environment-specific issues

## Dependencies

* Phases 0–8 completion
* Stable startup and IPC

## Subphases

### Subphase 9.1 — Startup + vault workflows (2 prompts)

Validate startup behavior (launch to interactive UI) and vault operations (open, close, reload) in sequence.

**Definition of done:**

* Startup completes without errors from launch to interactive UI
* Vault operations (open, close, reload) succeed
* All failures are documented with reproduction steps

**Non-goals:**

* No code fixes in this subphase (log failures for repair)
* No performance measurement

**Output:**

* Startup and vault workflow checklists
* Failure notes

---

### Subphase 9.2 — Note + search workflows (2 prompts)

Validate note creation, editing, saving, deletion, and search behavior.

**Definition of done:**

* Note creation, editing, saving, and deletion work end to end
* Search returns expected results for basic queries
* All failures are documented with reproduction steps

**Non-goals:**

* No code fixes in this subphase
* No edge-case search testing

**Output:**

* Note and search workflow checklists
* Failure notes

---

### Subphase 9.3 — Widget + PDF workflows (1 prompt)

Validate widget behavior and PDF viewing/annotation workflows.

**Definition of done:**

* Widget workflows (show, hide, persist, restore) work end to end
* PDF viewing and annotation workflows work end to end
* All failures are documented with reproduction steps

**Non-goals:**

* No code fixes in this subphase
* No performance measurement

**Output:**

* Widget and PDF workflow checklists
* Failure notes

---

### Subphase 9.4 — Regression review + report (1 prompt)

Review all runtime findings and produce the final validation report.

**Definition of done:**

* Runtime validation report is complete with findings and severity
* Regressions are catalogued with reference to responsible phase
* Gate B passes

**Non-goals:**

* No new validation beyond defined workflows
* No code changes

**Output:**

* Regression report
* `docs/reports/runtime-validation.md`

## Prompt Estimate

**Total:** 6 prompts across 4 subphases

## Acceptance Criteria

* Core workflows succeed end to end
* Failures are documented and repaired
* Gate B passes

## Exit Criteria

The application behaves reliably in realistic usage scenarios.

## Rollback Strategy

Revert workflow changes that introduce runtime instability.

## Lessons Learned

Document workflow assumptions and runtime edge cases.

---

# Phase 10 — Production Readiness

## Executive Summary

Prepare the application for packaging, distribution, and long-term maintenance.

## Objectives

* Verify packaging
* Validate build output
* Review security and logging
* Confirm release readiness

## Scope

* Packaging
* Build optimization
* Asset validation
* Native module verification
* Error reporting
* Logging
* Performance profiling
* Security review
* Installer validation

## Out of Scope

* New feature development
* Major architectural redesign

## Deliverables

* Production readiness report (permanent artifact: `docs/reports/production-readiness.md`)
* Release checklist

## Risks

* Packaging failures
* Platform-specific issues
* Missing assets
* Native module incompatibility

## Dependencies

* Phases 0–9 completion

## Subphases

### Subphase 10.1 — Packaging + asset validation (2 prompts)

Verify the application packages correctly and all required assets are included and correctly referenced.

**Definition of done:**

* Application packages successfully for target platform
* All required assets are included and correctly referenced
* Build artifacts are valid and complete

**Non-goals:**

* No performance analysis (deferred to Subphase 10.3)
* No security review

**Output:**

* Packaging checklist
* Build artifact verification
* Asset checklist with missing asset report

---

### Subphase 10.2 — Native module + logging review (2 prompts)

Verify native module compatibility. Review logging, error reporting, and diagnostic output for release readiness.

**Definition of done:**

* All native modules are compatible with target platform
* Logging level is appropriate for production
* Error reporting is functional (captures and surfaces errors)
* Diagnostic output meets release standards

**Non-goals:**

* No performance analysis
* No security review

**Output:**

* Native module compatibility notes
* Logging and error reporting review

---

### Subphase 10.3 — Performance + security review (2 prompts)

Review performance hotspots and basic security concerns before release.

**Definition of done:**

* Performance hotspots identified and either resolved or documented
* Basic security concerns reviewed (no critical issues)
 * Input validation
 * File system access patterns
 * IPC exposed surface
* Follow-up list documented for post-release

**Non-goals:**

* No deep security audit (basic review only)
* No performance optimization beyond obvious fixes

**Output:**

* Performance notes
* Security notes with follow-up list

---

### Subphase 10.4 — Release checklist + signoff (1 prompt)

Complete the final release checklist and produce the permanent artifact.

**Definition of done:**

* Release checklist is complete and signed off
* `docs/reports/production-readiness.md` documents all findings
* Gate A and Gate B pass
* Scoreboard finalized

**Non-goals:**

* No further code changes
* No additional testing

**Output:**

* Release checklist
* `docs/reports/production-readiness.md`

## Prompt Estimate

**Total:** 7 prompts across 4 subphases

## Acceptance Criteria

* Packaging succeeds
* Release artifacts are valid
* The application is ready for distribution
* Gate A and Gate B pass

## Exit Criteria

The application is ready for production use.

## Rollback Strategy

Revert release-specific changes if packaging or validation fails.

## Lessons Learned

Document release blockers and packaging assumptions.

---

# Recovery Scoreboard

The recovery program keeps a running dashboard of measurable progress. Updated after each phase.

| Metric | Before Phase 0 | Target | Current |
|--------|---------------|--------|---------|
| Build status | ❌ | ✅ | Measured at each gate |
| TypeScript errors | ~214 | 0 | Counted at each gate |
| Runtime startup | ❌ | ✅ | Measured at each gate |
| Broken IPC channels | 12+ | 0 | Counted in Phase 2 |
| Dead code files | ~148 | Reviewed | Counted in Phase 8 |
| Circular dependencies | ~19 | 0 | Counted in Phase 1 |
| Architecture compliance | ~32% | 100% | Measured in Phase 1 |
| Startup time | Unknown | <2s | Measured in Phase 9 |
| Memory usage | Unknown | <150MB | Measured in Phase 10 |

## Updating the Scoreboard

Every phase's final subphase updates the scoreboard. If a metric regresses, the gate must catch it before the next phase starts.

---

# Completion Checklist

The recovery program is complete when all of the following are true:

* Audit validation is complete
* Emergency stabilization is complete
* Build is stable (Gate A passes)
* Startup is stable (Gate A passes)
* Architecture migration is complete
* IPC is typed and documented
* Widget system is stable
* Vault and workspace behavior is stable
* Renderer structure is clean
* Core features are repaired
* Storage, search, and PDF workflows are stable
* Technical debt has been reduced
* Runtime validation has passed
* Production packaging succeeds

Additional completion indicators:

* The codebase is understandable without tribal knowledge
* Major subsystems have clear ownership
* The architecture matches the documented target state
* Future feature work is easier, not harder

---

# Estimated Project Size

| Phase | Subphases | Estimated Prompts |
| ----- | --------- | ----------------- |
| Phase -1 | 2 | 3 |
| Phase 0 | 4 | 7 |
| Phase 1 | 6 | 11 |
| Phase 2 | 5 | 9 |
| Phase 3 | 3 | 5 |
| Phase 4 | 4 | 6 |
| Phase 5 | 6 | 9 |
| Phase 6 | 4 | 8 |
| Phase 7 | 4 | 6 |
| Phase 8 | 3 | 4 |
| Phase 9 | 4 | 6 |
| Phase 10 | 4 | 7 |

**Total Estimated AI Prompts:** **81**

This total assumes that every subphase stays within the required 1–4 prompt range.

---

# Remaining Phases

Each of Phases 1–10 should follow the same template:

* Executive Summary
* Objectives
* Scope
* Out of Scope
* Deliverables
* Risks
* Dependencies
* Subphases (1–4 prompts each)
* Prompt Estimates
* Acceptance Criteria
* Exit Criteria
* Rollback Strategy
* Lessons Learned

This template should be used consistently so that every phase is easy to review, execute, and verify.

---

# End of Document
