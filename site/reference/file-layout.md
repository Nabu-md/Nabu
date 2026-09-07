# File Layout

Nabu is not opinionated about folder structure. It finds notes recursively across the whole vault, stores new notes in the root by default, and uses types and relationships for real organization.

```txt
my-vault/
  project-alpha.md
  weekly-review.md
  research/
    source-notes.md
  attachments/
    diagram.png
    source.pdf
  project.md
  person.md
  views/
    active-projects.yml
```

## Root Notes

Nabu works well with a flat vault. Folders are optional and can be useful for compatibility with other tools, but they are not required for people, projects, topics, or any other note category.

Type is not inferred from folder location. It comes from frontmatter, and relationships are expressed with wikilinks in fields. That is what Nabu uses for the sidebar, Properties panel, search, custom views, and neighborhood navigation.

## Special Folders

| Folder | Purpose |
| --- | --- |
| `views/` | Saved custom views. |
| `attachments/` | Images and other attached files. |

PDFs, images, and other non-Markdown files stay as normal files. Folder browsing can show them in place, and Settings controls whether PDFs, images, and unsupported files appear in All Notes.

Whiteboards are Markdown files with durable tldraw data, so they belong with notes rather than in `attachments/`.

Spreadsheets are also Markdown files. A note with `_display: sheet` stores ordinary frontmatter plus a CSV-like body and opens in the sheet editor.

Type definitions are Markdown notes with `type: Type` in frontmatter. New type documents are normal notes, and existing type documents in older folders still work.

## Git Files

If the vault is a Git repository, `.git/` belongs to Git. Nabu reads Git state but does not treat `.git/` as notes.
