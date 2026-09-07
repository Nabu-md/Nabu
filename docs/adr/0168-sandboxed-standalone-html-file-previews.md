---
type: ADR
id: "0168"
title: "Sandboxed standalone HTML file previews"
status: active
date: 2026-07-20
---

## Context

Generated documentation, reports, and prototypes are often stored as standalone `.html` or `.htm` files beside their styles and images. Opening those files only in the system browser interrupts the vault workflow, while rendering arbitrary vault HTML directly in Nabu's application document would expose the parent DOM and potentially the Tauri IPC surface. Authors also need a direct route back to the source without losing the preview.

## Decision

**Nabu renders standalone HTML files in the editor pane through a sanitized, opaque-origin iframe preview and uses the existing raw-mode control for source editing.**

Before setting `srcdoc`, the renderer removes scripts, event handlers, forms, nested frames, embedded objects, and other active controls. It adds a restrictive Content Security Policy that disables scripts, network connections, workers, forms, nested frames, objects, and base-URL changes. The iframe omits `allow-scripts` and `allow-same-origin`, so preview content cannot access Nabu's document, storage, or Tauri IPC.

Passive local resources such as images and stylesheets are resolved relative to the HTML file, accepted only when their normalized path remains inside the active vault, and converted to the existing scoped Tauri asset URL. Remote passive loads are removed. Links may open in a separate external browsing context with `noopener` and `noreferrer`; the existing explicit “open in default app” command remains available.

The preview occupies the same surface as the rich editor. The breadcrumb source button and `Cmd/Ctrl+\` switch an HTML file into the existing CodeMirror raw editor, where normal save behavior applies; toggling again rebuilds the preview from the current tab content.

## Alternatives considered

- **Keep opening HTML only in the system browser:** preserves a strong boundary but breaks in-app reading and makes preview/edit iteration cumbersome.
- **Render the document directly in the application DOM:** offers seamless styling but grants untrusted markup far too much access to Nabu's UI and runtime.
- **Use a sandboxed iframe with scripts enabled:** supports interactive prototypes, but expands the security and resource-governance surface beyond this viewer's needs.
- **Serve files from a temporary localhost server:** provides browser-like relative resource loading, but adds process, port, lifecycle, and network-origin complexity.

## Consequences

- HTML reports can be read and edited without leaving Nabu.
- Scripts and form-driven interactive prototypes do not execute in preview mode; users can still open the file in their default browser when that behavior is required.
- Local styles, images, fonts, and media work only when they remain inside the active vault.
- HTML previewing remains renderer-owned and does not require a new native command or persisted file kind.
