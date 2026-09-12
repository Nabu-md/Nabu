---
type: ADR
id: "0046"
title: "Starter vault cloned from GitHub at runtime — no bundled content"
status: active
date: 2026-04-08
---

## Context

Laputa ships an optional "Getting Started" vault to help new users understand types, properties, wikilinks, and relationships. Previously, all starter content (markdown files, view YAMLs) was stored inside the app repo under `getting-started-vault/` and written to disk via `create_getting_started_vault()`. This created friction: updating sample content required a new app release, the content grew stale quickly, and the bundled files added noise to the main repo.

## Decision

**The Getting Started vault is no longer bundled in the app repo. On first launch, if the user selects "Get started with a template", the app clones the public starter repo (`https://github.com/Nabu-md/laputa-getting-started.git`) into a user-chosen folder using the existing git clone infrastructure.**

- `getting_started.rs` now holds only the public repo URL constant and delegates to `clone_public_repo()`.
- The `getting-started-vault/` directory has been removed from the app repo.
- `create_getting_started_vault(targetPath)` takes an explicit target path (chosen via folder picker) instead of defaulting to Documents/Getting Started.
- Clone failures show a user-friendly error with an inline "Retry download" button (`canRetryTemplate`, `retryCreateVault`).
- A `clone_public_repo()` function was added to `github/clone.rs` to clone unauthenticated public repos without injecting OAuth tokens or configuring remote auth.

## Options considered

- **Option A — Keep bundled content (status quo)**: Simple, works offline. Downside: content tied to app release cycle, repo noise, growing file count.
- **Option B — Clone from GitHub at runtime (chosen)**: Content is always current; starter vault can be updated without an app release; removes ~25 markdown files + YAML from the main repo. Downside: requires network on first use; failure modes need UX handling (retry flow added).
- **Option C — Download a zip archive**: Avoids a git clone, smaller payload. Downside: loses the clean git history in the cloned vault; adds a zip extraction code path.

## Consequences

- New users need a network connection when selecting the template option. The empty vault and open-folder paths remain fully offline.
- The starter repo (`laputa-getting-started`) becomes a separate maintenance artifact.
- `LAPUTA_GETTING_STARTED_REPO_URL` env var allows overriding the URL in tests without hitting GitHub.
- Onboarding UX now distinguishes three creation modes: `creatingAction: 'template' | 'empty' | null`, each with distinct button state and status copy.
- Retry UX: `lastTemplatePath` is cached in `useOnboarding` so users can retry a failed clone to the same folder without re-picking it.
- Re-evaluation trigger: if offline-first support becomes a priority, consider bundling a minimal vault again or shipping a fallback zip.
