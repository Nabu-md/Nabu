//! Rich browser capture handlers for specialised web content extraction.
//!
//! This module provides three specialised capture handlers that transform common
//! web content into structured KnowledgeObjects before entering the
//! ProcessingPipeline.
//!
//! # Handlers
//!
//! - [`ArticleCaptureHandler`]: Extracts article content using Reader mode / Readability
//! - [`YouTubeCaptureHandler`]: Extracts YouTube video metadata
//! - [`GitHubRepositoryHandler`]: Extracts GitHub repository metadata
//!
//! # Architecture
//!
//! ```text
//! Safari Extension
//!     ↓
//! CaptureRequest
//!     ↓
//! CaptureEngine
//!     ↓
//! Specialised CaptureHandler
//!     ↓
//! KnowledgeObject
//!     ↓
//! ProcessingPipeline → StorageManager
//! ```
//!
//! All handlers implement the [`CaptureHandler`] trait and are registered
//! with the [`CaptureEngine`] under their respective source types.

use std::collections::HashMap;

use crate::capture::{CaptureHandler, CaptureRequest, CaptureResult, IngestionOptions};

/// MIME type for article captures.
const ARTICLE_MIME_TYPE: &str = "application/x-nabu-article";

/// MIME type for YouTube captures.
const YOUTUBE_MIME_TYPE: &str = "application/x-nabu-youtube";

/// MIME type for GitHub repository captures.
const GITHUB_MIME_TYPE: &str = "application/x-nabu-github";

// ---------------------------------------------------------------------------
// Article Capture Handler
// ---------------------------------------------------------------------------

/// Handler for extracting article content from web pages.
///
/// This handler uses a Readability-inspired algorithm to extract clean article
/// content from HTML. It removes navigation, ads, and unrelated page elements.
///
/// The handler is registered with the [`CaptureEngine`] under the source type
/// `"article"`.
///
/// # Payload Format
///
/// ```json
/// {
///   "html": "<html>...</html>",
///   "url": "https://example.com/article",
///   "title": "Article Title"
/// }
/// ```
pub struct ArticleCaptureHandler;

impl ArticleCaptureHandler {
    /// Creates a new article capture handler.
    pub fn new() -> Self {
        Self
    }

    /// Extracts article content from HTML using a Readability-inspired algorithm.
    fn extract_article(&self, html: &str, base_url: &str) -> ArticleExtract {
        // Parse HTML
        let document = match scraper::Html::parse_document(html) {
            doc => doc,
        };

        // Try to find the article element
        let article_content = self.find_article_content(&document);

        // Extract metadata
        let title = self.extract_title(&document);
        let author = self.extract_author(&document);
        let published_date = self.extract_published_date(&document);
        let canonical_url = self.extract_canonical_url(&document, base_url);
        let reading_time = self.estimate_reading_time(&article_content);

        ArticleExtract {
            title,
            author,
            published_date,
            canonical_url,
            reading_time,
            content: article_content,
        }
    }

    /// Finds the main article content element.
    fn find_article_content(&self, document: &scraper::Html) -> String {
        // Try common article selectors
        let selectors = [
            "article",
            "[role='article']",
            ".post-content",
            ".article-content",
            ".entry-content",
            ".content",
            "main",
            "#main",
            ".post",
            ".article",
        ];

        for selector in &selectors {
            if let Ok(html_selector) = scraper::Selector::parse(selector) {
                for element in document.select(&html_selector) {
                    let text = element.text().collect::<Vec<_>>().join(" ");
                    let trimmed = text.trim();
                    if trimmed.len() > 200 {
                        return trimmed.to_string();
                    }
                }
            }
        }

        // Fallback: extract all paragraph text
        let mut paragraphs = Vec::new();
        if let Ok(p_selector) = scraper::Selector::parse("p") {
            for p in document.select(&p_selector) {
                let text = p.text().collect::<String>();
                let trimmed = text.trim();
                if trimmed.len() > 50 {
                    paragraphs.push(trimmed.to_string());
                }
            }
        }

        if paragraphs.is_empty() {
            // Last resort: get all text
            document.root_element().text().collect::<String>()
        } else {
            paragraphs.join("\n\n")
        }
    }

    /// Extracts the page title.
    fn extract_title(&self, document: &scraper::Html) -> Option<String> {
        if let Ok(selector) = scraper::Selector::parse("h1") {
            for h1 in document.select(&selector) {
                let text = h1.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }

        if let Ok(selector) = scraper::Selector::parse("title") {
            for title in document.select(&selector) {
                let text = title.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }

        None
    }

    /// Extracts the author from meta tags or structured data.
    fn extract_author(&self, document: &scraper::Html) -> Option<String> {
        // Try meta tags
        let meta_selectors = [
            "meta[name='author']",
            "meta[property='article:author']",
            "[rel='author']",
            ".author",
            ".byline",
        ];

        for selector in &meta_selectors {
            if let Ok(s) = scraper::Selector::parse(selector) {
                for element in document.select(&s) {
                    if let Some(content) = element.value().attr("content") {
                        let trimmed = content.trim();
                        if !trimmed.is_empty() {
                            return Some(trimmed.to_string());
                        }
                    }
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }

        None
    }

    /// Extracts the publication date.
    fn extract_published_date(&self, document: &scraper::Html) -> Option<String> {
        let meta_selectors = [
            "meta[property='article:published_time']",
            "meta[name='publishedDate']",
            "meta[name='date']",
            "time[datetime]",
            ".published",
            ".date",
        ];

        for selector in &meta_selectors {
            if let Ok(s) = scraper::Selector::parse(selector) {
                for element in document.select(&s) {
                    if let Some(content) = element.value().attr("content") {
                        let trimmed = content.trim();
                        if !trimmed.is_empty() {
                            return Some(trimmed.to_string());
                        }
                    }
                    if let Some(datetime) = element.value().attr("datetime") {
                        let trimmed = datetime.trim();
                        if !trimmed.is_empty() {
                            return Some(trimmed.to_string());
                        }
                    }
                    let text = element.text().collect::<String>().trim().to_string();
                    if !text.is_empty() {
                        return Some(text);
                    }
                }
            }
        }

        None
    }

    /// Extracts the canonical URL.
    fn extract_canonical_url(&self, document: &scraper::Html, base_url: &str) -> String {
        if let Ok(selector) = scraper::Selector::parse("link[rel='canonical']") {
            for link in document.select(&selector) {
                if let Some(href) = link.value().attr("href") {
                    let trimmed = href.trim();
                    if !trimmed.is_empty() {
                        return trimmed.to_string();
                    }
                }
            }
        }

        if let Ok(selector) = scraper::Selector::parse("meta[property='og:url']") {
            for meta in document.select(&selector) {
                if let Some(content) = meta.value().attr("content") {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        return trimmed.to_string();
                    }
                }
            }
        }

        base_url.to_string()
    }

    /// Estimates reading time based on word count (200 words per minute).
    fn estimate_reading_time(&self, content: &str) -> Option<u32> {
        let word_count = content.split_whitespace().count();
        if word_count == 0 {
            return None;
        }
        let minutes = (word_count as f32 / 200.0).ceil() as u32;
        Some(minutes.max(1))
    }
}

impl Default for ArticleCaptureHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureHandler for ArticleCaptureHandler {
    fn source_type(&self) -> &'static str {
        "article"
    }

    fn can_handle(&self, request: &CaptureRequest) -> bool {
        request.source_type == "article"
    }

    fn capture(&self, request: CaptureRequest) -> CaptureResult {
        let html = match request.payload.get("html").and_then(|v| v.as_str()) {
            Some(h) => h,
            None => {
                return CaptureResult {
                    success: false,
                    knowledge_object_id: None,
                    error: Some("Missing 'html' in payload".to_string()),
                    message: "Article capture failed: missing HTML".to_string(),
                    payload: None,
                };
            }
        };

        let base_url = request
            .payload
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let extract = self.extract_article(html, base_url);

        // Build structured metadata
        let mut custom = HashMap::new();
        if let Some(ref author) = extract.author {
            custom.insert("author".to_string(), serde_json::json!(author));
        }
        if let Some(ref date) = extract.published_date {
            custom.insert("published_date".to_string(), serde_json::json!(date));
        }
        if let Some(rt) = extract.reading_time {
            custom.insert("reading_time_minutes".to_string(), serde_json::json!(rt));
        }
        custom.insert(
            "canonical_url".to_string(),
            serde_json::json!(extract.canonical_url),
        );

        // Serialize article content as JSON
        let article_data = serde_json::json!({
            "title": extract.title,
            "content": extract.content,
            "word_count": extract.content.split_whitespace().count(),
        });

        let raw_bytes = serde_json::to_vec(&article_data)
            .map_err(|e| format!("Failed to serialize article: {}", e))
            .unwrap_or_default();

        let ingestion_request = crate::capture::IngestionRequest {
            source: "article".to_string(),
            raw_bytes,
            mime_type: ARTICLE_MIME_TYPE.to_string(),
            vault_id: request.vault_id.clone(),
            source_file: None,
            options: IngestionOptions {
                create_knowledge_object: true,
                extract_metadata: true,
                custom,
            },
        };

        let payload = match serde_json::to_value(&ingestion_request) {
            Ok(p) => p,
            Err(e) => {
                return CaptureResult {
                    success: false,
                    knowledge_object_id: None,
                    error: Some(format!("Serialization error: {}", e)),
                    message: "Article capture failed: serialization error".to_string(),
                    payload: None,
                };
            }
        };

        CaptureResult {
            success: true,
            knowledge_object_id: None,
            error: None,
            message: format!(
                "Article captured: {}",
                extract.title.unwrap_or_else(|| "Untitled".to_string())
            ),
            payload: Some(payload),
        }
    }
}

/// Result of article extraction.
struct ArticleExtract {
    title: Option<String>,
    author: Option<String>,
    published_date: Option<String>,
    canonical_url: String,
    reading_time: Option<u32>,
    content: String,
}

// ---------------------------------------------------------------------------
// YouTube Capture Handler
// ---------------------------------------------------------------------------

/// Handler for extracting YouTube video metadata.
///
/// This handler extracts structured metadata from YouTube video pages.
/// It does NOT download the video or subtitles.
///
/// The handler is registered with the [`CaptureEngine`] under the source type
/// `"youtube"`.
///
/// # Payload Format
///
/// ```json
/// {
///   "html": "<html>...</html>",
///   "url": "https://www.youtube.com/watch?v=VIDEO_ID"
/// }
/// ```
pub struct YouTubeCaptureHandler;

impl YouTubeCaptureHandler {
    /// Creates a new YouTube capture handler.
    pub fn new() -> Self {
        Self
    }

    /// Extracts YouTube video metadata from HTML.
    fn extract_metadata(&self, html: &str, url: &str) -> YouTubeMetadata {
        let document = scraper::Html::parse_document(html);

        let title = self.extract_title(&document);
        let channel = self.extract_channel(&document);
        let publish_date = self.extract_publish_date(&document);
        let duration = self.extract_duration(&document);
        let thumbnail_url = self.extract_thumbnail_url(&document, url);
        let description = self.extract_description(&document);

        YouTubeMetadata {
            title,
            channel,
            publish_date,
            duration,
            thumbnail_url,
            description,
            video_url: url.to_string(),
        }
    }

    /// Extracts the video title.
    fn extract_title(&self, document: &scraper::Html) -> Option<String> {
        if let Ok(selector) = scraper::Selector::parse("h1.title yt-formatted-string") {
            for el in document.select(&selector) {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }

        if let Ok(selector) = scraper::Selector::parse("meta[property='og:title']") {
            for meta in document.select(&selector) {
                if let Some(content) = meta.value().attr("content") {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }

        None
    }

    /// Extracts the channel name.
    fn extract_channel(&self, document: &scraper::Html) -> Option<String> {
        if let Ok(selector) = scraper::Selector::parse("yt-formatted-string#text.ytd-channel-name") {
            for el in document.select(&selector) {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }

        if let Ok(selector) = scraper::Selector::parse("a.yt-simple-endpoint.style-scope.yt-formatted-string") {
            for el in document.select(&selector) {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() && text.len() < 100 {
                    return Some(text);
                }
            }
        }

        None
    }

    /// Extracts the publish date.
    fn extract_publish_date(&self, document: &scraper::Html) -> Option<String> {
        if let Ok(selector) = scraper::Selector::parse("meta[itemprop='datePublished']") {
            for meta in document.select(&selector) {
                if let Some(content) = meta.value().attr("content") {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }

        None
    }

    /// Extracts the video duration in ISO 8601 format.
    fn extract_duration(&self, document: &scraper::Html) -> Option<String> {
        if let Ok(selector) = scraper::Selector::parse("meta[itemprop='duration']") {
            for meta in document.select(&selector) {
                if let Some(content) = meta.value().attr("content") {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }

        None
    }

    /// Extracts the thumbnail URL.
    fn extract_thumbnail_url(&self, document: &scraper::Html, video_url: &str) -> Option<String> {
        if let Ok(selector) = scraper::Selector::parse("meta[property='og:image']") {
            for meta in document.select(&selector) {
                if let Some(content) = meta.value().attr("content") {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }

        // Fallback: construct from video ID
        if let Some(video_id) = self.extract_video_id(video_url) {
            Some(format!(
                "https://i.ytimg.com/vi/{}/maxresdefault.jpg",
                video_id
            ))
        } else {
            None
        }
    }

    /// Extracts the video description.
    fn extract_description(&self, document: &scraper::Html) -> Option<String> {
        if let Ok(selector) = scraper::Selector::parse("meta[property='og:description']") {
            for meta in document.select(&selector) {
                if let Some(content) = meta.value().attr("content") {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }

        None
    }

    /// Extracts the video ID from a YouTube URL.
    fn extract_video_id(&self, url: &str) -> Option<String> {
        let patterns = [
            r"youtube\.com/watch\?v=([^&]+)",
            r"youtu\.be/([^?]+)",
            r"youtube\.com/embed/([^?]+)",
        ];

        for pattern in &patterns {
            if let Some(captures) = regex::Regex::new(pattern)
                .ok()
                .and_then(|re| re.captures(url))
            {
                if let Some(id) = captures.get(1) {
                    return Some(id.as_str().to_string());
                }
            }
        }

        None
    }
}

impl Default for YouTubeCaptureHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureHandler for YouTubeCaptureHandler {
    fn source_type(&self) -> &'static str {
        "youtube"
    }

    fn can_handle(&self, request: &CaptureRequest) -> bool {
        request.source_type == "youtube"
    }

    fn capture(&self, request: CaptureRequest) -> CaptureResult {
        let html = match request.payload.get("html").and_then(|v| v.as_str()) {
            Some(h) => h,
            None => {
                return CaptureResult {
                    success: false,
                    knowledge_object_id: None,
                    error: Some("Missing 'html' in payload".to_string()),
                    message: "YouTube capture failed: missing HTML".to_string(),
                    payload: None,
                };
            }
        };

        let url = request
            .payload
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let metadata = self.extract_metadata(html, url);

        // Build structured metadata
        let mut custom = HashMap::new();
        if let Some(ref channel) = metadata.channel {
            custom.insert("channel".to_string(), serde_json::json!(channel));
        }
        if let Some(ref date) = metadata.publish_date {
            custom.insert("publish_date".to_string(), serde_json::json!(date));
        }
        if let Some(ref duration) = metadata.duration {
            custom.insert("duration".to_string(), serde_json::json!(duration));
        }
        if let Some(ref thumb) = metadata.thumbnail_url {
            custom.insert("thumbnail_url".to_string(), serde_json::json!(thumb));
        }
        if let Some(ref desc) = metadata.description {
            custom.insert("description".to_string(), serde_json::json!(desc));
        }

        // Serialize metadata as JSON
        let video_data = serde_json::json!({
            "title": metadata.title,
            "video_url": metadata.video_url,
            "channel": metadata.channel,
            "publish_date": metadata.publish_date,
            "duration": metadata.duration,
            "thumbnail_url": metadata.thumbnail_url,
            "description": metadata.description,
        });

        let raw_bytes = serde_json::to_vec(&video_data)
            .map_err(|e| format!("Failed to serialize YouTube metadata: {}", e))
            .unwrap_or_default();

        let ingestion_request = crate::capture::IngestionRequest {
            source: "youtube".to_string(),
            raw_bytes,
            mime_type: YOUTUBE_MIME_TYPE.to_string(),
            vault_id: request.vault_id.clone(),
            source_file: None,
            options: IngestionOptions {
                create_knowledge_object: true,
                extract_metadata: true,
                custom,
            },
        };

        let payload = match serde_json::to_value(&ingestion_request) {
            Ok(p) => p,
            Err(e) => {
                return CaptureResult {
                    success: false,
                    knowledge_object_id: None,
                    error: Some(format!("Serialization error: {}", e)),
                    message: "YouTube capture failed: serialization error".to_string(),
                    payload: None,
                };
            }
        };

        CaptureResult {
            success: true,
            knowledge_object_id: None,
            error: None,
            message: format!(
                "YouTube video captured: {}",
                metadata.title.unwrap_or_else(|| "Untitled".to_string())
            ),
            payload: Some(payload),
        }
    }
}

/// Extracted YouTube metadata.
struct YouTubeMetadata {
    title: Option<String>,
    channel: Option<String>,
    publish_date: Option<String>,
    duration: Option<String>,
    thumbnail_url: Option<String>,
    description: Option<String>,
    video_url: String,
}

// ---------------------------------------------------------------------------
// GitHub Repository Capture Handler
// ---------------------------------------------------------------------------

/// Handler for extracting GitHub repository metadata.
///
/// This handler extracts structured metadata from GitHub repository pages.
/// It does NOT clone repositories.
///
/// The handler is registered with the [`CaptureEngine`] under the source type
/// `"github"`.
///
/// # Payload Format
///
/// ```json
/// {
///   "html": "<html>...</html>",
///   "url": "https://github.com/owner/repo"
/// }
/// ```
pub struct GitHubRepositoryHandler;

impl GitHubRepositoryHandler {
    /// Creates a new GitHub repository handler.
    pub fn new() -> Self {
        Self
    }

    /// Extracts repository metadata from HTML.
    fn extract_metadata(&self, html: &str, url: &str) -> GitHubMetadata {
        let document = scraper::Html::parse_document(html);

        let (owner, repo_name) = self.parse_repo_path(url);
        let description = self.extract_description(&document);
        let star_count = self.extract_star_count(&document);
        let primary_language = self.extract_primary_language(&document);
        let license_info = self.extract_license(&document);
        let topics = self.extract_topics(&document);
        let readme_preview = self.extract_readme_preview(&document);

        GitHubMetadata {
            owner,
            repo_name,
            description,
            star_count,
            primary_language,
            license_info,
            topics,
            readme_preview,
            repo_url: url.to_string(),
        }
    }

    /// Parses owner and repository name from URL.
    fn parse_repo_path(&self, url: &str) -> (Option<String>, Option<String>) {
        let patterns = [
            r"github\.com/([^/]+)/([^/]+)/?$",
            r"github\.com/([^/]+)/([^/]+)",
        ];

        for pattern in &patterns {
            if let Some(captures) = regex::Regex::new(pattern)
                .ok()
                .and_then(|re| re.captures(url))
            {
                let owner = captures.get(1).map(|m| m.as_str().to_string());
                let repo = captures.get(2).map(|m| m.as_str().to_string());
                return (owner, repo);
            }
        }

        (None, None)
    }

    /// Extracts the repository description.
    fn extract_description(&self, document: &scraper::Html) -> Option<String> {
        if let Ok(selector) = scraper::Selector::parse("p.f4.my-3") {
            for el in document.select(&selector) {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }

        if let Ok(selector) = scraper::Selector::parse("meta[property='og:description']") {
            for meta in document.select(&selector) {
                if let Some(content) = meta.value().attr("content") {
                    let trimmed = content.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }

        None
    }

    /// Extracts the star count.
    fn extract_star_count(&self, document: &scraper::Html) -> Option<u64> {
        if let Ok(selector) = scraper::Selector::parse("span#repo-stars-counter-star") {
            for el in document.select(&selector) {
                let text = el.text().collect::<String>().trim().to_string();
                if let Ok(num) = text.replace(",", "").parse::<u64>() {
                    return Some(num);
                }
            }
        }

        if let Ok(selector) = scraper::Selector::parse("strong[aria-label='stargazers']") {
            for el in document.select(&selector) {
                let text = el.text().collect::<String>().trim().to_string();
                if let Ok(num) = text.replace(",", "").parse::<u64>() {
                    return Some(num);
                }
            }
        }

        None
    }

    /// Extracts the primary programming language.
    fn extract_primary_language(&self, document: &scraper::Html) -> Option<String> {
        if let Ok(selector) = scraper::Selector::parse("span[itemprop='programmingLanguage']") {
            for el in document.select(&selector) {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }

        if let Ok(selector) = scraper::Selector::parse("li.Language") {
            for el in document.select(&selector) {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }

        None
    }

    /// Extracts the license information.
    fn extract_license(&self, document: &scraper::Html) -> Option<String> {
        if let Ok(selector) = scraper::Selector::parse("a[href*='license']") {
            for el in document.select(&selector) {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() && text != "License" {
                    return Some(text);
                }
            }
        }

        None
    }

    /// Extracts repository topics.
    fn extract_topics(&self, document: &scraper::Html) -> Vec<String> {
        let mut topics = Vec::new();

        if let Ok(selector) = scraper::Selector::parse("a.topic-tag") {
            for el in document.select(&selector) {
                let text = el.text().collect::<String>().trim().to_string();
                if !text.is_empty() {
                    topics.push(text);
                }
            }
        }

        topics
    }

    /// Extracts a preview of the README.
    fn extract_readme_preview(&self, document: &scraper::Html) -> Option<String> {
        if let Ok(selector) = scraper::Selector::parse("article.markdown-body") {
            for el in document.select(&selector) {
                let text = el.text().collect::<String>().trim().to_string();
                if text.len() > 50 {
                    // Return first 500 characters as preview
                    return Some(text.chars().take(500).collect());
                }
            }
        }

        None
    }
}

impl Default for GitHubRepositoryHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl CaptureHandler for GitHubRepositoryHandler {
    fn source_type(&self) -> &'static str {
        "github"
    }

    fn can_handle(&self, request: &CaptureRequest) -> bool {
        request.source_type == "github"
    }

    fn capture(&self, request: CaptureRequest) -> CaptureResult {
        let html = match request.payload.get("html").and_then(|v| v.as_str()) {
            Some(h) => h,
            None => {
                return CaptureResult {
                    success: false,
                    knowledge_object_id: None,
                    error: Some("Missing 'html' in payload".to_string()),
                    message: "GitHub capture failed: missing HTML".to_string(),
                    payload: None,
                };
            }
        };

        let url = request
            .payload
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let metadata = self.extract_metadata(html, url);

        // Build structured metadata
        let mut custom = HashMap::new();
        if let Some(ref owner) = metadata.owner {
            custom.insert("owner".to_string(), serde_json::json!(owner));
        }
        if let Some(ref repo) = metadata.repo_name {
            custom.insert("repo_name".to_string(), serde_json::json!(repo));
        }
        if let Some(ref desc) = metadata.description {
            custom.insert("description".to_string(), serde_json::json!(desc));
        }
        if let Some(stars) = metadata.star_count {
            custom.insert("star_count".to_string(), serde_json::json!(stars));
        }
        if let Some(ref lang) = metadata.primary_language {
            custom.insert("primary_language".to_string(), serde_json::json!(lang));
        }
        if let Some(ref license) = metadata.license_info {
            custom.insert("license".to_string(), serde_json::json!(license));
        }
        if !metadata.topics.is_empty() {
            custom.insert("topics".to_string(), serde_json::json!(metadata.topics));
        }
        if let Some(ref readme) = metadata.readme_preview {
            custom.insert("readme_preview".to_string(), serde_json::json!(readme));
        }

        // Serialize metadata as JSON
        let repo_data = serde_json::json!({
            "owner": metadata.owner,
            "repo_name": metadata.repo_name,
            "description": metadata.description,
            "star_count": metadata.star_count,
            "primary_language": metadata.primary_language,
            "license": metadata.license_info,
            "topics": metadata.topics,
            "readme_preview": metadata.readme_preview,
            "repo_url": metadata.repo_url,
        });

        let raw_bytes = serde_json::to_vec(&repo_data)
            .map_err(|e| format!("Failed to serialize GitHub metadata: {}", e))
            .unwrap_or_default();

        let ingestion_request = crate::capture::IngestionRequest {
            source: "github".to_string(),
            raw_bytes,
            mime_type: GITHUB_MIME_TYPE.to_string(),
            vault_id: request.vault_id.clone(),
            source_file: None,
            options: IngestionOptions {
                create_knowledge_object: true,
                extract_metadata: true,
                custom,
            },
        };

        let payload = match serde_json::to_value(&ingestion_request) {
            Ok(p) => p,
            Err(e) => {
                return CaptureResult {
                    success: false,
                    knowledge_object_id: None,
                    error: Some(format!("Serialization error: {}", e)),
                    message: "GitHub capture failed: serialization error".to_string(),
                    payload: None,
                };
            }
        };

        let repo_display = metadata
            .owner
            .zip(metadata.repo_name)
            .map(|(o, r)| format!("{}/{}", o, r))
            .unwrap_or_else(|| "Unknown repository".to_string());

        CaptureResult {
            success: true,
            knowledge_object_id: None,
            error: None,
            message: format!("GitHub repository captured: {}", repo_display),
            payload: Some(payload),
        }
    }
}

/// Extracted GitHub repository metadata.
struct GitHubMetadata {
    owner: Option<String>,
    repo_name: Option<String>,
    description: Option<String>,
    star_count: Option<u64>,
    primary_language: Option<String>,
    license_info: Option<String>,
    topics: Vec<String>,
    readme_preview: Option<String>,
    repo_url: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // --- ArticleCaptureHandler tests ---

    #[test]
    fn article_source_type() {
        let handler = ArticleCaptureHandler::new();
        assert_eq!(handler.source_type(), "article");
    }

    #[test]
    fn article_can_handle_filters_by_source_type() {
        let handler = ArticleCaptureHandler::new();
        assert!(handler.can_handle(&CaptureRequest {
            source_type: "article".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }));
        assert!(!handler.can_handle(&CaptureRequest {
            source_type: "browser".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }));
    }

    #[test]
    fn article_capture_missing_html_fails() {
        let handler = ArticleCaptureHandler::new();
        let result = handler.capture(CaptureRequest {
            source_type: "article".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        });
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn article_capture_extracts_content() {
        let handler = ArticleCaptureHandler::new();
        let html = r#"
            <html>
            <head><title>Test Article</title></head>
            <body>
                <nav>Navigation</nav>
                <article>
                    <h1>Test Article Title</h1>
                    <p>This is the first paragraph of the article with enough content to be considered meaningful content for testing purposes.</p>
                    <p>This is the second paragraph with more content to ensure the extraction works correctly across multiple paragraphs.</p>
                </article>
                <footer>Footer</footer>
            </body>
            </html>
        "#;

        let result = handler.capture(CaptureRequest {
            source_type: "article".to_string(),
            payload: serde_json::json!({
                "html": html,
                "url": "https://example.com/article"
            }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        });

        assert!(result.success);
        assert!(result.payload.is_some());
    }

    // --- YouTubeCaptureHandler tests ---

    #[test]
    fn youtube_source_type() {
        let handler = YouTubeCaptureHandler::new();
        assert_eq!(handler.source_type(), "youtube");
    }

    #[test]
    fn youtube_can_handle_filters_by_source_type() {
        let handler = YouTubeCaptureHandler::new();
        assert!(handler.can_handle(&CaptureRequest {
            source_type: "youtube".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }));
        assert!(!handler.can_handle(&CaptureRequest {
            source_type: "browser".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }));
    }

    #[test]
    fn youtube_capture_missing_html_fails() {
        let handler = YouTubeCaptureHandler::new();
        let result = handler.capture(CaptureRequest {
            source_type: "youtube".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        });
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn youtube_capture_extracts_metadata() {
        let handler = YouTubeCaptureHandler::new();
        let html = r#"
            <html>
            <head>
                <meta property="og:title" content="Test Video">
                <meta property="og:description" content="Test description">
                <meta itemprop="duration" content="PT10M30S">
                <meta itemprop="datePublished" content="2024-01-15">
                <meta property="og:image" content="https://i.ytimg.com/vi/test/maxresdefault.jpg">
            </head>
            <body>
                <h1 class="title">Test Video Title</h1>
                <yt-formatted-string id="text">Test Channel</yt-formatted-string>
            </body>
            </html>
        "#;

        let result = handler.capture(CaptureRequest {
            source_type: "youtube".to_string(),
            payload: serde_json::json!({
                "html": html,
                "url": "https://www.youtube.com/watch?v=test123"
            }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        });

        assert!(result.success);
        assert!(result.payload.is_some());
    }

    // --- GitHubRepositoryHandler tests ---

    #[test]
    fn github_source_type() {
        let handler = GitHubRepositoryHandler::new();
        assert_eq!(handler.source_type(), "github");
    }

    #[test]
    fn github_can_handle_filters_by_source_type() {
        let handler = GitHubRepositoryHandler::new();
        assert!(handler.can_handle(&CaptureRequest {
            source_type: "github".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }));
        assert!(!handler.can_handle(&CaptureRequest {
            source_type: "browser".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        }));
    }

    #[test]
    fn github_capture_missing_html_fails() {
        let handler = GitHubRepositoryHandler::new();
        let result = handler.capture(CaptureRequest {
            source_type: "github".to_string(),
            payload: serde_json::json!({}),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        });
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    #[test]
    fn github_capture_extracts_metadata() {
        let handler = GitHubRepositoryHandler::new();
        let html = r#"
            <html>
            <head>
                <meta property="og:description" content="A test repository">
            </head>
            <body>
                <p class="f4 my-3">A test repository for testing purposes</p>
                <span id="repo-stars-counter-star">1,234</span>
                <span itemprop="programmingLanguage">Rust</span>
                <a class="topic-tag">web</a>
                <a class="topic-tag">rust</a>
                <a href="/owner/repo/blob/main/LICENSE">MIT License</a>
                <article class="markdown-body">
                    <p>This is the README content for the repository. It contains documentation and usage examples.</p>
                </article>
            </body>
            </html>
        "#;

        let result = handler.capture(CaptureRequest {
            source_type: "github".to_string(),
            payload: serde_json::json!({
                "html": html,
                "url": "https://github.com/owner/repo"
            }),
            vault_id: "vault-1".to_string(),
            context: HashMap::new(),
        });

        assert!(result.success);
        assert!(result.payload.is_some());
    }

    #[test]
    fn github_parse_repo_path() {
        let handler = GitHubRepositoryHandler::new();

        let (owner, repo) = handler.parse_repo_path("https://github.com/owner/repo");
        assert_eq!(owner, Some("owner".to_string()));
        assert_eq!(repo, Some("repo".to_string()));

        let (owner, repo) = handler.parse_repo_path("https://github.com/owner/repo/");
        assert_eq!(owner, Some("owner".to_string()));
        assert_eq!(repo, Some("repo".to_string()));
    }
}
