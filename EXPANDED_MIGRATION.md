# Nabu Migration Blueprint: Implementation-Grade Roadmap (Refined V2)

This document provides a granular, implementation-grade roadmap for migrating Nabu from Electron 39 to Tauri v2.

---

## Migration Readiness Checklist (Phase-0 Gate)
- [ ] `v1-electron-legacy` branch created.
- [ ] Complete dependency inventory of `package.json` produced.
- [ ] IPC inventory created (`ipc.md` updated).
- [ ] Performance baseline recorded (Electron memory/startup).
- [ ] Test coverage check completed.

---

## 1. Roadmap Details

### Phase 0: Safety & Environment
**0.1 Setup Archival Branch**
*   **Objective:** Secure current working state.
*   **Complexity:** Small.

**0.2 Initialize Tauri Workspace**
*   **Objective:** Set up Tauri workspace.
*   **Complexity:** Small.

**0.3 Security & CSP Configuration**
*   **Objective:** Define secure app permissions.
*   **Complexity:** Small.

**Verification Gate 0**
*   **Build:** `cargo check`, `npm run typecheck` (if configured for tauri).
*   **Runtime:** Application initializes and window displays.

---

### Phase 1: Core IPC Bridge & Native Vault Services

**1.1 Schema Mapping (Zod -> Serde)**
*   **Objective:** Define shared data structures in Rust.
*   **Inputs:** `src/shared/schemas.ts`.
*   **Outputs:** `src-tauri/src/models.rs`.
*   **Acceptance Criteria:** `cargo check` passes; Zod definitions match Rust Serde structs.
*   **Complexity:** Small.

**1.2 Native Filesystem Watcher (Chokidar -> Notify)**
*   **Objective:** Implement native file watcher.
*   **Inputs:** `src/main/watcher.ts`.
*   **Outputs:** `src-tauri/src/watcher.rs`.
*   **Acceptance Criteria:** `cargo test` confirms watcher emits events on file modification; `notify` correctly handles vault directory paths.
*   **Complexity:** Medium.

**1.3.1 Vault Settings Persistence**
*   **Objective:** Port JSON settings store to Tauri.
*   **Inputs:** `src/main/settings.ts`.
*   **Outputs:** `src-tauri/src/settings.rs`.
*   **Acceptance Criteria:** Settings persist across app restart; `cargo test` confirms CRUD operations work.
*   **Complexity:** Small.

**1.3.2 Vault Data Structure & CRUD**
*   **Objective:** Define Vault memory structure and basic operations.
*   **Inputs:** `src/main/vault-service.ts`.
*   **Outputs:** `src-tauri/src/vault.rs`.
*   **Acceptance Criteria:** `cargo test` verifies file reading/writing is successful; core data structures are memory-safe.
*   **Complexity:** Medium.

**1.3.3 IPC Bridge for Vault Operations**
*   **Objective:** Expose vault commands via Tauri.
*   **Inputs:** `src-tauri/src/vault.rs`.
*   **Outputs:** `src-tauri/src/commands.rs`.
*   **Acceptance Criteria:** Tauri commands (`invoke`) correctly handle serialization; unit tests verify IPC return types match TypeScript interfaces.
*   **Complexity:** Medium.

**1.4.1 Vision/OCR Native Implementation**
*   **Objective:** Port OCR processing.
*   **Inputs:** `src/main/ocr.ts`.
*   **Outputs:** `src-tauri/src/native/ocr.rs`.
*   **Acceptance Criteria:** Native FFI bindings compile; test suite processes sample image successfully via `invoke`.
*   **Complexity:** Medium.

**1.4.2 Whisper/Dictation Native Implementation**
*   **Objective:** Port dictation logic.
*   **Inputs:** `src/main/dictation.ts`.
*   **Outputs:** `src-tauri/src/native/dictation.rs`.
*   **Acceptance Criteria:** Test suite processes sample audio successfully via `invoke`.
*   **Complexity:** Medium.

**Verification Gate 1**
*   **Build:** `cargo test` passes (Watcher, Vault, FFI).
*   **Runtime:** Settings persist; OCR/Dictation bridge commands execute via Tauri IPC.
*   **Performance:** Memory usage baseline validated.

---

### Phase 2: App Block Integration

**2.1 Context Bridge Rewrite**
*   **Objective:** Replace Electron preload with Tauri invoke.
*   **Scope:** `src/preload/index.ts`.
*   **Acceptance Criteria:** TypeScript typecheck (`npm run typecheck`) passes; frontend calls via `window.__TAURI__.invoke` resolve without errors.
*   **Complexity:** Medium.

**2.2 WebView Sandbox Verification**
*   **Objective:** Test App Block isolation in Webview.
*   **Scope:** `src/renderer/`.
*   **Acceptance Criteria:** App blocks load within `iframe` without accessing prohibited Chromium-specific APIs; unit tests confirm sandbox environment consistency.
*   **Complexity:** Medium.

**Verification Gate 2**
*   **Build:** `npm run typecheck` passes.
*   **Runtime:** App Blocks load successfully in the WebView; IPC calls between Renderer and Rust Backend succeed.

---

### Phase 3: Search, Graph & Markdown AST Engine

**3.1.1 Tantivy Indexing Service**
*   **Objective:** Implement native indexing.
*   **Scope:** `src-tauri/src/search/`.
*   **Acceptance Criteria:** Indexing triggers on vault load; `cargo test` verifies index build for 1000 notes in < 500ms.
*   **Complexity:** Large.

**3.1.2 Petgraph Graph Service**
*   **Objective:** Implement native graph builder.
*   **Scope:** `src-tauri/src/graph/`.
*   **Acceptance Criteria:** Graph nodes and edges correctly represent the vault; `cargo test` confirms graph connectivity matches expected state.
*   **Complexity:** Large.

**3.1.3 Search & Graph IPC Bridge**
*   **Objective:** Expose indexing and graph results via IPC.
*   **Scope:** `src-tauri/src/commands.rs`.
*   **Acceptance Criteria:** Search queries return results in < 10ms; graph data serializes correctly.
*   **Complexity:** Small.

**3.2.1 Pulldown-cmark Basic Parser**
*   **Objective:** Implement basic MD parser.
*   **Scope:** `src-tauri/src/markdown/`.
*   **Acceptance Criteria:** `cargo test` confirms valid MD parsing output matches expected structure.
*   **Complexity:** Medium.

**3.2.2 Wikilink/Task Block Visitor**
*   **Objective:** Implement custom MD features.
*   **Scope:** `src-tauri/src/markdown/`.
*   **Acceptance Criteria:** Wikilinks (`[[Link]]`) and task blocks are correctly parsed; unit tests confirm visitor fidelity.
*   **Complexity:** Medium.

**3.2.3 Roundtrip Validation Suite**
*   **Objective:** Verify MD parsing parity.
*   **Scope:** `tests/integration/roundtrip.test.ts`.
*   **Acceptance Criteria:** Rust-parsed output matches the existing JS-parsed MD test suite output exactly.
*   **Complexity:** Small.

**Verification Gate 3**
*   **Build:** `cargo test` passes (Search, Graph, MD Engine).
*   **Runtime:** Search index builds correctly; Graph structure is accurate; MD parser passes roundtrip suite.
*   **Performance:** Search query latency < 10ms; Startup remains fast.

---

### Phase 4: Cleanup & Packaging

**4.1 Dependency Removal**
*   **Objective:** Remove Electron-specific code/files.
*   **Scope:** `package.json`, `src/main/electron/`.
*   **Acceptance Criteria:** Application launches successfully with NO Electron-related dependencies in `node_modules`.
*   **Complexity:** Medium.

**4.2.1 Tauri Dev/Production Configuration**
*   **Objective:** Finalize Tauri settings.
*   **Scope:** `tauri.conf.json`.
*   **Acceptance Criteria:** Production build completes without errors.
*   **Complexity:** Medium.

**4.2.2 Signing & Bundling**
*   **Objective:** Produce distribution artifacts.
*   **Scope:** `src-tauri/`, `tauri.conf.json`.
*   **Acceptance Criteria:** Properly signed installers produced for macOS/Windows.
*   **Complexity:** Medium.

**Verification Gate 4**
*   **Build:** Final `cargo build --release` passes.
*   **Runtime:** Production app launches, opens vault, and completes search query without panics.
*   **Performance:** Final RAM/CPU benchmark analysis against Phase 0 baseline confirms memory reduction target met.
