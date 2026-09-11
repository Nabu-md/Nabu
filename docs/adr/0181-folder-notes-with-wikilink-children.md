# 0181. Folder notes (`type: Folder`) render as tree folders with linked children

Date: 2026-09-10

## Status

Accepted

## Context

The Notion-parity plan asked for "folders as MD files with pages listed via
wikilinks" — a note that acts as both a folder and a note, containing
wikilinks to its children (Obsidian's "folder notes" concept). Nabu's sidebar
already renders real filesystem folders (`FolderNode[]`) plus the notes inside
them (`entriesForFolderNode`), and the wikilink graph already resolves note
targets through `resolveEntry`.

Two designs were considered:

1. **Derive the tree from folder notes** — a `type: Folder` note would replace
   the filesystem folder and the tree would list its wikilinked children.
   This breaks the existing filesystem-folder contract (`create_vault_folder`,
   drag-and-drop moves, context menus) and makes the tree undefined when a
   folder has no note.
2. **Folder notes as a rendering layer on top of real folders** — a note with
   `type: Folder` keeps living wherever the user saved it; when it appears in
   a folder's file listing, it renders as a folder-like row whose children are
   the notes its wikilinks resolve to. Opening the row expands the linked
   children; double click (or the context menu) opens the note itself.

## Decision

Option 2. A folder note is purely a frontmatter convention:

- `type: Folder` marks a note as a folder note (`isFolderNote`).
- `describeFolderNote` resolves the note's `outgoingLinks` to entries in link
  order, deduplicated, excluding the note itself.
- `FolderTree` renders such rows via `FolderNoteRow` with a caret, folder
  icon, and child count; expanding shows the linked children as ordinary
  file rows.

Wikilinks (not folder containment) define the children, so a folder note can
link notes across folders while staying portable to any Markdown editor.

## Consequences

- No backend changes: `type` is an ordinary frontmatter key and
  `outgoingLinks` are already extracted from the note body.
- Filesystem folder semantics are untouched; folder notes can never shadow or
  replace a real folder.
- A folder note with no resolvable links renders as a plain file row (no
  empty folder affordance).
- Future work: creation flow that scaffolds a `type: Folder` note together
  with a real folder, and moving child notes by drag-drop onto the row.
