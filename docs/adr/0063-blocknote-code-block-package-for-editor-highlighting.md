---
type: ADR
id: "0063"
title: "BlockNote code-block package for editor syntax highlighting"
status: active
date: 2026-04-13
---

## Context

Nabu uses BlockNote for rich-text editing. Fenced code blocks already render on BlockNote's dark `pre > code` surface, but they were missing syntax highlighting and inherited the muted inline-code chip background from the global `code` selector in `EditorTheme.css`. The QA expectation is a dark code block with highlighted tokens and light code text, without regressing inline-code styling elsewhere in the editor.

BlockNote documents syntax highlighting as a schema concern: replace the default `codeBlock` spec with `createCodeBlockSpec(...)` and provide a Shiki highlighter. Nabu also needs to preserve the existing default behavior for unlabeled code blocks, which should stay plain text instead of defaulting to JavaScript.

## Decision

**Nabu overrides the default BlockNote `codeBlock` spec with `@blocknote/code-block`, keeps `defaultLanguage: "text"`, and scopes the muted inline-code chip styling away from fenced code blocks.**

## Options considered

- **Use `@blocknote/code-block`** (chosen): first-party BlockNote path, ships supported language aliases and a bundled Shiki highlighter, renders `.shiki` token spans in-editor, and avoids maintaining a parallel ProseMirror plugin integration.
- **Use a custom `createCodeBlockSpec({ createHighlighter })` bundle**: also valid, but Nabu does not need a custom language/theme bundle beyond BlockNote's packaged setup right now.
- **Keep BlockNote defaults and only fix CSS**: removes the nested gray chip bug, but leaves fenced code blocks unhighlighted and fails the product requirement.

## Consequences

Nabu's highlighting now lives in the editor schema instead of an editor-side plugin hook. `src/components/editorSchema.tsx` swaps in `createCodeBlockSpec({ ...codeBlockOptions, defaultLanguage: "text" })`, which adds BlockNote's language selector plus Shiki token spans for supported fenced blocks. `EditorTheme.css` continues to keep the `pre > code` background transparent so BlockNote's dark code-block shell remains intact.

The tradeoff is one new first-party dependency and BlockNote's bundled language menu inside code blocks. If Nabu later needs a narrower bundle, custom themes, or export-time highlighting parity, this ADR should be superseded with a custom Shiki bundle decision.
