//! # Reader Mode — distraction-free reading experience
//!
//! Renders the active note's markdown in a clean, typography-optimised
//! layout. Supports focus mode (dim everything except the current paragraph),
//! adjustable line width, reading themes (sepia / dark / light), and font
//! sizing. All reader preferences are persisted via the settings store.
//!
//! The markdown renderer is a lightweight inline parser (headings, bold,
//! italic, code, links, lists, blockquotes, code blocks, tables) — no
//! external dependency, no HTML injection. It renders the same markdown
//! files the editor edits.

use crate::components::navigation::state::use_nav;
use crate::components::workspace::use_workspace;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

// ── Reader settings (persisted) ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReaderSettings {
    #[serde(default = "default_font_size")]
    font_size: u32,
    #[serde(default = "default_line_width")]
    line_width: u32,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default)]
    focus_mode: bool,
}

fn default_font_size() -> u32 {
    18
}
fn default_line_width() -> u32 {
    720
}
fn default_theme() -> String {
    "dark".to_string()
}

impl Default for ReaderSettings {
    fn default() -> Self {
        Self {
            font_size: default_font_size(),
            line_width: default_line_width(),
            theme: default_theme(),
            focus_mode: false,
        }
    }
}

const READER_SETTINGS_KEY: &str = "nabu.reader.settings";

fn load_reader_settings() -> ReaderSettings {
    // Synchronous-ish: we load via a blocking spawn in the component.
    ReaderSettings::default()
}

fn persist_reader_settings(settings: &ReaderSettings) {
    let value = serde_json::to_value(settings).unwrap_or_default();
    let args = serde_wasm_bindgen::to_value(
        &serde_json::json!({ "key": READER_SETTINGS_KEY, "value": value }),
    )
    .unwrap();
    spawn_local(async move {
        let _ = crate::ipc::tauri_invoke("settings_set", args).await;
    });
}

// ── Markdown renderer ───────────────────────────────────────────────

/// Renders a markdown string as safe HTML. This is a lightweight inline
/// parser that handles the most common markdown constructs. It escapes
/// HTML entities first, then applies formatting, so no raw HTML can be
/// injected.
fn render_markdown(md: &str) -> String {
    let escaped = html_escape(md);
    let mut html = String::with_capacity(escaped.len() * 2);
    let mut in_code_block = false;
    let mut in_table = false;
    let mut table_header_done = false;
    let mut lines = escaped.lines().peekable();

    while let Some(line) = lines.next() {
        // Code block fence
        if line.trim_start().starts_with("```") {
            if in_code_block {
                html.push_str("</code></pre>\n");
                in_code_block = false;
            } else {
                let lang = line.trim_start_matches('`').trim();
                html.push_str(&format!(
                    "<pre><code class=\"code-block\" data-lang=\"{}\">",
                    lang
                ));
                in_code_block = true;
            }
            continue;
        }
        if in_code_block {
            html.push_str(line);
            html.push('\n');
            continue;
        }

        // Table detection (line with | and next line with --- separators)
        if line.contains('|') && line.trim().starts_with('|') {
            if let Some(next) = lines.peek() {
                if next.contains("---") && next.contains('|') {
                    if !in_table {
                        html.push_str("<table class=\"md-table\">\n");
                        in_table = true;
                        table_header_done = false;
                    }
                    if !table_header_done {
                        html.push_str("<thead><tr>");
                        for cell in line.trim_matches('|').split('|') {
                            html.push_str(&format!("<th>{}</th>", cell.trim()));
                        }
                        html.push_str("</tr></thead><tbody>\n");
                        table_header_done = true;
                        lines.next(); // consume the separator line
                        continue;
                    }
                }
            }
            if in_table {
                html.push_str("<tr>");
                for cell in line.trim_matches('|').split('|') {
                    html.push_str(&format!("<td>{}</td>", cell.trim()));
                }
                html.push_str("</tr>\n");
                continue;
            }
        }
        if in_table {
            html.push_str("</tbody></table>\n");
            in_table = false;
        }

        // Headings
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            html.push_str(&format!("<h1>{}</h1>\n", render_inline(rest)));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            html.push_str(&format!("<h2>{}</h2>\n", render_inline(rest)));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            html.push_str(&format!("<h3>{}</h3>\n", render_inline(rest)));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("#### ") {
            html.push_str(&format!("<h4>{}</h4>\n", render_inline(rest)));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("##### ") {
            html.push_str(&format!("<h5>{}</h5>\n", render_inline(rest)));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("###### ") {
            html.push_str(&format!("<h6>{}</h6>\n", render_inline(rest)));
            continue;
        }

        // Blockquote
        if let Some(rest) = trimmed.strip_prefix("> ") {
            html.push_str(&format!("<blockquote>{}</blockquote>\n", render_inline(rest)));
            continue;
        }

        // Horizontal rule
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            html.push_str("<hr/>\n");
            continue;
        }

        // Task list items
        if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
            html.push_str(&format!(
                "<div class=\"task-item task-unchecked\"><input type=\"checkbox\" disabled /> {}</div>\n",
                render_inline(rest)
            ));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- [x] ") {
            html.push_str(&format!(
                "<div class=\"task-item task-checked\"><input type=\"checkbox\" checked disabled /> {}</div>\n",
                render_inline(rest)
            ));
            continue;
        }

        // Unordered list
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            let item = trimmed.trim_start_matches(['-', '*']).trim();
            html.push_str(&format!("<li>{}</li>\n", render_inline(item)));
            continue;
        }

        // Ordered list
        if let Some(rest) = trimmed.strip_prefix("1. ") {
            html.push_str(&format!("<li>{}</li>\n", render_inline(rest)));
            continue;
        }

        // Empty line → paragraph break
        if trimmed.is_empty() {
            html.push('\n');
            continue;
        }

        // Regular paragraph
        html.push_str(&format!("<p>{}</p>\n", render_inline(line)));
    }

    if in_code_block {
        html.push_str("</code></pre>\n");
    }
    if in_table {
        html.push_str("</tbody></table>\n");
    }

    html
}

/// Renders inline markdown: bold, italic, code, links, wikilinks.
fn render_inline(text: &str) -> String {
    let mut result = String::with_capacity(text.len() * 2);
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Bold: **text** or __text__
        if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
            if let Some(end) = find_double(&chars, i + 2, '*') {
                let inner: String = chars[i + 2..end].iter().collect();
                result.push_str(&format!("<strong>{}</strong>", render_inline(&inner)));
                i = end + 2;
                continue;
            }
        }
        // Italic: *text* or _text_
        if chars[i] == '*' || chars[i] == '_' {
            let marker = chars[i];
            if let Some(end) = find_single(&chars, i + 1, marker) {
                let inner: String = chars[i + 1..end].iter().collect();
                result.push_str(&format!("<em>{}</em>", render_inline(&inner)));
                i = end + 1;
                continue;
            }
        }
        // Inline code: `text`
        if chars[i] == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '`') {
                let inner: String = chars[i + 1..i + 1 + end].iter().collect();
                result.push_str(&format!("<code class=\"inline-code\">{}</code>", inner));
                i = i + 1 + end + 1;
                continue;
            }
        }
        // Wikilink: [[text]]
        if i + 1 < chars.len() && chars[i] == '[' && chars[i + 1] == '[' {
            // Find the closing ]]
            if let Some(close) = chars[i + 2..].windows(2).position(|w| w == [']', ']']) {
                let end = i + 2 + close;
                let inner: String = chars[i + 2..end].iter().collect();
                let target = inner.split('|').next().unwrap_or(&inner).to_string();
                result.push_str(&format!(
                    "<a class=\"wikilink\" href=\"#\" data-path=\"{}\">{}</a>",
                    target,
                    inner
                ));
                i = end + 2;
                continue;
            }
        }
        // Link: [text](url)
        if chars[i] == '[' {
            if let Some(close) = chars[i + 1..].iter().position(|&c| c == ']') {
                if chars.get(i + 1 + close + 1) == Some(&'(') {
                    if let Some(close_paren) = chars[i + 1 + close + 2..].iter().position(|&c| c == ')') {
                        let text_part: String = chars[i + 1..i + 1 + close].iter().collect();
                        let url_part: String = chars[i + 1 + close + 2..i + 1 + close + 2 + close_paren]
                            .iter()
                            .collect();
                        result.push_str(&format!(
                            "<a href=\"{}\" target=\"_blank\" rel=\"noopener\">{}</a>",
                            url_part, text_part
                        ));
                        i = i + 1 + close + 2 + close_paren + 1;
                        continue;
                    }
                }
            }
        }
        result.push(chars[i]);
        i += 1;
    }

    result
}

fn find_double(chars: &[char], start: usize, marker: char) -> Option<usize> {
    let mut i = start;
    while i + 1 < chars.len() {
        if chars[i] == marker && chars[i + 1] == marker {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn find_single(chars: &[char], start: usize, marker: char) -> Option<usize> {
    chars[start..].iter().position(|&c| c == marker).map(|p| start + p)
}

/// Escapes HTML special characters to prevent injection.
fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

// ── Reader Component ────────────────────────────────────────────────

#[component]
pub fn ReaderView() -> impl IntoView {
    let nav = use_nav();
    let workspace = use_workspace();

    let (content, set_content) = signal(String::new());
    let (settings, set_settings) = signal(load_reader_settings());
    let (loaded, set_loaded) = signal(false);
    let (show_settings, set_show_settings) = signal(false);

    // Load reader settings on mount.
    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(
            &serde_json::json!({ "key": READER_SETTINGS_KEY }),
        )
        .unwrap();
        let result = crate::ipc::tauri_invoke("settings_get", args).await;
        if let Ok(s) = serde_wasm_bindgen::from_value::<ReaderSettings>(result) {
            set_settings.set(s);
        }
    });

    // Load the active note's content.
    let load_content = move || {
        let path = workspace.active_path.get();
        spawn_local(async move {
            if let Some(ref p) = path {
                let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": p })).unwrap();
                let result = crate::ipc::tauri_invoke("note_read", args).await;
                if let Ok(text) = serde_wasm_bindgen::from_value::<String>(result) {
                    set_content.set(text);
                }
            }
            set_loaded.set(true);
        });
    };
    load_content();

    // Re-load when the active note changes.
    Effect::new(move |_| {
        let _ = workspace.active_path.get();
        load_content();
    });

    let rendered_html = move || render_markdown(&content.get());

    let update_settings = move |new: ReaderSettings| {
        persist_reader_settings(&new);
        set_settings.set(new);
    };

    let toggle_focus = move |_| {
        let mut s = settings.get();
        s.focus_mode = !s.focus_mode;
        update_settings(s);
    };

    let set_font_size = move |size: u32| {
        let mut s = settings.get();
        s.font_size = size;
        update_settings(s);
    };

    let set_line_width = move |width: u32| {
        let mut s = settings.get();
        s.line_width = width;
        update_settings(s);
    };

    let set_theme = move |theme: String| {
        let mut s = settings.get();
        s.theme = theme;
        update_settings(s);
    };

    let s = move || settings.get();

    view! {
        <div class="reader-view h-full overflow-auto bg-gray-950 text-gray-100"
            data-theme=move || s().theme
        >
            // Top toolbar (minimal, auto-hide)
            <div class="sticky top-0 z-10 flex items-center justify-between px-4 py-2 bg-gray-950/80 backdrop-blur border-b border-gray-800/50">
                <div class="flex items-center gap-3">
                    <span class="text-sm text-gray-400">
                        {move || workspace.active_path.get().unwrap_or_default()}
                    </span>
                </div>
                <div class="flex items-center gap-2">
                    <button
                        class=move || format!(
                            "px-2 py-1 text-xs rounded border {}",
                            if s().focus_mode { "bg-blue-900/50 border-blue-600 text-blue-300" } else { "border-gray-700 text-gray-400" }
                        )
                        on:click=toggle_focus
                        title="Toggle focus mode"
                    >
                        "🎯 Focus"
                    </button>
                    <button
                        class="px-2 py-1 text-xs rounded border border-gray-700 text-gray-400 hover:text-gray-200"
                        on:click=move |_| set_show_settings.set(!show_settings.get())
                    >
                        "⚙️"
                    </button>
                </div>
            </div>

            // Settings panel
            {move || if show_settings.get() {
                let current = settings.get();
                view! {
                    <div class="sticky top-12 z-10 mx-auto bg-gray-900 border border-gray-700 rounded-lg p-4 mb-4"
                        style=format!("max-width: {}px;", current.line_width)
                    >
                        <div class="space-y-3">
                            <div>
                                <label class="text-xs text-gray-500 uppercase tracking-wide">"Font Size"</label>
                                <div class="flex items-center gap-2 mt-1">
                                    <input type="range" min="14" max="28" value={current.font_size}
                                        class="flex-1"
                                        on:input=move |ev| {
                                            set_font_size(event_target_value(&ev).parse().unwrap_or(18));
                                        }
                                    />
                                    <span class="text-sm text-gray-400">{format!("{}px", current.font_size)}</span>
                                </div>
                            </div>
                            <div>
                                <label class="text-xs text-gray-500 uppercase tracking-wide">"Line Width"</label>
                                <div class="flex items-center gap-2 mt-1">
                                    <input type="range" min="480" max="960" step="40" value={current.line_width}
                                        class="flex-1"
                                        on:input=move |ev| {
                                            set_line_width(event_target_value(&ev).parse().unwrap_or(720));
                                        }
                                    />
                                    <span class="text-sm text-gray-400">{format!("{}px", current.line_width)}</span>
                                </div>
                            </div>
                            <div>
                                <label class="text-xs text-gray-500 uppercase tracking-wide">"Theme"</label>
                                <div class="flex gap-2 mt-1">
                                    {["dark", "sepia", "light"].iter().map(|theme| {
                                        let t = theme.to_string();
                                        let t_class = t.clone();
                                        let t_click = t.clone();
                                        view! {
                                            <button
                                                class=move || format!(
                                                    "px-3 py-1 text-xs rounded border {}",
                                                    if s().theme == t_class { "bg-blue-900/50 border-blue-600 text-blue-300" } else { "border-gray-700 text-gray-400" }
                                                )
                                                on:click=move |_| set_theme(t_click.clone())
                                            >
                                                {t}
                                            </button>
                                        }
                                    }).collect_view()}
                                </div>
                            </div>
                        </div>
                    </div>
                }.into_any()
            } else {
                view! {}.into_any()
            }}

            // Content
            <div class="reader-content mx-auto px-8 py-8"
                style=move || format!(
                    "max-width: {}px; font-size: {}px; line-height: 1.7;",
                    s().line_width, s().font_size
                )
            >
                {move || {
                    if !loaded.get() {
                        view! {
                            <div class="flex items-center justify-center py-20">
                                <crate::components::ui::feedback::LoadingBlock
                                    label="Loading…"
                                    size=crate::components::ui::feedback::SpinnerSize::Md
                                />
                            </div>
                        }.into_any()
                    } else if content.get().is_empty() {
                        view! {
                            <div class="flex items-center justify-center py-20 text-gray-500">
                                <div class="text-center">
                                    <div class="text-4xl mb-2">"📖"</div>
                                    <p class="text-sm">"Open a note to start reading"</p>
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        let html = rendered_html();
                        view! {
                            <div
                                class=move || format!(
                                    "reader-prose {}",
                                    if s().focus_mode { "reader-focus-mode" } else { "" }
                                )
                                inner_html=html
                            ></div>
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}