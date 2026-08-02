use crate::jobs::cancellation::CancellationToken;
use crate::jobs::workers::progress::ProgressReporter;
use crate::models::ObjectType;
use crate::processing::processor::{ProcessingContext, ProcessingResult, Processor};
use async_trait::async_trait;
use std::collections::HashMap;

/// Automatically determines the destination folder/path for a KnowledgeObject.
///
/// Uses classification, tags, object type, and custom rules to suggest
/// where the object should be filed in the vault.
pub struct AutoFiler {
    rules: HashMap<String, String>,
}

impl Default for AutoFiler {
    fn default() -> Self {
        Self::new()
    }
}

impl AutoFiler {
    pub fn new() -> Self {
        Self {
            rules: Self::default_rules(),
        }
    }

    fn default_rules() -> HashMap<String, String> {
        let mut rules = HashMap::new();
        rules.insert("invoice".to_string(), "Finance/Invoices".to_string());
        rules.insert("receipt".to_string(), "Finance/Receipts".to_string());
        rules.insert("meeting_note".to_string(), "Meetings".to_string());
        rules.insert("email".to_string(), "Inbox/Email".to_string());
        rules.insert("article".to_string(), "Reading/Articles".to_string());
        rules.insert("code".to_string(), "Code".to_string());
        rules.insert("bookmark".to_string(), "Bookmarks".to_string());
        rules
    }

    /// Add or override a routing rule.
    pub fn add_rule(&mut self, classification: impl Into<String>, folder: impl Into<String>) {
        self.rules.insert(classification.into(), folder.into());
    }
}

#[async_trait]
impl Processor for AutoFiler {
    fn name(&self) -> &'static str {
        "auto_filer"
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

        // Get classification
        let classification = object
            .custom_properties
            .get("classification")
            .and_then(|v| {
                if let crate::models::CustomPropertyValue::Text(t) = v {
                    Some(t.clone())
                } else {
                    None
                }
            });

        progress.set_progress(0.3);

        // Determine suggested folder
        let suggested_folder = classification
            .as_ref()
            .and_then(|class| self.rules.get(class))
            .cloned()
            .or_else(|| match object.object_type {
                ObjectType::Screenshot => Some("Screenshots".to_string()),
                ObjectType::Scan => Some("Scans".to_string()),
                ObjectType::AudioRecording => Some("Audio".to_string()),
                ObjectType::VideoRecording => Some("Videos".to_string()),
                ObjectType::YouTubeVideo => Some("Videos/YouTube".to_string()),
                ObjectType::Repository => Some("Code/Repositories".to_string()),
                ObjectType::Contact => Some("Contacts".to_string()),
                ObjectType::Project => Some("Projects".to_string()),
                ObjectType::Task => Some("Tasks".to_string()),
                ObjectType::Event => Some("Calendar".to_string()),
                ObjectType::Template => Some("Templates".to_string()),
                _ => Some("Inbox".to_string()),
            });

        progress.set_progress(0.6);

        if let Some(ref folder) = suggested_folder {
            // Compute suggested vault path
            let filename = object
                .metadata
                .title
                .as_deref()
                .map(sanitize_filename)
                .unwrap_or_else(|| object.id.to_string());

            let vault_path = format!("{}/{}.md", folder, filename);
            object.metadata.vault_path = Some(vault_path);

            object.custom_properties.insert(
                "suggested_folder".to_string(),
                crate::models::CustomPropertyValue::Text(folder.clone()),
            );
        }

        progress.set_progress(1.0);
        ProcessingResult::new(object)
    }

    fn supports(&self, object_type: &ObjectType) -> bool {
        !matches!(object_type, ObjectType::Dashboard | ObjectType::Collection)
    }
}

fn sanitize_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();

    let trimmed: String = sanitized.trim().to_string();
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::KnowledgeObject;

    #[tokio::test]
    async fn test_file_invoice() {
        let obj = KnowledgeObject::new(
            ObjectType::Document,
            crate::models::ObjectContent::PlainText("Invoice".to_string()),
        )
        .with_metadata(crate::models::ObjectMetadata {
            title: Some("Invoice #123".to_string()),
            ..Default::default()
        })
        .with_property(
            "classification",
            crate::models::CustomPropertyValue::Text("invoice".to_string()),
        );

        let ctx = ProcessingContext::new(obj);
        let filer = AutoFiler::new();
        let result = filer
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        let folder = result
            .object
            .custom_properties
            .get("suggested_folder")
            .and_then(|v| {
                if let crate::models::CustomPropertyValue::Text(t) = v {
                    Some(t.clone())
                } else {
                    None
                }
            });
        assert_eq!(folder, Some("Finance/Invoices".to_string()));
    }

    #[tokio::test]
    async fn test_inbox_fallback() {
        let obj = KnowledgeObject::new(
            ObjectType::Note,
            crate::models::ObjectContent::Markdown("A random note".to_string()),
        )
        .with_metadata(crate::models::ObjectMetadata {
            title: Some("Random Note".to_string()),
            ..Default::default()
        });

        let ctx = ProcessingContext::new(obj);
        let filer = AutoFiler::new();
        let result = filer
            .process(&ctx, ProgressReporter::noop(), CancellationToken::new())
            .await;

        let folder = result
            .object
            .custom_properties
            .get("suggested_folder")
            .and_then(|v| {
                if let crate::models::CustomPropertyValue::Text(t) = v {
                    Some(t.clone())
                } else {
                    None
                }
            });
        assert_eq!(folder, Some("Inbox".to_string()));
    }
}
