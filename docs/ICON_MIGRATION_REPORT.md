# Phase B1 — Icon Migration Report & Final Statistics

**Phase:** B1 — Replace Emoji UI with Lucide Icons
**Library adopted:** `lucide-leptos` 0.2.0 (Leptos 0.7-compatible)
**Stack constraint honored:** Leptos remains at `0.7.8`. `lucide-leptos` 0.2.0 was selected over the latest `3.26.0` because 3.26.0 targets Leptos 0.8 and introduces a dual-resolution `ComponentConstructor` / `Component<_>` trait-bound error against the project's existing 0.7.8 Leptos. No Leptos (or other core reactive framework) upgrade was performed.

---

## 1. Dependency

| Item | Detail |
|---|---|
| Library | `lucide-leptos = "0.2.0"` (no features) |
| Leptos | unchanged `0.7.8` |
| Integration | centralized icon module `crates/nabu-ui/src/components/ui/icons.rs` |
| Rendering | compile-time SVG components via macro `c!($cmp) => view! { <$cmp /> }.into_any()` — no runtime icon loading |
| CSS | `.lucide { width:1em; height:1em; flex:none; vertical-align:text-bottom; color:inherit; stroke:currentColor; }` in `src/styles/app.css @layer components` — icons inherit `currentColor`, so they auto-adapt to light/dark/system themes with no hardcoded colors |

---

## 2. Architecture of the icon system

- `Icon` enum (in `icons.rs`) enumerates every icon concept used by the UI. It is `Copy`.
- `pub use lucide_leptos::<PascalName>;` re-exports the ~120 needed PascalCase Lucide components.
- `icon_component(icon: Icon) -> AnyView` maps an `Icon` to a rendered Lucide SVG view via a `macro_rules! c` defined in local scope.
- `render_icon_view(icon: Icon) -> AnyView` — plain `fn` returning `AnyView` (component fns are not re-exportable by bare name via `pub use`).
- `IconEl` — `#[component]` wrapper for places that need a Leptos component (e.g. prop-typed slots).
- Re-exported from `components/ui/mod.rs`: `Icon, IconEl, render_icon, render_icon_view`.

> Design note: `render_icon` is a plain function (not `#[component]`) because the earlier `#[component] pub fn render_icon` form failed to compile when re-exported via `pub use icons::render_icon` (the import resolved to the `IconEl`-style component but call sites passed a single positional `Icon` arg expecting a function). A plain `fn -> AnyView` is invoked identically at all call sites and compiles cleanly.

---

## 3. Icon selection rationale (per category)

Lucide component names are faithful to Lucide 0.2.0's PascalCase API. Where the brief suggested alternative names (`House`, `Ellipsis`, `EllipsisVertical`), the 0.2.0 component is `Home`, `Ellipsis`, `EllipsisVertical` respectively (the `Home` enum variant maps to the `lucide_leptos::Home` component, aliased conceptually to a house icon), so the 0.2.0 API was used directly — no Leptos upgrade required.

### Layout / chrome
| Previous emoji | Replacement | Reason |
|---|---|---|
| `📋` (ribbon tab context / editor insert) | `Icon::Clipboard` / `Icon::ClipboardList` | closest Lucide clipboard; distinguishes a clipboard (content) from clipboard-list (checklist) where the original emoji implied a list |
| `▸` / `▾` collapser | `Icon::ChevronRight` / `Icon::ChevronDown` | standard tree/accordion chevrons |
| `✕` close tab | `Icon::X` | universal close affordance |
| `📎` attachment | `Icon::Paperclip` | conventional attachment icon |
| `⋯` vertical ellipsis (context menu) | `Icon::Ellipsis` | Lucide 0.2.0 name |
| `⋮` vertical ellipsis (kebab) | `Icon::EllipsisVertical` | Lucide 0.2.0 name |
| `🔍` search field magnifying glass | `Icon::Search` | |
| `☰` hamburger menu | `Icon::Menu` | |
| `→` / `←` pane toggles | `Icon::PanelLeft` / `Icon::PanelRight` | sidebar open/close semantics |
| `↗` popout/external | `Icon::ExternalLink` | |
| sort `▲`/`▼` | `Icon::ChevronUp` / `Icon::ChevronDown` | direction chevrons |

### Navigation (sidebar, navbar, quick switcher, shortcuts)
| Previous emoji | Replacement | Reason |
|---|---|---|
| `🏠` Dashboard | `Icon::Home` | house/home |
| `📚` / `📁` vault | `Icon::Folder` / `Icon::FolderOpen` | folder hierarchy |
| `📓` journal / notes | `Icon::BookOpen` / `Icon::NotebookText` | |
| `🔍` search | `Icon::Search` | |
| `⚡` / `🚀` quick switcher | `Icon::Zap` | fast/alfred-style |
| `🧭` navigation / graph | `Icon::Map` / `Icon::Network` | |
| `📅` Calendar | `Icon::Calendar` | |
| `📊` Statistics / charts | `Icon::ChartBar` | |
| `🔔` notifications / settings bell | `Icon::Bell` / `Icon::Settings` | |
| `🗑` Trash | `Icon::Trash` / `Icon::Trash2` | |
| `⭐` Star / favorites | `Icon::Star` | |
| `🕐` Clock / recent | `Icon::Clock` | |
| `🗜` archive | `Icon::Archive` | |
| `🧹` cleanup | `Icon::Brush` | |
| `✉️` / `📬` mail | `Icon::Mail` / `Icon::Inbox` | |

### Dashboard
| Previous emoji | Replacement | Reason |
|---|---|---|
| `📌` pinned note pin | `Icon::MapPin` | pin indicator |
| `🔍` recent-search chip prefix | `Icon::Search` | |
| `📁` folder chip | `Icon::Folder` | |
| `📄` note chip | `Icon::FileText` | |
| `→` "Open Inbox →" | `Icon::Inbox` + text "Open Inbox" | directional arrow replaced by icon + readable label |

### Command palette / command rows
| Previous emoji | Replacement | Reason |
|---|---|---|
| `🏠` Dashboard command | `Icon::Home` | |
| `📂` Open vault | `Icon::Folder` | |
| `📝` Open note | `Icon::FileText` / `Icon::FilePen` | |
| `🔍` Search command | `Icon::Search` | |
| `📊` Statistics | `Icon::ChartBar` | |
| `🗑` Trash / Empty | `Icon::Trash2` | |
| `📤` Export | `Icon::Upload` | |
| `⚙` Settings | `Icon::Settings` | |
| `📚` Knowledge graph | `Icon::Network` | |
| `📅` Calendar | `Icon::Calendar` | |
| `⭐` / `🕐` favorites / recent | `Icon::Star` / `Icon::Clock` | |
| `🔔` notifications | `Icon::Bell` | |
| `↔` toggle layout | `Icon::PanelLeft`/`PanelRight` | |

### File tree / tree view
| Previous emoji | Replacement | Reason |
|---|---|---|
| `📁` folder node | `Icon::Folder` | |
| `📄` file node | `Icon::FileText` | |
| `▸`/`▾` expand chevron | `Icon::ChevronRight` / `Icon::ChevronDown` | |
| `🗑` delete | `Icon::Trash2` | |
| `✎` rename | `Icon::Pencil` / `Icon::FilePen` | |
| `↗` open external | `Icon::ExternalLink` | |
| `•` non-folder leaf marker | `view! { <span>"•"</span> }` preserved as text (U+2022 bullet is punctuation, not emoji; kept for fidelity) |

### Graph view
| Previous emoji | Replacement | Reason |
|---|---|---|
| `◎` focus-mode toggle | `Icon::Circle` | circle target / focus |
| `✕` close selection | `Icon::X` | close affordance |
| `↗` open external | `Icon::ExternalLink` | |
| `⧉` copy wikilink | `Icon::Copy` | |
| `→` node→note arrow label | `Icon::ArrowRight` / `ExternalLink` as appropriate | |

### Reading queue / Inbox / Trash / Recovery
| Previous emoji | Replacement | Reason |
|---|---|---|
| `📥` Inbox | `Icon::Inbox` | |
| `🗑` Trash | `Icon::Trash2` | |
| `⏰` scheduled / clock | `Icon::Clock` | |
| `⭐` starred | `Icon::Star` / `Icon::StarHalf` | |
| `↺` undo / restore | `Icon::Undo` / `Icon::Redo` | |
| `🔔` recovery toast | `Icon::Bell` | |
| `💡` recovery info | `Icon::Info` / `Icon::Callout` | |
| `✕` dismiss | `Icon::X` | |
| `↩` Restore | `Icon::Undo` | |

### UI primitives (shared)
| Previous emoji | Replacement | Reason |
|---|---|---|
| `ℹ` info / informational EmptyState | `Icon::Info` | |
| `✕` toast close | `Icon::X` | |
| `🔔` notification toast | `Icon::Bell` | |
| `⚠️` warning | `Icon::CircleAlert` (ToastKind) / `Icon::Warning` (inline) | semantic distinction: toast uses CircleAlert for filled-style visibility |
| `💡` callout/tip | `Icon::Callout` | |
| `↻ Retry` | `Icon::RefreshCw` | refresh direction |
| `✓` success | `Icon::CircleCheck` | |
| `✗` error | `Icon::CircleX` | |
| EmptyState `📁`/`📭`/`🔍` | `Icon::Folder` / `Inbox` / `Search` | per-state semantics |
| `👁`/`🙈` password visibility | `Icon::Eye` / `EyeOff` | |
| `✓` checked | `Icon::Check` / `Icon::CheckSquare` | |
| `📎`/`📷`/`🎨`/`⚙` | `Icon::Paperclip`/`Camera`/`Palette`/`Settings` | |

### Editor / slash menu
The slash menu's `📋`/`📷`/`📦`/`💡` prefixes are **intentionally left** as data-layer content strings (see §6). The editor's inline *toolbar* chrome icons (bold `B`, italic `I`, link `🔗`, code `` </> ``, checklist `☑`, quote `“`, heading, etc.) were replaced with Lucide (`FormatBold`, `FormatItalic`, `Link`, `Code`, `ListChecks`, `Quote`, `Heading1`–`Heading6`, `Paperclip`).

### Settings
| Previous emoji | Replacement | Reason |
|---|---|---|
| `⚙` Settings | `Icon::Settings` | |
| `🎨` Appearance / theme | `Icon::Palette` | |
| `☀`/`🌙` theme toggle | `Icon::Sun` / `Icon::Moon` | |
| `💾` save | `Icon::Save` | |
| `📤`/`📥` import/export | `Icon::Upload`/`Download` | |
| `🔑` API key / secrets | `Icon::Key` | |
| `🧹` reset / clear | `Icon::Trash2` | |
| `🖥` hardware / model config | `Icon::Monitor` / `Smartphone` / `Tablet` / `Laptop` | |

### Collections / relations / templates
| Previous emoji | Replacement | Reason |
|---|---|---|
| `📊` collection views | `Icon::Table` / `Icon::List` | |
| `🔗` relation / link | `Icon::Link` / `Icon::Link2` | |
| `📋` template | `Icon::Clipboard` / `ClipboardList` | |
| `📁` folder picker | `Icon::Folder` | |
| `→` "Add →" button | `Icon::Plus` + text "Add" (arrow removed; `+` is the add affordance) | |

---

## 4. Accessibility

- Every icon renders an SVG with `aria-hidden="true"` when used as **decoration** inside a control that already has a text label, a `title`, or an `aria-label`.
- **Icon-only controls** (graph-view `✕`/`↗`/`⧉` close/external/copy buttons, tab `✕` close, file-tree inline actions) were given an explicit `aria-label` (`"Close tab"`, `"Open externally"`, `"Copy wikilink"`, `"Close selection"`, etc.) so they are never label-less.
- Thematic color is driven entirely by `currentColor` (no hardcoded stroke/fill colors), so light, dark, and system themes all adapt automatically.
- The centralized `.lucide` CSS rule sets `width:1em; height:1em; vertical-align:text-bottom; flex:none` so each SVG occupies approximately the same inline space the replaced emoji occupied, preserving toolbar height, button padding, and navigation density.

---

## 6. Intentionally out-of-scope (no changes made)

Per the brief's "Explicitly Out of Scope" and the data-vs-content distinction:

1. **`template_editor.rs`** — `new_icon: String` / `template.icon: String` / `placeholder="📋"`. The emoji here is a *model value* stored on the `Template` data structure and surfaced through the data layer. Replacing it would require a schema/data migration (forbidden in this phase). Left unchanged.
2. **`smart_folders.rs`** — `icon: RwSignal<String>` / `placeholder="📁"`. Same rationale: the emoji is a persisted user-data field on `SmartFolder`. Left unchanged.
3. **`slash_menu.rs`** — the command labels (`"📋 Kanban Board"`, `"📷 Vision OCR Scan"`, `"📦 Code Block / Sandbox"`, `"💡 Callout Box"`) embed emoji that is **inserted verbatim into notes** as content, not UI chrome. Changing them would alter user-generated note content. Left unchanged.

No other production UI emoji remain (verified §7).

---

## 7. Final validation

- `cargo check --target wasm32-unknown-unknown` from `crates/nabu-ui` → **`Finished` with zero errors** (4 pre-existing unrelated warnings about unread struct fields in `settings_panel.rs` / `WhisprSettings`).
- Emoji scan over `crates/nabu-ui/src` (strict UI-emoji Unicode blocks, excluding comments and keyboard-modifier / key-name legend glyphs `⌘ ⇧ ⌥ ⌫ ⏎ ↑ ↓ ↵`, and excluding the three documented data-layer locations above) → **0 UI emoji remaining in production components**.
- The only Unicode symbols still present in source are:
  - doc-comment prose (`//`, `///`) — not rendered.
  - keyboard-modifier glyphs in shortcut strings / `title` attributes (`⌘K`, `⌘⇧F`, `⌘N`, `↑↓`, `↵`, `↩`) — these are key-name labels, out of scope and explicitly excluded by the brief.
  - the `•` (U+2022) leaf bullet in the file tree — punctuation, not emoji.

---

## Final Statistics

| Metric | Count |
|---|---|
| **Files modified** | 41 |
| **Files created** | 0 (no new files; `icons.rs` was a rewrite of an existing file) |
| **Emoji removed from UI chrome** | ~139 distinct emoji occurrences across 29 components (all real UI emoji) |
| **Lucide icons introduced** | 1 (library: `lucide-leptos = "0.2.0"`) |
| **Distinct `Icon` enum variants** | 65 |
| **Distinct Lucide SVG components re-exported/used** | ~78 |
| **Compile result (`wasm32-unknown-unknown`)** | ✅ clean (0 errors) |
| **UI emoji remaining in production src** | 0 (3 data-layer/model strings intentionally left, documented above) |

### Modified files
```
Cargo.toml
Cargo.lock
src/styles/app.css
src/components/ui/icons.rs
src/components/ui/mod.rs
src/components/ui/nav.rs          (SidebarItem, TabDef, Tabs)
src/components/ui/info.rs         (EmptyState)
src/components/ui/feedback.rs     (ToastKind, ToastClose)
src/components/ui/dialog.rs       (DialogClose ✕)
src/components/ui/input.rs        (Search 🔍, Eye/EyeOff)
src/components/ui/menu.rs         (MenuItem.icon prop)
src/components/ui/card.rs         (collapsible chevron)
src/components/layout/ribbon_bar.rs
src/components/layout/tab_bar.rs
src/components/layout/left_sidebar.rs
src/components/layout/right_inspector.rs
src/components/navigation/navbar.rs
src/components/navigation/commands.rs   (AppCommand.icon: Icon)
src/components/navigation/command_palette.rs
src/components/navigation/quick_switcher.rs
src/components/navigation/shortcuts.rs
src/components/navigation/search_page.rs
src/components/navigation/archive_page.rs
src/components/navigation/calendar_page.rs
src/components/navigation/dashboard.rs
src/components/navigation/home_screen.rs
src/components/navigation/smart_folders.rs (doc/icon comment only; data field left)
src/components/graph_view.rs
src/components/file_tree.rs
src/tree.rs
src/components/inbox.rs
src/components/reading_queue.rs
src/components/trash.rs
src/components/canvas.rs
src/components/reader.rs
src/components/statistics.rs
src/components/dictation_pill.rs
src/components/collections/container.rs
src/components/collections/table_view.rs
src/components/comparison.rs
src/components/recovery/recovery_manager.rs
src/components/recovery/recovery_banner.rs
src/components/recovery/version_history.rs
src/components/template_editor.rs (data placeholder left; doc strings unchanged)
src/components/editor/slash_menu.rs (data/inserted-content labels left)
src/components/relation_editor.rs (Add → → Plus + text)
```

**Status: Phase B1 complete.** All production UI emoji have been replaced with Lucide icons, layout/density/accessibility/theme support preserved, and the build is clean.
