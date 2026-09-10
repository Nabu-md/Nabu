use crate::mini_apps_sql::{
    discover_apps, run_form_insert, run_view, MiniAppMeta, MiniAppRow, MiniAppSql,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// Guards against `..` traversal and absolute paths in vault-relative notes.
fn confine_to_vault(vault_path: &str, note_path: &str) -> Result<PathBuf, String> {
    let vault = PathBuf::from(vault_path.trim());
    if vault.as_os_str().is_empty() || !vault.is_dir() {
        return Err("A valid vault path is required".to_string());
    }
    if note_path.trim().is_empty() {
        return Err("A note path is required".to_string());
    }
    let relative = Path::new(note_path);
    if relative.is_absolute() {
        return Err(format!("Note path '{note_path}' must be vault-relative"));
    }
    if relative
        .components()
        .any(|component| component == Component::ParentDir || component == Component::RootDir)
    {
        return Err(format!("Note path '{note_path}' escapes the vault"));
    }
    Ok(vault.join(relative))
}

/// Extracts frontmatter key-value pairs from a note using the same
/// gray-matter parsing the rest of the backend uses, and returns the body.
fn split_frontmatter_meta(content: &str) -> (HashMap<String, Value>, String) {
    let parsed = gray_matter::Matter::<gray_matter::engine::YAML>::new().parse(content);
    let mut map = HashMap::new();
    if let Some(data) = parsed.data {
        // gray_matter's `Pod` converts into `serde_json::Value` via `Into`.
        let value: Value = data.into();
        if let Value::Object(entries) = value {
            for (key, value) in entries {
                map.insert(key, value);
            }
        }
    }
    (map, parsed.content)
}

fn read_mini_app_note(vault_path: &str, note_path: &str) -> Result<MiniAppSql, String> {
    let full_path = confine_to_vault(vault_path, note_path)?;
    let content = std::fs::read_to_string(&full_path)
        .map_err(|error| format!("Failed to read mini-app note: {error}"))?;
    let (frontmatter, body) = split_frontmatter_meta(&content);
    MiniAppSql::from_markdown(&frontmatter, &body, note_path)
}

/// Scans vault markdown notes for mini-app definitions (`app_id` frontmatter).
/// Bounded to a reasonable number of notes to keep vault scans snappy.
const MAX_DISCOVERY_NOTES: usize = 5_000;
const MAX_NOTE_BYTES: u64 = 2 * 1024 * 1024;

#[tauri::command]
pub fn discover_mini_apps(vault_path: String) -> Result<Vec<MiniAppMeta>, String> {
    let vault = PathBuf::from(vault_path.trim());
    if vault.as_os_str().is_empty() || !vault.is_dir() {
        return Err("A valid vault path is required".to_string());
    }

    let mut candidates: Vec<(String, String, String)> = Vec::new();
    let walker = walkdir::WalkDir::new(&vault)
        .max_depth(8)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
        });

    for entry in walker.take(MAX_DISCOVERY_NOTES) {
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        if metadata.len() > MAX_NOTE_BYTES {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        if !content.contains("app_id") {
            continue;
        }
        let (frontmatter, _) = split_frontmatter_meta(&content);
        if !frontmatter.contains_key("app_id") {
            continue;
        }
        let relative = entry
            .path()
            .strip_prefix(&vault)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_default();
        candidates.push((relative, serde_json::to_string(&frontmatter).unwrap_or_default(), content));
    }

    discover_apps(candidates)
}

#[tauri::command]
pub fn get_mini_app(vault_path: String, note_path: String) -> Result<MiniAppSql, String> {
    read_mini_app_note(&vault_path, &note_path)
}

#[tauri::command]
pub fn run_mini_app_view(
    vault_path: String,
    note_path: String,
    view_name: String,
    params: Option<HashMap<String, String>>,
) -> Result<Vec<MiniAppRow>, String> {
    let app = read_mini_app_note(&vault_path, &note_path)?;
    run_view(Path::new(vault_path.trim()), &app, &view_name, &params.unwrap_or_default())
}

#[tauri::command]
pub fn execute_mini_app_mut(
    vault_path: String,
    note_path: String,
    view_name: String,
    values: HashMap<String, String>,
) -> Result<(), String> {
    let app = read_mini_app_note(&vault_path, &note_path)?;
    run_form_insert(Path::new(vault_path.trim()), &app, &view_name, &values)
}

/// Markdown table rendering used by the export command.
fn markdown_table(rows: &[MiniAppRow]) -> String {
    let mut columns: Vec<String> = Vec::new();
    for row in rows {
        for column in row.keys() {
            if !columns.iter().any(|existing| existing == column) {
                columns.push(column.clone());
            }
        }
    }
    if columns.is_empty() {
        return "_No rows._\n".to_string();
    }

    let escape = |value: &Value| -> String {
        match value {
            Value::Null => String::new(),
            Value::String(text) => text.replace('|', "\\|").replace('\n', "<br>"),
            other => other.to_string(),
        }
    };

    let mut table = String::from("| ");
    table.push_str(&columns.join(" | "));
    table.push_str(" |\n|");
    for _ in &columns {
        table.push_str(" --- |");
    }
    table.push('\n');
    for row in rows {
        table.push_str("| ");
        let cells: Vec<String> = columns
            .iter()
            .map(|column| row.get(column).map(escape).unwrap_or_default())
            .collect();
        table.push_str(&cells.join(" | "));
        table.push_str(" |\n");
    }
    table
}

#[tauri::command]
pub fn export_mini_app_to_markdown(
    vault_path: String,
    note_path: String,
    view_name: String,
) -> Result<String, String> {
    let app = read_mini_app_note(&vault_path, &note_path)?;
    let rows = run_view(
        Path::new(vault_path.trim()),
        &app,
        &view_name,
        &HashMap::new(),
    )?;
    Ok(markdown_table(&rows))
}

/// Import request: column values for one record, submitted to a form view.
#[tauri::command]
pub fn import_mini_app_markdown_cmd(
    vault_path: String,
    note_path: String,
    view_name: String,
    rows: Vec<HashMap<String, String>>,
) -> Result<u32, String> {
    let app = read_mini_app_note(&vault_path, &note_path)?;
    let mut imported = 0u32;
    for row in rows {
        run_form_insert(Path::new(vault_path.trim()), &app, &view_name, &row)?;
        imported += 1;
    }
    Ok(imported)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_note(vault: &Path, relative: &str, content: &str) {
        let path = vault.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    const CRM_NOTE: &str = "---\ntitle: Customer Tracker\napp_id: crm\n---\n\n## Database Schema\n\nCREATE TABLE IF NOT EXISTS customers (\n  id INTEGER PRIMARY KEY,\n  name TEXT NOT NULL,\n  status TEXT DEFAULT 'lead'\n);\n\n## Views\n\n### All Customers\n\ntype: table\nquery: SELECT id, name, status FROM customers ORDER BY name\n\n### Add Customer\n\ntype: form\nfields:\n  - name: Name, column: name, type: text, required: true\nquery: INSERT INTO customers (name) VALUES (name)\n";

    #[test]
    fn get_mini_app_reads_vault_note() {
        let vault = tempfile::TempDir::new().unwrap();
        write_note(vault.path(), "notes/crm.md", CRM_NOTE);

        let app = get_mini_app(
            vault.path().to_string_lossy().into_owned(),
            "notes/crm.md".to_string(),
        )
        .unwrap();
        assert_eq!(app.app_id, "crm");
        assert_eq!(app.views.len(), 2);
    }

    #[test]
    fn get_mini_app_rejects_escape_paths() {
        let vault = tempfile::TempDir::new().unwrap();
        let error = get_mini_app(
            vault.path().to_string_lossy().into_owned(),
            "../outside.md".to_string(),
        )
        .unwrap_err();
        assert!(error.contains("escapes the vault"), "unexpected: {error}");
    }

    #[test]
    fn run_mini_app_view_executes_sql() {
        let vault = tempfile::TempDir::new().unwrap();
        write_note(vault.path(), "notes/crm.md", CRM_NOTE);
        let vault_str = vault.path().to_string_lossy().into_owned();

        execute_mini_app_mut(
            vault_str.clone(),
            "notes/crm.md".to_string(),
            "Add Customer".to_string(),
            HashMap::from([("name".to_string(), "ACME".to_string())]),
        )
        .unwrap();

        let rows = run_mini_app_view(
            vault_str,
            "notes/crm.md".to_string(),
            "All Customers".to_string(),
            None,
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["name"], Value::String("ACME".to_string()));
    }

    #[test]
    fn export_produces_markdown_table() {
        let vault = tempfile::TempDir::new().unwrap();
        write_note(vault.path(), "notes/crm.md", CRM_NOTE);
        let vault_str = vault.path().to_string_lossy().into_owned();

        execute_mini_app_mut(
            vault_str.clone(),
            "notes/crm.md".to_string(),
            "Add Customer".to_string(),
            HashMap::from([("name".to_string(), "Be | Co".to_string())]),
        )
        .unwrap();

        let markdown = export_mini_app_to_markdown(vault_str, "notes/crm.md".to_string(), "All Customers".to_string())
            .unwrap();
        assert!(markdown.starts_with("| id | name | status |"));
        assert!(markdown.contains("Be \\| Co"));
    }

    #[test]
    fn import_inserts_each_row() {
        let vault = tempfile::TempDir::new().unwrap();
        write_note(vault.path(), "notes/crm.md", CRM_NOTE);
        let vault_str = vault.path().to_string_lossy().into_owned();

        let imported = import_mini_app_markdown_cmd(
            vault_str,
            "notes/crm.md".to_string(),
            "Add Customer".to_string(),
            vec![
                HashMap::from([("name".to_string(), "A".to_string())]),
                HashMap::from([("name".to_string(), "B".to_string())]),
            ],
        )
        .unwrap();
        assert_eq!(imported, 2);
    }
}
