use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::ObjectType;
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Processes audio content through Whisper speech-to-text.
///
/// Currently a stub that simulates Whisper transcription.
/// In production, this would call Whisper.cpp or the macOS SFSpeechRecognizer.
///
/// The transcription result:
/// - Populates `transcription_text` custom property
/// - Sets `transcription_confidence` metadata
/// - Stores `transcription_duration_ms` for timing
pub struct WhisperProcessor;

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

        // Only process audio recordings
        if context.object.object_type != ObjectType::AudioRecording {
            return ProcessingResult::unmodified(context.object.clone());
        }

        progress.set_progress(0.3);
        let mut object = context.object.clone();

        // Simulate Whisper transcription
        let transcription = "This is a simulated Whisper transcription of the audio recording. \
            In production, this would be the output of Whisper.cpp or the macOS speech recognizer.";

        progress.set_progress(0.7);

        object.custom_properties.insert(
            "transcription_text".to_string(),
            crate::models::CustomPropertyValue::Text(transcription.to_string()),
        );

        object.custom_properties.insert(
            "transcription_confidence".to_string(),
            crate::models::CustomPropertyValue::Number(0.92),
        );

        object.custom_properties.insert(
            "transcription_duration_ms".to_string(),
            crate::models::CustomPropertyValue::Number(1500.0), // simulated 1.5s
        );

        // Use transcription as description
        if object.metadata.description.is_none() {
            object.metadata.description = Some(transcription.to_string());
        }

        object.metadata.mime_type = Some("audio/mp3".to_string());

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

    #[tokio::test]
    async fn test_whisper_transcription() {
        let obj = KnowledgeObject::new(
            ObjectType::AudioRecording,
            ObjectContent::Binary {
                mime_type: "audio/mp3".to_string(),
                data: vec![0, 1, 2, 3],
                filename: Some("recording.mp3".to_string()),
            },
        );

        let ctx = ProcessingContext::new(obj);
        let processor = WhisperProcessor;
        let result = processor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert!(result
            .object
            .custom_properties
            .contains_key("transcription_text"));
        assert!(result
            .object
            .custom_properties
            .contains_key("transcription_confidence"));
    }

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

        assert_eq!(result.modified, false);
    }
}
