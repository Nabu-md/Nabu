use std::sync::atomic::{AtomicU8, Ordering};

/// FluidVoice states surfaced to the dictation pill.
pub const FLUIDVOICE_STATE_IDLE: u8 = 0;
pub const FLUIDVOICE_STATE_LISTENING: u8 = 1;
pub const FLUIDVOICE_STATE_PROCESSING: u8 = 2;

static FLUIDVOICE_STATE: AtomicU8 = AtomicU8::new(FLUIDVOICE_STATE_IDLE);

/// Known FluidVoice speech models (see altic-dev/FluidVoice). The standalone
/// app exposes these through its own settings; we surface them read-only.
pub const FLUIDVOICE_MODELS: &[&str] = &[
    "nemotron",
    "parakeet",
    "apple_speech",
    "whisper",
];

const FLUIDVOICE_BUNDLE_ID: &str = "com.altic.FluidVoice";

fn set_fluidvoice_state(state: u8) {
    FLUIDVOICE_STATE.store(state, Ordering::SeqCst);
}

/// Reports whether FluidVoice is installed on this Mac (checked via
/// Spotlight-style bundle lookup through mdfind, with an /Applications
/// fallback).
#[tauri::command]
pub fn fluidvoice_installed() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        let probe = |program: &str, args: &[&str]| -> bool {
            Command::new(program)
                .args(args)
                .output()
                .map(|output| output.status.success() && !output.stdout.is_empty())
                .unwrap_or(false)
        };

        if probe("/usr/bin/mdfind", &[format!("kMDItemCFBundleIdentifier == '{FLUIDVOICE_BUNDLE_ID}'").as_str()]) {
            return Ok(true);
        }
        if probe("/usr/bin/mdfind", &[format!("kMDItemKind == 'Application' && kMDItemDisplayName == 'FluidVoice*'").as_str()]) {
            return Ok(true);
        }
        let home = std::env::var("HOME").unwrap_or_default();
        for base in ["/Applications", &format!("{home}/Applications")] {
            if std::path::Path::new(base).join("FluidVoice.app").exists() {
                return Ok(true);
            }
        }
        Ok(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(false)
    }
}

/// Returns the FluidVoice speech models Nabu can target.
#[tauri::command]
pub fn fluidvoice_models() -> Result<Vec<String>, String> {
    Ok(FLUIDVOICE_MODELS.iter().map(|model| model.to_string()).collect())
}

/// Starts FluidVoice dictation by invoking its global toggle through Apple
/// Events (osascript). FluidVoice runs as a separate process; no code is
/// linked, which keeps Nabu's AGPL licensing clean.
///
/// The requested model is recorded for diagnostics; the FluidVoice app
/// itself exposes model selection through its own settings, so the bridge
/// toggle starts dictation with whichever model the app has configured.
#[tauri::command]
pub fn fluidvoice_start(model: Option<String>) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        use std::sync::atomic::AtomicU8;

        static FLUIDVOICE_REQUESTED_MODEL_SET: AtomicU8 = AtomicU8::new(0);

        if let Some(requested) = model.as_deref() {
            FLUIDVOICE_REQUESTED_MODEL_SET.store(1, Ordering::SeqCst);
            eprintln!("[fluidvoice] start requested with model: {requested}");
        } else {
            FLUIDVOICE_REQUESTED_MODEL_SET.store(0, Ordering::SeqCst);
        }

        set_fluidvoice_state(FLUIDVOICE_STATE_LISTENING);
        let script = r#"tell application "System Events" to tell process "FluidVoice" to click menu bar item 1 of menu bar 2"#;
        let output = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(script)
            .output()
            .map_err(|error| format!("Failed to launch FluidVoice bridge: {error}"))?;
        if !output.status.success() {
            set_fluidvoice_state(FLUIDVOICE_STATE_IDLE);
            return Err(format!(
                "FluidVoice bridge failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = model;
        Err("FluidVoice integration requires macOS".into())
    }
}

/// Stops FluidVoice dictation via the same Apple Events bridge.
#[tauri::command]
pub fn fluidvoice_stop() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;

        set_fluidvoice_state(FLUIDVOICE_STATE_PROCESSING);
        let script = r#"tell application "System Events" to tell process "FluidVoice" to click menu bar item 1 of menu bar 2"#;
        let output = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(script)
            .output()
            .map_err(|error| {
                set_fluidvoice_state(FLUIDVOICE_STATE_IDLE);
                format!("Failed to launch FluidVoice bridge: {error}")
            })?;
        if !output.status.success() {
            set_fluidvoice_state(FLUIDVOICE_STATE_IDLE);
            return Err(format!(
                "FluidVoice bridge failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        set_fluidvoice_state(FLUIDVOICE_STATE_IDLE);
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("FluidVoice integration requires macOS".into())
    }
}

/// Returns the last known FluidVoice bridge state: 0 idle, 1 listening,
/// 2 processing.
#[tauri::command]
pub fn fluidvoice_status() -> Result<u8, String> {
    Ok(FLUIDVOICE_STATE.load(Ordering::SeqCst))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_list_is_stable() {
        let models = fluidvoice_models().unwrap();
        assert_eq!(models, vec!["nemotron", "parakeet", "apple_speech", "whisper"]);
    }

    #[test]
    fn status_defaults_to_idle() {
        assert_eq!(FLUIDVOICE_STATE.load(Ordering::SeqCst), FLUIDVOICE_STATE_IDLE);
    }
}
