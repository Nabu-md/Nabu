# 0175. MCP vault lifecycle uses the app registry

Status: active

Date: 2026-08-12

## Context

Vault-neutral MCP clients already resolve mounted workspaces from Nabu's installation-local `vaults.json`, but they could only operate on registered vaults. Attaching a prepared folder or cloning a repository therefore required GUI automation or direct config edits that did not update a running renderer.

## Decision

Expose `attach_vault` and `clone_vault` as explicit writable MCP tools. Both update the same current-namespace vault registry as the desktop app without changing `active_vault` or initializing Git implicitly.

- Attach requires an existing readable absolute directory, canonicalizes it, and rejects vaults nested inside one another.
- Clone uses the system Git configuration, disables interactive prompts, clones into a temporary sibling, and renames it into place only after success.
- Registry writes are serialized within the MCP process and atomically renamed into the preferred current namespace.
- A successful registration is added to the long-lived tool service's active path set immediately and broadcasts `vault_registry_changed` to connected Nabu renderers.
- The renderer reloads registry metadata while preserving its current active vault.

## Consequences

The same MCP connection can list and use a newly attached or cloned vault without reconnecting. Running desktop windows discover it without relaunching or switching the user's current workspace. Clone remains an open-world operation and relies on the user's existing Git authentication, matching ADR-0056.
