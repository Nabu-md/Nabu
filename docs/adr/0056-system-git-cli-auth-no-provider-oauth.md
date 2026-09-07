---
type: ADR
id: "0056"
title: "System git auth only — no provider-specific OAuth or repo APIs"
status: active
date: 2026-04-12
supersedes: "0019"
---

## Context

Nabu already uses the system `git` executable for the core remote workflow: commit, pull, push, status, history, and conflict resolution. The only provider-specific part left was GitHub authentication and repository management:

- GitHub Device Flow OAuth
- persisted `github_token` / `github_username` settings
- GitHub-only clone/create UI
- GitHub API calls for repo listing and creation

That split made the product more complex than the actual user need. Nabu's remote-sync users are developers who typically already have git configured via SSH keys, Git Credential Manager, Keychain helpers, or `gh auth`. The app was carrying a provider-specific auth stack even though the real transport path was already plain git CLI.

## Decision

**Nabu does not implement provider-specific authentication or remote-repository APIs. All remote auth is delegated to the user's existing system git configuration, and cloning is a generic "paste any git URL" flow.**

Concretely:

- remove GitHub Device Flow commands and UI
- remove persisted GitHub auth fields from app settings
- remove GitHub repo list/create API integration
- keep `clone_repo`, but make it a generic system-git clone command
- keep commit / pull / push behavior unchanged apart from surfacing raw git errors directly

## Options considered

- **Option A — Keep GitHub Device Flow OAuth** (ADR-0019, now superseded): polished GitHub-specific onboarding, but it preserves provider lock-in, token storage, and an entire second auth model beside system git.
- **Option B — Replace OAuth with manual PAT entry**: smaller implementation than Device Flow, but still provider-specific, still stores credentials in app settings, and still teaches users the wrong abstraction.
- **Option C — Pure system git auth** (chosen): one auth path, less code, works with any git host, and aligns the clone flow with the rest of Nabu's git stack. Downside: users must already have git auth configured outside the app.

## Consequences

- `CloneVaultModal` accepts any git URL and local destination path.
- `clone_repo` shells out to system git without injecting provider tokens.
- `git_push` / `git_pull` continue to rely on the same external git configuration; auth failures surface as raw git stderr.
- `SettingsPanel` no longer contains a GitHub connection section.
- Nabu no longer stores git-provider credentials in `settings.json`.
- GitHub, GitLab, Bitbucket, Gitea, and self-hosted remotes all work through the same product path.
- Creating or listing remote repos from inside Nabu is no longer supported; remote setup happens in the user's normal git tools.
- The Getting Started vault still clones from a public remote URL, but it now goes through the same generic git clone path as every other vault import.

Re-evaluate if Nabu later targets less technical users who cannot reasonably be expected to configure git outside the app.
