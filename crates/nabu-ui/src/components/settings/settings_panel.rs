//! # Settings Panel — Dioxus migration
//!
//! All 15 settings tabs are preserved with navigation, grouping, persistence,
//! and IPC interactions intact. Behaviour is preserved from the original
//! LePtOS version; only the framework glue changes.

use crate::components::ui::button::Button;
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

// ── AppSettings (mirrors backend) ───────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AppSettings {
    // Appearance
    pub theme: String,
    pub main_window_opacity: f32,
    pub floating_pill_opacity: f32,
    pub pill_hover_boost_opacity: bool,
    pub sidebar_width: f32,
    pub inspector_width: f32,
    pub font_size: f32,
    pub line_height: f32,
    pub reduced_motion: bool,
    pub high_contrast: bool,

    // Editor
    pub editor_mode: String,
    pub auto_pair_brackets: bool,
    pub show_line_numbers: bool,
    pub convert_pasted_html_to_markdown: bool,
    pub enable_notion_slash_menu: bool,
    pub auto_format_filler_words: bool,
    pub tab_size: u32,
    pub word_wrap: bool,
    pub spell_check: bool,
    pub auto_save_interval_secs: u32,

    // Markdown
    pub markdown_gfm: bool,
    pub markdown_preserve_line_breaks: bool,
    pub markdown_smart_quotes: bool,
    pub markdown_math_rendering: bool,
    pub markdown_diagram_rendering: bool,

    // Search
    pub search_index_on_startup: bool,
    pub search_max_results: u32,
    pub search_highlight_matches: bool,
    pub search_fuzzy_matching: bool,

    // Graph
    pub include_folders_in_graph: bool,
    pub folder_click_behavior: String,
    pub graph_node_physics_gravity: f32,
    pub graph_node_physics_spacing: f32,
    pub graph_show_tags_as_badges: bool,

    // Files & Vaults
    pub last_vault_path: String,
    #[serde(default)]
    pub recent_vaults: Vec<serde_json::Value>,
    pub default_new_note_path: String,
    pub trash_retention_policy: String,
    pub enable_daily_notes: bool,
    pub confirm_before_delete: bool,
    pub show_hidden_files: bool,
    pub sort_files_alphabetically: bool,

    // Import & Export
    pub default_export_format: String,
    pub export_include_metadata: bool,
    pub export_include_attachments: bool,
    pub import_duplicate_strategy: String,

    // OCR
    pub ocr_language: String,
    pub ocr_auto_process_scanned_pdfs: bool,
    pub ocr_confidence_threshold: f32,

    // Accessibility
    pub screen_reader_support: bool,
    pub keyboard_navigation: bool,
    pub focus_ring_visible: bool,

    // Performance
    pub max_undo_history: u32,
    pub worker_pool_size: u32,
    pub index_on_startup: bool,
    pub background_processing: bool,

    // Privacy
    pub launch_at_startup: bool,
    pub analytics_enabled: bool,
    pub crash_reporting_enabled: bool,
    pub auto_lock_on_idle: bool,
    pub auto_lock_timeout_mins: u32,

    // Keyboard Shortcuts
    pub voice_hotkey: String,
    pub quick_capture_hotkey: String,
    pub toggle_sidebar_hotkey: String,

    // Advanced
    pub force_sandbox_for_web_snippets: bool,
    pub debug_mode: bool,
    pub developer_tools: bool,
    pub experimental_features: bool,

    // Experimental
    pub whisper_model: String,
    pub enable_ai_summarization: bool,
    pub enable_semantic_search: bool,

    #[serde(default)]
    pub extra_settings: std::collections::HashMap<String, serde_json::Value>,
}

// ── Helpers ────────────────────────────────────────────────────────────────

/// Persists the full settings object to the backend via `settings_set_all`.
fn save_app_settings(settings: Signal<AppSettings>) {
    let updated = settings.read().clone();
    spawn_local(async move {
        let args = serde_wasm_bindgen::to_value(&updated).unwrap();
        let _ = crate::ipc::tauri_invoke("settings_set_all", args).await;
    });
}

/// Two-way checkbox bound to a bool settings field.
fn setting_checkbox(
    mut settings: Signal<AppSettings>,
    label: &'static str,
    get: fn(&AppSettings) -> bool,
    set: fn(&mut AppSettings, bool),
) -> Element {
    rsx! {
        label { class: "check-field" }
        input {
            r#type: "checkbox",
            checked: settings.with(|s| get(s)),
            onchange: move |ev: FormEvent| {
                let val = ev.checked();
                settings.with_mut(|s| set(s, val));
                save_app_settings(settings);
            },
        }
        span { "{label}" }
    }
}

/// Labeled number input bound to a `u32` settings field.
fn setting_number_u32(
    mut settings: Signal<AppSettings>,
    label: &str,
    min: u32,
    max: u32,
    default: u32,
    get: fn(&AppSettings) -> u32,
    set: fn(&mut AppSettings, u32),
) -> Element {
    rsx! {
        label { class: "field" }
        span { class: "field-label", "{label}" }
        input {
            r#type: "number",
            class: "input",
            min: "{min}",
            max: "{max}",
            value: "{settings.with(|s| get(s)).to_string()}",
            onchange: move |ev: FormEvent| {
                if let Ok(parsed) = ev.value().parse::<u32>() {
                    settings.with_mut(|s| set(s, parsed));
                } else {
                    settings.with_mut(|s| set(s, default));
                }
                save_app_settings(settings);
            },
        }
    }
}

/// Labeled range slider bound to an `f32` settings field.
fn setting_range_f32(
    mut settings: Signal<AppSettings>,
    label: &str,
    min: f32,
    max: f32,
    step: f32,
    default: f32,
    get: fn(&AppSettings) -> f32,
    set: fn(&mut AppSettings, f32),
) -> Element {
    rsx! {
        label { class: "field" }
        span { class: "field-label", "{label}" }
        input {
            r#type: "range",
            class: "input w-full",
            min: "{min}",
            max: "{max}",
            step: "{step}",
            value: "{settings.with(|s| get(s)).to_string()}",
            onchange: move |ev: FormEvent| {
                if let Ok(parsed) = ev.value().parse::<f32>() {
                    settings.with_mut(|s| set(s, parsed));
                } else {
                    settings.with_mut(|s| set(s, default));
                }
                save_app_settings(settings);
            },
        }
    }
}

/// Labeled text input bound to a `String` settings field.
fn setting_text(
    mut settings: Signal<AppSettings>,
    label: &str,
    get: fn(&AppSettings) -> String,
    set: fn(&mut AppSettings, String),
) -> Element {
    rsx! {
        label { class: "field" }
        span { class: "field-label", "{label}" }
        input {
            r#type: "text",
            class: "input",
            value: "{settings.with(|s| get(s))}",
            onchange: move |ev: FormEvent| {
                let val = ev.value();
                settings.with_mut(|s| set(s, val));
                save_app_settings(settings);
            },
        }
    }
}

/// Labeled <select> bound to a `String` settings field.
fn setting_select(
    mut settings: Signal<AppSettings>,
    label: &str,
    options: Vec<(&'static str, &'static str)>,
    get: fn(&AppSettings) -> String,
    set: fn(&mut AppSettings, String),
) -> Element {
    let value = settings.with(|s| get(s));
    rsx! {
        label { class: "field" }
        span { class: "field-label", "{label}" }
        select {
            class: "input",
            value: "{value}",
            onchange: move |ev: FormEvent| {
                let val = ev.value();
                settings.with_mut(|s| set(s, val));
                save_app_settings(settings);
            },
            for (val, lbl) in options {
                option { value: "{val}", "{lbl}" }
            }
        }
    }
}

// ── SettingsPanel ──────────────────────────────────────────────────────────

/// The root settings panel component.
///
/// Loads all settings from the backend on mount, renders a 15-tab sidebar
/// navigator, and delegates to section renderers for each tab.
#[component]
pub fn SettingsPanel() -> Element {
    let settings = use_signal(AppSettings::default);
    let active_tab = use_signal(|| "Appearance".to_string());

    // Load settings from backend on mount.
    use_effect(move || {
        let mut settings_load = settings;
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
            let result = crate::ipc::tauri_invoke("get_settings", args).await;
            if let Ok(loaded) = serde_wasm_bindgen::from_value::<AppSettings>(result) {
                settings_load.set(loaded);
            }
        });
    });

    let tabs = [
        "Appearance", "Editor", "Markdown", "Search", "Graph",
        "Files & Vaults", "Import & Export", "OCR", "Accessibility",
        "Performance", "Privacy", "Keyboard Shortcuts", "Advanced",
        "Experimental", "About",
    ];

    rsx! {
        div { class: "settings-panel flex h-full" }

        // Sidebar navigation
        nav {
            class: "w-1/4 border-r border-gray-700 bg-gray-900 p-2 flex flex-col gap-0.5",
            "aria-label": "Settings sections",
            for tab in tabs {
                {
                    let active = active_tab.read().as_str() == tab;
                    let mut at = active_tab;
                    rsx! {
                        button {
                            key: "{tab}",
                            r#type: "button",
                            class: if active {
                                "sidebar-item sidebar-item-active w-full text-left px-3 py-2 rounded"
                            } else {
                                "sidebar-item w-full text-left px-3 py-2 rounded"
                            },
                            onclick: move |_| {
                                at.set(tab.to_string());
                            },
                            "{tab}"
                        }
                    }
                }
            }
        }

        // Tab content
        div {
            class: "content w-3/4 p-4 text-white overflow-y-auto",
            match active_tab.read().as_str() {
                "Appearance"         => rsx! { { appearance_settings(settings) } },
                "Editor"             => rsx! { { editor_settings(settings) } },
                "Markdown"           => rsx! { { markdown_settings(settings) } },
                "Search"             => rsx! { { search_settings(settings) } },
                "Graph"              => rsx! { { graph_settings(settings) } },
                "Files & Vaults"     => rsx! { { files_settings(settings) } },
                "Import & Export"    => rsx! { { import_export_settings(settings) } },
                "OCR"                => rsx! { { ocr_settings(settings) } },
                "Accessibility"      => rsx! { { accessibility_settings(settings) } },
                "Performance"        => rsx! { { performance_settings(settings) } },
                "Privacy"            => rsx! { { privacy_settings(settings) } },
                "Keyboard Shortcuts" => rsx! { { keyboard_shortcuts_settings(settings) } },
                "Advanced"           => rsx! { { advanced_settings(settings) } },
                "Experimental"       => rsx! { { experimental_settings(settings) } },
                "About"              => rsx! { { about_settings() } },
                _                    => rsx! {},
            }
        }
    }
}

// ── Section renderers ──────────────────────────────────────────────────────

fn appearance_settings(settings: Signal<AppSettings>) -> Element {
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "Appearance & Opacity Controls" }
        div { class: "space-y-4" }

        {setting_select(settings, "Theme",
            vec![("Dark", "Dark"), ("Light", "Light"), ("System Sync", "System Sync")],
            |s| s.theme.clone(), |s, v| s.theme = v)}

        {setting_range_f32(settings, "Main Window Opacity", 0.2, 1.0, 0.05, 1.0,
            |s| s.main_window_opacity, |s, v| s.main_window_opacity = v)}
        {setting_range_f32(settings, "Floating Pill Opacity", 0.2, 1.0, 0.05, 0.8,
            |s| s.floating_pill_opacity, |s, v| s.floating_pill_opacity = v)}
        {setting_checkbox(settings, "Pill Hover Focus",
            |s| s.pill_hover_boost_opacity, |s, v| s.pill_hover_boost_opacity = v)}
        {setting_range_f32(settings, "Font Size", 0.75, 1.5, 0.05, 1.0,
            |s| s.font_size, |s, v| s.font_size = v)}
        {setting_range_f32(settings, "Line Height", 1.0, 2.0, 0.1, 1.5,
            |s| s.line_height, |s, v| s.line_height = v)}
        {setting_checkbox(settings, "Reduced Motion",
            |s| s.reduced_motion, |s, v| s.reduced_motion = v)}
        {setting_checkbox(settings, "High Contrast",
            |s| s.high_contrast, |s, v| s.high_contrast = v)}
        {setting_range_f32(settings, "Sidebar Width", 200.0, 600.0, 10.0, 280.0,
            |s| s.sidebar_width, |s, v| s.sidebar_width = v)}
        {setting_range_f32(settings, "Inspector Width", 200.0, 600.0, 10.0, 320.0,
            |s| s.inspector_width, |s, v| s.inspector_width = v)}
    }
}

fn editor_settings(settings: Signal<AppSettings>) -> Element {
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "Editor & Notion Block Menu" }
        div { class: "space-y-4" }

        {setting_select(settings, "Editing Mode",
            vec![("Live Preview", "Live Preview"), ("Source Markdown", "Source Markdown")],
            |s| s.editor_mode.clone(), |s, v| s.editor_mode = v)}

        {setting_checkbox(settings, "Auto-pair brackets",
            |s| s.auto_pair_brackets, |s, v| s.auto_pair_brackets = v)}
        {setting_checkbox(settings, "Show line numbers",
            |s| s.show_line_numbers, |s, v| s.show_line_numbers = v)}
        {setting_checkbox(settings, "Convert pasted HTML to Markdown",
            |s| s.convert_pasted_html_to_markdown, |s, v| s.convert_pasted_html_to_markdown = v)}
        {setting_checkbox(settings, "Enable Notion Slash Menu",
            |s| s.enable_notion_slash_menu, |s, v| s.enable_notion_slash_menu = v)}
        {setting_number_u32(settings, "Tab Size", 1, 16, 4,
            |s| s.tab_size, |s, v| s.tab_size = v)}
        {setting_checkbox(settings, "Word Wrap",
            |s| s.word_wrap, |s, v| s.word_wrap = v)}
        {setting_checkbox(settings, "Spell Check",
            |s| s.spell_check, |s, v| s.spell_check = v)}
        {setting_number_u32(settings, "Auto-Save Interval (seconds)", 5, 300, 30,
            |s| s.auto_save_interval_secs, |s, v| s.auto_save_interval_secs = v)}
    }
}

fn markdown_settings(settings: Signal<AppSettings>) -> Element {
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "Markdown Rendering" }
        div { class: "space-y-4" }

        {setting_checkbox(settings, "GitHub Flavored Markdown",
            |s| s.markdown_gfm, |s, v| s.markdown_gfm = v)}
        {setting_checkbox(settings, "Preserve Line Breaks",
            |s| s.markdown_preserve_line_breaks, |s, v| s.markdown_preserve_line_breaks = v)}
        {setting_checkbox(settings, "Smart Quotes",
            |s| s.markdown_smart_quotes, |s, v| s.markdown_smart_quotes = v)}
        {setting_checkbox(settings, "Math Rendering (LaTeX)",
            |s| s.markdown_math_rendering, |s, v| s.markdown_math_rendering = v)}
        {setting_checkbox(settings, "Diagram Rendering (Mermaid)",
            |s| s.markdown_diagram_rendering, |s, v| s.markdown_diagram_rendering = v)}
    }
}

fn search_settings(settings: Signal<AppSettings>) -> Element {
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "Search" }
        div { class: "space-y-4" }

        {setting_checkbox(settings, "Index on Startup",
            |s| s.search_index_on_startup, |s, v| s.search_index_on_startup = v)}
        {setting_number_u32(settings, "Max Results", 10, 500, 100,
            |s| s.search_max_results, |s, v| s.search_max_results = v)}
        {setting_checkbox(settings, "Highlight Matches",
            |s| s.search_highlight_matches, |s, v| s.search_highlight_matches = v)}
        {setting_checkbox(settings, "Fuzzy Matching",
            |s| s.search_fuzzy_matching, |s, v| s.search_fuzzy_matching = v)}
    }
}

fn graph_settings(settings: Signal<AppSettings>) -> Element {
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "Folder Graph & Canvas" }
        div { class: "space-y-4" }

        {setting_checkbox(settings, "Include Folders as Hub Nodes",
            |s| s.include_folders_in_graph, |s, v| s.include_folders_in_graph = v)}

        {setting_select(settings, "Folder Click Behavior",
            vec![("Open Folder Table View", "Open Folder Table View"), ("Browse Folder", "Browse Folder")],
            |s| s.folder_click_behavior.clone(), |s, v| s.folder_click_behavior = v)}

        {setting_range_f32(settings, "Gravity Strength", 0.0, 1.0, 0.1, 0.5,
            |s| s.graph_node_physics_gravity, |s, v| s.graph_node_physics_gravity = v)}
        {setting_range_f32(settings, "Node Spacing", 0.0, 1.0, 0.1, 1.0,
            |s| s.graph_node_physics_spacing, |s, v| s.graph_node_physics_spacing = v)}
        {setting_checkbox(settings, "Show Tags as Badges",
            |s| s.graph_show_tags_as_badges, |s, v| s.graph_show_tags_as_badges = v)}
    }
}

fn files_settings(settings: Signal<AppSettings>) -> Element {
    let vault_path = settings.with(|s| s.last_vault_path.clone());
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "Files & Vaults" }
        div { class: "space-y-4" }

        div {
            span { class: "text-sm text-gray-400", "Vault Location: " }
            span { "{vault_path}" }
        }

        div { class: "flex gap-2" }
        Button {
            on_click: move |_: MouseEvent| {
                let args = serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap();
                spawn_local(async move {
                    let _ = crate::ipc::tauri_invoke("open_settings", args).await;
                });
            },
            {"Change Vault..."}
        }

        {setting_select(settings, "Default New Note Path",
            vec![("Vault Root", "Vault Root"), ("Same Folder as Active Note", "Same Folder as Active Note"), ("Custom Subfolder", "Custom Subfolder")],
            |s| s.default_new_note_path.clone(), |s, v| s.default_new_note_path = v)}

        {setting_select(settings, "Trash Retention",
            vec![("7 Days", "7 Days"), ("30 Days", "30 Days"), ("90 Days", "90 Days"), ("Never", "Never")],
            |s| s.trash_retention_policy.clone(), |s, v| s.trash_retention_policy = v)}

        {setting_checkbox(settings, "Confirm Before Delete",
            |s| s.confirm_before_delete, |s, v| s.confirm_before_delete = v)}
        {setting_checkbox(settings, "Show Hidden Files",
            |s| s.show_hidden_files, |s, v| s.show_hidden_files = v)}
        {setting_checkbox(settings, "Sort Files Alphabetically",
            |s| s.sort_files_alphabetically, |s, v| s.sort_files_alphabetically = v)}
    }
}

fn import_export_settings(settings: Signal<AppSettings>) -> Element {
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "Import & Export" }
        div { class: "space-y-4" }

        {setting_select(settings, "Default Export Format",
            vec![("Markdown", "markdown"), ("HTML", "html"), ("PDF", "pdf"), ("Plain Text", "text"), ("JSON", "json")],
            |s| s.default_export_format.clone(), |s, v| s.default_export_format = v)}

        {setting_checkbox(settings, "Include Metadata in Export",
            |s| s.export_include_metadata, |s, v| s.export_include_metadata = v)}
        {setting_checkbox(settings, "Include Attachments in Export",
            |s| s.export_include_attachments, |s, v| s.export_include_attachments = v)}

        {setting_select(settings, "Import Duplicate Strategy",
            vec![("Skip", "skip"), ("Overwrite", "overwrite"), ("Rename", "rename")],
            |s| s.import_duplicate_strategy.clone(), |s, v| s.import_duplicate_strategy = v)}

        div { class: "mt-6 pt-4 border-t border-gray-700" }
        h3 { class: "text-lg font-semibold mb-2", "Settings Migration" }
        p { class: "text-sm text-gray-400 mb-4", "Export or import your settings between devices." }
        div { class: "flex gap-2" }
        Button { {"Export Settings"} }
        Button { {"Import Settings"} }
    }
}

fn ocr_settings(settings: Signal<AppSettings>) -> Element {
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "OCR Settings" }
        div { class: "space-y-4" }

        {setting_select(settings, "OCR Language",
            vec![("English", "eng"), ("Spanish", "spa"), ("French", "fra"), ("German", "deu"), ("Japanese", "jpn")],
            |s| s.ocr_language.clone(), |s, v| s.ocr_language = v)}

        {setting_checkbox(settings, "Auto-process Scanned PDFs",
            |s| s.ocr_auto_process_scanned_pdfs, |s, v| s.ocr_auto_process_scanned_pdfs = v)}

        {setting_range_f32(settings, "Confidence Threshold", 0.0, 1.0, 0.05, 0.7,
            |s| s.ocr_confidence_threshold, |s, v| s.ocr_confidence_threshold = v)}
    }
}

fn accessibility_settings(settings: Signal<AppSettings>) -> Element {
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "Accessibility" }
        div { class: "space-y-4" }

        {setting_checkbox(settings, "Screen Reader Support",
            |s| s.screen_reader_support, |s, v| s.screen_reader_support = v)}
        {setting_checkbox(settings, "Enhanced Keyboard Navigation",
            |s| s.keyboard_navigation, |s, v| s.keyboard_navigation = v)}
        {setting_checkbox(settings, "Visible Focus Ring",
            |s| s.focus_ring_visible, |s, v| s.focus_ring_visible = v)}
    }
}

fn performance_settings(settings: Signal<AppSettings>) -> Element {
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "Performance" }
        div { class: "space-y-4" }

        {setting_number_u32(settings, "Max Undo History", 10, 500, 100,
            |s| s.max_undo_history, |s, v| s.max_undo_history = v)}
        {setting_number_u32(settings, "Worker Pool Size", 1, 16, 4,
            |s| s.worker_pool_size, |s, v| s.worker_pool_size = v)}
        {setting_checkbox(settings, "Index on Startup",
            |s| s.index_on_startup, |s, v| s.index_on_startup = v)}
        {setting_checkbox(settings, "Background Processing",
            |s| s.background_processing, |s, v| s.background_processing = v)}
    }
}

fn privacy_settings(settings: Signal<AppSettings>) -> Element {
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "Privacy" }
        div { class: "space-y-4" }

        {setting_checkbox(settings, "Launch at Startup",
            |s| s.launch_at_startup, |s, v| s.launch_at_startup = v)}
        {setting_checkbox(settings, "Analytics Enabled",
            |s| s.analytics_enabled, |s, v| s.analytics_enabled = v)}
        {setting_checkbox(settings, "Crash Reporting",
            |s| s.crash_reporting_enabled, |s, v| s.crash_reporting_enabled = v)}
        {setting_checkbox(settings, "Auto-lock on Idle",
            |s| s.auto_lock_on_idle, |s, v| s.auto_lock_on_idle = v)}
        {setting_number_u32(settings, "Auto-lock Timeout (minutes)", 1, 120, 15,
            |s| s.auto_lock_timeout_mins, |s, v| s.auto_lock_timeout_mins = v)}
    }
}

fn keyboard_shortcuts_settings(settings: Signal<AppSettings>) -> Element {
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "Keyboard Shortcuts" }
        div { class: "space-y-4" }

        {setting_text(settings, "Voice Dictation",
            |s| s.voice_hotkey.clone(), |s, v| s.voice_hotkey = v)}
        {setting_text(settings, "Quick Capture",
            |s| s.quick_capture_hotkey.clone(), |s, v| s.quick_capture_hotkey = v)}
        {setting_text(settings, "Toggle Sidebar",
            |s| s.toggle_sidebar_hotkey.clone(), |s, v| s.toggle_sidebar_hotkey = v)}
    }
}

fn advanced_settings(settings: Signal<AppSettings>) -> Element {
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "Advanced" }
        div { class: "space-y-4" }

        {setting_checkbox(settings, "Force Sandbox for Web Snippets",
            |s| s.force_sandbox_for_web_snippets, |s, v| s.force_sandbox_for_web_snippets = v)}
        {setting_checkbox(settings, "Debug Mode",
            |s| s.debug_mode, |s, v| s.debug_mode = v)}
        {setting_checkbox(settings, "Developer Tools",
            |s| s.developer_tools, |s, v| s.developer_tools = v)}
        {setting_checkbox(settings, "Experimental Features",
            |s| s.experimental_features, |s, v| s.experimental_features = v)}

        div { class: "mt-6 pt-4 border-t border-gray-700" }
        Button { {"Reset to Defaults"} }
    }
}

fn experimental_settings(settings: Signal<AppSettings>) -> Element {
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "Experimental" }
        div { class: "space-y-4" }

        {setting_select(settings, "Whisper Model",
            vec![("ggml-tiny.en.bin", "ggml-tiny.en.bin"), ("ggml-base.en.bin", "ggml-base.en.bin"), ("ggml-small.en-q5_0.bin", "ggml-small.en-q5_0.bin")],
            |s| s.whisper_model.clone(), |s, v| s.whisper_model = v)}

        {setting_checkbox(settings, "Enable AI Summarization",
            |s| s.enable_ai_summarization, |s, v| s.enable_ai_summarization = v)}
        {setting_checkbox(settings, "Enable Semantic Search",
            |s| s.enable_semantic_search, |s, v| s.enable_semantic_search = v)}
    }
}

fn about_settings() -> Element {
    rsx! {
        h2 { class: "text-xl font-bold mb-4", "About Nabu" }
        div { class: "space-y-4" }
        div {
            p { class: "text-lg font-semibold", "Nabu" }
            p { class: "text-sm text-gray-400", "Version 0.1.0" }
            p { class: "text-sm text-gray-400", "Premium Markdown Knowledge Management" }
        }
        div { class: "pt-4 border-t border-gray-700" }
        p { class: "text-sm text-gray-400", "© 2024 Faro Labs" }
        p { class: "text-sm text-gray-400", "Licensed under AGPL-3.0" }
    }
}
