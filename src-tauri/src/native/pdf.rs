pub struct PdfAnnotator;
impl PdfAnnotator {
    pub fn new(_root: &std::path::Path) -> Self { Self }
    pub fn annotate(&self, _path: &str, _page: u32, _content: &str) -> anyhow::Result<()> { Ok(()) }
}
