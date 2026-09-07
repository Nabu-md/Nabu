# ADR 0134: Sheet Nodes With Plain-Text Workbook Storage

## Status

Experimental

## Context

Nabu should support spreadsheet-style content without losing its file-first, local-first, offline-first, and Git-friendly model. A sheet must remain inspectable and editable as plain text, while the interactive editor should behave like a spreadsheet and avoid a custom grid implementation owned by Nabu.

## Decision

Notes whose frontmatter resolves to `_display: sheet` are displayed with a dedicated sheet editor instead of the block note editor. `type` stays semantic and organizational metadata, so sheets can belong to any Nabu type. The body of the note is CSV-like plain text containing cell inputs and formulas. Sheet presentation metadata is stored in the same note frontmatter under `_sheet`.

Example:

```yaml
---
type: Project
_display: sheet
_sheet:
  frozen_rows: 1
  frozen_columns: 1
  columns:
    B:
      width: 180
  cells:
    C6:
      num_fmt: "0.00%"
      bold: true
      font_size: 15
      border_top: "thin #ff0000"
---
Metric,January,February
Revenue,1200,=B2*1.1
```

The prototype uses IronCalc's workbook package for the spreadsheet UI and formula engine. Nabu adapts between the plain-text note representation and the IronCalc workbook model on load/save.

## Consequences

- Spreadsheet data remains legible and diffable in Git.
- Common workbook UI behavior, including selection, keyboard navigation, formatting controls, and copy/paste, is delegated to IronCalc.
- Nabu-owned code is limited to format routing, the plain-text adapter, persistence safeguards, product-specific control hiding, and formula autocomplete. It should not grow into a custom spreadsheet grid.
- The current prototype is intentionally single-sheet. Cross-note cell references remain a future Nabu extension, not an IronCalc multi-sheet feature.
- `_sheet` stores common presentation state as plain YAML, including column widths, row heights, grid-line visibility, frozen rows/columns, number formats, borders, and basic cell styling.
- Metadata extraction is bounded and save serialization is debounced, with an idle-time pass when available, to avoid unbounded autosave scans. Larger workbooks may need incremental dirty-range tracking before this becomes production-ready.
- Formula autocomplete is a small Nabu-side enhancement over IronCalc's input surface; broader spreadsheet UI behavior should stay delegated to the workbook package.
- Simple Markdown wrappers in imported non-formula CSV cells can seed initial bold, italic, and strike cell styles, but saved sheet styling is represented in `_sheet` metadata rather than inline Markdown markers.
