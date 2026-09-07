# Use Media Previews

Media previews let you inspect vault files without leaving Nabu.

## Open A File

Select an image, PDF, media file, HTML file, or unsupported file from a folder or file list. Nabu opens supported files in the app and offers an external-open action for files that should use the system default app.

Standalone HTML files open as sanitized previews. Toggle raw mode to edit their source, and use the external-open action when a page needs scripts, forms, remote resources, or other browser behavior that the safe preview intentionally disables.

## All Notes Visibility

Open Settings to choose whether non-Markdown files appear in All Notes:

- PDFs.
- Images.
- Unsupported files.

Folder browsing still shows files in their folders even when a category is hidden from All Notes.

## Attachments

When you paste or drop an image into a note, Nabu copies it into the vault and references the copied file from Markdown.

When you paste a selection from a web page, Nabu also tries to import public `http` and `https` images into the vault. The text is pasted immediately while image imports finish in the background. Successful imports become portable `attachments/...` references; failed imports remain remote and produce a non-blocking message.

## Troubleshooting

If a preview does not render, open the file in the default app to confirm the file is valid, then check whether the file is inside the active vault and not blocked by operating-system permissions.

If a pasted web image stays remote, the host may have rejected the download, the response may not be a supported image, or the URL may have failed Nabu's local-network and size safety checks.
