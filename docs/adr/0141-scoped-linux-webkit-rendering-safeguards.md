---
type: ADR
id: "0141"
title: "Scoped Linux WebKit rendering safeguards"
status: active
date: 2026-06-18
---

## Context

Nabu needs Linux WebKitGTK startup safeguards because some Wayland/AppImage environments crash before the app can render. The existing startup path treated native Linux Wayland launches and sealed Linux AppImage launches the same way by setting both `WEBKIT_DISABLE_DMABUF_RENDERER=1` and `WEBKIT_DISABLE_COMPOSITING_MODE=1` unless the user had already provided either variable.

That broad fallback protected unstable AppImage launches, but it also applied the last-resort compositing disablement to native Wayland sessions. Native Wayland still needs the DMABUF crash workaround, but disabling WebKit compositing there can make windows feel unresponsive. The sealed AppImage runtime remains the verified environment that needs both rendering overrides.

## Decision

Nabu scopes Linux WebKit rendering safeguards by launch environment:

- Native Linux Wayland launches set `WEBKIT_DISABLE_DMABUF_RENDERER=1` by default, while preserving WebKit compositing unless the user explicitly disables it.
- Linux AppImage launches continue to set both `WEBKIT_DISABLE_DMABUF_RENDERER=1` and `WEBKIT_DISABLE_COMPOSITING_MODE=1` by default because the sealed AppImage path has the verified rendering failure this fallback protects.
- User-provided environment values remain authoritative per variable, so advanced users and distro-specific workarounds can override either safeguard.

## Alternatives considered

- **Scope the fallback by launch environment** (chosen): keeps the proven AppImage protection while avoiding a heavier native Wayland workaround that hurts responsiveness. Cons: the startup policy now distinguishes AppImage from native Linux sessions.
- **Keep both overrides for all Wayland and AppImage launches**: simplest and maximally conservative for crash avoidance, but applies the last-resort compositing workaround beyond the environment that actually needs it.
- **Remove the WebKit rendering overrides entirely**: restores default WebKitGTK behavior, but would reopen known Linux startup crashes in AppImage/Wayland environments.

## Consequences

Native Linux Wayland users keep the broad DMABUF crash workaround without losing WebKit compositing by default. AppImage users keep the sealed-runtime fallback that has been validated against the startup crash class.

Future Linux rendering work should treat `linux_appimage.rs` startup overrides as a capability/policy boundary, not as a single global Linux switch. Re-evaluate this decision if WebKitGTK or the AppImage runtime no longer requires these environment safeguards, or if another packaged Linux runtime develops a distinct rendering failure mode.
