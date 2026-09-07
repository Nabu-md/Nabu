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