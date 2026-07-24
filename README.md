# Nabu 🚀

**The lightning-fast, local-first knowledge base built in pure Rust.**

Nabu is a Markdown-native, open-source (AGPL-3.0) desktop application that bridges the gap between clean Markdown text and interactive web software. By leveraging Tauri v2 and Leptos WASM, Nabu delivers a high-performance desktop experience while keeping your data 100% portable, plain-text, and token-efficient.

---

## 💡 The Core Differentiator: HTML-Native App Blocks

Nabu treats Markdown as a launchpad for **lightweight, interactive software**. Because Nabu renders via a secure, sandboxed Leptos WASM layer, you can seamlessly embed full HTML, CSS, and JavaScript directly inside your plain-text notes.

### What this unlocks for your workflow:
- **AI-Generated Cockpits:** Instantly run interactive dashboards, simulators, or testing scripts right in your notes.
- **Modular Custom Tools:** Turn a standard note into a personalized Kanban board, tracker, or visualizer using standard web technologies.
- **Zero "Token Tax":** Since Nabu stores standard Markdown on disk, local AI agents and LLMs read your files with 1.0x token efficiency — no bloated proprietary widget code.

---

## 🔥 Features

### Foundation & File System
1. **Setup Wizard:** Secure vault directory initialization and path validation.
2. **Multi-vault Support:** Concurrent vault management with session isolation.
3. **Real-time Watcher:** Native `notify`-backed file system tracking with hot-reloading.
4. **Template Management:** `Tera`-powered template interpolation for note creation.
5. **Document Export Engine:** Markdown-to-HTML/PDF export pipelines.
6. **FileTree Navigation:** Recursive, reactive Leptos-based file navigation.

### Editor & Knowledge Base
7. **Note Editor:** Live markdown preview editor with interactive task checkbox support.
8. **Tag Parsing:** Real-time tag extraction and indexing.
9. **Full-Text Search:** High-performance search powered by `tantivy`.
10. **Backlink Resolution:** Deep graph traversal for interconnected note discovery.
11. **Dynamic Theme Engine:** Reactive dark/light mode switching persisted to disk.

### Graph & Sandbox
12. **Graph Engine:** Native `petgraph` + Canvas visualization.
13. **Tag/Blocks Graph View:** Filtered graph traversal modes.
14. **App Block Sandbox:** Secure `iframe` isolation for interactive widgets.

### Hardware Superpowers (Phase 2)
15. **macOS Native Features:** Fully functional backend FFI for macOS Vision OCR, PDF Annotation, and Whisper.cpp-based Audio Dictation.

---

## 🚀 Getting Started

### Prerequisites
- Rust (latest stable)
- Tauri CLI (`cargo install tauri-cli`)
- CMake (for Whisper.cpp native compilation)

### Build & Run
```bash
git clone https://github.com/Nabu-md/Nabu.git
cd Nabu
cargo tauri dev
```

---

## 🛠 Architecture

```text
crates/
├── nabu-core/       # Core Engine: Vault logic, AST, Indexing, Graph, FFI
├── nabu-ui/         # Leptos WASM UI components
src-tauri/           # Tauri v2 Desktop Shell & IPC Commands
```

## License
Copyright © 2024 Nabu Labs. Released under the **GNU Affero General Public License v3.0**.
