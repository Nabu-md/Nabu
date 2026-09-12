# Third-party notices

Nabu is licensed under AGPL-3.0-or-later. This file lists bundled or integrated
third-party projects and their licenses. Each entry links to the upstream
project, which is the authoritative source for its license text.

## Integrated services (separate process — no code linking)

| Project | License | Integration |
|---|---|---|
| [FluidVoice](https://github.com/altic-dev/FluidVoice) | GPL-3.0 | Optional on-device macOS dictation backend. Nabu communicates with the standalone FluidVoice app via Apple Events/clipboard only; no FluidVoice code is bundled, linked, or distributed with Nabu. |
| [Tolaria](https://github.com/Nabu-md/tolaria) | AGPL-3.0 | App skeleton and vault tooling. Nabu's MCP server, vault lifecycle, capture pipeline, and Git integration code derive from the Tolaria project. Tolaria is a fork-friendly sibling project that shares Nabu's local-first Markdown vault architecture; ongoing upstream work can be tracked via the `tolarria` git remote and cherry-picked selectively. |
| [Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M) | Apache-2.0 | Optional on-device neural text-to-speech. Nabu drives the standalone `koko` CLI as an external process for speech synthesis and word-timing extraction; no Kokoro model weights or code are bundled, linked, or distributed with Nabu. |

## Bundled Rust crates (compiled into the application)

| Crate | License | Purpose |
|---|---|---|
| [anydoc](https://crates.io/crates/anydoc) | MIT | Document-to-Markdown conversion |
| [harper-core](https://crates.io/crates/harper-core) | Apache-2.0 | Offline English grammar checking |
| [tauri](https://tauri.app) | Apache-2.0 (with LGPL-2.1 and Apple sample-code exceptions as noted upstream) | Desktop application framework |
| [ironcalc_base](https://github.com/ironcalc/IronCalc) | MIT | Spreadsheet engine |
| [reqwest](https://crates.io/crates/reqwest) | Apache-2.0/MIT | HTTP client |
| [tokio](https://crates.io/crates/tokio) | MIT | Async runtime |
| [serde](https://crates.io/crates/serde) | Apache-2.0/MIT (dual) | Serialization |
| [notify](https://crates.io/crates/notify) | CC0-1.0/Apache-2.0/MIT (dual) | File watching |
| [uuid](https://crates.io/crates/uuid) | Apache-2.0/MIT | Identifier generation |
| [regex](https://crates.io/crates/regex) | Apache-2.0/MIT | Regular expressions |
| [chrono](https://crates.io/crates/chrono) | Apache-2.0/MIT | Date/time handling |
| [walkdir](https://crates.io/crates/walkdir) | Apache-2.0/MIT (dual) | Directory traversal |
| [gray_matter](https://crates.io/crates/gray_matter) | Apache-2.0/MIT | Frontmatter parsing |
| [base64](https://crates.io/crates/base64) | Apache-2.0/MIT | Encoding |
| [csv](https://crates.io/crates/csv) | Apache-2.0/MIT (dual) | CSV parsing |
| [sentry](https://crates.io/crates/sentry) | MIT (client) | Optional error reporting |
| [tempfile](https://crates.io/crates/tempfile) | Apache-2.0/MIT | Temporary files |
| [dirs](https://crates.io/crates/dirs) | Apache-2.0/MIT | Platform directories |
| [objc2](https://crates.io/crates/objc2) stack | Apache-2.0/MIT (dual) | macOS framework bindings |
| [png](https://crates.io/crates/png) | Apache-2.0/MIT (dual) | PNG image encoding |
| [duckdb](https://duckdb.org) | MIT | Embedded analytical SQL engine |
| [feed-rs](https://crates.io/crates/feed-rs) | MIT | RSS/Atom/JSON Feed parsing |
| [active-win-pos-rs](https://crates.io/crates/active-win-pos-rs) | MIT | Active window detection |
| [async-imap](https://crates.io/crates/async-imap) | Apache-2.0/MIT (dual) | Async IMAP email ingestion |
| [mail-parser](https://crates.io/crates/mail-parser) | Apache-2.0 | MIME message parsing |
| [html2text](https://crates.io/crates/html2text) | Apache-2.0/MIT (dual) | HTML to plain-text conversion |
| [toml](https://crates.io/crates/toml) | Apache-2.0/MIT (dual) | TOML frontmatter/config parsing |
| [fastembed](https://github.com/Anush008/fastembed-rs) | Apache-2.0 | ONNX-based local semantic search embeddings |

## Bundled frontend packages

| Package | License | Purpose |
|---|---|---|
| [react](https://react.dev) / [react-dom](https://react.dev) | MIT | UI runtime |
| [vis-network](https://visjs.github.io/vis-network/) / [vis-data](https://visjs.github.io/vis-data/) | Apache-2.0 | Graph view layout & rendering |
| [@phosphor-icons/react](https://phosphoricons.com) | MIT | Icon set |
| [@tauri-apps/api](https://tauri.app) | Apache-2.0/MIT (dual) | Tauri IPC bindings |
| Tailwind CSS | MIT | Utility CSS |
| shadcn/ui components (Radix-based) | MIT | Accessible UI primitives |
| BlockNote / ProseMirror | MPL-2.0 | Rich text editing |
| tldraw | See upstream license | Whiteboard notes |
| mermaid | MIT | Diagram rendering |

## Notes

- Licenses for transitive dependencies are resolved automatically by the
  package managers (`cargo`, `pnpm`) at build time; run
  `cargo license` / `pnpm licenses` for a complete generated inventory.
- The Nabu name and logo are covered by the project's trademark policy
  (see `trademarks.md`).
