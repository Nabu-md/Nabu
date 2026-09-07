# AI Agent Not Found

Source: troubleshooting/ai-agent-not-found.md
URL: /troubleshooting/ai-agent-not-found

# AI Agent Not Found

Nabu can only launch local CLI agents that are installed and discoverable.

## Symptoms

- The AI panel says no supported agent is available.
- Claude Code or another agent works in one shell but not in Nabu.

## Checks

Open a terminal and run the agent command directly. For Claude Code:

```bash
claude --version
```

If the command fails, install or repair the agent first.

## Path Issues

Desktop apps can inherit a different `PATH` from your interactive shell. Nabu checks common install locations, but shell setup can still vary. Prefer installing CLI tools in standard locations or making them available from your login shell.

---

# Git Authentication

Source: troubleshooting/git-auth.md
URL: /troubleshooting/git-auth

# Git Authentication

Nabu uses system Git authentication. It does not manage provider passwords directly.

## Symptoms

- Push fails.
- Pull asks for credentials repeatedly.
- Remote fetch works in one terminal but not in Nabu.

## Checks

1. Open a terminal.
2. `cd` into the vault.
3. Run `git remote -v`.
4. Run `git fetch`.

If `git fetch` fails in the terminal, fix system Git auth first.

## Common Fixes

- Sign in with GitHub CLI.
- Configure SSH keys.
- Update the remote URL.
- Check your credential helper.

---

# Model Provider Connection

Source: troubleshooting/model-provider-connection.md
URL: /troubleshooting/model-provider-connection

# Model Provider Connection

Use this checklist when a local or API model provider does not connect.

## Local Providers

For Ollama or LM Studio:

1. Start the local model server.
2. Confirm the base URL in Nabu matches the server.
3. Confirm the model ID is installed and loaded by the provider.
4. Use the Settings test action again.

## API Providers

For hosted providers:

1. Confirm the provider kind and endpoint.
2. Confirm the model ID exists for your account.
3. Confirm the API key is saved locally or available in the configured environment variable.
4. Avoid storing secrets in the vault.

## Chat Mode Boundary

Direct model targets run in chat mode. If you need file-editing tools, use a coding agent target such as Claude Code, Codex, OpenCode, Pi, or Antigravity CLI.

---

# Sync Conflicts

Source: troubleshooting/sync-conflicts.md
URL: /troubleshooting/sync-conflicts

# Sync Conflicts

Sync conflicts happen when local and remote changes touch the same content.

## What To Do

1. Stop editing the conflicted note.
2. Open the conflict resolver if Nabu presents it.
3. Review both sides.
4. Choose the correct content or merge manually.
5. Commit the resolved file.
6. Push again.

## Prevent Conflicts

- Pull before starting work on another device.
- Push after meaningful sessions.
- Keep AI-generated edits in small commits.
- Avoid editing the same note on multiple devices at the same time.

---

# Vault Not Loading

Source: troubleshooting/vault-not-loading.md
URL: /troubleshooting/vault-not-loading

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