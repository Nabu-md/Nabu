# Vault Not Loading

Use this checklist when Nabu cannot open or refresh a vault.

## Check The Folder

- Confirm the folder exists.
- Confirm the folder contains readable files.
- Confirm Nabu has permission to access the folder.
- Try opening a smaller test vault to isolate the issue.

## Check Git

If the vault is a Git repository, verify it is not in a broken state:

```bash
git status
```

Resolve interrupted merges or corrupted repository state before retrying.

## Reload

Run `Reload Vault` from the command palette. This clears derived cache and rescans the filesystem.

