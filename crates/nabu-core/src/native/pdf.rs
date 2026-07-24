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
        let _ = std::fs::create_dir_all(&dir);
        Self { annotations_dir: dir }
    }

    pub fn annotate(&self, pdf_path: &str, page: u32, content: &str) -> anyhow::Result<()> {
        let doc = lopdf::Document::load(pdf_path).context("Failed to load PDF")?;
        if page as usize > doc.get_pages().len() {
            anyhow::bail!("Page {} out of range", page);
        }
        let ann = Annotation { page, content: content.into() };
        let ann_path = self.annotations_dir.join(format!("{}.json", page));
        std::fs::write(ann_path, serde_json::to_string(&ann)?)?;
        Ok(())
    }
}
