use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{KnowledgeObject, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

/// Generates AI-powered summaries of KnowledgeObject content.
///
/// Currently uses extractive summarization (picking key sentences).
/// In production, this would use:
/// - Local LLM (LLaMA, Mistral via candle/llama.cpp)
/// - OpenAI/Anthropic API (cloud option)
///
/// The summary is stored as:
/// - `ai_summary` custom property
/// - `summary_confidence` score
/// - `summary_method` for provenance
pub struct AiSummariser;

#[async_trait]
impl Processor for AiSummariser {
    fn name(&self) -> &'static str {
        "ai_summariser"
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

        // Get text content
        let text = match &object.content {
            ObjectContent::Markdown(s) => s.clone(),
            ObjectContent::PlainText(s) => s.clone(),
            ObjectContent::RichHtml(s) => strip_html_for_summary(s),
            _ => return ProcessingResult::unmodified(object),
        };

        // Skip very short content
        if text.len() < 100 {
            return ProcessingResult::unmodified(object);
        }

        progress.set_progress(0.4);

        // Generate extractive summary
        let summary = extractive_summarize(&text, 3);

        progress.set_progress(0.7);

        if !summary.is_empty() {
            object.custom_properties.insert(
                "ai_summary".to_string(),
                crate::models::CustomPropertyValue::Text(summary),
            );

            object.custom_properties.insert(
                "summary_confidence".to_string(),
                crate::models::CustomPropertyValue::Number(0.75),
            );

            object.custom_properties.insert(
                "summary_method".to_string(),
                crate::models::CustomPropertyValue::Text("extractive".to_string()),
            );

            // Update description with summary if no description exists
            if object.metadata.description.is_none() {
                object.metadata.description = object
                    .custom_properties
                    .get("ai_summary")
                    .and_then(|v| {
                        if let crate::models::CustomPropertyValue::Text(t) = v {
                            Some(t.clone())
                        } else {
                            None
                        }
                    });
            }
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
        )
    }
}

/// Generate an extractive summary by selecting the most important sentences.
/// Picks sentences from the beginning, middle, and end for coverage.
fn extractive_summarize(text: &str, num_sentences: usize) -> String {
    let sentences: Vec<&str> = text
        .split(|c: char| c == '.' || c == '!' || c == '?')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && s.len() > 20)
        .collect();

    if sentences.is_empty() {
        return String::new();
    }

    if sentences.len() <= num_sentences {
        return sentences.join(". ") + ".";
    }

    let mut selected = Vec::new();

    // First sentence
    selected.push(sentences[0]);

    // Middle sentence
    let mid = sentences.len() / 2;
    if mid > 0 && mid < sentences.len() - 1 {
        selected.push(sentences[mid]);
    }

    // Last sentence
    if sentences.len() > 2 {
        selected.push(sentences[sentences.len() - 1]);
    }

    selected.join(". ") + "."
}

fn strip_html_for_summary(html: &str) -> String {
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

    #[tokio::test]
    async fn test_summarise_article() {
        let text = "Artificial intelligence has transformed the way we interact with technology. \
            Machine learning algorithms can now recognize images, understand speech, and generate text. \
            These advances have led to breakthroughs in healthcare, finance, and education. \
            However, challenges remain in areas like bias, fairness, and transparency. \
            Researchers are actively working on addressing these concerns.";
        let obj = KnowledgeObject::new(
            ObjectType::Article,
            ObjectContent::Markdown(text.to_string()),
        );

        let ctx = ProcessingContext::new(obj);
        let summariser = AiSummariser;
        let result = summariser
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert!(result
            .object
            .custom_properties
            .contains_key("ai_summary"));
    }

    #[tokio::test]
    async fn test_skips_short_text() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown("Short.".to_string()),
        );

        let ctx = ProcessingContext::new(obj);
        let summariser = AiSummariser;
        let result = summariser
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        assert_eq!(result.modified, false);
    }
}
