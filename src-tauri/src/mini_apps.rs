use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use tauri::http::{header, Method, Request, Response, StatusCode};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

const MINI_APPS_DIR: &str = ".apps";
const BUNDLED_MINI_APPS_DIR: &str = "mini-apps";
const MANIFEST_FILE: &str = "manifest.json";
const DEFAULT_ENTRYPOINT: &str = "index.html";
const MAX_APP_FILE_BYTES: u64 = 8 * 1024 * 1024;
const MINI_APP_WINDOW_LABEL_PREFIX: &str = "miniapp-";

/// Registered vault roots whose `.apps` directories may contain mini-apps.
/// The protocol handler and IPC commands use this to resolve app directories
/// without re-deriving the active vault.
pub(crate) struct MiniAppRoots(pub(crate) Mutex<Vec<PathBuf>>);

impl Default for MiniAppRoots {
    fn default() -> Self {
        Self(Mutex::new(Vec::new()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MiniApp {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default = "default_entrypoint")]
    pub entrypoint_url: String,
    #[serde(default = "default_width")]
    pub width: f64,
    #[serde(default = "default_height")]
    pub height: f64,
    #[serde(default = "default_resizable")]
    pub resizable: bool,
    #[serde(default)]
    pub allow_vault_access: bool,
}

/// Context packaged with a mini-app launch. Passed to the new window via URL
/// params and made available to the app as `window.miniAppContext`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MiniAppContext {
    #[serde(default)]
    pub note_path: Option<String>,
    #[serde(default)]
    pub note_title: Option<String>,
    #[serde(default)]
    pub vault_path: Option<String>,
    #[serde(default)]
    pub extra: Option<serde_json::Value>,
}

fn default_entrypoint() -> String {
    DEFAULT_ENTRYPOINT.to_string()
}

fn default_width() -> f64 {
    720.0
}

fn default_height() -> f64 {
    560.0
}

fn default_resizable() -> bool {
    true
}

// ── Registry ────────────────────────────────────────────────────────────────

pub(crate) fn vault_apps_dir(vault_path: &Path) -> PathBuf {
    vault_path.join(MINI_APPS_DIR)
}

pub(crate) fn register_vault_root(app_handle: &tauri::AppHandle, vault_path: &Path) -> Result<(), String> {
    let apps_dir = vault_apps_dir(vault_path);
    let state: tauri::State<'_, MiniAppRoots> = app_handle.state();
    let mut roots = state
        .0
        .lock()
        .map_err(|_| "Failed to lock mini-app registry".to_string())?;
    if !roots.contains(&apps_dir) {
        roots.push(apps_dir);
    }
    Ok(())
}

fn bundled_mini_apps_dir(app_handle: &tauri::AppHandle) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(resource_path) = app_handle
        .path()
        .resolve(BUNDLED_MINI_APPS_DIR, tauri::path::BaseDirectory::Resource)
    {
        candidates.push(resource_path);
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join(BUNDLED_MINI_APPS_DIR),
    );
    candidates.into_iter().find(|path| path.is_dir())
}

fn registered_vault_apps_dirs(app_handle: &tauri::AppHandle) -> Vec<PathBuf> {
    let state: tauri::State<'_, MiniAppRoots> = app_handle.state();
    state
        .0
        .lock()
        .map(|roots| roots.clone())
        .unwrap_or_default()
}

/// Candidate root directories that may contain `{id}/manifest.json` apps.
/// Vault `.apps` directories take precedence over the bundled sample apps.
fn app_root_dirs(app_handle: &tauri::AppHandle) -> Vec<PathBuf> {
    let mut roots = registered_vault_apps_dirs(app_handle);
    if let Some(bundled) = bundled_mini_apps_dir(app_handle) {
        roots.push(bundled);
    }
    roots
}

fn read_manifest(app_dir: &Path) -> Option<MiniApp> {
    let manifest_path = app_dir.join(MANIFEST_FILE);
    let manifest = std::fs::read_to_string(manifest_path).ok()?;
    let mut app: MiniApp = serde_json::from_str(&manifest).ok()?;
    app.id = app_dir
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(&app.id)
        .to_string();
    if app.entrypoint_url.trim().is_empty() {
        app.entrypoint_url = default_entrypoint();
    }
    Some(app)
}

fn app_dir_for_id(app_handle: &tauri::AppHandle, id: &str) -> Option<PathBuf> {
    if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains("..") {
        return None;
    }
    app_root_dirs(app_handle)
        .into_iter()
        .map(|root| root.join(id))
        .find(|app_dir| app_dir.is_dir() && (app_dir.join(MANIFEST_FILE).is_file() || app_dir.join(DEFAULT_ENTRYPOINT).is_file()))
}

// ── IPC commands ────────────────────────────────────────────────────────────

/// Lists installed mini-apps: bundled sample apps plus apps installed in the
/// active vault's `.apps/` directory.
#[tauri::command]
pub fn list_mini_apps(
    app_handle: tauri::AppHandle,
    vault_path: Option<String>,
) -> Result<Vec<MiniApp>, String> {
    if let Some(vault_path) = vault_path.as_deref().filter(|path| !path.trim().is_empty()) {
        register_vault_root(&app_handle, Path::new(vault_path))?;
    }

    let mut by_id: HashMap<String, MiniApp> = HashMap::new();
    for root in app_root_dirs(&app_handle) {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let app_dir = entry.path();
            if !app_dir.is_dir() {
                continue;
            }
            let Some(app) = read_manifest(&app_dir) else {
                continue;
            };
            // Vault-installed apps override bundled apps with the same id.
            by_id.insert(app.id.clone(), app);
        }
    }

    let mut apps: Vec<MiniApp> = by_id.into_values().collect();
    apps.sort_by(|left, right| left.name.to_lowercase().cmp(&right.name.to_lowercase()));
    Ok(apps)
}

/// Returns the configuration for a specific mini-app.
#[tauri::command]
pub fn get_mini_app_config(
    app_handle: tauri::AppHandle,
    id: String,
    vault_path: Option<String>,
) -> Result<MiniApp, String> {
    if let Some(vault_path) = vault_path.as_deref().filter(|path| !path.trim().is_empty()) {
        register_vault_root(&app_handle, Path::new(vault_path))?;
    }
    let app_dir = app_dir_for_id(&app_handle, &id)
        .ok_or_else(|| format!("Mini-app '{id}' is not installed"))?;
    read_manifest(&app_dir).ok_or_else(|| format!("Mini-app '{id}' has an invalid manifest"))
}

/// Spawns a new `WebviewWindow` for the mini-app, passing context via URL
/// params. Returns the window label on success.
#[tauri::command]
pub fn open_mini_app_window(
    app_handle: tauri::AppHandle,
    id: String,
    context: Option<MiniAppContext>,
) -> Result<String, String> {
    let context = context.unwrap_or_default();
    let Some(vault_path) = context.vault_path.as_deref().filter(|path| !path.trim().is_empty()) else {
        return Err("A vault path is required to open a mini-app window".into());
    };
    register_vault_root(&app_handle, Path::new(vault_path))?;

    let config = get_mini_app_config(app_handle.clone(), id.clone(), Some(vault_path.to_string()))?;
    let label = format!("{MINI_APP_WINDOW_LABEL_PREFIX}{id}");

    let mut params = Vec::new();
    params.push(format!("window=mini-app"));
    params.push(format!("appId={}", url_encode(&id)));
    params.push(format!("vault={}", url_encode(vault_path)));
    if let Some(note_path) = context.note_path.as_deref() {
        params.push(format!("note={}", url_encode(note_path)));
    }
    if let Some(note_title) = context.note_title.as_deref() {
        params.push(format!("title={}", url_encode(note_title)));
    }
    if let Some(extra) = context.extra.as_ref() {
        params.push(format!("context={}", url_encode(&serde_json::to_string(extra).unwrap_or_else(|_| "null".into()))));
    }
    let route = format!("/?{}", params.join("&"));

    let window = WebviewWindowBuilder::new(&app_handle, &label, WebviewUrl::App(route.into()))
        .title(config.name.clone())
        .inner_size(config.width, config.height)
        .resizable(config.resizable)
        .min_inner_size(320.0, 240.0)
        .build()
        .map_err(|error| format!("Failed to open mini-app window: {error}"))?;
    let _ = window.set_focus();

    Ok(label)
}

/// Development helper: writes (or updates) a mini-app manifest inside the
/// active vault's `.apps/{id}/` directory, creating a placeholder entrypoint
/// if the app directory does not exist yet.
#[tauri::command]
pub fn save_mini_app_config(
    app_handle: tauri::AppHandle,
    config: MiniApp,
) -> Result<(), String> {
    if config.id.is_empty() || config.id.contains('/') || config.id.contains('\\') || config.id.contains("..") {
        return Err("Invalid mini-app id".into());
    }
    let roots = registered_vault_apps_dirs(&app_handle);
    let root = roots
        .first()
        .ok_or_else(|| "No vault is open; cannot save a mini-app".to_string())?;
    let app_dir = root.join(&config.id);
    std::fs::create_dir_all(&app_dir)
        .map_err(|error| format!("Failed to create mini-app directory: {error}"))?;
    if !app_dir.join(DEFAULT_ENTRYPOINT).exists() {
        std::fs::write(
            app_dir.join(DEFAULT_ENTRYPOINT),
            "<!doctype html><html><body><h1>Mini-app placeholder</h1></body></html>",
        )
        .map_err(|error| format!("Failed to create mini-app entrypoint: {error}"))?;
    }
    let manifest = serde_json::to_string_pretty(&config)
        .map_err(|error| format!("Failed to serialize mini-app manifest: {error}"))?;
    std::fs::write(app_dir.join(MANIFEST_FILE), manifest)
        .map_err(|error| format!("Failed to save mini-app manifest: {error}"))?;
    Ok(())
}

/// Opens the devtools window for a mini-app window (debug builds).
#[tauri::command]
pub fn open_mini_app_devtools(app_handle: tauri::AppHandle, label: String) -> Result<(), String> {
    let Some(window) = app_handle.get_webview_window(&label) else {
        return Err(format!("Mini-app window '{label}' not found"));
    };
    window.open_devtools();
    Ok(())
}

/// Removes a mini-app from every registered vault (bundled sample apps are
/// read-only and cannot be deleted).
#[tauri::command]
pub fn delete_mini_app(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    if id.is_empty() || id.contains('/') || id.contains('\\') || id.contains("..") {
        return Err("Invalid mini-app id".into());
    }
    let roots = registered_vault_apps_dirs(&app_handle);
    if roots.is_empty() {
        return Err("No vault is open; cannot delete a mini-app".into());
    }
    let mut deleted = false;
    for root in &roots {
        let app_dir = root.join(&id);
        if app_dir.is_dir() {
            std::fs::remove_dir_all(&app_dir)
                .map_err(|error| format!("Failed to delete mini-app: {error}"))?;
            deleted = true;
        }
    }
    if !deleted {
        return Err(format!("Mini-app '{id}' is not installed in any vault"));
    }
    Ok(())
}

// ── URI protocol handler ────────────────────────────────────────────────────

const MINI_APP_CSP: &str = "default-src 'none'; script-src 'unsafe-inline'; connect-src 'none'; worker-src 'none'; frame-src 'none'; form-action 'none'; base-uri 'none'; img-src data: blob:; media-src data: blob:; font-src data:; style-src 'unsafe-inline'";

fn content_type_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "txt" => "text/plain; charset=utf-8",
        "wasm" => "application/wasm",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

fn safe_app_relative_path(path: &str) -> Option<PathBuf> {
    let trimmed = path.trim_start_matches('/');
    let relative = Path::new(trimmed);
    let mut result = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => result.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    if result.as_os_str().is_empty() {
        return None;
    }
    Some(result)
}

fn mini_app_response(status: StatusCode, content_type: &'static str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::CONTENT_SECURITY_POLICY, MINI_APP_CSP)
        .header(header::REFERRER_POLICY, "no-referrer")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(body)
        .expect("static mini-app protocol response headers must be valid")
}

pub(crate) fn handle_request(
    context: tauri::UriSchemeContext<'_, tauri::Wry>,
    request: Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    if request.method() != Method::GET {
        return mini_app_response(
            StatusCode::METHOD_NOT_ALLOWED,
            "text/plain; charset=utf-8",
            b"Method not allowed".to_vec(),
        );
    }

    let raw_path = request.uri().path();
    let Some(relative) = safe_app_relative_path(raw_path) else {
        return mini_app_response(
            StatusCode::BAD_REQUEST,
            "text/plain; charset=utf-8",
            b"Invalid mini-app request".to_vec(),
        );
    };

    let Some(id) = relative.components().next().and_then(|component| match component {
        Component::Normal(part) => part.to_str(),
        _ => None,
    }) else {
        return mini_app_response(
            StatusCode::BAD_REQUEST,
            "text/plain; charset=utf-8",
            b"Invalid mini-app request".to_vec(),
        );
    };

    let Some(app_dir) = app_dir_for_id(context.app_handle(), id) else {
        return mini_app_response(
            StatusCode::NOT_FOUND,
            "text/plain; charset=utf-8",
            b"Mini-app not found".to_vec(),
        );
    };

    let entrypoint = read_manifest(&app_dir)
        .map(|app| app.entrypoint_url)
        .unwrap_or_else(|| DEFAULT_ENTRYPOINT.to_string());
    let requested = if relative.components().count() == 1 {
        PathBuf::from(&entrypoint)
    } else {
        let mut rest = PathBuf::new();
        for component in relative.components().skip(1) {
            match component {
                Component::Normal(part) => rest.push(part),
                _ => return mini_app_response(
                    StatusCode::BAD_REQUEST,
                    "text/plain; charset=utf-8",
                    b"Invalid mini-app request".to_vec(),
                ),
            }
        }
        rest
    };

    let file_path = app_dir.join(&requested);
    let canonical_app_dir = match std::fs::canonicalize(&app_dir) {
        Ok(path) => path,
        Err(_) => {
            return mini_app_response(
                StatusCode::NOT_FOUND,
                "text/plain; charset=utf-8",
                b"Mini-app not found".to_vec(),
            );
        }
    };
    let canonical_file = match std::fs::canonicalize(&file_path) {
        Ok(path) => path,
        Err(_) => {
            return mini_app_response(
                StatusCode::NOT_FOUND,
                "text/plain; charset=utf-8",
                b"Mini-app file not found".to_vec(),
            );
        }
    };
    if !canonical_file.starts_with(&canonical_app_dir) {
        return mini_app_response(
            StatusCode::FORBIDDEN,
            "text/plain; charset=utf-8",
            b"Forbidden".to_vec(),
        );
    }
    let Ok(metadata) = std::fs::metadata(&canonical_file) else {
        return mini_app_response(
            StatusCode::NOT_FOUND,
            "text/plain; charset=utf-8",
            b"Mini-app file not found".to_vec(),
        );
    };
    if !metadata.is_file() || metadata.len() > MAX_APP_FILE_BYTES {
        return mini_app_response(
            StatusCode::NOT_FOUND,
            "text/plain; charset=utf-8",
            b"Mini-app file not found".to_vec(),
        );
    }
    let body = match std::fs::read(&canonical_file) {
        Ok(body) => body,
        Err(_) => {
            return mini_app_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "text/plain; charset=utf-8",
                b"Failed to read mini-app file".to_vec(),
            );
        }
    };
    mini_app_response(StatusCode::OK, content_type_for_path(&canonical_file), body)
}

fn url_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest(id: &str, name: &str) -> String {
        format!(
            r#"{{"id":"{id}","name":"{name}","entrypoint_url":"index.html","width":640,"height":480,"resizable":true,"allow_vault_access":false}}"#
        )
    }

    #[test]
    fn safe_app_relative_path_rejects_traversal() {
        assert_eq!(
            safe_app_relative_path("/hello-world/index.html"),
            Some(PathBuf::from("hello-world/index.html"))
        );
        assert_eq!(safe_app_relative_path("/hello-world/../secret"), None);
        assert_eq!(safe_app_relative_path("/hello-world/.."), None);
        assert_eq!(safe_app_relative_path("/"), None);
        assert_eq!(safe_app_relative_path("/hello-world/./app.js"), Some(PathBuf::from("hello-world/app.js")));
    }

    #[test]
    fn read_manifest_defaults_missing_fields() {
        let dir = tempfile::TempDir::new().unwrap();
        let app_dir = dir.path().join("demo");
        std::fs::create_dir(&app_dir).unwrap();
        std::fs::write(app_dir.join("manifest.json"), sample_manifest("demo", "Demo App")).unwrap();

        let app = read_manifest(&app_dir).unwrap();
        // The id is derived from the app directory name.
        assert_eq!(app.id, "demo");
        assert_eq!(app.name, "Demo App");
        assert_eq!(app.entrypoint_url, "index.html");
        assert_eq!(app.width, 640.0);
        assert_eq!(app.height, 480.0);
        assert!(app.resizable);
        assert!(!app.allow_vault_access);
    }

    #[test]
    fn read_manifest_skips_invalid_json() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("manifest.json"), "not json").unwrap();
        assert!(read_manifest(dir.path()).is_none());
    }

    #[test]
    fn vault_apps_dir_joins_hidden_directory() {
        assert_eq!(
            vault_apps_dir(Path::new("/tmp/vault")),
            PathBuf::from("/tmp/vault/.apps")
        );
    }

    #[test]
    fn url_encode_encodes_reserved_characters() {
        assert_eq!(url_encode("hello world/ü"), "hello%20world%2F%C3%BC");
        assert_eq!(url_encode("plain-id"), "plain-id");
    }
}