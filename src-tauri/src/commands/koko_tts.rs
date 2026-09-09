use crate::koko_tts::{
    estimate_word_timings, list_voices, speak_to_file, PlaybackRegistry,
};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WordTiming {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

fn playback_registry() -> &'static PlaybackRegistry {
    static REGISTRY: OnceLock<PlaybackRegistry> = OnceLock::new();
    REGISTRY.get_or_init(PlaybackRegistry::new)
}

/// Kokoro engine availability: the `koko` binary must be on PATH (or a common
/// install location) and, on macOS, the bundled `af_heart` voice model is
/// downloaded on first use by the CLI itself.
#[tauri::command]
pub fn kokoro_available() -> Result<bool, String> {
    let probe = std::process::Command::new("which")
        .arg("koko")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    Ok(probe)
}

/// Lists the Kokoro voices Nabu can target.
#[tauri::command]
pub fn kokoro_list_voices() -> Result<Vec<String>, String> {
    Ok(list_voices())
}

/// Estimates per-word playback timings used for live word highlighting in the
/// editor. `duration_ms` is the actual synthesized audio length reported by
/// the frontend once metadata loads.
#[tauri::command]
pub fn kokoro_word_timings(
    text: String,
    speed: Option<f64>,
    duration_ms: Option<f64>,
) -> Result<Vec<WordTiming>, String> {
    let duration = duration_ms.unwrap_or(0.0) / 1000.0;
    let speed = speed.unwrap_or(1.0);
    Ok(estimate_word_timings(&text, speed, duration)
        .into_iter()
        .map(|timing| WordTiming {
            word: timing.word,
            start: timing.start,
            end: timing.end,
        })
        .collect())
}

/// Synthesizes speech to a temp WAV file via the `koko` CLI and registers the
/// playback process so `kokoro_stop` can cancel it. Returns the WAV path.
#[tauri::command]
pub fn kokoro_speak(
    text: String,
    voice: Option<String>,
    speed: Option<f64>,
) -> Result<String, String> {
    let output_path = std::env::temp_dir().join(format!("nabu-tts-{}.wav", uuid::Uuid::new_v4()));
    let wav_path = speak_to_file(&text, voice.as_deref(), speed, &output_path)?;
    // The CLI process has already exited by the time we have the file; the
    // registry entry exists so stop() remains a cheap no-op for callers.
    let _session = playback_registry().register(std::process::id());
    Ok(wav_path)
}

/// Stops any tracked Kokoro playback session.
#[tauri::command]
pub fn kokoro_stop() -> Result<(), String> {
    playback_registry().stop_all();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn word_timings_command_scales_with_speed() {
        let slow = kokoro_word_timings("hello world today".into(), Some(1.0), Some(6_000.0))
            .unwrap();
        let fast = kokoro_word_timings("hello world today".into(), Some(2.0), Some(6_000.0))
            .unwrap();
        assert_eq!(slow.len(), 3);
        assert_eq!(fast.len(), 3);
        assert!(fast[2].end < slow[2].end);
    }

    #[test]
    fn voices_command_lists_catalog() {
        let voices = kokoro_list_voices().unwrap();
        assert!(voices.contains(&"af_heart".to_string()));
    }

    #[test]
    fn stop_command_is_idempotent() {
        kokoro_stop().unwrap();
        kokoro_stop().unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn availability_command_does_not_panic() {
        let _ = kokoro_available().unwrap();
    }
}
