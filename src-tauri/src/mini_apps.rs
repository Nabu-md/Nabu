use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use tauri::http::{header, Method, Request, Response, StatusCode};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use crate::search;

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
    #[serde(default)]
    pub cron_jobs: Vec<MiniAppCronJob>,
}

/// A scheduled task declared in a mini-app manifest. Only meaningful for apps
/// with `allow_vault_access: true`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct MiniAppCronJob {
    /// Five-field cron expression in the app's local timezone, e.g. "0 8 * * 1-5".
    pub schedule: String,
    /// Task identifier passed to the mini-app when the schedule fires.
    pub task: String,
    /// Optional note path the task writes to.
    #[serde(default)]
    pub target_note: Option<String>,
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

/// A registered mini-app cron job resolved to its owning app and vault.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct MiniAppCronRegistration {
    pub app_id: String,
    pub app_name: String,
    pub vault_path: String,
    pub schedule: String,
    pub task: String,
    pub target_note: Option<String>,
    pub allow_vault_access: bool,
}

/// Lists every cron job declared by installed mini-apps across registered
/// vaults (plus bundled sample apps). Jobs for apps without
/// `allow_vault_access` are skipped: scheduled vault access is gated on the
/// same permission as interactive vault access.
#[tauri::command]
pub fn list_mini_app_cron_jobs(app_handle: tauri::AppHandle) -> Result<Vec<MiniAppCronRegistration>, String> {
    let mut registrations = Vec::new();
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
            if !app.allow_vault_access || app.cron_jobs.is_empty() {
                continue;
            }
            let vault_path = app_dir
                .parent()
                .and_then(Path::parent)
                .map(|vault_root| vault_root.to_string_lossy().into_owned())
                .unwrap_or_default();
            for job in &app.cron_jobs {
                registrations.push(MiniAppCronRegistration {
                    app_id: app.id.clone(),
                    app_name: app.name.clone(),
                    vault_path: vault_path.clone(),
                    schedule: job.schedule.clone(),
                    task: job.task.clone(),
                    target_note: job.target_note.clone(),
                    allow_vault_access: app.allow_vault_access,
                });
            }
        }
    }
    Ok(registrations)
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

// ── Vault MCP relay (allow_vault_access) ────────────────────────────────────

/// MCP tools a mini-app may call when `allow_vault_access: true`.
/// Vault-lifecycle tools (list_vaults, attach_vault, clone_vault) stay
/// reserved for AI agents and are never exposed to mini-apps.
const MINI_APP_ALLOWED_MCP_TOOLS: [&str; 7] = [
    "search_notes",
    "get_note",
    "create_note",
    "update_note",
    "append_to_note",
    "open_note",
    "refresh_vault",
];

fn is_mini_app_allowed_tool(tool: &str) -> bool {
    MINI_APP_ALLOWED_MCP_TOOLS.contains(&tool)
}

fn note_path_for_tool(args: &serde_json::Value) -> Option<String> {
    args.get("path")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// Resolve a note path for a mini-app tool call. Relative paths are anchored
/// to the mini-app's vault; absolute paths must stay inside that vault. The
/// target file may not exist yet (create_note), so the nearest existing
/// ancestor is canonicalized and the remaining segments rejoined.
fn resolve_relay_note_path(vault_path: &Path, args: &serde_json::Value) -> Option<PathBuf> {
    let raw_path = note_path_for_tool(args)?;
    let requested = PathBuf::from(&raw_path);
    let joined = if requested.is_absolute() {
        requested
    } else {
        vault_path.join(requested)
    };
    let canonical_vault = vault_path.canonicalize().ok()?;
    let (canonical_ancestor, tail) = canonical_ancestor_of(&joined)?;
    if !canonical_ancestor.starts_with(&canonical_vault) {
        return None;
    }
    Some(tail.into_iter().fold(canonical_ancestor, |current, segment| current.join(segment)))
}

fn canonical_ancestor_of(path: &Path) -> Option<(PathBuf, Vec<std::ffi::OsString>)> {
    let mut current = path;
    let mut tail = Vec::new();
    loop {
        if current.exists() {
            let canonical = current.canonicalize().ok()?;
            tail.reverse();
            return Some((canonical, tail));
        }
        tail.push(current.file_name()?.to_os_string());
        current = current.parent()?;
    }
}

/// Execute one MCP tool call on behalf of a mini-app with
/// `allow_vault_access: true`. All communication stays inside the app's vault
/// boundary; lifecycle tools are rejected.
#[tauri::command]
pub fn mcp_tool_call(
    tool: String,
    args: serde_json::Value,
    vault_path: String,
) -> Result<serde_json::Value, String> {
    if !is_mini_app_allowed_tool(&tool) {
        return Err(format!("Tool '{tool}' is not available to mini-apps"));
    }

    let vault_path = PathBuf::from(vault_path.trim());
    if vault_path.as_os_str().is_empty() || !vault_path.is_dir() {
        return Err("A valid vault path is required".into());
    }
    let vault_root = vault_path
        .canonicalize()
        .map_err(|error| format!("Vault path is unavailable: {error}"))?;

    match tool.as_str() {
        "search_notes" => {
            let query = args
                .get("query")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            let limit = args
                .get("limit")
                .and_then(serde_json::Value::as_u64)
                .map(|limit| limit.min(50) as usize)
                .unwrap_or(10);
            let response = search::search_vault(
                vault_root.to_string_lossy().as_ref(),
                &query,
                "keyword",
                limit,
            )?;
            Ok(serde_json::to_value(response.results).unwrap_or_else(|_| serde_json::json!([])))
        }
        "get_note" => {
            let note_path = resolve_relay_note_path(&vault_root, &args)
                .ok_or("Invalid note path")?;
            let content = std::fs::read_to_string(&note_path)
                .map_err(|error| format!("Failed to read note: {error}"))?;
            Ok(serde_json::json!({
                "path": note_path_for_tool(&args).unwrap_or_default(),
                "content": content,
            }))
        }
        "create_note" | "update_note" | "append_to_note" => {
            let content = args
                .get("content")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string();
            let note_path = resolve_relay_note_path(&vault_root, &args)
                .ok_or("Invalid note path")?;
            if !note_path.extension().is_some_and(|ext| ext == "md") {
                return Err("Mini-app notes must be markdown files ending in .md".into());
            }
            match tool.as_str() {
                "create_note" => {
                    if note_path.exists() {
                        return Err("Note already exists".into());
                    }
                    if let Some(parent) = note_path.parent() {
                        std::fs::create_dir_all(parent)
                            .map_err(|error| format!("Failed to create note folder: {error}"))?;
                    }
                    std::fs::write(&note_path, &content)
                        .map_err(|error| format!("Failed to create note: {error}"))?;
                }
                "update_note" => {
                    if !note_path.is_file() {
                        return Err("Note does not exist; use create_note".into());
                    }
                    std::fs::write(&note_path, &content)
                        .map_err(|error| format!("Failed to update note: {error}"))?;
                }
                "append_to_note" => {
                    use std::io::Write;
                    if !note_path.is_file() {
                        return Err("Note does not exist; use create_note".into());
                    }
                    let mut file = std::fs::OpenOptions::new()
                        .append(true)
                        .open(&note_path)
                        .map_err(|error| format!("Failed to open note: {error}"))?;
                    file.write_all(content.as_bytes())
                        .map_err(|error| format!("Failed to append to note: {error}"))?;
                }
                _ => unreachable!("tool was validated by the allowlist"),
            }
            Ok(serde_json::json!({ "path": note_path_for_tool(&args).unwrap_or_default(), "ok": true }))
        }
        "open_note" | "refresh_vault" => {
            // UI signals only: the mini-app shell surfaces them via Tauri
            // events so the main window can react without direct access.
            Ok(serde_json::json!({ "ok": true }))
        }
        _ => Err(format!("Tool '{tool}' is not available to mini-apps")),
    }
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
    fn relay_allowlist_blocks_lifecycle_tools_and_allows_vault_tools() {
        for tool in MINI_APP_ALLOWED_MCP_TOOLS {
            assert!(is_mini_app_allowed_tool(tool));
        }
        for tool in ["list_vaults", "attach_vault", "clone_vault", "shell", "exec"] {
            assert!(!is_mini_app_allowed_tool(tool));
        }
    }

    #[test]
    fn mcp_tool_call_rejects_disallowed_tools() {
        let dir = tempfile::TempDir::new().unwrap();
        let error = mcp_tool_call(
            "attach_vault".into(),
            serde_json::json!({}),
            dir.path().to_string_lossy().into_owned(),
        )
        .unwrap_err();
        assert!(error.contains("not available to mini-apps"));
    }

    #[test]
    fn mcp_tool_call_rejects_missing_vault() {
        let error = mcp_tool_call(
            "search_notes".into(),
            serde_json::json!({ "query": "x" }),
            "/nonexistent/vault/path".into(),
        )
        .unwrap_err();
        assert!(error.contains("vault path is required"));
    }

    #[test]
    fn mcp_tool_call_searches_inside_the_vault() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("alpha.md"), "# Alpha\n\nneedle").unwrap();

        let results = mcp_tool_call(
            "search_notes".into(),
            serde_json::json!({ "query": "needle" }),
            dir.path().to_string_lossy().into_owned(),
        )
        .unwrap();

        assert_eq!(results.as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn mcp_tool_call_note_roundtrip_stays_inside_vault() {
        let dir = tempfile::TempDir::new().unwrap();
        let vault = dir.path().to_string_lossy().into_owned();

        mcp_tool_call(
            "create_note".into(),
            serde_json::json!({ "path": "data/contacts.md", "content": "# Contacts\n" }),
            vault.clone(),
        )
        .unwrap();
        assert!(dir.path().join("data/contacts.md").is_file());

        let read = mcp_tool_call(
            "get_note".into(),
            serde_json::json!({ "path": "data/contacts.md" }),
            vault.clone(),
        )
        .unwrap();
        assert_eq!(read["content"], "# Contacts\n");

        mcp_tool_call(
            "append_to_note".into(),
            serde_json::json!({ "path": "data/contacts.md", "content": "Row\n" }),
            vault.clone(),
        )
        .unwrap();
        let updated = mcp_tool_call(
            "get_note".into(),
            serde_json::json!({ "path": "data/contacts.md" }),
            vault.clone(),
        )
        .unwrap();
        assert_eq!(updated["content"], "# Contacts\nRow\n");

        mcp_tool_call(
            "update_note".into(),
            serde_json::json!({ "path": "data/contacts.md", "content": "# Replaced\n" }),
            vault,
        )
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("data/contacts.md")).unwrap(),
            "# Replaced\n"
        );
    }

    #[test]
    fn mcp_tool_call_rejects_paths_outside_the_vault() {
        let vault = tempfile::TempDir::new().unwrap();
        let outside = tempfile::TempDir::new().unwrap();
        let outside_note = outside.path().join("outside.md");
        std::fs::write(&outside_note, "# Outside\n").unwrap();

        let relative_escape = mcp_tool_call(
            "get_note".into(),
            serde_json::json!({ "path": "../outside.md" }),
            vault.path().to_string_lossy().into_owned(),
        )
        .unwrap_err();
        assert!(relative_escape.contains("Invalid note path"));

        let absolute_escape = mcp_tool_call(
            "get_note".into(),
            serde_json::json!({ "path": outside_note.to_string_lossy() }),
            vault.path().to_string_lossy().into_owned(),
        )
        .unwrap_err();
        assert!(absolute_escape.contains("Invalid note path"));
    }

    #[test]
    fn mcp_tool_call_rejects_non_markdown_writes() {
        let dir = tempfile::TempDir::new().unwrap();
        let error = mcp_tool_call(
            "create_note".into(),
            serde_json::json!({ "path": "payload.txt", "content": "x" }),
            dir.path().to_string_lossy().into_owned(),
        )
        .unwrap_err();
        assert!(error.contains("markdown"));
    }

    #[test]
    fn mcp_tool_call_create_note_rejects_existing_note() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("exists.md"), "# Exists\n").unwrap();
        let error = mcp_tool_call(
            "create_note".into(),
            serde_json::json!({ "path": "exists.md", "content": "# New\n" }),
            dir.path().to_string_lossy().into_owned(),
        )
        .unwrap_err();
        assert!(error.contains("already exists"));
    }

    #[test]
    fn url_encode_encodes_reserved_characters() {
        assert_eq!(url_encode("hello world/ü"), "hello%20world%2F%C3%BC");
        assert_eq!(url_encode("plain-id"), "plain-id");
    }
}