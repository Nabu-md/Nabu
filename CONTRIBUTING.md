# Contributing to Nabu

Thanks for your interest in contributing. Nabu is AGPL-3.0 open-source, and every contribution makes the project stronger.

## Quick Start

```bash
# Prerequisites: Rust toolchain (stable), Node.js + npm (for the Tailwind CSS pipeline)
git clone https://github.com/Nabu/Nabu.git
cd nabu
npm install
npm run css:build   # generate ./generated/tailwind.css from src/styles/app.css
cargo build         # build nabu-core and the Tauri backend
```

The desktop application is built with **Tauri v2 + Rust** (backend in `src-tauri/` and
`crates/nabu-core/`) and a **Dioxus** frontend (in `crates/nabu-ui/`). There is no
Electron/Node backend. To run the desktop app during development, launch it through
the Tauri workflow (e.g. `cargo tauri dev` from `src-tauri/`, or the project's
`src-tauri/scripts/*` dev/build hooks configured in `src-tauri/tauri.conf.json`).

## Branch Strategy

No direct pushes to `main`. All work happens on feature branches.

```bash
# Create a branch from main
git checkout -b feat/your-feature-name
# or
git checkout -b fix/your-bug-fix

# Push and open a pull request
git push -u origin feat/your-feature-name
```

Pull requests must be reviewed before merging. Only maintainers can merge to `main`.

## Before You Open a PR

Run these locally to make sure the checks pass:

```bash
cargo check --workspace   # Rust — the workspace must compile
cargo test --workspace    # Rust — unit/integration tests must pass
npm run css:build         # Tailwind CSS pipeline must produce output
```

## Code Standards

- **Rust** is the application and backend language. The UI is Dioxus (Rust).
- **Rust formatting** uses the standard toolchain — run `cargo fmt --check` before committing.
- **Tests are mandatory.** New `nabu-core` / `src-tauri` functionality must include
  Rust unit or integration tests. Bug fixes must include a regression test.

## Testing

```bash
# Run all Rust unit and integration tests
cargo test --workspace

# Check the standalone Dioxus UI crate compiles
cargo check --manifest-path crates/nabu-ui/Cargo.toml
```

The test suite uses Rust `cfg(test)` unit tests plus `#[tokio::test]` integration
tests in `crates/nabu-core`. If you're adding a new module, include tests for its
invariants.

## Architecture Overview

```
src-tauri/        # Tauri v2 backend: commands, IPC, settings, recovery, history
crates/nabu-core/ # Rust core: models, storage, processing, index/graph, EventBus
crates/nabu-ui/   # Dioxus frontend (CSR, cdylib for wasm-bindgen)
src/styles/       # Tailwind CSS source → generated/tailwind.css
```

- **Tauri/Rust** handles file I/O, markdown processing, indexing, IPC, and state.
- **nabu-core** owns the domain models, storage, search index, and event bus.
- **nabu-ui** renders the UI and communicates with the backend over Tauri IPC commands.

## Questions?

Open a [Discussions](https://github.com/Nabu/Nabu/discussions) thread — we're responsive and friendly.
