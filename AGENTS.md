# Agent Guidelines — Nabu Phase W2

## Project Overview

Nabu is a Tauri + Leptos (CSR) desktop knowledge management app with a Rust core (`nabu-core`).
The UI lives in `crates/nabu-ui` (standalone workspace, Leptos 0.7.8, cdylib for wasm-bindgen).
The Tauri backend lives in `src-tauri/`.

## Architecture

```
crates/
  nabu-core/          — Rust core: models, processing, storage, indexer
  nabu-ui/            — Leptos 0.7 CSR frontend (cdylib)
src-tauri/            — Tauri commands & settings
```

## Compilation Notes

- `nabu-ui` is a **standalone workspace** (`crates/nabu-ui/Cargo.toml` has `[workspace]`).
  Compile from `crates/nabu-ui/`: `cargo check`
- Root workspace compiles `nabu-core` + `src-tauri` (not `nabu-ui`).
- Recent fix: `collections/view_switcher.rs` and `collections/shared/context.rs` were using Yew
  types; converted to Leptos `#[component]` + `Signal`.
- Recent fix: Missing icon re-exports (`GitCompare`, `GalleryVerticalEnd`) in `icons.rs`.
- Recent fix: `inbox.rs` had `tabIndex` → `tabindex`, nested `JsValue` in `serde_json::json!`,
  moved-value in closure, and E0515 lifetime error.

## IPC Commands (src-tauri/src/commands.rs)

| Command              | Args                          | Returns         |
|----------------------|---------------------------------|-----------------|
| `get_settings`       | —                               | `AppSettings`   |
| `settings_get`       | `{ key: String }`               | `serde_json::Value` |
| `settings_set`       | `{ key, value }`                | `Result<(), String>` |
| `settings_set_all`   | `AppSettings` (serialized)      | `Result<(), String>` |
| `open_settings`      | —                               | `Result<(), String>` |
| `toggle_dictation_pill` | —                            | `Result<(), String>` |
| `capture_file_drop`  | `{ filename, mime_type, data }` | `String` (id)   |
| `start_dictation`    | —                               | `Result<(), String>` |

## UX Gap Matrix

### Dictation Pill (`components/dictation_pill.rs`)

| Gap                    | Status     | Fix                                             |
|------------------------|------------|-------------------------------------------------|
| Opacity loaded but not applied to DOM | **Done** | Bound opacity signal to `style` attribute on root div |
| Clipboard cache panel missing | **Done** | Added panel showing recent clipboard entries, with copy-to-restore on click |
| Drop zone doesn't call IPC | **Done** | Wired `on:drop` to `capture_file_drop` command with file reading |
| Copy button does nothing | **Done** | Implemented clipboard write via `navigator.clipboard().write_text()` |

### Settings Panel (`components/settings/settings_panel.rs`)

| Gap                    | Status     | Fix                                             |
|------------------------|------------|-------------------------------------------------|
| font_size not exposed  | **Done**   | Added slider in AppearanceSettings              |
| line_height not exposed | **Done**  | Added slider in AppearanceSettings              |
| reduced_motion not exposed | **Done** | Added toggle in AppearanceSettings               |
| high_contrast not exposed | **Done** | Added toggle in AppearanceSettings                |
| sidebar_width not exposed | **Done** | Added slider in AppearanceSettings                |
| inspector_width not exposed | **Done** | Added slider in AppearanceSettings              |
| tab_size not exposed   | **Done**   | Added number input in EditorSettings              |
| word_wrap not exposed  | **Done**   | Added toggle in EditorSettings                    |
| spell_check not exposed | **Done**  | Added toggle in EditorSettings                    |
| auto_save_interval not exposed | **Done** | Added number input in EditorSettings          |
| graph_show_tags_as_badges not exposed | **Done** | Added toggle in GraphSettings       |
| font_size not exposed  | **Done**   | Added slider in AppearanceSettings              |
| line_height not exposed | **Done**  | Added slider in AppearanceSettings              |
| reduced_motion not exposed | **Done** | Added toggle in AppearanceSettings               |
| high_contrast not exposed | **Done** | Added toggle in AppearanceSettings                |
| sidebar_width not exposed | **Done** | Added slider in AppearanceSettings                |
| inspector_width not exposed | **Done** | Added slider in AppearanceSettings              |
| tab_size not exposed   | **Done**   | Added number input in EditorSettings              |
| word_wrap not exposed  | **Done**   | Added toggle in EditorSettings                    |
| spell_check not exposed | **Done**  | Added toggle in EditorSettings                    |
| auto_save_interval not exposed | **Done** | Added number input in EditorSettings          |
| graph_show_tags_as_badges not exposed | **Done** | Added toggle in GraphSettings       |
| GeneralSettings / WhisprSettings / FileSettings are dead code | **Open** | Remove or wire into tabs |

### Notifications & Undo/Redo

| Gap                    | Status     | Fix                                             |
|------------------------|------------|-------------------------------------------------|
| No toast on undo/redo  | **Done**   | `history.rs` already fires toasts via `use_toast()`; navbar.rs wired undo/redo buttons |
| No background progress for long ops | **Open** | Add progress signal to toast system       |

### Context Menus

| Gap                    | Status     | Notes                                           |
|------------------------|------------|-------------------------------------------------|
| File tree has context menu | Done | Routes through history.rs                       |
| Graph view has context menu | Done | Ad-hoc clipboard writes                         |
| Inbox lacks context menu | **Open** | Need to add                                    |
| Collections lack context menu | **Open** | Need to add                                   |
| No shared clipboard cache component | **Open** | Both graph and file tree write ad-hoc    |

### Accessibility

| Gap                    | Status     | Notes                                           |
|------------------------|------------|-------------------------------------------------|
| Keyboard navigation across views | Partial | Global shortcuts exist, but per-view nav varies |
| ARIA labels on icon-only buttons | Partial | Some have aria-label, need audit              |
| Focus management on modal dialogs | Partial | Dialogs have role="dialog" but focus trap unverified |
