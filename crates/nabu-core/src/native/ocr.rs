use anyhow::Result;

pub struct OcrEngine;

impl OcrEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn extract_text(&self, _image_path: &str) -> Result<String> {
        // FFI call to macOS Vision framework would go here
        Ok("OCR implementation stub".to_string())
    }
}
