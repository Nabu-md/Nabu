# Nabu Target Architecture

> **Status:** Phase 1.1 — Design (documentation only)
> **Scope:** This document defines the *target* architecture that Phase 1 subphases (1.2 – 1.6) will implement. It is a blueprint. No production code, file locations, imports, or behavior are changed by this document.
> **Authority:** Aligns with the Nabu Recovery Program — Product Principles, Architecture Goals 1–11, and the Phase 1 subphase plan.

---

## 1. Overview

Nabu is a local-first, desktop-first Electron application organized into **three architectural layers**:

| Layer | Runtime | Responsibility |
| --- | --- | --- |
| **Main** | Node/Electron main process | Filesystem, database, AI, search, PDF, dictation, IPC handlers, background work |
| **Renderer** | Chromium/React process | UI, user interaction, presentation, local UI state |
| **Shared** | Imported by both | Types, schemas, validation, contracts, pure utilities, domain models |

Within these layers, code is organized **by business capability (feature) rather than by technical layer** wherever practical (Architecture Goal 1). Dependencies flow in **one direction only** (Architecture Goal 8): UI → Services → Domain Models → Infrastructure → Electron APIs.

This document corresponds to **Architecture Goal 7 — Target Folder Layout**.

---

## 2. Proposed Folder Tree

The concrete target layout. Directories marked *(new)* do not yet exist and will be created in later subphases. Directories marked *(exists)* already exist and will be reorganized. This tree is the definition of "done" for the structural migration.

```
src/
  main/                         # MAIN LAYER — Electron / Node / background
    index.ts                    # App lifecycle, window creation, menu (bootstrap only)
    services/                   # (new) One file per domain service — application behavior
      VaultService.ts           #   open/close/scan/switch vault, file CRUD orchestration
      NoteService.ts            #   note lifecycle: create, save, rename, delete, compose
      SearchService.ts          #   full-text + vector/context search coordination
      IndexService.ts           #   index build/reindex, tag index, graph index
      PdfService.ts             #   PDF open, page render, annotations
      WidgetService.ts          #   always-on-top widget window lifecycle
      DictationService.ts       #   whisper/fn-monitor/audio capture coordination
      SettingsService.ts        #   settings + feature-toggle persistence
      TemplateService.ts        #   template listing & substitution
      adapters/                 #   (new) Infrastructure adapters (Goal 10)
        FileSystemAdapter.ts    #     interface + Electron/Node implementation
        ClipboardAdapter.ts
        DatabaseAdapter.ts
        AiAdapter.ts
    ipc/                        # (new) One file per feature area; register*IPC() bootstrap
      vault.ts                  #   registerVaultIPC()
      notes.ts                  #   registerNotesIPC()
      search.ts                 #   registerSearchIPC()
      pdf.ts                    #   registerPdfIPC()
      settings.ts               #   registerSettingsIPC()
      widgets.ts                #   registerWidgetsIPC()
      dictation.ts              #   registerDictationIPC()
      index.ts                  #   calls all register*IPC() functions
  preload/                      # PRELOAD — secure contextBridge (unchanged by Goal 7)
    index.ts                    #   exposes typed electronAPI over IPC channels
    index.d.ts                  #   global type declarations for the renderer
  renderer/                     # RENDERER LAYER — React presentation
    index.html
    src/
      main.tsx                  #   React entry
      App.tsx                   #   root shell / layout / providers
      features/                 # (new) One folder per feature capability
        notes/                  #   note editing, viewing, blocks, backlinks
        search/                 #   search UI, quick switcher, results
        graph/                  #   graph visualization
        settings/               #   settings & feature-toggle UI
        widgets/                #   clipboard / dictation widget UI
        pdf/                    #   PDF viewer UI
        vault/                  #   file tree, vault navigation, setup wizard
      components/               # (exists) Shared, feature-agnostic UI components
      hooks/                    # (new) Shared React hooks
      commands/                 # (exists) Command registry & registrations (Goal 11)
      markdown/                 # (exists) Renderer markdown pipeline
      utils/                    # (exists) Renderer-only pure helpers
      assets/                   # (exists) CSS, icons, images
  shared/                       # SHARED LAYER — no Electron/React/Browser/Node deps
    models/                     # (new) Domain models — see domain-models.md
      Note.ts
      Vault.ts
      Workspace.ts
      Tag.ts
      GraphNode.ts
      Attachment.ts
      index.ts
    schemas/                    # (new) Zod schemas (request/response/error contracts)
      index.ts
    types/                      # (new) TypeScript type definitions (non-domain)
      index.ts
    contracts/                  # (new, Phase 1.4) Typed IPC channel registry
      channels.ts               #   channel identifiers
      events.ts                 #   typed event-bus event definitions (Phase 1.5)
    utils/                      # (exists, to consolidate) Pure cross-layer utilities
```

> **Note on `preload/`:** Architecture Goal 7 lists only `main/`, `renderer/`, and `shared/`. The preload bridge is a thin, security-critical seam that belongs conceptually to the Main layer's IPC surface. It is documented here for completeness but is **not** reorganized by feature; it remains a single curated contract file.

### 2.1 Directory Responsibilities

**`src/main/`** — The only layer permitted to touch Electron, the filesystem, native modules, the database, AI models, and OS-level APIs.

- **`main/index.ts`** — Bootstrap only: create windows, wire the application menu, initialize services, call `ipc/index.ts` to register handlers. No business logic.
- **`main/services/`** — Application behavior. One file per domain capability. A service coordinates work, is testable in isolation, and **never renders UI or imports React**. Services depend on adapters, not directly on `fs`/`electron` (Goal 10).
- **`main/services/adapters/`** — Infrastructure adapters. Each platform boundary (filesystem, clipboard, database, AI) is expressed as an interface with an Electron/Node implementation. Services depend on the interface.
- **`main/ipc/`** — The IPC handler surface. One file per feature area exposing a `register*IPC()` function. `ipc/index.ts` is the single registration entry point. Handlers validate input with shared schemas, then delegate to a service. Handlers contain **no business logic**.

**`src/preload/`** — Security seam. Exposes a curated, typed `electronAPI` via `contextBridge`. The renderer never accesses `ipcRenderer` directly.

**`src/renderer/`** — Presentation only (Goal 9). Components display state and invoke commands; they never call Electron/filesystem APIs or contain domain logic.

- **`renderer/src/features/`** — One folder per business capability. A feature owns its UI, local state, commands, IPC-client calls, feature-specific utilities, and tests (Goal 1).
- **`renderer/src/components/`** — Shared, feature-agnostic presentational components reused across features.
- **`renderer/src/hooks/`** — Shared React hooks with no feature ownership.
- **`renderer/src/commands/`** — The command registry and per-feature command registrations (Goal 11).

**`src/shared/`** — Contracts and pure logic importable by both processes. **Must not import** Electron, React, Browser APIs, or Node APIs.

- **`shared/models/`** — Domain models (business concepts). Zero runtime dependencies. Defined in [domain-models.md](./domain-models.md).
- **`shared/schemas/`** — Zod validation schemas for IPC request/response/error contracts.
- **`shared/types/`** — Non-domain TypeScript types shared across layers (e.g., DTOs, AST helper types).
- **`shared/contracts/`** — The typed IPC channel registry and event-bus event definitions (populated in Phases 1.4/1.5).
- **`shared/utils/`** — Pure, side-effect-free utility functions.

---

## 3. Ownership Rules

Ownership defines *which layer owns each concern* and *what each layer may import*. These are the boundary rules that Phase 1.5 will enforce.

### 3.1 Ownership Matrix

| Concern | Owner | Notes |
| --- | --- | --- |
| Filesystem / vault I/O | **Main** (`services` via `adapters`) | Renderer never touches the filesystem |
| Database / index storage | **Main** (`IndexService`, adapters) | |
| AI / vector / OCR / dictation | **Main** (`SearchService`, `DictationService`) | Bundled models live in `resources/` |
| PDF loading & rendering | **Main** (`PdfService`) | Renderer displays rendered output |
| IPC handler registration | **Main** (`ipc/`) | One `register*IPC()` per feature |
| IPC bridge exposure | **Preload** (`preload/index.ts`) | Only curated methods exposed |
| UI rendering & interaction | **Renderer** (`features/`, `components/`) | Presentation only |
| Local UI state | **Renderer** | Scroll, focus, animation, transient view state |
| Commands (user actions) | **Renderer** (`commands/`) | Delegate to services via IPC |
| Domain models | **Shared** (`models/`) | Pure, dependency-free |
| Schemas / validation | **Shared** (`schemas/`) | Single source of truth for both processes |
| Cross-layer types & contracts | **Shared** (`types/`, `contracts/`) | |

### 3.2 Service Ownership (Main)

- Each service owns exactly one business capability and all logic for it.
- A service **may import**: `shared/models`, `shared/types`, `shared/schemas`, `shared/utils`, other services (sparingly, via constructor injection), and its infrastructure adapters.
- A service **may not import**: React, any `renderer/*` module, or Electron/Node platform APIs directly (those go through adapters).
- Background coordination between services uses the typed event bus (Phase 1.5), not ad-hoc cross-imports.

### 3.3 Renderer Ownership

- The renderer owns presentation and local UI state only (Goal 9).
- A feature folder owns its own UI, hooks, commands, and IPC-client calls.
- The renderer **may import**: `shared/models`, `shared/types`, `shared/schemas` (for client-side validation), `shared/contracts`, `shared/utils`, and the preload-exposed `electronAPI`.
- The renderer **may not import**: any `main/*` module, Electron APIs, `ipcRenderer` directly, or `fs`/Node APIs.
- Components **may not** contain domain logic, perform filesystem operations, or manage persistent application state.

### 3.4 Shared Ownership

- Shared owns the contracts that both processes agree on: models, schemas, types, IPC channel/event definitions, and pure utilities.
- Shared **may import**: only other `shared/*` modules and dependency-free third-party libraries (e.g., `zod`).
- Shared **may not import**: anything from `main/*` or `renderer/*`, Electron, React, Browser, or Node APIs.
- Domain models (`shared/models`) are the innermost ring and **import nothing** except sibling models and pure type-only libraries.

### 3.5 IPC Ownership

- IPC is treated as a **public API** (Goal 4). Every channel defines: a channel identifier, request schema, response schema, and error schema — all in `shared/schemas` / `shared/contracts`.
- String-literal channel names are **prohibited outside the IPC contract layer** (Goal 4).
- **Main `ipc/`** owns handler registration and delegation to services.
- **Preload** owns the curated bridge exposure.
- **Renderer** consumes IPC only through the preload `electronAPI` typed surface.

### 3.6 Import Rules Summary

```
shared/models   ← imports nothing (innermost)
shared/*        ← may import shared/* only
main/services   ← may import shared/*, adapters, sibling services
main/ipc        ← may import shared/*, main/services
preload         ← may import shared/contracts, main IPC channel ids
renderer/*      ← may import shared/*, preload electronAPI
```

Cross-layer imports are prohibited except through these approved contracts (Goal 6, "Layer Separation").

---

## 4. Migration Order

The migration proceeds strictly through Phase 1 subphases. Each step must leave the app building and behavior-identical (Product Principle 7 — Incremental Change; Gate A). The order below is the exact sequence Phase 1 will follow.

### Step 0 — Prerequisite baseline
- **What:** Green build baseline from Phase 0.
- **Why first:** A verifiable starting point is required to detect regressions.
- **Dependencies:** Phase -1 audit, Phase 0 stabilization.
- **Risk:** None (verification only).
- **Rollback:** N/A.

### Step 1 — Design (this phase, 1.1)
- **What:** Folder layout, domain models, ownership rules, migration order (this document set).
- **Why first:** Establishes the blueprint every later step targets; zero implementation risk.
- **Dependencies:** Step 0.
- **Risk:** None — documentation only.
- **Rollback:** Discard docs.

### Step 2 — Create `shared/models` scaffolding (part of 1.1 output → placed in 1.2)
- **What:** Add domain model type definitions in `src/shared/models/` (Note, Vault, Workspace, Tag, GraphNode, Attachment).
- **Why here:** Domain models are the innermost dependency ring; defining them first lets every later step import stable types. They add new files without moving anything, so risk is minimal.
- **Dependencies:** Step 1 (definitions).
- **Risk:** Low — additive only; possible naming overlap with existing `shared/types.ts` (mitigate by not deleting existing types yet).
- **Rollback:** Delete the new model files.

### Step 3 — Feature folder migration (Subphase 1.2)
- **What:** Create `main/services/`, `main/ipc/`, and `renderer/src/features/`; move existing files into feature-oriented folders; update imports.
- **Why here:** Structure must exist before logic is extracted into it. Moves are mechanical and verifiable by build.
- **Dependencies:** Steps 1–2.
- **Risk:** Medium — import breakage during moves; hidden coupling surfaced. Mitigate by moving one feature at a time and running the build after each move.
- **Rollback:** Revert file moves and import edits in the smallest set (per feature).

### Step 4 — Service layer extraction (Subphase 1.3)
- **What:** Extract business logic embedded in `main/index.ts`, `ipc.ts`, and UI into focused services under `main/services/`.
- **Why here:** Requires the folders from Step 3. Must precede IPC contract work so handlers can become thin delegators.
- **Dependencies:** Step 3.
- **Risk:** Medium-high — over-extraction of abstractions; accidental behavior change. Mitigate: no behavior changes, extract as thin wrappers first, keep public signatures stable.
- **Rollback:** Inline extracted service back into its origin file.

### Step 5 — Shared contracts + typed IPC framework (Subphase 1.4)
- **What:** Centralize schemas/types in `shared/schemas` & `shared/types`; build the typed IPC channel registry with request/response/error contracts.
- **Why here:** Services (Step 4) provide the stable surface the contracts describe; contracts must exist before layer enforcement.
- **Dependencies:** Steps 3–4.
- **Risk:** Medium — contract drift from current channels; mitigate by mapping every existing channel 1:1 with no handler changes.
- **Rollback:** Revert to existing `channels.ts` / `schemas.ts` usage.

### Step 6 — Event bus + layer enforcement (Subphase 1.5)
- **What:** Introduce the typed event bus for background workflows; enforce boundary rules; remove violating cross-layer imports.
- **Why here:** Enforcement is only meaningful once structure, services, and contracts are in place.
- **Dependencies:** Steps 3–5.
- **Risk:** Medium — removing a "legitimate" cross-layer import could break a workflow; mitigate by classifying each import before removal.
- **Rollback:** Restore the specific removed import; disable the event bus subscriber.

### Step 7 — Import cleanup, verification, ADRs (Subphase 1.6)
- **What:** Normalize import aliases; produce architecture validation report; write ADRs.
- **Why last:** Cleanup and documentation follow a stable structure.
- **Dependencies:** Steps 3–6.
- **Risk:** Low — alias changes can break resolution; mitigate with a full build + launch check.
- **Rollback:** Revert alias/config changes.

### 4.1 General Rollback Considerations
- Every step is a small, independently revertible commit (Principle 7).
- The universal rollback is: revert file moves, alias changes, and structural edits in the smallest possible set that restores the green build and runtime behavior.
- No step deletes an old construct until its replacement is verified, so any step can be undone without data or behavior loss.

---

## 5. Success Criteria (Phase 1.1)

- [x] Folder structure matches **Architecture Goal 7**.
- [x] Domain models are fully defined — see [domain-models.md](./domain-models.md).
- [x] Ownership rules are documented (Section 3).
- [x] Migration order is documented (Section 4).

This phase is **design only**. No files were moved, no imports changed, and no behavior was analyzed or altered.
