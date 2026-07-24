pub struct DictationEngine;

impl DictationEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn start_dictation(&self) -> anyhow::Result<String> {
        // Implementation for Whisper.cpp dictation
        Ok("Dictation stub".to_string())
    }
}
