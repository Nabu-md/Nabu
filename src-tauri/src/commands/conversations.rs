use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub messages: Vec<ConversationMessage>,
}

fn conversations_dir(vault_path: &str) -> PathBuf {
    let vault_path = crate::commands::expand_tilde(vault_path);
    std::path::Path::new(vault_path.as_ref()).join(".app-data").join("conversations")
}

fn sanitize_conversation_id(id: &str) -> String {
    id.chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' { ch } else { '_' })
        .collect()
}

fn conversation_path(vault_path: &str, id: &str) -> Result<PathBuf, String> {
    let safe_id = sanitize_conversation_id(id);
    if safe_id.is_empty() {
        return Err("Conversation id is required".into());
    }
    Ok(conversations_dir(vault_path).join(format!("{safe_id}.json")))
}

fn utc_now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// Lists saved chat threads for a vault, newest first.
#[tauri::command]
pub fn list_conversations(vault_path: String) -> Result<Vec<Conversation>, String> {
    let dir = conversations_dir(&vault_path);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut conversations = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|error| format!("Failed to read conversations directory: {error}"))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(_) => continue,
        };
        if let Ok(conversation) = serde_json::from_str::<Conversation>(&raw) {
            conversations.push(conversation);
        }
    }
    conversations.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(conversations)
}

/// Persists a chat thread for a vault.
#[tauri::command]
pub fn save_conversation(vault_path: String, conversation: Conversation) -> Result<(), String> {
    let path = conversation_path(&vault_path, &conversation.id)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| format!("Failed to create conversations directory: {error}"))?;
    }
    let serialized = serde_json::to_string_pretty(&conversation)
        .map_err(|error| format!("Failed to serialize conversation: {error}"))?;
    std::fs::write(&path, serialized).map_err(|error| format!("Failed to write conversation: {error}"))?;
    Ok(())
}

/// Creates an empty chat thread and returns its id.
#[tauri::command]
pub fn create_conversation(vault_path: String, title: Option<String>) -> Result<Conversation, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = utc_now_iso();
    let conversation = Conversation {
        id,
        title: title.unwrap_or_else(|| "New chat".into()),
        created_at: now.clone(),
        updated_at: now,
        messages: Vec::new(),
    };
    save_conversation(vault_path, conversation.clone())?;
    Ok(conversation)
}

/// Deletes a saved chat thread.
#[tauri::command]
pub fn delete_conversation(vault_path: String, id: String) -> Result<(), String> {
    let path = conversation_path(&vault_path, &id)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|error| format!("Failed to delete conversation: {error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_conversation_id_removes_path_traversal() {
        assert_eq!(sanitize_conversation_id("../../etc/passwd"), "______etc_passwd");
        assert_eq!(sanitize_conversation_id("abc-123_X"), "abc-123_X");
    }

    #[test]
    fn conversation_path_rejects_empty_id() {
        assert!(conversation_path("/tmp/vault", "").is_err());
    }
}
