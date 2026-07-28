use std::collections::HashMap;
use std::sync::Arc;

use crate::capture::{CaptureError, IngestionRequest, IngestionResult, IngestionStatus};
use crate::event_bus::{
    EVENT_ITEM_CAPTURED, EVENT_ITEM_PROCESSED, EventBus, ItemCaptured, ItemProcessed,
};
use crate::models::knowledge_object::{KnowledgeObject, ObjectContent, ObjectMetadata, ObjectType};

/// Transforms a normalized [`IngestionRequest`] into a [`KnowledgeObject`].
///
/// The pipeline is responsible for:
/// - determining the object type from MIME type
/// - selecting the appropriate content format
/// - populating base metadata available at ingestion time
///
/// No enrichment, parsing, or processing occurs here. Those responsibilities
/// belong to downstream processors.
///
/// The pipeline is stateless and may be instantiated once and reused for all
/// ingestion operations.
pub struct IngestionPipeline;

impl IngestionPipeline {
    /// Creates a new ingestion pipeline and registers it as a subscriber
    /// on the provided event bus.
    ///
    /// The pipeline subscribes to [`ItemCaptured`] events and publishes
    /// [`ItemProcessed`] events when processing completes.
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        let bus = event_bus.clone();
        event_bus.subscribe(EVENT_ITEM_CAPTURED, move |event: &ItemCaptured| {
            let request: IngestionRequest = event.into();
            let pipeline = IngestionPipeline;
            if let Ok(result) = pipeline.process_with_id(request, event.id) {
                if let Some(obj) = result.knowledge_object {
                    let processed = ItemProcessed::from_knowledge_object(&obj, result.warnings);
                    bus.publish(EVENT_ITEM_PROCESSED, &processed);
                }
            }
        });

        Self
    }

    /// Processes an [`IngestionRequest`] and returns an [`IngestionResult`].
    ///
    /// # Errors
    ///
    /// Returns [`CaptureError`] if the request cannot be transformed into a
    /// valid [`KnowledgeObject`]. Currently, the pipeline never fails under
    /// normal operation; all MIME types map to a valid `ObjectType` and
    /// `ObjectContent`.
    pub fn process(&self, request: IngestionRequest) -> Result<IngestionResult, CaptureError> {
        self.process_with_id(request, uuid::Uuid::new_v4())
    }

    /// Processes an [`IngestionRequest`] with a specific ID and returns an [`IngestionResult`].
    ///
    /// This is used by the event bus pipeline to preserve the capture ID
    /// across the ItemCaptured → ItemProcessed lifecycle.
    pub fn process_with_id(
        &self,
        request: IngestionRequest,
        id: uuid::Uuid,
    ) -> Result<IngestionResult, CaptureError> {
        let object_type = self.determine_object_type(&request.mime_type);
        let content = self.determine_content(&request.mime_type, &request.raw_bytes);
        let metadata = self.build_metadata(&request);
        let mut warnings = Vec::new();

        // Warn if JSON parsing fell back to plain text
        if request.mime_type == "application/json" && matches!(content, ObjectContent::PlainText) {
            warnings.push("JSON content was invalid; stored as plain text".to_string());
        }

        let knowledge_object = KnowledgeObject {
            id,
            object_type,
            vault_id: request.vault_id.clone(),
            created_at: Self::current_timestamp(),
            modified_at: Self::current_timestamp(),
            content,
            metadata,
        };

        Ok(IngestionResult {
            knowledge_object: Some(knowledge_object),
            knowledge_object_id: Some(id),
            source: request.source,
            timestamp: Self::current_timestamp(),
            status: IngestionStatus::Success,
            warnings,
        })
    }

    fn determine_object_type(&self, mime: &str) -> ObjectType {
        if mime == "text/plain" || mime.starts_with("text/markdown") {
            ObjectType::Note
        } else if mime.starts_with("text/")
            || mime == "application/json"
            || mime == "application/xml"
            || mime == "text/csv"
        {
            ObjectType::Document
        } else if mime == "application/pdf" {
            ObjectType::Pdf
        } else if mime.starts_with("image/") {
            ObjectType::Image
        } else if mime.starts_with("audio/") {
            ObjectType::AudioRecording
        } else if mime.starts_with("video/") {
            ObjectType::Video
        } else {
            ObjectType::Attachment
        }
    }

    fn determine_content(&self, mime: &str, raw_bytes: &[u8]) -> ObjectContent {
        if mime == "text/markdown" {
            ObjectContent::Markdown
        } else if mime.starts_with("text/") && mime != "text/html" {
            ObjectContent::PlainText
        } else if mime == "text/html" {
            ObjectContent::Html
        } else if mime == "application/json" {
            match serde_json::from_slice(raw_bytes) {
                Ok(value) => ObjectContent::Structured(value),
                Err(_) => ObjectContent::PlainText,
            }
        } else {
            ObjectContent::Binary
        }
    }

    fn build_metadata(&self, request: &IngestionRequest) -> ObjectMetadata {
        let title = request
            .source_file
            .as_ref()
            .and_then(|path| std::path::Path::new(path).file_stem())
            .and_then(|stem| stem.to_str())
            .map(|s| s.to_string());

        ObjectMetadata {
            title,
            author: None,
            language: None,
            source_url: None,
            source_file: request.source_file.clone(),
            mime_type: Some(request.mime_type.clone()),
            page_count: None,
            word_count: None,
            created: Some(Self::current_timestamp()),
            modified: Some(Self::current_timestamp()),
            custom: HashMap::new(),
        }
    }

    fn current_timestamp() -> String {
        let now = std::time::SystemTime::now();
        let duration = now
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_else(|_| {
                // Fallback for systems with clock issues; should never happen in practice.
                std::time::Duration::from_secs(0)
            });
        let secs = duration.as_secs();
        let millis = duration.subsec_millis();
        format!("{}.{:03}Z", secs, millis)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::IngestionOptions;
    use crate::event_bus::EventBus;

    #[test]
    fn process_text_file_creates_note() {
        let bus = Arc::new(EventBus::new());
        let pipeline = IngestionPipeline::new(bus);
        let request = IngestionRequest {
            source: "file_drop".to_string(),
            raw_bytes: b"Hello, world!".to_vec(),
            mime_type: "text/plain".to_string(),
            vault_id: "vault-1".to_string(),
            source_file: Some("/path/to/hello.txt".to_string()),
            options: IngestionOptions::default(),
        };

        let result = pipeline.process(request).unwrap();

        assert_eq!(result.status, IngestionStatus::Success);
        let obj = result.knowledge_object.unwrap();
        assert_eq!(obj.object_type, ObjectType::Note);
        assert_eq!(obj.content, ObjectContent::PlainText);
        assert_eq!(obj.vault_id, "vault-1");
        assert_eq!(obj.metadata.title, Some("hello".to_string()));
        assert_eq!(
            obj.metadata.source_file,
            Some("/path/to/hello.txt".to_string())
        );
        assert_eq!(obj.metadata.mime_type, Some("text/plain".to_string()));
    }

    #[test]
    fn process_markdown_file_creates_note_with_markdown_content() {
        let bus = Arc::new(EventBus::new());
        let pipeline = IngestionPipeline::new(bus);
        let request = IngestionRequest {
            source: "file_drop".to_string(),
            raw_bytes: b"# Title\n\nBody".to_vec(),
            mime_type: "text/markdown".to_string(),
            vault_id: "vault-1".to_string(),
            source_file: Some("/path/to/note.md".to_string()),
            options: IngestionOptions::default(),
        };

        let result = pipeline.process(request).unwrap();

        let obj = result.knowledge_object.unwrap();
        assert_eq!(obj.object_type, ObjectType::Note);
        assert_eq!(obj.content, ObjectContent::Markdown);
        assert_eq!(obj.metadata.title, Some("note".to_string()));
    }

    #[test]
    fn process_pdf_creates_pdf_object() {
        let bus = Arc::new(EventBus::new());
        let pipeline = IngestionPipeline::new(bus);
        let request = IngestionRequest {
            source: "file_drop".to_string(),
            raw_bytes: b"%PDF-1.4 fake".to_vec(),
            mime_type: "application/pdf".to_string(),
            vault_id: "vault-1".to_string(),
            source_file: Some("/path/to/doc.pdf".to_string()),
            options: IngestionOptions::default(),
        };

        let result = pipeline.process(request).unwrap();

        let obj = result.knowledge_object.unwrap();
        assert_eq!(obj.object_type, ObjectType::Pdf);
        assert_eq!(obj.content, ObjectContent::Binary);
    }

    #[test]
    fn process_image_creates_image_object() {
        let bus = Arc::new(EventBus::new());
        let pipeline = IngestionPipeline::new(bus);
        let request = IngestionRequest {
            source: "file_drop".to_string(),
            raw_bytes: b"\x89PNG\r\n\x1a\n".to_vec(),
            mime_type: "image/png".to_string(),
            vault_id: "vault-1".to_string(),
            source_file: Some("/path/to/img.png".to_string()),
            options: IngestionOptions::default(),
        };

        let result = pipeline.process(request).unwrap();

        let obj = result.knowledge_object.unwrap();
        assert_eq!(obj.object_type, ObjectType::Image);
        assert_eq!(obj.content, ObjectContent::Binary);
    }

    #[test]
    fn process_audio_creates_audio_recording_object() {
        let bus = Arc::new(EventBus::new());
        let pipeline = IngestionPipeline::new(bus);
        let request = IngestionRequest {
            source: "file_drop".to_string(),
            raw_bytes: b"ID3".to_vec(),
            mime_type: "audio/mpeg".to_string(),
            vault_id: "vault-1".to_string(),
            source_file: Some("/path/to/song.mp3".to_string()),
            options: IngestionOptions::default(),
        };

        let result = pipeline.process(request).unwrap();

        let obj = result.knowledge_object.unwrap();
        assert_eq!(obj.object_type, ObjectType::AudioRecording);
    }

    #[test]
    fn process_video_creates_video_object() {
        let bus = Arc::new(EventBus::new());
        let pipeline = IngestionPipeline::new(bus);
        let request = IngestionRequest {
            source: "file_drop".to_string(),
            raw_bytes: b"\x00\x00\x00\x20ftyp".to_vec(),
            mime_type: "video/mp4".to_string(),
            vault_id: "vault-1".to_string(),
            source_file: Some("/path/to/clip.mp4".to_string()),
            options: IngestionOptions::default(),
        };

        let result = pipeline.process(request).unwrap();

        let obj = result.knowledge_object.unwrap();
        assert_eq!(obj.object_type, ObjectType::Video);
    }

    #[test]
    fn process_json_creates_document_with_structured_content() {
        let bus = Arc::new(EventBus::new());
        let pipeline = IngestionPipeline::new(bus);
        let request = IngestionRequest {
            source: "file_drop".to_string(),
            raw_bytes: b"{\"key\": \"value\"}".to_vec(),
            mime_type: "application/json".to_string(),
            vault_id: "vault-1".to_string(),
            source_file: Some("/path/to/data.json".to_string()),
            options: IngestionOptions::default(),
        };

        let result = pipeline.process(request).unwrap();

        let obj = result.knowledge_object.unwrap();
        assert_eq!(obj.object_type, ObjectType::Document);
        assert!(matches!(obj.content, ObjectContent::Structured(_)));
    }

    #[test]
    fn process_html_creates_document_with_html_content() {
        let bus = Arc::new(EventBus::new());
        let pipeline = IngestionPipeline::new(bus);
        let request = IngestionRequest {
            source: "file_drop".to_string(),
            raw_bytes: b"<html><body>Hello</body></html>".to_vec(),
            mime_type: "text/html".to_string(),
            vault_id: "vault-1".to_string(),
            source_file: Some("/path/to/page.html".to_string()),
            options: IngestionOptions::default(),
        };

        let result = pipeline.process(request).unwrap();

        let obj = result.knowledge_object.unwrap();
        assert_eq!(obj.object_type, ObjectType::Document);
        assert_eq!(obj.content, ObjectContent::Html);
    }

    #[test]
    fn process_unknown_mime_creates_attachment() {
        let bus = Arc::new(EventBus::new());
        let pipeline = IngestionPipeline::new(bus);
        let request = IngestionRequest {
            source: "file_drop".to_string(),
            raw_bytes: b"\x00\x01\x02\x03".to_vec(),
            mime_type: "application/octet-stream".to_string(),
            vault_id: "vault-1".to_string(),
            source_file: Some("/path/to/data.bin".to_string()),
            options: IngestionOptions::default(),
        };

        let result = pipeline.process(request).unwrap();

        let obj = result.knowledge_object.unwrap();
        assert_eq!(obj.object_type, ObjectType::Attachment);
        assert_eq!(obj.content, ObjectContent::Binary);
    }

    #[test]
    fn process_sets_timestamps() {
        let bus = Arc::new(EventBus::new());
        let pipeline = IngestionPipeline::new(bus);
        let request = IngestionRequest {
            source: "file_drop".to_string(),
            raw_bytes: b"test".to_vec(),
            mime_type: "text/plain".to_string(),
            vault_id: "vault-1".to_string(),
            source_file: None,
            options: IngestionOptions::default(),
        };

        let result = pipeline.process(request).unwrap();

        let obj = result.knowledge_object.unwrap();
        assert!(!obj.created_at.is_empty());
        assert!(!obj.modified_at.is_empty());
        assert_eq!(obj.created_at, obj.modified_at);
    }

    #[test]
    fn process_without_source_file_has_no_title() {
        let bus = Arc::new(EventBus::new());
        let pipeline = IngestionPipeline::new(bus);
        let request = IngestionRequest {
            source: "file_drop".to_string(),
            raw_bytes: b"test".to_vec(),
            mime_type: "text/plain".to_string(),
            vault_id: "vault-1".to_string(),
            source_file: None,
            options: IngestionOptions::default(),
        };

        let result = pipeline.process(request).unwrap();

        let obj = result.knowledge_object.unwrap();
        assert_eq!(obj.metadata.title, None);
        assert_eq!(obj.metadata.source_file, None);
    }

    #[test]
    fn process_invalid_json_warns_and_falls_back_to_plain_text() {
        let bus = Arc::new(EventBus::new());
        let pipeline = IngestionPipeline::new(bus);
        let request = IngestionRequest {
            source: "file_drop".to_string(),
            raw_bytes: b"{invalid json".to_vec(),
            mime_type: "application/json".to_string(),
            vault_id: "vault-1".to_string(),
            source_file: Some("/path/to/bad.json".to_string()),
            options: IngestionOptions::default(),
        };

        let result = pipeline.process(request).unwrap();

        assert_eq!(result.status, IngestionStatus::Success);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("JSON content was invalid"));
        let obj = result.knowledge_object.unwrap();
        assert_eq!(obj.content, ObjectContent::PlainText);
    }
}
