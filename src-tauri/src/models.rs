use serde::{Deserialize, Serialize};

// Vault commands
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VaultGetCurrentPayload;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarksGetPayload {
    pub vault_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarksGetResult {
    pub bookmarks: std::collections::HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarksAddPayload {
    pub vault_path: String,
    pub list_name: String,
    pub file_path: String,
}

pub type BookmarksAddResult = BookmarksGetResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarksRemovePayload {
    pub vault_path: String,
    pub list_name: String,
    pub file_path: String,
}

pub type BookmarksRemoveResult = BookmarksGetResult;

// Widget commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetTogglePayload {
    pub start: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetMovePayload {
    pub dx: f64,
    pub dy: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetResizePayload {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetCreateNotePayload {
    pub name: String,
    pub content: String,
    pub timestamp: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetFetchTitlePayload {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WidgetSetShortcutPayload {
    pub shortcut: String,
}
