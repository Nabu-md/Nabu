---
type: ADR
id: "0167"
title: "Non-collapsible callout scope"
status: active
date: 2026-07-20
amends: "0160"
---

## Context

ADR-0160 introduced schema-backed callouts and included Obsidian's optional `+` and `-` fold markers. The disclosure state adds schema props, parsing branches, interactive rendering, and a rich-editor control that are not needed for Nabu's current callout scope.

## Decision

**Nabu supports only non-collapsible `[!type]` callouts in the rich editor for now.**

- A plain marker with an optional title becomes an editable `calloutBlock`.
- Markers with `+` or `-` immediately after the closing bracket remain ordinary blockquotes and use the generic quote import/export path.
- The callout schema stores type and title only. Callout surfaces are always expanded and render no disclosure control.
- The right-opening slash submenu inserts only plain, non-collapsible markers.

## Consequences

- Callout parsing, rendering, and serialization have one state instead of an initial-fold-state branch.
- Vaults containing collapsible Obsidian syntax remain readable as ordinary blockquotes without Nabu claiming support for disclosure behavior.
- Fold support can return later through a new decision that defines editing, persistence, accessibility, and migration behavior explicitly.
- ADR-0160 remains in force for editable inline bodies, Markdown durability, aliases, custom types, and the centralized import/export boundary.
