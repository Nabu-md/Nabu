pub struct OcrEngine;
impl OcrEngine {
    pub fn new() -> Self { Self }
    pub fn extract_text(&self, _path: &str) -> anyhow::Result<String> { Ok("".into()) }
}
