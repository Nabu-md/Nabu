use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct Annotation {
    pub page: u32,
    pub content: String,
}

pub struct PdfAnnotator {
    annotations_dir: std::path::PathBuf,
}

impl PdfAnnotator {
    pub fn new(vault_root: &std::path::Path) -> Self {
        let dir = vault_root.join(".nabu/annotations");
    pub fn annotate(&self, pdf_path: &str, page: u32, content: &str) -> Result<()> {
        let doc = Document::load(pdf_path).context("Failed to load PDF")?;
        if page == 0 || page as usize > doc.get_pages().len() {
            anyhow::bail!("Page {} out of range", page);
        }
        let ann = Annotation { page, content: content.into() };
        let ann_id = uuid::Uuid::new_v4();
        let ann_path = self.annotations_dir.join(format!("{}_{}.json", ann_id, page));
        
        let file = std::fs::File::create(&ann_path).context("Failed to create annotation file")?;
        serde_json::to_writer_pretty(file, &ann).context("Failed to serialize annotation")?;
        Ok(())
    }
}
