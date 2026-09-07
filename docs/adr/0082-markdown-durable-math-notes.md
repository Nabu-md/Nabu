---
type: ADR
id: "0082"
title: "Markdown-durable math in notes"
status: active
date: 2026-04-26
---

## Context

Nabu notes are durable Markdown files, while the main editor uses BlockNote and raw mode uses CodeMirror. Users coming from technical note-taking tools expect inline math such as `$E=mc^2$` and display math such as `$$ ... $$` to render inside notes without turning the note into an app-only document format.

BlockNote does not currently ship a first-party math block in the local editor package. Tiptap now offers an official Mathematics extension that renders KaTeX nodes, but Nabu's save path still depends on BlockNote's Markdown parser and `blocksToMarkdownLossy()` serializer. Adding opaque ProseMirror math nodes without an explicit Nabu serializer would risk losing or rewriting the original Markdown source.

## Decision

**Nabu will support note math through a Markdown placeholder round-trip owned by the editor pipeline.**

The initial implementation:

- Treats `$...$` as inline math and line-owned `$$...$$` / multiline `$$` blocks as display math.
- Converts math source to temporary placeholders before BlockNote parses Markdown.
- Replaces placeholders with Nabu schema nodes that render via the existing `katex` dependency.
- Serializes those schema nodes back to the original Markdown delimiters before saving or entering raw mode.
- Uses KaTeX with `throwOnError: false` and `trust: false` so malformed or untrusted formulas remain visible rather than breaking the note.

## Options considered

- **Nabu-owned placeholder round-trip with KaTeX rendering** (chosen): matches the existing wikilink architecture, preserves plain-text source, and avoids depending on BlockNote support for non-default ProseMirror math nodes.
- **Tiptap Mathematics extension directly in BlockNote**: attractive because it is official Tiptap and KaTeX-backed, but it does not by itself solve Nabu's BlockNote Markdown serializer contract.
- **Raw-mode-only math support**: preserves source but fails the rich editor reading experience users expect.
- **Store formulas as custom JSON/frontmatter metadata**: richer structured editing later, but violates the Markdown-first durability requirement.

## Consequences

- `src/utils/mathMarkdown.ts` is the canonical parser/serializer bridge for note math.
- The rich editor renders math as schema nodes; raw mode remains the most direct way to edit exact math source.
- CodeMirror raw editing keeps the literal Markdown delimiters, so imported Obsidian-style notes remain understandable outside Nabu.
- Future equation editing helpers can be added on top of the same Markdown source contract instead of changing the storage model.
- Re-evaluate direct Tiptap Mathematics integration only if it can be proven to preserve Nabu's Markdown save path without custom lossy behavior.
