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
        let pdf_name = std::path::Path::new(pdf_path).file_name()
            .context("Invalid PDF path")?
            .to_string_lossy();
        let ann_path = self.annotations_dir.join(format!("{}_page_{}.json", pdf_name, page));
        std::fs::write(ann_path, serde_json::to_string_pretty(&ann)?)?;
        Ok(())
    }
}
