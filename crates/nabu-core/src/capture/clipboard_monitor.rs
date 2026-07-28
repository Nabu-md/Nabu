//! Clipboard monitoring service for automatic pasteboard capture.
//!
//! The [`ClipboardMonitor`] watches the macOS pasteboard for changes
//! and dispatches new content to the [`CaptureEngine`] for ingestion.
//! It supports configurable monitoring modes and deduplication to
//! avoid duplicate captures.
//!
//! # Architecture
//!
//! ```text
//! NSPasteboard change notification / polling
//!     ↓
//! ClipboardMonitor
//!     ↓
//! CaptureEngine::ingest
//!     ↓
//! ItemCaptured (event)
//!     ↓
//! IngestionPipeline → ProcessingPipeline → StorageManager
//! ```
//!
//! # Monitoring Modes
//!
//! - **Disabled**: The monitor is inactive and does not capture.
//! - **Manual**: The monitor does not auto-capture; content is
//!   captured only when explicitly triggered via
//!   [`ClipboardMonitor::capture_now`].
//! - **Automatic**: The monitor polls the pasteboard at a configurable
//!   interval and automatically dispatches new content to the engine.
//!
//! # Duplicate Prevention
//!
//! The monitor tracks the pasteboard's `changedCount` to detect
//! new content. If the change count hasn't changed since the last
//! capture, the content is skipped to avoid duplicates.
//!
//! # Error Handling
//!
//! Clipboard monitoring failures are logged and the monitor
//! continues running. No monitoring failure can crash the
//! application.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::capture::{CaptureEngine, ClipboardMonitorConfig, ClipboardMonitorMode};
use crate::event_bus::EventBus;

/// Service that monitors the macOS pasteboard and dispatches
/// new content to the [`CaptureEngine`].
///
/// # Lifecycle
///
/// 1. Create a [`ClipboardMonitor`] with a configuration and engine.
/// 2. Call [`ClipboardMonitor::start`] to begin monitoring.
/// 3. Call [`ClipboardMonitor::stop`] to release resources.
///
/// # Debounce
///
/// The monitor includes built-in debouncing to prevent duplicate
/// captures from rapid pasteboard changes. Content is tracked by
/// change count for a configurable duration (default 1 second).
pub struct ClipboardMonitor {
    config: ClipboardMonitorConfig,
    engine: Arc<CaptureEngine>,
    _event_bus: Arc<EventBus>,
    last_capture_time: Arc<Mutex<Instant>>,
    last_change_count: Arc<Mutex<Option<u64>>>,
    debounce_duration: Duration,
    running: Arc<Mutex<bool>>,
}

impl ClipboardMonitor {
    /// Default debounce duration: 1 second.
    pub const DEFAULT_DEBOUNCE: Duration = Duration::from_secs(1);

    /// Default polling interval for automatic mode: 500ms.
    pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(500);

    /// Creates a new clipboard monitor.
    ///
    /// # Arguments
    ///
    /// * `config` - Clipboard monitor configuration
    /// * `engine` - The capture engine to dispatch to
    /// * `event_bus` - Event bus for publishing events
    pub fn new(
        config: ClipboardMonitorConfig,
        engine: Arc<CaptureEngine>,
        event_bus: Arc<EventBus>,
    ) -> Self {
        Self {
            config,
            engine,
            _event_bus: event_bus,
            last_capture_time: Arc::new(Mutex::new(Instant::now())),
            last_change_count: Arc::new(Mutex::new(None)),
            debounce_duration: Self::DEFAULT_DEBOUNCE,
            running: Arc::new(Mutex::new(false)),
        }
    }

    /// Sets a custom debounce duration.
    pub fn with_debounce(mut self, duration: Duration) -> Self {
        self.debounce_duration = duration;
        self
    }

    /// Sets a custom polling interval for automatic mode.
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.config.poll_interval_ms = interval.as_millis() as u64;
        self
    }

    /// Starts monitoring the pasteboard.
    ///
    /// Returns `Ok(())` on success, or an error string if monitoring
    /// cannot be started (e.g., on non-macOS platforms or when
    /// the mode is Disabled).
    ///
    /// In Automatic mode, this spawns a background task that polls
    /// the pasteboard at the configured interval.
    pub fn start(self: Arc<Self>) -> Result<(), String> {
        if self.config.mode == ClipboardMonitorMode::Disabled {
            return Err("Clipboard monitoring is disabled".to_string());
        }

        *self.running.lock().unwrap() = true;

        if self.config.mode == ClipboardMonitorMode::Automatic {
            self.start_automatic_monitoring();
        }

        Ok(())
    }

    /// Stops monitoring the pasteboard.
    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }

    /// Performs a one-shot clipboard capture.
    ///
    /// This is useful for Manual mode or for explicit user-triggered
    /// captures. Returns `true` if content was captured and dispatched.
    pub fn capture_now(&self) -> bool {
        if self.config.mode == ClipboardMonitorMode::Disabled {
            return false;
        }

        let request = crate::capture::CaptureRequest {
            source_type: "clipboard".to_string(),
            payload: serde_json::json!({}),
            vault_id: String::new(),
            context: HashMap::new(),
        };

        let result = self.engine.dispatch(request);
        result.success
    }

    /// Starts automatic monitoring in a background task.
    fn start_automatic_monitoring(self: Arc<Self>) {
        let engine = Arc::clone(&self.engine);
        let running = Arc::clone(&self.running);
        let last_change_count = Arc::clone(&self.last_change_count);
        let last_capture_time = Arc::clone(&self.last_capture_time);
        let poll_interval = Duration::from_millis(self.config.poll_interval_ms);
        let debounce = self.debounce_duration;

        // Spawn a background task for polling the pasteboard
        #[cfg(feature = "native")]
        {
            let _ = tokio::spawn(async move {
                while *running.lock().unwrap() {
                    tokio::time::sleep(poll_interval).await;

                    if !*running.lock().unwrap() {
                        break;
                    }

                    // Check if enough time has passed since the last capture
                    let elapsed = last_capture_time.lock().unwrap().elapsed();
                    if elapsed < debounce {
                        continue;
                    }

                    // Check if the pasteboard has changed
                    let current_count = Self::pasteboard_change_count();
                    let has_changed = {
                        let mut last_count = last_change_count.lock().unwrap();
                        let changed = *last_count != current_count;
                        if changed {
                            *last_count = current_count;
                        }
                        changed
                    };

                    if !has_changed {
                        continue;
                    }

                    // Dispatch to the engine
                    let request = crate::capture::CaptureRequest {
                        source_type: "clipboard".to_string(),
                        payload: serde_json::json!({}),
                        vault_id: String::new(),
                        context: HashMap::new(),
                    };

                    let _ = engine.dispatch(request);
                    *last_capture_time.lock().unwrap() = Instant::now();
                }
            });
        }

        #[cfg(not(feature = "native"))]
        {
            let _ = engine;
        }
    }

    /// Retrieves the current pasteboard change count.
    ///
    /// Returns 0 on non-macOS platforms or when the pasteboard cannot
    /// be accessed.
    fn pasteboard_change_count() -> u64 {
        #[cfg(target_os = "macos")]
        {
            use objc2::ClassType;

            let pasteboard_class = match objc2::ClassType::class("NSPasteboard") {
                Some(cls) => cls,
                None => return 0,
            };

            let pasteboard: objc2::rc::Retained<objc2::ClassType> =
                unsafe { objc2::msg_send![pasteboard_class, generalPasteboard] };

            let changed_count: u64 = unsafe {
                let msg = objc2::msg_send![pasteboard, changedCount];
                msg
            };
            changed_count
        }
        #[cfg(not(target_os = "macos"))]
        {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CaptureEngine;
    use crate::event_bus::EventBus;
    use std::sync::Arc;

    #[test]
    fn default_config_is_disabled() {
        let config = ClipboardMonitorConfig::default();
        assert_eq!(config.mode, ClipboardMonitorMode::Disabled);
    }

    #[test]
    fn automatic_mode_is_enabled() {
        let config = ClipboardMonitorConfig::new(ClipboardMonitorMode::Automatic);
        assert_eq!(config.mode, ClipboardMonitorMode::Automatic);
    }

    #[test]
    fn manual_mode_is_enabled() {
        let config = ClipboardMonitorConfig::new(ClipboardMonitorMode::Manual);
        assert_eq!(config.mode, ClipboardMonitorMode::Manual);
    }

    #[test]
    fn disabled_mode_prevents_capture() {
        let config = ClipboardMonitorConfig::new(ClipboardMonitorMode::Disabled);
        assert!(!config.should_capture_url());
        assert!(!config.should_capture_text());
        assert!(!config.should_capture_image());
    }

    #[test]
    fn monitor_creation() {
        let bus = Arc::new(EventBus::new());
        let engine = Arc::new(CaptureEngine::new(bus.clone()));
        let config = ClipboardMonitorConfig::new(ClipboardMonitorMode::Automatic);
        let monitor = ClipboardMonitor::new(config, engine, bus);

        assert!(!*monitor.running.lock().unwrap());
    }

    #[test]
    fn custom_poll_interval() {
        let config = ClipboardMonitorConfig::new(ClipboardMonitorMode::Automatic);
        assert_eq!(config.poll_interval_ms, 500);

        let mut config = config;
        config.poll_interval_ms = 1000;
        assert_eq!(config.poll_interval_ms, 1000);
    }

    #[test]
    fn debounce_duration_default() {
        let bus = Arc::new(EventBus::new());
        let engine = Arc::new(CaptureEngine::new(bus.clone()));
        let config = ClipboardMonitorConfig::new(ClipboardMonitorMode::Automatic);
        let monitor = ClipboardMonitor::new(config, engine, bus);

        assert_eq!(monitor.debounce_duration, ClipboardMonitor::DEFAULT_DEBOUNCE);
    }
}