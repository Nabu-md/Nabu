use crate::models::knowledge_object::KnowledgeObject;
use crate::processing::processor::{ProcessingResult, Processor};
use crate::native::pdf::{PDFDocument};
use anyhow::{Context, Result};
use objc2_foundation::{NSURL, NSString};

#[derive(Debug, Clone)]
pub struct PdfMetadataProcessor;

impl PdfMetadataProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Processor for PdfMetadataProcessor {
    fn name(&self) -> &'static str { "pdf_metadata_processor" }

    fn process(&self, knowledge_object: &mut KnowledgeObject) -> Result<ProcessingResult> {
        if !knowledge_object.id.to_string().ends_with(".pdf") {
            return Ok(ProcessingResult::skipped("Not a PDF"));
        }

        let path = std::path::PathBuf::from(knowledge_object.id.to_string());
        let url = NSURL::fileURLWithPath(&NSString::from_str(path.to_str().unwrap()));
        let doc = PDFDocument::initWithURL(&url)
            .context("Failed to load PDF")?;
        
        if let Some(attrs) = doc.documentAttributes() {
            // Need to map NSDictionary to something usable.
            // Simplified for now: just log that we have metadata
            println!("Extracted PDF metadata: {:?}", attrs);
        }
        
        Ok(ProcessingResult::success("PDF metadata extracted"))
    }
}
