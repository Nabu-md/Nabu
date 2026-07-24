// Placeholder module for export functionality
pub struct ExportEngine;
impl ExportEngine {
    pub fn new(_path: std::path::PathBuf) -> Self { Self }
    pub fn export_to_html(&self, _note: &std::path::Path, _out: &std::path::Path, _template: &str) -> anyhow::Result<()> { Ok(()) }
}
