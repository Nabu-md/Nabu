# ADR-0171: Separate vault application instances

**Status:** Active

**Date:** 2026-07-23

## Context

Nabu's ordinary desktop launch is intentionally single-instance, and additional Tauri windows inside that process share one macOS application identity. macOS therefore gives those windows one Dock tile and one Command-Tab entry with one process-wide application icon. That model cannot make concurrently open vaults individually identifiable in the Dock or app switcher.

The registered vault list already stores an installation-local accent color for each vault. Opening another vault must reuse that identity without changing which vault the next ordinary Nabu launch restores.

## Decision

- Keep ordinary Nabu launches single-instance. An explicit `--nabu-vault-instance <path>` launch skips the single-instance plugin and creates an independent application process.
- Launch packaged macOS instances through Launch Services with `open -n`, passing the canonical vault path and its allowlisted accent-color name as arguments. Development and non-macOS builds spawn the current executable directly.
- Override the process-local loaded vault list's active vault with the launch path. When that auxiliary process persists the shared registry, preserve the registry's existing global `active_vault` so it does not steal the next ordinary launch.
- Derive each process's application icon at runtime from Nabu's bundled light/dark icon. Recolor only the blue droplet pixels with the launched or active vault palette, keep blue as the fallback, and install the result as the process-wide AppKit application icon.
- Encode the recolored RGBA image as PNG, then use direct `objc2` AppKit/Foundation bindings for the macOS application-icon boundary. A narrow non-null Objective-C protocol wrapper keeps the Rust call site safe. The renderer reaches process creation through the `open_vault_in_new_window` Tauri command.

## Consequences

Each separately opened vault has its own renderer, native state, filesystem watchers, Dock tile, Command-Tab entry, and process-wide accent icon. The ordinary app retains its existing single-instance and deep-link behavior.

Separate processes consume more memory than in-process windows. Their identity remains the vault selected at launch even if the user later changes focus inside that process. Vault registration and settings remain installation-local shared data, while auxiliary saves do not change the ordinary launch's selected vault.
