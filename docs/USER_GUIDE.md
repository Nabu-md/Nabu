# Nabu User Guide

Comprehensive guide to using Nabu for knowledge management.

## Table of Contents

1. [Vault Management](#vault-management)
2. [Note Editing](#note-editing)
3. [Search](#search)
4. [Knowledge Graph](#knowledge-graph)
5. [Templates](#templates)
6. [Import & Export](#import--export)
7. [Reading Queue](#reading-queue)
8. [Inbox](#inbox)
9. [Smart Folders](#smart-folders)
10. [Archive](#archive)
11. [OCR & PDF](#ocr--pdf)
12. [Voice Dictation](#voice-dictation)
13. [Settings](#settings)

---

## Vault Management

### Creating a Vault

A vault is your primary workspace — a folder containing all your notes.

1. **First launch**: The setup wizard guides you through vault creation
2. **Later**: Go to **File** → **Create Vault** or use the vault selector
3. **Choose location**: Select an empty folder or create a new one
4. **Name it**: Give it a meaningful name

### Opening an Existing Vault

1. Click the vault name in the toolbar
2. Select from recent vaults or click "Browse"
3. Select the vault folder

### Vault Structure

Nabu respects your folder organization:

```
My Vault/
├── .nabu/              # Metadata (indexes, graph data)
│   ├── queue/          # Background processing queue
│   └── settings.json   # App settings
├── Inbox/              # Quick captures
├── Projects/           # Your folders
│   ├── Project A/
│   └── Project B/
├── Templates/          # Reusable note templates
├── Archive/            # Archived notes (hidden from navigation)
└── Daily Notes/        # Date-based notes
```

> **Tip**: The `.nabu` folder is safe to exclude from sync services. Your notes are just regular files.

### Switching Vaults

- Click the vault name in the toolbar
- Select a different vault from recent vaults
- Or click "Browse" to open a new vault

---

## Note Editing

### Markdown Syntax

Nabu supports standard Markdown plus extensions:

#### Text Formatting

```markdown
# Heading 1
## Heading 2
### Heading 3

**bold text**
*italic text*
***bold italic***
~~strikethrough~~

`inline code`

> blockquote
```

#### Lists

```markdown
- Unordered list item
- Another item
  - Nested item

1. Ordered list item
2. Another item
   1. Nested ordered item

- [ ] Task (unchecked)
- [x] Task (checked)
```

#### Links

```markdown
[Link text](https://example.com)

[[Internal Note]]

[[Note Title|Display Text]]

[[Note#Section]]

![Image alt text](path/to/image.png)
```

#### Code Blocks

````markdown
```rust
fn main() {
    println!("Hello, world!");
}
```
````

#### Tables

```markdown
| Column 1 | Column 2 | Column 3 |
|----------|----------|----------|
| Cell 1   | Cell 2   | Cell 3   |
| Cell 4   | Cell 5   | Cell 6   |
```

### Editor Modes

#### Live Preview

- See rendered Markdown as you type
- Split view: edit on left, preview on right
- Click anywhere in preview to jump to that section

#### Source Markdown

- Pure Markdown editing
- Syntax highlighting
- No preview pane

### Editor Features

- **Auto-pair brackets**: Automatically closes `()`, `[]`, `{}`, `""`, `''`
- **Line numbers**: Toggle in Settings → Editor
- **Word wrap**: Wrap long lines or keep them horizontal
- **Spell check**: Built-in spell checking (toggle in Settings)
- **Auto-save**: Automatically saves every 30 seconds (configurable)

---

## Search

### Full-Text Search

Press `Cmd+Shift+F` (macOS) or `Ctrl+Shift+F` (Windows/Linux) to open search.

#### Search Features

- **Case-insensitive**: Finds "rust", "RUST", and "Rust"
- **Fuzzy matching**: Finds "rn" in "rust" (optional, enable in Settings)
- **Highlighting**: Shows matching text in results
- **Context snippets**: Shows surrounding text for each match

#### Search Scope

- Searches all `.md` files in your vault
- Searches note titles and content
- Includes archived notes

### Quick Switcher

Press `Cmd+P` to quickly jump to any note by title.

---

## Knowledge Graph

### Viewing the Graph

1. Open any note
2. Click the **Graph** tab in the right inspector
3. The graph shows:
   - **Nodes**: Your notes (circles)
   - **Edges**: Wikilinks between notes (lines)
   - **Folder nodes**: Optional hub nodes for folders

### Graph Interactions

- **Click a node**: Opens that note
- **Drag nodes**: Rearrange the layout
- **Scroll**: Zoom in/out
- **Right-click**: Context menu

### Graph Settings

Configure in **Settings** → **Graph**:

- **Include folders**: Show folders as hub nodes
- **Gravity**: How strongly nodes attract each other
- **Spacing**: Distance between connected nodes
- **Tag badges**: Show tags on graph nodes

---

## Templates

### Creating Templates

1. Go to **Settings** → **Templates**
2. Click **"New Template"**
3. Fill in:
   - **Name**: Template identifier
   - **Description**: What it's for
   - **Body**: Template content
   - **Frontmatter Defaults**: YAML metadata
4. Save

### Template Example

```markdown
---
type: meeting
date: {{date}}
attendees: []
---

# Meeting Notes

**Date**: {{date}}
**Attendees**: {{attendees}}

## Agenda

1. 
2. 
3. 

## Action Items

- [ ] 
- [ ] 

## Notes

```

### Using Templates

1. Create a new note
2. Click the template icon in the toolbar
3. Select a template
4. The note is populated

---

## Import & Export

### Exporting Notes

1. Right-click a note (or select multiple)
2. Choose **Export**
3. Select format:
   - **Markdown** (.md)
   - **HTML** (.html)
   - **PDF** (.pdf)
   - **Plain Text** (.txt)
   - **JSON** (.json)

4. Choose destination folder
5. Click Export

### Importing Notes

Supported formats:

- **Markdown** (.md, .markdown)
- **Obsidian** (.md with wikilinks)
- **Notion** (exported Markdown)
- **Evernote** (ENEX → Markdown)
- **Plain Text** (.txt)

#### Import Process

1. Go to **File** → **Import**
2. Select files or folder
3. Choose import strategy:
   - **Skip**: Don't import if note exists
   - **Overwrite**: Replace existing note
   - **Rename**: Import with unique name
4. Click Import

---

## Reading Queue

Organize notes you're actively reading:

### Adding to Queue

1. Right-click a note
2. Select **Add to Reading Queue**
3. Set priority: Low, Normal, High

### Managing Queue

Access via left sidebar:

- **Status**: Unread → Reading → Completed → Archived
- **Progress**: Track how far you've read (0-100%)
- **Priority**: High-priority notes appear first

### Queue Actions

- **Set status**: Right-click → Mark as Reading/Completed
- **Set progress**: Slider in inspector
- **Archive completed**: Bulk action to archive finished notes

---

## Inbox

The Inbox is for quick captures and imported notes:

### Quick Capture

1. Press `Cmd+Shift+Space` (macOS) or `Ctrl+Shift+Space` (Windows/Linux)
2. Enter title and content
3. Press Enter — note goes to Inbox with status "Pending"

### Processing Inbox Items

1. Click **Inbox** in left sidebar
2. Review items
3. Actions:
   - **Approve**: Move to vault
   - **Reject**: Delete with reason
   - **Retry**: Reprocess
   - **Edit metadata**: Update title, tags, etc.
   - **Move**: Assign to folder

### Inbox Statuses

- **Pending**: Awaiting review
- **Processing**: Being processed (OCR, etc.)
- **Ready**: Processed, awaiting approval
- **Approved**: Moved to vault
- **Rejected**: Discarded
- **Failed**: Processing failed

---

## Smart Folders

Create dynamic, query-based folders:

### Creating a Smart Folder

1. Go to **Settings** → **Smart Folders**
2. Click **"New Smart Folder"**
3. Enter a query:

#### Query Syntax

```
tag:work              # Notes tagged with "work"
folder:Projects       # Notes in Projects folder
date:2024-01-15       # Notes dated 2024-01-15
before:2024-01-01     # Notes before 2024-01-01
after:2024-01-01      # Notes after 2024-01-01
meeting               # Full-text search for "meeting"
```

Combine queries (AND logic):

```
tag:work folder:Projects meeting
```

4. Save the smart folder
5. It appears in the sidebar

---

## Archive

Archive notes you want to hide but keep:

### Archiving

1. Right-click a note
2. Select **Archive**
3. Note moves to `archive/` folder
4. Still searchable, hidden from navigation

### Viewing Archived Notes

1. Go to **Archive** view (or click Archive in sidebar)
2. See all archived notes with original locations
3. Can restore or permanently delete

### Restoring

1. Open Archive view
2. Click **Restore** on a note
3. Note returns to original location

---

## OCR & PDF

### OCR Settings

Configure in **Settings** → **OCR**:

- **Language**: English, Spanish, French, German, Japanese
- **Auto-process**: Automatically OCR scanned PDFs
- **Confidence threshold**: Minimum confidence to accept text (0.0-1.0)

### Processing Scanned PDFs

1. Import or save a scanned PDF
2. Nabu detects it's scanned
3. OCR runs automatically (if enabled)
4. Extracted text is indexed and searchable
5. View OCR confidence in the PDF viewer

### PDF Viewer

- View PDFs inline
- See OCR'd text overlay
. Navigate pages
. Search within PDF

---

## Voice Dictation

### Starting Dictation

1. Press `Cmd+Shift+D` (macOS) or `Ctrl+Shift+D` (Windows/Linux)
2. The Dictation Pill appears
3. Start speaking
4. Press the hotkey again to stop

### Dictation Settings

- **Model**: Choose Whisper model (tiny, base, small)
- **Auto-format**: Remove filler words (um, uh, etc.)

### Dictation Pill

- Floating pill window
- Shows recording status
- Can be moved anywhere on screen
- Shows live transcription

---

## Settings

### Accessing Settings

Click the gear icon (⚙️) in the toolbar.

### Settings Sections

1. **Appearance**: Theme, opacity, font size
2. **Editor**: Editing mode, line numbers, auto-save
3. **Markdown**: GFM, math, diagrams
4. **Search**: Indexing, fuzzy matching
5. **Graph**: Physics, folder nodes
6. **Files & Vaults**: Paths, trash retention
7. **Import & Export**: Formats, duplicate strategy
8. **OCR**: Language, confidence threshold
9. **Accessibility**: Screen reader, keyboard navigation
10. **Performance**: Worker pool, undo history
11. **Privacy**: Launch at startup, analytics, crash reporting
12. **Keyboard Shortcuts**: Customize hotkeys
13. **Advanced**: Debug mode, developer tools
14. **Experimental**: AI features, semantic search

### Resetting Settings

1. Go to **Settings** → **Advanced**
2. Click **"Reset to Defaults"**
3. Confirm

---

## Keyboard Shortcuts

See [KEYBOARD_SHORTCUTS.md](KEYBOARD_SHORTCUTS.md) for the complete reference.

---

## Troubleshooting

See [FAQ.md](FAQ.md) for common issues and solutions.

---

## Privacy & Security

See [PRIVACY.md](PRIVACY.md) for information about data handling, telemetry, and crash reporting.