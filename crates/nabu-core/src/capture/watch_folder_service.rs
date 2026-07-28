use crate::capture::{CaptureEngine, WatchFolderConfig};
use crate::event_bus::EventBus;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher as NotifyWatcher};

/// Service that monitors configured watch folders and dispatches new files
/// to the [`CaptureEngine`] for ingestion.
///
/// # Lifecycle
///
/// 1. Create a [`WatchFolderService`] with a configuration and engine.
/// 2. Call [`WatchFolderService::start`] to begin watching.
/// 3. Call [`WatchFolderService::stop`] to release resources.
///
/// # Debounce
///
/// The service includes built-in debouncing to prevent duplicate imports
/// from multiple filesystem events (e.g., file copy + metadata update).
/// Files are tracked by path for a configurable duration (default 2 seconds).
pub struct WatchFolderService {
    config: WatchFolderConfig,
    engine: Arc<CaptureEngine>,
    _event_bus: Arc<EventBus>,
    _watcher: Option<RecommendedWatcher>,
    recent_files: Arc<Mutex<HashMap<PathBuf, Instant>>>,
    debounce_duration: Duration,
}

impl WatchFolderService {
    /// Default debounce duration: 2 seconds.
    pub const DEFAULT_DEBOUNCE: Duration = Duration::from_secs(2);

    /// Creates a new watch folder service.
    ///
    /// # Arguments
    ///
    /// * `config` - Watch folder configuration
    /// * `engine` - The capture engine to dispatch to
    /// * `event_bus` - Event bus for future subscriptions
    pub fn new(
        config: WatchFolderConfig,
        engine: Arc<CaptureEngine>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            config,
            engine,
            _event_bus: event_bus,
            _watcher: None,
            recent_files: Arc::new(Mutex::new(HashMap::new())),
            debounce_duration: Self::DEFAULT_DEBOUNCE,
        }
    }

    /// Sets a custom debounce duration.
    pub fn with_debounce(mut self, duration: Duration) -> Self {
        self.debounce_duration = duration;
        self
    }

    /// Starts watching all enabled folders from the configuration.
    ///
    /// Returns `Ok(())` on success, or an error string if no valid folders
    /// are configured or watcher setup fails.
    pub fn start(mut self) -> Result<Self, String> {
        let enabled_folders: Vec<_> = self
            .config
            .folders
            .iter()
            .filter(|f| f.enabled)
            .collect();

        if enabled_folders.is_empty() {
            return Err("No enabled watch folders configured".to_string());
        }

        let recent_files = Arc::clone(&self.recent_files);
        let engine = Arc::clone(&self.engine);
        let debounce = self.debounce_duration;

        // Create a new watcher with a callback that processes events
        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    for path in event.paths {
                        Self::handle_path(path.clone(), &engine, &recent_files, debounce);
                    }
                }
            },
            Config::default(),
        )
        .map_err(|e| format!("{}", e))?;

        // Watch each enabled folder
        for folder in enabled_folders {
            let path = PathBuf::from(&folder.path);
            watcher
                .watch(&path, RecursiveMode::Recursive)
                .map_err(|e| format!("{}", e))?;
        }

        self._watcher = Some(watcher);
        Ok(self)
    }

    /// Stops watching and releases resources.
    pub fn stop(mut self) {
        self._watcher = None;
    }

    fn handle_path(
        path: PathBuf,
        engine: &Arc<CaptureEngine>,
        recent_files: &Arc<Mutex<HashMap<PathBuf, Instant>>>,
        debounce: Duration,
    ) {
        // Only process files
        if !path.is_file() {
            return;
        }

        // Debounce: skip if seen recently
        {
            let mut recent = recent_files.lock().unwrap();
            let now = Instant::now();

            // Cleanup expired entries
            recent.retain(|_, last_time| now.duration_since(*last_time) < debounce);

            if recent.contains_key(&path) {
                return;
            }

            recent.insert(path.clone(), now);
        }

        // Dispatch to capture engine
        let request = crate::capture::CaptureRequest {
            source_type: "watch_folder".to_string(),
            payload: serde_json::json!({
                "file_path": path.to_str(),
                "folder_id": None::<String>
            }),
            vault_id: "default".to_string(),
            context: std::collections::HashMap::new(),
        };

        let _ = engine.ingest(request);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn rejects_no_enabled_folders() {
        let config = WatchFolderConfig {
            folders: vec![
                crate::capture::ImportFolder::new("/tmp/inbox_a").with_enabled(false),
                crate::capture::ImportFolder::new("/tmp/inbox_b").with_enabled(false),
            ],
        };

        let bus = Arc::new(EventBus::new());
        let engine = Arc::new(CaptureEngine::new(bus.clone()));
        let service = WatchFolderService::new(config, engine, bus);

        let result = service.start();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No enabled"));
    }

    #[test]
    fn creates_service_with_default_debounce() {
        let config = WatchFolderConfig::default();
        let bus = Arc::new(EventBus::new());
        let engine = Arc::new(CaptureEngine::new(bus.clone()));
        let service = WatchFolderService::new(config, Arc::clone(&engine), bus);
        assert_eq!(service.debounce_duration, WatchFolderService::DEFAULT_DEBOUNCE);
    }

    #[test]
    fn custom_debounce_is_applied() {
        let config = WatchFolderConfig::default();
        let bus = Arc::new(EventBus::new());
        let engine = Arc::new(CaptureEngine::new(bus.clone()));
        let custom = Duration::from_millis(500);
        let service = WatchFolderService::new(config, Arc::clone(&engine), bus)
            .with_debounce(custom);
        assert_eq!(service.debounce_duration, custom);
    }
}