# Getting Started with Nabu

Welcome to **Nabu** — your premium markdown knowledge management system. This guide will help you set up and start using Nabu in just a few minutes.

## Table of Contents

1. [Installation](#installation)
2. [First Launch](#first-launch)
3. [Creating Your First Vault](#creating-your-first-vault)
4. [Understanding the Interface](#understanding-the-interface)
5. [Creating Your First Note](#creating-your-first-note)
6. [Linking Notes](#linking-notes)
7. [Using Templates](#using-templates)
8. [Next Steps](#next-steps)

---

## Installation

### macOS

1. Download `Nabu-0.1.0.dmg` from the releases page
2. Double-click the DMG file
3. Drag Nabu to your Applications folder
4. On first launch, right-click and select "Open" to bypass Gatekeeper (if needed)
5. Nabu will automatically register `.md` and `.markdown` file associations

### Windows

1. Download `Nabu-0.1.0.exe` from the releases page
2. Run the installer
3. Follow the installation wizard
4. Nabu will be added to your Start Menu and taskbar
5. File associations for `.md` files are created automatically

### Linux

#### AppImage
1. Download `Nabu-0.1.0.AppImage`
2. Make it executable: `chmod +x Nabu-0.1.0.AppImage`
3. Run it: `./Nabu-0.1.0.AppImage`

#### Desktop Entry (optional)
```bash
nabu --install-desktop-entry
```

This installs a `.desktop` file to `~/.local/share/applications/` for integration with your desktop environment.

---

## First Launch

When you first open Nabu, you'll see the **Vault Setup Wizard**:

1. **Welcome Screen** — Brief introduction to Nabu
2. **Choose Vault Location** — Select where to store your notes
   - Click "Browse" to select a folder
   - Or choose "Create New Vault" for a fresh start
3. **Complete Setup** — Click "Finish" to enter the main app

> **Tip**: Your vault is just a regular folder on your computer. You can access it with any file manager or text editor at any time.

---

## Creating Your First Vault

A **vault** is simply a folder that contains your markdown notes. Nabu works with any folder, but we recommend:

### Best Practices

- **Location**: Store your vault in a sync-enabled folder (Dropbox, iCloud Drive, Google Drive, etc.) if you want multi-device access
- **Name**: Give it a descriptive name like `Knowledge Base`, `Notes`, or `Second Brain`
- **Structure**: You can organize with subfolders — Nabu respects your folder hierarchy

### Creating a Vault

1. Click **"Create New Vault"** in the welcome wizard
2. Choose a location and name
3. Click **"Select"** or **"Create"**
4. Nabu initializes the vault (creates `.nabu` metadata folder)

> **Note**: The `.nabu` folder stores search indexes, graph data, and other metadata. It's safe to exclude it from your sync service if desired.

---

## Understanding the Interface

Nabu's interface is designed for productivity:

### Layout

```
┌──────────────────────────────────────────────────────────────┐
│  ◀ ▶ ≡  Nabu                                    ⌕ 🔍 ⚙  │  ← Toolbar
├──────────┬───────────────────────────────────┬───────────────┤
│          │                                   │               │
│  Left    │      Main Content Area            │   Right       │
│  Sidebar │                                   │   Inspector    │
│          │                                   │               │
│  📁 Tree │    Your note content here         │  📊 Graph     │
│  📋 Inbox│                                   │  🔗 Links     │
│  📚 Queue│                                   │  🏷 Tags       │
│          │                                   │               │
├──────────┴───────────────────────────────────┴───────────────┤
│  Status Bar                                                   │
└──────────────────────────────────────────────────────────────┘
```

### Left Sidebar

- **File Tree**: Browse your vault structure
  - Click folders to expand/collapse
  - Click notes to open
  - Right-click for context menu
- **Inbox**: Captured notes awaiting organization
- **Reading Queue**: Notes you're actively reading

### Main Content Area

- **Note Editor**: Write in Markdown with live preview
- **Tab Bar**: Multiple notes open simultaneously
- **Toolbar**: Note-level actions (archive, delete, etc.)

### Right Inspector

- **Graph View**: Visualize connections between notes
- **Backlinks**: See which notes link to the current note
- **Outgoing Links**: See where this note links to
- **Unlinked Mentions**: Notes that mention the current title
- **Tags**: Frontmatter tags

---

## Creating Your First Note

### Method 1: From the File Tree

1. Right-click in the file tree
2. Select **"New Note"**
3. Enter a filename (e.g., `Getting Started.md`)
4. Start writing in Markdown

### Method 2: Quick Capture

1. Press `Cmd+Shift+Space` (macOS) or `Ctrl+Shift+Space` (Windows/Linux)
2. Enter a title and content
3. Press Enter — note goes to your Inbox

### Method 3: Daily Note

1. Click the calendar icon or press the daily note hotkey
2. Nabu creates `YYYY-MM-DD.md` if it doesn't exist
3. Perfect for journaling or daily logs

---

## Linking Notes

Nabu uses **wikilinks** (`[[Note Title]]`) to connect notes:

### Creating Links

1. Type `[[` in the editor
2. Start typing a note title
3. Select from the autocomplete dropdown
4. Press Enter or Tab to insert

### Link Examples

```markdown
# My Project Notes

See also: [[Meeting Notes]]

## Resources
- [[Design Mockups]]
- [[API Documentation]]

## Related
- [[Project Timeline#Phase 1]]
- [[Project Timeline#Phase 2]]
```

### Link Types

- **Internal**: `[[Note Title]]` — links to another note
- **With alias**: `[[Note Title|display text]]`
- **Heading anchor**: `[[Note Title#Heading]]`
- **Block anchor**: `[[Note Title^block-id]]`
- **External**: `[https://example.com](https://example.com)`

---

## Using Templates

Templates help you create consistent notes:

### Creating a Template

1. Go to **Settings** → **Templates** (or use the template picker)
2. Click **"New Template"**
3. Enter:
   - **Name**: e.g., "Meeting Notes"
   - **Description**: What it's for
   - **Body**: The template content
   - **Frontmatter Defaults**: Pre-populated YAML
4. Save

### Using a Template

1. Create a new note
2. Click the template icon in the toolbar
3. Select your template
4. The note is populated with the template content

---

## Next Steps

Now that you're up and running:

1. **Read the [User Guide](USER_GUIDE.md)** for detailed feature explanations
2. **Check [Keyboard Shortcuts](KEYBOARD_SHORTCUTS.md)** to boost productivity
3. **Explore [Import Guide](IMPORT_GUIDE.md)** to bring in existing notes
4. **Review [Privacy & Security](PRIVACY.md)** to understand data handling
5. **Visit [FAQ & Troubleshooting](FAQ.md)** if you encounter issues

---

## Quick Reference

### Essential Shortcuts

| Action | macOS | Windows/Linux |
|--------|-------|---------------|
| New Note | `Cmd+N` | `Ctrl+N` |
| Quick Capture | `Cmd+Shift+Space` | `Ctrl+Shift+Space` |
| Toggle Sidebar | `Cmd+B` | `Ctrl+B` |
| Save | `Cmd+S` | `Ctrl+S` |
| Search | `Cmd+Shift+F` | `Ctrl+Shift+F` |
| Bold | `Cmd+B` | `Ctrl+B` |
| Italic | `Cmd+I` | `Ctrl+I` |
| Link | `Cmd+K` | `Ctrl+K` |
| Voice Dictation | `Cmd+Shift+D` | `Ctrl+Shift+D` |

### Common Tasks

- **Organize notes**: Drag-and-drop in the file tree
- **Tag notes**: Use YAML frontmatter (`tags: [work, important]`)
- **Search**: Press `Cmd+Shift+F` for full-text search
- **Graph view**: Click the Graph tab in the right inspector
- **Archive**: Right-click a note → Archive
- **Settings**: Click the gear icon (⚙️) in the toolbar

---

## Getting Help

- **Documentation**: [Full documentation](https://docs.nabu.md)
- **Issues**: [GitHub Issues](https://github.com/farolabs/nabu/issues)
- **Community**: [Discord/Slack/Forum]
- **Email**: support@farolabs.com

---

**Welcome to Nabu!** Start building your second brain today. 🧠