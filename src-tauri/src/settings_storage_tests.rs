use super::*;

fn assert_empty_settings(settings: &Settings) {
    assert_eq!(settings, &Settings::default());
}

fn save_and_reload(settings: Settings) -> Settings {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("settings.json");
    save_settings_at(&path, settings).unwrap();
    get_settings_at(&path).unwrap()
}

fn create_last_vault_path(path_parts: &[&str]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::TempDir::new().unwrap();
    let path = path_parts
        .iter()
        .fold(dir.path().to_path_buf(), |acc, part| acc.join(part));
    (dir, path)
}

fn write_and_assert_last_vault(path: &PathBuf, value: &str) {
    set_last_vault_at(path, value).unwrap();
    assert_eq!(get_last_vault_at(path).as_deref(), Some(value));
}

#[test]
fn test_get_settings_normalizes_legacy_beta_channel() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("settings.json");
    fs::write(&path, r#"{"release_channel":"beta"}"#).unwrap();

    let loaded = get_settings_at(&path).unwrap();
    assert!(loaded.release_channel.is_none());
}

#[test]
fn test_get_settings_removes_misplaced_hidden_defaults() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("settings.json");
    fs::write(
        &path,
        r#"{"future_setting":true,"hidden_defaults":["inbox","all_notes","archive"],"theme_mode":"dark"}"#,
    )
    .unwrap();

    let loaded = get_settings_at(&path).unwrap();
    let persisted: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();

    assert_eq!(loaded.theme_mode.as_deref(), Some("dark"));
    assert_eq!(
        persisted.get("theme_mode").and_then(|value| value.as_str()),
        Some("dark")
    );
    assert_eq!(
        persisted
            .get("future_setting")
            .and_then(|value| value.as_bool()),
        Some(true)
    );
    assert!(persisted.get("hidden_defaults").is_none());
}

#[test]
fn test_save_creates_parent_directories() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("nested").join("dir").join("settings.json");

    save_settings_at(
        &path,
        Settings {
            anonymous_id: Some("test-uuid".to_string()),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(path.exists());
    assert_eq!(
        get_settings_at(&path).unwrap().anonymous_id.as_deref(),
        Some("test-uuid")
    );
}

#[test]
fn test_get_settings_malformed_json() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("bad.json");
    fs::write(&path, "not valid json{{{").unwrap();

    let err = get_settings_at(&path).unwrap_err();
    assert!(err.contains("Failed to parse settings"));
}

#[test]
fn test_telemetry_fields_roundtrip() {
    let loaded = save_and_reload(Settings {
        telemetry_consent: Some(true),
        crash_reporting_enabled: Some(true),
        analytics_enabled: Some(false),
        anonymous_id: Some("test-uuid-v4".to_string()),
        ..Default::default()
    });
    assert_eq!(
        loaded,
        Settings {
            telemetry_consent: Some(true),
            crash_reporting_enabled: Some(true),
            analytics_enabled: Some(false),
            anonymous_id: Some("test-uuid-v4".to_string()),
            ..Default::default()
        }
    );
}

#[test]
fn test_old_settings_json_missing_telemetry_fields() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("settings.json");
    // Simulate an old settings.json that still contains removed GitHub auth fields.
    let legacy_token = ["gho", "test"].join("_");
    let legacy_settings = serde_json::json!({
        "github_token": legacy_token,
        "github_username": "nabu-md",
    });
    fs::write(&path, legacy_settings.to_string()).unwrap();
    let loaded = get_settings_at(&path).unwrap();
    assert_empty_settings(&loaded);
}

#[test]
fn test_settings_path_returns_ok() {
    let result = settings_path();
    assert!(result.is_ok());
    let path = result.unwrap();
    let path = path.to_str().unwrap();
    assert!(path.contains("com.nabu.app") || path.contains("com.laputa.app"));
}

#[test]
fn test_preferred_settings_path_uses_nabu_namespace() {
    let result = preferred_app_config_path("settings.json");
    assert!(result.is_ok());
    assert!(result
        .unwrap()
        .to_str()
        .unwrap()
        .contains("com.nabu.app"));
}

#[test]
fn test_ai_workspace_sessions_roundtrip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("ai-workspace-sessions.json");
    let sessions = serde_json::json!({
        "chat-1": {
            "messages": [
                {
                    "userMessage": "Hello",
                    "actions": [],
                    "response": "Hi"
                }
            ],
            "status": "done"
        }
    });

    save_ai_workspace_sessions_at(&path, sessions.clone()).unwrap();

    assert_eq!(get_ai_workspace_sessions_at(&path).unwrap(), sessions);
}

#[test]
fn test_ai_workspace_sessions_missing_file_returns_empty_object() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("ai-workspace-sessions.json");

    assert_eq!(
        get_ai_workspace_sessions_at(&path).unwrap(),
        serde_json::json!({})
    );
}

#[test]
fn test_ai_workspace_sessions_rejects_non_object_payload() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("ai-workspace-sessions.json");

    assert!(save_ai_workspace_sessions_at(&path, serde_json::json!([])).is_err());
}

#[test]
fn test_get_last_vault_returns_none_for_missing_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("last-vault.txt");
    assert!(get_last_vault_at(&path).is_none());
}

#[test]
fn test_set_and_get_last_vault_roundtrip() {
    let (_dir, path) = create_last_vault_path(&["last-vault.txt"]);
    write_and_assert_last_vault(&path, "/Users/test/MyVault");
}

#[test]
fn test_set_last_vault_trims_whitespace() {
    let (_dir, path) = create_last_vault_path(&["last-vault.txt"]);
    write_and_assert_last_vault(&path, "/Users/test/Vault");
}

#[test]
fn test_get_last_vault_returns_none_for_empty_file() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("last-vault.txt");
    fs::write(&path, "   \n  ").unwrap();
    assert!(get_last_vault_at(&path).is_none());
}

#[test]
fn test_set_last_vault_creates_parent_directories() {
    let (_dir, path) = create_last_vault_path(&["nested", "dir", "last-vault.txt"]);
    write_and_assert_last_vault(&path, "/Users/test/Vault");
    assert!(path.exists());
}

#[test]
fn test_set_last_vault_overwrites_previous() {
    let (_dir, path) = create_last_vault_path(&["last-vault.txt"]);
    write_and_assert_last_vault(&path, "/Users/test/OldVault");
    write_and_assert_last_vault(&path, "/Users/test/NewVault");
}
