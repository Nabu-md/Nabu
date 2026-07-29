//! # Tracing Initialization
//!
//! Initializes the global tracing subscriber for Nabu.
//!
//! ## Configuration
//!
//! Log levels are controlled by the `NABU_LOG` or `RUST_LOG` environment
//! variable in standard `tracing-subscriber` env-filter format:
//!
//! - `NABU_LOG=debug` — Debug and above for all Nabu subsystems
//! - `NABU_LOG=nabu_core=debug,info` — Debug for core, info for everything else
//! - `NABU_LOG=off` — Silent
//!
//! If `NABU_LOG` is not set, `RUST_LOG` is used as fallback.
//! If neither is set, the default is `info` for release builds and `debug`
//! for debug builds.
//!
//! ## Log File Location
//!
//! Logs are written to `{vault_path}/.nabu/logs/nabu.log` when a vault path
//! is provided, or to `./.nabu/logs/nabu.log` otherwise.
//!
//! Log files are rotated daily and retained for 7 days by default.
//!
//! ## Safety
//!
//! Safe to call multiple times — subsequent calls are no-ops.
//! Must be called before any other tracing macro to capture all events.

use std::path::PathBuf;
use std::sync::OnceLock;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::Layer;

static INITIALIZED: OnceLock<bool> = OnceLock::new();

/// Default log directory relative to the vault or working directory.
const DEFAULT_LOG_DIR: &str = ".nabu/logs";

/// Maximum number of log files to retain during rotation.
const MAX_LOG_FILES: usize = 7;

/// Initialize the tracing subscriber with sensible defaults.
///
/// Uses `NABU_LOG` or `RUST_LOG` environment variables for filtering.
/// Writes JSON logs to a rotating file under `.nabu/logs/` and pretty-prints
/// to stderr in development mode.
///
/// ## Arguments
///
/// * `vault_path` — Optional path to the vault root. Logs are written to
///   `{vault_path}/.nabu/logs/`. If `None`, uses the current working directory.
/// * `app_name` — Application name for log file naming (default: "nabu").
///
/// ## Returns
///
/// Returns `true` if initialization succeeded or was already done.
/// Returns `false` only if initialization fails (e.g., cannot create log dir).
pub fn init(vault_path: Option<&std::path::Path>, app_name: &str) -> bool {
    if INITIALIZED.set(true).is_err() {
        // Already initialized — safe no-op
        return true;
    }

    // Build the env filter from environment variables
    let env_filter = build_env_filter();

    // Determine log directory
    let log_dir = resolve_log_dir(vault_path);

    // Create the log directory if needed
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        // Log directory creation failed — fall back to stderr-only
        eprintln!(
            "[nabu::diagnostics] Warning: Cannot create log directory {:?}: {}. \
             Falling back to stderr-only logging.",
            log_dir, e
        );
        init_stderr_only(env_filter);
        return false;
    }

    // Build the file appender layer (rolling, JSON)
    let file_layer = match crate::diagnostics::layers::rolling_file_layer(
        &log_dir,
        app_name,
        MAX_LOG_FILES,
    ) {
        Ok(layer) => layer,
        Err(e) => {
            eprintln!(
                "[nabu::diagnostics] Warning: Cannot initialize file logging: {}. \
                 Falling back to stderr-only.",
                e
            );
            init_stderr_only(env_filter);
            return false;
        }
    };

    // Build the stderr layer (pretty-printed for dev, compact for release)
    let stderr_layer = crate::diagnostics::layers::stderr_layer(
        cfg!(debug_assertions), // pretty in debug, compact in release
    );

    // Combine layers
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(stderr_layer)
        .with(file_layer);

    // Suppress the "try_init" warning — we already check with OnceLock
    let _ = subscriber.try_init();

    tracing::info!(
        subsystem = "diagnostics",
        component = "init",
        operation = "init",
        log_dir = %log_dir.display(),
        app_name = app_name,
        "Tracing initialized"
    );

    true
}

/// Initialize with stderr-only logging (no file output).
/// Used as fallback when file logging is unavailable.
fn init_stderr_only(env_filter: EnvFilter) {
    let stderr_layer = crate::diagnostics::layers::stderr_layer(true);
    let subscriber = tracing_subscriber::registry()
        .with(env_filter)
        .with(stderr_layer);

    let _ = subscriber.try_init();
}

/// Build an `EnvFilter` from `NABU_LOG` or `RUST_LOG`, with a sensible default.
fn build_env_filter() -> EnvFilter {
    // Check NABU_LOG first, then RUST_LOG
    let filter_string = std::env::var("NABU_LOG")
        .ok()
        .or_else(|| std::env::var("RUST_LOG").ok())
        .unwrap_or_else(default_log_level);

    match EnvFilter::try_new(&filter_string) {
        Ok(filter) => filter,
        Err(e) => {
            eprintln!(
                "[nabu::diagnostics] Warning: Invalid log filter '{}': {}. \
                 Using default 'info'.",
                filter_string, e
            );
            EnvFilter::new("info")
        }
    }
}

/// Default log level based on build profile.
fn default_log_level() -> String {
    if cfg!(debug_assertions) {
        "debug".to_string()
    } else {
        "info".to_string()
    }
}

/// Resolve the log directory path.
fn resolve_log_dir(vault_path: Option<&std::path::Path>) -> PathBuf {
    match vault_path {
        Some(vault) => {
            let mut dir = vault.to_path_buf();
            dir.push(DEFAULT_LOG_DIR);
            dir
        }
        None => PathBuf::from(DEFAULT_LOG_DIR),
    }
}

/// Check whether the tracing subscriber has been initialized.
pub fn is_initialized() -> bool {
    INITIALIZED.get().copied().unwrap_or(false)
}

/// Reset the initialization state (for testing only).
#[cfg(test)]
pub fn reset_for_testing() {
    INITIALIZED.take();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_init_with_vault_path() {
        let dir = tempdir().unwrap();
        let result = init(Some(dir.path()), "nabu-test");
        assert!(result);

        // Verify log directory was created
        let log_dir = dir.path().join(".nabu/logs");
        assert!(log_dir.exists());

        // Verify initialization is tracked
        assert!(is_initialized());

        // Second call is a no-op
        assert!(init(Some(dir.path()), "nabu-test"));
    }

    #[test]
    fn test_init_without_vault_path() {
        let result = init(None, "nabu-test");
        assert!(result);

        let log_dir = PathBuf::from(".nabu/logs");
        assert!(log_dir.exists());

        assert!(is_initialized());
    }

    #[test]
    fn test_build_env_filter_default() {
        // Clear env vars for this test
        std::env::remove_var("NABU_LOG");
        std::env::remove_var("RUST_LOG");

        let filter = build_env_filter();
        // Should not panic, should produce a valid filter
        let _ = filter;
    }

    #[test]
    fn test_build_env_filter_from_env() {
        std::env::set_var("NABU_LOG", "warn");
        let filter = build_env_filter();
        let _ = filter;
        std::env::remove_var("NABU_LOG");
    }

    #[test]
    fn test_resolve_log_dir_with_vault() {
        let vault = PathBuf::from("/tmp/test-vault");
        let dir = resolve_log_dir(Some(&vault));
        assert_eq!(dir, PathBuf::from("/tmp/test-vault/.nabu/logs"));
    }

    #[test]
    fn test_resolve_log_dir_without_vault() {
        let dir = resolve_log_dir(None);
        assert_eq!(dir, PathBuf::from(".nabu/logs"));
    }

    #[test]
    fn test_default_log_level() {
        let level = default_log_level();
        // Should be "debug" or "info" depending on build profile
        assert!(level == "debug" || level == "info");
    }
}
