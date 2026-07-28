//! Metadata extraction processor for enriched KnowledgeObjects.
//!
//! This processor inspects HTML content for standard metadata (Title, Author,
//! Publication Date, Description, Keywords, Canonical URL, Language) and
//! enriches the [`KnowledgeObject`] metadata, preserving user-edited values.

use crate::models::knowledge_object::{KnowledgeObject, ObjectType};
use crate::processing::processor::{ProcessingDecision, ProcessingResult, Processor};
use scraper::{Html, Selector};
use std::collections::HashMap;

/// Processor that enriches KnowledgeObjects with HTML metadata.
#[derive(Debug, Default)]
pub struct MetadataExtractor {}

impl MetadataExtractor {
    pub fn new() -> Self {
        Self::default()
    }

    fn extract_metadata(&self, html: &Html) -> HashMap<String, String> {
        let mut metadata = HashMap::new();

        // 1. Title
        if let Ok(title_selector) = Selector::parse("title") {
            if let Some(element) = html.select(&title_selector).next() {
                metadata.insert("title".to_string(), element.text().collect::<String>().trim().to_string());
            }
        }

        // 2. Meta tags (description, keywords, author)
        if let Ok(meta_selector) = Selector::parse("meta") {
            for element in html.select(&meta_selector) {
                let name = element.value().attr("name").unwrap_or("").to_lowercase();
                let property = element.value().attr("property").unwrap_or("").to_lowercase();
                let content = element.value().attr("content").unwrap_or("").trim().to_string();

                match name.as_str() {
                    "description" => { metadata.entry("description".to_string()).or_insert(content.clone()); }
                    "keywords" => { metadata.entry("keywords".to_string()).or_insert(content.clone()); }
                    "author" => { metadata.entry("author".to_string()).or_insert(content.clone()); }
                    _ => {}
                }

                // OG tags
                match property.as_str() {
                    "og:title" => { metadata.entry("title".to_string()).or_insert(content.clone()); }
                    "og:description" => { metadata.entry("description".to_string()).or_insert(content.clone()); }
                    "og:site_name" => { metadata.entry("site_name".to_string()).or_insert(content); }
                    _ => {}
                }
            }
        }

        // 3. Language
        if let Ok(html_selector) = Selector::parse("html") {
            if let Some(element) = html.select(&html_selector).next() {
                if let Some(lang) = element.value().attr("lang") {
                    metadata.insert("language".to_string(), lang.trim().to_string());
                }
            }
        }

        metadata
    }
}

impl Processor for MetadataExtractor {
    fn name(&self) -> &'static str {
        "metadata_extractor"
    }

    fn process(&self, mut knowledge_object: KnowledgeObject) -> ProcessingResult {
        // Only process types that are likely to have HTML metadata
        match knowledge_object.object_type {
            ObjectType::Bookmark | 
            ObjectType::Document |
            ObjectType::Website |
            ObjectType::ResearchPaper |
            ObjectType::Book => {
                if let Some(source_path) = &knowledge_object.metadata.source_file {
                    if let Ok(content) = std::fs::read_to_string(source_path) {
                        let html = Html::parse_document(&content);
                        let extracted = self.extract_metadata(&html);

                        for (key, value) in extracted {
                            match key.as_str() {
                                "title" => { if knowledge_object.metadata.title.is_none() { knowledge_object.metadata.title = Some(value); } }
                                "author" => { if knowledge_object.metadata.author.is_none() { knowledge_object.metadata.author = Some(value); } }
                                "language" => { if knowledge_object.metadata.language.is_none() { knowledge_object.metadata.language = Some(value); } }
                                _ => {
                                    if !knowledge_object.metadata.custom.contains_key(&key) {
                                        knowledge_object.metadata.custom.insert(key, serde_json::Value::String(value));
                                    }
                                }
                            }
                        }
                        return ProcessingResult::modified(knowledge_object, vec!["Metadata enriched".to_string()]);
                    }
                }
            }
            _ => {}
        }

        ProcessingResult::unchanged(knowledge_object)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::knowledge_object::KnowledgeObject;

    #[test]
    fn test_metadata_extraction() {
        let html = r#"
            <!DOCTYPE html>
            <html lang="en">
            <head>
                <title>Test Page</title>
                <meta name="description" content="Test description">
                <meta property="og:site_name" content="Test Site">
            </head>
            <body></body>
            </html>
        "#;
        
        let extractor = MetadataExtractor::new();
        let doc = Html::parse_document(html);
        let metadata = extractor.extract_metadata(&doc);
        
        assert_eq!(metadata.get("title").unwrap(), "Test Page");
        assert_eq!(metadata.get("description").unwrap(), "Test description");
        assert_eq!(metadata.get("site_name").unwrap(), "Test Site");
        assert_eq!(metadata.get("language").unwrap(), "en");
    }
}
