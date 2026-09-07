---
type: ADR
id: "0178"
title: "Custom protocol for scripted HTML blocks"
status: active
date: 2026-08-21
---

## Context

ADR-0157 permits inline scripts only for HTML fences that declare `scripts="sandboxed"`. The renderer originally navigated those iframes to a `data:text/html` URL. Packaged webviews apply the app window's production Content Security Policy to local-scheme frames in addition to the document's own policy, so the window's intentional ban on inline scripts still blocked the opted-in script. Development hid the defect because its React Refresh policy permits inline scripts.

Weakening the app window's `script-src` would expose every renderer surface to inline script execution. `srcdoc`, `data:`, and `blob:` cannot establish an independent policy boundary, so changing among those local document forms does not solve the packaged-build problem.

## Decision

**Nabu serves only opted-in scripted HTML block previews through the private `nabu-html-block` Tauri URI scheme.**

The renderer resolves vault expressions, sanitizes markup, and builds the complete iframe document exactly as before. It UTF-8/base64url encodes that sanitized document into the protocol path. The native protocol handler accepts only GET requests with one bounded valid payload, decodes UTF-8, and returns the document with a restrictive response-header CSP. The iframe still uses `sandbox="allow-scripts ..."` without `allow-same-origin`, so the loaded document receives an opaque origin.

Static HTML blocks continue to use `srcdoc` without script permission. Browser-mode tests retain a `data:` fallback because no native protocol exists there; packaged native QA is required for the protocol path. The app window keeps its existing `script-src`; `frame-src` adds only the private protocol origins needed by Tauri on Unix and Windows.

## Consequences

- Explicitly opted-in inline scripts run under the same policy in development and packaged builds.
- The app window still rejects inline scripts and does not grant the HTML block access to the app origin or Tauri IPC.
- Network requests, workers, nested frames, forms, base URLs, and remote resources remain blocked by the protocol response CSP and sanitizer.
- Preview payloads are stateless: there is no native registry, temporary file, cleanup command, or persisted HTML copy.
- Protocol URLs contain encoded sanitized markup and are bounded to eight MiB; malformed, nested, non-UTF-8, oversized, and non-GET requests fail closed.
- This ADR amends ADR-0157 only for the packaged document-delivery boundary; its explicit opt-in and sandbox rules remain in force.
