#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppSettings {
    pub theme: String,
    pub last_vault_path: String,
    #[serde(default)]
    pub recent_vaults: Vec<RecentVaultEntry>,
    
    // Tab 1: General & Modules
    #[serde(default)]
    pub launch_at_startup: bool,
    #[serde(default)]
    pub enable_daily_notes: bool,

    // Tab 2: Editor & Notion Block Menu
    #[serde(default)]
    pub editor_mode: String, // "Live Preview" | "Source Markdown"
    #[serde(default)]
    pub auto_pair_brackets: bool,
    #[serde(default)]
    pub show_line_numbers: bool,
    #[serde(default)]
    pub convert_pasted_html_to_markdown: bool,
    #[serde(default)]
    pub enable_notion_slash_menu: bool,

    // Tab 3: Whispr AI & Voice Dictation
    #[serde(default)]
    pub whisper_model: String,
    #[serde(default)]
    pub voice_hotkey: String,
    #[serde(default)]
    pub auto_format_filler_words: bool,

    // Tab 4: Appearance & Opacity Controls
    #[serde(default)]
    pub main_window_opacity: f32,
    #[serde(default)]
    pub floating_pill_opacity: f32,
    #[serde(default)]
    pub pill_hover_boost_opacity: bool,

    // Tab 5: Files, Trash & Sandboxing
    #[serde(default)]
    pub default_new_note_path: String,
    #[serde(default)]
    pub trash_retention_policy: String,
    #[serde(default)]
    pub force_sandbox_for_web_snippets: bool,

    // Tab 6: Folder Graph & Canvas
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
