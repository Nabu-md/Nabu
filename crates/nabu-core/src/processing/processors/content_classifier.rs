use crate::event_bus::{ItemProcessingProgressEvent, PipelineEvent};
use crate::event_bus::kinds::ITEM_PROCESSING_PROGRESS;
use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{ObjectContent, ObjectMetadata, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Detects content type from content patterns.
/// Uses simple heuristics — no ML required.
///
/// Classifies content into categories like:
/// - invoice, receipt, meeting note, article, code, email
pub struct ContentClassifier;

#[async_trait]
impl Processor for ContentClassifier {
    fn name(&self) -> &'static str {
        "content_classifier"
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

        // Get the text content to classify
        let text = match &object.content {
            ObjectContent::Markdown(s) => s.clone(),
            ObjectContent::PlainText(s) => s.clone(),
            ObjectContent::RichHtml(s) => strip_html_tags(s),
            ObjectContent::Uri(_) => {
                // URI objects are classified by their type
                return ProcessingResult::unmodified(object);
            }
            ObjectContent::Binary { .. } => {
                // Binary objects are classified by their handler
                return ProcessingResult::unmodified(object);
            }
        };

        progress.set_progress(0.3);

        // Classification heuristics
        let classification = classify_content(&text, &object.metadata);

        progress.set_progress(0.6);

        // Store classification in custom properties
        if let Some(class) = classification {
            object.custom_properties.insert(
                "classification".to_string(),
                crate::models::CustomPropertyValue::Text(class),
            );
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
                | ObjectType::CodeSnippet
                | ObjectType::Bookmark
        )
    }
}

fn classify_content(text: &str, metadata: &ObjectMetadata) -> Option<String> {
    let lower = text.to_lowercase();

    // Invoice detection
    if contains_any(&lower, &[
        "invoice", "invoice number", "invoice date", "total due",
        "amount due", "payment terms", "bill to",
    ]) {
        return Some("invoice".to_string());
    }

    // Receipt detection
    if contains_any(&lower, &[
        "receipt", "total", "tax", "subtotal", "payment method",
        "card ending", "thank you for your purchase",
    ]) && contains_any(&lower, &["$", "€", "£", "¥"]) {
        return Some("receipt".to_string());
    }

    // Meeting notes
    if contains_any(&lower, &[
        "meeting notes", "agenda", "action items", "minutes",
        "attendees", "discussion points", "next steps",
    ]) {
        return Some("meeting_note".to_string());
    }

    // Code snippet (heuristic: check raw text for code patterns)
    if text.contains("```") || text.contains("function ") || text.contains("def ") {
        return Some("code".to_string());
    }

    // Email
    if contains_any(&lower, &[
        "subject:", "from:", "to:", "cc:", "bcc:",
        "forwarded message", "original message",
    ]) && text.contains('@') {
        return Some("email".to_string());
    }

    // Article
    if text.len() > 500 && contains_any(&lower, &[
        "introduction", "conclusion", "summary", "abstract",
        "published", "author",
    ]) {
        return Some("article".to_string());
    }

    None
}

fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|p| text.contains(p))
}

fn strip_html_tags(html: &str) -> String {
    // Simple HTML tag stripping for classification purposes
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ => {
                if !in_tag {
                    result.push(c);
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::KnowledgeObject;

    #[tokio::test]
    async fn test_classify_invoice() {
        let obj = KnowledgeObject::new(
            ObjectType::Document,
            ObjectContent::PlainText(
                "INVOICE #1234\nInvoice Date: 2024-01-15\nTotal Due: $500.00\nPayment Terms: Net 30"
                    .to_string(),
            ),
        );
        let ctx = ProcessingContext::new(obj);
        let classifier = ContentClassifier;
        let result = classifier
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        let class = result
            .object
            .custom_properties
            .get("classification")
            .and_then(|v| {
                if let crate::models::CustomPropertyValue::Text(t) = v {
                    Some(t.clone())
                } else {
                    None
                }
            });
        assert_eq!(class, Some("invoice".to_string()));
    }

    #[tokio::test]
    async fn test_classify_meeting() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown(
                "# Meeting Notes\n\n## Attendees\n- Alice\n- Bob\n\n## Action Items\n- [ ] Do something"
                    .to_string(),
            ),
        );
        let ctx = ProcessingContext::new(obj);
        let classifier = ContentClassifier;
        let result = classifier
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        let class = result
            .object
            .custom_properties
            .get("classification")
            .and_then(|v| {
                if let crate::models::CustomPropertyValue::Text(t) = v {
                    Some(t.clone())
                } else {
                    None
                }
            });
        assert_eq!(class, Some("meeting_note".to_string()));
    }
}
