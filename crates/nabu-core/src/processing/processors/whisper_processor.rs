use std::path::PathBuf;

use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{CustomPropertyValue, ObjectContent, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Processes audio content through the local Whisper speech-to-text engine
/// ([`crate::native::whisper`]). No simulated transcription exists.
///
/// The model file is resolved from the `NABU_WHISPER_MODEL` environment
/// variable, falling back to `resources/whisper-models/ggml-base.en.bin`.
/// When no model is available the object is returned unmodified with an
/// error message — the pipeline never fabricates text.
pub struct WhisperProcessor;

fn resolve_model_path() -> PathBuf {
    std::env::var("NABU_WHISPER_MODEL")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("resources/whisper-models/ggml-base.en.bin"))
}

#[async_trait]
impl Processor for WhisperProcessor {
    fn name(&self) -> &'static str {
        "whisper_processor"
    }

    async fn process(
        &self,
        context: &ProcessingContext,
        progress: ProgressReporter,
        cancellation: CancellationToken,
    ) -> ProcessingResult {
        if cancellation.is_cancelled() {
            return ProcessingResult::unmodified(context.object.clone());
        }

        progress.set_progress(0.1);

        // Only process audio recordings.
        if context.object.object_type != ObjectType::AudioRecording {
            return ProcessingResult::unmodified(context.object.clone());
        }

        let audio_data = match &context.object.content {
            ObjectContent::Binary { data, .. } => data.clone(),
            _ => return ProcessingResult::unmodified(context.object.clone()),
        };

        progress.set_progress(0.3);
        let mut object = context.object.clone();

        let model_path = resolve_model_path();

        let engine_result = tokio::task::spawn_blocking(move || {
            crate::native::whisper::transcribe(&model_path, &audio_data)
        })
        .await;

        let transcription = match engine_result {
            Ok(Ok(t)) => t,
            Ok(Err(e)) => {
                tracing::warn!(
                    subsystem = "processing",
                    component = "whisper_processor",
                    object_id = %object.id,
                    error = %e,
                    "Whisper transcription unavailable"
                );
                return ProcessingResult {
                    object,
                    modified: false,
                    metadata: std::collections::HashMap::new(),
                    error: Some(e.to_string()),
                    diagnostics: Vec::new(),
                    stats: crate::processing::processor::ProcessingStats::new(),
                    status: crate::processing::processor::ExecutionStatus::Failed,
                };
            }
            Err(_) => return ProcessingResult::unmodified(object),
        };

        progress.set_progress(0.7);

        object.custom_properties.insert(
            "transcription_text".to_string(),
            CustomPropertyValue::Text(transcription.text.clone()),
        );

        object.custom_properties.insert(
            "transcription_confidence".to_string(),
            CustomPropertyValue::Number(transcription.confidence),
        );

        object.custom_properties.insert(
            "transcription_duration_ms".to_string(),
            CustomPropertyValue::Number(transcription.duration_ms as f64),
        );

        // Use transcription as description
        if object.metadata.description.is_none() {
            object.metadata.description = Some(transcription.text);
        }

        progress.set_progress(1.0);
        ProcessingResult::new(object)
    }

    fn supports(&self, object_type: &ObjectType) -> bool {
        matches!(object_type, ObjectType::AudioRecording)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::KnowledgeObject;

    #[tokio::test]
    async fn test_whisper_skips_non_audio() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown("Hello".to_string()),
        );

        let ctx = ProcessingContext::new(obj);
        let processor = WhisperProcessor;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert!(!result.modified);
    }

    #[tokio::test]
    async fn test_whisper_audio_without_model_is_graceful() {
        // No NABU_WHISPER_MODEL and no bundled model: the real engine must
        // report a model-not-found error instead of fabricating text. This
        // holds only while the default model file does not exist.
        let obj = KnowledgeObject::new(
            ObjectType::AudioRecording,
            ObjectContent::Binary {
                mime_type: "audio/wav".to_string(),
                data: Vec::new(),
                filename: Some("recording.wav".to_string()),
            },
        );

        let ctx = ProcessingContext::new(obj);
        let processor = WhisperProcessor;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        if cfg!(target_os = "macos") {
            // Either a real model is configured (then text is produced) or the
            // engine reports the missing model. Never simulated text.
            if std::env::var("NABU_WHISPER_MODEL").is_err()
                && !PathBuf::from("resources/whisper-models/ggml-base.en.bin").exists()
            {
                assert!(
                    !result
                        .object
                        .custom_properties
                        .contains_key("transcription_text"),
                    "no simulated transcription allowed"
                );
                assert!(result.error.is_some() || !result.modified);
            }
        } else {
            assert!(!result.modified);
        }
    }
}
