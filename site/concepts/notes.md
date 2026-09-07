# Notes

A note is a Markdown file with optional YAML frontmatter. Nabu reads the first H1 as the primary title and keeps the file on disk as the durable representation.

## Anatomy

```md
---
type: Project
status: Active
belongs_to:
  - "[[workspace]]"
---

# Launch Documentation

Draft the public Nabu docs and keep them close to code changes.
```

## Titles

The first H1 is the note title. Nabu uses that title wherever the note is displayed: note lists, search results, wikilink suggestions, relationship pickers, tabs, and window titles.

The title is separate from the filename. The filename stays visible in the breadcrumb so you can see the file on disk, and you can rename it independently when needed.

Use the breadcrumb action to rename the file to match the title. New untitled notes can also auto-rename from the first H1 the first time they get a real title. Turn this behavior off in Settings > Vault Content > Titles & Filenames if you prefer filenames to stay unchanged until you rename them manually.

## Body Links

Use `[[wikilinks]]` to connect notes from the body. Nabu shows autocomplete suggestions while you type, and links can resolve by filename or title.

## Frontmatter

Use frontmatter for structured fields such as type, status, date, URL, and relationships. Keep free-form thinking in the body.

Some notes can be displayed with specialized editors while keeping the same file-first model. A note with `_display: sheet` opens as a spreadsheet and stores its cells in a CSV-like body, while `type` remains available for organization. See [Spreadsheets](/concepts/spreadsheets).
