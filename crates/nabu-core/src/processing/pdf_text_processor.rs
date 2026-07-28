use crate::models::knowledge_object::{KnowledgeObject, ObjectContent};
use crate::processing::processor::{ProcessingResult, Processor};
use crate::native::pdf::{PDFDocument};
use anyhow::{Context, Result};
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

    fn process(&self, knowledge_object: &mut KnowledgeObject) -> Result<ProcessingResult> {
        if !knowledge_object.id.to_string().ends_with(".pdf") {
            return Ok(ProcessingResult::skipped("Not a PDF"));
        }

        let path = std::path::PathBuf::from(knowledge_object.id.to_string());
        if !path.exists() {
            return Ok(ProcessingResult::skipped("File not found"));
        }

        let url = NSURL::fileURLWithPath(&NSString::from_str(path.to_str().unwrap()));
        let doc = PDFDocument::initWithURL(&url)
            .context("Failed to load PDF")?;
        
        let mut extracted_text = String::new();
        let mut is_scanned = true; 
        
        for i in 0..doc.pageCount() {
            let page = doc.pageAtIndex(i).context("Failed to get page")?;
            if let Some(text) = page.string() {
                if let Ok(s) = text.to_str() {
                    extracted_text.push_str(s);
                    is_scanned = false; // Found text, so it's not scanned
                }
            }
        }
        
        if is_scanned {
            return Ok(ProcessingResult::success("Scanned PDF, needs OCR"));
        }
        
        knowledge_object.content = ObjectContent::Text(extracted_text);
        
        Ok(ProcessingResult::success("PDF text extracted"))
    }
}
