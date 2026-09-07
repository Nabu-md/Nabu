use serde::Deserialize;
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;

const APP_CONFIG_POLICY_JSON: &str = include_str!("../../mcp-server/app-config-policy.json");
const APP_CONFIG_NAMESPACE_ENV: &str = "NABU_APP_CONFIG_NAMESPACE";

#[derive(Debug, Deserialize)]
struct AppConfigPolicy {
    current_namespace: String,
    development_namespace: String,
    legacy_namespace: String,
    namespace_read_order: Vec<AppConfigNamespace>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AppConfigNamespace {
    Current,
    Legacy,
}

impl AppConfigPolicy {
    fn current_namespace(&self) -> &str {
        self.current_namespace_for(std::env::var(APP_CONFIG_NAMESPACE_ENV).ok().as_deref())
    }

    fn current_namespace_for(&self, requested_namespace: Option<&str>) -> &str {
        if requested_namespace.map(str::trim) == Some(self.development_namespace.as_str()) {
            &self.development_namespace
        } else {
            &self.current_namespace
        }
    }

    fn namespace_read_order(&self) -> &[AppConfigNamespace] {
        &self.namespace_read_order
    }

    fn namespace_dir(&self, namespace: &AppConfigNamespace) -> &str {
        match namespace {
            AppConfigNamespace::Current => self.current_namespace(),
            AppConfigNamespace::Legacy => &self.legacy_namespace,
        }
    }
}

fn app_config_policy() -> &'static AppConfigPolicy {
    static POLICY: OnceLock<AppConfigPolicy> = OnceLock::new();
    POLICY.get_or_init(|| {
        serde_json::from_str(APP_CONFIG_POLICY_JSON)
            .expect("mcp-server/app-config-policy.json must be valid")
    })
}

fn app_config_dir() -> Result<PathBuf, String> {
    primary_config_dir().ok_or_else(|| "Could not determine config directory".to_string())
}

fn primary_config_dir() -> Option<PathBuf> {
    primary_config_dir_from_sources(
        explicit_xdg_config_home(),
        dirs::home_dir(),
        dirs::config_dir(),
    )
}

fn primary_config_dir_from_sources(
    explicit_xdg: Option<PathBuf>,
    home: Option<PathBuf>,
    platform: Option<PathBuf>,
) -> Option<PathBuf> {
    explicit_xdg
        .and_then(absolute_path)
        .or_else(|| default_xdg_config_home(home))
        .or(platform)
}

fn explicit_xdg_config_home() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("XDG_CONFIG_HOME")?))
}

fn default_xdg_config_home(home: Option<PathBuf>) -> Option<PathBuf> {
    if cfg!(windows) {
        None
    } else {
        home.and_then(absolute_path)
            .map(|path| path.join(".config"))
    }
}

fn absolute_path(path: PathBuf) -> Option<PathBuf> {
    if path.is_absolute() {
        Some(path)
    } else {
        None
    }
}

fn preferred_path_in(config_dir: &Path, file_name: &str) -> PathBuf {
    config_dir
        .join(app_config_policy().current_namespace())
        .join(file_name)
}

fn writable_path_in_dirs(
    config_dirs: &[PathBuf],
    file_name: &str,
    can_write: impl Fn(&Path) -> bool,
) -> PathBuf {
    config_dirs
        .iter()
        .map(|config_dir| preferred_path_in(config_dir, file_name))
        .find(|candidate| can_write(candidate))
        .unwrap_or_else(|| preferred_path_in(&config_dirs[0], file_name))
}

fn config_dirs_with_write_path_first(
    config_dirs: &[PathBuf],
    file_name: &str,
    write_path: &Path,
) -> Vec<PathBuf> {
    let mut ordered = config_dirs.to_vec();
    if let Some(index) = ordered
        .iter()
        .position(|config_dir| preferred_path_in(config_dir, file_name) == write_path)
    {
        let write_dir = ordered.remove(index);
        ordered.insert(0, write_dir);
    }
    ordered
}

fn app_config_path_is_writable(path: &Path) -> bool {
    if path.exists() {
        return OpenOptions::new().write(true).open(path).is_ok();
    }

    let Some(parent) = path.parent() else {
        return false;
    };
    if fs::create_dir_all(parent).is_err() {
        return false;
    }

    static PROBE_COUNTER: AtomicU64 = AtomicU64::new(0);
    let probe = parent.join(format!(
        ".nabu-write-probe-{}-{}",
        std::process::id(),
        PROBE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let Ok(probe_file) = OpenOptions::new().write(true).create_new(true).open(&probe) else {
        return false;
    };
    drop(probe_file);
    let _ = fs::remove_file(probe);
    true
}

fn existing_or_preferred_path_in_dirs(config_dirs: &[PathBuf], file_name: &str) -> PathBuf {
    let policy = app_config_policy();
    for config_dir in config_dirs {
        for namespace in policy.namespace_read_order() {
            let candidate = config_dir
                .join(policy.namespace_dir(namespace))
                .join(file_name);
            if candidate.exists() {
                return candidate;
            }
        }
    }

    preferred_path_in(&config_dirs[0], file_name)
}

fn app_config_read_dirs() -> Result<Vec<PathBuf>, String> {
    let primary = app_config_dir()?;
    let mut dirs = vec![primary.clone()];
    if let Some(platform) = dirs::config_dir() {
        if platform != primary {
            dirs.push(platform);
        }
    }
    Ok(dirs)
}

pub(crate) fn preferred_app_config_path(file_name: &str) -> Result<PathBuf, String> {
    let config_dirs = app_config_read_dirs()?;
    Ok(writable_path_in_dirs(
        &config_dirs,
        file_name,
        app_config_path_is_writable,
    ))
}

pub(crate) fn resolve_existing_or_preferred_app_config_path(
    file_name: &str,
) -> Result<PathBuf, String> {
    let config_dirs = app_config_read_dirs()?;
    let write_path = writable_path_in_dirs(&config_dirs, file_name, app_config_path_is_writable);
    let read_dirs = config_dirs_with_write_path_first(&config_dirs, file_name, &write_path);
    Ok(existing_or_preferred_path_in_dirs(&read_dirs, file_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn absolute_temp_dir(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(name)
    }

    #[test]
    fn absolute_xdg_config_home_is_accepted() {
        let path = absolute_temp_dir("nabu-xdg-config");
        assert_eq!(absolute_path(path.clone()), Some(path));
    }

    #[test]
    fn relative_xdg_config_home_is_ignored() {
        assert!(absolute_path(PathBuf::from("relative-config")).is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn default_unix_config_home_uses_home_dot_config() {
        let home = absolute_temp_dir("nabu-home");
        let platform = absolute_temp_dir("nabu-platform-config");

        assert_eq!(
            primary_config_dir_from_sources(None, Some(home.clone()), Some(platform)),
            Some(home.join(".config"))
        );
    }

    #[test]
    fn explicit_xdg_config_home_wins_over_default_and_platform_paths() {
        let explicit = absolute_temp_dir("nabu-explicit-xdg");
        let home = absolute_temp_dir("nabu-home");
        let platform = absolute_temp_dir("nabu-platform-config");

        assert_eq!(
            primary_config_dir_from_sources(Some(explicit.clone()), Some(home), Some(platform)),
            Some(explicit)
        );
    }

    #[test]
    fn relative_xdg_config_home_falls_back_to_platform_when_no_home_is_available() {
        let platform = absolute_temp_dir("nabu-platform-config");

        assert_eq!(
            primary_config_dir_from_sources(
                Some(PathBuf::from("relative-config")),
                None,
                Some(platform.clone())
            ),
            Some(platform)
        );
    }

    #[test]
    fn preferred_path_uses_nabu_namespace() {
        let config_dir = absolute_temp_dir("nabu-config-root");
        let path = preferred_path_in(&config_dir, "settings.json");
        assert_eq!(
            path,
            config_dir.join("com.nabu.app").join("settings.json")
        );
    }

    #[test]
    fn unwritable_primary_config_uses_platform_write_path() {
        let primary = absolute_temp_dir("nabu-unwritable-primary");
        let platform = absolute_temp_dir("nabu-writable-platform");
        let config_dirs = [primary.clone(), platform.clone()];

        let path = writable_path_in_dirs(&config_dirs, "settings.json", |candidate| {
            candidate.starts_with(&platform)
        });

        assert_eq!(path, preferred_path_in(&platform, "settings.json"));
    }

    #[test]
    fn writable_fallback_is_read_before_stale_unwritable_primary() {
        let primary = tempfile::TempDir::new().unwrap();
        let platform = tempfile::TempDir::new().unwrap();
        let primary_path = preferred_path_in(primary.path(), "settings.json");
        let platform_path = preferred_path_in(platform.path(), "settings.json");
        std::fs::create_dir_all(primary_path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(platform_path.parent().unwrap()).unwrap();
        std::fs::write(&primary_path, r#"{"telemetry_consent":null}"#).unwrap();
        std::fs::write(&platform_path, r#"{"telemetry_consent":false}"#).unwrap();

        let config_dirs = [primary.path().to_path_buf(), platform.path().to_path_buf()];
        let write_path = writable_path_in_dirs(&config_dirs, "settings.json", |candidate| {
            candidate.starts_with(platform.path())
        });
        let read_dirs =
            config_dirs_with_write_path_first(&config_dirs, "settings.json", &write_path);

        assert_eq!(
            existing_or_preferred_path_in_dirs(&read_dirs, "settings.json"),
            platform_path
        );
    }

    #[test]
    fn declared_development_namespace_can_replace_current_namespace() {
        assert_eq!(
            app_config_policy().current_namespace_for(Some("com.nabu.app.dev")),
            "com.nabu.app.dev"
        );
    }

    #[test]
    fn unknown_requested_namespace_keeps_production_namespace() {
        assert_eq!(
            app_config_policy().current_namespace_for(Some("com.example.other")),
            "com.nabu.app"
        );
    }

    #[test]
    fn existing_preferred_path_wins_over_legacy_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let preferred = dir
            .path()
            .join(app_config_policy().current_namespace())
            .join("settings.json");
        let legacy = dir
            .path()
            .join(app_config_policy().legacy_namespace.as_str())
            .join("settings.json");
        std::fs::create_dir_all(preferred.parent().unwrap()).unwrap();
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&preferred, "{}").unwrap();
        std::fs::write(&legacy, "{}").unwrap();

        assert_eq!(
            existing_or_preferred_path_in_dirs(&[dir.path().to_path_buf()], "settings.json"),
            preferred
        );
    }

    #[test]
    fn legacy_path_is_read_when_preferred_path_is_absent() {
        let dir = tempfile::TempDir::new().unwrap();
        let legacy = dir
            .path()
            .join(app_config_policy().legacy_namespace.as_str())
            .join("vaults.json");
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&legacy, r#"{"vaults":[]}"#).unwrap();

        assert_eq!(
            existing_or_preferred_path_in_dirs(&[dir.path().to_path_buf()], "vaults.json"),
            legacy
        );
    }

    #[test]
    fn settings_and_vault_registry_share_namespace_fallback_order() {
        let dir = tempfile::TempDir::new().unwrap();

        for file_name in ["settings.json", "vaults.json"] {
            let legacy = dir
                .path()
                .join(app_config_policy().legacy_namespace.as_str())
                .join(file_name);
            std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
            std::fs::write(&legacy, "{}").unwrap();

            assert_eq!(
                existing_or_preferred_path_in_dirs(&[dir.path().to_path_buf()], file_name),
                legacy
            );
        }
    }

    #[test]
    fn previous_platform_config_dir_is_read_when_primary_dir_is_empty() {
        let primary = tempfile::TempDir::new().unwrap();
        let platform = tempfile::TempDir::new().unwrap();
        let existing = platform
            .path()
            .join(app_config_policy().current_namespace())
            .join("settings.json");
        std::fs::create_dir_all(existing.parent().unwrap()).unwrap();
        std::fs::write(&existing, "{}").unwrap();

        assert_eq!(
            existing_or_preferred_path_in_dirs(
                &[primary.path().to_path_buf(), platform.path().to_path_buf()],
                "settings.json"
            ),
            existing
        );
    }

    #[test]
    fn missing_files_use_preferred_path() {
        let dir = tempfile::TempDir::new().unwrap();
        let expected = dir
            .path()
            .join(app_config_policy().current_namespace())
            .join("last-vault.txt");

        assert_eq!(
            existing_or_preferred_path_in_dirs(&[dir.path().to_path_buf()], "last-vault.txt"),
            expected
        );
    }
}
