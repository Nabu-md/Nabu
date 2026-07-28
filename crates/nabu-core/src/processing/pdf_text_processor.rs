use crate::models::knowledge_object::{KnowledgeObject, ObjectContent};
use crate::processing::processor::{ProcessingResult, Processor};
use crate::native::pdf::PDFDocument;
use objc2_foundation::{NSURL, NSString};

#[derive(Debug, Clone)]
pub struct PdfTextProcessor;

impl PdfTextProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Processor for PdfTextProcessor {
    fn name(&self) -> &'static str { "pdf_text_processor" }

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

        let url = NSURL::fileURLWithPath(&NSString::from_str(source_file));

        let doc = match PDFDocument::initWithURL(&url) {
            Some(doc) => doc,
            None => return ProcessingResult::warning("Failed to load PDF document"),
        };

        let mut extracted_text = String::new();
        let mut is_scanned = true;

        for i in 0..doc.pageCount() {
            if let Some(page) = doc.pageAtIndex(i) {
                if let Some(text) = page.string() {
                    let s = text.to_str();
                    extracted_text.push_str(s);
                    is_scanned = false;
                }
            }
        }

        if is_scanned {
            return ProcessingResult::warning("Scanned PDF, needs OCR");
        }

        let mut ko = knowledge_object;
        ko.content = ObjectContent::PlainText;
        ko.metadata.custom.insert(
            "extracted_text".to_string(),
            serde_json::Value::String(extracted_text),
        );

        ProcessingResult::modified(ko, vec!["PDF text extracted".to_string()])
    }
}
