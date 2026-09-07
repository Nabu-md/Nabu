# Git

Source: concepts/git.md
URL: /concepts/git

# Git

Git is Nabu's recommended history and sync layer. Nabu can work with plain Markdown folders, and Git unlocks local history, recovery, remote backup, and multi-device workflows when you want them.

Nabu acts as a lightweight Git client for your vault. You can review changes, commit, pull, push, and inspect history without leaving the app.

## What Nabu Uses Git For

- Whole-vault commit history.
- Current diff for the vault.
- Per-note history.
- Current diff for an individual note.
- Pull and push.
- Conflict detection and resolution.
- Remote connection for local-only vaults.

## History And Diffs

Each note can show its own history and current diff, so you can understand how that file changed over time or what is unsaved relative to Git.

Nabu also shows a history of the whole vault. Use it when you want to review broader changes across multiple notes before committing or syncing.

## Local Commits

You can commit changes inside Nabu without leaving the app. This gives you useful restore points even before a remote is configured.

## Remotes

Connect a compatible Git remote when you want sync or backup. Nabu relies on your system Git authentication, so GitHub CLI, SSH keys, credential helpers, and existing Git configuration can continue to work.

## Vaults Inside A Parent Repository

A vault can be a folder inside a larger Git repository. Nabu discovers the nearest parent work tree but keeps the selected vault as the content boundary:

- status, diffs, history, and app-created commits include only vault files
- files elsewhere in the repository do not appear as Nabu notes
- pull, push, branch, merge, and rebase state still belong to the whole repository

The resolved Git root is visible in Settings when it differs from the vault root.