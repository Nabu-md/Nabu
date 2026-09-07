# Nabu

**The local-first, privacy-first knowledge base for thinkers.**

Nabu is a Markdown-native desktop app for thinkers who demand speed, sovereignty, and polish. Write in clean Markdown. Think with interactive canvases, live backlinks, and a relationship graph. Everything runs **on your machine** — no cloud, no telemetry unless you explicitly opt in.

Built with a **React 19 + TypeScript frontend** and a **Rust backend** powered by Tauri v2. One codebase, native performance, zero Electron.

---

## Quick Start

Download a prebuilt DMG from the [GitHub Releases](https://github.com/Nabu/Nabu/releases) page:

| Platform | Architecture | Build |
|----------|--------------|-------|
| macOS 13+ | Apple Silicon (arm64) | `Nabu-<version>-aarch64.dmg` |
| macOS 13+ | Intel (x86_64) | `Nabu-<version>-x86_64.dmg` |
| Windows 10+ | x86_64 | `Nabu-<version>-x64.msi` |
| Linux | x86_64 | `Nabu-<version>-x86_64.AppImage` |

---

## Why Nabu

| | Nabu |
|---|---|
| **Privacy** | All processing is local. Your notes never leave your machine unless you tell them to. |
| **Speed** | Native performance via Rust + Tauri. Startup in ~400ms, search in <10ms. |
| **Portability** | Plain Markdown on disk. No proprietary formats. Open your vault in any editor. |
| **Extensibility** | AI agent platform with multiple CLI integrations, MCP server, mini-apps, and plugin foundation. |
| **Resilience** | Automatic crash recovery, undo/redo, Git version control, and snapshot history. |

---

## Core Features

### Knowledge Management
- **Setup wizard** — first-launch flow to create or open a vault with native folder pickers
- **Recursive file tree** — reactive navigation with context menus, keyboard shortcuts, and command palette
- **Markdown editor** — dual raw/rich modes with live preview, task-checkboxes, tables, math (KaTeX), Mermaid diagrams, and wiki-links (`[[Note]]`)
- **Tag parsing** — real-time extraction from frontmatter with tag-based filtering
- **Full-text search** — in-memory index with relevance ranking and backlink discovery
- **Relationship graph** — interactive canvas visualization of your vault's link structure
- **Type system** — frontmatter-driven types with icons, colors, and property schemas
- **Saved views** — filtered/sorted virtual folders defined by query rules
- **Theme engine** — reactive dark/light/system modes persisted to settings

### Capture & Ingestion
- **Clipboard capture** — automatically ingest from system clipboard
- **Screenshot ingestion** — capture and embed images directly
- **File drop** — drag-and-drop files into the editor
- **Dictation pill** — floating scratchpad for voice input via FluidVoice/Whisper
- **OCR** — extract text from images and PDFs via macOS Vision
- **Document conversion** — import PDF, DOCX, PPTX, XLSX, ODT, EPUB, and more via AnyDoc

### AI & Agents
- **Multi-agent chat** — Claude, Codex, Copilot, Pi, OpenCode, Hermes, and Kiro integrations
- **Deep research** — multi-step research orchestrator with streaming output
- **MCP server** — expose vault capabilities to external AI clients
- **AI workspace** — side-by-side conversation surface with context-aware prompting
- **Grammar checking** — offline English grammar checking via Harper

### Version Control & Recovery
- **Git integration** — commit, pull, push, branch, conflict resolution, and remote management
- **Snapshot history** — browse file history with diff view
- **Session recovery** — workspace state persisted and restored across launches
- **Crash detection** — `.running` marker detects unclean shutdowns and offers recovery
- **Undo/Redo** — full history stack with per-operation reversibility

### Native Integrations
- **macOS Vision OCR** — automatic text extraction from images
- **PDF annotation** — dedicated viewer with highlight-to-note conversion
- **Whisper.cpp dictation** — local speech-to-text with configurable model sizes
- **IronCalc sheets** — embedded spreadsheet with formula support
- **tldraw whiteboards** — interactive canvas blocks inside notes
- **Auto-updater** — built-in Tauri updater with signature verification

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    src/ (React 19 + TypeScript)         │
│  ── UI framework ───── React 19, Tailwind CSS v4        │
│  ── Components ─────── shadcn/ui, BlockNote, tldraw     │
│  ── State ──────────── React hooks, Context, Signals    │
│  ── Build ──────────── Vite 7                          │
└────────┬─────────────────────────────────────────────────┘
          │ Tauri IPC (60+ invoke handlers)
┌────────┴─────────────────────────────────────────────────┐
│                 src-tauri/ (Rust + Tauri v2)            │
│  ── Backend ──────── Rust 1.77+, Tokio, gray_matter    │
│  ── Vault layer ───── file I/O, frontmatter, search     │
│  ── Git layer ─────── libgit2 via git2 CLI abstraction  │
│  ── AI layer ──────── CLI agent runtime, MCP bridge     │
│  ── Capture ───────── OCR, dictation, document convert  │
│  ── Platform ──────── updater, deep-link, single-instance│
└─────────────────────────────────────────────────────────┘
```

### Data Flow

```
Filesystem (.md + frontmatter)
    │
    ▼
Vault cache (~/.laputa/cache/) — fast startup index
    │
    ▼
React state (VaultEntry[]) — in-memory session graph
    │
    ▼
UI surfaces: NoteList, Editor, Graph, Inspector, etc.

Writes flow back through Tauri IPC → Rust → filesystem
```

---

## Building from Source

### Prerequisites
- Node.js 20+ and pnpm
- Rust 1.77.2+ (stable)
- System dependencies: `libwebkit2gtk-4.1-dev`, `libssl-dev`, `pkg-config` (Linux only)
- git CLI (required for Git integration)

### Development

```bash
# Install dependencies
pnpm install

# Run frontend in browser (mock data, no Rust needed)
pnpm dev
# → http://localhost:5173

# Run full Tauri desktop app
pnpm tauri dev
```

### Production Build

```bash
pnpm tauri build
```

Output: `src-tauri/target/release/bundle/` — signed DMG (macOS), MSI (Windows), AppImage (Linux).

---

## Documentation

| Resource | Description |
|----------|-------------|
| `docs/ARCHITECTURE.md` | Design principles, data flow, vault model |
| `docs/ABSTRACTIONS.md` | Domain models, conventions, editor serialization |
| `docs/GETTING-STARTED.md` | Dev setup, testing, sidecar validation |
| `AGENTS.md` | Development process, commit rules, QA checklist |
| `release-notes/` | Per-version changelogs |

---

## Community

- **Issues:** [GitHub Issues](https://github.com/Nabu/Nabu/issues)
- **Discussions:** [GitHub Discussions](https://github.com/Nabu/Nabu/discussions)
- **Contributing:** See `CONTRIBUTING.md`

---

## Built with open source

Nabu stands on the shoulders of these projects. Full transparency, full credit.

| Project | License | What it does |
|---|---|---|
| [Tauri](https://tauri.app) | Apache-2.0 | Desktop app framework |
| [React](https://react.dev) | MIT | UI library |
| [BlockNote](https://blocknotejs.org) | MPL-2.0 | Rich text editor |
| [tldraw](https://tldraw.com) | AGPL-3.0 | Whiteboard canvas |
| [Tailwind CSS](https://tailwindcss.com) | MIT | Styling engine |
| [shadcn/ui](https://ui.shadcn.com) | MIT | UI component patterns |
| [IronCalc](https://ironcalc.com) | MIT | Spreadsheet engine |
| [Harper](https://github.com/automattic/harper) | Apache-2.0 | Offline grammar checking |
| [AnyDoc](https://crates.io/crates/anydoc) | MIT | Document-to-Markdown conversion |
| [FluidVoice](https://github.com/altic-dev/FluidVoice) | GPLv3 | On-device dictation |
| [vis-network](https://visjs.org) | Apache-2.0 | Graph view visualization |
| [Mermaid](https://mermaid.js.org) | MIT | Diagram rendering |
| [KaTeX](https://katex.org) | MIT | Math rendering |
| [CodeMirror](https://codemirror.net) | MIT | Raw Markdown editor |
| [PostHog](https://posthog.com) | MIT | Product analytics |
| [Sentry](https://sentry.io) | BSD-3 | Crash reporting |

Additional dependency licenses are enumerated in [NOTICE.md](NOTICE.md).

## Security

If you believe you have found a security issue, please report it privately as described in [SECURITY.md](./SECURITY.md).

## License

Nabu is licensed under AGPL-3.0-or-later. The Nabu name and logo remain covered by the project's trademark policy.

---

*The name "Nabu" is inspired by the ancient Mesopotamian deity of writing and knowledge. This project is not affiliated with any commercial entity.*
