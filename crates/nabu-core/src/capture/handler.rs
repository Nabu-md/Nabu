use crate::models::{CaptureSource, KnowledgeObject, ObjectType};
use async_trait::async_trait;

/// Result of a capture operation.
#[derive(Debug, Clone)]
pub struct CaptureResult {
    /// The captured KnowledgeObject
    pub object: KnowledgeObject,
    /// Source of the capture
    pub source: CaptureSource,
    /// Whether this should be enqueued for async processing
    pub enqueue: bool,
}

impl CaptureResult {
    pub fn new(object: KnowledgeObject, source: CaptureSource) -> Self {
        Self {
            object,
            source,
            enqueue: true,
        }
    }

    pub fn with_no_enqueue(mut self) -> Self {
        self.enqueue = false;
        self
    }
}

/// The CaptureHandler trait — implemented by all capture sources.
///
/// Each handler knows how to create a KnowledgeObject from its source data.
/// Handlers do NOT process, store, or index — they only produce KnowledgeObjects.
/// Processing is handled by the ProcessingPipeline via the Job Queue.
#[async_trait]
pub trait CaptureHandler: Send + Sync {
    /// The name identifier of this handler
    fn name(&self) -> &'static str;

    /// The capture source type
    fn source(&self) -> CaptureSource;

    /// Capture content and return a KnowledgeObject.
    /// Returns None if this handler cannot handle the given request.
    async fn capture(&self, request: &CaptureRequest) -> Option<CaptureResult>;
}

/// A request to capture content from a source.
#[derive(Debug, Clone)]
pub struct CaptureRequest {
    /// Raw data from the source
    pub data: CaptureData,
    /// Optional metadata hint
    pub title: Option<String>,
    /// Optional source URL
    pub source_url: Option<String>,
    /// Optional content type hint
    pub mime_type: Option<String>,
}

impl CaptureRequest {
    pub fn new(data: CaptureData) -> Self {
        Self {
            data,
            title: None,
            source_url: None,
            mime_type: None,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.source_url = Some(url.into());
        self
    }
}

/// Raw data from a capture source.
#[derive(Debug, Clone)]
pub enum CaptureData {
    /// Text content (Markdown, plain text, HTML)
    Text(String),
    /// URL reference
    Uri(String),
    /// Binary data (images, audio, PDFs)
    Binary { mime_type: String, data: Vec<u8>, filename: Option<String> },
    /// Existing file path
    File(String),
}

/// Handles browser capture (pages, articles, YouTube, GitHub).
pub struct BrowserCaptureHandler;

#[async_trait]
impl CaptureHandler for BrowserCaptureHandler {
    fn name(&self) -> &'static str {
        "browser"
    }

    fn source(&self) -> CaptureSource {
        CaptureSource::Browser
    }

    async fn capture(&self, request: &CaptureRequest) -> Option<CaptureResult> {
        let object = create_text_object(request, ObjectType::Article)?;
        Some(CaptureResult::new(object, CaptureSource::Browser))
    }
}

/// Handles clipboard capture (text, URLs, images).
pub struct ClipboardHandler;

#[async_trait]
impl CaptureHandler for ClipboardHandler {
    fn name(&self) -> &'static str {
        "clipboard"
    }

    fn source(&self) -> CaptureSource {
        CaptureSource::Clipboard
    }

    async fn capture(&self, request: &CaptureRequest) -> Option<CaptureResult> {
        let object = create_text_object(request, ObjectType::Note)?;
        Some(CaptureResult::new(object, CaptureSource::Clipboard))
    }
}

/// Handles screenshot capture.
pub struct ScreenshotHandler;

#[async_trait]
impl CaptureHandler for ScreenshotHandler {
    fn name(&self) -> &'static str {
        "screenshot"
    }

    fn source(&self) -> CaptureSource {
        CaptureSource::Screenshot
    }

    async fn capture(&self, request: &CaptureRequest) -> Option<CaptureResult> {
        let object = create_binary_object(request, "image/png", ObjectType::Screenshot)?;
        Some(CaptureResult::new(object, CaptureSource::Screenshot))
    }
}

/// Handles file drop capture.
pub struct FileDropHandler;

#[async_trait]
impl CaptureHandler for FileDropHandler {
    fn name(&self) -> &'static str {
        "file_drop"
    }

    fn source(&self) -> CaptureSource {
        CaptureSource::FileDrop
    }

    async fn capture(&self, request: &CaptureRequest) -> Option<CaptureResult> {
        let object = create_binary_object(request, "application/octet-stream", ObjectType::Attachment)?;
        Some(CaptureResult::new(object, CaptureSource::FileDrop))
    }
}

/// Handles watch folder capture.
pub struct WatchFolderHandler;

#[async_trait]
impl CaptureHandler for WatchFolderHandler {
    fn name(&self) -> &'static str {
        "watch_folder"
    }

    fn source(&self) -> CaptureSource {
        CaptureSource::WatchFolder
    }

    async fn capture(&self, request: &CaptureRequest) -> Option<CaptureResult> {
        let object = create_text_object(request, ObjectType::Note)?;
        Some(CaptureResult::new(object, CaptureSource::WatchFolder))
    }
}

/// Handles Safari Reader capture.
pub struct SafariReaderHandler;

#[async_trait]
impl CaptureHandler for SafariReaderHandler {
    fn name(&self) -> &'static str {
        "safari_reader"
    }

    fn source(&self) -> CaptureSource {
        CaptureSource::SafariReader
    }

    async fn capture(&self, request: &CaptureRequest) -> Option<CaptureResult> {
        let object = create_text_object(request, ObjectType::Article)?;
        Some(CaptureResult::new(object, CaptureSource::SafariReader))
    }
}

/// Handles YouTube capture.
pub struct YouTubeCaptureHandler;

#[async_trait]
impl CaptureHandler for YouTubeCaptureHandler {
    fn name(&self) -> &'static str {
        "youtube"
    }

    fn source(&self) -> CaptureSource {
        CaptureSource::YouTube
    }

    async fn capture(&self, request: &CaptureRequest) -> Option<CaptureResult> {
        match &request.data {
            CaptureData::Uri(url) => {
                let mut object = KnowledgeObject::new(
                    ObjectType::YouTubeVideo,
                    crate::models::ObjectContent::Uri(url.clone()),
                );
                object.metadata.title = request.title.clone();
                object.metadata.source_url = Some(url.clone());
                Some(CaptureResult::new(object, CaptureSource::YouTube))
            }
            _ => None,
        }
    }
}

/// Handles GitHub repository capture.
pub struct GitHubRepositoryHandler;

#[async_trait]
impl CaptureHandler for GitHubRepositoryHandler {
    fn name(&self) -> &'static str {
        "github"
    }

    fn source(&self) -> CaptureSource {
        CaptureSource::GitHub
    }

    async fn capture(&self, request: &CaptureRequest) -> Option<CaptureResult> {
        match &request.data {
            CaptureData::Uri(url) => {
                let mut object = KnowledgeObject::new(
                    ObjectType::Repository,
                    crate::models::ObjectContent::Uri(url.clone()),
                );
                object.metadata.title = request.title.clone();
                object.metadata.source_url = Some(url.clone());
                Some(CaptureResult::new(object, CaptureSource::GitHub))
            }
            _ => None,
        }
    }
}

// Helper functions

fn create_text_object(request: &CaptureRequest, object_type: ObjectType) -> Option<KnowledgeObject> {
    match &request.data {
        CaptureData::Text(text) => {
            let content = if text.contains("```") || text.starts_with('#') {
                crate::models::ObjectContent::Markdown(text.clone())
            } else if text.starts_with("<!DOCTYPE") || text.starts_with("<html") {
                crate::models::ObjectContent::RichHtml(text.clone())
            } else {
                crate::models::ObjectContent::PlainText(text.clone())
            };

            let mut object = KnowledgeObject::new(object_type, content);
            object.metadata.title = request.title.clone();
            object.metadata.source_url = request.source_url.clone();
            object.metadata.mime_type = request.mime_type.clone();
            Some(object)
        }
        CaptureData::Uri(url) => {
            let mut object = KnowledgeObject::new(object_type, crate::models::ObjectContent::Uri(url.clone()));
            object.metadata.title = request.title.clone();
            object.metadata.source_url = request.source_url.clone();
            Some(object)
        }
        _ => None,
    }
}

fn create_binary_object(
    request: &CaptureRequest,
    default_mime: &str,
    object_type: ObjectType,
) -> Option<KnowledgeObject> {
    match &request.data {
        CaptureData::Binary { mime_type, data, filename } => {
            let mut object = KnowledgeObject::new(
                object_type,
                crate::models::ObjectContent::Binary {
                    mime_type: mime_type.clone(),
                    data: data.clone(),
                    filename: filename.clone(),
                },
            );
            object.metadata.title = request.title.clone();
            object.metadata.source_url = request.source_url.clone();
            object.metadata.mime_type = Some(mime_type.clone());
            object.metadata.original_filename = filename.clone();
            Some(object)
        }
        CaptureData::File(path) => {
            let filename = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());
            let mut object = KnowledgeObject::new(
                object_type,
                crate::models::ObjectContent::Binary {
                    mime_type: default_mime.to_string(),
                    data: Vec::new(),
                    filename: filename.clone(),
                },
            );
            object.metadata.title = request.title.clone();
            object.metadata.original_filename = filename;
            Some(object)
        }
        _ => None,
    }
}
