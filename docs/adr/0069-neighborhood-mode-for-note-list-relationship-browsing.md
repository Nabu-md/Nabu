---
type: ADR
id: "0069"
title: "Neighborhood mode for note-list relationship browsing"
status: active
date: 2026-04-19
---

## Context

Nabu already had a relationship-browsing state behind `SidebarSelection.kind === 'entity'`, but the product language and interaction model were still fuzzy. The pinned source note rendered as a special card instead of a normal note row, grouped relationship results were deduplicated across sections, and Cmd-click behaved like a legacy "open separately" affordance rather than a clear graph-navigation action.

The new note-list flow needed an explicit product concept for browsing related notes around a source note, plus keyboard semantics that matched the mouse flow. The team also wanted the list to preserve graph truth instead of collapsing overlapping relationships away when a note legitimately belonged to multiple groups.

## Decision

**Nabu formalizes `SidebarSelection.kind === 'entity'` as Neighborhood mode.** The note list now treats the selected note as the neighborhood source, pins it at the top using the standard active note-row styling, shows outgoing relationship groups first and inverse/backlink groups after, keeps empty groups visible with count `0`, and allows the same note to appear in multiple groups when multiple relationships are true.

**Neighborhood navigation is a distinct pivot action.** Plain click and plain `Enter` open the focused note without replacing the current neighborhood. Cmd/Ctrl-click and Cmd/Ctrl-`Enter` open the note and pivot the note list into that note's Neighborhood.

## Options considered

- **Reuse the existing `entity` selection as Neighborhood mode** (chosen): keeps the state model localized, avoids a second nearly-identical note-list mode, and lets sidebar navigation exit Neighborhood by selecting any other sidebar target. Cons: code still uses the historical `entity` name internally.
- **Add a new `neighborhood` selection variant**: clearer internal naming, but it duplicates the same source-note payload and would force wider selection-handling churn across the app for little product gain.
- **Keep the old implicit entity-browsing behavior**: lowest short-term engineering effort, but it leaves the product terminology inconsistent and preserves interaction mismatches like deduped groups and non-pivot Cmd-click behavior.

## Consequences

- Product, tests, and docs now refer to Neighborhood as a first-class note-list browsing mode.
- The note list preserves overlapping graph evidence: one note can appear in multiple groups when multiple relationships are true.
- Keyboard-only browsing now matches the pointer flow: arrow keys/open keep the current neighborhood, while Cmd/Ctrl-`Enter` pivots it.
- Sidebar navigation remains the exit path from Neighborhood because the app still models the mode through the existing selection union.
- Internal code still uses the `entity` discriminator, so future refactors should treat "entity selection" and "Neighborhood mode" as the same concept unless a broader navigation redesign justifies a new selection shape.
