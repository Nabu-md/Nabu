use crate::models::knowledge_object::KnowledgeObject;
use crate::processing::processor::{ProcessingResult, Processor};
use crate::native::pdf::PDFDocument;
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

        let url = match NSURL::fileURLWithPath(&NSString::from_str(source_file)) {
            Some(url) => url,
            None => return ProcessingResult::skipped("Invalid PDF path"),
        };

        let doc = match PDFDocument::initWithURL(&url) {
            Some(doc) => doc,
            None => return ProcessingResult::warning("Failed to load PDF document"),
        };

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

                    for j in 0..page_annotation_count {
                        if let Some(annotation) = annos.object_at_index(j) {
                            if let Some(content) = annotation.content() {
                                let content_str = content.to_string();
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
            return ProcessingResult::warning("No PDF annotations found");
        }

        let mut ko = knowledge_object;
        ko.metadata.custom.insert(
            "annotations".to_string(),
            serde_json::Value::Array(extracted_annotations)
        );

        if !annotation_edges.is_empty() {
            ko.metadata.custom.insert(
                "annotation_edges".to_string(),
                serde_json::Value::Array(annotation_edges)
            );
        }

        ProcessingResult::modified(ko, vec!["PDF annotations extracted with graph edges".to_string()])
    }
}
