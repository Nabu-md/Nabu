use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// One synthesized word with playback timings, for live highlighting.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KokoroWordTiming {
    pub word: String,
    pub start: f64,
    pub end: f64,
}

/// Splits text into words and distributes the total duration evenly,
/// weighted by word length (character count). Pure estimation — no audio
/// decode in Rust, keeping the command cheap and deterministic.
pub fn estimate_word_timings(text: &str, speed: f64, duration: f64) -> Vec<KokoroWordTiming> {
    let speed = if speed.is_finite() && speed > 0.0 {
        speed
    } else {
        1.0
    };
    let duration = if duration.is_finite() && duration > 0.0 {
        duration
    } else {
        0.0
    };

    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() || duration <= 0.0 {
        return words
            .into_iter()
            .map(|word| KokoroWordTiming {
                word: word.to_string(),
                start: 0.0,
                end: 0.0,
            })
            .collect();
    }

    let total_units: usize = words.iter().map(|word| word.chars().count().max(1)).sum();
    // `duration` is the WAV's native (1.0x) length; the audio element plays it
    // at `playbackRate = speed`, so the highlight timeline spans duration/speed.
    let scaled = duration / speed;
    let mut cursor: f64 = 0.0;
    words
        .into_iter()
        .map(|word| {
            let units = word.chars().count().max(1) as f64;
            let span = scaled * units / total_units as f64;
            let timing = KokoroWordTiming {
                word: word.to_string(),
                start: (cursor * 1000.0).round() / 1000.0,
                end: ((cursor + span) * 1000.0).round() / 1000.0,
            };
            cursor += span;
            timing
        })
        .collect()
}

/// Known Kokoro voice ids surfaced for the settings picker. The list mirrors
/// the upstream Kokoro-82M voice set (prefix: a=American, b=British; f/m =
/// voice gender).
pub const KOKORO_VOICES: &[&str] = &[
    "af_heart", "af_alloy", "af_aoede", "af_bella", "af_jessica", "af_kore", "af_nicole",
    "af_nova", "af_river", "af_sarah", "af_sky", "am_adam", "am_echo", "am_eric", "am_fenrir",
    "am_liam", "am_michael", "am_onyx", "am_puck", "am_santa", "bf_alice", "bf_emma", "bf_isabella",
    "bf_lily", "bm_daniel", "bm_fable", "bm_george", "bm_lewis",
];

/// Returns the voice catalogue.
pub fn list_voices() -> Vec<String> {
    KOKORO_VOICES.iter().map(|voice| voice.to_string()).collect()
}

/// Normalizes an optional speed setting, clamped to the documented 0.5–2.0
/// range. `None` passes through.
pub fn normalize_speed(speed: Option<f64>) -> Option<f64> {
    speed
        .filter(|speed| speed.is_finite())
        .map(|speed| speed.clamp(0.5, 2.0))
}

// ── Playback process tracking ───────────────────────────────────────────────

static PLAYBACK_SEQ: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub struct PlaybackRegistry {
    /// session id -> OS process id of the running synthesis/playback.
    sessions: Mutex<std::collections::HashMap<String, u32>>,
}

impl PlaybackRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, process_id: u32) -> String {
        let session = format!("koko-{}", PLAYBACK_SEQ.fetch_add(1, Ordering::Relaxed));
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.insert(session.clone(), process_id);
        }
        session
    }

    pub fn stop(&self, session: &str) -> bool {
        let removed = self
            .sessions
            .lock()
            .ok()
            .and_then(|mut sessions| sessions.remove(session));
        match removed {
            Some(process_id) => {
                #[cfg(target_os = "macos")]
                {
                    _ = std::process::Command::new("kill").arg(process_id.to_string()).output();
                }
                #[cfg(not(target_os = "macos"))]
                {
                    _ = process_id;
                }
                true
            }
            None => false,
        }
    }

    pub fn stop_all(&self) -> usize {
        let mut stopped = 0;
        if let Ok(sessions) = self.sessions.lock() {
            let ids: Vec<String> = sessions.keys().cloned().collect();
            for session in ids {
                if self.stop(&session) {
                    stopped += 1;
                }
            }
        }
        stopped
    }
}

/// Runs `koko text "<text>" --voice <voice> --output <wav>` when the CLI is
/// installed, returning the temp WAV path for the frontend to play. In
/// environments without the CLI (tests, CI) the call fails with actionable
/// guidance instead of pretending to synthesize.
pub fn speak_to_file(
    text: &str,
    voice: Option<&str>,
    speed: Option<f64>,
    output_path: &std::path::Path,
) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Nothing to read aloud: the selection is empty".to_string());
    }
    if text.len() > 20_000 {
        return Err("Selection is too long to read aloud (20k character limit)".to_string());
    }
    let voice = voice.unwrap_or("af_heart");

    let mut command = crate::hidden_command("koko");
    command
        .arg("text")
        .arg(text)
        .arg("--voice")
        .arg(voice)
        .arg("--output")
        .arg(output_path);
    if let Some(speed) = normalize_speed(speed) {
        command.arg("--speed").arg(speed.to_string());
    }

    let output = command
        .output()
        .map_err(|error| format!("Kokoro (koko) CLI is not available: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("Kokoro synthesis failed: {stderr}"));
    }
    Ok(output_path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timings_cover_duration_in_order() {
        let timings = estimate_word_timings("one two three four five", 1.0, 10.0);
        assert_eq!(timings.len(), 5);
        assert_eq!(timings[0].word, "one");
        assert!((timings[0].start - 0.0).abs() < 1e-6);
        assert!((timings[4].end - 10.0).abs() < 1e-3);
        for pair in timings.windows(2) {
            assert!(pair[1].start >= pair[0].end - 1e-9);
        }
    }

    #[test]
    fn speed_scales_timings_down() {
        let normal = estimate_word_timings("a b c d", 1.0, 8.0);
        let faster = estimate_word_timings("a b c d", 2.0, 8.0);
        assert!(faster.last().unwrap().end < normal.last().unwrap().end);
    }

    #[test]
    fn empty_text_yields_no_timings() {
        assert!(estimate_word_timings("   \n\t", 1.0, 5.0).is_empty());
    }

    #[test]
    fn zero_duration_yields_zero_timings() {
        let timings = estimate_word_timings("hello world", 1.0, 0.0);
        assert_eq!(timings.len(), 2);
        assert_eq!(timings[0].start, 0.0);
        assert_eq!(timings[1].end, 0.0);
    }

    #[test]
    fn non_finite_speed_falls_back_to_one() {
        let normal = estimate_word_timings("a b c", 1.0, 6.0);
        let nan_speed = estimate_word_timings("a b c", f64::NAN, 6.0);
        assert_eq!(normal.last().unwrap().end, nan_speed.last().unwrap().end);
    }

    #[test]
    fn voice_catalog_is_non_empty_and_prefixed() {
        let voices = list_voices();
        assert!(!voices.is_empty());
        assert!(voices.iter().all(|voice| voice.len() > 3 && voice.contains('_')));
        assert!(voices.iter().any(|voice| voice == "af_sky"));
    }

    #[test]
    fn normalize_speed_clamps_to_documented_range() {
        assert_eq!(normalize_speed(Some(0.1)), Some(0.5));
        assert_eq!(normalize_speed(Some(1.25)), Some(1.25));
        assert_eq!(normalize_speed(Some(9.9)), Some(2.0));
        assert_eq!(normalize_speed(Some(f64::NAN)), None);
        assert_eq!(normalize_speed(None), None);
    }

    #[test]
    fn speak_rejects_empty_text_without_spawning() {
        let error = speak_to_file("   ", None, None, std::path::Path::new("/tmp/x.wav"))
            .unwrap_err();
        assert!(error.contains("empty"), "unexpected: {error}");
    }

    #[test]
    fn playback_registry_tracks_sessions() {
        let registry = PlaybackRegistry::new();
        let session = registry.register(4_194_304);
        assert!(registry.stop(&session));
        assert!(!registry.stop(&session));
        assert_eq!(registry.stop_all(), 0);
    }
}
