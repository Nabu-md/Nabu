use crate::components::ui::button::Button;
use crate::components::ui::nav::SidebarItem;
use crate::components::ui::selection::{Checkbox, Select, SelectOption};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AppSettings {
    pub theme: String,
    pub last_vault_path: String,
    #[serde(default)]
    pub recent_vaults: Vec<serde_json::Value>,
    #[serde(default)]
    pub main_window_opacity: f32,
    #[serde(default)]
    pub floating_pill_opacity: f32,
    #[serde(default)]
    pub whisper_model: String,
    #[serde(default)]
    pub enable_daily_notes: bool,
    #[serde(default)]
    pub launch_at_startup: bool,
    #[serde(default)]
    pub editor_mode: String,
    #[serde(default)]
    pub auto_pair_brackets: bool,
    #[serde(default)]
    pub show_line_numbers: bool,
    #[serde(default)]
    pub convert_pasted_html_to_markdown: bool,
    #[serde(default)]
    pub enable_notion_slash_menu: bool,
    #[serde(default)]
    pub voice_hotkey: String,
    #[serde(default)]
    pub auto_format_filler_words: bool,
    #[serde(default)]
    pub pill_hover_boost_opacity: bool,
    #[serde(default)]
    pub default_new_note_path: String,
    #[serde(default)]
    pub trash_retention_policy: String,
    #[serde(default)]
    pub force_sandbox_for_web_snippets: bool,
    #[serde(default)]
    pub include_folders_in_graph: bool,
    #[serde(default)]
    pub folder_click_behavior: String,
    #[serde(default)]
    pub graph_node_physics_gravity: f32,
    #[serde(default)]
    pub graph_node_physics_spacing: f32,
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

    let (active_tab, set_active_tab) = signal("General & Modules".to_string());
    let tabs = vec![
        "General & Modules",
        "Editor & Notion Block Menu",
        "Whispr AI & Voice Dictation",
        "Appearance & Opacity Controls",
        "Files, Trash & Sandboxing",
        "Folder Graph & Canvas",
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
                    "General & Modules" => view! { <GeneralSettings settings=settings save=save_settings /> }.into_any(),
                    "Editor & Notion Block Menu" => view! { <EditorSettings settings=settings save=save_settings /> }.into_any(),
                    "Whispr AI & Voice Dictation" => view! { <WhisprSettings settings=settings save=save_settings /> }.into_any(),
                    "Appearance & Opacity Controls" => view! { <AppearanceSettings settings=settings save=save_settings /> }.into_any(),
                    "Files, Trash & Sandboxing" => view! { <FileSettings settings=settings save=save_settings /> }.into_any(),
                    "Folder Graph & Canvas" => view! { <GraphSettings settings=settings save=save_settings /> }.into_any(),
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
