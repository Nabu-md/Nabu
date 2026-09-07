# Manage Git Manually Or With AutoGit

Source: guides/commit-and-push.md
URL: /guides/commit-and-push

# Manage Git Manually Or With AutoGit

Nabu can act as a lightweight Git client for a Git-enabled vault. You can manage commits and pushes yourself, or enable AutoGit to create conservative checkpoints after editing pauses or when the app is no longer active.

## Manual Git

1. Open the Git or changes surface.
2. Review changed files.
3. Write a short commit message.
4. Commit locally.
5. Push when a remote is configured.

If the remote has changed, pull first and resolve any conflicts. If the vault has no remote, manual commits still give you local history, diffs, and rollback.

## AutoGit

AutoGit is available in Settings for Git-enabled vaults. When enabled, Nabu automatically commits and pushes saved local changes after an idle pause or after the app becomes inactive.

Use AutoGit when you want the safety of regular checkpoints without interrupting capture or editing. You can still inspect each note's current diff, review note history, and browse the whole-vault history before making larger manual commits.

## Use Small Commits

Small commits make it easier to understand what changed, roll back safely, and review AI-generated edits.