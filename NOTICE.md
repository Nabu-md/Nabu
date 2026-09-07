# Third-party notices

Nabu is licensed under AGPL-3.0-or-later. This file lists bundled or integrated
third-party projects and their licenses. Each entry links to the upstream
project, which is the authoritative source for its license text.

## Integrated services (separate process — no code linking)

| Project | License | Integration |
|---|---|---|
| [FluidVoice](https://github.com/altic-dev/FluidVoice) | GPL-3.0 | Optional on-device macOS dictation backend. Nabu communicates with the standalone FluidVoice app via Apple Events/clipboard only; no FluidVoice code is bundled, linked, or distributed with Nabu. |

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
