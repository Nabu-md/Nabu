# Audit 0.6 — UI System & Component Architecture

## 1. Executive Summary

Nabu's frontend is a **Leptos 0.7.8 CSR (Client-Side Rendering) WASM application** compiled as a `cdylib` in a standalone workspace at `crates/nabu-ui/`. It is embedded into a Tauri desktop shell via `wasm-bindgen` and communicates with the Rust backend through a single IPC bridge: `crate::ipc::tauri_invoke()`.

**Key findings:**

- **Component model:** The entire UI is a single-rooted reactive component tree rooted at `App` (`components/app.rs:78`), which is mounted via `leptos::mount_to_body` in `lib.rs:16`. There is no filesystem-based router — navigation is driven by a `ViewMode` enum held in a `NavContext` signal.
- **State architecture:** Three layered context providers at the root (`ThemeContext`, `ToastContext`, `WorkspaceContext`, `NavContext`, `SaveStatusContext`, `TaskContext`, `HistoryContext`) provide all shared state. Components capture these contexts at render time and thread them into `spawn_local` async blocks as plain `Copy` values.
- **Routing:** Signal-driven routing via `nav.view_mode.get()` match in `App` (`app.rs:357-469`). No lazy loading. All 16 view modes are statically compiled into the match.
- **Shared component library:** 9 modules under `components/ui/` providing 40+ reusable primitives. Centralized icon system (`Icon` enum + `render_icon` dispatch table at `icons.rs:543-719`). Design tokens in `src/styles/app.css` using CSS custom properties + Tailwind.
- **Backend integration:** 40+ `tauri_invoke` call sites across 15 components. All follow the pattern: capture context signal → `spawn_local(async { tauri_invoke(...) → serde_json::from_value })`.
- **Dead code:** 6 components (5 files) are defined but never rendered anywhere in the component tree. 1 module (`tree.rs`) is exported from `lib.rs` but never referenced by any component.

---

## 2. Frontend Initialization Flow

### 2.1 Native Entry → WASM Mount

**Evidence:**

- `crates/nabu-ui/src/lib.rs:12-23` — `#[wasm_bindgen(start)] pub fn start()` is the WASM entry point:
  1. Installs `console_error_panic_hook` for panic messages
  2. Calls `remove_boot_splash()` to remove the `boot-splash` div from `index.html`
  3. Mounts `App` via `leptos::mount_to_body`
  4. Wraps `App` in `<ToastProvider>` (provides `ToastContext`)

**Initialization sequence in `App` component:**

```
lib.rs::start()
  └─ mount_to_body
      └─ ToastProvider
          └─ App  (components/app.rs:78)
              ├─ provide_theme("dark")           → ThemeContext (lib.rs:43)
              ├─ provide_history()               → HistoryContext (history.rs)
              ├─ provide_save_status()           → SaveStatusContext (recovery/save_status.rs:61)
              ├─ provide_tasks()                 → TaskContext (ui/feedback.rs:764)
              ├─ provide_workspace()             → WorkspaceContext (workspace.rs:48)
              ├─ provide_navigation()            → NavContext (navigation/state.rs:214)
              ├─ load_all_nav_state(nav)         → async IPC: settings_get for recents, favourites, etc.
              ├─ load_notes_index(nav)           → async IPC: notes_index
              ├─ spawn_local(recovery_check)     → async IPC: recovery_check → pending_recovery signal
              └─ spawn_local(check_vault_exists) → async IPC: check_vault_exists → AppScreen::MainDashboard or VaultSetup
```

**AppScreen state machine** (`app.rs:41-46`):
- `Loading` → initial state
- `VaultSetup` → shown when `check_vault_exists` IPC returns no path
- `MainDashboard` → shown when a vault path exists

**Context capture pattern:** All contexts are `Copy` types (e.g., `WorkspaceContext` at `workspace.rs:29`, `NavContext` at `navigation/state.rs:175`). Components call `use_nav()` / `use_workspace()` at render time and store the result in a local variable, which is then moved into `spawn_local` async blocks and DOM event closures. This avoids `expect_context()` panics in async contexts (documented at `navigation/smart_folders.rs:15-19`).

---

## 3. Layout Architecture Diagram

```
App (app.rs:78)
├── Loading Screen (ui/feedback::LoadingBlock)
│   └── Skeleton rows
├── VaultSetup Wizard (vault_setup_wizard.rs)
└── MainDashboard (app.rs:313)
    ├── RecoveryBanner (recovery/recovery_banner.rs:16)
    ├── RibbonBar (layout/ribbon_bar.rs:16)        ← fixed left edge
    ├── LeftSidebar (layout/left_sidebar.rs:14)    ← collapsible (nav.show_left_sidebar)
    │   ├── FileTree (file_tree.rs:149)            ← full vault tree
    │   └── Collections (smart folders + searches)  ← virtual overlays
    ├── MainContent (app.rs:344)
    │   ├── TabBar (layout/tab_bar.rs:38)          ← workspace tabs
    │   ├── NavBar (navigation/navbar.rs:20)       ← breadcrumbs + actions
    │   └── ViewPort (app.rs:357)                  ← ViewMode match (16 pages)
    ├── RightInspector (layout/right_inspector.rs:28) ← collapsible (nav.show_right_inspector)
    ├── CommandPalette (navigation/command_palette.rs)
    ├── QuickSwitcher (navigation/quick_switcher.rs)
    ├── ShortcutReference (navigation/shortcuts.rs)
    └── DictationPill (dictation_pill.rs:9)          ← floating overlay
```

**Layout ownership:**
- **RibbonBar** (`ribbon_bar.rs:70-87`): Fixed 48px-wide vertical sidebar on the far left. Contains icon-only buttons for Vault Explorer, Search, Graph, Dictation, Canvas, Settings. State owned locally (`enabled` signal); view-mode changes delegate to `App` via `Arc<dyn Fn(ViewMode)>`.
- **LeftSidebar** (`left_sidebar.rs:14`): Vault file explorer. Width controlled by `nav.show_left_sidebar` (default: `true`). Contains `FileTree` + Collections section.
- **MainContent**: Flex column. Top = `TabBar` (tabs), then `NavBar` (breadcrumbs + nav actions), then scrollable `ViewPort`.
- **RightInspector** (`right_inspector.rs:28`): Right-side panel. Width controlled by `nav.show_right_inspector` (default: `true`). Shows tags/backlinks/mentions for the active note.
- **Overlays**: `CommandPalette`, `QuickSwitcher`, `ShortcutReference`, `DictationPill` are conditionally rendered. `ToastRegion` is always mounted inside `ToastProvider`.

**Overlay/dialog system:** `Dialog` component (`ui/dialog.rs:31`) uses `RwSignal<bool>` for open state, renders a `dialog-overlay` div with click-outside-to-close and Escape-to-close. `ConfirmDialog` and `AlertDialog` are higher-level wrappers.

---

## 4. Navigation Architecture

### 4.1 Navigation Surfaces

Five distinct navigation surfaces, all coordinated by `NavContext`:

| Surface | Component | File | State |
|---------|-----------|------|-------|
| Left ribbon | `RibbonBar` | `layout/ribbon_bar.rs:16` | Local `enabled` signal + `Arc<dyn Fn>` callbacks |
| Left sidebar | `LeftSidebar` | `layout/left_sidebar.rs:14` | `nav.show_left_sidebar` |
| Main view switcher | `NavBar` | `navigation/navbar.rs:20` | `nav.view_mode` |
| Command palette | `CommandPalette` | `navigation/command_palette.rs` | `nav.palette_open` |
| Quick switcher | `QuickSwitcher` | `navigation/quick_switcher.rs` | `nav.switcher_open` |

### 4.2 Command Palette

`CommandPalette` (`navigation/command_palette.rs`) is a full-screen overlay triggered by `nav.palette_open.set(true)`. It renders a searchable list of 20+ commands from `navigation/commands.rs:13-359` — structured as `CommandCategory` groups. Each command has an `id`, `label`, `icon`, `shortcut`, and a `run: Callback<NavContext>` closure.

**Commands include:**
- `nav.new_note` → `note_create_file` IPC
- `nav.open_graph` → `nav.view_mode.set(Graph)`
- `nav.open_settings` → `open_settings` IPC
- `nav.undo` / `nav.redo` → history IPC
- `nav.find_in_files` → opens search page

### 4.3 Quick Switcher

`QuickSwitcher` (`navigation/quick_switcher.rs`) is keyboard-first fuzzy search over note titles and paths. Uses `fuzzy_score()` from `navigation/state.rs` for scoring. Renders three sections: Recent, Pinned, All notes. Does not execute commands — purely navigation.

### 4.4 Shortcut Reference

`ShortcutReference` (`navigation/shortcuts.rs`) is a searchable dialog showing all 20+ keyboard shortcuts from `SHORTCUTS` constant (`shortcuts.rs:34-150`). Categories: Navigation, Search, Editor, View, Files, Graph, Canvas, Tools, System.

### 4.5 Tab Navigation

`TabBar` (`layout/tab_bar.rs:38`) renders workspace tabs from `workspace.tabs` signal. Each tab has: click-to-activate, middle-click-to-close, right-click-context-menu (Close/Close Others/Close All/Duplicate/Pin/Reveal), drag-and-drop reordering. The "+" tab creates a new note via `note_create_file` IPC.

### 4.6 Breadcrumbs

`BreadcrumbBar` (`navigation/breadcrumb.rs:30`) renders a `Breadcrumbs` component with dynamic crumbs based on `nav.view_mode`:
- Dashboard/Graph/Search/etc → single crumb with view label
- Editor → vault → folder hierarchy → note name (each clickable)

---

## 5. Routing Map

Nabu uses **signal-driven routing** — no URL-based router. The `ViewMode` enum (`navigation/state.rs:22-56`) defines 16 modes:

```rust
pub enum ViewMode {
    Dashboard, Editor, Graph, Search, Settings,
    Inbox, ReadingQueue, Templates, Trash, History,
    Recovery, Calendar, Archive, SmartFolders,
    Canvas, Reader, Comparison, Statistics,
}
```

**Route resolution** happens in `App` (`app.rs:357-469`) via a `match` on `nav.view_mode.get()`:

| ViewMode | Rendering | Backend IPC |
|----------|-----------|-------------|
| `Dashboard` | `<Dashboard />` | `inbox_get_queue`, `notes_index` |
| `Editor` | `<NoteEditor />` + optional `<HomeScreen />` | `note_read`, `note_save` |
| `Graph` | `<GraphView _mode=Default />` | `graph_data`, `note_links` |
| `Search` | `<SearchPage />` | `notes_search` |
| `Settings` | `<SettingsPanel />` | `get_settings`, `settings_set_all` |
| `Inbox` | `<Inbox />` | `inbox_get_queue`, `capture_file_drop`, `inbox_approve` |
| `ReadingQueue` | `<ReadingQueue />` | `reading_queue_list`, `reading_queue_mark_done` |
| `Templates` | `<TemplateEditor />` | `template_list`, `template_save` |
| `Trash` | `<Trash />` | `trash_list`, `trash_restore_many`, `trash_delete` |
| `History` | `<VersionHistory />` | `versions_all`, `versions_list`, `versions_restore` |
| `Recovery` | `<RecoveryManager />` | `versions_all`, `snapshot_create` |
| `Calendar` | `<CalendarPage />` | `calendar_notes`, `daily_note_for` |
| `Archive` | `<ArchivePage />` | (file_tree archive command) |
| `SmartFolders` | `<SmartFoldersPage />` | `smart_folder_evaluate` |
| `Canvas` | `<CanvasView />` | `canvas_list`, `canvas_save`, `canvas_get` |
| `Reader` | `<ReaderView />` | `note_read`, `settings_get` |
| `Comparison` | `<ComparisonView />` | `versions_list`, `notes_diff`, `versions_diff` |
| `Statistics` | `<StatisticsView />` | `statistics_get` |

**Navigation entry points:**
- RibbonBar buttons → `set_view_mode` callback → `nav.view_mode.set()`
- NavBar view switcher → `set_view_mode` callback → `nav.view_mode.set()`
- Command palette commands → `run: Callback<NavContext>` → `nav.view_mode.set()`
- Shortcut reference → documented; global listener installs actual bindings
- Shortcuts → `install_global_shortcuts()` in `shortcuts.rs` (window-level keydown)

**No lazy loading, no route guards, no nested routes.** All 16 views are compiled into the match expression and rendered inline.

**Parsing:** `parse_view_mode()` (`navigation/state.rs:58-80`) converts strings to `ViewMode` for session restore. `view_mode_key()` (`state.rs:83-104`) and `view_mode_label()` (`state.rs:107-128`) convert back.

---

## 6. Component Hierarchy

### 6.1 Root Tree

```
start() [lib.rs:12]
└── ToastProvider [ui/feedback.rs]
    └── App [app.rs:78]
        ├── ThemeContext [lib.rs:43]
        ├── HistoryContext [history.rs]
        ├── SaveStatusContext [save_status.rs:61]
        ├── TaskContext [feedback.rs:764]
        ├── WorkspaceContext [workspace.rs:48]
        ├── NavContext [navigation/state.rs:214]
        ├── RecoveryBanner [recovery_banner.rs:16]
        ├── RibbonBar [ribbon_bar.rs:16]
        ├── LeftSidebar [left_sidebar.rs:14]
        │   └── FileTree [file_tree.rs:149]
        │       └── TreeNodeView [file_tree.rs:274+]  (recursive)
        ├── MainContent [app.rs:344]
        │   ├── TabBar [tab_bar.rs:38]
        │   ├── NavBar [navbar.rs:20]
        │   │   └── BreadcrumbBar [breadcrumb.rs:30]
        │   │       └── Breadcrumbs [ui/nav.rs:98]
        │   └── ViewPort [app.rs:357]
        │       ├── Dashboard [dashboard.rs:16+]
        │       ├── NoteEditor [note_editor.rs:26]
        │       │   ├── SlashMenu [editor/slash_menu.rs:5]
        │       │   └── NoteView [note_view.rs:10]
        │       ├── GraphView [graph_view.rs:1]
        │       ├── SearchPage [search_page.rs:53]
        │       ├── SettingsPanel [settings_panel.rs:115]
        │       ├── Inbox [inbox.rs:1]
        │       ├── ReadingQueue [reading_queue.rs:78]
        │       ├── TemplateEditor [template_editor.rs:1]
        │       ├── Trash [trash.rs:328]
        │       ├── VersionHistory [version_history.rs:197]
        │       ├── RecoveryManager [recovery_manager.rs:63]
        │       ├── CalendarPage [calendar_page.rs:69]
        │       ├── ArchivePage [archive_page.rs:25]
        │       ├── SmartFoldersPage [smart_folders.rs:50]
        │       ├── CanvasView [canvas.rs:97]
        │       ├── ReaderView [reader.rs:336]
        │       ├── ComparisonView [comparison.rs:41]
        │       └── StatisticsView [statistics.rs:62]
        ├── RightInspector [right_inspector.rs:28]
        ├── CommandPalette [command_palette.rs]
        ├── QuickSwitcher [quick_switcher.rs]
        ├── ShortcutReference [shortcuts.rs]
        └── DictationPill [dictation_pill.rs:9]
```

### 6.2 Feature Component Hierarchies

**NoteEditor** (`note_editor.rs:26`):
```
NoteEditor
├── textarea (controlled by `content` signal)
├── SlashMenu (conditionally rendered when `/` typed)
├── NoteView (read-only preview, always rendered alongside)
└── SaveStatusIndicator (via use_save_status context)
```

**FileTree** (`file_tree.rs:149`):
```
FileTree
├── TreeContext (provided via provide_context)
├── TreeNodeView (recursive component)
│   ├── ContextMenu (ui/menu.rs)
│   ├── ConfirmDialog (ui/dialog.rs)
│   └── inline rename input
└── batch action bar (when multi-selected)
```

**GraphView** (`graph_view.rs`):
```
GraphView
├── HTMLCanvasElement (raw <canvas> with manual rendering)
├── Tabs (ui/nav.rs) for GraphMode switching
├── relationship panel (collapsible)
├── search/filter controls
├── EmptyState (when no data)
└── LoadingBlock (while fetching graph_data)
```

**Inbox** (`inbox.rs:1`):
```
Inbox
├── InboxItem[] (split-pane interface)
├── file drop handler (capture_file_drop IPC)
├── status filters (Pending/Processing/Ready/Approved/Rejected/Failed)
├── processing history panel
└── thumbnail preview
```

**SettingsPanel** (`settings_panel.rs:115`):
```
SettingsPanel
├── SidebarItem[] (tab navigation, 14 tabs)
└── Tab content (AppearanceSettings | EditorSettings | ... | AboutSettings)
    All tabs compose: SettingCheckbox, Select, TextInput, Button
```

**CanvasView** (`canvas.rs:97`):
```
CanvasView
├── CanvasNode[] (positioned absolutely)
├── CanvasEdge[] (SVG connectors)
├── viewport controls (zoom, pan, fit)
├── node editor panel
├── node menu (add, delete, group)
└── canvas save/load via IPC
```

**Trash** (`trash.rs:328`):
```
Trash
├── trash_list IPC results
├── ContextMenu per item (Restore, Delete, Show in Folder)
├── ConfirmDialog (Empty Trash)
└── batch restore/delete
```

---

## 7. Shared Component Library Inventory

Located in `components/ui/` — 9 modules, re-exported from `components/ui/mod.rs:19-50`.

**File:** `components/ui/mod.rs`
**Module exports:** `button`, `card`, `dialog`, `feedback`, `icons`, `info`, `input`, `layout`, `menu`, `nav`, `selection`

### 7.1 Component Inventory

| Component | Definition | Used By | Duplicates |
|-----------|-----------|---------|------------|
| `Button` | `button.rs:76` | RibbonBar, TabBar, SettingsPanel, FileTree, Inbox, Canvas, GraphView | None |
| `IconButton` | `button.rs:141` | RibbonBar, NavBar, TabBar, RightInspector, SearchPage | None |
| `Card` / `CardHeader` / `CardBody` / `CardFooter` | `card.rs:33` | SmartFoldersPage, LeftSidebar, SearchPage, Dashboard | None |
| `CollapsibleCard` | `card.rs:115` | RightInspector tabs, SettingsPanel tabs | None |
| `Dialog` / `ConfirmDialog` / `AlertDialog` / `PromptDialog` | `dialog.rs:31` | FileTree (delete/rename), TabBar (close others), Trash (empty), GraphView (focus) | None |
| `ToastProvider` / `ToastRegion` / `NotificationBell` / `use_toast` / `ToastContext` | `feedback.rs` | App (root), all components via `use_toast()` | None |
| `Spinner` / `LoadingBlock` / `LoadingOverlay` / `LoadingScreen` / `Skeleton` / `SkeletonList` | `feedback.rs` | NoteEditor (loading), SearchPage, GraphView, Inbox | None |
| `Alert` / `Banner` / `Badge` / `Progress` / `StatusDot` | `feedback.rs` | Various — Dashboard (badge), NoteEditor (save), Inbox (status) | None |
| `Tooltip` / `EmptyState` / `Callout` / `HelpText` | `info.rs` | SmartFoldersPage, GraphView, SearchPage, FileTree | None |
| `TextInput` / `Textarea` / `SearchInput` / `PasswordInput` / `NumberInput` | `input.rs` | SettingsPanel, SmartFoldersPage, SearchPage, CommandPalette | None |
| `Checkbox` / `Radio` / `Switch` / `Segmented` / `Select` / `SelectOption` | `selection.rs` | SettingsPanel, SmartFoldersPage | None |
| `Tabs` / `TabDef` | `nav.rs:34` | SettingsPanel, RightInspector, GraphView, Inbox | None |
| `Breadcrumbs` / `Breadcrumb` / `SidebarItem` / `ToolbarButton` / `NavGroup` | `nav.rs` | NavBar (breadcrumbs), SettingsPanel (sidebar), LeftSidebar (sidebar items) | None |
| `Panel` / `Section` / `Stack` / `Grid` / `Container` | `layout.rs` | Layout primitives — used in Dashboard, SearchPage | None |
| `DropdownMenu` / `OverflowMenu` / `ContextMenu` / `MenuItem` / `MenuSeparator` / `CommandMenu` / `CommandItem` | `menu.rs` | FileTree (context menu), TabBar (context menu), Tray menus | None |
| `Icon` enum / `render_icon` / `render_icon_view` / `IconEl` | `icons.rs:195` | **Every** component that needs an icon | None |

### 7.2 Icon System

**File:** `components/ui/icons.rs:195-779`

The `Icon` enum has 110 variants (Navigation, Files & Notes, Status & Feedback, Actions, Communication, Knowledge Graph, Calendar & Time, Views, Objects & Tools, Charts & Stats, Editor Slash Menu, Misc, Keyboard). Each variant maps to exactly one Lucide component via the `icon_component()` dispatch table (`icons.rs:543-719`). Call sites never import Lucide directly — they use `render_icon_view(Icon::Foo)` or `<IconEl icon=Icon::Foo />`.

**Evidence:** `app.rs:14` imports `Icon` from `components::ui::icons`; `navigation/navbar.rs:14` calls `render_icon_view(Icon::Search)`, etc.

---

## 8. Frontend State Ownership Map

### 8.1 Context-Provided State (Root-level)

| Context | Provider | File | Signals |
|---------|----------|------|---------|
| `ThemeContext` | `provide_theme()` | `lib.rs:43` | `theme: RwSignal<String>` |
| `ToastContext` | `<ToastProvider>` | `feedback.rs` | `toasts: RwSignal<Vec<ToastItem>>` |
| `TaskContext` | `provide_tasks()` | `feedback.rs:764` | `tasks: RwSignal<Vec<TaskInfo>>` |
| `HistoryContext` | `provide_history()` | `history.rs` | `can_undo`, `can_redo: RwSignal<bool>` |
| `SaveStatusContext` | `provide_save_status()` | `save_status.rs:61` | `status`, `detail: RwSignal<...>` |
| `WorkspaceContext` | `provide_workspace()` | `workspace.rs:48` | `tabs`, `active_path`, `refresh_tree`, `content_version` |
| `NavContext` | `provide_navigation()` | `navigation/state.rs:214` | 12 signals (view_mode, sidebars, search, nav, index, etc.) |

### 8.2 Feature State Ownership

| Feature | Owning Component | State Type | Notes |
|---------|-----------------|------------|-------|
| Workspace tabs | `App` + `WorkspaceContext` | `RwSignal<Vec<OpenTab>>` | Shared via context; TabBar, FileTree, NoteEditor all mutate |
| Active note | `App` + `WorkspaceContext` | `RwSignal<Option<String>>` | Synced from `workspace.active_path` via `Effect` (`app.rs:111-120`) |
| Editor content | `NoteEditor` | `RwSignal<String>` | Local to component; synced to backend via debounce |
| Editor cursor/scroll | `App` (signals) | `RwSignal<u32>` | Reported via callbacks; persisted in session |
| View mode / routing | `NavContext` | `RwSignal<ViewMode>` | Mutated by RibbonBar, NavBar, CommandPalette, QuickSwitcher, shortcuts |
| Sidebar visibility | `NavContext` | `show_left_sidebar: RwSignal<bool>` | Toggled by RibbonBar; persisted in session |
| Inspector visibility | `NavContext` | `show_right_inspector: RwSignal<bool>` | Persisted in session |
| Vault note index | `NavContext` | `notes_index: RwSignal<Vec<NoteIndexEntry>>` | Loaded via `notes_index` IPC at startup |
| Search results | `SearchPage` | `RwSignal<Vec<SearchHit>>` | Local to component |
| Graph canvas | `GraphView` | Local signals + canvas refs | Layout state local; data fetched via IPC |
| Inbox items | `Inbox` | `RwSignal<Vec<InboxItem>>` | Local to component |
| Canvas nodes | `CanvasView` | Local signals | Persisted via `canvas_save` IPC |
| Settings | `SettingsPanel` | `RwSignal<AppSettings>` | Loaded via `get_settings` IPC; saved via `settings_set_all` |
| Smart folders | `NavContext` | `smart_folders: RwSignal<Vec<SmartFolder>>` | Persisted via `settings_set` IPC |
| Saved searches | `NavContext` | `saved_searches: RwSignal<Vec<SavedSearch>>` | Persisted via `settings_set` IPC |
| Recent notes | `NavContext` | `recent_notes: RwSignal<Vec<String>>` | Persisted via `settings_set` IPC |
| Favourites | `NavContext` | `favourites: RwSignal<Vec<String>>` | Persisted via `settings_set` IPC |
| Session state | `App` (local signals) | `(active_note, cursor, scroll, sidebars)` | Persisted via `session_save` IPC in debounced `Effect` |

### 8.3 Lifted / Shared State Patterns

- `WorkspaceContext` is the canonical source of truth for tabs and active note. `TabBar`, `FileTree`, `NoteEditor`, `BreadcrumbBar`, and `RightInspector` all read from and write to it.
- `NavContext` holds all navigation state (view mode, sidebars, vault index, recent/favourites). It is the canonical source for `Dashboard`, `QuickSwitcher`, `HomeScreen`, and `BreadcrumbBar`.
- `content_version: RwSignal<(String, u32)>` in `WorkspaceContext` is a cross-component coordination signal: `RightInspector` calls `bump_content_version(ws, &path)` after linking a mention, and `NoteEditor` watches it to reload from disk.

### 8.4 Duplicated State

- `TreeContext` in `file_tree.rs:54-73` duplicates some navigation concepts (nodes, selected, expanded) that also exist in `NavContext.notes_index`. The file tree maintains its own `RwSignal<Vec<TreeNode>>` loaded from `tree_list` IPC, separate from the vault-wide `notes_index` used by Dashboard/Search/QuickSwitcher.
- `CanvasView` maintains its own canvas definition state local to the component, persisted via `canvas_save`/`canvas_get` IPCs — not integrated into `WorkspaceContext`.

---

## 9. Rendering Flow Analysis

### 9.1 "Open Note" Workflow

1. **Trigger:** User clicks a note in `FileTree` → calls `open_tab(ws, &path)` (`workspace.rs:75`)
2. **Workspace updates:** `open_tab` pushes to `ws.tabs` and sets `ws.active_path`
3. **App re-renders:** `Effect` at `app.rs:111-120` detects `workspace.active_path` change → sets `active_note` signal
4. **Editor renders:** `ViewPort` match arm for `Editor` (`app.rs:363-387`) sees `active_note.is_some()` → renders `<NoteEditor note_path=...>`
5. **NoteEditor mounts:** On mount, `spawn_local` at `note_editor.rs:63-73` calls `note_read` IPC → `set_content.set(saved)` → textarea renders with content
6. **Tab bar updates:** `TabBar` reads `workspace.tabs` → new tab appears with active state
7. **Breadcrumb updates:** `BreadcrumbBar` reads `workspace.active_path` → shows folder hierarchy + note name
8. **RightInspector updates:** `Effect` at `right_inspector.rs:39` detects `ws.active_path` change → calls `note_links` IPC → renders backlinks/mentions
9. **Session persists:** `Effect` at `app.rs:164-189` fires (debounced) → calls `session_save` IPC

**Re-render trigger chain:**
```
FileSystem change / user click
  → RwSignal update (workspace.active_path)
    → App Effect (app.rs:111) → active_note signal
      → ViewPort match arm re-evaluates
        → NoteEditor mounts (if new instance)
        → TabBar re-renders (tabs signal)
        → BreadcrumbBar re-renders (active_path signal)
        → RightInspector Effect fires (active_path signal)
    → App Effect (app.rs:164) → session_dirty bump
      → setTimeout → session_save IPC
```

### 9.2 "Save Note" Workflow

1. **User types** in textarea → `on:input` handler (`note_editor.rs:339`) sets `content` + bumps `dirty` + sets `has_unsaved`
2. **Debounced save** `Effect` (`note_editor.rs:117-156`) waits 800ms → calls `note_save` IPC with `{path, content}`
3. **Save status** updates: `Saving` → `Saved`/`Failed` via `save_status` context
4. **On success:** `has_unsaved.set(false)` → `SaveStatusIndicator` shows green dot
5. **On failure:** `SaveStatus.Failed` → 5-second interval (`note_editor.rs:162-175`) retries

**Backend interaction:** Only `note_save` is called — no graph/index update event fires. The `content_version` signal is NOT bumped on autosave (it is only bumped by `bump_content_version` in `RightInspector` after link/mention changes). This means cross-note references created via the inspector will trigger a reload, but external filesystem changes during an active editing session will be silently overwritten by `note_save`.

### 9.3 "Switch View" Workflow

1. **Trigger:** User clicks a view in RibbonBar/NavBar/CommandPalette
2. **`nav.view_mode.set(new_mode)`** (`state.rs:178`)
3. **App re-renders:** The `move || match nav.view_mode.get()` closure (`app.rs:357`) re-evaluates
4. **Old view unmounts, new view mounts**
5. **New view on-mount Effect** (if any) fires → calls relevant IPC to load data

---

## 10. Backend Integration Matrix

All frontend→backend communication uses `crate::ipc::tauri_invoke(cmd, args)` (`ipc.rs:9`), which wraps `window.__TAURI__.core.invoke()`.

### 10.1 IPC Command Inventory

| Command | Initiating Component | Return Type | Purpose |
|---------|---------------------|-------------|---------|
| `check_vault_exists` | `App` (`app.rs:237`) | `Option<String>` | Determine if vault is configured |
| `recovery_check` | `App` (`app.rs:129`) | `RecoveryStatus` | Crash recovery on startup |
| `get_settings` | `SettingsPanel` (`settings_panel.rs:119`), `provide_theme()` (`lib.rs:68`) | `AppSettings` | Load all settings |
| `settings_get` | `DictationPill` (`dictation_pill.rs:21`), `ThemeToggle`-equivalent in `provide_theme` (`lib.rs:57`), `ReaderView` (`reader.rs:351`) | `serde_json::Value` | Get single setting |
| `settings_set` | `navigation/state.rs:255`, `reader.rs:69`, `lib.rs:97` | `Result<(), String>` | Set single setting (used by nav state persistence) |
| `settings_set_all` | `SettingsPanel` (`settings_panel.rs:132`) | `Result<(), String>` | Save all settings |
| `notes_index` | `navigation/state.rs` (`load_notes_index`) | `Vec<NoteIndexEntry>` | Full vault index |
| `note_read` | `NoteEditor` (`note_editor.rs:66,104`), `ReaderView` (`reader.rs:363`) | `String` | Read note content |
| `note_save` | `NoteEditor` (`note_editor.rs:136`) | `Result<(), String>` | Write note content |
| `note_create_file` | `TabBar` (`tab_bar.rs:52`), `commands.rs` (`commands.rs:106`) | `()` | Create new note file |
| `note_duplicate` | `TabBar` (`tab_bar.rs:97`), `file_tree.rs:237` | `String` (new path) | Duplicate a note |
| `note_delete` | `file_tree.rs:281` | `Result<(), String>` | Move to trash (reversible) |
| `note_restore` | `trash.rs:771` | `Result<(), String>` | Restore from trash |
| `note_daily` | `commands.rs:173`, `ribbon_bar.rs:62` | `String` (path) | Create/open daily note |
| `tree_list` | `file_tree.rs:124` | `Vec<TreeNode>` | Full vault tree structure |
| `items_move` | `file_tree.rs:206` | `Result<(), String>` | Move files/folders |
| `archive_note` | `file_tree.rs:253`, `archive_page.rs` | `Result<(), String>` | Archive a note |
| `nodes_move` / `nodes_rename` | `file_tree.rs:162,349` | various | Atomic file operations (undoable) |
| `reveal_in_file_manager` | `file_tree.rs:320`, `commands.rs:160` | `Result<(), String>` | Open in OS file manager |
| `inbox_get_queue` | `Inbox` (`inbox.rs:128`), `Dashboard` (`dashboard.rs:189`) | `Vec<InboxItem>` | Load inbox items |
| `capture_file_drop` | `DictationPill` (`dictation_pill.rs:104`), `Inbox` (`inbox.rs:255`) | `String` (id) | Capture dragged/dropped files |
| `inbox_approve` | `Inbox` | `Result<(), String>` | Approve an inbox item |
| `inbox_reject` | `Inbox` | `Result<(), String>` | Reject an inbox item |
| `inbox_discard` | `Inbox` | `Result<(), String>` | Discard an inbox item |
| `smart_folder_evaluate` | `SmartFoldersPage` (`smart_folders.rs:72`) | `Vec<SmartFolderResult>` | Run a smart folder query |
| `notes_search` | `SearchPage` (`search_page.rs:113`) | `Vec<SearchHit>` | Full-text search |
| `graph_data` | `GraphView` (`graph_view.rs:237`) | `GraphData` | Knowledge graph nodes + edges |
| `note_links` | `RightInspector` (`right_inspector.rs:23`), `GraphView` (`graph_view.rs:202`) | `NoteLinks` | Backlinks + mentions |
| `link_mention` | `RightInspector` (`right_inspector.rs:68`), `GraphView` (`graph_view.rs:809`) | `String` | Convert mention to wikilink |
| `mention_ignore` | `RightInspector` (`right_inspector.rs:85`), `GraphView` (`graph_view.rs:826`) | `()` | Ignore mention suggestion |
| `statistics_get` | `StatisticsView` (`statistics.rs:76`) | `VaultStatistics` | Vault-wide metrics |
| `canvas_list` / `canvas_save` / `canvas_get` / `canvas_delete` | `CanvasView` (`canvas.rs:127,144,277,291`) | various | Canvas state persistence |
| `trash_list` / `trash_restore_many` / `trash_delete` / `trash_empty` | `Trash` (`trash.rs:209,238,277,297`) | various | Trash management |
| `versions_all` / `versions_list` / `versions_get` / `versions_restore` / `versions_diff` / `versions_duplicate` | `VersionHistory` (`version_history.rs:95-289`), `RecoveryManager` (`recovery_manager.rs:33-101,227`), `ComparisonView` (`comparison.rs:77,102,125`) | various | Snapshot/version management |
| `snapshot_create` | `VersionHistory` (`version_history.rs:235`) | `String` | Manual snapshot |
| `template_list` / `template_save` / `template_delete` / `template_duplicate` / `template_set_favourite` | `TemplateEditor` (`template_editor.rs:54-140`) | various | Template CRUD |
| `calendar_notes` / `daily_note_for` | `CalendarPage` (`calendar_page.rs:95,132`) | various | Calendar + daily notes |
| `session_save` / `session_load` / `session_clear` | `recovery/session.rs` (`session.rs:50,58,68`) | various | Session persistence |
| `reading_queue_list` / `reading_queue_add` / `reading_queue_mark_done` / `reading_queue_remove` | `ReadingQueue` (`reading_queue.rs:101,195,208,221,243`) | various | Reading queue CRUD |
| `history_undo` / `history_redo` / `history_status` | `history.rs` (`history.rs`) | various | Universal undo/redo |

### 10.2 Backend Dependency Mapping

```
Frontend Layer (nabu-ui)
  │  All calls go through: crate::ipc::tauri_invoke()  [ipc.rs:9]
  │  ↓
Tauri Shell (src-tauri)
  │  Commands defined in: src-tauri/src/commands.rs
  │  (40+ #[tauri::command] functions)
  │  ↓
Rust Core (nabu-core)
  │  Accessed via: ApplicationContext (registry/context.rs:141)
  │  Subsystems: StorageManager, EventBus, Indexer, VaultGraph,
  │               WorkerPool, DurableJobQueue, PipelineExecutor,
  │               CaptureEngine, HistoryManager
  │  ↓
Filesystem
```

**Note:** The `note_save` command at `src-tauri/src/recovery.rs:391` (from Audit 0.5) **bypasses** `StorageManager` entirely — it writes via `std::fs::write` directly, without publishing an `ITEM_STORED` event to `EventBus` or triggering `Indexer` or `VaultGraph` updates. This path was identified as a CRITICAL finding in Audit 0.5 (finding 14.1) and remains uncorrected.

---

## 11. Feature Architecture Breakdown

### 11.1 Dashboard

- **Entry component:** `Dashboard` (`navigation/dashboard.rs:16`)
- **Child hierarchy:** `NoteListSection` → `SidebarItem`-like note rows → `open_tab` / `activate_tab` / `toggle_favourite`
- **Backend integration:** `inbox_get_queue` (inbox section), `notes_index` (recently modified), `use_nav` context (recents, favourites, pinned)
- **Reusable components:** `Card`, `SidebarItem`, `IconEl`, `EmptyState`
- **Ownership boundary:** `NavContext` owns the data; `Dashboard` is a pure projection layer. No local state except `active_section` UI state.

### 11.2 Editor

- **Entry component:** `NoteEditor` (`note_editor.rs:26`)
- **Child hierarchy:** `textarea` → (loading state) `SkeletonList` → (loaded) `textarea` + `SlashMenu` + `NoteView` + `SaveStatusIndicator`
- **Backend integration:** `note_read` (mount + external change), `note_save` (debounced autosave), `note_create_file` (via TabBar)
- **Reusable components:** `SkeletonList`, `SlashMenu`, `NoteView`, `SaveStatusIndicator`
- **Ownership boundary:** `NoteEditor` owns `content`, `dirty`, `has_unsaved` locally. `SaveStatusContext` is shared. `workspace.active_path` is watched via `Effect` for cross-note reloads.

### 11.3 Search

- **Entry component:** `SearchPage` (`navigation/search_page.rs:53`)
- **Child hierarchy:** `SearchInput` → results list → `SearchHit` rows → `open_tab` on click
- **Backend integration:** `notes_search` IPC, query prefill from `nav.search_query`
- **Reusable components:** `SearchInput`, `Select` (sort), `EmptyState`, `Spinner`
- **Ownership boundary:** `SearchPage` owns all results state. Recent/saved searches are in `NavContext`.

### 11.4 Graph

- **Entry component:** `GraphView` (`graph_view.rs`)
- **Child hierarchy:** `<canvas>` (manual rendering) → relationship panel (`Tabs`) → `NoteLinks` data
- **Backend integration:** `graph_data` IPC (initial load), `note_links` IPC (inspector), `link_mention`/`mention_ignore` (actions)
- **Reusable components:** `Tabs`, `LoadingBlock`, `EmptyState`, `Spinner`
- **Ownership boundary:** `GraphView` owns all canvas state (nodes, edges, viewport, selection). Data is fetched on mount and on version bump.

### 11.5 Inbox

- **Entry component:** `Inbox` (`inbox.rs:1`)
- **Child hierarchy:** Split-pane (item list + detail panel) → `InboxItem` rows → status filter → processing history
- **Backend integration:** `inbox_get_queue`, `capture_file_drop`, `inbox_approve`/`reject`/`discard`, `note_read` (detail preview)
- **Reusable components:** `EmptyState`, `Spinner`, `Button`
- **Ownership boundary:** `Inbox` owns all items state. No shared context beyond `NavContext` for `open_tab`.

### 11.6 Settings

- **Entry component:** `SettingsPanel` (`settings_panel.rs:115`)
- **Child hierarchy:** Sidebar tabs → tab content → `SettingCheckbox`/`Select`/`TextInput` → save via `settings_set_all`
- **Backend integration:** `get_settings` (mount), `settings_set_all` (save)
- **Reusable components:** `SidebarItem`, `Checkbox`, `Select`, `TextInput`, `Button`, `Card`
- **Ownership boundary:** `SettingsPanel` owns `RwSignal<AppSettings>` locally. No shared context — settings are loaded and saved as a complete snapshot.

### 11.7 Canvas

- **Entry component:** `CanvasView` (`canvas.rs:97`)
- **Child hierarchy:** `<svg>` viewport → `CanvasNode` cards → connection edges → node editor panel → toolbar
- **Backend integration:** `canvas_list`, `canvas_save`, `canvas_get`, `canvas_delete`
- **Reusable components:** `Button`, `IconButton`, `EmptyState`, `Input`
- **Ownership boundary:** `CanvasView` owns all canvas state (nodes, edges, viewport). Uses `NavContext` for `open_tab` on note references.

### 11.8 Navigation / Discovery

- **Entry components:** `NavBar`, `RibbonBar`, `BreadcrumbBar`, `CommandPalette`, `QuickSwitcher`, `ShortcutReference`
- **Shared state:** `NavContext` — mutated by all navigation surfaces
- **Ownership boundary:** Navigation surfaces are thin presenters over `NavContext`. `NavContext` provides `use_nav()` for mutation and `load_all_nav_state()` for initialization. Persistence is handled by helper functions in `state.rs:244-259` that call `settings_set` IPC.

### 11.9 Recovery & History

- **Entry components:** `RecoveryBanner`, `RecoveryManager`, `VersionHistory`
- **Shared context:** `SaveStatusContext` (status indicator in NavBar)
- **Backend integration:** `recovery_check` (startup), `session_save`/`session_load`/`session_clear`, `versions_*` IPC family
- **Ownership boundary:** `App` owns session state signals; `recovery/session.rs` provides `session_save`/`session_load` IPC helpers. `HistoryContext` provides undo/redo state, driven by `history.rs` which calls `history_undo`/`history_redo` IPC.

---

## 12. Design System Architecture

### 12.1 Styling Architecture

**File:** `src/styles/app.css` (root `index.html` at `/Users/macbook/github code/Nabu/src/styles/app.css`)

The app uses **Tailwind CSS** (build-time processed via `npm run css:build`) with a thin layer of semantic CSS classes. The stylesheet is 2923 lines.

**Structure:**
1. `@tailwind base; @tailwind components; @tailwind utilities;` — Tailwind entry points
2. `:root` design tokens — CSS custom properties for colors, typography, spacing, elevation, radius, motion
3. Theme variants: `[data-theme="dark"]` (default), `[data-theme="light"]`, `[data-theme="system"]`
4. Base layer — reset, scrollbars, focus, selection
5. Component layer — semantic class names (`.btn`, `.card`, `.dialog`, `.sidebar-item`, `.navbar`, etc.)

### 12.2 Design Tokens

Tokens are CSS custom properties on `:root`, organized into:

| Category | Variables | Example |
|----------|-----------|---------|
| Color — accent | `--color-primary`, `--color-accent` | `59 130 246` (blue-600) |
| Color — gray scale | `--gray-50` through `--gray-950` | `--gray-950: 3 7 18` (background) |
| Color — status | `--color-success`, `--color-warning`, `--color-error`, `--color-info` | `34 197 94` (green-500) |
| Typography | `--font-sans`, `--font-mono`, `--text-display` through `--text-code` | `--text-body: 0.875rem` |
| Spacing | `--space-1` through `--space-16` | `--space-4: 1rem` |
| Elevation | `--shadow-card`, `--shadow-card-hover`, `--shadow-dialog`, `--shadow-popover` | `0 1px 2px rgba(0,0,0,0.4)` |
| Radius | `--radius-card: 0.75rem`, `--radius-dialog`, `--radius-panel`, `--radius-chip: 9999px` |
| Motion | `--ease-standard`, `--ease-out`, `--ease-in`, `--ease-spring`, `--duration-fast/normal/slow/slower` | `cubic-bezier(0.4, 0, 0.2, 1)` |

**Theme switching:** `data-theme` attribute on `document.documentElement`. Set by `apply_theme_to_document()` (`lib.rs:111`). "system" removes the attribute so `prefers-color-scheme` media query applies.

### 12.3 Component Layer

Semantic class names (not Tailwind utility-only):

| Class | Used By | CSS Location |
|-------|---------|-------------|
| `.btn` | `Button` component | `app.css` component layer |
| `.btn-primary`, `.btn-ghost`, `.btn-outline`, `.btn-danger`, `.btn-icon`, `.btn-sm/md/lg` | `Button` variants |
| `.card`, `.card-outlined`, `.card-elevated`, `.card-hover` | `Card` component |
| `.dialog-overlay`, `.dialog`, `.dialog-lg` | `Dialog` component |
| `.sidebar-item`, `.sidebar-item-active`, `.sidebar-item` variants | `SidebarItem` |
| `.navbar`, `.navbar-row`, `.navbar-actions`, `.navbar-action` | `NavBar` |
| `.tab-bar`, `.tab`, `.tab-active` | `TabBar` |
| `.toast-region`, `.toast`, `.toast-success/warning/error/info`, `.toast-action`, `.toast-close` | `ToastRegion` |
| `.notif-overlay`, `.notif-panel`, `.notif-item`, `.notif-badge` | `NotificationBell` |
| `.empty-state`, `.empty-state-icon`, `.empty-state-title`, `.empty-state-desc` | `EmptyState` |
| `.field`, `.field-label`, `.field-hint`, `.field-error` | `field_chrome()` in `input.rs` |
| `.switch`, `.switch-track`, `.switch-thumb` | `Switch` component |
| `.checkbox` | `Checkbox` |
| `.menu`, `.menu-item`, `.menu-item-active`, `.menu-item-danger`, `.menu-separator` | `MenuItem`, `DropdownMenu` |
| `.breadcrumbs`, `.breadcrumb-link`, `.breadcrumb-current`, `.breadcrumb-sep` | `Breadcrumbs` |
| `.skeleton`, `.skeleton-list`, `.skeleton-list-row` | `Skeleton`, `SkeletonList` |
| `.spinner`, `.spinner-sm/md/lg` | `Spinner` |
| `.badge`, `.badge-success/warning/error/info` | `Badge` |
| `.progress`, `.progress-fill` | `Progress` |
| `.status-dot`, `.status-dot-success/warning/error/info`, `.status-dot-pulse` | `StatusDot` |
| `.loading-block`, `.loading-overlay`, `.loading-screen` | `LoadingBlock` etc. |
| `.dash-card`, `.dash-card-header`, `.dash-card-body`, `.dash-empty` | `Dashboard` |
| `.file-tree`, `.tree-node` | `FileTree` |
| `.editor-textarea`, `.editor-drop-overlay` | `NoteEditor` |
| `.left-sidebar`, `.sidebar-left` | `LeftSidebar` |
| `.view-switcher` | `ViewSwitcher` |
| `.template-picker`, `.template-list` | `TemplatePicker` |
| `.file-tree` | `tree.rs::FileTree` (dead code, see §13) |

### 12.4 Icon System

**File:** `components/ui/icons.rs`

- **Source:** `lucide-leptos` crate (re-exports from `icons.rs:44-181`)
- **Abstraction:** `Icon` enum (`icons.rs:195`) — 110 variants named after concepts, not glyphs
- **Dispatch:** `icon_component(icon: Icon) -> AnyView` (`icons.rs:543`) — single match table maps each `Icon` variant to a Lucide component
- **API:** `render_icon_view(Icon)` returns `AnyView`; `render_icon(Icon, Option<&str>)` wraps in `<span class="lucide-icon">` with `aria-hidden="true"`; `IconEl` is a component wrapper
- **CSS:** Global `.lucide { width: 1em; height: 1em }` makes icons scale with `font-size` (`icons.rs:26-27`)

### 12.5 Consistency

All major components use the shared `ui/` library consistently. Notable exceptions:
- `SmartFoldersPage` (`smart_folders.rs:166-245`) uses inline `class="..."` strings with Tailwind utilities instead of `SidebarItem`
- `SidebarItem` in `SettingsPanel` sidebar uses `SidebarItem` properly (`settings_panel.rs:163`)
- `BreadcrumbBar` uses the shared `Breadcrumbs` component (`breadcrumb.rs:99`)
- `Dashboard` uses raw Tailwind classes for card styling (e.g., `dash-card`) rather than composing `Card` — this is a deliberate performance choice (the docblock at `dashboard.rs:18-26` shows it builds lightweight cards)

The icon system is 100% consistent — every component uses `render_icon_view()` or `IconEl` and never imports Lucide directly.

---

## 13. Dead & Obsolete Components

### 13.1 Dead Components (Defined But Never Rendered)

| Component | File | Lines | Verification |
|-----------|------|-------|-------------|
| `FileTree` (standalone) | `tree.rs:14` | 49 lines | `lib.rs:10` exports `pub mod tree;` but no component in `app.rs`, `components/mod.rs`, or any other file imports or renders `crate::tree::FileTree` or `crate::tree::TreeNodeView` |
| `TemplatePicker` | `template_picker.rs:10` | 52 lines | Defined and exported via `pub fn`, but `grep` for `TemplatePicker` across `src/` finds only the declaration. Never imported by `TemplateEditor` or any parent. The `TemplateEditor` (`template_editor.rs`) uses raw HTML instead. |
| `PdfViewer` | `pdf_viewer.rs:4` | 15 lines | Never imported or rendered anywhere. No `PDF` or `pdf` reference in any component that would use it. |
| `ThemeToggle` | `theme_toggle.rs:6` | 24 lines | Never imported or rendered. Theme switching is handled by `provide_theme()` / `ThemeContext` in `lib.rs` + the backend `settings_get` IPC. `ThemeToggle` references `expect_context::<ThemeContext>()` but is never mounted. |
| `RelationEditor` | `relation_editor.rs:14` | 276 lines | Never imported or rendered. Defined to take `KnowledgeObject`, `Vec<GraphEdge>`, callbacks. The graph/relationship feature uses raw `<canvas>` + `RightInspector` instead. |
| `SandboxedHtml` | `sandboxed_html.rs:4` | 8 lines | Never imported or rendered. A simple `<iframe srcdoc>` wrapper with no callers. |

### 13.2 Dead Modules

| Module | File | Verification |
|--------|------|-------------|
| `crate::tree` | `tree.rs` | Exported as `pub mod tree;` in `lib.rs:10`. Never referenced by any import statement (`use crate::tree::`) anywhere in the codebase. Contains a standalone `FileTree` + `TreeNodeView` that duplicates the real `components/file_tree.rs`. |

### 13.3 Migration Leftovers

| Component | Original | Current | Evidence |
|-----------|----------|---------|---------|
| `ViewSwitcher` | Yew | Leptos 0.7 | `collections/view_switcher.rs` was noted in Audit 0.5 as converted from Yew types to Leptos `#[component]`. The `Props` struct uses `#[derive(Props, PartialEq)]` (`view_switcher.rs:4`) which is the Yew v0.21+ pattern, not idiomatic Leptos 0.7 (which uses `#[component]` with `#[props]`). This is a migration artifact — the code works but the Props pattern is non-idiomatic. |
| `search_state` in collections | Yew | Leptos | `collections/shared/context.rs:1-7` uses the same comment style as the original Yew conversion. `SearchState` struct mirrors the AGENTS.md note: "converted from Yew types; converted to Leptos `#[component]` + `Signal`." |

### 13.4 Abandoned Layouts

None found. All four layout files (`left_sidebar.rs`, `ribbon_bar.rs`, `right_inspector.rs`, `tab_bar.rs`) are actively rendered by `App`.

---

## 14. Dependency Analysis

### 14.1 Dependency Direction

```
Feature Components (app.rs, navigation/*, graph_view, note_editor, inbox, canvas, etc.)
    ↓
Shared Component Library (components/ui/*)
    ↓
Icon System (ui/icons.rs — single leaf dependency)
    ↓
Leptos 0.7.8 + wasm-bindgen + web-sys (framework primitives)
```

**Verification:** `components/ui/mod.rs` declares no imports from feature components. Feature components import from `components/ui::*` and `crate::workspace`, `crate::history`, `crate::ipc`, `crate::models`. No circular dependencies exist.

### 14.2 Violations

| Violation | File | Details |
|-----------|------|---------|
| `lib.rs` imports `components::ui::feedback::ToastProvider` directly | `lib.rs:18` | The root WASM entry point (`lib.rs`) imports a UI component module, bypassing the `components/mod.rs` layer. This is a minor architectural inconsistency but not a functional problem. |
| `components/ui/feedback.rs` imports `crate::components::ui::icons` | `feedback.rs:3` | The `feedback` module imports icons directly, which is fine (icons is a leaf). No violation. |
| `components/app.rs` imports layout modules directly via full paths | `app.rs:3-31` | `App` imports from `crate::components::layout::*`, `crate::components::navigation::*`, etc. using fully-qualified paths rather than re-exports from `components/mod.rs`. This works but creates a tighter coupling between `App` and the internal module structure. |
| `tree.rs` duplicates `components/file_tree.rs` | `tree.rs:1-49` | Both define `FileTree` and `TreeNodeView`. `tree.rs` is dead code; `components/file_tree.rs` is the active implementation. This is a leftover from an earlier architecture iteration. |

### 14.3 Shared State Coupling

`WorkspaceContext` is the most widely shared state — used by 8+ components:
- `App` (owns tabs/active_path signals)
- `TabBar` (reads tabs, writes active_path/close_tab)
- `FileTree` (writes active_path via open_tab, writes refresh_tree)
- `NoteEditor` (reads active_path, writes content_version)
- `BreadcrumbBar` (reads active_path)
- `RightInspector` (reads active_path, writes content_version)
- `Dashboard` (reads notes_index via NavContext, calls open_tab)
- `QuickSwitcher` (reads notes_index, calls open_tab)
- `HomeScreen` (reads notes_index, calls open_tab)

This is appropriate coupling for a desktop app shell — the workspace is global state.

---

## 15. Future Capability Integration

### 15.1 Where New Capabilities Should Extend

| Capability | Recommended Extension Point | Evidence |
|------------|---------------------------|---------|
| **ACP sidebar** | `RightInspector` (`layout/right_inspector.rs`) — add a new `TabDef("assistant", "ACP")` tab | `RightInspector` already uses `Tabs` (`ui/nav.rs:34`) and `WorkspaceContext.active_path` for context. Adding an ACP tab follows the existing pattern at `right_inspector.rs:31-33` |
| **Syncthing status** | `NavBar` (`navigation/navbar.rs:20`) — add a status indicator in `navbar-actions` | `NavBar` is the canonical place for global status indicators (already hosts undo/redo/save status). `TaskContext` (`feedback.rs:728`) provides the background-task pattern for sync progress |
| **Harper diagnostics** | `NoteEditor` (`note_editor.rs:26`) — add an inline diagnostics panel | `NoteEditor` owns the content signal and textarea `NodeRef`. A diagnostics layer can reuse `ErrorPanel` (`feedback.rs:662`) and `Badge` components. The `use_tasks()` pattern (`feedback.rs:771`) supports background analysis |
| **Streaming responses** | `Inbox` (`inbox.rs:1`) or a new message surface in `RightInspector` | `Inbox` already has `spawn_local` + `serde_wasm_bindgen::from_value` patterns. For streaming, the `TaskContext` + `Progress` component (`feedback.rs:733`) provides the UI scaffold |
| **Capability settings** | `SettingsPanel` (`settings_panel.rs:115`) — add a new tab | `SettingsPanel` uses a tab list at `settings_panel.rs:137-153` with a match dispatch at `settings_panel.rs:172-189`. Adding a new tab requires: (1) add string to the `tabs` vec, (2) add a `match` arm, (3) create the settings sub-component using existing `SettingCheckbox`/`Select`/`TextInput` primitives |
| **Background activity indicators** | `TaskContext` + `TaskIndicator` (`feedback.rs`) | `TaskContext` (`feedback.rs:728`) provides `start()`/`progress()`/`finish()` — a capability registers a task via `use_tasks()` and the `TaskIndicator` in `NavBar` renders it automatically |
| **Capability notifications** | `ToastContext` / `use_toast()` (`feedback.rs`) | Every component calls `use_toast()` to emit toasts. New capabilities follow the same pattern: `let toasts = use_toast(); toasts.info("Title", "Message")` |

### 15.2 Extension Points Summary

```
Existing Abstraction         │ Reusable For
─────────────────────────────┼─────────────────────────────────────
ThemeContext (lib.rs:43)     │ Theme-aware capability surfaces
ToastContext (feedback.rs)   │ All capability notifications
TaskContext (feedback.rs)    │ Background activity indicators
WorkspaceContext (workspace) │ Active note + tab integration
NavContext (state.rs)        │ View switching + vault index
SaveStatusContext            │ Save/status indicators for capability data
HistoryContext (history.rs)  │ Undo/redo for capability actions
```

All contexts are `Copy` types captured at render time and threaded into async blocks — new capabilities simply call `use_nav()` / `use_workspace()` / `use_toast()` at the top of their component and use the existing `tauri_invoke` pattern.

---

## 16. Architectural Observations

### 16.1 Strengths

1. **Single IPC abstraction:** All backend calls go through `crate::ipc::tauri_invoke()` (`ipc.rs:9`). This is a thin, well-defined wrapper around `window.__TAURI__.core.invoke()`. Any new capability adds an entry to `src-tauri/src/commands.rs` and calls the same function.

2. **Layered context architecture:** Seven `Copy`-based contexts (`ThemeContext`, `ToastContext`, `TaskContext`, `HistoryContext`, `SaveStatusContext`, `WorkspaceContext`, `NavContext`) provide all shared state. The "capture at render time, thread into async" pattern is consistently documented and followed (see `smart_folders.rs:15-19`, `save_status.rs:7-10`, `history.rs:18-24`).

3. **Component library reuse:** 40+ shared primitives in `components/ui/` are used consistently across all feature components. The icon system (`Icon` enum + `render_icon_view` dispatch table) is 100% consistent — no component imports Lucide directly.

4. **Design token system:** CSS custom properties on `:root` with `data-theme` variants. Tailwind utilities are used for layout; semantic classes for components. Tokens are channel-based (e.g., `--gray-950: 3 7 18`) for `<alpha-value>` opacity support.

5. **Debouncing pattern:** Autosave (`note_editor.rs:117-156`), session persistence (`app.rs:163-189`), and retry logic (`note_editor.rs:162-175`) all follow the same `Effect` + `set_timeout` / `set_interval_with_handle` + `on_cleanup` pattern.

### 16.2 Concerns

1. **Dead code accumulation:** 6 components (`tree.rs::FileTree`, `TemplatePicker`, `PdfViewer`, `ThemeToggle`, `RelationEditor`, `SandboxedHtml`) and 1 module are defined but never rendered. This represents architectural drift — components were built but the UI evolved around them.

2. **Collections module is partially disconnected:** `CollectionContainer` (`collections/container.rs:18`) is the entry point for the collections views system (`table_view`, `board_view`, `gallery_view`, `calendar_view`, `view_switcher`) but is never imported or rendered anywhere. The entire `components/collections/` module (6 files) appears to be a migration target that was not completed — the AGENTS.md notes "converted from Yew types; converted to Leptos `#[component]` + `Signal`" but the container component itself has no parent.

3. **`tree.rs` is a full duplicate of the file tree concept:** It defines its own `TreeNode`, `FileTree`, and `TreeNodeView` with no connection to `components/file_tree.rs`. Both are module-root exports but only the `components/` version is rendered.

4. **NoteView is a no-op placeholder:** `note_view.rs:10` has a docblock stating "A markdown renderer was originally referenced here (`nabu_core::parser::parse_markdown_to_html`), but no such module exists in nabu-core, so the view renders the source text directly." The component is rendered alongside the editor (`note_editor.rs:367`) but is essentially a passthrough `<div>`.

5. **Non-idiomatic Props pattern in collections:** `collections/view_switcher.rs:4-8` uses `#[derive(Props, PartialEq)]` with a `Props` struct and `fn ViewSwitcher(props: &Props)` — this is the Yew v0.21+ component pattern, not idiomatic Leptos 0.7. The AGENTS.md notes this was converted but the Props struct was left behind.

6. **SettingsPanel has dead sub-components:** `GeneralSettings` (`settings_panel.rs:223`) and `WhisprSettings` (`settings_panel.rs:346`) are defined but not in the tab match at `settings_panel.rs:137-189` — they are orphaned helpers. The `tabs` vector lists 14 tabs but the match only has 13 arms (no "General" tab, despite `GeneralSettings` existing).

### 16.3 Backend Integration Gap (from Audit 0.5)

The frontend autosave path (`note_save` IPC) does not trigger any backend EventBus/IPC event back to the frontend. The `content_version` signal is only bumped by `RightInspector` actions (`bump_content_version`). There is no Tauri `#[listen]` event bridge on the frontend for `ITEM_STORED`, `IndexUpdated`, or `GraphUpdated` events. This means:
- External file changes are not detected (no `notify` watcher on the UI side)
- The graph view does not auto-refresh when notes are created/edited
- The vault index does not update when notes are added/removed from outside the UI

---

## 17. Conclusion: Recommendations for Capability Platform Integration

**If Nabu's future Capability Platform introduces new UI surfaces, they should integrate as follows:**

### Primary Extension Points (in priority order)

1. **For sidebar-level capabilities (ACP, Syncthing status):** Extend `RightInspector` (`right_inspector.rs:28`) by adding a `TabDef` to its `active_tab` signal. `RightInspector` already has the `Tabs` component, `RightInspector` is keyed off `workspace.active_path`, and it already handles `spawn_local` + IPC. An ACP tab would add: a new entry in the `Tabs` match, a data fetch Effect, and reuse `EmptyState`/`LoadingBlock` for states.

2. **For global status indicators (background activity, Syncthing):** Add to the `NavBar` (`navbar.rs:20`) `navbar-actions` div. Use `TaskContext` (`use_tasks()`) for background tasks and `ToastContext` for notifications. The `TaskIndicator` component (`feedback.rs:778`) already renders in the NavBar.

3. **For inline editor enhancements (Harper diagnostics):** Extend `NoteEditor` (`note_editor.rs:26`). It owns the `content` signal and `textarea` `NodeRef`. A diagnostics overlay can reuse `ErrorPanel` (`feedback.rs:662`) and `Badge` (`feedback.rs:432`) — positioned absolutely above the textarea using the existing `drag_hover` pattern.

4. **For capability settings:** Add a new tab to `SettingsPanel` (`settings_panel.rs:115`). The pattern is: (1) add string to `tabs` vec at line 137, (2) add match arm at line 172, (3) create the sub-component using `SettingCheckbox`/`Select`/`TextInput` + `save_settings` callback. The `save_settings` callback already serializes the full `AppSettings` and calls `settings_set_all` IPC.

### Shared Components to Reuse

| Capability Need | Reuse This Component | File |
|-----------------|---------------------|------|
| Any button | `Button` / `IconButton` | `ui/button.rs` |
| Modal dialogs | `Dialog` / `ConfirmDialog` / `AlertDialog` | `ui/dialog.rs` |
| Dropdown menus | `DropdownMenu` / `ContextMenu` | `ui/menu.rs` |
| Tabs | `Tabs` | `ui/nav.rs` |
| Form inputs | `TextInput` / `Textarea` / `Select` / `Checkbox` | `ui/input.rs` / `ui/selection.rs` |
| Status indicators | `Badge` / `StatusDot` / `Progress` | `ui/feedback.rs` |
| Toasts/notifications | `use_toast()` | `ui/feedback.rs:349` |
| Loading states | `LoadingBlock` / `Skeleton` / `Spinner` | `ui/feedback.rs` |
| Icons | `render_icon_view(Icon::Foo)` | `ui/icons.rs` |
| Empty states | `EmptyState` | `ui/info.rs` |
| Layout | `Panel` / `Section` / `Stack` / `Grid` | `ui/layout.rs` |

### Contexts to Use

```rust
// Capability components should capture at render time:
let nav = use_nav();           // ViewMode, vault_name, notes_index
let ws = use_workspace();      // tabs, active_path, content_version
let toasts = use_toast();      // ToastContext — ALL notifications
let tasks = use_tasks();       // TaskContext — background activity
```

All contexts are `Copy` and safe to thread into `spawn_local` async blocks.

### What to Avoid

- **Do not** create new IPC wrappers — use `crate::ipc::tauri_invoke()` directly
- **Do not** define new icon sets — extend the `Icon` enum in `ui/icons.rs` and add the Lucide re-export
- **Do not** create new dialog/modal implementations — use `Dialog`/`ConfirmDialog` from `ui/dialog.rs`
- **Do not** create new settings storage — add fields to `AppSettings` in `settings_panel.rs` and persist via the existing `save_settings` callback
- **Do not** duplicate the workspace/tab state — extend `WorkspaceContext` or `NavContext` instead

---

## 18. Implementation Notes

### 18.1 Compile Commands

From `docs/AGENTS.md`:
```
nabu-ui:  cargo check (from crates/nabu-ui/) — standalone workspace
root:     cargo check (includes nabu-core + src-tauri)
```

### 18.2 File Locations Summary

| Concern | Path |
|---------|------|
| Entry point | `crates/nabu-ui/src/lib.rs` |
| Root component | `crates/nabu-ui/src/components/app.rs` |
| IPC bridge | `crates/nabu-ui/src/ipc.rs` |
| Shared UI library | `crates/nabu-ui/src/components/ui/` |
| Navigation state | `crates/nabu-ui/src/components/navigation/state.rs` |
| Workspace state | `crates/nabu-ui/src/components/workspace.rs` |
| History (undo/redo) | `crates/nabu-ui/src/history.rs` |
| Theme | `crates/nabu-ui/src/lib.rs:43-126` |
| Design tokens | `src/styles/app.css` |
| Data models | `crates/nabu-ui/src/models.rs` (re-exports from nabu-core) |

### 18.3 IPC Command Registration

All IPC commands are defined in `src-tauri/src/commands.rs` as `#[tauri::command]` functions. The frontend does not need to know the Rust implementation — it only needs the command name (string) and the serialized argument/return types (via `serde_wasm_bindgen`).
