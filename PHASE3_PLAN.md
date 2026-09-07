# Phase 3: Feature Implementation Plan for TypeScript-Nabu

## Overview
Three features to build on the Tolia base:
1. **HTML Mini-Apps** — Sandboxed WebView windows for standalone HTML/JS apps
2. **Dictation Pill** — Floating dictation button with opacity loading, clipboard cache, and file drop-zone
3. **CLI AI Deep Research** — Multi-step web research + reasoning mode via Claude CLI

---

## Feature 1: HTML Mini-Apps

### Current State
- `html_block_protocol.rs` already exists (line 417 of `lib.rs`) — serves inline HTML blocks in editor via `nabu-html-block://` protocol
- `HtmlBlock.tsx` renders HTML blocks as iframes inline in notes
- No infrastructure for *standalone* mini-app windows

### Architecture
Mini-apps are standalone HTML/JS/CSS bundles that run in their own Tauri window with a restricted WebView. Unlike inline HTML blocks (which are part of a note), mini-apps are first-class windows that can persist, receive IPC, and optionally access vault data.

### Implementation Steps

#### 3.1.1: Backend — Mini-App Manager (`src-tauri/src/mini_apps.rs`)
Create a new Rust module that:
- Maintains a registry of installed mini-apps (JSON manifest in vault `.apps/` directory)
- Each mini-app manifest: `{ id, name, icon, entrypoint_url, width, height, resizable, allow_vault_access }`
- IPC commands:
  - `list_mini_apps() -> Vec<MiniApp>` — lists installed mini-apps
  - `get_mini_app_config(id) -> MiniApp` — returns config for a specific app
  - `open_mini_app_window(id, context: MiniAppContext) -> String` — spawns a new `WebviewWindow` with the app's URL, passes context via URL params + localStorage
  - `save_mini_app_config(config: MiniApp) -> Result<(), String>` — for development mode
  - `delete_mini_app(id) -> Result<(), String>`

Register IPC commands in `commands/mod.rs` and add to `app_invoke_handler!` in `lib.rs`.

#### 3.1.2: Backend — Mini-App Window Creation
Add a Tauri command `open_mini_app_window` that:
```rust
// Mirrors the pattern in vault_instance.rs for window launching
let label = format!("miniapp-{}", id);
let window = WebviewWindow::new(
    app_handle,
    &label,
    WebviewWindowOptions {
        url: format!("nabu-mini-app://{}", id),  // custom protocol
        width: config.width,
        height: config.height,
        resizable: config.resizable,
        decorations: true,
        ..Default::default()
    }
)?;
```
- Register a `nabu-mini-app://` URI protocol handler that serves the mini-app's HTML from the vault's `.apps/{id}/` directory
- Set `frame: false` + `decorations: false` for custom chrome if needed

#### 3.1.3: Frontend — Mini-App Launcher Hook
Create `src/hooks/useMiniApps.ts`:
- `listMiniApps()` — calls `list_mini_apps` IPC
- `openMiniApp(id, context)` — calls `open_mini_app_window` IPC
- `createMiniAppContextFromCurrentNote()` — packages current note path/title as context

#### 3.1.4: Frontend — Mini-App Launcher UI
Add to the Command Palette (`src/components/CommandPalette.tsx`) or a new dock item:
- "Mini Apps" section listing available apps
- Clicking an app opens it in a new window

#### 3.1.5: Frontend — Mini-App Window App Shell
Create `src/MiniAppWindowApp.tsx`:
- Route: matches `?window=mini-app&appId=...&context=...`
- Renders the mini-app as an `<iframe>` or `<webview>` tag pointing to `nabu-mini-app://{appId}/`
- Exposes window controls (close, toggle dev tools)
- Listens for IPC events from the mini-app (e.g., `mini-app-request-vault-data`)

### Sample Mini-App
Create `src-tauri/resources/mini-apps/hello-world/` with:
- `index.html` — simple reactive counter app (HTMX or vanilla JS)
- `manifest.json` — the mini-app config
This validates the full pipeline end-to-end.

---

## Feature 2: Dictation Pill

### Current State
- No dictation exists in the Tolia codebase
- HTML block protocol already provides sandboxed iframe infrastructure
- The AGENTS.md spec describes: opacity loading, clipboard cache panel, file drop-zone IPC, copy button

### Architecture
The dictation pill is a floating button that appears on note/editor surfaces. When clicked, it:
1. Opens a dictation recording session (via Web Speech API in the WebView)
2. Streams live audio transcription
3. Inserts text at cursor position or into a text area

### Implementation Steps

#### 3.2.1: Backend — Dictation IPC
Add to `src-tauri/src/commands/`:
- `start_dictation() -> Result<String, String>` — records audio (macOS speech framework)
- `stop_dictation() -> Result<(), String>` — stops recording
- `get_recent_clipboard_entries(count: usize) -> Vec<ClipboardEntry>` — returns recent clipboard cache
- `restore_clipboard_entry(id: String) -> Result<String, String>` — copies entry back to clipboard

Create a new module `dictation.rs` with the clipboard cache logic:
- On `copy_image_to_vault` (already exists), cache the clipboard entry
- Use macOS `NSPasteboard` to enumerate recent entries

#### 3.2.2: Frontend — Dictation Hook
Create `src/hooks/useDictation.ts`:
- `useSpeechRecognition()` — wraps Web Speech API, returns `{ text, isListening, start, stop }`
- `useClipboardCache()` — calls `get_recent_clipboard_entries` / `restore_clipboard_entry`
- `useFileDropZone()` — handles file drops, calls `capture_file_drop` (exists in Nabu but not in Tolia; add it)

#### 3.2.3: Frontend — Dictation Pill Component
Create `src/components/DictationPill.tsx`:
- Fixed-position floating button with opacity loading (via CSS transition)
- Click → opens a floating recording panel
- Panel shows: live transcription, clipboard cache entries (click to restore), drop zone
- Uses `navigator.mediaDevices.getUserMedia` for recording
- Calls `start_dictation`/`stop_dictation` IPC when running native

#### 3.2.4: Frontend — Integration
- Mount `DictationPill` in the main App.tsx or StatusBar
- Toggle via keyboard shortcut (`Cmd+Shift+D`)
- Respect `AppSettings` for position, opacity, enabled state

---

## Feature 3: CLI AI Deep Research

### Current State
- `claude_cli.rs` (1714 lines) provides `run_agent_stream` with full tool access + MCP vault tools
- `AgentStreamRequest` accepts: `message`, `system_prompt`, `vault_path`, `vault_paths`, `permission_mode`, `model`
- `antigravity_cli.rs` and other CLI agents follow the same pattern
- Frontend streaming: `streamAiAgent` → `streamNativeAiAgent` → IPC `stream_ai_agent` event with JSON events
- `aiAgentSession.ts` manages conversation state, abort, tool tracking
- No multi-step reasoning loop exists — it's single-prompt → stream

### Architecture
Deep research is a multi-step loop:
1. User submits a research query
2. Claude CLI runs with a "research agent" system prompt
3. Claude uses Bash to scrape web pages (WebFetch, curl, etc.) and writes findings to a vault note
4. Loop: Claude reads findings → identifies gaps → scrapes more → refines
5. Final pass: Claude synthesizes all findings into a structured report
6. Report is rendered in an AI workspace conversation

### Implementation Steps

#### 3.3.1: Backend — Deep Research System
Create `src-tauri/src/deep_research.rs`:
```rust
pub struct DeepResearchRequest {
    pub query: String,
    pub vault_path: String,
    pub vault_paths: Vec<String>,
    pub depth: u32,        // max iterations (default 3)
    pub model: Option<String>,
    pub permission_mode: AiAgentPermissionMode,
}

pub enum DeepResearchEvent {
    IterationStart { iteration: u32, goal: String },
    ToolStart { tool_name: String, tool_id: String },
    ToolDone { tool_id: String, output: String },
    SourceAdded { title: String, url: String, excerpt: String },
    InterimSummary { text: String },
    Result { report: String },
    Error { message: String },
    Done,
}
```

The orchestrator:
1. Writes a system prompt like: "You are a research agent. Your goal is to thoroughly research '{query}'. Use web scraping tools to gather information. After each tool call, decide if you need more sources or if you have enough to synthesize a final report. Write findings to NOTES.md in the vault. At the end, output your final report wrapped in <final-report> tags."
2. Spawns `claude -p` with agent mode + `--tools Bash` (or specific web tools)
3. Parses the JSON stream for `tool_input`/`tool_result` events
4. Emits `SourceAdded` events when web URLs are scraped
5. Counts iterations, caps at `depth`
6. On completion, extracts the final report from the response

Add a new IPC command `start_deep_research(request: DeepResearchRequest)` in `commands/ai.rs`.

#### 3.3.2: Frontend — Deep Research Hook
Create `src/hooks/useDeepResearch.ts`:
- `startDeepResearch(query, options) -> sessionId`
- Listens to `deep-research-stream` events
- Manages state: iterations, sources, interim summaries, final report
- Supports abort via the existing `abort_ai_agent_stream` pattern

#### 3.3.3: Frontend — Deep Research UI
Add to `AiWorkspace.tsx` or `AiPanel.tsx`:
- New "Deep Research" mode in the AI target picker
- Input: research query + depth slider (1-5)
- Progress display: iteration counter, sources found, live status
- Sources panel: clickable list of URLs scraped
- Final report: rendered as markdown with source citations

#### 3.3.4: CLI Wrapper
The deep research could also be invoked via a CLI subcommand:
```bash
npx nabu research "Impact of climate change on Mediterranean agriculture" --depth 3
```
This spawns the same Rust backend logic via a CLI wrapper that invokes the Tauri IPC.

---

## Implementation Priority
1. **HTML Mini-Apps** (medium effort, high value) — reuses existing window pattern
2. **CLI AI Deep Research** (medium effort, high value) — extends existing agent streaming
3. **Dictation Pill** (higher effort, medium value) — requires new audio backend + Web Speech API integration

---

## Retrospective: What Was Built
All three features were fully implemented across backend (Rust) and frontend (React):

- **mini_apps.rs** (566 lines): Full mini-app registry, URI protocol handler (`nabu-mini-app://`), WebviewWindow spawning, CSP-isolated asset serving, path-traversal protection
- **dictation.rs** + commands/dictation.rs** (315 lines total): Clipboard cache with 20-entry ring buffer, macOS audio input support, file drop capture pipeline
- **deep_research.rs** (510 lines): Multi-iteration research loop, URL extraction from tool calls, system prompt generation, CLI entry point
- **useMiniApps.ts, useDictation.ts, useDeepResearch.ts** (661 lines total): Frontend hooks with Tauri IPC + browser mock fallbacks
- **DictationPill.tsx** (261 lines): Floating pill with speech recognition, clipboard cache panel, drop zone
- **MiniAppWindowApp.tsx** (210 lines): Shell window for standalone mini-app apps with postMessage context relay
- **DeepResearchPanel.tsx** (250 lines): Full UI with iterations, sources, interim summaries, markdown report
- **hello-world mini-app** + manifest: End-to-end validation sample

---

## Key Integration Points (reference)
- `lib.rs:417` — `nabu-html-block` protocol registration (template for mini-app protocol)
- `lib.rs:434` — `nabu-mini-app` protocol registration (NEW: serves mini-app assets via custom URI scheme)
- `lib.rs:336-403` — `app_invoke_handler!` includes all new IPC commands: `start_dictation`, `stop_dictation`, `capture_file_drop`, `get_recent_clipboard_entries`, `restore_clipboard_entry`, `list_mini_apps`, `get_mini_app_config`, `open_mini_app_window`, `save_mini_app_config`, `delete_mini_app`, `open_mini_app_devtools`, `start_deep_research`, `abort_deep_research`
- `lib.rs:460-537` — CLI entry point `run_research_cli()` for `nabu research "<query>"` command
- `vault_instance.rs:4-131` — window creation pattern (reusable for mini-apps)
- `ai.rs:252-275` — `start_deep_research` + `abort_deep_research` IPC commands (stream via `run_desktop_stream`)
- `claude_invocation.rs:268-293` — agent args with tool policy (reuse for research)
- `streamAiAgent.ts:112-175` — frontend streaming pattern (mirrored in `useDeepResearch.ts`)
- `openAiWorkspaceWindow.ts:129-137` — `WebviewWindow` creation pattern (mirrored in `open_mini_app_window` backend command)
- `windowMode.ts` — window mode detection (reusable for mini-app detection)
