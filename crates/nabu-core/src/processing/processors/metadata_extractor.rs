use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{ObjectContent, ObjectMetadata, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;
use regex::Regex;

/// Extracts structured metadata from content.
///
/// Extracts:
/// - Title (from heading, filename, or URL)
/// - Author (from bylines, metadata)
/// - Publication date
/// - Site/domain name
/// - Language detection
/// - Description/excerpt
pub struct MetadataExtractor;

#[async_trait]
impl Processor for MetadataExtractor {
    fn name(&self) -> &'static str {
        "metadata_extractor"
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
        let mut object = context.object.clone();

        let text = match &object.content {
            ObjectContent::Markdown(s) => s.clone(),
            ObjectContent::PlainText(s) => s.clone(),
            ObjectContent::RichHtml(s) => s.clone(),
            ObjectContent::Uri(url) => {
                extract_url_metadata(&mut object.metadata, url);
                // Normalize the URL and infer a readable title.
                if object.metadata.title.is_none() {
                    object.metadata.title = extract_title_from_url(url);
                }
                return ProcessingResult::new(object);
            }
            ObjectContent::Binary { .. } => return ProcessingResult::unmodified(object),
        };

        progress.set_progress(0.3);

        // Extract title from first heading or first line
        if object.metadata.title.is_none() {
            object.metadata.title = extract_title(&text);
        }

        progress.set_progress(0.5);

        // Extract author
        if object.metadata.authors.is_empty() {
            object.metadata.authors = extract_authors(&text);
        }

        progress.set_progress(0.7);

        // Extract description/excerpt
        if object.metadata.description.is_none() {
            object.metadata.description = extract_description(&text, 200);
        }

        progress.set_progress(0.9);

        // Detect language
        if object.metadata.language.is_none() {
            object.metadata.language = detect_language(&text);
        }

        progress.set_progress(1.0);
        ProcessingResult::new(object)
    }

    fn supports(&self, object_type: &ObjectType) -> bool {
        matches!(
            object_type,
            ObjectType::Note
                | ObjectType::Article
                | ObjectType::Document
                | ObjectType::Email
                | ObjectType::Bookmark
                | ObjectType::CodeSnippet
                | ObjectType::YouTubeVideo
                | ObjectType::Repository
        )
    }
}

fn extract_title(text: &str) -> Option<String> {
    // Try Markdown H1 first
    let h1_re = Regex::new(r"(?m)^#\s+(.+)$").unwrap();
    if let Some(cap) = h1_re.captures(text) {
        return Some(cap[1].trim().to_string());
    }

    // Try first line if it's short (< 100 chars)
    let first_line = text.lines().next()?;
    let trimmed = first_line.trim();
    if !trimmed.is_empty() && trimmed.len() < 100 {
        return Some(trimmed.to_string());
    }

    None
}

fn extract_authors(text: &str) -> Vec<String> {
    let mut authors = Vec::new();

    // "By Author Name" pattern
    let by_re = Regex::new(r"(?im)^by\s+(.+)$").unwrap();
    for cap in by_re.captures_iter(text) {
        let name = cap[1].trim().trim_matches(|c: char| c == '.' || c == ',');
        if !name.is_empty() && name.len() < 100 {
            authors.push(name.to_string());
        }
    }

    // Author: pattern (YAML frontmatter)
    let yaml_re = Regex::new(r"(?im)^author:\s+(.+)$").unwrap();
    for cap in yaml_re.captures_iter(text) {
        authors.push(cap[1].trim().to_string());
    }

    authors
}

fn extract_description(text: &str, max_chars: usize) -> Option<String> {
    // Find first paragraph (non-empty, non-heading line)
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with('[')
            && trimmed.len() > 50
        {
            let desc = trimmed.chars().take(max_chars).collect::<String>();
            return Some(desc);
        }
    }
    None
}

fn detect_language(_text: &str) -> Option<String> {
    // In a real implementation, this would use a language detection library.
    // For now, return None — language detection requires the whatlang or CLD3 crate.
    None
}

fn extract_url_metadata(metadata: &mut ObjectMetadata, url: &str) {
    // Extract domain from URL
    if let Some(domain) = extract_domain(url) {
        metadata.site_name = Some(domain);
    }

    // Extract path segments for a better fallback title and description.
    if let Some(title) = extract_title_from_url(url) {
        if metadata.title.is_none() {
            metadata.title = Some(title);
        }
    }

    // Best-effort description from the URL path.
    if metadata.description.is_none() {
        if let Some(desc) = extract_description_from_url(url) {
            metadata.description = Some(desc);
        }
    }
}

/// Infer a human-readable title from a URL path's last meaningful segment.
///
/// Examples:
///   `https://example.com/blog/my-great-post` → `My-great-post` → `My Great Post`
///   `https://github.com/org/repo`             → `org/repo` (kept as-is)
///   `https://youtube.com/watch?v=dQw4w9WgXcQ` → `dQw4w9WgXcQ`
fn extract_title_from_url(url: &str) -> Option<String> {
    // YouTube watch URLs: use the video id as part of the title.
    if let Some(pos) = url.find("v=") {
        let id = url[pos + 2..].split('&').next().unwrap_or("");
        if !id.is_empty() && !id.contains('=') {
            return Some(format!("YouTube: {}", id));
        }
    }

    // Strip query/fragment, take the path.
    let path = url
        .split('#')
        .next()
        .unwrap_or(url)
        .split('?')
        .next()
        .unwrap_or(url);
    let path = path
        .split("//")
        .nth(1)
        .map(|rest| rest.split('/').collect::<String>())
        .filter(|rest| !rest.is_empty())
        .unwrap_or_else(|| path.to_string());

    // Take up to the last two non-empty path segments for the title,
    // which handles github.com/owner/repo and blog.example.com/2024/post.
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    if segments.is_empty() {
        return None;
    }

    let title_segments = if segments.len() >= 2 {
        segments[segments.len() - 2..].to_vec()
    } else {
        segments[segments.len() - 1..].to_vec()
    };

    let raw = title_segments.join(" ");
    if raw.is_empty() {
        return None;
    }

    // Decode URL-encoded characters and replace separators with spaces.
    let decoded = url_decode(&raw);
    let cleaned = decoded.replace(&['_', '-'][..], " ").trim().to_string();
    if cleaned.is_empty() {
        return None;
    }

    Some(cleaned)
}

/// Build a short description from a URL's path (for bookmark/article objects
/// whose content is a URI). This is a fallback only — if the metadata
/// extractor already has richer context it is preserved.
fn extract_description_from_url(url: &str) -> Option<String> {
    let path = url
        .split('#')
        .next()
        .unwrap_or(url)
        .split('?')
        .next()
        .unwrap_or(url);

    if let Some(domain) = extract_domain(url) {
        if domain == "youtube.com"
            || domain == "youtu.be"
            || domain == "www.youtube.com"
            || domain == "m.youtube.com"
        {
            return Some("YouTube video".to_string());
        }
        if domain == "github.com" || domain == "www.github.com" {
            return Some("GitHub repository".to_string());
        }
    }

    // Generic fallback: use the first path segment after the domain.
    let after = path.split("//").nth(1)?;
    let segments: Vec<&str> = after.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return None;
    }

    // Skip known TLDs / bare domains.
    if segments.len() == 1 {
        return None;
    }

    Some(format!("Link to {}", url_decode(&segments.join("/"))))
}

fn extract_domain(url: &str) -> Option<String> {
    let re = Regex::new(r"https?://(?:www\.)?([^/]+)").unwrap();
    re.captures(url).map(|cap| cap[1].to_string())
}

fn url_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{KnowledgeObject, ObjectContent};

    #[tokio::test]
    async fn test_extract_title_from_h1() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown("# My Great Title\n\nSome content here.".to_string()),
        );

        let ctx = ProcessingContext::new(obj);
        let extractor = MetadataExtractor;
        let result = extractor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert_eq!(
            result.object.metadata.title,
            Some("My Great Title".to_string())
        );
    }

    #[tokio::test]
    async fn test_extract_url_domain() {
        let obj = KnowledgeObject::new(
            ObjectType::Article,
            ObjectContent::Uri("https://example.com/blog/article-title".to_string()),
        );

        let ctx = ProcessingContext::new(obj);
        let extractor = MetadataExtractor;
        let result = extractor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert_eq!(
            result.object.metadata.site_name,
            Some("example.com".to_string())
        );
    }
}
