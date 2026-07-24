pub struct AudioEngine;
impl AudioEngine {
    pub fn new(_model_path: &str) -> anyhow::Result<Self> { Ok(Self) }
    pub fn transcribe(&self, _audio_data: &[f32]) -> anyhow::Result<String> { Ok("".into()) }
}
