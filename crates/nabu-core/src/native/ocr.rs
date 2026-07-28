use anyhow::Result;

pub struct OcrEngine;

impl OcrEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn extract_text(&self, image_path: &str) -> Result<String> {
        // Vision framework integration requires macOS-only `vision-rs` or direct FFI bindings.
        // This stub provides the interface for when real Vision integration is added.
        // See: https://github.com/apple/swift-vision for the Swift reference implementation.
        let _ = image_path;
        Ok(String::new())
    }
}
