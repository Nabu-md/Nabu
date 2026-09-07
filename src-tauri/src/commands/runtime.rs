use serde::Serialize;
use std::{sync::Mutex, time::Instant};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StartupMilestone {
    pub name: String,
    pub elapsed_ms: u64,
    pub renderer_elapsed_ms: Option<u64>,
    pub detail: Option<u64>,
}

pub struct StartupTimingState {
    started_at: Instant,
    trace: Mutex<Vec<StartupMilestone>>,
}

impl Default for StartupTimingState {
    fn default() -> Self {
        Self {
            started_at: Instant::now(),
            trace: Mutex::new(Vec::new()),
        }
    }
}

impl StartupTimingState {
    fn elapsed_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis() as u64
    }

    fn record_milestone(
        &self,
        name: &str,
        renderer_elapsed_ms: Option<u64>,
        detail: Option<u64>,
    ) -> StartupMilestone {
        let mut trace = self.trace.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(existing) = trace.iter().find(|entry| entry.name == name) {
            return existing.clone();
        }

        let milestone = StartupMilestone {
            name: name.to_string(),
            elapsed_ms: self.elapsed_ms(),
            renderer_elapsed_ms,
            detail,
        };
        trace.push(milestone.clone());
        milestone
    }

    fn trace(&self) -> Vec<StartupMilestone> {
        self.trace
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }
}

#[tauri::command]
pub fn get_startup_elapsed_ms(state: tauri::State<'_, StartupTimingState>) -> u64 {
    state.elapsed_ms()
}

fn valid_startup_milestone_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
}

#[tauri::command]
pub fn record_startup_milestone(
    state: tauri::State<'_, StartupTimingState>,
    name: String,
    renderer_elapsed_ms: Option<u64>,
    detail: Option<u64>,
) -> Result<StartupMilestone, String> {
    if !valid_startup_milestone_name(&name) {
        return Err("Invalid startup milestone name".to_string());
    }

    let milestone = state.record_milestone(&name, renderer_elapsed_ms, detail);
    if std::env::var("NABU_STARTUP_TRACE").as_deref() == Ok("1") {
        let json = serde_json::to_string(&milestone)
            .map_err(|error| format!("Failed to serialize startup milestone: {error}"))?;
        eprintln!("NABU_STARTUP_TRACE {json}");
    }
    Ok(milestone)
}

#[tauri::command]
pub fn get_startup_trace(state: tauri::State<'_, StartupTimingState>) -> Vec<StartupMilestone> {
    state.trace()
}

fn should_use_external_media_preview_for_appimage(is_linux_appimage: bool) -> bool {
    is_linux_appimage
}

fn map_print_result<E: std::fmt::Display>(result: Result<(), E>) -> Result<(), String> {
    result.map_err(|error| format!("Failed to open the system print dialog: {error}"))
}

#[cfg(all(desktop, target_os = "linux"))]
fn linux_appimage_running() -> bool {
    crate::linux_appimage::is_running()
}

#[cfg(not(all(desktop, target_os = "linux")))]
fn linux_appimage_running() -> bool {
    false
}

#[tauri::command]
pub fn should_use_external_media_preview() -> bool {
    should_use_external_media_preview_for_appimage(linux_appimage_running())
}

#[tauri::command]
pub fn print_current_webview(window: tauri::WebviewWindow) -> Result<(), String> {
    map_print_result(window.print())
}

#[cfg(test)]
mod tests {
    use super::{
        map_print_result, should_use_external_media_preview_for_appimage, StartupTimingState,
    };

    #[test]
    fn external_media_preview_is_limited_to_linux_appimage() {
        assert!(should_use_external_media_preview_for_appimage(true));
        assert!(!should_use_external_media_preview_for_appimage(false));
    }

    #[test]
    fn print_errors_are_formatted_for_the_renderer() {
        let result = map_print_result::<&str>(Err("printer unavailable"));

        assert_eq!(
            result,
            Err("Failed to open the system print dialog: printer unavailable".to_string())
        );
    }

    #[test]
    fn startup_clock_reports_elapsed_milliseconds() {
        assert!(StartupTimingState::default().elapsed_ms() < 100);
    }

    #[test]
    fn startup_trace_records_each_milestone_once_in_order() {
        let state = StartupTimingState::default();

        let renderer = state.record_milestone("renderer_module_loaded", Some(1), Some(7));
        let duplicate = state.record_milestone("renderer_module_loaded", Some(99), Some(99));
        let interactive = state.record_milestone("app_interactive", Some(10), None);

        assert_eq!(renderer.name, "renderer_module_loaded");
        assert_eq!(renderer.renderer_elapsed_ms, Some(1));
        assert_eq!(renderer.detail, Some(7));
        assert_eq!(duplicate, renderer);
        assert!(interactive.elapsed_ms >= renderer.elapsed_ms);
        assert_eq!(
            state
                .trace()
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            vec!["renderer_module_loaded", "app_interactive"]
        );
    }
}
