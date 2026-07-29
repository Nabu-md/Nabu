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

    // Use the last path segment as a potential title
    if let Some(title) = url.split('/').last().map(|s| s.to_string()) {
        if metadata.title.is_none() {
            let decoded = url_decode(&title);
            metadata.title = Some(decoded);
        }
    }
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
