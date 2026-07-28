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
            let keys = vec![
                ("Title", "Title"),
                ("Author", "Author"),
                ("Subject", "Subject"),
            ];

            let mut found = false;
            for (key_name, map_key) in keys {
                let ns_key = NSString::from_str(key_name);
                if let Some(val) = attrs.objectForKey(&ns_key) {
                    if let Some(s) = val.downcast_ref::<NSString>() {
                        knowledge_object.metadata.custom.insert(
                            map_key.to_string(), 
                            serde_json::Value::String(s.to_string())
                        );
                        found = true;
                    }
                }
            }
            
            if !found {
                return Ok(ProcessingResult::warning("No PDF metadata attributes found"));
            }
        } else {
            return Ok(ProcessingResult::warning("No PDF document attributes found"));
        }
        
        Ok(ProcessingResult::success("PDF metadata extracted"))
    }
}
