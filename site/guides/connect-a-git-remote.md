# Connect A Git Remote

Connect a remote when you want backup or sync beyond the current machine.

## Before You Start

Make sure the remote repository exists and your system Git can authenticate to it. Nabu uses system Git rather than storing provider-specific credentials.

## Steps

1. Open the bottom status bar remote chip, or run `Add Remote` from the command palette.
2. Paste the remote URL.
3. Confirm the remote name.
4. Fetch or push according to the app prompt.

## Recommended Auth

- SSH keys.
- GitHub CLI authentication.
- Existing Git credential helpers.
- macOS Keychain credentials for HTTPS remotes on macOS.

If authentication fails, see [Git Authentication](/troubleshooting/git-auth).
