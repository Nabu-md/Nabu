# Public Docs Plan

This document records the phase 1 information architecture for public Nabu documentation. The public docs source lives in `site/`; the existing `docs/` directory remains contributor, architecture, and agent context.

## Audiences

| Audience | Needs | Primary location |
|---|---|---|
| New users | Install, first launch, understand the app layout, clone the starter vault | `site/start/` |
| Active users | Learn concrete workflows such as organizing, Git sync, custom views, and AI | `site/guides/` |
| Power users | Understand file layout, frontmatter, filters, release channels, shortcuts, and platform support | `site/reference/` |
| Contributors and agents | Architecture, abstractions, ADRs, development workflow | `docs/`, `AGENTS.md` |

## Hosting Shape

The GitHub Pages output should reserve the root for public docs and mount release assets underneath it:

```text
/                  public docs home
/releases/         release history
/download/         latest stable download redirect
/stable/latest.json
/alpha/latest.json
/latest.json       compatibility alias for alpha latest
/latest-canary.json compatibility alias for alpha latest
```

## Current Coverage

The site covers stable behavior through v2026-07-22, including:

- Windows and Linux release artifacts.
- Stable and Alpha updater channels.
- Direct AI model providers and local/API model setup.
- Claude Code, Codex, GitHub Copilot, OpenCode, Pi, Antigravity CLI, Kiro, and Hermes agent targets.
- Per-agent model selection and explicit MCP setup for external AI tools.
- MCP note creation, full-note update, and append workflows.
- Rich-editor block selection, collapsible sections, callouts, code blocks, date/time commands, highlights, and todos.
- Table of contents, note width, raw mode, and invalid-frontmatter feedback.
- Media/PDF/HTML previews, durable web-image imports, All Notes visibility, and Markdown whiteboards.
- Parent Git repository support, AutoGit, manual commits, and remote sync.
- Spreadsheet notes, cross-note formulas, HTML blocks, and vault expressions.
- System theme mode and sidebar pluralization settings.

Every user-visible app change should answer:

```text
Public docs impact:
- updated: <pages>
- not needed because: <reason>

Getting Started vault impact:
- updated: <examples>
- not needed because: <reason>
```
