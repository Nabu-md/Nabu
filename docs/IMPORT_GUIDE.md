# Import Guide

Bring your existing notes into Nabu from various sources.

## Supported Import Sources

- **Markdown** (.md, .markdown) — Standard markdown files
- **Obsidian** — vaults with wikilinks
- **Notion** — exported markdown
- **Evernote** — ENEX format
- **Plain Text** (.txt) — Simple text files

## Import Methods

### Method 1: Import Files

Import individual files or folders.

1. Go to **File** → **Import**
2. Select files or folder to import
3. Choose import strategy (see below)
4. Click **Import**

### Method 2: Drag & Drop

1. Drag files/folders from your file manager
2. Drop them into Nabu's file tree
3. Choose import strategy
4. Click **Import**

### Method 3: Quick Capture

For single notes:
1. Press `Cmd+Shift+Space` (macOS) or `Ctrl+Shift+Space` (Windows/Linux)
2. Paste content
3. Press Enter — goes to Inbox

---

## Import Strategies

Choose how Nabu handles duplicates:

### Skip

- **Behavior**: Don't import if a note with the same path already exists
- **Use when**: You want to preserve existing notes
- **Best for**: Re-importing after a failed import

### Overwrite

- **Behavior**: Replace existing notes with imported versions
- **Use when**: You want to update existing notes
- **Best for**: Syncing from another source

### Rename

- **Behavior**: Import with a unique name (adds number suffix)
- **Use when**: You want to keep both versions
- **Best for**: Merging multiple sources

---

## Source-Specific Guides

### Importing from Obsidian

Obsidian uses standard Markdown with wikilinks, making it fully compatible.

#### Steps

1. **Export from Obsidian** (optional):
   - Obsidian vaults are already folders of `.md` files
   - You can point Nabu directly to the folder
   - Or copy the vault folder

2. **Import to Nabu**:
   - File → Import
   - Select your Obsidian vault folder
   - Choose strategy: **Rename** (safest) or **Skip**
   - Click Import

#### Wikilinks

Obsidian wikilinks (`[[Note Title]]`) are preserved and work in Nabu.

#### Attachments

- Obsidian attachments (images, PDFs) are imported as regular files
- Paths in markdown are preserved
- Consider copying the entire vault for best results

#### Frontmatter

YAML frontmatter is preserved:
```yaml
---
tags: [work, project]
date: 2024-01-15
---
```

### Importing from Notion

Notion's export format is Markdown, but may need cleanup.

#### Steps

1. **Export from Notion**:
   - Open Notion
   - Go to Settings & Members → Export
   - Select "Markdown & CSV"
   - Click "Export"
   - Unzip the downloaded file

2. **Import to Nabu**:
   - File → Import
   - Select the unzipped folder
   - Choose strategy: **Rename** (recommended)
   - Click Import

#### Cleanup Tips

Notion exports may contain:
- **HTML in markdown**: Nabu's editor can handle this, or use Find/Replace
- **Embedded files**: Check the `Attachments` folder
- **Database exports**: May need manual restructuring

#### Limitations

- Notion databases become multiple markdown files
- Some complex Notion blocks may not render perfectly
- Properties become YAML frontmatter

### Importing from Evernote

Evernote exports in ENEX format (XML).

#### Steps

1. **Export from Evernote**:
   - Select notes/notebooks in Evernote
   - File → Export Notes
   - Choose `.enex` format
   - Save file

2. **Import to Nabu**:
   - File → Import
   - Select `.enex` file
   - Choose strategy: **Rename** (recommended)
   - Click Import

#### Conversion

Nabu converts Evernote notes to Markdown:

- **Rich text** → Markdown formatting
- **Checklists** → Markdown task lists (`- [ ]`)
- **Tags** → YAML frontmatter tags
- **Attachments** → Saved to `Attachments/` folder
- **Images** → Embedded markdown images

#### Limitations

- Complex Evernote tables may need manual fixes
- Some Evernote-specific features (reminders, locations) are not preserved
- Large exports may take time

### Importing from Apple Notes

Apple Notes doesn't have a direct export, but you can:

#### Method 1: Copy/Paste

1. Open Apple Notes
2. Select notes
3. Copy content
4. In Nabu: Quick Capture (`Cmd+Shift+Space`)
5. Paste content
6. Save as note

#### Method 2: Third-party Export

1. Use a tool like [Notes Exporter](https://github.com/threeplanetssoftware/notes_exporter)
2. Export to Markdown
3. Import to Nabu

### Importing from Roam Research

Roam uses standard Markdown with some custom syntax.

#### Steps

1. **Export from Roam**:
   - Settings → Export
   - Choose "Markdown"
   - Download zip

2. **Import to Nabu**:
   - Unzip Roam export
   - File → Import
   - Select folder
   - Choose strategy

#### Syntax Conversion

- Roam's `{{[[TODO]]}}` → standard markdown checklists
- Roam blocks → standard markdown
- Wikilinks (`[[ ]]`) are preserved

### Importing from Logseq

Logseq is already Markdown-based.

#### Steps

1. **Locate Logseq vault**:
   - Usually in `~/logseq/` or custom location
   - Files are `.md` with YAML frontmatter

2. **Import to Nabu**:
   - File → Import
   - Select Logseq vault folder
   - Choose strategy: **Rename** (recommended)
   - Click Import

#### Considerations

- Logseq pages become Nabu notes
- Journal entries become daily notes
- Tags and properties are preserved

---

## Import Best Practices

### Before Importing

1. **Backup**: Always backup your source data
2. **Organize**: Clean up source files if needed
3. **Test**: Start with a small subset
4. **Plan**: Choose the right strategy

### During Import

1. **Monitor**: Watch for errors or warnings
2. **Validate**: Check a few imported notes
3. **Adjust**: Pause and adjust strategy if needed

### After Importing

1. **Verify**: Check folder structure
2. **Search**: Test search functionality
3. **Graph**: View graph to ensure links work
4. **Tags**: Verify tags are recognized

---

## Handling Import Issues

### Duplicate Notes

**Problem**: Same note imported multiple times

**Solution**:
- Use **Skip** strategy on re-import
- Manually delete duplicates
- Use smart folders to find duplicates

### Broken Links

**Problem**: Wikilinks point to non-existent notes

**Solution**:
- Check note titles match exactly (case-sensitive)
- Use "Unlinked Mentions" panel to fix
- Search for broken links: `[[` in search

### Missing Attachments

**Problem**: Images/PDFs not showing

**Solution**:
- Ensure attachments folder was copied
- Check file paths in markdown
- Use relative paths: `./image.png` or `folder/image.png`

### Encoding Issues

**Problem**: Special characters display incorrectly

**Solution**:
- Ensure source files are UTF-8 encoded
- Use a text editor to fix encoding
- Re-import corrected files

---

## Import Settings

Configure in **Settings** → **Import & Export**:

### Default Import Strategy

Set your preferred default:
- **Skip** (safest)
- **Overwrite**
- **Rename**

### Duplicate Handling

- **Skip**: Don't import duplicates
- **Overwrite**: Replace duplicates
- **Rename**: Import with suffix (`Copy (1).md`)

---

## Batch Import Tips

### Large Imports

For importing 100+ notes:

1. **Start small**: Test with 10-20 notes first
2. **Use Rename**: Safest strategy for large imports
3. **Monitor progress**: Watch the Inbox for status
4. **Batch process**: Use Inbox to approve/reject in bulk

### Performance

- **Large files**: Split very large notes (>1MB)
- **Many files**: Import in batches of 50-100
- **Network drives**: Copy locally first, then import

---

## Exporting from Nabu

See [User Guide → Import & Export](USER_GUIDE.md#import--export) for export options.

---

## Getting Help

If you encounter issues:

1. Check [FAQ](FAQ.md) for common problems
2. Review import logs in `.nabu/logs/`
3. Open an issue on GitHub with:
   - Source format
   - Error messages
   - Sample file (if possible)

---

## Quick Reference

| Source | Format | Strategy | Notes |
|--------|--------|----------|-------|
| Obsidian | Folder of .md | Rename | Full compatibility |
| Notion | Markdown export | Rename | May need cleanup |
| Evernote | .enex | Rename | Auto-converts to MD |
| Apple Notes | Copy/paste | N/A | Manual process |
| Roam | Markdown export | Rename | Good compatibility |
| Logseq | Folder of .md | Rename | Direct import |