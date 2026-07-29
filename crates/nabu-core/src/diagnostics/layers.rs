//! # Tracing Layers
//!
//! Logging layers for the tracing subscriber.
//!
//! Provides:
//! - `stderr_layer` — Human-readable output to stderr (pretty in dev, compact in release)
//! - `rolling_file_layer` — JSON file output with daily rotation

use std::path::Path;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::fmt::Layer as FmtLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::Layer;
use tracing::Subscriber;

/// Create a stderr logging layer.
///
/// ## Arguments
///
/// * `pretty` — If `true`, uses pretty-printed output (recommended for development).
///   If `false`, uses compact JSON-like output (recommended for production).
///
/// ## Returns
///
/// A boxed `Layer` suitable for combining with other layers.
pub fn stderr_layer<S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>>(pretty: bool) -> Box<dyn Layer<S> + Send + Sync> {
    if pretty {
        Box::new(
            tracing_subscriber::fmt::layer()
                .pretty()
                .with_target(true)
                .with_thread_ids(false)
                .with_thread_names(false)
                .with_file(true)
                .with_line_number(true)
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .with_writer(std::io::stderr),
        )
    } else {
        Box::new(
            tracing_subscriber::fmt::layer()
                .compact()
                .with_target(true)
                .with_thread_ids(false)
                .with_thread_names(false)
                .with_file(false)
                .with_line_number(false)
                .with_span_events(FmtSpan::NONE)
                .with_writer(std::io::stderr),
        )
    }
}

/// Create a rolling file logging layer.
///
/// Writes JSON-formatted logs to `{log_dir}/{app_name}.log` with daily rotation.
/// Old log files are renamed with date suffixes and retained up to `max_files`.
///
/// ## Arguments
///
/// * `log_dir` — Directory to write log files to. Created if it doesn't exist.
/// * `app_name` — Prefix for log filenames (e.g., "nabu" → "nabu.log", "nabu.2026-07-29.log").
/// * `max_files` — Maximum number of rotated log files to retain.
///
/// ## Returns
///
/// Returns a boxed `Layer` plus a `WorkerGuard` that must be kept alive for the
/// lifetime of the application.
pub fn rolling_file_layer<S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>>(
    log_dir: &Path,
    app_name: &str,
    max_files: usize,
) -> Result<(Box<dyn Layer<S> + Send + Sync>, WorkerGuard), String> {
    use tracing_appender::rolling::Rotation;

    // Ensure log directory exists
    std::fs::create_dir_all(log_dir)
        .map_err(|e| format!("Cannot create log directory {:?}: {}", log_dir, e))?;

    // Create the rolling file appender (daily rotation)
    let file_appender = tracing_appender::rolling::Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix(app_name)
        .filename_suffix("log")
        .max_log_files(max_files)
        .build(log_dir)
        .map_err(|e| format!("Cannot create rolling file appender: {}", e))?;

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(true)
        .with_line_number(true)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_current_span(true)
        .with_writer(non_blocking);

    Ok((Box::new(layer), guard))
}

/// Create a simple file logging layer (non-rolling, for testing).
///
/// ## Arguments
///
/// * `path` — Path to the log file.
///
/// ## Returns
///
/// Returns a boxed `Layer` plus a `WorkerGuard`.
#[cfg(test)]
pub fn test_file_layer<S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>>(
    path: &Path,
) -> Result<(Box<dyn Layer<S> + Send + Sync>, WorkerGuard), String> {
    let file = std::fs::File::create(path)
        .map_err(|e| format!("Cannot create test log file {:?}: {}", path, e))?;

    let (non_blocking, guard) = tracing_appender::non_blocking(file);

    let layer = tracing_subscriber::fmt::layer()
        .json()
        .with_target(true)
        .with_writer(non_blocking);

    Ok((Box::new(layer), guard))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_stderr_layer_creation() {
        let layer: Box<dyn Layer<tracing_subscriber::Registry> + Send + Sync> = stderr_layer(true);
        // Should not panic — just verify it's constructable
        let _ = layer;
    }

    #[test]
    fn test_rolling_file_layer_creation() {
        let dir = tempdir().unwrap();
        let result = rolling_file_layer::<tracing_subscriber::Registry>(dir.path(), "test-nabu", 7);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rolling_file_layer_nonexistent_dir() {
        let dir = Path::new("/nonexistent/path/that/should/not/exist");
        let result = rolling_file_layer::<tracing_subscriber::Registry>(dir, "test", 7);
        assert!(result.is_err());
    }

    #[test]
    fn test_multiple_layers_compose() {
        let dir = tempdir().unwrap();
        let result = rolling_file_layer(dir.path(), "compose-test", 3);
        assert!(result.is_ok());

        let (_file_layer, _guard) = result.unwrap();
        let _stderr = stderr_layer(true);

        // Verify they can be composed into a subscriber
        let _subscriber = tracing_subscriber::registry()
            .with(tracing_subscriber::filter::EnvFilter::new("info"))
            .with(_stderr)
            .with(_file_layer);
    }
}
