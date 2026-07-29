use crate::models::knowledge_object::KnowledgeObject;
use crate::processing::processor::{ProcessingResult, Processor};
use crate::native::pdf::PdfDocument;

#[derive(Debug, Clone)]
pub struct PdfMetadataProcessor;

impl PdfMetadataProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Processor for PdfMetadataProcessor {
    fn name(&self) -> &'static str { "pdf_metadata_processor" }

    fn process(&self, knowledge_object: KnowledgeObject) -> ProcessingResult {
        let source_file = match &knowledge_object.metadata.source_file {
            Some(path) => path,
            None => return ProcessingResult::skipped("No source file"),
        };

        if !source_file.to_lowercase().ends_with(".pdf") {
            return ProcessingResult::skipped("Not a PDF");
        }

        let path = std::path::PathBuf::from(source_file);
        if !path.exists() {
            return ProcessingResult::skipped("PDF file not found");
        }

        let doc = match PdfDocument::from_path(&path) {
            Some(doc) => doc,
            None => return ProcessingResult::warning("Failed to load PDF document"),
        };

        let attrs = doc.document_attributes();
        let keys = vec![
            ("Title", "Title"),
            ("Author", "Author"),
            ("Subject", "Subject"),
        ];

        let mut found = false;
        let mut ko = knowledge_object;
        for (key_name, map_key) in keys {
            if let Some(val) = attrs.get(key_name) {
                ko.metadata.custom.insert(
                    map_key.to_string(),
                    serde_json::Value::String(val.clone())
                );
                found = true;
            }
        }

        if !found {
            return ProcessingResult::warning("No PDF metadata attributes found");
        }

        ProcessingResult::modified(ko, vec!["PDF metadata extracted".to_string()])
    }
}
