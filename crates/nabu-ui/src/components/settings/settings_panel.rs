use crate::components::ui::button::Button;
use crate::components::ui::nav::SidebarItem;
use crate::components::ui::selection::{Checkbox, Select, SelectOption};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

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

#[component]
pub fn SettingsPanel() -> impl IntoView {
    let settings = RwSignal::new(AppSettings::default());

    spawn_local(async move {
        let result = crate::ipc::tauri_invoke(
            "get_settings",
            serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap(),
        )
        .await;
        if let Ok(loaded_settings) = serde_wasm_bindgen::from_value::<AppSettings>(result) {
            settings.set(loaded_settings);
        }
    });

    let save_settings = Callback::new(move |updated_settings: AppSettings| {
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&updated_settings).unwrap();
            let _ = crate::ipc::tauri_invoke("settings_set_all", args).await;
        });
    });

    let (active_tab, set_active_tab) = signal("Search".to_string());
    let tabs = vec![
        "Appearance",
        "Editor",
        "Markdown",
        "Search",
        "Graph",
        "Files & Vaults",
        "Import & Export",
        "OCR",
        "Accessibility",
        "Performance",
        "Privacy",
        "Keyboard Shortcuts",
        "Advanced",
        "Experimental",
        "About",
    ];

    view! {
        <div class="settings-panel flex h-full">
            <nav class="w-1/4 border-r border-gray-700 bg-gray-900 p-2 flex flex-col gap-0.5" aria-label="Settings sections">
                {move || tabs.iter().map(|tab_str| {
                    let tab = tab_str.to_string();
                    let tab_click = tab.clone();
                    let is_active = active_tab.get() == tab;
                    view! {
                        <SidebarItem
                            label=tab
                            active=is_active
                            on_click=Callback::new(move |_| set_active_tab.set(tab_click.clone()))
                        />
                    }
                }).collect_view()}
            </nav>
            <div class="content w-3/4 p-4 text-white">
                {move || match active_tab.get().as_str() {
                    "Appearance" => view! { <AppearanceSettings settings=settings save=save_settings /> }.into_any(),
                    "Editor" => view! { <EditorSettings settings=settings save=save_settings /> }.into_any(),
                    "Markdown" => view! { <MarkdownSettings settings=settings save=save_settings /> }.into_any(),
                    "Search" => view! { <SearchSettings settings=settings save=save_settings /> }.into_any(),
                    "Graph" => view! { <GraphSettings settings=settings save=save_settings /> }.into_any(),
                    "Files & Vaults" => view! { <FilesSettings settings=settings save=save_settings /> }.into_any(),
                    "Import & Export" => view! { <ImportExportSettings settings=settings save=save_settings /> }.into_any(),
                    "OCR" => view! { <OCRSettings settings=settings save=save_settings /> }.into_any(),
                    "Accessibility" => view! { <AccessibilitySettings settings=settings save=save_settings /> }.into_any(),
                    "Performance" => view! { <PerformanceSettings settings=settings save=save_settings /> }.into_any(),
                    "Privacy" => view! { <PrivacySettings settings=settings save=save_settings /> }.into_any(),
                    "Keyboard Shortcuts" => view! { <KeyboardShortcutsSettings settings=settings save=save_settings /> }.into_any(),
                    "Advanced" => view! { <AdvancedSettings settings=settings save=save_settings /> }.into_any(),
                    "Experimental" => view! { <ExperimentalSettings settings=settings save=save_settings /> }.into_any(),
                    "About" => view! { <AboutSettings /> }.into_any(),
                    _ => view! {}.into_any(),
                }}
            </div>
        </div>
    }
}

/// Two-way check helper: builds a `Checkbox` bound to a bool settings field.
#[component]
fn SettingCheckbox(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
    label: &'static str,
    get: Callback<AppSettings, bool>,
    set: Callback<(AppSettings, bool), ()>,
) -> impl IntoView {
    let checked = RwSignal::new(get.run(settings.get()));
    Effect::new(move |_| {
        let current = get.run(settings.get());
        if checked.get_untracked() != current {
            checked.set(current);
        }
    });
    let on_change = Callback::new(move |new_value: bool| {
        checked.set(new_value);
        let s = settings.get();
        set.run((s.clone(), new_value));
        settings.set(s.clone());
        save.run(s);
    });
    view! {
        <Checkbox checked=checked on_change=on_change label=label.to_string() />
    }
}

#[component]
fn GeneralSettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"General & Modules"</h2>
        <div class="space-y-4">
            <div><label class="text-sm text-gray-400">"Vault Location: " {move || settings.get().last_vault_path}</label></div>
            <Button>"Change Vault..."</Button>
            <SettingCheckbox
                settings=settings
                save=save
                label="Enable Daily Notes"
                get=Callback::new(|s: AppSettings| s.enable_daily_notes)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.enable_daily_notes = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Launch at Startup"
                get=Callback::new(|s: AppSettings| s.launch_at_startup)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.launch_at_startup = v)
            />
        </div>
    }
}

#[component]
fn EditorSettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Editor & Notion Block Menu"</h2>
        <div class="space-y-4">
            <Select
                label="Editing Mode"
                options=vec![
                    SelectOption::new("Live Preview", "Live Preview"),
                    SelectOption::new("Source Markdown", "Source Markdown"),
                ]
                value=derive_string_field(settings, |s: &AppSettings| s.editor_mode.clone())
                on_change=set_string_field(settings, save, |s: &mut AppSettings, v: String| s.editor_mode = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Auto-pair brackets"
                get=Callback::new(|s: AppSettings| s.auto_pair_brackets)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.auto_pair_brackets = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Show line numbers"
                get=Callback::new(|s: AppSettings| s.show_line_numbers)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.show_line_numbers = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Convert pasted HTML to Markdown"
                get=Callback::new(|s: AppSettings| s.convert_pasted_html_to_markdown)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.convert_pasted_html_to_markdown = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Enable Notion Slash Menu"
                get=Callback::new(|s: AppSettings| s.enable_notion_slash_menu)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.enable_notion_slash_menu = v)
            />
            <label class="field">
                <span class="field-label">"Tab Size"</span>
                <input
                    type="number"
                    class="input"
                    min="1"
                    max="16"
                    prop:value=move || settings.get().tab_size.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.tab_size = event_target_value(&ev).parse().unwrap_or(4);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
            <SettingCheckbox
                settings=settings
                save=save
                label="Word Wrap"
                get=Callback::new(|s: AppSettings| s.word_wrap)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.word_wrap = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Spell Check"
                get=Callback::new(|s: AppSettings| s.spell_check)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.spell_check = v)
            />
            <label class="field">
                <span class="field-label">"Auto-Save Interval (seconds)"</span>
                <input
                    type="number"
                    class="input"
                    min="5"
                    max="300"
                    prop:value=move || settings.get().auto_save_interval_secs.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.auto_save_interval_secs = event_target_value(&ev).parse().unwrap_or(30);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
        </div>
    }
}

#[component]
fn WhisprSettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Whispr AI & Voice Dictation"</h2>
        <div class="space-y-4">
            <Select
                label="Model"
                options=vec![
                    SelectOption::new("ggml-tiny.en.bin", "ggml-tiny.en.bin"),
                    SelectOption::new("ggml-base.en.bin", "ggml-base.en.bin"),
                    SelectOption::new("ggml-small.en-q5_0.bin", "ggml-small.en-q5_0.bin"),
                ]
                value=derive_string_field(settings, |s: &AppSettings| s.whisper_model.clone())
                on_change=set_string_field(settings, save, |s: &mut AppSettings, v: String| s.whisper_model = v)
            />
            <label class="field">
                <span class="field-label">"Voice Hotkey"</span>
                <input
                    type="text"
                    class="input"
                    prop:value=derive_string_field(settings, |s: &AppSettings| s.voice_hotkey.clone())
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.voice_hotkey = event_target_value(&ev);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
            <SettingCheckbox
                settings=settings
                save=save
                label="Auto-format filler words"
                get=Callback::new(|s: AppSettings| s.auto_format_filler_words)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.auto_format_filler_words = v)
            />
        </div>
    }
}

#[component]
fn AppearanceSettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Appearance & Opacity Controls"</h2>
        <div class="space-y-4">
            <Select
                label="Theme"
                options=vec![
                    SelectOption::new("Dark", "Dark"),
                    SelectOption::new("Light", "Light"),
                    SelectOption::new("System Sync", "System Sync"),
                ]
                value=derive_string_field(settings, |s: &AppSettings| s.theme.clone())
                on_change=set_string_field(settings, save, |s: &mut AppSettings, v: String| s.theme = v)
            />
            <label class="field">
                <span class="field-label">"Main Window Opacity"</span>
                <input
                    type="range"
                    class="input w-full"
                    min="0.2"
                    max="1.0"
                    step="0.05"
                    prop:value=move || settings.get().main_window_opacity.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.main_window_opacity = event_target_value(&ev).parse().unwrap_or(1.0);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
            <label class="field">
                <span class="field-label">"Floating Pill Opacity"</span>
                <input
                    type="range"
                    class="input w-full"
                    min="0.2"
                    max="1.0"
                    step="0.05"
                    prop:value=move || settings.get().floating_pill_opacity.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.floating_pill_opacity = event_target_value(&ev).parse().unwrap_or(0.8);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
            <SettingCheckbox
                settings=settings
                save=save
                label="Pill Hover Focus"
                get=Callback::new(|s: AppSettings| s.pill_hover_boost_opacity)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.pill_hover_boost_opacity = v)
            />
            <label class="field">
                <span class="field-label">"Font Size"</span>
                <input
                    type="range"
                    class="input w-full"
                    min="0.75"
                    max="1.5"
                    step="0.05"
                    prop:value=move || settings.get().font_size.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.font_size = event_target_value(&ev).parse().unwrap_or(1.0);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
            <label class="field">
                <span class="field-label">"Line Height"</span>
                <input
                    type="range"
                    class="input w-full"
                    min="1.0"
                    max="2.0"
                    step="0.1"
                    prop:value=move || settings.get().line_height.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.line_height = event_target_value(&ev).parse().unwrap_or(1.5);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
            <SettingCheckbox
                settings=settings
                save=save
                label="Reduced Motion"
                get=Callback::new(|s: AppSettings| s.reduced_motion)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.reduced_motion = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="High Contrast"
                get=Callback::new(|s: AppSettings| s.high_contrast)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.high_contrast = v)
            />
            <label class="field">
                <span class="field-label">"Sidebar Width"</span>
                <input
                    type="range"
                    class="input w-full"
                    min="200"
                    max="600"
                    step="10"
                    prop:value=move || settings.get().sidebar_width.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.sidebar_width = event_target_value(&ev).parse().unwrap_or(280.0);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
            <label class="field">
                <span class="field-label">"Inspector Width"</span>
                <input
                    type="range"
                    class="input w-full"
                    min="200"
                    max="600"
                    step="10"
                    prop:value=move || settings.get().inspector_width.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.inspector_width = event_target_value(&ev).parse().unwrap_or(320.0);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
        </div>
    }
}

#[component]
fn FileSettings(settings: RwSignal<AppSettings>, save: Callback<AppSettings, ()>) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Files, Trash & Sandboxing"</h2>
        <div class="space-y-4">
            <Select
                label="Default New Note Path"
                options=vec![
                    SelectOption::new("Vault Root", "Vault Root"),
                    SelectOption::new("Same Folder as Active Note", "Same Folder as Active Note"),
                    SelectOption::new("Custom Subfolder", "Custom Subfolder"),
                ]
                value=derive_string_field(settings, |s: &AppSettings| s.default_new_note_path.clone())
                on_change=set_string_field(settings, save, |s: &mut AppSettings, v: String| s.default_new_note_path = v)
            />
            <Select
                label="Trash Retention"
                options=vec![
                    SelectOption::new("7 Days", "7 Days"),
                    SelectOption::new("30 Days", "30 Days"),
                    SelectOption::new("90 Days", "90 Days"),
                    SelectOption::new("Never", "Never"),
                ]
                value=derive_string_field(settings, |s: &AppSettings| s.trash_retention_policy.clone())
                on_change=set_string_field(settings, save, |s: &mut AppSettings, v: String| s.trash_retention_policy = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Sandbox Security (iframe)"
                get=Callback::new(|s: AppSettings| s.force_sandbox_for_web_snippets)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.force_sandbox_for_web_snippets = v)
            />
        </div>
    }
}

#[component]
fn GraphSettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Folder Graph & Canvas"</h2>
        <div class="space-y-4">
            <SettingCheckbox
                settings=settings
                save=save
                label="Include Folders as Hub Nodes"
                get=Callback::new(|s: AppSettings| s.include_folders_in_graph)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.include_folders_in_graph = v)
            />
            <Select
                label="Folder Click Behavior"
                options=vec![
                    SelectOption::new("Open Folder Table View", "Open Folder Table View"),
                    SelectOption::new("Browse Folder", "Browse Folder"),
                ]
                value=derive_string_field(settings, |s: &AppSettings| s.folder_click_behavior.clone())
                on_change=set_string_field(settings, save, |s: &mut AppSettings, v: String| s.folder_click_behavior = v)
            />
            <label class="field">
                <span class="field-label">"Gravity Strength"</span>
                <input
                    type="range"
                    class="input w-full"
                    min="0"
                    max="1"
                    step="0.1"
                    prop:value=move || settings.get().graph_node_physics_gravity.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.graph_node_physics_gravity = event_target_value(&ev).parse().unwrap_or(0.5);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
            <label class="field">
                <span class="field-label">"Node Spacing"</span>
                <input
                    type="range"
                    class="input w-full"
                    min="0"
                    max="1"
                    step="0.1"
                    prop:value=move || settings.get().graph_node_physics_spacing.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.graph_node_physics_spacing = event_target_value(&ev).parse().unwrap_or(1.0);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
            <SettingCheckbox
                settings=settings
                save=save
                label="Show Tags as Badges"
                get=Callback::new(|s: AppSettings| s.graph_show_tags_as_badges)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.graph_show_tags_as_badges = v)
            />
        </div>
    }
}

/// Creates a derived read-only signal for a string settings field.
fn derive_string_field(
    settings: RwSignal<AppSettings>,
    get: impl Fn(&AppSettings) -> String + Copy + 'static,
) -> RwSignal<String> {
    let signal = RwSignal::new(get(&settings.get()));
    Effect::new(move |_| {
        let current = get(&settings.get());
        if signal.get_untracked() != current {
            signal.set(current);
        }
    });
    signal
}

/// Builds the write-back callback for a string settings field.
fn set_string_field(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
    set: impl Fn(&mut AppSettings, String) + Copy + Send + Sync + 'static,
) -> Callback<String, ()> {
    Callback::new(move |new_value: String| {
        let mut s = settings.get();
        set(&mut s, new_value.clone());
        settings.set(s.clone());
        save.run(s);
    })
}

// ── New Settings Sections ────────────────────────────────────────────────

#[component]
fn MarkdownSettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Markdown Rendering"</h2>
        <div class="space-y-4">
            <SettingCheckbox
                settings=settings
                save=save
                label="GitHub Flavored Markdown"
                get=Callback::new(|s: AppSettings| s.markdown_gfm)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.markdown_gfm = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Preserve Line Breaks"
                get=Callback::new(|s: AppSettings| s.markdown_preserve_line_breaks)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.markdown_preserve_line_breaks = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Smart Quotes"
                get=Callback::new(|s: AppSettings| s.markdown_smart_quotes)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.markdown_smart_quotes = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Math Rendering (LaTeX)"
                get=Callback::new(|s: AppSettings| s.markdown_math_rendering)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.markdown_math_rendering = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Diagram Rendering (Mermaid)"
                get=Callback::new(|s: AppSettings| s.markdown_diagram_rendering)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.markdown_diagram_rendering = v)
            />
        </div>
    }
}

#[component]
fn SearchSettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Search"</h2>
        <div class="space-y-4">
            <SettingCheckbox
                settings=settings
                save=save
                label="Index on Startup"
                get=Callback::new(|s: AppSettings| s.search_index_on_startup)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.search_index_on_startup = v)
            />
            <label class="field">
                <span class="field-label">"Max Results"</span>
                <input
                    type="number"
                    class="input"
                    min="10"
                    max="500"
                    prop:value=move || settings.get().search_max_results.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.search_max_results = event_target_value(&ev).parse().unwrap_or(100);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
            <SettingCheckbox
                settings=settings
                save=save
                label="Highlight Matches"
                get=Callback::new(|s: AppSettings| s.search_highlight_matches)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.search_highlight_matches = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Fuzzy Matching"
                get=Callback::new(|s: AppSettings| s.search_fuzzy_matching)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.search_fuzzy_matching = v)
            />
        </div>
    }
}

#[component]
fn FilesSettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Files & Vaults"</h2>
        <div class="space-y-4">
            <label class="text-sm text-gray-400">"Vault Location: " {move || settings.get().last_vault_path}</label>
            <Button>"Change Vault..."</Button>
            <Select
                label="Default New Note Path"
                options=vec![
                    SelectOption::new("Vault Root", "Vault Root"),
                    SelectOption::new("Same Folder as Active Note", "Same Folder as Active Note"),
                    SelectOption::new("Custom Subfolder", "Custom Subfolder"),
                ]
                value=derive_string_field(settings, |s: &AppSettings| s.default_new_note_path.clone())
                on_change=set_string_field(settings, save, |s: &mut AppSettings, v: String| s.default_new_note_path = v)
            />
            <Select
                label="Trash Retention"
                options=vec![
                    SelectOption::new("7 Days", "7 Days"),
                    SelectOption::new("30 Days", "30 Days"),
                    SelectOption::new("90 Days", "90 Days"),
                    SelectOption::new("Never", "Never"),
                ]
                value=derive_string_field(settings, |s: &AppSettings| s.trash_retention_policy.clone())
                on_change=set_string_field(settings, save, |s: &mut AppSettings, v: String| s.trash_retention_policy = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Confirm Before Delete"
                get=Callback::new(|s: AppSettings| s.confirm_before_delete)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.confirm_before_delete = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Show Hidden Files"
                get=Callback::new(|s: AppSettings| s.show_hidden_files)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.show_hidden_files = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Sort Files Alphabetically"
                get=Callback::new(|s: AppSettings| s.sort_files_alphabetically)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.sort_files_alphabetically = v)
            />
        </div>
    }
}

#[component]
fn ImportExportSettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Import & Export"</h2>
        <div class="space-y-4">
            <Select
                label="Default Export Format"
                options=vec![
                    SelectOption::new("Markdown", "markdown"),
                    SelectOption::new("HTML", "html"),
                    SelectOption::new("PDF", "pdf"),
                    SelectOption::new("Plain Text", "text"),
                    SelectOption::new("JSON", "json"),
                ]
                value=derive_string_field(settings, |s: &AppSettings| s.default_export_format.clone())
                on_change=set_string_field(settings, save, |s: &mut AppSettings, v: String| s.default_export_format = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Include Metadata in Export"
                get=Callback::new(|s: AppSettings| s.export_include_metadata)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.export_include_metadata = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Include Attachments in Export"
                get=Callback::new(|s: AppSettings| s.export_include_attachments)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.export_include_attachments = v)
            />
            <Select
                label="Import Duplicate Strategy"
                options=vec![
                    SelectOption::new("Skip", "skip"),
                    SelectOption::new("Overwrite", "overwrite"),
                    SelectOption::new("Rename", "rename"),
                ]
                value=derive_string_field(settings, |s: &AppSettings| s.import_duplicate_strategy.clone())
                on_change=set_string_field(settings, save, |s: &mut AppSettings, v: String| s.import_duplicate_strategy = v)
            />
            <div class="mt-6 pt-4 border-t border-gray-700">
                <h3 class="text-lg font-semibold mb-2">"Settings Migration"</h3>
                <p class="text-sm text-gray-400 mb-4">"Export or import your settings between devices."</p>
                <div class="flex gap-2">
                    <Button>"Export Settings"</Button>
                    <Button>"Import Settings"</Button>
                </div>
            </div>
        </div>
    }
}

#[component]
fn OCRSettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"OCR Settings"</h2>
        <div class="space-y-4">
            <Select
                label="OCR Language"
                options=vec![
                    SelectOption::new("English", "eng"),
                    SelectOption::new("Spanish", "spa"),
                    SelectOption::new("French", "fra"),
                    SelectOption::new("German", "deu"),
                    SelectOption::new("Japanese", "jpn"),
                ]
                value=derive_string_field(settings, |s: &AppSettings| s.ocr_language.clone())
                on_change=set_string_field(settings, save, |s: &mut AppSettings, v: String| s.ocr_language = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Auto-process Scanned PDFs"
                get=Callback::new(|s: AppSettings| s.ocr_auto_process_scanned_pdfs)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.ocr_auto_process_scanned_pdfs = v)
            />
            <label class="field">
                <span class="field-label">"Confidence Threshold"</span>
                <input
                    type="range"
                    class="input w-full"
                    min="0.0"
                    max="1.0"
                    step="0.05"
                    prop:value=move || settings.get().ocr_confidence_threshold.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.ocr_confidence_threshold = event_target_value(&ev).parse().unwrap_or(0.7);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
        </div>
    }
}

#[component]
fn AccessibilitySettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Accessibility"</h2>
        <div class="space-y-4">
            <SettingCheckbox
                settings=settings
                save=save
                label="Screen Reader Support"
                get=Callback::new(|s: AppSettings| s.screen_reader_support)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.screen_reader_support = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Enhanced Keyboard Navigation"
                get=Callback::new(|s: AppSettings| s.keyboard_navigation)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.keyboard_navigation = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Visible Focus Ring"
                get=Callback::new(|s: AppSettings| s.focus_ring_visible)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.focus_ring_visible = v)
            />
        </div>
    }
}

#[component]
fn PerformanceSettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Performance"</h2>
        <div class="space-y-4">
            <label class="field">
                <span class="field-label">"Max Undo History"</span>
                <input
                    type="number"
                    class="input"
                    min="10"
                    max="500"
                    prop:value=move || settings.get().max_undo_history.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.max_undo_history = event_target_value(&ev).parse().unwrap_or(100);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
            <label class="field">
                <span class="field-label">"Worker Pool Size"</span>
                <input
                    type="number"
                    class="input"
                    min="1"
                    max="16"
                    prop:value=move || settings.get().worker_pool_size.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.worker_pool_size = event_target_value(&ev).parse().unwrap_or(4);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
            <SettingCheckbox
                settings=settings
                save=save
                label="Index on Startup"
                get=Callback::new(|s: AppSettings| s.index_on_startup)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.index_on_startup = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Background Processing"
                get=Callback::new(|s: AppSettings| s.background_processing)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.background_processing = v)
            />
        </div>
    }
}

#[component]
fn PrivacySettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Privacy"</h2>
        <div class="space-y-4">
            <SettingCheckbox
                settings=settings
                save=save
                label="Launch at Startup"
                get=Callback::new(|s: AppSettings| s.launch_at_startup)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.launch_at_startup = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Analytics Enabled"
                get=Callback::new(|s: AppSettings| s.analytics_enabled)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.analytics_enabled = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Crash Reporting"
                get=Callback::new(|s: AppSettings| s.crash_reporting_enabled)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.crash_reporting_enabled = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Auto-lock on Idle"
                get=Callback::new(|s: AppSettings| s.auto_lock_on_idle)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.auto_lock_on_idle = v)
            />
            <label class="field">
                <span class="field-label">"Auto-lock Timeout (minutes)"</span>
                <input
                    type="number"
                    class="input"
                    min="1"
                    max="120"
                    prop:value=move || settings.get().auto_lock_timeout_mins.to_string()
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.auto_lock_timeout_mins = event_target_value(&ev).parse().unwrap_or(15);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
        </div>
    }
}

#[component]
fn KeyboardShortcutsSettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Keyboard Shortcuts"</h2>
        <div class="space-y-4">
            <label class="field">
                <span class="field-label">"Voice Dictation"</span>
                <input
                    type="text"
                    class="input"
                    prop:value=derive_string_field(settings, |s: &AppSettings| s.voice_hotkey.clone())
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.voice_hotkey = event_target_value(&ev);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
            <label class="field">
                <span class="field-label">"Quick Capture"</span>
                <input
                    type="text"
                    class="input"
                    prop:value=derive_string_field(settings, |s: &AppSettings| s.quick_capture_hotkey.clone())
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.quick_capture_hotkey = event_target_value(&ev);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
            <label class="field">
                <span class="field-label">"Toggle Sidebar"</span>
                <input
                    type="text"
                    class="input"
                    prop:value=derive_string_field(settings, |s: &AppSettings| s.toggle_sidebar_hotkey.clone())
                    on:change=move |ev| {
                        let mut s = settings.get();
                        s.toggle_sidebar_hotkey = event_target_value(&ev);
                        settings.set(s.clone());
                        save.run(s);
                    }
                />
            </label>
        </div>
    }
}

#[component]
fn AdvancedSettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Advanced"</h2>
        <div class="space-y-4">
            <SettingCheckbox
                settings=settings
                save=save
                label="Force Sandbox for Web Snippets"
                get=Callback::new(|s: AppSettings| s.force_sandbox_for_web_snippets)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.force_sandbox_for_web_snippets = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Debug Mode"
                get=Callback::new(|s: AppSettings| s.debug_mode)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.debug_mode = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Developer Tools"
                get=Callback::new(|s: AppSettings| s.developer_tools)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.developer_tools = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Experimental Features"
                get=Callback::new(|s: AppSettings| s.experimental_features)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.experimental_features = v)
            />
            <div class="mt-6 pt-4 border-t border-gray-700">
                <Button>"Reset to Defaults"</Button>
            </div>
        </div>
    }
}

#[component]
fn ExperimentalSettings(
    settings: RwSignal<AppSettings>,
    save: Callback<AppSettings, ()>,
) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Experimental"</h2>
        <div class="space-y-4">
            <Select
                label="Whisper Model"
                options=vec![
                    SelectOption::new("ggml-tiny.en.bin", "ggml-tiny.en.bin"),
                    SelectOption::new("ggml-base.en.bin", "ggml-base.en.bin"),
                    SelectOption::new("ggml-small.en-q5_0.bin", "ggml-small.en-q5_0.bin"),
                ]
                value=derive_string_field(settings, |s: &AppSettings| s.whisper_model.clone())
                on_change=set_string_field(settings, save, |s: &mut AppSettings, v: String| s.whisper_model = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Enable AI Summarization"
                get=Callback::new(|s: AppSettings| s.enable_ai_summarization)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.enable_ai_summarization = v)
            />
            <SettingCheckbox
                settings=settings
                save=save
                label="Enable Semantic Search"
                get=Callback::new(|s: AppSettings| s.enable_semantic_search)
                set=Callback::new(|(mut s, v): (AppSettings, bool)| s.enable_semantic_search = v)
            />
        </div>
    }
}

#[component]
fn AboutSettings() -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"About Nabu"</h2>
        <div class="space-y-4">
            <div>
                <p class="text-lg font-semibold">"Nabu"</p>
                <p class="text-sm text-gray-400">"Version 0.1.0"</p>
                <p class="text-sm text-gray-400">"Premium Markdown Knowledge Management"</p>
            </div>
            <div class="pt-4 border-t border-gray-700">
                <p class="text-sm text-gray-400">"© 2024 Faro Labs"</p>
                <p class="text-sm text-gray-400">"Licensed under AGPL-3.0"</p>
            </div>
        </div>
    }
}
