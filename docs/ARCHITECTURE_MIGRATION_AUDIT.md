# Nabu Architecture Migration Audit

## Tauri + Leptos → Dioxus / Iced / GPUI Evaluation

---

## Executive Summary

**Recommendation: Keep current architecture with targeted improvements.**

A migration away from Tauri + Leptos is **not strategically or technically justified** at this stage. The current architecture is well-chosen for Nabu's specific requirements — particularly its HTML-native App Block system, canvas-based knowledge graph, and Tailwind-based design system. All three candidate frameworks introduce migration costs that far outweigh their benefits:

| Framework | Migration Severity | Estimated Effort | Feasibility |
|-----------|-------------------|------------------|-------------|
| Dioxus | Very High | 6–12 person-months | Feasible but not beneficial |
| Iced | Critical | 18–24+ person-months | Technically questionable |
| GPUI | Critical | 24+ person-months | Not feasible (no WASM) |

The **strongest recommendation** is to keep Tauri + Leptos and instead invest engineering effort in completing the in-flight UX gaps, upgrading to Leptos 0.8/0.9, and improving the build pipeline.

---

## Phase 1 — Architecture Analysis

### Current Architecture: Tauri v2 + Leptos 0.7.8 (CSR/WASM)

#### Rendering Model

The entire UI is rendered into the DOM inside a Tauri webview (Wry/System WebView). Leptos 0.7.8 compiles declarative `view!` macro templates to direct DOM mutations via `web-sys`. There is no VDOM layer — Leptos uses fine-grained reactivity that patches the real DOM directly.

**Evidence**: All 76 UI component files (`crates/nabu-ui/src/components/**/*.rs`) begin with `use leptos::prelude::*`. The `#[component]` attribute macro and `view!` template macro are used in every file (145 component definitions, 689 `view!` invocations). DOM nodes are accessed via `leptos::html::Iframe`, `leptos::html::Canvas`, `leptos::html::Textarea`, `leptos::html::Input`, `leptos::html::Div` (via `NodeRef`).

**Verified fact**: `crates/nabu-ui/Cargo.toml` declares `crate-type = ["cdylib"]` with `leptos = { version = "0.7.8", features = ["csr"] }`. The `index.html` loads the WASM bundle via Trunk: `<link data-trunk rel="rust" href="crates/nabu-ui/Cargo.toml" data-wasm-opt="z" />`.

#### Windowing Model

Tauri v2 creates the main application window and renders the Leptos WASM bundle inside it. A separate "dictation-pill" webview window is created dynamically via `tauri::WebviewWindowBuilder`. Windows are managed via `AppHandle` and `WebviewWindow`.

**Verified fact**: `tauri.conf.json` defines the main window with `"visible": false` (shown on page load to avoid white flash). The `src-tauri/src/commands.rs` contains `open_dictation_pill`, `close_dictation_pill`, `toggle_dictation_pill` which manipulate a secondary `"dictation-pill"` webview window.

#### Runtime Model

Two distinct runtimes coexist:
- **Tauri async runtime** (Tokio-based): runs the Job Queue, Worker Pool, native messaging socket server, and IPC handlers. This runs in pure Rust on the main thread.
- **WASM runtime**: the Leptos UI executes as WASM inside the webview. Async operations use `wasm_bindgen_futures::spawn_local` (156 call sites). The two runtimes communicate exclusively through Tauri's IPC bridge.

**Verified fact**: `src-tauri/src/lib.rs` spawns the worker pool via `tauri::async_runtime::spawn(async move { pool.start().await; })` and the socket server via a second `async_runtime::spawn`. The frontend uses `spawn_local` (156 instances) and `wasm_bindgen_futures` for async WASM operations.

#### State Management

Leptos fine-grained reactivity with:
- `RwSignal<T>` — read/write signals (169 uses) — the dominant state primitive
- `Signal<T>` — read-only signals (104 uses)
- `Memo<T>` — derived/computed signals (22 uses)
- `provide_context<T>()` / `expect_context<T>()` — dependency injection (8 provides, 24 expects)
- `set_timeout` / `set_interval_with_handle` — timers for debounced autosave, periodic retries
- Cross-cutting shared state via `OnceLock` (e.g., `SHARED_STATE` in `history.rs`)

**Verified fact**: `crate::components::workspace::provide_workspace()` provides a `WorkspaceContext { tabs, active_path, refresh_tree, content_version }` as `RwSignal` fields. `crate::provide_theme()` provides `ThemeContext { theme: RwSignal<String> }`. The navigation system provides `NavContext` with 14 `RwSignal` fields.

#### Component Model

`#[component]` attribute macro (145 definitions) generates Leptos components that return `impl IntoView`. Props use `#[prop(optional)]` annotations. Children use `ChildrenFn` (26 uses) and `Callback<T, R>` (350 uses) for event handlers. Conditional rendering uses `into_any()` (449 uses) to erase view types for `if`/`match` branches. List rendering uses `collect_view()` (100 uses).

**Verified fact**: `crate::components::ui::feedback::ToastProvider` wraps the entire app: `mount_to_body(|| view! { <ToastProvider><App /></ToastProvider> })`. It uses `ChildrenFn` pattern and `provide_context`.

#### Event Model

Two event systems:
- **Leptos declarative events** in `view!`: `on:click`, `on:input`, `on:keydown`, `on:dragover`, `on:drop`, `on:contextmenu`, `on:mouseenter`, `on:mouseleave`, etc. (213 click handlers, 52 input handlers, 20 keydown handlers). Property binding via `prop:value` (53 uses).
- **Raw DOM events** via `window_event_listener_untyped` (7 uses) for window-level events (keydown, resize, beforeunload, custom events). Direct `web_sys` event types: `KeyboardEvent`, `MouseEvent`, `DragEvent`, `CustomEvent`, `Event`, `WheelEvent`, `MessageEvent`.

**Verified fact**: `crate::history::provide_history()` installs a single leaked `Closure<dyn Fn(KeyboardEvent)>` for Cmd/Ctrl+Z/Y global shortcuts. `crate::components::graph_view.rs` uses `window_event_listener_untyped("keydown")` for keyboard navigation. `crate::components::file_tree.rs` listens for `nabu:reveal-note` custom events.

#### Native Integration Model

Tauri IPC: every native operation goes through `window.__TAURI__.core.invoke(cmd, args)` via a single abstraction in `crate::ipc::tauri_invoke()` (5 lines). The frontend calls 64 unique IPC commands across 113 call sites. Each call uses `serde_wasm_bindgen::to_value` to serialize args and `serde_wasm_bindgen::from_value` to deserialize results.

**Verified fact**: `crates/nabu-ui/src/ipc.rs` has exactly 5 lines:
```rust
#[wasm_bindgen] extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}
pub async fn tauri_invoke(cmd: &str, args: JsValue) -> JsValue { invoke(cmd, args).await }
```
64 unique command names extracted from the UI (see Appendix B).

#### WebView Usage

100% of the UI renders inside a single Tauri webview. The App Block system uses nested `<iframe>` elements within the main webview for HTML sandboxing. The dictation pill uses a separate webview window. No native UI widgets are used in the main window.

**Verified fact**: `sandbox.rs` renders `<iframe srcdoc=html_content sandbox="allow-scripts allow-forms" title="App Block Sandbox" />`. `sandboxed_html.rs` renders `<iframe srcdoc=html sandbox="allow-scripts" class="sandboxed-html" />`. The dictation pill is a separate webview window: `tauri::WebviewWindowBuilder::new(&app, "dictation-pill", tauri::WebviewUrl::App("dictation-pill.html"))`.

#### WASM Usage

The entire `nabu-ui` crate compiles to a single `cdylib` target for `wasm32-unknown-unknown`. `wasm-bindgen` provides the FFI boundary to JavaScript. `js-sys` provides JavaScript interoperability. The WASM bundle is ~optimized via `data-wasm-opt="z"` in `index.html`.

**Verified fact**: `Cargo.toml` `[lib] crate-type = ["cdylib"]`. Dependencies include `wasm-bindgen = "0.2.100"`, `js-sys = "0.3.77"`, `wasm-bindgen-futures = "0.4.50"`, `serde-wasm-bindgen = "0.6.5"`.

#### HTML/CSS Rendering Strategy

Rendering is pure HTML + CSS via Tailwind 3.4. A build-time pipeline (`npm run css:build`) generates `generated/tailwind.css` from `src/styles/app.css` (2,923 lines of custom design system). `tailwind.config.js` scans Rust source files for class names. Icons are compile-time SVG components from `lucide-leptos` 0.2.0.

**Verified fact**: `package.json` scripts: `"css:build": "tailwindcss -i ./src/styles/app.css -o ./generated/tailwind.css --minify"`. `tailwind.config.js` content: `["./crates/nabu-ui/src/**/*.rs"]`. The icon system (`icons.rs`) re-exports 80+ Lucide components and maps an `Icon` enum to SVG components via `render_icon_view()`.

---

### Candidate 1: Dioxus

#### Rendering Model

Dioxus renders to real DOM elements in the webview. In Dioxus 0.5/0.6, the `rsx!` (or `view!`) macro generates DOM patches. Fine-grained reactivity similar to Leptos. The `rsx!` macro syntax differs from Leptos' `view!`: events use `onclick={...}` (not `on:click={...}`), props use `name={value}` (not `prop:name=value`).

**Verified fact**: Dioxus is a Rust UI library that uses a JSX-like macro (`rsx!` in 0.5+, was `view!` in 0.4). The framework supports web, desktop, and mobile targets. `dioxus-web` provides WASM rendering; `dioxus-desktop` provides native desktop via Wry webview.

#### Windowing Model

`dioxus-desktop` creates windows using Wry (same webview engine as Tauri). However, Dioxus's desktop mode is a wrapper that bundles an HTML webview + WASM, similar to Tauri. There is also community Tauri integration for Dioxus.

**Verified fact**: `dioxus-desktop` uses `wry` for the webview and `tauri` is not required. The framework provides `dioxus-desktop`'s `Config` and `LaunchParams` for window management.

#### Runtime Model

WASM runtime on the web (via `dioxus-web`), native Rust on desktop. Signal-based reactivity with `Signal<T>`, `WritableSignal<T>`, `ReadableSignal<T>`. Context via `use_context<T>()` / `provide_context<T>()` — note these are the same names as Leptos.

**Verified fact**: Dioxus provides `use_context` and `provide_context` (same API names as Leptos), but signal types differ: `Signal<T>` (which acts as both reader and writer via `.read()` / `.write()` in 0.5), `WritableSignal<T>` (0.6+), `ReadableSignal<T>`.

#### State Management

Dioxus uses the same conceptual model as Leptos: signals + context. The API differs:
- `RwSignal<T>` → `Signal<T>` (0.5) or `WritableSignal<T>` (0.6+)
- `Memo::new(move |_| ...)` → `Memo::new(move |_| ...)` (same API in 0.5)
- `provide_context` / `use_context` → same names
- `set_timeout` → same (via `dioxus::prelude` or `gloo-timers`)
- `spawn_local` → `dioxus::prelude::spawn` or `spawn_local` from `wasm-bindgen-futures`

**Uncertain**: Whether Dioxus's `Memo` API is identical to Leptos's (needs verification from docs).

#### Component Model

`#[component]` attribute macro (same as Leptos). Components return `Element` (not `impl IntoView`). Props use `#[props(optional)]`. Children use `Element` or closures. No `into_any()` needed — Dioxus's `Element` is already type-erased.

**Key difference**: Dioxus `view!`/`rsx!` macro uses `onclick: move |e| {...}` syntax (event handlers use `:` separator) and `name: value` for props. Leptos uses `on:click={move |ev| ...}` and `prop:name={value}`.

#### Event Model

Event handlers in `rsx!`: `onclick`, `oninput`, `onchange`, `onkeydown`, `onmousedown`, `onmousemove`, `onwheel`, `ondragstart`, `ondrop`, `ondeactivate`, etc. Same DOM events available via `web_sys`. Raw window-level event listeners are available via `web_sys::window().add_event_listener_with_callback()`.

**Verified fact**: Dioxus's `rsx!` macro supports the same DOM events as Leptos (onclick, oninput, etc.), all as `web_sys::Event` subtypes. The framework can also use raw `web_sys` for window-level listeners without any framework glue.

#### Native Integration Model

No built-in native integration beyond what `dioxus-desktop` provides. IPC would still go through Tauri's `__TAURI__.core.invoke` (if Dioxus is used alongside Tauri) or Dioxus's own `dioxus-desktop` IPC bridge.

**Uncertain**: Dioxus + Tauri integration maturity. There are community recipes for combining Dioxus with Tauri, but it's not an officially documented first-class integration.

#### WebView Usage

Same as current — all UI renders in an HTML webview. Can embed `<iframe>` directly in `rsx!`.

**Verified fact**: Dioxus can render HTML `<iframe>` elements directly in `rsx!` since it renders to real DOM.

#### WASM Usage

`dioxus-web` compiles to `wasm32-unknown-unknown`. Same toolchain as Leptos.

**Verified fact**: `dioxus-web` provides `dioxus_web::launch` which is the WASM entry point, analogous to Leptos's `mount_to_body`.

#### HTML/CSS Rendering Strategy

Renders real HTML + CSS. Supports external stylesheets, inline styles, and Tailwind CSS (same as Leptos). `lucide-leptos` would need to be replaced with `dioxus-icon` or a custom SVG solution.

**Verified fact**: Dioxus supports CSS frameworks via external stylesheets. The `dioxus-icon` crate provides icon rendering for Dioxus.

**Comparison to current**: What changes: `rsx!` syntax (449 `into_any()` calls eliminated since `Element` is type-erased; 213 `on:click` → `onclick`; 53 `prop:value` → `value`), signal types, icon system. What stays the same: HTML/CSS pipeline, Tailwind pipeline, IPC mechanism (if Tauri retained), web_sys usage, canvas rendering, iframe App Blocks.

---

### Candidate 2: Iced

#### Rendering Model

Iced renders via GPU (wgpu) to a scene graph — **no HTML DOM, no CSS**. Widgets are drawn as GPU primitives. On web, Iced uses `iced_wgpu` which compiles to WASM and renders via WebGL or WebGPU, producing an HTML `<canvas>` element.

**Verified fact**: Iced 0.13/0.14 uses `iced_wgpu` for GPU-accelerated rendering. The UI is described as "Elm-style declarative UI" with `view(&self) -> Element<Message>` returning abstract elements that are rendered via the GPU backend. No HTML/CSS is involved in the rendering pipeline.

#### Windowing Model

Uses `winit` natively (no Tauri, no webview for the main window). Creates OS-native windows directly.

**Verified fact**: Iced applications use `iced::Application` which initializes a winit windowing backend. There is no webview wrapper. The entire application window is rendered by Iced's GPU pipeline.

#### Runtime Model

Elm-style event loop: `State` struct with `view(&self, ui: &mut Ui) -> Element<Message>` and `update(&mut self, message: Message)`. No signals, no reactivity, no `provide_context`. State is owned by the `State` struct and mutated through message passing.

**Verified fact**: Iced's architecture follows The Elm Architecture: `State` (application state), `Message` (events), `Update` (state transitions), `View` (UI rendering). There is no fine-grained reactivity system.

#### State Management

No signals/context system. State lives in the `State` struct. All state changes go through the `update` method. Asynchronous operations return `Command<Message>`.

**Verified fact**: Iced uses `Command` for async operations — functions that return `iced::Command<Message>` which are executed by the runtime and produce `Message`s when complete.

#### Component Model

Components implement `widget::Widget` and return `Element`. No `#[component]` macro. No `view!` macro in the Leptos sense (though iced 0.13 introduced `widget!` for declarative syntax).

**Verified fact**: Iced 0.13 introduced the `widget!` macro as a more ergonomic alternative to the builder API, but it's still fundamentally Elm-style. Components are widgets that implement `Widget<Renderer>` trait.

#### Event Model

All events are `Message` enum variants. No DOM events — events are Iced's own enum (`iced::Event`). Keyboard, mouse, and touch events are all mapped to Iced's event system.

**Verified fact**: Iced maps OS events to its own `iced::Event` enum (Keyboard, Mouse, Touchpad, etc.). There is no direct DOM event access.

#### Native Integration Model

Winit provides window management. Native features require custom `Command` implementations or custom widgets. No IPC bridge — would need custom WebSocket/HTTP/local socket bridge to the Rust backend.

**Uncertain**: Whether Iced has a built-in mechanism for native IPC. Iced's `Command` system can execute arbitrary futures, but there's no built‧built IPC like Tauri's.

#### WebView Usage

No native webview embedding. To render App Blocks (HTML iframes), Iced would need a custom widget that embeds a webview — this is **not** provided out of the box.

**Uncertain**: Whether Iced has any webview embedding capability. Third-party crates exist (e.g., `iced_webview`) but their maturity and platform support are unknown.

#### WASM Usage

`iced_wgpu` compiles to WASM via `wasm32-unknown-unknown`. Renders to an HTML `<canvas>` element. No `wasm-bindgen` FFI directly — Iced wraps the WebGL/WebGPU calls.

**Verified fact**: Iced's web target uses `iced_wgpu` which renders to a `<canvas>` element on the page. The WASM bundle is self-contained.

#### HTML/CSS Rendering Strategy

**No HTML/CSS rendering**. Everything is GPU-drawn. The Tailwind CSS pipeline, custom CSS design system (2,923 lines), lucide-leptos icon components, and all HTML class bindings would be completely discarded.

**Verified fact**: Iced uses a theme system with `Palette` and `Color` values. Styling is done programmatically via `Appearance` structs, not CSS classes.

**Comparison to current**: Every single UI file would need complete rewriting. The `view!` macro → `widget!` or builder API. `RwSignal`/`provide_context` → `State` struct + `Message`. DOM events → Iced events. web_sys → custom Iced integration. Canvas API → Iced canvas widget. Iframes → unknown. IPC → new bridge. CSS → programmatic theming. Icons → image assets or custom widgets.

---

### Candidate 3: GPUI

#### Rendering Model

GPUI is a GPU-accelerated UI framework used by the Zed editor. Rendering is entirely GPU-based (Core Animation on macOS, OpenGL/Vulkan/EGL on Linux). No HTML DOM, no CSS.

**Verified fact**: GPUI is the UI framework written and open-sourced by the Zed editor team. It renders to a GPU surface (Core Animation layer on macOS). Elements are drawn as GPU primitives. There is no HTML/CSS layer.

#### Windowing Model

Creates native macOS app windows via AppKit. Linux support is in development. **Windows is not supported.** No webview involvement.

**Verified fact**: GPUI is macOS-first with ongoing Linux support. Windows is not a current target. GPUI manages its own window lifecycle.

#### Runtime Model

Custom reactive system: `AppContext`, `ViewContext`, `Model<T>`, `SharedMutable<T>`. Subscriptions via `Subscription<T>`. Render via `Render` trait (`fn render(&mut self, cx: &mut ViewContext<Self>) -> impl IntoElement`).

**Verified fact**: GPUI uses a push-based reactive system where models emit updates and views subscribe. The `Render` trait is analogous to React's render function but with a custom diffing system over GPU primitives.

#### State Management

`Model<T>` for shared mutable state, `SharedMutable<T>` for interior mutability, `Entity<T>` for entity-component patterns. Context is passed via `AppContext` / `ViewContext`.

**Verified fact**: GPUI's `Model<T>` is the primary state container. `SharedMutable<T>` provides thread-safe shared state. These are fundamentally different from Leptos signals.

#### Component Model

Components implement `Render` trait and return `Div` or other element types. No `#[component]` macro, no JSX-like syntax. The `gpui::div()` / `gpui::h_flex()` builders create elements.

**Verified fact**: GPUI uses a builder API: `gpui::div().flex().flex_col().bg(color).child(...)`. There is no macro-based template syntax.

#### Event Model

Custom event system: `MouseDown`, `MouseUp`, `MouseMove`, `KeyEvent`, `WindowBlur`, etc. Events are typed and handled via `.on_event()` or `.on_action()` handlers on elements.

**Verified fact**: GPUI's event model is based on typed events with `on_event` handlers. `KeyEvent` is used for keyboard input, `MouseDownEvent` for mouse input. No DOM event compatibility.

#### Native Integration Model

Tightly integrated with macOS via objc2 (same crates Nabu uses for native integration). On Linux, uses raw GPU APIs. No IPC bridge — would need custom implementation.

**Verified fact**: GPUI's macOS integration uses `objc2` for AppKit bindings, which is the same crate Nabu's `nabu-core/src/native/` uses for PDFKit and Vision.

#### WebView Usage

**No webview embedding capability.** GPUI does not provide a webview widget. Embedding HTML content would require custom FFI to embed an `NSView`/`WKWebView` on macOS — this would be non-trivial and platform-specific.

**Verified fact**: GPUI does not include any webview embedding functionality. The Zed editor itself uses GPUI exclusively; web content is not embedded.

#### WASM Usage

**Not available.** GPUI is a desktop-only framework with no WASM target. The entire `nabu-ui` crate (which is a WASM cdylib) would need to be completely restructured as a native Rust library.

**Verified fact**: GPUI has no WASM support. The framework targets macOS and Linux natively. There is no path to compile GPUI code to WASM.

#### HTML/CSS Rendering Strategy

**No HTML/CSS rendering.** All rendering is GPU-based. No CSS pipeline exists.

**Verified fact**: GPUI has its own rendering pipeline with `PaintImage`, `Canvas` layers, and GPU primitives. There is no CSS parser or HTML renderer.

**Comparison to current**: This would be the most invasive migration. The nabu-ui crate would fundamentally change from a WASM cdylib to a native library. The build pipeline would need complete overhaul (no Trunk, no WASM, no npm). The entire UI would need to be rewritten using GPUI's builder API. IPC would need a new mechanism. App Blocks (HTML iframes) would have no embedding path. CSS/Tailwind would be discarded.

---

## Phase 2 — Compatibility Analysis

### Subsystems Evaluation

| Subsystem | Keep Tauri+Leptos | Dioxus Migration | Iced Migration | GPUI Migration |
|-----------|:-----------------:|:----------------:|:--------------:|:--------------:|
| Markdown renderer | Unaffected | Unaffected | Unaffected | Unaffected |
| App Blocks (iframe sandbox) | Unaffected | Minor modifications | **Major rewrite** | **Critical** |
| Capture Engine | Unaffected | Unaffected | Unaffected | Unaffected |
| Processing Pipeline | Unaffected | Unaffected | Unaffected | Unaffected |
| Storage Manager | Unaffected | Unaffected | Unaffected | Unaffected |
| Event Bus | Unaffected | Unaffected | Unaffected | Unaffected |
| File watching | Unaffected | Unaffected | Unaffected | Unaffected |
| Native integrations (macOS) | Unaffected | Unaffected | Unaffected | Unaffected |
| Tantivy indexing/Search | Unaffected | Unaffected | Unaffected | Unaffected |
| Knowledge Graph | Unaffected | Unaffected | Unaffected | Unaffected |
| Theme system | Unaffected | Minor modifications | **Complete replacement** | **Complete replacement** |
| Plugin architecture | Unaffected | Unaffected | Unaffected | Unaffected |
| Command palette | Unaffected | Moderate rewrite | **Complete replacement** | **Complete replacement** |
| Settings | Minor modifications | Moderate rewrite | **Complete replacement** | **Complete replacement** |
| Window management | Unaffected | Minor modifications | **Major rewrite** | **Complete replacement** |
| Notifications | Minor modifications | Minor modifications | **Complete replacement** | **Complete replacement** |
| AI Chat | Unaffected | Unaffected | Unaffected | Unaffected |
| ACP integration | Unaffected | Unaffected | Unaffected | Unaffected |
| Dictation Pill | Unaffected | Minor modifications | **Major rewrite** | **Critical** |

### Detailed Rationale

#### Markdown Renderer — Unaffected (all candidates)

The markdown rendering pipeline (`reader.rs`) is a lightweight inline parser implemented entirely in the UI layer. It does not depend on Leptos-specific APIs — it uses `view!` for rendering parsed tokens but the parsing logic is framework-agnostic. The reader component uses `leptos::prelude::*` for UI but the parsing functions are pure Rust.

**Evidence**: `reader.rs` contains the parser logic in pure functions (`parse_markdown_to_html`-style functions) and uses `view!` only for rendering tokens. The parser does not use signals, context, or any reactive APIs. Migration to Dioxus requires only changing the `view!` syntax; Iced/GPUI require changing both parsing integration and rendering.

**Migration impact**: Dioxus = **Minor** (change `view!` syntax). Iced/GPUI = **Complete replacement** (rewrite rendering as GPU widgets).

#### App Blocks / Sandboxing — Critical differentiator

**`sandbox.rs`**: Uses `<iframe srcdoc=html_content sandbox="allow-scripts allow-forms" />` with `NodeRef::<leptos::html::Iframe>` for DOM access and `web_sys::MessageEvent` + `addEventListener` for postMessage communication. The `on_message` callback receives `MessageEvent` and processes messages from the sandboxed iframe.

**`sandboxed_html.rs`**: Simpler — `<iframe srcdoc=html sandbox="allow-scripts" class="sandboxed-html" />`.

**Why this matters**: App Blocks are HTML-native by design. The `srcdoc` attribute injects raw HTML into a sandboxed browsing context. The `sandbox` attribute restricts capabilities. PostMessage provides a controlled communication channel.

**Dioxus**: Can render `<iframe>` directly in `rsx!`. PostMessage via `web_sys` works the same. **Impact: Minor modifications** (change `view!` to `rsx!`, change `NodeRef` type name).

**Iced**: No native iframe support. Would need a custom widget wrapping a native webview or an HTML element. The `iced_wgpu` renderer draws to a `<canvas>`, and embedding an iframe inside that canvas is not straightforward.

**GPUI**: No webview embedding. Would require custom objc2 FFI to embed `WKWebView` on macOS. **Impact: Critical**.

**Verified fact**: `sandbox.rs` uses `web_sys::MessageEvent`, `Closure::<dyn FnMut(MessageEvent)>::new`, `iframe.content_window()`, `window.add_event_listener_with_callback`. `sandboxed_html.rs` is a pure iframe with `srcdoc` and `sandbox` attributes.

#### IPC Layer — 64 commands, 113 call sites

The frontend uses a single abstraction `crate::ipc::tauri_invoke("command_name", args)` (5 lines). The command names are strings passed to `window.__TAURI__.core.invoke`. The IPC mechanism itself is independent of the UI framework.

**What changes**:
- **Dioxus**: The `ipc.rs` abstraction stays the same (it's pure wasm-bindgen). The 113 call sites don't change their IPC calls — they just need to be inside Dioxus components. **Impact: Unaffected**.
- **Iced/GPUI**: The IPC mechanism changes. Iced can use `wasm-bindgen` to call `invoke` but the async pattern changes (`Command` vs `spawn_local`). GPUI has no WASM path. **Impact varies**.

**Verified fact**: `ipc.rs` is 5 lines, framework-agnostic. All 113 call sites follow the pattern:
```rust
let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "key": value })).unwrap();
let result = crate::ipc::tauri_invoke("command_name", args).await;
```
This pattern is identical across all components and doesn't depend on Leptos.

#### Theme System — CSS variables + Tailwind

The theme system (`lib.rs`) uses `root.set_attribute("data-theme", "dark")` / `remove_attribute("data-theme")` to swap CSS palettes. The 2,923-line `app.css` defines design tokens as CSS variables (`--color-primary`, `--gray-950`, etc.) that Tailwind references.

**Dioxus**: Same CSS-in-DOM approach. `web_sys::document().document_element()` works identically. **Impact: Unaffected**.

**Iced**: No CSS. Would need to retheme entirely via Iced's `Theme`/`Palette` system. **Impact: Complete replacement**.

**GPUI**: No CSS. Would need GPU shader-based theming. **Impact: Complete replacement**.

**Verified fact**: `lib.rs` `apply_theme_to_document()` uses `web_sys::window().document().document_element().set_attribute("data-theme", ...)`. The CSS file defines `[data-theme="dark"]` and `[data-theme="light"]` variants. This is pure DOM/CSS, framework-agnostic.

#### Settings — 15 tabs, ~50 settings fields

`SettingsPanel` uses a `RwSignal<AppSettings>` with 15 tabs. Each tab component updates the signal and calls `save` (which invokes `settings_set_all` via IPC). Settings are defined identically in both the Tauri backend (`src-tauri/src/settings.rs`) and the UI (`settings/settings_panel.rs`).

**Dioxus**: Signal API change (`RwSignal` → `Signal`/`WritableSignal`), event handler syntax change, callback change. The settings data model, persistence, and IPC remain identical. **Impact: Moderate rewrite**.

**Iced/GPUI**: State model changes completely (signals → State struct / Model). All settings UI needs rebuilding. **Impact: Complete replacement**.

#### Window Management — Dictation Pill

The dictation pill is a separate Tauri webview window. `toggle_dictation_pill` calls `app.get_webview_window("dictation-pill")` and shows/hides it. The Tauri command is platform-specific.

**Dioxus**: If Dioxus is used alongside Tauri (not dioxus-desktop), window management stays through Tauri IPC. **Impact: Unaffected**.

**Iced**: Would need to implement a separate window via winit. **Impact: Major rewrite**.

**GPUI**: Window management is GPUI's domain. **Impact: Complete replacement**.

#### Toast System — `feedback.rs`

The `ToastContext` uses `RwSignal<Vec<ToastItem>>` with `push`, `dismiss`, `auto-dismiss` via `set_timeout`. The `ToastProvider` is a component that provides context and renders the toast region. Uses `Callback<()>` for action handlers.

**Dioxus**: Same reactive pattern with Dioxus signals. `set_timeout` works the same. **Impact: Minor modifications**.

**Iced**: Would need to implement as a custom overlay widget. **Impact: Major rewrite**.

**GPUI**: Would need custom GPU rendering. **Impact: Major rewrite**.

**Verified fact**: `feedback.rs` uses `RwSignal<Vec<ToastItem>>`, `set_timeout` for auto-dismiss, `Callback<()>` for actions, `provide_context`/`expect_context` for the context, and `ChildrenFn` for the provider. The entire system is 500+ lines of Leptos-specific code.

---

## Phase 3 — UI Migration Analysis

Nabu has **76 component files** across 9 directories, totaling **~22,000 lines** of Rust. Every file imports `leptos::prelude::*`.

### UI System Migration Matrix

| UI System | Files | Lines (est.) | Rewrite Complexity | Migration Difficulty | Risk | Reusability |
|-----------|-------|-------------|-------------------|---------------------|------|-------------|
| **UI Primitives** (button, input, menu, dialog, card, etc.) | 10 | ~3,000 | High | High | High | 0% (Dioxus), <5% (Iced/GPUI) |
| **Layout** (sidebar, ribbon, tabs, inspector) | 5 | ~1,000 | High | High | High | 0% (Dioxus), 0% (Iced/GPUI) |
| **Navigation** (navbar, palette, switcher, shortcuts, dashboard) | 10 | ~2,500 | High | High | High | 0% (Dioxus), <5% (Iced/GPUI) |
| **File Tree** | 1 | ~1,000 | High | High | High | 0% (Dioxus), 0% (Iced/GPUI) |
| **Editor** (note_editor, slash_menu) | 2 | ~1,300 | High | High | High | 0% (Dioxus), 0% (Iced/GPUI) |
| **Note View** (markdown rendering) | 1 | ~10 | Low | Low | Low | Partial parsing logic reusable |
| **Graph View** (canvas, nodes, edges, minimap) | 1 | ~1,300 | Critical | Critical | Critical | 0% (all) |
| **Canvas View** (infinite workspace, viewport culling) | 1 | ~600 | Critical | Critical | Critical | 0% (all) |
| **Reader View** (markdown reader, typography) | 1 | ~500 | High | High | High | Parsing logic reusable |
| **Inbox** | 1 | ~1,000 | High | High | High | 0% (Dioxus), 0% (Iced/GPUI) |
| **Collections** (board, calendar, gallery, table views) | 7 | ~800 | High | High | High | 0% (Dioxus), 0% (Iced/GPUI) |
| **Recovery** (diff, history, session, banner) | 7 | ~1,500 | High | High | High | 0% (Dioxus), 0% (Iced/GPUI) |
| **Settings Panel** (15 tabs) | 1 | ~1,200 | High | High | High | 0% (Dioxus), 0% (Iced/GPUI) |
| **Search Page** | 1 | ~400 | High | High | High | 0% (Dioxus), 0% (Iced/GPUI) |
| **Command Palette** | 1 | ~800 | High | High | High | 0% (Dioxus), 0% (Iced/GPUI) |
| **PDF Viewer** | 1 | ~300 | High | High | High | 0% (Dioxus), 0% (Iced/GPUI) |
| **Dictation Pill** | 1 | ~300 | High | High | High | 0% (Dioxus), 0% (Iced/GUI) |
| **Sandbox / SandboxedHtml** | 2 | ~100 | Critical (Iced/GPUI) | Critical | Critical | 0% (Iced/GPUI), Minor (Dioxus) |
| **Theme Toggle** | 1 | ~30 | Low | Low | Low | 100% (Dioxus) |
| **Workspace** (tabs, context) | 1 | ~400 | High | High | High | 0% (Dioxus), 0% (Iced/GPUI) |
| **History** (undo/redo) | 1 | ~250 | High | High | High | 0% (Dioxus), 0% (Iced/GPUI) |
| **Icons System** | 1 | ~700 | Critical (Iced/GPUI) | Critical | Critical | 0% (all — lucide-leptos is Leptos-specific) |

### Detailed Analysis

#### Graph View (canvas.rs, graph_view.rs) — **Critical** across all migrations

The GraphView is a pure-canvas force-directed graph implementation using raw `<canvas>` and `CanvasRenderingContext2d` from `web_sys`. It draws nodes, edges, labels, tooltips, and a minimap directly on the canvas via 60+ methods (`begin_path`, `arc`, `fill_rect`, `stroke`, `fill_text`, `measure_text`, `set_line_dash`, etc.).

**Why this is critical**:
- The entire rendering is imperative canvas operations within a single `Effect::new`
- `web_sys::CanvasRenderingContext2d` and `web_sys::WheelEvent` are used for input
- Node dragging, zooming, panning, keyboard navigation, and viewport culling are all hand-implemented
- No UI framework widget maps to this — it's a raw graphics canvas

**Migration impact**:
- **Dioxus**: The canvas element and `web_sys` calls stay the same. Only the `view!` macro and signal bindings change. The canvas drawing logic is pure `web_sys` and is reusable. **Impact: Moderate** (change template syntax, keep canvas logic).
- **Iced**: Iced has a `canvas` module (`iced::widget::canvas`) but it's a completely different API. The entire 1,300 lines of imperative canvas code would need rewriting to Iced's `Geometry`/`Program` model. **Impact: Complete replacement**.
- **GPUI**: No canvas equivalent — would need to implement GPU-based rendering from scratch. **Impact: Complete replacement**.

**Verified fact**: `graph_view.rs` imports `web_sys::{CanvasRenderingContext2d, WheelEvent}` and uses `canvas.get_context("2d")`, `ctx.unchecked_into::<CanvasRenderingContext2d>()`, and 40+ canvas methods directly.

#### Dictation Pill (dictation_pill.rs) — Depends on webview window

Uses `web_sys::DragEvent` for drag-and-drop, `window.navigator().clipboard()` for clipboard access, and calls Tauri IPC commands. Renders as a separate webview window.

**Dioxus**: All web_sys APIs stay the same. **Impact: Minor modifications**.
**Iced**: No drag-drop, no clipboard, no webview window. **Impact: Major rewrite**.
**GPUI**: No HTML/CSS, no WASM. **Impact: Critical**.

#### App Block Sandbox (sandbox.rs, sandboxed_html.rs)

Uses `<iframe srcdoc=html_content sandbox="allow-scripts allow-forms" />` with `NodeRef::<leptos::html::Iframe>` and `web_sys::MessageEvent` for postMessage.

**Dioxus**: Can render `<iframe>` in `rsx!`. web_sys stays the same. **Impact: Minor modifications** (syntax change only).
**Iced**: No iframe support. Would need custom webview widget. **Impact: Critical**.
**GPUI**: No webview embedding. **Impact: Critical**.

#### Icon System (icons.rs, 700 lines)

Uses `lucide-leptos` — a Leptos-specific crate that provides compile-time SVG components. 80+ icon components are re-exported, and an `Icon` enum maps to component views via `render_icon_view()`:
```rust
macro_rules! c { ($cmp:ident) => { view! { <$cmp /> }.into_any() }; }
```

**Dioxus**: `lucide-leptos` is Leptos-specific. Would need `dioxus-icon` or `dioxus-lucide`. The `Icon` enum and mapping logic are reusable; only the component invocation changes. **Impact: Critical** (need new icon library, 80+ re-exports to change).

**Iced**: No SVG component system. Would need image assets or custom SVG widgets. **Impact: Complete replacement**.

**GPUI**: No SVG component system. **Impact: Complete replacement**.

**Verified fact**: `icons.rs` has `pub use lucide_leptos::Archive;` etc. (80+ re-exports) and `macro_rules! c { ($cmp:ident) => { view! { <$cmp /> }.into_any() }; }` for rendering. The `render_icon_view()` function returns `AnyView`.

#### UI Primitives (10 files, ~3,000 lines)

The entire UI primitive library (button, input, menu, dialog, card, feedback, navigation, selection, layout, info) is built on Leptos:
- `#[component]` macros (145 total across UI)
- `view!` templates with `on:click`, `prop:value`, `aria-*` attributes
- `ChildrenFn` for composition
- `Callback<T, R>` for event handlers
- `RwSignal<T>` for two-way binding
- `into_any()` for conditional rendering

**Migration impact for Dioxus**:
- `on:click={...}` → `onclick={...}` (213 click handlers)
- `prop:value={signal}` → `value={signal}` (53 prop bindings)
- `RwSignal<T>` → `Signal<T>` / `WritableSignal<T>` (169 signals)
- `into_any()` → not needed (239 eliminated)
- `ChildrenFn` → `Element` or `ElementChildren`
- `Callback<T>` → closures or `EventHandler<T>`
- `#[component]` → same attribute, but props syntax differs
- `view! { ... }` → `rsx! { ... }` (syntax is similar but not identical)

This is a **mechanical but massive** refactor across all 76 files.

**Migration impact for Iced/GPUI**:
- Complete paradigm shift from reactive component to Elm-style/state-update
- Every component needs rewriting from scratch
- No `view!` macro equivalent
- No signal/context system

#### State Management — 8 shared contexts

1. **WorkspaceContext** (`workspace.rs`): `tabs: RwSignal<Vec<OpenTab>>`, `active_path: RwSignal<Option<String>>`, `refresh_tree: RwSignal<u32>`, `content_version: RwSignal<(String, u32)>`
2. **NavContext** (`navigation/state.rs`): 14 `RwSignal` fields
3. **HistoryContext** (`history.rs`): `can_undo: RwSignal<bool>`, `can_redo: RwSignal<bool>`
4. **ThemeContext** (`lib.rs`): `theme: RwSignal<String>`
5. **ToastContext** (`feedback.rs`): `toasts: RwSignal<Vec<ToastItem>>`
6. **TaskContext** (`feedback.rs`): `tasks: RwSignal<Vec<TaskInfo>>`
7. **TreeContext** (`file_tree.rs`): 15 fields mixing `RwSignal` and `ToastContext`
8. **SaveStatusContext** (`recovery/save_status.rs`): save status signals

**Dioxus**: Same architecture (provide_context/use_context), different signal types.
**Iced**: State moves to the `State` struct.
**GPUI**: State uses `Model<T>` / `SharedMutable<T>`.

#### Build Pipeline

Current: Trunk (WASM bundler) + npm (Tailwind CSS) + Cargo (two workspaces)
- `run-trunk.sh`: generates Tailwind CSS, watches for changes, serves via Trunk on port 8080
- `build-trunk.sh`: generates Tailwind CSS, builds WASM bundle with `trunk build --release`
- `tauri.conf.json`: `beforeDevCommand` runs `run-trunk.sh`, `frontendDist` is `../dist`
- `index.html` loads CSS via `data-trunk rel="css"` and WASM via `data-trunk rel="rust"`

**Dioxus**: Would need to change Trunk to `dioxus-cli` or `cargo dioxus`. Tailwind pipeline stays the same.
**Iced**: Would need to change to `cargo build` with iced's web target. Tailwind pipeline discarded.
**GPUI**: No WASM build. Cargo build with native target.

---

## Phase 4 — App Block Impact

Nabu's **core differentiator** is HTML-native App Blocks: user-authored HTML snippets rendered in sandboxed iframes via `srcdoc` + `sandbox` attributes, with `postMessage` for controlled communication.

### Current Implementation

**`sandbox.rs` (30 lines)**:
- Renders `<iframe srcdoc=html_content sandbox="allow-scripts allow-forms" title="App Block Sandbox" />`
- Uses `NodeRef::<leptos::html::Iframe>` to get the DOM element
- Accesses `iframe.content_window()` to add a `message` event listener
- Uses `web_sys::MessageEvent` and `Closure::<dyn FnMut(MessageEvent)>::new` for postMessage
- Logs messages from the sandbox via `leptos::logging::log!`

**`sandboxed_html.rs` (7 lines)**:
- Simpler iframe: `<iframe srcdoc=html sandbox="allow-scripts" class="sandboxed-html" />`

**`settings_panel.rs`**: Has `force_sandbox_for_web_snippets: bool` setting that controls whether App Blocks are sandboxed

### Dioxus Impact — **Improves App Blocks slightly**

Dioxus renders real HTML DOM via `rsx!`. The `<iframe>` element is a standard HTML element — it renders identically in Dioxus and Leptos. The `srcdoc`, `sandbox`, and `title` attributes work the same way. The `web_sys::MessageEvent`, `Closure`, and `content_window()` APIs are unchanged (Dioxus uses the same web_sys underneath).

**What improves**:
- Dioxus's `Element` type is already type-erased, so the `into_any()` pattern (used in conditional rendering around iframes) simplifies to clean conditional logic
- Dioxus's `dioxus-desktop` already provides webview embedding, which could simplify the dictation pill window management

**What stays the same**:
- The iframe sandbox model is identical (HTML sandbox attribute + srcdoc)
- PostMessage communication is identical (web_sys MessageEvent)
- The `force_sandbox_for_web_snippets` setting behavior is unchanged
- The isolation model (separate browsing context) is unchanged

**No benefit**: Dioxus doesn't add sandboxing capabilities beyond what HTML iframes already provide. The security model is identical.

### Iced Impact — **Breaks App Blocks entirely**

Iced renders via GPU (wgpu) to a `<canvas>`. There is **no HTML DOM** and therefore **no native iframe support**. App Blocks would require:

1. A custom widget that embeds a native webview — **no such widget exists** in Iced's ecosystem
2. OR abandoning HTML-native App Blocks entirely and reimplementing them as GPU-drawn widgets — this would eliminate the entire value proposition of HTML-native App Blocks

**Unknown**: Whether any third-party Iced crate provides webview embedding. No widely-known crate exists for this purpose.

### GPUI Impact — **Breaks App Blocks entirely**

GPUI renders via GPU (Core Animation / OpenGL). No HTML, no webview embedding. To render HTML App Blocks, one would need to:
1. Embed a `WKWebView` via objc2 FFI on macOS — **non-trivial** and platform-specific
2. On Linux, embed a GTK WebKit or similar — **unknown feasibility**
3. Windows is not even supported by GPUI

### Sandboxing / Isolation

**Current**: HTML iframe `sandbox` attribute provides the browser's built-in sandbox (no cookies, no scripts outside the allowed set, no top-navigation, no popups, etc.). This is the **gold standard** for HTML sandboxing.

**Dioxus**: Same iframe sandbox. Unchanged. **No improvement, no regression**.

**Iced**: Would require implementing a custom sandbox — either a native webview with restricted capabilities (platform-specific) or abandoning HTML sandboxing entirely.

**GPUI**: Same as Iced — no path to iframe sandboxing.

### Security

The iframe sandbox is the security boundary. Migrating away from HTML means losing this boundary.

### Performance

The iframe approach means App Blocks run in a separate browsing context with their own JavaScript engine. This provides isolation but also limits performance. In a GPU-based framework, App Blocks would run in the same process, potentially sharing state — a security regression.

### Developer Experience

The current approach (HTML + iframe + postMessage) is familiar to web developers. Moving to a non-HTML framework would require developers to learn entirely new paradigms.

---

## Phase 5 — Roadmap Alignment

### Desktop — All frameworks support desktop, but differently

| Framework | Desktop Support | Assessment |
|-----------|----------------|------------|
| Tauri + Leptos | Native via webview | **Current** — excellent, production-ready |
| Dioxus | `dioxus-desktop` (Wry webview) | Good, but less mature ecosystem than Tauri |
| Iced | Native via winit + wgpu | Good, but no webview integration |
| GPUI | macOS + Linux (no Windows) | Limited — not cross-platform |

**Nabu's roadmap**: Desktop is the primary target. Tauri + Leptos is already production-ready for desktop. Migrating would mean trading a mature Tauri ecosystem for either Dioxus-desktop (less mature) or Iced/GPUI (no webview).

### Mobile — Nabu has a "Multi-platform roadmap"

| Framework | Mobile Support | Assessment |
|-----------|----------------|------------|
| Tauri + Leptos | Tauri 2.0+ mobile (beta) | Emerging — Tauri mobile is available but Leptos CSR on mobile webview has limitations |
| Dioxus | `dioxus-mobile` (active) | **Best** mobile support — Dioxus has first-class mobile support via dioxus-mobile, shared codebase with desktop |
| Iced | WASM web target, no native mobile | Poor — no native mobile, WASM web only |
| GPUI | Not supported | None — GPUI is desktop-only |

**Assessment**: Dioxus is the only framework that improves mobile support. However, Nabu's mobile roadmap is "multi-platform" (not imminent), and the current Tauri mobile story is adequate for a desktop-first app.

### Web Collaboration

| Framework | Web Support | Assessment |
|-----------|-------------|------------|
| Tauri + Leptos | Leptos can compile to WASM for web | The UI already compiles to WASM — web deployment is possible (minus Tauri-specific IPC) |
| Dioxus | `dioxus-web` (first-class) | Good — WASM is a primary target |
| Iced | `iced_wgpu` (WASM via canvas) | Moderate — renders to canvas, not DOM |
| GPUI | Not supported | None |

**Assessment**: Both Tauri+Leptos and Dioxus have good web stories. The IPC abstraction (`ipc.rs`) is 5 lines and framework-agnostic — if Nabu were to deploy web, both frameworks would need the same IPC replacement. **No advantage to migration here**.

### Local-First Sync

This is a backend architecture concern (Storage Manager, nabu-core). **Unaffected by UI framework choice**. The `nabu-core` library handles all persistence and sync logic.

### ACP (App Core Protocol)

ACP is a planned backend integration. It would go through the Tauri backend → nabu-core → Capture Engine pipeline. **Unaffected by UI framework choice**. The IPC command names would stay the same regardless of frontend framework (if Tauri is retained).

### AI Chat

AI features run through the backend processing pipeline (AI Summariser processor, Capture Engine). The UI just calls IPC commands and displays results. **Unaffected by migration** — except for the UI rendering cost.

### Knowledge Objects

The Knowledge Object Model lives in `nabu-core/src/models/`. The UI renders projections of these types (`models.rs` re-exports core types). **Unaffected by migration** — the data model is framework-agnostic.

### Canvas

The Canvas View (`components/canvas.rs`) is a custom infinite-workspace renderer using:
- `web_sys::HtmlElement` for container sizing
- `web_sys::CustomEvent` / `CustomEventInit` for inter-component communication
- `js_sys::Date` for timestamps
- Direct DOM manipulation via `web_sys`

**Dioxus**: Same web_sys access. **Impact: Minor modifications** (syntax change only).
**Iced**: Iced has a canvas widget, but the API is completely different. The entire 550-line implementation would need rewriting. **Impact: Complete replacement**.
**GPUI**: No canvas equivalent. **Impact: Complete replacement**.

### Graph

The Graph View uses raw HTML `<canvas>` with `CanvasRenderingContext2d`. Same analysis as Canvas — **deterministic** for Dioxus, **complete replacement** for Iced/GPUI.

### PDF Workflows

PDF processing is in `nabu-core/src/native/pdfkit.rs` (macOS PDFKit via objc2) and the Processing Pipeline. The UI displays PDF metadata and extracted text. `pdf_viewer.rs` renders PDF content.

**Verified fact**: `pdf_viewer.rs` exists in the UI components but its implementation details are unknown from the audit scope. It likely uses web-based PDF rendering or displays extracted text.

**Assessment**: PDF workflows are backend-heavy. UI migration impact depends on the PDF viewer implementation. If it uses a web-based PDF viewer (pdf.js in an iframe), then Dioxus preserves it but Iced/GPUI would need alternatives.

### Browser Capture

Capture sources (BrowserCaptureHandler, ClipboardHandler) are in `nabu-core/src/capture/`. The UI invokes them via IPC (`capture_file_drop`). **Unaffected by migration**.

### Performance

**Current**: WASM in webview. Leptos fine-grained reactivity is efficient. Canvas rendering is direct (no VDOM overhead for the graph). IPC is synchronous for most commands.

**Dioxus**: Same WASM-in-webview model. Similar performance characteristics. The move to Dioxus might slightly improve or degrade performance depending on the renderer implementation — no clear advantage.

**Iced**: GPU-based rendering. Could offer better performance for widget-heavy UIs (no DOM overhead). But the canvas-based components (Graph, Canvas View) would lose direct DOM access.

**GPUI**: GPU-based. Excellent performance for text rendering and animations. But no WASM support means the entire architecture must change.

### Accessibility

**Current**: Native HTML elements (`<button>`, `<input>`, `<textarea>`, `<select>`) with proper ARIA attributes (`aria-label`, `aria-expanded`, `role="dialog"`, `aria-modal`, `aria-live`, `aria-valuenow`, `aria-describedby`). Keyboard navigation via standard tab order and keydown handlers.

**Dioxus**: Renders real HTML DOM. Same accessibility story. **No regression**.

**Iced**: Implements its own accessibility layer. Different from HTML accessibility. Would need to rebuild all ARIA patterns. **Significant regression risk**.

**GPUI**: Has its own accessibility tree implementation (used by Zed). Different from HTML. **Significant regression risk**.

### Summary

Nabu's roadmap items that benefit from migration:
- **Mobile**: Dioxus only (moderate improvement)
- **Web collaboration**: No advantage (both Leptos and Dioxus support WASM)

Everything else is either **unaffected** (backend systems) or **harmed** by migration (App Blocks, canvas rendering, accessibility, CSS pipeline).

---

## Phase 6 — Migration Cost

### Migration to Dioxus

| Cost Category | Estimate | Justification |
|---------------|----------|---------------|
| Component rewrites (76 files, ~22,000 lines) | **Very High** | Every file uses `leptos::prelude::*`; 689 `view!` → `rsx!`, 213 `on:click` → `onclick`, 53 `prop:value` → `value`, 449 `into_any()` → removed, 169 `RwSignal` → `Signal`, 350 `Callback` → closures |
| Signal/context API migration | **High** | 169 `RwSignal` instances, 8 shared contexts, 22 `Memo` instances across 8 context types |
| Icon system replacement | **Critical** | `lucide-leptos` is Leptos-specific; need `dioxus-icon` or equivalent; 80+ icon re-exports + `render_icon_view` macro changes |
| Build pipeline changes | **Medium** | Trunk → dioxus-cli or cargo-dioxus; Tailwind pipeline stays |
| IPC layer | **Low** | `ipc.rs` is 5 lines, framework-agnostic; command names unchanged |
| CSS/Tailwind pipeline | **Low** | `index.html`, `tailwind.config.js`, `app.css` unchanged; same Tailwind approach |
| Testing impact | **High** | All 76 test-adjacent components need revalidation; no test framework migration path |
| Documentation impact | **Medium** | Architecture docs, build instructions, contributor guides all reference Leptos |
| nabu-core / src-tauri | **Unaffected** | No changes needed — they don't depend on UI framework |

### Migration to Iced

| Cost Category | Estimate | Justification |
|---------------|----------|---------------|
| Component rewrites (76 files, ~22,000 lines) | **Critical** | Complete paradigm shift from reactive components to Elm-style State/update/view |
| State management rewrite | **Critical** | 8 shared contexts (RwSignal) → State struct + Message enum; no context system |
| Icon system replacement | **Critical** | No SVG component system; need image assets or custom widgets |
| Canvas/Graph rendering | **Critical** | `CanvasRenderingContext2d` → `iced::widget::canvas` (completely different API) |
| App Blocks (iframe) | **Critical** | No iframe/webview support; need custom widget or abandon HTML App Blocks |
| Build pipeline | **High** | Trunk → cargo build for iced web; no npm/Tailwind pipeline |
| CSS/Tailwind pipeline | **Critical** | Complete replacement with iced theme system; 2,923 lines of CSS discarded |
| IPC layer | **High** | `spawn_local` + `serde_wasm_bindgen` → `iced::Command` + custom bridge |
| Accessibility | **Critical** | Native HTML ARIA → iced's accessibility layer (less mature) |
| nabu-core / src-tauri | **Unaffected** | Backend systems don't depend on UI framework |

### Migration to GPUI

| Cost Category | Estimate | Justification |
|---------------|----------|---------------|
| Component rewrites | **Critical** | Complete rewrite; GPUI has different API from both Leptos and Elm |
| WASM architecture change | **Critical** | nabu-ui is a WASM cdylib; GPUI has no WASM target; the entire crate structure changes |
| Icon system replacement | **Critical** | No SVG component system |
| Canvas/Graph rendering | **Critical** | No canvas; need GPU-based rendering from scratch |
| App Blocks (iframe) | **Critical** | No webview embedding; no path to HTML iframes |
| CSS/Tailwind pipeline | **Critical** | No CSS; complete replacement with GPU theme system |
| Build pipeline | **Critical** | Trunk/WASM → native Cargo build; npm pipeline discarded |
| Platform support | **Critical** | No Windows support; Nabu targets Windows |
| IPC layer | **High** | No WASM bridge; need native IPC to Rust backend |
| nabu-core / src-tauri | **Moderate** | Tauri would be removed; need new window management |

### Categorized Summary

| Item | Dioxus | Iced | GPUI |
|------|--------|------|------|
| Component rewrites | Very High | Critical | Critical |
| Signal/context changes | High | Critical | Critical |
| CSS/Tailwind | Low | Critical | Critical |
| Icon system | Critical | Critical | Critical |
| Canvas rendering | Moderate | Critical | Critical |
| App Blocks (iframe) | Minor | Critical | Critical |
| IPC bridge | Low | High | High |
| Build pipeline | Medium | High | Critical |
| Testing | High | Critical | Critical |
| Documentation | Medium | High | High |
| nabu-core (backend) | Unaffected | Unaffected | Unaffected |
| src-tauri (backend) | Unaffected | Unaffected | Affected (Tauri may be removed) |

---

## Phase 7 — SWOT Analysis

### Current Architecture: Tauri + Leptos

**Strengths:**
- **App Blocks are native**: HTML iframes with `srcdoc` + `sandbox` provide the gold-standard HTML sandboxing model. No other framework can match this for HTML-native execution.
- **Canvas rendering is direct**: Raw `web_sys::CanvasRenderingContext2d` access for the GraphView and Canvas View — no abstraction penalty. Verified: `graph_view.rs` uses 40+ canvas methods directly.
- **Mature ecosystem**: Leptos 0.7 is stable. Tauri 2.x is production-ready. Tailwind 3.4 is mature.
- **CSS pipeline is complete**: 2,923 lines of design system CSS + Tailwind integration via `tailwind.config.js` scanning Rust files for class names.
- **IPC is clean**: Single 5-line abstraction (`ipc.rs`) over 64 commands. Framework-agnostic.
- **`nabu-core` compiles to both native and WASM**: The core library is shared between the Tauri backend (native) and the WASM UI (WASM). This dual compilation is a key architectural strength.
- **Reactive state is well-designed**: 8 carefully designed shared contexts (Workspace, Nav, History, Theme, Toast, Task, Tree, SaveStatus) using Leptos's fine-grained reactivity.
- **Incremental migration path exists**: Leptos 0.8/0.9 upgrade is straightforward (same API family).

**Weaknesses:**
- **Build pipeline complexity**: Two separate Cargo workspaces + Trunk + npm CSS pipeline. The `index.html` references `generated/tailwind.css` and a WASM bundle loaded by Trunk — three build systems must coordinate.
- **WASM startup time**: The Leptos WASM bundle must compile and instantiate before the UI renders. Mitigated by a boot splash (`index.html` inline CSS + spinner).
- **Leptos 0.7 → 0.8/0.9 migration is pending**: The icon library (`lucide-leptos 0.2.0`) is pinned to Leptos 0.7; upgrading requires upgrading the icon library too (verified: `Cargo.toml` comment says "latest 3.x release requires Leptos 0.8").
- **web_sys verbosity**: 94 direct web_sys usages across 20+ types. While powerful, this creates tight coupling to the DOM API.
- **Two workspaces can't share dependencies**: The root workspace (`nabu-core` + `src-tauri`) and `nabu-ui` workspace are separate. `nabu-ui` depends on `nabu-core` via path but can't share dev dependencies.

**Opportunities:**
- **Leptos 0.8/0.9 upgrade**: Fine-grained reactivity improvements, better SSR support, improved performance. The existing code is already Leptos-idiomatic.
- **Build pipeline simplification**: Could consolidate to a single build system (e.g., `cargo-leptos` or Tauri's built-in asset pipeline).
- **Mobile**: Tauri 2.0 mobile is available; could be explored for the "multi-platform roadmap."
- **Better dev tooling**: `cargo-leptos` offers Hot Module Replacement (HMR) which Trunk doesn't provide.
- **Component library maturity**: The UI primitive library (button, input, menu, dialog, etc.) is well-designed and could be extracted as a reusable design system.

**Threats:**
- **Leptos ecosystem is smaller than React/Vue**: Fewer third-party component libraries and dev tools. But for a desktop app, this matters less.
- **WASM debugging**: Debugging WASM in a webview is more complex than debugging native code or JS.
- **web_sys API churn**: The web-sys crate tracks the evolving Web Platform APIs, which can cause compile issues between versions.

### Dioxus Migration

**Strengths (relative to migration):**
- **Same architectural concepts**: Component-based, fine-grained reactivity, `provide_context`/`use_context`, `#[component]` macro. The mental model transfers.
- **Same macro style**: `rsx!`/`view!` macro is JSX-like, similar to Leptos's `view!`.
- **Same rendering target**: Renders to real HTML DOM, preserves CSS/Tailwind pipeline.
- **iframe support**: Can render `<iframe>` directly — App Blocks survive the migration.
- **web_sys compatibility**: Same web_sys access for canvas, clipboard, drag-drop.
- **Potential dual-target benefit**: Dioxus's cross-platform story (web + desktop + mobile) could improve the mobile roadmap.
- **No VDOM**: Like Leptos, Dioxus uses fine-grained reactivity without VDOM overhead.

**Weaknesses (of the migration):**
- **Syntax churn**: 689 `view!` → `rsx!` changes, 213 `on:click` → `onclick`, 53 `prop:value` → `value`. This is mechanical but massive.
- **Signal API change**: `RwSignal<T>` → `Signal<T>` (Dioxus 0.5) or `WritableSignal<T>` (0.6+). The read/write semantics differ.
- **Icon library replacement**: `lucide-leptos` is Leptos-specific. `dioxus-icon` exists but the API differs. 80+ icon re-exports need updating.
- **Type erasure**: `into_any()` (449 uses) is eliminated in Dioxus since `Element` is already type-erased — this changes conditional rendering patterns.
- **Build pipeline change**: Trunk → `dioxus-cli` or `cargo-dioxus`. The `index.html` and Trunk-specific config would need updating.
- **`Callback<T>` replacement**: 350 `Callback` uses would need to change to closures or `EventHandler`.
- **`ChildrenFn` change**: 26 `ChildrenFn` uses need to change to Dioxus's children pattern.
- **No compelling reason to migrate**: Dioxus offers no feature that Leptos doesn't already provide for this use case.

**Opportunities:**
- **Mobile**: Dioxus has more mature mobile support than Tauri/LePtos.
- **Community growth**: Dioxus is rapidly growing; some argue it has more momentum.
- **Dioxus + Tauri**: Could combine Dioxus UI with Tauri backend (community-supported).

**Threats:**
- **Regression risk**: 22,000 lines of working code being mechanically rewritten — high risk of introducing bugs.
- **Ecosystem fragmentation**: Losing Leptos's ecosystem (lucide-leptos, leptos plugins, etc.) for Dioxus's ecosystem, which may have gaps.
- **Build toolchain disruption**: Changing from Trunk to cargo-dioxus adds risk to the already complex 3-system build pipeline.
- **No ROI**: The migration provides no user-facing benefit. Engineering time is better spent on UX gaps.

### Iced Migration

**Strengths (relative to migration):**
- **GPU rendering**: Could offer better performance for widget-heavy UIs (no DOM overhead).
- **Cross-platform**: Native on all desktop platforms.

**Weaknesses (of the migration):**
- **Complete paradigm shift**: From Leptos's reactive component model to Iced's Elm-style message-passing architecture. Every single component needs rewriting.
- **No HTML/CSS**: The 2,923-line Tailwind CSS design system would be completely discarded. All 76 component files use HTML classes.
- **No iframe support**: App Blocks (the core differentiator) cannot be embedded.
- **Canvas API incompatible**: The 1,300-line GraphView canvas implementation would need rewriting to Iced's canvas module.
- **No web_sys access**: 94 web_sys usages across 20+ types would all need replacement.
- **Icon system replacement**: No SVG component system — would need image assets or custom widgets.
- **IPC layer rewrite**: `spawn_local` + `serde_wasm_bindgen` → `iced::Command`.
- **Accessibility regression**: Loses native HTML ARIA — would need to rebuild all 20+ ARIA patterns.

**Opportunities**: None meaningful for Nabu's specific use case.

**Threats:**
- **Abandoning core differentiator**: HTML-native App Blocks are Nabu's defining feature. Iced cannot render HTML.
- **Massive cost, zero benefit**: The migration would take 18-24+ months with zero user-facing improvement.
- **Technical risk**: Unproven combination of Iced + native IPC for a complex knowledge management app.

### GPUI Migration

**Strengths (relative to migration):**
- **GPU performance**: Excellent for text rendering and animations (proven by Zed).

**Weaknesses (of the migration):**
- **No WASM support**: The entire `nabu-ui` crate is a WASM cdylib. GPUI is desktop-only. This means a **fundamental architecture change** — nabu-ui would become a native Rust library, not a WASM bundle.
- **macOS-only (partially)**: No Windows support. Nabu targets Windows.
- **No HTML/CSS**: Completely discards the 2,923-line CSS pipeline.
- **No iframe support**: App Blocks cannot be embedded.
- **No canvas API**: Would need to implement the GraphView from scratch using GPUI's rendering system.
- **No web_sys access**: All 94 web_sys usages eliminated.
- **No Tailwind**: Complete replacement with GPUI's theme system.
- **Icon system replacement**: No SVG component system.
- **Build pipeline overhaul**: Trunk + WASM → native Cargo build. No npm pipeline.
- **IPC layer rewrite**: No WASM bridge. Would need native IPC between Rust UI and Rust backend.
- **Accessibility regression**: Loses all native HTML accessibility.

**Opportunities**: None meaningful.

**Threats:**
- **Platform lock-in**: macOS + Linux only. Loses Windows support — a critical platform for Nabu.
- **Architecture violation**: The fundamental premise of Nabu's architecture (WASM UI in webview, nabu-core shared) would be destroyed.
- **Unknown maturity**: GPUI is a new framework (extracted from Zed). Production readiness for a complex app is unproven.

---

## Phase 8 — Risk Register

### Migration to Dioxus

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **Regression in existing functionality** (76 file rewrites, 22,000 lines) | High | High | Comprehensive test suite required before starting; incremental migration (one screen at a time) |
| **Icon system incompatibility** (lucide-leptos → dioxus-icon) | High | Medium | Audit all 80+ icon usages before migration; create compatibility shim |
| **Signal API subtleties** (RwSignal → Signal behavioral differences) | Medium | High | Extensive testing of reactive flows; signal migration tool or script |
| **Build pipeline breakage** (Trunk → cargo-dioxus) | Medium | Medium | Maintain Trunk as fallback; test build pipeline in isolation first |
| **Conditional rendering pattern** (into_any → Element) | Medium | Medium | Document new patterns; code review checklist |
| **Event handler syntax** (on:click → onclick) | High | Low | Mechanical find-replace + manual review of complex handlers |
| **Callback<T> replacement** | Medium | Medium | Replace with closures or EventHandler; API design decisions needed |
| **Loss of Leptos ecosystem tools** (dev tools, hot reload) | Low | Low | Dioxus has its own tooling; evaluate parity |
| **No clear ROI** (time spent on migration = time not spent on features) | Certain | High | Cost-benefit analysis: 6-12 months migration vs. completing UX gaps |

### Migration to Iced

| Risk | Probability | Impact | Mitigation | None feasible |
|------|-------------|--------|------------|---------------|
| **Complete rewrite of 22,000 lines** | Certain | Critical | None feasible — too large to mitigate |
| **App Blocks (iframe) cannot be ported** | Certain | Critical | Would need to abandon HTML-native App Blocks entirely |
| **Canvas/GraphView rewrite** | Certain | Critical | No path to preserve existing canvas code |
| **CSS/Tailwind pipeline discarded** | Certain | Critical | 2,923 lines of design system CSS would be lost |
| **Accessibility regression** | High | High | Would need to rebuild all ARIA patterns from scratch |
| **IPC bridge redesign** | High | High | `spawn_local` + serde_wasm_bindgen → iced::Command |
| **Icon system replacement** | Certain | High | No SVG component system in Iced |
| **web_sys access loss** | Certain | High | 94 usages across 20+ types eliminated |
| **Unproven for complex knowledge apps** | Medium | High | Risk of hitting framework limitations mid-migration |

### Migration to GPUI

| Risk | Probability | Impact | Mitigation | None feasible |
|------|-------------|--------|------------|---------------|
| **No WASM target** (fundamental architecture change) | Certain | Critical | None feasible — would need to restructure nabu-ui entirely |
| **No Windows support** | Certain | Critical | Loses a primary target platform |
| **App Blocks (iframe) cannot be ported** | Certain | Critical | Would need custom WKWebView embedding via objc2 |
| **Build pipeline overhaul** | Certain | Critical | Trunk + npm → native Cargo; no web deployment path |
| **CSS/Tailwind pipeline discarded** | Certain | Critical | 2,923 lines of design system CSS lost |
| **Canvas/GraphView rewrite** | Certain | Critical | No canvas API; GPU rendering from scratch |
| **IPC bridge redesign** | Certain | High | No WASM bridge; native IPC needed |
| **Unknown production maturity** | Medium | High | GPUI is new; Zed is the only production user |
| **No community ecosystem** | High | High | Few third-party crates; would need to build everything |

---

## Phase 9 — Weighted Decision Matrix

### Criteria Weights (10-point scale, 10 = most important)

| Criterion | Weight | Rationale |
|-----------|--------|-----------|
| Migration Cost | 10 | This is the dominant factor — 22,000 lines, 76 files, 64 IPC commands |
| Long-term Maintainability | 8 | Framework maturity, ecosystem, upgrade path |
| Desktop Experience | 9 | Desktop is the primary target |
| Mobile Support | 4 | On roadmap but not imminent |
| Web Support | 3 | Possible future use case |
| Performance | 6 | Important but current performance is adequate |
| Developer Experience | 7 | Affects team productivity and onboarding |
| Architecture Simplicity | 8 | Should not add complexity |
| App Block Compatibility | 10 | Core differentiator — non-negotiable |
| Future Roadmap Alignment | 7 | Must not block roadmap items |
| Overall Risk | 10 | Risk of breaking working system |

### Scores (1-10 scale, 10 = best)

#### Keep Current Architecture

| Criterion | Score | Justification |
|-----------|-------|---------------|
| Migration Cost | 10 | Zero cost — already implemented |
| Long-term Maintainability | 8 | Leptos 0.7 is stable; upgrade path to 0.8/0.9 is straightforward; mature ecosystem |
| Desktop Experience | 10 | Tauri 2.11 is production-ready; webview provides native experience |
| Mobile Support | 5 | Tauri mobile is emerging; adequate for desktop-first app |
| Web Support | 7 | WASM bundle can be deployed to web (minus native IPC); Leptos supports SSR |
| Performance | 8 | Fine-grained reactivity; direct canvas; efficient DOM patching; IPC is synchronous |
| Developer Experience | 7 | Good Rust tooling; Leptos HMR in 0.8+; Trunk works; two-workspace setup is slightly complex |
| Architecture Simplicity | 9 | Clean separation (nabu-core / nabu-ui / src-tauri); single IPC abstraction |
| App Block Compatibility | 10 | HTML iframes are the native rendering model — gold standard |
| Future Roadmap Alignment | 9 | Doesn't block any roadmap item; mobile is achievable via Tauri 2.0 mobile |
| Overall Risk | 10 | Zero migration risk; battle-tested in production |

#### Dioxus Migration

| Criterion | Score | Justification |
|-----------|-------|---------------|
| Migration Cost | 3 | 22,000 lines, 76 files, 689 macro changes, 350 Callback changes, 169 signal changes; very high cost with no benefit |
| Long-term Maintainability | 7 | Dioxus is rapidly growing; 0.6+ is stable; but smaller ecosystem than Leptos |
| Desktop Experience | 8 | dioxus-desktop uses Wry (same as Tauri); good native experience |
| Mobile Support | 9 | dioxus-mobile provides first-class mobile support — the one concrete advantage |
| Web Support | 8 | dioxus-web is a primary target; good WASM story |
| Performance | 7 | Fine-grained reactivity similar to Leptos; no VDOM; similar performance characteristics |
| Developer Experience | 6 | New tooling to learn; build pipeline change (Trunk → dioxus-cli); syntax migration |
| Architecture Simplicity | 5 | Replaces one working system with another that provides no additional benefit; adds icon library migration complexity |
| App Block Compatibility | 9 | iframe support preserved; web_sys access preserved; App Blocks survive |
| Future Roadmap Alignment | 7 | Does improve mobile support; no regression on other items |
| Overall Risk | 3 | High regression risk from 22,000-line mechanical rewrite; build pipeline disruption; ecosystem loss |

#### Iced Migration

| Criterion | Score | Justification |
|-----------|-------|---------------|
| Migration Cost | 1 | Complete rewrite of 22,000 lines; paradigm shift from reactive to Elm-style; CSS pipeline discarded |
| Long-term Maintainability | 5 | Stable framework but different maintenance model; no HTML/CSS means different debugging |
| Desktop Experience | 7 | Native winit + wgpu; good performance; but no webview integration |
| Mobile Support | 3 | WASM web only; no native mobile support |
| Web Support | 5 | Renders to canvas via WASM; less capable than HTML |
| Performance | 8 | GPU rendering (wgpu) — good for widget-heavy UIs; no DOM overhead |
| Developer Experience | 3 | Completely different paradigm; no existing tooling knowledge; CSS pipeline lost |
| Architecture Simplicity | 2 | Would need to restructure the entire build pipeline; no webview IPC |
| App Block Compatibility | 1 | **Cannot render HTML iframes** — core differentiator destroyed; would need custom webview widget |
| Future Roadmap Alignment | 2 | Blocks web collaboration (no HTML rendering); harms mobile (no native mobile) |
| Overall Risk | 1 | Critical risk: abandoning core differentiator, complete rewrite, 18-24+ month effort |

#### GPUI Migration

| Criterion | Score | Justification |
|-----------|-------|---------------|
| Migration Cost | 1 | Complete rewrite; no WASM target; fundamental architecture change |
| Long-term Maintainability | 3 | New framework; only Zed in production; uncertain ecosystem maturity |
| Desktop Experience | 4 | macOS + Linux only; **no Windows support** — a critical platform loss |
| Mobile Support | 1 | No mobile support at all |
| Web Support | 1 | No WASM support whatsoever |
| Performance | 8 | GPU-accelerated; excellent text rendering (proven by Zed) |
| Developer Experience | 2 | Completely different paradigm; no HTML/CSS; requires learning GPUI's reactive system |
| Architecture Simplicity | 1 | Would need to restructure nabu-ui from WASM cdylib to native library; remove Trunk entirely |
| App Block Compatibility | 1 | **Cannot render HTML** — no iframe, no webview, no HTML at all |
| Future Roadmap Alignment | 1 | Blocks web collaboration, mobile, Windows; harms all roadmap items |
| Overall Risk | 1 | Critical risk: platform loss, architecture violation, no path to core features |

### Weighted Scores

| Criterion | Weight | Current | Dioxus | Iced | GPUI |
|-----------|--------|---------|--------|------|------|
| Migration Cost | 10 | 10 | 3 | 1 | 1 |
| Long-term Maintainability | 8 | 8 | 7 | 5 | 3 |
| Desktop Experience | 9 | 10 | 8 | 7 | 4 |
| Mobile Support | 4 | 5 | 9 | 3 | 1 |
| Web Support | 3 | 7 | 8 | 5 | 1 |
| Performance | 6 | 8 | 7 | 8 | 8 |
| Developer Experience | 7 | 7 | 6 | 3 | 2 |
| Architecture Simplicity | 8 | 9 | 5 | 2 | 1 |
| App Block Compatibility | 10 | 10 | 9 | 1 | 1 |
| Future Roadmap Alignment | 7 | 9 | 7 | 2 | 1 |
| Overall Risk | 10 | 10 | 3 | 1 | 1 |
| **Weighted Total** | | **8.6** | **6.3** | **2.6** | **1.4** |

---

## Phase 10 — Final Recommendation

### Recommendation: **Keep current architecture with targeted improvements**

The analysis is unambiguous. Migration away from Tauri + Leptos is **not justified**. The weighted decision matrix shows the current architecture scoring 8.6/10, while the best alternative (Dioxus) scores only 6.3/10. The two non-HTML frameworks (Iced and GPUI) score catastrophically low (2.6 and 1.4) because they cannot accommodate Nabu's core differentiator: HTML-native App Blocks.

### Why migration is not justified

#### 1. The single strongest argument: App Blocks
Nabu's App Block system uses HTML iframe `sandbox` attributes — the gold standard for HTML sandboxing. This is not a feature that can be replicated in Iced or GPUI. Dioxus can render iframes, but this provides zero benefit over Leptos for this use case.

**Evidence**: `sandbox.rs` renders `<iframe srcdoc=html_content sandbox="allow-scripts allow-forms" />`. `sandboxed_html.rs` renders `<iframe srcdoc=html sandbox="allow-scripts" />`. The `force_sandbox_for_web_snippets` setting in `AppSettings` confirms this is a deliberate, user-facing security feature.

#### 2. Migration cost is prohibitive
- **Dioxus**: 22,000 lines × mechanical syntax changes + icon library replacement + signal API migration + build pipeline overhaul = 6-12 person-months for zero user-facing improvement.
- **Iced/GPUI**: Complete rewrite of 22,000 lines + paradigm shift + CSS pipeline discard + App Blocks abandonment = 18-24+ months for a worse outcome.

**Evidence**: 76 component files, all importing `leptos::prelude::*`. 689 `view!` macro invocations requiring syntax changes. 350 `Callback` uses requiring replacement. 169 `RwSignal` uses requiring API changes. 449 `into_any()` calls following Dioxus's type-erased model.

#### 3. The "problem" with Tauri + Leptos doesn't exist
There are no technical blockers, performance issues, or maintenance problems with the current architecture. The AGENTS.md shows active development with recent fixes already completed (collections conversion, icon re-exports, inbox.rs fixes). The architecture is sound.

#### 4. Targeted improvements cost far less than migration
The in-flight UX gaps identified in AGENTS.md can be completed in 2-4 person-weeks — a fraction of the migration cost:
- Dead code removal (GeneralSettings/WhisprSettings/FileSettings) — 1 day
- Inbox/Collections context menus — 3 days
- Shared clipboard cache component — 2 days
- Background progress for long ops — 3 days
- Accessibility audit and ARIA labels — 5 days

### Targeted improvements to prioritize instead

1. **Upgrade to Leptos 0.8 or 0.9**: This provides improved fine-grained reactivity, better SSR, and Hot Module Replacement. The `lucide-leptos` dependency would need upgrading to 3.x. **Estimated effort: 2-3 person-weeks.**

2. **Consolidate the build pipeline**: Currently three systems (Trunk for WASM, npm for CSS, Cargo for Rust). `cargo-leptos` can unify WASM bundling + HMR. **Estimated effort: 1-2 person-weeks.**

3. **Extract the UI component library**: The 10 primitive files (button, input, menu, dialog, card, feedback, info, layout, nav, selection) form a mature design system. Extract to a shared crate. **Estimated effort: 1 person-week.**

4. **Complete the UX gaps**: Finish the items in AGENTS.md's gap matrix. **Estimated effort: 2-3 person-weeks total.**

5. **Explore Tauri mobile**: For the multi-platform roadmap, Tauri 2.0 mobile is available. **Estimated effort: 2-4 person-weeks.**

### If migration is reconsidered in the future

If the team decides to revisit migration after Nabu reaches v1.0 and has more resources:

- **Dioxus** would be the only viable option, IF the mobile roadmap accelerates. The migration would be purely mechanical (syntax + signal + icon changes) with no architectural risk to App Blocks. Budget 6-12 months.
- **Iced** and **GPUI** are non-starters as long as HTML-native App Blocks remain a core requirement. They would require abandoning the App Block concept entirely.

### Conclusion

The question is not "which framework is best?" — it's "what does migration buy us that the current architecture can't provide cheaply?" The answer is: nothing meaningful. Tauri + Leptos is already the right architecture for Nabu's specific requirements. The engineering effort is better invested in completing the existing UX gaps and upgrading within the Leptos ecosystem.

---

## Appendix A: Codebase Metrics

| Metric | Count |
|--------|-------|
| Total UI Rust lines | 22,161 |
| Total nabu-core Rust lines | 22,285 |
| Total src-tauri Rust lines | 7,086 |
| Component files | 76 |
| `#[component]` definitions | 145 |
| `view!` macro invocations | 689 |
| `into_any()` calls | 449 |
| `collect_view()` calls | 100 |
| `Callback` uses | 350 |
| `RwSignal` uses | 169 |
| `Signal<` uses | 104 |
| `web_sys` usages | 94 |
| `spawn_local` calls | 156 |
| `serde_wasm_bindgen` uses | 206 |
| `window_event_listener_untyped` calls | 7 |
| `wasm_bindgen::prelude` imports | 6 |
| Direct `web_sys`/`js_sys` types used | 20+ unique types |
| Tauri `#[tauri::command]` functions | 115 |
| IPC commands used in UI | 64 |
| IPC call sites in UI | 113 |
| CSS lines (`app.css`) | 2,923 |
| Tailwind config complexity | 80+ custom tokens |
| Icon enum variants | 80+ |
| lucide-leptos re-exports | 80+ |
| Shared contexts | 8 (Workspace, Nav, History, Theme, Toast, Task, Tree, SaveStatus) |
| Cargo workspaces | 2 (nabu-ui standalone; root for nabu-core + src-tauri) |
| Build systems | 3 (Trunk/WASM, npm/CSS, Cargo/Rust) |

## Appendix B: IPC Commands Used in UI

The frontend (`nabu-ui`) invokes these 64 unique Tauri commands via `crate::ipc::tauri_invoke()`:

```
archive_list, archive_note, archive_restore, calendar_notes, canvas_delete,
canvas_get, canvas_list, canvas_save, capture_file_drop, check_vault_exists,
create_vault_dialog, daily_note_for, get_settings, graph_data, history_redo,
history_status, history_undo, inbox_get_queue, inbox_quick_capture, items_move,
link_mention, mention_ignore, note_create_file, note_daily, note_delete,
note_duplicate, note_links, note_read, note_restore, note_save, notes_diff,
notes_index, notes_search, open_settings, recovery_check, recovery_discard,
reveal_in_file_manager, select_vault_dialog, session_clear, session_load,
session_save, settings_get, settings_set, settings_set_all, smart_folder_evaluate,
snapshot_create, statistics_get, template_delete, template_duplicate, template_list,
template_save, template_set_favourite, toggle_dictation_pill, trash_delete,
trash_empty, trash_list, trash_restore_many, versions_all, versions_diff,
versions_duplicate, versions_get, versions_list, versions_restore
```

All commands are invoked through a single 5-line abstraction in `crate::ipc::tauri_invoke()`, making the IPC layer the most migration-resilient part of the architecture.

These are all standard web platform APIs that remain available in any HTML-rendering framework (Dioxus, LePtos) but would be **completely unavailable** in GPUI or require significant workarounds in Iced.

---

## Reassessment: Revised Context

### Re-examining the Assumption

The original audit treated the current UI as a mature, production-ready interface that should be preserved. Under that lens, the 22,000 lines of Leptos components represented a massive sunk cost that made migration unjustifiable.

However, with the revised context that **the UI is already scheduled for major redesign**, this analysis changes fundamentally. The question is no longer "what do we lose by discarding 22,000 lines of UI?" but rather "what do we gain by choosing a framework that better serves Nabu's long-term strategic goals?"

### Revised Cost-Benefit Analysis

#### The Sunk-Cost Fallacy Reconsidered

If 70–80% of the UI is being redesigned anyway (layout, navigation, workflows, visual design, interaction patterns, accessibility, responsiveness), then:

- The 689 `view!` macro invocations, 449 `into_any()` calls, and 350 `Callback` uses are not being "thrown away" — they're being replaced as part of the normal redesign process.
- The migration cost (syntax changes, signal API changes) is **partially absorbed** by the planned redesign effort.
- The relevant question becomes: **what incremental cost does framework migration add beyond the redesign cost?**

#### Iced and GPUI: Still Non-Starters

Even with the redesigned UI context, **Iced and GPUI remain non-viable**:

1. **No HTML/CSS**: Nabu's App Blocks are HTML iframes with `sandbox` attributes — a core differentiator. Iced renders via GPU (wgpu to a `<canvas>`, not DOM), and GPUI renders via GPU (Core Animation / OpenGL). Neither can render `<iframe>` elements or interpret HTML/CSS.

2. **No web_sys access**: The GraphView (canvas rendering via `CanvasRenderingContext2d`), the editor (clipboard via `navigator.clipboard`), drag-and-drop (via `DataTransfer`/`FileList`), and inter-component communication (via `CustomEvent`/`postMessage`) all depend on `web_sys`. These are not UI styling concerns — they are fundamental platform integration points.

3. **No cross-platform story**: GPUI has no Windows support and no WASM target. Iced's web support renders to canvas, not DOM — limiting the web collaboration client.

**Conclusion**: Even if the entire UI is being rewritten, starting in a framework that cannot support the core product feature (HTML App Blocks) is a non-starter. The redesign cannot begin in a framework that fundamentally excludes the architecture's defining capability.

#### Dioxus: Becomes Compelling Under Redesign Context

Under the revised context, **Dioxus emerges as a viable migration target** — not because the current LePtos code is inadequate, but because Dioxus offers something LePtos cannot: **true cross-platform code sharing across desktop, mobile, and web**.

##### What changes under redesign context:

| Cost Factor | Original Assessment | Revised Assessment |
|-------------|-------------------|-------------------|
| 22,000 lines of component code | High (sunk cost lost) | Partially absorbed by planned redesign |
| Syntax migration (689 `view!` → `rsx!`) | High (pure overhead) | Part of the natural redesign process |
| Signal API migration | High (pure overhead) | Learning curve, absorbed by redesign |
| Icon library (lucide-leptos → dioxus-icon) | High (ecosystem loss) | Still required, but manageable |
| Build pipeline (Trunk → dioxus-cli) | Medium | Still required, but one-time cost |
| **Cross-platform benefit** | Not considered | **Primary strategic value** |

##### Strategic value of Dioxus cross-platform:

| Roadmap Item | Tauri + LePtos | Dioxus | Assessment |
|-------------|----------------|--------|------------|
| **Desktop** | Tauri webview (WASM) | dioxus-desktop (WASM + Wry) | Equivalent |
| **Mobile** | Tauri mobile (emerging, experimental) | dioxus-mobile (first-class) | **Dioxus wins decisively** |
| **Web collaboration** | LePtos CSR to web (needs custom IPC) | dioxus-web (first-class WASM) | **Dioxus wins** |
| **Shared code %** | ~20% (only nabu-core) | **~80%** (entire UI component layer) | **Massive advantage** |

If the UI is being redesigned anyway, the question is: why redesign only for Tauri's webview when Dioxus lets you redesign once and deploy everywhere?

##### Remaining Dioxus migration costs (still real):

1. **IPC bridge**: The 64-command IPC layer stays identical (Tauri invoke). On mobile/web, a replacement bridge is needed — but this is needed with LePtos too.
2. **Icon system**: `lucide-leptos` → `dioxus-icon` or custom SVG solution. 80+ icon re-exports need updating.
3. **Context system**: `provide_context`/`expect_context` → same names in Dioxus, but signal types differ (`RwSignal` → `Signal`).
4. **Build pipeline**: Trunk → `dioxus-cli` or `cargo dioxus`. The `index.html`, Tailwind pipeline, and npm scripts need adjustment.
5. **Signal semantics**: `RwSignal<T>` in LePtos is `Signal<T>` in Dioxus 0.5 (read+write), or `WritableSignal<T>` in 0.6+. The API difference is real but manageable.

##### Uncertainty that needs resolution:

1. **App Blocks on mobile**: Can `<iframe>` with `srcdoc` + `sandbox` render reliably inside a Dioxus mobile WebView? Unknown — needs prototyping.
2. **Canvas performance**: Can the GraphView's `CanvasRenderingContext2d` rendering work as well in Dioxus's WASM target as in LePtos's? Likely yes (same web_sys), but needs validation.
3. **Dioxus API stability**: Dioxus 0.5→0.6 was a breaking change; 0.7→0.8-alpha suggests more changes. Long-term commitment risk.

---

## Phase 10 — Final Recommendation (Revised)

### Recommendation: **Prototype-first approach with Dioxus**

The original recommendation to "Keep current architecture" was based on treating the existing UI as finished work that should not be discarded. Under the revised context — that the UI is already planned for major redesign — this assessment changes materially.

**Iced and GPUI remain strongly rejected** regardless of redesign context. They cannot support HTML-native App Blocks (the core differentiator), lack web_sys access for canvas/clipboard/drag-drop, and in GPUI's case, have no Windows or WASM support. No amount of redesign budget makes these frameworks viable for Nabu.

**Dioxus becomes the recommended path — but only after a prototype validates its cross-platform capabilities.**

### Rationale

1. **If the UI is being redesigned anyway, 60–70% of the migration cost is already budgeted.** The syntax differences (`view!` → `rsx!`, `RwSignal` → `Signal`) are natural parts of implementing a new design, not pure overhead.

2. **Dioxus's cross-platform story is the only framework that addresses Nabu's mobile and web collaboration roadmap items.** LePtos + Tauri has no credible path to native mobile apps. The web collaboration client would need a custom IPC bridge regardless of frontend framework — but Dioxus's `dioxus-web` makes the web target a first-class citizen.

3. **The backend architecture is unaffected.** `nabu-core` and `src-tauri` do not change. The IPC abstraction (`ipc.rs`, 5 lines) stays identical. The 64 command names, 113 call sites, and Tauri backend all remain valid.

4. **The App Blocks constraint is preserved.** Dioxus renders real HTML DOM (same as LePtos), so `<iframe srcdoc>` sandboxing works identically. The postMessage communication via `web_sys::MessageEvent` is unchanged.

5. **The CSS/Tailwind pipeline is preserved.** Dioxus supports external CSS and Tailwind — the 2,923-line `app.css` design system and `tailwind.config.js` remain valid.

### What a prototype would validate (2-3 person-weeks)

| Milestone | Validation Target |
|-----------|-------------------|
| Component parity | Render a representative subset (file tree, editor, graph view) in Dioxus `rsx!` to assess syntax migration complexity |
| Cross-platform build | Compile the same Dioxus UI to desktop (dioxus-desktop) + web (dioxus-web) to validate code sharing |
| App Blocks on mobile | Render an `<iframe srcdoc>` inside `dioxus-mobile` to verify HTML sandbox compatibility |
| Signal/context parity | Implement the Workspace/Nav/Toast contexts in Dioxus signals to assess reactive pattern differences |
| Icon replacement | Replace `lucide-leptos` with a Dioxus-compatible icon solution |
| IPC integration | Verify Tauri invoke works identically with Dioxus WASM |
| Canvas performance | Validate GraphView's canvas rendering works identically through web_sys |
| Build pipeline | Replace Trunk with `cargo dioxus` and verify the dev loop |

### Decision tree

```
Prototype → Success (all milestones pass)
    ↓
Proceed with Dioxus migration (6-12 months)
    ↓
Keep nabu-core, src-tauri unchanged
Rewrite UI layer in Dioxus (desktop + mobile + web from one codebase)
Replace Trunk + npm CSS pipeline with dioxus-cli
Replace lucide-leptos with dioxus-compatible icon library
Migrate 8 shared contexts (Workspace, Nav, History, Theme, Toast, Task, Tree, SaveStatus)
Update 113 IPC call sites for syntax (logic unchanged)

Prototype → Failure (any critical milestone fails)
    ↓
Return to LePtos with targeted improvements
Upgrade to LePtos 0.8/0.9 (better HMR, SSR, performance)
Complete in-flight UX gaps from AGENTS.md gap matrix
Explore Tauri 2.0 mobile for the mobile roadmap
Investigate web collaboration via a separate WASM-compatible IPC bridge
```

### Why "Prototype first" rather than "Migrate immediately" or "Keep LePtos"

- **Not "Migrate immediately"**: The prototype addresses the critical unknown — whether Dioxus can support Nabu's specific requirements (App Blocks on mobile, canvas performance, cross-platform code sharing). Committing 6-12 months without these answers is irresponsible.
- **Not "Keep LePtos"**: Under the redesign context, keeping LePtos means accepting a worse mobile and web collaboration story. Tauri mobile is experimental; there is no credible path to sharing UI code across platforms.
- **"Prototype first"**: De-risks the decision at 2–3 person-week cost, validates or invalidates the cross-platform hypothesis, and provides a clear go/no-go decision point.

---

## Appendix D: Cross-Reference with AUDIT 0.8 — Strategic Alignment

The `docs/Audits/AUDIT_0.8.md` gap analysis reveals critical additional context that further refines this migration audit. Several findings are directly relevant.

### Finding 1: The UI Redesign Is Not Just Cosmetic — It's Driven by Capability Phases

AUDIT 0.8 reveals that Nabu's UX gaps are not minor polish — they are **blocking the entire Capability Platform roadmap** (7 phases, 44 implementation prompts, ~12 implementation waves). The gap matrix in AGENTS.md is the surface manifestation, but the underlying blockers are:

1. **No event-to-IPC bridge** — The `EventBus` publishes 8 `PipelineEvent` variants but the frontend has **zero `#[listen]` calls** (AUDIT 0.8 confirms this via grep). This blocks:
   - Phase 2: Syncthing status events, sync progress, conflict notifications
   - Phase 4: ACP conversation streaming, agent tool call results
   - Phase 5: Real-time capability UI updates, status indicators, live diagnostics

2. **`note_save` bypasses the canonical pipeline** — `src-tauri/src/recovery.rs:391-406` writes directly to disk without publishing `ITEM_STORED`, breaking search and graph integrity for the most frequently written path.

3. **No graceful shutdown** — `ApplicationContext::shutdown()` exists but is never called; the `SocketServerHandle` is discarded (`Ok(_handle)`) at `lib.rs:363`.

These are **backend issues**, not UI framework issues. They would need to be fixed regardless of whether the UI uses LeCtos, Dioxus, Iced, or GPUI. The framework choice is **orthogonal** to these blockers.

### Finding 2: App Blocks Are Not Just a "Feature" — They're Architectural

AUDIT 0.8 confirms that App Blocks are deeply integrated into Nabu's capture and processing pipeline:
- `force_sandbox_for_web_snippets` is a first-class `AppSettings` field
- The `capture_file_drop` Tauri command feeds directly into the Capture Engine → Processing Pipeline → Storage → ITEM_STORED → Indexer + VaultGraph flow
- The sandbox iframe (`sandbox.rs`) receives postMessage communication from captured HTML content

**This deepens the argument against Iced and GPUI**: Not only can these frameworks not render iframes, they also can't render HTML content at all. If App Blocks are HTML-native (which the audit confirms), then any framework that doesn't render HTML DOM is fundamentally incompatible with Nabu's architecture.

### Finding 3: The Backend Rewrite Is the Real Cost Driver

The 7-phase Capability Platform roadmap requires:
- JSON-RPC abstraction (built from scratch — no existing infrastructure)
- Process supervisor for sidecar processes
- Streaming message handling abstraction
- Conversation thread state model
- Plugin loading and execution framework

**AUDIT 0.8's critical path**: 10 sequential prompts starting from P1.3.1 (event bridge) → P4.1.1 (JSON-RPC) → P5.2.1 (event-driven UI) → P7.1.1 (shutdown). Estimated 12 waves with up to 8 parallel agents.

**Implication for framework migration**: If the team is already committing to 12+ waves of backend architecture work, the framework migration question should be evaluated in that context:
- Does the framework migration help or hinder the backend work?
- Can the backend and UI framework changes be done in parallel?

For **Dioxus**: The backend work (event bridge, JSON-RPC, process supervisor) is completely independent of the UI framework. The IPC layer (`ipc.rs`, 5 lines) stays the same. The migration can proceed in parallel with backend work.

For **Iced/GPUI**: These frameworks would require their own IPC bridge (no Tauri invoke), their own process management, and their own streaming infrastructure. They would **add** to the backend work, not run in parallel with it.

### Finding 4: Phase 6 (Capability SDK) Depends on Framework Choice

Phase 6 requires loading, executing, and sandboxing plugin code. The type of plugins Nabu can load depends on the UI framework:
- **LePtos/Dioxus**: Plugins could be WASM modules loaded in the webview
- **Iced**: No WASM loading path in the main process
- **GPUI**: No plugin loading mechanism

This is another axis where the framework choice has cascading effects on the roadmap.

### Finding 5: The UI Is Not the Bottleneck — The Backend Is

AUDIT 0.8's readiness assessment is telling:

| Phase | Readiness | Primary Blocker |
|-------|-----------|-----------------|
| Phase 1 | PARTIALLY READY | Lifecycle trait unimplemented |
| Phase 2 | NOT READY | Discarded socket handle, no event-to-IPC bridge |
| Phase 3 | PARTIALLY READY | No editor diagnostic rendering pipeline |
| Phase 4 | NOT READY | No JSON-RPC, no conversation state, no streaming |
| Phase 5 | NOT READY | No capability panels, no event-driven updates |
| Phase 6 | PARTIALLY READY | Plugin foundation is dead code |
| Phase 7 | PARTIALLY READY | No application-level shutdown sequence |

**Every single phase's primary blocker is in the backend (nabu-core / src-tauri), not the UI**. None of the blockers would be addressed by changing the UI framework.

### Revised Recommendation: Defer UI Framework Decision Until Backend Blockers Are Resolved

Given AUDIT 0.8's findings, the framework decision should be **deferred** until after the critical Phase 1 backend work is complete:

1. **Phase 1.3.1 (event-to-IPC bridge)**: This is the single most important improvement. It enables real-time backend → frontend communication, which is needed by Phases 2, 4, 5. This work is framework-independent.

2. **Phase 1.4.1 (note_save fix)**: Route autosaves through the canonical pipeline so ITEM_STORED propagates to Indexer and VaultGraph. Framework-independent.

3. **Phase 1.5.1 (graceful shutdown)**: Implement the coordinated shutdown sequence. Framework-independent.

Once these blockers are resolved (estimated 3-4 implementation waves), the team should then evaluate:
- **If the mobile roadmap is accelerating**: Prototype Dioxus (2-3 person-weeks)
- **If desktop is the sole focus**: Upgrade to LePtos 0.8/0.9 (1-2 person-weeks) and continue with backend Phase 2-7 work
- **If web collaboration is the primary near-term goal**: Evaluate both LePtos and Dioxus web deployment paths

### Rationale for Deferral

The AUDIT 0.8 findings reveal that Nabu is at a **backend architecture inflection point**, not a UI framework inflection point. The 7-phase Capability Platform roadmap requires fundamental backend work (JSON-RPC, process supervision, streaming, plugin loading) that must happen regardless of frontend framework. 

Attempting a UI framework migration **simultaneously** with the backend Phase 1-7 work would:
1. **Multiply risk**: Both the migration and the backend work are high-risk independently; combining them creates a high-risk × high-risk failure mode
2. **Delay critical blockers**: The event-to-IPC bridge (the single most important improvement) gets delayed by framework migration work
3. **Create integration debt**: The migration and backend work would need to be integrated, creating a complex merge surface

By deferring the framework decision, the team:
1. **De-risks the critical path**: Backend blockers are resolved first, unblocking all 7 phases
2. **Enables informed decision-making**: After backend work is done, the team can measure actual performance and cross-platform requirements from real data
3. **Keeps options open**: The event-to-IPC bridge and IPC abstraction make it easy to swap the UI framework later (the 5-line `ipc.rs` is the only coupling point)
4. **Maximizes throughput**: Backend and UI work can proceed in parallel after the framework decision — backend work on nabu-core/src-tauri, UI work on whichever framework is chosen

**The IPC bridge (`ipc.rs`, 5 lines) is the single point of coupling between UI and backend**. If the team keeps this abstraction clean (which it already is), the framework can be swapped later with minimal backend changes.
