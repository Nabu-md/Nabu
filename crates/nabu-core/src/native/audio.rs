use anyhow::{Result, Context};
use whisper_rs::{WhisperContext, FullParams, SamplingStrategy};

pub struct AudioEngine {
    context: WhisperContext,
}

impl AudioEngine {
    pub fn new(model_path: &str) -> Result<Self> {
        let context = WhisperContext::init_from_file(model_path)
            .map_err(|_| anyhow::anyhow!("Failed to load whisper model: {}", model_path))?;
        Ok(Self { context })
    }

    pub fn transcribe(&self, audio_data: &[f32]) -> Result<String> {
        let mut state = self.context.create_state().context("Failed to create state")?;
        
        let params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        state.full(params, audio_data).context("Transcription failed")?;
        
        let num_segments = state.full_n_segments().context("Failed to get segments")?;
        let mut transcription = String::new();
        
        for i in 0..num_segments {
            transcription.push_str(&state.full_get_segment_text(i).context("Failed to get segment text")?);
        }
        
        Ok(transcription)
    }
}
