use thiserror::Error;

/// Errors produced by the native macOS engines.
#[derive(Debug, Error)]
pub enum NativeError {
    /// The native engine requires macOS and is unavailable on this platform.
    #[error("native engine unavailable on this platform (requires macOS)")]
    UnsupportedPlatform,

    /// The supplied input bytes could not be interpreted by the engine.
    #[error("invalid input data: {0}")]
    InvalidData(String),

    /// The underlying framework call failed.
    #[error("native call failed: {0}")]
    CallFailed(String),

    /// The requested model file does not exist.
    #[error("whisper model not found: {0}")]
    ModelNotFound(String),

    /// The audio data is not in a supported format (expected WAV PCM/float).
    #[error("unsupported audio format: {0}")]
    UnsupportedAudio(String),
}
