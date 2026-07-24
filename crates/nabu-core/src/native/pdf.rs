use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Annotation {
    pub page: u32,
    pub content: String,
}

pub struct PdfAnnotator;

impl PdfAnnotator {
    pub fn new() -> Self {
        Self
    }

    pub fn annotate(&self, path: &str, page: u32, content: &str) -> anyhow::Result<()> {
        let ann = Annotation { page, content: content.into() };
        let json = serde_json::to_string(&ann)?;
        // Save annotation
        Ok(())
    }
}
