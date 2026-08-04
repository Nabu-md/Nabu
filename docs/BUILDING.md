# Building Nabu from Source

This guide covers installing the toolchain, building for development, and
producing production release artifacts. A clean checkout can be built to a
release-ready desktop bundle by following these steps — the whole pipeline is
wrapped in **`scripts/build.sh`**.

---

## 1. Prerequisites

| Tool | Version | Purpose | Install |
|---|---|---|---|
| Rust toolchain | `1.97.1` (pinned, MSRV) | Compiles backend (`src-tauri`), core (`nabu-core`), and the Leptos frontend to WASM | [rustup](https://rustup.rs) or `mise install` |
| `wasm32-unknown-unknown` target | — | Frontend compilation target | `rustup target add wasm32-unknown-unknown` |
| macOS targets | `x86_64-apple-darwin`, `aarch64-apple-darwin` | Intel + Apple Silicon (universal builds) | `rustup target add x86_64-apple-darwin aarch64-apple-darwin` |
| Node.js + npm | Node ≥ 20 | Build-time Tailwind CSS pipeline | [nodejs.org](https://nodejs.org) or `mise install` |
| npm dependencies | — | Tailwind CSS | `npm install` |
| [Trunk](https://trunkrs.dev) | ≥ 0.21 | Bundles the WASM frontend | `cargo install trunk` or `mise install` |
| Tauri CLI | v2 (matches `tauri = "2"` in `src-tauri/Cargo.toml`) | Packaging / `cargo tauri build` | `cargo install tauri-cli --version ^2` or `mise install` |
| macOS command-line tools | — | `lipo` / `iconutil` (universal + DMG packaging) | `xcode-select --install` |

> **Tip:** the repo ships a [`mise.toml`](../mise.toml) pinning the Rust and
> Node toolchains plus `trunk`. If you use [mise](https://mise.jdx.dev), a
> single `mise install` provisions everything except the npm packages.

### Preflight check

Run this after any environment change — it verifies every tool, target, and
the npm install, and prints an actionable fix for anything missing:

```bash
scripts/check-env.sh
```

---

## 2. Development (hot-reload)

```bash
npm install              # once
cargo tauri dev          # builds the WASM frontend and launches the app
```

`cargo tauri dev` runs `src-tauri/scripts/run-trunk.sh` first, which generates
the Tailwind stylesheet, serves the frontend on `localhost:8080`, and watches
for changes. The desktop window is wired to that dev server.

---

## 3. Release build

The single documented command is:

```bash
scripts/build.sh
```

This script:

1. **Validates prerequisites** — fails fast with a clear message and the exact
   install command if anything is missing (full diagnostic: `scripts/check-env.sh`).
2. **Detects the host platform and architecture** — picks the correct Rust
   target triple automatically.
3. **Builds the frontend** — Tailwind CSS (`npm run css:build`) then
   `trunk build --release` into `dist/`.
4. **Builds desktop bundles** — `cargo tauri build` for the native target.
5. **Reports artifacts** — prints the bundle directory and produced files.

### Options

| Command | Result |
|---|---|
| `scripts/build.sh` | Native release bundle for this machine |
| `scripts/build.sh --universal` | macOS **universal** fat binary (Intel + Apple Silicon) + DMG |
| `scripts/build.sh --bundles app` | App bundle only, no installers |
| `scripts/build.sh --bundles app,dmg` | Specific bundle targets (any Tauri bundle type) |
| `scripts/build.sh --help` | Usage summary |

> Legacy: `scripts/build-release.sh` still exists as a deprecated wrapper for
> `scripts/build.sh --universal`.

---

## 4. Expected outputs

| Platform | Artifacts | Location |
|---|---|---|
| macOS (native) | `Nabu.app` (+ `.dmg` if targeted) | `src-tauri/target/<triple>/release/bundle/` |
| macOS (universal) | `Nabu.app`, `Nabu.dmg` | `src-tauri/target/universal-apple-darwin/release/bundle/` |
| Linux | `nabu.deb` / `nabu.rpm` / AppImage | `src-tauri/target/<triple>/release/bundle/` |
| Windows | `Nabu.exe`, `Nabu.msi` / NSIS installer | `src-tauri\target\<triple>\release\bundle\` |

The exact installer set follows `bundle.targets` in
[`src-tauri/tauri.conf.json`](../src-tauri/tauri.conf.json) (`"all"` by
default) or the `--bundles` override.

---

## 5. Related pipelines

| Task | Command |
|---|---|
| Regenerate all app icons from the master | `scripts/gen-icons.sh` (uses `resources/icon-master.png`) |
| Workspace type check | `cargo check --workspace` (mise: `mise run check`) |
| Tests | `cargo nextest run --workspace` (mise: `mise run test`) |
| Dependency audit | `cargo deny check` (mise: `mise run audit`) |

---

## 6. Troubleshooting

| Symptom | Fix |
|---|---|
| `trunk: command not found` | `cargo install trunk`, or `mise install` |
| `tauri-cli not found` | `cargo install tauri-cli --version ^2` |
| `error[E0463]: can't find crate for core` (wasm) | `rustup target add wasm32-unknown-unknown` |
| `error: linking with \`lipo\` failed` | `xcode-select --install` (macOS) |
| `npm: not found` | Install Node.js ≥ 20, or `mise install` |
| Build fails after a Rust toolchain bump | Reinstall the pinned toolchain (`mise install` or `rustup toolchain install 1.97.1`) and run `scripts/check-env.sh` |
