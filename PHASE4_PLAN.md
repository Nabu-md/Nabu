# Phase 4: Nabu Polish & Rebrand Plan

## Overview
Distance Nabu further from its Tolia origins by:
1. Simplifying the sidebar to a pure folder tree (remove Inbox, All Notes, Archive, Views, Types)
2. Making the dictation pill a separate top-level floating window
3. Adding a chat thread UI to the AI panel
4. Continuing the Tolia → Nabu rename purge

---

## Feature 1: Simplified Folder-Tree Sidebar

### Current State
The sidebar (`Sidebar.tsx`, 708 lines) has these sections:
- **SidebarTopNav** (`SidebarTopNav.tsx`): Inbox nav, search, all-notes, archive links
- **FavoritesSection**: Starred notes
- **SidebarViewsNavigation**: Saved views (ViewsSection)
- **TypesSection**: Note types grouping
- **FolderTree** (`FolderTree.tsx`, 321 lines): Already exists but nested inside the full sidebar

The App.tsx passes `showInbox`, `inboxCount`, `allNotesFileVisibility` props and renders `Sidebar` + `NoteList` as separate components.

### Plan
**Goal**: Replace the entire sidebar with just a folder tree + search/filter at the top. When you click a folder, show its notes below.

#### 4.1.1: Create `NabuSidebar.tsx`
- New component that replaces `Sidebar.tsx` + `NoteList.tsx` for the folder view
- Top section: Search input + filter pills (same as existing `NoteList` filters)
- Main section: `FolderTree` component (already exists at `src/components/FolderTree.tsx`)
- When a folder is selected: render notes inside that folder as a flat list below the tree
- No Inbox, All Notes, Archive, Views, or Types sections
- Props: `vaultPath`, `entries`, `onNoteSelect`, `onFolderSelect`

#### 4.1.2: Create `NabuSidebarNoteList.tsx`
- Renders notes under the selected folder
- Reuses existing `NoteList` filtering logic but scoped to the selected folder
- Search input at the top filters the current folder's notes
- File count + total size in the footer

#### 4.1.3: Modify `App.tsx`
- Replace `<Sidebar>` + `<NoteList>` with `<NabuSidebar>`
- Remove `showInbox`, `inboxCount`, `allNotesFileVisibility` logic
- Remove `SidebarViewItem`, `ViewsSection`, `TypesSection` imports
- Keep search functionality but scope it to the folder tree view

#### 4.1.4: Remove dead code
- Delete `SidebarTypesSection.tsx`, `SidebarViewItem.tsx`, `FavoritesSection.tsx` (optional — can keep Favorites as a "Starred" folder)
- Remove `SidebarViewsNavigation` component
- Clean up `SidebarTopNav.tsx` (remove Inbox/all-notes/archive buttons)
- Remove `useInboxOrganizeAdvance` hook usage from App.tsx

---

## Feature 2: Dictation Pill as Standalone Window

### Current State
The dictation pill (`DictationPill.tsx`) is mounted inside the main App.tsx as an overlay (`position: fixed; bottom: 20px`). It has:
- Opacity loading (configurable via `opacity` prop)
- Clipboard cache panel (via `useClipboardCache` hook → `get_recent_clipboard_entries` IPC)
- File drop zone (via `useFileDropZone` hook → `capture_file_drop` IPC)
- Web Speech API transcription (via `useSpeechRecognition` hook)
- Toggle shortcut: Cmd+Shift+D

### Plan
**Goal**: Move the dictation pill to its own Tauri window that floats above the main app.

#### 4.2.1: Backend — Dictation Window
Add to `src-tauri/src/commands/dictation.rs`:
- `open_dictation_window() -> Result<String, String>` — spawns a new `WebviewWindow` with a lightweight route
- Register the window label: `dictation-pill`

Mirror the pattern from `open_mini_app_window`:
```rust
let label = "dictation-pill";
WebviewWindowBuilder::new(&app_handle, label, WebviewUrl::App("/?window=dictation-pill".into()))
    .title("Dictation")
    .inner_size(360.0, 320.0)
    .resizable(false)
    .minimizable(false)
    .maximizable(false)
    .decorations(false)  // frameless, custom chrome
    .always_on_top(true)
    .build()?;
```

Add `decorations: false` + custom drag region in the frontend (like `App.tsx` line 187-189 error overlay).

Add to `lib.rs` IPC handler registration.

#### 4.2.2: Frontend — Dictation Window Route
In `App.tsx`, add a route check (mirroring `windowMode.ts` patterns):
```tsx
if (isDictationWindow()) {
  return <DictationWindowApp />
}
```

Create `src/DictationWindowApp.tsx`:
- Renders `<DictationPill>` without the toggle button (the window itself IS the pill)
- Window has no title bar (decorations: false)
- Drag region at top (like the main app's traffic light area)
- Close button in top-right corner

#### 4.2.3: Frontend — Move DictationPill Mount
- Remove `<DictationPill>` from the main App.tsx render
- Add a menu item or keyboard shortcut to open the dictation window
- Keep Cmd+Shift+D as the shortcut to open the dictation window

---

## Feature 3: Chat Thread in AI Panel

### Current State
- `AiPanel.tsx` (334 lines) — uses `AiPanelController` from `useAiPanelController`
- `AiPanelMessageHistory` (`AiPanelChrome.tsx`) — renders messages as a flat list
- No concept of "conversations" or "threads" within a single panel session
- `AiWorkspace.tsx` (1099 lines) has the workspace window with `useAiWorkspacePublishedContext` but that's a separate window

### Plan
**Goal**: Add conversation threading so users can see a list of chat threads on the left, select one to view its messages on the right.

#### 4.3.1: Backend — Conversation Storage
Create `src-tauri/src/commands/conversations.rs`:
- `list_conversations(vault_path) -> Vec<Conversation>` — returns saved chat threads
- `save_conversation(vault_path, conversation) -> Result<(), String>` — persists a thread
- `create_conversation(vault_path, title?) -> String` — creates a new empty thread

Conversation storage format (`.app-data/conversations/{id}.json`):
```json
{
  "id": "uuid",
  "title": "Chat about X",
  "created_at": "2025-01-01T00:00:00Z",
  "updated_at": "2025-01-01T00:00:00Z",
  "messages": [...]
}
```

Register IPC commands in `commands/mod.rs` + `lib.rs`.

#### 4.3.2: Frontend — Conversation Hook
Create `src/hooks/useAiConversations.ts`:
- `conversations` — list of saved threads
- `activeConversationId` — currently selected thread
- `createConversation()`, `selectConversation(id)`, `saveCurrentConversation()`
- Persists via IPC, falls back to localStorage in browser mode

#### 4.3.3: UI — Conversation Sidebar
In `AiPanelView` (or `AiPanel.tsx`), add a collapsible left sidebar:
- Shows conversation list (newest first)
- "New chat" button at the top
- Each item shows title + last message + timestamp
- Active conversation highlighted
- Clicking loads messages into the existing message history

#### 4.3.4: Integration
- When user sends a message, auto-save after each exchange
- Auto-generate conversation title from first message (truncate to 50 chars)
- "Clear conversation" becomes "New chat" (creates a fresh thread)

---

## Implementation Order (Phase 4)
1. **Folder-tree sidebar** (highest impact, removes confusion) — ~2-3 days
2. **Chat threads in AI panel** (high value, medium effort) — ~2-3 days
3. **Dictation pill as separate window** (medium value, medium effort) — ~1-2 days
4. **Dead code purge** (cleanup after features land) — ~1 day

---

## Scope Boundaries
- **Not changing**: Editor component, graph view, settings panel, vault management
- **Keeping**: FolderTree.tsx (existing), NoteList filtering logic, search bar
- **Removing**: Inbox system entirely (keep the data model for backward compat but hide all UI)
