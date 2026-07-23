use thiserror::Error;

/// Audio segment produced by a transcription pass.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DictationSegment {
    pub text: String,
    pub language: Option<String>,
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
}

/// Final transcription result returned to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DictationResult {
    pub text: String,
    pub language: Option<String>,
    pub segments: Vec<DictationSegment>,
}

/// Errors produced by the dictation engine.
#[derive(Debug, Error)]
pub enum DictationError {
    #[error("audio input missing")]
    Missing,
    #[error("unsupported audio format: {0}")]
    UnsupportedFormat(String),
    #[error("transcription failed: {0}")]
    Transcription(String),
    #[error("model initialization failed: {0}")]
    ModelInit(String),
}

/// Native dictation engine abstraction.
///
/// The production implementation may use `whisper-rs` or another stable
/// crate once the appropriate native dependency is confirmed.
pub trait DictationEngine {
    fn transcribe(&self, audio_path: &str) -> Result<DictationResult, DictationError>;
}

/// Defensive no-op engine used when dictation backend is unavailable.
#[derive(Debug, Default)]
pub struct NoopDictationEngine;

impl DictationEngine for NoopDictationEngine {
    fn transcribe(&self, audio_path: &str) -> Result<DictationResult, DictationError> {
        let path = std::path::Path::new(audio_path);
        if path.as_os_str().is_empty() || !path.exists() {
            return Err(DictationError::Missing);
        }

        // The production engine will actually perform transcription here.
        Ok(DictationResult::default())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn noop_engine_accepts_existing_path() {
        let engine = NoopDictationEngine::default();
        let tmp = std::env::temp_dir().join("nabu-dictation-noop-test.txt");
        fs::write(&tmp, b"placeholder").unwrap();

        let result = engine.transcribe(tmp.to_str().unwrap()).unwrap();
        assert!(result.text.is_empty());
        assert!(result.segments.is_empty());

        let _ = fs::remove_file(tmp);
    }

    #[test]
    fn noop_engine_rejects_missing_path() {
        let engine = NoopDictationEngine::default();
        let err = engine.transcribe("/tmp/nabu-dictation-missing").unwrap_err();
        assert!(matches!(err, DictationError::Missing));
    }
}
