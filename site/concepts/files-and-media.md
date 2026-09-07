# Files And Media

Nabu starts with Markdown notes, but a vault can also contain images, PDFs, media files, whiteboards, and other local files.

## Mermaid Diagrams

Use Mermaid code blocks when a note needs a diagram that should stay plain text and versionable.

````md
```mermaid
flowchart LR
  Idea --> Draft --> Review --> Publish
```
````

Nabu renders Mermaid diagrams in the editor while keeping the source in Markdown.

## Attachments

Images pasted into the editor are saved into the vault as normal files. They remain portable and can be opened by other tools.

When pasted web content contains eligible remote images, Nabu imports those images into `attachments/` in the background and rewrites the pasted references to local paths. Text appears immediately. If an image cannot be imported safely, its original remote reference remains editable instead of blocking the paste.

## Previews

Nabu can preview common image files, PDFs, and supported media files in the app. Files without an in-app preview can still be opened in the default system app.

Settings control whether PDFs, images, and unsupported files appear in All Notes. Folder browsing still shows files in their folders.

## HTML Files

Standalone `.html` and `.htm` files can open in a sanitized in-app preview. Local images, styles, fonts, and media work when their paths stay inside the vault.

Preview mode removes scripts, forms, nested frames, and remote resources. Use raw mode to edit the source, or open the file in the default browser when it needs interactive browser behavior.

## Whiteboards

Whiteboards use tldraw in the editor, but their durable representation stays in Markdown. That keeps them inside the vault and versioned by Git with the rest of your notes.

## Git Boundary

If generated or local-only files are ignored by Git, Nabu can hide them from notes, search, quick open, and folders. Use this when build artifacts or private local files should not behave like vault content.
