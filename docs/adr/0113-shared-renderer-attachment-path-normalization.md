---
type: ADR
id: "0113"
title: "Shared renderer attachment path normalization"
status: active
date: 2026-05-07
---

## Context

Nabu already treats vault attachments as ordinary files under `attachments/`, and ADRs around previews and asset scoping rely on Tauri asset URLs to render them safely. In practice, attachment handling had started to fragment across the renderer: some flows joined `vaultPath + attachments/...`, some decoded `convertFileSrc` URLs directly, some handled Windows separators ad hoc, and some only worked for one editor surface.

That duplication turned attachment behavior into a drift risk. Opening file blocks, following editor links, serializing raw-mode Markdown, rewriting image URLs after vault switches, and copying dropped files into the vault all needed the same three representations to stay in sync:

- portable markdown references such as `attachments/report.pdf`
- Tauri asset URLs used by the renderer
- absolute filesystem paths inside the active vault

## Decision

**Nabu centralizes attachment path conversion in a single renderer-owned primitive and keeps portable `attachments/...` references as the canonical cross-surface representation.**

Specifically:

1. `src/utils/vaultAttachments.ts` is the single owner for converting between portable attachment references, Tauri asset URLs, and active-vault filesystem paths.
2. Editor rendering, raw-mode serialization, image upload/drop flows, file-block open actions, and parsed-media cleanup must call that shared primitive instead of carrying local path/URL conversion logic.
3. Renderer code may derive absolute paths only relative to the current active vault and must reject asset URLs or relative paths that fall outside that boundary.
4. Tauri asset URLs remain a transport/rendering detail, not a persisted vault format.

## Alternatives considered

- **Shared renderer attachment-path primitive with portable persisted refs** (chosen): keeps behavior consistent across media rendering, editor actions, and vault switching while preserving Markdown portability.
- **Per-feature helpers for each attachment surface**: simpler locally, but repeats Windows/path-normalization rules and lets editor actions drift apart.
- **Persist absolute paths or Tauri asset URLs in Markdown/editor state**: would couple notes to one machine or one runtime session and make vault content less portable.
- **Push all attachment conversion into backend commands**: could reduce renderer logic, but the renderer still needs a shared local model for in-memory markdown rewriting, link activation, and preview URL handling.

## Consequences

- Attachment behavior becomes consistent across previews, editor links, toolbar/file-block opens, drag-drop imports, and markdown serialization.
- Vault content stays portable because persisted references remain `attachments/...` paths rather than machine-specific absolute paths or session-specific asset URLs.
- Cross-platform edge cases such as Windows separators, encoded asset URLs, and vault-switch rewrites now have one place to harden and test.
- The Rust command layer remains the write/read security boundary; this ADR only centralizes renderer-side normalization before those commands are called or asset URLs are rendered.
- Future attachment/media features should extend `vaultAttachments.ts` rather than introducing new ad hoc conversion helpers.
