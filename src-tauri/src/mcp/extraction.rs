use std::fs;
#[cfg(all(desktop, target_os = "linux"))]
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
#[cfg(all(desktop, target_os = "linux"))]
use std::time::{Duration, Instant, SystemTime};

const VERSION_MARKER_FILE: &str = ".nabu-version";
#[cfg(all(desktop, target_os = "linux"))]
const LOCK_FILE: &str = "mcp-server.lock";
const STAGING_DIR: &str = "mcp-server.staging";
const BACKUP_DIR: &str = "mcp-server.previous";
#[cfg(all(desktop, target_os = "linux"))]
const LOCK_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(all(desktop, target_os = "linux"))]
const STALE_LOCK_AFTER: Duration = Duration::from_secs(120);

#[cfg(all(desktop, target_os = "linux"))]
pub(super) fn ready_stable_mcp_server_dir() -> Option<PathBuf> {
    let stable_dir = stable_mcp_server_dir().ok()?;
    stable_mcp_server_dir_is_ready(&stable_dir).then_some(stable_dir)
}

#[cfg(all(desktop, target_os = "linux"))]
pub(crate) fn extract_mcp_server_to_stable_dir(app_version: &str) -> Result<PathBuf, String> {
    let source_dir = super::mcp_server_dir()?;
    let target_dir = stable_mcp_server_dir()?;

    if !needs_extraction(app_version, &target_dir) {
        return Ok(target_dir);
    }

    let _lock = ExtractionLock::acquire(&extraction_lock_path()?)?;
    if !needs_extraction(app_version, &target_dir) {
        return Ok(target_dir);
    }

    replace_stable_server_dir(&source_dir, &target_dir, app_version)?;
    Ok(target_dir)
}

fn stable_mcp_server_dir() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|data_dir| data_dir.join("nabu").join("mcp-server"))
        .ok_or_else(|| "Unable to resolve data directory for stable MCP server path".to_string())
}

fn stable_mcp_server_dir_is_ready(dir: &Path) -> bool {
    mcp_server_dir_has_files(dir) && read_version_marker(dir).is_some()
}

fn mcp_server_dir_has_files(dir: &Path) -> bool {
    dir.join("index.js").is_file() && dir.join("ws-bridge.js").is_file()
}

fn needs_extraction(app_version: &str, target_dir: &Path) -> bool {
    !mcp_server_dir_has_files(target_dir)
        || read_version_marker(target_dir).as_deref() != Some(app_version)
}

fn read_version_marker(dir: &Path) -> Option<String> {
    fs::read_to_string(dir.join(VERSION_MARKER_FILE))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn write_version_marker(dir: &Path, app_version: &str) -> Result<(), String> {
    let marker = dir.join(VERSION_MARKER_FILE);
    fs::write(&marker, app_version)
        .map_err(|e| format!("Failed to write version marker {}: {e}", marker.display()))
}

#[cfg(all(desktop, target_os = "linux"))]
fn extraction_lock_path() -> Result<PathBuf, String> {
    let stable_dir = stable_mcp_server_dir()?;
    stable_dir
        .parent()
        .map(|parent| parent.join(LOCK_FILE))
        .ok_or_else(|| {
            format!(
                "Stable MCP server path has no parent: {}",
                stable_dir.display()
            )
        })
}

fn replace_stable_server_dir(
    source_dir: &Path,
    target_dir: &Path,
    app_version: &str,
) -> Result<(), String> {
    let parent = target_dir.parent().ok_or_else(|| {
        format!(
            "Stable MCP server path has no parent: {}",
            target_dir.display()
        )
    })?;
    let staging_dir = parent.join(STAGING_DIR);
    let backup_dir = parent.join(BACKUP_DIR);

    remove_dir_if_exists(&staging_dir)?;
    remove_dir_if_exists(&backup_dir)?;
    copy_dir_all(source_dir, &staging_dir)?;
    write_version_marker(&staging_dir, app_version)?;
    swap_staging_into_place(&staging_dir, target_dir, &backup_dir)?;
    Ok(())
}

fn swap_staging_into_place(
    staging_dir: &Path,
    target_dir: &Path,
    backup_dir: &Path,
) -> Result<(), String> {
    if target_dir.exists() {
        fs::rename(target_dir, backup_dir)
            .map_err(|e| format!("Failed to move stable MCP server aside: {e}"))?;
    }

    if let Err(error) = fs::rename(staging_dir, target_dir) {
        if backup_dir.exists() {
            let _ = fs::rename(backup_dir, target_dir);
        }
        return Err(format!("Failed to activate stable MCP server: {error}"));
    }

    remove_dir_if_exists(backup_dir)
}

fn copy_dir_all(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target)
        .map_err(|e| format!("Failed to create {}: {e}", target.display()))?;

    for entry in fs::read_dir(source).map_err(|e| {
        format!(
            "Failed to read MCP server directory {}: {e}",
            source.display()
        )
    })? {
        let entry = entry.map_err(|e| format!("Failed to read MCP server entry: {e}"))?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else {
            fs::copy(&source_path, &target_path).map_err(|e| {
                format!(
                    "Failed to copy {} to {}: {e}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }

    Ok(())
}

fn remove_dir_if_exists(path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_dir_all(path)
            .map_err(|e| format!("Failed to remove {}: {e}", path.display()))?;
    }
    Ok(())
}

#[cfg(all(desktop, target_os = "linux"))]
struct ExtractionLock {
    path: PathBuf,
}

#[cfg(all(desktop, target_os = "linux"))]
impl ExtractionLock {
    fn acquire(path: &Path) -> Result<Self, String> {
        let started = Instant::now();
        loop {
            match Self::try_create(path) {
                Ok(()) => {
                    return Ok(Self {
                        path: path.to_path_buf(),
                    });
                }
                Err(error) if lock_is_stale(path) => {
                    let _ = fs::remove_file(path);
                    log::warn!("Removed stale MCP extraction lock after error: {error}");
                }
                Err(error) if started.elapsed() >= LOCK_TIMEOUT => return Err(error),
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        }
    }

    fn try_create(path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create MCP extraction lock dir: {e}"))?;
        }

        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| format!("Failed to acquire MCP extraction lock: {e}"))?;
        use std::io::Write;
        writeln!(file, "{}", std::process::id())
            .map_err(|e| format!("Failed to write MCP extraction lock: {e}"))
    }
}

#[cfg(all(desktop, target_os = "linux"))]
impl Drop for ExtractionLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(all(desktop, target_os = "linux"))]
fn lock_is_stale(path: &Path) -> bool {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_some_and(|age| age >= STALE_LOCK_AFTER)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_server_dir(parent: &Path) -> PathBuf {
        let dir = parent.join("server");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("index.js"), "console.log('index');").unwrap();
        fs::write(dir.join("ws-bridge.js"), "console.log('bridge');").unwrap();
        dir
    }

    #[test]
    fn stable_mcp_server_dir_uses_app_data_dir() {
        let expected = dirs::data_dir()
            .expect("data dir should exist")
            .join("nabu")
            .join("mcp-server");

        assert_eq!(stable_mcp_server_dir().unwrap(), expected);
    }

    #[test]
    fn stable_mcp_server_dir_requires_marker_and_files() {
        let tmp = tempfile::tempdir().unwrap();
        let stable_dir = tmp.path().join("nabu").join("mcp-server");

        fs::create_dir_all(&stable_dir).unwrap();
        fs::write(stable_dir.join("index.js"), "").unwrap();
        fs::write(stable_dir.join("ws-bridge.js"), "").unwrap();
        assert!(!stable_mcp_server_dir_is_ready(&stable_dir));

        write_version_marker(&stable_dir, "2026.5.14").unwrap();
        assert!(stable_mcp_server_dir_is_ready(&stable_dir));
    }

    #[test]
    fn needs_extraction_tracks_version_marker() {
        let target = tempfile::tempdir().unwrap();
        fs::write(target.path().join("index.js"), "").unwrap();
        fs::write(target.path().join("ws-bridge.js"), "").unwrap();

        assert!(needs_extraction("2026.5.14", target.path()));
        write_version_marker(target.path(), "2026.5.14").unwrap();
        assert!(!needs_extraction("2026.5.14", target.path()));
        assert!(needs_extraction("2026.5.15", target.path()));
    }

    #[test]
    fn copy_dir_all_copies_nested_server_files() {
        let tmp = tempfile::tempdir().unwrap();
        let source = create_server_dir(tmp.path());
        fs::create_dir_all(source.join("nested")).unwrap();
        fs::write(source.join("nested").join("package.json"), "{}").unwrap();
        let target = tmp.path().join("target");

        copy_dir_all(&source, &target).unwrap();

        assert!(target.join("index.js").is_file());
        assert!(target.join("ws-bridge.js").is_file());
        assert!(target.join("nested").join("package.json").is_file());
    }

    #[test]
    fn replace_stable_server_dir_swaps_versioned_copy() {
        let tmp = tempfile::tempdir().unwrap();
        let source = create_server_dir(tmp.path());
        let target = tmp.path().join("nabu").join("mcp-server");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("stale.txt"), "old").unwrap();

        replace_stable_server_dir(&source, &target, "2026.5.14").unwrap();

        assert!(target.join("index.js").is_file());
        assert!(!target.join("stale.txt").exists());
        assert_eq!(read_version_marker(&target), Some("2026.5.14".to_string()));
    }
}
