use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::{ObjectContent, ObjectType};
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;

use regex::Regex;

/// Extracts dates and timestamps from content to build a processing timeline.
/// Stores extracted dates in custom properties for use by other processors.
pub struct TimelineExtractor;

#[async_trait]
impl Processor for TimelineExtractor {
    fn name(&self) -> &'static str {
        "timeline_extractor"
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
            ObjectContent::Uri(_) => return ProcessingResult::unmodified(object),
            ObjectContent::Binary { .. } => return ProcessingResult::unmodified(object),
        };

        progress.set_progress(0.3);

        // Extract all dates from the content
        let dates = extract_dates(&text);
        progress.set_progress(0.6);

        if let Some(earliest) = dates.first() {
            object.custom_properties.insert(
                "timeline_earliest_date".to_string(),
                crate::models::CustomPropertyValue::Date(earliest.to_string()),
            );
        }

        if let Some(latest) = dates.last() {
            object.custom_properties.insert(
                "timeline_latest_date".to_string(),
                crate::models::CustomPropertyValue::Date(latest.to_string()),
            );
        }

        object.custom_properties.insert(
            "timeline_date_count".to_string(),
            crate::models::CustomPropertyValue::Number(dates.len() as f64),
        );

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
        )
    }
}

/// Extract all ISO 8601 dates and common date formats from text.
fn extract_dates(text: &str) -> Vec<String> {
    let mut dates = Vec::new();

    // ISO 8601 dates: 2024-01-15, 2024-01-15T14:30:00Z
    let iso_re = Regex::new(r"\d{4}-\d{2}-\d{2}(T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2}))?").unwrap();
    for cap in iso_re.find_iter(text) {
        dates.push(cap.as_str().to_string());
    }

    // US dates: 01/15/2024, 1/15/2024
    let us_re = Regex::new(r"\b\d{1,2}/\d{1,2}/\d{4}\b").unwrap();
    for cap in us_re.find_iter(text) {
        dates.push(cap.as_str().to_string());
    }

    // European dates: 15/01/2024, 15.01.2024
    let eu_re = Regex::new(r"\b\d{1,2}[./]\d{1,2}[./]\d{4}\b").unwrap();
    for cap in eu_re.find_iter(text) {
        if !dates.contains(&cap.as_str().to_string()) {
            dates.push(cap.as_str().to_string());
        }
    }

    dates.sort();
    dates.dedup();
    dates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::KnowledgeObject;

    #[tokio::test]
    async fn test_extract_iso_dates() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            ObjectContent::Markdown(
                "Meeting on 2024-01-15. Follow-up on 2024-03-20T14:30:00Z."
                    .to_string(),
            ),
        );

        let ctx = ProcessingContext::new(obj);
        let extractor = TimelineExtractor;
        let result = extractor
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        let count = result
            .object
            .custom_properties
            .get("timeline_date_count")
            .and_then(|v| {
                if let crate::models::CustomPropertyValue::Number(n) = v {
                    Some(*n as usize)
                } else {
                    None
                }
            });
        assert_eq!(count, Some(2));
    }
}
