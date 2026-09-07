---
type: ADR
id: "0162"
title: "Safe local import of remote paste images"
status: active
date: 2026-07-19
---

## Context

Web selections commonly place formatted HTML and remote image URLs on the clipboard. Pasting those URLs directly into a local-first note leaves the note dependent on the source website, while downloading them from the renderer would expose native network and filesystem access to editor markup. The paste itself must also stay responsive and preserve non-image content when a download fails.

## Decision

**Nabu pastes web content immediately, then imports eligible remote images through a vault-scoped native command and rewrites only the pasted image references that were saved successfully.**

The renderer extracts unique `http` and `https` image references from rich HTML and Markdown clipboard payloads. Rich mode lets BlockNote perform its normal synchronous paste and later updates matching image blocks. Raw mode inserts a Markdown representation synchronously and replaces the inserted range only if it has not been edited while downloads were running. Failed references remain remote and produce one localized, non-blocking message.

The Rust boundary resolves each destination to a public address and pins the HTTP client to that address. It rejects local/private/link-local destinations, URL credentials, unsupported schemes, unsafe redirects, non-image or unsupported MIME types, responses above 15 MiB, and requests that exceed bounded connect/total timeouts. Redirects repeat the same validation. Bytes are buffered before the attachment file is created, and filenames come from the final URL stem plus the validated MIME extension.

Saved references use the existing portable `attachments/...` representation and the existing unique attachment-path owner. Telemetry records only editor surface and aggregate success/failure counts; URLs, paths, filenames, and note content are excluded.

## Alternatives considered

- **Fetch images in the renderer:** simpler integration, but weakens the native security boundary and makes filesystem writes harder to constrain.
- **Wait for all downloads before pasting:** avoids later rewrites, but blocks capture, risks losing the whole selection, and makes slow hosts visible as editor latency.
- **Leave all URLs remote and offer a later command:** safer to implement, but does not make ordinary web capture durable or offline-readable.
- **Proxy downloads through a hosted Nabu service:** could centralize filtering, but violates the local-first path and adds content disclosure plus service availability dependencies.

## Consequences

- Rich and raw editors share one remote-image extraction/import model while retaining their native paste behavior.
- Partial failure is non-destructive: text and successful images remain, and failed links stay editable.
- Private-network images and SVG are intentionally not imported from the web-paste path.
- The native command performs DNS and network work on the blocking pool instead of the Tauri command thread.
- Remote image imports always follow Nabu's existing vault attachment convention and portable Markdown path rules.
