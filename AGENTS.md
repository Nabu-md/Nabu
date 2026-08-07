# Agent Guidelines — Nabu Phase W2 (Dioxus Migration)

## Project Overview

Nabu is a Tauri + Dioxus (CSR) desktop knowledge management app with a Rust core (`nabu-core`).
The UI lives in `crates/nabu-ui` (standalone workspace, Dioxus 0.6.3, cdylib for wasm-bindgen).
The Tauri backend lives in `src-tauri/`.

## Architecture

```
crates/
  nabu-core/          — Rust core: models, processing, storage, indexer
  nabu-ui/            — Dioxus 0.6.3 CSR frontend (cdylib)
src-tauri/            — Tauri commands & settings
```

## Compilation Notes

- `nabu-ui` is a **standalone workspace** (`crates/nabu-ui/Cargo.toml` has `[workspace]`).
  Compile from `crates/nabu-ui/`: `cargo check`
- Root workspace compiles `nabu-core` + `src-tauri` (not `nabu-ui`).
- Phase 0 migration: **Complete.** All LePtOS views have been migrated to Dioxus 0.6.3.
  - `app.rs` — all view modes wired (Settings, Inbox, Templates, History, Recovery, Statistics)
  - `settings_panel.rs` — 15 tabs, `AppSettings` struct, IPC persistence, 6 setting helper functions
  - `recovery/*` (session, save_status, diff_view, recovery_banner, version_history, recovery_manager) — full Dioxus migration
  - `inbox.rs` — ~1300-line Inbox component with batch handlers, drag/drop, keyboard shortcuts
  - `dictation_pill.rs` — opacity loading, clipboard cache, drop-zone IPC, mode switching
  - `statistics.rs` — vault metrics, growth histogram, tags, recently modified/created
  - `template_editor.rs` / `template_picker.rs` — backend-wired template management

## Dioxus 0.6.3 Migration Patterns

Key type mappings (LePtOS → Dioxus):
- `RwSignal<T>` → `Signal<T>` (with `mut` binding for `set`/`with_mut` which take `&mut self`)
- `impl IntoView` → `Element`
- `AnyView` → `Element`
- `view!{}` → `rsx!{}`
- `Callback::run()` → `Callback::call()`
- `Effect::new()` → `use_effect()`
- `for` loops in `rsx!` with `for item in &items { ... }`
- `prop:value=signal` → `value: "{signal.read()}"` + `oninput:`
- `on:click=` → `onclick:`
- `on:dblclick` → `ondoubleclick`
- `ev.data().as_web_event()` or `ev.as_web_event()` via `WebEventExt` from `dioxus::web`
- `Signal::read()` takes `&self`; `Signal::set()` and `Signal::with_mut()` take `&mut self`

Important patterns:
- **Function calls returning `Element` in `rsx!`**: Must be wrapped in `{ }` — e.g., `{setting_checkbox(...)}`
- **Signal Copy semantics**: `Signal<T>` is `Copy`, freely passed to helper functions and moved into closures. Use `mut` binding when calling `.set()` or `.with_mut()`
- **Callback Copy semantics**: `Callback<T>` is `Copy`, can be freely captured by closures
- **Precompute format strings before `rsx!`**: Dioxus 0.6 `rsx!` does NOT support `{if cond { "str" } else { "str" }}` inside string literals. Precompute conditional values
- **Precompute conditional class strings before `rsx!`**: Same limitation for class names. Use `let class = if cond { "a" } else { "b" };`
- **Pre-extract prop values before `rsx!`**: When a `move` closure captures a non-`Copy` variable, pre-extract fields to locals to avoid "borrow of moved value" errors
- **Raw closures for event handlers**: Use `move |_: MouseEvent| { ... }` directly instead of `Callback::new(closure)` — `EventHandler` auto-implements `From<F>` for `FnMut` closures
- **Component invocation in `rsx!`**: Component invocations like `InboxPreview { item: item.clone() }` must be used directly as children without additional `{ }` wrapping
- **Use `write_unchecked()` for interior mutability in `Fn` closures**: Inside `Fn`-bound contexts, use `signal.write_unchecked()` instead of `set()`/`with_mut()` when `&mut self` isn't available
- **Use `ev.as_web_event()` for web_sys access**: `Event<T>` derefs to `Rc<T>` → `T`, so `as_web_event()` works through double-deref
- **String clone for non-Copy captures in `FnMut` closures**: Clone `String` variables before `move` closures, or use `.clone()` inside the closure

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
| `statistics_get`     | `{}`                            | `VaultStatistics` |
| `versions_all`       | `{}`                            | `Vec<NoteSummary>` |
| `versions_list`      | `{ path: String }`              | `Vec<VersionMeta>` |
| `versions_get`       | `{ path, id }`                  | `String` (content) |
| `versions_diff`      | `{ path, id_a, id_b }`          | `Vec<DiffRow>` |
| `versions_restore`   | `{ path, id }`                  | `()` |
| `versions_duplicate` | `{ path, id, dest }`            | `()` |
| `snapshot_create`    | `{ path }`                      | `VersionMeta` |
| `template_list`      | `{}`                            | `Vec<Template>` |
| `template_save`      | `{ template: Template }`        | `()` |
| `template_delete`    | `{ name }`                      | `()` |
| `template_duplicate` | `{ name }`                      | `Template` |
| `template_set_favourite` | `{ name, favourite }`        | `()` |
| `inbox_*`            | Various                         | Various |

## UX Gap Matrix

### Dictation Pill (`components/dictation_pill.rs`)

| Gap                    | Status     | Fix                                             |
|------------------------|------------|-------------------------------------------------|
| Opacity loaded but not applied to DOM | **Done** | Bound opacity signal to `style` attribute on root div |
| Clipboard cache panel missing | **Done** | Added panel showing recent clipboard entries, with copy-to-restore on click |
| Drop zone doesn't call IPC | **Done** | Wired `ondrop` to `capture_file_drop` command with file reading |
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
| GeneralSettings / WhisprSettings / FileSettings are dead code | **Removed** | Removed from tab list; consolidated into 15 active tabs |

### Notifications & Undo/Redo

| Gap                    | Status     | Fix                                             |
|------------------------|------------|-------------------------------------------------|
| No toast on undo/redo  | **Done**   | `history.rs` fires toasts via `use_toast()`; navbar.rs wired undo/redo buttons |
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
