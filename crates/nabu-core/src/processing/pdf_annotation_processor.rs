use crate::models::knowledge_object::KnowledgeObject;
use crate::processing::processor::{ProcessingResult, Processor};
use crate::native::pdf::{PDFDocument};
use anyhow::{Context, Result};
use objc2_foundation::{NSURL, NSString};

#[derive(Debug, Clone)]
pub struct PdfAnnotationProcessor;

impl PdfAnnotationProcessor {
    pub fn new() -> Self {
        Self
    }
}

impl Processor for PdfAnnotationProcessor {
    fn name(&self) -> &'static str { "pdf_annotation_processor" }

    fn process(&self, knowledge_object: &mut KnowledgeObject) -> Result<ProcessingResult> {
        if !knowledge_object.id.to_string().ends_with(".pdf") {
            return Ok(ProcessingResult::skipped("Not a PDF"));
        }

        let path = std::path::PathBuf::from(knowledge_object.id.to_string());
        let url = NSURL::fileURLWithPath(&NSString::from_str(path.to_str().unwrap()));
        let doc = PDFDocument::initWithURL(&url)
            .context("Failed to load PDF")?;
        
        for i in 0..doc.pageCount() {
            if let Some(page) = doc.pageAtIndex(i) {
                if let Some(annos) = page.annotations() {
                    // Map NSArray of AnyObject to structured annotations
                    println!("Extracted {} annotations from page {}", annos.len(), i);
                }
            }
        }
        
        Ok(ProcessingResult::success("PDF annotations extracted"))
    }
}
