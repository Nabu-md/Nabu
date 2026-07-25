use anyhow::Result;

pub struct OcrEngine;

impl OcrEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn extract_text(&self, image_path: &str) -> Result<String> {
        Ok(format!("OCR placeholder for: {}", image_path))
    }
}
