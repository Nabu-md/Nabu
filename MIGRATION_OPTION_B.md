# Migration Option B: Pure Rust Architecture

## Overview
This document outlines the pivot from a TypeScript-heavy architecture to a 100% Pure Rust Architecture using Tauri v2 and Leptos WASM.

## Architectural Blueprint
- **Frontend Layer**: Leptos (Rust WASM) with Tailwind CSS for styling.
- **Core Engine**: Native Rust workspace crates (`crates/nabu-core/`) handling:
    - Vault parsing (AST-based, `pulldown-cmark`)
    - File watching (`notify`)
    - Vector search (`tantivy`)
    - Graph calculations (`petgraph`)
- **App Block Handling**: Dynamic HTML/JS content is rendered within sandboxed `<iframe>` elements using `srcdoc` to isolate the host application from unprivileged code execution.

## Directory Structure
```text
/
├── Cargo.toml                # Workspace definition
├── crates/
│   ├── nabu-core/            # Vault logic, parser, indexing
│   └── nabu-ui/              # Leptos WASM Frontend
├── src-tauri/                # Tauri v2 Desktop Shell & Native Hooks
└── ... (Other resources)
```

## Roadmap

| Phase | Description | Goal |
| :--- | :--- | :--- |
| **0** | **Bootstrap** | Wipe JS infrastructure, initialize Cargo workspace, scaffold `nabu-ui` and `nabu-core`. |
| **1** | **Vault State** | Port `vault-registry`, `view-state`, and `FileTree` to Rust/Leptos signals. |
| **2** | **Markdown** | Port `pulldown-cmark` parser and `NoteView` to render natively. |
| **3** | **Graph View** | Migrate `d3-force` to `petgraph` + Canvas-based rendering in `nabu-ui`. |
| **4** | **Sandboxing** | Implement dynamic `<iframe srcdoc>` for secure App Block injection. |
