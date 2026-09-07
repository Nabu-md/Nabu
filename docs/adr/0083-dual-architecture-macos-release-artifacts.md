---
type: ADR
id: "0083"
title: "Dual-architecture macOS release artifacts"
status: active
date: 2026-04-26
supersedes: "0080"
---

## Context

ADR-0080 made Nabu's desktop release pipeline cross-platform, but the macOS leg still shipped only Apple Silicon artifacts. That left Intel Mac users without a compatible build in both the alpha feed and stable releases, even though Tauri and Rust can produce `x86_64-apple-darwin` bundles from the same release workflow.

The updater manifest also needs to distinguish macOS CPU architectures. A generic `darwin` or macOS-only entry would make it too easy for an Intel installation to see an Apple Silicon updater bundle, and browser user agents cannot reliably tell Apple Silicon Macs apart from Intel Macs.

## Decision

**Nabu publishes macOS release artifacts for both Apple Silicon (`darwin-aarch64`) and Intel (`darwin-x86_64`) in every alpha and stable release.**

- Alpha and stable workflows build the macOS matrix for `aarch64-apple-darwin` and `x86_64-apple-darwin`.
- Alpha manifests include signed updater tarballs for `darwin-aarch64` and `darwin-x86_64`.
- Stable manifests include both macOS updater tarballs and both manual DMG downloads, alongside the existing Windows x64 and Linux x86_64 entries.
- Release jobs normalize macOS artifact filenames with the architecture suffix before publishing so GitHub release assets stay unambiguous.
- The stable download page exposes separate Apple Silicon and Intel Mac links. When both Mac links exist, a generic macOS browser is not auto-redirected because user-agent architecture detection is unreliable.
- The cross-platform filename portability decisions from ADR-0080 remain in force.

## Options considered

- **Publish separate Apple Silicon and Intel Mac artifacts** (chosen): gives each updater client an architecture-specific manifest key and gives users explicit manual download links. Cons: doubles the macOS release matrix and signing/notarization surface.
- **Publish a universal macOS binary**: gives users one download, but requires lipo/re-sign/notarize coordination and reintroduces the artifact-combining complexity the release pipeline intentionally avoided.
- **Keep Apple Silicon-only macOS releases**: keeps CI cheaper, but leaves Intel Mac users unsupported and makes the release artifacts inconsistent with Nabu's desktop support goals.

## Consequences

- macOS release jobs now run one matrix entry per CPU architecture.
- Release manifest consumers must treat `darwin-aarch64` and `darwin-x86_64` as distinct platform keys.
- Stable manual downloads show two Mac choices instead of pretending browser detection can select the right CPU architecture.
- Future macOS release changes must validate both updater and manual-download artifacts for both architectures.
