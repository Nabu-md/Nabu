use wasm_bindgen_futures::spawn_local;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};

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
        let result = crate::ipc::tauri_invoke("settings_get_all", serde_wasm_bindgen::to_value(&serde_json::json!({})).unwrap()).await;
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
            <div class="tabs w-1/4 border-r border-gray-700 bg-gray-900 p-4">
                {tabs.iter().map(|tab_str| {
                    let tab = tab_str.to_string();
                    let tab_for_class = tab.clone();
                    let tab_for_click = tab.clone();
                    view! {
                        <button
                            class=move || format!("block w-full text-left p-2 {}", if active_tab.get() == tab_for_class { "bg-gray-800 text-white" } else { "text-gray-400" })
                            on:click=move |_| set_active_tab.set(tab_for_click.clone())
                        >
                            {tab}
                        </button>
                    }
                }).collect_view()}
            </div>
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

#[component]
fn GeneralSettings(settings: RwSignal<AppSettings>, save: Callback<AppSettings, ()>) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"General & Modules"</h2>
        <div class="space-y-4">
            <div><label>"Vault Location: " {move || settings.get().last_vault_path}</label></div>
            <button class="bg-gray-700 p-2 rounded">"Change Vault..."</button>
            <label class="block"><input type="checkbox" checked=move || settings.get().enable_daily_notes on:change=move |ev| {
                let mut s = settings.get();
                s.enable_daily_notes = event_target_checked(&ev);
                settings.set(s.clone());
                save.run(s);
            }/> " Enable Daily Notes"</label>
            <label class="block"><input type="checkbox" checked=move || settings.get().launch_at_startup on:change=move |ev| {
                let mut s = settings.get();
                s.launch_at_startup = event_target_checked(&ev);
                settings.set(s.clone());
                save.run(s);
            }/> " Launch at Startup"</label>
        </div>
    }
}

#[component]
fn EditorSettings(settings: RwSignal<AppSettings>, save: Callback<AppSettings, ()>) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Editor & Notion Block Menu"</h2>
        <div class="space-y-4">
            <label class="block">"Editing Mode: "
            <select class="bg-gray-800 p-1" prop:value=move || settings.get().editor_mode on:change=move |ev| {
                let mut s = settings.get();
                s.editor_mode = event_target_value(&ev);
                settings.set(s.clone());
                save.run(s);
            }>
                <option>"Live Preview"</option>
                <option>"Source Markdown"</option>
            </select>
            </label>
            <label class="block"><input type="checkbox" checked=move || settings.get().auto_pair_brackets on:change=move |ev| {
                let mut s = settings.get();
                s.auto_pair_brackets = event_target_checked(&ev);
                settings.set(s.clone());
                save.run(s);
            }/> " Auto-pair brackets"</label>
            <label class="block"><input type="checkbox" checked=move || settings.get().show_line_numbers on:change=move |ev| {
                let mut s = settings.get();
                s.show_line_numbers = event_target_checked(&ev);
                settings.set(s.clone());
                save.run(s);
            }/> " Show line numbers"</label>
            <label class="block"><input type="checkbox" checked=move || settings.get().convert_pasted_html_to_markdown on:change=move |ev| {
                let mut s = settings.get();
                s.convert_pasted_html_to_markdown = event_target_checked(&ev);
                settings.set(s.clone());
                save.run(s);
            }/> " Convert pasted HTML to Markdown"</label>
            <label class="block"><input type="checkbox" checked=move || settings.get().enable_notion_slash_menu on:change=move |ev| {
                let mut s = settings.get();
                s.enable_notion_slash_menu = event_target_checked(&ev);
                settings.set(s.clone());
                save.run(s);
            }/> " Enable Notion Slash Menu"</label>
        </div>
    }
}

#[component]
fn WhisprSettings(settings: RwSignal<AppSettings>, save: Callback<AppSettings, ()>) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Whispr AI & Voice Dictation"</h2>
        <div class="space-y-4">
            <label class="block">"Model: "
            <select class="bg-gray-800 p-1" prop:value=move || settings.get().whisper_model on:change=move |ev| {
                let mut s = settings.get();
                s.whisper_model = event_target_value(&ev);
                settings.set(s.clone());
                save.run(s);
            }>
                <option>"ggml-tiny.en.bin"</option>
                <option>"ggml-base.en.bin"</option>
                <option>"ggml-small.en-q5_0.bin"</option>
            </select>
            </label>
            <label class="block">"Voice Hotkey: "
            <input type="text" class="bg-gray-800 p-1" prop:value=move || settings.get().voice_hotkey on:change=move |ev| {
                let mut s = settings.get();
                s.voice_hotkey = event_target_value(&ev);
                settings.set(s.clone());
                save.run(s);
            }/>
            </label>
            <label class="block"><input type="checkbox" checked=move || settings.get().auto_format_filler_words on:change=move |ev| {
                let mut s = settings.get();
                s.auto_format_filler_words = event_target_checked(&ev);
                settings.set(s.clone());
                save.run(s);
            }/> " Auto-format filler words"</label>
        </div>
    }
}

#[component]
fn AppearanceSettings(settings: RwSignal<AppSettings>, save: Callback<AppSettings, ()>) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Appearance & Opacity Controls"</h2>
        <div class="space-y-4">
            <label class="block">"Theme: "
            <select class="bg-gray-800 p-1" prop:value=move || settings.get().theme on:change=move |ev| {
                let mut s = settings.get();
                s.theme = event_target_value(&ev);
                settings.set(s.clone());
                save.run(s);
            }>
                <option>"Dark"</option>
                <option>"Light"</option>
                <option>"System Sync"</option>
            </select>
            </label>
            <label class="block">"Main Window Opacity: "
            <input type="range" class="w-full" min="0.2" max="1.0" step="0.05" prop:value=move || settings.get().main_window_opacity on:change=move |ev| {
                let mut s = settings.get();
                s.main_window_opacity = event_target_value(&ev).parse().unwrap_or(1.0);
                settings.set(s.clone());
                save.run(s);
            }/>
            </label>
            <label class="block">"Floating Pill Opacity: "
            <input type="range" class="w-full" min="0.2" max="1.0" step="0.05" prop:value=move || settings.get().floating_pill_opacity on:change=move |ev| {
                let mut s = settings.get();
                s.floating_pill_opacity = event_target_value(&ev).parse().unwrap_or(0.8);
                settings.set(s.clone());
                save.run(s);
            }/>
            </label>
            <label class="block"><input type="checkbox" checked=move || settings.get().pill_hover_boost_opacity on:change=move |ev| {
                let mut s = settings.get();
                s.pill_hover_boost_opacity = event_target_checked(&ev);
                settings.set(s.clone());
                save.run(s);
            }/> " Pill Hover Focus"</label>
        </div>
    }
}

#[component]
fn FileSettings(settings: RwSignal<AppSettings>, save: Callback<AppSettings, ()>) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Files, Trash & Sandboxing"</h2>
        <div class="space-y-4">
            <label class="block">"Default New Note Path: "
            <select class="bg-gray-800 p-1" prop:value=move || settings.get().default_new_note_path on:change=move |ev| {
                let mut s = settings.get();
                s.default_new_note_path = event_target_value(&ev);
                settings.set(s.clone());
                save.run(s);
            }>
                <option>"Vault Root"</option>
                <option>"Same Folder as Active Note"</option>
                <option>"Custom Subfolder"</option>
            </select>
            </label>
            <label class="block">"Trash Retention: "
            <select class="bg-gray-800 p-1" prop:value=move || settings.get().trash_retention_policy on:change=move |ev| {
                let mut s = settings.get();
                s.trash_retention_policy = event_target_value(&ev);
                settings.set(s.clone());
                save.run(s);
            }>
                <option>"Move to System Trash"</option>
                <option>"Move to .trash Vault Folder"</option>
                <option>"Permanently Delete"</option>
            </select>
            </label>
            <label class="block"><input type="checkbox" checked=move || settings.get().force_sandbox_for_web_snippets on:change=move |ev| {
                let mut s = settings.get();
                s.force_sandbox_for_web_snippets = event_target_checked(&ev);
                settings.set(s.clone());
                save.run(s);
            }/> " Sandbox Security (iframe)"</label>
        </div>
    }
}

#[component]
fn GraphSettings(settings: RwSignal<AppSettings>, save: Callback<AppSettings, ()>) -> impl IntoView {
    view! {
        <h2 class="text-xl font-bold mb-4">"Folder Graph & Canvas"</h2>
        <div class="space-y-4">
            <label class="block"><input type="checkbox" checked=move || settings.get().include_folders_in_graph on:change=move |ev| {
                let mut s = settings.get();
                s.include_folders_in_graph = event_target_checked(&ev);
                settings.set(s.clone());
                save.run(s);
            }/> " Include Folders as Hub Nodes"</label>
            <label class="block">"Folder Click Behavior: "
            <select class="bg-gray-800 p-1" prop:value=move || settings.get().folder_click_behavior on:change=move |ev| {
                let mut s = settings.get();
                s.folder_click_behavior = event_target_value(&ev);
                settings.set(s.clone());
                save.run(s);
            }>
                <option>"Open Folder Table View"</option>
                <option>"Browse Folder"</option>
            </select>
            </label>
            <label class="block">"Gravity Strength: "
            <input type="range" class="w-full" min="0" max="1" step="0.1" prop:value=move || settings.get().graph_node_physics_gravity on:change=move |ev| {
                let mut s = settings.get();
                s.graph_node_physics_gravity = event_target_value(&ev).parse().unwrap_or(0.5);
                settings.set(s.clone());
                save.run(s);
            }/>
            </label>
            <label class="block">"Node Spacing: "
            <input type="range" class="w-full" min="0" max="1" step="0.1" prop:value=move || settings.get().graph_node_physics_spacing on:change=move |ev| {
                let mut s = settings.get();
                s.graph_node_physics_spacing = event_target_value(&ev).parse().unwrap_or(1.0);
                settings.set(s.clone());
                save.run(s);
            }/>
            </label>
        </div>
    }
}
