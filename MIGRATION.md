# Nabu Migration Blueprint: Electron 39 to Tauri v2 + Rust

This document defines the architectural strategy and phased execution roadmap for migrating Nabu from the Electron framework to Tauri v2 with a Rust backend.

## 1. Executive Summary & Migration Goals

The objective is to modernize Nabu's core infrastructure to leverage Rust’s memory safety, concurrency, and performance, while preserving Nabu's signature feature: sandboxed HTML App Blocks.

### Primary Motivations:
*   **Performance:** Reduce idle memory footprint from ~300MB to ~40MB.
*   **Startup:** Achieve sub-300ms application startup times.
*   **Indexing/Watching:** Replace Node-based `chokidar` and JS-based AST processing with native Rust `notify` and `tantivy` for near-instant search and indexing.
*   **Compatibility:** Maintain full support for existing `.md` files and sandboxed App Block functionality.

### Architectural Target:
*   **Backend:** Rust (Tauri v2 Core + Tokio for async tasks).
*   **IPC:** Tauri Commands (`invoke`) with Serde serialization.
*   **Renderer:** React 19 + WebView2 (Windows) / WKWebView (macOS).

---

## 2. Technical Stack Mapping & Dependency Audit

| Feature | Legacy Electron (Node.js) | Tauri v2 + Rust Equivalent |
| :--- | :--- | :--- |
| **File Watching** | `chokidar` | `notify` (Rust Crate) |
| **IPC** | `ipcMain` / `ipcRenderer` | Tauri Commands (`invoke` / `listen`) |
| **Markdown AST** | `remark` / `unified` | `pulldown-cmark` / `markdown-rs` |
| **AI/OCR/Whisper**| Node-native (`Whisper.cpp`) | `whisper-rs` (Native FFI) |
| **Native Vision/OCR**| Node-native bindings | `macOS Vision` framework bindings |
| **Search Indexing** | In-memory `Map` / custom logic | `tantivy` |
| **Graph Logic** | `d3-force` / custom | `petgraph` |
| **Schema Validation** | `zod` | `serde` + `zod` (for frontend) |

---

## 3. Phased Execution Roadmap

### Phase 0: Safety & Environment Setup
- [ ] Subphase 0.1: Create `v1-electron-legacy` branch for archival.
- [ ] Subphase 0.2: Initialize `src-tauri` workspace:
  ```bash
  npm install @tauri-apps/cli
  cargo tauri init
  ```
- [ ] Subphase 0.3: Configure `tauri.conf.json` with secure CSP and capability definitions.

### Phase 1: Core IPC Bridge & Native Vault Services
*Goal: Port backend logic while keeping the Electron renderer mostly intact to verify backend integrity.*
- [ ] Subphase 1.1: Map `src/shared/schemas.ts` Zod definitions to Serde Rust structs.
- [ ] Subphase 1.2: Implement native filesystem watcher (`watcher.ts` ➡️ `notify`).
- [ ] Subphase 1.3: Port `vault-service.ts` and `settings.ts` (JSON store persistence) to Tauri Commands and Rust `dirs`/`tauri-plugin-store`.
- [ ] Subphase 1.4: Map native FFI integrations: `whisper-rs` for Fn-key dictation and macOS Vision framework for OCR processing.
- [ ] **Acceptance Criteria:** Vault opens, filesystem events correctly trigger backend updates in Rust; settings persist; OCR/Dictation bridge works.

### Phase 2: App Block & Webview Runtime Integration
*Goal: Bridge the gap between Tauri WebView and existing sandboxed App Blocks.*
- [ ] Subphase 2.1: Rewrite `src/preload/index.ts` context bridge to use `window.__TAURI__.invoke`.
- [ ] Subphase 2.2: Verify `iframe` sandbox behavior in `WKWebView`/`WebView2`.
- [ ] **Acceptance Criteria:** App Blocks load, execute JS, and communicate via `postMessage` successfully without Chromium-specific APIs.

### Phase 3: Search, Graph & Markdown AST Engine
*Goal: Replace the heavy lifting of remark/unified.*
- [ ] Subphase 3.1: Implement `tantivy` indexer and `petgraph` graph builder in Rust.
- [ ] Subphase 3.2: Implement custom Markdown visitor in `pulldown-cmark` for wiki-links (`[[Link]]`) and task blocks.
  > **Pragmatic Fallback:** If full AST parity in Rust delays Phase 3, keep JS `remark` in the React frontend for Note Rendering, and use `pulldown-cmark` in Rust exclusively for fast index/graph extraction.
- [ ] **Acceptance Criteria:** Search returns results <10ms; graph renders correctly; markdown AST parity is verified against `unit/` test suite.

### Phase 4: Cleanup, Polish & Packaging
- [ ] Subphase 4.1: Remove `electron`, `chokidar`, `electron-builder`, `electron-vite`.
- [ ] Subphase 4.2: Configure Tauri bundler for production signing (universal DMG, Linux).
- [ ] Subphase 4.3: Perform final RAM/CPU benchmark analysis.

---

## 4. Risks & Mitigations Matrix

| Risk | Impact | Mitigation Strategy |
| :--- | :--- | :--- |
| **Rendering Parity** | High | Use `debug` builds for WebView to monitor CSP/WebView behavior; keep fallback logic if necessary. |
| **Cache Incompatibility** | Medium | Implement versioned metadata in `.nabu/`; Rust-side migration scripts for index format updates. |
| **AST Visitor Parity** | High | Use `roundtrip` tests (`tests/integration/roundtrip.test.ts`) to ensure Rust-parsed MD matches original JS-parsed MD. |
