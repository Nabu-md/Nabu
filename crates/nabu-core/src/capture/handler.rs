use crate::models::{CaptureSource, KnowledgeObject, ObjectContent, ObjectMetadata, ObjectType};
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

    pub fn with_mime_type(mut self, mime: impl Into<String>) -> Self {
        self.mime_type = Some(mime.into());
        self
    }

    /// Attach binary content (images, audio, PDFs) to this request.
    /// Convenience for callers that already have raw bytes.
    pub fn with_binary(
        mut self,
        mime_type: impl Into<String>,
        data: Vec<u8>,
        filename: Option<String>,
    ) -> Self {
        self.data = CaptureData::Binary {
            mime_type: mime_type.into(),
            data,
            filename,
        };
        self
    }

    /// Returns the URL string if this request carries a `CaptureData::Uri`,
    /// otherwise falls back to `source_url`. Used by routing handlers.
    pub fn url(&self) -> Option<&str> {
        match &self.data {
            CaptureData::Uri(url) => Some(url),
            _ => self.source_url.as_deref(),
        }
    }

    /// Whether this request's data is a URL (either `CaptureData::Uri` or a
    /// `Text` payload that parses as a bare URL).
    pub fn is_url(&self) -> bool {
        match &self.data {
            CaptureData::Uri(_) => true,
            CaptureData::Text(t) => is_url(t),
            _ => false,
        }
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
    Binary {
        mime_type: String,
        data: Vec<u8>,
        filename: Option<String>,
    },
    /// Existing file path
    File(String),
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Cheap URL scheme detection — used to distinguish a bare URL copied as text
/// from regular note text. Mirrors the detection in the MetadataExtractor and
/// the native messaging socket so clipboard/URL capture is consistent.
fn is_url(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("ftp://")
        || trimmed.starts_with("file://")
}

/// Extract the domain (without `www.`) from a URL, if present.
fn extract_domain(url: &str) -> Option<String> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re =
        RE.get_or_init(|| regex::Regex::new(r"https?://(?:www\.)?([^/]+)").expect("valid regex"));
    re.captures(url).map(|cap| cap[1].to_string())
}

/// True when the URL points at a YouTube watch or short link.
fn is_youtube_url(url: &str) -> bool {
    if let Some(domain) = extract_domain(url) {
        (domain == "youtube.com"
            || domain == "youtu.be"
            || domain == "m.youtube.com"
            || domain == "www.youtube.com")
            && (url.contains("/watch")
                || url.contains("/shorts")
                || url.contains("/embed")
                || url.contains("/v/"))
    } else {
        false
    }
}

/// True when the URL points at a GitHub repository or issue.
fn is_github_url(url: &str) -> bool {
    if let Some(domain) = extract_domain(url) {
        domain == "github.com" || domain == "www.github.com"
    } else {
        false
    }
}

macro_rules! regex_lazy {
    ($pattern:expr) => {{
        use std::sync::OnceLock;
        static RE: OnceLock<regex::Regex> = OnceLock::new();
        RE.get_or_init(|| regex::Regex::new($pattern).unwrap())
    }};
}

// ── Handlers ─────────────────────────────────────────────────────────

/// Handles browser capture (pages, articles, YouTube, GitHub).
///
/// The handler routes requests by URL domain: YouTube URLs are delegated to
/// the [YouTubeCaptureHandler] logic, GitHub URLs to the
/// [GitHubRepositoryHandler] logic, and everything else becomes a Bookmark or
/// Article depending on whether the request carries text content.
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
        // Route by URL domain for known special cases.
        if let Some(url) = request.url() {
            if is_youtube_url(url) {
                return YouTubeCaptureHandler.capture(request).await;
            }
            if is_github_url(url) {
                return GitHubRepositoryHandler.capture(request).await;
            }
            // General URL from the browser → Bookmark.
            return BookmarkCaptureHandler.capture(request).await;
        }

        // Text-only browser capture (e.g. a copied article snippet) → Article.
        let object = create_text_object(request, ObjectType::Article)?;
        Some(CaptureResult::new(object, CaptureSource::Browser))
    }
}

/// Handles clipboard capture (text, URLs, images).
///
/// - A URL in text form is captured as a [Bookmark] ([CaptureSource::Url]).
/// - Binary image data is captured as a [Screenshot] ([CaptureSource::Clipboard]).
/// - Everything else becomes a [Note] ([CaptureSource::Clipboard]).
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
        match &request.data {
            // Clipboard text that is actually a URL → Bookmark.
            CaptureData::Text(text) if is_url(text) => {
                let url = text.trim().to_string();
                let mut object =
                    KnowledgeObject::new(ObjectType::Bookmark, ObjectContent::Uri(url.clone()));
                object.metadata.title = request.title.clone();
                object.metadata.source_url = Some(url);
                object.metadata.mime_type = request.mime_type.clone();
                Some(CaptureResult::new(object, CaptureSource::Url))
            }
            // Clipboard image/png data → Screenshot.
            CaptureData::Binary {
                mime_type, data, ..
            } if mime_type.starts_with("image/") => {
                let mut object = KnowledgeObject::new(
                    ObjectType::Screenshot,
                    ObjectContent::Binary {
                        mime_type: mime_type.clone(),
                        data: data.clone(),
                        filename: None,
                    },
                );
                object.metadata.title = request.title.clone();
                object.metadata.mime_type = Some(mime_type.clone());
                Some(CaptureResult::new(object, CaptureSource::Clipboard))
            }
            // Plain text → Note.
            _ => {
                let object = create_text_object(request, ObjectType::Note)?;
                Some(CaptureResult::new(object, CaptureSource::Clipboard))
            }
        }
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
        let object =
            create_binary_object(request, "application/octet-stream", ObjectType::Attachment)?;
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
///
/// Reader-mode HTML is stripped to readable article text/markdown and stored
/// as an [Article] object. The HTML→text conversion is handled by the
/// [`html_to_article`] helper so the extraction logic is not duplicated.
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
        ArticleCaptureHandler
            .handle(request, CaptureSource::SafariReader)
            .await
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
                let mut object =
                    KnowledgeObject::new(ObjectType::YouTubeVideo, ObjectContent::Uri(url.clone()));
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
                let mut object =
                    KnowledgeObject::new(ObjectType::Repository, ObjectContent::Uri(url.clone()));
                object.metadata.title = request.title.clone();
                object.metadata.source_url = Some(url.clone());
                Some(CaptureResult::new(object, CaptureSource::GitHub))
            }
            _ => None,
        }
    }
}

/// Handles generic URL / bookmark capture.
///
/// Captures a URL (text pasted into the command palette, browser extension
/// "bookmark" button, etc.) as a [Bookmark] object. The title is extracted
/// from the request or from the URL path by the metadata extractor downstream.
pub struct BookmarkCaptureHandler;

#[async_trait]
impl CaptureHandler for BookmarkCaptureHandler {
    fn name(&self) -> &'static str {
        "bookmark"
    }

    fn source(&self) -> CaptureSource {
        CaptureSource::Url
    }

    async fn capture(&self, request: &CaptureRequest) -> Option<CaptureResult> {
        match &request.data {
            CaptureData::Uri(url) => {
                let mut object =
                    KnowledgeObject::new(ObjectType::Bookmark, ObjectContent::Uri(url.clone()));
                object.metadata.title = request.title.clone();
                object.metadata.source_url = Some(url.clone());
                object.metadata.mime_type = request.mime_type.clone();
                Some(CaptureResult::new(object, CaptureSource::Url))
            }
            CaptureData::Text(text) if is_url(text) => {
                let url = text.trim().to_string();
                let mut object =
                    KnowledgeObject::new(ObjectType::Bookmark, ObjectContent::Uri(url.clone()));
                object.metadata.title = request.title.clone();
                object.metadata.source_url = Some(url);
                Some(CaptureResult::new(object, CaptureSource::Url))
            }
            _ => None,
        }
    }
}

/// Handles article capture from reader-mode / readability HTML.
///
/// This handler accepts either a `CaptureData::Text` (HTML or Markdown) or a
/// `CaptureData::Binary` (raw page bytes) and produces a [KnowledgeObject] of
/// type [Article]. HTML is converted to Markdown via [`html_to_article`] so the
/// downstream [MetadataExtractor] has plain text to parse.
///
/// Both [SafariReaderHandler] and [ArticleCaptureHandler] share the extraction
/// logic to avoid duplication.
pub struct ArticleCaptureHandler;

impl ArticleCaptureHandler {
    /// Shared extraction used by [ArticleCaptureHandler] and
    /// [SafariReaderHandler]. Produces an [Article] object from text or HTML.
    async fn handle(
        &self,
        request: &CaptureRequest,
        source: CaptureSource,
    ) -> Option<CaptureResult> {
        let text = match &request.data {
            CaptureData::Text(t) => Some(t.clone()),
            // HTML handed over as a binary blob — decode then extract.
            CaptureData::Binary { data, .. } => {
                let html = String::from_utf8_lossy(data).into_owned();
                if html.is_empty() {
                    return None;
                }
                Some(html)
            }
            _ => return None,
        };

        let text = text?;
        // Detect HTML and convert to article markdown; otherwise treat as
        // plain/markdown text directly.
        let content =
            if text.contains("<html") || text.contains("<body") || text.contains("<article") {
                let mut meta = ObjectMetadata::default();
                let markdown = html_to_article(&text, &mut meta);
                KnowledgeObject::new(ObjectType::Article, ObjectContent::Markdown(markdown))
                    .with_metadata({
                        let mut m = ObjectMetadata::default();
                        m.title = meta.title.or(request.title.clone());
                        m.source_url = meta.source_url.or(request.source_url.clone());
                        m
                    })
            } else {
                // Already text/markdown — use create_text_object's content detection.
                create_text_object(request, ObjectType::Article)?
            };

        let mut object = content;
        object.metadata.title = object.metadata.title.or(request.title.clone());
        object.metadata.source_url = request.source_url.clone();
        object.metadata.mime_type = request.mime_type.clone();
        Some(CaptureResult::new(object, source))
    }
}

#[async_trait]
impl CaptureHandler for ArticleCaptureHandler {
    fn name(&self) -> &'static str {
        "article"
    }

    fn source(&self) -> CaptureSource {
        CaptureSource::Article
    }

    async fn capture(&self, request: &CaptureRequest) -> Option<CaptureResult> {
        self.handle(request, CaptureSource::Article).await
    }
}

// ── Extraction helpers ────────────────────────────────────────────────

/// Minimal readability-style HTML → Markdown conversion.
///
/// This is a *fallback* reader — it strips boilerplate (scripts, styles,
/// navs, ads) and keeps the article body. It is intentionally lightweight:
/// it does not fetch the page (that happens upstream in the browser extension
/// which sends already-extracted reader HTML). The `metadata` out-parameter
/// receives a best-effort title and canonical URL if present in the HTML.
fn html_to_article(html: &str, metadata: &mut ObjectMetadata) -> String {
    // Extract <title>
    if metadata.title.is_none() {
        if let Some(title) = extract_tag_content(html, "title") {
            let title = title.trim().to_string();
            if !title.is_empty() {
                metadata.title = Some(title);
            }
        }
    }

    // Extract canonical URL
    if metadata.source_url.is_none() {
        if let Some(content) =
            extract_meta_property(html, "og:url").or_else(|| extract_meta_name(html, "canonical"))
        {
            let content = content.trim().to_string();
            if !content.is_empty() {
                metadata.source_url = Some(content);
            }
        }
    }

    // Extract description
    if metadata.description.is_none() {
        if let Some(content) = extract_meta_property(html, "og:description")
            .or_else(|| extract_meta_name_content(html, "description"))
        {
            let content = content.trim().to_string();
            if !content.is_empty() {
                metadata.description = Some(content);
            }
        }
    }

    // Try <article> first, then fall back to <body>.
    let body = extract_tag_content(html, "article")
        .or_else(|| extract_tag_content(html, "body"))
        .unwrap_or_else(|| html.to_string());

    // Strip boilerplate tags.
    let cleaned = strip_boilerplate_tags(&body);

    // Convert block-level HTML to Markdown.
    html_block_to_markdown(&cleaned)
}

/// Extract the inner text of the first occurrence of `tag`.
fn extract_tag_content(html: &str, tag: &str) -> Option<String> {
    let open = format!("<{}", tag);
    let close = format!("</{}", tag);
    let start = html.find(&open)?;
    // Skip past the opening tag's attributes.
    let tag_end = html[start..].find('>')? + start + 1;
    let end = html[tag_end..].find(&close)? + tag_end;
    Some(html[tag_end..end].to_string())
}

/// Extract a `<meta property="X" content="Y">` value.
fn extract_meta_property(html: &str, property: &str) -> Option<String> {
    let needle = format!(r#"property="{}""#, property);
    let start = html.find(&needle)?;
    let after = &html[start..];
    let content_start = after.find("content=")?;
    let quote = after.as_bytes()[content_start + 9];
    let q = quote as char;
    let content_start = content_start + 10;
    let content_end = after[content_start..].find(q)?;
    Some(after[content_start..content_start + content_end].to_string())
}

/// Extract a `<meta name="X" content="Y">` value.
fn extract_meta_name_content(html: &str, name: &str) -> Option<String> {
    let needle = format!(r#"name="{}""#, name);
    let start = html.find(&needle)?;
    let after = &html[start..];
    let content_start = after.find("content=")?;
    let quote = after.as_bytes()[content_start + 9];
    let q = quote as char;
    let content_start = content_start + 10;
    let content_end = after[content_start..].find(q)?;
    Some(after[content_start..content_start + content_end].to_string())
}

/// Extract content from `<meta name="canonical">` (uses `content=` attr).
fn extract_meta_name(html: &str, name: &str) -> Option<String> {
    extract_meta_name_content(html, name)
}

/// Remove script, style, nav, header, footer, and aside blocks.
fn strip_boilerplate_tags(html: &str) -> String {
    let block_tags = [
        "script", "style", "nav", "header", "footer", "aside", "noscript",
    ];
    let mut result = html.to_string();
    for tag in &block_tags {
        // Remove entire tag contents (including self-closing).
        loop {
            let open = format!("<{}", tag);
            let start = match result.find(&open) {
                Some(s) => s,
                None => break,
            };
            // Find the end of the opening tag.
            let tag_end = match result[start..].find('>') {
                Some(e) => start + e + 1,
                None => break,
            };
            // If it's self-closing or void-like, just remove the tag.
            let close = format!("</{}", tag);
            if let Some(end) = result[tag_end..].find(&close) {
                let end_abs = tag_end + end + close.len() + 2; // +2 for "</>"
                result.drain(start..end_abs.min(result.len()));
            } else {
                // No closing tag — remove just the opening tag.
                let end_abs = (start + tag_end - start).min(result.len());
                let mut remove_end = tag_end;
                // Skip to end of self-closing tag.
                if result[tag_end..].starts_with("/>") {
                    remove_end = tag_end + 2;
                }
                result.drain(start..remove_end.min(result.len()));
            }
        }
    }
    result
}

/// Convert HTML block/inline elements to Markdown text.
///
/// Each block-level tag is replaced by its inner text, prefixed and suffixed
/// with appropriate Markdown whitespace so paragraphs, headings, lists, etc.
/// round-trip cleanly.
fn html_block_to_markdown(html: &str) -> String {
    let mut text = html.to_string();

    // (tag, prefix, suffix) — prefix/suffix are applied around the inner text.
    let block_rules: &[(&str, &str, &str)] = &[
        ("div", "", "\n"),
        ("p", "\n\n", "\n"),
        ("br", "\n", ""),
        ("h1", "\n# ", "\n"),
        ("h2", "\n## ", "\n"),
        ("h3", "\n### ", "\n"),
        ("h4", "\n#### ", "\n"),
        ("li", "\n- ", "\n"),
        ("ul", "\n", "\n"),
        ("ol", "\n", "\n"),
        ("blockquote", "\n> ", "\n"),
        ("pre", "\n```\n", "\n```\n"),
        ("td", "", "\t"),
        ("tr", "", "\n"),
        ("strong", "**", "**"),
        ("b", "**", "**"),
        ("em", "*", "*"),
        ("i", "*", "*"),
        ("code", "`", "`"),
    ];

    for (tag, prefix, suffix) in block_rules {
        let open = format!("<{}", tag);
        let close = format!("</{}", tag);
        while let Some(start) = text.find(&open) {
            // Handle attributes on the opening tag.
            let after_open = &text[start..];
            let tag_end = match after_open.find('>') {
                Some(e) => e,
                None => break,
            };
            if let Some(end) = text[start + tag_end..].find(&close) {
                let end_abs = start + tag_end + end + close.len() + 2;
                let inner = text[start + tag_end + 1..start + tag_end + end].to_string();
                let replacement = format!("{}{}{}", prefix, inner, suffix);
                text.replace_range(start..end_abs, &replacement);
            } else {
                // Self-closing or unclosed — remove the opening tag.
                let end_abs = (start + tag_end + 1).min(text.len());
                text.drain(start..end_abs);
            }
        }
    }

    // Strip any remaining tags.
    let re_tags = regex_lazy!(r"<[^>]+>");
    let text = re_tags.replace_all(&text, "").to_string();

    // Decode common HTML entities.
    let text = text
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'");

    // Collapse excessive blank lines.
    let mut result = String::new();
    let mut blank_run = 0;
    for line in text.lines() {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run <= 2 {
                result.push('\n');
            }
        } else {
            blank_run = 0;
            result.push_str(line);
            result.push('\n');
        }
    }
    result.trim().to_string()
}

// ── Object creation helpers ──────────────────────────────────────────

fn create_text_object(
    request: &CaptureRequest,
    object_type: ObjectType,
) -> Option<KnowledgeObject> {
    match &request.data {
        CaptureData::Text(text) => {
            let content = if text.contains("```") || text.starts_with('#') {
                ObjectContent::Markdown(text.clone())
            } else if text.starts_with("<!DOCTYPE") || text.starts_with("<html") {
                ObjectContent::RichHtml(text.clone())
            } else {
                ObjectContent::PlainText(text.clone())
            };

            let mut object = KnowledgeObject::new(object_type, content);
            object.metadata.title = request.title.clone();
            object.metadata.source_url = request.source_url.clone();
            object.metadata.mime_type = request.mime_type.clone();
            Some(object)
        }
        CaptureData::Uri(url) => {
            let mut object = KnowledgeObject::new(object_type, ObjectContent::Uri(url.clone()));
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
        CaptureData::Binary {
            mime_type,
            data,
            filename,
        } => {
            let mut object = KnowledgeObject::new(
                object_type,
                ObjectContent::Binary {
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
                ObjectContent::Binary {
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

// ── Tests ─�───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_clipboard_text_becomes_note() {
        let handler = ClipboardHandler;
        let request = CaptureRequest::new(CaptureData::Text("Hello, world!".to_string()));
        let result = handler.capture(&request).await;
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.object.object_type, ObjectType::Note);
        assert_eq!(result.source, CaptureSource::Clipboard);
    }

    #[tokio::test]
    async fn test_clipboard_url_becomes_bookmark() {
        let handler = ClipboardHandler;
        let request =
            CaptureRequest::new(CaptureData::Text("https://example.com/article".to_string()));
        let result = handler.capture(&request).await.unwrap();
        assert_eq!(result.object.object_type, ObjectType::Bookmark);
        assert_eq!(result.source, CaptureSource::Url);
        assert_eq!(
            result.object.content,
            ObjectContent::Uri("https://example.com/article".to_string())
        );
    }

    #[tokio::test]
    async fn test_clipboard_image_becomes_screenshot() {
        let handler = ClipboardHandler;
        let request = CaptureRequest::new(CaptureData::Binary {
            mime_type: "image/png".to_string(),
            data: vec![0x89, 0x50, 0x4e, 0x47],
            filename: None,
        });
        let result = handler.capture(&request).await.unwrap();
        assert_eq!(result.object.object_type, ObjectType::Screenshot);
        assert_eq!(result.source, CaptureSource::Clipboard);
    }

    #[tokio::test]
    async fn test_bookmark_handler_uri() {
        let handler = BookmarkCaptureHandler;
        let request = CaptureRequest::new(CaptureData::Uri("https://example.com/page".to_string()))
            .with_title("Example");
        let result = handler.capture(&request).await.unwrap();
        assert_eq!(result.object.object_type, ObjectType::Bookmark);
        assert_eq!(result.source, CaptureSource::Url);
        assert_eq!(
            result.object.metadata.source_url.as_deref(),
            Some("https://example.com/page")
        );
    }

    #[tokio::test]
    async fn test_bookmark_handler_text_url() {
        let handler = BookmarkCaptureHandler;
        let request = CaptureRequest::new(CaptureData::Text(
            "https://example.com/from-text".to_string(),
        ));
        let result = handler.capture(&request).await.unwrap();
        assert_eq!(result.object.object_type, ObjectType::Bookmark);
        assert_eq!(result.source, CaptureSource::Url);
    }

    #[tokio::test]
    async fn test_bookmark_handler_rejects_non_url_text() {
        let handler = BookmarkCaptureHandler;
        let request = CaptureRequest::new(CaptureData::Text("not a url".to_string()));
        assert!(handler.capture(&request).await.is_none());
    }

    #[tokio::test]
    async fn test_youtube_handler() {
        let handler = YouTubeCaptureHandler;
        let request = CaptureRequest::new(CaptureData::Uri(
            "https://youtube.com/watch?v=dQw4w9WgXcQ".to_string(),
        ));
        let result = handler.capture(&request).await.unwrap();
        assert_eq!(result.object.object_type, ObjectType::YouTubeVideo);
        assert_eq!(result.source, CaptureSource::YouTube);
    }

    #[tokio::test]
    async fn test_github_handler() {
        let handler = GitHubRepositoryHandler;
        let request =
            CaptureRequest::new(CaptureData::Uri("https://github.com/org/repo".to_string()));
        let result = handler.capture(&request).await.unwrap();
        assert_eq!(result.object.object_type, ObjectType::Repository);
        assert_eq!(result.source, CaptureSource::GitHub);
    }

    #[tokio::test]
    async fn test_browser_routes_youtube() {
        let handler = BrowserCaptureHandler;
        let request = CaptureRequest::new(CaptureData::Uri(
            "https://youtube.com/watch?v=test".to_string(),
        ));
        let result = handler.capture(&request).await.unwrap();
        assert_eq!(result.object.object_type, ObjectType::YouTubeVideo);
        assert_eq!(result.source, CaptureSource::YouTube);
    }

    #[tokio::test]
    async fn test_browser_routes_github() {
        let handler = BrowserCaptureHandler;
        let request =
            CaptureRequest::new(CaptureData::Uri("https://github.com/org/repo".to_string()));
        let result = handler.capture(&request).await.unwrap();
        assert_eq!(result.object.object_type, ObjectType::Repository);
        assert_eq!(result.source, CaptureSource::GitHub);
    }

    #[tokio::test]
    async fn test_browser_routes_generic_url_as_bookmark() {
        let handler = BrowserCaptureHandler;
        let request =
            CaptureRequest::new(CaptureData::Uri("https://example.com/article".to_string()));
        let result = handler.capture(&request).await.unwrap();
        assert_eq!(result.object.object_type, ObjectType::Bookmark);
        assert_eq!(result.source, CaptureSource::Browser);
    }

    #[tokio::test]
    async fn test_browser_text_becomes_article() {
        let handler = BrowserCaptureHandler;
        let request = CaptureRequest::new(CaptureData::Text("Some article text".to_string()));
        let result = handler.capture(&request).await.unwrap();
        assert_eq!(result.object.object_type, ObjectType::Article);
    }

    #[tokio::test]
    async fn test_article_handler_html() {
        let handler = ArticleCaptureHandler;
        let html = r#"<html><head><title>My Article</title></head><body><article><p>Hello world</p></article></body></html>"#.to_string();
        let request = CaptureRequest::new(CaptureData::Text(html));
        let result = handler.capture(&request).await.unwrap();
        assert_eq!(result.object.object_type, ObjectType::Article);
        assert_eq!(result.source, CaptureSource::Article);
        assert_eq!(result.object.metadata.title.as_deref(), Some("My Article"));
        match &result.object.content {
            ObjectContent::Markdown(md) => {
                assert!(md.contains("Hello world"));
            }
            _ => panic!("expected markdown content"),
        }
    }

    #[tokio::test]
    async fn test_safari_reader_html() {
        let handler = SafariReaderHandler;
        let html = r#"<html><head><title>Reader Test</title></head><body><article><p>Readable content</p></article></body></html>"#.to_string();
        let request = CaptureRequest::new(CaptureData::Text(html));
        let result = handler.capture(&request).await.unwrap();
        assert_eq!(result.object.object_type, ObjectType::Article);
        assert_eq!(result.source, CaptureSource::SafariReader);
    }

    #[tokio::test]
    async fn test_html_to_article_extracts_metadata() {
        let html = r#"<html><head><title>Test Title</title><meta name="description" content="A test page"></head><body><article><p>Body text</p></article></body></html>"#;
        let mut meta = ObjectMetadata::default();
        let markdown = html_to_article(html, &mut meta);
        assert_eq!(meta.title.as_deref(), Some("Test Title"));
        assert!(meta.description.is_some());
        assert!(markdown.contains("Body text"));
    }

    #[tokio::test]
    async fn test_is_url_detection() {
        assert!(is_url("https://example.com"));
        assert!(is_url("http://example.com"));
        assert!(!is_url("not a url"));
        assert!(!is_url("Hello world"));
    }

    #[tokio::test]
    async fn test_request_url_helper() {
        let uri_req = CaptureRequest::new(CaptureData::Uri("https://example.com".to_string()));
        assert_eq!(uri_req.url(), Some("https://example.com"));

        let text_req = CaptureRequest::new(CaptureData::Text("https://example.com".to_string()));
        assert_eq!(text_req.url(), Some("https://example.com"));

        let plain_req = CaptureRequest::new(CaptureData::Text("hello".to_string()));
        assert_eq!(plain_req.url(), None);
    }
}
