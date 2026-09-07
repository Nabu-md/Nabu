# Supported Platforms

Source: reference/supported-platforms.md
URL: /reference/supported-platforms

# Supported Platforms

Nabu is a desktop app built with Tauri. Releases currently target macOS, Windows, and Linux.

| Platform | Current support | Notes |
| --- | --- | --- |
| macOS | Primary | Main development and QA target. Apple Silicon and Intel artifacts are published. |
| Windows | Supported, early | NSIS installers and signed updater bundles are published. Menu, shell-path, and credential-helper behavior receive platform-specific fixes as they appear. |
| Linux | Supported, early | AppImage, deb, and RPM artifacts are published. Behavior can depend on distro WebKitGTK packages, Wayland/X11 details, and input-method setup. |

## Support Policy

Primary support means the platform is part of normal development and release validation. Supported, early means release artifacts exist and the app is expected to work, but platform-specific bugs can take longer to diagnose than macOS issues.

## Reporting Platform Bugs

Include:

- Nabu version.
- Operating system and version.
- CPU architecture.
- Whether the vault is local-only or connected to a remote.
- Steps to reproduce.