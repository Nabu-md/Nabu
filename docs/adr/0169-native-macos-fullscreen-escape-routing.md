---
type: ADR
id: "0169"
title: "Native macOS fullscreen Escape routing"
status: active
date: 2026-07-20
---

## Context

macOS owns Escape as the system command for leaving native window fullscreen. Nabu also uses Escape to dismiss shadcn/Radix dialogs and popovers. Preventing the DOM keyboard event is not sufficient in WKWebView: AppKit still processes the same physical keypress and exits fullscreen after the overlay closes.

## Decision

Nabu installs a narrow AppKit local key monitor on macOS. The renderer reports whether a visible dismissible surface is open through shared shadcn dialog/popover refs and the legacy Settings panel lifecycle. When the main window is fullscreen and that state is open, the monitor consumes physical Escape and dispatches a synthetic Escape into the focused webview element so the overlay closes without AppKit leaving fullscreen. Every other key and every Escape outside that exact state continues through the normal native path.

The monitor uses `objc2-app-kit` directly. Its two platform-boundary operations carry explicit `SAFETY` invariants: AppKit supplies a non-null callback event for the callback lifetime, and the block returns only that same event pointer or null.

## Options considered

- **Audited AppKit local monitor with renderer surface state** (chosen): preserves standard macOS fullscreen Escape when no overlay is open while giving dialogs first and exclusive handling.
- **Conditional native menu accelerator**: closes the overlay, but AppKit still exits fullscreen after the menu event runs.
- **Global Escape shortcut**: receives the key, but does not prevent AppKit's fullscreen exit and unnecessarily captures a system-wide key.
- **Third-party safe AppKit wrapper**: compiled successfully but left the webview blank after processing Escape in native QA.
- **DOM `preventDefault()` only**: keeps the logic in the renderer but does not stop AppKit's fullscreen transition in WKWebView.
- **Exit and immediately re-enter fullscreen**: avoids permanent exit but produces two visible Space animations and unstable layout transitions.
- **Disable Escape fullscreen exit globally**: simpler native handling, but breaks the expected macOS way to leave fullscreen when no overlay is open.

## Consequences

- Renderer surface lifecycles keep one boolean native state synchronized while dismissible surfaces open and close.
- macOS owns the physical-key routing decision; the renderer still owns each overlay's normal Escape behavior.
- Native QA must verify both branches: dialog Escape stays fullscreen, and a subsequent bare Escape exits fullscreen.
