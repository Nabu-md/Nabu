use crate::models::knowledge_object::KnowledgeObject;
use crate::processing::processor::{ProcessingResult, Processor};
use crate::native::pdf::{PDFDocument, PdfEngine};
use crate::models::graph::RelationType;
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
        
        let mut extracted_annotations = Vec::new();
        let mut annotation_edges = Vec::new();

        for i in 0..doc.pageCount() {
            if let Some(page) = doc.pageAtIndex(i) {
                if let Some(annos) = page.annotations() {
                    let page_annotation_count = annos.len();
                    extracted_annotations.push(serde_json::json!({
                        "page": i,
                        "annotation_count": page_annotation_count
                    }));

                    // Create graph edges for annotations that reference other documents
                    // This enables annotation-based relationship discovery
                    for j in 0..page_annotation_count {
                        if let Some(annotation) = annos.object_at_index(j) {
                            // Extract annotation content and create graph edges
                            // for cross-document references found in annotations
                            if let Some(content) = annotation.content() {
                                let content_str = content.to_string();
                                // Check if annotation references another document
                                if content_str.contains("file://") || content_str.contains(".pdf") {
                                    annotation_edges.push(serde_json::json!({
                                        "page": i,
                                        "type": "file_reference",
                                        "content": content_str
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }
        
        if extracted_annotations.is_empty() {
            return Ok(ProcessingResult::warning("No PDF annotations found"));
        }
        
        knowledge_object.metadata.custom.insert(
            "annotations".to_string(),
            serde_json::Value::Array(extracted_annotations)
        );

        // Store annotation graph edges for graph integration
        if !annotation_edges.is_empty() {
            knowledge_object.metadata.custom.insert(
                "annotation_edges".to_string(),
                serde_json::Value::Array(annotation_edges)
            );
        }
        
        Ok(ProcessingResult::success("PDF annotations extracted with graph edges"))
    }
}
