---
type: ADR
id: "0121"
title: "AppImage external fallback for audio and video previews"
status: active
date: 2026-05-15
supersedes: "0110"
---

## Context

ADR-0110 standardized in-app previews for image, audio, video, and PDF vault files through the shared `FilePreview` surface and Tauri asset URLs. In practice, Linux AppImage builds run audio and video playback through WebKitGTK, and that runtime has proven unstable enough that mounting the same in-webview media controls is not a reliable default for packaged Linux releases.

Nabu still needs one binary-preview model across platforms: previewability should remain renderer-inferred from filename extensions, binary files should remain ordinary vault entries, and external-open actions must continue to re-enter the active-vault command boundary before the OS opens a file.

## Decision

**Nabu keeps in-app image and PDF previews everywhere, but Linux AppImage builds fall back to external-open controls for audio and video instead of mounting in-webview media playback.**

- `FilePreview` remains the single renderer-owned surface for supported binary vault files.
- The preview policy is runtime-owned: the renderer asks the native runtime whether external media fallback is required before rendering audio or video elements.
- Linux AppImage builds return `true` for that runtime check and suppress in-webview audio/video previews; other targets keep the existing native HTML media controls.
- Editor-embedded BlockNote audio/video blocks follow the same runtime gate so binary preview behavior stays consistent between note bodies and file previews.

## Alternatives considered
- **Runtime-gated external fallback on Linux AppImage** (chosen): keeps one preview architecture while containing a platform-specific runtime instability. Cons: AppImage users lose inline playback for audio/video.
- **Keep in-app audio/video previews on every platform**: preserves feature parity, but continues shipping a known unstable playback path on AppImage.
- **Disable all binary previews on Linux**: simpler policy, but unnecessarily removes stable image/PDF previews and weakens the file-first editor experience.

## Consequences

Nabu now treats audio/video preview as a runtime capability decision rather than a universal guarantee of the binary preview system. Linux AppImage users see explicit external-open fallback controls for audio and video, while other platforms keep the richer in-app playback path.

This keeps the filesystem-first binary model, scoped asset access, and active-vault validation boundary intact without introducing persisted media types or a separate media subsystem. Re-evaluate this decision if AppImage media playback becomes stable enough to restore inline playback without special handling, or if other packaged runtimes need their own preview capability gates.
